use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Deserialize)]
pub struct BatchRequest {
    pub config: Config,
    pub cells: Vec<InputCell>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_shell")]
    pub shell: String,
    #[serde(default)]
    pub shell_args: Vec<String>,
    #[serde(default = "default_prompt")]
    pub prompt: String,
    #[serde(default)]
    pub prompt_regex: Option<String>,
    #[serde(default)]
    pub ps2: Option<String>,
    #[serde(default)]
    pub ps2_regex: Option<String>,
    #[serde(default = "default_cols")]
    pub cols: u16,
    #[serde(default = "default_rows")]
    pub rows: u16,
    #[serde(default = "default_true")]
    pub ansi: bool,
    #[serde(default = "default_timeout")]
    pub timeout: f64,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub init: Vec<String>,
    #[serde(default)]
    pub spacing: bool,
    #[serde(default = "default_delay")]
    pub delay: f64,
    #[serde(default)]
    pub hold: f64,
    #[serde(default = "default_echo")]
    pub echo: EchoMode,
    #[serde(default)]
    pub keep_last_prompt: bool,
    #[serde(default = "default_highlight")]
    pub highlight: HighlightSpec,
    #[serde(default, alias = "font")]
    pub font_family: Option<String>,
    #[serde(default, alias = "fontsize")]
    pub font_size: Option<String>,
    #[serde(default)]
    pub line_height: Option<String>,
    #[serde(default)]
    pub theme_bg: Option<String>,
    #[serde(default)]
    pub theme_fg: Option<String>,
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default)]
    pub typing: TypingConfig,
    #[serde(default)]
    pub record: Vec<String>,
    #[serde(default)]
    pub verbose: bool,
    #[serde(default)]
    pub trailing_spaces: bool,
    #[serde(default)]
    pub disable_auto_indent: bool,
    #[serde(default)]
    pub docker: Option<DockerConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DockerConfig {
    pub image: String,
    #[serde(default = "default_pull_policy")]
    pub pull: String,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub workdir: Option<String>,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub network: Option<String>,
    #[serde(default)]
    pub memory: Option<String>,
    #[serde(default)]
    pub cpus: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub ports: Vec<String>,
    #[serde(default)]
    pub volumes: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct InputCell {
    pub id: u32,
    pub code: String,
    #[serde(default)]
    pub label: Option<String>,
    pub options: CellOptions,
    #[serde(default)]
    pub line_options: Vec<LineOptions>,
    #[serde(default)]
    pub source_lines: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CellOptions {
    #[serde(default)]
    pub echo: Option<EchoMode>,
    #[serde(default = "default_true")]
    pub output: bool,
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default)]
    pub scroll: Option<bool>,
    #[serde(default)]
    pub keep_last_prompt: Option<bool>,
    #[serde(default)]
    pub ansi: Option<bool>,
    #[serde(default)]
    pub spacing: Option<bool>,
    #[serde(default)]
    pub typing: Option<TypingConfig>,
    #[serde(default)]
    pub timeout: Option<f64>,
    #[serde(default)]
    pub hold: Option<f64>,
    #[serde(default)]
    pub trailing_spaces: Option<bool>,
    #[serde(default)]
    pub literal: Option<bool>,
    #[serde(default)]
    pub delay: Option<f64>,
    #[serde(default, alias = "font")]
    pub font_family: Option<String>,
    #[serde(default)]
    pub font_size: Option<String>,
    #[serde(default)]
    pub line_height: Option<String>,
    #[serde(default)]
    pub theme_bg: Option<String>,
    #[serde(default)]
    pub theme_fg: Option<String>,
    #[serde(default)]
    pub callouts: Vec<LineSpec>,
    #[serde(default)]
    pub remove: Vec<LineSpec>,
    #[serde(default)]
    pub truncate: Vec<LineSpec>,
    #[serde(default)]
    pub highlight: Option<HighlightSpec>,
    #[serde(default)]
    pub enter: Option<bool>,
    #[serde(default)]
    pub expect_prompt: Option<bool>,
}

impl fmt::Display for CellOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if let Some(echo) = &self.echo {
            match echo {
                EchoMode::Mode(m) if m != "terminal" => parts.push(format!("echo: {}", m)),
                EchoMode::Bool(b) => parts.push(format!("echo: {}", b)),
                _ => {}
            }
        }
        if !self.output { parts.push("output: false".to_string()); }
        if self.fullscreen { parts.push("fullscreen: true".to_string()); }
        if let Some(s) = self.scroll { parts.push(format!("scroll: {}", s)); }
        if self.keep_last_prompt == Some(true) { parts.push("keep-last-prompt: true".to_string()); }
        if let Some(a) = self.ansi { parts.push(format!("ansi: {}", a)); }
        if let Some(s) = self.spacing { parts.push(format!("spacing: {}", s)); }
        if let Some(ref t) = self.typing {
            match t {
                TypingConfig::Disabled(false) => parts.push("typing: false".to_string()),
                TypingConfig::Enabled { speed, error_rate } => {
                    parts.push(format!("typing: human, speed: {}, error-rate: {}", speed, error_rate));
                }
                _ => {}
            }
        }
        if let Some(t) = self.timeout { parts.push(format!("timeout: {}", t)); }
        if let Some(h) = self.hold { parts.push(format!("hold: {}", h)); }
        if let Some(ts) = self.trailing_spaces { parts.push(format!("trailing-spaces: {}", ts)); }
        if let Some(l) = self.literal { parts.push(format!("literal: {}", l)); }
        if let Some(d) = self.delay { parts.push(format!("delay: {}", d)); }
        if parts.is_empty() {
            write!(f, "no options")
        } else {
            write!(f, "{}", parts.join(", "))
        }
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum EchoMode {
    Bool(bool),
    Mode(String),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum LineSpec {
    Index(i32),
    Expr(String),
}


#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum HighlightSpec {
    Bool(bool),
    Language(String),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum TypingConfig {
    Disabled(bool),
    Enabled {
        #[serde(default = "default_speed")]
        speed: f64,
        #[serde(default)]
        error_rate: f64,
    },
}

impl Default for TypingConfig {
    fn default() -> Self {
        TypingConfig::Disabled(false)
    }
}

impl TypingConfig {
    pub fn is_enabled(&self) -> bool {
        matches!(self, TypingConfig::Enabled { .. })
    }

    pub fn speed(&self) -> f64 {
        match self {
            TypingConfig::Enabled { speed, .. } => *speed,
            _ => default_speed(),
        }
    }

    pub fn error_rate(&self) -> f64 {
        match self {
            TypingConfig::Enabled { error_rate, .. } => *error_rate,
            _ => 0.0,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct LineOptions {
    pub line_index: u32,
    #[serde(default)]
    pub literal: Option<bool>,
    #[serde(default)]
    pub enter: Option<bool>,
    #[serde(default)]
    pub delay: Option<f64>,
    #[serde(default)]
    pub hold: Option<f64>,
    #[serde(default)]
    pub expect_prompt: Option<bool>,
    #[serde(default)]
    pub timeout: Option<f64>,
    #[serde(default)]
    pub typing: Option<bool>,
    #[serde(default)]
    pub assert: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CellResult {
    pub id: u32,
    pub html: String,
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recorded_assertions: Vec<RecordedAssertion>,
}

#[derive(Debug, Serialize)]
pub struct RecordedAssertion {
    pub line_index: u32,
    pub output: String,
}

fn default_shell() -> String {
    "bash".to_string()
}

fn default_prompt() -> String {
    "$".to_string()
}

fn default_cols() -> u16 {
    80
}

fn default_rows() -> u16 {
    24
}

fn default_true() -> bool {
    true
}

fn default_timeout() -> f64 {
    10.0
}

fn default_format() -> String {
    "html".to_string()
}


fn default_delay() -> f64 {
    0.1
}

fn default_echo() -> EchoMode {
    EchoMode::Mode("terminal".to_string())
}

fn default_highlight() -> HighlightSpec {
    HighlightSpec::Language("bash".to_string())
}

fn default_speed() -> f64 {
    60.0
}

fn default_pull_policy() -> String {
    "missing".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_json() -> &'static str {
        r#"{"config":{},"cells":[{"id":1,"code":"echo hi","options":{},"line_options":[],"source_lines":[]}]}"#
    }

    #[test]
    fn batch_request_minimal_deserialization() {
        let req: BatchRequest = serde_json::from_str(minimal_json()).unwrap();
        assert_eq!(req.config.shell, "bash");
        assert_eq!(req.config.cols, 80);
        assert_eq!(req.config.rows, 24);
        assert_eq!(req.config.timeout, 10.0);
        assert_eq!(req.config.format, "html");
        assert!(req.config.record.is_empty());
        assert!(req.config.init.is_empty());
        assert_eq!(req.cells.len(), 1);
        assert_eq!(req.cells[0].code, "echo hi");
    }

    #[test]
    fn echo_mode_bool_false() {
        let v: EchoMode = serde_json::from_str("false").unwrap();
        assert_eq!(v, EchoMode::Bool(false));
    }

    #[test]
    fn echo_mode_bool_true() {
        let v: EchoMode = serde_json::from_str("true").unwrap();
        assert_eq!(v, EchoMode::Bool(true));
    }

    #[test]
    fn echo_mode_string() {
        let v: EchoMode = serde_json::from_str(r#""source""#).unwrap();
        assert_eq!(v, EchoMode::Mode("source".to_string()));
    }

    #[test]
    fn line_spec_index() {
        let v: LineSpec = serde_json::from_str("1").unwrap();
        assert!(matches!(v, LineSpec::Index(1)));
    }

    #[test]
    fn line_spec_expr_pattern() {
        let v: LineSpec = serde_json::from_str(r#""hello""#).unwrap();
        assert!(matches!(v, LineSpec::Expr(ref s) if s == "hello"));
    }

    #[test]
    fn line_spec_expr_range() {
        let v: LineSpec = serde_json::from_str(r#""3:7""#).unwrap();
        assert!(matches!(v, LineSpec::Expr(ref s) if s == "3:7"));
    }

    #[test]
    fn truncate_field_deserializes() {
        let json = r#"{"config":{},"cells":[{"id":1,"code":"x","options":{"truncate":["3:7",":5",-1]},"line_options":[],"source_lines":[]}]}"#;
        let req: BatchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.cells[0].options.truncate.len(), 3);
    }

    #[test]
    fn highlight_spec_bool() {
        let v: HighlightSpec = serde_json::from_str("false").unwrap();
        matches!(v, HighlightSpec::Bool(false));
    }

    #[test]
    fn highlight_spec_language() {
        let v: HighlightSpec = serde_json::from_str(r#""python""#).unwrap();
        matches!(v, HighlightSpec::Language(s) if s == "python");
    }

    #[test]
    fn typing_config_disabled() {
        let v: TypingConfig = serde_json::from_str("false").unwrap();
        assert!(!v.is_enabled());
    }

    #[test]
    fn typing_config_enabled() {
        let v: TypingConfig = serde_json::from_str(r#"{"speed":80.0,"error_rate":0.05}"#).unwrap();
        assert!(v.is_enabled());
        assert_eq!(v.speed(), 80.0);
        assert_eq!(v.error_rate(), 0.05);
    }

    #[test]
    fn record_empty_array_works() {
        let json = r#"{"config":{"record":[]},"cells":[]}"#;
        let req: BatchRequest = serde_json::from_str(json).unwrap();
        assert!(req.config.record.is_empty());
    }

    #[test]
    fn record_with_paths() {
        let json = r#"{"config":{"record":["out.cast","out.termshow"]},"cells":[]}"#;
        let req: BatchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.config.record, vec!["out.cast", "out.termshow"]);
    }

    #[test]
    fn cell_options_display_no_options() {
        let json = r#"{"config":{},"cells":[{"id":1,"code":"x","options":{},"line_options":[],"source_lines":[]}]}"#;
        let req: BatchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(format!("{}", req.cells[0].options), "no options");
    }

    #[test]
    fn cell_options_display_with_options() {
        let json = r#"{"config":{},"cells":[{"id":1,"code":"x","options":{"echo":false,"fullscreen":true},"line_options":[],"source_lines":[]}]}"#;
        let req: BatchRequest = serde_json::from_str(json).unwrap();
        let display = format!("{}", req.cells[0].options);
        assert!(display.contains("echo: false"));
        assert!(display.contains("fullscreen: true"));
    }

    #[test]
    fn line_options_all_fields() {
        let json = r#"{"line_index":2,"literal":false,"enter":true,"delay":0.5,"hold":1.0,"expect_prompt":false}"#;
        let lo: LineOptions = serde_json::from_str(json).unwrap();
        assert_eq!(lo.line_index, 2);
        assert_eq!(lo.literal, Some(false));
        assert_eq!(lo.enter, Some(true));
        assert_eq!(lo.delay, Some(0.5));
        assert_eq!(lo.hold, Some(1.0));
        assert_eq!(lo.expect_prompt, Some(false));
    }

    #[test]
    fn line_options_minimal() {
        let json = r#"{"line_index":0}"#;
        let lo: LineOptions = serde_json::from_str(json).unwrap();
        assert_eq!(lo.line_index, 0);
        assert_eq!(lo.literal, None);
        assert_eq!(lo.enter, None);
        assert_eq!(lo.delay, None);
        assert_eq!(lo.hold, None);
        assert_eq!(lo.expect_prompt, None);
    }

    #[test]
    fn docker_config_minimal() {
        let json = r#"{"config":{"docker":{"image":"python:3.12"}},"cells":[]}"#;
        let req: BatchRequest = serde_json::from_str(json).unwrap();
        let docker = req.config.docker.unwrap();
        assert_eq!(docker.image, "python:3.12");
        assert_eq!(docker.pull, "missing");
        assert!(docker.volumes.is_empty());
        assert!(docker.ports.is_empty());
        assert!(docker.args.is_empty());
        assert!(docker.platform.is_none());
        assert!(docker.workdir.is_none());
        assert!(docker.user.is_none());
        assert!(docker.network.is_none());
        assert!(docker.memory.is_none());
        assert!(docker.cpus.is_none());
        assert!(docker.name.is_none());
    }

    #[test]
    fn docker_config_full() {
        let json = r#"{"config":{"docker":{"image":"ubuntu:22.04","platform":"linux/amd64","pull":"always","workdir":"/app","user":"1000:1000","network":"none","memory":"256m","cpus":"0.5","name":"test-ctr","ports":["8080:8080"],"volumes":["/host/data:/data"],"args":["--read-only"],"env":{"FOO":"bar"}}},"cells":[]}"#;
        let req: BatchRequest = serde_json::from_str(json).unwrap();
        let docker = req.config.docker.unwrap();
        assert_eq!(docker.image, "ubuntu:22.04");
        assert_eq!(docker.platform.as_deref(), Some("linux/amd64"));
        assert_eq!(docker.pull, "always");
        assert_eq!(docker.workdir.as_deref(), Some("/app"));
        assert_eq!(docker.user.as_deref(), Some("1000:1000"));
        assert_eq!(docker.network.as_deref(), Some("none"));
        assert_eq!(docker.memory.as_deref(), Some("256m"));
        assert_eq!(docker.cpus.as_deref(), Some("0.5"));
        assert_eq!(docker.name.as_deref(), Some("test-ctr"));
        assert_eq!(docker.ports, vec!["8080:8080"]);
        assert_eq!(docker.volumes, vec!["/host/data:/data"]);
        assert_eq!(docker.args, vec!["--read-only"]);
        assert_eq!(docker.env.get("FOO").map(|s| s.as_str()), Some("bar"));
    }

    #[test]
    fn docker_config_absent() {
        let json = r#"{"config":{},"cells":[]}"#;
        let req: BatchRequest = serde_json::from_str(json).unwrap();
        assert!(req.config.docker.is_none());
    }

    #[test]
    fn docker_config_pull_default() {
        let json = r#"{"config":{"docker":{"image":"alpine"}},"cells":[]}"#;
        let req: BatchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.config.docker.unwrap().pull, "missing");
    }
}
