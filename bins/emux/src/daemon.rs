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
        // Pre-encode messages once, then write the bytes to each client.
        let encoded_msgs: Vec<Vec<u8>> = pty_output
            .iter()
            .filter_map(|(pane_id, data)| {
                let msg = ServerMessage::PtyOutput {
                    pane_id: *pane_id,
                    data: data.clone(),
                };
                emux_ipc::codec::encode(&msg).ok()
            })
            .collect();

        let mut dead_attached = Vec::new();
        for (i, stream) in attached_clients.iter_mut().enumerate() {
            for encoded in &encoded_msgs {
                if std::io::Write::write_all(stream, encoded).is_err() {
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
                                    if let Err(e) = crate::app::pty_write_all(&mut pt.pty, &data) {
                                        eprintln!("daemon: PTY write error: {e}");
                                    }
                                }
                            }
                            let _ = emux_ipc::codec::write_message(stream, &ServerMessage::Ack);
                            to_remove.push((*id, false));
                        }
                        other => {
                            let mut dp = DaemonPanes {
                                terminals: &mut pane_terminals,
                            };
                            let ipc_result =
                                crate::ipc_handler::handle_ipc(&mut session, &mut dp, other);
                            let _ = emux_ipc::codec::write_message(stream, &ipc_result.response);
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

// ---------------------------------------------------------------------------
// PaneAccess impl for daemon's PTY state
// ---------------------------------------------------------------------------

/// Wrapper providing PaneAccess for the daemon's pane terminals.
struct DaemonPanes<'a> {
    terminals: &'a mut std::collections::HashMap<u32, emux_daemon::server::PaneTerminal>,
}

impl crate::ipc_handler::PaneAccess for DaemonPanes<'_> {
    fn capture_pane(&mut self, pane_id: u32) -> Option<String> {
        let pt = self.terminals.get_mut(&pane_id)?;
        // Drain any pending PTY output before capturing.
        let mut buf = [0u8; 65536];
        loop {
            match std::io::Read::read(&mut pt.pty, &mut buf) {
                Ok(0) => break,
                Ok(n) => pt.parser.advance(&mut pt.screen, &buf[..n]),
                Err(_) => break,
            }
        }
        let content = (0..pt.screen.rows())
            .map(|r| pt.screen.row_text(r))
            .collect::<Vec<_>>()
            .join("\n");
        Some(content)
    }

    fn send_keys(&mut self, pane_id: u32, data: &[u8]) -> Result<(), String> {
        if let Some(pt) = self.terminals.get_mut(&pane_id) {
            crate::app::pty_write_all(&mut pt.pty, data).map_err(|e| format!("write error: {e}"))
        } else {
            Err(format!("pane {pane_id} not found"))
        }
    }

    fn remove_pane(&mut self, pane_id: u32) {
        self.terminals.remove(&pane_id);
    }

    fn spawn_pane(&mut self, pane_id: u32, cols: usize, rows: usize) -> Result<(), String> {
        match spawn_pane_terminal(cols, rows) {
            Ok(pt) => {
                self.terminals.insert(pane_id, pt);
                Ok(())
            }
            Err(e) => Err(format!("spawn error: {e}")),
        }
    }
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
