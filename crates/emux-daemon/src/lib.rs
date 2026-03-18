//! Daemon process: server, client connection handling, and persistence.

pub mod client;
pub mod persistence;
pub mod server;

use std::fmt;

/// Unique identifier for a connected client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientId(pub u64);

/// Errors produced by the daemon.
#[derive(Debug)]
pub enum DaemonError {
    Io(std::io::Error),
    Codec(emux_ipc::CodecError),
    SocketExists(String),
    NotConnected,
    InvalidClient(ClientId),
}

impl fmt::Display for DaemonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DaemonError::Io(e) => write!(f, "IO error: {e}"),
            DaemonError::Codec(e) => write!(f, "Codec error: {e}"),
            DaemonError::SocketExists(p) => write!(f, "Socket already exists: {p}"),
            DaemonError::NotConnected => write!(f, "Not connected"),
            DaemonError::InvalidClient(id) => write!(f, "Invalid client: {id:?}"),
        }
    }
}

impl std::error::Error for DaemonError {}

impl From<std::io::Error> for DaemonError {
    fn from(e: std::io::Error) -> Self {
        DaemonError::Io(e)
    }
}

impl From<emux_ipc::CodecError> for DaemonError {
    fn from(e: emux_ipc::CodecError) -> Self {
        DaemonError::Codec(e)
    }
}
