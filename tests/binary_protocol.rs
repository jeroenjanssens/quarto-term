use std::process::Command;

fn binary_path() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{}/target/debug/quarto-term", manifest_dir)
}

fn run_binary(json: &str) -> serde_json::Value {
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

fn minimal_config() -> &'static str {
    r#""config":{"shell":"bash","shell_args":["--norc","--noprofile"],"prompt":"$","timeout":5.0}"#
}

fn make_request(cells_json: &str) -> String {
    format!("{{{},{}}}", minimal_config(), cells_json)
}

fn cell(id: u32, code: &str, extra_opts: &str) -> String {
    let opts = if extra_opts.is_empty() {
        "{}".to_string()
    } else {
        format!("{{{}}}", extra_opts)
    };
    format!(
        r#"{{"id":{},"code":"{}","options":{},"line_options":[],"source_lines":[]}}"#,
        id, code, opts
    )
}

#[test]
fn minimal_request_echo_hello() {
    let json = make_request(&format!(r#""cells":[{}]"#, cell(1, "echo hello", "")));
    let result = run_binary(&json);
    let arr = result.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], 1);
    assert!(arr[0]["error"].is_null());
    let html = arr[0]["html"].as_str().unwrap();
    assert!(html.contains("echo hello"), "html should contain command: {}", html);
    assert!(html.contains("hello"), "html should contain output");
}

#[test]
fn state_persistence_between_cells() {
    let cells = format!(
        r#""cells":[{},{}]"#,
        cell(1, "export MY_TEST_VAR=persistent42", ""),
        cell(2, "echo $MY_TEST_VAR", "")
    );
    let json = make_request(&cells);
    let result = run_binary(&json);
    let arr = result.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    let html2 = arr[1]["html"].as_str().unwrap();
    assert!(html2.contains("persistent42"), "cell 2 should show var value: {}", html2);
}

#[test]
fn html_escaping_in_output() {
    let json = make_request(&format!(
        r#""cells":[{}]"#,
        cell(1, r#"echo '<b>&foo</b>'"#, "")
    ));
    let result = run_binary(&json);
    let html = result[0]["html"].as_str().unwrap();
    assert!(html.contains("&lt;b&gt;"), "should escape <: {}", html);
    assert!(html.contains("&amp;foo"), "should escape &: {}", html);
}

#[test]
fn echo_false_produces_empty() {
    let json = make_request(&format!(
        r#""cells":[{}]"#,
        cell(1, "echo hidden", r#""echo":false,"output":false"#)
    ));
    let result = run_binary(&json);
    let html = result[0]["html"].as_str().unwrap();
    assert_eq!(html, "");
}

#[test]
fn echo_source_mode() {
    let json = make_request(&format!(
        r#""cells":[{}]"#,
        cell(1, "echo hi", r#""echo":"source","output":false"#)
    ));
    let result = run_binary(&json);
    let html = result[0]["html"].as_str().unwrap();
    assert!(html.contains("term-source"), "should have source class: {}", html);
    assert!(html.contains("echo hi"), "should contain source code: {}", html);
}

#[test]
fn format_latex() {
    let json = format!(
        r#"{{"config":{{"shell":"bash","shell_args":["--norc","--noprofile"],"prompt":"$","timeout":5.0,"format":"latex"}},"cells":[{}]}}"#,
        cell(1, "echo tex", "")
    );
    let result = run_binary(&json);
    let html = result[0]["html"].as_str().unwrap();
    assert!(html.contains("\\begin{tcolorbox}"), "should have latex wrapper: {}", html);
}

#[test]
fn format_markdown() {
    let json = format!(
        r#"{{"config":{{"shell":"bash","shell_args":["--norc","--noprofile"],"prompt":"$","timeout":5.0,"format":"markdown"}},"cells":[{}]}}"#,
        cell(1, "echo md", "")
    );
    let result = run_binary(&json);
    let html = result[0]["html"].as_str().unwrap();
    assert!(html.contains("```console"), "should have markdown wrapper: {}", html);
}

#[test]
fn invalid_json_returns_error() {
    let result = run_binary("not valid json");
    let arr = result.as_array().unwrap();
    assert_eq!(arr[0]["id"], 0);
    assert!(arr[0]["error"].as_str().unwrap().contains("failed to parse"));
}

#[test]
fn timeout_error_on_missing_prompt() {
    let json = format!(
        r#"{{"config":{{"shell":"bash","shell_args":["--norc","--noprofile"],"prompt":"NEVER_MATCH_THIS_PROMPT_XYZ","timeout":1.0}},"cells":[{}]}}"#,
        cell(1, "echo x", "")
    );
    let result = run_binary(&json);
    let arr = result.as_array().unwrap();
    let error = arr[0]["error"].as_str().unwrap_or("");
    assert!(
        error.contains("timeout") || error.contains("failed to start"),
        "should have timeout or start error: {}", error
    );
}

#[test]
fn callouts_by_index() {
    let json = make_request(&format!(
        r#""cells":[{{"id":1,"code":"echo line1\necho line2\necho line3","options":{{"callouts":[1,-1]}},"line_options":[],"source_lines":[]}}]"#
    ));
    let result = run_binary(&json);
    let html = result[0]["html"].as_str().unwrap();
    assert!(html.contains("term-callout"), "should have callout: {}", html);
}

#[test]
fn remove_by_pattern() {
    let json = make_request(&format!(
        r#""cells":[{{"id":1,"code":"echo KEEP_THIS\necho REMOVE_THIS\necho KEEP_ALSO","options":{{"remove":["REMOVE_THIS"]}},"line_options":[],"source_lines":[]}}]"#
    ));
    let result = run_binary(&json);
    let html = result[0]["html"].as_str().unwrap();
    assert!(!html.contains("REMOVE_THIS"), "removed line should not appear: {}", html);
    assert!(html.contains("KEEP_THIS"), "kept lines should appear: {}", html);
}

#[test]
fn spacing_inserts_blank_lines() {
    let json = format!(
        r#"{{"config":{{"shell":"bash","shell_args":["--norc","--noprofile"],"prompt":"$","timeout":5.0,"spacing":true}},"cells":[{}]}}"#,
        cell(1, "echo a\necho b", "")
    );
    let result = run_binary(&json);
    let html = result[0]["html"].as_str().unwrap();
    // With spacing, there should be a blank line between the two command blocks
    let lines: Vec<&str> = html.split('\n').collect();
    let has_empty = lines.iter().any(|l| l.trim().is_empty());
    assert!(has_empty, "spacing should insert blank lines: {:?}", lines);
}

#[test]
fn ansi_color_in_output() {
    // Use echo with $'...' syntax to embed literal escape byte
    let json = r#"{"config":{"shell":"bash","shell_args":["--norc","--noprofile"],"prompt":"$","timeout":5.0},"cells":[{"id":1,"code":"echo $'\\e[31mRED\\e[0m'","options":{},"line_options":[],"source_lines":[]}]}"#;
    let result = run_binary(json);
    let html = result[0]["html"].as_str().unwrap();
    assert!(html.contains("var(--term-1)") || html.contains("color:"), "should have color styling: {}", html);
}

#[test]
fn ansi_disabled() {
    let json = r#"{"config":{"shell":"bash","shell_args":["--norc","--noprofile"],"prompt":"$","timeout":5.0},"cells":[{"id":1,"code":"echo $'\\e[31mRED\\e[0m'","options":{"ansi":false},"line_options":[],"source_lines":[]}]}"#;
    let result = run_binary(json);
    let html = result[0]["html"].as_str().unwrap();
    assert!(!html.contains("<span"), "should have no spans with ansi:false: {}", html);
}

#[test]
fn keep_last_prompt() {
    let json = make_request(&format!(
        r#""cells":[{}]"#,
        cell(1, "echo hi", r#""keep_last_prompt":true"#)
    ));
    let result = run_binary(&json);
    let html = result[0]["html"].as_str().unwrap();
    // The trailing prompt $ should appear at the end (bash-3.2$ or $)
    assert!(html.contains("$\n") || html.ends_with("$</code></pre>\n"),
        "should have trailing prompt: {}", html);
}

#[test]
fn line_options_expect_prompt_false() {
    let json = make_request(&format!(
        r#""cells":[{{"id":1,"code":"sleep 0.1","options":{{}},"line_options":[{{"line_index":0,"expect_prompt":false}}],"source_lines":[]}}]"#
    ));
    let result = run_binary(&json);
    assert!(result[0]["error"].is_null(), "should not error with expect_prompt:false");
}

#[test]
fn line_options_literal_false_sends_keycode() {
    // ctrl-c as literal:false should send interrupt (0x03), not the text "ctrl-c"
    let json = make_request(&format!(
        r#""cells":[{{"id":1,"code":"sleep 100\nctrl-c","options":{{}},"line_options":[{{"line_index":0,"expect_prompt":false,"hold":0.3}},{{"line_index":1,"literal":false,"delay":0.3}}],"source_lines":[]}}]"#
    ));
    let result = run_binary(&json);
    // Should not contain the literal text "ctrl-c" in the output
    let html = result[0]["html"].as_str().unwrap();
    assert!(!html.contains("ctrl-c"), "literal:false should send keycode, not text: {}", html);
}

#[test]
fn line_options_enter_false() {
    // With enter:false, the text is sent but no CR follows — so the command doesn't execute
    let json = make_request(&format!(
        r#""cells":[{{"id":1,"code":"echo NO_EXEC","options":{{}},"line_options":[{{"line_index":0,"enter":false,"expect_prompt":false,"hold":0.3}}],"source_lines":[]}}]"#
    ));
    let result = run_binary(&json);
    let html = result[0]["html"].as_str().unwrap();
    // The command shouldn't produce output since enter was never pressed
    assert!(!html.contains("\nNO_EXEC\n"), "enter:false should not execute command");
}

#[test]
fn line_options_hold() {
    let json = make_request(&format!(
        r#""cells":[{{"id":1,"code":"echo fast","options":{{}},"line_options":[{{"line_index":0,"hold":0.5}}],"source_lines":[]}}]"#
    ));
    let result = run_binary(&json);
    assert!(result[0]["error"].is_null(), "hold should not cause error");
    let html = result[0]["html"].as_str().unwrap();
    assert!(html.contains("fast"), "output should appear: {}", html);
}

#[test]
fn cell_literal_false_multikey() {
    // Cell-level literal:false splits space-separated tokens as keycodes
    let json = make_request(&format!(
        r#""cells":[{{"id":1,"code":"echo hello","options":{{"literal":false}},"line_options":[],"source_lines":[]}}]"#
    ));
    let result = run_binary(&json);
    // "echo" and "hello" sent as separate keycodes; since they're not named keys,
    // they just send their text bytes. The command still executes.
    assert!(result[0]["error"].is_null());
}

#[test]
fn cell_delay() {
    let json = make_request(&format!(
        r#""cells":[{{"id":1,"code":"echo d1\necho d2","options":{{"delay":0.2}},"line_options":[],"source_lines":[]}}]"#
    ));
    let result = run_binary(&json);
    assert!(result[0]["error"].is_null());
    let html = result[0]["html"].as_str().unwrap();
    assert!(html.contains("d1"), "first command output: {}", html);
    assert!(html.contains("d2"), "second command output: {}", html);
}

#[test]
fn cell_hold() {
    let json = make_request(&format!(
        r#""cells":[{}]"#,
        cell(1, "echo held", r#""hold":0.3"#)
    ));
    let result = run_binary(&json);
    assert!(result[0]["error"].is_null());
    let html = result[0]["html"].as_str().unwrap();
    assert!(html.contains("held"), "output should appear: {}", html);
}

#[test]
fn cell_fullscreen() {
    let json = make_request(&format!(
        r#""cells":[{}]"#,
        cell(1, "echo screen", r#""fullscreen":true"#)
    ));
    let result = run_binary(&json);
    let html = result[0]["html"].as_str().unwrap();
    assert!(html.contains("term-screen"), "fullscreen uses term-screen class: {}", html);
}

#[test]
fn cell_trailing_spaces_true() {
    // printf with trailing spaces should preserve them with trailing_spaces:true
    let json = make_request(&format!(
        r#""cells":[{{"id":1,"code":"printf 'hi   \\n'","options":{{"trailing_spaces":true}},"line_options":[],"source_lines":[]}}]"#
    ));
    let result = run_binary(&json);
    let html = result[0]["html"].as_str().unwrap();
    assert!(html.contains("hi   "), "trailing spaces should be preserved: {}", html);
}

#[test]
fn cell_highlight_language() {
    let json = make_request(&format!(
        r#""cells":[{}]"#,
        cell(1, "print('hi')", r#""echo":"source","output":false,"highlight":"python""#)
    ));
    let result = run_binary(&json);
    let html = result[0]["html"].as_str().unwrap();
    assert!(html.contains("language-python"), "highlight language: {}", html);
}

#[test]
fn cell_include_false_equivalent() {
    // echo:false + output:false is equivalent to include:false
    let json = make_request(&format!(
        r#""cells":[{}]"#,
        cell(1, "echo hidden_setup", r#""echo":"false","output":false"#)
    ));
    let result = run_binary(&json);
    let html = result[0]["html"].as_str().unwrap();
    assert_eq!(html, "", "include:false should produce empty: {}", html);
}

#[test]
fn config_trailing_spaces() {
    let json = format!(
        r#"{{"config":{{"shell":"bash","shell_args":["--norc","--noprofile"],"prompt":"$","timeout":5.0,"trailing_spaces":true}},"cells":[{{"id":1,"code":"printf 'end   \\n'","options":{{}},"line_options":[],"source_lines":[]}}]}}"#
    );
    let result = run_binary(&json);
    let html = result[0]["html"].as_str().unwrap();
    assert!(html.contains("end   "), "doc-level trailing_spaces: {}", html);
}

#[test]
fn config_fontsize() {
    let json = format!(
        r#"{{"config":{{"shell":"bash","shell_args":["--norc","--noprofile"],"prompt":"$","timeout":5.0,"fontsize":"0.8em"}},"cells":[{}]}}"#,
        cell(1, "echo fs", "")
    );
    let result = run_binary(&json);
    let html = result[0]["html"].as_str().unwrap();
    assert!(html.contains("font-size:0.8em"), "fontsize in output: {}", html);
}

#[test]
fn config_prompt_regex() {
    let json = format!(
        r#"{{"config":{{"shell":"bash","shell_args":["--norc","--noprofile"],"prompt":"$","prompt_regex":"\\$\\s*$","timeout":5.0}},"cells":[{}]}}"#,
        cell(1, "echo regex", "")
    );
    let result = run_binary(&json);
    assert!(result[0]["error"].is_null(), "prompt_regex should work");
    let html = result[0]["html"].as_str().unwrap();
    assert!(html.contains("regex"), "output with custom regex: {}", html);
}
