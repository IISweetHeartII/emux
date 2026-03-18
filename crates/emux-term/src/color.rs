//! Color definitions and palette management.

use serde::{Deserialize, Serialize};

/// Terminal color representation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Color {
    /// Default foreground or background color.
    #[default]
    Default,
    /// Indexed color (0-255).
    Indexed(u8),
    /// 24-bit true color.
    Rgb(u8, u8, u8),
}

