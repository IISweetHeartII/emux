//! Client-side daemon connection logic.

#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::path::Path;

use emux_ipc::{codec, ClientMessage, ServerMessage};

use crate::DaemonError;

/// A client connected to a daemon over a Unix domain socket (Unix) or TCP
/// loopback (Windows).
pub struct DaemonClient {
    #[cfg(unix)]
    stream: UnixStream,
    #[cfg(windows)]
    stream: std::net::TcpStream,
}

impl DaemonClient {
    /// Connect to the daemon at the given socket path.
    ///
    /// On Unix this connects to a Unix domain socket. On Windows the path is
    /// expected to be a file containing the TCP port number; the client
    /// connects to `127.0.0.1:<port>`.
    pub fn connect(socket_path: &Path) -> Result<Self, DaemonError> {
        #[cfg(unix)]
        let stream = UnixStream::connect(socket_path)?;

        #[cfg(windows)]
        let stream = {
            let contents = std::fs::read_to_string(socket_path).map_err(|e| {
                DaemonError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("cannot read port file {}: {e}", socket_path.display()),
                ))
            })?;
            let port: u16 = contents.trim().parse().map_err(|_| {
                DaemonError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("invalid port in {}", socket_path.display()),
                ))
            })?;
            std::net::TcpStream::connect(("127.0.0.1", port))?
        };

        Ok(Self { stream })
    }

    /// Send a message to the daemon.
    pub fn send(&mut self, msg: ClientMessage) -> Result<(), DaemonError> {
        codec::write_message(&mut self.stream, &msg)?;
        Ok(())
    }

    /// Receive a message from the daemon (blocking).
    pub fn recv(&mut self) -> Result<ServerMessage, DaemonError> {
        let msg: ServerMessage = codec::read_message(&mut self.stream)?;
        Ok(msg)
    }

    /// Send a Ping and expect a Pong back.
    pub fn ping(&mut self) -> Result<(), DaemonError> {
        self.send(ClientMessage::Ping)?;
        let reply = self.recv()?;
        match reply {
            ServerMessage::Pong => Ok(()),
            other => Err(DaemonError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("expected Pong, got {other:?}"),
            ))),
        }
    }

    /// Detach from the daemon (sends Detach then drops the connection).
    pub fn detach(mut self) {
        let _ = self.send(ClientMessage::Detach);
        // stream is dropped here, closing the connection
    }
}
