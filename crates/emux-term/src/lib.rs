//! Terminal emulation core: grid, screen, cursor, and rendering state.

pub mod color;
pub mod cursor;
pub mod grid;
pub mod input;
pub mod modes;
pub mod performer;
pub mod screen;
pub mod search;
pub mod selection;

pub use color::Color;
pub use cursor::{Cursor, CursorShape, SavedCursor};
pub use grid::{Cell, CellAttrs, Grid, Row, UnderlineStyle};
pub use modes::{KittyKeyboardFlags, Modes, MouseMode};
pub use screen::{ClearTabStop, DamageMode, DamageRegion, EraseDisplay, EraseLine, Screen, SearchError, SearchMatch, SearchState};
pub use selection::{Selection, SelectionMode, SelectionPoint, SelectionState, osc52_clipboard};
