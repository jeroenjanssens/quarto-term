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

    let fence = fence_for_content(&inner);
    format!("{fence}console\n{inner}\n{fence}\n")
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

    let fence = fence_for_content(&inner);
    format!("{fence}text\n{inner}\n{fence}\n")
}

fn fence_for_content(content: &str) -> String {
    let mut max_run = 0;
    let mut current_run = 0;
    for ch in content.chars() {
        if ch == '`' {
            current_run += 1;
            if current_run > max_run {
                max_run = current_run;
            }
        } else {
            current_run = 0;
        }
    }
    let fence_len = if max_run < 3 { 3 } else { max_run + 1 };
    "`".repeat(fence_len)
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

    #[test]
    fn fence_for_content_no_backticks() {
        assert_eq!(fence_for_content("hello world"), "```");
    }

    #[test]
    fn fence_for_content_with_triple_backticks() {
        assert_eq!(fence_for_content("some ```code``` here"), "````");
    }

    #[test]
    fn fence_for_content_with_long_run() {
        assert_eq!(fence_for_content("a ````` b"), "``````");
    }

    #[test]
    fn render_lines_with_backticks_in_content() {
        let lines = vec![make_line("echo ```hello```")];
        let result = render_lines_to_markdown(&lines);
        assert!(result.starts_with("````console\n"));
        assert!(result.ends_with("\n````\n"));
    }
}
