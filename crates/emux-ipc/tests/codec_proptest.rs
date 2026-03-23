use std::io::Cursor;

use emux_ipc::codec;
use emux_ipc::{ClientMessage, PaneEntry, ServerMessage, SessionEntry, SplitDirection};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Strategies for generating arbitrary messages
// ---------------------------------------------------------------------------

fn arb_split_direction() -> impl Strategy<Value = SplitDirection> {
    prop_oneof![
        Just(SplitDirection::Horizontal),
        Just(SplitDirection::Vertical),
    ]
}

fn arb_client_message() -> impl Strategy<Value = ClientMessage> {
    prop_oneof![
        Just(ClientMessage::Ping),
        Just(ClientMessage::GetVersion),
        prop::collection::vec(any::<u8>(), 0..64).prop_map(|data| ClientMessage::KeyInput { data }),
        (any::<u16>(), any::<u16>()).prop_map(|(cols, rows)| ClientMessage::Resize { cols, rows }),
        proptest::option::of("[a-z]{0,8}")
            .prop_map(|direction| ClientMessage::SpawnPane { direction }),
        any::<u32>().prop_map(|pane_id| ClientMessage::KillPane { pane_id }),
        any::<u32>().prop_map(|pane_id| ClientMessage::FocusPane { pane_id }),
        Just(ClientMessage::Detach),
        (any::<u16>(), any::<u16>()).prop_map(|(cols, rows)| ClientMessage::Attach { cols, rows }),
        Just(ClientMessage::ListSessions),
        "[a-z]{1,16}".prop_map(|name| ClientMessage::KillSession { name }),
        (arb_split_direction(), proptest::option::of(any::<u16>()))
            .prop_map(|(direction, size)| ClientMessage::SplitPane { direction, size }),
        any::<u32>().prop_map(|pane_id| ClientMessage::CapturePane { pane_id }),
        (any::<u32>(), "[a-z ]{0,32}")
            .prop_map(|(pane_id, keys)| ClientMessage::SendKeys { pane_id, keys }),
        Just(ClientMessage::ListPanes),
        any::<u32>().prop_map(|pane_id| ClientMessage::GetPaneInfo { pane_id }),
        (any::<u32>(), any::<u16>(), any::<u16>()).prop_map(|(pane_id, cols, rows)| {
            ClientMessage::ResizePane {
                pane_id,
                cols,
                rows,
            }
        }),
        (any::<u32>(), "[a-z]{0,16}")
            .prop_map(|(pane_id, title)| { ClientMessage::SetPaneTitle { pane_id, title } }),
    ]
}

fn arb_pane_entry() -> impl Strategy<Value = PaneEntry> {
    (
        any::<u32>(),
        "[a-z]{0,8}",
        any::<u16>(),
        any::<u16>(),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(
            |(id, title, cols, rows, active, has_notification)| PaneEntry {
                id,
                title,
                cols,
                rows,
                active,
                has_notification,
            },
        )
}

fn arb_session_entry() -> impl Strategy<Value = SessionEntry> {
    (
        "[a-z]{1,8}",
        0..10usize,
        0..10usize,
        1..200usize,
        1..100usize,
    )
        .prop_map(|(name, tabs, panes, cols, rows)| SessionEntry {
            name,
            tabs,
            panes,
            cols,
            rows,
        })
}

fn arb_server_message() -> impl Strategy<Value = ServerMessage> {
    prop_oneof![
        Just(ServerMessage::Pong),
        any::<u32>().prop_map(|version| ServerMessage::Version { version }),
        (any::<u32>(), "[a-z ]{0,32}")
            .prop_map(|(pane_id, content)| ServerMessage::Render { pane_id, content }),
        any::<u32>().prop_map(|pane_id| ServerMessage::SpawnResult { pane_id }),
        "[a-z ]{0,32}".prop_map(|message| ServerMessage::Error { message }),
        Just(ServerMessage::Ack),
        prop::collection::vec(arb_session_entry(), 0..5)
            .prop_map(|sessions| { ServerMessage::SessionList { sessions } }),
        (any::<u32>(), "[a-z ]{0,32}")
            .prop_map(|(pane_id, content)| { ServerMessage::PaneCaptured { pane_id, content } }),
        prop::collection::vec(arb_pane_entry(), 0..5)
            .prop_map(|panes| ServerMessage::PaneList { panes }),
        arb_pane_entry().prop_map(|pane| ServerMessage::PaneInfo { pane }),
        (any::<u32>(), prop::collection::vec(any::<u8>(), 0..64))
            .prop_map(|(pane_id, data)| { ServerMessage::PtyOutput { pane_id, data } }),
        Just(ServerMessage::LayoutChanged),
        Just(ServerMessage::SessionEnded),
    ]
}

// ---------------------------------------------------------------------------
// Property tests
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn client_message_roundtrip(msg in arb_client_message()) {
        let encoded = codec::encode(&msg).unwrap();
        let decoded: ClientMessage = codec::decode(&encoded[4..]).unwrap();
        prop_assert_eq!(msg, decoded);
    }

    #[test]
    fn server_message_roundtrip(msg in arb_server_message()) {
        let encoded = codec::encode(&msg).unwrap();
        let decoded: ServerMessage = codec::decode(&encoded[4..]).unwrap();
        prop_assert_eq!(msg, decoded);
    }

    #[test]
    fn client_stream_roundtrip(msg in arb_client_message()) {
        let mut buf = Vec::new();
        codec::write_message(&mut buf, &msg).unwrap();
        let mut cursor = Cursor::new(buf);
        let decoded: ClientMessage = codec::read_message(&mut cursor).unwrap();
        prop_assert_eq!(msg, decoded);
    }

    #[test]
    fn server_stream_roundtrip(msg in arb_server_message()) {
        let mut buf = Vec::new();
        codec::write_message(&mut buf, &msg).unwrap();
        let mut cursor = Cursor::new(buf);
        let decoded: ServerMessage = codec::read_message(&mut cursor).unwrap();
        prop_assert_eq!(msg, decoded);
    }

    #[test]
    fn length_prefix_correctness(msg in arb_client_message()) {
        let encoded = codec::encode(&msg).unwrap();
        let len = u32::from_be_bytes([encoded[0], encoded[1], encoded[2], encoded[3]]) as usize;
        prop_assert_eq!(len, encoded.len() - 4);
    }
}

#[test]
fn truncated_payload_returns_error() {
    let msg = ClientMessage::Ping;
    let encoded = codec::encode(&msg).unwrap();
    // Truncate the payload.
    let truncated = &encoded[..encoded.len() - 1];
    let mut cursor = Cursor::new(truncated);
    let result: Result<ClientMessage, _> = codec::read_message(&mut cursor);
    assert!(result.is_err());
}

#[test]
fn oversized_message_returns_error() {
    // Craft a length prefix claiming 20 MiB.
    let huge_len: u32 = 20 * 1024 * 1024;
    let mut buf = Vec::new();
    buf.extend_from_slice(&huge_len.to_be_bytes());
    buf.extend_from_slice(b"{}"); // tiny payload
    let mut cursor = Cursor::new(buf);
    let result: Result<ClientMessage, _> = codec::read_message(&mut cursor);
    assert!(result.is_err());
}
