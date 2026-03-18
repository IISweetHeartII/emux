//! Text selection handling for copy/paste.
//!
//! [`Selection`] tracks a start and end position within the terminal grid
//! (including scrollback), supports both normal (stream) and rectangular
//! (block) selection modes, and can extract the selected text as a `String`.

use crate::grid::Grid;

/// A position in the terminal, using an absolute coordinate system where
/// row 0 is the first (oldest) scrollback line. Viewport rows follow after
/// the scrollback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionPoint {
    /// Absolute row: 0 = oldest scrollback line, scrollback_len = first viewport row.
    pub row: usize,
    /// Column index (0-based).
    pub col: usize,
}

impl SelectionPoint {
    pub fn new(row: usize, col: usize) -> Self {
        Self { row, col }
    }
}

impl PartialOrd for SelectionPoint {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SelectionPoint {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.row.cmp(&other.row).then(self.col.cmp(&other.col))
    }
}

/// The kind of selection being made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    /// Normal stream selection (like clicking and dragging in a terminal).
    Normal,
    /// Rectangular / block selection (like Alt+click in many terminals).
    Rectangular,
}

/// Current state of the selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionState {
    /// No selection active.
    None,
    /// User is actively extending the selection.
    Selecting,
    /// Selection is complete (user released or confirmed).
    Selected,
}

/// Tracks a text selection within the terminal grid + scrollback.
#[derive(Debug, Clone)]
pub struct Selection {
    /// The anchor point (where selection started).
    pub start: SelectionPoint,
    /// The moving end of the selection.
    pub end: SelectionPoint,
    /// Selection mode (normal vs rectangular).
    pub mode: SelectionMode,
    /// Current state.
    pub state: SelectionState,
}

impl Selection {
    /// Start a new selection at the given point.
    pub fn start(point: SelectionPoint, mode: SelectionMode) -> Self {
        Self {
            start: point,
            end: point,
            mode,
            state: SelectionState::Selecting,
        }
    }

    /// Extend the selection to a new endpoint.
    pub fn extend(&mut self, point: SelectionPoint) {
        self.end = point;
    }

    /// Mark the selection as complete.
    pub fn finalize(&mut self) {
        self.state = SelectionState::Selected;
    }

    /// Return the ordered (top-left, bottom-right) bounds of the selection.
    pub fn ordered(&self) -> (SelectionPoint, SelectionPoint) {
        if self.start <= self.end {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        }
    }

    /// Return whether a given absolute row/col is inside the selection.
    pub fn contains(&self, row: usize, col: usize) -> bool {
        let (begin, end) = self.ordered();
        match self.mode {
            SelectionMode::Normal => {
                if row < begin.row || row > end.row {
                    return false;
                }
                if begin.row == end.row {
                    col >= begin.col && col <= end.col
                } else if row == begin.row {
                    col >= begin.col
                } else if row == end.row {
                    col <= end.col
                } else {
                    true
                }
            }
            SelectionMode::Rectangular => {
                let left = begin.col.min(end.col);
                let right = begin.col.max(end.col);
                row >= begin.row && row <= end.row && col >= left && col <= right
            }
        }
    }

    /// Extract the selected text from the grid.
    ///
    /// `scrollback_len` is `grid.scrollback_len()` so we can translate
    /// absolute row indices into scrollback vs viewport indices.
    pub fn get_text(&self, grid: &Grid) -> String {
        let (begin, end) = self.ordered();
        let sb_len = grid.scrollback_len();
        let mut result = String::new();

        match self.mode {
            SelectionMode::Normal => {
                for abs_row in begin.row..=end.row {
                    let row_text = row_full_text(grid, abs_row, sb_len);
                    let cols = row_text.chars().count();

                    let start_col = if abs_row == begin.row { begin.col } else { 0 };
                    let end_col = if abs_row == end.row {
                        end.col
                    } else {
                        cols.saturating_sub(1)
                    };

                    // Extract the relevant substring by char indices.
                    let chars: Vec<char> = row_text.chars().collect();
                    let sc = start_col.min(chars.len());
                    let ec = (end_col + 1).min(chars.len());
                    if sc < ec {
                        let slice: String = chars[sc..ec].iter().collect();
                        result.push_str(slice.trim_end());
                    }

                    if abs_row < end.row {
                        result.push('\n');
                    }
                }
            }
            SelectionMode::Rectangular => {
                let left = begin.col.min(self.start.col.min(self.end.col));
                let right = begin.col.max(self.start.col.max(self.end.col));
                for abs_row in begin.row..=end.row {
                    let row_text = row_full_text(grid, abs_row, sb_len);
                    let chars: Vec<char> = row_text.chars().collect();
                    let sc = left.min(chars.len());
                    let ec = (right + 1).min(chars.len());
                    if sc < ec {
                        let slice: String = chars[sc..ec].iter().collect();
                        result.push_str(slice.trim_end());
                    }
                    if abs_row < end.row {
                        result.push('\n');
                    }
                }
            }
        }

        result
    }
}

/// Get the full text of a row by its absolute index (scrollback + viewport).
fn row_full_text(grid: &Grid, abs_row: usize, scrollback_len: usize) -> String {
    if abs_row < scrollback_len {
        grid.scrollback_row_text_full(abs_row)
    } else {
        let vp_row = abs_row - scrollback_len;
        if vp_row < grid.rows() {
            grid.row_text_full(vp_row)
        } else {
            String::new()
        }
    }
}

/// Encode a string as base64 for OSC 52 clipboard.
pub fn osc52_clipboard(text: &str) -> Vec<u8> {
    let encoded = base64_encode(text.as_bytes());
    let mut seq = Vec::new();
    seq.extend_from_slice(b"\x1b]52;c;");
    seq.extend_from_slice(encoded.as_bytes());
    seq.extend_from_slice(b"\x1b\\");
    seq
}

/// Minimal base64 encoder (no external dependency needed).
fn base64_encode(input: &[u8]) -> String {
    const CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        out.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

// ─── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Grid;

    /// Helper: create a grid and fill viewport rows with given strings.
    fn grid_with_lines(lines: &[&str], cols: usize) -> Grid {
        let rows = lines.len();
        let mut grid = Grid::new(cols, rows);
        for (r, text) in lines.iter().enumerate() {
            for (c, ch) in text.chars().enumerate() {
                if c < cols {
                    grid.cell_mut(r, c).c = ch;
                }
            }
        }
        grid
    }

    #[test]
    fn selection_single_row() {
        let grid = grid_with_lines(&["Hello, world!"], 20);
        let sb = grid.scrollback_len();
        let mut sel = Selection::start(
            SelectionPoint::new(sb, 0),
            SelectionMode::Normal,
        );
        sel.extend(SelectionPoint::new(sb, 4));
        sel.finalize();
        assert_eq!(sel.get_text(&grid), "Hello");
    }

    #[test]
    fn selection_multi_row() {
        let grid = grid_with_lines(&["Line one", "Line two", "Line three"], 20);
        let sb = grid.scrollback_len();
        let mut sel = Selection::start(
            SelectionPoint::new(sb, 5),
            SelectionMode::Normal,
        );
        sel.extend(SelectionPoint::new(sb + 2, 3));
        sel.finalize();
        assert_eq!(sel.get_text(&grid), "one\nLine two\nLine");
    }

    #[test]
    fn selection_rectangular() {
        let grid = grid_with_lines(&["ABCDE", "FGHIJ", "KLMNO"], 10);
        let sb = grid.scrollback_len();
        let mut sel = Selection::start(
            SelectionPoint::new(sb, 1),
            SelectionMode::Rectangular,
        );
        sel.extend(SelectionPoint::new(sb + 2, 3));
        sel.finalize();
        assert_eq!(sel.get_text(&grid), "BCD\nGHI\nLMN");
    }

    #[test]
    fn selection_reversed_direction() {
        let grid = grid_with_lines(&["Hello, world!"], 20);
        let sb = grid.scrollback_len();
        // Start at end, extend to beginning.
        let mut sel = Selection::start(
            SelectionPoint::new(sb, 4),
            SelectionMode::Normal,
        );
        sel.extend(SelectionPoint::new(sb, 0));
        sel.finalize();
        assert_eq!(sel.get_text(&grid), "Hello");
    }

    #[test]
    fn selection_contains_normal() {
        let sel = Selection::start(
            SelectionPoint::new(1, 3),
            SelectionMode::Normal,
        );
        let mut sel = sel;
        sel.extend(SelectionPoint::new(3, 5));

        // Before selection
        assert!(!sel.contains(0, 0));
        // Start row, before start col
        assert!(!sel.contains(1, 2));
        // Start row, at start col
        assert!(sel.contains(1, 3));
        // Middle row
        assert!(sel.contains(2, 0));
        assert!(sel.contains(2, 100));
        // End row, at end col
        assert!(sel.contains(3, 5));
        // End row, after end col
        assert!(!sel.contains(3, 6));
        // After selection
        assert!(!sel.contains(4, 0));
    }

    #[test]
    fn selection_contains_rectangular() {
        let mut sel = Selection::start(
            SelectionPoint::new(1, 2),
            SelectionMode::Rectangular,
        );
        sel.extend(SelectionPoint::new(3, 5));

        assert!(!sel.contains(0, 3));
        assert!(sel.contains(1, 2));
        assert!(sel.contains(2, 4));
        assert!(sel.contains(3, 5));
        assert!(!sel.contains(2, 1));
        assert!(!sel.contains(2, 6));
        assert!(!sel.contains(4, 3));
    }

    #[test]
    fn selection_with_scrollback() {
        let mut grid = Grid::new(10, 3);
        // Fill viewport rows
        for (c, ch) in "Viewport 0".chars().enumerate() {
            if c < 10 {
                grid.cell_mut(0, c).c = ch;
            }
        }
        // Scroll up to push a line into scrollback
        grid.scroll_up(0, 3, 1);
        // The old row 0 ("Viewport 0") is now in scrollback
        // Fill the new row 0 (which is blank after scroll)
        for (c, ch) in "New row  0".chars().enumerate() {
            if c < 10 {
                grid.cell_mut(0, c).c = ch;
            }
        }

        let sb = grid.scrollback_len();
        assert_eq!(sb, 1);

        // Select from scrollback into viewport
        let mut sel = Selection::start(
            SelectionPoint::new(0, 0),
            SelectionMode::Normal,
        );
        sel.extend(SelectionPoint::new(sb, 2));
        sel.finalize();
        assert_eq!(sel.get_text(&grid), "Viewport 0\nNew");
    }

    #[test]
    fn selection_state_transitions() {
        let sel = Selection::start(
            SelectionPoint::new(0, 0),
            SelectionMode::Normal,
        );
        assert_eq!(sel.state, SelectionState::Selecting);

        let mut sel = sel;
        sel.extend(SelectionPoint::new(1, 5));
        assert_eq!(sel.state, SelectionState::Selecting);

        sel.finalize();
        assert_eq!(sel.state, SelectionState::Selected);
    }

    #[test]
    fn osc52_encoding() {
        let seq = osc52_clipboard("hello");
        let s = String::from_utf8(seq).unwrap();
        assert!(s.starts_with("\x1b]52;c;"));
        assert!(s.ends_with("\x1b\\"));
        // "hello" in base64 is "aGVsbG8="
        assert!(s.contains("aGVsbG8="));
    }

    #[test]
    fn base64_encode_empty() {
        assert_eq!(base64_encode(b""), "");
    }

    #[test]
    fn base64_encode_known_values() {
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn selection_empty_when_same_point() {
        let grid = grid_with_lines(&["Hello"], 10);
        let sb = grid.scrollback_len();
        let mut sel = Selection::start(
            SelectionPoint::new(sb, 2),
            SelectionMode::Normal,
        );
        sel.extend(SelectionPoint::new(sb, 2));
        sel.finalize();
        assert_eq!(sel.get_text(&grid), "l");
    }
}
