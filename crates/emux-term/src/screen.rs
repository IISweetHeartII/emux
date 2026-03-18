//! Screen state management (primary and alternate screens).
//!
//! [`Screen`] is the central type that owns the visible grid, cursor, drawing
//! attributes ("pen"), scroll region, tab stops, and the alternate screen
//! buffer.  It exposes the operations that higher layers (the VT performer,
//! the multiplexer) call to mutate terminal state: character output, cursor
//! movement, erasing, scrolling, and mode switching.
//!
//! Wide (CJK) characters, autowrap, insert mode, left/right margins
//! (DECSLRM), and origin mode (DECOM) are all handled here.

use emux_vt::Charset;
use serde::{Deserialize, Serialize};

use crate::color::Color;
use crate::cursor::{Cursor, SavedCursor};
use crate::grid::{CellAttrs, Grid};
use crate::modes::Modes;

const DEFAULT_TAB_INTERVAL: usize = 8;

/// Build the default 256-color palette.
fn default_palette() -> Vec<(u8, u8, u8)> {
    let mut p = Vec::with_capacity(256);
    // Standard ANSI colors 0-7
    let ansi = [
        (0, 0, 0), (205, 0, 0), (0, 205, 0), (205, 205, 0),
        (0, 0, 238), (205, 0, 205), (0, 205, 205), (229, 229, 229),
    ];
    // Bright ANSI colors 8-15
    let bright = [
        (127, 127, 127), (255, 0, 0), (0, 255, 0), (255, 255, 0),
        (92, 92, 255), (255, 0, 255), (0, 255, 255), (255, 255, 255),
    ];
    for &c in &ansi { p.push(c); }
    for &c in &bright { p.push(c); }
    // 216-color cube (indices 16-231)
    for r in 0..6u8 {
        for g in 0..6u8 {
            for b in 0..6u8 {
                let rv = if r == 0 { 0 } else { 55 + 40 * r };
                let gv = if g == 0 { 0 } else { 55 + 40 * g };
                let bv = if b == 0 { 0 } else { 55 + 40 * b };
                p.push((rv, gv, bv));
            }
        }
    }
    // Grayscale ramp (indices 232-255)
    for i in 0..24u8 {
        let v = 8 + 10 * i;
        p.push((v, v, v));
    }
    p
}

// ---------------------------------------------------------------------------
// Damage tracking
// ---------------------------------------------------------------------------

/// A damaged (changed) region of the screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DamageRegion {
    pub row: usize,
    pub col_start: usize,
    pub col_end: usize,
}

/// How damage events are coalesced before being reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum DamageMode {
    /// Individual cell-level rectangles.
    #[default]
    Cell,
    /// Each damage is expanded to the full row width.
    Row,
    /// Any damage marks the entire screen.
    Screen,
    /// Track scroll operations as scroll damage.
    Scroll,
}


/// Erase display mode (ED).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EraseDisplay {
    /// Erase from cursor to end of screen.
    Below = 0,
    /// Erase from start of screen to cursor.
    Above = 1,
    /// Erase entire screen.
    All = 2,
    /// Erase scrollback buffer.
    Scrollback = 3,
}

/// Erase line mode (EL).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EraseLine {
    /// Erase from cursor to end of line.
    ToRight = 0,
    /// Erase from start of line to cursor.
    ToLeft = 1,
    /// Erase entire line.
    All = 2,
}

/// Tab stop clear mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearTabStop {
    /// Clear tab stop at current column.
    Current = 0,
    /// Clear all tab stops.
    All = 3,
}

/// Terminal screen state combining grid, cursor, and modes.
///
/// A `Screen` manages two grids (primary and alternate), a cursor, drawing
/// attributes, scroll margins, tab stops, and character set state.  It is
/// the target of parsed VT actions via the [`Performer`](emux_vt::Performer)
/// trait implementation in [`performer`](crate::performer).
///
/// Create with [`Screen::new`], then feed input through the VT parser or
/// call mutation methods directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Screen {
    pub grid: Grid,
    pub cursor: Cursor,
    pub(crate) saved_cursor: Option<SavedCursor>,
    pub modes: Modes,
    pub(crate) scroll_top: usize,
    pub(crate) scroll_bottom: usize,
    /// Left margin (0-based, inclusive). 0 means no left margin.
    pub(crate) scroll_left: usize,
    /// Right margin (0-based, exclusive). cols means no right margin.
    pub(crate) scroll_right: usize,
    /// Current drawing attributes (the "pen").
    pub(crate) pen: CellAttrs,
    /// Current foreground color.
    pub(crate) fg: Color,
    /// Current background color.
    pub(crate) bg: Color,
    /// Tab stops: `tab_stops`[col] is true if there is a tab stop at column `col`.
    pub(crate) tab_stops: Vec<bool>,
    /// Window title.
    pub title: String,
    /// G0 character set.
    pub(crate) charset_g0: Charset,
    /// G1 character set.
    pub(crate) charset_g1: Charset,
    /// Active character set: 0 = G0, 1 = G1.
    pub(crate) active_charset: u8,
    /// Pending wrap state: cursor is logically past the last column,
    /// and will wrap on the next printable character.
    pending_wrap: bool,
    /// Last printed character (for REP sequence).
    pub(crate) last_char: Option<char>,
    /// Color palette (256 colors, initialized with default ANSI palette).
    pub(crate) palette: Vec<(u8, u8, u8)>,
    /// SGR attribute stack for XTPUSHSGR / XTPOPSGR.
    pub(crate) sgr_stack: Vec<(CellAttrs, Color, Color)>,
    /// Saved DEC private mode settings for XTSAVE / XTRESTORE.
    pub(crate) saved_modes: Option<Modes>,
    /// Current hyperlink URI (OSC 8).
    pub(crate) hyperlink: Option<String>,
    /// Whether bold causes ANSI colors 0-7 to map to 8-15.
    pub(crate) bold_is_bright: bool,
    /// Write-back buffer for terminal responses (DA, DSR, OSC queries).
    pub(crate) response_buf: Vec<u8>,

    /// Current working directory reported by OSC 7.
    pub working_directory: Option<String>,

    /// Viewport scroll offset: 0 = at bottom, >0 = scrolled up.
    pub(crate) viewport_offset: usize,

    // ── Alternate screen buffer ───────────────────────────────────────
    /// The alternate screen grid.
    alt_grid: Grid,
    /// Saved cursor for the inactive screen (main cursor saved when on alt,
    /// alt cursor saved when on main).
    alt_cursor: Cursor,
    /// Saved cursor state specifically for mode-1049 DECSC/DECRC behaviour.
    alt_saved_cursor: Option<SavedCursor>,

    // ── Search state ─────────────────────────────────────────────────
    /// Current search state (not serialized).
    #[serde(skip)]
    search: Option<SearchState>,

    // ── Damage tracking ─────────────────────────────────────────────
    /// Accumulated damage regions since last `take_damage()`.
    #[serde(skip)]
    damage_list: Vec<DamageRegion>,
    /// How damage is coalesced.
    #[serde(skip)]
    damage_mode: DamageMode,
}

impl Screen {
    /// Create a new screen with default tab stops every 8 columns.
    pub fn new(cols: usize, rows: usize) -> Self {
        let mut tab_stops = vec![false; cols];
        let mut col = DEFAULT_TAB_INTERVAL;
        while col < cols {
            tab_stops[col] = true;
            col += DEFAULT_TAB_INTERVAL;
        }

        Self {
            grid: Grid::new(cols, rows),
            cursor: Cursor::default(),
            saved_cursor: None,
            modes: Modes::default(),
            scroll_top: 0,
            scroll_bottom: rows,
            scroll_left: 0,
            scroll_right: cols,
            pen: CellAttrs::default(),
            fg: Color::Default,
            bg: Color::Default,
            tab_stops,
            title: String::new(),
            charset_g0: Charset::Ascii,
            charset_g1: Charset::Ascii,
            active_charset: 0,
            pending_wrap: false,
            last_char: None,
            palette: default_palette(),
            sgr_stack: Vec::new(),
            saved_modes: None,
            hyperlink: None,
            bold_is_bright: false,
            response_buf: Vec::new(),
            working_directory: None,
            viewport_offset: 0,
            alt_grid: Grid::new(cols, rows),
            alt_cursor: Cursor::default(),
            alt_saved_cursor: None,
            search: None,
            damage_list: Vec::new(),
            damage_mode: DamageMode::default(),
        }
    }

    /// Number of columns.
    pub fn cols(&self) -> usize {
        self.grid.cols()
    }

    /// Number of rows.
    pub fn rows(&self) -> usize {
        self.grid.rows()
    }

    /// Get the active charset.
    fn active_charset(&self) -> Charset {
        if self.active_charset == 1 {
            self.charset_g1
        } else {
            self.charset_g0
        }
    }

    /// Map a character through the active charset.
    fn map_char(&self, c: char) -> char {
        if c as u32 <= 0x7e {
            self.active_charset().map(c as u8)
        } else {
            c
        }
    }

    /// Print a character at the cursor position and advance the cursor.
    /// Handles autowrap, wide characters, and insert mode.
    pub fn write_char(&mut self, c: char) {
        // Snap viewport to bottom on new output
        self.viewport_offset = 0;

        let c = self.map_char(c);
        self.last_char = Some(c);
        let width = char_width(c);
        let cols = self.cols();
        let rows = self.rows();
        let right_edge = self.effective_right();

        if cols == 0 || rows == 0 {
            return;
        }

        // Zero-width characters: skip (combining chars handled separately in the future)
        if width == 0 {
            return;
        }

        // Handle pending wrap
        if self.pending_wrap {
            if self.modes.autowrap {
                // Mark the current row as having wrapped
                self.grid.row_mut(self.cursor.row).flags.continuation = false;
                self.carriage_return();
                self.linefeed();
                // Mark the new row as a continuation
                self.grid.row_mut(self.cursor.row).flags.continuation = true;
            } else {
                // Without autowrap, stay at last column
                self.cursor.col = right_edge - 1;
            }
            self.pending_wrap = false;
        }

        // Handle wide char that doesn't fit on current line
        if width == 2 && self.cursor.col >= right_edge - 1 {
            if self.modes.autowrap {
                // Leave a blank padding cell at the end of this line
                let row = self.cursor.row;
                let col = self.cursor.col;
                if col < cols {
                    let cell = self.grid.cell_mut(row, col);
                    cell.c = ' ';
                    cell.width = 1;
                }
                self.carriage_return();
                self.linefeed();
                self.grid.row_mut(self.cursor.row).flags.continuation = true;
            } else {
                self.cursor.col = right_edge.saturating_sub(2);
                if self.cursor.col >= cols {
                    self.cursor.col = 0;
                }
            }
        }

        // Insert mode: shift cells right (bounded by right margin)
        if self.modes.insert {
            let right = self.effective_right();
            self.grid.insert_cells_bounded(self.cursor.row, self.cursor.col, width as usize, right);
        }

        let row = self.cursor.row;
        let col = self.cursor.col;

        // Clean up wide char fragments before overwriting.
        if col < cols {
            let existing_width = self.grid.cell(row, col).width;
            // Overwriting the head of a wide char: clear the spacer tail
            if existing_width == 2 && col + 1 < cols {
                let spacer = self.grid.cell_mut(row, col + 1);
                spacer.c = ' ';
                spacer.width = 1;
            }
            // Overwriting a spacer tail (width == 0): clear the wide char head
            if existing_width == 0 && col > 0 {
                let head = self.grid.cell_mut(row, col - 1);
                if head.width == 2 {
                    head.c = ' ';
                    head.width = 1;
                }
            }
        }
        // For wide chars being written, check if continuation cell is a wide char head
        if width == 2 && col + 1 < cols {
            let next_width = self.grid.cell(row, col + 1).width;
            if next_width == 2 && col + 2 < cols {
                let spacer = self.grid.cell_mut(row, col + 2);
                spacer.c = ' ';
                spacer.width = 1;
            }
        }

        // Resolve effective fg (bold+bright mapping)
        let effective_fg = self.effective_fg();

        // Write the cell
        if col < cols {
            let cell = self.grid.cell_mut(row, col);
            cell.c = c;
            cell.width = width;
            cell.fg = effective_fg;
            cell.bg = self.bg;
            cell.attrs = self.pen;
            cell.hyperlink = self.hyperlink.clone();
        }

        // For wide chars, write continuation cell
        if width == 2 && col + 1 < cols {
            let cell = self.grid.cell_mut(row, col + 1);
            cell.c = ' ';
            cell.width = 0; // continuation marker
            cell.fg = effective_fg;
            cell.bg = self.bg;
            cell.attrs = self.pen;
            cell.hyperlink = self.hyperlink.clone();
        }

        // Record damage for the written cell(s)
        self.record_damage(row, col, col + width as usize);

        // Advance cursor
        let new_col = col + width as usize;
        if new_col >= right_edge {
            // At or past right margin
            if self.modes.autowrap {
                self.cursor.col = right_edge - 1;
                self.pending_wrap = true;
            } else {
                self.cursor.col = right_edge - 1;
            }
        } else {
            self.cursor.col = new_col;
        }
    }

    /// Line feed: move cursor down, scroll if at bottom of scroll region.
    pub fn linefeed(&mut self) {
        self.pending_wrap = false;

        if self.cursor.row + 1 == self.scroll_bottom {
            // At bottom of scroll region, scroll up
            self.record_scroll_damage(self.scroll_top, self.scroll_bottom);
            if self.has_lr_margins() && self.cursor_in_lr_margins() {
                self.grid.scroll_up_region(
                    self.scroll_top, self.scroll_bottom,
                    self.scroll_left, self.scroll_right, 1,
                );
            } else {
                self.grid.scroll_up(self.scroll_top, self.scroll_bottom, 1);
            }
        } else if self.cursor.row + 1 < self.rows() {
            self.cursor.row += 1;
        }

        // In newline mode, also do carriage return
        if self.modes.newline {
            self.cursor.col = 0;
        }
    }

    /// Carriage return: move cursor to left margin (or column 0 if cursor
    /// is left of the left margin or no left/right margins are active).
    pub fn carriage_return(&mut self) {
        if self.has_lr_margins() && self.cursor.col >= self.scroll_left {
            self.cursor.col = self.scroll_left;
        } else {
            self.cursor.col = 0;
        }
        self.pending_wrap = false;
    }

    /// Backspace: move cursor left by one.
    /// With reverse wrap mode (DECSET 45) and autowrap enabled, wraps to the
    /// last column of the previous line when at column 0.
    pub fn backspace(&mut self) {
        if self.cursor.col > 0 {
            self.cursor.col -= 1;
            self.pending_wrap = false;
        } else if self.modes.reverse_wrap && self.modes.autowrap && self.cursor.row > 0 {
            self.cursor.row -= 1;
            self.cursor.col = self.cols().saturating_sub(1);
            self.pending_wrap = false;
        }
    }

    /// Tab: move cursor to next tab stop.
    pub fn tab(&mut self) {
        let cols = self.cols();
        let start = self.cursor.col + 1;
        self.pending_wrap = false;

        for c in start..cols {
            if self.tab_stops.get(c).copied().unwrap_or(false) {
                self.cursor.col = c;
                return;
            }
        }
        // No more tab stops, move to last column
        self.cursor.col = cols.saturating_sub(1);
    }

    /// Move cursor forward by `count` tab stops (CHT).
    pub fn tab_forward(&mut self, count: usize) {
        for _ in 0..count {
            self.tab();
        }
    }

    /// Move cursor backward by `count` tab stops (CBT).
    pub fn tab_backward(&mut self, count: usize) {
        self.pending_wrap = false;
        for _ in 0..count {
            if self.cursor.col == 0 {
                break;
            }
            let start = self.cursor.col - 1;
            let mut found = false;
            for c in (0..=start).rev() {
                if self.tab_stops.get(c).copied().unwrap_or(false) {
                    self.cursor.col = c;
                    found = true;
                    break;
                }
            }
            if !found {
                self.cursor.col = 0;
            }
        }
    }

    /// Move cursor up by `count` rows, clamping to scroll region top.
    /// If the cursor is inside the scroll region, it clamps to `scroll_top`.
    /// If the cursor is above the scroll region, it clamps to row 0.
    pub fn cursor_up(&mut self, count: usize) {
        let top = if self.cursor.row >= self.scroll_top && self.cursor.row < self.scroll_bottom {
            self.scroll_top
        } else {
            0
        };
        let new_row = self.cursor.row.saturating_sub(count);
        self.cursor.row = new_row.max(top);
        self.pending_wrap = false;
    }

    /// Move cursor down by `count` rows, clamping to scroll region bottom.
    /// If the cursor is inside the scroll region, it clamps to `scroll_bottom` - 1.
    /// If the cursor is below the scroll region, it clamps to the screen bottom.
    pub fn cursor_down(&mut self, count: usize) {
        let bottom = if self.cursor.row >= self.scroll_top && self.cursor.row < self.scroll_bottom {
            self.scroll_bottom.saturating_sub(1)
        } else {
            self.rows().saturating_sub(1)
        };
        let new_row = self.cursor.row + count;
        self.cursor.row = new_row.min(bottom);
        self.pending_wrap = false;
    }

    /// Move cursor left by `count` columns.
    pub fn cursor_left(&mut self, count: usize) {
        self.cursor.col = self.cursor.col.saturating_sub(count);
        self.pending_wrap = false;
    }

    /// Move cursor right by `count` columns, clamping to last column.
    pub fn cursor_right(&mut self, count: usize) {
        let max_col = self.cols().saturating_sub(1);
        self.cursor.col = (self.cursor.col + count).min(max_col);
        self.pending_wrap = false;
    }

    /// Set cursor position (CUP/HVP). Input is 1-based.
    /// Row and column are clamped to screen bounds.
    /// In origin mode, row and column are relative to the scroll region margins.
    pub fn cursor_position(&mut self, row: usize, col: usize) {
        let row = row.saturating_sub(1); // Convert to 0-based
        let col = col.saturating_sub(1);

        if self.modes.origin {
            let abs_row = (self.scroll_top + row).min(self.scroll_bottom.saturating_sub(1));
            self.cursor.row = abs_row;
            let abs_col = (self.scroll_left + col).min(self.scroll_right.saturating_sub(1));
            self.cursor.col = abs_col;
        } else {
            self.cursor.row = row.min(self.rows().saturating_sub(1));
            self.cursor.col = col.min(self.cols().saturating_sub(1));
        }

        self.pending_wrap = false;
    }

    /// Erase display (ED).
    pub fn erase_display(&mut self, mode: EraseDisplay) {
        self.erase_display_impl(mode, false);
    }

    /// Selective erase in display (DECSED) — like ED but skips protected cells.
    pub fn selective_erase_display(&mut self, mode: EraseDisplay) {
        self.erase_display_impl(mode, true);
    }

    /// Shared implementation for erase_display and selective_erase_display.
    fn erase_display_impl(&mut self, mode: EraseDisplay, selective: bool) {
        let rows = self.rows();
        let cols = self.cols();
        match mode {
            EraseDisplay::Below => {
                self.record_damage(self.cursor.row, self.cursor.col, cols);
                for r in (self.cursor.row + 1)..rows {
                    self.record_damage(r, 0, cols);
                }
                self.clear_region_impl(
                    self.cursor.row,
                    self.cursor.col,
                    self.cursor.row,
                    cols.saturating_sub(1),
                    selective,
                );
                for r in (self.cursor.row + 1)..rows {
                    self.clear_row_impl(r, selective);
                }
            }
            EraseDisplay::Above => {
                for r in 0..self.cursor.row {
                    self.record_damage(r, 0, cols);
                    self.clear_row_impl(r, selective);
                }
                self.record_damage(self.cursor.row, 0, self.cursor.col + 1);
                self.clear_region_impl(self.cursor.row, 0, self.cursor.row, self.cursor.col, selective);
            }
            EraseDisplay::All => {
                for r in 0..rows {
                    self.record_damage(r, 0, cols);
                    self.clear_row_impl(r, selective);
                }
            }
            EraseDisplay::Scrollback => {
                while self.grid.pop_scrollback().is_some() {}
            }
        }
    }

    /// Erase line (EL).
    pub fn erase_line(&mut self, mode: EraseLine) {
        self.erase_line_impl(mode, false);
    }

    /// Selective erase in line (DECSEL) — like EL but skips protected cells.
    pub fn selective_erase_line(&mut self, mode: EraseLine) {
        self.erase_line_impl(mode, true);
    }

    /// Shared implementation for erase_line and selective_erase_line.
    fn erase_line_impl(&mut self, mode: EraseLine, selective: bool) {
        let cols = self.cols();
        let row = self.cursor.row;
        match mode {
            EraseLine::ToRight => {
                self.record_damage(row, self.cursor.col, cols);
                self.clear_region_impl(row, self.cursor.col, row, cols.saturating_sub(1), selective);
            }
            EraseLine::ToLeft => {
                self.record_damage(row, 0, self.cursor.col + 1);
                self.clear_region_impl(row, 0, row, self.cursor.col, selective);
            }
            EraseLine::All => {
                self.record_damage(row, 0, cols);
                self.clear_row_impl(row, selective);
            }
        }
    }

    /// Clear a rectangular region. If `selective`, skip protected cells.
    fn clear_region_impl(&mut self, top: usize, left: usize, bottom: usize, right: usize, selective: bool) {
        if selective {
            let bottom = bottom.min(self.rows().saturating_sub(1));
            let right = right.min(self.cols().saturating_sub(1));
            for r in top..=bottom {
                for c in left..=right {
                    let cell = self.grid.cell_mut(r, c);
                    if !cell.attrs.protected {
                        cell.reset();
                    }
                }
            }
        } else {
            self.grid.clear_region(top, left, bottom, right);
        }
    }

    /// Clear a row. If `selective`, skip protected cells.
    fn clear_row_impl(&mut self, row: usize, selective: bool) {
        if selective {
            let cols = self.cols();
            for c in 0..cols {
                let cell = self.grid.cell_mut(row, c);
                if !cell.attrs.protected {
                    cell.reset();
                }
            }
        } else {
            self.grid.clear_row(row);
        }
    }

    /// Scroll the scroll region up by `count` lines.
    pub fn scroll_up(&mut self, count: usize) {
        self.record_scroll_damage(self.scroll_top, self.scroll_bottom);
        if self.has_lr_margins() {
            self.grid.scroll_up_region(
                self.scroll_top, self.scroll_bottom,
                self.scroll_left, self.scroll_right, count,
            );
        } else {
            self.grid.scroll_up(self.scroll_top, self.scroll_bottom, count);
        }
    }

    /// Scroll the scroll region down by `count` lines.
    pub fn scroll_down(&mut self, count: usize) {
        self.record_scroll_damage(self.scroll_top, self.scroll_bottom);
        if self.has_lr_margins() {
            self.grid.scroll_down_region(
                self.scroll_top, self.scroll_bottom,
                self.scroll_left, self.scroll_right, count,
            );
        } else {
            self.grid.scroll_down(self.scroll_top, self.scroll_bottom, count);
        }
    }

    /// Insert `count` blank lines at the cursor row within the scroll region.
    pub fn insert_lines(&mut self, count: usize) {
        let row = self.cursor.row;
        if row >= self.scroll_top && row < self.scroll_bottom {
            self.grid.insert_lines(row, count, self.scroll_top, self.scroll_bottom);
        }
        self.cursor.col = 0;
        self.pending_wrap = false;
    }

    /// Delete `count` lines at the cursor row within the scroll region.
    pub fn delete_lines(&mut self, count: usize) {
        let row = self.cursor.row;
        if row >= self.scroll_top && row < self.scroll_bottom {
            self.grid.delete_lines(row, count, self.scroll_top, self.scroll_bottom);
        }
        self.cursor.col = 0;
        self.pending_wrap = false;
    }

    /// Insert `count` blank characters at cursor position (ICH).
    pub fn insert_chars(&mut self, count: usize) {
        let right = self.effective_right();
        let row = self.cursor.row;
        self.record_damage(row, self.cursor.col, right);
        self.grid.insert_cells_bounded(row, self.cursor.col, count, right);
        self.pending_wrap = false;
    }

    /// Delete `count` characters at cursor position (DCH).
    pub fn delete_chars(&mut self, count: usize) {
        let right = self.effective_right();
        let row = self.cursor.row;
        self.record_damage(row, self.cursor.col, right);
        self.grid.delete_cells_bounded(row, self.cursor.col, count, right);
        self.pending_wrap = false;
    }

    /// Erase `count` characters starting at cursor position (ECH).
    pub fn erase_chars(&mut self, count: usize) {
        let row = self.cursor.row;
        let col = self.cursor.col;
        let cols = self.cols();
        self.record_damage(row, col, (col + count).min(cols));
        self.grid.erase_chars(row, col, count);
        self.pending_wrap = false;
    }

    /// Save cursor state (DECSC).
    pub fn save_cursor(&mut self) {
        self.saved_cursor = Some(SavedCursor {
            row: self.cursor.row,
            col: self.cursor.col,
            attrs: self.pen,
            fg: self.fg,
            bg: self.bg,
            charset_g0: self.charset_g0,
            charset_g1: self.charset_g1,
            active_charset: self.active_charset,
            origin_mode: self.modes.origin,
            pending_wrap: self.pending_wrap,
        });
    }

    /// Restore cursor state (DECRC).
    pub fn restore_cursor(&mut self) {
        if let Some(ref saved) = self.saved_cursor {
            self.cursor.row = saved.row.min(self.rows().saturating_sub(1));
            self.cursor.col = saved.col.min(self.cols().saturating_sub(1));
            self.pen = saved.attrs;
            self.fg = saved.fg;
            self.bg = saved.bg;
            self.charset_g0 = saved.charset_g0;
            self.charset_g1 = saved.charset_g1;
            self.active_charset = saved.active_charset;
            self.modes.origin = saved.origin_mode;
            self.pending_wrap = saved.pending_wrap;
        } else {
            // If no saved cursor, reset to home position with defaults
            self.cursor.row = 0;
            self.cursor.col = 0;
            self.pen = CellAttrs::default();
            self.fg = Color::Default;
            self.bg = Color::Default;
            self.pending_wrap = false;
        }
    }

    /// Set scroll region (DECSTBM). Input is 1-based.
    /// top=0, bottom=0 means reset to full screen.
    pub fn set_scroll_region(&mut self, top: usize, bottom: usize) {
        let rows = self.rows();
        let top = if top == 0 { 1 } else { top };
        let bottom = if bottom == 0 { rows } else { bottom };

        // Convert to 0-based, clamp
        let top = (top - 1).min(rows.saturating_sub(1));
        let bottom = bottom.min(rows);

        // Region must be at least 2 lines
        if bottom > top + 1 {
            self.scroll_top = top;
            self.scroll_bottom = bottom;
        }

        // DECSTBM also moves cursor to home (within origin)
        if self.modes.origin {
            self.cursor.row = self.scroll_top;
            self.cursor.col = self.scroll_left;
        } else {
            self.cursor.row = 0;
            self.cursor.col = 0;
        }
        self.pending_wrap = false;
    }

    /// Set left and right margins (DECSLRM). Input is 1-based.
    /// Only effective when DECLRMM (mode 69) is enabled.
    pub fn set_left_right_margin(&mut self, left: usize, right: usize) {
        if !self.modes.left_right_margin {
            return;
        }
        let cols = self.cols();
        let left = if left == 0 { 1 } else { left };
        let right = if right == 0 { cols } else { right };

        // Convert to 0-based
        let left = (left - 1).min(cols.saturating_sub(1));
        let right = right.min(cols);

        // Region must be at least 2 columns wide
        if right > left + 1 {
            self.scroll_left = left;
            self.scroll_right = right;
        }

        // DECSLRM moves cursor to home (within origin)
        if self.modes.origin {
            self.cursor.row = self.scroll_top;
            self.cursor.col = self.scroll_left;
        } else {
            self.cursor.row = 0;
            self.cursor.col = 0;
        }
        self.pending_wrap = false;
    }

    /// Check whether left/right margins are active (non-default).
    pub fn has_lr_margins(&self) -> bool {
        self.modes.left_right_margin && (self.scroll_left != 0 || self.scroll_right != self.cols())
    }

    /// Effective right margin for the current line (exclusive).
    fn effective_right(&self) -> usize {
        if self.has_lr_margins() {
            self.scroll_right
        } else {
            self.cols()
        }
    }

    /// Check if cursor column is within the left/right margin area.
    fn cursor_in_lr_margins(&self) -> bool {
        !self.has_lr_margins()
            || (self.cursor.col >= self.scroll_left && self.cursor.col < self.scroll_right)
    }

    // ── Alternate screen buffer ─────────────────────────────────────

    /// Enter the alternate screen buffer (modes 47 / 1047 / 1049).
    ///
    /// * Swaps `grid` and `alt_grid` so the primary content is preserved.
    /// * Copies the current cursor into `alt_cursor` (saves primary cursor).
    /// * Clears the (now active) alternate grid.
    /// * If `save_cursor` is true (mode 1049), also performs DECSC.
    pub fn enter_alt_screen(&mut self, save_cursor_decsc: bool) {
        if self.modes.alt_screen {
            return; // already on alt screen
        }
        self.modes.alt_screen = true;

        if save_cursor_decsc {
            self.save_cursor(); // DECSC
        }

        // Save primary cursor, swap grids
        std::mem::swap(&mut self.cursor, &mut self.alt_cursor);
        std::mem::swap(&mut self.grid, &mut self.alt_grid);

        // Clear the alternate grid and reset cursor to home
        let rows = self.rows();
        for r in 0..rows {
            self.grid.clear_row(r);
        }
        self.cursor = Cursor::default();
        self.cursor.visible = true;
        self.pending_wrap = false;
        self.record_full_damage();
    }

    /// Leave the alternate screen buffer.
    ///
    /// * Swaps grids back so the primary content is restored.
    /// * Restores the primary cursor from `alt_cursor`.
    /// * If `restore_cursor` is true (mode 1049), also performs DECRC.
    pub fn leave_alt_screen(&mut self, restore_cursor_decrc: bool) {
        if !self.modes.alt_screen {
            return; // already on main screen
        }
        self.modes.alt_screen = false;

        // Swap grids back (primary grid returns to `self.grid`)
        std::mem::swap(&mut self.grid, &mut self.alt_grid);
        std::mem::swap(&mut self.cursor, &mut self.alt_cursor);
        self.pending_wrap = false;

        if restore_cursor_decrc {
            self.restore_cursor(); // DECRC
        }
        self.record_full_damage();
    }

    /// Full terminal reset (RIS).
    pub fn reset(&mut self) {
        let cols = self.cols();
        let rows = self.rows();
        *self = Self::new(cols, rows);
    }

    /// Set tab stop at current cursor column (HTS).
    pub fn set_tab_stop(&mut self) {
        let col = self.cursor.col;
        if col < self.tab_stops.len() {
            self.tab_stops[col] = true;
        }
    }

    /// Clear tab stop(s) (TBC).
    pub fn clear_tab_stop(&mut self, mode: ClearTabStop) {
        match mode {
            ClearTabStop::Current => {
                let col = self.cursor.col;
                if col < self.tab_stops.len() {
                    self.tab_stops[col] = false;
                }
            }
            ClearTabStop::All => {
                for ts in &mut self.tab_stops {
                    *ts = false;
                }
            }
        }
    }

    /// Resize the screen.
    pub fn resize(&mut self, new_cols: usize, new_rows: usize) {
        let old_rows = self.rows();
        let old_cols = self.cols();

        // Reflow main grid with cursor tracking
        let cursor_delta = self.grid.resize_with_cursor(
            new_cols,
            new_rows,
            Some((self.cursor.row, self.cursor.col)),
        );

        // Apply cursor delta from reflow
        let new_cursor_row = (self.cursor.row as isize + cursor_delta)
            .max(0) as usize;
        self.cursor.row = new_cursor_row;

        // Alt grid: no reflow (alt screen doesn't reflow)
        self.alt_grid.resize(new_cols, new_rows);

        // Adjust scroll region
        let was_full_tb = self.scroll_top == 0 && self.scroll_bottom == old_rows;
        if was_full_tb {
            self.scroll_top = 0;
            self.scroll_bottom = new_rows;
        } else {
            self.scroll_bottom = self.scroll_bottom.min(new_rows);
            self.scroll_top = self.scroll_top.min(self.scroll_bottom.saturating_sub(1));
            if self.scroll_bottom == 0 {
                self.scroll_bottom = new_rows;
            }
        }

        // Adjust left/right margins
        let was_full_lr = self.scroll_left == 0 && self.scroll_right == old_cols;
        if was_full_lr {
            self.scroll_left = 0;
            self.scroll_right = new_cols;
        } else {
            self.scroll_right = self.scroll_right.min(new_cols);
            self.scroll_left = self.scroll_left.min(self.scroll_right.saturating_sub(1));
            if self.scroll_right == 0 {
                self.scroll_right = new_cols;
            }
        }

        // Adjust cursor
        self.cursor.row = self.cursor.row.min(new_rows.saturating_sub(1));
        self.cursor.col = self.cursor.col.min(new_cols.saturating_sub(1));

        // Adjust saved cursor position to fit within new dimensions
        if let Some(ref mut saved) = self.saved_cursor {
            saved.row = saved.row.min(new_rows.saturating_sub(1));
            saved.col = saved.col.min(new_cols.saturating_sub(1));
        }

        // Adjust tab stops
        let old_len = self.tab_stops.len();
        self.tab_stops.resize(new_cols, false);
        // Set default tab stops in the new area
        for c in old_len..new_cols {
            if c % DEFAULT_TAB_INTERVAL == 0 && c > 0 {
                self.tab_stops[c] = true;
            }
        }

        self.pending_wrap = false;
    }

    /// Get text content of a row (trimming trailing spaces).
    pub fn row_text(&self, row: usize) -> String {
        self.grid.row_text(row)
    }

    /// Reverse index: move cursor up, scroll down if at top of scroll region.
    pub fn reverse_index(&mut self) {
        if self.cursor.row == self.scroll_top {
            if self.has_lr_margins() && self.cursor_in_lr_margins() {
                self.grid.scroll_down_region(
                    self.scroll_top, self.scroll_bottom,
                    self.scroll_left, self.scroll_right, 1,
                );
            } else {
                self.grid.scroll_down(self.scroll_top, self.scroll_bottom, 1);
            }
        } else if self.cursor.row > 0 {
            self.cursor.row -= 1;
        }
        self.pending_wrap = false;
    }

    /// Index: move cursor down, scroll up if at bottom of scroll region.
    pub fn index(&mut self) {
        self.linefeed();
    }

    /// Check if the cursor is in pending wrap state.
    pub fn is_pending_wrap(&self) -> bool {
        self.pending_wrap
    }

    /// Clear pending wrap state.
    pub fn clear_pending_wrap(&mut self) {
        self.pending_wrap = false;
    }

    /// Soft terminal reset (DECSTR / CSI ! p).
    /// Resets modes, pen, charset but keeps grid content.
    pub fn soft_reset(&mut self) {
        self.modes = Modes::default();
        self.pen = CellAttrs::default();
        self.fg = Color::Default;
        self.bg = Color::Default;
        self.charset_g0 = Charset::Ascii;
        self.charset_g1 = Charset::Ascii;
        self.active_charset = 0;
        self.saved_cursor = None;
        self.pending_wrap = false;
        self.cursor.visible = true;
        self.cursor.shape = crate::cursor::CursorShape::Block;
        self.scroll_top = 0;
        self.scroll_bottom = self.rows();
        self.scroll_left = 0;
        self.scroll_right = self.cols();
    }

    /// DECALN: Screen alignment test pattern.
    /// Fills entire screen with 'E', resets scroll margins, moves cursor to (0,0).
    pub fn decaln(&mut self) {
        let cols = self.cols();
        let rows = self.rows();

        // Fill entire screen with 'E'
        for r in 0..rows {
            for c in 0..cols {
                let cell = self.grid.cell_mut(r, c);
                cell.c = 'E';
                cell.width = 1;
                cell.fg = Color::Default;
                cell.bg = Color::Default;
                cell.attrs = CellAttrs::default();
            }
        }

        // Reset scroll margins
        self.scroll_top = 0;
        self.scroll_bottom = rows;

        // Move cursor to home
        self.cursor.row = 0;
        self.cursor.col = 0;
        self.pending_wrap = false;
    }

    /// Set cursor style (DECSCUSR).
    /// 0 = default (blinking block), 1 = blinking block, 2 = steady block,
    /// 3 = blinking underline, 4 = steady underline, 5 = blinking bar, 6 = steady bar.
    pub fn set_cursor_style(&mut self, style: u16) {
        use crate::cursor::CursorShape;
        self.cursor.shape = match style {
            3 | 4 => CursorShape::Underline,
            5 | 6 => CursorShape::Bar,
            _ => CursorShape::Block,
        };
    }

    /// Current drawing attributes (the "pen").
    pub fn pen(&self) -> &CellAttrs {
        &self.pen
    }

    /// Current foreground color (with bold-is-bright mapping applied).
    pub fn fg(&self) -> Color {
        self.effective_fg()
    }

    /// Compute effective foreground color: when bold_is_bright is enabled and
    /// the pen is bold, ANSI colors 0-7 map to their bright counterparts 8-15.
    fn effective_fg(&self) -> Color {
        if self.bold_is_bright && self.pen.bold
            && let Color::Indexed(idx) = self.fg
                && idx < 8 {
                    return Color::Indexed(idx + 8);
                }
        self.fg
    }

    /// Current background color.
    pub fn bg(&self) -> Color {
        self.bg
    }

    /// Top row of the scroll region (0-based, inclusive).
    pub fn scroll_top(&self) -> usize {
        self.scroll_top
    }

    /// Bottom row of the scroll region (0-based, exclusive).
    pub fn scroll_bottom(&self) -> usize {
        self.scroll_bottom
    }

    /// Whether there is a saved cursor state (DECSC).
    pub fn has_saved_cursor(&self) -> bool {
        self.saved_cursor.is_some()
    }

    /// Scroll viewport up by `n` lines (viewing history).
    pub fn scroll_viewport_up(&mut self, n: usize) {
        let max = self.grid.scrollback_len();
        self.viewport_offset = (self.viewport_offset + n).min(max);
    }

    /// Current viewport scroll offset (0 = at bottom).
    pub fn viewport_offset(&self) -> usize {
        self.viewport_offset
    }

    /// Drain the response buffer (terminal -> host responses like DA, DSR, OSC query replies).
    pub fn drain_response(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.response_buf)
    }

    /// Set whether bold causes ANSI colors 0-7 to brighten to 8-15.
    pub fn set_bold_is_bright(&mut self, val: bool) {
        self.bold_is_bright = val;
    }

    /// Get the palette color at index.
    pub fn palette_color(&self, idx: u8) -> (u8, u8, u8) {
        self.palette.get(idx as usize).copied().unwrap_or((0, 0, 0))
    }

    /// Set a palette color.
    pub fn set_palette_color(&mut self, idx: u8, r: u8, g: u8, b: u8) {
        if (idx as usize) < self.palette.len() {
            self.palette[idx as usize] = (r, g, b);
        }
    }

    /// Push SGR state onto stack (XTPUSHSGR).
    pub fn push_sgr(&mut self) {
        self.sgr_stack.push((self.pen, self.fg, self.bg));
    }

    /// Pop SGR state from stack (XTPOPSGR).
    pub fn pop_sgr(&mut self) {
        if let Some((attrs, fg, bg)) = self.sgr_stack.pop() {
            let protected = self.pen.protected;
            self.pen = attrs;
            self.pen.protected = protected;
            self.fg = fg;
            self.bg = bg;
        }
    }

    /// Save current DEC private mode settings (XTSAVE).
    pub fn save_dec_modes(&mut self) {
        self.saved_modes = Some(self.modes.clone());
    }

    /// Restore saved DEC private mode settings (XTRESTORE).
    pub fn restore_dec_modes(&mut self) {
        if let Some(modes) = self.saved_modes.take() {
            self.modes = modes;
        }
    }

    /// Fill rectangular area with a character (DECFRA).
    /// Parameters are 1-based: top, left, bottom, right.
    pub fn fill_rect(&mut self, ch: char, top: usize, left: usize, bottom: usize, right: usize) {
        let rows = self.rows();
        let cols = self.cols();
        let top = top.saturating_sub(1).min(rows.saturating_sub(1));
        let left = left.saturating_sub(1).min(cols.saturating_sub(1));
        let bottom = bottom.min(rows);
        let right = right.min(cols);
        for r in top..bottom {
            for c in left..right {
                let cell = self.grid.cell_mut(r, c);
                cell.c = ch;
                cell.width = 1;
                cell.fg = self.fg;
                cell.bg = self.bg;
                cell.attrs = self.pen;
            }
        }
    }

    /// Copy rectangular area (DECCRA).
    /// All parameters are 1-based.
    pub fn copy_rect(
        &mut self,
        src_top: usize, src_left: usize, src_bottom: usize, src_right: usize,
        dst_top: usize, dst_left: usize,
    ) {
        let rows = self.rows();
        let cols = self.cols();
        let st = src_top.saturating_sub(1).min(rows.saturating_sub(1));
        let sl = src_left.saturating_sub(1).min(cols.saturating_sub(1));
        let sb = src_bottom.min(rows);
        let sr = src_right.min(cols);
        let dt = dst_top.saturating_sub(1).min(rows.saturating_sub(1));
        let dl = dst_left.saturating_sub(1).min(cols.saturating_sub(1));

        let h = sb.saturating_sub(st);
        let w = sr.saturating_sub(sl);
        if h == 0 || w == 0 {
            return;
        }

        // Copy to a temp buffer to handle overlap
        let mut buf = Vec::with_capacity(h * w);
        for r in st..sb {
            for c in sl..sr {
                if r < rows && c < cols {
                    buf.push(self.grid.cell(r, c).clone());
                } else {
                    buf.push(crate::grid::Cell::default());
                }
            }
        }

        // Paste from temp buffer
        for dr in 0..h {
            for dc in 0..w {
                let tr = dt + dr;
                let tc = dl + dc;
                if tr < rows && tc < cols {
                    *self.grid.cell_mut(tr, tc) = buf[dr * w + dc].clone();
                }
            }
        }
    }

    /// Selective erase rectangular area (DECSERA).
    /// Parameters are 1-based. Erases non-protected cells in the rect.
    pub fn selective_erase_rect(&mut self, top: usize, left: usize, bottom: usize, right: usize) {
        let rows = self.rows();
        let cols = self.cols();
        let top = top.saturating_sub(1).min(rows.saturating_sub(1));
        let left = left.saturating_sub(1).min(cols.saturating_sub(1));
        let bottom = bottom.min(rows);
        let right = right.min(cols);
        for r in top..bottom {
            for c in left..right {
                let cell = self.grid.cell_mut(r, c);
                if !cell.attrs.protected {
                    cell.reset();
                }
            }
        }
    }
    // ── Damage tracking ─────────────────────────────────────────────

    /// Record damage for a cell-level change.
    fn record_damage(&mut self, row: usize, col_start: usize, col_end: usize) {
        let cols = self.cols();
        let rows = self.rows();
        if row >= rows {
            return;
        }
        match self.damage_mode {
            DamageMode::Cell => {
                self.damage_list.push(DamageRegion {
                    row,
                    col_start,
                    col_end: col_end.min(cols),
                });
            }
            DamageMode::Row => {
                self.damage_list.push(DamageRegion {
                    row,
                    col_start: 0,
                    col_end: cols,
                });
            }
            DamageMode::Screen => {
                // Any damage marks the whole screen; coalesce to one region on take.
                self.damage_list.push(DamageRegion {
                    row: 0,
                    col_start: 0,
                    col_end: cols,
                });
            }
            DamageMode::Scroll => {
                // In scroll mode, cell-level damage is recorded as-is (scroll
                // operations produce their own scroll damage entries).
                self.damage_list.push(DamageRegion {
                    row,
                    col_start,
                    col_end: col_end.min(cols),
                });
            }
        }
    }

    /// Record damage for a scroll operation.
    fn record_scroll_damage(&mut self, top: usize, bottom: usize) {
        let cols = self.cols();
        match self.damage_mode {
            DamageMode::Scroll => {
                for r in top..bottom {
                    self.damage_list.push(DamageRegion {
                        row: r,
                        col_start: 0,
                        col_end: cols,
                    });
                }
            }
            _ => {
                // In non-scroll mode, scroll just damages the affected rows.
                for r in top..bottom {
                    self.damage_list.push(DamageRegion {
                        row: r,
                        col_start: 0,
                        col_end: cols,
                    });
                }
            }
        }
    }

    /// Record damage for the entire screen.
    fn record_full_damage(&mut self) {
        let rows = self.rows();
        let cols = self.cols();
        for r in 0..rows {
            self.damage_list.push(DamageRegion {
                row: r,
                col_start: 0,
                col_end: cols,
            });
        }
    }

    /// Take all accumulated damage, resetting the internal list.
    /// In Screen mode, any accumulated damage is coalesced to cover the entire screen.
    pub fn take_damage(&mut self) -> Vec<DamageRegion> {
        let mut damage = std::mem::take(&mut self.damage_list);
        if self.damage_mode == DamageMode::Screen && !damage.is_empty() {
            let rows = self.rows();
            let cols = self.cols();
            damage.clear();
            for r in 0..rows {
                damage.push(DamageRegion {
                    row: r,
                    col_start: 0,
                    col_end: cols,
                });
            }
        }
        // Merge overlapping damage on same row (for Cell mode).
        if self.damage_mode == DamageMode::Cell || self.damage_mode == DamageMode::Scroll {
            damage = Self::merge_damage(damage);
        }
        damage
    }

    /// Merge overlapping/adjacent damage regions on the same row.
    fn merge_damage(mut regions: Vec<DamageRegion>) -> Vec<DamageRegion> {
        if regions.is_empty() {
            return regions;
        }
        regions.sort_by(|a, b| a.row.cmp(&b.row).then(a.col_start.cmp(&b.col_start)));
        let mut merged = Vec::new();
        let mut current = regions[0].clone();
        for r in regions.into_iter().skip(1) {
            if r.row == current.row && r.col_start <= current.col_end {
                current.col_end = current.col_end.max(r.col_end);
            } else {
                merged.push(current);
                current = r;
            }
        }
        merged.push(current);
        merged
    }

    /// Set the damage coalescing mode.
    pub fn set_damage_mode(&mut self, mode: DamageMode) {
        self.damage_mode = mode;
    }

    /// Get the current damage mode.
    pub fn damage_mode(&self) -> DamageMode {
        self.damage_mode
    }
}

// ── Smart scrollback search ──────────────────────────────────────────

// Types are defined in the dedicated `search` module and re-exported.
pub use crate::search::{SearchError, SearchMatch, SearchState};

impl Screen {
    /// Access the current search state (if any).
    pub fn search_state(&self) -> &Option<SearchState> {
        &self.search
    }

    /// Collect the text for every row in the combined buffer (scrollback then viewport).
    fn all_row_texts(&self) -> Vec<String> {
        let sb_len = self.grid.scrollback_len();
        let vp_rows = self.rows();
        let mut texts = Vec::with_capacity(sb_len + vp_rows);
        for i in 0..sb_len {
            texts.push(self.grid.scrollback_row_text(i));
        }
        for r in 0..vp_rows {
            texts.push(self.grid.row_text(r));
        }
        texts
    }

    /// Search forward for `query`, populating the search state with all
    /// matches and setting the current match to the first one found
    /// at or after the viewport top.
    pub fn search_forward(&mut self, query: &str, case_sensitive: bool) -> Vec<SearchMatch> {
        let texts = self.all_row_texts();
        let matches = crate::search::find_all_matches(&texts, query, case_sensitive);
        let sb_len = self.grid.scrollback_len();

        let current = if matches.is_empty() {
            None
        } else {
            matches
                .iter()
                .position(|m| m.row >= sb_len)
                .or(Some(0))
        };

        let result = matches.clone();
        self.search = Some(SearchState {
            query: query.to_string(),
            matches,
            current,
            case_sensitive,
            regex: false,
        });
        result
    }

    /// Search backward for `query`, populating the search state with all
    /// matches and setting the current match to the last one found
    /// before the viewport top.
    pub fn search_backward(&mut self, query: &str, case_sensitive: bool) -> Vec<SearchMatch> {
        let texts = self.all_row_texts();
        let matches = crate::search::find_all_matches(&texts, query, case_sensitive);
        let sb_len = self.grid.scrollback_len();

        let current = if matches.is_empty() {
            None
        } else {
            matches
                .iter()
                .rposition(|m| m.row < sb_len)
                .or(Some(matches.len() - 1))
        };

        let result = matches.clone();
        self.search = Some(SearchState {
            query: query.to_string(),
            matches,
            current,
            case_sensitive,
            regex: false,
        });
        result
    }

    /// Search using a regex pattern.
    pub fn search_regex(
        &mut self,
        pattern: &str,
        case_sensitive: bool,
    ) -> Result<Vec<SearchMatch>, SearchError> {
        let texts = self.all_row_texts();
        let matches = crate::search::find_all_matches_regex(&texts, pattern, case_sensitive)?;
        let sb_len = self.grid.scrollback_len();

        let current = if matches.is_empty() {
            None
        } else {
            matches
                .iter()
                .position(|m| m.row >= sb_len)
                .or(Some(0))
        };

        let result = matches.clone();
        self.search = Some(SearchState {
            query: pattern.to_string(),
            matches,
            current,
            case_sensitive,
            regex: true,
        });
        Ok(result)
    }

    /// Advance to the next match (wrapping around).
    pub fn search_next(&mut self) -> Option<&SearchMatch> {
        let state = self.search.as_mut()?;
        if state.matches.is_empty() {
            return None;
        }
        let next = match state.current {
            Some(idx) => (idx + 1) % state.matches.len(),
            None => 0,
        };
        state.current = Some(next);
        let state = self.search.as_ref().unwrap();
        Some(&state.matches[state.current.unwrap()])
    }

    /// Move to the previous match (wrapping around).
    pub fn search_prev(&mut self) -> Option<&SearchMatch> {
        let state = self.search.as_mut()?;
        if state.matches.is_empty() {
            return None;
        }
        let prev = match state.current {
            Some(0) => state.matches.len() - 1,
            Some(idx) => idx - 1,
            None => state.matches.len() - 1,
        };
        state.current = Some(prev);
        let state = self.search.as_ref().unwrap();
        Some(&state.matches[state.current.unwrap()])
    }

    /// Clear the search state and remove all highlights.
    pub fn clear_search(&mut self) {
        self.search = None;
    }

    /// Get the currently active match, if any.
    pub fn current_match(&self) -> Option<&SearchMatch> {
        let state = self.search.as_ref()?;
        let idx = state.current?;
        state.matches.get(idx)
    }

    /// Get all matches that are currently visible in the viewport.
    pub fn visible_matches(&self) -> Vec<&SearchMatch> {
        let state = match self.search.as_ref() {
            Some(s) => s,
            None => return Vec::new(),
        };
        let sb_len = self.grid.scrollback_len();
        let vp_start = sb_len;
        let vp_end = sb_len + self.rows();
        state
            .matches
            .iter()
            .filter(|m| m.row >= vp_start && m.row < vp_end)
            .collect()
    }
}

/// Determine the display width of a character.
/// Returns 2 for CJK wide characters, 1 for most others, 0 for combining.
fn char_width(c: char) -> u8 {
    let cp = c as u32;
    // Fast path: ASCII printable (most common case)
    if cp < 0x7F {
        return if cp >= 0x20 { 1 } else { 0 };
    }

    // Zero-width characters
    if cp == 0x200B  // Zero-Width Space
        || cp == 0x200C  // Zero-Width Non-Joiner
        || cp == 0x200D  // Zero-Width Joiner
        || cp == 0xFEFF  // BOM / Zero-Width No-Break Space
        || cp == 0x2060  // Word Joiner
        || cp == 0x2061  // Function Application
        || cp == 0x2062  // Invisible Times
        || cp == 0x2063  // Invisible Separator
        || cp == 0x2064  // Invisible Plus
        || cp == 0x180E  // Mongolian Vowel Separator
    {
        return 0;
    }

    // Combining characters (partial list of common ranges)
    if (0x0300..=0x036F).contains(&cp)   // Combining Diacritical Marks
        || (0x1AB0..=0x1AFF).contains(&cp) // Combining Diacritical Marks Extended
        || (0x1DC0..=0x1DFF).contains(&cp) // Combining Diacritical Marks Supplement
        || (0x20D0..=0x20FF).contains(&cp) // Combining Diacritical Marks for Symbols
        || (0xFE20..=0xFE2F).contains(&cp) // Combining Half Marks
    {
        return 0;
    }

    // Wide characters
    if (0x1100..=0x115F).contains(&cp)    // Hangul Jamo
        || (0x2329..=0x232A).contains(&cp) // CJK angle brackets
        || (0x2E80..=0x303E).contains(&cp) // CJK misc
        || (0x3040..=0x33BF).contains(&cp) // Hiragana, Katakana, Bopomofo, etc.
        || (0x3400..=0x4DBF).contains(&cp) // CJK Unified Ideographs Extension A
        || (0x4E00..=0x9FFF).contains(&cp) // CJK Unified Ideographs
        || (0xA000..=0xA4CF).contains(&cp) // Yi
        || (0xAC00..=0xD7A3).contains(&cp) // Hangul Syllables
        || (0xF900..=0xFAFF).contains(&cp) // CJK Compatibility Ideographs
        || (0xFE10..=0xFE19).contains(&cp) // Vertical forms
        || (0xFE30..=0xFE6F).contains(&cp) // CJK Compatibility Forms
        || (0xFF01..=0xFF60).contains(&cp) // Fullwidth Forms
        || (0xFFE0..=0xFFE6).contains(&cp) // Fullwidth Signs
        || (0x1F000..=0x1F9FF).contains(&cp) // Various emoji/symbols
        || (0x20000..=0x2FA1F).contains(&cp) // CJK Extension B and beyond
        || (0x30000..=0x3134F).contains(&cp) // CJK Extension G
    {
        return 2;
    }

    // Control characters
    if cp < 0x20 || (0x7F..=0x9F).contains(&cp) {
        return 0;
    }

    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_screen_default_state() {
        let s = Screen::new(80, 25);
        assert_eq!(s.cols(), 80);
        assert_eq!(s.rows(), 25);
        assert_eq!(s.cursor.row, 0);
        assert_eq!(s.cursor.col, 0);
        assert_eq!(s.scroll_top, 0);
        assert_eq!(s.scroll_bottom, 25);
        // Tab stops every 8 columns
        assert!(!s.tab_stops[0]);
        assert!(s.tab_stops[8]);
        assert!(s.tab_stops[16]);
    }

    #[test]
    fn write_char_basic() {
        let mut s = Screen::new(80, 25);
        s.write_char('A');
        s.write_char('B');
        s.write_char('C');
        assert_eq!(s.grid.cell(0, 0).c, 'A');
        assert_eq!(s.grid.cell(0, 1).c, 'B');
        assert_eq!(s.grid.cell(0, 2).c, 'C');
        assert_eq!(s.cursor.col, 3);
        assert_eq!(s.cursor.row, 0);
    }

    #[test]
    fn write_char_autowrap() {
        let mut s = Screen::new(5, 3);
        for c in "ABCDE".chars() {
            s.write_char(c);
        }
        // After writing 5 chars to a 5-col screen, cursor should be at end with pending wrap
        assert_eq!(s.cursor.row, 0);
        assert!(s.is_pending_wrap());

        // Next char triggers wrap
        s.write_char('F');
        assert_eq!(s.cursor.row, 1);
        assert_eq!(s.cursor.col, 1);
        assert_eq!(s.grid.cell(1, 0).c, 'F');
    }

    #[test]
    fn linefeed_scroll() {
        let mut s = Screen::new(80, 3);
        s.cursor.row = 2;
        s.linefeed();
        // Should have scrolled
        assert_eq!(s.cursor.row, 2);
    }

    #[test]
    fn cursor_position_1based() {
        let mut s = Screen::new(80, 25);
        s.cursor_position(5, 10);
        assert_eq!(s.cursor.row, 4);
        assert_eq!(s.cursor.col, 9);
    }

    #[test]
    fn erase_display_below() {
        let mut s = Screen::new(10, 3);
        for c in "ABCDEFGHIJ".chars() {
            s.write_char(c);
        }
        s.cursor.row = 0;
        s.cursor.col = 5;
        s.erase_display(EraseDisplay::Below);
        assert_eq!(s.grid.cell(0, 4).c, 'E');
        assert_eq!(s.grid.cell(0, 5).c, ' ');
    }

    #[test]
    fn scroll_region() {
        let mut s = Screen::new(80, 25);
        s.set_scroll_region(5, 20);
        assert_eq!(s.scroll_top, 4);
        assert_eq!(s.scroll_bottom, 20);
    }

    #[test]
    fn tab_stops() {
        let mut s = Screen::new(80, 25);
        s.tab();
        assert_eq!(s.cursor.col, 8);
        s.tab();
        assert_eq!(s.cursor.col, 16);
    }

    #[test]
    fn save_restore_cursor() {
        let mut s = Screen::new(80, 25);
        s.cursor.row = 5;
        s.cursor.col = 10;
        s.pen.bold = true;
        s.save_cursor();

        s.cursor.row = 0;
        s.cursor.col = 0;
        s.pen.bold = false;
        s.restore_cursor();

        assert_eq!(s.cursor.row, 5);
        assert_eq!(s.cursor.col, 10);
        assert!(s.pen.bold);
    }

    #[test]
    fn row_text() {
        let mut s = Screen::new(80, 25);
        for c in "Hello".chars() {
            s.write_char(c);
        }
        assert_eq!(s.row_text(0), "Hello");
    }

    #[test]
    fn wide_char_placement() {
        let mut s = Screen::new(80, 25);
        s.write_char('\u{FF10}'); // fullwidth digit zero
        assert_eq!(s.grid.cell(0, 0).c, '\u{FF10}');
        assert_eq!(s.grid.cell(0, 0).width, 2);
        assert_eq!(s.grid.cell(0, 1).width, 0);
        assert_eq!(s.cursor.col, 2);
    }
}
