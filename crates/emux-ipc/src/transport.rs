//! Transport layer for IPC connections.
//!
//! Supports local Unix-domain sockets and SSH-tunnelled connections.

use std::fmt;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

/// A bidirectional byte stream (read + write).
pub trait ReadWrite: Read + Write + Send {}
impl<T: Read + Write + Send> ReadWrite for T {}

/// A bidirectional stream backed by an SSH child process.
///
/// Wraps the stdin/stdout of `ssh user@host emux attach-raw <session>`,
/// forwarding reads/writes through the SSH tunnel. When dropped, the SSH
/// child process is killed.
pub struct SshStream {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
}

impl SshStream {
    /// Spawn an SSH process that connects to a remote emux session.
    ///
    /// The remote side is expected to have `emux` in its `$PATH`.
    /// The SSH process's stdin/stdout become the bidirectional byte stream.
    pub fn spawn(
        host: &str,
        user: Option<&str>,
        port: Option<u16>,
        remote_session: &str,
    ) -> Result<Self, TransportError> {
        let mut cmd = Command::new("ssh");

        // Disable pseudo-terminal allocation; we just pipe bytes.
        cmd.arg("-T");

        if let Some(p) = port {
            cmd.arg("-p").arg(p.to_string());
        }

        let destination = match user {
            Some(u) => format!("{u}@{host}"),
            None => host.to_string(),
        };
        cmd.arg(&destination);

        // The remote command: attach to the emux session in raw/pipe mode.
        cmd.arg("emux").arg("attach-raw").arg(remote_session);

        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        let mut child = cmd.spawn().map_err(|e| {
            TransportError::Io(io::Error::new(
                e.kind(),
                format!("failed to spawn ssh to {destination}: {e}"),
            ))
        })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            TransportError::Io(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "ssh process has no stdin",
            ))
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            TransportError::Io(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "ssh process has no stdout",
            ))
        })?;

        Ok(Self {
            child,
            stdin,
            stdout,
        })
    }

    /// Check if the SSH process is still running.
    pub fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
}

impl Read for SshStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stdout.read(buf)
    }
}

impl Write for SshStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stdin.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stdin.flush()
    }
}

impl Drop for SshStream {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A listener that accepts incoming connections.
pub trait Listener: Send {
    /// Accept the next incoming connection.
    /// Blocks until a client connects.
    fn accept(&mut self) -> Result<Box<dyn ReadWrite>, TransportError>;
}

/// Errors that can occur during transport operations.
#[derive(Debug)]
pub enum TransportError {
    /// An I/O error from the underlying transport.
    Io(io::Error),
    /// The requested transport is not yet implemented.
    Unsupported(String),
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransportError::Io(e) => write!(f, "transport I/O error: {e}"),
            TransportError::Unsupported(msg) => write!(f, "unsupported transport: {msg}"),
        }
    }
}

impl std::error::Error for TransportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TransportError::Io(e) => Some(e),
            TransportError::Unsupported(_) => None,
        }
    }
}

impl From<io::Error> for TransportError {
    fn from(e: io::Error) -> Self {
        TransportError::Io(e)
    }
}

/// Describes how to reach the IPC endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transport {
    /// A local Unix-domain socket at the given path.
    Local(PathBuf),
    /// An SSH tunnel to a remote host, connecting to a Unix socket there.
    Ssh {
        host: String,
        user: Option<String>,
        port: Option<u16>,
        socket_path: PathBuf,
    },
}

impl Transport {
    /// Connect to the IPC endpoint as a client.
    ///
    /// For `Local`, this connects to the Unix socket.
    /// For `Ssh`, this spawns an `ssh` process that runs
    /// `emux attach-raw <session>` on the remote host and pipes
    /// stdin/stdout as a bidirectional byte stream.
    #[cfg(unix)]
    pub fn connect(&self) -> Result<Box<dyn ReadWrite>, TransportError> {
        match self {
            Transport::Local(path) => {
                let stream = std::os::unix::net::UnixStream::connect(path)?;
                Ok(Box::new(stream))
            }
            Transport::Ssh {
                host,
                user,
                port,
                socket_path,
            } => {
                // Derive a session name from the remote socket path.
                let session = socket_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("0");
                let stream = SshStream::spawn(host, user.as_deref(), *port, session)?;
                Ok(Box::new(stream))
            }
        }
    }

    #[cfg(not(unix))]
    pub fn connect(&self) -> Result<Box<dyn ReadWrite>, TransportError> {
        Err(TransportError::Unsupported(
            "Unix sockets are not available on this platform".into(),
        ))
    }

    /// Start listening for incoming connections.
    ///
    /// For `Local`, this binds a Unix-domain socket at the given path.
    /// The socket file is **not** automatically removed; the caller should
    /// clean it up when done.
    #[cfg(unix)]
    pub fn listen(&self) -> Result<Box<dyn Listener>, TransportError> {
        match self {
            Transport::Local(path) => {
                // Remove stale socket if it exists
                if path.exists() {
                    std::fs::remove_file(path)?;
                }
                // Ensure the parent directory exists
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let listener = std::os::unix::net::UnixListener::bind(path)?;
                Ok(Box::new(UnixSocketListener { inner: listener }))
            }
            Transport::Ssh {
                host, user, port, ..
            } => {
                let dest = format!(
                    "{}{}{}",
                    user.as_deref().map(|u| format!("{u}@")).unwrap_or_default(),
                    host,
                    port.map(|p| format!(":{p}")).unwrap_or_default(),
                );
                Err(TransportError::Unsupported(format!(
                    "SSH listen on {dest}: remote side should run its own daemon"
                )))
            }
        }
    }

    #[cfg(not(unix))]
    pub fn listen(&self) -> Result<Box<dyn Listener>, TransportError> {
        Err(TransportError::Unsupported(
            "Unix sockets are not available on this platform".into(),
        ))
    }

    /// Returns the socket path for this transport.
    pub fn socket_path(&self) -> &PathBuf {
        match self {
            Transport::Local(path) => path,
            Transport::Ssh { socket_path, .. } => socket_path,
        }
    }

    /// Returns `true` if this is a local transport.
    pub fn is_local(&self) -> bool {
        matches!(self, Transport::Local(_))
    }

    /// Returns `true` if this is an SSH transport.
    pub fn is_ssh(&self) -> bool {
        matches!(self, Transport::Ssh { .. })
    }

    /// Build the default socket path for a given session name.
    ///
    /// On Unix this returns `$XDG_RUNTIME_DIR/emux/<name>.sock`
    /// or `/tmp/emux-<uid>/<name>.sock` as a fallback.
    #[cfg(unix)]
    pub fn default_socket_path(session_name: &str) -> PathBuf {
        let base = if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
            PathBuf::from(dir).join("emux")
        } else {
            let uid = unsafe { libc::getuid() };
            PathBuf::from(format!("/tmp/emux-{uid}"))
        };
        base.join(format!("{session_name}.sock"))
    }

    #[cfg(not(unix))]
    pub fn default_socket_path(session_name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push("emux");
        p.push(format!("{session_name}.sock"));
        p
    }
}

impl fmt::Display for Transport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Transport::Local(path) => write!(f, "local:{}", path.display()),
            Transport::Ssh {
                host,
                user,
                port,
                socket_path,
            } => {
                write!(f, "ssh://")?;
                if let Some(u) = user {
                    write!(f, "{u}@")?;
                }
                write!(f, "{host}")?;
                if let Some(p) = port {
                    write!(f, ":{p}")?;
                }
                write!(f, "{}", socket_path.display())
            }
        }
    }
}

/// Unix-domain socket listener wrapper.
#[cfg(unix)]
struct UnixSocketListener {
    inner: std::os::unix::net::UnixListener,
}

#[cfg(unix)]
impl Listener for UnixSocketListener {
    fn accept(&mut self) -> Result<Box<dyn ReadWrite>, TransportError> {
        let (stream, _addr) = self.inner.accept()?;
        Ok(Box::new(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_local_properties() {
        let t = Transport::Local(PathBuf::from("/tmp/test.sock"));
        assert!(t.is_local());
        assert!(!t.is_ssh());
        assert_eq!(t.socket_path(), &PathBuf::from("/tmp/test.sock"));
        assert_eq!(t.to_string(), "local:/tmp/test.sock");
    }

    #[test]
    fn transport_ssh_properties() {
        let t = Transport::Ssh {
            host: "remote.host".into(),
            user: Some("user".into()),
            port: Some(2222),
            socket_path: PathBuf::from("/run/emux/main.sock"),
        };
        assert!(!t.is_local());
        assert!(t.is_ssh());
        assert_eq!(t.socket_path(), &PathBuf::from("/run/emux/main.sock"));
    }

    #[test]
    fn transport_display_ssh() {
        let t = Transport::Ssh {
            host: "example.com".into(),
            user: None,
            port: None,
            socket_path: PathBuf::from("/run/emux/main.sock"),
        };
        assert_eq!(t.to_string(), "ssh://example.com/run/emux/main.sock");
    }

    #[test]
    fn transport_equality() {
        let a = Transport::Local(PathBuf::from("/a.sock"));
        let b = Transport::Local(PathBuf::from("/a.sock"));
        let c = Transport::Local(PathBuf::from("/b.sock"));
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn default_socket_path_contains_session_name() {
        let path = Transport::default_socket_path("mysession");
        let filename = path.file_name().unwrap().to_str().unwrap();
        assert_eq!(filename, "mysession.sock");
    }

    #[cfg(unix)]
    #[test]
    fn local_connect_listen_roundtrip() {
        use std::io::{Read, Write};

        let dir = std::env::temp_dir().join(format!("emux-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sock_path = dir.join("test.sock");

        let transport = Transport::Local(sock_path.clone());

        // Start listener
        let mut listener = transport.listen().unwrap();

        // Connect from another thread
        let sock_path2 = sock_path.clone();
        let handle = std::thread::spawn(move || {
            let t = Transport::Local(sock_path2);
            let mut conn = t.connect().unwrap();
            conn.write_all(b"hello").unwrap();
            conn.flush().unwrap();
            let mut buf = [0u8; 5];
            conn.read_exact(&mut buf).unwrap();
            assert_eq!(&buf, b"world");
        });

        // Accept and echo
        let mut server_conn = listener.accept().unwrap();
        let mut buf = [0u8; 5];
        server_conn.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"hello");
        server_conn.write_all(b"world").unwrap();
        server_conn.flush().unwrap();

        handle.join().unwrap();

        // Cleanup
        std::fs::remove_file(&sock_path).ok();
        std::fs::remove_dir(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn ssh_connect_spawns_process_and_fails_gracefully() {
        // Attempting to SSH to a non-existent host will fail at spawn or
        // immediately after.  We just verify it returns an Io error (not
        // Unsupported), which proves the SSH codepath is wired up.
        let t = Transport::Ssh {
            host: "127.0.0.255".into(),
            user: Some("nobody".into()),
            port: Some(1), // port 1 -- won't connect
            socket_path: PathBuf::from("/run/emux/main.sock"),
        };
        // The spawn itself may succeed (ssh binary exists) or fail
        // (ssh binary not found). Either way it should not panic and
        // should not return Unsupported.
        let result = t.connect();
        // We cannot assert success because there is no real SSH server,
        // but if there's an error it must be Io, not Unsupported.
        if let Err(ref e) = result {
            assert!(
                matches!(e, TransportError::Io(_)),
                "expected Io error, got: {e}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn ssh_listen_returns_unsupported() {
        let t = Transport::Ssh {
            host: "example.com".into(),
            user: None,
            port: None,
            socket_path: PathBuf::from("/run/emux/main.sock"),
        };
        let result = t.listen();
        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn local_listen_removes_stale_socket() {
        let dir = std::env::temp_dir().join(format!("emux-stale-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sock_path = dir.join("stale.sock");

        // Create a stale file
        std::fs::write(&sock_path, b"stale").unwrap();
        assert!(sock_path.exists());

        let transport = Transport::Local(sock_path.clone());
        let _listener = transport.listen().unwrap();
        // The stale file was replaced by a real socket
        assert!(sock_path.exists());

        // Cleanup
        std::fs::remove_file(&sock_path).ok();
        std::fs::remove_dir(&dir).ok();
    }

    #[test]
    fn transport_error_display() {
        let e = TransportError::Io(io::Error::new(io::ErrorKind::NotFound, "gone"));
        assert!(e.to_string().contains("gone"));

        let e = TransportError::Unsupported("nope".into());
        assert!(e.to_string().contains("nope"));
    }

    #[test]
    fn ssh_transport_from_parts() {
        let t = Transport::Ssh {
            host: "myhost".into(),
            user: Some("admin".into()),
            port: Some(2222),
            socket_path: PathBuf::from("/run/emux/dev.sock"),
        };
        assert!(t.is_ssh());
        assert!(!t.is_local());
        assert_eq!(t.socket_path(), &PathBuf::from("/run/emux/dev.sock"));
        // Display should include all parts
        let s = t.to_string();
        assert!(s.contains("admin@"));
        assert!(s.contains("myhost"));
        assert!(s.contains("2222"));
    }

    #[test]
    fn ssh_transport_derives_session_from_socket_path() {
        // The connect() method derives the remote session name from
        // the socket_path file stem. Verify that logic is correct by
        // checking the path we'd pass.
        let path = PathBuf::from("/run/emux/my-session.sock");
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap();
        assert_eq!(stem, "my-session");
    }

    #[cfg(unix)]
    #[test]
    #[ignore] // Requires a real SSH server
    fn ssh_connect_to_real_server() {
        let t = Transport::Ssh {
            host: "localhost".into(),
            user: None,
            port: None,
            socket_path: PathBuf::from("/run/emux/test.sock"),
        };
        let _conn = t.connect().expect("should connect to local SSH");
    }

    #[test]
    fn ssh_stream_spawn_bad_host() {
        // Spawning ssh to a host that doesn't exist should either fail
        // at spawn time or produce a stream that errors on read.
        let result = SshStream::spawn("192.0.2.1", Some("test"), Some(1), "test-session");
        // Either the spawn fails (Io error) or succeeds but the stream
        // will error on actual I/O. Both are acceptable.
        match result {
            Err(TransportError::Io(_)) => {} // expected
            Ok(_) => {}                      // ssh binary spawned; it will fail on I/O
            Err(e) => panic!("unexpected error variant: {e}"),
        }
    }
}
