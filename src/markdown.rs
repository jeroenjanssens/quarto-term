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

    format!("```\n{inner}\n```\n")
}

pub fn render_fullscreen_to_markdown(lines: &[RenderedLine]) -> String {
    render_lines_to_markdown(lines)
}
