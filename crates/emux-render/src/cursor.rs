//! Cursor rendering (block, beam, underline styles).

use crossterm::cursor::SetCursorStyle;
use emux_term::CursorShape;

/// Map a terminal cursor shape to the corresponding crossterm cursor style.
pub fn cursor_style(shape: CursorShape) -> SetCursorStyle {
    match shape {
        CursorShape::Block => SetCursorStyle::SteadyBlock,
        CursorShape::Underline => SetCursorStyle::SteadyUnderScore,
        CursorShape::Bar => SetCursorStyle::SteadyBar,
    }
}
