//! Damage tracking for incremental screen updates.

/// Tracks which rows need redrawing to avoid full-screen repaints.
pub struct DamageTracker {
    dirty_rows: Vec<bool>,
    full_redraw: bool,
}

impl DamageTracker {
    /// Create a new tracker with the given number of rows, initially marked for full redraw.
    pub fn new(rows: usize) -> Self {
        Self {
            dirty_rows: vec![false; rows],
            full_redraw: true,
        }
    }

    /// Mark a single row as dirty.
    pub fn mark_row(&mut self, row: usize) {
        if row < self.dirty_rows.len() {
            self.dirty_rows[row] = true;
        }
    }

    /// Mark all rows for redraw.
    pub fn mark_all(&mut self) {
        self.full_redraw = true;
    }

    /// Clear all damage state.
    pub fn clear(&mut self) {
        for d in &mut self.dirty_rows {
            *d = false;
        }
        self.full_redraw = false;
    }

    /// Check whether a specific row needs redrawing.
    pub fn is_dirty(&self, row: usize) -> bool {
        self.full_redraw || self.dirty_rows.get(row).copied().unwrap_or(false)
    }

    /// Check whether any redraw is needed at all.
    pub fn needs_redraw(&self) -> bool {
        self.full_redraw || self.dirty_rows.iter().any(|&d| d)
    }

    /// Return a list of row indices that need redrawing.
    pub fn dirty_rows(&self) -> Vec<usize> {
        (0..self.dirty_rows.len())
            .filter(|&r| self.is_dirty(r))
            .collect()
    }

    /// Resize the tracker to a new row count, marking a full redraw.
    pub fn resize(&mut self, rows: usize) {
        self.dirty_rows.resize(rows, false);
        self.full_redraw = true;
    }
}
