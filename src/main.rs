mod color;
mod error;
mod keymap;
mod latex;
mod markdown;
mod protocol;
mod recorder;
mod renderer;
mod session;
mod terminal_line;
mod typst;

use std::io::{self, Read};
use std::process::Command;

use protocol::{BatchRequest, CellResult};
use session::PtySession;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|s| s.as_str()) == Some("check") {
        std::process::exit(run_check(&args[2..]));
    }

    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();

    let request: BatchRequest = match serde_json::from_str(&input) {
        Ok(r) => r,
        Err(e) => {
            let error_result = vec![CellResult {
                id: 0,
                html: String::new(),
                error: Some(format!("failed to parse input: {e}")),
                recorded_assertions: Vec::new(),
            }];
            println!("{}", serde_json::to_string(&error_result).unwrap());
            return;
        }
    };

    if request.config.verbose {
        if let Some(ref docker) = request.config.docker {
            eprintln!("quarto-term: docker mode (image: {})", docker.image);
        }
        eprintln!("quarto-term: starting session (shell: {}, prompt: {:?})",
            request.config.shell, request.config.prompt);
    }

    let mut session = match PtySession::new(&request.config) {
        Ok(s) => s,
        Err(e) => {
            let results: Vec<CellResult> = request
                .cells
                .iter()
                .map(|cell| CellResult {
                    id: cell.id,
                    html: String::new(),
                    error: Some(format!("session failed to start: {e}")),
                    recorded_assertions: Vec::new(),
                })
                .collect();
            println!("{}", serde_json::to_string(&results).unwrap());
            return;
        }
    };

    if request.config.verbose {
        eprintln!("quarto-term: session ready, executing {} cells", request.cells.len());
        for path in &request.config.record {
            eprintln!("quarto-term: recording to {}", path);
        }
    }

    let mut results = Vec::with_capacity(request.cells.len());

    for (idx, cell) in request.cells.iter().enumerate() {
        if request.config.verbose {
            let label = cell.label.as_deref().unwrap_or_else(|| "");
            if label.is_empty() {
                eprintln!("quarto-term: executing cell {} ({})", idx + 1, cell.options);
            } else {
                eprintln!("quarto-term: executing cell {} \"{}\" ({})", idx + 1, label, cell.options);
            }
        }

        let result = session.execute_cell(cell);

        if request.config.verbose {
            if let Some(ref err) = result.error {
                eprintln!("quarto-term: cell {} error: {}", cell.id, err);
            }
        }

        results.push(result);
    }

    session.finish();

    if request.config.verbose {
        for path in &request.config.record {
            eprintln!("quarto-term: recording finished: {}", path);
        }
    }

    println!("{}", serde_json::to_string(&results).unwrap());
}

fn run_check(args: &[String]) -> i32 {
    let record_path = args.iter()
        .position(|a| a == "--record")
        .and_then(|i| args.get(i + 1).filter(|a| !a.starts_with('-')))
        .cloned();
    let file = match args.iter().find(|a| !a.starts_with('-') && !a.ends_with(".cast") && !a.ends_with(".termshow") || a.ends_with(".qmd")) {
        Some(f) => f,
        None => {
            eprintln!("usage: quarto-term check [--record <path.cast>] <file.qmd>");
            return 1;
        }
    };

    if !std::path::Path::new(file).exists() {
        eprintln!("quarto-term check: file not found: {}", file);
        return 1;
    }

    let mut cmd = Command::new("quarto");
    cmd.args(["render", file, "--to", "html"]);
    cmd.env("QUARTO_TERM_CHECK", "1");
    if let Some(ref path) = record_path {
        cmd.env("QUARTO_TERM_RECORD", path);
    }

    let status = cmd.status();

    match status {
        Ok(s) if s.success() => {
            if let Some(ref path) = record_path {
                eprintln!("quarto-term check: PASSED, recorded to {} ({})", path, file);
            } else {
                eprintln!("quarto-term check: PASSED ({})", file);
            }
            0
        }
        Ok(_) => {
            eprintln!("quarto-term check: FAILED ({})", file);
            1
        }
        Err(e) => {
            eprintln!("quarto-term check: failed to run quarto: {}", e);
            1
        }
    }
}
