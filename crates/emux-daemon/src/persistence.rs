//! Session persistence and restoration.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Serializable snapshot of a pane, including scrollback.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PaneSnapshot {
    /// Pane identifier.
    pub id: u32,
    /// Pane title (e.g. from OSC 0/2).
    pub title: String,
    /// Lines of text stored in the scrollback buffer.
    pub scrollback: Vec<String>,
    /// Working directory reported by OSC 7 (if available).
    #[serde(default)]
    pub working_directory: Option<String>,
    /// Scroll offset (0 = bottom).
    #[serde(default)]
    pub scroll_offset: usize,
}

/// Serializable snapshot of a tab.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TabSnapshot {
    /// Tab display name.
    pub name: String,
    /// Snapshots of all panes within this tab.
    pub panes: Vec<PaneSnapshot>,
    /// Which pane was focused in this tab.
    #[serde(default)]
    pub active_pane_id: Option<u32>,
}

/// Serializable snapshot of a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshot {
    /// Session name.
    pub name: String,
    /// Terminal width in columns.
    pub cols: usize,
    /// Terminal height in rows.
    pub rows: usize,
    /// Total number of tabs.
    pub tab_count: usize,
    /// Ordered list of tab names.
    pub tab_names: Vec<String>,
    #[serde(default)]
    pub tabs: Vec<TabSnapshot>,
    /// Which tab was active when the snapshot was taken.
    #[serde(default)]
    pub active_tab_index: usize,
}

/// Metadata about a persisted session (returned by `list_sessions`).
#[derive(Debug, Clone)]
pub struct SessionInfo {
    /// Session name extracted from the snapshot.
    pub name: String,
    /// File path to the snapshot JSON file.
    pub path: std::path::PathBuf,
}

/// Return the default sessions directory: `~/.local/share/emux/sessions/`.
pub fn sessions_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from).map(|home| {
        home.join(".local")
            .join("share")
            .join("emux")
            .join("sessions")
    })
}

/// Return the default snapshot path for a session name.
pub fn default_snapshot_path(session_name: &str) -> Option<PathBuf> {
    sessions_dir().map(|dir| dir.join(format!("{session_name}.json")))
}

/// Create a [`SessionSnapshot`] from a live [`emux_mux::Session`].
pub fn snapshot_from_session(session: &emux_mux::Session) -> SessionSnapshot {
    let mut tabs = Vec::new();
    for i in 0..session.tab_count() {
        if let Some(tab) = session.tab(i) {
            let mut panes = Vec::new();
            for pane_id in tab.pane_ids() {
                if let Some(pane) = tab.pane(pane_id) {
                    panes.push(PaneSnapshot {
                        id: pane_id,
                        title: pane.title().to_owned(),
                        scrollback: pane.scrollback().to_vec(),
                        working_directory: None, // Caller can fill this in
                        scroll_offset: pane.scroll_offset(),
                    });
                }
            }
            tabs.push(TabSnapshot {
                name: tab.name().to_owned(),
                panes,
                active_pane_id: tab.active_pane_id(),
            });
        }
    }
    SessionSnapshot {
        name: session.name().to_owned(),
        cols: session.size().cols,
        rows: session.size().rows,
        tab_count: session.tab_count(),
        tab_names: session
            .tab_names()
            .into_iter()
            .map(|s| s.to_owned())
            .collect(),
        tabs,
        active_tab_index: session.active_tab_index(),
    }
}

/// Restore a [`emux_mux::Session`] from a [`SessionSnapshot`].
pub fn session_from_snapshot(snap: &SessionSnapshot) -> emux_mux::Session {
    let mut session = emux_mux::Session::new(&snap.name, snap.cols, snap.rows);
    // The constructor already creates one default tab; rename it if we have tab data.
    if let Some(first_name) = snap.tab_names.first()
        && let Some(tab) = session.tab_mut(0)
    {
        tab.rename(first_name.as_str());
    }
    // Create additional tabs.
    for name in snap.tab_names.iter().skip(1) {
        session.new_tab(name.as_str());
    }
    // Restore the active tab index.
    if snap.active_tab_index < session.tab_count() {
        session.switch_tab(snap.active_tab_index);
    }
    session
}

/// Save a session snapshot to a JSON file.
///
/// The parent directory is created automatically if it does not exist.
pub fn save_session(session: &emux_mux::Session, path: &Path) -> Result<(), io::Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let snap = snapshot_from_session(session);
    let json = serde_json::to_string_pretty(&snap).map_err(io::Error::other)?;
    // Atomic-ish write: write to a temp file then rename.
    let tmp_path = path.with_extension("json.tmp");
    std::fs::write(&tmp_path, &json)?;
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

/// Load a session snapshot from a JSON file (without building a Session).
pub fn load_snapshot(path: &Path) -> Result<SessionSnapshot, io::Error> {
    let json = std::fs::read_to_string(path)?;
    serde_json::from_str(&json).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Load a session from a JSON snapshot file.
pub fn load_session(path: &Path) -> Result<emux_mux::Session, io::Error> {
    let snap = load_snapshot(path)?;
    Ok(session_from_snapshot(&snap))
}

/// List all `*.json` session snapshots in a directory.
pub fn list_sessions(base_dir: &Path) -> Vec<SessionInfo> {
    let mut infos = Vec::new();
    let entries = match std::fs::read_dir(base_dir) {
        Ok(e) => e,
        Err(_) => return infos,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json")
            && let Ok(contents) = std::fs::read_to_string(&path)
            && let Ok(snap) = serde_json::from_str::<SessionSnapshot>(&contents)
        {
            infos.push(SessionInfo {
                name: snap.name,
                path,
            });
        }
    }
    infos
}
