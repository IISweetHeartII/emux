//! E2E tests: spawn the real `emux` binary in a PTY, connect to its
//! agent IPC socket, and verify that AI agents can control it.
//!
//! These tests prove that the full stack works end-to-end:
//! binary → event loop → agent socket → PTY → Screen → CapturePane.

#[cfg(unix)]
mod agent_ipc_tests {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    use emux_ipc::{ClientMessage, ServerMessage};
    use emux_pty::{CommandBuilder, PtySize, UnixPty};

    static PTY_LOCK: Mutex<()> = Mutex::new(());

    fn set_nonblocking(fd: i32) {
        unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFL);
            libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
        }
    }

    /// RAII guard that cleans up the emux process and daemon on drop.
    struct EmuxTestGuard {
        session_name: String,
        running: std::sync::Arc<std::sync::atomic::AtomicBool>,
        child_pid: i32,
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl Drop for EmuxTestGuard {
        fn drop(&mut self) {
            // 1. Stop the drain thread.
            self.running
                .store(false, std::sync::atomic::Ordering::Relaxed);

            // 2. Kill the emux TUI process (which also owns the PTY).
            unsafe {
                libc::kill(self.child_pid, libc::SIGTERM);
            }
            std::thread::sleep(Duration::from_millis(100));
            unsafe {
                libc::kill(self.child_pid, libc::SIGKILL);
            }

            // 3. Kill the daemon that `emux new` forked.
            let bin = env!("CARGO_BIN_EXE_emux");
            let _ = std::process::Command::new(bin)
                .args(["kill", &self.session_name])
                .output();
            std::thread::sleep(Duration::from_millis(200));

            // 4. Remove stale socket files.
            let sock_dir = std::env::temp_dir().join("emux-sockets");
            let _ = std::fs::remove_file(sock_dir.join(format!("emux-{}.sock", self.session_name)));
            let _ = std::fs::remove_file(
                sock_dir.join(format!("emux-agent-{}.sock", self.session_name)),
            );
            let _ = std::fs::remove_file(
                std::env::temp_dir().join(format!("emux-test-{}", self.session_name)),
            );
        }
    }

    /// Spawn emux in a real PTY. Returns a guard that cleans up on drop.
    fn spawn_emux_with_session(session_name: &str) -> EmuxTestGuard {
        let lock = PTY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::thread::sleep(Duration::from_millis(100));

        let bin = env!("CARGO_BIN_EXE_emux");
        let mut cmd = CommandBuilder::new(bin);
        cmd.arg("new");
        cmd.arg(session_name);
        cmd.env("TERM", "xterm-256color");
        cmd.env("EMUX_CONFIG", "/dev/null");
        cmd.env("EMUX_LOG", "/tmp/emux-test-debug.log");

        let size = PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        };
        let mut pty = UnixPty::spawn(&cmd, size).expect("failed to spawn emux");
        let child_pid = pty.child_pid() as i32;
        set_nonblocking(pty.master_raw_fd());

        // Spawn a background thread to continuously drain PTY output.
        let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let r = running.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 65536];
            while r.load(std::sync::atomic::Ordering::Relaxed) {
                match pty.read(&mut buf) {
                    Ok(0) => break,
                    Ok(_) => {}
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
            // pty is dropped here — sends SIGHUP to child
        });

        std::thread::sleep(Duration::from_millis(100));
        EmuxTestGuard {
            session_name: session_name.to_owned(),
            running,
            child_pid,
            _lock: lock,
        }
    }

    /// Wait for the agent socket file to appear.
    fn wait_for_agent_socket(session_name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir()
            .join("emux-sockets")
            .join(format!("emux-agent-{session_name}.sock"));
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            if path.exists() {
                // Give the listener a moment to be ready.
                std::thread::sleep(Duration::from_millis(100));
                return path;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("agent socket did not appear at {:?} within 5s", path);
    }

    /// Send an IPC message and get the response (retries on transient errors).
    fn ipc_send(sock_path: &std::path::Path, msg: &ClientMessage) -> ServerMessage {
        let start = Instant::now();
        loop {
            let result = try_ipc_send(sock_path, msg);
            match result {
                Ok(resp) => return resp,
                Err(e) => {
                    if start.elapsed() >= Duration::from_secs(5) {
                        panic!("ipc_send to {:?} failed after 5s: {}", sock_path, e);
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }
    }

    fn try_ipc_send(
        sock_path: &std::path::Path,
        msg: &ClientMessage,
    ) -> Result<ServerMessage, Box<dyn std::error::Error>> {
        let mut stream = UnixStream::connect(sock_path)?;
        stream.set_read_timeout(Some(Duration::from_secs(3)))?;
        stream.set_write_timeout(Some(Duration::from_secs(3)))?;

        let payload = emux_ipc::codec::encode(msg)?;
        stream.write_all(&payload)?;
        stream.flush()?;

        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf)?;
        let len = u32::from_be_bytes(len_buf) as usize;

        let mut resp_buf = vec![0u8; len];
        stream.read_exact(&mut resp_buf)?;
        Ok(emux_ipc::codec::decode(&resp_buf)?)
    }

    /// Wait for a marker to appear in CapturePane output.
    fn wait_for_capture(
        sock_path: &std::path::Path,
        pane_id: u32,
        marker: &str,
        timeout: Duration,
    ) -> String {
        let start = Instant::now();
        loop {
            let resp = ipc_send(sock_path, &ClientMessage::CapturePane { pane_id });
            if let ServerMessage::PaneCaptured { content, .. } = &resp {
                if content.contains(marker) {
                    return content.clone();
                }
            }
            if start.elapsed() >= timeout {
                if let ServerMessage::PaneCaptured { content, .. } = resp {
                    return content;
                }
                return String::new();
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn unique_session() -> String {
        use std::time::SystemTime;
        let t = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("test-{t}")
    }

    // ===================================================================
    // Tests
    // ===================================================================

    #[test]
    #[ignore] // requires real PTY — not for CI
    fn agent_ipc_list_panes() {
        let session = unique_session();
        let _guard = spawn_emux_with_session(&session);
        let sock = wait_for_agent_socket(&session);

        let resp = ipc_send(&sock, &ClientMessage::ListPanes);
        match resp {
            ServerMessage::PaneList { panes } => {
                assert!(!panes.is_empty(), "should have at least one pane");
                assert!(panes[0].active);
                println!("ListPanes OK: {} pane(s)", panes.len());
            }
            other => panic!("expected PaneList, got {:?}", other),
        }
    }

    #[test]
    #[ignore]
    fn agent_ipc_send_keys_and_capture() {
        let session = unique_session();
        let _guard = spawn_emux_with_session(&session);
        let sock = wait_for_agent_socket(&session);

        // Wait for shell to initialize inside emux.
        std::thread::sleep(Duration::from_secs(1));

        // Get pane ID.
        let pane_id = match ipc_send(&sock, &ClientMessage::ListPanes) {
            ServerMessage::PaneList { panes } => panes[0].id,
            other => panic!("expected PaneList, got {:?}", other),
        };

        // Send a command through the agent IPC.
        let resp = ipc_send(
            &sock,
            &ClientMessage::SendKeys {
                pane_id,
                keys: "echo AGENT_IPC_WORKS\n".into(),
            },
        );
        assert_eq!(resp, ServerMessage::Ack, "SendKeys should return Ack");

        // Drain PTY so emux can render.
        std::thread::sleep(Duration::from_millis(200));

        // Capture and verify.
        let content = wait_for_capture(&sock, pane_id, "AGENT_IPC_WORKS", Duration::from_secs(5));
        assert!(
            content.contains("AGENT_IPC_WORKS"),
            "CapturePane should show the echoed text.\nActual:\n{}",
            content
        );
        println!("SendKeys + CapturePane OK!");
    }

    #[test]
    #[ignore]
    fn agent_ipc_split_pane_and_use() {
        let session = unique_session();
        let _guard = spawn_emux_with_session(&session);
        let sock = wait_for_agent_socket(&session);

        std::thread::sleep(Duration::from_secs(1));

        // Split pane via IPC.
        let new_pane_id = match ipc_send(
            &sock,
            &ClientMessage::SplitPane {
                direction: emux_ipc::SplitDirection::Vertical,
                size: None,
            },
        ) {
            ServerMessage::SpawnResult { pane_id } => pane_id,
            other => panic!("expected SpawnResult, got {:?}", other),
        };

        // Wait for new pane's shell.
        std::thread::sleep(Duration::from_millis(500));

        // Send command to new pane.
        ipc_send(
            &sock,
            &ClientMessage::SendKeys {
                pane_id: new_pane_id,
                keys: "echo NEW_PANE_OK\n".into(),
            },
        );

        std::thread::sleep(Duration::from_millis(300));

        let content = wait_for_capture(&sock, new_pane_id, "NEW_PANE_OK", Duration::from_secs(5));
        assert!(
            content.contains("NEW_PANE_OK"),
            "new pane should show echoed text.\nActual:\n{}",
            content
        );

        // Verify ListPanes now shows 2 panes.
        let resp = ipc_send(&sock, &ClientMessage::ListPanes);
        match resp {
            ServerMessage::PaneList { panes } => {
                assert_eq!(panes.len(), 2, "should have 2 panes after split");
            }
            other => panic!("expected PaneList, got {:?}", other),
        }

        println!("SplitPane + SendKeys + CapturePane on new pane OK!");
    }

    #[test]
    #[ignore]
    fn agent_ipc_full_dev_loop() {
        let session = unique_session();
        let _guard = spawn_emux_with_session(&session);
        let sock = wait_for_agent_socket(&session);

        std::thread::sleep(Duration::from_secs(1));

        let pane_id = match ipc_send(&sock, &ClientMessage::ListPanes) {
            ServerMessage::PaneList { panes } => panes[0].id,
            other => panic!("expected PaneList, got {:?}", other),
        };

        // Step 1: AI checks what directory we're in.
        ipc_send(
            &sock,
            &ClientMessage::SendKeys {
                pane_id,
                keys: "pwd && echo PWD_DONE\n".into(),
            },
        );
        std::thread::sleep(Duration::from_millis(300));

        let content = wait_for_capture(&sock, pane_id, "PWD_DONE", Duration::from_secs(3));
        assert!(content.contains('/'), "should see a path");

        // Step 2: AI runs a build check.
        ipc_send(
            &sock,
            &ClientMessage::SendKeys {
                pane_id,
                keys: "true && echo BUILD_PASS || echo BUILD_FAIL\n".into(),
            },
        );
        std::thread::sleep(Duration::from_millis(300));

        let content = wait_for_capture(&sock, pane_id, "BUILD_PASS", Duration::from_secs(3));
        assert!(content.contains("BUILD_PASS"));

        // Step 3: AI creates a file.
        let marker = format!("emux_e2e_{}", std::process::id());
        let tmp = format!("/tmp/{marker}.txt");
        ipc_send(
            &sock,
            &ClientMessage::SendKeys {
                pane_id,
                keys: format!("echo '{marker}' > {tmp} && echo FILE_OK\n"),
            },
        );
        std::thread::sleep(Duration::from_millis(300));

        wait_for_capture(&sock, pane_id, "FILE_OK", Duration::from_secs(3));

        // Verify file on disk.
        let start = Instant::now();
        let mut found = false;
        while start.elapsed() < Duration::from_secs(3) {
            if std::path::Path::new(&tmp).exists() {
                let c = std::fs::read_to_string(&tmp).unwrap();
                assert!(c.contains(&marker));
                found = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(found, "file should exist on disk");
        let _ = std::fs::remove_file(&tmp);

        println!("Full AI dev loop OK: pwd → build check → file creation → verified");
    }
}
