//! IPC message types and serialization.

use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;

/// Direction for splitting a pane.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

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

    // -- Agent / AI team support --

    /// Split the focused pane. Returns `SpawnResult` with the new pane ID.
    SplitPane { direction: SplitDirection, size: Option<u16> },
    /// Capture the visible content of a pane as text.
    CapturePane { pane_id: u32 },
    /// Send text/keystrokes to a specific pane.
    SendKeys { pane_id: u32, keys: String },
    /// List all panes with their IDs, titles, dimensions, and active state.
    ListPanes,
    /// Get detailed info about a specific pane.
    GetPaneInfo { pane_id: u32 },
    /// Resize a specific pane.
    ResizePane { pane_id: u32, cols: u16, rows: u16 },
    /// Set the title of a pane.
    SetPaneTitle { pane_id: u32, title: String },
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

/// Metadata about a pane, returned by `ListPanes` and `GetPaneInfo`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaneEntry {
    /// Pane identifier.
    pub id: u32,
    /// Pane title (user-settable or process-derived).
    pub title: String,
    /// Width in columns.
    pub cols: u16,
    /// Height in rows.
    pub rows: u16,
    /// Whether this pane currently has focus.
    pub active: bool,
    /// Whether this pane has an unread notification/bell.
    pub has_notification: bool,
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

    // -- Agent / AI team support --

    /// Response to `CapturePane`: the visible text content of the pane.
    PaneCaptured { pane_id: u32, content: String },
    /// Response to `ListPanes`: all panes in the active tab.
    PaneList { panes: Vec<PaneEntry> },
    /// Response to `GetPaneInfo`: info about a single pane.
    PaneInfo { pane: PaneEntry },
}
