//! Level-2 integration tests for emux-mux.
//!
//! These tests exercise the integration between Session, Tab, and Pane,
//! focusing on lifecycle management, resize propagation, and state
//! consistency across operations.

use emux_mux::{Session, SplitDirection, Tab};

// ---------------------------------------------------------------------------
// 1. Session resize propagation
// ---------------------------------------------------------------------------

#[test]
fn session_resize_updates_all_tabs_and_panes() {
    let mut session = Session::new("test", 80, 24);

    // Create 3 tabs, split panes in each
    session.new_tab("Tab 2");
    session.new_tab("Tab 3");

    // Split panes in tab 0
    session.switch_tab(0);
    session
        .active_tab_mut()
        .split_pane(SplitDirection::Vertical);

    // Split panes in tab 1
    session.switch_tab(1);
    session
        .active_tab_mut()
        .split_pane(SplitDirection::Horizontal);

    // Split panes in tab 2
    session.switch_tab(2);
    session
        .active_tab_mut()
        .split_pane(SplitDirection::Vertical);
    session
        .active_tab_mut()
        .split_pane(SplitDirection::Horizontal);

    // Resize the session
    session.resize(120, 40);

    // Verify session size updated
    let size = session.size();
    assert_eq!(size.cols, 120);
    assert_eq!(size.rows, 40);

    // Verify every tab's pane positions fit within the new size
    for tab_idx in 0..session.tab_count() {
        let tab = session.tab(tab_idx).unwrap();
        let positions = tab.compute_positions();
        for (pane_id, pos) in &positions {
            assert!(
                pos.col + pos.cols <= 120,
                "tab {tab_idx}, pane {pane_id}: col {} + cols {} > 120",
                pos.col,
                pos.cols
            );
            assert!(
                pos.row + pos.rows <= 40,
                "tab {tab_idx}, pane {pane_id}: row {} + rows {} > 40",
                pos.row,
                pos.rows
            );
        }
    }
}

#[test]
fn session_resize_pane_sizes_sum_to_total() {
    let mut session = Session::new("test", 80, 24);
    session
        .active_tab_mut()
        .split_pane(SplitDirection::Vertical);
    session
        .active_tab_mut()
        .split_pane(SplitDirection::Vertical);

    session.resize(120, 40);

    let positions = session.active_tab().compute_positions();
    // For a purely vertical split, all panes share the same rows, cols sum to total
    let total_cols: usize = positions.iter().map(|(_, p)| p.cols).sum();
    assert_eq!(
        total_cols, 120,
        "vertical split cols should sum to total width"
    );
    for (_, pos) in &positions {
        assert_eq!(pos.rows, 40);
    }
}

#[test]
fn session_resize_horizontal_split_rows_sum_to_total() {
    let mut session = Session::new("test", 80, 24);
    session
        .active_tab_mut()
        .split_pane(SplitDirection::Horizontal);
    session
        .active_tab_mut()
        .split_pane(SplitDirection::Horizontal);

    session.resize(100, 30);

    let positions = session.active_tab().compute_positions();
    let total_rows: usize = positions.iter().map(|(_, p)| p.rows).sum();
    assert_eq!(
        total_rows, 30,
        "horizontal split rows should sum to total height"
    );
    for (_, pos) in &positions {
        assert_eq!(pos.cols, 100);
    }
}

#[test]
fn session_resize_smaller_clamps_pane_sizes() {
    let mut session = Session::new("test", 80, 24);
    // 3 vertical splits
    session
        .active_tab_mut()
        .split_pane(SplitDirection::Vertical);
    session
        .active_tab_mut()
        .split_pane(SplitDirection::Vertical);
    session
        .active_tab_mut()
        .split_pane(SplitDirection::Vertical);

    // Resize much smaller
    session.resize(20, 10);

    let positions = session.active_tab().compute_positions();
    for (_, pos) in &positions {
        assert!(pos.cols >= 1, "pane should have at least 1 col");
        assert!(pos.rows >= 1, "pane should have at least 1 row");
    }
    let total_cols: usize = positions.iter().map(|(_, p)| p.cols).sum();
    assert_eq!(total_cols, 20);
}

// ---------------------------------------------------------------------------
// 2. Tab lifecycle: create → switch → close → verify
// ---------------------------------------------------------------------------

#[test]
fn tab_lifecycle_create_switch_close() {
    let mut session = Session::new("test", 80, 24);
    assert_eq!(session.tab_count(), 1);
    assert_eq!(session.active_tab_index(), 0);

    // Create tabs
    let _tab1_id = session.new_tab("Tab 2");
    assert_eq!(session.tab_count(), 2);
    assert_eq!(session.active_tab_index(), 1); // new tab is active

    let _tab2_id = session.new_tab("Tab 3");
    assert_eq!(session.tab_count(), 3);
    assert_eq!(session.active_tab_index(), 2);

    // Switch to first tab
    assert!(session.switch_tab(0));
    assert_eq!(session.active_tab_index(), 0);
    assert_eq!(session.active_tab().name(), "Tab 1");

    // Switch to second tab
    assert!(session.switch_tab(1));
    assert_eq!(session.active_tab_index(), 1);
    assert_eq!(session.active_tab().name(), "Tab 2");

    // Close the middle tab
    assert!(session.close_tab(1));
    assert_eq!(session.tab_count(), 2);

    // Active tab index should be adjusted
    assert!(session.active_tab_index() < session.tab_count());

    // Remaining tab names
    let names = session.tab_names();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"Tab 1"));
    assert!(names.contains(&"Tab 3"));
}

#[test]
fn tab_lifecycle_cannot_close_last_tab() {
    let mut session = Session::new("test", 80, 24);
    assert_eq!(session.tab_count(), 1);
    assert!(!session.close_tab(0), "should not close the last tab");
    assert_eq!(session.tab_count(), 1);
}

#[test]
fn tab_lifecycle_close_active_tab_selects_neighbor() {
    let mut session = Session::new("test", 80, 24);
    session.new_tab("Tab 2");
    session.new_tab("Tab 3");

    // Active is tab 2 (index 2)
    assert_eq!(session.active_tab_index(), 2);

    // Close active tab
    assert!(session.close_tab(2));
    assert_eq!(session.tab_count(), 2);
    // Active should now be the last remaining tab (index 1)
    assert!(session.active_tab_index() < session.tab_count());
}

#[test]
fn tab_lifecycle_close_first_tab_adjusts_index() {
    let mut session = Session::new("test", 80, 24);
    session.new_tab("Tab 2");
    session.new_tab("Tab 3");

    // Switch to tab 2 (index 2)
    session.switch_tab(2);
    assert_eq!(session.active_tab_index(), 2);

    // Close the first tab (index 0)
    assert!(session.close_tab(0));
    assert_eq!(session.tab_count(), 2);
    // Active tab index should shift down by 1 since the deleted tab was before it
    assert_eq!(session.active_tab_index(), 1);
}

#[test]
fn tab_lifecycle_switch_out_of_bounds_fails() {
    let mut session = Session::new("test", 80, 24);
    session.new_tab("Tab 2");

    assert!(
        !session.switch_tab(5),
        "switching to non-existent tab should fail"
    );
    // Active tab should not change
    assert_eq!(session.active_tab_index(), 1);
}

#[test]
fn tab_lifecycle_next_prev_cycle() {
    let mut session = Session::new("test", 80, 24);
    session.new_tab("Tab 2");
    session.new_tab("Tab 3");

    session.switch_tab(0);

    // Cycle forward through all tabs
    session.next_tab();
    assert_eq!(session.active_tab_index(), 1);
    session.next_tab();
    assert_eq!(session.active_tab_index(), 2);
    session.next_tab();
    assert_eq!(session.active_tab_index(), 0); // wrap

    // Cycle backward
    session.prev_tab();
    assert_eq!(session.active_tab_index(), 2); // wrap
    session.prev_tab();
    assert_eq!(session.active_tab_index(), 1);
    session.prev_tab();
    assert_eq!(session.active_tab_index(), 0);
}

#[test]
fn tab_lifecycle_toggle_previous_tab() {
    let mut session = Session::new("test", 80, 24);
    session.new_tab("Tab 2");
    session.new_tab("Tab 3");

    session.switch_tab(0);
    session.switch_tab(2); // previous = 0, active = 2

    assert!(session.toggle_previous_tab());
    assert_eq!(session.active_tab_index(), 0);

    assert!(session.toggle_previous_tab());
    assert_eq!(session.active_tab_index(), 2);
}

#[test]
fn tab_lifecycle_panes_independent_across_tabs() {
    // Verify that splitting panes in one tab doesn't affect another
    let mut session = Session::new("test", 80, 24);
    session.new_tab("Tab 2");

    // Split panes in tab 0
    session.switch_tab(0);
    session
        .active_tab_mut()
        .split_pane(SplitDirection::Vertical);
    session
        .active_tab_mut()
        .split_pane(SplitDirection::Horizontal);
    assert_eq!(session.active_tab().pane_count(), 3);

    // Tab 1 should still have 1 pane
    assert_eq!(session.tab(1).unwrap().pane_count(), 1);
}

// ---------------------------------------------------------------------------
// 3. Pane split + close cycle
// ---------------------------------------------------------------------------

#[test]
fn pane_split_close_cycle_vertical() {
    let mut tab = Tab::new(0, "test", 80, 24);
    let _p0 = tab.active_pane_id().unwrap();

    // Split 3 times vertically
    let p1 = tab.split_pane(SplitDirection::Vertical).unwrap();
    let _p2 = tab.split_pane(SplitDirection::Vertical).unwrap();
    let _p3 = tab.split_pane(SplitDirection::Vertical).unwrap();
    assert_eq!(tab.pane_count(), 4);

    // Close the middle pane
    assert!(tab.close_pane(p1));
    assert_eq!(tab.pane_count(), 3);
    assert!(tab.pane(p1).is_none(), "closed pane should not exist");

    // Verify remaining panes are valid
    let positions = tab.compute_positions();
    assert_eq!(positions.len(), 3);
    let total_cols: usize = positions.iter().map(|(_, p)| p.cols).sum();
    assert_eq!(total_cols, 80, "remaining panes should fill the width");
}

#[test]
fn pane_split_close_cycle_horizontal() {
    let mut tab = Tab::new(0, "test", 80, 24);
    let p0 = tab.active_pane_id().unwrap();

    // Split 3 times horizontally
    tab.focus_pane(p0);
    let _p1 = tab.split_pane(SplitDirection::Horizontal).unwrap();
    tab.focus_pane(p0);
    let _p2 = tab.split_pane(SplitDirection::Horizontal).unwrap();
    assert_eq!(tab.pane_count(), 3);

    // Close the first pane
    assert!(tab.close_pane(p0));
    assert_eq!(tab.pane_count(), 2);

    // Verify remaining panes
    let positions = tab.compute_positions();
    assert_eq!(positions.len(), 2);
    let total_rows: usize = positions.iter().map(|(_, p)| p.rows).sum();
    assert_eq!(total_rows, 24, "remaining panes should fill the height");
}

#[test]
fn pane_split_close_reverse_order() {
    // Split into 4 panes, close them in reverse creation order
    let mut tab = Tab::new(0, "test", 80, 24);
    let p0 = tab.active_pane_id().unwrap();
    let p1 = tab.split_pane(SplitDirection::Vertical).unwrap();
    tab.focus_pane(p0);
    let p2 = tab.split_pane(SplitDirection::Horizontal).unwrap();
    tab.focus_pane(p1);
    let p3 = tab.split_pane(SplitDirection::Horizontal).unwrap();
    assert_eq!(tab.pane_count(), 4);

    // Close in reverse order: p3, p2, p1
    assert!(tab.close_pane(p3));
    assert_eq!(tab.pane_count(), 3);
    verify_no_overlap(&tab);

    assert!(tab.close_pane(p2));
    assert_eq!(tab.pane_count(), 2);
    verify_no_overlap(&tab);

    assert!(tab.close_pane(p1));
    assert_eq!(tab.pane_count(), 1);
    // Last pane should be full size
    let positions = tab.compute_positions();
    assert_eq!(positions.len(), 1);
    assert_eq!(positions[0].1.cols, 80);
    assert_eq!(positions[0].1.rows, 24);
}

#[test]
fn pane_split_close_creation_order() {
    // Split into 4 panes, close them in creation order (except last)
    let mut tab = Tab::new(0, "test", 80, 24);
    let p0 = tab.active_pane_id().unwrap();
    let p1 = tab.split_pane(SplitDirection::Vertical).unwrap();
    tab.focus_pane(p0);
    let p2 = tab.split_pane(SplitDirection::Horizontal).unwrap();
    tab.focus_pane(p1);
    let _p3 = tab.split_pane(SplitDirection::Horizontal).unwrap();
    assert_eq!(tab.pane_count(), 4);

    // Close in creation order: p0, p1, p2 (can't close last pane)
    assert!(tab.close_pane(p0));
    assert_eq!(tab.pane_count(), 3);
    verify_no_overlap(&tab);

    assert!(tab.close_pane(p1));
    assert_eq!(tab.pane_count(), 2);
    verify_no_overlap(&tab);

    assert!(tab.close_pane(p2));
    assert_eq!(tab.pane_count(), 1);
    let positions = tab.compute_positions();
    assert_eq!(positions[0].1.cols, 80);
    assert_eq!(positions[0].1.rows, 24);
}

#[test]
fn pane_close_last_pane_fails() {
    let mut tab = Tab::new(0, "test", 80, 24);
    let p0 = tab.active_pane_id().unwrap();
    assert!(!tab.close_pane(p0), "cannot close the last pane");
    assert_eq!(tab.pane_count(), 1);
}

#[test]
fn pane_close_nonexistent_pane_fails() {
    let mut tab = Tab::new(0, "test", 80, 24);
    assert!(!tab.close_pane(999), "closing nonexistent pane should fail");
}

#[test]
fn pane_close_active_updates_focus() {
    let mut tab = Tab::new(0, "test", 80, 24);
    let p0 = tab.active_pane_id().unwrap();
    let p1 = tab.split_pane(SplitDirection::Vertical).unwrap();

    // p1 is active after split
    assert_eq!(tab.active_pane_id(), Some(p1));

    // Close the active pane
    assert!(tab.close_pane(p1));

    // Focus should move to remaining pane
    assert_eq!(tab.active_pane_id(), Some(p0));
}

#[test]
fn pane_split_close_mixed_directions() {
    // Stress test: alternate horizontal and vertical splits, then close randomly
    let mut tab = Tab::new(0, "test", 120, 40);
    let mut pane_ids = vec![tab.active_pane_id().unwrap()];

    // Create 6 panes with alternating split directions
    for i in 0..5 {
        let dir = if i % 2 == 0 {
            SplitDirection::Vertical
        } else {
            SplitDirection::Horizontal
        };
        if let Some(id) = tab.split_pane(dir) {
            pane_ids.push(id);
        }
    }

    let total_panes = tab.pane_count();
    assert!(total_panes >= 3, "should have at least 3 panes");

    // Close panes from the middle outward
    let to_close: Vec<_> = pane_ids[1..pane_ids.len() - 1].to_vec();
    for id in to_close {
        if tab.pane_count() > 1 {
            tab.close_pane(id);
            verify_positions_in_bounds(&tab, 120, 40);
        }
    }

    // At least one pane should remain
    assert!(tab.pane_count() >= 1);
    assert!(tab.active_pane_id().is_some());
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Verify that no two pane positions overlap.
fn verify_no_overlap(tab: &Tab) {
    let positions = tab.compute_positions();
    for i in 0..positions.len() {
        for j in (i + 1)..positions.len() {
            let a = &positions[i].1;
            let b = &positions[j].1;
            let h_overlap = a.col < b.col + b.cols && b.col < a.col + a.cols;
            let v_overlap = a.row < b.row + b.rows && b.row < a.row + a.rows;
            assert!(
                !(h_overlap && v_overlap),
                "panes {} and {} overlap: {:?} vs {:?}",
                positions[i].0,
                positions[j].0,
                a,
                b
            );
        }
    }
}

/// Verify all pane positions are within bounds.
fn verify_positions_in_bounds(tab: &Tab, max_cols: usize, max_rows: usize) {
    let positions = tab.compute_positions();
    for (id, pos) in &positions {
        assert!(
            pos.col + pos.cols <= max_cols,
            "pane {id}: col {} + cols {} > {max_cols}",
            pos.col,
            pos.cols
        );
        assert!(
            pos.row + pos.rows <= max_rows,
            "pane {id}: row {} + rows {} > {max_rows}",
            pos.row,
            pos.rows
        );
    }
}
