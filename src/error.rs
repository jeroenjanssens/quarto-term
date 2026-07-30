use std::fmt;
use std::io;

#[derive(Debug)]
pub enum TermError {
    SpawnFailed(String),
    PromptTimeout { elapsed_secs: f64, last_output: String },
    ShellExited,
    RegexCompile(String),
    Io(io::Error),
}

impl fmt::Display for TermError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SpawnFailed(msg) => write!(f, "failed to spawn shell: {msg}"),
            Self::PromptTimeout { elapsed_secs, last_output } => {
                write!(
                    f,
                    "timeout waiting for prompt after {elapsed_secs:.1}s\n  Last output: {:?}",
                    truncate_end(last_output, 500)
                )
            }
            Self::ShellExited => write!(f, "shell process exited unexpectedly"),
            Self::RegexCompile(msg) => write!(f, "invalid prompt regex: {msg}"),
            Self::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for TermError {}

impl From<io::Error> for TermError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

fn truncate_end(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[s.len() - max..]
    }
}
