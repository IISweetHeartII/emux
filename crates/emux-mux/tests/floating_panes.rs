//! TDD specs for floating pane support.
//!
//! Floating panes hover above the tiled layout and can be freely positioned,
//! resized, toggled, and re-embedded. These tests define the expected behavior
//! before implementation begins.

use emux_mux::Tab;

// ---------------------------------------------------------------------------
// 1. Creation
// ---------------------------------------------------------------------------

#[test]
fn create_floating_pane_default_size_and_position() {
    // A floating pane created without explicit geometry should appear centered
    // in the tab with a sensible default size (e.g., 50% width, 50% height).
    let mut tab = Tab::new(0, "test", 80, 24);
    let fp_id = tab.new_floating_pane();
    assert_eq!(tab.floating_pane_count(), 1);
    let fp = tab.floating_pane(fp_id).unwrap();
    assert_eq!(fp.width, 40);
    assert_eq!(fp.height, 12);
    // Centered
    assert_eq!(fp.x, 20);
    assert_eq!(fp.y, 6);
}

#[test]
fn create_floating_pane_with_custom_coordinates() {
    // When x, y, width, height are specified, the floating pane must use
    // exactly those values.
    let mut tab = Tab::new(0, "test", 80, 24);
    let fp_id = tab.new_floating_pane_with_coords(10, 5, 30, 10);
    let fp = tab.floating_pane(fp_id).unwrap();
    assert_eq!(fp.x, 10);
    assert_eq!(fp.y, 5);
    assert_eq!(fp.width, 30);
    assert_eq!(fp.height, 10);
}

#[test]
fn create_floating_pane_clamped_to_viewport() {
    // If the requested position+size exceeds the terminal viewport, the pane
    // should be clamped so it stays fully visible.
    let mut tab = Tab::new(0, "test", 80, 24);
    let fp_id = tab.new_floating_pane_with_coords(70, 20, 30, 10);
    let fp = tab.floating_pane(fp_id).unwrap();
    // Width clamped to 30 (fits), x clamped so pane stays in viewport
    assert!(fp.x + fp.width <= 80);
    assert!(fp.y + fp.height <= 24);
}

// ---------------------------------------------------------------------------
// 2. Visibility toggle
// ---------------------------------------------------------------------------

#[test]
fn toggle_floating_pane_off() {
    // Hiding a visible floating pane removes it from the render tree but keeps
    // its process running.
    let mut tab = Tab::new(0, "test", 80, 24);
    let _fp_id = tab.new_floating_pane();
    assert!(tab.is_floating_visible());
    tab.toggle_floating_panes();
    assert!(!tab.is_floating_visible());
    // Still exists
    assert_eq!(tab.floating_pane_count(), 1);
}

#[test]
fn toggle_floating_pane_on() {
    // Showing a hidden floating pane restores it at its previous position.
    let mut tab = Tab::new(0, "test", 80, 24);
    let fp_id = tab.new_floating_pane_with_coords(10, 5, 30, 10);
    tab.toggle_floating_panes(); // hide
    tab.toggle_floating_panes(); // show
    assert!(tab.is_floating_visible());
    let fp = tab.floating_pane(fp_id).unwrap();
    assert_eq!(fp.x, 10);
    assert_eq!(fp.y, 5);
}

#[test]
fn toggle_all_floating_panes() {
    // A global toggle hides/shows every floating pane at once.
    let mut tab = Tab::new(0, "test", 80, 24);
    tab.new_floating_pane();
    tab.new_floating_pane();
    assert_eq!(tab.floating_pane_count(), 2);
    tab.toggle_floating_panes();
    assert!(!tab.is_floating_visible());
    // Positions in compute_positions should not include floating panes
    let positions = tab.compute_positions();
    // Only the tiled pane should be there
    assert_eq!(positions.len(), 1);
    tab.toggle_floating_panes();
    let positions = tab.compute_positions();
    // Tiled + 2 floating
    assert_eq!(positions.len(), 3);
}

// ---------------------------------------------------------------------------
// 3. Movement
// ---------------------------------------------------------------------------

#[test]
fn move_floating_pane_by_offset() {
    // Moving a pane by (dx, dy) should update its top-left corner accordingly.
    let mut tab = Tab::new(0, "test", 80, 24);
    let fp_id = tab.new_floating_pane_with_coords(10, 5, 20, 10);
    tab.move_floating_pane(fp_id, 15, 8);
    let fp = tab.floating_pane(fp_id).unwrap();
    assert_eq!(fp.x, 15);
    assert_eq!(fp.y, 8);
}

#[test]
fn move_floating_pane_clamped_to_viewport() {
    // Dragging a pane past the terminal edge should clamp it inside the
    // viewport.
    let mut tab = Tab::new(0, "test", 80, 24);
    let fp_id = tab.new_floating_pane_with_coords(10, 5, 20, 10);
    tab.move_floating_pane(fp_id, 200, 200);
    let fp = tab.floating_pane(fp_id).unwrap();
    assert!(fp.x + fp.width <= 80);
    assert!(fp.y + fp.height <= 24);
}

// ---------------------------------------------------------------------------
// 4. Resizing
// ---------------------------------------------------------------------------

#[test]
fn resize_floating_pane_larger() {
    // Increasing width/height updates geometry and notifies the PTY of the
    // new size.
    let mut tab = Tab::new(0, "test", 80, 24);
    let fp_id = tab.new_floating_pane_with_coords(5, 3, 20, 10);
    tab.resize_floating_pane(fp_id, 40, 15);
    let fp = tab.floating_pane(fp_id).unwrap();
    assert_eq!(fp.width, 40);
    assert_eq!(fp.height, 15);
    assert_eq!(fp.pane.size().cols, 40);
    assert_eq!(fp.pane.size().rows, 15);
}

#[test]
fn resize_floating_pane_smaller() {
    // Decreasing width/height works symmetrically.
    let mut tab = Tab::new(0, "test", 80, 24);
    let fp_id = tab.new_floating_pane_with_coords(5, 3, 30, 15);
    tab.resize_floating_pane(fp_id, 10, 5);
    let fp = tab.floating_pane(fp_id).unwrap();
    assert_eq!(fp.width, 10);
    assert_eq!(fp.height, 5);
}

#[test]
fn resize_floating_pane_minimum_size() {
    // A floating pane cannot be resized below a minimum (e.g., 5x2).
    let mut tab = Tab::new(0, "test", 80, 24);
    let fp_id = tab.new_floating_pane_with_coords(5, 3, 20, 10);
    tab.resize_floating_pane(fp_id, 1, 1);
    let fp = tab.floating_pane(fp_id).unwrap();
    assert!(fp.width >= 5);
    assert!(fp.height >= 2);
}

// ---------------------------------------------------------------------------
// 5. Focus
// ---------------------------------------------------------------------------

#[test]
fn focus_floating_pane_brings_to_front() {
    // Focusing a floating pane should move it to the top of the z-order.
    let mut tab = Tab::new(0, "test", 80, 24);
    let fp1 = tab.new_floating_pane_with_coords(0, 0, 20, 10);
    let _fp2 = tab.new_floating_pane_with_coords(5, 5, 20, 10);
    // fp2 is on top now; focus fp1 to bring it to front
    tab.focus_pane(fp1);
    let ids = tab.floating_pane_ids();
    assert_eq!(*ids.last().unwrap(), fp1);
}

#[test]
fn focus_cycle_includes_floating_panes() {
    // Cycling focus (next pane) should visit both tiled and floating panes.
    let mut tab = Tab::new(0, "test", 80, 24);
    let tiled_id = tab.pane_ids()[0];
    let fp_id = tab.new_floating_pane();
    // Currently focused on fp_id (just created)
    assert_eq!(tab.active_pane_id(), Some(fp_id));
    tab.focus_next();
    // Should wrap to tiled
    assert_eq!(tab.active_pane_id(), Some(tiled_id));
    tab.focus_next();
    // Back to floating
    assert_eq!(tab.active_pane_id(), Some(fp_id));
}

#[test]
fn focus_cycle_from_tiled_to_floating() {
    // After the last tiled pane, focus should jump to the first floating pane.
    let mut tab = Tab::new(0, "test", 80, 24);
    let tiled_id = tab.pane_ids()[0];
    let fp_id = tab.new_floating_pane();
    // Focus the tiled pane first
    tab.focus_pane(tiled_id);
    assert_eq!(tab.active_pane_id(), Some(tiled_id));
    tab.focus_next();
    assert_eq!(tab.active_pane_id(), Some(fp_id));
}

#[test]
fn focus_cycle_from_floating_to_tiled() {
    // After the last floating pane, focus should wrap back to the first tiled
    // pane.
    let mut tab = Tab::new(0, "test", 80, 24);
    let tiled_id = tab.pane_ids()[0];
    let fp_id = tab.new_floating_pane();
    // Focus on floating pane
    assert_eq!(tab.active_pane_id(), Some(fp_id));
    tab.focus_next();
    assert_eq!(tab.active_pane_id(), Some(tiled_id));
}

// ---------------------------------------------------------------------------
// 6. Multiple floating panes
// ---------------------------------------------------------------------------

#[test]
fn multiple_floating_panes_coexist() {
    // Several floating panes can exist simultaneously, each with independent
    // position and size.
    let mut tab = Tab::new(0, "test", 80, 24);
    let fp1 = tab.new_floating_pane_with_coords(0, 0, 20, 10);
    let fp2 = tab.new_floating_pane_with_coords(30, 5, 15, 8);
    let fp3 = tab.new_floating_pane_with_coords(50, 10, 25, 12);
    assert_eq!(tab.floating_pane_count(), 3);
    let f1 = tab.floating_pane(fp1).unwrap();
    let f2 = tab.floating_pane(fp2).unwrap();
    let f3 = tab.floating_pane(fp3).unwrap();
    assert_eq!(f1.x, 0);
    assert_eq!(f2.x, 30);
    assert_eq!(f3.x, 50);
}

#[test]
fn overlap_detection_reports_collisions() {
    // When two floating panes overlap, the system should be able to report
    // which panes collide (advisory, not blocking).
    let mut tab = Tab::new(0, "test", 80, 24);
    let fp1 = tab.new_floating_pane_with_coords(0, 0, 20, 10);
    let fp2 = tab.new_floating_pane_with_coords(10, 5, 20, 10);
    let fp3 = tab.new_floating_pane_with_coords(60, 0, 10, 5);
    assert!(tab.floating_panes_overlap(fp1, fp2));
    assert!(!tab.floating_panes_overlap(fp1, fp3));
    let overlaps = tab.overlapping_floating_panes();
    assert_eq!(overlaps.len(), 1);
    assert!(overlaps.contains(&(fp1, fp2)));
}

// ---------------------------------------------------------------------------
// 7. Interaction with tiled layout
// ---------------------------------------------------------------------------

#[test]
fn floating_pane_renders_over_tiled_layout() {
    // In the render order, floating panes must appear above every tiled pane.
    let mut tab = Tab::new(0, "test", 80, 24);
    let tiled_id = tab.pane_ids()[0];
    let fp_id = tab.new_floating_pane();
    let positions = tab.compute_positions();
    // The tiled pane should come first, then the floating pane
    assert_eq!(positions[0].0, tiled_id);
    assert_eq!(positions[1].0, fp_id);
}

#[test]
fn close_floating_pane() {
    // Closing a floating pane removes it entirely and does not affect the
    // tiled layout beneath.
    let mut tab = Tab::new(0, "test", 80, 24);
    let fp_id = tab.new_floating_pane();
    assert_eq!(tab.floating_pane_count(), 1);
    tab.close_floating_pane(fp_id);
    assert_eq!(tab.floating_pane_count(), 0);
    // Tiled pane still exists
    assert_eq!(tab.pane_count(), 1);
}

#[test]
fn embed_floating_pane_into_tiled_layout() {
    // A floating pane can be "pinned" back into the tiled layout, becoming a
    // regular split pane. Its process continues without restart.
    let mut tab = Tab::new(0, "test", 80, 24);
    let fp_id = tab.new_floating_pane();
    assert_eq!(tab.floating_pane_count(), 1);
    assert_eq!(tab.pane_count(), 1); // tiled panes only
    tab.embed_floating_pane(fp_id);
    assert_eq!(tab.floating_pane_count(), 0);
    assert_eq!(tab.pane_count(), 2); // now part of tiled layout
    assert!(tab.pane(fp_id).is_some());
}

// ---------------------------------------------------------------------------
// 8. Z-order management
// ---------------------------------------------------------------------------

#[test]
fn z_order_newly_created_pane_on_top() {
    // A newly created floating pane should have the highest z-index.
    let mut tab = Tab::new(0, "test", 80, 24);
    let _fp1 = tab.new_floating_pane();
    let fp2 = tab.new_floating_pane();
    let ids = tab.floating_pane_ids();
    assert_eq!(*ids.last().unwrap(), fp2);
}

#[test]
fn z_order_swap_two_floating_panes() {
    // Explicitly swapping the z-order of two floating panes should change
    // their render order.
    let mut tab = Tab::new(0, "test", 80, 24);
    let fp1 = tab.new_floating_pane();
    let fp2 = tab.new_floating_pane();
    let ids_before = tab.floating_pane_ids();
    assert_eq!(ids_before, vec![fp1, fp2]);
    tab.swap_floating_z_order(fp1, fp2);
    let ids_after = tab.floating_pane_ids();
    assert_eq!(ids_after, vec![fp2, fp1]);
}

#[test]
fn z_order_send_to_back() {
    // Sending a floating pane to back should give it the lowest z-index among
    // floating panes.
    let mut tab = Tab::new(0, "test", 80, 24);
    let fp1 = tab.new_floating_pane();
    let fp2 = tab.new_floating_pane();
    let fp3 = tab.new_floating_pane();
    tab.send_floating_to_back(fp3);
    let ids = tab.floating_pane_ids();
    assert_eq!(ids[0], fp3);
    assert_eq!(ids[1], fp1);
    assert_eq!(ids[2], fp2);
}
