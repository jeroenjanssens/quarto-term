use regex::Regex;

use crate::error::TermError;
use crate::protocol::Config;
use crate::renderer::{self, RenderedLine};

pub struct PromptMatcher {
    pub prompt_re: Regex,
    pub prompt_prefix_re: Regex,
    pub ps2_re: Option<Regex>,
    pub ps2_prefix_re: Option<Regex>,
}

impl PromptMatcher {
    pub fn new(config: &Config) -> Result<Self, TermError> {
        let prompt_pattern = match &config.prompt_regex {
            Some(re) => re.clone(),
            None => format!("{}\\s*$", regex::escape(&config.prompt)),
        };
        let prompt_re = Regex::new(&prompt_pattern)
            .map_err(|e| TermError::RegexCompile(e.to_string()))?;

        let prompt_prefix_pattern = match &config.prompt_regex {
            Some(re) => format!("^(?:{})", re.trim_end_matches('$')),
            None => format!("^{}\\s?", regex::escape(&config.prompt)),
        };
        let prompt_prefix_re = Regex::new(&prompt_prefix_pattern).unwrap_or_else(|_| {
            Regex::new(&format!("^{}\\s?", regex::escape(&config.prompt))).unwrap()
        });

        let ps2_re = if let Some(ref re) = config.ps2_regex {
            Some(Regex::new(re).map_err(|e| TermError::RegexCompile(e.to_string()))?)
        } else {
            config
                .ps2
                .as_ref()
                .map(|s| {
                    let escaped = regex::escape(s.trim_end());
                    Regex::new(&format!("^{}\\s*$", escaped))
                })
                .transpose()
                .map_err(|e| TermError::RegexCompile(e.to_string()))?
        };

        let ps2_prefix_re = if let Some(ref re) = config.ps2_regex {
            Regex::new(&format!("^(?:{})", re.trim_end_matches('$'))).ok()
        } else {
            config
                .ps2
                .as_ref()
                .and_then(|s| {
                    let escaped = regex::escape(s.trim_end());
                    Regex::new(&format!("^{}\\s?", escaped)).ok()
                })
        };

        Ok(Self { prompt_re, prompt_prefix_re, ps2_re, ps2_prefix_re })
    }

    pub fn is_primary_prompt(&self, text: &str) -> bool {
        self.prompt_re.is_match(text)
    }

    pub fn matches_any_prompt(&self, text: &str) -> bool {
        if self.prompt_re.is_match(text) {
            return true;
        }
        if let Some(ref ps2) = self.ps2_re {
            return ps2.is_match(text);
        }
        false
    }

    pub fn matches_ps2(&self, text: &str) -> bool {
        if let Some(ref ps2) = self.ps2_re {
            ps2.is_match(text)
        } else {
            false
        }
    }

    pub fn is_only_prompt(&self, text: &str) -> bool {
        if let Some(m) = self.prompt_re.find(text) {
            if m.end() >= text.len() {
                return true;
            }
        }
        false
    }

    pub fn mark_prompts(&self, lines: &mut [RenderedLine]) {
        for line in lines.iter_mut() {
            let prompt_len = if let Some(m) = self.prompt_prefix_re.find(&line.text) {
                m.end()
            } else if let Some(ref ps2) = self.ps2_prefix_re {
                if let Some(m) = ps2.find(&line.text) {
                    m.end()
                } else {
                    0
                }
            } else {
                0
            };

            if prompt_len > 0 {
                let prompt_text = &line.text[..prompt_len];
                let char_len = prompt_text.chars().count();
                line.markup = renderer::wrap_prompt_span(&line.markup, char_len, prompt_text);
            } else {
                line.markup = format!("<span class=\"term-output\">{}</span>", line.markup);
            }
        }
    }
}
