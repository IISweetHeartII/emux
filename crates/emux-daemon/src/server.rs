//! Daemon server — listens for client connections and manages sessions.

use std::collections::HashMap;
#[allow(unused_imports)]
use std::io::{self, Read as _, Write as _};
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Instant;

use emux_ipc::{ClientMessage, ServerMessage, SplitDirection, codec};
use emux_mux::{PaneId, Session};
use emux_term::Screen;
use emux_vt::Parser;

use crate::persistence;
use crate::{ClientId, DaemonError};

/// A connected client and its stream.
struct ClientConnection {
    id: ClientId,
    #[cfg(unix)]
    stream: UnixStream,
    #[cfg(windows)]
    stream: std::net::TcpStream,
}

/// Default auto-save interval in seconds.
const AUTO_SAVE_INTERVAL_SECS: u64 = 30;

/// Terminal state for a single pane managed by the daemon.
pub struct PaneTerminal {
    /// PTY master end (for reading output and writing input).
    #[cfg(unix)]
    pub pty: emux_pty::UnixPty,
    #[cfg(windows)]
    pub pty: emux_pty::WinPty,
    /// VT parser state machine.
    pub parser: Parser,
    /// Terminal screen (grid + cursor + scrollback).
    pub screen: Screen,
}

/// The daemon server: owns a session, listens on a socket, and serves
/// attached clients.
pub struct DaemonServer {
    socket_path: PathBuf,
    #[cfg(unix)]
    listener: UnixListener,
    #[cfg(windows)]
    listener: std::net::TcpListener,
    session: Session,
    clients: Vec<ClientConnection>,
    next_client_id: u64,
    /// Path where session state is periodically persisted.
    snapshot_path: Option<PathBuf>,
    /// Last time the session was auto-saved.
    last_save: Instant,
    /// Whether the session has been modified since the last save.
    dirty: bool,
    /// Per-pane terminal state (PTY + Screen). Managed by the daemon so
    /// AI agents can `SendKeys` and `CapturePane` on real terminals.
    pub pane_terminals: HashMap<PaneId, PaneTerminal>,
}

impl DaemonServer {
    /// Start the daemon, binding a socket for the given session name.
    ///
    /// On Unix the socket is a Unix domain socket placed at
    /// `<temp_dir>/emux-test-<session_name>`.
    /// On Windows a TCP loopback listener is used instead and the assigned
    /// port is written to a `.port` file at the same location.
    ///
    /// If a saved snapshot exists at the default location, the session is
    /// restored from it automatically.
    pub fn start(session_name: &str) -> Result<Self, DaemonError> {
        let socket_path = std::env::temp_dir().join(format!("emux-test-{session_name}"));

        #[cfg(unix)]
        {
            // Clean up stale socket if no process owns it.
            if socket_path.exists() {
                // Try connecting to see if it is alive.
                match std::os::unix::net::UnixStream::connect(&socket_path) {
                    Ok(_) => {
                        // Something is listening — refuse.
                        return Err(DaemonError::SocketExists(socket_path.display().to_string()));
                    }
                    Err(_) => {
                        // Stale socket; remove it.
                        let _ = std::fs::remove_file(&socket_path);
                    }
                }
            }
        }

        #[cfg(windows)]
        {
            // On Windows we use a port file instead of a socket file.
            if socket_path.exists() {
                // Try reading the port and connecting to see if a daemon is alive.
                if let Ok(contents) = std::fs::read_to_string(&socket_path) {
                    if let Ok(port) = contents.trim().parse::<u16>() {
                        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
                            return Err(DaemonError::SocketExists(
                                socket_path.display().to_string(),
                            ));
                        }
                    }
                }
                // Stale port file; remove it.
                let _ = std::fs::remove_file(&socket_path);
            }
        }

        #[cfg(unix)]
        let listener = {
            let l = UnixListener::bind(&socket_path)?;
            l.set_nonblocking(true)?;
            l
        };

        #[cfg(windows)]
        let listener = {
            let l = std::net::TcpListener::bind("127.0.0.1:0")?;
            l.set_nonblocking(true)?;
            // Write the port to the socket_path file so clients can find us.
            let port = l.local_addr()?.port();
            std::fs::write(&socket_path, port.to_string())?;
            l
        };

        // Try to restore from a saved snapshot; fall back to a fresh session.
        let snapshot_path = persistence::default_snapshot_path(session_name);
        let session = if let Some(ref snap_path) = snapshot_path {
            persistence::load_session(snap_path)
                .unwrap_or_else(|_| Session::new(session_name, 80, 24))
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
            pane_terminals: HashMap::new(),
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

    /// Path to the socket (Unix domain socket path, or port file on Windows).
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

            // -- Agent / AI team protocol --
            ClientMessage::SplitPane { direction, size } => {
                let dir = match direction {
                    SplitDirection::Horizontal => emux_mux::SplitDirection::Horizontal,
                    SplitDirection::Vertical => emux_mux::SplitDirection::Vertical,
                };
                let _ = size;
                match self.session.active_tab_mut().split_pane(dir) {
                    Some(pane_id) => {
                        // Spawn a real PTY + Screen for this pane.
                        let positions = self.session.active_tab().compute_positions();
                        let (cols, rows) = positions
                            .iter()
                            .find(|(id, _)| *id == pane_id)
                            .map(|(_, p)| (p.cols, p.rows))
                            .unwrap_or((80, 24));
                        if let Ok(pt) = Self::spawn_pane_terminal(cols, rows) {
                            self.pane_terminals.insert(pane_id, pt);
                        }
                        self.dirty = true;
                        ServerMessage::SpawnResult { pane_id }
                    }
                    None => ServerMessage::Error {
                        message: "cannot split pane".into(),
                    },
                }
            }
            ClientMessage::CapturePane { pane_id } => {
                let tab = self.session.active_tab();
                if tab.pane(pane_id).is_none() {
                    return ServerMessage::Error {
                        message: format!("pane {pane_id} not found"),
                    };
                }
                // If we have a real terminal, drain PTY output first, then read Screen.
                if let Some(pt) = self.pane_terminals.get_mut(&pane_id) {
                    // Drain any pending PTY output into the screen.
                    Self::drain_pty_output(pt);
                    // Extract visible text from the screen.
                    let content = (0..pt.screen.rows())
                        .map(|r| pt.screen.row_text(r))
                        .collect::<Vec<_>>()
                        .join("\n");
                    ServerMessage::PaneCaptured { pane_id, content }
                } else {
                    // No PTY attached — return empty content sized to the pane.
                    let positions = tab.compute_positions();
                    let (cols, rows) = positions
                        .iter()
                        .find(|(id, _)| *id == pane_id)
                        .map(|(_, p)| (p.cols, p.rows))
                        .unwrap_or((0, 0));
                    let content = (0..rows)
                        .map(|_| " ".repeat(cols))
                        .collect::<Vec<_>>()
                        .join("\n");
                    ServerMessage::PaneCaptured { pane_id, content }
                }
            }
            ClientMessage::SendKeys { pane_id, keys } => {
                let tab = self.session.active_tab();
                if tab.pane(pane_id).is_none() {
                    return ServerMessage::Error {
                        message: format!("pane {pane_id} not found"),
                    };
                }
                // Write keys to the real PTY if available.
                if let Some(pt) = self.pane_terminals.get_mut(&pane_id) {
                    if let Err(e) = pt.pty.write(keys.as_bytes()) {
                        return ServerMessage::Error {
                            message: format!("PTY write error: {e}"),
                        };
                    }
                }
                ServerMessage::Ack
            }
            ClientMessage::ListPanes => {
                let tab = self.session.active_tab();
                let positions = tab.compute_positions();
                let active = tab.active_pane_id();
                let panes: Vec<emux_ipc::PaneEntry> = positions
                    .iter()
                    .map(|(id, pos)| {
                        let pane = tab.pane(*id);
                        emux_ipc::PaneEntry {
                            id: *id,
                            title: pane.map(|p| p.title().to_owned()).unwrap_or_default(),
                            cols: pos.cols as u16,
                            rows: pos.rows as u16,
                            active: active == Some(*id),
                            has_notification: pane.map(|p| p.has_notification()).unwrap_or(false),
                        }
                    })
                    .collect();
                ServerMessage::PaneList { panes }
            }
            ClientMessage::GetPaneInfo { pane_id } => {
                let tab = self.session.active_tab();
                if let Some(pane) = tab.pane(pane_id) {
                    let positions = tab.compute_positions();
                    let pos = positions.iter().find(|(id, _)| *id == pane_id);
                    let (cols, rows) = pos
                        .map(|(_, p)| (p.cols as u16, p.rows as u16))
                        .unwrap_or((0, 0));
                    let active = tab.active_pane_id() == Some(pane_id);
                    ServerMessage::PaneInfo {
                        pane: emux_ipc::PaneEntry {
                            id: pane_id,
                            title: pane.title().to_owned(),
                            cols,
                            rows,
                            active,
                            has_notification: pane.has_notification(),
                        },
                    }
                } else {
                    ServerMessage::Error {
                        message: format!("pane {pane_id} not found"),
                    }
                }
            }
            ClientMessage::ResizePane {
                pane_id,
                cols,
                rows,
            } => {
                let tab = self.session.active_tab_mut();
                if tab.pane(pane_id).is_none() {
                    return ServerMessage::Error {
                        message: format!("pane {pane_id} not found"),
                    };
                }
                // Compute current pane size and resize by delta.
                let positions = tab.compute_positions();
                if let Some((_, pos)) = positions.iter().find(|(id, _)| *id == pane_id) {
                    let dcols = cols as i32 - pos.cols as i32;
                    let drows = rows as i32 - pos.rows as i32;
                    if dcols != 0 {
                        tab.resize_pane(pane_id, emux_mux::ResizeDirection::Right, dcols);
                    }
                    if drows != 0 {
                        tab.resize_pane(pane_id, emux_mux::ResizeDirection::Down, drows);
                    }
                    self.dirty = true;
                    ServerMessage::Ack
                } else {
                    ServerMessage::Error {
                        message: format!("cannot resize pane {pane_id}"),
                    }
                }
            }
            ClientMessage::SetPaneTitle { pane_id, title } => {
                if let Some(pane) = self.session.active_tab_mut().pane_mut(pane_id) {
                    pane.set_title(title);
                    self.dirty = true;
                    ServerMessage::Ack
                } else {
                    ServerMessage::Error {
                        message: format!("pane {pane_id} not found"),
                    }
                }
            }
            ClientMessage::Attach { cols, rows } => {
                self.session.resize(cols as usize, rows as usize);
                ServerMessage::Ack
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
    pub fn recv_from_client(&mut self, client_id: ClientId) -> Result<ClientMessage, DaemonError> {
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

    /// Get the current persistence snapshot path.
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
        let new_socket_path = std::env::temp_dir().join(format!("emux-test-{new_name}"));

        #[cfg(unix)]
        {
            std::fs::rename(&self.socket_path, &new_socket_path)?;
        }

        #[cfg(windows)]
        {
            // On Windows the "socket" is a port file. Re-write it at the new
            // path and remove the old one.
            if let Ok(contents) = std::fs::read_to_string(&self.socket_path) {
                std::fs::write(&new_socket_path, &contents)?;
            }
            let _ = std::fs::remove_file(&self.socket_path);
        }

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

    /// Spawn a PTY + Screen for a pane of the given dimensions.
    fn spawn_pane_terminal(cols: usize, rows: usize) -> Result<PaneTerminal, DaemonError> {
        let size = emux_pty::PtySize {
            rows: rows as u16,
            cols: cols as u16,
            pixel_width: 0,
            pixel_height: 0,
        };
        let cmd = emux_pty::CommandBuilder::default_shell();

        #[cfg(unix)]
        let pty =
            emux_pty::UnixPty::spawn(&cmd, size).map_err(|e| io::Error::other(e.to_string()))?;
        #[cfg(windows)]
        let pty =
            emux_pty::WinPty::spawn(&cmd, size).map_err(|e| io::Error::other(e.to_string()))?;

        #[cfg(unix)]
        {
            // Set the PTY master to non-blocking so drain_pty_output doesn't hang.
            unsafe {
                let fd = pty.master_raw_fd();
                let flags = libc::fcntl(fd, libc::F_GETFL);
                libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
            }
        }

        Ok(PaneTerminal {
            pty,
            parser: Parser::new(),
            screen: Screen::new(cols, rows),
        })
    }

    /// Drain all available PTY output into the terminal's Screen.
    fn drain_pty_output(pt: &mut PaneTerminal) {
        let mut buf = [0u8; 65536];
        loop {
            match pt.pty.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    pt.parser.advance(&mut pt.screen, &buf[..n]);
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => break,
                #[cfg(unix)]
                Err(ref e) if e.raw_os_error() == Some(libc::EIO) => break,
                Err(_) => break,
            }
        }
    }

    /// Drain PTY output for all pane terminals. Call this periodically
    /// from the daemon event loop to keep Screen state up to date.
    pub fn poll_pty_output(&mut self) {
        for pt in self.pane_terminals.values_mut() {
            Self::drain_pty_output(pt);
        }
    }

    /// Spawn a PTY for an existing pane (e.g. the initial pane).
    pub fn spawn_terminal_for_pane(&mut self, pane_id: PaneId) -> Result<(), DaemonError> {
        let positions = self.session.active_tab().compute_positions();
        let (cols, rows) = positions
            .iter()
            .find(|(id, _)| *id == pane_id)
            .map(|(_, p)| (p.cols, p.rows))
            .unwrap_or((80, 24));
        let pt = Self::spawn_pane_terminal(cols, rows)?;
        self.pane_terminals.insert(pane_id, pt);
        Ok(())
    }

    /// Shut down: save session state, drop all clients, and remove the socket file.
    pub fn shutdown(mut self) {
        // Final save before shutdown.
        let _ = self.save_now();
        drop(self.listener);
        let _ = std::fs::remove_file(&self.socket_path);
    }
}
