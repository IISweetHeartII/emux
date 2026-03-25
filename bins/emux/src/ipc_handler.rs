//! Shared IPC message handler used by both daemon and event_loop agent socket.
//!
//! This eliminates the duplication between `daemon::handle_ipc_message` and
//! `event_loop::handle_agent_message`. Both callers provide a `&mut Session`
//! and a `PaneAccess` implementation.

use emux_ipc::{ClientMessage, ServerMessage};
use emux_mux::{PaneId, Session, SplitDirection};

// ---------------------------------------------------------------------------
// PaneAccess trait — abstracts over App<P> vs daemon's HashMap<PaneId, PaneTerminal>
// ---------------------------------------------------------------------------

/// Trait providing PTY/Screen access for IPC message handling.
/// Both the standalone event loop (via `App`) and the daemon (via its own
/// `HashMap<PaneId, PaneTerminal>`) implement this.
pub(crate) trait PaneAccess {
    /// Capture the visible screen content of a pane as a string.
    fn capture_pane(&mut self, pane_id: PaneId) -> Option<String>;

    /// Write bytes to a pane's PTY. Returns an error message on failure.
    fn send_keys(&mut self, pane_id: PaneId, data: &[u8]) -> Result<(), String>;

    /// Remove a pane's PTY state (for KillPane).
    fn remove_pane(&mut self, pane_id: PaneId);

    /// Spawn a new PTY for a pane after a split. Returns Ok on success.
    fn spawn_pane(&mut self, pane_id: PaneId, cols: usize, rows: usize) -> Result<(), String>;
}

// ---------------------------------------------------------------------------
// Shared handler
// ---------------------------------------------------------------------------

/// Result from handle_ipc that may request side-effects the caller must perform.
pub(crate) struct IpcResult {
    pub response: ServerMessage,
    /// If true, the caller should sync PTY sizes after this message.
    #[cfg_attr(not(unix), allow(dead_code))]
    pub sync_sizes: bool,
}

impl IpcResult {
    fn reply(response: ServerMessage) -> Self {
        Self {
            response,
            sync_sizes: false,
        }
    }

    fn reply_with_sync(response: ServerMessage) -> Self {
        Self {
            response,
            sync_sizes: true,
        }
    }
}

/// Handle an IPC ClientMessage using the given session and pane access.
/// This is the single source of truth for IPC message → response mapping.
pub(crate) fn handle_ipc(
    session: &mut Session,
    panes: &mut dyn PaneAccess,
    msg: ClientMessage,
) -> IpcResult {
    match msg {
        ClientMessage::Ping => IpcResult::reply(ServerMessage::Pong),
        ClientMessage::GetVersion => IpcResult::reply(ServerMessage::Version {
            version: emux_ipc::PROTOCOL_VERSION,
        }),
        ClientMessage::Resize { cols, rows } => {
            session.resize(cols as usize, rows as usize);
            IpcResult::reply(ServerMessage::Ack)
        }
        ClientMessage::ListSessions => {
            let entry = emux_ipc::SessionEntry {
                name: session.name().to_owned(),
                tabs: session.tab_count(),
                panes: session.active_tab().pane_count(),
                cols: session.size().cols,
                rows: session.size().rows,
            };
            IpcResult::reply(ServerMessage::SessionList {
                sessions: vec![entry],
            })
        }
        ClientMessage::ListPanes => {
            let tab = session.active_tab();
            let positions = tab.compute_positions();
            let active = tab.active_pane_id();
            let pane_entries = positions
                .iter()
                .map(|(id, pos)| emux_ipc::PaneEntry {
                    id: *id,
                    title: tab
                        .pane(*id)
                        .map(|p| p.title().to_owned())
                        .unwrap_or_default(),
                    cols: pos.cols as u16,
                    rows: pos.rows as u16,
                    active: active == Some(*id),
                    has_notification: tab.pane(*id).map(|p| p.has_notification()).unwrap_or(false),
                })
                .collect();
            IpcResult::reply(ServerMessage::PaneList {
                panes: pane_entries,
            })
        }
        ClientMessage::GetPaneInfo { pane_id } => {
            let tab = session.active_tab();
            if let Some(pane) = tab.pane(pane_id) {
                let positions = tab.compute_positions();
                let (cols, rows) = positions
                    .iter()
                    .find(|(id, _)| *id == pane_id)
                    .map(|(_, p)| (p.cols as u16, p.rows as u16))
                    .unwrap_or((0, 0));
                IpcResult::reply(ServerMessage::PaneInfo {
                    pane: emux_ipc::PaneEntry {
                        id: pane_id,
                        title: pane.title().to_owned(),
                        cols,
                        rows,
                        active: tab.active_pane_id() == Some(pane_id),
                        has_notification: pane.has_notification(),
                    },
                })
            } else {
                IpcResult::reply(ServerMessage::Error {
                    message: format!("pane {pane_id} not found"),
                })
            }
        }
        ClientMessage::CapturePane { pane_id } => {
            if let Some(content) = panes.capture_pane(pane_id) {
                IpcResult::reply(ServerMessage::PaneCaptured { pane_id, content })
            } else {
                IpcResult::reply(ServerMessage::Error {
                    message: format!("pane {pane_id} not found"),
                })
            }
        }
        ClientMessage::SendKeys { pane_id, keys } => {
            match panes.send_keys(pane_id, keys.as_bytes()) {
                Ok(()) => IpcResult::reply(ServerMessage::Ack),
                Err(msg) => IpcResult::reply(ServerMessage::Error { message: msg }),
            }
        }
        ClientMessage::SetPaneTitle { pane_id, title } => {
            if let Some(pane) = session.active_tab_mut().pane_mut(pane_id) {
                pane.set_title(title);
                IpcResult::reply(ServerMessage::Ack)
            } else {
                IpcResult::reply(ServerMessage::Error {
                    message: format!("pane {pane_id} not found"),
                })
            }
        }
        ClientMessage::FocusPane { pane_id } => {
            if session.active_tab_mut().focus_pane(pane_id) {
                IpcResult::reply(ServerMessage::Ack)
            } else {
                IpcResult::reply(ServerMessage::Error {
                    message: format!("pane {pane_id} not found"),
                })
            }
        }
        ClientMessage::SplitPane { direction, .. } => {
            let dir = match direction {
                emux_ipc::SplitDirection::Horizontal => SplitDirection::Horizontal,
                emux_ipc::SplitDirection::Vertical => SplitDirection::Vertical,
            };
            match session.active_tab_mut().split_pane(dir) {
                Some(new_id) => {
                    let positions = session.active_tab().compute_positions();
                    let (cols, rows) = positions
                        .iter()
                        .find(|(id, _)| *id == new_id)
                        .map(|(_, p)| (p.cols, p.rows))
                        .unwrap_or((80, 24));
                    match panes.spawn_pane(new_id, cols, rows) {
                        Ok(()) => IpcResult::reply_with_sync(ServerMessage::SpawnResult {
                            pane_id: new_id,
                        }),
                        Err(e) => IpcResult::reply(ServerMessage::Error {
                            message: format!("spawn error: {e}"),
                        }),
                    }
                }
                None => IpcResult::reply(ServerMessage::Error {
                    message: "cannot split pane".into(),
                }),
            }
        }
        ClientMessage::SpawnPane { ref direction } => {
            let dir = match direction.as_deref() {
                Some("horizontal") => SplitDirection::Horizontal,
                _ => SplitDirection::Vertical,
            };
            match session.active_tab_mut().split_pane(dir) {
                Some(new_id) => {
                    let positions = session.active_tab().compute_positions();
                    let (cols, rows) = positions
                        .iter()
                        .find(|(id, _)| *id == new_id)
                        .map(|(_, p)| (p.cols, p.rows))
                        .unwrap_or((80, 24));
                    match panes.spawn_pane(new_id, cols, rows) {
                        Ok(()) => IpcResult::reply_with_sync(ServerMessage::SpawnResult {
                            pane_id: new_id,
                        }),
                        Err(e) => IpcResult::reply(ServerMessage::Error {
                            message: format!("spawn error: {e}"),
                        }),
                    }
                }
                None => IpcResult::reply(ServerMessage::Error {
                    message: "cannot split pane".into(),
                }),
            }
        }
        ClientMessage::KillPane { pane_id } => {
            if session.active_tab_mut().close_pane(pane_id) {
                panes.remove_pane(pane_id);
                IpcResult::reply_with_sync(ServerMessage::Ack)
            } else {
                IpcResult::reply(ServerMessage::Error {
                    message: format!("cannot kill pane {pane_id}"),
                })
            }
        }
        ClientMessage::ResizePane {
            pane_id,
            cols,
            rows,
        } => {
            let tab = session.active_tab_mut();
            if tab.pane(pane_id).is_none() {
                return IpcResult::reply(ServerMessage::Error {
                    message: format!("pane {pane_id} not found"),
                });
            }
            let positions = tab.compute_positions();
            if let Some((_, pos)) = positions.iter().find(|(id, _)| *id == pane_id) {
                let dc = cols as i32 - pos.cols as i32;
                let dr = rows as i32 - pos.rows as i32;
                if dc != 0 {
                    tab.resize_pane(pane_id, emux_mux::ResizeDirection::Right, dc);
                }
                if dr != 0 {
                    tab.resize_pane(pane_id, emux_mux::ResizeDirection::Down, dr);
                }
                IpcResult::reply_with_sync(ServerMessage::Ack)
            } else {
                IpcResult::reply(ServerMessage::Error {
                    message: format!("pane {pane_id} not found"),
                })
            }
        }
        // Connection lifecycle messages — handled by the caller's loop, not here.
        ClientMessage::Attach { .. }
        | ClientMessage::Detach
        | ClientMessage::KillSession { .. }
        | ClientMessage::KeyInput { .. } => IpcResult::reply(ServerMessage::Ack),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Minimal mock PaneAccess for testing.
    struct MockPanes {
        panes: HashMap<PaneId, String>, // pane_id → screen content
    }

    impl MockPanes {
        fn new() -> Self {
            let mut panes = HashMap::new();
            panes.insert(0, "hello world".to_string());
            Self { panes }
        }
    }

    impl PaneAccess for MockPanes {
        fn capture_pane(&mut self, pane_id: PaneId) -> Option<String> {
            self.panes.get(&pane_id).cloned()
        }

        fn send_keys(&mut self, pane_id: PaneId, _data: &[u8]) -> Result<(), String> {
            if self.panes.contains_key(&pane_id) {
                Ok(())
            } else {
                Err(format!("pane {pane_id} not found"))
            }
        }

        fn remove_pane(&mut self, pane_id: PaneId) {
            self.panes.remove(&pane_id);
        }

        fn spawn_pane(
            &mut self,
            pane_id: PaneId,
            _cols: usize,
            _rows: usize,
        ) -> Result<(), String> {
            self.panes.insert(pane_id, String::new());
            Ok(())
        }
    }

    fn test_session() -> Session {
        Session::new("test", 80, 24)
    }

    #[test]
    fn ipc_ping() {
        let mut s = test_session();
        let mut p = MockPanes::new();
        let r = handle_ipc(&mut s, &mut p, ClientMessage::Ping);
        assert!(matches!(r.response, ServerMessage::Pong));
    }

    #[test]
    fn ipc_capture_pane_found() {
        let mut s = test_session();
        let mut p = MockPanes::new();
        let r = handle_ipc(&mut s, &mut p, ClientMessage::CapturePane { pane_id: 0 });
        match r.response {
            ServerMessage::PaneCaptured { content, .. } => assert_eq!(content, "hello world"),
            other => panic!("expected PaneCaptured, got {other:?}"),
        }
    }

    #[test]
    fn ipc_capture_pane_not_found() {
        let mut s = test_session();
        let mut p = MockPanes::new();
        let r = handle_ipc(&mut s, &mut p, ClientMessage::CapturePane { pane_id: 999 });
        assert!(matches!(r.response, ServerMessage::Error { .. }));
    }

    #[test]
    fn ipc_send_keys_found() {
        let mut s = test_session();
        let mut p = MockPanes::new();
        let r = handle_ipc(
            &mut s,
            &mut p,
            ClientMessage::SendKeys {
                pane_id: 0,
                keys: "hello".into(),
            },
        );
        assert!(matches!(r.response, ServerMessage::Ack));
    }

    #[test]
    fn ipc_send_keys_not_found() {
        let mut s = test_session();
        let mut p = MockPanes::new();
        let r = handle_ipc(
            &mut s,
            &mut p,
            ClientMessage::SendKeys {
                pane_id: 999,
                keys: "x".into(),
            },
        );
        assert!(matches!(r.response, ServerMessage::Error { .. }));
    }

    #[test]
    fn ipc_split_pane_spawns() {
        let mut s = test_session();
        let mut p = MockPanes::new();
        let r = handle_ipc(
            &mut s,
            &mut p,
            ClientMessage::SplitPane {
                direction: emux_ipc::SplitDirection::Vertical,
                size: None,
            },
        );
        assert!(matches!(r.response, ServerMessage::SpawnResult { .. }));
        assert!(r.sync_sizes);
    }

    #[test]
    fn ipc_list_panes() {
        let mut s = test_session();
        let mut p = MockPanes::new();
        let r = handle_ipc(&mut s, &mut p, ClientMessage::ListPanes);
        match r.response {
            ServerMessage::PaneList { panes } => assert!(!panes.is_empty()),
            other => panic!("expected PaneList, got {other:?}"),
        }
    }

    #[test]
    fn ipc_resize() {
        let mut s = test_session();
        let mut p = MockPanes::new();
        let r = handle_ipc(
            &mut s,
            &mut p,
            ClientMessage::Resize {
                cols: 120,
                rows: 40,
            },
        );
        assert!(matches!(r.response, ServerMessage::Ack));
        assert_eq!(s.size().cols, 120);
    }
}
