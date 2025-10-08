use std::fs;
use x11rb::rust_connection::RustConnection;

mod commands;
mod config;
mod error;
mod fuzzy;
mod ui;

use config::Config;

fn main() -> Result<(), error::LauncherError> {
    let cfg_path = dirs::config_dir().map(|p| p.join("rufi").join("rufirc.toml"));

    if let Some(path) = &cfg_path {
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        if !path.exists() {
            let default_cfg = Config::default();
            let toml_str = toml::to_string(&default_cfg)?;
            fs::write(path, toml_str)?;
        }
    }

    let cfg = if let Some(path) = &cfg_path {
        Config::load(path.to_str().unwrap_or_default())
    } else {
        Config::default()
    };

    let (conn, screen_num) = RustConnection::connect(None)?;
    ui::run_ui(cfg, conn, screen_num)
}
