use std::io::{Read, Write};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use rand::Rng;

use crate::docker;
use crate::error::TermError;
use crate::keymap;
use crate::latex;
use crate::markdown;
use crate::output_transform;
use crate::prompt::PromptMatcher;
use crate::protocol::{
    CalloutValues, CellResult, Config, EchoMode, HighlightSpec, InputCell,
    OutputFormat, RecordedAssertion,
};
use crate::recorder::Recorder;
use crate::renderer::{self, RenderedLine};
use crate::terminal_line;
use crate::typst;

struct ResolvedLineOpts {
    literal: bool,
    enter: bool,
    hold: f64,
    expect_prompt: bool,
}

pub struct PtySession {
    writer: Box<dyn Write + Send>,
    rx: Receiver<Vec<u8>>,
    vt: avt::Vt,
    prompt: PromptMatcher,
    config: Config,
    recorders: Vec<Recorder>,
    output_since_cell_start: Vec<u8>,
    cursor_row_after_last_cell: usize,
    scrollback_after_last_cell: usize,
}

impl PtySession {
    pub fn new(config: &Config) -> Result<Self, TermError> {
        let prompt = PromptMatcher::new(config)?;

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: config.rows,
                cols: config.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| TermError::SpawnFailed(e.to_string()))?;

        let cmd = if let Some(ref docker) = config.docker {
            docker::check_docker_available()?;
            docker::maybe_pull_image(docker, config.verbose)?;
            // Without a shell, the image's CMD is re-run with the session env
            let image_cmd = if config.shell.is_none() {
                docker::image_entrypoint_and_cmd(&docker.image).map(|(_, cmd)| cmd)
            } else {
                None
            };
            docker::build_docker_command(docker, config, image_cmd.as_deref())
        } else {
            let mut cmd = CommandBuilder::new(config.local_shell());
            for arg in &config.shell_args {
                cmd.arg(arg);
            }
            cmd.env("TERM", "xterm-256color");
            cmd.env("COLORTERM", "truecolor");
            cmd.env("LC_ALL", "en_US.UTF-8");
            for (k, v) in &config.env {
                cmd.env(k, v);
            }
            cmd
        };

        let _child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| TermError::SpawnFailed(e.to_string()))?;

        drop(pair.slave);

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| TermError::SpawnFailed(e.to_string()))?;

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| TermError::SpawnFailed(e.to_string()))?;

        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let vt = avt::Vt::builder()
            .size(config.cols as usize, config.rows as usize)
            .scrollback_limit(10000)
            .build();

        let mut session = Self {
            writer,
            rx,
            vt,
            prompt,
            config: config.clone(),
            recorders: Vec::new(),
            output_since_cell_start: Vec::new(),
            cursor_row_after_last_cell: 0,
            scrollback_after_last_cell: 0,
        };

        session.wait_for_prompt()?;

        for init_cmd in &config.init {
            let cmd = format!("{}\r", init_cmd.trim_end());
            session.writer.write_all(cmd.as_bytes())
                .map_err(|_| TermError::ShellExited)?;
            session.writer.flush().map_err(|_| TermError::ShellExited)?;
            session.drain_pty();
            session.wait_for_prompt()?;
        }

        session.recorders = config
            .record
            .iter()
            .map(|path| Recorder::new(path, config.cols, config.rows))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| TermError::SpawnFailed(format!("recorder: {e}")))?;

        if !session.recorders.is_empty() {
            let prompt_str = format!("{} ", config.prompt);
            for rec in &mut session.recorders {
                rec.record_output(prompt_str.as_bytes());
            }
        }

        session.save_position();

        Ok(session)
    }

    pub fn finish(&mut self) {
        if !self.recorders.is_empty() {
            let _ = self.writer.write_all(b"exit\r");
            let _ = self.writer.flush();
            thread::sleep(Duration::from_millis(200));
            self.drain_pty();
            for rec in &mut self.recorders {
                rec.finish();
            }
        }
    }

    pub fn execute_cell(&mut self, cell: &InputCell) -> CellResult {
        self.output_since_cell_start.clear();

        let before_cursor_row = self.cursor_row_after_last_cell;
        let before_scrollback = self.scrollback_after_last_cell;

        let orig_timeout = self.config.timeout;
        let cell_timeout = cell.options.timeout.unwrap_or(orig_timeout);
        self.config.timeout = cell_timeout;

        let typing = cell.options.typing.as_ref().unwrap_or(&self.config.typing);
        let is_human = typing.is_enabled();
        let speed = typing.speed();
        let error_rate = typing.error_rate();

        let lines: Vec<&str> = cell.code.lines().collect();
        let mut error: Option<String> = None;
        let mut recorded_assertions: Vec<RecordedAssertion> = Vec::new();

        let cell_literal = cell.options.literal.unwrap_or(true);
        let cell_delay = cell.options.delay.unwrap_or(self.config.delay);
        let mut last_enter = true;

        for (idx, line_text) in lines.iter().enumerate() {
            let line_opts = cell
                .line_options
                .iter()
                .find(|lo| lo.line_index == idx as u32);

            let literal = line_opts
                .and_then(|lo| lo.literal)
                .unwrap_or(cell_literal);
            let delay = line_opts
                .and_then(|lo| lo.delay)
                .unwrap_or(cell_delay);
            let hold = line_opts
                .and_then(|lo| lo.hold)
                .unwrap_or(0.1);
            let cell_enter = cell.options.enter.unwrap_or(literal);
            let enter = line_opts
                .and_then(|lo| lo.enter)
                .unwrap_or(cell_enter);
            let cell_expect = cell.options.expect_prompt.unwrap_or(enter);
            let expect_prompt = line_opts
                .and_then(|lo| lo.expect_prompt)
                .unwrap_or(cell_expect);
            last_enter = enter;

            self.config.timeout = line_opts
                .and_then(|lo| lo.timeout)
                .unwrap_or(cell_timeout);

            let line_is_human = match line_opts.and_then(|lo| lo.typing) {
                Some(true) => true,
                Some(false) => false,
                None => is_human,
            };

            if self.config.verbose {
                let source = cell.source_lines.get(idx).map(|s| s.as_str()).unwrap_or(line_text);
                eprintln!("quarto-term:   > {}", source);
            }

            if delay > 0.0 {
                thread::sleep(Duration::from_secs_f64(delay));
            }

            let output_before = self.output_since_cell_start.len();

            let use_paste = self.config.strip_auto_indent && literal && idx > 0
                && !self.last_line_is_primary_prompt();

            if !literal && line_text.contains(' ') {
                let keys: Vec<&str> = line_text.split_whitespace().collect();
                for (ki, key) in keys.iter().enumerate() {
                    let is_last = ki == keys.len() - 1;
                    let key_enter = if is_last { enter } else { false };
                    let key_expect = if is_last { expect_prompt } else { false };
                    let key_hold = if is_last { hold } else { 0.0 };
                    let key_opts = ResolvedLineOpts {
                        literal: false,
                        enter: key_enter,
                        hold: key_hold,
                        expect_prompt: key_expect,
                    };
                    if ki > 0 && delay > 0.0 {
                        thread::sleep(Duration::from_secs_f64(delay));
                    }
                    if let Err(e) = self.send_line_resolved(key, &key_opts, false, speed, error_rate) {
                        error = Some(e.to_string());
                        break;
                    }
                }
                if error.is_some() {
                    break;
                }
            } else if use_paste {
                let resolved = ResolvedLineOpts {
                    literal,
                    enter,
                    hold,
                    expect_prompt,
                };
                let prefixed = format!("\x15{}", line_text);
                if let Err(e) = self.send_line_resolved(&prefixed, &resolved, line_is_human, speed, error_rate) {
                    error = Some(e.to_string());
                    break;
                }
            } else {
                let resolved = ResolvedLineOpts {
                    literal,
                    enter,
                    hold,
                    expect_prompt,
                };
                if let Err(e) = self.send_line_resolved(line_text, &resolved, line_is_human, speed, error_rate) {
                    error = Some(e.to_string());
                    break;
                }
            }

            if let Some(assert_spec) = line_opts.and_then(|lo| lo.assert.as_ref()) {
                let line_output = String::from_utf8_lossy(
                    &self.output_since_cell_start[output_before..],
                ).to_string();
                recorded_assertions.push(RecordedAssertion {
                    line_index: idx as u32,
                    output: line_output.clone(),
                });
                if let Some(msg) = Self::check_assertion_patterns(&line_output, assert_spec.patterns()) {
                    error = Some(format!("line {}: {}", idx + 1, msg));
                    break;
                }
            }
        }

        if error.is_none() && last_enter && self.prompt.ps2_re.is_some() && !self.last_line_is_primary_prompt() {
            if let Err(e) = self.recover_primary_prompt() {
                error = Some(e.to_string());
            }
        }

        let cell_hold = cell.options.hold.unwrap_or(self.config.hold);
        if cell_hold > 0.0 {
            self.drain_during(Duration::from_secs_f64(cell_hold));
        }

        self.config.timeout = orig_timeout;

        let use_ansi = cell.options.ansi.unwrap_or(self.config.ansi);
        let markup = self.build_cell_output(cell, before_cursor_row, before_scrollback, use_ansi);

        if error.is_none() {
            if let Some(ref assert_spec) = cell.options.assert {
                let cell_output = String::from_utf8_lossy(&self.output_since_cell_start).to_string();
                error = Self::check_assertion_patterns(&cell_output, assert_spec.patterns());
            }
        }

        let scroll = cell.options.scroll.unwrap_or(!cell.options.fullscreen);
        if scroll {
            self.save_position();
        }

        CellResult {
            id: cell.id,
            markup,
            error,
            recorded_assertions,
        }
    }

    fn send_line_resolved(
        &mut self,
        text: &str,
        opts: &ResolvedLineOpts,
        human: bool,
        speed: f64,
        error_rate: f64,
    ) -> Result<(), TermError> {
        if human && opts.literal {
            self.type_human(text, speed, error_rate)?;
            if opts.enter {
                let cr = [b'\r'];
                self.writer.write_all(&cr).map_err(|_| TermError::ShellExited)?;
                self.writer.flush().map_err(|_| TermError::ShellExited)?;
            }
        } else {
            let bytes = if opts.literal {
                let mut b = text.as_bytes().to_vec();
                if opts.enter {
                    b.push(b'\r');
                }
                b
            } else {
                let key = keymap::translate(text);
                let mut b = key.bytes;
                if opts.enter && !key.is_named {
                    b.push(b'\r');
                }
                b
            };

            self.writer
                .write_all(&bytes)
                .map_err(|_| TermError::ShellExited)?;
            self.writer.flush().map_err(|_| TermError::ShellExited)?;
        }

        if opts.hold > 0.0 {
            self.drain_during(Duration::from_secs_f64(opts.hold));
        } else {
            self.drain_pty();
        }

        if opts.expect_prompt {
            if !self.last_line_matches_prompt() {
                self.wait_for_prompt()?;
            }
        }

        Ok(())
    }

    fn recover_primary_prompt(&mut self) -> Result<(), TermError> {
        for attempt in 0..3 {
            if attempt < 2 {
                self.writer.write_all(b"\r").ok();
            } else {
                self.writer.write_all(b"\x03").ok();
            }
            self.writer.flush().ok();
            thread::sleep(Duration::from_millis(50));
            self.drain_pty();
            if self.last_line_is_primary_prompt() {
                return Ok(());
            }
        }
        if !self.last_line_is_primary_prompt() {
            self.wait_for_prompt()?;
        }
        Ok(())
    }

    fn check_assertion_patterns(output: &str, patterns: &[String]) -> Option<String> {
        for pattern in patterns {
            let matched = regex::Regex::new(pattern)
                .map(|re| re.is_match(output))
                .unwrap_or_else(|_| output.contains(pattern.as_str()));
            if !matched {
                return Some(format!(
                    "assertion failed: pattern {:?} not found in output",
                    pattern
                ));
            }
        }
        None
    }

    fn wait_for_prompt(&mut self) -> Result<(), TermError> {
        let deadline = Instant::now() + Duration::from_secs_f64(self.config.timeout);

        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                let last = String::from_utf8_lossy(
                    &self.output_since_cell_start[self.output_since_cell_start.len().saturating_sub(500)..],
                )
                .to_string();
                return Err(TermError::PromptTimeout {
                    elapsed_secs: self.config.timeout,
                    last_output: last,
                });
            }

            match self.rx.recv_timeout(remaining.min(Duration::from_millis(50))) {
                Ok(bytes) => {
                    self.process_bytes(&bytes);

                    if self.last_line_matches_prompt() {
                        thread::sleep(Duration::from_millis(10));
                        self.drain_pty();
                        if self.last_line_matches_prompt() {
                            return Ok(());
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    continue;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(TermError::ShellExited);
                }
            }
        }
    }

    fn type_human(&mut self, text: &str, speed: f64, error_rate: f64) -> Result<(), TermError> {
        let mut rng = rand::thread_rng();
        let base_ms = 12_000.0 / speed.max(1.0);
        let chars: Vec<char> = text.chars().collect();

        let mut i = 0;
        while i < chars.len() {
            let ch = chars[i];

            // Decide whether to make a typo
            if error_rate > 0.0 && rng.gen::<f64>() < error_rate {
                let wrong = crate::typing::adjacent_key(ch, &mut rng);
                self.emit_char(wrong)?;
                let pause = crate::typing::lognormal_ms(400.0, 0.4, &mut rng);
                thread::sleep(Duration::from_millis(pause));
                self.drain_pty();
                self.emit_byte(0x7f)?;
                let pause = crate::typing::lognormal_ms(80.0, 0.3, &mut rng);
                thread::sleep(Duration::from_millis(pause));
                self.drain_pty();
            }

            let factor = crate::typing::bigram_factor(if i > 0 { Some(chars[i - 1]) } else { None }, ch);
            let delay = crate::typing::lognormal_ms(base_ms * factor, 0.4, &mut rng);
            thread::sleep(Duration::from_millis(delay));

            self.emit_char(ch)?;
            self.drain_pty();
            i += 1;
        }
        Ok(())
    }

    fn emit_char(&mut self, ch: char) -> Result<(), TermError> {
        let mut buf = [0u8; 4];
        let bytes = ch.encode_utf8(&mut buf).as_bytes();
        self.writer.write_all(bytes).map_err(|_| TermError::ShellExited)?;
        self.writer.flush().map_err(|_| TermError::ShellExited)?;
        Ok(())
    }

    fn emit_byte(&mut self, b: u8) -> Result<(), TermError> {
        let bytes = [b];
        self.writer.write_all(&bytes).map_err(|_| TermError::ShellExited)?;
        self.writer.flush().map_err(|_| TermError::ShellExited)?;
        Ok(())
    }

    fn process_bytes(&mut self, bytes: &[u8]) {
        for rec in &mut self.recorders {
            rec.record_output(bytes);
        }
        self.output_since_cell_start.extend_from_slice(bytes);
        let text = String::from_utf8_lossy(bytes);
        self.vt.feed_str(&text);
        self.respond_to_dsr(bytes);
    }

    fn respond_to_dsr(&mut self, bytes: &[u8]) {
        let mut start = 0;
        while start < bytes.len() {
            if bytes[start..].starts_with(b"\x1b[6n") {
                // Device Status Report: respond with cursor position
                let row = self.vt.cursor().row + 1;
                let col = self.vt.cursor().col + 1;
                let response = format!("\x1b[{};{}R", row, col);
                let _ = self.writer.write_all(response.as_bytes());
                let _ = self.writer.flush();
                start += 4;
            } else if bytes[start..].starts_with(b"\x1b[c") || bytes[start..].starts_with(b"\x1b[0c") {
                // Device Attributes: respond as VT220
                let _ = self.writer.write_all(b"\x1b[?62;22c");
                let _ = self.writer.flush();
                start += if bytes[start..].starts_with(b"\x1b[0c") { 4 } else { 3 };
            } else if bytes[start..].starts_with(b"\x1b]11;?\x07") {
                // Background color query (OSC 11): respond with dark background
                let _ = self.writer.write_all(b"\x1b]11;rgb:0000/0000/0000\x1b\\");
                let _ = self.writer.flush();
                start += 6;
            } else if bytes[start..].starts_with(b"\x1b]10;?\x07") {
                // Foreground color query (OSC 10): respond with light foreground
                let _ = self.writer.write_all(b"\x1b]10;rgb:ffff/ffff/ffff\x1b\\");
                let _ = self.writer.flush();
                start += 6;
            } else {
                start += 1;
            }
        }
    }

    fn drain_during(&mut self, duration: Duration) {
        let deadline = Instant::now() + duration;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            match self.rx.recv_timeout(remaining.min(Duration::from_millis(50))) {
                Ok(bytes) => {
                    self.process_bytes(&bytes);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    }

    fn drain_pty(&mut self) {
        loop {
            match self.rx.try_recv() {
                Ok(bytes) => {
                    self.process_bytes(&bytes);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
    }

    fn last_line_is_primary_prompt(&self) -> bool {
        let view_lines: Vec<_> = self.vt.view().collect();
        for line in view_lines.iter().rev() {
            let text = line_text(line);
            if !text.is_empty() {
                return self.prompt.is_primary_prompt(&text);
            }
        }
        false
    }

    fn last_line_matches_prompt(&self) -> bool {
        let cursor_col = self.vt.cursor().col;
        let view_lines: Vec<_> = self.vt.view().collect();
        if self.prompt.ps2_re.is_some() && cursor_col > 0 {
            let cursor_row = self.vt.cursor().row;
            if let Some(cursor_line) = view_lines.get(cursor_row) {
                let cursor_text = line_text(cursor_line);
                if self.prompt.matches_ps2(&cursor_text) {
                    return true;
                }
            }
        }
        for line in view_lines.iter().rev() {
            let text = line_text(line);
            if !text.is_empty() {
                return self.prompt.matches_any_prompt(&text);
            }
        }
        false
    }

    fn is_only_prompt(&self, text: &str) -> bool {
        self.prompt.is_only_prompt(text)
    }

    fn mark_prompts(&self, lines: &mut [RenderedLine]) {
        self.prompt.mark_prompts(lines);
    }

    fn save_position(&mut self) {
        let scrollback_count = self.vt.lines().count().saturating_sub(self.config.rows as usize);
        self.scrollback_after_last_cell = scrollback_count;
        self.cursor_row_after_last_cell = self.vt.cursor().row;
    }

    fn scrollback_count(&self) -> usize {
        self.vt.lines().count().saturating_sub(self.config.rows as usize)
    }

    fn build_cell_output(
        &self,
        cell: &InputCell,
        before_cursor_row: usize,
        before_scrollback: usize,
        ansi: bool,
    ) -> String {
        let trailing_spaces = cell.options.trailing_spaces.unwrap_or(self.config.trailing_spaces);
        let mut out = String::new();
        let format = self.config.format;
        let font_size = cell.options.font_size.as_deref().or(self.config.font_size.as_deref());
        let font_family = cell.options.font_family.as_deref().or(self.config.font_family.as_deref());
        let line_height = cell.options.line_height.as_deref().or(self.config.line_height.as_deref());
        let html_style = renderer::HtmlStyle { font_size, font_family, line_height };

        let echo_mode = cell.options.echo.as_ref().unwrap_or(&self.config.echo);

        match echo_mode {
            EchoMode::Bool(false) => {}
            EchoMode::Mode(m) if m == "false" => {}
            EchoMode::Mode(m) if m == "source" => {
                match format {
                    OutputFormat::Latex => {
                        out.push_str(&format!(
                            "\\begin{{verbatim}}\n{}\n\\end{{verbatim}}\n",
                            &cell.code
                        ));
                    }
                    OutputFormat::Markdown => {
                        out.push_str(&format!("```bash\n{}\n```\n", &cell.code));
                    }
                    _ => {
                        let highlight = cell.options.highlight.as_ref().unwrap_or(&self.config.highlight);
                        let lang = match highlight {
                            HighlightSpec::Language(l) if renderer::is_safe_language_name(l) => l.as_str(),
                            HighlightSpec::Language(_) => "text",
                            HighlightSpec::Bool(false) => "text",
                            HighlightSpec::Bool(true) => "bash",
                        };
                        let style_attr = html_style.to_attr();
                        out.push_str(&format!(
                            "<pre class=\"term-source\"{style_attr}><code class=\"language-{lang}\">{}</code></pre>\n",
                            html_escape_basic(&cell.code)
                        ));
                    }
                }
            }
            _ => {
                // "terminal" mode (default): output IS the echo
            }
        }

        let is_terminal_mode = matches!(echo_mode, EchoMode::Mode(ref m) if m == "terminal")
            || matches!(echo_mode, EchoMode::Bool(true));
        let show_output = cell.options.output || is_terminal_mode;

        if show_output {
            let mut lines = if cell.options.fullscreen {
                self.capture_fullscreen(ansi, format, trailing_spaces)
            } else {
                self.capture_new_lines(before_cursor_row, before_scrollback, ansi, format, trailing_spaces)
            };

            let keep_last_prompt = cell.options.keep_last_prompt.unwrap_or(self.config.keep_last_prompt);
            if !keep_last_prompt {
                while let Some(last) = lines.last() {
                    if last.text.is_empty() || self.is_only_prompt(&last.text) {
                        lines.pop();
                    } else {
                        break;
                    }
                }
            }

            let use_spacing = cell.options.spacing.unwrap_or(self.config.spacing);
            if use_spacing && !cell.options.fullscreen {
                output_transform::apply_spacing(&mut lines, &self.config.prompt);
            }

            output_transform::apply_remove(&mut lines, &cell.options.remove);
            output_transform::apply_truncate(&mut lines, &cell.options.truncate, format);
            let callout_values = cell.options.callout_values.clone()
                .unwrap_or(CalloutValues::Str("123456789".into()));
            let callout_format = cell.options.callout_format.as_deref()
                .unwrap_or("<{}>");
            output_transform::apply_callouts(
                &mut lines, &cell.options.callout, &callout_values, callout_format,
            );

            let colorscheme_bg = cell.options.colorscheme_bg.as_deref().or(self.config.colorscheme_bg.as_deref());
            let colorscheme_fg = cell.options.colorscheme_fg.as_deref().or(self.config.colorscheme_fg.as_deref());

            match format {
                OutputFormat::Latex => {
                    let theme = latex::LatexTheme {
                        bg: colorscheme_bg,
                        fg: colorscheme_fg,
                        font_size,
                        font_family,
                        line_height,
                    };
                    if cell.options.fullscreen {
                        out.push_str(&latex::render_fullscreen_to_latex(&lines, &theme));
                    } else {
                        out.push_str(&latex::render_lines_to_latex(&lines, &theme));
                    }
                }
                OutputFormat::Typst => {
                    let theme = typst::TypstTheme {
                        bg: colorscheme_bg,
                        fg: colorscheme_fg,
                        font_size,
                        font_family,
                        line_height,
                    };
                    if cell.options.fullscreen {
                        out.push_str(&typst::render_fullscreen_to_typst(&lines, &theme));
                    } else {
                        out.push_str(&typst::render_lines_to_typst(&lines, &theme));
                    }
                }
                OutputFormat::Markdown => {
                    if cell.options.fullscreen {
                        out.push_str(&markdown::render_fullscreen_to_markdown(&lines));
                    } else {
                        out.push_str(&markdown::render_lines_to_markdown(&lines));
                    }
                }
                OutputFormat::Html => {
                    if cell.options.fullscreen {
                        out.push_str(&renderer::render_fullscreen_to_html(&lines, &html_style));
                    } else {
                        self.mark_prompts(&mut lines);
                        let hide_echo = matches!(echo_mode, EchoMode::Bool(false))
                            || matches!(echo_mode, EchoMode::Mode(ref m) if m == "false");
                        let hide_output = !cell.options.output;
                        if hide_echo || hide_output {
                            lines.retain(|line| {
                                if hide_echo && line.is_prompt { return false; }
                                if hide_output && !line.is_prompt { return false; }
                                true
                            });
                        }
                        out.push_str(&renderer::render_lines_to_html(&lines, "term-output", &html_style));
                    }
                }
            }
        }

        out
    }

    fn capture_new_lines(
        &self,
        before_cursor_row: usize,
        before_scrollback: usize,
        ansi: bool,
        format: OutputFormat,
        trailing_spaces: bool,
    ) -> Vec<RenderedLine> {
        let current_scrollback = self.scrollback_count();
        let current_cursor_row = self.vt.cursor().row;

        let all_lines: Vec<_> = self.vt.lines().collect();

        let start_line = before_scrollback + before_cursor_row;
        let end_line = current_scrollback + current_cursor_row;

        // Include the current cursor line if it has content
        let end_line = if end_line < all_lines.len() {
            let text = line_text(&all_lines[end_line]);
            if !text.is_empty() { end_line + 1 } else { end_line }
        } else {
            end_line
        };

        if end_line <= start_line || start_line >= all_lines.len() {
            return Vec::new();
        }

        let end = end_line.min(all_lines.len());

        all_lines[start_line..end]
            .iter()
            .map(|line| render_line_for_format(line, ansi, trailing_spaces, format))
            .collect()
    }

    fn capture_fullscreen(&self, ansi: bool, format: OutputFormat, trailing_spaces: bool) -> Vec<RenderedLine> {
        self.vt
            .view()
            .map(|line| render_line_for_format(line, ansi, trailing_spaces, format))
            .collect()
    }
}


fn render_line_for_format(line: &avt::Line, ansi: bool, trailing_spaces: bool, format: OutputFormat) -> RenderedLine {
    match format {
        OutputFormat::Latex => latex::render_line(line, ansi, trailing_spaces),
        OutputFormat::Typst => typst::render_line(line, ansi, trailing_spaces),
        _ => renderer::render_line(line, ansi, trailing_spaces),
    }
}


fn line_text(line: &avt::Line) -> String {
    terminal_line::line_to_text(line)
}

fn html_escape_basic(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}


#[cfg(test)]
mod tests {
    use super::*;

    // --- html_escape_basic ---

    #[test]
    fn html_escape_basic_works() {
        assert_eq!(html_escape_basic("a&b"), "a&amp;b");
        assert_eq!(html_escape_basic("<>"), "&lt;&gt;");
        assert_eq!(html_escape_basic("hello"), "hello");
    }

}
