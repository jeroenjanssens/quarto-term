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

    let mut runs = String::new();
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
                runs.push_str(&format!("<w:r><w:t xml:space=\"preserve\">{escaped}</w:t></w:r>"));
            } else {
                let rpr = run_properties(pen);
                runs.push_str(&format!("<w:r>{rpr}<w:t xml:space=\"preserve\">{escaped}</w:t></w:r>"));
            }
        }

        i = j;
    }

    if !trailing_spaces {
        // Trimming trailing spaces in OpenXML runs is complex; the text field handles it
    }

    RenderedLine { html: runs, text }
}

pub struct DocxTheme<'a> {
    pub bg: Option<&'a str>,
    pub fg: Option<&'a str>,
    pub font_size: Option<&'a str>,
    pub font_family: Option<&'a str>,
    pub line_height: Option<&'a str>,
}

pub fn render_lines_to_docx(lines: &[RenderedLine], theme: &DocxTheme) -> String {
    if lines.is_empty() {
        return String::new();
    }

    let ppr = paragraph_properties(theme);

    let mut paragraphs = String::new();
    for line in lines {
        if line.html.is_empty() {
            paragraphs.push_str(&format!("<w:p>{ppr}</w:p>"));
        } else {
            paragraphs.push_str(&format!("<w:p>{ppr}{}</w:p>", line.html));
        }
    }

    let shading = theme.bg.map(|bg| {
        let bg_clean = bg.trim_start_matches('#');
        format!(
            "<w:tcPr><w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{bg_clean}\"/></w:tcPr>"
        )
    }).unwrap_or_default();

    format!(
        "<w:tbl>\
         <w:tblPr>\
         <w:tblW w:w=\"5000\" w:type=\"pct\"/>\
         <w:tblBorders>\
         <w:top w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"D0D0D0\"/>\
         <w:left w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"D0D0D0\"/>\
         <w:bottom w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"D0D0D0\"/>\
         <w:right w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"D0D0D0\"/>\
         </w:tblBorders>\
         <w:tblCellMar><w:top w:w=\"80\" w:type=\"dxa\"/><w:left w:w=\"120\" w:type=\"dxa\"/><w:bottom w:w=\"80\" w:type=\"dxa\"/><w:right w:w=\"120\" w:type=\"dxa\"/></w:tblCellMar>\
         </w:tblPr>\
         <w:tr><w:tc>\
         {shading}\
         {paragraphs}\
         </w:tc></w:tr>\
         </w:tbl>"
    )
}

pub fn render_fullscreen_to_docx(lines: &[RenderedLine], theme: &DocxTheme) -> String {
    render_lines_to_docx(lines, theme)
}

fn paragraph_properties(theme: &DocxTheme) -> String {
    let mut rpr_parts = Vec::new();

    let font_name = theme.font_family
        .map(|f| f.split(',').next().unwrap_or(f).trim().trim_matches('"').trim_matches('\''))
        .unwrap_or("Courier New");

    rpr_parts.push(format!(
        "<w:rFonts w:ascii=\"{font_name}\" w:hAnsi=\"{font_name}\" w:cs=\"{font_name}\"/>"
    ));

    if let Some(fs) = theme.font_size {
        if let Some(half_pts) = css_size_to_half_points(fs) {
            rpr_parts.push(format!("<w:sz w:val=\"{half_pts}\"/><w:szCs w:val=\"{half_pts}\"/>"));
        }
    }

    if let Some(fg) = theme.fg {
        let fg_clean = fg.trim_start_matches('#');
        rpr_parts.push(format!("<w:color w:val=\"{fg_clean}\"/>"));
    }

    let mut ppr_parts = Vec::new();
    if let Some(lh) = theme.line_height {
        if let Some(twips) = line_height_to_twips(lh, theme.font_size) {
            ppr_parts.push(format!("<w:spacing w:line=\"{twips}\" w:lineRule=\"exact\"/>"));
        }
    }

    let mut ppr = String::from("<w:pPr>");
    ppr.push_str(&format!("<w:rPr>{}</w:rPr>", rpr_parts.join("")));
    for part in &ppr_parts {
        ppr.push_str(part);
    }
    ppr.push_str("</w:pPr>");

    ppr
}

fn run_properties(pen: &Pen) -> String {
    let mut parts = Vec::new();

    let fg = if pen.is_inverse() {
        pen.background()
    } else {
        pen.foreground()
    };

    if let Some(color) = fg {
        let hex = color_to_hex(color);
        parts.push(format!("<w:color w:val=\"{hex}\"/>"));
    }

    if pen.is_bold() {
        parts.push("<w:b/>".to_string());
    }
    if pen.is_italic() {
        parts.push("<w:i/>".to_string());
    }
    if pen.is_underline() {
        parts.push("<w:u w:val=\"single\"/>".to_string());
    }
    if pen.is_strikethrough() {
        parts.push("<w:strike/>".to_string());
    }

    if parts.is_empty() {
        String::new()
    } else {
        format!("<w:rPr>{}</w:rPr>", parts.join(""))
    }
}

fn css_size_to_half_points(s: &str) -> Option<u32> {
    let s = s.trim();
    if let Some(em) = s.strip_suffix("em") {
        if let Ok(val) = em.parse::<f64>() {
            return Some((val * 22.0).round() as u32);
        }
    }
    if let Some(pt) = s.strip_suffix("pt") {
        if let Ok(val) = pt.parse::<f64>() {
            return Some((val * 2.0).round() as u32);
        }
    }
    None
}

fn line_height_to_twips(lh: &str, font_size: Option<&str>) -> Option<u32> {
    let lh_val = lh.trim().parse::<f64>().ok()?;
    let base_pt = font_size
        .and_then(|fs| {
            fs.strip_suffix("em")
                .and_then(|v| v.parse::<f64>().ok())
                .map(|v| v * 11.0)
                .or_else(|| fs.strip_suffix("pt").and_then(|v| v.parse::<f64>().ok()))
        })
        .unwrap_or(11.0);
    Some((lh_val * base_pt * 20.0).round() as u32)
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

    #[test]
    fn xml_escape_basic() {
        assert_eq!(xml_escape("<a>&\"b\"</a>"), "&lt;a&gt;&amp;&quot;b&quot;&lt;/a&gt;");
    }

    #[test]
    fn xml_escape_passthrough() {
        assert_eq!(xml_escape("hello"), "hello");
    }

    #[test]
    fn css_size_to_half_points_em() {
        assert_eq!(css_size_to_half_points("0.8em"), Some(18));
    }

    #[test]
    fn css_size_to_half_points_pt() {
        assert_eq!(css_size_to_half_points("10pt"), Some(20));
    }

    #[test]
    fn css_size_to_half_points_invalid() {
        assert_eq!(css_size_to_half_points("large"), None);
    }

    #[test]
    fn line_height_to_twips_basic() {
        // 1.2 * 11pt * 20 = 264
        assert_eq!(line_height_to_twips("1.2", None), Some(264));
    }

    #[test]
    fn line_height_to_twips_with_font_size() {
        // 1.2 * (0.8 * 11) * 20 = 211.2 -> 211
        assert_eq!(line_height_to_twips("1.2", Some("0.8em")), Some(211));
    }

    #[test]
    fn render_lines_to_docx_empty() {
        let theme = DocxTheme { bg: None, fg: None, font_size: None, font_family: None, line_height: None };
        assert_eq!(render_lines_to_docx(&[], &theme), "");
    }

    #[test]
    fn render_lines_to_docx_basic() {
        let lines = vec![
            RenderedLine { html: "<w:r><w:t xml:space=\"preserve\">hello</w:t></w:r>".to_string(), text: "hello".to_string() },
        ];
        let theme = DocxTheme { bg: None, fg: None, font_size: None, font_family: None, line_height: None };
        let result = render_lines_to_docx(&lines, &theme);
        assert!(result.contains("<w:tbl>"));
        assert!(result.contains("hello"));
        assert!(result.contains("Courier New"));
    }

    #[test]
    fn render_lines_to_docx_with_theme() {
        let lines = vec![
            RenderedLine { html: "<w:r><w:t xml:space=\"preserve\">x</w:t></w:r>".to_string(), text: "x".to_string() },
        ];
        let theme = DocxTheme { bg: Some("1a1b26"), fg: Some("c0caf5"), font_size: Some("0.8em"), font_family: Some("Fira Code"), line_height: None };
        let result = render_lines_to_docx(&lines, &theme);
        assert!(result.contains("w:fill=\"1a1b26\""));
        assert!(result.contains("Fira Code"));
        assert!(result.contains("w:val=\"c0caf5\""));
    }
}
