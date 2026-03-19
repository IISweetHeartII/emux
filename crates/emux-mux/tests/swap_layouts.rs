//! TDD specs for swap layouts (auto-layout based on pane count).
//!
//! Swap layouts automatically rearrange panes when the pane count changes.
//! Users can also cycle through layouts manually. Layout definitions can come
//! from built-in presets or TOML configuration.

use emux_mux::Tab;
use emux_mux::layout::{LayoutNode, SplitDirection};

/// Helper: create a vertical split template for 2 panes (side by side).
fn side_by_side_template() -> LayoutNode {
    LayoutNode::Split {
        direction: SplitDirection::Vertical,
        ratio: 0.5,
        first: Box::new(LayoutNode::Leaf(100)),
        second: Box::new(LayoutNode::Leaf(101)),
    }
}

/// Helper: create a template for 3 panes — one full-width on top, two on bottom.
fn one_top_two_bottom_template() -> LayoutNode {
    LayoutNode::Split {
        direction: SplitDirection::Horizontal,
        ratio: 0.5,
        first: Box::new(LayoutNode::Leaf(100)),
        second: Box::new(LayoutNode::Split {
            direction: SplitDirection::Vertical,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf(101)),
            second: Box::new(LayoutNode::Leaf(102)),
        }),
    }
}

/// Helper: create a 2x2 grid template for 4 panes.
fn grid_2x2_template() -> LayoutNode {
    LayoutNode::Split {
        direction: SplitDirection::Horizontal,
        ratio: 0.5,
        first: Box::new(LayoutNode::Split {
            direction: SplitDirection::Vertical,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf(100)),
            second: Box::new(LayoutNode::Leaf(101)),
        }),
        second: Box::new(LayoutNode::Split {
            direction: SplitDirection::Vertical,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf(102)),
            second: Box::new(LayoutNode::Leaf(103)),
        }),
    }
}

// ---------------------------------------------------------------------------
// 1. Layout registration
// ---------------------------------------------------------------------------

#[test]
fn register_swap_layout_two_panes_side_by_side() {
    // Register a layout named "side-by-side" that arranges exactly 2 panes as
    // equal-width vertical splits.
    let mut tab = Tab::new(0, "test", 80, 24);
    tab.register_swap_layout("side-by-side", Some(2), Some(2), side_by_side_template());
    assert_eq!(tab.swap_layouts().len(), 1);
    assert_eq!(tab.swap_layouts()[0].name, "side-by-side");
    assert_eq!(tab.swap_layouts()[0].min_panes, Some(2));
    assert_eq!(tab.swap_layouts()[0].max_panes, Some(2));
}

#[test]
fn register_swap_layout_three_panes_one_top_two_bottom() {
    // Register a layout for 3 panes: one full-width pane on top, two
    // equal-width panes on the bottom row.
    let mut tab = Tab::new(0, "test", 80, 24);
    tab.register_swap_layout(
        "1-top-2-bottom",
        Some(3),
        Some(3),
        one_top_two_bottom_template(),
    );
    assert_eq!(tab.swap_layouts().len(), 1);
    assert_eq!(tab.swap_layouts()[0].name, "1-top-2-bottom");
}

#[test]
fn register_swap_layout_four_panes_grid() {
    // Register a 2x2 grid layout for exactly 4 panes.
    let mut tab = Tab::new(0, "test", 80, 24);
    tab.register_swap_layout("grid", Some(4), Some(4), grid_2x2_template());
    assert_eq!(tab.swap_layouts().len(), 1);
    assert_eq!(tab.swap_layouts()[0].name, "grid");
}

#[test]
fn register_duplicate_layout_replaces_previous() {
    // Registering a layout with the same pane-count key replaces the earlier
    // definition without error.
    let mut tab = Tab::new(0, "test", 80, 24);
    tab.register_swap_layout("side-by-side", Some(2), Some(2), side_by_side_template());
    assert_eq!(tab.swap_layouts().len(), 1);

    // Register again with same name but different range
    tab.register_swap_layout("side-by-side", Some(2), Some(3), side_by_side_template());
    assert_eq!(tab.swap_layouts().len(), 1);
    assert_eq!(tab.swap_layouts()[0].max_panes, Some(3));
}

// ---------------------------------------------------------------------------
// 2. Automatic layout swapping
// ---------------------------------------------------------------------------

#[test]
fn adding_pane_triggers_layout_swap() {
    // Starting with 1 pane and adding a second should automatically apply the
    // 2-pane swap layout if one is registered.
    let mut tab = Tab::new(0, "test", 80, 24);
    tab.register_swap_layout("side-by-side", Some(2), Some(2), side_by_side_template());

    // Split to get 2 panes
    tab.split_pane(SplitDirection::Vertical);
    assert_eq!(tab.pane_count(), 2);
    assert_eq!(tab.current_swap_layout_name(), Some("side-by-side"));

    // Verify the layout is a vertical split at the root
    let positions = tab.compute_positions();
    assert_eq!(positions.len(), 2);
    // Both panes should have the same number of rows (full height)
    assert_eq!(positions[0].1.rows, positions[1].1.rows);
    // Widths should split the 80 cols
    assert_eq!(positions[0].1.cols + positions[1].1.cols, 80);
}

#[test]
fn removing_pane_triggers_layout_swap_back() {
    // Going from 3 panes down to 2 should swap to the 2-pane layout.
    let mut tab = Tab::new(0, "test", 80, 24);
    tab.register_swap_layout("side-by-side", Some(2), Some(2), side_by_side_template());
    tab.register_swap_layout(
        "1-top-2-bottom",
        Some(3),
        Some(3),
        one_top_two_bottom_template(),
    );

    // Get to 3 panes
    tab.split_pane(SplitDirection::Vertical);
    tab.split_pane(SplitDirection::Horizontal);
    assert_eq!(tab.pane_count(), 3);
    assert_eq!(tab.current_swap_layout_name(), Some("1-top-2-bottom"));

    // Close one pane to go back to 2
    let ids = tab.pane_ids();
    let last_id = *ids.last().unwrap();
    tab.close_pane(last_id);
    assert_eq!(tab.pane_count(), 2);
    assert_eq!(tab.current_swap_layout_name(), Some("side-by-side"));
}

#[test]
fn no_registered_layout_falls_back_to_default_split() {
    // If no swap layout is registered for the current pane count, the system
    // should fall back to default equal-split behavior.
    let mut tab = Tab::new(0, "test", 80, 24);
    // Don't register any swap layouts

    tab.split_pane(SplitDirection::Vertical);
    assert_eq!(tab.pane_count(), 2);
    assert!(tab.current_swap_layout_name().is_none());

    // Layout should still work (default split behavior)
    let positions = tab.compute_positions();
    assert_eq!(positions.len(), 2);
}

#[test]
fn layout_swap_preserves_pane_order() {
    // When a layout swap occurs, panes should keep their logical order (the
    // first pane remains "pane 0", etc.) even though geometry changes.
    let mut tab = Tab::new(0, "test", 80, 24);
    tab.register_swap_layout("side-by-side", Some(2), Some(2), side_by_side_template());

    let first_pane = tab.pane_ids()[0];
    tab.split_pane(SplitDirection::Horizontal);
    let second_pane = tab.pane_ids()[1];

    // After swap layout is applied, pane order should be preserved
    let ids_after = tab.pane_ids();
    assert_eq!(ids_after[0], first_pane);
    assert_eq!(ids_after[1], second_pane);
}

// ---------------------------------------------------------------------------
// 3. Manual cycling
// ---------------------------------------------------------------------------

#[test]
fn manual_swap_next_layout() {
    // If multiple layouts are registered for the same pane count, cycling
    // "next" should advance to the next layout variant.
    let mut tab = Tab::new(0, "test", 80, 24);

    // Register two layouts both valid for 2 panes
    tab.register_swap_layout("side-by-side", Some(2), Some(2), side_by_side_template());
    let hsplit = LayoutNode::Split {
        direction: SplitDirection::Horizontal,
        ratio: 0.5,
        first: Box::new(LayoutNode::Leaf(100)),
        second: Box::new(LayoutNode::Leaf(101)),
    };
    tab.register_swap_layout("stacked", Some(2), Some(2), hsplit);

    tab.split_pane(SplitDirection::Vertical);
    assert_eq!(tab.pane_count(), 2);
    // Auto-swap picks the first matching
    assert_eq!(tab.current_swap_layout_name(), Some("side-by-side"));

    tab.next_swap_layout();
    assert_eq!(tab.current_swap_layout_name(), Some("stacked"));
}

#[test]
fn manual_swap_prev_layout() {
    // Cycling "prev" should go to the previous layout variant and wrap around
    // at the beginning.
    let mut tab = Tab::new(0, "test", 80, 24);

    tab.register_swap_layout("side-by-side", Some(2), Some(2), side_by_side_template());
    let hsplit = LayoutNode::Split {
        direction: SplitDirection::Horizontal,
        ratio: 0.5,
        first: Box::new(LayoutNode::Leaf(100)),
        second: Box::new(LayoutNode::Leaf(101)),
    };
    tab.register_swap_layout("stacked", Some(2), Some(2), hsplit);

    tab.split_pane(SplitDirection::Vertical);
    assert_eq!(tab.current_swap_layout_name(), Some("side-by-side"));

    // prev from the first should wrap to the last
    tab.prev_swap_layout();
    assert_eq!(tab.current_swap_layout_name(), Some("stacked"));
}

#[test]
fn manual_swap_wraps_around() {
    // After the last registered variant, "next" should wrap to the first.
    let mut tab = Tab::new(0, "test", 80, 24);

    tab.register_swap_layout("side-by-side", Some(2), Some(2), side_by_side_template());
    let hsplit = LayoutNode::Split {
        direction: SplitDirection::Horizontal,
        ratio: 0.5,
        first: Box::new(LayoutNode::Leaf(100)),
        second: Box::new(LayoutNode::Leaf(101)),
    };
    tab.register_swap_layout("stacked", Some(2), Some(2), hsplit);

    tab.split_pane(SplitDirection::Vertical);
    // Currently on "side-by-side" (index 0)
    tab.next_swap_layout(); // -> "stacked"
    tab.next_swap_layout(); // -> wraps to "side-by-side"
    assert_eq!(tab.current_swap_layout_name(), Some("side-by-side"));
}

// ---------------------------------------------------------------------------
// 4. Content preservation
// ---------------------------------------------------------------------------

#[test]
fn layout_swap_preserves_pane_content() {
    // The PTY output buffer and scrollback of each pane must survive a layout
    // swap unchanged.
    let mut tab = Tab::new(0, "test", 80, 24);
    tab.register_swap_layout("side-by-side", Some(2), Some(2), side_by_side_template());

    // Set title on the first pane as a proxy for "content"
    let first_id = tab.pane_ids()[0];
    tab.pane_mut(first_id).unwrap().set_title("my-shell");

    tab.split_pane(SplitDirection::Horizontal);
    let second_id = tab.pane_ids()[1];
    tab.pane_mut(second_id).unwrap().set_title("vim");

    // Layout swap was triggered; verify titles survived
    assert_eq!(tab.pane(first_id).unwrap().title(), "my-shell");
    assert_eq!(tab.pane(second_id).unwrap().title(), "vim");
}

#[test]
fn layout_swap_sends_resize_to_pty() {
    // After a layout swap, each pane's PTY should receive a SIGWINCH with the
    // new dimensions.
    let mut tab = Tab::new(0, "test", 80, 24);

    // Register a side-by-side layout for 2 panes
    tab.register_swap_layout("side-by-side", Some(2), Some(2), side_by_side_template());

    // Split horizontally first (default: stacked top/bottom)
    tab.split_pane(SplitDirection::Horizontal);
    assert_eq!(tab.pane_count(), 2);

    // The swap layout should have applied the vertical split template.
    // Check that pane sizes reflect the new vertical-split geometry.
    let positions = tab.compute_positions();
    assert_eq!(positions.len(), 2);
    // In a side-by-side (vertical) split of 80 cols, each pane gets ~40 cols
    // and full 24 rows.
    for (_, pos) in &positions {
        assert_eq!(pos.rows, 24);
    }
    assert_eq!(positions[0].1.cols + positions[1].1.cols, 80);

    // Also check the pane objects themselves were resized
    for (id, pos) in &positions {
        let pane = tab.pane(*id).unwrap();
        assert_eq!(pane.size().cols, pos.cols);
        assert_eq!(pane.size().rows, pos.rows);
    }
}

// ---------------------------------------------------------------------------
// 5. Configuration
// ---------------------------------------------------------------------------

#[test]
fn load_swap_layout_from_toml() {
    // A TOML config block like:
    //   [[swap_layout]]
    //   pane_count = 2
    //   template = "vsplit"
    // should register the corresponding layout at startup.
    let toml_str = r#"
[[swap_layout]]
name = "side-by-side"
pane_count = 2
template = "vsplit"
"#;
    let layouts = emux_mux::parse_swap_layout_toml(toml_str).unwrap();
    assert_eq!(layouts.len(), 1);
    assert_eq!(layouts[0].name, "side-by-side");
    assert_eq!(layouts[0].min_panes, Some(2));
    assert_eq!(layouts[0].max_panes, Some(2));

    // Register the parsed layout into a tab and verify it works
    let mut tab = Tab::new(0, "test", 80, 24);
    let sl = &layouts[0];
    tab.register_swap_layout(&sl.name, sl.min_panes, sl.max_panes, sl.layout.clone());
    assert_eq!(tab.swap_layouts().len(), 1);
}

#[test]
fn toml_layout_with_percentage_splits() {
    // TOML definitions should support percentage-based split ratios, e.g.,
    //   splits = ["30%", "70%"]
    let toml_str = r#"
[[swap_layout]]
name = "weighted"
pane_count = 2
direction = "vertical"
splits = ["30%", "70%"]
"#;
    let layouts = emux_mux::parse_swap_layout_toml(toml_str).unwrap();
    assert_eq!(layouts.len(), 1);
    assert_eq!(layouts[0].name, "weighted");

    // Register and apply: the ratio should be ~0.3
    let mut tab = Tab::new(0, "test", 100, 24);
    let sl = &layouts[0];
    tab.register_swap_layout(&sl.name, sl.min_panes, sl.max_panes, sl.layout.clone());
    tab.split_pane(SplitDirection::Vertical);
    assert_eq!(tab.pane_count(), 2);

    // The swap layout should have been applied with ~30/70 column split
    let positions = tab.compute_positions();
    assert_eq!(positions.len(), 2);
    let left_cols = positions[0].1.cols;
    let right_cols = positions[1].1.cols;
    assert_eq!(left_cols + right_cols, 100);
    // 30% of 100 = 30, allow some rounding tolerance
    assert!(
        left_cols >= 28 && left_cols <= 32,
        "left_cols was {left_cols}"
    );
}

#[test]
fn toml_layout_invalid_pane_count_is_error() {
    // A TOML layout with pane_count = 0 should produce a parse error.
    let toml_str = r#"
[[swap_layout]]
name = "bad"
pane_count = 0
direction = "vertical"
"#;
    let result = emux_mux::parse_swap_layout_toml(toml_str);
    assert!(result.is_err());
}
