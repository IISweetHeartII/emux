//! General configuration options.

use serde::{Deserialize, Serialize};

use crate::keys::KeyBindings;
use crate::theme::Theme;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Color theme configuration.
    pub theme: Theme,
    /// Key binding configuration.
    pub keys: KeyBindings,
    /// Font size in points.
    pub font_size: f32,
    /// Optional font family name override.
    pub font_family: Option<String>,
    /// Maximum number of lines kept in the scrollback buffer.
    pub scrollback_limit: usize,
    /// Number of columns per tab stop.
    pub tab_width: usize,
    /// Cursor shape: "block", "underline", or "bar".
    pub cursor_shape: String,
    /// Whether the cursor should blink.
    pub cursor_blink: bool,
    /// Whether bold text is rendered using bright ANSI colors.
    pub bold_is_bright: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            keys: KeyBindings::default(),
            font_size: 14.0,
            font_family: None,
            scrollback_limit: 10_000,
            tab_width: 8,
            cursor_shape: "block".into(),
            cursor_blink: true,
            bold_is_bright: false,
        }
    }
}
