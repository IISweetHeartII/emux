//! Window abstraction — a named container of tabs.
//!
//! The hierarchy is: Session -> Windows -> Tabs -> Panes.
//! A [`Window`] groups related tabs and tracks which one is active.

use crate::pane::PaneSize;
use crate::tab::{Tab, TabId};

/// Unique window identifier within a session.
pub type WindowId = u32;

/// A window contains multiple tabs and tracks which one is active.
#[derive(Debug)]
pub struct Window {
    id: WindowId,
    name: String,
    tabs: Vec<Tab>,
    active_tab_idx: usize,
    next_tab_id: TabId,
    size: PaneSize,
}

impl Window {
    /// Create a new window with one default tab.
    pub fn new(id: WindowId, name: impl Into<String>, cols: usize, rows: usize) -> Self {
        let tab = Tab::new(0, "Tab 1", cols, rows);
        Self {
            id,
            name: name.into(),
            tabs: vec![tab],
            active_tab_idx: 0,
            next_tab_id: 1,
            size: PaneSize::new(cols, rows),
        }
    }

    /// Window ID.
    pub fn id(&self) -> WindowId {
        self.id
    }

    /// Window name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Rename the window.
    pub fn rename(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    /// Number of tabs in this window.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Add a new tab with the given name. Returns its ID.
    pub fn add_tab(&mut self, name: impl Into<String>) -> TabId {
        let tab_id = self.next_tab_id;
        self.next_tab_id += 1;
        let tab = Tab::new(tab_id, name, self.size.cols, self.size.rows);
        self.tabs.push(tab);
        self.active_tab_idx = self.tabs.len() - 1;
        tab_id
    }

    /// Remove a tab by index. Returns `true` if removed.
    /// Cannot remove the last tab.
    pub fn remove_tab(&mut self, index: usize) -> bool {
        if self.tabs.len() <= 1 || index >= self.tabs.len() {
            return false;
        }
        self.tabs.remove(index);
        if self.active_tab_idx >= self.tabs.len() {
            self.active_tab_idx = self.tabs.len() - 1;
        } else if self.active_tab_idx > index {
            self.active_tab_idx -= 1;
        }
        true
    }

    /// Remove a tab by its TabId. Returns `true` if removed.
    pub fn remove_tab_by_id(&mut self, tab_id: TabId) -> bool {
        if self.tabs.len() <= 1 {
            return false;
        }
        if let Some(idx) = self.tabs.iter().position(|t| t.id() == tab_id) {
            self.remove_tab(idx)
        } else {
            false
        }
    }

    /// Get a reference to the active tab.
    pub fn active_tab(&self) -> &Tab {
        &self.tabs[self.active_tab_idx]
    }

    /// Get a mutable reference to the active tab.
    pub fn active_tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active_tab_idx]
    }

    /// Active tab index.
    pub fn active_tab_index(&self) -> usize {
        self.active_tab_idx
    }

    /// Switch to a tab by index. Returns `true` if the index was valid.
    pub fn switch_tab(&mut self, index: usize) -> bool {
        if index < self.tabs.len() {
            self.active_tab_idx = index;
            true
        } else {
            false
        }
    }

    /// Switch to the next tab (wrapping).
    pub fn next_tab(&mut self) -> bool {
        if self.tabs.len() <= 1 {
            return false;
        }
        self.active_tab_idx = (self.active_tab_idx + 1) % self.tabs.len();
        true
    }

    /// Switch to the previous tab (wrapping).
    pub fn prev_tab(&mut self) -> bool {
        if self.tabs.len() <= 1 {
            return false;
        }
        self.active_tab_idx = if self.active_tab_idx == 0 {
            self.tabs.len() - 1
        } else {
            self.active_tab_idx - 1
        };
        true
    }

    /// Get a reference to a tab by index.
    pub fn tab(&self, index: usize) -> Option<&Tab> {
        self.tabs.get(index)
    }

    /// Get a mutable reference to a tab by index.
    pub fn tab_mut(&mut self, index: usize) -> Option<&mut Tab> {
        self.tabs.get_mut(index)
    }

    /// Get tab names.
    pub fn tab_names(&self) -> Vec<&str> {
        self.tabs.iter().map(|t| t.name()).collect()
    }

    /// Resize the window (and all tabs).
    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.size = PaneSize::new(cols, rows);
        for tab in &mut self.tabs {
            tab.resize(cols, rows);
        }
    }

    /// Get the window size.
    pub fn size(&self) -> PaneSize {
        self.size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_window_has_one_tab() {
        let w = Window::new(0, "main", 80, 24);
        assert_eq!(w.id(), 0);
        assert_eq!(w.name(), "main");
        assert_eq!(w.tab_count(), 1);
        assert_eq!(w.active_tab_index(), 0);
    }

    #[test]
    fn add_tab_increments_count() {
        let mut w = Window::new(0, "win", 80, 24);
        let id1 = w.add_tab("second");
        assert_eq!(w.tab_count(), 2);
        assert_eq!(w.active_tab_index(), 1); // newly added tab is active
        assert_eq!(id1, 1);

        let id2 = w.add_tab("third");
        assert_eq!(w.tab_count(), 3);
        assert_eq!(id2, 2);
    }

    #[test]
    fn remove_tab_by_index() {
        let mut w = Window::new(0, "win", 80, 24);
        w.add_tab("second");
        w.add_tab("third");
        assert_eq!(w.tab_count(), 3);

        // Remove the middle tab
        assert!(w.remove_tab(1));
        assert_eq!(w.tab_count(), 2);
    }

    #[test]
    fn cannot_remove_last_tab() {
        let mut w = Window::new(0, "win", 80, 24);
        assert!(!w.remove_tab(0));
        assert_eq!(w.tab_count(), 1);
    }

    #[test]
    fn remove_tab_adjusts_active_index() {
        let mut w = Window::new(0, "win", 80, 24);
        w.add_tab("second");
        w.add_tab("third");
        // Active is 2 (third tab)
        assert_eq!(w.active_tab_index(), 2);

        // Remove tab 0 -> active should shift down
        assert!(w.remove_tab(0));
        assert_eq!(w.active_tab_index(), 1);
    }

    #[test]
    fn remove_active_tab_clamps() {
        let mut w = Window::new(0, "win", 80, 24);
        w.add_tab("second");
        // active = 1
        assert!(w.remove_tab(1));
        assert_eq!(w.active_tab_index(), 0);
    }

    #[test]
    fn switch_tab() {
        let mut w = Window::new(0, "win", 80, 24);
        w.add_tab("second");
        w.add_tab("third");

        assert!(w.switch_tab(0));
        assert_eq!(w.active_tab_index(), 0);
        assert!(w.switch_tab(2));
        assert_eq!(w.active_tab_index(), 2);
        assert!(!w.switch_tab(5)); // out of range
    }

    #[test]
    fn next_prev_tab() {
        let mut w = Window::new(0, "win", 80, 24);
        w.add_tab("second");
        w.add_tab("third");
        w.switch_tab(0);

        assert!(w.next_tab());
        assert_eq!(w.active_tab_index(), 1);
        assert!(w.next_tab());
        assert_eq!(w.active_tab_index(), 2);
        assert!(w.next_tab()); // wrap
        assert_eq!(w.active_tab_index(), 0);

        assert!(w.prev_tab()); // wrap backward
        assert_eq!(w.active_tab_index(), 2);
        assert!(w.prev_tab());
        assert_eq!(w.active_tab_index(), 1);
    }

    #[test]
    fn next_prev_single_tab_returns_false() {
        let mut w = Window::new(0, "win", 80, 24);
        assert!(!w.next_tab());
        assert!(!w.prev_tab());
    }

    #[test]
    fn rename_window() {
        let mut w = Window::new(0, "old", 80, 24);
        w.rename("new");
        assert_eq!(w.name(), "new");
    }

    #[test]
    fn tab_names() {
        let mut w = Window::new(0, "win", 80, 24);
        w.add_tab("alpha");
        w.add_tab("beta");
        let names = w.tab_names();
        assert_eq!(names, vec!["Tab 1", "alpha", "beta"]);
    }

    #[test]
    fn remove_tab_by_id() {
        let mut w = Window::new(0, "win", 80, 24);
        let id = w.add_tab("removable");
        assert_eq!(w.tab_count(), 2);
        assert!(w.remove_tab_by_id(id));
        assert_eq!(w.tab_count(), 1);
    }

    #[test]
    fn resize_propagates() {
        let mut w = Window::new(0, "win", 80, 24);
        w.add_tab("second");
        w.resize(120, 40);
        assert_eq!(w.size().cols, 120);
        assert_eq!(w.size().rows, 40);
        // Tabs should be resized too
        assert_eq!(w.tab(0).unwrap().size().cols, 120);
        assert_eq!(w.tab(1).unwrap().size().cols, 120);
    }

    #[test]
    fn active_tab_accessors() {
        let mut w = Window::new(0, "win", 80, 24);
        assert_eq!(w.active_tab().name(), "Tab 1");
        w.active_tab_mut().rename("renamed");
        assert_eq!(w.active_tab().name(), "renamed");
    }
}
