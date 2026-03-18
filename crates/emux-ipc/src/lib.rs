//! Inter-process communication protocol and transport.

pub mod codec;
pub mod messages;
pub mod transport;

pub use codec::{decode, encode, read_message, write_message, CodecError};
pub use messages::{ClientMessage, ServerMessage, SessionEntry, PROTOCOL_VERSION};
pub use transport::{Listener, ReadWrite, SshStream, Transport, TransportError};
