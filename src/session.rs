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
use crate::protocol::{AnnotationSpec, CellResult, Config, EchoMode, InputCell};
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
    ps2_re: Option<Regex>,
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
        let ps2_re = config
            .ps2
            .as_ref()
            .map(|s| {
                let escaped = regex::escape(s.trim_end());
                Regex::new(&format!("^{}\\s*$", escaped))
            })
            .transpose()
            .map_err(|e| TermError::RegexCompile(e.to_string()))?;

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: config.rows,
                cols: config.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| TermError::SpawnFailed(e.to_string()))?;

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
            ps2_re,
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
        if let Some(t) = cell.options.timeout {
            self.config.timeout = t;
        }

        let typing = cell.options.typing.as_ref().unwrap_or(&self.config.typing);
        let is_human = typing.is_enabled();
        let speed = typing.speed();
        let error_rate = typing.error_rate();

        let lines: Vec<&str> = cell.code.lines().collect();
        let mut error: Option<String> = None;

        let cell_literal = cell.options.literal.unwrap_or(true);
        let cell_delay = cell.options.delay.unwrap_or(0.1);

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
            let enter = line_opts
                .and_then(|lo| lo.enter)
                .unwrap_or(literal);
            let expect_prompt = line_opts
                .and_then(|lo| lo.expect_prompt)
                .unwrap_or(enter);

            if self.config.verbose {
                let source = cell.source_lines.get(idx).map(|s| s.as_str()).unwrap_or(line_text);
                eprintln!("quarto-term:   > {}", source);
            }

            if delay > 0.0 {
                thread::sleep(Duration::from_secs_f64(delay));
            }

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
            } else {
                let resolved = ResolvedLineOpts {
                    literal,
                    enter,
                    hold,
                    expect_prompt,
                };
                if let Err(e) = self.send_line_resolved(line_text, &resolved, is_human, speed, error_rate) {
                    error = Some(e.to_string());
                    break;
                }
            }
        }

        if let Some(hold) = cell.options.hold {
            if hold > 0.0 {
                self.drain_during(Duration::from_secs_f64(hold));
            }
        }


        self.config.timeout = orig_timeout;

        let use_ansi = cell.options.ansi.unwrap_or(self.config.ansi);
        let html = self.build_cell_html(cell, before_cursor_row, before_scrollback, use_ansi);

        let scroll = cell.options.scroll.unwrap_or(!cell.options.fullscreen);
        if scroll {
            self.save_position();
        }

        CellResult {
            id: cell.id,
            html,
            error,
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
                let mut b = keymap::translate_keycode(text);
                if opts.enter && !is_keycode_name(text) {
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
                    for rec in &mut self.recorders {
                        rec.record_output(&bytes);
                    }
                    self.output_since_cell_start.extend_from_slice(&bytes);
                    let text = String::from_utf8_lossy(&bytes);
                    self.vt.feed_str(&text);

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

    fn drain_during(&mut self, duration: Duration) {
        let deadline = Instant::now() + duration;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            match self.rx.recv_timeout(remaining.min(Duration::from_millis(50))) {
                Ok(bytes) => {
                    for rec in &mut self.recorders {
                        rec.record_output(&bytes);
                    }
                    self.output_since_cell_start.extend_from_slice(&bytes);
                    let text = String::from_utf8_lossy(&bytes);
                    self.vt.feed_str(&text);
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
                    for rec in &mut self.recorders {
                        rec.record_output(&bytes);
                    }
                    self.output_since_cell_start.extend_from_slice(&bytes);
                    let text = String::from_utf8_lossy(&bytes);
                    self.vt.feed_str(&text);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
    }

    fn last_line_matches_prompt(&self) -> bool {
        let view_lines: Vec<_> = self.vt.view().collect();
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
        if let Some(ref ps2) = self.ps2_re {
            return ps2.is_match(text);
        }
        false
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

        let echo_mode = &cell.options.echo;

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
                        let lang = match &cell.options.highlight {
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

            if !cell.options.keep_last_prompt {
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

fn apply_remove(lines: &mut Vec<RenderedLine>, specs: &[AnnotationSpec]) {
    if specs.is_empty() {
        return;
    }

    let mut to_remove = Vec::new();

    for spec in specs {
        match spec {
            AnnotationSpec::Index(i) => {
                let idx = if *i > 0 {
                    (*i as usize).saturating_sub(1)
                } else if *i < 0 {
                    (lines.len() as i32 + i) as usize
                } else {
                    continue;
                };
                if idx < lines.len() {
                    to_remove.push(idx);
                }
            }
            AnnotationSpec::Pattern(pat) => {
                if let Ok(re) = Regex::new(pat) {
                    for (idx, line) in lines.iter().enumerate() {
                        if re.is_match(&line.text) {
                            to_remove.push(idx);
                        }
                    }
                }
            }
        }
    }

    to_remove.sort_unstable();
    to_remove.dedup();
    for idx in to_remove.into_iter().rev() {
        lines.remove(idx);
    }
}

fn apply_callouts(lines: &mut Vec<RenderedLine>, specs: &[AnnotationSpec]) {
    if specs.is_empty() {
        return;
    }

    for (n, spec) in specs.iter().enumerate() {
        let annotation_num = n + 1;
        match spec {
            AnnotationSpec::Index(i) => {
                let idx = if *i > 0 {
                    (*i as usize).saturating_sub(1)
                } else if *i < 0 {
                    (lines.len() as i32 + i) as usize
                } else {
                    continue;
                };
                if let Some(line) = lines.get_mut(idx) {
                    line.html = format!(
                        "{} <span class=\"term-callout\">&lt;{}&gt;</span>",
                        line.html, annotation_num
                    );
                }
            }
            AnnotationSpec::Pattern(pat) => {
                if let Ok(re) = Regex::new(pat) {
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

fn is_keycode_name(s: &str) -> bool {
    let lower = s.to_lowercase();
    matches!(
        lower.as_str(),
        "enter" | "return" | "cr" | "tab" | "escape" | "esc" | "backspace" | "bs"
            | "delete" | "del" | "space" | "up" | "down" | "left" | "right"
            | "home" | "end" | "pageup" | "page_up" | "pagedown" | "page_down"
            | "insert" | "f1" | "f2" | "f3" | "f4" | "f5" | "f6"
            | "f7" | "f8" | "f9" | "f10" | "f11" | "f12"
    ) || lower.starts_with("ctrl-")
        || lower.starts_with("c-")
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

use crate::protocol::HighlightSpec;

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
        apply_remove(&mut lines, &[AnnotationSpec::Index(1)]);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "b");
    }

    #[test]
    fn apply_remove_negative_index() {
        let mut lines = vec![make_rendered("a"), make_rendered("b"), make_rendered("c")];
        apply_remove(&mut lines, &[AnnotationSpec::Index(-1)]);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1].text, "b");
    }

    #[test]
    fn apply_remove_zero_index_skipped() {
        let mut lines = vec![make_rendered("a"), make_rendered("b")];
        apply_remove(&mut lines, &[AnnotationSpec::Index(0)]);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn apply_remove_out_of_bounds() {
        let mut lines = vec![make_rendered("a")];
        apply_remove(&mut lines, &[AnnotationSpec::Index(99)]);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn apply_remove_pattern() {
        let mut lines = vec![
            make_rendered("keep this"),
            make_rendered("remove_me"),
            make_rendered("also keep"),
        ];
        apply_remove(&mut lines, &[AnnotationSpec::Pattern("remove_me".to_string())]);
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
        apply_remove(&mut lines, &[AnnotationSpec::Index(1), AnnotationSpec::Index(-1)]);
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
        apply_callouts(&mut lines, &[AnnotationSpec::Index(1)]);
        assert!(lines[0].html.contains("term-callout"));
        assert!(lines[0].html.contains("&lt;1&gt;"));
        assert!(!lines[1].html.contains("term-callout"));
    }

    #[test]
    fn apply_callouts_negative_index() {
        let mut lines = vec![make_rendered("a"), make_rendered("b"), make_rendered("c")];
        apply_callouts(&mut lines, &[AnnotationSpec::Index(-1)]);
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
        apply_callouts(&mut lines, &[AnnotationSpec::Pattern("target".to_string())]);
        assert!(lines[1].html.contains("term-callout"));
        assert!(!lines[2].html.contains("term-callout"));
    }

    #[test]
    fn apply_callouts_sequential_numbering() {
        let mut lines = vec![make_rendered("a"), make_rendered("b"), make_rendered("c")];
        apply_callouts(&mut lines, &[AnnotationSpec::Index(1), AnnotationSpec::Index(3)]);
        assert!(lines[0].html.contains("&lt;1&gt;"));
        assert!(lines[2].html.contains("&lt;2&gt;"));
    }

    // --- is_keycode_name ---

    #[test]
    fn is_keycode_name_known_keys() {
        assert!(is_keycode_name("enter"));
        assert!(is_keycode_name("tab"));
        assert!(is_keycode_name("escape"));
        assert!(is_keycode_name("up"));
        assert!(is_keycode_name("f1"));
        assert!(is_keycode_name("backspace"));
    }

    #[test]
    fn is_keycode_name_ctrl_prefix() {
        assert!(is_keycode_name("ctrl-c"));
        assert!(is_keycode_name("c-x"));
    }

    #[test]
    fn is_keycode_name_plain_text() {
        assert!(!is_keycode_name("hello"));
        assert!(!is_keycode_name("a"));
        assert!(!is_keycode_name("echo"));
    }

    #[test]
    fn is_keycode_name_case_insensitive() {
        assert!(is_keycode_name("Enter"));
        assert!(is_keycode_name("TAB"));
        assert!(is_keycode_name("Ctrl-C"));
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
