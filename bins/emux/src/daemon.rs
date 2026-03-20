#[allow(unused_imports)]
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[allow(unused_imports)]
use emux_ipc::{ClientMessage, ServerMessage};
#[allow(unused_imports)]
use emux_mux::{Session, SplitDirection};

use crate::AppError;

/// Directory where daemon sockets are stored.
pub(crate) fn socket_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("emux-sockets");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Socket path for a given session name.
///
/// On Unix this is a `.sock` file (Unix domain socket).
/// On Windows this is a `.port` file containing the TCP port number.
pub(crate) fn socket_path_for(name: &str) -> PathBuf {
    #[cfg(unix)]
    {
        socket_dir().join(format!("emux-{name}.sock"))
    }
    #[cfg(windows)]
    {
        socket_dir().join(format!("emux-{name}.port"))
    }
}

/// List all live daemon sockets and return (name, path) pairs.
pub(crate) fn list_live_sessions() -> Vec<(String, PathBuf)> {
    let dir = socket_dir();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    let mut result = Vec::new();

    #[cfg(unix)]
    let (prefix, suffix) = ("emux-", ".sock");
    #[cfg(windows)]
    let (prefix, suffix) = ("emux-", ".port");

    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(fname) = path.file_name().and_then(|f| f.to_str())
            && let Some(name) = fname
                .strip_prefix(prefix)
                .and_then(|s| s.strip_suffix(suffix))
        {
            // Check if the daemon is alive by trying to connect.
            #[cfg(unix)]
            let alive = std::os::unix::net::UnixStream::connect(&path).is_ok();

            #[cfg(windows)]
            let alive = {
                std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|c| c.trim().parse::<u16>().ok())
                    .map(|port| std::net::TcpStream::connect(("127.0.0.1", port)).is_ok())
                    .unwrap_or(false)
            };

            if alive {
                result.push((name.to_owned(), path));
            } else {
                // Stale socket/port file — clean up.
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    result
}

/// Start a daemon server for the given session name.
///
/// The daemon runs as a **forked child process** (Unix) or background thread
/// (Windows) so it survives after the client disconnects (true detach).
/// Returns the socket path once the daemon is ready to accept connections.
pub(crate) fn start_daemon_server(session_name: &str) -> Result<PathBuf, AppError> {
    let sock_path = socket_path_for(session_name);

    // Clean up stale socket/port file if any.
    if sock_path.exists() {
        #[cfg(unix)]
        let alive = std::os::unix::net::UnixStream::connect(&sock_path).is_ok();

        #[cfg(windows)]
        let alive = std::fs::read_to_string(&sock_path)
            .ok()
            .and_then(|c| c.trim().parse::<u16>().ok())
            .map(|port| std::net::TcpStream::connect(("127.0.0.1", port)).is_ok())
            .unwrap_or(false);

        if alive {
            return Err(format!("session '{}' is already running", session_name).into());
        }
        let _ = std::fs::remove_file(&sock_path);
    }

    // Bind the listener BEFORE forking/spawning so the parent can be sure
    // it's ready.
    #[cfg(unix)]
    let listener = {
        let l = std::os::unix::net::UnixListener::bind(&sock_path)?;
        l.set_nonblocking(true)?;
        l
    };

    #[cfg(windows)]
    let listener = {
        let l = std::net::TcpListener::bind("127.0.0.1:0")?;
        l.set_nonblocking(true)?;
        let port = l.local_addr()?.port();
        std::fs::write(&sock_path, port.to_string())?;
        l
    };

    #[cfg(unix)]
    {
        // Fork a child process for the daemon.
        let pid = unsafe { libc::fork() };
        match pid {
            -1 => {
                return Err(AppError::Io(io::Error::last_os_error()));
            }
            0 => {
                // ── Child process (daemon) ───────────────────────
                // Detach from the controlling terminal so the daemon
                // keeps running after the parent exits.
                unsafe { libc::setsid() };

                // Close stdin/stdout/stderr so we don't hold the
                // parent's terminal.
                unsafe {
                    libc::close(0);
                    libc::close(1);
                    libc::close(2);
                }

                let name = session_name.to_owned();
                let path = sock_path.clone();
                run_daemon_loop(&name, listener, &path);
                std::process::exit(0);
            }
            _parent_pid => {
                // ── Parent process ───────────────────────────────
                // Wait a moment for the child to be ready.
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }

    #[cfg(windows)]
    {
        // On Windows, fall back to a background thread (no fork).
        let name = session_name.to_owned();
        let path = sock_path.clone();
        std::thread::spawn(move || {
            run_daemon_loop(&name, listener, &path);
        });
        std::thread::sleep(Duration::from_millis(50));
    }

    Ok(sock_path)
}

/// The daemon event loop — owns PTYs and streams output to attached clients.
///
/// This is the tmux-style architecture: the daemon process owns all PTYs,
/// parsers, and screen state. Clients attach and receive PTY output; when
/// they detach the PTYs keep running.
#[cfg(unix)]
pub(crate) fn run_daemon_loop(
    session_name: &str,
    listener: std::os::unix::net::UnixListener,
    socket_path: &Path,
) {
    run_daemon_loop_inner(session_name, listener, socket_path);
}

/// Windows variant.
#[cfg(windows)]
pub(crate) fn run_daemon_loop(
    session_name: &str,
    listener: std::net::TcpListener,
    socket_path: &Path,
) {
    run_daemon_loop_inner(session_name, listener, socket_path);
}

/// Shared daemon loop: owns PTYs, polls them, and streams output to
/// attached rendering clients.
fn run_daemon_loop_inner<L: DaemonListener>(session_name: &str, listener: L, socket_path: &Path) {
    use emux_daemon::server::PaneTerminal;
    use emux_mux::PaneId;
    use std::collections::HashMap;

    // Create session and PTY state directly (no DaemonServer — it would
    // try to bind its own socket, but we already have `listener`).
    let mut session = Session::new(session_name, 80, 24);
    let mut pane_terminals: HashMap<PaneId, PaneTerminal> = HashMap::new();

    // Spawn a real PTY for the initial pane.
    let initial_panes = session.active_tab().pane_ids();
    for pane_id in initial_panes {
        let positions = session.active_tab().compute_positions();
        let (cols, rows) = positions
            .iter()
            .find(|(id, _)| *id == pane_id)
            .map(|(_, p)| (p.cols, p.rows))
            .unwrap_or((80, 24));
        if let Ok(pt) = spawn_pane_terminal(cols, rows) {
            pane_terminals.insert(pane_id, pt);
        }
    }

    // Attached rendering clients (long-lived connections that receive PTY output).
    let mut attached_clients: Vec<L::Stream> = Vec::new();
    // One-shot IPC clients (send command, receive response, disconnect).
    let mut ipc_clients: Vec<(u64, L::Stream)> = Vec::new();
    let mut next_id: u64 = 1;
    let mut shutdown = false;

    while !shutdown {
        // ---- Accept new connections ----
        if let Some(stream) = listener.try_accept() {
            let _ = stream.set_nonblocking_compat(false);
            let _ = stream.set_read_timeout_compat(Some(Duration::from_millis(50)));
            ipc_clients.push((next_id, stream));
            next_id += 1;
        }

        // ---- Poll PTY output ----
        let mut pty_output: Vec<(u32, Vec<u8>)> = Vec::new();
        let pane_ids: Vec<u32> = pane_terminals.keys().copied().collect();
        for pane_id in &pane_ids {
            if let Some(pt) = pane_terminals.get_mut(pane_id) {
                let mut buf = [0u8; 65536];
                loop {
                    match pt.pty.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            let data = buf[..n].to_vec();
                            pt.parser.advance(&mut pt.screen, &data);
                            pty_output.push((*pane_id, data));
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => break,
                        #[cfg(unix)]
                        Err(ref e) if e.raw_os_error() == Some(libc::EIO) => break,
                        Err(_) => break,
                    }
                }
            }
        }

        // ---- Stream PTY output to attached clients ----
        let mut dead_attached = Vec::new();
        for (i, stream) in attached_clients.iter_mut().enumerate() {
            for (pane_id, data) in &pty_output {
                let msg = ServerMessage::PtyOutput {
                    pane_id: *pane_id,
                    data: data.clone(),
                };
                if emux_ipc::codec::write_message(stream, &msg).is_err() {
                    dead_attached.push(i);
                    break;
                }
            }
        }
        for i in dead_attached.into_iter().rev() {
            attached_clients.remove(i);
        }

        // ---- Process IPC messages from one-shot clients ----
        let mut to_remove = Vec::new();
        for (id, stream) in ipc_clients.iter_mut() {
            match emux_ipc::codec::read_message::<_, ClientMessage>(stream) {
                Ok(msg) => {
                    match msg {
                        ClientMessage::Attach { cols, rows } => {
                            // Upgrade this connection to an attached rendering client.
                            session.resize(cols as usize, rows as usize);
                            // Send Ack, then the client will be moved to attached_clients.
                            let _ = emux_ipc::codec::write_message(stream, &ServerMessage::Ack);
                            // Mark for removal from ipc_clients (will be added to attached).
                            to_remove.push((*id, true)); // true = move to attached
                        }
                        ClientMessage::KillSession { ref name } => {
                            if name == session.name() {
                                shutdown = true;
                            }
                            let _ = emux_ipc::codec::write_message(stream, &ServerMessage::Ack);
                            to_remove.push((*id, false));
                        }
                        ClientMessage::Detach => {
                            let _ = emux_ipc::codec::write_message(stream, &ServerMessage::Ack);
                            to_remove.push((*id, false));
                        }
                        ClientMessage::KeyInput { data } => {
                            // Write to the active pane's PTY.
                            if let Some(active) = session.active_tab().active_pane_id() {
                                if let Some(pt) = pane_terminals.get_mut(&active) {
                                    let _ = pt.pty.write(&data);
                                }
                            }
                            let _ = emux_ipc::codec::write_message(stream, &ServerMessage::Ack);
                            to_remove.push((*id, false));
                        }
                        other => {
                            let reply =
                                handle_ipc_message(&mut session, &mut pane_terminals, other);
                            let _ = emux_ipc::codec::write_message(stream, &reply);
                            to_remove.push((*id, false));
                        }
                    }
                }
                Err(emux_ipc::CodecError::Io(ref e))
                    if e.kind() == io::ErrorKind::WouldBlock
                        || e.kind() == io::ErrorKind::TimedOut => {}
                Err(_) => {
                    to_remove.push((*id, false));
                }
            }
        }

        // Move attached clients, remove processed IPC clients.
        for (id, move_to_attached) in to_remove.into_iter().rev() {
            if let Some(pos) = ipc_clients.iter().position(|(cid, _)| *cid == id) {
                let (_, stream) = ipc_clients.remove(pos);
                if move_to_attached {
                    let _ = stream.set_nonblocking_compat(true);
                    attached_clients.push(stream);
                }
            }
        }

        // ---- Auto-save (best-effort) ----
        // Auto-save logic could be added here if persistence is needed.

        // Sleep briefly to avoid busy-looping.
        if pty_output.is_empty() {
            std::thread::sleep(Duration::from_millis(16));
        } else {
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    // Cleanup.
    let _ = std::fs::remove_file(socket_path);
    // Also remove agent socket if present.
    let agent_sock = socket_dir().join(format!("emux-agent-{session_name}.sock"));
    let _ = std::fs::remove_file(&agent_sock);
}

/// Spawn a PTY + Screen for a pane.
fn spawn_pane_terminal(
    cols: usize,
    rows: usize,
) -> Result<emux_daemon::server::PaneTerminal, io::Error> {
    let size = emux_pty::PtySize {
        rows: rows as u16,
        cols: cols as u16,
        pixel_width: 0,
        pixel_height: 0,
    };
    let cmd = emux_pty::CommandBuilder::default_shell();

    #[cfg(unix)]
    let pty = emux_pty::UnixPty::spawn(&cmd, size).map_err(|e| io::Error::other(e.to_string()))?;
    #[cfg(windows)]
    let pty = emux_pty::WinPty::spawn(&cmd, size).map_err(|e| io::Error::other(e.to_string()))?;

    #[cfg(unix)]
    unsafe {
        let fd = pty.master_raw_fd();
        let flags = libc::fcntl(fd, libc::F_GETFL);
        libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }

    Ok(emux_daemon::server::PaneTerminal {
        pty,
        parser: emux_vt::Parser::new(),
        screen: emux_term::Screen::new(cols, rows),
    })
}

/// Handle an IPC message using session + pane_terminals directly.
fn handle_ipc_message(
    session: &mut Session,
    pane_terminals: &mut std::collections::HashMap<u32, emux_daemon::server::PaneTerminal>,
    msg: ClientMessage,
) -> ServerMessage {
    match msg {
        ClientMessage::Ping => ServerMessage::Pong,
        ClientMessage::GetVersion => ServerMessage::Version {
            version: emux_ipc::PROTOCOL_VERSION,
        },
        ClientMessage::Resize { cols, rows } => {
            session.resize(cols as usize, rows as usize);
            ServerMessage::Ack
        }
        ClientMessage::ListSessions => {
            let entry = emux_ipc::SessionEntry {
                name: session.name().to_owned(),
                tabs: session.tab_count(),
                panes: session.active_tab().pane_count(),
                cols: session.size().cols,
                rows: session.size().rows,
            };
            ServerMessage::SessionList {
                sessions: vec![entry],
            }
        }
        ClientMessage::ListPanes => {
            let tab = session.active_tab();
            let positions = tab.compute_positions();
            let active = tab.active_pane_id();
            let panes = positions
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
            ServerMessage::PaneList { panes }
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
                ServerMessage::PaneInfo {
                    pane: emux_ipc::PaneEntry {
                        id: pane_id,
                        title: pane.title().to_owned(),
                        cols,
                        rows,
                        active: tab.active_pane_id() == Some(pane_id),
                        has_notification: pane.has_notification(),
                    },
                }
            } else {
                ServerMessage::Error {
                    message: format!("pane {pane_id} not found"),
                }
            }
        }
        ClientMessage::CapturePane { pane_id } => {
            if let Some(pt) = pane_terminals.get_mut(&pane_id) {
                let mut buf = [0u8; 65536];
                loop {
                    match pt.pty.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => pt.parser.advance(&mut pt.screen, &buf[..n]),
                        Err(_) => break,
                    }
                }
                let content = (0..pt.screen.rows())
                    .map(|r| pt.screen.row_text(r))
                    .collect::<Vec<_>>()
                    .join("\n");
                ServerMessage::PaneCaptured { pane_id, content }
            } else {
                ServerMessage::Error {
                    message: format!("pane {pane_id} not found"),
                }
            }
        }
        ClientMessage::SendKeys { pane_id, keys } => {
            if let Some(pt) = pane_terminals.get_mut(&pane_id) {
                let _ = pt.pty.write(keys.as_bytes());
                ServerMessage::Ack
            } else {
                ServerMessage::Error {
                    message: format!("pane {pane_id} not found"),
                }
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
                    if let Ok(pt) = spawn_pane_terminal(cols, rows) {
                        pane_terminals.insert(new_id, pt);
                    }
                    ServerMessage::SpawnResult { pane_id: new_id }
                }
                None => ServerMessage::Error {
                    message: "cannot split pane".into(),
                },
            }
        }
        ClientMessage::SetPaneTitle { pane_id, title } => {
            if let Some(pane) = session.active_tab_mut().pane_mut(pane_id) {
                pane.set_title(title);
                ServerMessage::Ack
            } else {
                ServerMessage::Error {
                    message: format!("pane {pane_id} not found"),
                }
            }
        }
        ClientMessage::FocusPane { pane_id } => {
            if session.active_tab_mut().focus_pane(pane_id) {
                ServerMessage::Ack
            } else {
                ServerMessage::Error {
                    message: format!("pane {pane_id} not found"),
                }
            }
        }
        ClientMessage::SpawnPane { ref direction } => {
            let dir = match direction.as_deref() {
                Some("horizontal") => SplitDirection::Horizontal,
                _ => SplitDirection::Vertical,
            };
            match session.active_tab_mut().split_pane(dir) {
                Some(pane_id) => ServerMessage::SpawnResult { pane_id },
                None => ServerMessage::Error {
                    message: "cannot split pane".into(),
                },
            }
        }
        ClientMessage::KillPane { pane_id } => {
            if session.active_tab_mut().close_pane(pane_id) {
                pane_terminals.remove(&pane_id);
                ServerMessage::Ack
            } else {
                ServerMessage::Error {
                    message: format!("cannot kill pane {pane_id}"),
                }
            }
        }
        ClientMessage::Attach { .. }
        | ClientMessage::Detach
        | ClientMessage::KillSession { .. }
        | ClientMessage::KeyInput { .. } => ServerMessage::Ack,
        ClientMessage::ResizePane {
            pane_id,
            cols,
            rows,
        } => {
            let tab = session.active_tab_mut();
            if tab.pane(pane_id).is_none() {
                return ServerMessage::Error {
                    message: format!("pane {pane_id} not found"),
                };
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
                ServerMessage::Ack
            } else {
                ServerMessage::Error {
                    message: format!("pane {pane_id} not found"),
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Listener abstraction for Unix/Windows
// ---------------------------------------------------------------------------

trait StreamCompat: std::io::Read + std::io::Write + Sized {
    fn set_nonblocking_compat(&self, nonblocking: bool) -> io::Result<()>;
    fn set_read_timeout_compat(&self, timeout: Option<Duration>) -> io::Result<()>;
}

trait DaemonListener {
    type Stream: StreamCompat;
    fn try_accept(&self) -> Option<Self::Stream>;
}

#[cfg(unix)]
impl StreamCompat for std::os::unix::net::UnixStream {
    fn set_nonblocking_compat(&self, nonblocking: bool) -> io::Result<()> {
        self.set_nonblocking(nonblocking)
    }
    fn set_read_timeout_compat(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.set_read_timeout(timeout)
    }
}

#[cfg(unix)]
impl DaemonListener for std::os::unix::net::UnixListener {
    type Stream = std::os::unix::net::UnixStream;
    fn try_accept(&self) -> Option<Self::Stream> {
        match self.accept() {
            Ok((stream, _)) => Some(stream),
            Err(_) => None,
        }
    }
}

#[cfg(windows)]
impl StreamCompat for std::net::TcpStream {
    fn set_nonblocking_compat(&self, nonblocking: bool) -> io::Result<()> {
        self.set_nonblocking(nonblocking)
    }
    fn set_read_timeout_compat(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.set_read_timeout(timeout)
    }
}

#[cfg(windows)]
impl DaemonListener for std::net::TcpListener {
    type Stream = std::net::TcpStream;
    fn try_accept(&self) -> Option<Self::Stream> {
        match self.accept() {
            Ok((stream, _)) => Some(stream),
            Err(_) => None,
        }
    }
}

// Legacy loop body — replaced by run_daemon_loop_inner which owns PTYs.
// Kept as dead code temporarily for reference; will be removed.
#[allow(dead_code)]
fn _run_daemon_loop_body_legacy<S: std::io::Read + std::io::Write>(
    session: &mut Session,
    clients: &mut Vec<(u64, S)>,
    _next_id: &mut u64,
    shutdown: &mut bool,
) {
    use emux_ipc::codec;

    // Process messages from each client.
    let mut to_remove = Vec::new();
    let mut per_client_reply: Vec<(u64, ServerMessage)> = Vec::new();
    let mut broadcast_msgs: Vec<ServerMessage> = Vec::new();

    for (id, stream) in clients.iter_mut() {
        match codec::read_message::<_, ClientMessage>(stream) {
            Ok(msg) => {
                let (reply, should_broadcast) = match msg {
                    ClientMessage::Ping => (ServerMessage::Pong, false),
                    ClientMessage::GetVersion => (
                        ServerMessage::Version {
                            version: emux_ipc::PROTOCOL_VERSION,
                        },
                        false,
                    ),
                    ClientMessage::Resize { cols, rows } => {
                        session.resize(cols as usize, rows as usize);
                        (ServerMessage::Ack, false)
                    }
                    ClientMessage::Detach => {
                        to_remove.push(*id);
                        (ServerMessage::Ack, false)
                    }
                    ClientMessage::ListSessions => {
                        let entry = emux_ipc::SessionEntry {
                            name: session.name().to_owned(),
                            tabs: session.tab_count(),
                            panes: session.active_tab().pane_count(),
                            cols: session.size().cols,
                            rows: session.size().rows,
                        };
                        (
                            ServerMessage::SessionList {
                                sessions: vec![entry],
                            },
                            false,
                        )
                    }
                    ClientMessage::KillSession { ref name } => {
                        if name == session.name() {
                            *shutdown = true;
                            (ServerMessage::Ack, false)
                        } else {
                            (
                                ServerMessage::Error {
                                    message: format!("no such session: {name}"),
                                },
                                false,
                            )
                        }
                    }
                    ClientMessage::SpawnPane { ref direction } => {
                        let dir = match direction.as_deref() {
                            Some("horizontal") => SplitDirection::Horizontal,
                            _ => SplitDirection::Vertical,
                        };
                        match session.active_tab_mut().split_pane(dir) {
                            Some(pane_id) => (ServerMessage::SpawnResult { pane_id }, true),
                            None => (
                                ServerMessage::Error {
                                    message: "cannot split pane".into(),
                                },
                                false,
                            ),
                        }
                    }
                    ClientMessage::KillPane { pane_id } => {
                        if session.active_tab_mut().close_pane(pane_id) {
                            (ServerMessage::Ack, true)
                        } else {
                            (
                                ServerMessage::Error {
                                    message: format!("cannot kill pane {pane_id}"),
                                },
                                false,
                            )
                        }
                    }
                    ClientMessage::FocusPane { pane_id } => {
                        if session.active_tab_mut().focus_pane(pane_id) {
                            (ServerMessage::Ack, true)
                        } else {
                            (
                                ServerMessage::Error {
                                    message: format!("pane {pane_id} not found"),
                                },
                                false,
                            )
                        }
                    }
                    ClientMessage::KeyInput { .. } => (ServerMessage::Ack, true),

                    // Agent / AI team protocol — layout-only handling in daemon.
                    // Real PTY-backed handling happens in the event loop's
                    // agent socket (process_agent_ipc).
                    ClientMessage::SplitPane { direction, .. } => {
                        let dir = match direction {
                            emux_ipc::SplitDirection::Horizontal => SplitDirection::Horizontal,
                            emux_ipc::SplitDirection::Vertical => SplitDirection::Vertical,
                        };
                        match session.active_tab_mut().split_pane(dir) {
                            Some(pane_id) => (ServerMessage::SpawnResult { pane_id }, true),
                            None => (
                                ServerMessage::Error {
                                    message: "cannot split pane".into(),
                                },
                                false,
                            ),
                        }
                    }
                    ClientMessage::ListPanes => {
                        let tab = session.active_tab();
                        let positions = tab.compute_positions();
                        let active = tab.active_pane_id();
                        let panes = positions
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
                                has_notification: tab
                                    .pane(*id)
                                    .map(|p| p.has_notification())
                                    .unwrap_or(false),
                            })
                            .collect();
                        (ServerMessage::PaneList { panes }, false)
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
                            (
                                ServerMessage::PaneInfo {
                                    pane: emux_ipc::PaneEntry {
                                        id: pane_id,
                                        title: pane.title().to_owned(),
                                        cols,
                                        rows,
                                        active: tab.active_pane_id() == Some(pane_id),
                                        has_notification: pane.has_notification(),
                                    },
                                },
                                false,
                            )
                        } else {
                            (
                                ServerMessage::Error {
                                    message: format!("pane {pane_id} not found"),
                                },
                                false,
                            )
                        }
                    }
                    ClientMessage::SetPaneTitle { pane_id, title } => {
                        if let Some(pane) = session.active_tab_mut().pane_mut(pane_id) {
                            pane.set_title(title);
                            (ServerMessage::Ack, false)
                        } else {
                            (
                                ServerMessage::Error {
                                    message: format!("pane {pane_id} not found"),
                                },
                                false,
                            )
                        }
                    }
                    ClientMessage::CapturePane { .. }
                    | ClientMessage::SendKeys { .. }
                    | ClientMessage::ResizePane { .. }
                    | ClientMessage::Attach { .. } => (ServerMessage::Ack, false),
                };
                // Always send the reply to the originating client.
                per_client_reply.push((*id, reply.clone()));
                // For state-changing messages, broadcast an Ack to all
                // OTHER clients so they know to refresh.
                if should_broadcast {
                    broadcast_msgs.push(reply);
                }
            }
            Err(emux_ipc::CodecError::Io(ref e))
                if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut => {
            }
            Err(_) => {
                to_remove.push(*id);
            }
        }
    }

    // Send per-client replies.
    for (id, reply) in &per_client_reply {
        if let Some((_, stream)) = clients.iter_mut().find(|(cid, _)| cid == id)
            && codec::write_message(stream, reply).is_err()
        {
            to_remove.push(*id);
        }
    }

    // Broadcast state-change notifications to all clients that did NOT
    // originate the message (session sharing: all viewers see updates).
    if !broadcast_msgs.is_empty() {
        let originator_ids: std::collections::HashSet<u64> =
            per_client_reply.iter().map(|(id, _)| *id).collect();
        for (id, stream) in clients.iter_mut() {
            if !originator_ids.contains(id) {
                for bcast in &broadcast_msgs {
                    if codec::write_message(stream, bcast).is_err() {
                        to_remove.push(*id);
                        break;
                    }
                }
            }
        }
    }

    // Remove disconnected clients.
    to_remove.sort_unstable();
    to_remove.dedup();
    for id in &to_remove {
        clients.retain(|(cid, _)| cid != id);
    }
}
