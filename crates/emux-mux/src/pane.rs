//! Pane management — individual terminal instances within a window.

pub type PaneId = u32;

/// Size of a pane in rows and columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneSize {
    pub rows: usize,
    pub cols: usize,
}

/// Constraints that prevent a pane from being split or resized along an axis.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PaneConstraints {
    pub fixed_rows: Option<usize>,
    pub fixed_cols: Option<usize>,
}

impl PaneSize {
    pub fn new(cols: usize, rows: usize) -> Self {
        Self { rows, cols }
    }
}

/// A single pane, representing one terminal instance.
#[derive(Debug)]
pub struct Pane {
    id: PaneId,
    title: String,
    previous_title: Option<String>,
    size: PaneSize,
    cleared: bool,
    scroll_offset: usize,
    constraints: PaneConstraints,
    scrollback: Vec<String>,
}

impl Pane {
    /// Create a new pane with the given ID and dimensions.
    pub fn new(id: PaneId, cols: usize, rows: usize) -> Self {
        Self {
            id,
            title: String::new(),
            previous_title: None,
            size: PaneSize::new(cols, rows),
            cleared: false,
            scroll_offset: 0,
            constraints: PaneConstraints::default(),
            scrollback: Vec::new(),
        }
    }

    pub fn id(&self) -> PaneId {
        self.id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.previous_title = Some(self.title.clone());
        self.title = title.into();
    }

    /// Undo the last rename, restoring the previous title.
    pub fn undo_rename(&mut self) -> bool {
        if let Some(prev) = self.previous_title.take() {
            self.title = prev;
            true
        } else {
            false
        }
    }

    pub fn set_cleared(&mut self, cleared: bool) {
        self.cleared = cleared;
    }

    pub fn is_cleared(&self) -> bool {
        self.cleared
    }

    /// Scroll up by the given number of lines.
    pub fn scroll_up(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(lines);
    }

    /// Scroll down by the given number of lines.
    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    /// Scroll to the top of the scrollback.
    pub fn scroll_to_top(&mut self) {
        // In a real implementation, this would go to the top of scrollback.
        // For now, set to a large value.
        self.scroll_offset = usize::MAX;
    }

    /// Scroll to the bottom (most recent output).
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    /// Get the current scroll offset.
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    pub fn size(&self) -> PaneSize {
        self.size
    }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.size = PaneSize::new(cols, rows);
    }

    /// Get the pane constraints.
    pub fn constraints(&self) -> &PaneConstraints {
        &self.constraints
    }

    /// Set pane constraints.
    pub fn set_constraints(&mut self, constraints: PaneConstraints) {
        self.constraints = constraints;
    }

    /// Check if this pane has a fixed number of columns.
    pub fn has_fixed_cols(&self) -> bool {
        self.constraints.fixed_cols.is_some()
    }

    /// Check if this pane has a fixed number of rows.
    pub fn has_fixed_rows(&self) -> bool {
        self.constraints.fixed_rows.is_some()
    }

    /// Push a line to the scrollback buffer.
    pub fn push_scrollback(&mut self, line: impl Into<String>) {
        self.scrollback.push(line.into());
    }

    /// Get the scrollback buffer.
    pub fn scrollback(&self) -> &[String] {
        &self.scrollback
    }
}
