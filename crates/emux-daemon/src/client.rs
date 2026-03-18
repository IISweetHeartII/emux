//! Client-side daemon connection logic.

use std::os::unix::net::UnixStream;
use std::path::Path;

use emux_ipc::{codec, ClientMessage, ServerMessage};

use crate::DaemonError;

/// A client connected to a daemon over a Unix domain socket.
pub struct DaemonClient {
    stream: UnixStream,
}

impl DaemonClient {
    /// Connect to the daemon at the given socket path.
    pub fn connect(socket_path: &Path) -> Result<Self, DaemonError> {
        let stream = UnixStream::connect(socket_path)?;
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
