use std::io::{Read, Write};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use rand::Rng;
use regex::Regex;

use crate::error::TermError;
use crate::keymap;
use crate::latex;
use crate::markdown;
use crate::protocol::{
    CellResult, Config, DockerConfig, EchoMode, HighlightSpec, InputCell, LineSpec,
    RecordedAssertion,
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
    prompt_re: Regex,
    prompt_prefix_re: Regex,
    ps2_re: Option<Regex>,
    ps2_prefix_re: Option<Regex>,
    config: Config,
    recorders: Vec<Recorder>,
    output_since_cell_start: Vec<u8>,
    cursor_row_after_last_cell: usize,
    scrollback_after_last_cell: usize,
}

impl PtySession {
    pub fn new(config: &Config) -> Result<Self, TermError> {
        let prompt_pattern = match &config.prompt_regex {
            Some(re) => re.clone(),
            None => format!("{}\\s*$", regex::escape(&config.prompt)),
        };
        let prompt_re = Regex::new(&prompt_pattern)
            .map_err(|e| TermError::RegexCompile(e.to_string()))?;
        let prompt_prefix_pattern = match &config.prompt_regex {
            Some(re) => format!("^(?:{})", re.trim_end_matches('$')),
            None => format!("^{}\\s?", regex::escape(&config.prompt)),
        };
        let prompt_prefix_re = Regex::new(&prompt_prefix_pattern).unwrap_or_else(|_| {
            Regex::new(&format!("^{}\\s?", regex::escape(&config.prompt))).unwrap()
        });
        let ps2_re = if let Some(ref re) = config.ps2_regex {
            Some(Regex::new(re).map_err(|e| TermError::RegexCompile(e.to_string()))?)
        } else {
            config
                .ps2
                .as_ref()
                .map(|s| {
                    let escaped = regex::escape(s.trim_end());
                    Regex::new(&format!("^{}\\s*$", escaped))
                })
                .transpose()
                .map_err(|e| TermError::RegexCompile(e.to_string()))?
        };
        let ps2_prefix_re = if let Some(ref re) = config.ps2_regex {
            Regex::new(&format!("^(?:{})", re.trim_end_matches('$'))).ok()
        } else {
            config
                .ps2
                .as_ref()
                .and_then(|s| {
                    let escaped = regex::escape(s.trim_end());
                    Regex::new(&format!("^{}\\s?", escaped)).ok()
                })
        };

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
            check_docker_available()?;
            maybe_pull_image(docker, config.verbose)?;
            build_docker_command(docker, config)
        } else {
            let mut cmd = CommandBuilder::new(&config.shell);
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

        let recorders: Vec<Recorder> = config
            .record
            .iter()
            .filter_map(|path| Recorder::new(path, config.cols, config.rows).ok())
            .collect();

        let mut session = Self {
            writer,
            rx,
            vt,
            prompt_re,
            prompt_prefix_re,
            ps2_re,
            ps2_prefix_re,
            config: config.clone(),
            recorders,
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

            let use_paste = self.config.disable_auto_indent && literal && idx > 0
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
                for pattern in assert_spec.patterns() {
                    let matched = regex::Regex::new(pattern)
                        .map(|re| re.is_match(&line_output))
                        .unwrap_or_else(|_| line_output.contains(pattern.as_str()));
                    if !matched {
                        error = Some(format!(
                            "assertion failed on line {}: pattern {:?} not found in output",
                            idx + 1, pattern
                        ));
                        break;
                    }
                }
                if error.is_some() {
                    break;
                }
            }
        }

        if error.is_none() && self.ps2_re.is_some() && !self.last_line_is_primary_prompt() {
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
                    break;
                }
            }
            if !self.last_line_is_primary_prompt() {
                if let Err(e) = self.wait_for_prompt() {
                    error = Some(e.to_string());
                }
            }
        }

        let cell_hold = cell.options.hold.unwrap_or(self.config.hold);
        if cell_hold > 0.0 {
            self.drain_during(Duration::from_secs_f64(cell_hold));
        }

        self.config.timeout = orig_timeout;

        let use_ansi = cell.options.ansi.unwrap_or(self.config.ansi);
        let html = self.build_cell_html(cell, before_cursor_row, before_scrollback, use_ansi);

        if error.is_none() {
            if let Some(ref assert_spec) = cell.options.assert {
                let cell_output = String::from_utf8_lossy(&self.output_since_cell_start).to_string();
                for pattern in assert_spec.patterns() {
                    let matched = regex::Regex::new(pattern)
                        .map(|re| re.is_match(&cell_output))
                        .unwrap_or_else(|_| cell_output.contains(pattern.as_str()));
                    if !matched {
                        error = Some(format!(
                            "assertion failed: pattern {:?} not found in cell output",
                            pattern
                        ));
                        break;
                    }
                }
            }
        }

        let scroll = cell.options.scroll.unwrap_or(!cell.options.fullscreen);
        if scroll {
            self.save_position();
        }

        CellResult {
            id: cell.id,
            html,
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
                let wrong = adjacent_key(ch, &mut rng);
                self.emit_char(wrong)?;
                // Recognition delay before correction
                let pause = lognormal_ms(400.0, 0.4, &mut rng);
                thread::sleep(Duration::from_millis(pause));
                self.drain_pty();
                // Backspace
                self.emit_byte(0x7f)?;
                let pause = lognormal_ms(80.0, 0.3, &mut rng);
                thread::sleep(Duration::from_millis(pause));
                self.drain_pty();
            }

            // Compute delay based on context
            let factor = bigram_factor(if i > 0 { Some(chars[i - 1]) } else { None }, ch);
            let delay = lognormal_ms(base_ms * factor, 0.4, &mut rng);
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
                return self.prompt_re.is_match(&text);
            }
        }
        false
    }

    fn last_line_matches_prompt(&self) -> bool {
        let cursor_col = self.vt.cursor().col;
        let view_lines: Vec<_> = self.vt.view().collect();
        if let Some(ref ps2) = self.ps2_re {
            if cursor_col > 0 {
                let cursor_row = self.vt.cursor().row;
                if let Some(cursor_line) = view_lines.get(cursor_row) {
                    let cursor_text = line_text(cursor_line);
                    if ps2.is_match(&cursor_text) {
                        return true;
                    }
                }
            }
        }
        for line in view_lines.iter().rev() {
            let text = line_text(line);
            if !text.is_empty() {
                if self.prompt_re.is_match(&text) {
                    return true;
                }
                if let Some(ref ps2) = self.ps2_re {
                    return ps2.is_match(&text);
                }
                return false;
            }
        }
        false
    }

    fn is_only_prompt(&self, text: &str) -> bool {
        if let Some(m) = self.prompt_re.find(text) {
            if m.end() >= text.len() {
                return true;
            }
        }
        false
    }

    fn mark_prompts(&self, lines: &mut [RenderedLine]) {
        for line in lines.iter_mut() {
            let prompt_len = if let Some(m) = self.prompt_prefix_re.find(&line.text) {
                m.end()
            } else if let Some(ref ps2) = self.ps2_prefix_re {
                if let Some(m) = ps2.find(&line.text) {
                    m.end()
                } else {
                    0
                }
            } else {
                0
            };

            if prompt_len > 0 {
                let char_len = line.text[..prompt_len].chars().count();
                line.html = renderer::wrap_prompt_span(&line.html, char_len);
            }
        }
    }

    fn save_position(&mut self) {
        let scrollback_count = self.vt.lines().count().saturating_sub(self.config.rows as usize);
        self.scrollback_after_last_cell = scrollback_count;
        self.cursor_row_after_last_cell = self.vt.cursor().row;
    }

    fn scrollback_count(&self) -> usize {
        self.vt.lines().count().saturating_sub(self.config.rows as usize)
    }

    fn build_cell_html(
        &self,
        cell: &InputCell,
        before_cursor_row: usize,
        before_scrollback: usize,
        ansi: bool,
    ) -> String {
        let trailing_spaces = cell.options.trailing_spaces.unwrap_or(self.config.trailing_spaces);
        let mut out = String::new();
        let format = self.config.format.as_str();
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
                    "latex" => {
                        out.push_str(&format!(
                            "\\begin{{verbatim}}\n{}\n\\end{{verbatim}}\n",
                            &cell.code
                        ));
                    }
                    "markdown" => {
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

        let show_output = cell.options.output
            || matches!(echo_mode, EchoMode::Mode(m) if m == "terminal")
            || matches!(echo_mode, EchoMode::Bool(true));

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
                apply_spacing(&mut lines, &self.config.prompt);
            }

            apply_remove(&mut lines, &cell.options.remove);
            apply_truncate(&mut lines, &cell.options.truncate, format);
            apply_callouts(&mut lines, &cell.options.callouts);

            let theme_bg = cell.options.theme_bg.as_deref().or(self.config.theme_bg.as_deref());
            let theme_fg = cell.options.theme_fg.as_deref().or(self.config.theme_fg.as_deref());

            match format {
                "latex" => {
                    let theme = latex::LatexTheme {
                        bg: theme_bg,
                        fg: theme_fg,
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
                "typst" => {
                    let theme = typst::TypstTheme {
                        bg: theme_bg,
                        fg: theme_fg,
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
                "markdown" => {
                    if cell.options.fullscreen {
                        out.push_str(&markdown::render_fullscreen_to_markdown(&lines));
                    } else {
                        out.push_str(&markdown::render_lines_to_markdown(&lines));
                    }
                }
                _ => {
                    self.mark_prompts(&mut lines);
                    if cell.options.fullscreen {
                        out.push_str(&renderer::render_fullscreen_to_html(&lines, &html_style));
                    } else {
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
        format: &str,
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

    fn capture_fullscreen(&self, ansi: bool, format: &str, trailing_spaces: bool) -> Vec<RenderedLine> {
        self.vt
            .view()
            .map(|line| render_line_for_format(line, ansi, trailing_spaces, format))
            .collect()
    }
}

fn check_docker_available() -> Result<(), TermError> {
    let out = std::process::Command::new("docker")
        .arg("info")
        .arg("--format")
        .arg("{{.ServerVersion}}")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    match out {
        Ok(s) if s.success() => Ok(()),
        Ok(_) => Err(TermError::SpawnFailed(
            "docker daemon is not running or not accessible".to_string(),
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(TermError::SpawnFailed(
            "docker not found in PATH".to_string(),
        )),
        Err(e) => Err(TermError::SpawnFailed(format!("docker check failed: {e}"))),
    }
}

fn maybe_pull_image(docker: &DockerConfig, verbose: bool) -> Result<(), TermError> {
    match docker.pull.as_str() {
        "never" => return Ok(()),
        "always" => {}
        _ => {
            let mut check = std::process::Command::new("docker");
            check.args(["image", "inspect", &docker.image]);
            check.stdout(std::process::Stdio::null());
            check.stderr(std::process::Stdio::null());
            let status = check
                .status()
                .map_err(|e| TermError::SpawnFailed(format!("docker image inspect failed: {e}")))?;
            if status.success() {
                return Ok(());
            }
        }
    }
    if verbose {
        eprintln!("quarto-term: pulling image {}", docker.image);
    }
    let mut pull = std::process::Command::new("docker");
    pull.arg("pull");
    if let Some(ref platform) = docker.platform {
        pull.arg("--platform");
        pull.arg(platform);
    }
    pull.arg(&docker.image);
    let status = pull
        .status()
        .map_err(|e| TermError::SpawnFailed(format!("docker pull failed: {e}")))?;
    if !status.success() {
        return Err(TermError::SpawnFailed(format!(
            "failed to pull image '{}': docker pull exited with {}",
            docker.image, status
        )));
    }
    Ok(())
}

fn build_docker_command(docker: &DockerConfig, config: &Config) -> CommandBuilder {
    let mut cmd = CommandBuilder::new("docker");
    cmd.arg("run");
    cmd.arg("--rm");
    cmd.arg("-i");
    cmd.arg("-t");

    if let Some(ref platform) = docker.platform {
        cmd.arg("--platform");
        cmd.arg(platform);
    }
    if let Some(ref name) = docker.name {
        cmd.arg("--name");
        cmd.arg(name);
    }
    if let Some(ref workdir) = docker.workdir {
        cmd.arg("--workdir");
        cmd.arg(workdir);
    }
    if let Some(ref user) = docker.user {
        cmd.arg("--user");
        cmd.arg(user);
    }
    if let Some(ref network) = docker.network {
        cmd.arg("--network");
        cmd.arg(network);
    }
    if let Some(ref memory) = docker.memory {
        cmd.arg("--memory");
        cmd.arg(memory);
    }
    if let Some(ref cpus) = docker.cpus {
        cmd.arg("--cpus");
        cmd.arg(cpus);
    }
    for port in &docker.ports {
        cmd.arg("-p");
        cmd.arg(port);
    }
    for vol in &docker.volumes {
        cmd.arg("-v");
        cmd.arg(vol);
    }
    for (k, v) in &docker.env {
        cmd.arg("--env");
        cmd.arg(format!("{k}={v}"));
    }
    for (k, v) in &config.env {
        cmd.arg("--env");
        cmd.arg(format!("{k}={v}"));
    }
    cmd.arg("--env");
    cmd.arg("TERM=xterm-256color");
    cmd.arg("--env");
    cmd.arg("COLORTERM=truecolor");
    cmd.arg("--env");
    cmd.arg("LC_ALL=en_US.UTF-8");

    for arg in &docker.args {
        cmd.arg(arg);
    }

    cmd.arg(&docker.image);
    cmd.arg(&config.shell);
    for arg in &config.shell_args {
        cmd.arg(arg);
    }

    cmd
}

fn render_line_for_format(line: &avt::Line, ansi: bool, trailing_spaces: bool, format: &str) -> RenderedLine {
    match format {
        "latex" => latex::render_line(line, ansi, trailing_spaces),
        "typst" => typst::render_line(line, ansi, trailing_spaces),
        _ => renderer::render_line(line, ansi, trailing_spaces),
    }
}

fn apply_spacing(lines: &mut Vec<RenderedLine>, prompt: &str) {
    let mut insertions = Vec::new();
    let prefix = format!("{} ", prompt);
    for (i, line) in lines.iter().enumerate() {
        if i > 0 && line.text.starts_with(&prefix) {
            insertions.push(i);
        }
    }
    for (offset, idx) in insertions.into_iter().enumerate() {
        lines.insert(idx + offset, RenderedLine {
            html: String::new(),
            text: String::new(),
        });
    }
}

fn resolve_single_index(i: i32, len: usize) -> Option<usize> {
    if i > 0 {
        let idx = (i as usize).saturating_sub(1);
        if idx < len { Some(idx) } else { None }
    } else if i < 0 {
        let idx = len as i32 + i;
        if idx >= 0 && (idx as usize) < len { Some(idx as usize) } else { None }
    } else {
        None
    }
}

fn parse_range_spec(s: &str, len: usize) -> Vec<usize> {
    let parts: Vec<&str> = s.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Vec::new();
    }

    let start_str = parts[0].trim();
    let end_str = parts[1].trim();

    let start = if start_str.is_empty() {
        0i32
    } else if let Ok(n) = start_str.parse::<i32>() {
        n
    } else {
        return Vec::new();
    };

    let end = if end_str.is_empty() {
        len as i32
    } else if let Ok(n) = end_str.parse::<i32>() {
        n
    } else {
        return Vec::new();
    };

    let resolve = |v: i32| -> usize {
        if v > 0 {
            (v as usize).saturating_sub(1)
        } else if v < 0 {
            let r = len as i32 + v;
            if r >= 0 { r as usize } else { 0 }
        } else {
            0
        }
    };

    let start_idx = resolve(start);
    let end_idx = if end_str.is_empty() {
        len
    } else {
        resolve(end) + 1
    };

    let start_idx = start_idx.min(len);
    let end_idx = end_idx.min(len);

    if start_idx >= end_idx {
        return Vec::new();
    }

    (start_idx..end_idx).collect()
}

fn is_range_syntax(s: &str) -> bool {
    s.contains(':') && s.bytes().all(|b| b.is_ascii_digit() || b == b':' || b == b'-' || b == b' ')
}

fn resolve_line_specs(specs: &[LineSpec], lines: &[RenderedLine]) -> Vec<usize> {
    let len = lines.len();
    let mut indices = Vec::new();

    for spec in specs {
        match spec {
            LineSpec::Index(i) => {
                if let Some(idx) = resolve_single_index(*i, len) {
                    indices.push(idx);
                }
            }
            LineSpec::Expr(s) => {
                if let Ok(n) = s.parse::<i32>() {
                    if let Some(idx) = resolve_single_index(n, len) {
                        indices.push(idx);
                    }
                } else if is_range_syntax(s) {
                    indices.extend(parse_range_spec(s, len));
                } else if let Ok(re) = Regex::new(s) {
                    for (idx, line) in lines.iter().enumerate() {
                        if re.is_match(&line.text) {
                            indices.push(idx);
                        }
                    }
                }
            }
        }
    }

    indices.sort_unstable();
    indices.dedup();
    indices
}

fn apply_remove(lines: &mut Vec<RenderedLine>, specs: &[LineSpec]) {
    if specs.is_empty() {
        return;
    }
    let to_remove = resolve_line_specs(specs, lines);
    for idx in to_remove.into_iter().rev() {
        lines.remove(idx);
    }
}

fn apply_truncate(lines: &mut Vec<RenderedLine>, specs: &[LineSpec], format: &str) {
    if specs.is_empty() {
        return;
    }
    let to_truncate = resolve_line_specs(specs, lines);
    if to_truncate.is_empty() {
        return;
    }

    // Group consecutive indices into runs
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut run_start = to_truncate[0];
    let mut run_end = to_truncate[0];

    for &idx in &to_truncate[1..] {
        if idx == run_end + 1 {
            run_end = idx;
        } else {
            runs.push((run_start, run_end));
            run_start = idx;
            run_end = idx;
        }
    }
    runs.push((run_start, run_end));

    // Process runs from end to start to preserve indices
    for &(start, end) in runs.iter().rev() {
        let count = end - start + 1;
        let msg = format_truncation_message(count, format);
        // Remove lines in this run (from end to start+1), keep start for the message
        for idx in (start + 1..=end).rev() {
            lines.remove(idx);
        }
        // Replace the first line of the run with the message
        lines[start] = msg;
    }
}

fn format_truncation_message(count: usize, format: &str) -> RenderedLine {
    let word = if count == 1 { "line" } else { "lines" };
    let text = format!("[{} {} truncated]", count, word);
    let html = match format {
        "latex" => format!("\\textit{{{}}}", text),
        "typst" => format!("_{}_", text),
        "markdown" => format!("*{}*", text),
        _ => format!("<span class=\"term-truncated\">{}</span>", text),
    };
    RenderedLine { html, text }
}

fn apply_callouts(lines: &mut Vec<RenderedLine>, specs: &[LineSpec]) {
    if specs.is_empty() {
        return;
    }

    for (n, spec) in specs.iter().enumerate() {
        let annotation_num = n + 1;
        match spec {
            LineSpec::Index(i) => {
                if let Some(idx) = resolve_single_index(*i, lines.len()) {
                    if let Some(line) = lines.get_mut(idx) {
                        line.html = format!(
                            "{} <span class=\"term-callout\">&lt;{}&gt;</span>",
                            line.html, annotation_num
                        );
                    }
                }
            }
            LineSpec::Expr(s) => {
                if is_range_syntax(s) {
                    let indices = parse_range_spec(s, lines.len());
                    if let Some(&idx) = indices.first() {
                        if let Some(line) = lines.get_mut(idx) {
                            line.html = format!(
                                "{} <span class=\"term-callout\">&lt;{}&gt;</span>",
                                line.html, annotation_num
                            );
                        }
                    }
                } else if let Ok(re) = Regex::new(s) {
                    for line in lines.iter_mut() {
                        if re.is_match(&line.text) {
                            line.html = format!(
                                "{} <span class=\"term-callout\">&lt;{}&gt;</span>",
                                line.html, annotation_num
                            );
                            break;
                        }
                    }
                }
            }
        }
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

fn lognormal_ms(mean: f64, sigma: f64, rng: &mut impl Rng) -> u64 {
    let mu = mean.ln() - (sigma * sigma) / 2.0;
    // Box-Muller transform for normal sample
    let u1: f64 = rng.gen::<f64>().max(1e-10);
    let u2: f64 = rng.gen::<f64>();
    let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
    let sample = (mu + sigma * z).exp();
    (sample as u64).max(10)
}

fn bigram_factor(prev: Option<char>, curr: char) -> f64 {
    match prev {
        None => 1.0,
        Some(p) => {
            if curr == ' ' { return 1.0; }
            if p == ' ' { return 1.3; }
            if p == '.' || p == ',' || p == ';' || p == ':' { return 1.8; }
            let same_hand = on_same_hand(p, curr);
            if p == curr { return 1.4; }
            if same_hand { 1.1 } else { 0.9 }
        }
    }
}

fn on_same_hand(a: char, b: char) -> bool {
    let left = "qwertasdfgzxcvb12345`~!@#$%";
    let a_left = left.contains(a.to_ascii_lowercase());
    let b_left = left.contains(b.to_ascii_lowercase());
    a_left == b_left
}

fn adjacent_key(ch: char, rng: &mut impl Rng) -> char {
    let neighbors: &[(char, &[char])] = &[
        ('a', &['s', 'q', 'z', 'w']),
        ('b', &['v', 'g', 'h', 'n']),
        ('c', &['x', 'd', 'f', 'v']),
        ('d', &['s', 'e', 'r', 'f', 'c', 'x']),
        ('e', &['w', 'r', 'd', 's', '3', '4']),
        ('f', &['d', 'r', 't', 'g', 'v', 'c']),
        ('g', &['f', 't', 'y', 'h', 'b', 'v']),
        ('h', &['g', 'y', 'u', 'j', 'n', 'b']),
        ('i', &['u', 'o', 'k', 'j', '8', '9']),
        ('j', &['h', 'u', 'i', 'k', 'm', 'n']),
        ('k', &['j', 'i', 'o', 'l', ',', 'm']),
        ('l', &['k', 'o', 'p', ';', '.', ',']),
        ('m', &['n', 'j', 'k', ',']),
        ('n', &['b', 'h', 'j', 'm']),
        ('o', &['i', 'p', 'l', 'k', '9', '0']),
        ('p', &['o', '[', ';', 'l', '0', '-']),
        ('q', &['w', 'a', '1', '2']),
        ('r', &['e', 't', 'f', 'd', '4', '5']),
        ('s', &['a', 'w', 'e', 'd', 'x', 'z']),
        ('t', &['r', 'y', 'g', 'f', '5', '6']),
        ('u', &['y', 'i', 'j', 'h', '7', '8']),
        ('v', &['c', 'f', 'g', 'b']),
        ('w', &['q', 'e', 's', 'a', '2', '3']),
        ('x', &['z', 's', 'd', 'c']),
        ('y', &['t', 'u', 'h', 'g', '6', '7']),
        ('z', &['a', 's', 'x']),
        ('1', &['2', 'q']),
        ('2', &['1', '3', 'q', 'w']),
        ('3', &['2', '4', 'w', 'e']),
        ('4', &['3', '5', 'e', 'r']),
        ('5', &['4', '6', 'r', 't']),
        ('6', &['5', '7', 't', 'y']),
        ('7', &['6', '8', 'y', 'u']),
        ('8', &['7', '9', 'u', 'i']),
        ('9', &['8', '0', 'i', 'o']),
        ('0', &['9', '-', 'o', 'p']),
        (' ', &[' ']),
    ];

    let lower = ch.to_ascii_lowercase();
    for (k, adj) in neighbors {
        if *k == lower {
            let picked = adj[rng.gen_range(0..adj.len())];
            if ch.is_uppercase() { return picked.to_ascii_uppercase(); }
            return picked;
        }
    }
    ch
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn make_rendered(text: &str) -> RenderedLine {
        RenderedLine { html: text.to_string(), text: text.to_string() }
    }

    // --- apply_spacing ---

    #[test]
    fn apply_spacing_single_command() {
        let mut lines = vec![make_rendered("$ echo hi"), make_rendered("hi")];
        apply_spacing(&mut lines, "$");
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn apply_spacing_two_commands() {
        let mut lines = vec![
            make_rendered("$ echo a"),
            make_rendered("a"),
            make_rendered("$ echo b"),
            make_rendered("b"),
        ];
        apply_spacing(&mut lines, "$");
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[2].text, "");
    }

    #[test]
    fn apply_spacing_custom_prompt() {
        let mut lines = vec![
            make_rendered("> cmd1"),
            make_rendered("out1"),
            make_rendered("> cmd2"),
        ];
        apply_spacing(&mut lines, ">");
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[2].text, "");
    }

    #[test]
    fn apply_spacing_no_prompt_prefix() {
        let mut lines = vec![make_rendered("abc"), make_rendered("def")];
        apply_spacing(&mut lines, "$");
        assert_eq!(lines.len(), 2);
    }

    // --- apply_remove ---

    #[test]
    fn apply_remove_empty_specs() {
        let mut lines = vec![make_rendered("a"), make_rendered("b")];
        apply_remove(&mut lines, &[]);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn apply_remove_positive_index() {
        let mut lines = vec![make_rendered("a"), make_rendered("b"), make_rendered("c")];
        apply_remove(&mut lines, &[LineSpec::Index(1)]);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "b");
    }

    #[test]
    fn apply_remove_negative_index() {
        let mut lines = vec![make_rendered("a"), make_rendered("b"), make_rendered("c")];
        apply_remove(&mut lines, &[LineSpec::Index(-1)]);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1].text, "b");
    }

    #[test]
    fn apply_remove_zero_index_skipped() {
        let mut lines = vec![make_rendered("a"), make_rendered("b")];
        apply_remove(&mut lines, &[LineSpec::Index(0)]);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn apply_remove_out_of_bounds() {
        let mut lines = vec![make_rendered("a")];
        apply_remove(&mut lines, &[LineSpec::Index(99)]);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn apply_remove_pattern() {
        let mut lines = vec![
            make_rendered("keep this"),
            make_rendered("remove_me"),
            make_rendered("also keep"),
        ];
        apply_remove(&mut lines, &[LineSpec::Expr("remove_me".to_string())]);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "keep this");
        assert_eq!(lines[1].text, "also keep");
    }

    #[test]
    fn apply_remove_multiple_specs() {
        let mut lines = vec![
            make_rendered("a"),
            make_rendered("b"),
            make_rendered("c"),
            make_rendered("d"),
        ];
        apply_remove(&mut lines, &[LineSpec::Index(1), LineSpec::Index(-1)]);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "b");
        assert_eq!(lines[1].text, "c");
    }

    // --- apply_callouts ---

    #[test]
    fn apply_callouts_empty_specs() {
        let mut lines = vec![make_rendered("a")];
        apply_callouts(&mut lines, &[]);
        assert_eq!(lines[0].html, "a");
    }

    #[test]
    fn apply_callouts_positive_index() {
        let mut lines = vec![make_rendered("a"), make_rendered("b")];
        apply_callouts(&mut lines, &[LineSpec::Index(1)]);
        assert!(lines[0].html.contains("term-callout"));
        assert!(lines[0].html.contains("&lt;1&gt;"));
        assert!(!lines[1].html.contains("term-callout"));
    }

    #[test]
    fn apply_callouts_negative_index() {
        let mut lines = vec![make_rendered("a"), make_rendered("b"), make_rendered("c")];
        apply_callouts(&mut lines, &[LineSpec::Index(-1)]);
        assert!(lines[2].html.contains("term-callout"));
        assert!(lines[2].html.contains("&lt;1&gt;"));
    }

    #[test]
    fn apply_callouts_pattern() {
        let mut lines = vec![
            make_rendered("no match"),
            make_rendered("target line"),
            make_rendered("another"),
        ];
        apply_callouts(&mut lines, &[LineSpec::Expr("target".to_string())]);
        assert!(lines[1].html.contains("term-callout"));
        assert!(!lines[2].html.contains("term-callout"));
    }

    #[test]
    fn apply_callouts_sequential_numbering() {
        let mut lines = vec![make_rendered("a"), make_rendered("b"), make_rendered("c")];
        apply_callouts(&mut lines, &[LineSpec::Index(1), LineSpec::Index(3)]);
        assert!(lines[0].html.contains("&lt;1&gt;"));
        assert!(lines[2].html.contains("&lt;2&gt;"));
    }

    // --- parse_range_spec ---

    #[test]
    fn parse_range_start_end() {
        assert_eq!(parse_range_spec("3:7", 10), vec![2, 3, 4, 5, 6]);
    }

    #[test]
    fn parse_range_open_start() {
        assert_eq!(parse_range_spec(":3", 10), vec![0, 1, 2]);
    }

    #[test]
    fn parse_range_open_end() {
        assert_eq!(parse_range_spec("8:", 10), vec![7, 8, 9]);
    }

    #[test]
    fn parse_range_negative_start() {
        assert_eq!(parse_range_spec("-3:", 10), vec![7, 8, 9]);
    }

    #[test]
    fn parse_range_negative_both() {
        assert_eq!(parse_range_spec("-5:-2", 10), vec![5, 6, 7, 8]);
    }

    #[test]
    fn parse_range_out_of_bounds_fully() {
        assert_eq!(parse_range_spec("8:20", 5), Vec::<usize>::new());
    }

    #[test]
    fn parse_range_partially_out_of_bounds() {
        assert_eq!(parse_range_spec("4:20", 5), vec![3, 4]);
    }

    #[test]
    fn parse_range_invalid() {
        assert_eq!(parse_range_spec("abc:def", 10), Vec::<usize>::new());
    }

    // --- resolve_line_specs with ranges ---

    #[test]
    fn resolve_specs_range() {
        let lines = vec![make_rendered("a"), make_rendered("b"), make_rendered("c"),
                         make_rendered("d"), make_rendered("e")];
        let specs = vec![LineSpec::Expr("2:4".to_string())];
        assert_eq!(resolve_line_specs(&specs, &lines), vec![1, 2, 3]);
    }

    #[test]
    fn resolve_specs_mixed() {
        let lines = vec![make_rendered("a"), make_rendered("b"), make_rendered("c"),
                         make_rendered("d"), make_rendered("e")];
        let specs = vec![LineSpec::Index(1), LineSpec::Expr("-1:".to_string())];
        assert_eq!(resolve_line_specs(&specs, &lines), vec![0, 4]);
    }

    #[test]
    fn resolve_specs_regex() {
        let lines = vec![make_rendered("hello"), make_rendered("world"), make_rendered("hello again")];
        let specs = vec![LineSpec::Expr("^hello".to_string())];
        assert_eq!(resolve_line_specs(&specs, &lines), vec![0, 2]);
    }

    // --- apply_truncate ---

    #[test]
    fn apply_truncate_empty_specs() {
        let mut lines = vec![make_rendered("a"), make_rendered("b")];
        apply_truncate(&mut lines, &[], "html");
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn apply_truncate_single_run() {
        let mut lines: Vec<_> = (1..=10).map(|i| make_rendered(&format!("line {}", i))).collect();
        apply_truncate(&mut lines, &[LineSpec::Expr("3:7".to_string())], "html");
        assert_eq!(lines.len(), 6);
        assert_eq!(lines[0].text, "line 1");
        assert_eq!(lines[1].text, "line 2");
        assert!(lines[2].html.contains("term-truncated"));
        assert!(lines[2].text.contains("5 lines truncated"));
        assert_eq!(lines[3].text, "line 8");
        assert_eq!(lines[4].text, "line 9");
        assert_eq!(lines[5].text, "line 10");
    }

    #[test]
    fn apply_truncate_two_runs() {
        let mut lines: Vec<_> = (1..=10).map(|i| make_rendered(&format!("line {}", i))).collect();
        apply_truncate(&mut lines, &[LineSpec::Expr(":2".to_string()), LineSpec::Expr("-2:".to_string())], "html");
        assert_eq!(lines.len(), 8);
        assert!(lines[0].text.contains("2 lines truncated"));
        assert_eq!(lines[1].text, "line 3");
        assert_eq!(lines[6].text, "line 8");
        assert!(lines[7].text.contains("2 lines truncated"));
    }

    #[test]
    fn apply_truncate_adjacent_merge() {
        let mut lines: Vec<_> = (1..=10).map(|i| make_rendered(&format!("line {}", i))).collect();
        apply_truncate(&mut lines, &[LineSpec::Expr("2:4".to_string()), LineSpec::Expr("5:6".to_string())], "html");
        assert_eq!(lines.len(), 6);
        assert_eq!(lines[0].text, "line 1");
        assert!(lines[1].text.contains("5 lines truncated"));
        assert_eq!(lines[2].text, "line 7");
    }

    #[test]
    fn apply_truncate_single_line() {
        let mut lines = vec![make_rendered("a"), make_rendered("b"), make_rendered("c")];
        apply_truncate(&mut lines, &[LineSpec::Index(2)], "html");
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].text, "a");
        assert!(lines[1].text.contains("1 line truncated"));
        assert_eq!(lines[2].text, "c");
    }

    #[test]
    fn apply_truncate_latex_format() {
        let mut lines = vec![make_rendered("a"), make_rendered("b"), make_rendered("c")];
        apply_truncate(&mut lines, &[LineSpec::Index(2)], "latex");
        assert!(lines[1].html.contains("\\textit"));
    }

    #[test]
    fn apply_truncate_markdown_format() {
        let mut lines = vec![make_rendered("a"), make_rendered("b"), make_rendered("c")];
        apply_truncate(&mut lines, &[LineSpec::Index(2)], "markdown");
        assert_eq!(lines[1].html, "*[1 line truncated]*");
    }

    // --- apply_remove with ranges ---

    #[test]
    fn apply_remove_range() {
        let mut lines: Vec<_> = (1..=5).map(|i| make_rendered(&format!("line {}", i))).collect();
        apply_remove(&mut lines, &[LineSpec::Expr("2:4".to_string())]);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "line 1");
        assert_eq!(lines[1].text, "line 5");
    }

    // --- html_escape_basic ---

    #[test]
    fn html_escape_basic_works() {
        assert_eq!(html_escape_basic("a&b"), "a&amp;b");
        assert_eq!(html_escape_basic("<>"), "&lt;&gt;");
        assert_eq!(html_escape_basic("hello"), "hello");
    }

    // --- lognormal_ms ---

    #[test]
    fn lognormal_ms_minimum_floor() {
        let mut rng = StdRng::seed_from_u64(42);
        for _ in 0..1000 {
            let val = lognormal_ms(50.0, 0.4, &mut rng);
            assert!(val >= 10);
        }
    }

    #[test]
    fn lognormal_ms_reasonable_range() {
        let mut rng = StdRng::seed_from_u64(123);
        let samples: Vec<u64> = (0..1000).map(|_| lognormal_ms(100.0, 0.4, &mut rng)).collect();
        let mean: f64 = samples.iter().map(|&x| x as f64).sum::<f64>() / 1000.0;
        assert!(mean > 50.0 && mean < 200.0);
    }

    // --- bigram_factor ---

    #[test]
    fn bigram_factor_none_prev() {
        assert_eq!(bigram_factor(None, 'a'), 1.0);
    }

    #[test]
    fn bigram_factor_space_after_punct() {
        assert_eq!(bigram_factor(Some('.'), 'a'), 1.8);
        assert_eq!(bigram_factor(Some(','), 'b'), 1.8);
    }

    #[test]
    fn bigram_factor_same_char() {
        assert_eq!(bigram_factor(Some('a'), 'a'), 1.4);
    }

    #[test]
    fn bigram_factor_space_curr() {
        assert_eq!(bigram_factor(Some('a'), ' '), 1.0);
    }

    #[test]
    fn bigram_factor_space_prev() {
        assert_eq!(bigram_factor(Some(' '), 'a'), 1.3);
    }

    #[test]
    fn bigram_factor_same_hand() {
        // q and w are both left hand
        assert_eq!(bigram_factor(Some('q'), 'w'), 1.1);
    }

    #[test]
    fn bigram_factor_cross_hand() {
        // f is left, j is right
        assert_eq!(bigram_factor(Some('f'), 'j'), 0.9);
    }

    // --- adjacent_key ---

    #[test]
    fn adjacent_key_known_char() {
        let mut rng = StdRng::seed_from_u64(42);
        let result = adjacent_key('a', &mut rng);
        assert!(['s', 'q', 'z', 'w'].contains(&result));
    }

    #[test]
    fn adjacent_key_uppercase() {
        let mut rng = StdRng::seed_from_u64(42);
        let result = adjacent_key('A', &mut rng);
        assert!(result.is_uppercase());
    }

    #[test]
    fn adjacent_key_unknown() {
        let mut rng = StdRng::seed_from_u64(42);
        let result = adjacent_key('!', &mut rng);
        assert_eq!(result, '!');
    }
}
