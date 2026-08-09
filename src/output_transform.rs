use regex::Regex;

use crate::protocol::{CalloutValues, LineSpec, OutputFormat};
use crate::renderer::RenderedLine;

pub fn apply_spacing(lines: &mut Vec<RenderedLine>, prompt: &str) {
    let mut insertions = Vec::new();
    let prefix = format!("{} ", prompt);
    for (i, line) in lines.iter().enumerate() {
        if i > 0 && line.text.starts_with(&prefix) {
            insertions.push(i);
        }
    }
    for (offset, idx) in insertions.into_iter().enumerate() {
        lines.insert(idx + offset, RenderedLine {
            markup: String::new(),
            text: String::new(),
        });
    }
}

pub fn apply_remove(lines: &mut Vec<RenderedLine>, specs: &[LineSpec]) {
    if specs.is_empty() {
        return;
    }
    let to_remove = resolve_line_specs(specs, lines);
    for idx in to_remove.into_iter().rev() {
        lines.remove(idx);
    }
}

pub fn apply_truncate(lines: &mut Vec<RenderedLine>, specs: &[LineSpec], format: OutputFormat) {
    if specs.is_empty() {
        return;
    }
    let to_truncate = resolve_line_specs(specs, lines);
    if to_truncate.is_empty() {
        return;
    }

    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut run_start = to_truncate[0];
    let mut run_end = to_truncate[0];

    for &idx in &to_truncate[1..] {
        if idx == run_end + 1 {
            run_end = idx;
        } else {
            runs.push((run_start, run_end));
            run_start = idx;
            run_end = idx;
        }
    }
    runs.push((run_start, run_end));

    for &(start, end) in runs.iter().rev() {
        let count = end - start + 1;
        let msg = format_truncation_message(count, format);
        for idx in (start + 1..=end).rev() {
            lines.remove(idx);
        }
        lines[start] = msg;
    }
}

fn get_callout_value(values: &CalloutValues, index: usize) -> Option<String> {
    match values {
        CalloutValues::Str(s) => s.chars().nth(index).map(|c| c.to_string()),
        CalloutValues::List(v) => v.get(index).cloned(),
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

pub fn apply_callouts(
    lines: &mut Vec<RenderedLine>,
    specs: &[LineSpec],
    values: &CalloutValues,
    format_str: &str,
) {
    if specs.is_empty() {
        return;
    }

    for (n, spec) in specs.iter().enumerate() {
        let val = match get_callout_value(values, n) {
            Some(v) => v,
            None => continue,
        };
        let marker = html_escape(&format_str.replace("{}", &val));

        match spec {
            LineSpec::Index(i) => {
                if let Some(idx) = resolve_single_index(*i, lines.len()) {
                    if let Some(line) = lines.get_mut(idx) {
                        line.markup = format!(
                            "{} <span class=\"term-callout\">{}</span>",
                            line.markup, marker
                        );
                    }
                }
            }
            LineSpec::Expr(s) => {
                if is_range_syntax(s) {
                    let indices = parse_range_spec(s, lines.len());
                    if let Some(&idx) = indices.first() {
                        if let Some(line) = lines.get_mut(idx) {
                            line.markup = format!(
                                "{} <span class=\"term-callout\">{}</span>",
                                line.markup, marker
                            );
                        }
                    }
                } else if let Ok(re) = Regex::new(s) {
                    for line in lines.iter_mut() {
                        if re.is_match(&line.text) {
                            line.markup = format!(
                                "{} <span class=\"term-callout\">{}</span>",
                                line.markup, marker
                            );
                            break;
                        }
                    }
                }
            }
        }
    }
}

fn format_truncation_message(count: usize, format: OutputFormat) -> RenderedLine {
    let word = if count == 1 { "line" } else { "lines" };
    let text = format!("[{} {} truncated]", count, word);
    let markup = match format {
        OutputFormat::Latex => format!("\\textit{{{}}}", text),
        OutputFormat::Typst => format!("_{}_", text),
        OutputFormat::Markdown => format!("*{}*", text),
        OutputFormat::Html => format!("<span class=\"term-truncated\">{}</span>", text),
    };
    RenderedLine { markup, text }
}

fn resolve_line_specs(specs: &[LineSpec], lines: &[RenderedLine]) -> Vec<usize> {
    let len = lines.len();
    let mut indices = Vec::new();

    for spec in specs {
        match spec {
            LineSpec::Index(i) => {
                if let Some(idx) = resolve_single_index(*i, len) {
                    indices.push(idx);
                }
            }
            LineSpec::Expr(s) => {
                if let Ok(n) = s.parse::<i32>() {
                    if let Some(idx) = resolve_single_index(n, len) {
                        indices.push(idx);
                    }
                } else if is_range_syntax(s) {
                    indices.extend(parse_range_spec(s, len));
                } else if let Ok(re) = Regex::new(s) {
                    for (idx, line) in lines.iter().enumerate() {
                        if re.is_match(&line.text) {
                            indices.push(idx);
                        }
                    }
                }
            }
        }
    }

    indices.sort_unstable();
    indices.dedup();
    indices
}

fn resolve_single_index(i: i32, len: usize) -> Option<usize> {
    if i > 0 {
        let idx = (i as usize).saturating_sub(1);
        if idx < len { Some(idx) } else { None }
    } else if i < 0 {
        let idx = len as i32 + i;
        if idx >= 0 && (idx as usize) < len { Some(idx as usize) } else { None }
    } else {
        None
    }
}

fn parse_range_spec(s: &str, len: usize) -> Vec<usize> {
    let parts: Vec<&str> = s.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Vec::new();
    }

    let start_str = parts[0].trim();
    let end_str = parts[1].trim();

    let start = if start_str.is_empty() {
        0i32
    } else if let Ok(n) = start_str.parse::<i32>() {
        n
    } else {
        return Vec::new();
    };

    let end = if end_str.is_empty() {
        len as i32
    } else if let Ok(n) = end_str.parse::<i32>() {
        n
    } else {
        return Vec::new();
    };

    let resolve = |v: i32| -> usize {
        if v > 0 {
            (v as usize).saturating_sub(1)
        } else if v < 0 {
            let r = len as i32 + v;
            if r >= 0 { r as usize } else { 0 }
        } else {
            0
        }
    };

    let start_idx = resolve(start);
    let end_idx = if end_str.is_empty() {
        len
    } else {
        resolve(end) + 1
    };

    let start_idx = start_idx.min(len);
    let end_idx = end_idx.min(len);

    if start_idx >= end_idx {
        return Vec::new();
    }

    (start_idx..end_idx).collect()
}

fn is_range_syntax(s: &str) -> bool {
    s.contains(':') && s.bytes().all(|b| b.is_ascii_digit() || b == b':' || b == b'-' || b == b' ')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::rendered as make_rendered;

    // --- apply_spacing ---

    #[test]
    fn apply_spacing_single_command() {
        let mut lines = vec![make_rendered("$ echo hi"), make_rendered("hi")];
        apply_spacing(&mut lines, "$");
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn apply_spacing_two_commands() {
        let mut lines = vec![
            make_rendered("$ echo a"),
            make_rendered("a"),
            make_rendered("$ echo b"),
            make_rendered("b"),
        ];
        apply_spacing(&mut lines, "$");
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[2].text, "");
    }

    #[test]
    fn apply_spacing_custom_prompt() {
        let mut lines = vec![
            make_rendered("> cmd1"),
            make_rendered("out1"),
            make_rendered("> cmd2"),
        ];
        apply_spacing(&mut lines, ">");
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[2].text, "");
    }

    #[test]
    fn apply_spacing_no_prompt_prefix() {
        let mut lines = vec![make_rendered("abc"), make_rendered("def")];
        apply_spacing(&mut lines, "$");
        assert_eq!(lines.len(), 2);
    }

    // --- apply_remove ---

    #[test]
    fn apply_remove_empty_specs() {
        let mut lines = vec![make_rendered("a"), make_rendered("b")];
        apply_remove(&mut lines, &[]);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn apply_remove_positive_index() {
        let mut lines = vec![make_rendered("a"), make_rendered("b"), make_rendered("c")];
        apply_remove(&mut lines, &[LineSpec::Index(1)]);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "b");
    }

    #[test]
    fn apply_remove_negative_index() {
        let mut lines = vec![make_rendered("a"), make_rendered("b"), make_rendered("c")];
        apply_remove(&mut lines, &[LineSpec::Index(-1)]);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1].text, "b");
    }

    #[test]
    fn apply_remove_zero_index_skipped() {
        let mut lines = vec![make_rendered("a"), make_rendered("b")];
        apply_remove(&mut lines, &[LineSpec::Index(0)]);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn apply_remove_out_of_bounds() {
        let mut lines = vec![make_rendered("a")];
        apply_remove(&mut lines, &[LineSpec::Index(99)]);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn apply_remove_pattern() {
        let mut lines = vec![
            make_rendered("keep this"),
            make_rendered("remove_me"),
            make_rendered("also keep"),
        ];
        apply_remove(&mut lines, &[LineSpec::Expr("remove_me".to_string())]);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "keep this");
        assert_eq!(lines[1].text, "also keep");
    }

    #[test]
    fn apply_remove_multiple_specs() {
        let mut lines = vec![
            make_rendered("a"),
            make_rendered("b"),
            make_rendered("c"),
            make_rendered("d"),
        ];
        apply_remove(&mut lines, &[LineSpec::Index(1), LineSpec::Index(-1)]);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "b");
        assert_eq!(lines[1].text, "c");
    }

    // --- apply_callouts ---

    fn default_callout_values() -> CalloutValues {
        CalloutValues::Str("123456789".to_string())
    }

    #[test]
    fn apply_callouts_empty_specs() {
        let mut lines = vec![make_rendered("a")];
        apply_callouts(&mut lines, &[], &default_callout_values(), "<{}>");
        assert_eq!(lines[0].markup, "a");
    }

    #[test]
    fn apply_callouts_positive_index() {
        let mut lines = vec![make_rendered("a"), make_rendered("b")];
        apply_callouts(&mut lines, &[LineSpec::Index(1)], &default_callout_values(), "<{}>");
        assert!(lines[0].markup.contains("term-callout"));
        assert!(lines[0].markup.contains("&lt;1&gt;"));
        assert!(!lines[1].markup.contains("term-callout"));
    }

    #[test]
    fn apply_callouts_negative_index() {
        let mut lines = vec![make_rendered("a"), make_rendered("b"), make_rendered("c")];
        apply_callouts(&mut lines, &[LineSpec::Index(-1)], &default_callout_values(), "<{}>");
        assert!(lines[2].markup.contains("term-callout"));
        assert!(lines[2].markup.contains("&lt;1&gt;"));
    }

    #[test]
    fn apply_callouts_pattern() {
        let mut lines = vec![
            make_rendered("no match"),
            make_rendered("target line"),
            make_rendered("another"),
        ];
        apply_callouts(&mut lines, &[LineSpec::Expr("target".to_string())], &default_callout_values(), "<{}>");
        assert!(lines[1].markup.contains("term-callout"));
        assert!(!lines[2].markup.contains("term-callout"));
    }

    #[test]
    fn apply_callouts_sequential_numbering() {
        let mut lines = vec![make_rendered("a"), make_rendered("b"), make_rendered("c")];
        apply_callouts(&mut lines, &[LineSpec::Index(1), LineSpec::Index(3)], &default_callout_values(), "<{}>");
        assert!(lines[0].markup.contains("&lt;1&gt;"));
        assert!(lines[2].markup.contains("&lt;2&gt;"));
    }

    #[test]
    fn apply_callouts_custom_values_and_format() {
        let mut lines = vec![make_rendered("a"), make_rendered("b"), make_rendered("c")];
        let values = CalloutValues::List(vec!["one".to_string(), "two".to_string()]);
        apply_callouts(&mut lines, &[LineSpec::Index(1), LineSpec::Index(2)], &values, "[{}]");
        assert!(lines[0].markup.contains("[one]"));
        assert!(lines[1].markup.contains("[two]"));
    }

    #[test]
    fn apply_callouts_string_values() {
        let mut lines = vec![make_rendered("a"), make_rendered("b"), make_rendered("c")];
        let values = CalloutValues::Str("abc".to_string());
        apply_callouts(&mut lines, &[LineSpec::Index(1), LineSpec::Index(3)], &values, "({})");
        assert!(lines[0].markup.contains("(a)"));
        assert!(lines[2].markup.contains("(b)"));
    }

    // --- parse_range_spec ---

    #[test]
    fn parse_range_start_end() {
        assert_eq!(parse_range_spec("3:7", 10), vec![2, 3, 4, 5, 6]);
    }

    #[test]
    fn parse_range_open_start() {
        assert_eq!(parse_range_spec(":3", 10), vec![0, 1, 2]);
    }

    #[test]
    fn parse_range_open_end() {
        assert_eq!(parse_range_spec("8:", 10), vec![7, 8, 9]);
    }

    #[test]
    fn parse_range_negative_start() {
        assert_eq!(parse_range_spec("-3:", 10), vec![7, 8, 9]);
    }

    #[test]
    fn parse_range_negative_both() {
        assert_eq!(parse_range_spec("-5:-2", 10), vec![5, 6, 7, 8]);
    }

    #[test]
    fn parse_range_out_of_bounds_fully() {
        assert_eq!(parse_range_spec("8:20", 5), Vec::<usize>::new());
    }

    #[test]
    fn parse_range_partially_out_of_bounds() {
        assert_eq!(parse_range_spec("4:20", 5), vec![3, 4]);
    }

    #[test]
    fn parse_range_invalid() {
        assert_eq!(parse_range_spec("abc:def", 10), Vec::<usize>::new());
    }

    // --- resolve_line_specs with ranges ---

    #[test]
    fn resolve_specs_range() {
        let lines = vec![make_rendered("a"), make_rendered("b"), make_rendered("c"),
                         make_rendered("d"), make_rendered("e")];
        let specs = vec![LineSpec::Expr("2:4".to_string())];
        assert_eq!(resolve_line_specs(&specs, &lines), vec![1, 2, 3]);
    }

    #[test]
    fn resolve_specs_mixed() {
        let lines = vec![make_rendered("a"), make_rendered("b"), make_rendered("c"),
                         make_rendered("d"), make_rendered("e")];
        let specs = vec![LineSpec::Index(1), LineSpec::Expr("-1:".to_string())];
        assert_eq!(resolve_line_specs(&specs, &lines), vec![0, 4]);
    }

    #[test]
    fn resolve_specs_regex() {
        let lines = vec![make_rendered("hello"), make_rendered("world"), make_rendered("hello again")];
        let specs = vec![LineSpec::Expr("^hello".to_string())];
        assert_eq!(resolve_line_specs(&specs, &lines), vec![0, 2]);
    }

    // --- apply_truncate ---

    #[test]
    fn apply_truncate_empty_specs() {
        let mut lines = vec![make_rendered("a"), make_rendered("b")];
        apply_truncate(&mut lines, &[], OutputFormat::Html);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn apply_truncate_single_run() {
        let mut lines: Vec<_> = (1..=10).map(|i| make_rendered(&format!("line {}", i))).collect();
        apply_truncate(&mut lines, &[LineSpec::Expr("3:7".to_string())], OutputFormat::Html);
        assert_eq!(lines.len(), 6);
        assert_eq!(lines[0].text, "line 1");
        assert_eq!(lines[1].text, "line 2");
        assert!(lines[2].markup.contains("term-truncated"));
        assert!(lines[2].text.contains("5 lines truncated"));
        assert_eq!(lines[3].text, "line 8");
        assert_eq!(lines[4].text, "line 9");
        assert_eq!(lines[5].text, "line 10");
    }

    #[test]
    fn apply_truncate_two_runs() {
        let mut lines: Vec<_> = (1..=10).map(|i| make_rendered(&format!("line {}", i))).collect();
        apply_truncate(&mut lines, &[LineSpec::Expr(":2".to_string()), LineSpec::Expr("-2:".to_string())], OutputFormat::Html);
        assert_eq!(lines.len(), 8);
        assert!(lines[0].text.contains("2 lines truncated"));
        assert_eq!(lines[1].text, "line 3");
        assert_eq!(lines[6].text, "line 8");
        assert!(lines[7].text.contains("2 lines truncated"));
    }

    #[test]
    fn apply_truncate_adjacent_merge() {
        let mut lines: Vec<_> = (1..=10).map(|i| make_rendered(&format!("line {}", i))).collect();
        apply_truncate(&mut lines, &[LineSpec::Expr("2:4".to_string()), LineSpec::Expr("5:6".to_string())], OutputFormat::Html);
        assert_eq!(lines.len(), 6);
        assert_eq!(lines[0].text, "line 1");
        assert!(lines[1].text.contains("5 lines truncated"));
        assert_eq!(lines[2].text, "line 7");
    }

    #[test]
    fn apply_truncate_single_line() {
        let mut lines = vec![make_rendered("a"), make_rendered("b"), make_rendered("c")];
        apply_truncate(&mut lines, &[LineSpec::Index(2)], OutputFormat::Html);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].text, "a");
        assert!(lines[1].text.contains("1 line truncated"));
        assert_eq!(lines[2].text, "c");
    }

    #[test]
    fn apply_truncate_latex_format() {
        let mut lines = vec![make_rendered("a"), make_rendered("b"), make_rendered("c")];
        apply_truncate(&mut lines, &[LineSpec::Index(2)], OutputFormat::Latex);
        assert!(lines[1].markup.contains("\\textit"));
    }

    #[test]
    fn apply_truncate_markdown_format() {
        let mut lines = vec![make_rendered("a"), make_rendered("b"), make_rendered("c")];
        apply_truncate(&mut lines, &[LineSpec::Index(2)], OutputFormat::Markdown);
        assert_eq!(lines[1].markup, "*[1 line truncated]*");
    }

    // --- apply_remove with ranges ---

    #[test]
    fn apply_remove_range() {
        let mut lines: Vec<_> = (1..=5).map(|i| make_rendered(&format!("line {}", i))).collect();
        apply_remove(&mut lines, &[LineSpec::Expr("2:4".to_string())]);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "line 1");
        assert_eq!(lines[1].text, "line 5");
    }
}
