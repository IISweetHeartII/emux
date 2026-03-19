//! Self-upgrade support: check for new releases and replace the running binary.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

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

    // Minimal JSON parsing — extract "tag_name" without pulling in serde_json.
    let tag = extract_json_string(&body, "tag_name")
        .ok_or("could not find tag_name in release response")?;

    let version = tag.trim_start_matches('v').to_string();

    Ok(ReleaseInfo { tag, version })
}

/// Extract a string value for a given key from a JSON object (simple parser).
fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{key}\"");
    let start = json.find(&pattern)?;
    let after_key = &json[start + pattern.len()..];
    // Skip whitespace and colon
    let after_colon = after_key.find(':').map(|i| &after_key[i + 1..])?;
    let trimmed = after_colon.trim_start();
    if !trimmed.starts_with('"') {
        return None;
    }
    let value_start = 1; // skip opening quote
    let value_end = trimmed[value_start..].find('"')?;
    Some(trimmed[value_start..value_start + value_end].to_string())
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

/// Download and replace the current binary with the latest release.
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
        return Err(format!("download failed for {url}"));
    }

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

/// Auto-update on startup. Checks once per day; if a newer version exists,
/// downloads and replaces the binary automatically, then prompts restart.
pub(crate) fn check_update_notice() {
    // Only check once per day — store last check timestamp.
    let marker_path = env::temp_dir().join("emux-update-check");
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

        // Cache the check time regardless of result.
        let _ = fs::write(&marker_path, &release.version);

        if !is_newer(CURRENT_VERSION, &release.version) {
            return;
        }

        eprintln!(
            "\x1b[33mUpdating emux v{CURRENT_VERSION} → v{}...\x1b[0m",
            release.version
        );

        match download_and_replace(&release) {
            Ok(()) => {
                eprintln!(
                    "\x1b[32mUpdated to emux v{}.\x1b[0m Restart emux to use the new version.",
                    release.version
                );
            }
            Err(e) => {
                eprintln!(
                    "\x1b[31mAuto-update failed: {e}\x1b[0m Run \x1b[1memux upgrade\x1b[0m to retry."
                );
            }
        }
    });
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
        assert_eq!(extract_json_string(json, "tag_name"), Some("v0.2.0".into()));
    }

    #[test]
    fn extract_missing_key_returns_none() {
        let json = r#"{"name": "test"}"#;
        assert_eq!(extract_json_string(json, "tag_name"), None);
    }

    #[test]
    fn current_target_returns_valid_triple() {
        assert!(current_target().is_ok());
    }
}
