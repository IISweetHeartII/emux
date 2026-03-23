//! Self-upgrade support: check for new releases and replace the running binary.
//!
//! Security: the downloaded archive is verified against a `SHA256SUMS` file
//! published alongside each GitHub release. Auto-update on startup only
//! *notifies* the user; the binary is never replaced without an explicit
//! `emux upgrade` invocation.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use sha2::{Digest, Sha256};

const REPO: &str = "IISweetHeartII/emux";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Information about the latest GitHub release.
struct ReleaseInfo {
    tag: String,
    version: String,
}

/// Check GitHub for the latest release version.
fn fetch_latest_release() -> Result<ReleaseInfo, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");

    let output = Command::new("curl")
        .args(["-fsSL", "-H", "Accept: application/vnd.github+json", &url])
        .output()
        .map_err(|e| format!("failed to run curl: {e}"))?;

    if !output.status.success() {
        return Err("failed to fetch release info from GitHub".into());
    }

    let body = String::from_utf8_lossy(&output.stdout);

    let json: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("invalid JSON from GitHub: {e}"))?;

    let tag = json["tag_name"]
        .as_str()
        .ok_or("could not find tag_name in release response")?
        .to_string();

    let version = tag.trim_start_matches('v').to_string();

    Ok(ReleaseInfo { tag, version })
}

/// Determine the target triple for the current platform.
fn current_target() -> Result<&'static str, String> {
    let os = env::consts::OS;
    let arch = env::consts::ARCH;

    match (os, arch) {
        ("linux", "x86_64") => Ok("x86_64-unknown-linux-gnu"),
        ("linux", "aarch64") => Ok("aarch64-unknown-linux-gnu"),
        ("macos", "x86_64") => Ok("x86_64-apple-darwin"),
        ("macos", "aarch64") => Ok("aarch64-apple-darwin"),
        ("windows", "x86_64") => Ok("x86_64-pc-windows-msvc"),
        ("windows", "aarch64") => Ok("aarch64-pc-windows-msvc"),
        _ => Err(format!("unsupported platform: {os}/{arch}")),
    }
}

/// Get the path to the currently running binary.
fn current_exe_path() -> Result<PathBuf, String> {
    env::current_exe().map_err(|e| format!("cannot determine current executable path: {e}"))
}

/// Compare two semver version strings. Returns true if `latest` is newer.
fn is_newer(current: &str, latest: &str) -> bool {
    let parse = |v: &str| -> (u32, u32, u32) {
        let parts: Vec<u32> = v.split('.').filter_map(|p| p.parse().ok()).collect();
        (
            parts.first().copied().unwrap_or(0),
            parts.get(1).copied().unwrap_or(0),
            parts.get(2).copied().unwrap_or(0),
        )
    };
    parse(latest) > parse(current)
}

/// Fetch the `SHA256SUMS` file for a release and return the expected hash
/// for the given archive filename.
fn fetch_expected_checksum(release: &ReleaseInfo, archive_name: &str) -> Result<String, String> {
    let url = format!(
        "https://github.com/{REPO}/releases/download/{}/SHA256SUMS",
        release.tag
    );
    let output = Command::new("curl")
        .args(["-fsSL", &url])
        .output()
        .map_err(|e| format!("failed to fetch SHA256SUMS: {e}"))?;

    if !output.status.success() {
        return Err("SHA256SUMS not found for this release — cannot verify integrity".into());
    }

    let body = String::from_utf8_lossy(&output.stdout);
    for line in body.lines() {
        // Format: "<hex-hash>  <filename>" or "<hex-hash> <filename>"
        let parts: Vec<&str> = line.splitn(2, |c: char| c.is_whitespace()).collect();
        if parts.len() == 2 && parts[1].trim() == archive_name {
            return Ok(parts[0].to_lowercase());
        }
    }

    Err(format!(
        "no checksum found for '{archive_name}' in SHA256SUMS"
    ))
}

/// Compute the SHA-256 hash of a file by streaming, returning the lowercase hex string.
fn sha256_file(path: &std::path::Path) -> Result<String, String> {
    use std::io::Read;
    let mut file =
        fs::File::open(path).map_err(|e| format!("failed to open file for checksum: {e}"))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| format!("failed to read file for checksum: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Download and replace the current binary with the latest release.
///
/// The archive is verified against the `SHA256SUMS` file published with the
/// release before extraction and installation.
///
/// **Threat model note:** SHA256SUMS and the archive are both fetched from
/// GitHub Releases. If an attacker compromises the release (e.g. via account
/// takeover), they control both files. Checksums protect against corruption
/// and MITM, but not a compromised release author. For that, cryptographic
/// signatures (Minisign/GPG) would be needed — deferred for now.
fn download_and_replace(release: &ReleaseInfo) -> Result<(), String> {
    let target = current_target()?;
    let exe_path = current_exe_path()?;

    let is_windows = cfg!(target_os = "windows");
    let ext = if is_windows { "zip" } else { "tar.gz" };
    let archive_name = format!("emux-{}-{target}.{ext}", release.tag);
    let url = format!(
        "https://github.com/{REPO}/releases/download/{}/{archive_name}",
        release.tag
    );

    // Fetch expected checksum BEFORE downloading the archive.
    eprint!("Verifying release integrity... ");
    let expected_hash = fetch_expected_checksum(release, &archive_name)?;
    eprintln!("ok");

    eprintln!("Downloading {}...", archive_name);

    // Download to a temp directory.
    let tmp_dir = env::temp_dir().join("emux-upgrade");
    let _ = fs::remove_dir_all(&tmp_dir);
    fs::create_dir_all(&tmp_dir).map_err(|e| format!("failed to create temp dir: {e}"))?;

    let archive_path = tmp_dir.join(&archive_name);

    let status = Command::new("curl")
        .args(["-fsSL", "-o"])
        .arg(&archive_path)
        .arg(&url)
        .status()
        .map_err(|e| format!("failed to download: {e}"))?;

    if !status.success() {
        let _ = fs::remove_dir_all(&tmp_dir);
        return Err(format!("download failed for {url}"));
    }

    // Verify SHA-256 checksum.
    eprint!("Verifying checksum... ");
    let actual_hash = sha256_file(&archive_path)?;
    if actual_hash != expected_hash {
        let _ = fs::remove_dir_all(&tmp_dir);
        return Err(format!(
            "checksum mismatch!\n  expected: {expected_hash}\n  actual:   {actual_hash}\n\
             The downloaded file may have been tampered with. Aborting upgrade."
        ));
    }
    eprintln!("ok (SHA-256 verified)");

    // Extract.
    if is_windows {
        // Use PowerShell to extract zip on Windows.
        let status = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!(
                    "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                    archive_path.display(),
                    tmp_dir.display()
                ),
            ])
            .status()
            .map_err(|e| format!("failed to extract zip: {e}"))?;
        if !status.success() {
            return Err("failed to extract zip archive".into());
        }
    } else {
        let status = Command::new("tar")
            .args(["xzf"])
            .arg(&archive_path)
            .arg("-C")
            .arg(&tmp_dir)
            .status()
            .map_err(|e| format!("failed to extract: {e}"))?;
        if !status.success() {
            return Err("failed to extract tar archive".into());
        }
    }

    // Replace the current binary.
    let binary_name = if is_windows { "emux.exe" } else { "emux" };
    let new_binary = tmp_dir.join(binary_name);

    if !new_binary.exists() {
        return Err(format!(
            "extracted binary not found at {}",
            new_binary.display()
        ));
    }

    // On Unix, we can replace the running binary by renaming.
    // On Windows, rename the old one first since it may be locked.
    let backup_path = exe_path.with_extension("old");
    let _ = fs::remove_file(&backup_path);

    if is_windows {
        fs::rename(&exe_path, &backup_path)
            .map_err(|e| format!("failed to backup current binary: {e}"))?;
    }

    fs::copy(&new_binary, &exe_path).map_err(|e| format!("failed to install new binary: {e}"))?;

    // Set executable permission on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&exe_path, fs::Permissions::from_mode(0o755));
    }

    // Cleanup.
    let _ = fs::remove_dir_all(&tmp_dir);
    let _ = fs::remove_file(&backup_path);

    Ok(())
}

/// `emux upgrade` — check for updates and self-upgrade.
pub(crate) fn cmd_upgrade() -> Result<(), crate::AppError> {
    eprintln!("Current version: v{CURRENT_VERSION}");
    eprint!("Checking for updates... ");

    let release = fetch_latest_release()
        .map_err(|e| crate::AppError::Msg(format!("update check failed: {e}")))?;

    if !is_newer(CURRENT_VERSION, &release.version) {
        eprintln!("already up to date.");
        return Ok(());
    }

    eprintln!("v{} available!", release.version);
    download_and_replace(&release)
        .map_err(|e| crate::AppError::Msg(format!("upgrade failed: {e}")))?;

    eprintln!("Upgraded to v{}.", release.version);
    Ok(())
}

/// Check for updates on startup (once per day) and notify the user.
///
/// This function only prints a notice — it never downloads or replaces the
/// binary. The user must run `emux upgrade` explicitly to apply updates.
pub(crate) fn check_update_notice() {
    // Only check once per day — store last check timestamp in the user's
    // data directory (not /tmp) to prevent other users from resetting it.
    let marker_path = marker_file_path();
    if let Ok(meta) = fs::metadata(&marker_path) {
        if let Ok(modified) = meta.modified() {
            if modified.elapsed().unwrap_or_default().as_secs() < 86400 {
                return;
            }
        }
    }

    // Spawn a background thread so we don't block startup.
    std::thread::spawn(move || {
        let Ok(release) = fetch_latest_release() else {
            return;
        };

        // Update the marker file regardless of result.
        if let Some(parent) = marker_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&marker_path, &release.version);

        if !is_newer(CURRENT_VERSION, &release.version) {
            return;
        }

        eprintln!(
            "\x1b[33memux v{} available\x1b[0m (current: v{CURRENT_VERSION}). \
             Run \x1b[1memux upgrade\x1b[0m to update.",
            release.version
        );
    });
}

/// Path to the update-check marker file, stored in the user's data directory.
fn marker_file_path() -> PathBuf {
    if let Ok(home) = env::var("HOME") {
        PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("emux")
            .join(".update-check")
    } else {
        env::temp_dir().join("emux-update-check")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_newer_detects_patch_bump() {
        assert!(is_newer("0.1.0", "0.1.1"));
    }

    #[test]
    fn is_newer_detects_minor_bump() {
        assert!(is_newer("0.1.0", "0.2.0"));
    }

    #[test]
    fn is_newer_detects_major_bump() {
        assert!(is_newer("0.1.0", "1.0.0"));
    }

    #[test]
    fn is_newer_same_version() {
        assert!(!is_newer("0.1.0", "0.1.0"));
    }

    #[test]
    fn is_newer_older_version() {
        assert!(!is_newer("0.2.0", "0.1.0"));
    }

    #[test]
    fn extract_tag_name_from_json() {
        let json = r#"{"tag_name": "v0.2.0", "name": "Release v0.2.0"}"#;
        let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(parsed["tag_name"].as_str(), Some("v0.2.0"));
    }

    #[test]
    fn extract_missing_key_returns_none() {
        let json = r#"{"name": "test"}"#;
        let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
        assert!(parsed["tag_name"].as_str().is_none());
    }

    #[test]
    fn current_target_returns_valid_triple() {
        assert!(current_target().is_ok());
    }

    #[test]
    fn sha256_file_computes_correct_hash() {
        let dir = std::env::temp_dir().join(format!("emux-sha256-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test.bin");
        std::fs::write(&path, b"hello world").unwrap();

        let hash = sha256_file(&path).unwrap();
        // SHA-256 of "hello world" is well-known.
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn sha256_file_empty_file() {
        let dir = std::env::temp_dir().join(format!("emux-sha256-empty-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("empty.bin");
        std::fs::write(&path, b"").unwrap();

        let hash = sha256_file(&path).unwrap();
        // SHA-256 of empty input.
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn sha256_file_nonexistent_returns_error() {
        let result = sha256_file(std::path::Path::new("/tmp/emux-does-not-exist-xyz"));
        assert!(result.is_err());
    }

    #[test]
    fn marker_file_path_contains_emux() {
        let path = marker_file_path();
        let path_str = path.to_string_lossy();
        assert!(
            path_str.contains("emux"),
            "marker path should contain 'emux': {path_str}"
        );
    }

    #[test]
    fn marker_file_path_not_in_tmp_root() {
        // The marker should be in the user's data directory, not bare /tmp/
        let path = marker_file_path();
        let path_str = path.to_string_lossy();
        // Should NOT be directly /tmp/emux-update-check (old location)
        // unless HOME is not set.
        if std::env::var("HOME").is_ok() {
            assert!(
                !path_str.starts_with("/tmp/emux-update-check"),
                "marker should not be in /tmp when HOME is set: {path_str}"
            );
        }
    }
}
