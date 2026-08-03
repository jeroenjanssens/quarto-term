use avt::{Color, Line, Pen};

use crate::color;
use crate::terminal_line;

pub struct RenderedLine {
    pub html: String,
    pub text: String,
}

pub fn render_line(line: &Line, ansi: bool, trailing_spaces: bool) -> RenderedLine {
    let text = terminal_line::line_to_text(line);

    if !ansi {
        let html = if trailing_spaces {
            html_escape(&terminal_line::line_to_text_raw(line))
        } else {
            html_escape(&text)
        };
        return RenderedLine { html, text };
    }

    let (runs, pens) = terminal_line::styled_runs(line);
    if runs.is_empty() {
        return RenderedLine {
            html: String::new(),
            text,
        };
    }

    let mut html = String::new();

    for run in &runs {
        if run.is_default {
            html.push_str(&html_escape(&run.text));
        } else {
            let style = pen_to_style(&pens[run.pen_idx]);
            if style.is_empty() {
                html.push_str(&html_escape(&run.text));
            } else {
                html.push_str("<span style=\"");
                html.push_str(&style);
                html.push_str("\">");
                html.push_str(&html_escape(&run.text));
                html.push_str("</span>");
            }
        }
    }

    let html = if trailing_spaces { html } else { trim_trailing_spaces_html(&html) };

    RenderedLine { html, text }
}

pub struct HtmlStyle<'a> {
    pub font_size: Option<&'a str>,
    pub font_family: Option<&'a str>,
    pub line_height: Option<&'a str>,
}

impl<'a> HtmlStyle<'a> {
    pub fn to_attr(&self) -> String {
        let mut parts = Vec::new();
        if let Some(fs) = self.font_size {
            if is_safe_css_value(fs) {
                parts.push(format!("font-size:{fs}"));
            }
        }
        if let Some(f) = self.font_family {
            if is_safe_css_value(f) {
                parts.push(format!("font-family:{f}"));
            }
        }
        if let Some(lh) = self.line_height {
            if is_safe_css_value(lh) {
                parts.push(format!("line-height:{lh}"));
            }
        }
        if parts.is_empty() {
            String::new()
        } else {
            format!(" style=\"{}\"", parts.join(";"))
        }
    }
}

pub fn is_safe_css_value(s: &str) -> bool {
    !s.contains('"') && !s.contains('\'') && !s.contains(';')
        && !s.contains('<') && !s.contains('>') && !s.contains('}')
        && !s.contains('{') && !s.contains('\\')
}

pub fn is_safe_language_name(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'+' || b == b'-' || b == b'.')
}

pub fn is_safe_font_name(s: &str) -> bool {
    !s.is_empty() && s.len() <= 128
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b' ' || b == b'-' || b == b'_' || b == b'.')
}

pub fn is_safe_hex_color(s: &str) -> bool {
    (s.len() == 6 || s.len() == 7)
        && s.strip_prefix('#').unwrap_or(s).bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn render_lines_to_html(lines: &[RenderedLine], css_class: &str, style: &HtmlStyle) -> String {
    if lines.is_empty() {
        return String::new();
    }

    let inner: String = lines
        .iter()
        .map(|l| l.html.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    let style_attr = style.to_attr();
    format!("<pre class=\"{css_class}\"{style_attr}><code>{inner}</code></pre>\n")
}

pub fn render_fullscreen_to_html(lines: &[RenderedLine], style: &HtmlStyle) -> String {
    if lines.is_empty() {
        return String::new();
    }

    let inner: String = lines
        .iter()
        .map(|l| l.html.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    let style_attr = style.to_attr();
    format!("<pre class=\"term-screen\"{style_attr}><code>{inner}</code></pre>\n")
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
        Color::Indexed(i) => {
            let (r, g, b) = color::indexed_color_rgb(i);
            format!("#{:02x}{:02x}{:02x}", r, g, b)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_line(ansi_str: &str, cols: usize) -> Line {
        let mut vt = avt::Vt::builder().size(cols, 1).build();
        vt.feed_str(ansi_str);
        let line = vt.view().next().unwrap().clone();
        line
    }

    #[test]
    fn html_escape_ampersand() {
        assert_eq!(html_escape("a&b"), "a&amp;b");
    }

    #[test]
    fn html_escape_lt_gt() {
        assert_eq!(html_escape("<tag>"), "&lt;tag&gt;");
    }

    #[test]
    fn html_escape_quote() {
        assert_eq!(html_escape(r#"say "hi""#), "say &quot;hi&quot;");
    }

    #[test]
    fn html_escape_passthrough() {
        assert_eq!(html_escape("hello world"), "hello world");
    }

    #[test]
    fn html_escape_empty() {
        assert_eq!(html_escape(""), "");
    }

    #[test]
    fn trim_trailing_spaces_removes_trailing() {
        assert_eq!(trim_trailing_spaces_html("hello   "), "hello");
    }

    #[test]
    fn trim_trailing_spaces_preserves_leading() {
        assert_eq!(trim_trailing_spaces_html("  hello"), "  hello");
    }

    #[test]
    fn trim_trailing_spaces_noop() {
        assert_eq!(trim_trailing_spaces_html("hello"), "hello");
    }

    #[test]
    fn color_to_css_rgb() {
        let color = Color::rgb(255, 128, 0);
        assert_eq!(color_to_css(color), "#ff8000");
    }

    #[test]
    fn color_to_css_indexed_0() {
        let color = Color::Indexed(0);
        assert_eq!(color_to_css(color), "var(--term-0)");
    }

    #[test]
    fn color_to_css_indexed_15() {
        let color = Color::Indexed(15);
        assert_eq!(color_to_css(color), "var(--term-15)");
    }

    #[test]
    fn color_to_css_indexed_6cube_first() {
        let color = Color::Indexed(16);
        assert_eq!(color_to_css(color), "#000000");
    }

    #[test]
    fn color_to_css_indexed_6cube_red() {
        // index 196 = 16 + 5*36 + 0*6 + 0 = pure red in 6-cube
        let color = Color::Indexed(196);
        assert_eq!(color_to_css(color), "#ff0000");
    }

    #[test]
    fn color_to_css_indexed_grayscale_first() {
        let color = Color::Indexed(232);
        assert_eq!(color_to_css(color), "#080808");
    }

    #[test]
    fn color_to_css_indexed_grayscale_last() {
        let color = Color::Indexed(255);
        // 8 + 10 * (255-232) = 8 + 230 = 238
        assert_eq!(color_to_css(color), "#eeeeee");
    }

    #[test]
    fn render_line_plain_text_no_ansi() {
        let line = make_line("hello", 80);
        let result = render_line(&line, false, false);
        assert_eq!(result.text, "hello");
        assert_eq!(result.html, "hello");
    }

    #[test]
    fn render_line_html_escape_no_ansi() {
        let line = make_line("<b>&</b>", 80);
        let result = render_line(&line, false, false);
        assert_eq!(result.html, "&lt;b&gt;&amp;&lt;/b&gt;");
    }

    #[test]
    fn render_line_bold_ansi() {
        let line = make_line("\x1b[1mBOLD\x1b[0m", 80);
        let result = render_line(&line, true, false);
        assert!(result.html.contains("font-weight:bold"));
        assert!(result.html.contains("BOLD"));
    }

    #[test]
    fn render_line_italic_ansi() {
        let line = make_line("\x1b[3mitalic\x1b[0m", 80);
        let result = render_line(&line, true, false);
        assert!(result.html.contains("font-style:italic"));
    }

    #[test]
    fn render_line_fg_color_indexed() {
        let line = make_line("\x1b[31mred\x1b[0m", 80);
        let result = render_line(&line, true, false);
        assert!(result.html.contains("color:var(--term-1)"));
    }

    #[test]
    fn render_line_fg_color_rgb() {
        let line = make_line("\x1b[38;2;255;128;0morange\x1b[0m", 80);
        let result = render_line(&line, true, false);
        assert!(result.html.contains("color:#ff8000"));
    }

    #[test]
    fn render_line_underline_strikethrough() {
        let line = make_line("\x1b[4;9mboth\x1b[0m", 80);
        let result = render_line(&line, true, false);
        assert!(result.html.contains("text-decoration:underline line-through"));
    }

    #[test]
    fn render_line_inverse() {
        let line = make_line("\x1b[7minv\x1b[0m", 80);
        let result = render_line(&line, true, false);
        assert!(result.html.contains("var(--term-bg"));
        assert!(result.html.contains("var(--term-fg"));
    }

    #[test]
    fn render_lines_to_html_empty() {
        let style = HtmlStyle { font_size: None, font_family: None, line_height: None };
        let result = render_lines_to_html(&[], "term-output", &style);
        assert_eq!(result, "");
    }

    #[test]
    fn render_lines_to_html_wraps_correctly() {
        let lines = vec![
            RenderedLine { html: "line1".to_string(), text: "line1".to_string() },
            RenderedLine { html: "line2".to_string(), text: "line2".to_string() },
        ];
        let style = HtmlStyle { font_size: None, font_family: None, line_height: None };
        let result = render_lines_to_html(&lines, "term-output", &style);
        assert!(result.contains("<pre class=\"term-output\">"));
        assert!(result.contains("line1\nline2"));
        assert!(result.contains("</code></pre>"));
    }

    #[test]
    fn render_lines_to_html_with_font_size() {
        let lines = vec![
            RenderedLine { html: "x".to_string(), text: "x".to_string() },
        ];
        let style = HtmlStyle { font_size: Some("0.8em"), font_family: None, line_height: None };
        let result = render_lines_to_html(&lines, "term-output", &style);
        assert!(result.contains("style=\"font-size:0.8em\""));
    }

    #[test]
    fn render_lines_to_html_with_font_family() {
        let lines = vec![
            RenderedLine { html: "x".to_string(), text: "x".to_string() },
        ];
        let style = HtmlStyle { font_size: None, font_family: Some("Fira Code"), line_height: None };
        let result = render_lines_to_html(&lines, "term-output", &style);
        assert!(result.contains("style=\"font-family:Fira Code\""));
    }

    #[test]
    fn render_lines_to_html_with_line_height() {
        let lines = vec![
            RenderedLine { html: "x".to_string(), text: "x".to_string() },
        ];
        let style = HtmlStyle { font_size: None, font_family: None, line_height: Some("1.5") };
        let result = render_lines_to_html(&lines, "term-output", &style);
        assert!(result.contains("style=\"line-height:1.5\""));
    }

    #[test]
    fn render_lines_to_html_with_all_style_fields() {
        let lines = vec![
            RenderedLine { html: "x".to_string(), text: "x".to_string() },
        ];
        let style = HtmlStyle { font_size: Some("0.8em"), font_family: Some("Fira Code"), line_height: Some("1.4") };
        let result = render_lines_to_html(&lines, "term-output", &style);
        assert!(result.contains("font-size:0.8em"));
        assert!(result.contains("font-family:Fira Code"));
        assert!(result.contains("line-height:1.4"));
    }

    #[test]
    fn render_fullscreen_to_html_uses_screen_class() {
        let lines = vec![
            RenderedLine { html: "x".to_string(), text: "x".to_string() },
        ];
        let style = HtmlStyle { font_size: None, font_family: None, line_height: None };
        let result = render_fullscreen_to_html(&lines, &style);
        assert!(result.contains("<pre class=\"term-screen\">"));
    }

    #[test]
    fn is_safe_css_value_accepts_normal_values() {
        assert!(is_safe_css_value("0.8em"));
        assert!(is_safe_css_value("Fira Code, monospace"));
        assert!(is_safe_css_value("1.5"));
        assert!(is_safe_css_value("12px"));
    }

    #[test]
    fn is_safe_css_value_rejects_injection() {
        assert!(!is_safe_css_value("\" onmouseover=\"alert(1)"));
        assert!(!is_safe_css_value("'; background: url(evil)"));
        assert!(!is_safe_css_value("12px; color: red"));
        assert!(!is_safe_css_value("x<script>"));
        assert!(!is_safe_css_value("x}body{color:red"));
        assert!(!is_safe_css_value("a\\b"));
    }

    #[test]
    fn is_safe_language_name_accepts_valid() {
        assert!(is_safe_language_name("bash"));
        assert!(is_safe_language_name("c++"));
        assert!(is_safe_language_name("objective-c"));
        assert!(is_safe_language_name("html5"));
        assert!(is_safe_language_name("f.sharp"));
    }

    #[test]
    fn is_safe_language_name_rejects_injection() {
        assert!(!is_safe_language_name("x\" onmouseover=\"alert(1)"));
        assert!(!is_safe_language_name(""));
        assert!(!is_safe_language_name("a b"));
        assert!(!is_safe_language_name("<script>"));
    }

    #[test]
    fn html_style_rejects_unsafe_values() {
        let style = HtmlStyle {
            font_size: Some("\" onmouseover=\"alert(1)"),
            font_family: None,
            line_height: None,
        };
        assert_eq!(style.to_attr(), "");
    }

    #[test]
    fn is_safe_font_name_accepts_valid() {
        assert!(is_safe_font_name("Fira Code"));
        assert!(is_safe_font_name("Courier New"));
        assert!(is_safe_font_name("JetBrains Mono"));
        assert!(is_safe_font_name("SF-Mono"));
    }

    #[test]
    fn is_safe_font_name_rejects_injection() {
        assert!(!is_safe_font_name(""));
        assert!(!is_safe_font_name("font}\\input{/etc/passwd"));
        assert!(!is_safe_font_name("x\"][#read(\"/etc/passwd\")"));
    }

    #[test]
    fn is_safe_hex_color_accepts_valid() {
        assert!(is_safe_hex_color("1a1b26"));
        assert!(is_safe_hex_color("c0caf5"));
        assert!(is_safe_hex_color("#ff0080"));
        assert!(is_safe_hex_color("AABBCC"));
    }

    #[test]
    fn is_safe_hex_color_rejects_invalid() {
        assert!(!is_safe_hex_color(""));
        assert!(!is_safe_hex_color("red"));
        assert!(!is_safe_hex_color("1a1b2"));
        assert!(!is_safe_hex_color("1a1b26ff"));
        assert!(!is_safe_hex_color("\")[#read("));
    }
}
