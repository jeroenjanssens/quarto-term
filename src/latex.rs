use avt::{Color, Line, Pen};

use crate::renderer::RenderedLine;

pub fn render_line(line: &Line, ansi: bool, trailing_spaces: bool) -> RenderedLine {
    let text = line_to_text(line);

    if !ansi {
        let src = if trailing_spaces { line_to_text_raw(line) } else { text.clone() };
        return RenderedLine {
            html: latex_escape(&src),
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

    let mut latex = String::new();
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
            let escaped = latex_escape(&chunk_text);
            if pen.is_default() {
                latex.push_str(&escaped);
            } else {
                let wrapped = wrap_with_pen(&escaped, pen);
                latex.push_str(&wrapped);
            }
        }

        i = j;
    }

    let latex = if trailing_spaces { latex } else { latex.trim_end().to_string() };

    RenderedLine { html: latex, text }
}

pub struct LatexTheme<'a> {
    pub bg: Option<&'a str>,
    pub fg: Option<&'a str>,
    pub font_size: Option<&'a str>,
    pub font_family: Option<&'a str>,
    pub line_height: Option<&'a str>,
}

pub fn render_lines_to_latex(lines: &[RenderedLine], theme: &LatexTheme) -> String {
    if lines.is_empty() {
        return String::new();
    }

    let inner: String = lines
        .iter()
        .map(|l| l.html.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    let preamble = latex_preamble(theme);
    let box_opts = tcolorbox_opts(theme.bg, theme.fg);
    format!(
        "\\begin{{tcolorbox}}[{box_opts}]\n\
         {preamble}\\begin{{Verbatim}}[commandchars=\\\\\\{{\\}},breaklines=true]\n\
         {inner}\n\
         \\end{{Verbatim}}\n\
         \\end{{tcolorbox}}\n"
    )
}

pub fn render_fullscreen_to_latex(lines: &[RenderedLine], theme: &LatexTheme) -> String {
    if lines.is_empty() {
        return String::new();
    }

    let inner: String = lines
        .iter()
        .map(|l| l.html.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    let preamble = latex_preamble(theme);
    let box_opts = tcolorbox_opts(theme.bg, theme.fg);
    format!(
        "\\begin{{tcolorbox}}[{box_opts}]\n\
         {preamble}\\begin{{Verbatim}}[commandchars=\\\\\\{{\\}},breaklines=true]\n\
         {inner}\n\
         \\end{{Verbatim}}\n\
         \\end{{tcolorbox}}\n"
    )
}

fn latex_preamble(theme: &LatexTheme) -> String {
    let mut parts = Vec::new();
    if let Some(lh) = theme.line_height {
        if let Some(val) = parse_line_height(lh) {
            parts.push(format!("\\linespread{{{val}}}\\selectfont\n"));
        }
    }
    if let Some(font) = theme.font_family {
        let name = font.split(',').next().unwrap_or(font).trim().trim_matches('"').trim_matches('\'');
        parts.push(format!("\\setmonofont{{{name}}}\n"));
    }
    parts.push(css_fontsize_to_latex(theme.font_size));
    parts.concat()
}

fn parse_line_height(s: &str) -> Option<&str> {
    let trimmed = s.trim();
    if trimmed.parse::<f64>().is_ok() {
        Some(trimmed)
    } else {
        None
    }
}

fn tcolorbox_opts(bg: Option<&str>, fg: Option<&str>) -> String {
    let colback = if bg.is_some() { "termbg" } else { "black!5!white" };
    let colupper = if fg.is_some() { "termfg" } else { "black" };
    let colframe = if bg.is_some() { "termbg!80!black" } else { "black!50!white" };

    format!("colback={colback},colframe={colframe},colupper={colupper},boxrule=0.5pt,arc=3pt,left=6pt,right=6pt,top=4pt,bottom=4pt")
}

fn css_fontsize_to_latex(fontsize: Option<&str>) -> String {
    match fontsize {
        None => String::new(),
        Some(s) => {
            let cmd = match s {
                "0.6em" | "5pt" | "6pt" => "\\tiny",
                "0.7em" | "7pt" | "8pt" => "\\scriptsize",
                "0.75em" | "9pt" => "\\footnotesize",
                "0.8em" | "0.85em" | "10pt" => "\\small",
                "1em" | "11pt" | "12pt" => "\\normalsize",
                "1.1em" | "1.2em" | "14pt" => "\\large",
                _ => "\\small",
            };
            format!("{cmd}\n")
        }
    }
}

fn wrap_with_pen(text: &str, pen: &Pen) -> String {
    let mut result = text.to_string();

    let fg = if pen.is_inverse() {
        pen.background()
    } else {
        pen.foreground()
    };

    if let Some(color) = fg {
        let hex = color_to_hex(color);
        result = format!("\\textcolor[HTML]{{{hex}}}{{{result}}}");
    }

    if pen.is_bold() {
        result = format!("\\textbf{{{result}}}");
    }
    if pen.is_italic() {
        result = format!("\\textit{{{result}}}");
    }
    if pen.is_underline() {
        result = format!("\\underline{{{result}}}");
    }

    result
}

fn color_to_hex(color: Color) -> String {
    match color {
        Color::RGB(rgb) => {
            format!("{:02X}{:02X}{:02X}", rgb.r, rgb.g, rgb.b)
        }
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

fn latex_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\textbackslash{}"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            _ => out.push(ch),
        }
    }
    out
}

fn line_to_text(line: &Line) -> String {
    line_to_text_raw(line).trim_end().to_string()
}

fn line_to_text_raw(line: &Line) -> String {
    line
        .cells()
        .iter()
        .filter(|c| c.width() > 0)
        .map(|c| {
            let ch = c.char();
            if ch == '\0' { ' ' } else { ch }
        })
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

    fn make_line(ansi_str: &str, cols: usize) -> Line {
        let mut vt = avt::Vt::builder().size(cols, 1).build();
        vt.feed_str(ansi_str);
        let line = vt.view().next().unwrap().clone();
        line
    }

    #[test]
    fn latex_escape_backslash() {
        assert_eq!(latex_escape("a\\b"), "a\\textbackslash{}b");
    }

    #[test]
    fn latex_escape_braces() {
        assert_eq!(latex_escape("{x}"), "\\{x\\}");
    }

    #[test]
    fn latex_escape_combined() {
        assert_eq!(latex_escape("\\{"), "\\textbackslash{}\\{");
    }

    #[test]
    fn latex_escape_passthrough() {
        assert_eq!(latex_escape("hello"), "hello");
    }

    #[test]
    fn color_to_hex_rgb() {
        let c = Color::rgb(255, 0, 128);
        assert_eq!(color_to_hex(c), "FF0080");
    }

    #[test]
    fn color_to_hex_indexed_0_black() {
        let c = Color::Indexed(0);
        assert_eq!(color_to_hex(c), "000000");
    }

    #[test]
    fn color_to_hex_indexed_1_red() {
        let c = Color::Indexed(1);
        assert_eq!(color_to_hex(c), "CD3131");
    }

    #[test]
    fn color_to_hex_indexed_6cube() {
        let c = Color::Indexed(16);
        assert_eq!(color_to_hex(c), "000000");
    }

    #[test]
    fn color_to_hex_indexed_grayscale() {
        let c = Color::Indexed(232);
        assert_eq!(color_to_hex(c), "080808");
    }

    #[test]
    fn css_fontsize_to_latex_none() {
        assert_eq!(css_fontsize_to_latex(None), "");
    }

    #[test]
    fn css_fontsize_to_latex_06em() {
        assert_eq!(css_fontsize_to_latex(Some("0.6em")), "\\tiny\n");
    }

    #[test]
    fn css_fontsize_to_latex_08em() {
        assert_eq!(css_fontsize_to_latex(Some("0.8em")), "\\small\n");
    }

    #[test]
    fn css_fontsize_to_latex_1em() {
        assert_eq!(css_fontsize_to_latex(Some("1em")), "\\normalsize\n");
    }

    #[test]
    fn css_fontsize_to_latex_fallback() {
        assert_eq!(css_fontsize_to_latex(Some("13pt")), "\\small\n");
    }

    #[test]
    fn render_lines_to_latex_empty() {
        let theme = LatexTheme { bg: None, fg: None, font_size: None, font_family: None, line_height: None };
        assert_eq!(render_lines_to_latex(&[], &theme), "");
    }

    #[test]
    fn render_lines_to_latex_wraps_tcolorbox() {
        let lines = vec![
            RenderedLine { html: "hello".to_string(), text: "hello".to_string() },
        ];
        let theme = LatexTheme { bg: None, fg: None, font_size: None, font_family: None, line_height: None };
        let result = render_lines_to_latex(&lines, &theme);
        assert!(result.contains("\\begin{tcolorbox}"));
        assert!(result.contains("\\begin{Verbatim}"));
        assert!(result.contains("hello"));
        assert!(result.contains("\\end{Verbatim}"));
        assert!(result.contains("\\end{tcolorbox}"));
    }

    #[test]
    fn render_lines_to_latex_with_theme() {
        let lines = vec![
            RenderedLine { html: "x".to_string(), text: "x".to_string() },
        ];
        let theme = LatexTheme { bg: Some("#1a1b26"), fg: Some("#c0caf5"), font_size: Some("0.8em"), font_family: None, line_height: None };
        let result = render_lines_to_latex(&lines, &theme);
        assert!(result.contains("colback=termbg"));
        assert!(result.contains("colupper=termfg"));
        assert!(result.contains("\\small"));
    }

    #[test]
    fn render_lines_to_latex_with_font_family() {
        let lines = vec![
            RenderedLine { html: "x".to_string(), text: "x".to_string() },
        ];
        let theme = LatexTheme { bg: None, fg: None, font_size: None, font_family: Some("Fira Code, monospace"), line_height: None };
        let result = render_lines_to_latex(&lines, &theme);
        assert!(result.contains("\\setmonofont{Fira Code}"));
    }

    #[test]
    fn render_lines_to_latex_with_line_height() {
        let lines = vec![
            RenderedLine { html: "x".to_string(), text: "x".to_string() },
        ];
        let theme = LatexTheme { bg: None, fg: None, font_size: None, font_family: None, line_height: Some("1.4") };
        let result = render_lines_to_latex(&lines, &theme);
        assert!(result.contains("\\linespread{1.4}\\selectfont"));
    }

    #[test]
    fn render_line_plain_no_ansi() {
        let line = make_line("hello", 80);
        let result = render_line(&line, false, false);
        assert_eq!(result.html, "hello");
        assert_eq!(result.text, "hello");
    }

    #[test]
    fn render_line_escapes_latex() {
        let line = make_line("a\\{b}", 80);
        let result = render_line(&line, false, false);
        assert!(result.html.contains("\\textbackslash{}"));
        assert!(result.html.contains("\\{"));
        assert!(result.html.contains("\\}"));
    }

    #[test]
    fn render_line_bold_ansi() {
        let line = make_line("\x1b[1mBOLD\x1b[0m", 80);
        let result = render_line(&line, true, false);
        assert!(result.html.contains("\\textbf{BOLD}"));
    }

    #[test]
    fn render_line_colored_ansi() {
        let line = make_line("\x1b[31mred\x1b[0m", 80);
        let result = render_line(&line, true, false);
        assert!(result.html.contains("\\textcolor[HTML]{CD3131}{red}"));
    }
}
