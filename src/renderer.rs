use avt::{Cell, Color, Line, Pen};

pub struct RenderedLine {
    pub html: String,
    pub text: String,
}

pub fn render_line(line: &Line, ansi: bool) -> RenderedLine {
    let text = line_to_text(line);

    if !ansi {
        return RenderedLine {
            html: html_escape(&text),
            text,
        };
    }

    let cells: Vec<&Cell> = line.cells().iter().collect();
    if cells.is_empty() {
        return RenderedLine {
            html: String::new(),
            text,
        };
    }

    let mut html = String::new();
    let mut i = 0;

    while i < cells.len() {
        let pen = cells[i].pen();
        let mut chunk_text = String::new();

        let mut j = i;
        while j < cells.len() && pens_equal(cells[j].pen(), pen) {
            if cells[j].width() > 0 {
                let ch = cells[j].char();
                chunk_text.push(if ch == '\0' { ' ' } else { ch });
            }
            j += 1;
        }

        if !chunk_text.is_empty() {
            if pen.is_default() {
                html.push_str(&html_escape(&chunk_text));
            } else {
                let style = pen_to_style(pen);
                if style.is_empty() {
                    html.push_str(&html_escape(&chunk_text));
                } else {
                    html.push_str("<span style=\"");
                    html.push_str(&style);
                    html.push_str("\">");
                    html.push_str(&html_escape(&chunk_text));
                    html.push_str("</span>");
                }
            }
        }

        i = j;
    }

    let html = trim_trailing_spaces_html(&html);

    RenderedLine { html, text }
}

pub fn render_lines_to_html(lines: &[RenderedLine], css_class: &str) -> String {
    if lines.is_empty() {
        return String::new();
    }

    let inner: String = lines
        .iter()
        .map(|l| l.html.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    format!("<pre class=\"{css_class}\"><code>{inner}</code></pre>\n")
}

pub fn render_fullscreen_to_html(lines: &[RenderedLine], _cols: u16) -> String {
    if lines.is_empty() {
        return String::new();
    }

    let inner: String = lines
        .iter()
        .map(|l| l.html.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    format!("<pre class=\"term-screen\"><code>{inner}</code></pre>\n")
}

fn line_to_text(line: &Line) -> String {
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

fn pens_equal(a: &Pen, b: &Pen) -> bool {
    a.foreground() == b.foreground()
        && a.background() == b.background()
        && a.is_bold() == b.is_bold()
        && a.is_faint() == b.is_faint()
        && a.is_italic() == b.is_italic()
        && a.is_underline() == b.is_underline()
        && a.is_strikethrough() == b.is_strikethrough()
        && a.is_inverse() == b.is_inverse()
}

fn pen_to_style(pen: &Pen) -> String {
    let mut parts = Vec::new();

    let (fg, bg) = if pen.is_inverse() {
        (pen.background(), pen.foreground())
    } else {
        (pen.foreground(), pen.background())
    };

    if let Some(color) = fg {
        parts.push(format!("color:{}", color_to_css(color)));
    } else if pen.is_inverse() {
        parts.push("color:var(--term-bg, #000)".to_string());
    }

    if let Some(color) = bg {
        parts.push(format!("background:{}", color_to_css(color)));
    } else if pen.is_inverse() {
        parts.push("background:var(--term-fg, #fff)".to_string());
    }

    if pen.is_bold() {
        parts.push("font-weight:bold".to_string());
    }
    if pen.is_faint() {
        parts.push("opacity:0.5".to_string());
    }
    if pen.is_italic() {
        parts.push("font-style:italic".to_string());
    }
    if pen.is_underline() && pen.is_strikethrough() {
        parts.push("text-decoration:underline line-through".to_string());
    } else if pen.is_underline() {
        parts.push("text-decoration:underline".to_string());
    } else if pen.is_strikethrough() {
        parts.push("text-decoration:line-through".to_string());
    }

    parts.join(";")
}

fn color_to_css(color: Color) -> String {
    match color {
        Color::RGB(rgb) => format!("#{:02x}{:02x}{:02x}", rgb.r, rgb.g, rgb.b),
        Color::Indexed(i) if i < 16 => format!("var(--term-{i})", i = i),
        Color::Indexed(i) if i < 232 => {
            let idx = i - 16;
            let r = idx / 36;
            let g = (idx % 36) / 6;
            let b = idx % 6;
            let to_byte = |v: u8| -> u8 {
                if v == 0 { 0 } else { 55 + v * 40 }
            };
            format!("#{:02x}{:02x}{:02x}", to_byte(r), to_byte(g), to_byte(b))
        }
        Color::Indexed(i) => {
            let l = 8 + 10 * (i as u16 - 232) as u8;
            format!("#{:02x}{:02x}{:02x}", l, l, l)
        }
    }
}

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

fn trim_trailing_spaces_html(s: &str) -> String {
    s.trim_end().to_string()
}
