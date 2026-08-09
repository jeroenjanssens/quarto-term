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
        html: text.to_string(),
        text: text.to_string(),
    }
}

pub fn rendered_html(html: &str, text: &str) -> RenderedLine {
    RenderedLine {
        html: html.to_string(),
        text: text.to_string(),
    }
}

