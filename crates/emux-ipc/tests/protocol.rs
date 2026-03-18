//! TDD specs for the IPC protocol (message encoding/decoding).
//!
//! All communication between the daemon and clients goes through a
//! length-prefixed binary protocol. These tests verify correct serialization,
//! deserialization, and edge-case handling.

use emux_ipc::codec::{self, CodecError};
use emux_ipc::messages::{ClientMessage, ServerMessage, PROTOCOL_VERSION};
use std::io::Cursor;

// ---------------------------------------------------------------------------
// 1. Individual message types
// ---------------------------------------------------------------------------

#[test]
fn encode_decode_ping() {
    let msg = ClientMessage::Ping;
    let bytes = codec::encode(&msg);
    // skip 4-byte length prefix
    let decoded: ClientMessage = codec::decode(&bytes[4..]).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
fn encode_decode_pong() {
    let msg = ServerMessage::Pong;
    let bytes = codec::encode(&msg);
    let decoded: ServerMessage = codec::decode(&bytes[4..]).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
fn encode_decode_key_input() {
    let msg = ClientMessage::KeyInput {
        data: vec![0x1b, 0x5b, 0x41], // ESC [ A (arrow up)
    };
    let bytes = codec::encode(&msg);
    let decoded: ClientMessage = codec::decode(&bytes[4..]).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
fn encode_decode_render() {
    let content = "A".repeat(80 * 24);
    let msg = ServerMessage::Render {
        pane_id: 1,
        content,
    };
    let bytes = codec::encode(&msg);
    let decoded: ServerMessage = codec::decode(&bytes[4..]).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
fn encode_decode_spawn_pane() {
    let msg = ClientMessage::SpawnPane {
        direction: Some("right".to_string()),
    };
    let bytes = codec::encode(&msg);
    let decoded: ClientMessage = codec::decode(&bytes[4..]).unwrap();
    assert_eq!(msg, decoded);

    // Also test with None direction
    let msg2 = ClientMessage::SpawnPane { direction: None };
    let bytes2 = codec::encode(&msg2);
    let decoded2: ClientMessage = codec::decode(&bytes2[4..]).unwrap();
    assert_eq!(msg2, decoded2);
}

#[test]
fn encode_decode_resize() {
    let msg = ClientMessage::Resize {
        cols: 120,
        rows: 40,
    };
    let bytes = codec::encode(&msg);
    let decoded: ClientMessage = codec::decode(&bytes[4..]).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
fn encode_decode_close_pane() {
    let msg = ClientMessage::KillPane { pane_id: 42 };
    let bytes = codec::encode(&msg);
    let decoded: ClientMessage = codec::decode(&bytes[4..]).unwrap();
    assert_eq!(msg, decoded);
}

// ---------------------------------------------------------------------------
// 2. Roundtrip properties
// ---------------------------------------------------------------------------

#[test]
fn roundtrip_preserves_data_identity() {
    // Client messages
    let client_msgs: Vec<ClientMessage> = vec![
        ClientMessage::Ping,
        ClientMessage::GetVersion,
        ClientMessage::KeyInput {
            data: vec![0x61, 0x62, 0x63],
        },
        ClientMessage::Resize { cols: 80, rows: 24 },
        ClientMessage::SpawnPane {
            direction: Some("down".into()),
        },
        ClientMessage::KillPane { pane_id: 7 },
        ClientMessage::FocusPane { pane_id: 3 },
        ClientMessage::Detach,
    ];
    for msg in &client_msgs {
        let mut buf = Vec::new();
        codec::write_message(&mut buf, msg).unwrap();
        let mut cursor = Cursor::new(&buf);
        let decoded: ClientMessage = codec::read_message(&mut cursor).unwrap();
        assert_eq!(msg, &decoded);
    }

    // Server messages
    let server_msgs: Vec<ServerMessage> = vec![
        ServerMessage::Pong,
        ServerMessage::Version {
            version: PROTOCOL_VERSION,
        },
        ServerMessage::Render {
            pane_id: 0,
            content: "hello".into(),
        },
        ServerMessage::SpawnResult { pane_id: 1 },
        ServerMessage::Error {
            message: "bad".into(),
        },
        ServerMessage::Ack,
    ];
    for msg in &server_msgs {
        let mut buf = Vec::new();
        codec::write_message(&mut buf, msg).unwrap();
        let mut cursor = Cursor::new(&buf);
        let decoded: ServerMessage = codec::read_message(&mut cursor).unwrap();
        assert_eq!(msg, &decoded);
    }
}

#[test]
fn roundtrip_empty_payload() {
    // Ping and Pong have no inner data; verify the wire format is compact.
    let ping_bytes = codec::encode(&ClientMessage::Ping);
    let pong_bytes = codec::encode(&ServerMessage::Pong);

    // 4 bytes length prefix + JSON payload (e.g. `"Ping"`)
    // The payload should be small and deterministic.
    let ping_payload = &ping_bytes[4..];
    let pong_payload = &pong_bytes[4..];

    // Verify length prefix matches actual payload size
    let ping_len = u32::from_le_bytes(ping_bytes[..4].try_into().unwrap()) as usize;
    assert_eq!(ping_len, ping_payload.len());

    let pong_len = u32::from_le_bytes(pong_bytes[..4].try_into().unwrap()) as usize;
    assert_eq!(pong_len, pong_payload.len());

    // Roundtrip
    let decoded_ping: ClientMessage = codec::decode(ping_payload).unwrap();
    assert_eq!(decoded_ping, ClientMessage::Ping);

    let decoded_pong: ServerMessage = codec::decode(pong_payload).unwrap();
    assert_eq!(decoded_pong, ServerMessage::Pong);
}

// ---------------------------------------------------------------------------
// 3. Edge cases
// ---------------------------------------------------------------------------

#[test]
fn large_payload_64kb() {
    let content = "X".repeat(64 * 1024);
    let msg = ServerMessage::Render {
        pane_id: 99,
        content: content.clone(),
    };
    let mut buf = Vec::new();
    codec::write_message(&mut buf, &msg).unwrap();
    let mut cursor = Cursor::new(&buf);
    let decoded: ServerMessage = codec::read_message(&mut cursor).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
fn multiple_messages_in_sequence() {
    let messages: Vec<ClientMessage> = vec![
        ClientMessage::Ping,
        ClientMessage::KeyInput {
            data: vec![0x0d],
        },
        ClientMessage::Resize { cols: 80, rows: 24 },
        ClientMessage::Detach,
    ];

    let mut buf = Vec::new();
    for msg in &messages {
        codec::write_message(&mut buf, msg).unwrap();
    }

    let mut cursor = Cursor::new(&buf);
    for expected in &messages {
        let decoded: ClientMessage = codec::read_message(&mut cursor).unwrap();
        assert_eq!(expected, &decoded);
    }
}

#[test]
fn partial_read_blocks_until_complete() {
    // Encode a message, then try to read from a truncated buffer.
    // The reader should return an IO error (UnexpectedEof) rather than garbage.
    let msg = ClientMessage::Ping;
    let bytes = codec::encode(&msg);

    // Truncate: only provide the length prefix + half the payload
    let half = 4 + (bytes.len() - 4) / 2;
    let truncated = &bytes[..half];
    let mut cursor = Cursor::new(truncated);
    let result: Result<ClientMessage, CodecError> = codec::read_message(&mut cursor);
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// 4. Versioning and unknown types
// ---------------------------------------------------------------------------

#[test]
fn version_negotiation_compatible() {
    // Client sends GetVersion, server responds with matching version.
    let request = ClientMessage::GetVersion;
    let response = ServerMessage::Version {
        version: PROTOCOL_VERSION,
    };

    let mut buf = Vec::new();
    codec::write_message(&mut buf, &request).unwrap();
    codec::write_message(&mut buf, &response).unwrap();

    let mut cursor = Cursor::new(&buf);
    let decoded_req: ClientMessage = codec::read_message(&mut cursor).unwrap();
    let decoded_resp: ServerMessage = codec::read_message(&mut cursor).unwrap();

    assert_eq!(decoded_req, ClientMessage::GetVersion);
    if let ServerMessage::Version { version } = decoded_resp {
        assert_eq!(version, PROTOCOL_VERSION);
    } else {
        panic!("expected Version response");
    }
}

#[test]
fn version_negotiation_incompatible() {
    // Server responds with a different (newer) version; client detects mismatch.
    let response = ServerMessage::Version { version: 999 };

    let mut buf = Vec::new();
    codec::write_message(&mut buf, &response).unwrap();

    let mut cursor = Cursor::new(&buf);
    let decoded: ServerMessage = codec::read_message(&mut cursor).unwrap();

    if let ServerMessage::Version { version } = decoded {
        assert_ne!(version, PROTOCOL_VERSION, "versions should differ");
    } else {
        panic!("expected Version response");
    }
}

#[test]
fn unknown_message_type_returns_error() {
    // Craft a payload with an unrecognized enum variant.
    let bogus_json = br#""BogusVariantThatDoesNotExist""#;
    let result: Result<ClientMessage, CodecError> = codec::decode(bogus_json);
    assert!(result.is_err());

    let result2: Result<ServerMessage, CodecError> = codec::decode(bogus_json);
    assert!(result2.is_err());
}
