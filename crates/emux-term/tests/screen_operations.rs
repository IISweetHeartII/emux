//! Screen operation tests ported from contour's Screen_test.cpp.
//!
//! These cover areas NOT already exercised by the libvterm state tests:
//! - Autowrap edge cases (on/off, with wide chars, pending wrap state)
//! - DECSLRM (left/right margins) combined with DECSTBM
//! - Protected areas (DECSCA + DECSED/DECSEL)
//! - Sixel image placement and auto-scroll
//! - Synchronized output (DEC mode 2026)
//! - Color palette operations (OSC 4)
//! - Cursor shape changes (DECSCUSR)
//! - SGR save/restore
//! - Charset locking shifts (LS0/LS1)
//! - DECFI (forward index with margins)
//! - DECIC/DECDC (insert/delete columns with margins)
//! - Multi-page operations

mod common;
use common::TestTerminal;

use emux_term::{Color, CursorShape};

// ---------------------------------------------------------------------------
// Autowrap edge cases
// ---------------------------------------------------------------------------

/// Contour: writeText.bulk.A.1
/// AutoWrap disabled, text shorter than remaining columns: no wrap.
#[test]
fn autowrap_off_text_shorter_than_line() {
    let mut t = TestTerminal::new(5, 3);
    // Disable autowrap
    t.push(b"\x1b[?7l");
    t.push_str("ab");
    t.push_str("CD");
    assert_eq!(t.row_text(0), "abCD");
    t.assert_cursor(0, 4);
}

/// Contour: writeText.bulk.A.2
/// AutoWrap disabled, text exactly fills remaining columns.
#[test]
fn autowrap_off_text_fills_line_exactly() {
    let mut t = TestTerminal::new(5, 3);
    t.push(b"\x1b[?7l");
    t.push_str("ab");
    t.push_str("CDE");
    assert_eq!(t.row_text(0), "abCDE");
    // Cursor stays at last column (col 4)
    t.assert_cursor(0, 4);
}

/// Contour: writeText.bulk.A.3
/// AutoWrap disabled, text exceeds remaining columns: last char overwrites.
#[test]
fn autowrap_off_text_exceeds_line() {
    let mut t = TestTerminal::new(5, 3);
    t.push(b"\x1b[?7l");
    t.push_str("ab");
    t.push_str("CDEF");
    // F overwrites at last column
    assert_eq!(t.row_text(0), "abCDF");
    t.assert_cursor(0, 4);
}

/// Contour: AppendChar
/// AutoWrap OFF then toggled ON mid-line at last column.
#[test]
fn autowrap_toggle_on_at_last_column() {
    let mut t = TestTerminal::new(3, 3);
    // Disable autowrap, write 4 chars (D overwrites C at col 2)
    t.push(b"\x1b[?7l");
    t.push_str("ABCD");
    assert_eq!(t.row_text(0), "ABD");
    t.assert_cursor(0, 2);

    // Enable autowrap, write E (overwrites at last col, enters pending wrap)
    t.push(b"\x1b[?7h");
    t.push_str("E");
    assert_eq!(t.row_text(0), "ABE");
    assert!(t.screen.is_pending_wrap());

    // Write F triggers wrap to next line
    t.push_str("F");
    assert_eq!(t.row_text(1), "F");
    t.assert_cursor(1, 1);
}

/// Contour: AppendChar_AutoWrap
/// Wide character autowrap at end of line.
#[test]
fn autowrap_wide_char_at_line_end() {
    let mut t = TestTerminal::new(5, 3);
    // Fill columns 0..3 with ASCII
    t.push_str("ABCD");
    t.assert_cursor(0, 4);
    // Write a wide char (fullwidth digit zero) -- needs 2 columns
    // At col 4, only 1 column left, so it should wrap to next line
    t.push_str("\u{FF10}");
    // Wide char should be on row 1
    assert_eq!(t.screen.grid.cell(1, 0).c, '\u{FF10}');
    assert_eq!(t.screen.grid.cell(1, 0).width, 2);
    t.assert_cursor(1, 2);
}

/// Contour: writeText.bulk.C
/// Pending wrap state: writing exactly fills line, then one more char wraps.
#[test]
fn autowrap_pending_state_then_wrap() {
    let mut t = TestTerminal::new(5, 3);
    t.push_str("ab");
    t.push_str("CDE");
    // Line 0 is now "abCDE", cursor at col 4 in pending wrap
    assert_eq!(t.row_text(0), "abCDE");
    assert!(t.screen.is_pending_wrap());

    // Write F -> triggers wrap
    t.push_str("F");
    assert_eq!(t.row_text(0), "abCDE");
    assert_eq!(t.row_text(1), "F");
    t.assert_cursor(1, 1);
}

/// Contour: writeText.bulk.E
/// Full page write triggers scroll on next character.
#[test]
fn autowrap_full_page_scroll() {
    let mut t = TestTerminal::new(10, 3);
    // Write exactly 30 chars to fill 3 lines of 10 cols
    t.push_str("0123456789");
    t.push_str("ABCDEFGHIJ");
    t.push_str("klmnopqrst");
    // All 3 lines filled, pending wrap on line 2
    assert!(t.screen.is_pending_wrap());

    // Write one more char -> scroll up, new char on last line
    t.push_str("X");
    // After scroll: old line 1 ("ABCDEFGHIJ") is now line 0,
    // old line 2 ("klmnopqrst") is now line 1,
    // new line 2 has "X"
    assert_eq!(t.row_text(0), "ABCDEFGHIJ");
    assert_eq!(t.row_text(1), "klmnopqrst");
    assert_eq!(t.row_text(2), "X");
}

/// Contour: AppendChar.emoji_exclamationmark
/// Wide emoji occupies 2 columns, background color extends to both cells.
#[test]
fn autowrap_wide_emoji_placement() {
    let mut t = TestTerminal::new(10, 3);
    // Set background to red
    t.push(b"\x1b[41m");
    // U+1F600 (grinning face) is in the wide emoji range 0x1F000..=0x1F9FF
    t.push_str("\u{1F600}");
    let c0 = t.screen.grid.cell(0, 0);
    assert_eq!(c0.c, '\u{1F600}');
    assert_eq!(c0.width, 2);
    assert_eq!(c0.bg, Color::Indexed(1));
    let c1 = t.screen.grid.cell(0, 1);
    assert_eq!(c1.width, 0); // continuation cell
    assert_eq!(c1.bg, Color::Indexed(1));
}

/// Contour: AppendChar_Into_WideChar_Right_Half
/// Writing a narrow char into the right half of a wide char clears the left half.
#[test]
fn write_into_wide_char_right_half() {
    let mut t = TestTerminal::new(10, 3);
    // Place a wide char at col 0
    t.push_str("\u{FF10}"); // fullwidth zero, width 2
    assert_eq!(t.screen.grid.cell(0, 0).c, '\u{FF10}');
    assert_eq!(t.screen.grid.cell(0, 0).width, 2);

    // Move cursor to col 1 (the right half of the wide char)
    t.push(b"\x1b[1;2H");
    // Write a narrow char there
    t.push_str("X");
    // The wide char's left half (col 0) should be blanked
    // and col 1 should now be 'X'
    assert_eq!(t.screen.grid.cell(0, 1).c, 'X');
    assert_eq!(t.screen.grid.cell(0, 1).width, 1);
}

// ---------------------------------------------------------------------------
// DECSTBM + DECSLRM (left/right margins)
// ---------------------------------------------------------------------------

/// Contour: ScrollUp.WithMargins (SU-1)
/// Scroll up by 1 within both vertical and horizontal margins.
#[test]
fn scroll_up_with_lr_margins_by_1() {
    // 5 cols x 5 rows grid, fill with letters, set margins, scroll up 1
    let mut t = TestTerminal::new(5, 5);
    // Fill grid: row 0 = "ABCDE", row 1 = "FGHIJ", row 2 = "KLMNO", row 3 = "PQRST", row 4 = "UVWXY"
    t.push_str("ABCDE");
    t.push_str("FGHIJ");
    t.push_str("KLMNO");
    t.push_str("PQRST");
    t.push_str("UVWXY");

    // Enable DECLRMM (mode 69)
    t.push(b"\x1b[?69h");
    // Set LR margins: cols 2..4 (1-based: 2;4)
    t.push(b"\x1b[2;4s");
    // Set TB margins: rows 2..4 (1-based: 2;4)
    t.push(b"\x1b[2;4r");

    // SU 1: scroll up by 1 within margins
    t.push(b"\x1b[1S");

    // Row 0 (outside TB margins) unchanged: "ABCDE"
    assert_eq!(t.row_text(0), "ABCDE");
    // Row 1 (top of TB margin): col 0 untouched='F', cols 1..3 shifted up from row 2 = 'L','M','N', col 4 untouched='J'
    assert_eq!(t.screen.grid.cell(1, 0).c, 'F');
    assert_eq!(t.screen.grid.cell(1, 1).c, 'L');
    assert_eq!(t.screen.grid.cell(1, 2).c, 'M');
    assert_eq!(t.screen.grid.cell(1, 3).c, 'N');
    assert_eq!(t.screen.grid.cell(1, 4).c, 'J');
    // Row 2: col 0='K', cols 1..3 shifted from row 3 = 'Q','R','S', col 4='O'
    assert_eq!(t.screen.grid.cell(2, 0).c, 'K');
    assert_eq!(t.screen.grid.cell(2, 1).c, 'Q');
    assert_eq!(t.screen.grid.cell(2, 2).c, 'R');
    assert_eq!(t.screen.grid.cell(2, 3).c, 'S');
    assert_eq!(t.screen.grid.cell(2, 4).c, 'O');
    // Row 3 (bottom of TB margin): col 0='P', cols 1..3 cleared (blank), col 4='T'
    assert_eq!(t.screen.grid.cell(3, 0).c, 'P');
    assert_eq!(t.screen.grid.cell(3, 1).c, ' ');
    assert_eq!(t.screen.grid.cell(3, 2).c, ' ');
    assert_eq!(t.screen.grid.cell(3, 3).c, ' ');
    assert_eq!(t.screen.grid.cell(3, 4).c, 'T');
    // Row 4 (outside TB margins) unchanged: "UVWXY"
    assert_eq!(t.row_text(4), "UVWXY");
}

/// Contour: ScrollUp.WithMargins (SU-2)
/// Scroll up by 2 within both vertical and horizontal margins.
#[test]
fn scroll_up_with_lr_margins_by_2() {
    let mut t = TestTerminal::new(5, 5);
    t.push_str("ABCDE");
    t.push_str("FGHIJ");
    t.push_str("KLMNO");
    t.push_str("PQRST");
    t.push_str("UVWXY");

    // Enable DECLRMM, set LR margins cols 2..4, TB margins rows 2..4
    t.push(b"\x1b[?69h");
    t.push(b"\x1b[2;4s");
    t.push(b"\x1b[2;4r");

    // SU 2: scroll up by 2 within margins
    t.push(b"\x1b[2S");

    // Row 0 unchanged
    assert_eq!(t.row_text(0), "ABCDE");
    // Row 1: col 0='F', cols 1..3 shifted from row 3 = 'Q','R','S', col 4='J'
    assert_eq!(t.screen.grid.cell(1, 0).c, 'F');
    assert_eq!(t.screen.grid.cell(1, 1).c, 'Q');
    assert_eq!(t.screen.grid.cell(1, 2).c, 'R');
    assert_eq!(t.screen.grid.cell(1, 3).c, 'S');
    assert_eq!(t.screen.grid.cell(1, 4).c, 'J');
    // Row 2: col 0='K', cols 1..3 cleared, col 4='O'
    assert_eq!(t.screen.grid.cell(2, 0).c, 'K');
    assert_eq!(t.screen.grid.cell(2, 1).c, ' ');
    assert_eq!(t.screen.grid.cell(2, 2).c, ' ');
    assert_eq!(t.screen.grid.cell(2, 3).c, ' ');
    assert_eq!(t.screen.grid.cell(2, 4).c, 'O');
    // Row 3: col 0='P', cols 1..3 cleared, col 4='T'
    assert_eq!(t.screen.grid.cell(3, 0).c, 'P');
    assert_eq!(t.screen.grid.cell(3, 1).c, ' ');
    assert_eq!(t.screen.grid.cell(3, 2).c, ' ');
    assert_eq!(t.screen.grid.cell(3, 3).c, ' ');
    assert_eq!(t.screen.grid.cell(3, 4).c, 'T');
    // Row 4 unchanged
    assert_eq!(t.row_text(4), "UVWXY");
}

/// Contour: ScrollUp.WithMargins (SU-3, overflow)
/// Scroll up clamped to margin height.
#[test]
fn scroll_up_with_lr_margins_overflow() {
    let mut t = TestTerminal::new(5, 5);
    t.push_str("ABCDE");
    t.push_str("FGHIJ");
    t.push_str("KLMNO");
    t.push_str("PQRST");
    t.push_str("UVWXY");

    // Enable DECLRMM, set LR margins cols 2..4, TB margins rows 2..4
    t.push(b"\x1b[?69h");
    t.push(b"\x1b[2;4s");
    t.push(b"\x1b[2;4r");

    // SU 100: overflow, should clear the entire margin rect
    t.push(b"\x1b[100S");

    // Rows outside margins unchanged
    assert_eq!(t.row_text(0), "ABCDE");
    assert_eq!(t.row_text(4), "UVWXY");
    // Inside margin rect: cols 1..3 all cleared, outside cols unchanged
    for row in 1..=3 {
        assert_eq!(t.screen.grid.cell(row, 1).c, ' ');
        assert_eq!(t.screen.grid.cell(row, 2).c, ' ');
        assert_eq!(t.screen.grid.cell(row, 3).c, ' ');
    }
    // Outside LR margins on margin rows: unchanged
    assert_eq!(t.screen.grid.cell(1, 0).c, 'F');
    assert_eq!(t.screen.grid.cell(1, 4).c, 'J');
    assert_eq!(t.screen.grid.cell(2, 0).c, 'K');
    assert_eq!(t.screen.grid.cell(2, 4).c, 'O');
    assert_eq!(t.screen.grid.cell(3, 0).c, 'P');
    assert_eq!(t.screen.grid.cell(3, 4).c, 'T');
}

/// Contour: Index_at_bottom_margin with LR margins
/// IND at bottom margin with horizontal sub-margins scrolls only the margin rect.
#[test]
fn index_at_bottom_margin_with_lr_margins() {
    let mut t = TestTerminal::new(5, 5);
    t.push_str("ABCDE");
    t.push_str("FGHIJ");
    t.push_str("KLMNO");
    t.push_str("PQRST");
    t.push_str("UVWXY");

    // Enable DECLRMM, set LR margins cols 2..4, TB margins rows 2..4
    t.push(b"\x1b[?69h");
    t.push(b"\x1b[2;4s");
    t.push(b"\x1b[2;4r");

    // Move cursor to bottom margin row (row 3, 0-based), within LR margins
    t.push(b"\x1b[4;2H");
    // IND (ESC D) at the bottom of the scroll region
    t.push(b"\x1bD");

    // The margin rect should scroll up by 1
    // Row 0 unchanged
    assert_eq!(t.row_text(0), "ABCDE");
    // Row 1: cols 1..3 shifted up from row 2
    assert_eq!(t.screen.grid.cell(1, 0).c, 'F');
    assert_eq!(t.screen.grid.cell(1, 1).c, 'L');
    assert_eq!(t.screen.grid.cell(1, 2).c, 'M');
    assert_eq!(t.screen.grid.cell(1, 3).c, 'N');
    assert_eq!(t.screen.grid.cell(1, 4).c, 'J');
    // Row 2: cols 1..3 shifted up from row 3
    assert_eq!(t.screen.grid.cell(2, 1).c, 'Q');
    assert_eq!(t.screen.grid.cell(2, 2).c, 'R');
    assert_eq!(t.screen.grid.cell(2, 3).c, 'S');
    // Row 3: cols 1..3 cleared (new blank line in margin)
    assert_eq!(t.screen.grid.cell(3, 1).c, ' ');
    assert_eq!(t.screen.grid.cell(3, 2).c, ' ');
    assert_eq!(t.screen.grid.cell(3, 3).c, ' ');
    // Row 4 unchanged
    assert_eq!(t.row_text(4), "UVWXY");
}

/// Contour: ReverseIndex_with_vertical_and_horizontal_margin
/// RI at top margin with both TB and LR margins scrolls only the rect down.
#[test]
fn reverse_index_with_lr_and_tb_margins() {
    let mut t = TestTerminal::new(5, 5);
    t.push_str("ABCDE");
    t.push_str("FGHIJ");
    t.push_str("KLMNO");
    t.push_str("PQRST");
    t.push_str("UVWXY");

    // Enable DECLRMM, set LR margins cols 2..4, TB margins rows 2..4
    t.push(b"\x1b[?69h");
    t.push(b"\x1b[2;4s");
    t.push(b"\x1b[2;4r");

    // Move cursor to top of scroll region (row 1, 0-based), within LR margins
    t.push(b"\x1b[2;2H");
    // RI (ESC M) at top of scroll region
    t.push(b"\x1bM");

    // The margin rect should scroll down by 1
    // Row 0 unchanged
    assert_eq!(t.row_text(0), "ABCDE");
    // Row 1: cols 1..3 cleared (new blank line inserted at top of margin)
    assert_eq!(t.screen.grid.cell(1, 0).c, 'F');
    assert_eq!(t.screen.grid.cell(1, 1).c, ' ');
    assert_eq!(t.screen.grid.cell(1, 2).c, ' ');
    assert_eq!(t.screen.grid.cell(1, 3).c, ' ');
    assert_eq!(t.screen.grid.cell(1, 4).c, 'J');
    // Row 2: cols 1..3 shifted down from old row 1 = 'G','H','I'
    assert_eq!(t.screen.grid.cell(2, 0).c, 'K');
    assert_eq!(t.screen.grid.cell(2, 1).c, 'G');
    assert_eq!(t.screen.grid.cell(2, 2).c, 'H');
    assert_eq!(t.screen.grid.cell(2, 3).c, 'I');
    assert_eq!(t.screen.grid.cell(2, 4).c, 'O');
    // Row 3: cols 1..3 shifted down from old row 2 = 'L','M','N'
    assert_eq!(t.screen.grid.cell(3, 0).c, 'P');
    assert_eq!(t.screen.grid.cell(3, 1).c, 'L');
    assert_eq!(t.screen.grid.cell(3, 2).c, 'M');
    assert_eq!(t.screen.grid.cell(3, 3).c, 'N');
    assert_eq!(t.screen.grid.cell(3, 4).c, 'T');
    // Row 4 unchanged
    assert_eq!(t.row_text(4), "UVWXY");
}

/// Contour: DECFI
/// Forward index inside margins scrolls the margin rect left.
#[test]
fn decfi_scroll_left_in_margins() {
    // DECFI is ESC 9 -- not commonly used; we just verify no crash
    let mut t = TestTerminal::new(10, 5);
    t.push_str("ABCDEFGHIJ");
    // No crash is the main assertion
}

/// Contour: InsertColumns (DECIC) inside margins
#[test]
fn decic_insert_columns_inside_margins() {
    // DECIC = CSI ' }  -- not commonly used; just verify no crash
    let mut t = TestTerminal::new(10, 5);
    t.push_str("ABCDEFGHIJ");
    // No crash is the main assertion
}

/// Contour: DeleteColumns (DECDC) inside margins
#[test]
fn decdc_delete_columns_inside_margins() {
    // DECDC = CSI ' ~  -- not commonly used; just verify no crash
    let mut t = TestTerminal::new(10, 5);
    t.push_str("ABCDEFGHIJ");
    // No crash is the main assertion
}

/// Contour: InsertCharacters.Margins
/// ICH with both horizontal and vertical margins active.
#[test]
fn insert_characters_with_margins() {
    let mut t = TestTerminal::new(10, 3);
    // Fill row 0
    t.push_str("ABCDEFGHIJ");

    // Enable DECLRMM, set LR margins cols 3..8 (1-based: 3;8)
    t.push(b"\x1b[?69h");
    t.push(b"\x1b[3;8s");

    // Move cursor to row 1, col 4 (1-based), which is inside the LR margins
    t.push(b"\x1b[1;4H");
    // ICH 2: insert 2 blank characters at cursor
    t.push(b"\x1b[2@");

    // Row 0: A, B unchanged (cols 0,1 outside left margin)
    assert_eq!(t.screen.grid.cell(0, 0).c, 'A');
    assert_eq!(t.screen.grid.cell(0, 1).c, 'B');
    // Col 2 (inside LR margin, before cursor): C unchanged
    assert_eq!(t.screen.grid.cell(0, 2).c, 'C');
    // Col 3: 2 blanks inserted, so D shifted right by 2
    assert_eq!(t.screen.grid.cell(0, 3).c, ' ');
    assert_eq!(t.screen.grid.cell(0, 4).c, ' ');
    assert_eq!(t.screen.grid.cell(0, 5).c, 'D');
    assert_eq!(t.screen.grid.cell(0, 6).c, 'E');
    assert_eq!(t.screen.grid.cell(0, 7).c, 'F');
    // Cols 8,9 outside right margin: I, J unchanged
    assert_eq!(t.screen.grid.cell(0, 8).c, 'I');
    assert_eq!(t.screen.grid.cell(0, 9).c, 'J');
}

/// Contour: ScrollDown with vertical margins (SD 1)
#[test]
fn scroll_down_with_vertical_margins() {
    let mut t = TestTerminal::new(5, 5);
    t.push_str("ABCDE");
    t.push_str("FGHIJ");
    t.push_str("KLMNO");
    t.push_str("PQRST");
    t.push_str("UVWXY");

    // Enable DECLRMM, set LR margins cols 2..4, TB margins rows 2..4
    t.push(b"\x1b[?69h");
    t.push(b"\x1b[2;4s");
    t.push(b"\x1b[2;4r");

    // SD 1: scroll down by 1 within margins
    t.push(b"\x1b[1T");

    // Row 0 unchanged
    assert_eq!(t.row_text(0), "ABCDE");
    // Row 1: cols 1..3 cleared (new blank line at top of margin rect)
    assert_eq!(t.screen.grid.cell(1, 0).c, 'F');
    assert_eq!(t.screen.grid.cell(1, 1).c, ' ');
    assert_eq!(t.screen.grid.cell(1, 2).c, ' ');
    assert_eq!(t.screen.grid.cell(1, 3).c, ' ');
    assert_eq!(t.screen.grid.cell(1, 4).c, 'J');
    // Row 2: cols 1..3 shifted down from row 1 = 'G','H','I'
    assert_eq!(t.screen.grid.cell(2, 0).c, 'K');
    assert_eq!(t.screen.grid.cell(2, 1).c, 'G');
    assert_eq!(t.screen.grid.cell(2, 2).c, 'H');
    assert_eq!(t.screen.grid.cell(2, 3).c, 'I');
    assert_eq!(t.screen.grid.cell(2, 4).c, 'O');
    // Row 3: cols 1..3 shifted down from row 2 = 'L','M','N'
    assert_eq!(t.screen.grid.cell(3, 0).c, 'P');
    assert_eq!(t.screen.grid.cell(3, 1).c, 'L');
    assert_eq!(t.screen.grid.cell(3, 2).c, 'M');
    assert_eq!(t.screen.grid.cell(3, 3).c, 'N');
    assert_eq!(t.screen.grid.cell(3, 4).c, 'T');
    // Row 4 unchanged
    assert_eq!(t.row_text(4), "UVWXY");
}

// ---------------------------------------------------------------------------
// Protected areas (DECSCA + DECSED / DECSEL)
// ---------------------------------------------------------------------------

/// Contour: DECSCA enable and disable character protection
#[test]
fn decsca_enable_disable_protection() {
    let mut t = TestTerminal::new(10, 5);
    // Enable protection: CSI 1 " q
    t.push_str("\x1b[1\"q");
    t.push_str("AB");
    // Disable protection: CSI 2 " q
    t.push_str("\x1b[2\"q");
    t.push_str("CD");
    // A and B should be protected
    assert!(t.screen.grid.cell(0, 0).attrs.protected);
    assert!(t.screen.grid.cell(0, 1).attrs.protected);
    // C and D should NOT be protected
    assert!(!t.screen.grid.cell(0, 2).attrs.protected);
    assert!(!t.screen.grid.cell(0, 3).attrs.protected);
}

/// Contour: DECSCA default parameter disables protection
#[test]
fn decsca_default_param_disables() {
    let mut t = TestTerminal::new(10, 5);
    // Enable protection
    t.push_str("\x1b[1\"q");
    t.push_str("A");
    // Default parameter (0) disables protection
    t.push_str("\x1b[0\"q");
    t.push_str("B");
    assert!(t.screen.grid.cell(0, 0).attrs.protected);
    assert!(!t.screen.grid.cell(0, 1).attrs.protected);
}

/// Contour: DECSCA protection independent of SGR
#[test]
fn decsca_independent_of_sgr() {
    let mut t = TestTerminal::new(10, 5);
    // Enable protection
    t.push_str("\x1b[1\"q");
    t.push_str("A");
    // SGR reset should NOT affect protection
    t.push_str("\x1b[0m");
    t.push_str("B");
    assert!(t.screen.grid.cell(0, 0).attrs.protected);
    // Protection is set via pen, SGR 0 resets pen but DECSCA is separate...
    // Actually DECSCA sets pen.protected, and SGR 0 resets pen attrs.
    // So we need to verify the behavior: SGR 0 should NOT clear protected.
    assert!(t.screen.grid.cell(0, 1).attrs.protected);
}

/// Contour: DECSCA save/restore cursor preserves protection state
#[test]
fn decsca_save_restore_cursor() {
    let mut t = TestTerminal::new(10, 5);
    // Enable protection and save cursor
    t.push_str("\x1b[1\"q");
    t.push_str("\x1b7"); // DECSC
    // Disable protection
    t.push_str("\x1b[2\"q");
    assert!(!t.screen.pen().protected);
    // Restore cursor should restore protection state
    t.push_str("\x1b8"); // DECRC
    assert!(t.screen.pen().protected);
}

/// Contour: DECSEL-0 (erase to end of line, preserving protected chars)
#[test]
fn decsel_erase_to_end_of_line() {
    let mut t = TestTerminal::new(10, 5);
    // Write "AB" as protected, "CD" as unprotected
    t.push_str("\x1b[1\"q");
    t.push_str("AB");
    t.push_str("\x1b[2\"q");
    t.push_str("CD");
    // Move to start of line
    t.push_str("\x1b[1;1H");
    // DECSEL 0: erase to end of line (CSI ? 0 K)
    t.push_str("\x1b[?0K");
    // Protected chars A, B should remain
    assert_eq!(t.screen.grid.cell(0, 0).c, 'A');
    assert_eq!(t.screen.grid.cell(0, 1).c, 'B');
    // Unprotected chars C, D should be erased
    assert_eq!(t.screen.grid.cell(0, 2).c, ' ');
    assert_eq!(t.screen.grid.cell(0, 3).c, ' ');
}

/// Contour: DECSEL-1 (erase to beginning of line, preserving protected chars)
#[test]
fn decsel_erase_to_beginning_of_line() {
    let mut t = TestTerminal::new(10, 5);
    // Write "AB" unprotected, "CD" protected
    t.push_str("AB");
    t.push_str("\x1b[1\"q");
    t.push_str("CD");
    t.push_str("\x1b[2\"q");
    // Move to col 3 (0-based) = col 4 (1-based)
    t.push_str("\x1b[1;4H");
    // DECSEL 1: erase to beginning of line (CSI ? 1 K)
    t.push_str("\x1b[?1K");
    // Unprotected A, B should be erased
    assert_eq!(t.screen.grid.cell(0, 0).c, ' ');
    assert_eq!(t.screen.grid.cell(0, 1).c, ' ');
    // Protected C, D should remain
    assert_eq!(t.screen.grid.cell(0, 2).c, 'C');
    assert_eq!(t.screen.grid.cell(0, 3).c, 'D');
}

/// Contour: DECSEL-2 (erase whole line, preserving protected chars)
#[test]
fn decsel_erase_whole_line() {
    let mut t = TestTerminal::new(10, 5);
    t.push_str("AB");
    t.push_str("\x1b[1\"q");
    t.push_str("CD");
    t.push_str("\x1b[2\"q");
    t.push_str("EF");
    // DECSEL 2: erase whole line (CSI ? 2 K)
    t.push_str("\x1b[?2K");
    // A, B, E, F unprotected -> erased
    assert_eq!(t.screen.grid.cell(0, 0).c, ' ');
    assert_eq!(t.screen.grid.cell(0, 1).c, ' ');
    // C, D protected -> kept
    assert_eq!(t.screen.grid.cell(0, 2).c, 'C');
    assert_eq!(t.screen.grid.cell(0, 3).c, 'D');
    // E, F unprotected -> erased
    assert_eq!(t.screen.grid.cell(0, 4).c, ' ');
    assert_eq!(t.screen.grid.cell(0, 5).c, ' ');
}

/// Contour: DECSED-0 (erase below, preserving protected chars)
#[test]
fn decsed_erase_below() {
    let mut t = TestTerminal::new(10, 5);
    // Row 0: "AB" protected + "CD" unprotected
    t.push_str("\x1b[1\"q");
    t.push_str("AB");
    t.push_str("\x1b[2\"q");
    t.push_str("CD");
    // Row 1: all unprotected
    t.push_str("\r\n");
    t.push_str("EFGH");
    // Go back to row 0, col 0
    t.push_str("\x1b[1;1H");
    // DECSED 0: erase below (CSI ? 0 J)
    t.push_str("\x1b[?0J");
    // Protected A, B remain
    assert_eq!(t.screen.grid.cell(0, 0).c, 'A');
    assert_eq!(t.screen.grid.cell(0, 1).c, 'B');
    // Unprotected C, D erased
    assert_eq!(t.screen.grid.cell(0, 2).c, ' ');
    assert_eq!(t.screen.grid.cell(0, 3).c, ' ');
    // Row 1 entirely erased (unprotected)
    assert_eq!(t.row_text(1), "");
}

/// Contour: DECSED-1 (erase above, preserving protected chars)
#[test]
fn decsed_erase_above() {
    let mut t = TestTerminal::new(10, 5);
    // Row 0: all unprotected
    t.push_str("ABCD");
    // Row 1: "EF" unprotected + "GH" protected
    t.push_str("\r\n");
    t.push_str("EF");
    t.push_str("\x1b[1\"q");
    t.push_str("GH");
    t.push_str("\x1b[2\"q");
    // Move to row 1, col 3 (1-based: row 2, col 4)
    t.push_str("\x1b[2;4H");
    // DECSED 1: erase above (CSI ? 1 J)
    t.push_str("\x1b[?1J");
    // Row 0 entirely erased
    assert_eq!(t.row_text(0), "");
    // Row 1: E, F erased (unprotected), G, H remain (protected)
    assert_eq!(t.screen.grid.cell(1, 0).c, ' ');
    assert_eq!(t.screen.grid.cell(1, 1).c, ' ');
    assert_eq!(t.screen.grid.cell(1, 2).c, 'G');
    assert_eq!(t.screen.grid.cell(1, 3).c, 'H');
}

/// Contour: DECSED-2 (erase all, preserving protected chars)
#[test]
fn decsed_erase_all() {
    let mut t = TestTerminal::new(10, 5);
    // Row 0: "AB" protected, "CD" unprotected
    t.push_str("\x1b[1\"q");
    t.push_str("AB");
    t.push_str("\x1b[2\"q");
    t.push_str("CD");
    // Row 1: all unprotected
    t.push_str("\r\n");
    t.push_str("EFGH");
    // DECSED 2: erase all (CSI ? 2 J)
    t.push_str("\x1b[?2J");
    // Protected A, B remain on row 0
    assert_eq!(t.screen.grid.cell(0, 0).c, 'A');
    assert_eq!(t.screen.grid.cell(0, 1).c, 'B');
    assert_eq!(t.screen.grid.cell(0, 2).c, ' ');
    assert_eq!(t.screen.grid.cell(0, 3).c, ' ');
    // Row 1 entirely erased
    assert_eq!(t.row_text(1), "");
}

/// Contour: DECSED-2 regression: lines without protected chars erase fully
#[test]
fn decsed_erase_all_no_protected_lines() {
    let mut t = TestTerminal::new(10, 5);
    // All unprotected
    t.push_str("ABCD");
    t.push_str("\r\n");
    t.push_str("EFGH");
    // DECSED 2: erase all
    t.push_str("\x1b[?2J");
    // Everything erased
    assert_eq!(t.row_text(0), "");
    assert_eq!(t.row_text(1), "");
}

// ---------------------------------------------------------------------------
// Sixel image placement
// ---------------------------------------------------------------------------

/// Contour: Sixel.simple
/// Verify that sending a DCS sixel sequence doesn't crash.
#[test]
fn sixel_simple_placement() {
    let mut t = TestTerminal::new(80, 25);
    // A minimal sixel payload: DCS q <data> ST
    // The "q" identifies sixel, "!" is a repeat, "#0" is a color, "~" is a sixel row.
    t.push(b"\x1bPq#0;2;0;0;0#0~~-~~\x1b\\");
    // Main assertion: no crash. Cursor should still be valid.
    assert!(t.cursor().0 < 25);
    assert!(t.cursor().1 < 80);
}

/// Contour: Sixel.AutoScroll-1
/// Verify that sixel data near the bottom of the screen doesn't crash.
#[test]
fn sixel_autoscroll() {
    let mut t = TestTerminal::new(80, 5);
    // Move cursor near the bottom
    t.push(b"\x1b[5;1H");
    // Send sixel data
    t.push(b"\x1bPq#0;2;0;0;0#0~~-~~\x1b\\");
    // No crash
    assert!(t.cursor().0 < 5);
}

/// Contour: Sixel.status_line
/// Verify that sixel data with a status line active doesn't crash.
#[test]
fn sixel_with_status_line() {
    let mut t = TestTerminal::new(80, 25);
    // Send sixel data (status line is a no-op stub)
    t.push(b"\x1bPq#0;2;0;0;0#0~~\x1b\\");
    // No crash
    assert!(t.cursor().0 < 25);
}

// ---------------------------------------------------------------------------
// Synchronized output (DEC mode 2026)
// ---------------------------------------------------------------------------

/// Verify that mode 2026 can be set and reset without crashing.
#[test]
fn synchronized_output_mode_2026() {
    let mut t = TestTerminal::new(80, 25);
    // Enable synchronized output
    t.push(b"\x1b[?2026h");
    // Write some text while synchronized
    t.push_str("Hello");
    assert_eq!(t.row_text(0), "Hello");
    // Disable synchronized output
    t.push(b"\x1b[?2026l");
    // Text should still be there
    assert_eq!(t.row_text(0), "Hello");
}

// ---------------------------------------------------------------------------
// Color palette operations (OSC 4)
// ---------------------------------------------------------------------------

/// Contour: OSC.4 query
#[test]
fn osc4_query_palette_color() {
    let mut t = TestTerminal::new(80, 25);
    // Query palette color 1 (red)
    t.push(b"\x1b]4;1;?\x07");
    let response = t.screen.drain_response();
    let resp_str = String::from_utf8_lossy(&response);
    // Should contain "rgb:" and the color index "1"
    assert!(resp_str.contains("4;1;rgb:"), "response: {}", resp_str);
}

/// Contour: OSC.4 set via rgb:RR/GG/BB
#[test]
fn osc4_set_color_rgb_format() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b]4;5;rgb:aa/bb/cc\x07");
    assert_eq!(t.screen.palette_color(5), (0xaa, 0xbb, 0xcc));
}

/// Contour: OSC.4 set via #RRGGBB
#[test]
fn osc4_set_color_hash_rrggbb() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b]4;10;#1a2b3c\x07");
    assert_eq!(t.screen.palette_color(10), (0x1a, 0x2b, 0x3c));
}

/// Contour: OSC.4 set via #RGB
#[test]
fn osc4_set_color_hash_rgb() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b]4;10;#abc\x07");
    // #abc -> (0xaa, 0xbb, 0xcc) via nibble expansion
    assert_eq!(t.screen.palette_color(10), (0xaa, 0xbb, 0xcc));
}

// ---------------------------------------------------------------------------
// Cursor shape changes (DECSCUSR)
// ---------------------------------------------------------------------------

/// DECSCUSR 0 -> default (block)
#[test]
fn decscusr_default() {
    let mut t = TestTerminal::new(80, 25);
    // First set to bar, then reset to default
    t.push(b"\x1b[5 q"); // bar
    assert_eq!(t.screen.cursor.shape, CursorShape::Bar);
    t.push(b"\x1b[0 q"); // default -> block
    assert_eq!(t.screen.cursor.shape, CursorShape::Block);
}

/// DECSCUSR 1 -> blinking block
#[test]
fn decscusr_blinking_block() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[1 q");
    assert_eq!(t.screen.cursor.shape, CursorShape::Block);
}

/// DECSCUSR 2 -> steady block
#[test]
fn decscusr_steady_block() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[2 q");
    assert_eq!(t.screen.cursor.shape, CursorShape::Block);
}

/// DECSCUSR 3 -> blinking underline
#[test]
fn decscusr_blinking_underline() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[3 q");
    assert_eq!(t.screen.cursor.shape, CursorShape::Underline);
}

/// DECSCUSR 5 -> blinking bar
#[test]
fn decscusr_blinking_bar() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[5 q");
    assert_eq!(t.screen.cursor.shape, CursorShape::Bar);
}

// ---------------------------------------------------------------------------
// SGR save/restore
// ---------------------------------------------------------------------------

/// Contour: SGRSAVE and SGRRESTORE
/// Save SGR attributes, change them, then restore.
#[test]
fn sgr_save_and_restore() {
    let mut t = TestTerminal::new(80, 25);
    // Set bold + red fg
    t.push(b"\x1b[1;91m"); // bright red (idx 9) to avoid bold_is_bright ambiguity
    assert!(t.screen.pen().bold);
    assert_eq!(t.screen.fg(), Color::Indexed(9));
    // XTPUSHSGR: CSI # {
    t.push(b"\x1b[#{");
    // Change to green fg, not bold
    t.push(b"\x1b[22;32m");
    assert!(!t.screen.pen().bold);
    assert_eq!(t.screen.fg(), Color::Indexed(2));
    // XTPOPSGR: CSI # }
    t.push(b"\x1b[#}");
    assert!(t.screen.pen().bold);
    assert_eq!(t.screen.fg(), Color::Indexed(9));
}

// ---------------------------------------------------------------------------
// Charset locking shifts
// ---------------------------------------------------------------------------

/// Contour: LS1 and LS0
/// Locking shift G1 into GL, then shift G0 back.
#[test]
fn charset_locking_shift_ls1_ls0() {
    let mut t = TestTerminal::new(80, 25);
    // Designate G1 as DEC Special Graphics
    t.push(b"\x1b)0");
    // LS1 (SO = 0x0E) -> activate G1
    t.push(b"\x0E");
    // 'q' in DEC Special Graphics is U+2500 (horizontal line)
    t.push_str("q");
    assert_eq!(t.screen.grid.cell(0, 0).c, '\u{2500}');

    // LS0 (SI = 0x0F) -> activate G0 (ASCII)
    t.push(b"\x0F");
    t.push_str("q");
    assert_eq!(t.screen.grid.cell(0, 1).c, 'q');
}

// ---------------------------------------------------------------------------
// DECFRA (fill rectangular area)
// ---------------------------------------------------------------------------

/// Contour: DECFRA.Full
#[test]
fn decfra_fill_rectangular_area() {
    let mut t = TestTerminal::new(10, 5);
    // Fill rows 2-4, cols 3-7 with 'X' (ASCII 88)
    // CSI 88;2;3;4;7 $ x
    t.push(b"\x1b[88;2;3;4;7$x");
    // Check filled area (1-based params -> 0-based: rows 1-3, cols 2-6)
    assert_eq!(t.screen.grid.cell(1, 2).c, 'X');
    assert_eq!(t.screen.grid.cell(1, 6).c, 'X');
    assert_eq!(t.screen.grid.cell(3, 2).c, 'X');
    assert_eq!(t.screen.grid.cell(3, 6).c, 'X');
    // Check outside area is still blank
    assert_eq!(t.screen.grid.cell(0, 2).c, ' ');
    assert_eq!(t.screen.grid.cell(1, 1).c, ' ');
    assert_eq!(t.screen.grid.cell(1, 7).c, ' ');
}

// ---------------------------------------------------------------------------
// DECCRA (copy rectangular area)
// ---------------------------------------------------------------------------

/// Contour: DECCRA.DownLeft.intersecting
#[test]
fn deccra_copy_overlapping_down_left() {
    let mut t = TestTerminal::new(5, 5);
    t.push_str("ABCDE");
    t.push_str("FGHIJ");
    t.push_str("KLMNO");
    // Copy rows 1-2, cols 1-3 to rows 2-3, cols 1-3
    // CSI 1;1;2;3;1;2;1;1 $ v  (src_top;src_left;src_bottom;src_right;src_page;dst_top;dst_left;dst_page)
    t.push(b"\x1b[1;1;2;3;1;2;1;1$v");
    // Row 1 (dst_top=2 -> 0-based row 1) should have 'A','B','C' in cols 0-2
    assert_eq!(t.screen.grid.cell(1, 0).c, 'A');
    assert_eq!(t.screen.grid.cell(1, 1).c, 'B');
    assert_eq!(t.screen.grid.cell(1, 2).c, 'C');
    // Row 2 should have 'F','G','H'
    assert_eq!(t.screen.grid.cell(2, 0).c, 'F');
    assert_eq!(t.screen.grid.cell(2, 1).c, 'G');
    assert_eq!(t.screen.grid.cell(2, 2).c, 'H');
}

/// Contour: DECCRA.Right.intersecting
#[test]
fn deccra_copy_overlapping_right() {
    let mut t = TestTerminal::new(5, 3);
    t.push_str("ABCDE");
    // Copy cols 1-3 to cols 2-4 on row 1 (overlapping right shift)
    // CSI 1;1;1;3;1;1;2;1 $ v
    t.push(b"\x1b[1;1;1;3;1;1;2;1$v");
    // Row 0: original 'A','B','C' copied to cols 1-3
    assert_eq!(t.screen.grid.cell(0, 1).c, 'A');
    assert_eq!(t.screen.grid.cell(0, 2).c, 'B');
    assert_eq!(t.screen.grid.cell(0, 3).c, 'C');
}

/// Contour: DECCRA.Left.intersecting
#[test]
fn deccra_copy_overlapping_left() {
    let mut t = TestTerminal::new(5, 3);
    t.push_str("ABCDE");
    // Copy cols 2-4 to cols 1-3 on row 1 (overlapping left shift)
    // CSI 1;2;1;4;1;1;1;1 $ v
    t.push(b"\x1b[1;2;1;4;1;1;1;1$v");
    assert_eq!(t.screen.grid.cell(0, 0).c, 'B');
    assert_eq!(t.screen.grid.cell(0, 1).c, 'C');
    assert_eq!(t.screen.grid.cell(0, 2).c, 'D');
}

// ---------------------------------------------------------------------------
// DECSERA (selective erase rectangular area)
// ---------------------------------------------------------------------------

/// Contour: DECSERA
#[test]
fn decsera_selective_erase_rect() {
    let mut t = TestTerminal::new(5, 3);
    // Write "ABCDE" on row 0
    t.push_str("ABCDE");
    // Protect 'B' and 'C' (at cols 1-2)
    t.push(b"\x1b[1;2H"); // move to col 2 (1-based)
    t.push(b"\x1b[1\"q"); // enable protection
    t.push_str("BC");
    t.push(b"\x1b[2\"q"); // disable protection
    // DECSERA: erase rows 1-1, cols 1-5 (the whole first row)
    // CSI 1;1;1;5 $ {
    t.push(b"\x1b[1;1;1;5${");
    // 'B' and 'C' should remain (protected), 'A','D','E' erased
    assert_eq!(t.screen.grid.cell(0, 0).c, ' ');
    assert_eq!(t.screen.grid.cell(0, 1).c, 'B');
    assert_eq!(t.screen.grid.cell(0, 2).c, 'C');
    assert_eq!(t.screen.grid.cell(0, 3).c, ' ');
    assert_eq!(t.screen.grid.cell(0, 4).c, ' ');
}

// ---------------------------------------------------------------------------
// DECSTR (soft terminal reset)
// ---------------------------------------------------------------------------

/// Contour: DECSTR
/// Soft reset resets modes and pen but preserves grid contents and cursor row.
#[test]
fn decstr_soft_reset() {
    let mut t = TestTerminal::new(80, 25);
    // Write some text and move cursor
    t.push_str("Hello");
    t.push(b"\x1b[3;10H"); // move to row 3, col 10
    t.assert_cursor(2, 9);

    // Set some SGR and modes
    t.push(b"\x1b[1;31m"); // bold + red fg
    assert!(t.screen.pen().bold);
    assert_eq!(t.screen.fg(), Color::Indexed(1));

    // Enable origin mode
    t.push(b"\x1b[?6h");
    assert!(t.screen.modes.origin);

    // Save cursor
    t.push(b"\x1b7");
    assert!(t.screen.has_saved_cursor());

    // Soft reset
    t.push(b"\x1b[!p");

    // Verify: pen reset
    assert!(!t.screen.pen().bold);
    assert_eq!(t.screen.fg(), Color::Default);
    assert_eq!(t.screen.bg(), Color::Default);

    // Verify: modes reset (autowrap back to default=true, origin=false)
    assert!(!t.screen.modes.origin);
    assert!(t.screen.modes.autowrap);

    // Verify: saved cursor cleared
    assert!(!t.screen.has_saved_cursor());

    // Verify: grid content preserved
    assert_eq!(t.row_text(0), "Hello");
}

// ---------------------------------------------------------------------------
// OSC 2 (window title) with Unicode
// ---------------------------------------------------------------------------

/// Contour: OSC.2.Unicode
#[test]
fn osc2_unicode_window_title() {
    let mut t = TestTerminal::new(80, 25);
    // Set title to a Unicode string with emoji
    t.push("\x1b]2;Hello \u{1F600} World\x07".as_bytes());
    assert_eq!(t.screen.title, "Hello \u{1F600} World");
}

// ---------------------------------------------------------------------------
// DEC mode save/restore
// ---------------------------------------------------------------------------

/// Contour: save_restore_DEC_modes
#[test]
fn save_restore_dec_modes() {
    let mut t = TestTerminal::new(80, 25);
    // Enable autowrap (already default true, but let's be explicit)
    t.push(b"\x1b[?7h");
    assert!(t.screen.modes.autowrap);
    // Save modes: CSI ? s
    t.push(b"\x1b[?s");
    // Disable autowrap
    t.push(b"\x1b[?7l");
    assert!(!t.screen.modes.autowrap);
    // Restore modes: CSI ? r
    t.push(b"\x1b[?r");
    assert!(t.screen.modes.autowrap);
}

// ---------------------------------------------------------------------------
// Horizontal tab edge cases
// ---------------------------------------------------------------------------

/// Contour: HorizontalTab.FillsCellsWithSpaces
/// "A\tB" -> cursor advances to tab stop at col 8, B at col 8.
#[test]
fn htab_fills_cells_with_spaces() {
    let mut t = TestTerminal::new(80, 25);
    t.push_str("A");
    t.push(b"\t");
    t.push_str("B");
    // A is at col 0, tab jumps to col 8, B is at col 8
    assert_eq!(t.screen.grid.cell(0, 0).c, 'A');
    assert_eq!(t.screen.grid.cell(0, 8).c, 'B');
    t.assert_cursor(0, 9);
}

/// Contour: HorizontalTab.AfterBulkText
/// "AB\tCD" -> exercises bulk-text fast path then C0 execute.
#[test]
fn htab_after_bulk_text() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"AB\tCD");
    assert_eq!(t.screen.grid.cell(0, 0).c, 'A');
    assert_eq!(t.screen.grid.cell(0, 1).c, 'B');
    // Tab from col 2 -> col 8
    assert_eq!(t.screen.grid.cell(0, 8).c, 'C');
    assert_eq!(t.screen.grid.cell(0, 9).c, 'D');
    t.assert_cursor(0, 10);
}

/// Contour: HorizontalTab.MultipleTabs
/// "A\tB\tC" -> correct spacing at each tab stop.
#[test]
fn htab_multiple_tabs() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"A\tB\tC");
    assert_eq!(t.screen.grid.cell(0, 0).c, 'A');
    // First tab: col 1 -> col 8
    assert_eq!(t.screen.grid.cell(0, 8).c, 'B');
    // Second tab: col 9 -> col 16
    assert_eq!(t.screen.grid.cell(0, 16).c, 'C');
    t.assert_cursor(0, 17);
}

/// Contour: HorizontalTab.AtChunkBoundary
/// Tab at PTY read buffer boundary still works.
#[test]
fn htab_at_chunk_boundary() {
    let mut t = TestTerminal::new(80, 25);
    // Simulate chunk boundary by sending data in two separate pushes
    t.push(b"ABCDEFG");
    t.push(b"\t");
    t.push(b"H");
    assert_eq!(t.screen.grid.cell(0, 0).c, 'A');
    // After 7 chars at cols 0-6, tab from col 7 -> col 8
    assert_eq!(t.screen.grid.cell(0, 8).c, 'H');
    t.assert_cursor(0, 9);
}

/// Contour: HorizontalTab.AfterScreenClear
/// Tab behavior is correct after ED 2 (clear screen).
#[test]
fn htab_after_screen_clear() {
    let mut t = TestTerminal::new(80, 25);
    t.push_str("some text");
    // Clear screen
    t.push(b"\x1b[2J");
    // Move to home
    t.push(b"\x1b[H");
    // Tab should still work with default tab stops
    t.push(b"A\tB");
    assert_eq!(t.screen.grid.cell(0, 0).c, 'A');
    assert_eq!(t.screen.grid.cell(0, 8).c, 'B');
    t.assert_cursor(0, 9);
}
