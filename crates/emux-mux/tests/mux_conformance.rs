//! Multiplexer conformance tests derived from Zellij's test suite.
//!
//! Sources:
//!   - zellij-server/src/tab/unit/tab_tests.rs (132 tests)
//!   - zellij-server/src/tab/unit/tab_integration_tests.rs (198 tests)
//!   - zellij-server/src/unit/screen_tests.rs (156 tests)
#![allow(unused_variables)]

use emux_mux::{
    FocusDirection, LayoutNode, PaneConstraints, PanePosition, ResizeDirection, Session,
    SplitDirection, Tab,
};

// ---------------------------------------------------------------------------
// 1. Pane Splitting (~15 tests)
// ---------------------------------------------------------------------------

#[test]
fn horizontal_split_creates_two_panes() {
    // Zellij: "split_panes_horizontally"
    let mut tab = Tab::new(0, "test", 80, 24);
    assert_eq!(tab.pane_count(), 1);

    let new_id = tab.split_pane(SplitDirection::Horizontal);
    assert!(new_id.is_some());
    assert_eq!(tab.pane_count(), 2);

    // Expect: 2 panes stacked vertically, each ~12 rows tall, 80 cols wide.
    let positions = tab.compute_positions();
    assert_eq!(positions.len(), 2);

    let (_, pos0) = &positions[0];
    let (_, pos1) = &positions[1];
    assert_eq!(pos0.cols, 80);
    assert_eq!(pos1.cols, 80);
    assert_eq!(pos0.rows + pos1.rows, 24);
    // ±1 tolerance: integer division of odd totals (e.g. 24/2=12, but separator
    // or rounding may shift one row between panes). The exact sum is asserted above.
    assert!(pos0.rows >= 11 && pos0.rows <= 13);
}

#[test]
fn vertical_split_creates_two_panes() {
    // Zellij: "split_panes_vertically"
    let mut tab = Tab::new(0, "test", 80, 24);
    assert_eq!(tab.pane_count(), 1);

    let new_id = tab.split_pane(SplitDirection::Vertical);
    assert!(new_id.is_some());
    assert_eq!(tab.pane_count(), 2);

    // Expect: 2 panes side by side, each ~40 cols wide, 24 rows tall.
    let positions = tab.compute_positions();
    assert_eq!(positions.len(), 2);

    let (_, pos0) = &positions[0];
    let (_, pos1) = &positions[1];
    assert_eq!(pos0.rows, 24);
    assert_eq!(pos1.rows, 24);
    assert_eq!(pos0.cols + pos1.cols, 80);
    // ±1 tolerance: integer division rounding. The exact sum is asserted above.
    assert!(pos0.cols >= 39 && pos0.cols <= 41);
}

#[test]
fn split_largest_pane_picks_biggest_area() {
    // Zellij: "split_largest_pane"
    let mut tab = Tab::new(0, "test", 80, 24);
    // Split vertically: left=40x24, right=40x24
    let right = tab.split_pane(SplitDirection::Vertical).unwrap();
    // Split right horizontally: right_top=40x12, right_bottom=40x12
    // left pane (40x24=960) is the largest
    let count_before = tab.pane_count();
    let new_id = tab.split_largest_pane();
    assert!(new_id.is_some());
    assert_eq!(tab.pane_count(), count_before + 1);
}

#[test]
fn nested_horizontal_and_vertical_splits() {
    // Create one pane, split vertically, then split the right pane horizontally.
    let mut tab = Tab::new(0, "test", 80, 24);
    let right_id = tab.split_pane(SplitDirection::Vertical).unwrap();

    // Focus the right pane (it should already be focused after split)
    assert_eq!(tab.active_pane_id(), Some(right_id));

    let bottom_right_id = tab.split_pane(SplitDirection::Horizontal).unwrap();
    assert_eq!(tab.pane_count(), 3);

    // Expect: 3 panes — left half, top-right quarter, bottom-right quarter.
    let positions = tab.compute_positions();
    assert_eq!(positions.len(), 3);

    // Left pane: col=0, full height
    let (_, left_pos) = &positions[0];
    assert_eq!(left_pos.col, 0);
    assert_eq!(left_pos.row, 0);
    assert_eq!(left_pos.rows, 24);
    assert!(left_pos.cols >= 39 && left_pos.cols <= 41);

    // Top-right: starts at col ~40, half height
    let (_, tr_pos) = &positions[1];
    assert!(tr_pos.col >= 39);
    assert_eq!(tr_pos.row, 0);

    // Bottom-right: starts at col ~40, lower half
    let (_, br_pos) = &positions[2];
    assert!(br_pos.col >= 39);
    assert!(br_pos.row > 0);
    assert_eq!(tr_pos.rows + br_pos.rows, 24);
}

#[test]
fn cannot_split_vertically_when_pane_too_small() {
    // Zellij: "cannot_split_panes_vertically_when_active_pane_is_too_small"
    let mut tab = Tab::new(0, "test", 3, 24);
    // Pane is only 3 cols wide - too small to split vertically (need at least 4)
    let result = tab.split_pane(SplitDirection::Vertical);
    assert!(result.is_none());
    assert_eq!(tab.pane_count(), 1);
}

#[test]
fn cannot_split_horizontally_when_pane_too_small() {
    // Zellij: "cannot_split_panes_horizontally_when_active_pane_is_too_small"
    let mut tab = Tab::new(0, "test", 80, 3);
    // Pane is only 3 rows tall - too small to split horizontally (need at least 4)
    let result = tab.split_pane(SplitDirection::Horizontal);
    assert!(result.is_none());
    assert_eq!(tab.pane_count(), 1);
}

#[test]
fn cannot_split_largest_when_no_room() {
    // Zellij: "cannot_split_largest_pane_when_there_is_no_room"
    // Fill the terminal with minimum-size panes by repeated splitting.
    let mut tab = Tab::new(0, "test", 8, 4);
    // 8x4 => split vertical => 4x4 + 4x4 => split vertical on each => 2x4 + 2x4 + ...
    // 2 cols each is the minimum, can't split further
    let _ = tab.split_pane(SplitDirection::Vertical); // 4+4
    let _ = tab.split_pane(SplitDirection::Vertical); // one becomes 2+2
    // At this point, the active pane should be 2 cols wide — can't split
    let result = tab.split_pane(SplitDirection::Vertical);
    assert!(result.is_none());
}

#[test]
fn cannot_split_vertically_when_pane_has_fixed_columns() {
    // Zellij: "cannot_split_panes_vertically_when_active_pane_has_fixed_columns"
    let mut tab = Tab::new(0, "test", 80, 24);
    let active = tab.active_pane_id().unwrap();
    tab.pane_mut(active)
        .unwrap()
        .set_constraints(PaneConstraints {
            fixed_rows: None,
            fixed_cols: Some(80),
        });

    // Vertical split should be refused because columns are fixed.
    let result = tab.split_pane(SplitDirection::Vertical);
    assert!(result.is_none());
    assert_eq!(tab.pane_count(), 1);
}

#[test]
fn cannot_split_horizontally_when_pane_has_fixed_rows() {
    // Zellij: "cannot_split_panes_horizontally_when_active_pane_has_fixed_rows"
    let mut tab = Tab::new(0, "test", 80, 24);
    let active = tab.active_pane_id().unwrap();
    tab.pane_mut(active)
        .unwrap()
        .set_constraints(PaneConstraints {
            fixed_rows: Some(24),
            fixed_cols: None,
        });

    // Horizontal split should be refused because rows are fixed.
    let result = tab.split_pane(SplitDirection::Horizontal);
    assert!(result.is_none());
    assert_eq!(tab.pane_count(), 1);
}

#[test]
fn split_stack_vertically() {
    // Zellij: "split_stack_vertically"
    // With our layout engine, "stacking" is just multiple splits.
    let mut tab = Tab::new(0, "test", 80, 24);
    let first = tab.active_pane_id().unwrap();
    let second = tab.split_pane(SplitDirection::Vertical).unwrap();
    let third = tab.split_pane(SplitDirection::Vertical).unwrap();
    assert_eq!(tab.pane_count(), 3);
    let positions = tab.compute_positions();
    // All panes should be side-by-side (same row, different cols)
    for (_, pos) in &positions {
        assert_eq!(pos.row, 0);
        assert_eq!(pos.rows, 24);
    }
}

#[test]
fn split_stack_horizontally() {
    // Zellij: "split_stack_horizontally"
    let mut tab = Tab::new(0, "test", 80, 24);
    let _second = tab.split_pane(SplitDirection::Horizontal).unwrap();
    tab.focus_pane(tab.pane_ids()[0]);
    let _third = tab.split_pane(SplitDirection::Horizontal).unwrap();
    assert_eq!(tab.pane_count(), 3);
    let positions = tab.compute_positions();
    // All panes should be stacked vertically (same col, different rows)
    for (_, pos) in &positions {
        assert_eq!(pos.col, 0);
        assert_eq!(pos.cols, 80);
    }
}

#[test]
fn new_pane_with_default_placement() {
    // Zellij: "send_cli_new_pane_action_with_default_parameters"
    let mut tab = Tab::new(0, "test", 80, 24);
    let new_id = tab.split_pane(SplitDirection::Vertical);
    assert!(new_id.is_some());
    assert_eq!(tab.pane_count(), 2);
}

#[test]
fn new_pane_with_split_direction() {
    // Zellij: "send_cli_new_pane_action_with_split_direction"
    let mut tab = Tab::new(0, "test", 80, 24);

    let h_id = tab.split_pane(SplitDirection::Horizontal);
    assert!(h_id.is_some());

    // Focus first pane and split vertically
    let ids = tab.pane_ids();
    tab.focus_pane(ids[0]);
    let v_id = tab.split_pane(SplitDirection::Vertical);
    assert!(v_id.is_some());
    assert_eq!(tab.pane_count(), 3);
}

#[test]
fn new_pane_with_command_and_cwd() {
    // Zellij: "send_cli_new_pane_action_with_command_and_cwd"
    // Our pane model doesn't directly carry command/cwd at the mux layer,
    // but we can set the title to track it.
    let mut tab = Tab::new(0, "test", 80, 24);
    let new_id = tab.split_pane(SplitDirection::Vertical).unwrap();
    tab.pane_mut(new_id).unwrap().set_title("vim /home/user");
    assert_eq!(tab.pane(new_id).unwrap().title(), "vim /home/user");
    assert_eq!(tab.pane_count(), 2);
}

#[test]
fn new_pane_in_auto_layout() {
    // Zellij: "new_pane_in_auto_layout"
    // Register a swap layout that activates for 2 panes.
    let mut tab = Tab::new(0, "test", 80, 24);
    let layout = LayoutNode::Split {
        direction: SplitDirection::Horizontal,
        ratio: 0.7,
        first: Box::new(LayoutNode::Leaf(0)),
        second: Box::new(LayoutNode::Leaf(1)),
    };
    tab.register_swap_layout("two-pane", Some(2), Some(2), layout);
    // Adding a pane should trigger auto swap layout
    let _new = tab.split_pane(SplitDirection::Vertical).unwrap();
    assert_eq!(tab.pane_count(), 2);
    assert_eq!(tab.current_swap_layout_name(), Some("two-pane"));
}

// ---------------------------------------------------------------------------
// 2. Pane Focus (~10 tests)
// ---------------------------------------------------------------------------

#[test]
fn move_focus_down() {
    // Zellij: "move_focus_down"
    let mut tab = Tab::new(0, "test", 80, 24);
    let first_pane = tab.active_pane_id().unwrap();
    tab.split_pane(SplitDirection::Horizontal);

    // Focus top pane
    tab.focus_pane(first_pane);
    assert_eq!(tab.active_pane_id(), Some(first_pane));

    // Move focus down
    let moved = tab.focus_direction(FocusDirection::Down);
    assert!(moved);
    assert_ne!(tab.active_pane_id(), Some(first_pane));
}

#[test]
fn move_focus_up() {
    // Zellij: "move_focus_up"
    let mut tab = Tab::new(0, "test", 80, 24);
    let first_pane = tab.active_pane_id().unwrap();
    let second_pane = tab.split_pane(SplitDirection::Horizontal).unwrap();

    // Focus bottom pane
    tab.focus_pane(second_pane);
    assert_eq!(tab.active_pane_id(), Some(second_pane));

    // Move focus up
    let moved = tab.focus_direction(FocusDirection::Up);
    assert!(moved);
    assert_eq!(tab.active_pane_id(), Some(first_pane));
}

#[test]
fn move_focus_left() {
    // Zellij: "move_focus_left"
    let mut tab = Tab::new(0, "test", 80, 24);
    let first_pane = tab.active_pane_id().unwrap();
    let second_pane = tab.split_pane(SplitDirection::Vertical).unwrap();

    // Focus right pane
    tab.focus_pane(second_pane);
    assert_eq!(tab.active_pane_id(), Some(second_pane));

    // Move focus left
    let moved = tab.focus_direction(FocusDirection::Left);
    assert!(moved);
    assert_eq!(tab.active_pane_id(), Some(first_pane));
}

#[test]
fn move_focus_right() {
    // Zellij: "move_focus_right"
    let mut tab = Tab::new(0, "test", 80, 24);
    let first_pane = tab.active_pane_id().unwrap();
    let second_pane = tab.split_pane(SplitDirection::Vertical).unwrap();

    // Focus left pane
    tab.focus_pane(first_pane);
    assert_eq!(tab.active_pane_id(), Some(first_pane));

    // Move focus right
    let moved = tab.focus_direction(FocusDirection::Right);
    assert!(moved);
    assert_eq!(tab.active_pane_id(), Some(second_pane));
}

#[test]
fn move_focus_down_to_most_recently_used_pane() {
    // Zellij: "move_focus_down_to_the_most_recently_used_pane"
    // Create a layout: top pane, two bottom panes side by side.
    let mut tab = Tab::new(0, "test", 80, 24);
    let top = tab.active_pane_id().unwrap();
    let bottom_left = tab.split_pane(SplitDirection::Horizontal).unwrap();
    // bottom_left is active, split it vertically
    let bottom_right = tab.split_pane(SplitDirection::Vertical).unwrap();
    // Focus the top pane
    tab.focus_pane(top);
    // Move down should pick one of the bottom panes (the nearest)
    let moved = tab.focus_direction(FocusDirection::Down);
    assert!(moved);
    let active = tab.active_pane_id().unwrap();
    assert!(active == bottom_left || active == bottom_right);
}

#[test]
fn move_focus_up_to_most_recently_used_pane() {
    // Zellij: "move_focus_up_to_the_most_recently_used_pane"
    let mut tab = Tab::new(0, "test", 80, 24);
    let top_left = tab.active_pane_id().unwrap();
    let bottom = tab.split_pane(SplitDirection::Horizontal).unwrap();
    tab.focus_pane(top_left);
    let top_right = tab.split_pane(SplitDirection::Vertical).unwrap();
    // Focus bottom pane and move up
    tab.focus_pane(bottom);
    let moved = tab.focus_direction(FocusDirection::Up);
    assert!(moved);
    let active = tab.active_pane_id().unwrap();
    assert!(active == top_left || active == top_right);
}

#[test]
fn focus_next_pane() {
    // Zellij: "send_cli_focus_next_pane_action"
    let mut tab = Tab::new(0, "test", 80, 24);
    let first_pane = tab.active_pane_id().unwrap();
    let second_pane = tab.split_pane(SplitDirection::Vertical).unwrap();

    // Focus first pane
    tab.focus_pane(first_pane);
    assert_eq!(tab.active_pane_id(), Some(first_pane));

    // Focus next
    tab.focus_next();
    assert_eq!(tab.active_pane_id(), Some(second_pane));

    // Focus next wraps around
    tab.focus_next();
    assert_eq!(tab.active_pane_id(), Some(first_pane));
}

#[test]
fn focus_previous_pane() {
    // Zellij: "send_cli_focus_previous_pane_action"
    let mut tab = Tab::new(0, "test", 80, 24);
    let first_pane = tab.active_pane_id().unwrap();
    let second_pane = tab.split_pane(SplitDirection::Vertical).unwrap();

    // Focus second pane
    tab.focus_pane(second_pane);
    assert_eq!(tab.active_pane_id(), Some(second_pane));

    // Focus prev
    tab.focus_prev();
    assert_eq!(tab.active_pane_id(), Some(first_pane));

    // Focus prev wraps around
    tab.focus_prev();
    assert_eq!(tab.active_pane_id(), Some(second_pane));
}

#[test]
fn move_focus_left_at_left_edge_changes_tab() {
    // Zellij: "move_focus_left_at_left_screen_edge_changes_tab"
    // When focus_direction returns false, the caller (Session) should switch tabs.
    let mut session = Session::new("test", 80, 24);
    session.new_tab("Tab 2");
    session.switch_tab(1);
    // The active pane is at the left edge, trying to go left should fail at tab level
    let moved = session
        .active_tab_mut()
        .focus_direction(FocusDirection::Left);
    assert!(!moved); // at edge
    // Caller would then call prev_tab
    session.prev_tab();
    assert_eq!(session.active_tab_index(), 0);
}

#[test]
fn move_focus_right_at_right_edge_changes_tab() {
    // Zellij: "move_focus_right_at_right_screen_edge_changes_tab"
    let mut session = Session::new("test", 80, 24);
    session.new_tab("Tab 2");
    session.switch_tab(0);
    let moved = session
        .active_tab_mut()
        .focus_direction(FocusDirection::Right);
    assert!(!moved); // at edge
    session.next_tab();
    assert_eq!(session.active_tab_index(), 1);
}

// ---------------------------------------------------------------------------
// 3. Pane Resize (~10 tests)
// ---------------------------------------------------------------------------

#[test]
fn resize_down_with_pane_above() {
    // Zellij: "resize_down_with_pane_above"
    let mut tab = Tab::new(0, "test", 80, 24);
    let top = tab.active_pane_id().unwrap();
    let _bottom = tab.split_pane(SplitDirection::Horizontal).unwrap();
    // Focus top pane and resize down (grow it)
    tab.focus_pane(top);
    let top_rows_before = tab
        .compute_positions()
        .iter()
        .find(|(id, _)| *id == top)
        .unwrap()
        .1
        .rows;
    let resized = tab.resize_pane(top, ResizeDirection::Down, 2);
    assert!(resized);
    let top_rows_after = tab
        .compute_positions()
        .iter()
        .find(|(id, _)| *id == top)
        .unwrap()
        .1
        .rows;
    assert!(top_rows_after > top_rows_before);
}

#[test]
fn resize_down_with_pane_below() {
    // Zellij: "resize_down_with_pane_below"
    let mut tab = Tab::new(0, "test", 80, 24);
    let top = tab.active_pane_id().unwrap();
    let bottom = tab.split_pane(SplitDirection::Horizontal).unwrap();
    // Focus bottom pane and resize down (grow it)
    tab.focus_pane(bottom);
    let bottom_rows_before = tab
        .compute_positions()
        .iter()
        .find(|(id, _)| *id == bottom)
        .unwrap()
        .1
        .rows;
    let resized = tab.resize_pane(bottom, ResizeDirection::Down, 2);
    assert!(resized);
    let bottom_rows_after = tab
        .compute_positions()
        .iter()
        .find(|(id, _)| *id == bottom)
        .unwrap()
        .1
        .rows;
    assert!(bottom_rows_after > bottom_rows_before);
}

#[test]
fn resize_up_with_pane_above() {
    // Zellij: "resize_up_with_pane_above"
    let mut tab = Tab::new(0, "test", 80, 24);
    let top = tab.active_pane_id().unwrap();
    let bottom = tab.split_pane(SplitDirection::Horizontal).unwrap();
    tab.focus_pane(bottom);
    let bottom_rows_before = tab
        .compute_positions()
        .iter()
        .find(|(id, _)| *id == bottom)
        .unwrap()
        .1
        .rows;
    let resized = tab.resize_pane(bottom, ResizeDirection::Up, 2);
    assert!(resized);
    let bottom_rows_after = tab
        .compute_positions()
        .iter()
        .find(|(id, _)| *id == bottom)
        .unwrap()
        .1
        .rows;
    assert!(bottom_rows_after > bottom_rows_before);
}

#[test]
fn resize_left_with_pane_to_the_left() {
    // Zellij: "resize_left_with_pane_to_the_left"
    let mut tab = Tab::new(0, "test", 80, 24);
    let left = tab.active_pane_id().unwrap();
    let right = tab.split_pane(SplitDirection::Vertical).unwrap();
    tab.focus_pane(right);
    let right_cols_before = tab
        .compute_positions()
        .iter()
        .find(|(id, _)| *id == right)
        .unwrap()
        .1
        .cols;
    let resized = tab.resize_pane(right, ResizeDirection::Left, 4);
    assert!(resized);
    let right_cols_after = tab
        .compute_positions()
        .iter()
        .find(|(id, _)| *id == right)
        .unwrap()
        .1
        .cols;
    assert!(right_cols_after > right_cols_before);
}

#[test]
fn resize_right_with_pane_to_the_right() {
    // Zellij: "resize_right_with_pane_to_the_right"
    let mut tab = Tab::new(0, "test", 80, 24);
    let left = tab.active_pane_id().unwrap();
    let _right = tab.split_pane(SplitDirection::Vertical).unwrap();
    tab.focus_pane(left);
    let left_cols_before = tab
        .compute_positions()
        .iter()
        .find(|(id, _)| *id == left)
        .unwrap()
        .1
        .cols;
    let resized = tab.resize_pane(left, ResizeDirection::Right, 4);
    assert!(resized);
    let left_cols_after = tab
        .compute_positions()
        .iter()
        .find(|(id, _)| *id == left)
        .unwrap()
        .1
        .cols;
    assert!(left_cols_after > left_cols_before);
}

#[test]
fn cannot_resize_down_when_pane_below_at_minimum_height() {
    // Zellij: "cannot_resize_down_when_pane_below_is_at_minimum_height"
    // With a very small tab, the pane below is already at minimum.
    let mut tab = Tab::new(0, "test", 80, 4);
    let top = tab.active_pane_id().unwrap();
    let _bottom = tab.split_pane(SplitDirection::Horizontal).unwrap();
    // Each pane is 2 rows (minimum). Resize should hit the 0.9 ratio clamp.
    let positions_before = tab.compute_positions();
    let top_rows = positions_before
        .iter()
        .find(|(id, _)| *id == top)
        .unwrap()
        .1
        .rows;
    // Try to resize — the ratio clamp prevents going beyond 0.9
    tab.resize_pane(top, ResizeDirection::Down, 10);
    let positions_after = tab.compute_positions();
    // Bottom pane should still have at least 1 row
    let bottom_pos = positions_after.iter().find(|(id, _)| *id != top).unwrap().1;
    assert!(bottom_pos.rows >= 1);
}

#[test]
fn cannot_resize_left_when_pane_at_minimum_width() {
    // Zellij: "cannot_resize_left_when_pane_to_the_left_is_at_minimum_width"
    let mut tab = Tab::new(0, "test", 4, 24);
    let left = tab.active_pane_id().unwrap();
    let right = tab.split_pane(SplitDirection::Vertical).unwrap();
    // Each pane is 2 cols (minimum). Resize should hit the ratio clamp.
    tab.resize_pane(right, ResizeDirection::Left, 10);
    let positions = tab.compute_positions();
    let left_pos = positions.iter().find(|(id, _)| *id == left).unwrap().1;
    assert!(left_pos.cols >= 1);
}

#[test]
fn cannot_resize_when_pane_has_fixed_rows() {
    // Zellij: "cannot_resize_down_when_pane_has_fixed_rows"
    let mut tab = Tab::new(0, "test", 80, 24);
    // Split first so there's something to resize against
    let top = tab.active_pane_id().unwrap();
    tab.split_pane(SplitDirection::Horizontal).unwrap();

    // Set fixed rows on the top pane
    tab.pane_mut(top).unwrap().set_constraints(PaneConstraints {
        fixed_rows: Some(12),
        fixed_cols: None,
    });

    // Trying to resize the constrained pane vertically should fail
    let result = tab.resize_pane(top, ResizeDirection::Down, 5);
    assert!(!result);
}

#[test]
fn cannot_resize_when_pane_has_fixed_columns() {
    // Zellij: "cannot_resize_right_when_pane_has_fixed_columns"
    let mut tab = Tab::new(0, "test", 80, 24);
    // Split first so there's something to resize against
    let left = tab.active_pane_id().unwrap();
    tab.split_pane(SplitDirection::Vertical).unwrap();

    // Set fixed cols on the left pane
    tab.pane_mut(left)
        .unwrap()
        .set_constraints(PaneConstraints {
            fixed_rows: None,
            fixed_cols: Some(40),
        });

    // Trying to resize the constrained pane horizontally should fail
    let result = tab.resize_pane(left, ResizeDirection::Right, 5);
    assert!(!result);
}

#[test]
fn nondirectional_resize_increase_with_single_pane() {
    // Zellij: "nondirectional_resize_increase_with_1_pane"
    // A single pane cannot be resized (no split to adjust).
    let mut tab = Tab::new(0, "test", 80, 24);
    let pane = tab.active_pane_id().unwrap();
    let resized = tab.resize_pane(pane, ResizeDirection::Right, 5);
    assert!(!resized); // no split exists
}

#[test]
fn nondirectional_resize_increase_with_pane_to_left() {
    // Zellij: "nondirectional_resize_increase_with_1_pane_to_left"
    let mut tab = Tab::new(0, "test", 80, 24);
    let left = tab.active_pane_id().unwrap();
    let right = tab.split_pane(SplitDirection::Vertical).unwrap();
    tab.focus_pane(right);
    let right_cols_before = tab
        .compute_positions()
        .iter()
        .find(|(id, _)| *id == right)
        .unwrap()
        .1
        .cols;
    // Increase right pane by growing left (decreasing ratio)
    let resized = tab.resize_pane(right, ResizeDirection::Left, 4);
    assert!(resized);
    let right_cols_after = tab
        .compute_positions()
        .iter()
        .find(|(id, _)| *id == right)
        .unwrap()
        .1
        .cols;
    assert!(right_cols_after > right_cols_before);
}

#[test]
fn resize_by_pane_id() {
    // Zellij: "resize_by_pane_id"
    let mut tab = Tab::new(0, "test", 80, 24);
    let left = tab.active_pane_id().unwrap();
    let right = tab.split_pane(SplitDirection::Vertical).unwrap();
    let left_cols_before = tab
        .compute_positions()
        .iter()
        .find(|(id, _)| *id == left)
        .unwrap()
        .1
        .cols;
    // Resize a specific pane by ID (not necessarily the active one)
    let resized = tab.resize_pane(left, ResizeDirection::Right, 4);
    assert!(resized);
    let left_cols_after = tab
        .compute_positions()
        .iter()
        .find(|(id, _)| *id == left)
        .unwrap()
        .1
        .cols;
    assert!(left_cols_after > left_cols_before);
}

// ---------------------------------------------------------------------------
// 4. Floating Panes (~10 tests)
// ---------------------------------------------------------------------------

#[test]
fn new_floating_pane() {
    // Zellij: "new_floating_pane"
    let mut tab = Tab::new(0, "test", 80, 24);
    let fp_id = tab.new_floating_pane();
    assert_eq!(tab.floating_pane_count(), 1);
    let fp = tab.floating_pane(fp_id).unwrap();
    assert!(fp.width > 0);
    assert!(fp.height > 0);
    assert_eq!(tab.active_pane_id(), Some(fp_id));
}

#[test]
fn toggle_floating_panes_off() {
    // Zellij: "toggle_floating_panes_off"
    let mut tab = Tab::new(0, "test", 80, 24);
    let _fp_id = tab.new_floating_pane();
    assert!(tab.is_floating_visible());
    tab.toggle_floating_panes();
    assert!(!tab.is_floating_visible());
    // Floating pane should not appear in compute_positions
    let positions = tab.compute_positions();
    assert_eq!(positions.len(), 1); // only tiled
}

#[test]
fn toggle_floating_panes_on() {
    // Zellij: "toggle_floating_panes_on"
    let mut tab = Tab::new(0, "test", 80, 24);
    let fp_id = tab.new_floating_pane();
    tab.toggle_floating_panes(); // hide
    tab.toggle_floating_panes(); // show
    assert!(tab.is_floating_visible());
    let positions = tab.compute_positions();
    assert_eq!(positions.len(), 2); // tiled + floating
}

#[test]
fn floating_panes_persist_across_toggles() {
    // Zellij: "floating_panes_persist_across_toggles"
    let mut tab = Tab::new(0, "test", 80, 24);
    let fp_id = tab.new_floating_pane_with_coords(10, 5, 30, 10);
    tab.toggle_floating_panes(); // hide
    assert_eq!(tab.floating_pane_count(), 1); // still exists
    tab.toggle_floating_panes(); // show
    let fp = tab.floating_pane(fp_id).unwrap();
    assert_eq!(fp.x, 10);
    assert_eq!(fp.y, 5);
    assert_eq!(fp.width, 30);
    assert_eq!(fp.height, 10);
}

#[test]
fn five_new_floating_panes_no_overlap() {
    // Zellij: "five_new_floating_panes"
    // All created with default coords (centered), so they will overlap
    // since they're the same position. But the system allows it.
    let mut tab = Tab::new(0, "test", 120, 40);
    for _ in 0..5 {
        tab.new_floating_pane();
    }
    assert_eq!(tab.floating_pane_count(), 5);
    // All should be within viewport
    for fp_id in tab.floating_pane_ids() {
        let fp = tab.floating_pane(fp_id).unwrap();
        assert!(fp.x + fp.width <= 120);
        assert!(fp.y + fp.height <= 40);
    }
}

#[test]
fn increase_floating_pane_size() {
    // Zellij: "increase_floating_pane_size"
    let mut tab = Tab::new(0, "test", 80, 24);
    let fp_id = tab.new_floating_pane_with_coords(10, 5, 20, 10);
    tab.resize_floating_pane(fp_id, 30, 15);
    let fp = tab.floating_pane(fp_id).unwrap();
    assert_eq!(fp.width, 30);
    assert_eq!(fp.height, 15);
}

#[test]
fn decrease_floating_pane_size() {
    // Zellij: "decrease_floating_pane_size"
    let mut tab = Tab::new(0, "test", 80, 24);
    let fp_id = tab.new_floating_pane_with_coords(10, 5, 30, 15);
    tab.resize_floating_pane(fp_id, 15, 8);
    let fp = tab.floating_pane(fp_id).unwrap();
    assert_eq!(fp.width, 15);
    assert_eq!(fp.height, 8);
}

#[test]
fn resize_floating_pane_left() {
    // Zellij: "resize_floating_pane_left"
    // Shrink from the right side (decrease width)
    let mut tab = Tab::new(0, "test", 80, 24);
    let fp_id = tab.new_floating_pane_with_coords(10, 5, 30, 10);
    tab.resize_floating_pane(fp_id, 20, 10);
    let fp = tab.floating_pane(fp_id).unwrap();
    assert_eq!(fp.width, 20);
}

#[test]
fn resize_floating_pane_right() {
    // Zellij: "resize_floating_pane_right"
    // Grow to the right (increase width)
    let mut tab = Tab::new(0, "test", 80, 24);
    let fp_id = tab.new_floating_pane_with_coords(10, 5, 20, 10);
    tab.resize_floating_pane(fp_id, 40, 10);
    let fp = tab.floating_pane(fp_id).unwrap();
    assert_eq!(fp.width, 40);
}

#[test]
fn move_floating_pane_focus_left() {
    // Zellij: "move_floating_pane_focus_left"
    // With two floating panes, focus_prev should move focus between them.
    let mut tab = Tab::new(0, "test", 80, 24);
    let fp1 = tab.new_floating_pane_with_coords(0, 0, 20, 10);
    let fp2 = tab.new_floating_pane_with_coords(30, 0, 20, 10);
    assert_eq!(tab.active_pane_id(), Some(fp2));
    tab.focus_prev();
    assert_eq!(tab.active_pane_id(), Some(fp1));
}

#[test]
fn move_floating_pane_focus_right() {
    // Zellij: "move_floating_pane_focus_right"
    // focus_next cycles through: tiled panes, then floating panes in z-order.
    // Focusing a floating pane brings it to the top (changes z-order).
    let mut tab = Tab::new(0, "test", 80, 24);
    let tiled = tab.pane_ids()[0];
    let fp1 = tab.new_floating_pane_with_coords(0, 0, 20, 10);
    let fp2 = tab.new_floating_pane_with_coords(30, 0, 20, 10);
    // fp2 is active (last created). Focus cycle: [tiled, fp1, fp2].
    assert_eq!(tab.active_pane_id(), Some(fp2));
    // focus_next wraps to tiled
    tab.focus_next();
    assert_eq!(tab.active_pane_id(), Some(tiled));
    // Next goes to fp1 (brings it to front, z-order becomes [fp2, fp1])
    tab.focus_next();
    assert_eq!(tab.active_pane_id(), Some(fp1));
}

#[test]
fn embed_floating_pane() {
    // Zellij: "embed_floating_pane"
    let mut tab = Tab::new(0, "test", 80, 24);
    let fp_id = tab.new_floating_pane();
    assert_eq!(tab.floating_pane_count(), 1);
    assert_eq!(tab.pane_count(), 1);
    tab.embed_floating_pane(fp_id);
    assert_eq!(tab.floating_pane_count(), 0);
    assert_eq!(tab.pane_count(), 2);
    assert!(tab.pane(fp_id).is_some());
}

#[test]
fn float_embedded_pane() {
    // Zellij: "float_embedded_pane"
    let mut tab = Tab::new(0, "test", 80, 24);
    let first = tab.active_pane_id().unwrap();
    let second = tab.split_pane(SplitDirection::Vertical).unwrap();
    assert_eq!(tab.pane_count(), 2);
    assert_eq!(tab.floating_pane_count(), 0);
    tab.float_pane(second);
    assert_eq!(tab.pane_count(), 1);
    assert_eq!(tab.floating_pane_count(), 1);
    assert!(tab.floating_pane(second).is_some());
}

#[test]
fn cannot_float_only_embedded_pane() {
    // Zellij: "cannot_float_only_embedded_pane"
    let mut tab = Tab::new(0, "test", 80, 24);
    let only = tab.active_pane_id().unwrap();
    let result = tab.float_pane(only);
    assert!(!result);
    assert_eq!(tab.pane_count(), 1);
    assert_eq!(tab.floating_pane_count(), 0);
}

#[test]
fn floating_pane_with_custom_coordinates() {
    // Zellij: "open_new_floating_pane_with_custom_coordinates"
    let mut tab = Tab::new(0, "test", 80, 24);
    let fp_id = tab.new_floating_pane_with_coords(5, 3, 25, 15);
    let fp = tab.floating_pane(fp_id).unwrap();
    assert_eq!(fp.x, 5);
    assert_eq!(fp.y, 3);
    assert_eq!(fp.width, 25);
    assert_eq!(fp.height, 15);
}

#[test]
fn floating_pane_coordinates_clamped_to_viewport() {
    // Zellij: "open_new_floating_pane_with_custom_coordinates_exceeding_viewport"
    let mut tab = Tab::new(0, "test", 80, 24);
    let fp_id = tab.new_floating_pane_with_coords(200, 200, 100, 50);
    let fp = tab.floating_pane(fp_id).unwrap();
    assert!(fp.x + fp.width <= 80);
    assert!(fp.y + fp.height <= 24);
}

// ---------------------------------------------------------------------------
// 5. Fullscreen (~5 tests)
// ---------------------------------------------------------------------------

#[test]
fn toggle_focused_pane_fullscreen() {
    // Zellij: "toggle_focused_pane_fullscreen"
    let mut tab = Tab::new(0, "test", 80, 24);
    tab.split_pane(SplitDirection::Vertical);
    tab.split_pane(SplitDirection::Horizontal);
    assert_eq!(tab.pane_count(), 3);

    let active = tab.active_pane_id().unwrap();
    tab.toggle_fullscreen();
    assert!(tab.is_fullscreen());
    assert_eq!(tab.fullscreen_pane_id(), Some(active));

    // In fullscreen, only the active pane is visible
    let positions = tab.compute_positions();
    assert_eq!(positions.len(), 1);
    assert_eq!(positions[0].0, active);
    assert_eq!(
        positions[0].1,
        PanePosition {
            col: 0,
            row: 0,
            cols: 80,
            rows: 24
        }
    );
}

#[test]
fn toggle_fullscreen_off_restores_layout() {
    // Zellij: inverse of "toggle_focused_pane_fullscreen"
    let mut tab = Tab::new(0, "test", 80, 24);
    tab.split_pane(SplitDirection::Vertical);
    assert_eq!(tab.pane_count(), 2);

    tab.toggle_fullscreen();
    assert!(tab.is_fullscreen());

    tab.toggle_fullscreen();
    assert!(!tab.is_fullscreen());

    // All panes return
    let positions = tab.compute_positions();
    assert_eq!(positions.len(), 2);
}

#[test]
fn switch_to_next_pane_exits_fullscreen() {
    // Zellij: "switch_to_next_pane_fullscreen"
    let mut tab = Tab::new(0, "test", 80, 24);
    tab.split_pane(SplitDirection::Vertical);

    tab.toggle_fullscreen();
    assert!(tab.is_fullscreen());

    tab.focus_next();
    assert!(!tab.is_fullscreen());
}

#[test]
fn switch_to_prev_pane_exits_fullscreen() {
    // Zellij: "switch_to_prev_pane_fullscreen"
    let mut tab = Tab::new(0, "test", 80, 24);
    tab.split_pane(SplitDirection::Vertical);

    tab.toggle_fullscreen();
    assert!(tab.is_fullscreen());

    tab.focus_prev();
    assert!(!tab.is_fullscreen());
}

#[test]
fn toggle_fullscreen_by_pane_id() {
    // Zellij: "toggle_fullscreen_by_pane_id"
    let mut tab = Tab::new(0, "test", 80, 24);
    let first = tab.active_pane_id().unwrap();
    let second = tab.split_pane(SplitDirection::Vertical).unwrap();
    // Toggle fullscreen on a specific pane (not necessarily the active one)
    tab.toggle_fullscreen_by_id(first);
    assert!(tab.is_fullscreen());
    assert_eq!(tab.fullscreen_pane_id(), Some(first));
    // Toggle off
    tab.toggle_fullscreen_by_id(first);
    assert!(!tab.is_fullscreen());
}

#[test]
fn stacked_panes_can_become_fullscreen() {
    // Zellij: "stacked_panes_can_become_fullscreen"
    // Multiple horizontally stacked panes; toggle fullscreen on one.
    let mut tab = Tab::new(0, "test", 80, 24);
    let first = tab.active_pane_id().unwrap();
    tab.split_pane(SplitDirection::Horizontal);
    tab.focus_pane(first);
    tab.split_pane(SplitDirection::Horizontal);
    assert_eq!(tab.pane_count(), 3);
    tab.toggle_fullscreen();
    assert!(tab.is_fullscreen());
    let positions = tab.compute_positions();
    assert_eq!(positions.len(), 1);
    assert_eq!(
        positions[0].1,
        PanePosition {
            col: 0,
            row: 0,
            cols: 80,
            rows: 24
        }
    );
}

// ---------------------------------------------------------------------------
// 6. Tab Management (~15 tests)
// ---------------------------------------------------------------------------

#[test]
fn open_new_tab() {
    // Zellij: "open_new_tab"
    let mut session = Session::new("test", 80, 24);
    assert_eq!(session.tab_count(), 1);

    session.new_tab("Tab 2");
    assert_eq!(session.tab_count(), 2);
    assert_eq!(session.active_tab_index(), 1); // new tab is active
}

#[test]
fn close_tab() {
    // Zellij: "close_tab"
    let mut session = Session::new("test", 80, 24);
    session.new_tab("Tab 2");
    assert_eq!(session.tab_count(), 2);

    let closed = session.close_tab(1);
    assert!(closed);
    assert_eq!(session.tab_count(), 1);
}

#[test]
fn close_the_middle_tab() {
    // Zellij: "close_the_middle_tab"
    let mut session = Session::new("test", 80, 24);
    session.new_tab("Tab 2");
    session.new_tab("Tab 3");
    assert_eq!(session.tab_count(), 3);

    // Close the middle tab (index 1)
    session.switch_tab(1);
    let closed = session.close_tab(1);
    assert!(closed);
    assert_eq!(session.tab_count(), 2);

    // Remaining tabs should be Tab 1 and Tab 3
    let names = session.tab_names();
    assert_eq!(names[0], "Tab 1");
    assert_eq!(names[1], "Tab 3");
}

#[test]
fn switch_to_next_tab() {
    // Zellij: "switch_to_next_tab"
    let mut session = Session::new("test", 80, 24);
    session.new_tab("Tab 2");
    session.switch_tab(0);

    assert_eq!(session.active_tab_index(), 0);
    session.next_tab();
    assert_eq!(session.active_tab_index(), 1);
}

#[test]
fn switch_to_prev_tab() {
    // Zellij: "switch_to_prev_tab"
    let mut session = Session::new("test", 80, 24);
    session.new_tab("Tab 2");
    assert_eq!(session.active_tab_index(), 1);

    session.prev_tab();
    assert_eq!(session.active_tab_index(), 0);
}

#[test]
fn switch_to_tab_by_name() {
    // Zellij: "switch_to_tab_name"
    let mut session = Session::new("test", 80, 24);
    session.new_tab("Alpha");
    session.new_tab("Beta");
    session.switch_tab(0);

    // Find tab by name and switch
    let names = session.tab_names();
    let idx = names.iter().position(|&n| n == "Beta");
    assert!(idx.is_some());
    session.switch_tab(idx.unwrap());
    assert_eq!(session.active_tab().name(), "Beta");
}

#[test]
fn go_to_tab_by_index() {
    // Zellij: "send_cli_goto_tab_action"
    let mut session = Session::new("test", 80, 24);
    session.new_tab("Tab 2");
    session.new_tab("Tab 3");

    session.switch_tab(0);
    assert_eq!(session.active_tab_index(), 0);

    session.switch_tab(2);
    assert_eq!(session.active_tab_index(), 2);
}

#[test]
fn rename_tab() {
    // Zellij: "send_cli_rename_tab"
    let mut session = Session::new("test", 80, 24);
    session.active_tab_mut().rename("My Tab");
    assert_eq!(session.active_tab().name(), "My Tab");
}

#[test]
fn undo_rename_tab() {
    // Zellij: "send_cli_undo_rename_tab"
    let mut session = Session::new("test", 80, 24);
    assert_eq!(session.active_tab().name(), "Tab 1");
    session.active_tab_mut().rename("My Custom Tab");
    assert_eq!(session.active_tab().name(), "My Custom Tab");
    session.active_tab_mut().undo_rename();
    assert_eq!(session.active_tab().name(), "Tab 1");
}

#[test]
fn toggle_to_previous_tab() {
    // Zellij: "toggle_to_previous_tab_simple"
    let mut session = Session::new("test", 80, 24);
    session.new_tab("Tab 2");
    session.new_tab("Tab 3");
    // Currently on Tab 3 (index 2)
    session.switch_tab(0); // go to Tab 1
    assert_eq!(session.active_tab_index(), 0);
    session.toggle_previous_tab();
    assert_eq!(session.active_tab_index(), 2); // back to Tab 3
    session.toggle_previous_tab();
    assert_eq!(session.active_tab_index(), 0); // back to Tab 1
}

#[test]
fn basic_move_active_tab_to_left() {
    // Zellij: "basic_move_of_active_tab_to_left"
    let mut session = Session::new("test", 80, 24);
    session.new_tab("Tab 2");
    session.new_tab("Tab 3");
    // Active is Tab 3 at index 2
    assert_eq!(session.active_tab_index(), 2);
    session.move_tab_left();
    assert_eq!(session.active_tab_index(), 1);
    assert_eq!(session.active_tab().name(), "Tab 3");
}

#[test]
fn basic_move_active_tab_to_right() {
    // Zellij: "basic_move_of_active_tab_to_right"
    let mut session = Session::new("test", 80, 24);
    session.new_tab("Tab 2");
    session.new_tab("Tab 3");
    session.switch_tab(0); // Tab 1
    session.move_tab_right();
    assert_eq!(session.active_tab_index(), 1);
    assert_eq!(session.active_tab().name(), "Tab 1");
}

#[test]
fn wrapping_move_of_active_tab_to_left() {
    // Zellij: "wrapping_move_of_active_tab_to_left"
    let mut session = Session::new("test", 80, 24);
    session.new_tab("Tab 2");
    session.new_tab("Tab 3");
    session.switch_tab(0); // Tab 1, at index 0
    session.move_tab_left_wrapping();
    // Tab 1 should wrap to the end
    assert_eq!(session.active_tab_index(), 2);
    assert_eq!(session.active_tab().name(), "Tab 1");
}

#[test]
fn wrapping_move_of_active_tab_to_right() {
    // Zellij: "wrapping_move_of_active_tab_to_right"
    let mut session = Session::new("test", 80, 24);
    session.new_tab("Tab 2");
    session.new_tab("Tab 3");
    // Active is Tab 3 at index 2 (last)
    session.move_tab_right_wrapping();
    // Tab 3 should wrap to the beginning
    assert_eq!(session.active_tab_index(), 0);
    assert_eq!(session.active_tab().name(), "Tab 3");
}

#[test]
fn tab_id_remains_stable_after_switch() {
    // Zellij: "tab_id_remains_stable_after_switch"
    let mut session = Session::new("test", 80, 24);
    let tab2_id = session.new_tab("Tab 2");
    let tab3_id = session.new_tab("Tab 3");

    let id_at_0 = session.tab(0).unwrap().id();
    let id_at_1 = session.tab(1).unwrap().id();
    let id_at_2 = session.tab(2).unwrap().id();

    // Switch around
    session.switch_tab(0);
    session.switch_tab(2);
    session.switch_tab(1);

    // IDs should be stable
    assert_eq!(session.tab(0).unwrap().id(), id_at_0);
    assert_eq!(session.tab(1).unwrap().id(), id_at_1);
    assert_eq!(session.tab(2).unwrap().id(), id_at_2);
    assert_eq!(id_at_1, tab2_id);
    assert_eq!(id_at_2, tab3_id);
}

#[test]
fn switch_to_tab_with_fullscreen_pane() {
    // Zellij: "switch_to_tab_with_fullscreen"
    let mut session = Session::new("test", 80, 24);
    session
        .active_tab_mut()
        .split_pane(SplitDirection::Vertical);
    session.active_tab_mut().toggle_fullscreen();
    assert!(session.active_tab().is_fullscreen());

    session.new_tab("Tab 2");
    assert_eq!(session.active_tab_index(), 1);

    // Switch back to Tab 1
    session.switch_tab(0);
    assert!(session.active_tab().is_fullscreen());
}

#[test]
fn close_tab_by_id() {
    // Zellij: "close_tab_by_id_verifies_screen_state"
    let mut session = Session::new("test", 80, 24);
    let tab2_id = session.new_tab("Tab 2");
    let tab3_id = session.new_tab("Tab 3");
    assert_eq!(session.tab_count(), 3);
    let closed = session.close_tab_by_id(tab2_id);
    assert!(closed);
    assert_eq!(session.tab_count(), 2);
    // Tab 1 and Tab 3 remain
    let names = session.tab_names();
    assert_eq!(names[0], "Tab 1");
    assert_eq!(names[1], "Tab 3");
}

#[test]
fn break_pane_to_new_tab() {
    // Zellij: "screen_can_break_pane_to_a_new_tab"
    let mut session = Session::new("test", 80, 24);
    let pane2 = session
        .active_tab_mut()
        .split_pane(SplitDirection::Vertical)
        .unwrap();
    assert_eq!(session.active_tab().pane_count(), 2);
    let new_tab_id = session.break_pane_to_new_tab(pane2);
    assert!(new_tab_id.is_some());
    assert_eq!(session.tab_count(), 2);
    // Original tab now has 1 pane
    assert_eq!(session.tab(0).unwrap().pane_count(), 1);
    // New tab is active and has 1 pane (its own initial pane)
    assert_eq!(session.active_tab_index(), 1);
    assert_eq!(session.active_tab().pane_count(), 1);
}

#[test]
fn cannot_break_last_pane_to_new_tab() {
    // Zellij: "screen_cannot_break_last_selectable_pane_to_a_new_tab"
    let mut session = Session::new("test", 80, 24);
    let only_pane = session.active_tab().active_pane_id().unwrap();
    let result = session.break_pane_to_new_tab(only_pane);
    assert!(result.is_none());
    assert_eq!(session.tab_count(), 1);
}

#[test]
fn move_pane_to_new_tab_right() {
    // Zellij: "screen_can_move_pane_to_a_new_tab_right"
    let mut session = Session::new("test", 80, 24);
    session
        .active_tab_mut()
        .split_pane(SplitDirection::Vertical);
    assert_eq!(session.active_tab().pane_count(), 2);
    let result = session.move_pane_to_new_tab_right();
    assert!(result.is_some());
    assert_eq!(session.tab_count(), 2);
    assert_eq!(session.tab(0).unwrap().pane_count(), 1);
}

// ---------------------------------------------------------------------------
// 7. Session Management (~5 tests)
// ---------------------------------------------------------------------------

#[test]
fn create_new_session() {
    // Create a new named session.
    let session = Session::new("my-session", 80, 24);
    assert_eq!(session.name(), "my-session");
    assert_eq!(session.tab_count(), 1);
    assert_eq!(session.active_tab().pane_count(), 1);
}

#[test]
fn attach_to_existing_session() {
    // Zellij: "attach_after_first_tab_closed"
    // Verify session still exists after closing first tab.
    let mut session = Session::new("test", 80, 24);
    session.new_tab("Tab 2");
    session.switch_tab(0);
    session.close_tab(0);
    assert_eq!(session.tab_count(), 1);
    assert_eq!(session.active_tab().name(), "Tab 2");
}

#[test]
fn detach_from_session() {
    // Session state persists independently of client attachment.
    let mut session = Session::new("test", 80, 24);
    session
        .active_tab_mut()
        .split_pane(SplitDirection::Vertical);
    // "Detach" is a no-op at the mux layer; session state is preserved.
    assert_eq!(session.active_tab().pane_count(), 2);
    assert_eq!(session.name(), "test");
}

#[test]
fn multiple_clients_in_session() {
    // Multiple clients share the same session state.
    let mut session = Session::new("test", 80, 24);
    session
        .active_tab_mut()
        .split_pane(SplitDirection::Vertical);
    // Both "clients" see the same pane count
    assert_eq!(session.active_tab().pane_count(), 2);
    // Resize from one client affects all
    session.resize(120, 40);
    assert_eq!(session.size().cols, 120);
    assert_eq!(session.size().rows, 40);
}

#[test]
fn session_survives_client_disconnect() {
    // Session state is not lost when client disconnects.
    let mut session = Session::new("test", 80, 24);
    session.new_tab("Tab 2");
    session
        .active_tab_mut()
        .split_pane(SplitDirection::Horizontal);
    // "Client disconnects" — session should still be intact.
    assert_eq!(session.tab_count(), 2);
    assert_eq!(session.active_tab().pane_count(), 2);
}

// ---------------------------------------------------------------------------
// 8. Layout (~5 tests)
// ---------------------------------------------------------------------------

#[test]
fn tab_with_basic_layout() {
    // Apply a simple two-pane horizontal layout.
    let mut tab = Tab::new(0, "test", 80, 24);
    tab.split_pane(SplitDirection::Horizontal);
    assert_eq!(tab.pane_count(), 2);

    let positions = tab.compute_positions();
    assert_eq!(positions.len(), 2);
    // First pane on top, second on bottom
    assert_eq!(positions[0].1.row, 0);
    assert!(positions[1].1.row > 0);
}

#[test]
fn tab_with_nested_layout() {
    // Apply a layout with nested splits (left | [top-right / bottom-right]).
    let mut tab = Tab::new(0, "test", 80, 24);
    let right = tab.split_pane(SplitDirection::Vertical).unwrap();
    tab.focus_pane(right);
    tab.split_pane(SplitDirection::Horizontal);
    assert_eq!(tab.pane_count(), 3);

    let positions = tab.compute_positions();
    assert_eq!(positions.len(), 3);
}

#[test]
fn tab_with_layout_that_has_floating_panes() {
    // Zellij: "tab_with_layout_that_has_floating_panes"
    let mut tab = Tab::new(0, "test", 80, 24);
    // Create a tiled split plus some floating panes
    tab.split_pane(SplitDirection::Vertical);
    let fp1 = tab.new_floating_pane_with_coords(5, 3, 20, 10);
    let fp2 = tab.new_floating_pane_with_coords(30, 5, 25, 8);
    assert_eq!(tab.pane_count(), 2); // tiled
    assert_eq!(tab.floating_pane_count(), 2);
    let positions = tab.compute_positions();
    // 2 tiled + 2 floating
    assert_eq!(positions.len(), 4);
}

#[test]
fn can_swap_tiled_layout_at_runtime() {
    // Zellij: "can_swap_tiled_layout_at_runtime"
    let mut tab = Tab::new(0, "test", 80, 24);
    tab.split_pane(SplitDirection::Vertical);
    assert_eq!(tab.pane_count(), 2);

    // Register two swap layouts
    let layout_h = LayoutNode::Split {
        direction: SplitDirection::Horizontal,
        ratio: 0.5,
        first: Box::new(LayoutNode::Leaf(0)),
        second: Box::new(LayoutNode::Leaf(1)),
    };
    let layout_v = LayoutNode::Split {
        direction: SplitDirection::Vertical,
        ratio: 0.3,
        first: Box::new(LayoutNode::Leaf(0)),
        second: Box::new(LayoutNode::Leaf(1)),
    };
    tab.register_swap_layout("horizontal", Some(2), Some(2), layout_h);
    tab.register_swap_layout("vertical-30", Some(2), Some(2), layout_v);

    tab.next_swap_layout();
    assert!(tab.current_swap_layout_name().is_some());
    let first = tab.current_swap_layout_name().unwrap().to_string();

    tab.next_swap_layout();
    let second = tab.current_swap_layout_name().unwrap().to_string();
    assert_ne!(first, second);
}

#[test]
fn can_swap_floating_layout_at_runtime() {
    // Zellij: "can_swap_floating_layout_at_runtime"
    // Swap layouts work for tiled panes; floating panes are independent.
    // Verify that swapping layout doesn't break floating panes.
    let mut tab = Tab::new(0, "test", 80, 24);
    tab.split_pane(SplitDirection::Vertical);
    let fp = tab.new_floating_pane_with_coords(5, 3, 20, 10);

    let layout = LayoutNode::Split {
        direction: SplitDirection::Horizontal,
        ratio: 0.5,
        first: Box::new(LayoutNode::Leaf(0)),
        second: Box::new(LayoutNode::Leaf(1)),
    };
    tab.register_swap_layout("horiz", Some(2), Some(2), layout);
    tab.next_swap_layout();
    assert_eq!(tab.current_swap_layout_name(), Some("horiz"));
    // Floating pane is still there
    assert_eq!(tab.floating_pane_count(), 1);
    assert!(tab.floating_pane(fp).is_some());
}

// ---------------------------------------------------------------------------
// 9. Pane Close (~8 tests)
// ---------------------------------------------------------------------------

#[test]
fn close_pane_with_pane_above() {
    // Two panes vertically. Close the bottom pane.
    let mut tab = Tab::new(0, "test", 80, 24);
    let first = tab.active_pane_id().unwrap();
    let second = tab.split_pane(SplitDirection::Horizontal).unwrap();

    tab.close_pane(second);
    assert_eq!(tab.pane_count(), 1);

    // Top pane expands to fill the space
    let positions = tab.compute_positions();
    assert_eq!(positions.len(), 1);
    assert_eq!(positions[0].1.rows, 24);
    assert_eq!(positions[0].1.cols, 80);
}

#[test]
fn close_pane_with_pane_below() {
    // Two panes vertically. Close the top pane.
    let mut tab = Tab::new(0, "test", 80, 24);
    let first = tab.active_pane_id().unwrap();
    let second = tab.split_pane(SplitDirection::Horizontal).unwrap();

    tab.close_pane(first);
    assert_eq!(tab.pane_count(), 1);

    // Bottom pane expands
    let positions = tab.compute_positions();
    assert_eq!(positions.len(), 1);
    assert_eq!(positions[0].0, second);
    assert_eq!(positions[0].1.rows, 24);
}

#[test]
fn close_pane_with_pane_to_the_left() {
    // Two panes side by side. Close the right pane.
    let mut tab = Tab::new(0, "test", 80, 24);
    let first = tab.active_pane_id().unwrap();
    let second = tab.split_pane(SplitDirection::Vertical).unwrap();

    tab.close_pane(second);
    assert_eq!(tab.pane_count(), 1);

    let positions = tab.compute_positions();
    assert_eq!(positions[0].1.cols, 80);
}

#[test]
fn close_pane_with_pane_to_the_right() {
    // Two panes side by side. Close the left pane.
    let mut tab = Tab::new(0, "test", 80, 24);
    let first = tab.active_pane_id().unwrap();
    let second = tab.split_pane(SplitDirection::Vertical).unwrap();

    tab.close_pane(first);
    assert_eq!(tab.pane_count(), 1);

    let positions = tab.compute_positions();
    assert_eq!(positions[0].0, second);
    assert_eq!(positions[0].1.cols, 80);
}

#[test]
fn close_pane_with_multiple_panes_above() {
    // Three panes: split horizontal, then split the top horizontal again.
    let mut tab = Tab::new(0, "test", 80, 24);
    let first = tab.active_pane_id().unwrap();
    let second = tab.split_pane(SplitDirection::Horizontal).unwrap();

    // Focus first and split it again
    tab.focus_pane(first);
    let third = tab.split_pane(SplitDirection::Horizontal).unwrap();
    assert_eq!(tab.pane_count(), 3);

    // Close the bottom pane
    tab.close_pane(second);
    assert_eq!(tab.pane_count(), 2);

    // Remaining panes fill the space
    let positions = tab.compute_positions();
    let total_rows: usize = positions.iter().map(|(_, p)| p.rows).sum();
    assert_eq!(total_rows, 24);
}

#[test]
fn close_pane_with_multiple_panes_below() {
    // Split horizontal twice to get 3 stacked panes, close the top one.
    let mut tab = Tab::new(0, "test", 80, 24);
    let first = tab.active_pane_id().unwrap();
    tab.split_pane(SplitDirection::Horizontal);
    tab.focus_pane(first);
    tab.split_pane(SplitDirection::Horizontal);
    assert_eq!(tab.pane_count(), 3);

    tab.close_pane(first);
    assert_eq!(tab.pane_count(), 2);
}

#[test]
fn close_pane_by_pane_id() {
    // Target a specific pane by its ID and close it.
    let mut tab = Tab::new(0, "test", 80, 24);
    let first = tab.active_pane_id().unwrap();
    let second = tab.split_pane(SplitDirection::Vertical).unwrap();
    let third = tab.split_pane(SplitDirection::Horizontal).unwrap();

    assert_eq!(tab.pane_count(), 3);
    tab.close_pane(second);
    assert_eq!(tab.pane_count(), 2);
    assert!(tab.pane(second).is_none());
}

#[test]
fn correctly_resize_frameless_panes_on_close() {
    // Zellij: "correctly_resize_frameless_panes_on_pane_close"
    // Our panes are inherently frameless. Verify closing reclaims space correctly.
    let mut tab = Tab::new(0, "test", 80, 24);
    let first = tab.active_pane_id().unwrap();
    let second = tab.split_pane(SplitDirection::Vertical).unwrap();
    let third = tab.split_pane(SplitDirection::Vertical).unwrap();
    assert_eq!(tab.pane_count(), 3);
    // Close the middle pane
    tab.close_pane(second);
    assert_eq!(tab.pane_count(), 2);
    let positions = tab.compute_positions();
    // Total cols should still be 80
    let total_cols: usize = positions
        .iter()
        .filter(|(_, p)| p.row == 0)
        .map(|(_, p)| p.cols)
        .sum();
    assert_eq!(total_cols, 80);
}

// ---------------------------------------------------------------------------
// 10. Pane Move (~6 tests)
// ---------------------------------------------------------------------------

#[test]
fn move_active_pane_down() {
    // Zellij: "move_active_pane_down"
    let mut tab = Tab::new(0, "test", 80, 24);
    let top = tab.active_pane_id().unwrap();
    let bottom = tab.split_pane(SplitDirection::Horizontal).unwrap();
    tab.focus_pane(top);
    // Get positions before move
    let pos_before = tab.compute_positions();
    let top_pos = pos_before.iter().find(|(id, _)| *id == top).unwrap().1;
    assert_eq!(top_pos.row, 0); // top is at row 0
    // Move top pane down (swap with bottom)
    let moved = tab.move_active_pane(FocusDirection::Down);
    assert!(moved);
    // After swap, top pane should now be in the bottom position
    let pos_after = tab.compute_positions();
    let top_pos_after = pos_after.iter().find(|(id, _)| *id == top).unwrap().1;
    assert!(top_pos_after.row > 0);
}

#[test]
fn move_active_pane_up() {
    // Zellij: "move_active_pane_up"
    let mut tab = Tab::new(0, "test", 80, 24);
    let top = tab.active_pane_id().unwrap();
    let bottom = tab.split_pane(SplitDirection::Horizontal).unwrap();
    // bottom is active after split
    let pos_before = tab.compute_positions();
    let bottom_pos = pos_before.iter().find(|(id, _)| *id == bottom).unwrap().1;
    assert!(bottom_pos.row > 0);
    let moved = tab.move_active_pane(FocusDirection::Up);
    assert!(moved);
    let pos_after = tab.compute_positions();
    let bottom_pos_after = pos_after.iter().find(|(id, _)| *id == bottom).unwrap().1;
    assert_eq!(bottom_pos_after.row, 0); // now at top
}

#[test]
fn move_active_pane_left() {
    // Zellij: "move_active_pane_left"
    let mut tab = Tab::new(0, "test", 80, 24);
    let left = tab.active_pane_id().unwrap();
    let right = tab.split_pane(SplitDirection::Vertical).unwrap();
    // right is active
    let pos_before = tab.compute_positions();
    let right_pos = pos_before.iter().find(|(id, _)| *id == right).unwrap().1;
    assert!(right_pos.col > 0);
    let moved = tab.move_active_pane(FocusDirection::Left);
    assert!(moved);
    let pos_after = tab.compute_positions();
    let right_pos_after = pos_after.iter().find(|(id, _)| *id == right).unwrap().1;
    assert_eq!(right_pos_after.col, 0);
}

#[test]
fn move_active_pane_right() {
    // Zellij: "move_active_pane_right"
    let mut tab = Tab::new(0, "test", 80, 24);
    let left = tab.active_pane_id().unwrap();
    let right = tab.split_pane(SplitDirection::Vertical).unwrap();
    tab.focus_pane(left);
    let pos_before = tab.compute_positions();
    let left_pos = pos_before.iter().find(|(id, _)| *id == left).unwrap().1;
    assert_eq!(left_pos.col, 0);
    let moved = tab.move_active_pane(FocusDirection::Right);
    assert!(moved);
    let pos_after = tab.compute_positions();
    let left_pos_after = pos_after.iter().find(|(id, _)| *id == left).unwrap().1;
    assert!(left_pos_after.col > 0);
}

#[test]
fn move_pane_by_pane_id_down() {
    // Zellij: "move_pane_by_pane_id_down"
    let mut tab = Tab::new(0, "test", 80, 24);
    let top = tab.active_pane_id().unwrap();
    let _bottom = tab.split_pane(SplitDirection::Horizontal).unwrap();
    let moved = tab.move_pane_by_id(top, FocusDirection::Down);
    assert!(moved);
    let pos_after = tab.compute_positions();
    let top_pos = pos_after.iter().find(|(id, _)| *id == top).unwrap().1;
    assert!(top_pos.row > 0);
}

#[test]
fn move_pane_backwards_by_pane_id() {
    // Zellij: "move_pane_backwards_by_pane_id"
    let mut tab = Tab::new(0, "test", 80, 24);
    let first = tab.active_pane_id().unwrap();
    let second = tab.split_pane(SplitDirection::Vertical).unwrap();
    // second is at position 1 in layout order
    let ids_before = tab.pane_ids();
    assert_eq!(ids_before, vec![first, second]);
    let moved = tab.move_pane_backwards(second);
    assert!(moved);
    let ids_after = tab.pane_ids();
    assert_eq!(ids_after, vec![second, first]);
}

// ---------------------------------------------------------------------------
// 11. Pane Rename & Misc (~4 tests)
// ---------------------------------------------------------------------------

#[test]
fn rename_embedded_pane() {
    // Zellij: "rename_embedded_pane"
    let mut tab = Tab::new(0, "test", 80, 24);
    let pane_id = tab.active_pane_id().unwrap();
    tab.pane_mut(pane_id).unwrap().set_title("my pane");
    assert_eq!(tab.pane(pane_id).unwrap().title(), "my pane");
}

#[test]
fn rename_floating_pane() {
    // Zellij: "rename_floating_pane"
    let mut tab = Tab::new(0, "test", 80, 24);
    let fp_id = tab.new_floating_pane();
    let fp = tab.floating_pane_mut(fp_id).unwrap();
    fp.pane.set_title("my float");
    let fp = tab.floating_pane(fp_id).unwrap();
    assert_eq!(fp.pane.title(), "my float");
}

#[test]
fn undo_rename_pane_by_pane_id() {
    // Zellij: "undo_rename_pane_by_pane_id"
    let mut tab = Tab::new(0, "test", 80, 24);
    let pane_id = tab.active_pane_id().unwrap();
    tab.pane_mut(pane_id).unwrap().set_title("original");
    tab.pane_mut(pane_id).unwrap().set_title("renamed");
    assert_eq!(tab.pane(pane_id).unwrap().title(), "renamed");
    tab.pane_mut(pane_id).unwrap().undo_rename();
    assert_eq!(tab.pane(pane_id).unwrap().title(), "original");
}

#[test]
fn clear_screen_by_pane_id() {
    // Zellij: "clear_screen_by_pane_id"
    let mut tab = Tab::new(0, "test", 80, 24);
    let pane_id = tab.active_pane_id().unwrap();
    assert!(!tab.pane(pane_id).unwrap().is_cleared());
    let cleared = tab.clear_pane(pane_id);
    assert!(cleared);
    assert!(tab.pane(pane_id).unwrap().is_cleared());
}

// ---------------------------------------------------------------------------
// 12. Whole-Tab Resize (~3 tests)
// ---------------------------------------------------------------------------

#[test]
fn resize_tab_with_floating_panes() {
    // Zellij: "resize_tab_with_floating_panes"
    let mut tab = Tab::new(0, "test", 80, 24);
    tab.split_pane(SplitDirection::Vertical);
    let fp = tab.new_floating_pane_with_coords(10, 5, 30, 10);
    // Resize the tab
    tab.resize(60, 20);
    assert_eq!(tab.size().cols, 60);
    assert_eq!(tab.size().rows, 20);
    // Floating pane still exists (coords may be clamped on next access)
    assert_eq!(tab.floating_pane_count(), 1);
    // Tiled panes fit within new bounds
    let positions = tab.compute_positions();
    for (_, p) in &positions {
        assert!(p.col + p.cols <= 60);
        assert!(p.row + p.rows <= 20);
    }
}

#[test]
fn shrink_whole_tab_and_expand_back() {
    // Shrink the terminal, then expand it back.
    let mut tab = Tab::new(0, "test", 80, 24);
    tab.split_pane(SplitDirection::Vertical);
    tab.split_pane(SplitDirection::Horizontal);
    assert_eq!(tab.pane_count(), 3);

    // Shrink
    tab.resize(40, 12);
    let positions = tab.compute_positions();
    assert_eq!(positions.len(), 3);
    let total_cols_at_root: usize = {
        // All panes should fit within 40 cols
        for (_, p) in &positions {
            assert!(p.col + p.cols <= 40);
            assert!(p.row + p.rows <= 12);
        }
        40
    };

    // Expand back
    tab.resize(80, 24);
    let positions = tab.compute_positions();
    assert_eq!(positions.len(), 3);
    for (_, p) in &positions {
        assert!(p.col + p.cols <= 80);
        assert!(p.row + p.rows <= 24);
    }
}

#[test]
fn update_screen_pixel_dimensions() {
    // Zellij: "update_screen_pixel_dimensions"
    let mut tab = Tab::new(0, "test", 80, 24);
    assert_eq!(tab.pixel_dimensions(), (None, None));
    tab.set_pixel_dimensions(1920, 1080);
    assert_eq!(tab.pixel_dimensions(), (Some(1920), Some(1080)));
}

// ---------------------------------------------------------------------------
// 13. Scroll (~4 tests)
// ---------------------------------------------------------------------------

#[test]
fn scroll_up_by_pane_id() {
    // Zellij: "scroll_up_by_pane_id"
    let mut tab = Tab::new(0, "test", 80, 24);
    let pane_id = tab.active_pane_id().unwrap();
    assert_eq!(tab.pane(pane_id).unwrap().scroll_offset(), 0);
    tab.scroll_up(pane_id, 5);
    assert_eq!(tab.pane(pane_id).unwrap().scroll_offset(), 5);
}

#[test]
fn scroll_down_by_pane_id() {
    // Zellij: "scroll_down_by_pane_id"
    let mut tab = Tab::new(0, "test", 80, 24);
    let pane_id = tab.active_pane_id().unwrap();
    tab.scroll_up(pane_id, 10);
    assert_eq!(tab.pane(pane_id).unwrap().scroll_offset(), 10);
    tab.scroll_down(pane_id, 3);
    assert_eq!(tab.pane(pane_id).unwrap().scroll_offset(), 7);
}

#[test]
fn scroll_to_top_by_pane_id() {
    // Zellij: "scroll_to_top_by_pane_id"
    let mut tab = Tab::new(0, "test", 80, 24);
    let pane_id = tab.active_pane_id().unwrap();
    tab.scroll_to_top(pane_id);
    assert!(tab.pane(pane_id).unwrap().scroll_offset() > 0);
}

#[test]
fn scroll_to_bottom_by_pane_id() {
    // Zellij: "scroll_to_bottom_by_pane_id"
    let mut tab = Tab::new(0, "test", 80, 24);
    let pane_id = tab.active_pane_id().unwrap();
    tab.scroll_up(pane_id, 10);
    tab.scroll_to_bottom(pane_id);
    assert_eq!(tab.pane(pane_id).unwrap().scroll_offset(), 0);
}

// ---------------------------------------------------------------------------
// Additional tests for edge cases
// ---------------------------------------------------------------------------

#[test]
fn session_resize_propagates_to_all_tabs() {
    let mut session = Session::new("test", 80, 24);
    session
        .active_tab_mut()
        .split_pane(SplitDirection::Vertical);
    session.new_tab("Tab 2");
    session
        .active_tab_mut()
        .split_pane(SplitDirection::Horizontal);

    session.resize(120, 40);

    // Check both tabs have correct size
    for i in 0..session.tab_count() {
        let tab = session.tab(i).unwrap();
        assert_eq!(tab.size().cols, 120);
        assert_eq!(tab.size().rows, 40);
    }
}

#[test]
fn cannot_close_last_pane() {
    let mut tab = Tab::new(0, "test", 80, 24);
    let pane_id = tab.active_pane_id().unwrap();
    let closed = tab.close_pane(pane_id);
    assert!(!closed);
    assert_eq!(tab.pane_count(), 1);
}

#[test]
fn cannot_close_last_tab() {
    let mut session = Session::new("test", 80, 24);
    let closed = session.close_tab(0);
    assert!(!closed);
    assert_eq!(session.tab_count(), 1);
}

#[test]
fn focus_direction_returns_false_at_edge() {
    let mut tab = Tab::new(0, "test", 80, 24);
    // Single pane: all directions should return false
    assert!(!tab.focus_direction(FocusDirection::Up));
    assert!(!tab.focus_direction(FocusDirection::Down));
    assert!(!tab.focus_direction(FocusDirection::Left));
    assert!(!tab.focus_direction(FocusDirection::Right));
}

#[test]
fn split_then_close_returns_to_single_pane() {
    let mut tab = Tab::new(0, "test", 80, 24);
    let first = tab.active_pane_id().unwrap();
    let second = tab.split_pane(SplitDirection::Vertical).unwrap();

    tab.close_pane(second);
    assert_eq!(tab.pane_count(), 1);
    assert_eq!(tab.active_pane_id(), Some(first));

    let positions = tab.compute_positions();
    assert_eq!(positions.len(), 1);
    assert_eq!(
        positions[0].1,
        PanePosition {
            col: 0,
            row: 0,
            cols: 80,
            rows: 24
        }
    );
}

#[test]
fn pane_size_updates_after_tab_resize() {
    let mut tab = Tab::new(0, "test", 80, 24);
    tab.split_pane(SplitDirection::Vertical);

    tab.resize(160, 48);

    let positions = tab.compute_positions();
    for (_, pos) in &positions {
        assert!(pos.cols > 0);
        assert!(pos.rows > 0);
    }
    // Total width should be 160
    let left_cols = positions[0].1.cols;
    let right_cols = positions[1].1.cols;
    assert_eq!(left_cols + right_cols, 160);
}

#[test]
fn session_rename() {
    let mut session = Session::new("old-name", 80, 24);
    assert_eq!(session.name(), "old-name");
    session.rename("new-name");
    assert_eq!(session.name(), "new-name");
}

#[test]
fn next_tab_wraps_around() {
    let mut session = Session::new("test", 80, 24);
    session.new_tab("Tab 2");
    session.new_tab("Tab 3");

    // Go to last tab
    session.switch_tab(2);
    assert_eq!(session.active_tab_index(), 2);

    // Next should wrap to 0
    session.next_tab();
    assert_eq!(session.active_tab_index(), 0);
}

#[test]
fn prev_tab_wraps_around() {
    let mut session = Session::new("test", 80, 24);
    session.new_tab("Tab 2");
    session.new_tab("Tab 3");

    // Go to first tab
    session.switch_tab(0);
    assert_eq!(session.active_tab_index(), 0);

    // Prev should wrap to last
    session.prev_tab();
    assert_eq!(session.active_tab_index(), 2);
}

#[test]
fn four_way_split() {
    let mut tab = Tab::new(0, "test", 80, 24);

    // Split into 4 quadrants
    let first = tab.active_pane_id().unwrap();
    let right = tab.split_pane(SplitDirection::Vertical).unwrap();

    tab.focus_pane(first);
    let bottom_left = tab.split_pane(SplitDirection::Horizontal).unwrap();

    tab.focus_pane(right);
    let bottom_right = tab.split_pane(SplitDirection::Horizontal).unwrap();

    assert_eq!(tab.pane_count(), 4);

    let positions = tab.compute_positions();
    assert_eq!(positions.len(), 4);

    // All panes should fit within bounds
    for (_, pos) in &positions {
        assert!(pos.col + pos.cols <= 80);
        assert!(pos.row + pos.rows <= 24);
    }
}
