//! Terminal grid storage and scrollback buffer.
//!
//! The [`Grid`] holds a fixed-size matrix of [`Row`]s (each containing
//! [`Cell`]s) plus a bounded scrollback [`VecDeque`].  All coordinate-level
//! operations live here: cell access, row clearing, rectangular region
//! clearing, scrolling (full-width and margin-bounded), insert/delete of
//! cells and lines, erase-characters, and resize with content reflow.
//!
//! Wide-character boundary handling is done at the `clear_region` and
//! `erase_chars` level so callers do not need to worry about splitting a
//! double-width glyph.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::color::Color;

const DEFAULT_SCROLLBACK_LIMIT: usize = 10_000;

/// Underline style variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum UnderlineStyle {
    #[default]
    None,
    Single,
    Double,
    Curly,
}

/// Cell rendering attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellAttrs {
    pub bold: bool,
    pub italic: bool,
    pub underline: UnderlineStyle,
    pub blink: bool,
    pub reverse: bool,
    pub invisible: bool,
    pub strikethrough: bool,
    pub protected: bool,
}

impl Default for CellAttrs {
    fn default() -> Self {
        Self {
            bold: false,
            italic: false,
            underline: UnderlineStyle::None,
            blink: false,
            reverse: false,
            invisible: false,
            strikethrough: false,
            protected: false,
        }
    }
}

impl CellAttrs {
    /// Return whether all attributes are at their default values.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// A single cell in the grid.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cell {
    /// The character stored in this cell.
    pub c: char,
    /// Display width: 1 for normal, 2 for wide, 0 for continuation of wide char.
    pub width: u8,
    /// Foreground color.
    pub fg: Color,
    /// Background color.
    pub bg: Color,
    /// Rendering attributes.
    pub attrs: CellAttrs,
    /// Hyperlink URI (OSC 8).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hyperlink: Option<String>,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            c: ' ',
            width: 1,
            fg: Color::Default,
            bg: Color::Default,
            attrs: CellAttrs::default(),
            hyperlink: None,
        }
    }
}

impl Cell {
    /// Create a new default (blank) cell.
    pub fn blank() -> Self {
        Self::default()
    }

    /// Reset this cell to default.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Row continuation flag - marks whether a row is a continuation of the previous row
/// (i.e., the line was wrapped due to reaching the right margin).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LineFlags {
    /// True if this line is a continuation of the previous line (soft-wrapped).
    pub continuation: bool,
}

/// A single row of cells with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Row {
    pub cells: Vec<Cell>,
    pub flags: LineFlags,
}

impl Row {
    /// Create a new row with the given number of default (blank) cells.
    pub fn new(cols: usize) -> Self {
        Self {
            cells: vec![Cell::default(); cols],
            flags: LineFlags::default(),
        }
    }

    /// Reset all cells in this row to defaults and clear line flags.
    pub fn reset(&mut self) {
        for cell in &mut self.cells {
            cell.reset();
        }
        self.flags = LineFlags::default();
    }

    /// Resize this row to the given number of columns, appending blank cells or truncating.
    pub fn resize(&mut self, cols: usize) {
        self.cells.resize_with(cols, Cell::default);
    }
}

/// Terminal grid storage.
///
/// A 2-D matrix of [`Row`]s backed by a `Vec`, plus a bounded scrollback
/// ring buffer (`VecDeque<Row>`).  Lines scrolled off the top of the screen
/// are pushed into the scrollback (up to `scrollback_limit`); on resize,
/// scrollback lines are recovered when the screen grows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Grid {
    cols: usize,
    rows: usize,
    lines: Vec<Row>,
    scrollback: VecDeque<Row>,
    scrollback_limit: usize,
}

impl Grid {
    /// Create a new grid with the given dimensions, filled with default cells.
    pub fn new(cols: usize, rows: usize) -> Self {
        let lines = (0..rows).map(|_| Row::new(cols)).collect();
        Self {
            cols,
            rows,
            lines,
            scrollback: VecDeque::new(),
            scrollback_limit: DEFAULT_SCROLLBACK_LIMIT,
        }
    }

    /// Number of columns in the grid.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Number of rows in the grid.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Number of lines currently stored in the scrollback buffer.
    pub fn scrollback_len(&self) -> usize {
        self.scrollback.len()
    }

    /// Set the maximum number of lines kept in the scrollback buffer.
    pub fn set_scrollback_limit(&mut self, limit: usize) {
        self.scrollback_limit = limit;
        while self.scrollback.len() > self.scrollback_limit {
            self.scrollback.pop_front();
        }
    }

    /// Get a reference to a cell (clamped to valid range).
    pub fn cell(&self, row: usize, col: usize) -> &Cell {
        let r = row.min(self.rows.saturating_sub(1));
        let c = col.min(self.cols.saturating_sub(1));
        &self.lines[r].cells[c]
    }

    /// Get a mutable reference to a cell (clamped to valid range).
    pub fn cell_mut(&mut self, row: usize, col: usize) -> &mut Cell {
        let r = row.min(self.rows.saturating_sub(1));
        let c = col.min(self.cols.saturating_sub(1));
        &mut self.lines[r].cells[c]
    }

    /// Get a reference to a row (clamped to valid range).
    pub fn row(&self, row: usize) -> &Row {
        let r = row.min(self.rows.saturating_sub(1));
        &self.lines[r]
    }

    /// Get a mutable reference to a row (clamped to valid range).
    pub fn row_mut(&mut self, row: usize) -> &mut Row {
        let r = row.min(self.rows.saturating_sub(1));
        &mut self.lines[r]
    }

    /// Scroll the region [top, bottom) up by `count` lines.
    /// Lines scrolled off the top of the region are pushed to scrollback
    /// only if top == 0 (i.e., the scroll region starts at the top of the screen).
    pub fn scroll_up(&mut self, top: usize, bottom: usize, count: usize) {
        if bottom <= top {
            return;
        }
        let count = count.min(bottom - top);
        if count == 0 {
            return;
        }

        // Push lines to scrollback if scrolling from the very top
        if top == 0 {
            for i in 0..count {
                let old_row = std::mem::replace(&mut self.lines[i], Row::new(self.cols));
                self.scrollback.push_back(old_row);
                while self.scrollback.len() > self.scrollback_limit {
                    self.scrollback.pop_front();
                }
            }
        }

        // Rotate lines up within the region
        self.lines[top..bottom].rotate_left(count);

        // Clear the newly exposed lines at the bottom of the region
        for i in (bottom - count)..bottom {
            self.lines[i] = Row::new(self.cols);
        }
    }

    /// Scroll the region [top, bottom) down by `count` lines.
    /// Lines scrolled off the bottom are discarded.
    pub fn scroll_down(&mut self, top: usize, bottom: usize, count: usize) {
        if bottom <= top {
            return;
        }
        let count = count.min(bottom - top);
        if count == 0 {
            return;
        }

        // Rotate lines down within the region
        self.lines[top..bottom].rotate_right(count);

        // Clear the newly exposed lines at the top of the region
        for i in top..(top + count) {
            self.lines[i] = Row::new(self.cols);
        }
    }

    /// Clear a row, filling it with default cells.
    pub fn clear_row(&mut self, row: usize) {
        self.lines[row].reset();
    }

    /// Clear a rectangular region [top..=bottom, left..=right].
    /// Handles wide character boundaries: if the left edge splits a wide char,
    /// the head is also cleared; if the right edge splits a wide char, the
    /// continuation is also cleared.
    pub fn clear_region(&mut self, top: usize, left: usize, bottom: usize, right: usize) {
        let bottom = bottom.min(self.rows.saturating_sub(1));
        let right = right.min(self.cols.saturating_sub(1));
        for r in top..=bottom {
            // If left edge is a continuation cell (width==0), clear the head too
            if left > 0 && left <= right
                && self.lines[r].cells[left].width == 0
            {
                self.lines[r].cells[left - 1].reset();
            }
            // If right edge is a wide char head (width==2), clear the continuation too
            if right + 1 < self.cols
                && self.lines[r].cells[right].width == 2
            {
                self.lines[r].cells[right + 1].reset();
            }
            for c in left..=right {
                self.lines[r].cells[c].reset();
            }
        }
    }

    /// Resize the grid to new dimensions, handling content reflow.
    ///
    /// When the number of columns changes, logical lines (sequences of rows
    /// joined by the continuation flag) are reflowed: wider terminals unwrap
    /// soft-wrapped lines, narrower terminals re-wrap them.
    ///
    /// Returns the cursor-row delta (signed) that callers should apply to keep
    /// the cursor on the same logical line.
    pub fn resize(&mut self, new_cols: usize, new_rows: usize) -> isize {
        self.resize_with_cursor(new_cols, new_rows, None)
    }

    /// Like [`resize`](Self::resize) but also tracks a cursor position through
    /// the reflow so it stays on the correct cell.  `cursor` is (row, col) in
    /// the viewport; returns adjusted (row, col).
    pub fn resize_with_cursor(
        &mut self,
        new_cols: usize,
        new_rows: usize,
        cursor: Option<(usize, usize)>,
    ) -> isize {
        if new_cols == self.cols && new_rows == self.rows {
            return 0;
        }

        let old_cols = self.cols;
        let mut cursor_row_delta: isize = 0;

        // Handle column resize with reflow
        if new_cols != old_cols {
            // Collect all rows: scrollback + viewport into one list of logical lines.
            let sb_len = self.scrollback.len();
            let mut all_rows: Vec<Row> = Vec::with_capacity(sb_len + self.lines.len());
            for row in self.scrollback.drain(..) {
                all_rows.push(row);
            }
            all_rows.append(&mut self.lines);

            // The cursor's absolute row index in all_rows
            let abs_cursor_row = cursor.map(|(r, _)| sb_len + r);
            let cursor_col = cursor.map(|(_, c)| c).unwrap_or(0);

            // Reflow all_rows at the new column width.
            let (reflowed, new_abs_cursor) =
                Self::reflow_lines(all_rows, old_cols, new_cols, abs_cursor_row, cursor_col);

            // Split reflowed back into scrollback and viewport.
            // The viewport gets the last new_rows lines (or fewer if not enough).
            let total = reflowed.len();
            let vp_count = total.min(new_rows);
            let sb_count = total - vp_count;

            self.scrollback = reflowed[..sb_count].to_vec().into();
            self.lines = reflowed[sb_count..].to_vec();

            // Trim scrollback to limit
            while self.scrollback.len() > self.scrollback_limit {
                self.scrollback.pop_front();
            }

            // Compute cursor row delta
            if let Some(new_abs) = new_abs_cursor {
                let new_vp_row = new_abs.saturating_sub(sb_count);
                let old_vp_row = cursor.map(|(r, _)| r).unwrap_or(0);
                cursor_row_delta = new_vp_row as isize - old_vp_row as isize;
            }

            self.cols = new_cols;
        }

        // Handle row resize
        if new_rows > self.lines.len() {
            // Growing: try to pop lines from scrollback first
            let extra = new_rows - self.lines.len();
            let from_scrollback = extra.min(self.scrollback.len());

            // Prepend lines from scrollback
            let mut new_lines = Vec::with_capacity(new_rows);
            for _ in 0..from_scrollback {
                if let Some(mut row) = self.scrollback.pop_back() {
                    row.resize(new_cols);
                    new_lines.push(row);
                }
            }
            new_lines.append(&mut self.lines);
            // Fill remaining with blank lines
            while new_lines.len() < new_rows {
                new_lines.push(Row::new(new_cols));
            }
            self.lines = new_lines;
            // Cursor moved down by however many scrollback lines were prepended
            cursor_row_delta += from_scrollback as isize;
        } else if new_rows < self.lines.len() {
            // Shrinking: push excess lines to scrollback
            let excess = self.lines.len() - new_rows;
            for _ in 0..excess {
                if !self.lines.is_empty() {
                    // Check if bottom lines are empty; if so, remove from bottom
                    let last_idx = self.lines.len() - 1;
                    let last_empty = self.lines[last_idx]
                        .cells
                        .iter()
                        .all(|c| c.c == ' ' && c.attrs == CellAttrs::default());
                    if last_empty {
                        self.lines.pop();
                    } else {
                        // Push top line to scrollback
                        let row = self.lines.remove(0);
                        self.scrollback.push_back(row);
                        while self.scrollback.len() > self.scrollback_limit {
                            self.scrollback.pop_front();
                        }
                        cursor_row_delta -= 1;
                    }
                }
            }
        }

        self.cols = new_cols;
        self.rows = new_rows;
        cursor_row_delta
    }

    /// Reflow a list of rows from `old_cols` to `new_cols`.
    ///
    /// Groups rows into logical lines (using the continuation flag), joins each
    /// group into one long cell buffer, then re-splits at `new_cols`.
    ///
    /// Tracks `cursor_abs_row` through the reflow and returns the new absolute
    /// row index.
    fn reflow_lines(
        rows: Vec<Row>,
        old_cols: usize,
        new_cols: usize,
        cursor_abs_row: Option<usize>,
        cursor_col: usize,
    ) -> (Vec<Row>, Option<usize>) {
        let mut result: Vec<Row> = Vec::new();
        let mut new_cursor_row: Option<usize> = None;

        // Group into logical lines: a logical line starts at a row where
        // `continuation == false` and includes all subsequent rows where
        // `continuation == true`.
        let mut i = 0;
        while i < rows.len() {
            // Collect all rows belonging to this logical line.
            let start = i;
            i += 1;
            while i < rows.len() && rows[i].flags.continuation {
                i += 1;
            }
            // rows[start..i] is one logical line.
            let line_len = i - start;

            // Determine if cursor is in this logical line
            let cursor_in_line = cursor_abs_row.is_some_and(|cr| cr >= start && cr < i);

            // Single-row logical line (no continuations): just resize, don't reflow.
            if line_len == 1 {
                let mut row = rows[start].clone();
                row.resize(new_cols);
                let row_idx = result.len();
                if cursor_in_line {
                    new_cursor_row = Some(row_idx);
                }
                result.push(row);
                continue;
            }

            // Multi-row logical line: join and re-split.
            let cursor_offset_in_line = if cursor_in_line {
                let cr = cursor_abs_row.unwrap();
                (cr - start) * old_cols + cursor_col
            } else {
                0
            };

            // Join cells from all rows in this logical line into one buffer.
            let mut cells: Vec<Cell> = Vec::new();
            for row in &rows[start..i] {
                cells.extend(row.cells.iter().cloned());
            }

            // Trim trailing blank cells from the joined line.
            let content_len = {
                let mut len = cells.len();
                while len > 0 {
                    let c = &cells[len - 1];
                    if c.c != ' ' || c.width != 1 || !c.attrs.is_default()
                        || c.fg != Color::Default || c.bg != Color::Default
                        || c.hyperlink.is_some()
                    {
                        break;
                    }
                    len -= 1;
                }
                len
            };

            // Split into rows of new_cols width
            if content_len == 0 {
                // Empty logical line (all continuation rows were blank)
                let row_idx = result.len();
                result.push(Row::new(new_cols));
                if cursor_in_line {
                    new_cursor_row = Some(row_idx);
                }
            } else {
                let num_rows = content_len.div_ceil(new_cols);
                for r in 0..num_rows {
                    let row_start = r * new_cols;
                    let row_end = ((r + 1) * new_cols).min(cells.len());
                    let mut new_row = Row::new(new_cols);
                    for (j, cell_idx) in (row_start..row_end).enumerate() {
                        if cell_idx < cells.len() {
                            new_row.cells[j] = cells[cell_idx].clone();
                        }
                    }
                    // First row of logical line is canonical (not continuation)
                    // Subsequent rows are continuations
                    new_row.flags.continuation = r > 0;

                    let row_idx = result.len();
                    // Check if cursor falls in this output row
                    if cursor_in_line {
                        let row_cell_start = row_start;
                        let row_cell_end = (r + 1) * new_cols;
                        if cursor_offset_in_line >= row_cell_start
                            && cursor_offset_in_line < row_cell_end
                        {
                            new_cursor_row = Some(row_idx);
                        }
                    }

                    result.push(new_row);
                }
                // If cursor was past end of content, put it on the last row
                if cursor_in_line && new_cursor_row.is_none() {
                    new_cursor_row = Some(result.len() - 1);
                }
            }
        }

        (result, new_cursor_row)
    }

    /// Insert `count` blank cells at (row, col), shifting existing cells to the right.
    /// Cells shifted past the right edge are lost. (ICH)
    pub fn insert_cells(&mut self, row: usize, col: usize, count: usize) {
        self.insert_cells_bounded(row, col, count, self.cols);
    }

    /// Delete `count` cells at (row, col), shifting remaining cells to the left.
    /// Blank cells are inserted at the right edge. (DCH)
    pub fn delete_cells(&mut self, row: usize, col: usize, count: usize) {
        self.delete_cells_bounded(row, col, count, self.cols);
    }

    /// Insert `count` blank lines at `row` within the scroll region [top, bottom).
    /// Lines pushed past the bottom of the region are lost. (IL)
    pub fn insert_lines(&mut self, row: usize, count: usize, top: usize, bottom: usize) {
        if row < top || row >= bottom {
            return;
        }
        let count = count.min(bottom - row);

        // Move lines down within the region
        for i in (row..(bottom - count)).rev() {
            self.lines[i + count] = self.lines[i].clone();
        }
        // Clear inserted lines
        for i in row..(row + count) {
            self.lines[i] = Row::new(self.cols);
        }
    }

    /// Delete `count` lines at `row` within the scroll region [top, bottom).
    /// Blank lines are inserted at the bottom of the region. (DL)
    pub fn delete_lines(&mut self, row: usize, count: usize, top: usize, bottom: usize) {
        if row < top || row >= bottom {
            return;
        }
        let count = count.min(bottom - row);

        // Move lines up within the region
        for i in row..(bottom - count) {
            self.lines[i] = self.lines[i + count].clone();
        }
        // Clear lines at the bottom of the region
        for i in (bottom - count)..bottom {
            self.lines[i] = Row::new(self.cols);
        }
    }

    /// Erase `count` characters starting at (row, col), replacing with blanks.
    /// Does not shift cells. Handles wide char boundaries. (ECH)
    pub fn erase_chars(&mut self, row: usize, col: usize, count: usize) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        let end = (col + count).min(self.cols);
        // Handle wide char boundary at start
        if col > 0 && self.lines[row].cells[col].width == 0 {
            self.lines[row].cells[col - 1].reset();
        }
        // Handle wide char boundary at end
        if end < self.cols && end > col
            && self.lines[row].cells[end - 1].width == 2
        {
            self.lines[row].cells[end].reset();
        }
        for c in col..end {
            self.lines[row].cells[c].reset();
        }
    }

    /// Get text content of a row (trimming trailing spaces).
    pub fn row_text(&self, row: usize) -> String {
        let mut s = String::new();
        for cell in &self.lines[row].cells {
            if cell.width == 0 {
                // Continuation cell for wide char, skip
                continue;
            }
            s.push(cell.c);
        }
        // Trim trailing spaces
        let trimmed_len = s.trim_end_matches(' ').len();
        s.truncate(trimmed_len);
        s
    }

    /// Get a reference to a scrollback row by index.
    /// Index 0 is the oldest scrollback line.
    pub fn scrollback_row(&self, index: usize) -> Option<&Row> {
        self.scrollback.get(index)
    }

    /// Pop a line from the back of the scrollback.
    pub fn pop_scrollback(&mut self) -> Option<Row> {
        self.scrollback.pop_back()
    }

    /// Push a line to the scrollback.
    pub fn push_scrollback(&mut self, row: Row) {
        self.scrollback.push_back(row);
        while self.scrollback.len() > self.scrollback_limit {
            self.scrollback.pop_front();
        }
    }

    /// Get the text content of a scrollback row (trimming trailing spaces).
    /// `index` 0 is the oldest scrollback line.
    pub fn scrollback_row_text(&self, index: usize) -> String {
        if index >= self.scrollback.len() {
            return String::new();
        }
        let row = &self.scrollback[index];
        let mut s = String::new();
        for cell in &row.cells {
            if cell.width == 0 {
                continue;
            }
            s.push(cell.c);
        }
        let trimmed_len = s.trim_end_matches(' ').len();
        s.truncate(trimmed_len);
        s
    }

    /// Get the full text of a scrollback row (without trimming).
    pub fn scrollback_row_text_full(&self, index: usize) -> String {
        if index >= self.scrollback.len() {
            return String::new();
        }
        let row = &self.scrollback[index];
        let mut s = String::new();
        for cell in &row.cells {
            if cell.width == 0 {
                continue;
            }
            s.push(cell.c);
        }
        s
    }

    /// Get the full text of a viewport row (without trimming).
    pub fn row_text_full(&self, row: usize) -> String {
        let mut s = String::new();
        for cell in &self.lines[row].cells {
            if cell.width == 0 {
                continue;
            }
            s.push(cell.c);
        }
        s
    }

    // ── Margin-aware operations (DECSLRM support) ────────────────────

    /// Scroll the rectangular region [top, bottom) x [left, right) up by `count` lines.
    /// Only cells within [left, right) are shifted; cells outside are untouched.
    pub fn scroll_up_region(
        &mut self, top: usize, bottom: usize, left: usize, right: usize, count: usize,
    ) {
        if bottom <= top || right <= left {
            return;
        }
        let count = count.min(bottom - top);
        if count == 0 {
            return;
        }

        // Move cells up within the column range
        for r in top..(bottom - count) {
            for c in left..right.min(self.cols) {
                self.lines[r].cells[c] = self.lines[r + count].cells[c].clone();
            }
        }
        // Clear the newly exposed cells at the bottom
        for r in (bottom - count)..bottom {
            for c in left..right.min(self.cols) {
                self.lines[r].cells[c] = Cell::default();
            }
        }
    }

    /// Scroll the rectangular region [top, bottom) x [left, right) down by `count` lines.
    /// Only cells within [left, right) are shifted; cells outside are untouched.
    pub fn scroll_down_region(
        &mut self, top: usize, bottom: usize, left: usize, right: usize, count: usize,
    ) {
        if bottom <= top || right <= left {
            return;
        }
        let count = count.min(bottom - top);
        if count == 0 {
            return;
        }

        // Move cells down within the column range
        for r in (top..(bottom - count)).rev() {
            for c in left..right.min(self.cols) {
                self.lines[r + count].cells[c] = self.lines[r].cells[c].clone();
            }
        }
        // Clear the newly exposed cells at the top
        for r in top..(top + count) {
            for c in left..right.min(self.cols) {
                self.lines[r].cells[c] = Cell::default();
            }
        }
    }

    /// Insert `count` blank cells at (row, col), shifting existing cells right.
    /// Cells shifted past `right_bound` (exclusive) are lost. (ICH with margin)
    pub fn insert_cells_bounded(
        &mut self, row: usize, col: usize, count: usize, right_bound: usize,
    ) {
        let right_bound = right_bound.min(self.cols);
        if col >= right_bound {
            return;
        }
        let count = count.min(right_bound - col);
        let cells = &mut self.lines[row].cells;

        // Shift cells to the right within [col, right_bound)
        for i in (col + count..right_bound).rev() {
            cells[i] = cells[i - count].clone();
        }
        // Clear the inserted cells
        for cell in cells.iter_mut().take((col + count).min(right_bound)).skip(col) {
            *cell = Cell::default();
        }
    }

    /// Delete `count` cells at (row, col), shifting remaining cells left.
    /// Blank cells are inserted at the right bound. (DCH with margin)
    pub fn delete_cells_bounded(
        &mut self, row: usize, col: usize, count: usize, right_bound: usize,
    ) {
        let right_bound = right_bound.min(self.cols);
        if col >= right_bound {
            return;
        }
        let count = count.min(right_bound - col);
        let cells = &mut self.lines[row].cells;

        // Shift cells to the left within [col, right_bound)
        for i in col..(right_bound - count) {
            cells[i] = cells[i + count].clone();
        }
        // Clear the cells at the right
        for cell in cells.iter_mut().take(right_bound).skip(right_bound - count) {
            *cell = Cell::default();
        }
    }
}
