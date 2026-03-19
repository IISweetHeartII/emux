//! Pane management — individual terminal instances within a window.

use std::path::{Path, PathBuf};

pub type PaneId = u32;

/// Size of a pane in rows and columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneSize {
    /// Number of rows (height in characters).
    pub rows: usize,
    /// Number of columns (width in characters).
    pub cols: usize,
}

/// Constraints that prevent a pane from being split or resized along an axis.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PaneConstraints {
    /// If set, the pane cannot be resized vertically.
    pub fixed_rows: Option<usize>,
    /// If set, the pane cannot be resized horizontally.
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
    has_notification: bool,
    notification_text: Option<String>,
    working_directory: Option<PathBuf>,
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
            has_notification: false,
            notification_text: None,
            working_directory: None,
        }
    }

    /// Returns the pane's unique identifier.
    pub fn id(&self) -> PaneId {
        self.id
    }

    /// Returns the pane's current title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Set the pane title, saving the previous title for undo.
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

    /// Mark or unmark the pane as cleared.
    pub fn set_cleared(&mut self, cleared: bool) {
        self.cleared = cleared;
    }

    /// Returns whether the pane has been cleared.
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

    /// Returns the current pane dimensions.
    pub fn size(&self) -> PaneSize {
        self.size
    }

    /// Resize the pane to the given dimensions.
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

    /// Set the working directory of this pane.
    pub fn set_working_directory(&mut self, path: PathBuf) {
        self.working_directory = Some(path);
    }

    /// Get the working directory of this pane, if known.
    pub fn working_directory(&self) -> Option<&Path> {
        self.working_directory.as_deref()
    }

    /// Whether this pane has an unread notification.
    pub fn has_notification(&self) -> bool {
        self.has_notification
    }

    /// Get the notification text, if any.
    pub fn notification_text(&self) -> Option<&str> {
        self.notification_text.as_deref()
    }

    /// Set a notification on this pane.
    pub fn set_notification(&mut self, text: impl Into<String>) {
        self.has_notification = true;
        self.notification_text = Some(text.into());
    }

    /// Clear the notification on this pane.
    pub fn clear_notification(&mut self) {
        self.has_notification = false;
        self.notification_text = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_notification_default() {
        let p = Pane::new(0, 80, 25);
        assert!(!p.has_notification());
        assert!(p.notification_text().is_none());
    }

    #[test]
    fn pane_set_notification() {
        let mut p = Pane::new(0, 80, 25);
        p.set_notification("Build complete");
        assert!(p.has_notification());
        assert_eq!(p.notification_text(), Some("Build complete"));
    }

    #[test]
    fn pane_clear_notification() {
        let mut p = Pane::new(0, 80, 25);
        p.set_notification("Hello");
        p.clear_notification();
        assert!(!p.has_notification());
        assert!(p.notification_text().is_none());
    }

    #[test]
    fn pane_resize() {
        let mut p = Pane::new(0, 80, 25);
        assert_eq!(p.size().cols, 80);
        assert_eq!(p.size().rows, 25);
        p.resize(120, 40);
        assert_eq!(p.size().cols, 120);
        assert_eq!(p.size().rows, 40);
    }

    #[test]
    fn pane_scroll_operations() {
        let mut p = Pane::new(0, 80, 25);
        p.push_scrollback("line 1".to_string());
        p.push_scrollback("line 2".to_string());
        p.push_scrollback("line 3".to_string());
        assert_eq!(p.scrollback().len(), 3);
        assert_eq!(p.scroll_offset(), 0);

        p.scroll_up(5);
        assert_eq!(p.scroll_offset(), 5);

        p.scroll_down(3);
        assert_eq!(p.scroll_offset(), 2);

        p.scroll_to_top();
        assert_eq!(p.scroll_offset(), usize::MAX);

        p.scroll_to_bottom();
        assert_eq!(p.scroll_offset(), 0);
    }

    #[test]
    fn pane_title_operations() {
        let mut p = Pane::new(0, 80, 25);
        assert_eq!(p.title(), "");

        p.set_title("my pane");
        assert_eq!(p.title(), "my pane");

        p.set_title("renamed");
        assert_eq!(p.title(), "renamed");

        // undo should restore previous title
        assert!(p.undo_rename());
        assert_eq!(p.title(), "my pane");

        // undo again with nothing to restore
        assert!(!p.undo_rename());
    }

    #[test]
    fn pane_constraints() {
        let mut p = Pane::new(0, 80, 25);
        assert!(!p.has_fixed_cols());
        assert!(!p.has_fixed_rows());

        p.set_constraints(PaneConstraints {
            fixed_cols: Some(40),
            fixed_rows: None,
        });
        assert!(p.has_fixed_cols());
        assert!(!p.has_fixed_rows());
        assert_eq!(p.constraints().fixed_cols, Some(40));

        p.set_constraints(PaneConstraints {
            fixed_cols: None,
            fixed_rows: Some(10),
        });
        assert!(!p.has_fixed_cols());
        assert!(p.has_fixed_rows());
        assert_eq!(p.constraints().fixed_rows, Some(10));
    }

    #[test]
    fn pane_working_directory() {
        let mut p = Pane::new(0, 80, 25);
        assert!(p.working_directory().is_none());

        p.set_working_directory(PathBuf::from("/home/user/projects"));
        assert_eq!(
            p.working_directory(),
            Some(Path::new("/home/user/projects"))
        );
    }
}
