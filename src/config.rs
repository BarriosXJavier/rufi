use crate::theme;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Deserialize, Serialize, Debug, Clone, Copy)]
pub struct Theme {
    pub bg_color: u32,
    pub fg_color: u32,
    pub selected_bg: u32,
    pub selected_fg: u32,
    pub border_color: u32,
    pub query_bg: u32,
    pub accent_color: u32,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Config {
    pub theme_name: Option<String>,
    pub font: String,
    pub font_size: u16,
    pub width: u16,
    pub height: u16,
    pub item_height: u16,
    pub padding: u16,
    pub border_width: u16,
    pub corner_radius: u16,
    pub max_results: usize,
    pub show_descriptions: bool,
    pub show_icons: bool,
    pub cache_timeout: u64, // timeout in secs
    pub theme: Theme,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme_name: Some("catppuccin-mocha".to_string()),
            font: "JetBrains Mono".into(),
            font_size: 18,
            width: 800,
            height: 500,
            item_height: 64,
            padding: 16,
            border_width: 2,
            corner_radius: 12,
            max_results: 50,
            show_descriptions: true,
            show_icons: true,
            cache_timeout: 300,
            theme: Theme {
                bg_color: 0x1e1e2e,      // catppuccin mocha base
                fg_color: 0xcdd6f4,      // catppuccin mocha text
                selected_bg: 0x89b4fa,   // catppuccin mocha blue
                selected_fg: 0x1e1e2e,   // catppuccin mocha base
                border_color: 0x6c7086,  // catppuccin mocha surface2
                query_bg: 0x313244,      // catppuccin mocha surface0
                accent_color: 0xf38ba8,  // catppuccin mocha pink
            },
        }
    }
}

impl Config {
    pub fn load(path: &str) -> Self {
        match fs::read_to_string(path) {
            Ok(data) => {
                let mut cfg: Config = toml::from_str(&data).unwrap_or_default();
                cfg.resolve_theme();
                cfg
            }
            Err(_) => {
                let mut cfg = Self::default();
                cfg.resolve_theme();
                cfg
            }
        }
    }

    pub fn resolve_theme(&mut self) {
        if let Some(theme_name) = &self.theme_name {
            if let Some(theme) = theme::get_theme(theme_name) {
                self.theme = theme;
            }
        }
    }
}
