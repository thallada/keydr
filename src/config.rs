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
    #[serde(default = "default_passage_book")]
    pub passage_book: String,
    #[serde(default = "default_passage_downloads_enabled")]
    pub passage_downloads_enabled: bool,
    #[serde(default = "default_passage_download_dir")]
    pub passage_download_dir: String,
    #[serde(default = "default_passage_paragraphs_per_book")]
    pub passage_paragraphs_per_book: usize,
    #[serde(default = "default_passage_onboarding_done")]
    pub passage_onboarding_done: bool,
    #[serde(default = "default_code_downloads_enabled")]
    pub code_downloads_enabled: bool,
    #[serde(default = "default_code_download_dir")]
    pub code_download_dir: String,
    #[serde(default = "default_code_snippets_per_repo")]
    pub code_snippets_per_repo: usize,
    #[serde(default = "default_code_onboarding_done")]
    pub code_onboarding_done: bool,
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
fn default_passage_book() -> String {
    "all".to_string()
}
fn default_passage_downloads_enabled() -> bool {
    false
}
fn default_passage_download_dir() -> String {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("keydr")
        .join("passages")
        .to_string_lossy()
        .to_string()
}
fn default_passage_paragraphs_per_book() -> usize {
    100
}
fn default_passage_onboarding_done() -> bool {
    false
}
fn default_code_downloads_enabled() -> bool {
    false
}
fn default_code_download_dir() -> String {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("keydr")
        .join("code")
        .to_string_lossy()
        .to_string()
}
fn default_code_snippets_per_repo() -> usize {
    200
}
fn default_code_onboarding_done() -> bool {
    false
}

impl Default for Config {
    fn default() -> Self {
        Self {
            target_wpm: default_target_wpm(),
            theme: default_theme(),
            keyboard_layout: default_keyboard_layout(),
            word_count: default_word_count(),
            code_language: default_code_language(),
            passage_book: default_passage_book(),
            passage_downloads_enabled: default_passage_downloads_enabled(),
            passage_download_dir: default_passage_download_dir(),
            passage_paragraphs_per_book: default_passage_paragraphs_per_book(),
            passage_onboarding_done: default_passage_onboarding_done(),
            code_downloads_enabled: default_code_downloads_enabled(),
            code_download_dir: default_code_download_dir(),
            code_snippets_per_repo: default_code_snippets_per_repo(),
            code_onboarding_done: default_code_onboarding_done(),
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

    /// Validate `code_language` against known options, resetting to default if invalid.
    /// Call after deserialization to handle stale/renamed keys from old configs.
    pub fn normalize_code_language(&mut self, valid_keys: &[&str]) {
        // Backwards compatibility: old "shell" key is now "bash".
        if self.code_language == "shell" {
            self.code_language = "bash".to_string();
        }
        if !valid_keys.contains(&self.code_language.as_str()) {
            self.code_language = default_code_language();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_serde_defaults_from_empty() {
        // Simulates loading an old config file with no code drill fields
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config.code_downloads_enabled, false);
        assert_eq!(config.code_snippets_per_repo, 200);
        assert_eq!(config.code_onboarding_done, false);
        assert!(!config.code_download_dir.is_empty());
        assert!(config.code_download_dir.contains("code"));
    }

    #[test]
    fn test_config_serde_defaults_from_old_fields_only() {
        // Simulates a config file that only has pre-existing fields
        let toml_str = r#"
target_wpm = 60
theme = "monokai"
code_language = "go"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.target_wpm, 60);
        assert_eq!(config.theme, "monokai");
        assert_eq!(config.code_language, "go");
        // New fields should have defaults
        assert_eq!(config.code_downloads_enabled, false);
        assert_eq!(config.code_snippets_per_repo, 200);
        assert_eq!(config.code_onboarding_done, false);
    }

    #[test]
    fn test_config_serde_roundtrip() {
        let config = Config::default();
        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();
        assert_eq!(config.code_downloads_enabled, deserialized.code_downloads_enabled);
        assert_eq!(config.code_download_dir, deserialized.code_download_dir);
        assert_eq!(config.code_snippets_per_repo, deserialized.code_snippets_per_repo);
        assert_eq!(config.code_onboarding_done, deserialized.code_onboarding_done);
    }

    #[test]
    fn test_normalize_code_language_valid_key_unchanged() {
        let mut config = Config::default();
        config.code_language = "python".to_string();
        let valid_keys = vec!["rust", "python", "javascript", "go", "all"];
        config.normalize_code_language(&valid_keys);
        assert_eq!(config.code_language, "python");
    }

    #[test]
    fn test_normalize_code_language_invalid_key_resets() {
        let mut config = Config::default();
        config.code_language = "haskell".to_string();
        let valid_keys = vec!["rust", "python", "javascript", "go", "all"];
        config.normalize_code_language(&valid_keys);
        assert_eq!(config.code_language, "rust");
    }

    #[test]
    fn test_normalize_code_language_empty_string_resets() {
        let mut config = Config::default();
        config.code_language = String::new();
        let valid_keys = vec!["rust", "python", "javascript", "go", "all"];
        config.normalize_code_language(&valid_keys);
        assert_eq!(config.code_language, "rust");
    }

    #[test]
    fn test_normalize_code_language_shell_maps_to_bash() {
        let mut config = Config::default();
        config.code_language = "shell".to_string();
        let valid_keys = vec!["rust", "python", "javascript", "go", "bash", "all"];
        config.normalize_code_language(&valid_keys);
        assert_eq!(config.code_language, "bash");
    }
}
