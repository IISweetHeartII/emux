//! Session management — groups of tabs.

use crate::pane::PaneSize;
use crate::project::ProjectInfo;
use crate::tab::{Tab, TabId};

pub type SessionId = u64;

/// A session contains multiple tabs and tracks the active one.
#[derive(Debug)]
pub struct Session {
    id: SessionId,
    name: String,
    previous_name: Option<String>,
    tabs: Vec<Tab>,
    active_tab: usize,
    previous_tab: Option<usize>,
    next_tab_id: TabId,
    size: PaneSize,
    project: Option<ProjectInfo>,
}

impl Session {
    /// Create a new session with one default tab.
    pub fn new(name: impl Into<String>, cols: usize, rows: usize) -> Self {
        let name = name.into();
        let tab = Tab::new(0, "Tab 1", cols, rows);
        Self {
            id: 0,
            name,
            previous_name: None,
            tabs: vec![tab],
            active_tab: 0,
            previous_tab: None,
            next_tab_id: 1,
            size: PaneSize::new(cols, rows),
            project: None,
        }
    }

    /// Create a new session with a specific ID.
    pub fn with_id(id: SessionId, name: impl Into<String>, cols: usize, rows: usize) -> Self {
        let mut session = Self::new(name, cols, rows);
        session.id = id;
        session
    }

    /// Session ID.
    pub fn id(&self) -> SessionId {
        self.id
    }

    /// Session name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Rename the session.
    pub fn rename(&mut self, name: impl Into<String>) {
        self.previous_name = Some(self.name.clone());
        self.name = name.into();
    }

    /// Set the project associated with this session.
    pub fn set_project(&mut self, project: ProjectInfo) {
        self.project = Some(project);
    }

    /// Get the project name, if a project is associated.
    pub fn project_name(&self) -> Option<&str> {
        self.project.as_ref().map(|p| p.name.as_str())
    }

    /// Get the current git branch of the associated project.
    pub fn git_branch(&self) -> Option<&str> {
        self.project.as_ref().and_then(|p| p.branch.as_deref())
    }

    /// Get the associated project info.
    pub fn project(&self) -> Option<&ProjectInfo> {
        self.project.as_ref()
    }

    /// Add a new tab and make it active.
    pub fn new_tab(&mut self, name: impl Into<String>) -> TabId {
        let tab_id = self.next_tab_id;
        self.next_tab_id += 1;
        let tab = Tab::new(tab_id, name, self.size.cols, self.size.rows);
        self.tabs.push(tab);
        self.active_tab = self.tabs.len() - 1;
        tab_id
    }

    /// Close a tab by index. Returns true if closed.
    /// Cannot close the last tab.
    pub fn close_tab(&mut self, index: usize) -> bool {
        if self.tabs.len() <= 1 || index >= self.tabs.len() {
            return false;
        }
        self.tabs.remove(index);
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        } else if self.active_tab > index {
            self.active_tab -= 1;
        } else if self.active_tab == index && self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        }
        true
    }

    /// Switch to a tab by index.
    pub fn switch_tab(&mut self, index: usize) -> bool {
        if index < self.tabs.len() {
            if index != self.active_tab {
                self.previous_tab = Some(self.active_tab);
            }
            self.active_tab = index;
            true
        } else {
            false
        }
    }

    /// Switch to the next tab.
    pub fn next_tab(&mut self) -> bool {
        if self.tabs.len() <= 1 {
            return false;
        }
        self.active_tab = (self.active_tab + 1) % self.tabs.len();
        true
    }

    /// Switch to the previous tab.
    pub fn prev_tab(&mut self) -> bool {
        if self.tabs.len() <= 1 {
            return false;
        }
        self.active_tab = if self.active_tab == 0 {
            self.tabs.len() - 1
        } else {
            self.active_tab - 1
        };
        true
    }

    /// Get the active tab index.
    pub fn active_tab_index(&self) -> usize {
        self.active_tab
    }

    /// Get a reference to the active tab.
    pub fn active_tab(&self) -> &Tab {
        &self.tabs[self.active_tab]
    }

    /// Get a mutable reference to the active tab.
    pub fn active_tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active_tab]
    }

    /// Get a reference to a tab by index.
    pub fn tab(&self, index: usize) -> Option<&Tab> {
        self.tabs.get(index)
    }

    /// Get a mutable reference to a tab by index.
    pub fn tab_mut(&mut self, index: usize) -> Option<&mut Tab> {
        self.tabs.get_mut(index)
    }

    /// Number of tabs.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Get tab names.
    pub fn tab_names(&self) -> Vec<&str> {
        self.tabs.iter().map(|t| t.name()).collect()
    }

    /// Resize the session (all tabs).
    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.size = PaneSize::new(cols, rows);
        for tab in &mut self.tabs {
            tab.resize(cols, rows);
        }
    }

    /// Get session size.
    pub fn size(&self) -> PaneSize {
        self.size
    }

    /// Toggle to the previously active tab.
    pub fn toggle_previous_tab(&mut self) -> bool {
        if let Some(prev) = self.previous_tab
            && prev < self.tabs.len()
        {
            let current = self.active_tab;
            self.active_tab = prev;
            self.previous_tab = Some(current);
            return true;
        }
        false
    }

    /// Move the active tab to the left.
    pub fn move_tab_left(&mut self) -> bool {
        if self.active_tab == 0 || self.tabs.len() <= 1 {
            return false;
        }
        self.tabs.swap(self.active_tab, self.active_tab - 1);
        self.active_tab -= 1;
        true
    }

    /// Move the active tab to the right.
    pub fn move_tab_right(&mut self) -> bool {
        if self.active_tab >= self.tabs.len() - 1 || self.tabs.len() <= 1 {
            return false;
        }
        self.tabs.swap(self.active_tab, self.active_tab + 1);
        self.active_tab += 1;
        true
    }

    /// Move the active tab to the left with wrapping.
    pub fn move_tab_left_wrapping(&mut self) -> bool {
        if self.tabs.len() <= 1 {
            return false;
        }
        if self.active_tab == 0 {
            // Wrap: move first tab to end
            let tab = self.tabs.remove(0);
            self.tabs.push(tab);
            self.active_tab = self.tabs.len() - 1;
        } else {
            self.tabs.swap(self.active_tab, self.active_tab - 1);
            self.active_tab -= 1;
        }
        true
    }

    /// Move the active tab to the right with wrapping.
    pub fn move_tab_right_wrapping(&mut self) -> bool {
        if self.tabs.len() <= 1 {
            return false;
        }
        if self.active_tab >= self.tabs.len() - 1 {
            // Wrap: move last tab to beginning
            let tab = self.tabs.remove(self.active_tab);
            self.tabs.insert(0, tab);
            self.active_tab = 0;
        } else {
            self.tabs.swap(self.active_tab, self.active_tab + 1);
            self.active_tab += 1;
        }
        true
    }

    /// Close a tab by its TabId. Returns true if closed.
    pub fn close_tab_by_id(&mut self, tab_id: TabId) -> bool {
        if self.tabs.len() <= 1 {
            return false;
        }
        if let Some(idx) = self.tabs.iter().position(|t| t.id() == tab_id) {
            self.close_tab(idx)
        } else {
            false
        }
    }

    /// Break a pane from the active tab into a new tab.
    /// Returns the new tab's ID, or None if the active tab has only one pane.
    pub fn break_pane_to_new_tab(&mut self, pane_id: crate::pane::PaneId) -> Option<TabId> {
        let tab = &self.tabs[self.active_tab];
        // Can't break the last pane
        if tab.pane_count() <= 1 {
            return None;
        }
        // Verify pane exists
        tab.pane(pane_id)?;

        // Close the pane from the current tab
        self.tabs[self.active_tab].close_pane(pane_id);

        // Create a new tab (it will have its own initial pane)
        let tab_id = self.next_tab_id;
        self.next_tab_id += 1;
        let new_tab = Tab::new(
            tab_id,
            format!("Tab {}", tab_id + 1),
            self.size.cols,
            self.size.rows,
        );
        // Insert new tab to the right of the active tab
        let insert_pos = self.active_tab + 1;
        self.tabs.insert(insert_pos, new_tab);
        self.previous_tab = Some(self.active_tab);
        self.active_tab = insert_pos;

        Some(tab_id)
    }

    /// Move the active pane of the current tab to a new tab on the right.
    pub fn move_pane_to_new_tab_right(&mut self) -> Option<TabId> {
        let active_pane = self.tabs[self.active_tab].active_pane_id()?;
        self.break_pane_to_new_tab(active_pane)
    }

    /// Undo rename of the active tab.
    pub fn undo_rename_tab(&mut self) -> bool {
        let tab = &mut self.tabs[self.active_tab];
        // Tab stores previous name internally
        tab.undo_rename()
    }
}
