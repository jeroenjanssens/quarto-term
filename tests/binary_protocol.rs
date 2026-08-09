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
        cell_with_opts(1, "echo line1\necho line2\necho line3", json!({"callouts": [1, -1]})),
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
