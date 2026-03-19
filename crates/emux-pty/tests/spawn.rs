//! Integration tests for PTY spawning.

#[cfg(unix)]
mod unix_tests {
    use emux_pty::cmdbuilder::CommandBuilder;
    use emux_pty::unix::{PtySize, UnixPty};
    use std::io::Write;

    fn default_size() -> PtySize {
        PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        }
    }

    #[test]
    fn test_spawn_echo_and_read_output() {
        let mut cmd = CommandBuilder::new("/bin/echo");
        cmd.arg("hello");

        let mut pty = UnixPty::spawn(&cmd, default_size()).expect("failed to spawn");

        // Read output; echo should produce "hello\n" (potentially with some
        // terminal processing, e.g. "hello\r\n").
        let mut output = Vec::new();
        let mut buf = [0u8; 1024];

        // Read until EOF or we get enough data.
        loop {
            match pty.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => output.extend_from_slice(&buf[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
            if output.len() > 4 {
                break;
            }
        }

        let text = String::from_utf8_lossy(&output);
        assert!(
            text.contains("hello"),
            "expected output to contain 'hello', got: {text:?}"
        );

        // The child should have exited.
        let status = pty.wait().expect("failed to wait");
        assert!(status.success(), "expected success, got: {status}");
    }

    #[test]
    fn test_spawn_shell_send_command() {
        let cmd = CommandBuilder::new("/bin/sh");

        let mut pty = UnixPty::spawn(&cmd, default_size()).expect("failed to spawn");

        // Send a command to the shell.
        pty.write_all(b"echo PTY_TEST_OK\n")
            .expect("failed to write");

        // Read output until we find our marker.
        let mut output = Vec::new();
        let mut buf = [0u8; 4096];
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);

        loop {
            if std::time::Instant::now() > deadline {
                break;
            }
            match pty.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => output.extend_from_slice(&buf[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(ref e)
                    if e.raw_os_error() == Some(nix::libc::EIO)
                        || e.kind() == std::io::ErrorKind::WouldBlock =>
                {
                    break;
                }
                Err(_) => break,
            }

            let text = String::from_utf8_lossy(&output);
            if text.contains("PTY_TEST_OK") {
                break;
            }
        }

        let text = String::from_utf8_lossy(&output);
        assert!(
            text.contains("PTY_TEST_OK"),
            "expected output to contain 'PTY_TEST_OK', got: {text:?}"
        );

        // Tell the shell to exit.
        let _ = pty.write_all(b"exit\n");
    }

    #[test]
    fn test_resize_does_not_panic() {
        let cmd = CommandBuilder::new("/bin/sh");
        let pty = UnixPty::spawn(&cmd, default_size()).expect("failed to spawn");

        let new_size = PtySize {
            rows: 40,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        };

        pty.resize(new_size).expect("resize should not fail");

        // Resize again with different values.
        let another_size = PtySize {
            rows: 10,
            cols: 40,
            pixel_width: 100,
            pixel_height: 200,
        };

        pty.resize(another_size)
            .expect("second resize should not fail");

        // Clean up: the Drop impl will SIGHUP the child.
    }

    #[test]
    fn test_child_exit_detection() {
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.arg("-c").arg("exit 42");

        let mut pty = UnixPty::spawn(&cmd, default_size()).expect("failed to spawn");

        // Drain any output so the child can finish.
        let mut buf = [0u8; 1024];
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if std::time::Instant::now() > deadline {
                break;
            }
            match pty.read(&mut buf) {
                Ok(0) => break,
                Ok(_) => continue,
                Err(_) => break,
            }
        }

        let status = pty.wait().expect("failed to wait");
        match status {
            emux_pty::unix::ExitStatus::Code(code) => {
                assert_eq!(code, 42, "expected exit code 42, got {code}");
            }
            other => panic!("expected exit code, got: {other}"),
        }
    }

    #[test]
    fn test_child_pid_is_valid() {
        let cmd = CommandBuilder::new("/bin/sh");
        let pty = UnixPty::spawn(&cmd, default_size()).expect("failed to spawn");

        let pid = pty.child_pid();
        assert!(pid > 0, "child pid should be positive, got {pid}");
    }

    #[test]
    fn test_default_shell_builder() {
        let cmd = CommandBuilder::default_shell();
        let program = cmd.program();
        assert!(!program.is_empty(), "default shell should not be empty");
        // It should be either from $SHELL or /bin/sh.
        assert!(
            program.contains("sh")
                || program.contains("zsh")
                || program.contains("bash")
                || program.contains("fish"),
            "expected a shell program, got: {program}"
        );
    }
}
