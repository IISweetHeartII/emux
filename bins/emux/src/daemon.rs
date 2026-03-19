use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;


use emux_ipc::{ClientMessage, ServerMessage};
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
    { socket_dir().join(format!("emux-{name}.sock")) }
    #[cfg(windows)]
    { socket_dir().join(format!("emux-{name}.port")) }
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
            && let Some(name) = fname.strip_prefix(prefix).and_then(|s| s.strip_suffix(suffix))
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

/// The daemon event loop — accepts clients and processes messages.
///
/// The `listener` parameter is a `UnixListener` on Unix or a `TcpListener` on
/// Windows. Both implement similar accept/set_nonblocking APIs.
#[cfg(unix)]
pub(crate) fn run_daemon_loop(
    session_name: &str,
    listener: std::os::unix::net::UnixListener,
    socket_path: &Path,
) {
    let mut session = Session::new(session_name, 80, 24);
    let mut clients: Vec<(u64, std::os::unix::net::UnixStream)> = Vec::new();
    let mut next_id: u64 = 1;
    let mut shutdown = false;

    while !shutdown {
        // Accept new connections (non-blocking).
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(Duration::from_millis(50)));
                clients.push((next_id, stream));
                next_id += 1;
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(_) => break,
        }

        run_daemon_loop_body(&mut session, &mut clients, &mut next_id, &mut shutdown);

        std::thread::sleep(Duration::from_millis(100));
    }

    // Cleanup socket on shutdown.
    let _ = std::fs::remove_file(socket_path);
}

/// The daemon event loop — Windows variant using `TcpListener`.
#[cfg(windows)]
pub(crate) fn run_daemon_loop(
    session_name: &str,
    listener: std::net::TcpListener,
    socket_path: &Path,
) {
    let mut session = Session::new(session_name, 80, 24);
    let mut clients: Vec<(u64, std::net::TcpStream)> = Vec::new();
    let mut next_id: u64 = 1;
    let mut shutdown = false;

    while !shutdown {
        // Accept new connections (non-blocking).
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(Duration::from_millis(50)));
                clients.push((next_id, stream));
                next_id += 1;
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(_) => break,
        }

        run_daemon_loop_body(&mut session, &mut clients, &mut next_id, &mut shutdown);

        std::thread::sleep(Duration::from_millis(100));
    }

    // Cleanup port file on shutdown.
    let _ = std::fs::remove_file(socket_path);
}

/// Shared daemon loop body: process messages from all clients.
///
/// This is factored out to avoid duplicating the message-handling logic
/// between the Unix and Windows `run_daemon_loop` variants.
fn run_daemon_loop_body<S: std::io::Read + std::io::Write>(
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
                    ClientMessage::GetVersion => (ServerMessage::Version {
                        version: emux_ipc::PROTOCOL_VERSION,
                    }, false),
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
                        (ServerMessage::SessionList {
                            sessions: vec![entry],
                        }, false)
                    }
                    ClientMessage::KillSession { ref name } => {
                        if name == session.name() {
                            *shutdown = true;
                            (ServerMessage::Ack, false)
                        } else {
                            (ServerMessage::Error {
                                message: format!("no such session: {name}"),
                            }, false)
                        }
                    }
                    ClientMessage::SpawnPane { ref direction } => {
                        let dir = match direction.as_deref() {
                            Some("horizontal") => SplitDirection::Horizontal,
                            _ => SplitDirection::Vertical,
                        };
                        match session.active_tab_mut().split_pane(dir) {
                            Some(pane_id) => (ServerMessage::SpawnResult { pane_id }, true),
                            None => (ServerMessage::Error {
                                message: "cannot split pane".into(),
                            }, false),
                        }
                    }
                    ClientMessage::KillPane { pane_id } => {
                        if session.active_tab_mut().close_pane(pane_id) {
                            (ServerMessage::Ack, true)
                        } else {
                            (ServerMessage::Error {
                                message: format!("cannot kill pane {pane_id}"),
                            }, false)
                        }
                    }
                    ClientMessage::FocusPane { pane_id } => {
                        if session.active_tab_mut().focus_pane(pane_id) {
                            (ServerMessage::Ack, true)
                        } else {
                            (ServerMessage::Error {
                                message: format!("pane {pane_id} not found"),
                            }, false)
                        }
                    }
                    ClientMessage::KeyInput { .. } => (ServerMessage::Ack, true),

                    // Agent / AI team protocol -- stubs until handler implementation.
                    ClientMessage::SplitPane { .. }
                    | ClientMessage::CapturePane { .. }
                    | ClientMessage::SendKeys { .. }
                    | ClientMessage::ListPanes
                    | ClientMessage::GetPaneInfo { .. }
                    | ClientMessage::ResizePane { .. }
                    | ClientMessage::SetPaneTitle { .. } => (ServerMessage::Error {
                        message: "not yet implemented".into(),
                    }, false),
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
                if e.kind() == io::ErrorKind::WouldBlock
                    || e.kind() == io::ErrorKind::TimedOut => {}
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
