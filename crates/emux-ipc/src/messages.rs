//! IPC message types and serialization.

use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ClientMessage {
    Ping,
    GetVersion,
    KeyInput { data: Vec<u8> },
    Resize { cols: u16, rows: u16 },
    SpawnPane { direction: Option<String> },
    KillPane { pane_id: u32 },
    FocusPane { pane_id: u32 },
    Detach,
    ListSessions,
    KillSession { name: String },
}

/// Metadata about an active session returned by `ListSessions`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionEntry {
    /// Session name.
    pub name: String,
    /// Number of tabs in the session.
    pub tabs: usize,
    /// Number of panes in the active tab.
    pub panes: usize,
    /// Terminal width in columns.
    pub cols: usize,
    /// Terminal height in rows.
    pub rows: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ServerMessage {
    Pong,
    Version { version: u32 },
    Render { pane_id: u32, content: String },
    SpawnResult { pane_id: u32 },
    Error { message: String },
    Ack,
    SessionList { sessions: Vec<SessionEntry> },
}
