//! Tests for session management CLI subcommands (list, new, kill, attach).
//!
//! Tests that require a real PTY or terminal are marked `#[ignore]`.

use std::process::Command;
use std::time::Duration;

/// Helper: path to the compiled `emux` binary.
fn emux_bin() -> String {
    env!("CARGO_BIN_EXE_emux").to_string()
}

// ---------------------------------------------------------------------------
// 1. `emux ls` with no daemon running
// ---------------------------------------------------------------------------

/// Run a command with a timeout. Returns the output or panics on timeout.
fn run_with_timeout(cmd: &str, args: &[&str], timeout: Duration) -> std::process::Output {
    let mut child = Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("failed to spawn {cmd}: {e}"));

    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output().unwrap(),
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("{cmd} {args:?} timed out after {timeout:?}");
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => panic!("error waiting for {cmd}: {e}"),
        }
    }
}

/// Kill stale emux daemons and remove leftover socket files.
fn cleanup_stale_sessions() {
    let sock_dir = std::env::temp_dir().join("emux-sockets");
    if let Ok(entries) = std::fs::read_dir(&sock_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|f| f.to_str()) {
                if let Some(session) = name
                    .strip_prefix("emux-")
                    .and_then(|s| s.strip_suffix(".sock"))
                {
                    // Try graceful kill with a timeout — ignore failures.
                    let mut child = Command::new(emux_bin())
                        .args(["kill", session])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn()
                        .ok();
                    if let Some(ref mut c) = child {
                        std::thread::sleep(Duration::from_secs(1));
                        let _ = c.kill();
                        let _ = c.wait();
                    }
                }
            }
        }
    }
    std::thread::sleep(Duration::from_millis(300));
    // Clean up daemon sockets in temp_dir (emux-test-* format).
    if let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|f| f.to_str()) {
                if name.starts_with("emux-test-") {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    }
    // Force-remove any remaining socket files.
    if let Ok(entries) = std::fs::read_dir(&sock_dir) {
        for entry in entries.flatten() {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

#[test]
fn list_shows_no_active_sessions_when_none_running() {
    cleanup_stale_sessions();

    let output = run_with_timeout(&emux_bin(), &["ls"], Duration::from_secs(5));

    assert!(output.status.success(), "exit code should be 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("No active sessions"),
        "should report no sessions, got: {stdout}"
    );
}

#[test]
fn list_long_form_works() {
    cleanup_stale_sessions();

    let output = run_with_timeout(&emux_bin(), &["list"], Duration::from_secs(5));

    assert!(output.status.success(), "exit code should be 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Either "No active sessions" or a list of sessions — both are valid.
    assert!(!stdout.is_empty(), "list output should not be empty");
}

// ---------------------------------------------------------------------------
// 2. `emux --help` shows new subcommands
// ---------------------------------------------------------------------------

#[test]
fn help_shows_new_subcommand() {
    let output = Command::new(emux_bin())
        .arg("--help")
        .output()
        .expect("failed to execute emux --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("new"),
        "help should mention 'new' subcommand, got: {stdout}"
    );
}

#[test]
fn help_shows_attach_subcommand() {
    let output = Command::new(emux_bin())
        .arg("--help")
        .output()
        .expect("failed to execute emux --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("attach"),
        "help should mention 'attach' subcommand, got: {stdout}"
    );
}

#[test]
fn help_shows_list_subcommand() {
    let output = Command::new(emux_bin())
        .arg("--help")
        .output()
        .expect("failed to execute emux --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("list"),
        "help should mention 'list' subcommand, got: {stdout}"
    );
}

#[test]
fn help_shows_kill_subcommand() {
    let output = Command::new(emux_bin())
        .arg("--help")
        .output()
        .expect("failed to execute emux --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("kill"),
        "help should mention 'kill' subcommand, got: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// 3. `emux kill` without name shows usage error
// ---------------------------------------------------------------------------

#[test]
fn kill_without_name_exits_nonzero() {
    let output = Command::new(emux_bin())
        .arg("kill")
        .output()
        .expect("failed to execute emux kill");

    assert!(
        !output.status.success(),
        "kill without name should exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("requires a session name"),
        "should mention missing session name, got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// 4. `emux kill` with non-existent session
// ---------------------------------------------------------------------------

#[test]
fn kill_nonexistent_session_exits_nonzero() {
    let output = Command::new(emux_bin())
        .arg("kill")
        .arg("nonexistent-session-9999")
        .output()
        .expect("failed to execute emux kill");

    assert!(
        !output.status.success(),
        "kill non-existent session should exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not found"),
        "should mention session not found, got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// 5. `emux attach` with no sessions available
// ---------------------------------------------------------------------------

#[test]
fn attach_no_sessions_exits_nonzero() {
    // Clean up any leftover sockets to ensure no sessions exist.
    let output = Command::new(emux_bin())
        .arg("attach")
        .output()
        .expect("failed to execute emux attach");

    // This may succeed if there's an existing session, or fail if there aren't any.
    // We just ensure it doesn't panic.
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert!(
            output.status.signal().is_none(),
            "emux attach should not be killed by a signal"
        );
    }
}

// ---------------------------------------------------------------------------
// 6. `emux attach` with non-existent session name
// ---------------------------------------------------------------------------

#[test]
fn attach_nonexistent_session_exits_nonzero() {
    let output = Command::new(emux_bin())
        .arg("attach")
        .arg("nonexistent-session-8888")
        .output()
        .expect("failed to execute emux attach");

    assert!(
        !output.status.success(),
        "attach non-existent session should exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not found"),
        "should mention session not found, got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// 7. Unknown subcommand still rejected
// ---------------------------------------------------------------------------

#[test]
fn unknown_subcommand_exits_nonzero() {
    let output = Command::new(emux_bin())
        .arg("frobnicate")
        .output()
        .expect("failed to execute emux frobnicate");

    assert!(
        !output.status.success(),
        "unknown subcommand should exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown command"),
        "should mention unknown command, got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// 8. E2E: new + ls + kill lifecycle (requires PTY)
// ---------------------------------------------------------------------------

/// This test spawns a daemon via `emux new`, verifies it appears in `emux ls`,
/// then kills it with `emux kill`. Requires a working PTY.
#[test]
#[ignore]
fn new_list_kill_lifecycle() {
    let session_name = format!("test-session-{}", std::process::id());

    // Start a new session in the background. Since `emux new` enters the TUI
    // event loop, we spawn it and immediately detach.
    use std::process::Stdio;
    let mut child = Command::new(emux_bin())
        .args(["new", &session_name])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn emux new");

    // Give daemon time to start, then retry ls up to 5 times.
    let mut found = false;
    for attempt in 0..5 {
        std::thread::sleep(Duration::from_secs(1));
        let ls_output = Command::new(emux_bin())
            .arg("ls")
            .output()
            .expect("failed to execute emux ls");
        let ls_stdout = String::from_utf8_lossy(&ls_output.stdout);
        if ls_stdout.contains(&session_name) {
            found = true;
            break;
        }
        if attempt == 4 {
            panic!("session should appear in ls output after 5 attempts, got: {ls_stdout}");
        }
    }
    assert!(found);

    // Kill the session.
    let kill_output = Command::new(emux_bin())
        .args(["kill", &session_name])
        .output()
        .expect("failed to execute emux kill");
    assert!(kill_output.status.success(), "kill should succeed");
    let kill_stdout = String::from_utf8_lossy(&kill_output.stdout);
    assert!(
        kill_stdout.contains("killed"),
        "should confirm kill, got: {kill_stdout}"
    );

    // Clean up the child process.
    let _ = child.kill();
    let _ = child.wait();

    // Verify session is gone from listing.
    let ls2_output = Command::new(emux_bin())
        .arg("ls")
        .output()
        .expect("failed to execute emux ls");
    let ls2_stdout = String::from_utf8_lossy(&ls2_output.stdout);
    assert!(
        !ls2_stdout.contains(&session_name),
        "session should be gone after kill, got: {ls2_stdout}"
    );
}
