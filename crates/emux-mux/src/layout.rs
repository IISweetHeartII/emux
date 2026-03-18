//! Layout engine for arranging panes within a window.

use crate::pane::PaneId;

/// Direction of a split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    /// Split horizontally: panes are stacked top/bottom.
    Horizontal,
    /// Split vertically: panes are side by side left/right.
    Vertical,
}

/// A node in the binary layout tree.
#[derive(Debug, Clone)]
pub enum LayoutNode {
    Leaf(PaneId),
    Split {
        direction: SplitDirection,
        ratio: f32,
        first: Box<LayoutNode>,
        second: Box<LayoutNode>,
    },
}

/// Absolute position and size of a pane within the tab area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanePosition {
    pub col: usize,
    pub row: usize,
    pub cols: usize,
    pub rows: usize,
}

impl LayoutNode {
    /// Collect all pane IDs in this subtree (left-to-right / top-to-bottom order).
    pub fn pane_ids(&self) -> Vec<PaneId> {
        match self {
            LayoutNode::Leaf(id) => vec![*id],
            LayoutNode::Split { first, second, .. } => {
                let mut ids = first.pane_ids();
                ids.extend(second.pane_ids());
                ids
            }
        }
    }

    /// Count leaves.
    pub fn count(&self) -> usize {
        match self {
            LayoutNode::Leaf(_) => 1,
            LayoutNode::Split { first, second, .. } => first.count() + second.count(),
        }
    }

    /// Split a target pane, returning true if found and split.
    fn split(&mut self, target: PaneId, new_pane: PaneId, direction: SplitDirection) -> bool {
        match self {
            LayoutNode::Leaf(id) if *id == target => {
                let old = LayoutNode::Leaf(target);
                let new = LayoutNode::Leaf(new_pane);
                *self = LayoutNode::Split {
                    direction,
                    ratio: 0.5,
                    first: Box::new(old),
                    second: Box::new(new),
                };
                true
            }
            LayoutNode::Leaf(_) => false,
            LayoutNode::Split { first, second, .. } => {
                first.split(target, new_pane, direction)
                    || second.split(target, new_pane, direction)
            }
        }
    }

    /// Remove a pane, returning the collapsed node (or None if this was the leaf).
    fn remove(&mut self, target: PaneId) -> RemoveResult {
        match self {
            LayoutNode::Leaf(id) if *id == target => RemoveResult::Removed,
            LayoutNode::Leaf(_) => RemoveResult::NotFound,
            LayoutNode::Split { first, second, .. } => {
                let first_result = first.remove(target);
                match first_result {
                    RemoveResult::Removed => {
                        // First child was the target; collapse to second.
                        RemoveResult::Replaced(second.as_ref().clone())
                    }
                    RemoveResult::Replaced(replacement) => {
                        **first = replacement;
                        RemoveResult::NotFound // signal: handled internally
                    }
                    RemoveResult::NotFound => {
                        let second_result = second.remove(target);
                        match second_result {
                            RemoveResult::Removed => {
                                RemoveResult::Replaced(first.as_ref().clone())
                            }
                            RemoveResult::Replaced(replacement) => {
                                **second = replacement;
                                RemoveResult::NotFound // handled internally
                            }
                            RemoveResult::NotFound => RemoveResult::NotFound,
                        }
                    }
                }
            }
        }
    }

    /// Compute positions for all panes given a bounding rectangle.
    fn compute_positions_inner(
        &self,
        col: usize,
        row: usize,
        cols: usize,
        rows: usize,
        out: &mut Vec<(PaneId, PanePosition)>,
    ) {
        match self {
            LayoutNode::Leaf(id) => {
                out.push((*id, PanePosition { col, row, cols, rows }));
            }
            LayoutNode::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                match direction {
                    SplitDirection::Horizontal => {
                        let first_rows = ((rows as f32) * ratio).round() as usize;
                        let first_rows = first_rows.max(1).min(rows.saturating_sub(1));
                        let second_rows = rows - first_rows;
                        first.compute_positions_inner(col, row, cols, first_rows, out);
                        second.compute_positions_inner(col, row + first_rows, cols, second_rows, out);
                    }
                    SplitDirection::Vertical => {
                        let first_cols = ((cols as f32) * ratio).round() as usize;
                        let first_cols = first_cols.max(1).min(cols.saturating_sub(1));
                        let second_cols = cols - first_cols;
                        first.compute_positions_inner(col, row, first_cols, rows, out);
                        second.compute_positions_inner(col + first_cols, row, second_cols, rows, out);
                    }
                }
            }
        }
    }

    /// Adjust the split ratio of the nearest ancestor of `target` that splits
    /// along `axis`. Returns true if a ratio was adjusted.
    fn adjust_ratio(&mut self, target: PaneId, axis: SplitDirection, delta: f32) -> bool {
        match self {
            LayoutNode::Leaf(_) => false,
            LayoutNode::Split { direction, ratio, first, second } => {
                let in_first = first.pane_ids().contains(&target);
                let in_second = second.pane_ids().contains(&target);
                if !in_first && !in_second {
                    return false;
                }
                // If this split matches the axis and the target is directly in one branch
                if *direction == axis {
                    if in_first {
                        // Growing the first child means increasing the ratio
                        let new_ratio = (*ratio + delta).clamp(0.1, 0.9);
                        *ratio = new_ratio;
                        return true;
                    } else {
                        // Target is in second child; growing second means decreasing ratio
                        let new_ratio = (*ratio - delta).clamp(0.1, 0.9);
                        *ratio = new_ratio;
                        return true;
                    }
                }
                // Otherwise recurse into the branch containing the target
                if in_first {
                    first.adjust_ratio(target, axis, delta)
                } else {
                    second.adjust_ratio(target, axis, delta)
                }
            }
        }
    }

    /// Swap two leaf pane IDs in the tree.
    pub fn swap_leaves(&mut self, a: PaneId, b: PaneId) -> bool {
        // First replace a with a sentinel, then b with a, then sentinel with b
        let sentinel = u32::MAX;
        if !self.replace_leaf(a, sentinel) {
            return false;
        }
        if !self.replace_leaf(b, a) {
            // Undo
            self.replace_leaf(sentinel, a);
            return false;
        }
        self.replace_leaf(sentinel, b);
        true
    }

    /// Replace a leaf's pane ID with a new ID.
    pub fn replace_leaf(&mut self, old_id: PaneId, new_id: PaneId) -> bool {
        match self {
            LayoutNode::Leaf(id) if *id == old_id => {
                *id = new_id;
                true
            }
            LayoutNode::Leaf(_) => false,
            LayoutNode::Split { first, second, .. } => {
                first.replace_leaf(old_id, new_id) || second.replace_leaf(old_id, new_id)
            }
        }
    }

    /// Find the pane position for a specific pane given bounds.
    pub fn find_position(
        &self,
        target: PaneId,
        col: usize,
        row: usize,
        cols: usize,
        rows: usize,
    ) -> Option<PanePosition> {
        let mut positions = Vec::new();
        self.compute_positions_inner(col, row, cols, rows, &mut positions);
        positions.into_iter().find(|(id, _)| *id == target).map(|(_, pos)| pos)
    }
}

enum RemoveResult {
    NotFound,
    Removed,
    Replaced(LayoutNode),
}

/// The layout engine manages the binary tree of pane arrangements.
#[derive(Debug)]
pub struct LayoutEngine {
    root: Option<LayoutNode>,
}

impl LayoutEngine {
    /// Create an empty layout.
    pub fn new() -> Self {
        Self { root: None }
    }

    /// Add a pane as a leaf. If the layout is empty, this becomes the root.
    /// If non-empty, splits the last pane vertically (fallback behavior).
    pub fn add_pane(&mut self, pane_id: PaneId) {
        match &self.root {
            None => {
                self.root = Some(LayoutNode::Leaf(pane_id));
            }
            Some(_) => {
                // Find last pane and split it vertically
                let ids = self.pane_ids();
                if let Some(&last) = ids.last() {
                    self.split(last, pane_id, SplitDirection::Vertical);
                }
            }
        }
    }

    /// Split a target pane, creating a new split node.
    pub fn split(
        &mut self,
        target_pane_id: PaneId,
        new_pane_id: PaneId,
        direction: SplitDirection,
    ) -> bool {
        if let Some(ref mut root) = self.root {
            root.split(target_pane_id, new_pane_id, direction)
        } else {
            false
        }
    }

    /// Remove a pane from the layout, collapsing the tree.
    pub fn remove_pane(&mut self, pane_id: PaneId) -> bool {
        if let Some(ref mut root) = self.root {
            match root.remove(pane_id) {
                RemoveResult::Removed => {
                    self.root = None;
                    true
                }
                RemoveResult::Replaced(new_root) => {
                    self.root = Some(new_root);
                    true
                }
                RemoveResult::NotFound => {
                    // Check if it was handled internally (nested removal)
                    // The pane might have been removed from a deeper level
                    !self.pane_ids().contains(&pane_id)
                }
            }
        } else {
            false
        }
    }

    /// List all pane IDs in the layout tree.
    pub fn pane_ids(&self) -> Vec<PaneId> {
        match &self.root {
            Some(root) => root.pane_ids(),
            None => Vec::new(),
        }
    }

    /// Number of panes in the layout.
    pub fn count(&self) -> usize {
        match &self.root {
            Some(root) => root.count(),
            None => 0,
        }
    }

    /// Compute the absolute position of every pane given the total tab area.
    pub fn compute_positions(
        &self,
        total_cols: usize,
        total_rows: usize,
    ) -> Vec<(PaneId, PanePosition)> {
        let mut out = Vec::new();
        if let Some(ref root) = self.root {
            root.compute_positions_inner(0, 0, total_cols, total_rows, &mut out);
        }
        out
    }

    /// Adjust the split ratio for the nearest ancestor of `target` on the given axis.
    pub fn adjust_ratio(&mut self, target: PaneId, axis: SplitDirection, delta: f32) -> bool {
        if let Some(ref mut root) = self.root {
            root.adjust_ratio(target, axis, delta)
        } else {
            false
        }
    }

    /// Swap two pane IDs in the layout tree.
    pub fn swap_leaves(&mut self, a: PaneId, b: PaneId) -> bool {
        if let Some(ref mut root) = self.root {
            root.swap_leaves(a, b)
        } else {
            false
        }
    }

    /// Get the root node (for inspection).
    pub fn root(&self) -> Option<&LayoutNode> {
        self.root.as_ref()
    }

    /// Replace the entire layout tree with a new root.
    /// This is used by swap layouts to apply a template.
    pub fn set_root(&mut self, root: LayoutNode) {
        self.root = Some(root);
    }

    /// Apply a layout template to the current set of pane IDs.
    /// The template tree's leaf IDs are replaced with the actual pane IDs
    /// in left-to-right order. If the template has fewer leaves than panes,
    /// extra panes are split off the last leaf. If more leaves, extra leaves
    /// are trimmed.
    pub fn apply_template(&mut self, template: &LayoutNode, pane_ids: &[PaneId]) {
        if pane_ids.is_empty() {
            return;
        }
        let mut new_tree = template.clone();
        let template_ids = new_tree.pane_ids();
        // Replace template leaf IDs with real pane IDs
        for (i, &real_id) in pane_ids.iter().enumerate() {
            if i < template_ids.len() {
                new_tree.replace_leaf(template_ids[i], real_id);
            }
        }
        // If we have more panes than template slots, split extra onto the last leaf
        if pane_ids.len() > template_ids.len() {
            let mut last_id = pane_ids[template_ids.len() - 1];
            for &extra_id in &pane_ids[template_ids.len()..] {
                new_tree.split(last_id, extra_id, SplitDirection::Vertical);
                last_id = extra_id;
            }
        }
        // If template has more slots than panes, prune extra leaves
        if template_ids.len() > pane_ids.len() {
            for &extra_template_id in &template_ids[pane_ids.len()..] {
                new_tree.remove(extra_template_id);
            }
        }
        self.root = Some(new_tree);
    }
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self::new()
    }
}
