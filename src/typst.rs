use avt::{Color, Line, Pen};

use crate::color;
use crate::render_backend::{self, RenderBackend};
use crate::renderer::{self, RenderedLine};

struct TypstBackend;

impl RenderBackend for TypstBackend {
    fn escape(&self, text: &str) -> String {
        typst_escape(text)
    }

    fn wrap_styled(&self, escaped_text: &str, pen: &Pen) -> String {
        wrap_with_pen(escaped_text, pen)
    }
}

pub fn render_line(line: &Line, ansi: bool, trailing_spaces: bool) -> RenderedLine {
    render_backend::render_line_generic(&TypstBackend, line, ansi, trailing_spaces)
}

pub struct TypstTheme<'a> {
    pub bg: Option<&'a str>,
    pub fg: Option<&'a str>,
    pub font_size: Option<&'a str>,
    pub font_family: Option<&'a str>,
    pub line_height: Option<&'a str>,
}

pub fn render_lines_to_typst(lines: &[RenderedLine], theme: &TypstTheme) -> String {
    if lines.is_empty() {
        return String::new();
    }

    let has_markup = lines.iter().any(|l| l.markup.contains("#text(") || l.markup.contains("#strong[") || l.markup.contains("#emph[") || l.markup.contains("#underline["));

    let mut block_params = Vec::new();
    if let Some(bg) = theme.bg {
        if renderer::is_safe_hex_color(bg) {
            block_params.push(format!("fill: rgb(\"#{bg}\")"));
        }
    }
    block_params.push("radius: 4pt".to_string());
    block_params.push("inset: 8pt".to_string());
    block_params.push("width: 100%".to_string());

    let mut result = String::new();
    result.push_str(&format!("#block({})[", block_params.join(", ")));
    result.push('\n');

    let mut text_params = Vec::new();
    let font_name = theme.font_family
        .map(|f| f.split(',').next().unwrap_or(f).trim().trim_matches('"').trim_matches('\'').to_string())
        .filter(|name| renderer::is_safe_font_name(name))
        .unwrap_or_else(|| "Courier New".to_string());
    text_params.push(format!("font: \"{}\"", font_name));
    if let Some(fs) = theme.font_size {
        if let Some(pt) = css_size_to_typst(fs) {
            text_params.push(format!("size: {pt}"));
        }
    }
    if let Some(fg) = theme.fg {
        if renderer::is_safe_hex_color(fg) {
            text_params.push(format!("fill: rgb(\"#{fg}\")"));
        }
    }
    result.push_str(&format!("#set text({})\n", text_params.join(", ")));

    if let Some(lh) = theme.line_height {
        if let Ok(val) = lh.trim().parse::<f64>() {
            result.push_str(&format!("#set par(leading: {:.1}em)\n", val - 1.0));
        }
    }

    if has_markup {
        let inner: String = lines
            .iter()
            .map(|l| l.markup.as_str())
            .collect::<Vec<_>>()
            .join("\\\n");
        result.push_str(&inner);
        result.push('\n');
    } else {
        let inner: String = lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let max_backtick_run = inner
            .as_bytes()
            .split(|&b| b != b'`')
            .map(|run| run.len())
            .max()
            .unwrap_or(0);
        let fence_len = max_backtick_run.max(3) + 1;
        let fence: String = std::iter::repeat('`').take(fence_len).collect();
        result.push_str(&format!("{fence}\n{inner}\n{fence}\n"));
    }

    result.push_str("]\n");
    result
}

pub fn render_fullscreen_to_typst(lines: &[RenderedLine], theme: &TypstTheme) -> String {
    render_lines_to_typst(lines, theme)
}

fn css_size_to_typst(s: &str) -> Option<String> {
    let s = s.trim();
    if let Some(em) = s.strip_suffix("em") {
        if let Ok(val) = em.parse::<f64>() {
            return Some(format!("{:.2}em", val));
        }
    }
    if let Some(pt) = s.strip_suffix("pt") {
        if pt.parse::<f64>().is_ok() {
            return Some(format!("{}pt", pt));
        }
    }
    None
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
        result = format!("#text(fill: rgb(\"#{hex}\"))[{result}]");
    }

    if pen.is_bold() {
        result = format!("#strong[{result}]");
    }
    if pen.is_italic() {
        result = format!("#emph[{result}]");
    }
    if pen.is_underline() {
        result = format!("#underline[{result}]");
    }

    result
}

fn color_to_hex(color: Color) -> String {
    color::color_to_hex(color)
}

fn typst_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '#' => out.push_str("\\#"),
            '$' => out.push_str("\\$"),
            '_' => out.push_str("\\_"),
            '*' => out.push_str("\\*"),
            '@' => out.push_str("\\@"),
            '<' => out.push_str("\\<"),
            '>' => out.push_str("\\>"),
            '`' => out.push_str("\\`"),
            '[' => out.push_str("\\["),
            ']' => out.push_str("\\]"),
            _ => out.push(ch),
        }
    }
    out
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{rendered, rendered_html};

    #[test]
    fn typst_escape_special_chars() {
        assert_eq!(typst_escape("#hello $world"), "\\#hello \\$world");
        assert_eq!(typst_escape("[31m]"), "\\[31m\\]");
    }

    #[test]
    fn typst_escape_backslash() {
        assert_eq!(typst_escape("\\"), "\\\\");
        assert_eq!(typst_escape("path\\to\\file"), "path\\\\to\\\\file");
        assert_eq!(
            typst_escape(r#"\] \#read("Cargo.toml") \["#),
            r#"\\\] \\\#read("Cargo.toml") \\\["#
        );
    }

    #[test]
    fn typst_escape_passthrough() {
        assert_eq!(typst_escape("hello world"), "hello world");
    }

    #[test]
    fn css_size_to_typst_em() {
        assert_eq!(css_size_to_typst("0.8em"), Some("0.80em".to_string()));
    }

    #[test]
    fn css_size_to_typst_pt() {
        assert_eq!(css_size_to_typst("10pt"), Some("10pt".to_string()));
    }

    #[test]
    fn css_size_to_typst_invalid() {
        assert_eq!(css_size_to_typst("large"), None);
    }

    #[test]
    fn render_lines_to_typst_empty() {
        let theme = TypstTheme { bg: None, fg: None, font_size: None, font_family: None, line_height: None };
        assert_eq!(render_lines_to_typst(&[], &theme), "");
    }

    #[test]
    fn render_lines_to_typst_basic() {
        let theme = TypstTheme { bg: None, fg: None, font_size: None, font_family: None, line_height: None };
        let result = render_lines_to_typst(&[rendered("hello")], &theme);
        assert!(result.contains("#block("));
        assert!(result.contains("hello"));
    }

    #[test]
    fn render_lines_to_typst_with_theme() {
        let theme = TypstTheme { bg: Some("1a1b26"), fg: Some("c0caf5"), font_size: Some("0.8em"), font_family: Some("Fira Code"), line_height: Some("1.3") };
        let result = render_lines_to_typst(&[rendered("x")], &theme);
        assert!(result.contains("fill: rgb(\"#1a1b26\")"));
        assert!(result.contains("font: \"Fira Code\""));
        assert!(result.contains("size: 0.80em"));
        assert!(result.contains("fill: rgb(\"#c0caf5\")"));
        assert!(result.contains("leading: 0.3em"));
    }

    #[test]
    fn render_lines_to_typst_with_ansi_markup() {
        let lines = vec![rendered_html("#text(fill: rgb(\"#CD3131\"))[red]", "red")];
        let theme = TypstTheme { bg: Some("1a1b26"), fg: None, font_size: None, font_family: None, line_height: None };
        let result = render_lines_to_typst(&lines, &theme);
        assert!(result.contains("#text(fill: rgb(\"#CD3131\"))[red]"));
        assert!(!result.contains("```"));
    }

    #[test]
    fn render_lines_to_typst_plain_uses_raw() {
        let theme = TypstTheme { bg: None, fg: None, font_size: None, font_family: None, line_height: None };
        let result = render_lines_to_typst(&[rendered("hello")], &theme);
        assert!(result.contains("```"));
    }

    #[test]
    fn render_lines_to_typst_backtick_injection() {
        let lines = vec![rendered("``` #read(\"/etc/passwd\")")];
        let theme = TypstTheme { bg: None, fg: None, font_size: None, font_family: None, line_height: None };
        let result = render_lines_to_typst(&lines, &theme);
        assert!(result.contains("````"));
        assert!(result.contains("``` #read"));
    }

    #[test]
    fn render_lines_to_typst_long_backtick_run() {
        let lines = vec![rendered("`````test`````")];
        let theme = TypstTheme { bg: None, fg: None, font_size: None, font_family: None, line_height: None };
        let result = render_lines_to_typst(&lines, &theme);
        assert!(result.contains("``````"));
    }
}
