use std::fmt;

#[derive(Debug)]
pub enum TermError {
    SpawnFailed(String),
    PromptTimeout { elapsed_secs: f64, last_output: String },
    ShellExited,
    RegexCompile(String),
    InitFailed { command: String, reason: String },
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
            Self::InitFailed { command, reason } => write!(
                f,
                "init command {command:?} did not return to the prompt: {reason}\n  \
                 Hint: the shell may be waiting for more input, or the command \
                 may not be valid for this shell (is a project-level `init` \
                 meant for another shell?)"
            ),
        }
    }
}

impl std::error::Error for TermError {}

fn truncate_end(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[s.len() - max..]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_end_shorter_than_max() {
        assert_eq!(truncate_end("hello", 10), "hello");
    }

    #[test]
    fn truncate_end_exact_max() {
        assert_eq!(truncate_end("hello", 5), "hello");
    }

    #[test]
    fn truncate_end_longer_than_max() {
        assert_eq!(truncate_end("hello world", 5), "world");
    }

    #[test]
    fn truncate_end_empty() {
        assert_eq!(truncate_end("", 10), "");
    }

    #[test]
    fn display_spawn_failed() {
        let err = TermError::SpawnFailed("no such file".to_string());
        assert_eq!(err.to_string(), "failed to spawn shell: no such file");
    }

    #[test]
    fn display_prompt_timeout() {
        let err = TermError::PromptTimeout {
            elapsed_secs: 10.0,
            last_output: "some output".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("timeout waiting for prompt after 10.0s"));
        assert!(msg.contains("some output"));
    }

    #[test]
    fn display_shell_exited() {
        let err = TermError::ShellExited;
        assert_eq!(err.to_string(), "shell process exited unexpectedly");
    }

    #[test]
    fn display_init_failed() {
        let err = TermError::InitFailed {
            command: "SELECT 1".to_string(),
            reason: "the shell is at the continuation prompt".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.starts_with(
            "init command \"SELECT 1\" did not return to the prompt: \
             the shell is at the continuation prompt"
        ));
        assert!(msg.contains("Hint:"));
    }

    #[test]
    fn display_regex_compile() {
        let err = TermError::RegexCompile("bad pattern".to_string());
        assert_eq!(err.to_string(), "invalid prompt regex: bad pattern");
    }
}
