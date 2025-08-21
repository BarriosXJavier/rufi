use x11rb::rust_connection::RustConnection;

mod commands;
mod config;
mod error;
mod fuzzy;
mod ui;

use config::Config;

fn main() -> Result<(), error::LauncherError> {
    let cfg_path = dirs::home_dir().unwrap_or_default().join(".rufirc");
    let cfg = Config::load(cfg_path.to_str().unwrap_or(".rufirc"));

    let (conn, screen_num) = RustConnection::connect(None)?;
    ui::run_ui(cfg, conn, screen_num)
}
