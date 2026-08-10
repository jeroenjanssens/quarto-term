use avt::Line;
use crate::renderer::RenderedLine;

pub fn make_line(ansi_str: &str, cols: usize) -> Line {
    let mut vt = avt::Vt::builder().size(cols, 1).build();
    vt.feed_str(ansi_str);
    let line = vt.view().next().unwrap().clone();
    line
}

pub fn rendered(text: &str) -> RenderedLine {
    RenderedLine {
        markup: text.to_string(),
        text: text.to_string(),
        is_prompt: false,
    }
}

pub fn rendered_markup(markup: &str, text: &str) -> RenderedLine {
    RenderedLine {
        markup: markup.to_string(),
        text: text.to_string(),
        is_prompt: false,
    }
}

pub fn rendered_lines(texts: &[&str]) -> Vec<RenderedLine> {
    texts.iter().map(|t| rendered(t)).collect()
}

