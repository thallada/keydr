use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_target_wpm")]
    pub target_wpm: u32,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_keyboard_layout")]
    pub keyboard_layout: String,
    #[serde(default = "default_word_count")]
    pub word_count: usize,
    #[serde(default = "default_code_language")]
    pub code_language: String,
}

fn default_target_wpm() -> u32 {
    35
}
fn default_theme() -> String {
    "terminal-default".to_string()
}
fn default_keyboard_layout() -> String {
    "qwerty".to_string()
}
fn default_word_count() -> usize {
    20
}
fn default_code_language() -> String {
    "rust".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            target_wpm: default_target_wpm(),
            theme: default_theme(),
            keyboard_layout: default_keyboard_layout(),
            word_count: default_word_count(),
            code_language: default_code_language(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    #[allow(dead_code)]
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }

    fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("keydr")
            .join("config.toml")
    }

    pub fn target_cpm(&self) -> f64 {
        self.target_wpm as f64 * 5.0
    }
}
