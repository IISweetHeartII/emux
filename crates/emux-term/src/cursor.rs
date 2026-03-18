//! Cursor position and style tracking.

use serde::{Deserialize, Serialize};

use crate::color::Color;
use crate::grid::CellAttrs;

/// Cursor shape.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CursorShape {
    #[default]
    Block,
    Underline,
    Bar,
}

/// Cursor state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cursor {
    /// Row position in the viewport (0-based).
    pub row: usize,
    /// Column position in the viewport (0-based).
    pub col: usize,
    /// Whether the cursor is visible.
    pub visible: bool,
    /// Visual shape of the cursor.
    pub shape: CursorShape,
}

impl Default for Cursor {
    fn default() -> Self {
        Self {
            row: 0,
            col: 0,
            visible: true,
            shape: CursorShape::Block,
        }
    }
}

/// Saved cursor state for DECSC/DECRC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedCursor {
    /// Saved row position.
    pub row: usize,
    /// Saved column position.
    pub col: usize,
    /// Saved cell rendering attributes.
    pub attrs: CellAttrs,
    /// Saved foreground color.
    pub fg: Color,
    /// Saved background color.
    pub bg: Color,
    /// Saved G0 character set.
    pub charset_g0: emux_vt::Charset,
    /// Saved G1 character set.
    pub charset_g1: emux_vt::Charset,
    /// Saved active character set index (0 = G0, 1 = G1).
    pub active_charset: u8,
    /// Saved origin mode (DECOM) state.
    pub origin_mode: bool,
    /// Saved pending-wrap flag.
    pub pending_wrap: bool,
}
