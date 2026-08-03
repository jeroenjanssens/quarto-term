mod error;
mod keymap;
mod latex;
mod markdown;
mod protocol;
mod recorder;
mod renderer;
mod session;
mod typst;

use std::io::{self, Read};

use protocol::{BatchRequest, CellResult};
use session::PtySession;

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();

    let request: BatchRequest = match serde_json::from_str(&input) {
        Ok(r) => r,
        Err(e) => {
            let error_result = vec![CellResult {
                id: 0,
                html: String::new(),
                error: Some(format!("failed to parse input: {e}")),
            }];
            println!("{}", serde_json::to_string(&error_result).unwrap());
            return;
        }
    };

    if request.config.verbose {
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
