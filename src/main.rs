use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, Read};

#[derive(Deserialize)]
struct Cell {
    code: String,
    options: Options,
}

#[derive(Deserialize)]
struct Options {
    #[serde(default = "default_true")]
    echo: bool,
    #[serde(default = "default_true")]
    output: bool,
    #[serde(default)]
    reverse: bool,
    #[serde(default)]
    prefix: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Serialize)]
struct CellResult {
    html: String,
}

struct Session {
    variables: HashMap<String, String>,
    cell_count: usize,
}

impl Session {
    fn new() -> Self {
        Self {
            variables: HashMap::new(),
            cell_count: 0,
        }
    }

    fn execute(&mut self, code: &str) -> String {
        self.cell_count += 1;
        let mut output_lines = Vec::new();

        for line in code.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("let ") {
                if let Some((name, value)) = rest.split_once('=') {
                    let name = name.trim().to_string();
                    let value = self.interpolate(value.trim());
                    self.variables.insert(name.clone(), value.clone());
                    output_lines.push(format!("{name} = {value}"));
                }
            } else if let Some(name) = line.strip_prefix("print ") {
                let name = name.trim();
                match self.variables.get(name) {
                    Some(val) => output_lines.push(val.clone()),
                    None => output_lines.push(format!("undefined: {name}")),
                }
            } else if !line.is_empty() {
                output_lines.push(format!("[cell {}] {}", self.cell_count, line));
            }
        }

        output_lines.join("\n")
    }

    fn interpolate(&self, value: &str) -> String {
        let mut result = value.to_string();
        for (k, v) in &self.variables {
            result = result.replace(&format!("${{{k}}}"), v);
        }
        result
    }
}

fn main() {
    let mut input_str = String::new();
    io::stdin().read_to_string(&mut input_str).unwrap();

    let cells: Vec<Cell> = serde_json::from_str(&input_str).unwrap();
    let mut session = Session::new();
    let mut results: Vec<CellResult> = Vec::new();

    for cell in &cells {
        let mut html = String::new();

        if cell.options.echo {
            let code_display = if cell.options.reverse {
                cell.code.chars().rev().collect::<String>()
            } else {
                cell.code.clone()
            };

            let prefixed = match &cell.options.prefix {
                Some(prefix) => code_display
                    .lines()
                    .map(|line| format!("{prefix}{line}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
                None => code_display,
            };

            html.push_str(&format!(
                "<pre><code class=\"language-term\">{}</code></pre>\n",
                html_escape(&prefixed)
            ));
        }

        if cell.options.output {
            let result = session.execute(&cell.code);
            if !result.is_empty() {
                html.push_str(&format!(
                    "<div class=\"cell-output\"><pre>{}</pre></div>\n",
                    html_escape(&result)
                ));
            }
        }

        results.push(CellResult { html });
    }

    println!("{}", serde_json::to_string(&results).unwrap());
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
