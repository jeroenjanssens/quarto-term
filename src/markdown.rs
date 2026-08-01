use crate::renderer::RenderedLine;

pub fn render_lines_to_markdown(lines: &[RenderedLine]) -> String {
    if lines.is_empty() {
        return String::new();
    }

    let inner: String = lines
        .iter()
        .map(|l| l.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    format!("```console\n{inner}\n```\n")
}

pub fn render_fullscreen_to_markdown(lines: &[RenderedLine]) -> String {
    if lines.is_empty() {
        return String::new();
    }

    let inner: String = lines
        .iter()
        .map(|l| l.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    format!("```text\n{inner}\n```\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_line(text: &str) -> RenderedLine {
        RenderedLine { html: format!("<b>{}</b>", text), text: text.to_string() }
    }

    #[test]
    fn render_lines_to_markdown_empty() {
        assert_eq!(render_lines_to_markdown(&[]), "");
    }

    #[test]
    fn render_lines_to_markdown_uses_text() {
        let lines = vec![make_line("$ echo hi"), make_line("hi")];
        let result = render_lines_to_markdown(&lines);
        assert_eq!(result, "```console\n$ echo hi\nhi\n```\n");
        assert!(!result.contains("<b>"));
    }

    #[test]
    fn render_fullscreen_to_markdown_empty() {
        assert_eq!(render_fullscreen_to_markdown(&[]), "");
    }

    #[test]
    fn render_fullscreen_to_markdown_uses_text_fence() {
        let lines = vec![make_line("screen")];
        let result = render_fullscreen_to_markdown(&lines);
        assert!(result.starts_with("```text\n"));
    }
}
