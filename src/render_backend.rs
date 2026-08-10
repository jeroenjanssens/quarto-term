use avt::{Line, Pen};

use crate::renderer::RenderedLine;
use crate::terminal_line;

pub trait RenderBackend {
    fn escape(&self, text: &str) -> String;
    fn wrap_styled(&self, escaped_text: &str, pen: &Pen) -> String;
    fn trim_trailing(&self, output: String) -> String {
        output.trim_end().to_string()
    }
}

pub fn render_line_generic<B: RenderBackend>(
    backend: &B,
    line: &Line,
    ansi: bool,
    trailing_spaces: bool,
) -> RenderedLine {
    let text = terminal_line::line_to_text(line);

    if !ansi {
        let src = if trailing_spaces {
            terminal_line::line_to_text_raw(line)
        } else {
            text.clone()
        };
        return RenderedLine {
            markup: backend.escape(&src),
            text,
            is_prompt: false,
        };
    }

    let (runs, pens) = terminal_line::styled_runs(line);
    if runs.is_empty() {
        return RenderedLine {
            markup: String::new(),
            text,
            is_prompt: false,
        };
    }

    let mut output = String::new();

    for run in &runs {
        let escaped = backend.escape(&run.text);
        if run.is_default {
            output.push_str(&escaped);
        } else {
            let wrapped = backend.wrap_styled(&escaped, &pens[run.pen_idx]);
            output.push_str(&wrapped);
        }
    }

    let output = if trailing_spaces {
        output
    } else {
        backend.trim_trailing(output)
    };

    RenderedLine { markup: output, text, is_prompt: false }
}
