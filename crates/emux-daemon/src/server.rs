//! Daemon server — listens for client connections and manages sessions.

use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Instant;

use emux_ipc::{codec, ClientMessage, ServerMessage};
use emux_mux::Session;

use crate::{ClientId, DaemonError};
use crate::persistence;

/// A connected client and its stream.
struct ClientConnection {
    id: ClientId,
    stream: UnixStream,
}

/// Default auto-save interval in seconds.
const AUTO_SAVE_INTERVAL_SECS: u64 = 30;

/// The daemon server: owns a session, listens on a Unix socket, and serves
/// attached clients.
pub struct DaemonServer {
    socket_path: PathBuf,
    listener: UnixListener,
    session: Session,
    clients: Vec<ClientConnection>,
    next_client_id: u64,
    /// Path where session state is periodically persisted.
    snapshot_path: Option<PathBuf>,
    /// Last time the session was auto-saved.
    last_save: Instant,
    /// Whether the session has been modified since the last save.
    dirty: bool,
}

impl DaemonServer {
    /// Start the daemon, binding a Unix socket for the given session name.
    ///
    /// The socket is placed at `<temp_dir>/emux-test-<session_name>`.
    /// If a saved snapshot exists at the default location, the session is
    /// restored from it automatically.
    pub fn start(session_name: &str) -> Result<Self, DaemonError> {
        let socket_path = std::env::temp_dir().join(format!("emux-test-{session_name}"));

        // Clean up stale socket if no process owns it.
        if socket_path.exists() {
            // Try connecting to see if it is alive.
            match std::os::unix::net::UnixStream::connect(&socket_path) {
                Ok(_) => {
                    // Something is listening — refuse.
                    return Err(DaemonError::SocketExists(
                        socket_path.display().to_string(),
                    ));
                }
                Err(_) => {
                    // Stale socket; remove it.
                    let _ = std::fs::remove_file(&socket_path);
                }
            }
        }

        let listener = UnixListener::bind(&socket_path)?;
        listener.set_nonblocking(true)?;

        // Try to restore from a saved snapshot; fall back to a fresh session.
        let snapshot_path = persistence::default_snapshot_path(session_name);
        let session = if let Some(ref snap_path) = snapshot_path {
            persistence::load_session(snap_path).unwrap_or_else(|_| {
                Session::new(session_name, 80, 24)
            })
        } else {
            Session::new(session_name, 80, 24)
        };

        Ok(Self {
            socket_path,
            listener,
            session,
            clients: Vec::new(),
            next_client_id: 1,
            snapshot_path,
            last_save: Instant::now(),
            dirty: false,
        })
    }

    /// Start the daemon with a specific snapshot path (useful for testing).
    pub fn start_with_snapshot_path(
        session_name: &str,
        snapshot_path: Option<PathBuf>,
    ) -> Result<Self, DaemonError> {
        let mut server = Self::start(session_name)?;
        server.snapshot_path = snapshot_path;
        Ok(server)
    }

    /// Path to the Unix domain socket.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Borrow the session.
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// Mutably borrow the session.
    pub fn session_mut(&mut self) -> &mut Session {
        &mut self.session
    }

    /// Accept one pending client connection (non-blocking).
    pub fn accept_client(&mut self) -> Result<ClientId, DaemonError> {
        let (stream, _addr) = self.listener.accept()?;
        stream.set_nonblocking(false)?;
        let id = ClientId(self.next_client_id);
        self.next_client_id += 1;
        self.clients.push(ClientConnection { id, stream });
        Ok(id)
    }

    /// Number of connected clients.
    pub fn client_count(&self) -> usize {
        self.clients.len()
    }

    /// Disconnect (drop) a client by id.
    pub fn disconnect_client(&mut self, id: ClientId) {
        self.clients.retain(|c| c.id != id);
    }

    /// Process one [`ClientMessage`] and return the corresponding
    /// [`ServerMessage`].
    pub fn handle_message(&mut self, client_id: ClientId, msg: ClientMessage) -> ServerMessage {
        let _ = client_id; // available for future per-client logic
        match msg {
            ClientMessage::Ping => ServerMessage::Pong,
            ClientMessage::GetVersion => ServerMessage::Version {
                version: emux_ipc::PROTOCOL_VERSION,
            },
            ClientMessage::Resize { cols, rows } => {
                self.session.resize(cols as usize, rows as usize);
                self.dirty = true;
                ServerMessage::Ack
            }
            ClientMessage::FocusPane { pane_id } => {
                let ok = self.session.active_tab_mut().focus_pane(pane_id);
                if ok {
                    ServerMessage::Ack
                } else {
                    ServerMessage::Error {
                        message: format!("pane {pane_id} not found"),
                    }
                }
            }
            ClientMessage::KeyInput { .. } => {
                // In a real implementation this would write to the pane PTY.
                ServerMessage::Ack
            }
            ClientMessage::SpawnPane { direction } => {
                let dir = match direction.as_deref() {
                    Some("horizontal") => emux_mux::SplitDirection::Horizontal,
                    _ => emux_mux::SplitDirection::Vertical,
                };
                match self.session.active_tab_mut().split_pane(dir) {
                    Some(pane_id) => {
                        self.dirty = true;
                        ServerMessage::SpawnResult { pane_id }
                    }
                    None => ServerMessage::Error {
                        message: "cannot split pane".into(),
                    },
                }
            }
            ClientMessage::KillPane { pane_id } => {
                let ok = self.session.active_tab_mut().close_pane(pane_id);
                if ok {
                    self.dirty = true;
                    ServerMessage::Ack
                } else {
                    ServerMessage::Error {
                        message: format!("cannot kill pane {pane_id}"),
                    }
                }
            }
            ClientMessage::Detach => ServerMessage::Ack,
            ClientMessage::ListSessions => {
                let entry = emux_ipc::SessionEntry {
                    name: self.session.name().to_owned(),
                    tabs: self.session.tab_count(),
                    panes: self.session.active_tab().pane_count(),
                    cols: self.session.size().cols,
                    rows: self.session.size().rows,
                };
                ServerMessage::SessionList {
                    sessions: vec![entry],
                }
            }
            ClientMessage::KillSession { ref name } => {
                if name == self.session.name() {
                    ServerMessage::Ack
                } else {
                    ServerMessage::Error {
                        message: format!("no such session: {name}"),
                    }
                }
            }
        }
    }

    /// Send a [`ServerMessage`] to a specific client.
    pub fn send_to_client(
        &mut self,
        client_id: ClientId,
        msg: &ServerMessage,
    ) -> Result<(), DaemonError> {
        let conn = self
            .clients
            .iter_mut()
            .find(|c| c.id == client_id)
            .ok_or(DaemonError::InvalidClient(client_id))?;
        codec::write_message(&mut conn.stream, msg)?;
        Ok(())
    }

    /// Read a [`ClientMessage`] from a specific client (blocking).
    pub fn recv_from_client(
        &mut self,
        client_id: ClientId,
    ) -> Result<ClientMessage, DaemonError> {
        let conn = self
            .clients
            .iter_mut()
            .find(|c| c.id == client_id)
            .ok_or(DaemonError::InvalidClient(client_id))?;
        let msg: ClientMessage = codec::read_message(&mut conn.stream)?;
        Ok(msg)
    }

    /// Mark the session as dirty (modified since last save).
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Set or override the snapshot path.
    pub fn set_snapshot_path(&mut self, path: Option<PathBuf>) {
        self.snapshot_path = path;
    }

    /// Get the current snapshot path.
    pub fn snapshot_path(&self) -> Option<&Path> {
        self.snapshot_path.as_deref()
    }

    /// Save the session state to disk immediately.
    pub fn save_now(&mut self) -> Result<(), DaemonError> {
        if let Some(ref path) = self.snapshot_path {
            persistence::save_session(&self.session, path)?;
            self.last_save = Instant::now();
            self.dirty = false;
        }
        Ok(())
    }

    /// Check whether enough time has elapsed and the session is dirty, and
    /// if so, save automatically. Call this from the daemon event loop.
    ///
    /// Returns `true` if a save was performed.
    pub fn maybe_auto_save(&mut self) -> bool {
        if !self.dirty {
            return false;
        }
        let elapsed = self.last_save.elapsed();
        if elapsed.as_secs() < AUTO_SAVE_INTERVAL_SECS {
            return false;
        }
        // Best-effort save; don't crash the daemon on failure.
        let _ = self.save_now();
        true
    }

    /// Rename the session and move the socket file accordingly.
    pub fn rename_session(&mut self, new_name: &str) -> Result<(), DaemonError> {
        let new_socket_path =
            std::env::temp_dir().join(format!("emux-test-{new_name}"));
        std::fs::rename(&self.socket_path, &new_socket_path)?;
        self.session.rename(new_name);
        self.socket_path = new_socket_path;
        self.snapshot_path = persistence::default_snapshot_path(new_name);
        self.dirty = true;
        Ok(())
    }

    /// Broadcast a [`ServerMessage`] to all connected clients.
    ///
    /// Clients that fail to receive the message are collected into the returned
    /// vector so the caller can disconnect them.
    pub fn broadcast_to_all_clients(&mut self, msg: &ServerMessage) -> Vec<ClientId> {
        let mut failed = Vec::new();
        for conn in &mut self.clients {
            if codec::write_message(&mut conn.stream, msg).is_err() {
                failed.push(conn.id);
            }
        }
        failed
    }

    /// Return the IDs of all currently connected clients.
    pub fn client_ids(&self) -> Vec<ClientId> {
        self.clients.iter().map(|c| c.id).collect()
    }

    /// Shut down: save session state, drop all clients, and remove the socket file.
    pub fn shutdown(mut self) {
        // Final save before shutdown.
        let _ = self.save_now();
        drop(self.listener);
        let _ = std::fs::remove_file(&self.socket_path);
    }
}
