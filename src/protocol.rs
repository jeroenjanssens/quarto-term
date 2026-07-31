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
    #[serde(default)]
    pub fontsize: Option<String>,
    #[serde(default)]
    pub theme_bg: Option<String>,
    #[serde(default)]
    pub theme_fg: Option<String>,
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default)]
    pub typing: TypingConfig,
    #[serde(default)]
    pub record: Option<String>,
    #[serde(default)]
    pub verbose: bool,
    #[serde(default)]
    pub trailing_spaces: bool,
    #[serde(default)]
    #[allow(dead_code)]
    pub marker: Option<String>,
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
    #[serde(default = "default_echo")]
    pub echo: EchoMode,
    #[serde(default = "default_true")]
    pub output: bool,
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default)]
    pub scroll: Option<bool>,
    #[serde(default)]
    pub keep_last_prompt: bool,
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
    #[serde(default)]
    pub callouts: Vec<AnnotationSpec>,
    #[serde(default)]
    pub remove: Vec<AnnotationSpec>,
    #[serde(default = "default_highlight")]
    pub highlight: HighlightSpec,
}

impl fmt::Display for CellOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        match &self.echo {
            EchoMode::Mode(m) if m != "terminal" => parts.push(format!("echo: {}", m)),
            EchoMode::Bool(b) => parts.push(format!("echo: {}", b)),
            _ => {}
        }
        if !self.output { parts.push("output: false".to_string()); }
        if self.fullscreen { parts.push("fullscreen: true".to_string()); }
        if let Some(s) = self.scroll { parts.push(format!("scroll: {}", s)); }
        if self.keep_last_prompt { parts.push("keep-last-prompt: true".to_string()); }
        if let Some(a) = self.ansi { parts.push(format!("ansi: {}", a)); }
        if let Some(s) = self.spacing { parts.push(format!("spacing: {}", s)); }
        if let Some(ref t) = self.typing {
            match t {
                TypingConfig::Disabled(false) => parts.push("typing: false".to_string()),
                TypingConfig::Enabled { speed, error_rate, .. } => {
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
pub enum AnnotationSpec {
    Index(i32),
    Pattern(String),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum HighlightSpec {
    Bool(bool),
    Language(String),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
#[allow(dead_code)]
pub enum TypingConfig {
    Disabled(bool),
    Enabled {
        #[serde(default = "default_typing_mode")]
        #[allow(dead_code)]
        mode: String,
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
}

#[derive(Debug, Serialize)]
pub struct CellResult {
    pub id: u32,
    pub html: String,
    pub error: Option<String>,
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


fn default_echo() -> EchoMode {
    EchoMode::Mode("terminal".to_string())
}

fn default_highlight() -> HighlightSpec {
    HighlightSpec::Language("bash".to_string())
}

fn default_typing_mode() -> String {
    "human".to_string()
}

fn default_speed() -> f64 {
    60.0
}
