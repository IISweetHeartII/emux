//! Tab abstraction — contains one layout and its panes.

use std::collections::HashMap;

use crate::layout::{LayoutEngine, LayoutNode, PanePosition, SplitDirection};
use crate::pane::{Pane, PaneId, PaneSize};

pub type TabId = u32;

/// Focus direction for directional navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Resize direction for pane resizing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Minimum pane dimensions.
const MIN_PANE_COLS: usize = 2;
const MIN_PANE_ROWS: usize = 2;

/// Minimum floating pane dimensions.
const MIN_FLOATING_COLS: usize = 5;
const MIN_FLOATING_ROWS: usize = 2;

/// A floating pane that hovers above the tiled layout.
#[derive(Debug)]
pub struct FloatingPane {
    pub pane: Pane,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub visible: bool,
}

/// A swap layout definition: a named layout template activated by pane count.
#[derive(Debug, Clone)]
pub struct SwapLayout {
    pub name: String,
    pub min_panes: Option<usize>,
    pub max_panes: Option<usize>,
    pub layout: LayoutNode,
}

/// A tab contains a layout engine and a set of panes.
#[derive(Debug)]
pub struct Tab {
    id: TabId,
    name: String,
    previous_name: Option<String>,
    layout: LayoutEngine,
    panes: HashMap<PaneId, Pane>,
    active_pane: Option<PaneId>,
    next_pane_id: PaneId,
    size: PaneSize,
    fullscreen_pane: Option<PaneId>,
    floating_panes: Vec<FloatingPane>,
    show_floating: bool,
    swap_layouts: Vec<SwapLayout>,
    current_swap_index: Option<usize>,
    pixel_width: Option<usize>,
    pixel_height: Option<usize>,
}

impl Tab {
    /// Create a new tab with one default pane.
    pub fn new(id: TabId, name: impl Into<String>, cols: usize, rows: usize) -> Self {
        let pane_id = 0;
        let pane = Pane::new(pane_id, cols, rows);
        let mut layout = LayoutEngine::new();
        layout.add_pane(pane_id);

        let mut panes = HashMap::new();
        panes.insert(pane_id, pane);

        Self {
            id,
            name: name.into(),
            previous_name: None,
            layout,
            panes,
            active_pane: Some(pane_id),
            next_pane_id: 1,
            size: PaneSize::new(cols, rows),
            fullscreen_pane: None,
            floating_panes: Vec::new(),
            show_floating: true,
            swap_layouts: Vec::new(),
            current_swap_index: None,
            pixel_width: None,
            pixel_height: None,
        }
    }

    /// Tab ID.
    pub fn id(&self) -> TabId {
        self.id
    }

    /// Tab name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Rename the tab.
    pub fn rename(&mut self, name: impl Into<String>) {
        self.previous_name = Some(self.name.clone());
        self.name = name.into();
    }

    /// Undo the last rename.
    pub fn undo_rename(&mut self) -> bool {
        if let Some(prev) = self.previous_name.take() {
            self.name = prev;
            true
        } else {
            false
        }
    }

    /// Number of panes.
    pub fn pane_count(&self) -> usize {
        self.panes.len()
    }

    /// Active pane ID.
    pub fn active_pane_id(&self) -> Option<PaneId> {
        self.active_pane
    }

    /// Get pane IDs in layout order.
    pub fn pane_ids(&self) -> Vec<PaneId> {
        self.layout.pane_ids()
    }

    /// Get a reference to a pane by ID.
    pub fn pane(&self, id: PaneId) -> Option<&Pane> {
        self.panes.get(&id)
    }

    /// Get a mutable reference to a pane by ID.
    pub fn pane_mut(&mut self, id: PaneId) -> Option<&mut Pane> {
        self.panes.get_mut(&id)
    }

    /// Get the active pane.
    pub fn active_pane(&self) -> Option<&Pane> {
        self.active_pane.and_then(|id| self.panes.get(&id))
    }

    /// Get the tab size.
    pub fn size(&self) -> PaneSize {
        self.size
    }

    /// Check if a pane can be split in the given direction.
    fn can_split(&self, pane_id: PaneId, direction: SplitDirection) -> bool {
        // Check constraints first
        if let Some(pane) = self.panes.get(&pane_id) {
            match direction {
                SplitDirection::Vertical => {
                    if pane.has_fixed_cols() {
                        return false;
                    }
                }
                SplitDirection::Horizontal => {
                    if pane.has_fixed_rows() {
                        return false;
                    }
                }
            }
        }

        let positions = self.layout.compute_positions(self.size.cols, self.size.rows);
        if let Some((_, pos)) = positions.iter().find(|(id, _)| *id == pane_id) {
            match direction {
                SplitDirection::Vertical => pos.cols >= MIN_PANE_COLS * 2,
                SplitDirection::Horizontal => pos.rows >= MIN_PANE_ROWS * 2,
            }
        } else {
            false
        }
    }

    /// Split the active pane in the given direction.
    /// Returns the new pane's ID, or None if split is impossible.
    pub fn split_pane(&mut self, direction: SplitDirection) -> Option<PaneId> {
        // Exit fullscreen if active
        if self.fullscreen_pane.is_some() {
            self.fullscreen_pane = None;
        }

        let active = self.active_pane?;

        if !self.can_split(active, direction) {
            return None;
        }

        let new_id = self.next_pane_id;
        self.next_pane_id += 1;

        // Compute the new pane's size based on the split
        let positions = self.layout.compute_positions(self.size.cols, self.size.rows);
        let active_pos = positions.iter().find(|(id, _)| *id == active).map(|(_, p)| *p)?;

        let (new_cols, new_rows) = match direction {
            SplitDirection::Horizontal => {
                let first_rows = active_pos.rows / 2;
                let second_rows = active_pos.rows - first_rows;
                (active_pos.cols, second_rows)
            }
            SplitDirection::Vertical => {
                let first_cols = active_pos.cols / 2;
                let second_cols = active_pos.cols - first_cols;
                (second_cols, active_pos.rows)
            }
        };

        let pane = Pane::new(new_id, new_cols, new_rows);
        self.panes.insert(new_id, pane);
        self.layout.split(active, new_id, direction);

        // Update the active pane's size too
        self.sync_pane_sizes();

        self.active_pane = Some(new_id);

        // Try auto swap layout
        self.auto_swap_layout();

        Some(new_id)
    }

    /// Close a pane by ID. Returns true if the pane was closed.
    /// Cannot close the last pane.
    pub fn close_pane(&mut self, pane_id: PaneId) -> bool {
        if self.panes.len() <= 1 {
            return false;
        }

        if !self.panes.contains_key(&pane_id) {
            return false;
        }

        // Exit fullscreen
        if self.fullscreen_pane == Some(pane_id) {
            self.fullscreen_pane = None;
        }

        self.layout.remove_pane(pane_id);
        self.panes.remove(&pane_id);

        // If we closed the active pane, focus the first remaining pane
        if self.active_pane == Some(pane_id) {
            self.active_pane = self.layout.pane_ids().first().copied();
        }

        // Update pane sizes
        self.sync_pane_sizes();

        // Try auto swap layout
        self.auto_swap_layout();

        true
    }

    /// Focus a specific pane.
    pub fn focus_pane(&mut self, pane_id: PaneId) -> bool {
        if self.panes.contains_key(&pane_id) {
            // Exit fullscreen when switching focus
            if self.fullscreen_pane.is_some() && self.fullscreen_pane != Some(pane_id) {
                self.fullscreen_pane = None;
            }
            self.active_pane = Some(pane_id);
            true
        } else if self.floating_panes.iter().any(|fp| fp.pane.id() == pane_id) {
            self.active_pane = Some(pane_id);
            self.bring_floating_to_front(pane_id);
            true
        } else {
            false
        }
    }

    /// Build the combined focus list: tiled IDs followed by visible floating IDs.
    fn focus_cycle_ids(&self) -> Vec<PaneId> {
        let mut ids = self.layout.pane_ids();
        if self.show_floating {
            ids.extend(self.floating_panes.iter().map(|fp| fp.pane.id()));
        }
        ids
    }

    /// Focus the next pane in layout order.
    pub fn focus_next(&mut self) -> bool {
        // Exit fullscreen
        if self.fullscreen_pane.is_some() {
            self.fullscreen_pane = None;
        }

        let ids = self.focus_cycle_ids();
        if ids.is_empty() {
            return false;
        }
        if let Some(active) = self.active_pane
            && let Some(pos) = ids.iter().position(|&id| id == active) {
                let next = (pos + 1) % ids.len();
                let next_id = ids[next];
                self.active_pane = Some(next_id);
                // Bring floating pane to front when focused
                if self.floating_panes.iter().any(|fp| fp.pane.id() == next_id) {
                    self.bring_floating_to_front(next_id);
                }
                return true;
            }
        self.active_pane = ids.first().copied();
        true
    }

    /// Focus the previous pane in layout order.
    pub fn focus_prev(&mut self) -> bool {
        // Exit fullscreen
        if self.fullscreen_pane.is_some() {
            self.fullscreen_pane = None;
        }

        let ids = self.focus_cycle_ids();
        if ids.is_empty() {
            return false;
        }
        if let Some(active) = self.active_pane
            && let Some(pos) = ids.iter().position(|&id| id == active) {
                let prev = if pos == 0 { ids.len() - 1 } else { pos - 1 };
                let prev_id = ids[prev];
                self.active_pane = Some(prev_id);
                if self.floating_panes.iter().any(|fp| fp.pane.id() == prev_id) {
                    self.bring_floating_to_front(prev_id);
                }
                return true;
            }
        self.active_pane = ids.last().copied();
        true
    }

    /// Move focus in a direction based on pane positions.
    /// Returns true if focus moved, false if at edge (caller may switch tabs).
    pub fn focus_direction(&mut self, direction: FocusDirection) -> bool {
        let active = match self.active_pane {
            Some(id) => id,
            None => return false,
        };

        let positions = self.layout.compute_positions(self.size.cols, self.size.rows);
        let active_pos = match positions.iter().find(|(id, _)| *id == active) {
            Some((_, pos)) => *pos,
            None => return false,
        };

        // Find the best candidate in the given direction
        let mut best: Option<(PaneId, usize)> = None; // (id, distance)

        // Center of the active pane
        let active_center_col = active_pos.col * 2 + active_pos.cols;
        let active_center_row = active_pos.row * 2 + active_pos.rows;

        for &(id, pos) in &positions {
            if id == active {
                continue;
            }
            let center_col = pos.col * 2 + pos.cols;
            let center_row = pos.row * 2 + pos.rows;

            let is_candidate = match direction {
                FocusDirection::Up => {
                    pos.row + pos.rows <= active_pos.row
                        && ranges_overlap(pos.col, pos.col + pos.cols, active_pos.col, active_pos.col + active_pos.cols)
                }
                FocusDirection::Down => {
                    pos.row >= active_pos.row + active_pos.rows
                        && ranges_overlap(pos.col, pos.col + pos.cols, active_pos.col, active_pos.col + active_pos.cols)
                }
                FocusDirection::Left => {
                    pos.col + pos.cols <= active_pos.col
                        && ranges_overlap(pos.row, pos.row + pos.rows, active_pos.row, active_pos.row + active_pos.rows)
                }
                FocusDirection::Right => {
                    pos.col >= active_pos.col + active_pos.cols
                        && ranges_overlap(pos.row, pos.row + pos.rows, active_pos.row, active_pos.row + active_pos.rows)
                }
            };

            if is_candidate {
                let dist = active_center_col.abs_diff(center_col)
                    + active_center_row.abs_diff(center_row);
                if best.is_none() || dist < best.unwrap().1 {
                    best = Some((id, dist));
                }
            }
        }

        if let Some((target_id, _)) = best {
            if self.fullscreen_pane.is_some() {
                self.fullscreen_pane = None;
            }
            self.active_pane = Some(target_id);
            true
        } else {
            false
        }
    }

    /// Recompute and apply pane sizes from the layout engine.
    fn sync_pane_sizes(&mut self) {
        let positions = self.layout.compute_positions(self.size.cols, self.size.rows);
        for (id, pos) in &positions {
            if let Some(p) = self.panes.get_mut(id) {
                p.resize(pos.cols, pos.rows);
            }
        }
    }

    /// Resize the whole tab.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.size = PaneSize::new(cols, rows);
        // Recompute all pane sizes
        self.sync_pane_sizes();
    }

    /// Resize a specific pane in a direction by an amount (in character cells).
    /// The pane grows in the specified direction (taking space from the neighbor).
    pub fn resize_pane(&mut self, pane_id: PaneId, direction: ResizeDirection, amount: i32) -> bool {
        if let Some(pane) = self.panes.get(&pane_id) {
            // Refuse resizing if the pane has fixed constraints on the relevant axis
            match direction {
                ResizeDirection::Up | ResizeDirection::Down => {
                    if pane.has_fixed_rows() {
                        return false;
                    }
                }
                ResizeDirection::Left | ResizeDirection::Right => {
                    if pane.has_fixed_cols() {
                        return false;
                    }
                }
            }
        } else {
            return false;
        }

        // Map resize direction to layout axis
        // "grow" delta is always positive — adjust_ratio handles the branch side
        let axis = match direction {
            ResizeDirection::Down | ResizeDirection::Up => SplitDirection::Horizontal,
            ResizeDirection::Right | ResizeDirection::Left => SplitDirection::Vertical,
        };

        // Convert amount in cells to a ratio delta (always positive = grow this pane)
        let total = match axis {
            SplitDirection::Horizontal => self.size.rows as f32,
            SplitDirection::Vertical => self.size.cols as f32,
        };
        if total <= 0.0 {
            return false;
        }
        let delta = amount as f32 / total;

        let adjusted = self.layout.adjust_ratio(pane_id, axis, delta);
        if adjusted {
            self.sync_pane_sizes();
        }
        adjusted
    }

    /// Toggle fullscreen for the active pane.
    pub fn toggle_fullscreen(&mut self) -> bool {
        if let Some(fs) = self.fullscreen_pane.take() {
            // Exiting fullscreen
            let _ = fs;
            true
        } else if let Some(active) = self.active_pane {
            self.fullscreen_pane = Some(active);
            true
        } else {
            false
        }
    }

    /// Check if a pane is in fullscreen mode.
    pub fn is_fullscreen(&self) -> bool {
        self.fullscreen_pane.is_some()
    }

    /// Get the fullscreen pane ID.
    pub fn fullscreen_pane_id(&self) -> Option<PaneId> {
        self.fullscreen_pane
    }

    /// Compute positions, respecting fullscreen state.
    /// Floating panes appear after tiled panes in the returned list (higher z-order).
    pub fn compute_positions(&self) -> Vec<(PaneId, PanePosition)> {
        let mut positions = if let Some(fs_id) = self.fullscreen_pane {
            vec![(fs_id, PanePosition {
                col: 0,
                row: 0,
                cols: self.size.cols,
                rows: self.size.rows,
            })]
        } else {
            self.layout.compute_positions(self.size.cols, self.size.rows)
        };

        // Append visible floating panes in z-order
        if self.show_floating {
            for fp in &self.floating_panes {
                if fp.visible {
                    positions.push((fp.pane.id(), PanePosition {
                        col: fp.x,
                        row: fp.y,
                        cols: fp.width,
                        rows: fp.height,
                    }));
                }
            }
        }

        positions
    }

    /// Get the layout engine.
    pub fn layout(&self) -> &LayoutEngine {
        &self.layout
    }

    // -----------------------------------------------------------------------
    // Swap layout support
    // -----------------------------------------------------------------------

    /// Register a swap layout with a name and pane count range.
    /// If a layout with the same name already exists, it is replaced.
    pub fn register_swap_layout(
        &mut self,
        name: impl Into<String>,
        min_panes: Option<usize>,
        max_panes: Option<usize>,
        layout: LayoutNode,
    ) {
        let name = name.into();
        // Replace existing layout with the same name
        if let Some(existing) = self.swap_layouts.iter_mut().find(|sl| sl.name == name) {
            existing.min_panes = min_panes;
            existing.max_panes = max_panes;
            existing.layout = layout;
        } else {
            self.swap_layouts.push(SwapLayout {
                name,
                min_panes,
                max_panes,
                layout,
            });
        }
    }

    /// Get the registered swap layouts.
    pub fn swap_layouts(&self) -> &[SwapLayout] {
        &self.swap_layouts
    }

    /// Called after pane count changes to automatically apply a matching layout.
    pub fn auto_swap_layout(&mut self) {
        let count = self.panes.len();
        // Find the first layout whose range includes the current pane count
        let matching_index = self.swap_layouts.iter().position(|sl| {
            let min_ok = sl.min_panes.is_none_or(|min| count >= min);
            let max_ok = sl.max_panes.is_none_or(|max| count <= max);
            min_ok && max_ok
        });

        if let Some(idx) = matching_index {
            // Only apply if it's different from the current swap layout
            if self.current_swap_index != Some(idx) {
                self.apply_swap_layout_at(idx);
            }
        } else {
            self.current_swap_index = None;
        }
    }

    /// Manually cycle to the next registered swap layout.
    pub fn next_swap_layout(&mut self) {
        if self.swap_layouts.is_empty() {
            return;
        }
        let count = self.panes.len();
        let applicable: Vec<usize> = self.swap_layouts.iter().enumerate()
            .filter(|(_, sl)| {
                let min_ok = sl.min_panes.is_none_or(|min| count >= min);
                let max_ok = sl.max_panes.is_none_or(|max| count <= max);
                min_ok && max_ok
            })
            .map(|(i, _)| i)
            .collect();
        if applicable.is_empty() {
            return;
        }
        let current_pos = self.current_swap_index
            .and_then(|idx| applicable.iter().position(|&i| i == idx));
        let next_pos = match current_pos {
            Some(pos) => (pos + 1) % applicable.len(),
            None => 0,
        };
        self.apply_swap_layout_at(applicable[next_pos]);
    }

    /// Manually cycle to the previous registered swap layout.
    pub fn prev_swap_layout(&mut self) {
        if self.swap_layouts.is_empty() {
            return;
        }
        let count = self.panes.len();
        let applicable: Vec<usize> = self.swap_layouts.iter().enumerate()
            .filter(|(_, sl)| {
                let min_ok = sl.min_panes.is_none_or(|min| count >= min);
                let max_ok = sl.max_panes.is_none_or(|max| count <= max);
                min_ok && max_ok
            })
            .map(|(i, _)| i)
            .collect();
        if applicable.is_empty() {
            return;
        }
        let current_pos = self.current_swap_index
            .and_then(|idx| applicable.iter().position(|&i| i == idx));
        let prev_pos = match current_pos {
            Some(pos) => {
                if pos == 0 { applicable.len() - 1 } else { pos - 1 }
            }
            None => applicable.len() - 1,
        };
        self.apply_swap_layout_at(applicable[prev_pos]);
    }

    /// Get the name of the currently active swap layout.
    pub fn current_swap_layout_name(&self) -> Option<&str> {
        self.current_swap_index
            .and_then(|idx| self.swap_layouts.get(idx))
            .map(|sl| sl.name.as_str())
    }

    /// Apply the swap layout at the given index.
    fn apply_swap_layout_at(&mut self, idx: usize) {
        let template = self.swap_layouts[idx].layout.clone();
        let pane_ids = self.layout.pane_ids();
        self.layout.apply_template(&template, &pane_ids);
        self.current_swap_index = Some(idx);
        self.sync_pane_sizes();
    }

    // -----------------------------------------------------------------------
    // Floating pane support
    // -----------------------------------------------------------------------

    /// Create a floating pane with default size (50% of tab) centered.
    pub fn new_floating_pane(&mut self) -> PaneId {
        let w = (self.size.cols / 2).max(MIN_FLOATING_COLS);
        let h = (self.size.rows / 2).max(MIN_FLOATING_ROWS);
        let x = (self.size.cols.saturating_sub(w)) / 2;
        let y = (self.size.rows.saturating_sub(h)) / 2;
        self.new_floating_pane_with_coords(x, y, w, h)
    }

    /// Create a floating pane with custom coordinates, clamped to the viewport.
    pub fn new_floating_pane_with_coords(&mut self, x: usize, y: usize, w: usize, h: usize) -> PaneId {
        let id = self.next_pane_id;
        self.next_pane_id += 1;

        // Clamp dimensions to viewport
        let w = w.max(MIN_FLOATING_COLS).min(self.size.cols);
        let h = h.max(MIN_FLOATING_ROWS).min(self.size.rows);
        let x = x.min(self.size.cols.saturating_sub(w));
        let y = y.min(self.size.rows.saturating_sub(h));

        let pane = Pane::new(id, w, h);
        let fp = FloatingPane {
            pane,
            x,
            y,
            width: w,
            height: h,
            visible: true,
        };
        self.floating_panes.push(fp);
        self.show_floating = true;
        self.active_pane = Some(id);
        id
    }

    /// Toggle visibility of all floating panes.
    pub fn toggle_floating_panes(&mut self) {
        self.show_floating = !self.show_floating;
        // When hiding floating panes, move focus to tiled if currently on a floating pane
        if !self.show_floating {
            if let Some(active) = self.active_pane
                && self.floating_panes.iter().any(|fp| fp.pane.id() == active) {
                    self.active_pane = self.layout.pane_ids().first().copied();
                }
        } else {
            // When showing, focus the top floating pane if any
            if let Some(fp) = self.floating_panes.last() {
                self.active_pane = Some(fp.pane.id());
            }
        }
    }

    /// Whether floating panes are currently visible.
    pub fn is_floating_visible(&self) -> bool {
        self.show_floating
    }

    /// Number of floating panes (regardless of visibility toggle).
    pub fn floating_pane_count(&self) -> usize {
        self.floating_panes.len()
    }

    /// Close and remove a floating pane by ID.
    pub fn close_floating_pane(&mut self, id: PaneId) -> bool {
        let idx = self.floating_panes.iter().position(|fp| fp.pane.id() == id);
        if let Some(idx) = idx {
            self.floating_panes.remove(idx);
            if self.active_pane == Some(id) {
                // Focus the next floating pane or fall back to tiled
                self.active_pane = self.floating_panes.last().map(|fp| fp.pane.id())
                    .or_else(|| self.layout.pane_ids().first().copied());
            }
            true
        } else {
            false
        }
    }

    /// Embed a floating pane into the tiled layout.
    /// The pane is removed from floating and added to the tiled layout tree.
    pub fn embed_floating_pane(&mut self, id: PaneId) -> bool {
        let idx = self.floating_panes.iter().position(|fp| fp.pane.id() == id);
        if let Some(idx) = idx {
            let fp = self.floating_panes.remove(idx);
            self.panes.insert(id, fp.pane);
            self.layout.add_pane(id);
            self.sync_pane_sizes();
            self.active_pane = Some(id);
            true
        } else {
            false
        }
    }

    /// Float a tiled pane (move it from tiled layout to floating).
    pub fn float_pane(&mut self, id: PaneId) -> bool {
        if !self.panes.contains_key(&id) {
            return false;
        }
        // Don't allow floating the last tiled pane
        if self.panes.len() <= 1 {
            return false;
        }
        let pane = self.panes.remove(&id).unwrap();
        self.layout.remove_pane(id);
        self.sync_pane_sizes();

        let w = (self.size.cols / 2).max(MIN_FLOATING_COLS);
        let h = (self.size.rows / 2).max(MIN_FLOATING_ROWS);
        let x = (self.size.cols.saturating_sub(w)) / 2;
        let y = (self.size.rows.saturating_sub(h)) / 2;

        let mut fp_pane = pane;
        fp_pane.resize(w, h);
        let fp = FloatingPane {
            pane: fp_pane,
            x,
            y,
            width: w,
            height: h,
            visible: true,
        };
        self.floating_panes.push(fp);
        self.show_floating = true;
        self.active_pane = Some(id);
        true
    }

    /// Resize a floating pane. Dimensions are clamped to minimums and viewport.
    pub fn resize_floating_pane(&mut self, id: PaneId, width: usize, height: usize) -> bool {
        if let Some(fp) = self.floating_panes.iter_mut().find(|fp| fp.pane.id() == id) {
            let w = width.max(MIN_FLOATING_COLS).min(self.size.cols);
            let h = height.max(MIN_FLOATING_ROWS).min(self.size.rows);
            // Re-clamp position so pane stays in viewport
            fp.x = fp.x.min(self.size.cols.saturating_sub(w));
            fp.y = fp.y.min(self.size.rows.saturating_sub(h));
            fp.width = w;
            fp.height = h;
            fp.pane.resize(w, h);
            true
        } else {
            false
        }
    }

    /// Move a floating pane to a new position, clamped to the viewport.
    pub fn move_floating_pane(&mut self, id: PaneId, x: usize, y: usize) -> bool {
        if let Some(fp) = self.floating_panes.iter_mut().find(|fp| fp.pane.id() == id) {
            fp.x = x.min(self.size.cols.saturating_sub(fp.width));
            fp.y = y.min(self.size.rows.saturating_sub(fp.height));
            true
        } else {
            false
        }
    }

    /// Get a reference to a floating pane by ID.
    pub fn floating_pane(&self, id: PaneId) -> Option<&FloatingPane> {
        self.floating_panes.iter().find(|fp| fp.pane.id() == id)
    }

    /// Get a mutable reference to a floating pane by ID.
    pub fn floating_pane_mut(&mut self, id: PaneId) -> Option<&mut FloatingPane> {
        self.floating_panes.iter_mut().find(|fp| fp.pane.id() == id)
    }

    /// Get floating pane IDs in z-order (first = bottom, last = top).
    pub fn floating_pane_ids(&self) -> Vec<PaneId> {
        self.floating_panes.iter().map(|fp| fp.pane.id()).collect()
    }

    /// Check if two floating panes overlap. Returns true if they do.
    pub fn floating_panes_overlap(&self, a: PaneId, b: PaneId) -> bool {
        let fa = self.floating_panes.iter().find(|fp| fp.pane.id() == a);
        let fb = self.floating_panes.iter().find(|fp| fp.pane.id() == b);
        if let (Some(fa), Some(fb)) = (fa, fb) {
            let a_right = fa.x + fa.width;
            let a_bottom = fa.y + fa.height;
            let b_right = fb.x + fb.width;
            let b_bottom = fb.y + fb.height;
            fa.x < b_right && fb.x < a_right && fa.y < b_bottom && fb.y < a_bottom
        } else {
            false
        }
    }

    /// Find all pairs of overlapping floating panes.
    pub fn overlapping_floating_panes(&self) -> Vec<(PaneId, PaneId)> {
        let mut result = Vec::new();
        for i in 0..self.floating_panes.len() {
            for j in (i + 1)..self.floating_panes.len() {
                let a_id = self.floating_panes[i].pane.id();
                let b_id = self.floating_panes[j].pane.id();
                if self.floating_panes_overlap(a_id, b_id) {
                    result.push((a_id, b_id));
                }
            }
        }
        result
    }

    /// Bring a floating pane to the top of the z-order.
    pub fn bring_floating_to_front(&mut self, id: PaneId) -> bool {
        let idx = self.floating_panes.iter().position(|fp| fp.pane.id() == id);
        if let Some(idx) = idx {
            let fp = self.floating_panes.remove(idx);
            self.floating_panes.push(fp);
            true
        } else {
            false
        }
    }

    /// Toggle fullscreen for a specific pane by ID.
    pub fn toggle_fullscreen_by_id(&mut self, pane_id: PaneId) -> bool {
        if !self.panes.contains_key(&pane_id) {
            return false;
        }
        if self.fullscreen_pane == Some(pane_id) {
            self.fullscreen_pane = None;
        } else {
            self.fullscreen_pane = Some(pane_id);
            self.active_pane = Some(pane_id);
        }
        true
    }

    /// Split the largest pane (by area). Returns the new pane ID.
    pub fn split_largest_pane(&mut self) -> Option<PaneId> {
        // Exit fullscreen
        if self.fullscreen_pane.is_some() {
            self.fullscreen_pane = None;
        }

        let positions = self.layout.compute_positions(self.size.cols, self.size.rows);
        // Find the pane with the largest area
        let largest = positions.iter()
            .max_by_key(|(_, pos)| pos.cols * pos.rows)?;
        let target_id = largest.0;

        // Decide split direction based on aspect ratio
        let target_pos = largest.1;
        let direction = if target_pos.cols >= target_pos.rows {
            SplitDirection::Vertical
        } else {
            SplitDirection::Horizontal
        };

        // Focus the target first, then split
        let old_active = self.active_pane;
        self.active_pane = Some(target_id);
        let result = self.split_pane(direction);
        if result.is_none() {
            self.active_pane = old_active;
        }
        result
    }

    /// Move the active pane in a direction by swapping with the neighbor.
    pub fn move_active_pane(&mut self, direction: FocusDirection) -> bool {
        let active = match self.active_pane {
            Some(id) => id,
            None => return false,
        };

        let positions = self.layout.compute_positions(self.size.cols, self.size.rows);
        let active_pos = match positions.iter().find(|(id, _)| *id == active) {
            Some((_, pos)) => *pos,
            None => return false,
        };

        // Find neighbor in the given direction (same logic as focus_direction)
        let active_center_col = active_pos.col * 2 + active_pos.cols;
        let active_center_row = active_pos.row * 2 + active_pos.rows;
        let mut best: Option<(PaneId, usize)> = None;

        for &(id, pos) in &positions {
            if id == active { continue; }
            let center_col = pos.col * 2 + pos.cols;
            let center_row = pos.row * 2 + pos.rows;
            let is_candidate = match direction {
                FocusDirection::Up => pos.row + pos.rows <= active_pos.row
                    && ranges_overlap(pos.col, pos.col + pos.cols, active_pos.col, active_pos.col + active_pos.cols),
                FocusDirection::Down => pos.row >= active_pos.row + active_pos.rows
                    && ranges_overlap(pos.col, pos.col + pos.cols, active_pos.col, active_pos.col + active_pos.cols),
                FocusDirection::Left => pos.col + pos.cols <= active_pos.col
                    && ranges_overlap(pos.row, pos.row + pos.rows, active_pos.row, active_pos.row + active_pos.rows),
                FocusDirection::Right => pos.col >= active_pos.col + active_pos.cols
                    && ranges_overlap(pos.row, pos.row + pos.rows, active_pos.row, active_pos.row + active_pos.rows),
            };
            if is_candidate {
                let dist = active_center_col.abs_diff(center_col) + active_center_row.abs_diff(center_row);
                if best.is_none() || dist < best.unwrap().1 {
                    best = Some((id, dist));
                }
            }
        }

        if let Some((neighbor_id, _)) = best {
            // Swap in the layout tree
            let swapped = self.layout.swap_leaves(active, neighbor_id);
            if swapped {
                self.sync_pane_sizes();
            }
            swapped
        } else {
            false
        }
    }

    /// Move a specific pane in a direction by swapping with neighbor.
    pub fn move_pane_by_id(&mut self, pane_id: PaneId, direction: FocusDirection) -> bool {
        let old_active = self.active_pane;
        self.active_pane = Some(pane_id);
        let result = self.move_active_pane(direction);
        if !result {
            self.active_pane = old_active;
        }
        result
    }

    /// Move a pane backwards in layout order (swap with previous pane).
    pub fn move_pane_backwards(&mut self, pane_id: PaneId) -> bool {
        let ids = self.layout.pane_ids();
        if let Some(pos) = ids.iter().position(|&id| id == pane_id)
            && pos > 0 {
                let prev_id = ids[pos - 1];
                let swapped = self.layout.swap_leaves(pane_id, prev_id);
                if swapped {
                    self.sync_pane_sizes();
                }
                return swapped;
            }
        false
    }

    /// Set the pixel dimensions of the tab viewport.
    pub fn set_pixel_dimensions(&mut self, pixel_width: usize, pixel_height: usize) {
        self.pixel_width = Some(pixel_width);
        self.pixel_height = Some(pixel_height);
    }

    /// Get the pixel dimensions.
    pub fn pixel_dimensions(&self) -> (Option<usize>, Option<usize>) {
        (self.pixel_width, self.pixel_height)
    }

    /// Scroll up by pane ID.
    pub fn scroll_up(&mut self, pane_id: PaneId, lines: usize) -> bool {
        if let Some(pane) = self.panes.get_mut(&pane_id) {
            pane.scroll_up(lines);
            true
        } else {
            false
        }
    }

    /// Scroll down by pane ID.
    pub fn scroll_down(&mut self, pane_id: PaneId, lines: usize) -> bool {
        if let Some(pane) = self.panes.get_mut(&pane_id) {
            pane.scroll_down(lines);
            true
        } else {
            false
        }
    }

    /// Scroll to top by pane ID.
    pub fn scroll_to_top(&mut self, pane_id: PaneId) -> bool {
        if let Some(pane) = self.panes.get_mut(&pane_id) {
            pane.scroll_to_top();
            true
        } else {
            false
        }
    }

    /// Scroll to bottom by pane ID.
    pub fn scroll_to_bottom(&mut self, pane_id: PaneId) -> bool {
        if let Some(pane) = self.panes.get_mut(&pane_id) {
            pane.scroll_to_bottom();
            true
        } else {
            false
        }
    }

    /// Clear the screen of a specific pane (reset its title as a proxy).
    pub fn clear_pane(&mut self, pane_id: PaneId) -> bool {
        if let Some(pane) = self.panes.get_mut(&pane_id) {
            // In a real terminal, this would clear the screen buffer.
            // For now we mark it as cleared.
            pane.set_cleared(true);
            true
        } else {
            false
        }
    }

    /// Send a floating pane to the back (lowest z-order).
    pub fn send_floating_to_back(&mut self, id: PaneId) -> bool {
        let idx = self.floating_panes.iter().position(|fp| fp.pane.id() == id);
        if let Some(idx) = idx {
            let fp = self.floating_panes.remove(idx);
            self.floating_panes.insert(0, fp);
            true
        } else {
            false
        }
    }

    /// Swap the z-order of two floating panes.
    pub fn swap_floating_z_order(&mut self, a: PaneId, b: PaneId) -> bool {
        let idx_a = self.floating_panes.iter().position(|fp| fp.pane.id() == a);
        let idx_b = self.floating_panes.iter().position(|fp| fp.pane.id() == b);
        if let (Some(ia), Some(ib)) = (idx_a, idx_b) {
            self.floating_panes.swap(ia, ib);
            true
        } else {
            false
        }
    }
}

/// Check if two ranges [a_start, a_end) and [b_start, b_end) overlap.
fn ranges_overlap(a_start: usize, a_end: usize, b_start: usize, b_end: usize) -> bool {
    a_start < b_end && b_start < a_end
}
