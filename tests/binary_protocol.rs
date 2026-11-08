use serde_json::{json, Value};
use std::process::Command;

fn binary_path() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{}/target/debug/quarto-term", manifest_dir)
}

fn run_binary(input: &Value) -> Value {
    run_binary_raw(&input.to_string())
}

fn run_binary_raw(json: &str) -> Value {
    let output = Command::new(binary_path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child.stdin.take().unwrap().write_all(json.as_bytes()).unwrap();
            child.wait_with_output()
        })
        .expect("failed to run binary");

    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|_| panic!("invalid JSON output: {}", String::from_utf8_lossy(&output.stdout)))
}

fn request(cells: Vec<Value>) -> Value {
    json!({
        "config": {
            "shell": "bash",
            "shell_args": ["--norc", "--noprofile"],
            "prompt": "$",
            "timeout": 5.0
        },
        "cells": cells
    })
}

fn request_with_config(config: Value, cells: Vec<Value>) -> Value {
    json!({
        "config": config,
        "cells": cells
    })
}

fn cell(id: u32, code: &str) -> Value {
    json!({
        "id": id,
        "code": code,
        "options": {},
        "line_options": [],
        "source_lines": []
    })
}

fn cell_with_opts(id: u32, code: &str, opts: Value) -> Value {
    json!({
        "id": id,
        "code": code,
        "options": opts,
        "line_options": [],
        "source_lines": []
    })
}

fn cell_with_line_opts(id: u32, code: &str, opts: Value, line_opts: Value) -> Value {
    json!({
        "id": id,
        "code": code,
        "options": opts,
        "line_options": line_opts,
        "source_lines": []
    })
}

fn get_markup(result: &Value, index: usize) -> &str {
    result[index]["markup"].as_str().unwrap()
}

#[test]
fn minimal_request_echo_hello() {
    let result = run_binary(&request(vec![cell(1, "echo hello")]));
    let arr = result.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], 1);
    assert!(arr[0]["error"].is_null());
    let html = get_markup(&result, 0);
    assert!(html.contains("echo hello"), "html should contain command: {}", html);
    assert!(html.contains("hello"), "html should contain output");
}

#[test]
fn state_persistence_between_cells() {
    let result = run_binary(&request(vec![
        cell(1, "export MY_TEST_VAR=persistent42"),
        cell(2, "echo $MY_TEST_VAR"),
    ]));
    let arr = result.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    let html2 = get_markup(&result, 1);
    assert!(html2.contains("persistent42"), "cell 2 should show var value: {}", html2);
}

#[test]
fn html_escaping_in_output() {
    let result = run_binary(&request(vec![cell(1, "echo '<b>&foo</b>'")]));
    let html = get_markup(&result, 0);
    assert!(html.contains("&lt;b&gt;"), "should escape <: {}", html);
    assert!(html.contains("&amp;foo"), "should escape &: {}", html);
}

#[test]
fn echo_false_produces_empty() {
    let result = run_binary(&request(vec![
        cell_with_opts(1, "echo hidden", json!({"echo": false, "output": false})),
    ]));
    let html = get_markup(&result, 0);
    assert_eq!(html, "");
}

#[test]
fn echo_source_mode() {
    let result = run_binary(&request(vec![
        cell_with_opts(1, "echo hi", json!({"echo": "source", "output": false})),
    ]));
    let html = get_markup(&result, 0);
    assert!(html.contains("term-source"), "should have source class: {}", html);
    assert!(html.contains("echo hi"), "should contain source code: {}", html);
}

#[test]
fn format_latex() {
    let config = json!({
        "shell": "bash",
        "shell_args": ["--norc", "--noprofile"],
        "prompt": "$",
        "timeout": 5.0,
        "format": "latex"
    });
    let result = run_binary(&request_with_config(config, vec![cell(1, "echo tex")]));
    let html = get_markup(&result, 0);
    assert!(html.contains("\\begin{tcolorbox}"), "should have latex wrapper: {}", html);
}

#[test]
fn format_markdown() {
    let config = json!({
        "shell": "bash",
        "shell_args": ["--norc", "--noprofile"],
        "prompt": "$",
        "timeout": 5.0,
        "format": "markdown"
    });
    let result = run_binary(&request_with_config(config, vec![cell(1, "echo md")]));
    let html = get_markup(&result, 0);
    assert!(html.contains("```console"), "should have markdown wrapper: {}", html);
}

#[test]
fn invalid_json_returns_error() {
    let result = run_binary_raw("not valid json");
    let arr = result.as_array().unwrap();
    assert_eq!(arr[0]["id"], 0);
    assert!(arr[0]["error"].as_str().unwrap().contains("failed to parse"));
}

#[test]
fn timeout_error_on_missing_prompt() {
    let config = json!({
        "shell": "bash",
        "shell_args": ["--norc", "--noprofile"],
        "prompt": "NEVER_MATCH_THIS_PROMPT_XYZ",
        "timeout": 1.0
    });
    let result = run_binary(&request_with_config(config, vec![cell(1, "echo x")]));
    let arr = result.as_array().unwrap();
    let error = arr[0]["error"].as_str().unwrap_or("");
    assert!(
        error.contains("timeout") || error.contains("failed to start"),
        "should have timeout or start error: {}", error
    );
}

#[test]
fn callouts_by_index() {
    let result = run_binary(&request(vec![
        cell_with_opts(1, "echo line1\necho line2\necho line3", json!({"callout": [1, -1]})),
    ]));
    let html = get_markup(&result, 0);
    assert!(html.contains("term-callout"), "should have callout: {}", html);
}

#[test]
fn remove_by_pattern() {
    let result = run_binary(&request(vec![
        cell_with_opts(1, "echo KEEP_THIS\necho REMOVE_THIS\necho KEEP_ALSO", json!({"remove": ["REMOVE_THIS"]})),
    ]));
    let html = get_markup(&result, 0);
    assert!(!html.contains("REMOVE_THIS"), "removed line should not appear: {}", html);
    assert!(html.contains("KEEP_THIS"), "kept lines should appear: {}", html);
}

#[test]
fn spacing_inserts_blank_lines() {
    let config = json!({
        "shell": "bash",
        "shell_args": ["--norc", "--noprofile"],
        "prompt": "$",
        "timeout": 5.0,
        "spacing": true
    });
    let result = run_binary(&request_with_config(config, vec![cell(1, "echo a\necho b")]));
    let html = get_markup(&result, 0);
    let lines: Vec<&str> = html.split('\n').collect();
    let has_empty = lines.iter().any(|l| l.trim().is_empty());
    assert!(has_empty, "spacing should insert blank lines: {:?}", lines);
}

#[test]
fn ansi_color_in_output() {
    let result = run_binary(&request(vec![
        cell(1, "echo $'\\e[31mRED\\e[0m'"),
    ]));
    let html = get_markup(&result, 0);
    assert!(html.contains("var(--term-1)") || html.contains("color:"), "should have color styling: {}", html);
}

#[test]
fn ansi_disabled() {
    let result = run_binary(&request(vec![
        cell_with_opts(1, "echo $'\\e[31mRED\\e[0m'", json!({"ansi": false})),
    ]));
    let html = get_markup(&result, 0);
    assert!(!html.contains("style="), "should have no style spans with ansi:false: {}", html);
}

#[test]
fn keep_last_prompt() {
    let result = run_binary(&request(vec![
        cell_with_opts(1, "echo hi", json!({"keep_last_prompt": true})),
    ]));
    let html = get_markup(&result, 0);
    assert!(
        html.contains("$\n") || html.contains("$</span>\n") || html.contains("$</code></pre>\n") || html.contains("$</span></code></pre>\n"),
        "should have trailing prompt: {}", html
    );
}

#[test]
fn line_options_expect_prompt_false() {
    let result = run_binary(&request(vec![
        cell_with_line_opts(1, "sleep 0.1", json!({}), json!([{"line_index": 0, "expect_prompt": false}])),
    ]));
    assert!(result[0]["error"].is_null(), "should not error with expect_prompt:false");
}

#[test]
fn line_options_literal_false_sends_keycode() {
    let result = run_binary(&request(vec![
        cell_with_line_opts(
            1, "sleep 100\nctrl-c", json!({}),
            json!([
                {"line_index": 0, "expect_prompt": false, "hold": 0.3},
                {"line_index": 1, "literal": false, "delay": 0.3}
            ]),
        ),
    ]));
    let html = get_markup(&result, 0);
    assert!(!html.contains("ctrl-c"), "literal:false should send keycode, not text: {}", html);
}

#[test]
fn line_options_enter_false() {
    let result = run_binary(&request(vec![
        cell_with_line_opts(
            1, "echo NO_EXEC", json!({}),
            json!([{"line_index": 0, "enter": false, "expect_prompt": false, "hold": 0.3}]),
        ),
    ]));
    let html = get_markup(&result, 0);
    assert!(!html.contains("\nNO_EXEC\n"), "enter:false should not execute command");
}

#[test]
fn line_options_hold() {
    let result = run_binary(&request(vec![
        cell_with_line_opts(1, "echo fast", json!({}), json!([{"line_index": 0, "hold": 0.5}])),
    ]));
    assert!(result[0]["error"].is_null(), "hold should not cause error");
    let html = get_markup(&result, 0);
    assert!(html.contains("fast"), "output should appear: {}", html);
}

#[test]
fn cell_literal_false_multikey() {
    let result = run_binary(&request(vec![
        cell_with_opts(1, "echo hello", json!({"literal": false})),
    ]));
    assert!(result[0]["error"].is_null());
}

#[test]
fn cell_delay() {
    let result = run_binary(&request(vec![
        cell_with_opts(1, "echo d1\necho d2", json!({"delay": 0.2})),
    ]));
    assert!(result[0]["error"].is_null());
    let html = get_markup(&result, 0);
    assert!(html.contains("d1"), "first command output: {}", html);
    assert!(html.contains("d2"), "second command output: {}", html);
}

#[test]
fn cell_hold() {
    let result = run_binary(&request(vec![
        cell_with_opts(1, "echo held", json!({"hold": 0.3})),
    ]));
    assert!(result[0]["error"].is_null());
    let html = get_markup(&result, 0);
    assert!(html.contains("held"), "output should appear: {}", html);
}

#[test]
fn cell_fullscreen() {
    let result = run_binary(&request(vec![
        cell_with_opts(1, "echo screen", json!({"fullscreen": true})),
    ]));
    let html = get_markup(&result, 0);
    assert!(html.contains("term-screen"), "fullscreen uses term-screen class: {}", html);
}

#[test]
fn cell_trailing_spaces_true() {
    let result = run_binary(&request(vec![
        cell_with_opts(1, "printf 'hi   \\n'", json!({"trailing_spaces": true})),
    ]));
    let html = get_markup(&result, 0);
    assert!(html.contains("hi   "), "trailing spaces should be preserved: {}", html);
}

#[test]
fn cell_highlight_language() {
    let result = run_binary(&request(vec![
        cell_with_opts(1, "print('hi')", json!({"echo": "source", "output": false, "highlight": "python"})),
    ]));
    let html = get_markup(&result, 0);
    assert!(html.contains("language-python"), "highlight language: {}", html);
}

#[test]
fn cell_include_false_equivalent() {
    let result = run_binary(&request(vec![
        cell_with_opts(1, "echo hidden_setup", json!({"echo": "false", "output": false})),
    ]));
    let html = get_markup(&result, 0);
    assert_eq!(html, "", "include:false should produce empty: {}", html);
}

#[test]
fn config_trailing_spaces() {
    let config = json!({
        "shell": "bash",
        "shell_args": ["--norc", "--noprofile"],
        "prompt": "$",
        "timeout": 5.0,
        "trailing_spaces": true
    });
    let result = run_binary(&request_with_config(config, vec![cell(1, "printf 'end   \\n'")]));
    let html = get_markup(&result, 0);
    assert!(html.contains("end   "), "doc-level trailing_spaces: {}", html);
}

#[test]
fn config_font_size() {
    let config = json!({
        "shell": "bash",
        "shell_args": ["--norc", "--noprofile"],
        "prompt": "$",
        "timeout": 5.0,
        "font_size": "0.8em"
    });
    let result = run_binary(&request_with_config(config, vec![cell(1, "echo fs")]));
    let html = get_markup(&result, 0);
    assert!(html.contains("font-size:0.8em"), "font_size in output: {}", html);
}

#[test]
fn config_prompt_regex() {
    let config = json!({
        "shell": "bash",
        "shell_args": ["--norc", "--noprofile"],
        "prompt": "$",
        "prompt_regex": "\\$\\s*$",
        "timeout": 5.0
    });
    let result = run_binary(&request_with_config(config, vec![cell(1, "echo regex")]));
    assert!(result[0]["error"].is_null(), "prompt_regex should work");
    let html = get_markup(&result, 0);
    assert!(html.contains("regex"), "output with custom regex: {}", html);
}

#[test]
fn docker_config_error_propagates() {
    let config = json!({
        "shell": "bash",
        "shell_args": ["--norc", "--noprofile"],
        "prompt": "$",
        "timeout": 3.0,
        "docker": {
            "image": "quarto-term-nonexistent-test-image-zzz999",
            "pull": "never"
        }
    });
    let result = run_binary(&request_with_config(config, vec![cell(1, "echo hi")]));
    let error = result[0]["error"].as_str().unwrap_or("");
    assert!(!error.is_empty(), "docker error should propagate: {:?}", result[0]);
}

// ---------------------------------------------------------------------------
// Multibyte characters split between PTY reads
// ---------------------------------------------------------------------------

/// ~60 KB of 2-, 3-, and 4-byte characters, written at once by `cat` (like
/// `bat` does). The PTY delivers it in chunks whose boundaries fall inside
/// characters: with 0.7.1, every run had 95 replacement characters. (Output
/// from many small writes, e.g. a loop of `printf`, only splits sometimes.)
/// Shell builtins only, so it stays cheap when the tests run in parallel.
const MULTIBYTE_LINES: usize = 300;
const MULTIBYTE_CODE: &str = "f=$(mktemp); l=$(printf '─%.0s' {1..60})' ✓é😀'; for i in {1..300}; do echo \"$l\"; done > \"$f\"; cat \"$f\"; rm \"$f\"";

fn multibyte_line() -> String {
    format!("{} ✓é😀", "─".repeat(60))
}

#[test]
fn multibyte_output_larger_than_a_read_is_intact() {
    let config = json!({
        "shell": "bash",
        "shell_args": ["--norc", "--noprofile"],
        "prompt": "$",
        "timeout": 20.0,
        "rows": 400
    });
    let result = run_binary(&request_with_config(config, vec![cell(1, MULTIBYTE_CODE)]));
    assert!(result[0]["error"].is_null(), "no error: {:?}", result[0]["error"]);
    let html = get_markup(&result, 0);
    assert!(!html.contains('\u{FFFD}'), "no replacement characters in output");
    assert_eq!(html.matches(&multibyte_line()).count(), MULTIBYTE_LINES, "every line is intact");
}

#[test]
fn multibyte_output_is_intact_in_recordings() {
    let dir = std::env::temp_dir().join(format!("quarto-term-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let cast = dir.join("multibyte.cast");
    let config = json!({
        "shell": "bash",
        "shell_args": ["--norc", "--noprofile"],
        "prompt": "$",
        "timeout": 20.0,
        "record": [cast.to_str().unwrap()]
    });
    let result = run_binary(&request_with_config(config, vec![cell(1, MULTIBYTE_CODE)]));
    assert!(result[0]["error"].is_null(), "no error: {:?}", result[0]["error"]);

    let recording = std::fs::read_to_string(&cast).unwrap();
    let mut output = String::new();
    for line in recording.lines().skip(1) {
        let event: Value = serde_json::from_str(line).unwrap();
        if event[1] == "o" {
            output.push_str(event[2].as_str().unwrap());
        }
    }
    std::fs::remove_dir_all(&dir).ok();
    assert!(!output.contains('\u{FFFD}'), "no replacement characters in the recording");
    assert_eq!(output.matches(&multibyte_line()).count(), MULTIBYTE_LINES, "every line is intact");
}

// ---------------------------------------------------------------------------
// Shortcode-like output ({{< ... >}}) must not reach Quarto as `{{<`
// ---------------------------------------------------------------------------

fn config_for_format(format: &str) -> Value {
    json!({
        "shell": "bash",
        "shell_args": ["--norc", "--noprofile"],
        "prompt": "$",
        "timeout": 10.0,
        "format": format
    })
}

#[test]
fn html_output_escapes_braces() {
    let result = run_binary(&request(vec![cell(1, "echo 'x{{< meta title >}}y'")]));
    let html = get_markup(&result, 0);
    assert!(!html.contains('{'), "no literal braces in HTML: {}", html);
    assert!(
        html.contains("x&#123;&#123;&lt; meta title &gt;}}y"),
        "braces are entities: {}",
        html
    );
}

#[test]
fn html_output_escapes_braces_before_colored_text() {
    // `{{` followed by a colored span becomes `{{<span ...>` without escaping
    let result = run_binary(&request(vec![cell(
        1,
        "printf 'alice{{\\033[32m2\\033[0m,3}}.txt\\n'",
    )]));
    let html = get_markup(&result, 0);
    assert!(!html.contains("{{"), "no literal `{{{{` in HTML: {}", html);
    assert!(html.contains("alice&#123;&#123;<span"), "colored text after braces: {}", html);
}

#[test]
fn html_source_echo_escapes_braces() {
    let result = run_binary(&request(vec![cell_with_opts(
        1,
        "echo '{{< meta title >}}'",
        json!({"echo": "source", "output": false}),
    )]));
    let html = get_markup(&result, 0);
    assert!(html.contains("term-source"), "source block: {}", html);
    assert!(!html.contains('{'), "no literal braces in source: {}", html);
}

#[test]
fn latex_output_escapes_braces() {
    let result = run_binary(&request_with_config(
        config_for_format("latex"),
        vec![cell(1, "echo 'x{{< meta title >}}y'")],
    ));
    let tex = get_markup(&result, 0);
    assert!(!tex.contains("{{"), "no literal `{{{{` in LaTeX: {}", tex);
    assert!(tex.contains("x\\{\\{< meta title >\\}\\}y"), "escaped braces: {}", tex);
}

#[test]
fn typst_plain_output_avoids_shortcode_opener() {
    // Plain output is a raw block, where nothing can be escaped; with `{{<`
    // it becomes a #raw() string literal instead
    let result = run_binary(&request_with_config(
        config_for_format("typst"),
        vec![cell(1, "echo 'x{{< meta title >}}y'")],
    ));
    let typ = get_markup(&result, 0);
    assert!(!typ.contains("{{<"), "no `{{{{<` in Typst: {}", typ);
    assert!(typ.contains("#raw(block: true, "), "raw string literal: {}", typ);
    assert!(typ.contains("x{{\\u{3c} meta title >}}y"), "escaped `<`: {}", typ);
}

#[test]
fn typst_colored_output_escapes_angle_brackets() {
    let result = run_binary(&request_with_config(
        config_for_format("typst"),
        vec![cell(1, "printf 'x{{< \\033[32mmeta\\033[0m >}}y\\n'")],
    ));
    let typ = get_markup(&result, 0);
    assert!(!typ.contains("{{<"), "no `{{{{<` in Typst: {}", typ);
    assert!(typ.contains("x{{\\< #text("), "escaped `<` before colored text: {}", typ);
}

#[test]
fn typst_plain_output_without_shortcodes_is_unchanged() {
    let result = run_binary(&request_with_config(
        config_for_format("typst"),
        vec![cell(1, "echo 'a <b> {c}'")],
    ));
    let typ = get_markup(&result, 0);
    assert!(typ.contains("````\n"), "still a fenced raw block: {}", typ);
    assert!(typ.contains("\na <b> {c}\n"), "text as is: {}", typ);
}

#[test]
fn markdown_output_keeps_braces_literal() {
    // Markdown can't escape them; the Lua filter turns such blocks into code
    // blocks with shortcodes="false" instead
    let result = run_binary(&request_with_config(
        config_for_format("markdown"),
        vec![cell(1, "echo 'x{{< meta title >}}y'")],
    ));
    let md = get_markup(&result, 0);
    assert!(md.contains("x{{< meta title >}}y"), "literal text: {}", md);
}

// ---------------------------------------------------------------------------
// Invisible zero-width characters must not take up a column
// ---------------------------------------------------------------------------

/// The text of each output line (the markup without tags)
fn output_lines(html: &str) -> Vec<String> {
    html.lines()
        .filter(|l| l.contains("term-output"))
        .map(|l| {
            let mut text = String::new();
            let mut in_tag = false;
            for c in l.chars() {
                match c {
                    '<' => in_tag = true,
                    '>' => in_tag = false,
                    _ if !in_tag => text.push(c),
                    _ => {}
                }
            }
            text
        })
        .collect()
}

fn config_80_cols() -> Value {
    json!({
        "shell": "bash",
        "shell_args": ["--norc", "--noprofile"],
        "prompt": "$",
        "timeout": 5.0,
        "cols": 80
    })
}

#[test]
fn bom_at_start_of_full_line_does_not_wrap() {
    // A file that starts with a byte-order mark, cut to 80 columns
    let code = "printf '\\xef\\xbb\\xbf%s\\n' $(printf 'x%.0s' {1..80})";
    let result = run_binary(&request_with_config(config_80_cols(), vec![cell(1, code)]));
    let lines = output_lines(get_markup(&result, 0));
    let x80 = "x".repeat(80);
    assert!(lines.iter().any(|l| l == &x80), "one line of 80 x's: {lines:?}");
    assert!(!lines.iter().any(|l| l == "x"), "no wrapped x: {lines:?}");
}

#[test]
fn zero_width_space_and_word_joiner_do_not_take_columns() {
    let code = "printf 'xxxxxxxxxx\\xe2\\x80\\x8bxxxxxxxxxx\\xe2\\x81\\xa0%s\\n' $(printf 'x%.0s' {1..60})";
    let result = run_binary(&request_with_config(config_80_cols(), vec![cell(1, code)]));
    let lines = output_lines(get_markup(&result, 0));
    assert!(lines.iter().any(|l| l == &"x".repeat(80)), "one line of 80 x's: {lines:?}");
}

#[test]
fn zero_width_joiner_and_combining_marks_are_kept() {
    // These change how their neighbors look, so they stay in the output
    let code = "printf 'e\\xcc\\x81 a\\xe2\\x80\\x8db\\n'";
    let result = run_binary(&request(vec![cell(1, code)]));
    let html = get_markup(&result, 0);
    assert!(html.contains("e\u{301}"), "combining mark kept: {html}");
    assert!(html.contains("a\u{200D}b"), "zero-width joiner kept: {html}");
}

#[test]
fn recordings_keep_invisible_characters() {
    let dir = std::env::temp_dir().join(format!("quarto-term-bom-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let cast = dir.join("bom.cast");
    let mut config = config_80_cols();
    config["record"] = json!([cast.to_str().unwrap()]);
    let result = run_binary(&request_with_config(config, vec![cell(1, "printf '\\xef\\xbb\\xbfbom\\n'")]));
    assert!(result[0]["error"].is_null(), "no error: {:?}", result[0]["error"]);
    let recording = std::fs::read_to_string(&cast).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    // serde_json writes the BOM as is
    assert!(recording.contains("\u{FEFF}bom"), "BOM in the recording: {recording}");
}
