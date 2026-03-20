//! End-to-end tests simulating an AI agent controlling emux via IPC.
//!
//! These tests spawn real PTYs, send real shell commands, and verify
//! that CapturePane returns the actual terminal output. This is the
//! proof that AI agents can autonomously develop through emux.

#[cfg(unix)]
mod agent_tests {
    use std::time::Duration;

    use emux_daemon::ClientId;
    use emux_daemon::server::DaemonServer;
    use emux_ipc::{ClientMessage, ServerMessage};

    fn unique_name(base: &str) -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::SystemTime;
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let t = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let c = COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("{base}-{t}-{c}")
    }

    /// Helper: create a server with a real PTY on the initial pane.
    fn server_with_pty() -> (DaemonServer, u32) {
        let name = unique_name("agent-e2e");
        let mut server = DaemonServer::start(&name).unwrap();
        let id = ClientId(0);
        let pane_id = match server.handle_message(id, ClientMessage::ListPanes) {
            ServerMessage::PaneList { panes } => panes[0].id,
            other => panic!("expected PaneList, got {:?}", other),
        };
        server.spawn_terminal_for_pane(pane_id).unwrap();
        // Wait for shell to fully initialize (prompt ready).
        std::thread::sleep(Duration::from_millis(800));
        server.poll_pty_output();
        (server, pane_id)
    }

    /// Helper: wait for a marker string to appear in the pane's captured output.
    /// Polls up to `max_wait` with 100ms intervals.
    fn wait_for_output(
        server: &mut DaemonServer,
        pane_id: u32,
        marker: &str,
        max_wait: Duration,
    ) -> String {
        let id = ClientId(0);
        let start = std::time::Instant::now();
        loop {
            server.poll_pty_output();
            let resp = server.handle_message(id, ClientMessage::CapturePane { pane_id });
            let content = match resp {
                ServerMessage::PaneCaptured { content, .. } => content,
                _ => String::new(),
            };
            if content.contains(marker) {
                return content;
            }
            if start.elapsed() >= max_wait {
                return content;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    // ---------------------------------------------------------------
    // 1. Basic: echo and capture
    // ---------------------------------------------------------------

    #[test]
    fn echo_and_capture() {
        let (mut server, pane_id) = server_with_pty();
        let id = ClientId(0);

        server.handle_message(
            id,
            ClientMessage::SendKeys {
                pane_id,
                keys: "echo HELLO_EMUX\n".into(),
            },
        );

        let content = wait_for_output(&mut server, pane_id, "HELLO_EMUX", Duration::from_secs(3));
        assert!(
            content.contains("HELLO_EMUX"),
            "should see echoed text. Got:\n{}",
            content
        );

        server.shutdown();
    }

    // ---------------------------------------------------------------
    // 2. Multiple commands in sequence
    // ---------------------------------------------------------------

    #[test]
    fn sequential_commands() {
        let (mut server, pane_id) = server_with_pty();
        let id = ClientId(0);

        for i in 0..3 {
            server.handle_message(
                id,
                ClientMessage::SendKeys {
                    pane_id,
                    keys: format!("echo SEQ_{i}\n"),
                },
            );
            std::thread::sleep(Duration::from_millis(200));
        }

        let content = wait_for_output(&mut server, pane_id, "SEQ_2", Duration::from_secs(3));
        assert!(content.contains("SEQ_0"), "missing SEQ_0\n{}", content);
        assert!(content.contains("SEQ_1"), "missing SEQ_1\n{}", content);
        assert!(content.contains("SEQ_2"), "missing SEQ_2\n{}", content);

        server.shutdown();
    }

    // ---------------------------------------------------------------
    // 3. File creation: AI creates a file and verifies it
    // ---------------------------------------------------------------

    #[test]
    fn create_file_and_read_back() {
        let (mut server, pane_id) = server_with_pty();
        let id = ClientId(0);

        let tmp_file = format!("/tmp/emux-agent-test-{}.txt", unique_name("file"));

        // Step 1: Create the file.
        server.handle_message(
            id,
            ClientMessage::SendKeys {
                pane_id,
                keys: format!("echo 'AI_WAS_HERE' > {tmp_file}\n"),
            },
        );
        std::thread::sleep(Duration::from_millis(500));
        server.poll_pty_output();

        // Step 2: Read it back via cat, using a unique output marker that
        // cannot appear from input echo alone.
        server.handle_message(
            id,
            ClientMessage::SendKeys {
                pane_id,
                keys: format!("cat {tmp_file} && echo READ_BACK_DONE_42\n"),
            },
        );

        // Wait for the cat output — this proves the file was created AND read.
        let content = wait_for_output(
            &mut server,
            pane_id,
            "READ_BACK_DONE_42",
            Duration::from_secs(10),
        );
        assert!(
            content.contains("AI_WAS_HERE"),
            "cat should show file content. Got:\n{}",
            content
        );

        let _ = std::fs::remove_file(&tmp_file);
        server.shutdown();
    }

    // ---------------------------------------------------------------
    // 4. Multi-pane: split, run different commands in each pane
    // ---------------------------------------------------------------

    #[test]
    fn multi_pane_independent_commands() {
        let (mut server, pane0) = server_with_pty();
        let id = ClientId(0);

        // Split to create pane 1 (SplitPane auto-spawns PTY)
        let pane1 = match server.handle_message(
            id,
            ClientMessage::SplitPane {
                direction: emux_ipc::SplitDirection::Vertical,
                size: None,
            },
        ) {
            ServerMessage::SpawnResult { pane_id } => pane_id,
            other => panic!("expected SpawnResult, got {:?}", other),
        };

        std::thread::sleep(Duration::from_millis(500));
        server.poll_pty_output();

        // Send different commands to each pane
        server.handle_message(
            id,
            ClientMessage::SendKeys {
                pane_id: pane0,
                keys: "echo PANE_ZERO\n".into(),
            },
        );
        server.handle_message(
            id,
            ClientMessage::SendKeys {
                pane_id: pane1,
                keys: "echo PANE_ONE\n".into(),
            },
        );

        // Wait for both to complete
        let cap0 = wait_for_output(&mut server, pane0, "PANE_ZERO", Duration::from_secs(3));
        let cap1 = wait_for_output(&mut server, pane1, "PANE_ONE", Duration::from_secs(3));

        assert!(
            cap0.contains("PANE_ZERO"),
            "pane 0 should have PANE_ZERO. Got:\n{}",
            cap0
        );
        assert!(
            cap1.contains("PANE_ONE"),
            "pane 1 should have PANE_ONE. Got:\n{}",
            cap1
        );

        // Each pane should NOT have the other's output
        assert!(
            !cap0.contains("PANE_ONE"),
            "pane 0 should NOT have PANE_ONE"
        );
        assert!(
            !cap1.contains("PANE_ZERO"),
            "pane 1 should NOT have PANE_ZERO"
        );

        server.shutdown();
    }

    // ---------------------------------------------------------------
    // 5. SetTitle + GetPaneInfo roundtrip
    // ---------------------------------------------------------------

    #[test]
    fn set_title_and_verify() {
        let (mut server, pane_id) = server_with_pty();
        let id = ClientId(0);

        server.handle_message(
            id,
            ClientMessage::SetPaneTitle {
                pane_id,
                title: "claude-agent".into(),
            },
        );

        let resp = server.handle_message(id, ClientMessage::GetPaneInfo { pane_id });
        match resp {
            ServerMessage::PaneInfo { pane } => {
                assert_eq!(pane.title, "claude-agent");
                assert!(pane.active);
                assert!(pane.cols > 0);
                assert!(pane.rows > 0);
            }
            other => panic!("expected PaneInfo, got {:?}", other),
        }

        server.shutdown();
    }

    // ---------------------------------------------------------------
    // 6. Simulate AI dev loop: run cargo, check result, react
    // ---------------------------------------------------------------

    #[test]
    fn ai_dev_loop_simulation() {
        let (mut server, pane_id) = server_with_pty();
        let id = ClientId(0);

        // Step 1: AI runs a command and checks exit status marker
        server.handle_message(
            id,
            ClientMessage::SendKeys {
                pane_id,
                keys: "true && echo BUILD_OK || echo BUILD_FAIL\n".into(),
            },
        );

        let content = wait_for_output(&mut server, pane_id, "BUILD_OK", Duration::from_secs(3));
        assert!(
            content.contains("BUILD_OK"),
            "should see BUILD_OK. Got:\n{}",
            content
        );

        // Step 2: AI detects failure
        server.handle_message(
            id,
            ClientMessage::SendKeys {
                pane_id,
                keys: "false && echo STEP2_OK || echo STEP2_FAIL\n".into(),
            },
        );

        let content = wait_for_output(&mut server, pane_id, "STEP2_FAIL", Duration::from_secs(3));
        assert!(
            content.contains("STEP2_FAIL"),
            "should detect failure. Got:\n{}",
            content
        );

        server.shutdown();
    }

    // ---------------------------------------------------------------
    // 7. Stress: large output capture
    // ---------------------------------------------------------------

    #[test]
    fn large_output_capture() {
        let (mut server, pane_id) = server_with_pty();
        let id = ClientId(0);

        // Generate 200 lines of output.
        server.handle_message(
            id,
            ClientMessage::SendKeys {
                pane_id,
                keys: "seq 1 200 && echo LARGE_DONE\n".into(),
            },
        );

        let content = wait_for_output(&mut server, pane_id, "LARGE_DONE", Duration::from_secs(10));
        assert!(
            content.contains("LARGE_DONE"),
            "should survive large output. Got {} bytes",
            content.len()
        );

        server.shutdown();
    }

    // ---------------------------------------------------------------
    // 8. PTY exit detection
    // ---------------------------------------------------------------

    #[test]
    fn pty_exit_detection() {
        let (mut server, pane_id) = server_with_pty();
        let id = ClientId(0);

        // Send exit to the shell.
        server.handle_message(
            id,
            ClientMessage::SendKeys {
                pane_id,
                keys: "exit\n".into(),
            },
        );

        // Wait for the process to exit (retry up to 3 seconds).
        let start = std::time::Instant::now();
        let mut exited = false;
        while start.elapsed() < Duration::from_secs(3) {
            // Drain output to detect EOF.
            server.poll_pty_output();
            if let Some(pt) = server.pane_terminals.get(&pane_id) {
                if !pt.pty.is_alive() {
                    exited = true;
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        assert!(exited, "PTY should not be alive after shell exit");

        server.shutdown();
    }

    // ---------------------------------------------------------------
    // 9. Concurrent agents on different panes
    // ---------------------------------------------------------------

    #[test]
    fn concurrent_agents_different_panes() {
        let (mut server, pane0) = server_with_pty();
        let id = ClientId(0);

        // Create a second pane.
        let pane1 = match server.handle_message(
            id,
            ClientMessage::SplitPane {
                direction: emux_ipc::SplitDirection::Horizontal,
                size: None,
            },
        ) {
            ServerMessage::SpawnResult { pane_id } => pane_id,
            other => panic!("expected SpawnResult, got {:?}", other),
        };

        std::thread::sleep(Duration::from_millis(500));
        server.poll_pty_output();

        // "Agent A" sends to pane0, "Agent B" sends to pane1 — interleaved.
        server.handle_message(
            id,
            ClientMessage::SendKeys {
                pane_id: pane0,
                keys: "echo AGENT_A_1\n".into(),
            },
        );
        server.handle_message(
            id,
            ClientMessage::SendKeys {
                pane_id: pane1,
                keys: "echo AGENT_B_1\n".into(),
            },
        );
        server.handle_message(
            id,
            ClientMessage::SendKeys {
                pane_id: pane0,
                keys: "echo AGENT_A_2\n".into(),
            },
        );
        server.handle_message(
            id,
            ClientMessage::SendKeys {
                pane_id: pane1,
                keys: "echo AGENT_B_2\n".into(),
            },
        );

        let cap0 = wait_for_output(&mut server, pane0, "AGENT_A_2", Duration::from_secs(3));
        let cap1 = wait_for_output(&mut server, pane1, "AGENT_B_2", Duration::from_secs(3));

        assert!(cap0.contains("AGENT_A_1") && cap0.contains("AGENT_A_2"));
        assert!(cap1.contains("AGENT_B_1") && cap1.contains("AGENT_B_2"));

        // No cross-contamination.
        assert!(!cap0.contains("AGENT_B"));
        assert!(!cap1.contains("AGENT_A"));

        server.shutdown();
    }
}
