use std::io::{Read, Write};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use regex::Regex;

use crate::error::TermError;
use crate::keymap;
use crate::protocol::{
    AnnotationSpec, CellResult, Config, EchoMode, InputCell, LineOptions,
};
use crate::recorder::Recorder;
use crate::renderer::{self, RenderedLine};

pub struct PtySession {
    writer: Box<dyn Write + Send>,
    rx: Receiver<Vec<u8>>,
    vt: avt::Vt,
    prompt_re: Regex,
    config: Config,
    recorder: Option<Recorder>,
    output_since_cell_start: Vec<u8>,
    cursor_row_after_last_cell: usize,
    scrollback_after_last_cell: usize,
}

impl PtySession {
    pub fn new(config: &Config) -> Result<Self, TermError> {
        let prompt_re = Regex::new(&config.prompt)
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

        let recorder = config
            .record
            .as_ref()
            .and_then(|path| Recorder::new(path, config.cols, config.rows).ok());

        let mut session = Self {
            writer,
            rx,
            vt,
            prompt_re,
            config: config.clone(),
            recorder,
            output_since_cell_start: Vec::new(),
            cursor_row_after_last_cell: 0,
            scrollback_after_last_cell: 0,
        };

        session.wait_for_prompt()?;
        session.save_position();

        Ok(session)
    }

    pub fn execute_cell(&mut self, cell: &InputCell) -> CellResult {
        self.output_since_cell_start.clear();

        let before_cursor_row = self.cursor_row_after_last_cell;
        let before_scrollback = self.scrollback_after_last_cell;

        let lines: Vec<&str> = cell.code.lines().collect();
        let mut error: Option<String> = None;

        for (idx, line_text) in lines.iter().enumerate() {
            let line_opts = cell
                .line_options
                .iter()
                .find(|lo| lo.line_index == idx as u32);

            let default_opts = LineOptions {
                line_index: idx as u32,
                literal: true,
                enter: None,
                wait: 0.0,
                hold: 0.1,
                expect_prompt: None,
            };
            let opts = line_opts.unwrap_or(&default_opts);

            if let Err(e) = self.send_line(line_text, opts) {
                error = Some(e.to_string());
                break;
            }
        }

        let use_ansi = cell.options.ansi.unwrap_or(self.config.ansi);
        let html = self.build_cell_html(cell, before_cursor_row, before_scrollback, use_ansi);

        if cell.options.scroll {
            self.save_position();
        }

        CellResult {
            id: cell.id,
            html,
            error,
        }
    }

    fn send_line(&mut self, text: &str, opts: &LineOptions) -> Result<(), TermError> {
        if opts.wait > 0.0 {
            thread::sleep(Duration::from_secs_f64(opts.wait));
        }

        let bytes = if opts.literal {
            let mut b = text.as_bytes().to_vec();
            if opts.effective_enter() {
                b.push(b'\r');
            }
            b
        } else {
            let mut b = keymap::translate_keycode(text);
            if opts.effective_enter() && !is_keycode_name(text) {
                b.push(b'\r');
            }
            b
        };

        if let Some(rec) = &mut self.recorder {
            rec.record_input(&bytes);
        }

        self.writer
            .write_all(&bytes)
            .map_err(|_| TermError::ShellExited)?;
        self.writer.flush().map_err(|_| TermError::ShellExited)?;

        if opts.hold > 0.0 {
            thread::sleep(Duration::from_secs_f64(opts.hold));
        }

        self.drain_pty();

        if opts.effective_expect_prompt() {
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
                    if let Some(rec) = &mut self.recorder {
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

    fn drain_pty(&mut self) {
        loop {
            match self.rx.try_recv() {
                Ok(bytes) => {
                    if let Some(rec) = &mut self.recorder {
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
                return self.prompt_re.is_match(&text);
            }
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
        let mut html = String::new();

        let echo_mode = &cell.options.echo;

        match echo_mode {
            EchoMode::Bool(false) => {}
            EchoMode::Mode(m) if m == "false" => {}
            EchoMode::Mode(m) if m == "source" => {
                let lang = match &cell.options.highlight {
                    HighlightSpec::Language(l) => l.as_str(),
                    HighlightSpec::Bool(false) => "text",
                    HighlightSpec::Bool(true) => "bash",
                };
                html.push_str(&format!(
                    "<pre class=\"term-source\"><code class=\"language-{lang}\">{}</code></pre>\n",
                    html_escape_basic(&cell.code)
                ));
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
                self.capture_fullscreen(ansi)
            } else {
                self.capture_new_lines(before_cursor_row, before_scrollback, ansi)
            };

            if !cell.options.keep_last_prompt && !cell.options.fullscreen {
                if let Some(last) = lines.last() {
                    if self.prompt_re.is_match(&last.text) {
                        lines.pop();
                    }
                }
            }

            apply_remove(&mut lines, &cell.options.remove);
            apply_callouts(&mut lines, &cell.options.callouts);

            if cell.options.fullscreen {
                html.push_str(&renderer::render_fullscreen_to_html(&lines, self.config.cols));
            } else {
                html.push_str(&renderer::render_lines_to_html(&lines, "term-output"));
            }
        }

        html
    }

    fn capture_new_lines(
        &self,
        before_cursor_row: usize,
        before_scrollback: usize,
        ansi: bool,
    ) -> Vec<RenderedLine> {
        let current_scrollback = self.scrollback_count();
        let current_cursor_row = self.vt.cursor().row;

        let all_lines: Vec<_> = self.vt.lines().collect();

        let start_line = before_scrollback + before_cursor_row;
        let end_line = current_scrollback + current_cursor_row;

        if end_line <= start_line || start_line >= all_lines.len() {
            return Vec::new();
        }

        let end = end_line.min(all_lines.len());

        all_lines[start_line..end]
            .iter()
            .map(|line| renderer::render_line(line, ansi))
            .collect()
    }

    fn capture_fullscreen(&self, ansi: bool) -> Vec<RenderedLine> {
        self.vt
            .view()
            .map(|line| renderer::render_line(line, ansi))
            .collect()
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
    let s: String = line
        .cells()
        .iter()
        .filter(|c| c.width() > 0)
        .map(|c| {
            let ch = c.char();
            if ch == '\0' { ' ' } else { ch }
        })
        .collect();
    s.trim_end().to_string()
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

use crate::protocol::HighlightSpec;
