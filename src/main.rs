use std::fs;
use x11rb::rust_connection::RustConnection;
use clap::Parser;

mod commands;
mod config;
mod error;
mod fuzzy;
mod ui;
mod theme;

use config::Config;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    theme: Option<String>,
    #[arg(long = "available-themes")]
    available_themes: bool,
}

fn main() -> Result<(), error::LauncherError> {
    let args = Args::parse();

    if args.available_themes {
        println!("Available themes:");
        for theme in theme::list_themes() {
            println!("- {}", theme);
        }
        return Ok(());
    }

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

    let mut cfg = if let Some(path) = &cfg_path {
        Config::load(path.to_str().unwrap_or_default())
    } else {
        Config::default()
    };

    if let Some(theme_name) = args.theme {
        cfg.theme_name = Some(theme_name);
        cfg.resolve_theme();
    }

    let (conn, screen_num) = RustConnection::connect(None)?;
    ui::run_ui(cfg, conn, screen_num)
}
