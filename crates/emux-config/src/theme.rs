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
        }
    }
}
