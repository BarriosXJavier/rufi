use clap::Parser;
use std::fs;
use x11rb::rust_connection::RustConnection;

mod commands;
mod config;
mod error;
mod fuzzy;
mod theme;
mod ui;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    theme: Option<String>,
    #[arg(long = "available-themes")]
    available_themes: bool,
}

fn load_or_create_config(cfg_path: Option<std::path::PathBuf>) -> Result<config::Config, error::LauncherError> {
    if let Some(path) = &cfg_path {
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        if !path.exists() {
            let default_cfg = config::Config::default();
            let toml_str = toml::to_string(&default_cfg)?;
            fs::write(path, toml_str)?;
        }
    }

    let mut cfg = if let Some(path) = &cfg_path {
        config::Config::load(path.to_str().expect("Could not convert config path to string"))
    } else {
        config::Config::default()
    };
    Ok(cfg)
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

    let mut cfg = load_or_create_config(cfg_path.clone())?;

    if let Some(theme_name) = args.theme {
        cfg.theme_name = Some(theme_name);
        cfg.resolve_theme();

        // Save the theme to the config file
        if let Some(path) = &cfg_path {
            let toml_str = toml::to_string(&cfg)?;
            fs::write(path, toml_str)?;
            println!("Theme '{}' saved to {}", cfg.theme_name.clone().expect("Theme name should be set if we are saving it"), path.display());
        } else {
            eprintln!("Could not determine config path to save theme.");
        }
        // Do not return here, continue to launch UI
    }

    let (conn, screen_num) = RustConnection::connect(None)?;
    ui::run_ui(cfg, conn, screen_num)
}
