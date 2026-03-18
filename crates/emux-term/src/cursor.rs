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
    pub row: usize,
    pub col: usize,
    pub visible: bool,
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
    pub row: usize,
    pub col: usize,
    pub attrs: CellAttrs,
    pub fg: Color,
    pub bg: Color,
    pub charset_g0: emux_vt::Charset,
    pub charset_g1: emux_vt::Charset,
    pub active_charset: u8,
    pub origin_mode: bool,
    pub pending_wrap: bool,
}
