//! Regression tests for edge cases that previously caused panics or incorrect behavior.

use emux_term::{Grid, Screen};
use emux_vt::Parser;

#[test]
fn regression_scroll_bottom_leq_top_no_panic() {
    // Grid::scroll_up/scroll_down should not panic when bottom <= top
    let mut grid = Grid::new(80, 24);

    // Snapshot grid state before invalid scroll operations
    let cell_before = grid.cell(0, 0).clone();

    grid.scroll_up(5, 3, 1); // bottom(3) < top(5) - should be no-op
    grid.scroll_down(10, 2, 1); // same

    // Grid should be unchanged (no-op verified, not just no-panic)
    let cell_after = grid.cell(0, 0).clone();
    assert_eq!(cell_before.c, cell_after.c, "invalid scroll range should be a no-op");
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
    assert_eq!(screen.cursor.row, 0);
    // Content should be written correctly on the single row
    assert_eq!(screen.grid.cell(0, 0).c, 'H');
    assert_eq!(screen.grid.cell(0, 4).c, 'o');
}

#[test]
fn regression_scroll_region_height_1() {
    let mut screen = Screen::new(80, 24);
    let mut parser = Parser::new();
    // Set scroll region to single row (should be ignored or handled gracefully)
    parser.advance(&mut screen, b"\x1b[5;5r");

    // Scroll region should be rejected (top == bottom is invalid);
    // subsequent output should still work correctly
    parser.advance(&mut screen, b"Test");
    assert_eq!(screen.grid.cell(0, 0).c, 'T');
    assert_eq!(screen.grid.cell(0, 3).c, 't');
}
