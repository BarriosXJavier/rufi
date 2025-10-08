use crate::config::ConfigTheme;

pub fn get_theme(name: &str) -> Option<ConfigTheme> {
    match name {
        "catppuccin-mocha" => Some(ConfigTheme {
            bg_color: 0x1e1e2e,
            fg_color: 0xcdd6f4,
            selected_bg: 0x89b4fa,
            selected_fg: 0x1e1e2e,
            border_color: 0x6c7086,
            query_bg: 0x313244,
            accent_color: 0xf38ba8,
        }),
        "catppuccin-latte" => Some(ConfigTheme {
            bg_color: 0xeff1f5,
            fg_color: 0x4c4f69,
            selected_bg: 0x1e66f5,
            selected_fg: 0xeff1f5,
            border_color: 0xacb0be,
            query_bg: 0xccd0da,
            accent_color: 0xd20f39,
        }),
        "nord-dark" => Some(ConfigTheme {
            bg_color: 0x2E3440,
            fg_color: 0xD8DEE9,
            selected_bg: 0x88C0D0,
            selected_fg: 0x2E3440,
            border_color: 0x4C566A,
            query_bg: 0x3B4252,
            accent_color: 0x8FBCBB,
        }),
        "nord-light" => Some(ConfigTheme {
            bg_color: 0xECEFF4,
            fg_color: 0x2E3440,
            selected_bg: 0x88C0D0,
            selected_fg: 0x2E3440,
            border_color: 0xD8DEE9,
            query_bg: 0xE5E9F0,
            accent_color: 0x81A1C1,
        }),
        "dracula" => Some(ConfigTheme {
            bg_color: 0x282a36,
            fg_color: 0xf8f8f2,
            selected_bg: 0xbd93f9,
            selected_fg: 0x282a36,
            border_color: 0x44475a,
            query_bg: 0x44475a,
            accent_color: 0xff79c6,
        }),
        "tokyonight-dark" => Some(ConfigTheme {
            bg_color: 0x1a1b26,
            fg_color: 0xa9b1d6,
            selected_bg: 0x7aa2f7,
            selected_fg: 0x1a1b26,
            border_color: 0x414868,
            query_bg: 0x24283b,
            accent_color: 0xbb9af7,
        }),
        "tokyonight-light" => Some(ConfigTheme {
            bg_color: 0xd5d6db,
            fg_color: 0x343b58,
            selected_bg: 0x3454a4,
            selected_fg: 0xd5d6db,
            border_color: 0x9699a3,
            query_bg: 0xc8c9ce,
            accent_color: 0x8c73cc,
        }),
        "gruvbox-dark" => Some(ConfigTheme {
            bg_color: 0x282828,
            fg_color: 0xebdbb2,
            selected_bg: 0x83a598,
            selected_fg: 0x282828,
            border_color: 0x504945,
            query_bg: 0x3c3836,
            accent_color: 0xfe8019,
        }),
        "gruvbox-light" => Some(ConfigTheme {
            bg_color: 0xfbf1c7,
            fg_color: 0x3c3836,
            selected_bg: 0x83a598,
            selected_fg: 0xfbf1c7,
            border_color: 0xbdae93,
            query_bg: 0xebdbb2,
            accent_color: 0xd65d0e,
        }),
        _ => None,
    }
}

pub fn list_themes() -> Vec<&'static str> {
    vec![
        "catppuccin-mocha",
        "catppuccin-latte",
        "nord-dark",
        "nord-light",
        "dracula",
        "tokyonight-dark",
        "tokyonight-light",
        "gruvbox-dark",
        "gruvbox-light",
    ]
}
