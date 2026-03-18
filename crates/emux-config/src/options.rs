//! General configuration options.

use serde::{Deserialize, Serialize};

use crate::keys::KeyBindings;
use crate::theme::Theme;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub theme: Theme,
    pub keys: KeyBindings,
    pub font_size: f32,
    pub font_family: Option<String>,
    pub scrollback_limit: usize,
    pub tab_width: usize,
    pub cursor_shape: String,
    pub cursor_blink: bool,
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
