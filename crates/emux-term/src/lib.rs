//! Terminal emulation core: grid, screen, cursor, and rendering state.

pub mod color;
pub mod cursor;
pub mod grid;
pub mod hints;
pub mod input;
pub mod modes;
pub mod performer;
pub mod screen;
pub mod search;
pub mod selection;

pub use color::Color;
pub use cursor::{Cursor, CursorShape, SavedCursor};
pub use grid::{Cell, CellAttrs, Grid, Row, UnderlineStyle};
pub use hints::{HintKind, HintMatch};
pub use modes::{KittyKeyboardFlags, Modes, MouseMode};
pub use screen::{
    ClearTabStop, DamageMode, DamageRegion, EraseDisplay, EraseLine, Notification, Screen,
    SearchError, SearchMatch, SearchState, ShellMark, ShellMarkKind,
};
pub use selection::{
    Selection, SelectionMode, SelectionPoint, SelectionState, base64_decode, base64_encode,
    osc52_clipboard,
};
