use thiserror::Error;
use x11rb::rust_connection::ConnectError;

#[derive(Debug, Error)]
pub enum LauncherError {
    #[error("X11 connection error: {0}")]
    X11Connection(#[from] x11rb::errors::ConnectionError),
    #[error("X11 connect error: {0}")]
    X11Connect(#[from] ConnectError),
    #[error("X11 reply error: {0}")]
    X11Reply(#[from] x11rb::errors::ReplyError),
    #[error("X11 reply or ID error: {0}")]
    X11ReplyOrId(#[from] x11rb::errors::ReplyOrIdError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parsing error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("X11 parsing error: {0}")]
    X11Parse(#[from] x11rb::errors::ParseError),
}
