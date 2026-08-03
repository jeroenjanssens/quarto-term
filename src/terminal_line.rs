use avt::{Line, Pen};

pub fn line_to_text(line: &Line) -> String {
    line_to_text_raw(line).trim_end().to_string()
}

pub fn line_to_text_raw(line: &Line) -> String {
    line.cells()
        .iter()
        .filter(|c| c.width() > 0)
        .map(|c| {
            let ch = c.char();
            if ch == '\0' { ' ' } else { ch }
        })
        .collect()
}

pub fn pens_equal(a: &Pen, b: &Pen) -> bool {
    a.foreground() == b.foreground()
        && a.background() == b.background()
        && a.is_bold() == b.is_bold()
        && a.is_faint() == b.is_faint()
        && a.is_italic() == b.is_italic()
        && a.is_underline() == b.is_underline()
        && a.is_strikethrough() == b.is_strikethrough()
        && a.is_inverse() == b.is_inverse()
}

pub struct StyledRun {
    pub text: String,
    pub is_default: bool,
    pub pen_idx: usize,
}

pub fn styled_runs(line: &Line) -> (Vec<StyledRun>, Vec<Pen>) {
    let cells: Vec<_> = line.cells().iter().collect();
    let mut runs = Vec::new();
    let mut pens = Vec::new();

    let mut i = 0;
    while i < cells.len() {
        let pen = cells[i].pen().clone();
        let mut chunk_text = String::new();

        let mut j = i;
        while j < cells.len() && pens_equal(cells[j].pen(), &pen) {
            if cells[j].width() > 0 {
                let ch = cells[j].char();
                chunk_text.push(if ch == '\0' { ' ' } else { ch });
            }
            j += 1;
        }

        if !chunk_text.is_empty() {
            let pen_idx = pens.len();
            let is_default = pen.is_default();
            pens.push(pen);
            runs.push(StyledRun { text: chunk_text, is_default, pen_idx });
        }

        i = j;
    }

    (runs, pens)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_line(ansi_str: &str, cols: usize) -> Line {
        let mut vt = avt::Vt::builder().size(cols, 1).build();
        vt.feed_str(ansi_str);
        let lines: Vec<Line> = vt.view().cloned().collect();
        lines.into_iter().next().unwrap()
    }

    #[test]
    fn line_to_text_trims_trailing() {
        let line = make_line("hello", 80);
        assert_eq!(line_to_text(&line), "hello");
    }

    #[test]
    fn line_to_text_raw_preserves_trailing() {
        let line = make_line("hi", 10);
        let raw = line_to_text_raw(&line);
        assert_eq!(raw.len(), 10);
        assert!(raw.starts_with("hi"));
    }

    #[test]
    fn styled_runs_plain_text() {
        let line = make_line("hello", 80);
        let (runs, _pens) = styled_runs(&line);
        assert_eq!(runs.len(), 1);
        assert!(runs[0].text.starts_with("hello"));
        assert!(runs[0].is_default);
    }

    #[test]
    fn styled_runs_with_color() {
        let line = make_line("\x1b[31mred\x1b[0m normal", 80);
        let (runs, _pens) = styled_runs(&line);
        assert!(runs.len() >= 2);
        assert_eq!(runs[0].text, "red");
        assert!(!runs[0].is_default);
    }

    #[test]
    fn pens_equal_default() {
        let pen = Pen::default();
        assert!(pens_equal(&pen, &pen));
    }
}
