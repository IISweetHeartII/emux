//! End-to-end / CLI integration tests for the `emux` binary.
//!
//! These tests spawn the real binary via `std::process::Command` and verify
//! exit codes, stdout/stderr output, and basic resilience properties.

use std::process::Command;
use std::time::Duration;

/// Helper: path to the compiled `emux` binary.
fn emux_bin() -> String {
    env!("CARGO_BIN_EXE_emux").to_string()
}

// ---------------------------------------------------------------------------
// 1. CLI flag tests
// ---------------------------------------------------------------------------

#[test]
fn help_flag_prints_usage_and_exits_zero() {
    let output = Command::new(emux_bin())
        .arg("--help")
        .output()
        .expect("failed to execute emux --help");

    assert!(output.status.success(), "exit code should be 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Usage"),
        "help output should contain 'Usage', got: {stdout}"
    );
    assert!(
        stdout.contains("emux"),
        "help output should mention 'emux', got: {stdout}"
    );
}

#[test]
fn short_help_flag_prints_usage_and_exits_zero() {
    let output = Command::new(emux_bin())
        .arg("-h")
        .output()
        .expect("failed to execute emux -h");

    assert!(output.status.success(), "exit code should be 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Usage"),
        "short help output should contain 'Usage', got: {stdout}"
    );
}

#[test]
fn version_flag_prints_version_and_exits_zero() {
    let output = Command::new(emux_bin())
        .arg("--version")
        .output()
        .expect("failed to execute emux --version");

    assert!(output.status.success(), "exit code should be 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.starts_with("emux "),
        "version output should start with 'emux ', got: {stdout}"
    );
    // Verify the version string contains a semver-like pattern.
    let version_part = stdout.trim().strip_prefix("emux ").unwrap();
    assert!(
        version_part.contains('.'),
        "version should contain a dot (semver), got: {version_part}"
    );
}

#[test]
fn short_version_flag_prints_version_and_exits_zero() {
    let output = Command::new(emux_bin())
        .arg("-V")
        .output()
        .expect("failed to execute emux -V");

    assert!(output.status.success(), "exit code should be 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.starts_with("emux "),
        "-V output should start with 'emux ', got: {stdout}"
    );
}

#[test]
fn invalid_flag_exits_nonzero() {
    let output = Command::new(emux_bin())
        .arg("--invalid")
        .output()
        .expect("failed to execute emux --invalid");

    assert!(
        !output.status.success(),
        "exit code should be non-zero for invalid flag"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown option"),
        "stderr should mention 'unknown option', got: {stderr}"
    );
}

#[test]
fn unknown_short_flag_exits_nonzero() {
    let output = Command::new(emux_bin())
        .arg("-Z")
        .output()
        .expect("failed to execute emux -Z");

    assert!(
        !output.status.success(),
        "exit code should be non-zero for unknown flag"
    );
}

#[test]
fn unknown_subcommand_exits_nonzero() {
    let output = Command::new(emux_bin())
        .arg("doesnotexist")
        .output()
        .expect("failed to execute emux doesnotexist");

    assert!(
        !output.status.success(),
        "exit code should be non-zero for unknown subcommand"
    );
}

// ---------------------------------------------------------------------------
// 2. Version consistency
// ---------------------------------------------------------------------------

#[test]
fn version_matches_cargo_pkg_version() {
    let output = Command::new(emux_bin())
        .arg("--version")
        .output()
        .expect("failed to execute emux --version");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected_version = env!("CARGO_PKG_VERSION");
    assert!(
        stdout.contains(expected_version),
        "version output should contain '{expected_version}', got: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// 3. Help content completeness
// ---------------------------------------------------------------------------

#[test]
fn help_mentions_keybindings() {
    let output = Command::new(emux_bin())
        .arg("--help")
        .output()
        .expect("failed to execute emux --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Keybindings") || stdout.contains("keybindings"),
        "help should mention keybindings, got: {stdout}"
    );
}

#[test]
fn help_mentions_leader() {
    let output = Command::new(emux_bin())
        .arg("--help")
        .output()
        .expect("failed to execute emux --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Leader") || stdout.contains("leader"),
        "help should mention leader key, got: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// 3b. Help lists subcommands (new, attach, list, kill)
// ---------------------------------------------------------------------------

#[test]
fn help_mentions_new_subcommand() {
    let output = Command::new(emux_bin())
        .arg("--help")
        .output()
        .expect("failed to execute emux --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("new"),
        "help should mention 'new' subcommand, got: {stdout}"
    );
}

#[test]
fn help_mentions_attach_subcommand() {
    let output = Command::new(emux_bin())
        .arg("--help")
        .output()
        .expect("failed to execute emux --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("attach"),
        "help should mention 'attach' subcommand, got: {stdout}"
    );
}

#[test]
fn help_mentions_list_subcommand() {
    let output = Command::new(emux_bin())
        .arg("--help")
        .output()
        .expect("failed to execute emux --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("list"),
        "help should mention 'list' subcommand, got: {stdout}"
    );
}

#[test]
fn help_mentions_kill_subcommand() {
    let output = Command::new(emux_bin())
        .arg("--help")
        .output()
        .expect("failed to execute emux --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("kill"),
        "help should mention 'kill' subcommand, got: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// 4. Binary spawns without immediate crash (no TTY required)
// ---------------------------------------------------------------------------

/// Spawn the binary without a TTY. It will fail at terminal::size() or
/// enable_raw_mode() because stdin is not a terminal. We verify it exits
/// with an error (not a panic/signal) and does so promptly.
#[test]
fn binary_exits_gracefully_without_tty() {
    use std::process::Stdio;

    let mut child = Command::new(emux_bin())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn emux");

    // The process should exit quickly since it has no TTY.
    let timeout = Duration::from_secs(5);
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // It exited. Without a TTY it's expected to fail, but it
                // should NOT have been killed by a signal (i.e. no panic/SIGSEGV).
                #[cfg(unix)]
                {
                    use std::os::unix::process::ExitStatusExt;
                    assert!(
                        status.signal().is_none(),
                        "emux should not be killed by a signal, got signal: {:?}",
                        status.signal()
                    );
                }
                return;
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    panic!("emux did not exit within {timeout:?} without a TTY");
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => panic!("error waiting for emux: {e}"),
        }
    }
}

// ---------------------------------------------------------------------------
// 5. Large input resilience (WouldBlock regression)
// ---------------------------------------------------------------------------

/// Feed a large chunk of data to stdin when spawning without a TTY.
/// The binary should exit without panicking or hanging. This is a
/// regression guard for WouldBlock handling — even though the binary
/// won't fully process input without a TTY, it must not crash.
#[test]
fn large_stdin_does_not_crash() {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = Command::new(emux_bin())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn emux");

    // Write a large payload to stdin. The binary will likely fail before
    // reading any of this (no TTY), but we want to ensure no panic.
    if let Some(mut stdin) = child.stdin.take() {
        let payload = vec![b'A'; 1024 * 1024]; // 1 MB
        // Ignore write errors — the process may have already exited.
        let _ = stdin.write_all(&payload);
        drop(stdin);
    }

    let timeout = Duration::from_secs(5);
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                #[cfg(unix)]
                {
                    use std::os::unix::process::ExitStatusExt;
                    assert!(
                        status.signal().is_none(),
                        "emux should not be killed by a signal with large input, got signal: {:?}",
                        status.signal()
                    );
                }
                let _ = status; // suppress unused warning on non-unix
                return;
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    panic!("emux hung with large stdin input (possible WouldBlock issue)");
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => panic!("error waiting for emux: {e}"),
        }
    }
}

// ---------------------------------------------------------------------------
// 6. Multiple invalid args
// ---------------------------------------------------------------------------

#[test]
fn only_first_invalid_arg_reported() {
    let output = Command::new(emux_bin())
        .args(["--foo", "--bar"])
        .output()
        .expect("failed to execute emux");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // The binary processes args[1] first, so it should mention --foo.
    assert!(
        stderr.contains("--foo"),
        "stderr should mention the first invalid arg '--foo', got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// 7. Help suggests --help on bad flag
// ---------------------------------------------------------------------------

#[test]
fn error_message_suggests_help() {
    let output = Command::new(emux_bin())
        .arg("--bad")
        .output()
        .expect("failed to execute emux --bad");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--help"),
        "error message should suggest --help, got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// 8. SSH subcommand
// ---------------------------------------------------------------------------

#[test]
fn help_mentions_ssh_subcommand() {
    let output = Command::new(emux_bin())
        .arg("--help")
        .output()
        .expect("failed to execute emux --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ssh"),
        "help should mention 'ssh' subcommand, got: {stdout}"
    );
}

#[test]
fn ssh_without_destination_exits_nonzero() {
    let output = Command::new(emux_bin())
        .arg("ssh")
        .output()
        .expect("failed to execute emux ssh");

    assert!(
        !output.status.success(),
        "emux ssh with no args should fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("missing destination") || stderr.contains("destination"),
        "should mention missing destination, got: {stderr}"
    );
}

#[test]
fn ssh_with_invalid_destination_exits_nonzero() {
    let output = Command::new(emux_bin())
        .args(["ssh", "@badhost"])
        .output()
        .expect("failed to execute emux ssh @badhost");

    assert!(!output.status.success(), "emux ssh @badhost should fail");
}

#[test]
fn ssh_with_unknown_subcommand_exits_nonzero() {
    let output = Command::new(emux_bin())
        .args(["ssh", "host", "bogus"])
        .output()
        .expect("failed to execute emux ssh host bogus");

    assert!(!output.status.success(), "emux ssh host bogus should fail");
}
