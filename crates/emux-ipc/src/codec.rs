//! Message framing codec for IPC streams.
//!
//! Wire format: `[4-byte big-endian length][serde_json payload]`

use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fmt;
use std::io::{self, Read, Write};

/// Errors that can occur during encoding/decoding.
#[derive(Debug)]
pub enum CodecError {
    Io(io::Error),
    Json(serde_json::Error),
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodecError::Io(e) => write!(f, "IO error: {e}"),
            CodecError::Json(e) => write!(f, "JSON error: {e}"),
        }
    }
}

impl std::error::Error for CodecError {}

impl From<io::Error> for CodecError {
    fn from(e: io::Error) -> Self {
        CodecError::Io(e)
    }
}

impl From<serde_json::Error> for CodecError {
    fn from(e: serde_json::Error) -> Self {
        CodecError::Json(e)
    }
}

/// Encode a message to bytes: `[4-byte big-endian length][serde_json payload]`.
pub fn encode<T: Serialize>(msg: &T) -> Result<Vec<u8>, CodecError> {
    let payload = serde_json::to_vec(msg)?;
    let len = payload.len() as u32;
    let mut buf = Vec::with_capacity(4 + payload.len());
    buf.extend_from_slice(&len.to_be_bytes());
    buf.extend_from_slice(&payload);
    Ok(buf)
}

/// Decode a message from a JSON payload (bytes after the length prefix).
pub fn decode<T: DeserializeOwned>(data: &[u8]) -> Result<T, CodecError> {
    Ok(serde_json::from_slice(data)?)
}

/// Write a length-prefixed message to a writer.
pub fn write_message<W: Write, T: Serialize>(writer: &mut W, msg: &T) -> Result<(), CodecError> {
    let bytes = encode(msg)?;
    writer.write_all(&bytes)?;
    writer.flush()?;
    Ok(())
}

/// Maximum allowed message size (16 MiB).
const MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

/// Read a length-prefixed message from a reader.
pub fn read_message<R: Read, T: DeserializeOwned>(reader: &mut R) -> Result<T, CodecError> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_MESSAGE_SIZE {
        return Err(CodecError::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("message too large: {len} bytes (max {MAX_MESSAGE_SIZE})"),
        )));
    }

    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload)?;
    decode(&payload)
}
