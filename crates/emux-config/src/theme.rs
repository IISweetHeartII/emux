//! Theme and color scheme configuration.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    /// Background color (hex string).
    pub background: String,
    /// Default foreground/text color (hex string).
    pub foreground: String,
    /// Cursor color (hex string).
    pub cursor: String,
    /// Selection highlight background color (hex string).
    pub selection_bg: String,
    /// ANSI color palette (indices 0-7 normal, 8-15 bright).
    pub colors: [String; 16],
    /// Status bar background color.
    #[serde(default = "default_statusbar_bg")]
    pub statusbar_bg: String,
    /// Active tab accent color.
    #[serde(default = "default_accent")]
    pub accent: String,
    /// Active pane border color.
    #[serde(default = "default_border_active")]
    pub border_active: String,
    /// Inactive pane border color.
    #[serde(default = "default_border_inactive")]
    pub border_inactive: String,
    /// Enable Powerline-style separators in the status bar.
    #[serde(default = "default_powerline")]
    pub powerline: bool,
}

fn default_statusbar_bg() -> String {
    "#080808".into()
}
fn default_accent() -> String {
    "#00AFFF".into()
}
fn default_border_active() -> String {
    "#00AFFF".into()
}
fn default_border_inactive() -> String {
    "#303030".into()
}
fn default_powerline() -> bool {
    true
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: "#282C34".into(),
            foreground: "#ABB2BF".into(),
            cursor: "#528BFF".into(),
            selection_bg: "#3E4451".into(),
            colors: [
                // Normal 0-7
                "#1D1F21".into(),
                "#CC6666".into(),
                "#B5BD68".into(),
                "#F0C674".into(),
                "#81A2BE".into(),
                "#B294BB".into(),
                "#8ABEB7".into(),
                "#C5C8C6".into(),
                // Bright 8-15
                "#666666".into(),
                "#D54E53".into(),
                "#B9CA4A".into(),
                "#E7C547".into(),
                "#7AA6DA".into(),
                "#C397D8".into(),
                "#70C0B1".into(),
                "#EAEAEA".into(),
            ],
            statusbar_bg: default_statusbar_bg(),
            accent: default_accent(),
            border_active: default_border_active(),
            border_inactive: default_border_inactive(),
            powerline: default_powerline(),
        }
    }
}
