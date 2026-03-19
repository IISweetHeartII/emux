//! Inter-process communication protocol and transport.

pub mod codec;
pub mod messages;
pub mod transport;

pub use codec::{CodecError, decode, encode, read_message, write_message};
pub use messages::{
    ClientMessage, PROTOCOL_VERSION, PaneEntry, ServerMessage, SessionEntry, SplitDirection,
};
pub use transport::{Listener, ReadWrite, SshStream, Transport, TransportError};
