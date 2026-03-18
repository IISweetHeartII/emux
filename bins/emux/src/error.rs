//! Application-level error type for emux.

use std::io;

/// Top-level error type used throughout the emux binary.
#[derive(Debug)]
pub enum AppError {
    /// An I/O error.
    Io(io::Error),
    /// A PTY-related error.
    Pty(emux_pty::PtyError),
    /// A configuration error.
    Config(String),
    /// A daemon-related error.
    Daemon(String),
    /// A generic message error.
    Msg(String),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Io(e) => write!(f, "IO error: {e}"),
            AppError::Pty(e) => write!(f, "PTY error: {e}"),
            AppError::Config(msg) => write!(f, "config error: {msg}"),
            AppError::Daemon(msg) => write!(f, "daemon error: {msg}"),
            AppError::Msg(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AppError::Io(e) => Some(e),
            AppError::Pty(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for AppError {
    fn from(e: io::Error) -> Self {
        AppError::Io(e)
    }
}

impl From<emux_pty::PtyError> for AppError {
    fn from(e: emux_pty::PtyError) -> Self {
        AppError::Pty(e)
    }
}

impl From<Box<dyn std::error::Error>> for AppError {
    fn from(e: Box<dyn std::error::Error>) -> Self {
        AppError::Msg(e.to_string())
    }
}

impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError::Msg(s)
    }
}

impl From<&str> for AppError {
    fn from(s: &str) -> Self {
        AppError::Msg(s.to_owned())
    }
}
