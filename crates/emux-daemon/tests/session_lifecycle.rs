//! TDD specs for the session daemon lifecycle.
//!
//! The daemon owns sessions and outlives any individual client connection.
//! Clients attach/detach over a Unix socket. Sessions can be snapshotted to
//! disk and restored.

use std::path::PathBuf;

use emux_daemon::ClientId;
use emux_daemon::client::DaemonClient;
use emux_daemon::persistence;
use emux_daemon::server::DaemonServer;
use emux_ipc::{ClientMessage, ServerMessage};

/// Helper: generate a short unique session name.
///
/// Uses a compact format to stay within the Unix socket path limit
/// (104 bytes on macOS).  Format: `t-{base_prefix}-{pid}-{counter}`.
fn unique_name(base: &str) -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let prefix: String = base.chars().take(8).collect();
    format!("t{}-{}-{n}", std::process::id(), prefix)
}

/// Helper: temp directory for snapshot tests.
fn temp_snapshot_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emux-snap-{}", unique_name("dir")));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// ---------------------------------------------------------------------------
// 1. Daemon startup
// ---------------------------------------------------------------------------

#[test]
fn start_daemon_creates_socket() {
    let name = unique_name("creates-socket");
    let server = DaemonServer::start(&name).unwrap();
    assert!(
        server.socket_path().exists(),
        "socket file should exist after start"
    );
    server.shutdown();
}

#[test]
fn start_daemon_creates_default_session() {
    // On first start, the daemon should create a default session with one tab
    // and one pane.
    let name = unique_name("default-session");
    let server = DaemonServer::start(&name).unwrap();
    assert_eq!(server.session().tab_count(), 1);
    assert_eq!(server.session().active_tab().pane_count(), 1);
    assert_eq!(server.session().name(), name);
    server.shutdown();
}

#[test]
fn start_daemon_refuses_duplicate_socket() {
    // If a socket already exists and the owning process is alive, starting a
    // second daemon for the same session should fail with an error.
    let name = unique_name("duplicate-socket");
    let server1 = DaemonServer::start(&name).unwrap();
    // Second start should fail because the socket is in use.
    let result = DaemonServer::start(&name);
    assert!(result.is_err());
    server1.shutdown();
}

// ---------------------------------------------------------------------------
// 2. Client attach / detach
// ---------------------------------------------------------------------------

#[test]
fn client_attach_to_daemon() {
    let name = unique_name("attach");
    let mut server = DaemonServer::start(&name).unwrap();

    // Client connects.
    let client = DaemonClient::connect(server.socket_path()).unwrap();
    let _client_id = server.accept_client().unwrap();

    assert_eq!(server.client_count(), 1);
    drop(client);
    server.shutdown();
}

#[test]
fn client_detach_preserves_session() {
    let name = unique_name("detach");
    let mut server = DaemonServer::start(&name).unwrap();

    let client = DaemonClient::connect(server.socket_path()).unwrap();
    let client_id = server.accept_client().unwrap();

    // Client detaches.
    client.detach();
    server.disconnect_client(client_id);

    // Session is still alive.
    assert_eq!(server.client_count(), 0);
    assert_eq!(server.session().name(), name);
    server.shutdown();
}

#[test]
fn client_detach_from_daemon() {
    // After detach, the client's connection is closed but the session and all
    // pane processes keep running.
    let name = unique_name("detach-daemon");
    let mut server = DaemonServer::start(&name).unwrap();
    let client = DaemonClient::connect(server.socket_path()).unwrap();
    let client_id = server.accept_client().unwrap();
    assert_eq!(server.client_count(), 1);

    client.detach();
    server.disconnect_client(client_id);
    assert_eq!(server.client_count(), 0);
    // Session is still alive
    assert_eq!(server.session().tab_count(), 1);
    server.shutdown();
}

#[test]
fn daemon_persists_after_all_clients_detach() {
    // Even with zero attached clients, the daemon should remain alive and
    // pane processes should continue executing.
    let name = unique_name("persist-after-detach");
    let mut server = DaemonServer::start(&name).unwrap();
    let client = DaemonClient::connect(server.socket_path()).unwrap();
    let client_id = server.accept_client().unwrap();

    server
        .session_mut()
        .active_tab_mut()
        .split_pane(emux_mux::SplitDirection::Vertical);
    assert_eq!(server.session().active_tab().pane_count(), 2);

    client.detach();
    server.disconnect_client(client_id);
    assert_eq!(server.client_count(), 0);
    assert_eq!(server.session().active_tab().pane_count(), 2);
    assert!(server.socket_path().exists());
    server.shutdown();
}

#[test]
fn multiple_clients_attach_to_same_session() {
    // Two clients connected simultaneously should both see the same session.
    let name = unique_name("multi-client");
    let mut server = DaemonServer::start(&name).unwrap();

    let _client1 = DaemonClient::connect(server.socket_path()).unwrap();
    let _id1 = server.accept_client().unwrap();

    let _client2 = DaemonClient::connect(server.socket_path()).unwrap();
    let _id2 = server.accept_client().unwrap();

    assert_eq!(server.client_count(), 2);
    server.shutdown();
}

#[test]
fn client_attach_receives_viewport_sized_to_smallest() {
    // When multiple clients have different terminal sizes, the session
    // viewport should shrink to the smallest common size.
    let name = unique_name("viewport-smallest");
    let mut server = DaemonServer::start(&name).unwrap();

    let reply1 = server.handle_message(
        ClientId(1),
        ClientMessage::Resize {
            cols: 120,
            rows: 40,
        },
    );
    assert_eq!(reply1, ServerMessage::Ack);
    let reply2 = server.handle_message(ClientId(2), ClientMessage::Resize { cols: 80, rows: 24 });
    assert_eq!(reply2, ServerMessage::Ack);
    assert_eq!(server.session().size().cols, 80);
    assert_eq!(server.session().size().rows, 24);
    server.shutdown();
}

// ---------------------------------------------------------------------------
// 3. Ping / pong
// ---------------------------------------------------------------------------

#[test]
fn ping_pong() {
    let name = unique_name("ping");
    let mut server = DaemonServer::start(&name).unwrap();

    let mut client = DaemonClient::connect(server.socket_path()).unwrap();
    let client_id = server.accept_client().unwrap();

    // Client sends Ping.
    client.send(ClientMessage::Ping).unwrap();

    // Server reads it and produces a response.
    let msg = server.recv_from_client(client_id).unwrap();
    assert_eq!(msg, ClientMessage::Ping);

    let reply = server.handle_message(client_id, msg);
    assert_eq!(reply, ServerMessage::Pong);

    // Server sends the response back.
    server.send_to_client(client_id, &reply).unwrap();

    // Client receives Pong.
    let received = client.recv().unwrap();
    assert_eq!(received, ServerMessage::Pong);

    server.shutdown();
}

// ---------------------------------------------------------------------------
// 4. Snapshot / restore
// ---------------------------------------------------------------------------

#[test]
fn session_snapshot_save_load() {
    let name = unique_name("snapshot");
    let server = DaemonServer::start(&name).unwrap();

    let dir = temp_snapshot_dir();
    let snap_path = dir.join("session.json");

    persistence::save_session(server.session(), &snap_path).unwrap();
    assert!(snap_path.exists());

    let restored = persistence::load_session(&snap_path).unwrap();
    assert_eq!(restored.name(), server.session().name());
    assert_eq!(restored.tab_count(), server.session().tab_count());

    // Cleanup.
    server.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn session_snapshot_to_disk() {
    // Requesting a snapshot should serialize session metadata (tabs, pane
    // layout, CWDs) to a file on disk.
    let name = unique_name("snapshot-to-disk");
    let mut server = DaemonServer::start(&name).unwrap();
    server.session_mut().new_tab("Second Tab");
    assert_eq!(server.session().tab_count(), 2);

    let dir = temp_snapshot_dir();
    let snap_path = dir.join("session.json");
    persistence::save_session(server.session(), &snap_path).unwrap();
    assert!(snap_path.exists());

    // Verify file contents
    let contents = std::fs::read_to_string(&snap_path).unwrap();
    assert!(contents.contains(&name));

    server.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn session_restore_from_snapshot() {
    // Restoring from a snapshot file should recreate tabs and pane layout.
    let name = unique_name("snapshot-restore");
    let mut server = DaemonServer::start(&name).unwrap();
    server.session_mut().new_tab("Tab 2");
    server.session_mut().new_tab("Tab 3");

    let dir = temp_snapshot_dir();
    let snap_path = dir.join("session.json");
    persistence::save_session(server.session(), &snap_path).unwrap();

    let restored = persistence::load_session(&snap_path).unwrap();
    assert_eq!(restored.name(), name);
    assert_eq!(restored.tab_count(), 3);
    let names = restored.tab_names();
    assert_eq!(names[0], "Tab 1");
    assert_eq!(names[1], "Tab 2");
    assert_eq!(names[2], "Tab 3");

    server.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn snapshot_includes_scrollback() {
    // The snapshot should optionally include scrollback buffers so that
    // terminal history survives a full restart.
    let name = unique_name("scrollback");
    let mut server = DaemonServer::start(&name).unwrap();

    // Push some scrollback content into the active pane
    let tab = server.session_mut().active_tab_mut();
    let pane_id = tab.active_pane_id().unwrap();
    let pane = tab.pane_mut(pane_id).unwrap();
    pane.push_scrollback("line 1: hello world");
    pane.push_scrollback("line 2: cargo test");
    pane.push_scrollback("line 3: all passed");

    let dir = temp_snapshot_dir();
    let snap_path = dir.join("session.json");
    persistence::save_session(server.session(), &snap_path).unwrap();

    // Verify the JSON contains scrollback data
    let contents = std::fs::read_to_string(&snap_path).unwrap();
    assert!(contents.contains("hello world"));
    assert!(contents.contains("cargo test"));
    assert!(contents.contains("all passed"));

    // Verify the structured snapshot has scrollback
    let snap: persistence::SessionSnapshot = serde_json::from_str(&contents).unwrap();
    assert!(!snap.tabs.is_empty());
    let tab_snap = &snap.tabs[0];
    assert!(!tab_snap.panes.is_empty());
    let pane_snap = &tab_snap.panes[0];
    assert_eq!(pane_snap.scrollback.len(), 3);
    assert_eq!(pane_snap.scrollback[0], "line 1: hello world");

    server.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restore_missing_file_returns_error() {
    // Attempting to restore from a nonexistent snapshot path should return a
    // clear error.
    let result =
        persistence::load_session(std::path::Path::new("/tmp/nonexistent-emux-snapshot.json"));
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// 5. Session management
// ---------------------------------------------------------------------------

#[test]
fn list_sessions() {
    let dir = temp_snapshot_dir();

    // Save two session snapshots.
    let s1 = emux_mux::Session::new("alpha", 80, 24);
    let s2 = emux_mux::Session::new("beta", 120, 40);
    persistence::save_session(&s1, &dir.join("alpha.json")).unwrap();
    persistence::save_session(&s2, &dir.join("beta.json")).unwrap();

    let sessions = persistence::list_sessions(&dir);
    assert_eq!(sessions.len(), 2);

    let names: Vec<&str> = sessions.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"alpha"));
    assert!(names.contains(&"beta"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn kill_session_removes_socket() {
    let name = unique_name("kill");
    let server = DaemonServer::start(&name).unwrap();
    let path = server.socket_path().to_owned();
    assert!(path.exists());

    server.shutdown();
    assert!(!path.exists(), "socket should be removed after shutdown");
}

#[test]
fn session_rename() {
    let name = unique_name("rename-src");
    let mut server = DaemonServer::start(&name).unwrap();
    let old_path = server.socket_path().to_owned();

    let new_name = unique_name("rename-dst");
    server.rename_session(&new_name).unwrap();

    assert!(!old_path.exists(), "old socket should be gone");
    assert!(server.socket_path().exists(), "new socket should exist");
    assert_eq!(server.session().name(), new_name);

    server.shutdown();
}

#[test]
fn list_active_sessions() {
    // Listing sessions should return metadata for saved sessions.
    let dir = temp_snapshot_dir();
    let s1 = emux_mux::Session::new("session-alpha", 80, 24);
    let s2 = emux_mux::Session::new("session-beta", 120, 40);
    let s3 = emux_mux::Session::new("session-gamma", 100, 30);
    persistence::save_session(&s1, &dir.join("alpha.json")).unwrap();
    persistence::save_session(&s2, &dir.join("beta.json")).unwrap();
    persistence::save_session(&s3, &dir.join("gamma.json")).unwrap();

    let sessions = persistence::list_sessions(&dir);
    assert_eq!(sessions.len(), 3);
    let names: Vec<&str> = sessions.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"session-alpha"));
    assert!(names.contains(&"session-beta"));
    assert!(names.contains(&"session-gamma"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn kill_session() {
    // Killing a session should remove the socket file.
    let name = unique_name("kill-session");
    let server = DaemonServer::start(&name).unwrap();
    let path = server.socket_path().to_owned();
    assert!(path.exists());
    server.shutdown();
    assert!(!path.exists(), "socket should be removed after shutdown");
}

#[test]
fn stale_socket_cleanup() {
    // If a socket file exists but no process owns it (stale), the daemon
    // startup should remove it and proceed normally.
    let name = unique_name("stale-cleanup");
    // Create a stale socket by starting and shutting down without removing the socket
    let socket_path = std::env::temp_dir().join(format!("emux-test-{name}"));
    // Create a fake stale socket file
    std::fs::write(&socket_path, b"").unwrap();
    assert!(socket_path.exists());

    // Starting the daemon should clean up the stale socket and succeed.
    let server = DaemonServer::start(&name).unwrap();
    assert!(server.socket_path().exists());
    server.shutdown();
}

// ---------------------------------------------------------------------------
// 6. Data flow
// ---------------------------------------------------------------------------

#[test]
fn client_receives_render_updates() {
    // Verify the server can send render-like messages to a connected client.
    let name = unique_name("render-updates");
    let mut server = DaemonServer::start(&name).unwrap();

    let mut client = DaemonClient::connect(server.socket_path()).unwrap();
    let client_id = server.accept_client().unwrap();

    // Server sends an ack (simulating a render update notification).
    server
        .send_to_client(client_id, &ServerMessage::Ack)
        .unwrap();
    let msg = client.recv().unwrap();
    assert_eq!(msg, ServerMessage::Ack);

    server.shutdown();
}

#[test]
fn client_sends_key_input_to_daemon() {
    // Key input sent by a client should be handled by the daemon.
    let name = unique_name("key-input");
    let mut server = DaemonServer::start(&name).unwrap();

    let mut client = DaemonClient::connect(server.socket_path()).unwrap();
    let client_id = server.accept_client().unwrap();

    // Client sends key input.
    client
        .send(ClientMessage::KeyInput {
            data: vec![b'a', b'b', b'c'],
        })
        .unwrap();
    let msg = server.recv_from_client(client_id).unwrap();
    match msg {
        ClientMessage::KeyInput { ref data } => assert_eq!(data, &vec![b'a', b'b', b'c']),
        _ => panic!("expected KeyInput"),
    }
    let reply = server.handle_message(client_id, msg);
    assert_eq!(reply, ServerMessage::Ack);

    server.shutdown();
}

#[test]
fn client_reconnect_after_network_drop() {
    // A client that loses its connection should be able to reconnect and
    // receive the current state without restarting the session.
    let name = unique_name("reconnect");
    let mut server = DaemonServer::start(&name).unwrap();

    // First connection
    let client1 = DaemonClient::connect(server.socket_path()).unwrap();
    let id1 = server.accept_client().unwrap();
    assert_eq!(server.client_count(), 1);

    // Simulate network drop
    drop(client1);
    server.disconnect_client(id1);
    assert_eq!(server.client_count(), 0);

    // Session still alive
    assert_eq!(server.session().tab_count(), 1);

    // Reconnect
    let _client2 = DaemonClient::connect(server.socket_path()).unwrap();
    let _id2 = server.accept_client().unwrap();
    assert_eq!(server.client_count(), 1);

    server.shutdown();
}

#[test]
fn daemon_shutdown_graceful() {
    // A graceful shutdown should remove the socket.
    let name = unique_name("graceful-shutdown");
    let mut server = DaemonServer::start(&name).unwrap();

    // Connect a client
    let _client = DaemonClient::connect(server.socket_path()).unwrap();
    let _id = server.accept_client().unwrap();

    let path = server.socket_path().to_owned();
    assert!(path.exists());

    server.shutdown();
    assert!(!path.exists());
}

// ---------------------------------------------------------------------------
// 7. Enhanced persistence: roundtrip with new snapshot fields
// ---------------------------------------------------------------------------

#[test]
fn snapshot_preserves_scroll_offset() {
    let name = unique_name("scroll-offset");
    let mut server = DaemonServer::start(&name).unwrap();

    // Set scroll offset on a pane.
    let tab = server.session_mut().active_tab_mut();
    let pane_id = tab.active_pane_id().unwrap();
    tab.scroll_up(pane_id, 42);

    let dir = temp_snapshot_dir();
    let snap_path = dir.join("session.json");
    persistence::save_session(server.session(), &snap_path).unwrap();

    let snap = persistence::load_snapshot(&snap_path).unwrap();
    assert_eq!(snap.tabs[0].panes[0].scroll_offset, 42);

    server.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn snapshot_preserves_active_tab_index() {
    let name = unique_name("active-tab-idx");
    let mut server = DaemonServer::start(&name).unwrap();
    server.session_mut().new_tab("Second");
    server.session_mut().new_tab("Third");
    // Active tab should now be 2 (the last one created).
    assert_eq!(server.session().active_tab_index(), 2);

    let dir = temp_snapshot_dir();
    let snap_path = dir.join("session.json");
    persistence::save_session(server.session(), &snap_path).unwrap();

    let snap = persistence::load_snapshot(&snap_path).unwrap();
    assert_eq!(snap.active_tab_index, 2);

    // Round-trip: the restored session should have the same active tab.
    let restored = persistence::load_session(&snap_path).unwrap();
    assert_eq!(restored.active_tab_index(), 2);

    server.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn snapshot_preserves_active_pane_id() {
    let name = unique_name("active-pane");
    let mut server = DaemonServer::start(&name).unwrap();

    // Split to create a second pane; new pane becomes active.
    let new_id = server
        .session_mut()
        .active_tab_mut()
        .split_pane(emux_mux::SplitDirection::Vertical)
        .unwrap();

    let dir = temp_snapshot_dir();
    let snap_path = dir.join("session.json");
    persistence::save_session(server.session(), &snap_path).unwrap();

    let snap = persistence::load_snapshot(&snap_path).unwrap();
    assert_eq!(snap.tabs[0].active_pane_id, Some(new_id));

    server.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn snapshot_preserves_working_directory_field() {
    // The working_directory field should round-trip through JSON even if None.
    let snap = persistence::PaneSnapshot {
        id: 0,
        title: "test".into(),
        scrollback: vec![],
        working_directory: Some("/home/user".into()),
        scroll_offset: 0,
    };
    let json = serde_json::to_string(&snap).unwrap();
    let restored: persistence::PaneSnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.working_directory, Some("/home/user".into()));

    // None case
    let snap_none = persistence::PaneSnapshot {
        id: 1,
        title: "t2".into(),
        scrollback: vec![],
        working_directory: None,
        scroll_offset: 5,
    };
    let json_none = serde_json::to_string(&snap_none).unwrap();
    let restored_none: persistence::PaneSnapshot = serde_json::from_str(&json_none).unwrap();
    assert!(restored_none.working_directory.is_none());
    assert_eq!(restored_none.scroll_offset, 5);
}

#[test]
fn crash_recovery_restores_session() {
    // Simulate crash recovery: save, then start a new daemon that picks up
    // the saved state.
    let dir = temp_snapshot_dir();
    let snap_path = dir.join("crash-test.json");

    // Create a session with 3 tabs.
    let mut session = emux_mux::Session::new("crash-test", 100, 30);
    session.new_tab("work");
    session.new_tab("logs");
    persistence::save_session(&session, &snap_path).unwrap();

    // "Crash" -- just drop the session.
    drop(session);

    // Restore from file.
    let restored = persistence::load_session(&snap_path).unwrap();
    assert_eq!(restored.name(), "crash-test");
    assert_eq!(restored.tab_count(), 3);
    assert_eq!(restored.size().cols, 100);
    assert_eq!(restored.size().rows, 30);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_creates_parent_directories() {
    let dir = std::env::temp_dir()
        .join(format!("emux-nested-{}", unique_name("nested")))
        .join("deep")
        .join("path");
    let snap_path = dir.join("session.json");

    let session = emux_mux::Session::new("nested", 80, 24);
    persistence::save_session(&session, &snap_path).unwrap();
    assert!(snap_path.exists());

    let _ = std::fs::remove_dir_all(dir.parent().unwrap().parent().unwrap().parent().unwrap());
}

#[test]
fn auto_save_marks_dirty_on_state_change() {
    let name = unique_name("dirty");
    let dir = temp_snapshot_dir();
    let snap_path = dir.join("dirty.json");

    let mut server =
        DaemonServer::start_with_snapshot_path(&name, Some(snap_path.clone())).unwrap();

    // Initially not dirty -- auto-save should be a no-op.
    assert!(!server.maybe_auto_save());

    // Trigger a state change.
    server.mark_dirty();

    // auto-save won't fire yet because the interval hasn't elapsed,
    // but save_now should work.
    server.save_now().unwrap();
    assert!(snap_path.exists());

    let restored = persistence::load_session(&snap_path).unwrap();
    assert_eq!(restored.name(), name);

    server.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn atomic_save_does_not_leave_tmp_file() {
    let dir = temp_snapshot_dir();
    let snap_path = dir.join("atomic.json");

    let session = emux_mux::Session::new("atomic", 80, 24);
    persistence::save_session(&session, &snap_path).unwrap();

    // The .tmp file should have been renamed away.
    let tmp_path = snap_path.with_extension("json.tmp");
    assert!(!tmp_path.exists());
    assert!(snap_path.exists());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sessions_dir_helper_returns_expected_path() {
    // sessions_dir should return ~/.local/share/emux/sessions/
    if let Some(dir) = persistence::sessions_dir() {
        assert!(dir.ends_with("sessions"));
        assert!(dir.to_string_lossy().contains("emux"));
    }
}

// ---------------------------------------------------------------------------
// 8. Session sharing — multiple clients on the same session
// ---------------------------------------------------------------------------

#[test]
fn session_sharing_multiple_clients_see_same_state() {
    // Two clients attached to the same session should both see state changes
    // made through either client's messages.
    let name = unique_name("sharing-state");
    let mut server = DaemonServer::start(&name).unwrap();

    let _client1 = DaemonClient::connect(server.socket_path()).unwrap();
    let id1 = server.accept_client().unwrap();

    let _client2 = DaemonClient::connect(server.socket_path()).unwrap();
    let id2 = server.accept_client().unwrap();

    assert_eq!(server.client_count(), 2);

    // Client 1 requests a pane split.
    let reply = server.handle_message(
        id1,
        ClientMessage::SpawnPane {
            direction: Some("vertical".into()),
        },
    );
    assert!(matches!(reply, ServerMessage::SpawnResult { .. }));
    assert_eq!(server.session().active_tab().pane_count(), 2);

    // Client 2 queries sessions — it should see the updated pane count.
    let reply2 = server.handle_message(id2, ClientMessage::ListSessions);
    match reply2 {
        ServerMessage::SessionList { sessions } => {
            assert_eq!(sessions[0].panes, 2);
        }
        _ => panic!("expected SessionList"),
    }

    server.shutdown();
}

#[test]
fn session_sharing_broadcast_to_all_clients() {
    // The broadcast_to_all_clients method should deliver a message to every
    // connected client.
    let name = unique_name("sharing-broadcast");
    let mut server = DaemonServer::start(&name).unwrap();

    let mut client1 = DaemonClient::connect(server.socket_path()).unwrap();
    let _id1 = server.accept_client().unwrap();

    let mut client2 = DaemonClient::connect(server.socket_path()).unwrap();
    let _id2 = server.accept_client().unwrap();

    // Broadcast an Ack to all clients.
    let failed = server.broadcast_to_all_clients(&ServerMessage::Ack);
    assert!(failed.is_empty(), "no clients should fail");

    // Both clients should receive the message.
    let msg1 = client1.recv().unwrap();
    assert_eq!(msg1, ServerMessage::Ack);

    let msg2 = client2.recv().unwrap();
    assert_eq!(msg2, ServerMessage::Ack);

    server.shutdown();
}

#[test]
fn session_sharing_input_from_any_client_is_handled() {
    // Input from any client should be processed by the daemon (all clients
    // share the same session, so key input from any source is valid).
    let name = unique_name("sharing-input");
    let mut server = DaemonServer::start(&name).unwrap();

    let mut client1 = DaemonClient::connect(server.socket_path()).unwrap();
    let id1 = server.accept_client().unwrap();

    let mut client2 = DaemonClient::connect(server.socket_path()).unwrap();
    let id2 = server.accept_client().unwrap();

    // Client 1 sends key input.
    client1
        .send(ClientMessage::KeyInput { data: vec![b'x'] })
        .unwrap();
    let msg = server.recv_from_client(id1).unwrap();
    let reply = server.handle_message(id1, msg);
    assert_eq!(reply, ServerMessage::Ack);

    // Client 2 sends key input.
    client2
        .send(ClientMessage::KeyInput { data: vec![b'y'] })
        .unwrap();
    let msg = server.recv_from_client(id2).unwrap();
    let reply = server.handle_message(id2, msg);
    assert_eq!(reply, ServerMessage::Ack);

    server.shutdown();
}

#[test]
fn session_sharing_client_ids_returns_all_connected() {
    let name = unique_name("sharing-ids");
    let mut server = DaemonServer::start(&name).unwrap();

    assert!(server.client_ids().is_empty());

    let _c1 = DaemonClient::connect(server.socket_path()).unwrap();
    let id1 = server.accept_client().unwrap();
    assert_eq!(server.client_ids().len(), 1);
    assert_eq!(server.client_ids()[0], id1);

    let _c2 = DaemonClient::connect(server.socket_path()).unwrap();
    let id2 = server.accept_client().unwrap();
    assert_eq!(server.client_ids().len(), 2);
    assert!(server.client_ids().contains(&id1));
    assert!(server.client_ids().contains(&id2));

    server.disconnect_client(id1);
    assert_eq!(server.client_ids().len(), 1);
    assert_eq!(server.client_ids()[0], id2);

    server.shutdown();
}

#[test]
fn session_sharing_disconnect_one_client_keeps_others() {
    // Disconnecting one shared client should not affect other clients.
    let name = unique_name("sharing-disconnect");
    let mut server = DaemonServer::start(&name).unwrap();

    let _c1 = DaemonClient::connect(server.socket_path()).unwrap();
    let id1 = server.accept_client().unwrap();

    let mut c2 = DaemonClient::connect(server.socket_path()).unwrap();
    let id2 = server.accept_client().unwrap();

    // Disconnect client 1.
    server.disconnect_client(id1);
    assert_eq!(server.client_count(), 1);

    // Client 2 should still be able to communicate.
    c2.send(ClientMessage::Ping).unwrap();
    let msg = server.recv_from_client(id2).unwrap();
    assert_eq!(msg, ClientMessage::Ping);
    let reply = server.handle_message(id2, msg);
    server.send_to_client(id2, &reply).unwrap();
    let recv = c2.recv().unwrap();
    assert_eq!(recv, ServerMessage::Pong);

    server.shutdown();
}

// ---------------------------------------------------------------------------
// Agent / AI IPC commands
// ---------------------------------------------------------------------------

#[test]
fn agent_list_panes_returns_initial_pane() {
    let name = unique_name("agent-list-panes");
    let mut server = DaemonServer::start(&name).unwrap();
    let id = ClientId(0);
    let resp = server.handle_message(id, ClientMessage::ListPanes);
    match resp {
        ServerMessage::PaneList { panes } => {
            assert_eq!(panes.len(), 1, "should have one initial pane");
            assert!(panes[0].active, "initial pane should be active");
            assert!(panes[0].cols > 0);
            assert!(panes[0].rows > 0);
        }
        other => panic!("expected PaneList, got {:?}", other),
    }
    server.shutdown();
}

#[test]
fn agent_split_pane_creates_new_pane() {
    let name = unique_name("agent-split");
    let mut server = DaemonServer::start(&name).unwrap();
    let id = ClientId(0);

    let resp = server.handle_message(
        id,
        ClientMessage::SplitPane {
            direction: emux_ipc::SplitDirection::Vertical,
            size: None,
        },
    );
    let new_id = match resp {
        ServerMessage::SpawnResult { pane_id } => pane_id,
        other => panic!("expected SpawnResult, got {:?}", other),
    };

    // ListPanes should now show 2 panes.
    let resp = server.handle_message(id, ClientMessage::ListPanes);
    match resp {
        ServerMessage::PaneList { panes } => {
            assert_eq!(panes.len(), 2);
            assert!(panes.iter().any(|p| p.id == new_id));
        }
        other => panic!("expected PaneList, got {:?}", other),
    }
    server.shutdown();
}

#[test]
fn agent_get_pane_info_returns_details() {
    let name = unique_name("agent-getinfo");
    let mut server = DaemonServer::start(&name).unwrap();
    let id = ClientId(0);

    let pane_id = match server.handle_message(id, ClientMessage::ListPanes) {
        ServerMessage::PaneList { panes } => panes[0].id,
        _ => panic!("expected PaneList"),
    };

    let resp = server.handle_message(id, ClientMessage::GetPaneInfo { pane_id });
    match resp {
        ServerMessage::PaneInfo { pane } => {
            assert_eq!(pane.id, pane_id);
            assert!(pane.active);
        }
        other => panic!("expected PaneInfo, got {:?}", other),
    }

    // Non-existent pane should return error.
    let resp = server.handle_message(id, ClientMessage::GetPaneInfo { pane_id: 9999 });
    assert!(matches!(resp, ServerMessage::Error { .. }));

    server.shutdown();
}

#[test]
fn agent_set_pane_title_updates_title() {
    let name = unique_name("agent-title");
    let mut server = DaemonServer::start(&name).unwrap();
    let id = ClientId(0);

    let pane_id = match server.handle_message(id, ClientMessage::ListPanes) {
        ServerMessage::PaneList { panes } => panes[0].id,
        _ => panic!("expected PaneList"),
    };

    let resp = server.handle_message(
        id,
        ClientMessage::SetPaneTitle {
            pane_id,
            title: "my-agent".into(),
        },
    );
    assert_eq!(resp, ServerMessage::Ack);

    // Verify title changed.
    let resp = server.handle_message(id, ClientMessage::GetPaneInfo { pane_id });
    match resp {
        ServerMessage::PaneInfo { pane } => {
            assert_eq!(pane.title, "my-agent");
        }
        other => panic!("expected PaneInfo, got {:?}", other),
    }
    server.shutdown();
}

#[test]
fn agent_capture_pane_returns_content() {
    let name = unique_name("agent-capture");
    let mut server = DaemonServer::start(&name).unwrap();
    let id = ClientId(0);

    let pane_id = match server.handle_message(id, ClientMessage::ListPanes) {
        ServerMessage::PaneList { panes } => panes[0].id,
        _ => panic!("expected PaneList"),
    };

    let resp = server.handle_message(id, ClientMessage::CapturePane { pane_id });
    match resp {
        ServerMessage::PaneCaptured {
            pane_id: pid,
            content,
        } => {
            assert_eq!(pid, pane_id);
            assert!(
                !content.is_empty(),
                "capture should return non-empty content"
            );
        }
        other => panic!("expected PaneCaptured, got {:?}", other),
    }

    // Non-existent pane should error.
    let resp = server.handle_message(id, ClientMessage::CapturePane { pane_id: 9999 });
    assert!(matches!(resp, ServerMessage::Error { .. }));

    server.shutdown();
}

#[test]
fn agent_send_keys_to_valid_pane_acks() {
    let name = unique_name("agent-sendkeys");
    let mut server = DaemonServer::start(&name).unwrap();
    let id = ClientId(0);

    let pane_id = match server.handle_message(id, ClientMessage::ListPanes) {
        ServerMessage::PaneList { panes } => panes[0].id,
        _ => panic!("expected PaneList"),
    };

    let resp = server.handle_message(
        id,
        ClientMessage::SendKeys {
            pane_id,
            keys: "echo hello\n".into(),
        },
    );
    assert_eq!(resp, ServerMessage::Ack);

    // Non-existent pane should error.
    let resp = server.handle_message(
        id,
        ClientMessage::SendKeys {
            pane_id: 9999,
            keys: "test".into(),
        },
    );
    assert!(matches!(resp, ServerMessage::Error { .. }));

    server.shutdown();
}

#[test]
fn agent_resize_pane_adjusts_layout() {
    let name = unique_name("agent-resize");
    let mut server = DaemonServer::start(&name).unwrap();
    let id = ClientId(0);

    // Split first so we have a pane to resize.
    server.handle_message(
        id,
        ClientMessage::SplitPane {
            direction: emux_ipc::SplitDirection::Vertical,
            size: None,
        },
    );

    let pane_id = match server.handle_message(id, ClientMessage::ListPanes) {
        ServerMessage::PaneList { panes } => panes[0].id,
        _ => panic!("expected PaneList"),
    };

    let resp = server.handle_message(
        id,
        ClientMessage::ResizePane {
            pane_id,
            cols: 60,
            rows: 20,
        },
    );
    assert_eq!(resp, ServerMessage::Ack);

    // Non-existent pane should error.
    let resp = server.handle_message(
        id,
        ClientMessage::ResizePane {
            pane_id: 9999,
            cols: 10,
            rows: 10,
        },
    );
    assert!(matches!(resp, ServerMessage::Error { .. }));

    server.shutdown();
}

#[test]
fn agent_set_title_nonexistent_pane_errors() {
    let name = unique_name("agent-title-err");
    let mut server = DaemonServer::start(&name).unwrap();
    let id = ClientId(0);
    let resp = server.handle_message(
        id,
        ClientMessage::SetPaneTitle {
            pane_id: 9999,
            title: "nope".into(),
        },
    );
    assert!(matches!(resp, ServerMessage::Error { .. }));
    server.shutdown();
}

// ---------------------------------------------------------------------------
// E2E: Agent sends command via PTY, reads output via CapturePane
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn agent_e2e_send_keys_and_capture_output() {
    // Full AI agent flow:
    // 1. Spawn a PTY for the initial pane
    // 2. SendKeys "echo agent_test_marker\n"
    // 3. CapturePane and verify the marker appears
    let name = unique_name("agent-e2e");
    let mut server = DaemonServer::start(&name).unwrap();
    let id = ClientId(0);

    let pane_id = match server.handle_message(id, ClientMessage::ListPanes) {
        ServerMessage::PaneList { panes } => panes[0].id,
        other => panic!("expected PaneList, got {:?}", other),
    };

    // Spawn a real PTY for this pane.
    server.spawn_terminal_for_pane(pane_id).unwrap();

    // Wait for shell to initialize.
    std::thread::sleep(std::time::Duration::from_millis(500));
    server.poll_pty_output();

    // Send a command.
    let resp = server.handle_message(
        id,
        ClientMessage::SendKeys {
            pane_id,
            keys: "echo agent_test_marker_42\n".into(),
        },
    );
    assert_eq!(resp, ServerMessage::Ack);

    // Wait for the command to execute.
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Capture the pane content.
    let resp = server.handle_message(id, ClientMessage::CapturePane { pane_id });
    match resp {
        ServerMessage::PaneCaptured { content, .. } => {
            assert!(
                content.contains("agent_test_marker_42"),
                "captured content should contain the echoed marker.\nActual content:\n{}",
                content
            );
        }
        other => panic!("expected PaneCaptured, got {:?}", other),
    }

    server.shutdown();
}

#[cfg(unix)]
#[test]
fn agent_e2e_split_and_capture_new_pane() {
    // SplitPane via IPC creates a new pane with its own PTY.
    let name = unique_name("agent-e2e-split");
    let mut server = DaemonServer::start(&name).unwrap();
    let id = ClientId(0);

    let new_pane_id = match server.handle_message(
        id,
        ClientMessage::SplitPane {
            direction: emux_ipc::SplitDirection::Vertical,
            size: None,
        },
    ) {
        ServerMessage::SpawnResult { pane_id } => pane_id,
        other => panic!("expected SpawnResult, got {:?}", other),
    };

    // Wait for the new shell to start.
    std::thread::sleep(std::time::Duration::from_millis(500));
    server.poll_pty_output();

    // Send a command to the new pane.
    let resp = server.handle_message(
        id,
        ClientMessage::SendKeys {
            pane_id: new_pane_id,
            keys: "echo split_pane_works\n".into(),
        },
    );
    assert_eq!(resp, ServerMessage::Ack);

    std::thread::sleep(std::time::Duration::from_millis(500));

    let resp = server.handle_message(
        id,
        ClientMessage::CapturePane {
            pane_id: new_pane_id,
        },
    );
    match resp {
        ServerMessage::PaneCaptured { content, .. } => {
            assert!(
                content.contains("split_pane_works"),
                "new pane should contain echoed text.\nActual:\n{}",
                content
            );
        }
        other => panic!("expected PaneCaptured, got {:?}", other),
    }

    server.shutdown();
}
