//! Configuration file discovery and loading.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::options::Config;

/// Errors that can occur when loading configuration.
#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Parse(toml::de::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "IO error: {e}"),
            ConfigError::Parse(e) => write!(f, "TOML parse error: {e}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<std::io::Error> for ConfigError {
    fn from(e: std::io::Error) -> Self {
        ConfigError::Io(e)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(e: toml::de::Error) -> Self {
        ConfigError::Parse(e)
    }
}

/// Returns the default config file path: `~/.config/emux/config.toml`.
pub fn config_path() -> Option<PathBuf> {
    dirs_or_home().map(|home| home.join(".config").join("emux").join("config.toml"))
}

/// Deprecated alias kept for internal use.
fn default_config_path() -> Option<PathBuf> {
    config_path()
}

fn dirs_or_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Load configuration from `~/.config/emux/config.toml`, falling back to
/// defaults if the file does not exist.
pub fn load_config() -> Config {
    let Some(path) = default_config_path() else {
        return Config::default();
    };
    if !path.exists() {
        return Config::default();
    }
    load_from_path(&path).unwrap_or_default()
}

/// Load and parse a config file at the given path. Partial configs are merged
/// with defaults so that any missing field gets its default value.
pub fn load_from_path(path: &Path) -> Result<Config, ConfigError> {
    let content = std::fs::read_to_string(path)?;
    let value: toml::Value = toml::from_str(content.trim()).map_err(ConfigError::Parse)?;
    Ok(merge_with_defaults(value))
}

/// Merge a partial TOML value with the full default config. Fields present in
/// `partial` override the defaults; missing fields keep their default values.
pub fn merge_with_defaults(partial: toml::Value) -> Config {
    let default_value =
        toml::Value::try_from(Config::default()).expect("default config must serialize");
    let merged = deep_merge(default_value, partial);
    merged
        .try_into::<Config>()
        .unwrap_or_else(|_| Config::default())
}

/// Recursively merge `overlay` into `base`. For tables, keys in `overlay`
/// override keys in `base`; for all other types the overlay value wins.
fn deep_merge(base: toml::Value, overlay: toml::Value) -> toml::Value {
    match (base, overlay) {
        (toml::Value::Table(mut base_map), toml::Value::Table(overlay_map)) => {
            for (key, overlay_val) in overlay_map {
                let merged_val = if let Some(base_val) = base_map.remove(&key) {
                    deep_merge(base_val, overlay_val)
                } else {
                    overlay_val
                };
                base_map.insert(key, merged_val);
            }
            toml::Value::Table(base_map)
        }
        (_, overlay) => overlay,
    }
}

// ---------------------------------------------------------------------------
// Config hot-reload watcher
// ---------------------------------------------------------------------------

/// Watches a config file for changes by polling its mtime.
///
/// No external dependencies required -- uses `std::fs::metadata().modified()`.
pub struct ConfigWatcher {
    path: PathBuf,
    last_mtime: Option<SystemTime>,
}

impl ConfigWatcher {
    /// Create a watcher for the given config file path.
    /// Records the current mtime so the first call to `check` only fires if
    /// the file has been modified *after* this point.
    pub fn new(path: PathBuf) -> Self {
        let last_mtime = std::fs::metadata(&path)
            .ok()
            .and_then(|m| m.modified().ok());
        Self { path, last_mtime }
    }

    /// Create a watcher using the default config path.
    /// Returns `None` if `$HOME` is not set.
    pub fn for_default_path() -> Option<Self> {
        config_path().map(Self::new)
    }

    /// The path being watched.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Check whether the config file has been modified since the last check.
    ///
    /// If the file was modified (or appeared after being absent), reload it and
    /// return the new `Config`. Otherwise return `None`.
    pub fn check(&mut self) -> Option<Config> {
        let current_mtime = std::fs::metadata(&self.path)
            .ok()
            .and_then(|m| m.modified().ok());

        let changed = match (self.last_mtime, current_mtime) {
            (Some(prev), Some(cur)) => cur != prev,
            (None, Some(_)) => true, // file appeared
            _ => false,
        };

        if changed {
            self.last_mtime = current_mtime;
            load_from_path(&self.path).ok()
        } else {
            None
        }
    }
}
