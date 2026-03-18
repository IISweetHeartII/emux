//! Configuration loading and schema definitions.

pub mod keys;
pub mod loader;
pub mod options;
pub mod theme;

pub use keys::KeyBindings;
pub use loader::{config_path, load_config, load_from_path, merge_with_defaults, ConfigError, ConfigWatcher};
pub use options::Config;
pub use theme::Theme;
