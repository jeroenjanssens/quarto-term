use avt::{Color, Line, Pen};

use crate::renderer::RenderedLine;

pub fn render_line(line: &Line, ansi: bool) -> RenderedLine {
    let text = line_to_text(line);

    if !ansi {
        return RenderedLine {
            html: latex_escape(&text),
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

    let latex = latex.trim_end().to_string();

    RenderedLine { html: latex, text }
}

pub fn render_lines_to_latex(lines: &[RenderedLine], _css_class: &str, fontsize: Option<&str>) -> String {
    if lines.is_empty() {
        return String::new();
    }

    let inner: String = lines
        .iter()
        .map(|l| l.html.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    let font_cmd = css_fontsize_to_latex(fontsize);
    format!(
        "\\begin{{tcolorbox}}[colback=black!5!white,colframe=black!50!white,boxrule=0.5pt,arc=3pt,left=6pt,right=6pt,top=4pt,bottom=4pt]\n\
         {font_cmd}\\begin{{Verbatim}}[commandchars=\\\\\\{{\\}},breaklines=true]\n\
         {inner}\n\
         \\end{{Verbatim}}\n\
         \\end{{tcolorbox}}\n"
    )
}

pub fn render_fullscreen_to_latex(lines: &[RenderedLine], fontsize: Option<&str>) -> String {
    if lines.is_empty() {
        return String::new();
    }

    let inner: String = lines
        .iter()
        .map(|l| l.html.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    let font_cmd = css_fontsize_to_latex(fontsize);
    format!(
        "\\begin{{tcolorbox}}[colback=black!90!white,colframe=black!70!white,colupper=white,boxrule=0.5pt,arc=3pt,left=6pt,right=6pt,top=4pt,bottom=4pt]\n\
         {font_cmd}\\begin{{Verbatim}}[commandchars=\\\\\\{{\\}},breaklines=true]\n\
         {inner}\n\
         \\end{{Verbatim}}\n\
         \\end{{tcolorbox}}\n"
    )
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
