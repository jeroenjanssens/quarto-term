use avt::{Color, Line, Pen};

use crate::renderer::RenderedLine;

pub fn render_line(line: &Line, ansi: bool, trailing_spaces: bool) -> RenderedLine {
    let text = line_to_text(line);

    if !ansi {
        let src = if trailing_spaces { line_to_text_raw(line) } else { text.clone() };
        return RenderedLine {
            html: xml_escape(&src),
            text,
        };
    }

    let cells: Vec<&avt::Cell> = line.cells().iter().collect();
    if cells.is_empty() {
        return RenderedLine {
            html: String::new(),
            text,
        };
    }

    let mut spans = String::new();
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
            let escaped = xml_escape(&chunk_text);
            if pen.is_default() {
                spans.push_str(&format!("<text:span>{escaped}</text:span>"));
            } else {
                let style = span_style(pen);
                spans.push_str(&format!("<text:span text:style-name=\"{style}\">{escaped}</text:span>"));
            }
        }

        i = j;
    }

    RenderedLine { html: spans, text }
}

pub fn render_lines_to_odt(lines: &[RenderedLine]) -> String {
    if lines.is_empty() {
        return String::new();
    }

    let mut paragraphs = String::new();

    for line in lines {
        if line.html.is_empty() {
            paragraphs.push_str("<text:p text:style-name=\"Preformatted_20_Text\"/>");
        } else {
            paragraphs.push_str(&format!(
                "<text:p text:style-name=\"Preformatted_20_Text\">{}</text:p>",
                line.html
            ));
        }
    }

    paragraphs
}

pub fn render_fullscreen_to_odt(lines: &[RenderedLine]) -> String {
    render_lines_to_odt(lines)
}


fn span_style(pen: &Pen) -> String {
    let fg = if pen.is_inverse() { pen.background() } else { pen.foreground() };
    let mut parts = Vec::new();
    if let Some(color) = fg {
        parts.push(format!("c{}", color_to_hex(color)));
    }
    if pen.is_bold() { parts.push("b".to_string()); }
    if pen.is_italic() { parts.push("i".to_string()); }
    // Return a synthetic style name — ODT requires pre-declared styles,
    // but pandoc passes through raw content as-is
    format!("T{}", parts.join(""))
}

fn color_to_hex(color: Color) -> String {
    match color {
        Color::RGB(rgb) => format!("{:02X}{:02X}{:02X}", rgb.r, rgb.g, rgb.b),
        Color::Indexed(i) if i < 16 => {
            let (r, g, b) = ansi_index_to_rgb(i);
            format!("{:02X}{:02X}{:02X}", r, g, b)
        }
        Color::Indexed(i) if i < 232 => {
            let idx = i - 16;
            let r = idx / 36;
            let g = (idx % 36) / 6;
            let b = idx % 6;
            let to_byte = |v: u8| -> u8 { if v == 0 { 0 } else { 55 + v * 40 } };
            format!("{:02X}{:02X}{:02X}", to_byte(r), to_byte(g), to_byte(b))
        }
        Color::Indexed(i) => {
            let l = 8 + 10 * (i as u16 - 232) as u8;
            format!("{:02X}{:02X}{:02X}", l, l, l)
        }
    }
}

fn ansi_index_to_rgb(i: u8) -> (u8, u8, u8) {
    match i {
        0 => (0, 0, 0),
        1 => (205, 49, 49),
        2 => (13, 188, 121),
        3 => (229, 229, 16),
        4 => (36, 114, 200),
        5 => (188, 63, 188),
        6 => (17, 168, 205),
        7 => (229, 229, 229),
        8 => (102, 102, 102),
        9 => (241, 76, 76),
        10 => (35, 209, 139),
        11 => (245, 245, 67),
        12 => (59, 142, 234),
        13 => (214, 112, 214),
        14 => (41, 184, 219),
        15 => (229, 229, 229),
        _ => (255, 255, 255),
    }
}

fn xml_escape(s: &str) -> String {
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

fn line_to_text(line: &Line) -> String {
    line_to_text_raw(line).trim_end().to_string()
}

fn line_to_text_raw(line: &Line) -> String {
    line.cells().iter()
        .filter(|c| c.width() > 0)
        .map(|c| { let ch = c.char(); if ch == '\0' { ' ' } else { ch } })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::RenderedLine;

    #[test]
    fn render_lines_to_odt_empty() {
        assert_eq!(render_lines_to_odt(&[]), "");
    }

    #[test]
    fn render_lines_to_odt_basic() {
        let lines = vec![
            RenderedLine { html: "<text:span>hello</text:span>".to_string(), text: "hello".to_string() },
        ];
        let result = render_lines_to_odt(&lines);
        assert!(result.contains("Preformatted_20_Text"));
        assert!(result.contains("hello"));
    }
}
