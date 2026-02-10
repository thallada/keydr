use std::fs;

use ratatui::style::Color;
use rust_embed::Embed;
use serde::{Deserialize, Serialize};

#[derive(Embed)]
#[folder = "assets/themes/"]
struct ThemeAssets;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub colors: ThemeColors,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThemeColors {
    pub bg: String,
    pub fg: String,
    pub text_correct: String,
    pub text_incorrect: String,
    pub text_incorrect_bg: String,
    pub text_pending: String,
    pub text_cursor_bg: String,
    pub text_cursor_fg: String,
    pub focused_key: String,
    pub accent: String,
    pub accent_dim: String,
    pub border: String,
    pub border_focused: String,
    pub header_bg: String,
    pub header_fg: String,
    pub bar_filled: String,
    pub bar_empty: String,
    pub error: String,
    pub warning: String,
    pub success: String,
}

impl Theme {
    pub fn load(name: &str) -> Option<Self> {
        // Try user themes dir
        if let Some(config_dir) = dirs::config_dir() {
            let user_theme_path = config_dir.join("keydr").join("themes").join(format!("{name}.toml"));
            if let Ok(content) = fs::read_to_string(&user_theme_path) {
                if let Ok(theme) = toml::from_str::<Theme>(&content) {
                    return Some(theme);
                }
            }
        }

        // Try bundled themes
        let filename = format!("{name}.toml");
        if let Some(file) = ThemeAssets::get(&filename) {
            if let Ok(content) = std::str::from_utf8(file.data.as_ref()) {
                if let Ok(theme) = toml::from_str::<Theme>(content) {
                    return Some(theme);
                }
            }
        }

        None
    }

    pub fn available_themes() -> Vec<String> {
        ThemeAssets::iter()
            .filter_map(|f| {
                f.strip_suffix(".toml").map(|n| n.to_string())
            })
            .collect()
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::load("catppuccin-mocha").unwrap_or_else(|| Self {
            name: "default".to_string(),
            colors: ThemeColors::default(),
        })
    }
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            bg: "#1e1e2e".to_string(),
            fg: "#cdd6f4".to_string(),
            text_correct: "#a6e3a1".to_string(),
            text_incorrect: "#f38ba8".to_string(),
            text_incorrect_bg: "#45273a".to_string(),
            text_pending: "#585b70".to_string(),
            text_cursor_bg: "#f5e0dc".to_string(),
            text_cursor_fg: "#1e1e2e".to_string(),
            focused_key: "#f9e2af".to_string(),
            accent: "#89b4fa".to_string(),
            accent_dim: "#45475a".to_string(),
            border: "#45475a".to_string(),
            border_focused: "#89b4fa".to_string(),
            header_bg: "#313244".to_string(),
            header_fg: "#cdd6f4".to_string(),
            bar_filled: "#89b4fa".to_string(),
            bar_empty: "#313244".to_string(),
            error: "#f38ba8".to_string(),
            warning: "#f9e2af".to_string(),
            success: "#a6e3a1".to_string(),
        }
    }
}

impl ThemeColors {
    pub fn parse_color(hex: &str) -> Color {
        let hex = hex.trim_start_matches('#');
        if hex.len() == 6 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&hex[0..2], 16),
                u8::from_str_radix(&hex[2..4], 16),
                u8::from_str_radix(&hex[4..6], 16),
            ) {
                return Color::Rgb(r, g, b);
            }
        }
        Color::White
    }

    pub fn bg(&self) -> Color { Self::parse_color(&self.bg) }
    pub fn fg(&self) -> Color { Self::parse_color(&self.fg) }
    pub fn text_correct(&self) -> Color { Self::parse_color(&self.text_correct) }
    pub fn text_incorrect(&self) -> Color { Self::parse_color(&self.text_incorrect) }
    pub fn text_incorrect_bg(&self) -> Color { Self::parse_color(&self.text_incorrect_bg) }
    pub fn text_pending(&self) -> Color { Self::parse_color(&self.text_pending) }
    pub fn text_cursor_bg(&self) -> Color { Self::parse_color(&self.text_cursor_bg) }
    pub fn text_cursor_fg(&self) -> Color { Self::parse_color(&self.text_cursor_fg) }
    pub fn focused_key(&self) -> Color { Self::parse_color(&self.focused_key) }
    pub fn accent(&self) -> Color { Self::parse_color(&self.accent) }
    pub fn accent_dim(&self) -> Color { Self::parse_color(&self.accent_dim) }
    pub fn border(&self) -> Color { Self::parse_color(&self.border) }
    pub fn border_focused(&self) -> Color { Self::parse_color(&self.border_focused) }
    pub fn header_bg(&self) -> Color { Self::parse_color(&self.header_bg) }
    pub fn header_fg(&self) -> Color { Self::parse_color(&self.header_fg) }
    pub fn bar_filled(&self) -> Color { Self::parse_color(&self.bar_filled) }
    pub fn bar_empty(&self) -> Color { Self::parse_color(&self.bar_empty) }
    pub fn error(&self) -> Color { Self::parse_color(&self.error) }
    pub fn warning(&self) -> Color { Self::parse_color(&self.warning) }
    pub fn success(&self) -> Color { Self::parse_color(&self.success) }
}
