//! Regression tests for edge cases that previously caused panics or incorrect behavior.

use emux_term::{Grid, Screen};
use emux_vt::Parser;

#[test]
fn regression_scroll_bottom_leq_top_no_panic() {
    // Grid::scroll_up/scroll_down should not panic when bottom <= top
    let mut grid = Grid::new(80, 24);
    grid.scroll_up(5, 3, 1); // bottom(3) < top(5) - should be no-op
    grid.scroll_down(10, 2, 1); // same
    // No panic = pass
}

#[test]
fn regression_backspace_at_origin() {
    let mut screen = Screen::new(80, 24);
    let mut parser = Parser::new();
    // Cursor at (0,0), send BS
    parser.advance(&mut screen, b"\x08");
    assert_eq!(screen.cursor.row, 0);
    assert_eq!(screen.cursor.col, 0);
}

#[test]
fn regression_1_row_terminal() {
    let mut screen = Screen::new(80, 1);
    let mut parser = Parser::new();
    parser.advance(&mut screen, b"Hello World");
    // Should not panic
    assert_eq!(screen.cursor.row, 0);
}

#[test]
fn regression_scroll_region_height_1() {
    let mut screen = Screen::new(80, 24);
    let mut parser = Parser::new();
    // Set scroll region to single row (should be ignored or handled gracefully)
    parser.advance(&mut screen, b"\x1b[5;5r");
    // Should not panic, scroll region should be rejected
}
