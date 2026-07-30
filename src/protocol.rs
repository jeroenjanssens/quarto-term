use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    pub spacing: bool,
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default)]
    pub record: Option<String>,
    #[serde(default)]
    pub verbose: bool,
}

#[derive(Debug, Deserialize)]
pub struct InputCell {
    pub id: u32,
    pub code: String,
    pub options: CellOptions,
    #[serde(default)]
    pub line_options: Vec<LineOptions>,
}

#[derive(Debug, Deserialize)]
pub struct CellOptions {
    #[serde(default = "default_echo")]
    pub echo: EchoMode,
    #[serde(default = "default_true")]
    pub output: bool,
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default = "default_true")]
    pub scroll: bool,
    #[serde(default)]
    pub keep_last_prompt: bool,
    #[serde(default)]
    pub ansi: Option<bool>,
    #[serde(default)]
    pub spacing: Option<bool>,
    #[serde(default)]
    pub callouts: Vec<AnnotationSpec>,
    #[serde(default)]
    pub remove: Vec<AnnotationSpec>,
    #[serde(default = "default_highlight")]
    pub highlight: HighlightSpec,
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
pub struct LineOptions {
    pub line_index: u32,
    #[serde(default = "default_true")]
    pub literal: bool,
    #[serde(default)]
    pub enter: Option<bool>,
    #[serde(default)]
    pub wait: f64,
    #[serde(default = "default_hold")]
    pub hold: f64,
    #[serde(default)]
    pub expect_prompt: Option<bool>,
}

impl LineOptions {
    pub fn effective_enter(&self) -> bool {
        self.enter.unwrap_or(self.literal)
    }

    pub fn effective_expect_prompt(&self) -> bool {
        self.expect_prompt.unwrap_or(self.effective_enter())
    }
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
    "[\\$#>]\\s*$".to_string()
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

fn default_hold() -> f64 {
    0.1
}

fn default_echo() -> EchoMode {
    EchoMode::Mode("terminal".to_string())
}

fn default_highlight() -> HighlightSpec {
    HighlightSpec::Language("bash".to_string())
}
