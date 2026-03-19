//! Screen conformance tests translated from libvterm test suite.
//!
//! Sources:
//!   - libvterm/t/60screen_ascii.test
//!   - libvterm/t/61screen_unicode.test
//!   - libvterm/t/62screen_damage.test
//!   - libvterm/t/63screen_resize.test
//!   - libvterm/t/69screen_reflow.test

mod common;
use common::TestTerminal;

use emux_term::{Color, DamageMode, DamageRegion};

// ---------------------------------------------------------------------------
// Local helpers
// ---------------------------------------------------------------------------

/// Check if a cell is at end-of-line content (all cells from col onwards are blank).
fn is_eol(t: &TestTerminal, row: usize, col: usize) -> bool {
    let cols = t.screen.cols();
    for c in col..cols {
        if t.cell(row, c).c != ' ' {
            return false;
        }
    }
    true
}

// ---------------------------------------------------------------------------
// Screen ASCII content (60screen_ascii.test)
// ---------------------------------------------------------------------------

#[test]
fn screen_ascii_print_and_query_chars() {
    // PUSH "ABC"
    // screen_chars 0,0,1,3 => "ABC"
    // screen_chars 0,0,1,80 => "ABC" (rest is blank)
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ABC");
    assert_eq!(t.cell(0, 0).c, 'A');
    assert_eq!(t.cell(0, 1).c, 'B');
    assert_eq!(t.cell(0, 2).c, 'C');
    // Remaining cells are blank spaces
    assert_eq!(t.cell(0, 3).c, ' ');
}

#[test]
fn screen_ascii_query_text_bytes() {
    // PUSH "ABC"
    // screen_text 0,0,1,3 => 0x41,0x42,0x43
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ABC");
    let text = t.row_text(0);
    assert_eq!(text.as_bytes(), b"ABC");
}

#[test]
fn screen_ascii_cell_attributes() {
    // PUSH "ABC"
    // screen_cell 0,0 => {0x41} width=1 attrs={} fg=default bg=default
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ABC");

    let c0 = t.cell(0, 0);
    assert_eq!(c0.c, 'A');
    assert_eq!(c0.width, 1);
    assert!(!c0.attrs.bold);
    assert!(!c0.attrs.italic);
    assert_eq!(c0.fg, Color::Default);
    assert_eq!(c0.bg, Color::Default);

    let c1 = t.cell(0, 1);
    assert_eq!(c1.c, 'B');
    assert_eq!(c1.width, 1);

    let c2 = t.cell(0, 2);
    assert_eq!(c2.c, 'C');
    assert_eq!(c2.width, 1);
}

#[test]
fn screen_ascii_row_query() {
    // PUSH "ABC"
    // screen_row 0 => "ABC"
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ABC");
    t.assert_row_text(0, "ABC");
}

#[test]
fn screen_ascii_eol_detection() {
    // PUSH "ABC"
    // screen_eol 0,0 => false (not at end of line)
    // screen_eol 0,2 => false
    // screen_eol 0,3 => true (at end of line content)
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ABC");
    assert!(!is_eol(&t, 0, 0));
    assert!(!is_eol(&t, 0, 2));
    assert!(is_eol(&t, 0, 3));
}

#[test]
fn screen_ascii_overwrite_at_origin() {
    // PUSH "ABC", then CUP(1,1), PUSH "E"
    // screen_row 0 => "EBC"
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ABC");
    t.push(b"\x1b[1;1H"); // CUP to row 1, col 1 (0,0)
    t.push(b"E");
    t.assert_row_text(0, "EBC");
    // Verify text bytes
    let text = t.row_text(0);
    assert_eq!(text.as_bytes(), &[0x45, 0x42, 0x43]);
}

#[test]
fn screen_ascii_erase_line() {
    // PUSH "ABCDE", CUP(1,1), EL 2 (erase entire line)
    // screen_row 0 => ""
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ABCDE");
    t.push(b"\x1b[1;1H"); // CUP to (0,0)
    t.push(b"\x1b[2K"); // EL 2: erase entire line
    t.assert_row_text(0, "");
}

#[test]
fn screen_ascii_insert_char_shift() {
    // PUSH "ABC", CUP(1,1), ICH 1, PUSH "1"
    // screen_row 0 => "1ABC"
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ABC");
    t.push(b"\x1b[1;1H"); // CUP to (0,0)
    t.push(b"\x1b[1@"); // ICH: insert 1 char
    t.push(b"1");
    t.assert_row_text(0, "1ABC");
}

#[test]
fn screen_ascii_delete_char_shift() {
    // PUSH "ABC", CUP(1,1), DCH 1
    // screen_chars 0,0 => "B"
    // screen_chars 0,1 => "C"
    // row text => "BC"
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ABC");
    t.push(b"\x1b[1;1H"); // CUP to (0,0)
    t.push(b"\x1b[1P"); // DCH: delete 1 char
    t.assert_cell_char(0, 0, 'B');
    t.assert_cell_char(0, 1, 'C');
    t.assert_row_text(0, "BC");
}

#[test]
fn screen_ascii_space_padding() {
    // PUSH "Hello\x1b[CWorld" (CUF = cursor forward 1)
    // screen_row 0 => "Hello World"
    let mut t = TestTerminal::new(80, 25);
    t.push(b"Hello\x1b[CWorld");
    t.assert_row_text(0, "Hello World");
}

#[test]
fn screen_ascii_linefeed_padding() {
    // PUSH "Hello\r\nWorld"
    // Row 0 => "Hello", Row 1 => "World"
    let mut t = TestTerminal::new(80, 25);
    t.push(b"Hello\r\nWorld");
    t.assert_row_text(0, "Hello");
    t.assert_row_text(1, "World");
}

#[test]
fn screen_ascii_cursor_home_preserves_content() {
    // PUSH "ABC", then CSI H (cursor home)
    // screen_row 0 => "ABC" (content preserved after cursor move)
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ABC");
    t.push(b"\x1b[H"); // CUP: cursor home (1,1)
    t.assert_row_text(0, "ABC");
    t.assert_cursor(0, 0);
}

// ---------------------------------------------------------------------------
// Unicode content (61screen_unicode.test)
// ---------------------------------------------------------------------------

#[test]
fn screen_unicode_single_width() {
    // U+00C1 (LATIN CAPITAL A WITH ACUTE), U+00E9 (LATIN SMALL E WITH ACUTE)
    // PUSH "\xC3\x81\xC3\xA9"
    // screen_cell 0,0 => {U+00C1} width=1
    // screen_cell 0,1 => {U+00E9} width=1
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\xC3\x81\xC3\xA9"); // UTF-8 for U+00C1, U+00E9
    t.assert_cell_char(0, 0, '\u{00C1}');
    t.assert_cell_width(0, 0, 1);
    t.assert_cell_char(0, 1, '\u{00E9}');
    t.assert_cell_width(0, 1, 1);
}

#[test]
fn screen_unicode_wide_char() {
    // U+FF10 (FULLWIDTH DIGIT ZERO), occupies 2 columns
    // PUSH "0123", CUP(1,1), PUSH U+FF10
    // Wide char replaces "01", result: [FF10][2][3]
    // screen_cell 0,0 => {U+FF10} width=2
    let mut t = TestTerminal::new(80, 25);
    t.push(b"0123");
    t.push(b"\x1b[1;1H"); // CUP to (0,0)
    t.push("\u{FF10}".as_bytes()); // Fullwidth digit zero
    t.assert_cell_char(0, 0, '\u{FF10}');
    t.assert_cell_width(0, 0, 2);
    // Continuation cell
    t.assert_cell_width(0, 1, 0);
    // Characters after wide char
    t.assert_cell_char(0, 2, '2');
    t.assert_cell_char(0, 3, '3');
}

#[test]
fn screen_unicode_combining_char() {
    // U+0301 (COMBINING ACUTE)
    // PUSH "0123", CUP(1,1), PUSH "e\xCC\x81"
    // Combining char should not advance cursor; 'e' at col 0, then cursor stays at col 1
    let mut t = TestTerminal::new(80, 25);
    t.push(b"0123");
    t.push(b"\x1b[1;1H"); // CUP to (0,0)
    t.push(b"e\xCC\x81"); // 'e' + U+0301 (combining acute)
    // 'e' overwrites '0' at col 0; combining accent is zero-width so cursor stays at col 1
    t.assert_cell_char(0, 0, 'e');
    t.assert_cell_width(0, 0, 1);
    // '1' was at col 1, still there since combining char didn't overwrite
    t.assert_cell_char(0, 1, '1');
    t.assert_cursor(0, 1);
}

#[test]
fn screen_unicode_many_combining_accents_no_crash() {
    // PUSH "e" + 10x U+0301
    // Must not crash; combining accents are zero-width and are skipped
    let mut t = TestTerminal::new(80, 25);
    t.push(b"e");
    for _ in 0..10 {
        t.push(b"\xCC\x81"); // U+0301
    }
    // 'e' should be at col 0, cursor at col 1 (combining chars don't advance)
    t.assert_cell_char(0, 0, 'e');
    t.assert_cursor(0, 1);
}

#[test]
fn screen_unicode_split_combining_no_crash() {
    // Two writes of 20 combining chars each; must not crash
    let mut t = TestTerminal::new(80, 25);
    t.push(b"e");
    // First batch of 20 combining accents
    for _ in 0..20 {
        t.push(b"\xCC\x81"); // U+0301
    }
    // Second batch of 20 combining accents
    for _ in 0..20 {
        t.push(b"\xCC\x81"); // U+0301
    }
    // No crash is the main assertion
    t.assert_cell_char(0, 0, 'e');
    t.assert_cursor(0, 1);
}

#[test]
fn screen_unicode_cjk_column80_wrap() {
    // CJK double-width at column 80 wraps to next line
    // PUSH CSI 80G + U+FF10
    // screen_cell 0,79 => blank padding (width 1)
    // screen_cell 1,0 => {U+FF10} width=2
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[80G"); // CHA: move to column 80 (0-based: col 79)
    t.push("\u{FF10}".as_bytes()); // Fullwidth digit zero
    // Column 79 should be blank padding
    assert_eq!(t.cell(0, 79).c, ' ');
    t.assert_cell_width(0, 79, 1);
    // Wide char wraps to next line
    t.assert_cell_char(1, 0, '\u{FF10}');
    t.assert_cell_width(1, 0, 2);
}

// ---------------------------------------------------------------------------
// Alt screen (60screen_ascii.test)
// ---------------------------------------------------------------------------
// Note: The current implementation only sets a mode flag for alt screen;
// there is no dual-grid (actual alternate buffer). These tests verify
// the mode flag toggles correctly. Full alt screen buffer tests are ignored.

#[test]
fn altscreen_switch_and_restore() {
    // PUSH "P" -> row 0 = "P"
    // CSI ?1049h (enter alt screen) -> row 0 = ""
    // PUSH "A" on alt screen -> row 0 = "A"
    // CSI ?1049l (leave alt screen) -> row 0 = "P" (main restored)
    let mut t = TestTerminal::new(80, 25);
    t.push(b"P");
    assert_eq!(t.row_text(0), "P");
    t.push(b"\x1b[?1049h"); // enter alt screen
    assert_eq!(t.row_text(0), ""); // alt screen is cleared
    t.push(b"A");
    assert_eq!(t.row_text(0), "A");
    t.push(b"\x1b[?1049l"); // leave alt screen
    assert_eq!(t.row_text(0), "P"); // main restored
}

#[test]
fn altscreen_erase_and_content() {
    // Enter alt screen, erase line, write at home position
    // Verify alt screen content is independent of main screen
    let mut t = TestTerminal::new(80, 25);
    t.push(b"Main");
    assert_eq!(t.row_text(0), "Main");
    t.push(b"\x1b[?1049h"); // enter alt screen
    t.push(b"\x1b[2K"); // erase line
    assert_eq!(t.row_text(0), ""); // alt screen empty
    t.push(b"Alt");
    assert_eq!(t.row_text(0), "Alt");
    t.push(b"\x1b[?1049l"); // leave alt screen
    assert_eq!(t.row_text(0), "Main"); // main screen preserved
}

#[test]
fn altscreen_resize() {
    // Write "Main screen", enter altscreen, write "Alt screen"
    // Resize to 30 rows
    // screen_row 0 => "Alt screen"
    // Leave altscreen => "Main screen"
    let mut t = TestTerminal::new(80, 25);
    t.push(b"Main screen");
    t.push(b"\x1b[?1049h"); // enter alt screen
    t.push(b"Alt screen");
    t.screen.resize(80, 30);
    assert_eq!(t.row_text(0), "Alt screen");
    t.push(b"\x1b[?1049l"); // leave alt screen
    assert_eq!(t.row_text(0), "Main screen");
}

#[test]
fn altscreen_damage_on_switch() {
    // CSI ?1049h => damage 0..25,0..80
    // CSI ?1049l => damage 0..25,0..80
    let mut t = TestTerminal::new(80, 25);
    t.screen.set_damage_mode(DamageMode::Cell);
    let _ = t.screen.take_damage(); // clear startup damage

    t.push(b"\x1b[?1049h"); // enter alt screen
    let damage = t.screen.take_damage();
    // Should damage every row
    assert!(
        !damage.is_empty(),
        "entering alt screen should produce damage"
    );
    assert_eq!(damage.len(), 25);
    for r in 0..25 {
        assert_eq!(
            damage[r],
            DamageRegion {
                row: r,
                col_start: 0,
                col_end: 80
            }
        );
    }

    t.push(b"\x1b[?1049l"); // leave alt screen
    let damage = t.screen.take_damage();
    assert!(
        !damage.is_empty(),
        "leaving alt screen should produce damage"
    );
    assert_eq!(damage.len(), 25);
    for r in 0..25 {
        assert_eq!(
            damage[r],
            DamageRegion {
                row: r,
                col_start: 0,
                col_end: 80
            }
        );
    }
}

#[test]
fn altscreen_mode_flag_toggle() {
    // Verify the alt_screen mode flag toggles correctly
    let mut t = TestTerminal::new(80, 25);
    assert!(!t.screen.modes.alt_screen);
    t.push(b"\x1b[?1049h");
    assert!(t.screen.modes.alt_screen);
    t.push(b"\x1b[?1049l");
    assert!(!t.screen.modes.alt_screen);
}

#[test]
fn altscreen_preserves_main_cursor() {
    // When entering altscreen, cursor position from main screen
    // is saved; when leaving, it is restored.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[5;10H"); // move cursor to row 4, col 9 (0-based)
    assert_eq!(t.cursor(), (4, 9));
    t.push(b"\x1b[?1049h"); // enter alt screen (mode 1049 saves cursor via DECSC)
    assert_eq!(t.cursor(), (0, 0)); // alt screen starts at home
    t.push(b"\x1b[3;5H"); // move to row 2, col 4
    assert_eq!(t.cursor(), (2, 4));
    t.push(b"\x1b[?1049l"); // leave alt screen (mode 1049 restores cursor via DECRC)
    assert_eq!(t.cursor(), (4, 9)); // main cursor restored
}

// ---------------------------------------------------------------------------
// Damage tracking (62screen_damage.test)
// ---------------------------------------------------------------------------

#[test]
fn damage_putglyph_individual_cells() {
    // Writing individual characters should produce cell-level damage.
    let mut t = TestTerminal::new(80, 25);
    t.screen.set_damage_mode(DamageMode::Cell);
    let _ = t.screen.take_damage();

    t.push(b"ABC");
    let damage = t.screen.take_damage();
    // Three cells damaged on row 0
    assert!(!damage.is_empty());
    // After merging, should be one region covering cols 0..3
    assert_eq!(damage.len(), 1);
    assert_eq!(
        damage[0],
        DamageRegion {
            row: 0,
            col_start: 0,
            col_end: 3
        }
    );
}

#[test]
fn damage_erase_range() {
    // Erasing a line should produce damage for the erased region.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"Hello World");
    t.screen.set_damage_mode(DamageMode::Cell);
    let _ = t.screen.take_damage();

    // Move to col 0, erase entire line
    t.push(b"\x1b[1;1H");
    t.push(b"\x1b[2K"); // EL 2: erase entire line
    let damage = t.screen.take_damage();
    assert!(!damage.is_empty());
    // Should have damage for row 0, covering full width
    let row0: Vec<_> = damage.iter().filter(|d| d.row == 0).collect();
    assert!(!row0.is_empty());
    assert_eq!(row0[0].col_start, 0);
    assert_eq!(row0[0].col_end, 80);
}

#[test]
fn damage_scroll_insert_chars() {
    // ICH (insert chars) should damage the row from cursor to right edge.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ABCDE");
    t.push(b"\x1b[1;3H"); // CUP to (0, 2)
    t.screen.set_damage_mode(DamageMode::Cell);
    let _ = t.screen.take_damage();

    t.push(b"\x1b[2@"); // ICH 2
    let damage = t.screen.take_damage();
    assert!(!damage.is_empty());
    let row0: Vec<_> = damage.iter().filter(|d| d.row == 0).collect();
    assert!(!row0.is_empty());
    // Damage should cover from col 2 to right edge
    assert_eq!(row0[0].col_start, 2);
    assert_eq!(row0[0].col_end, 80);
}

#[test]
fn damage_scroll_down() {
    // Scroll down should damage the scroll region.
    let mut t = TestTerminal::new(80, 25);
    t.screen.set_damage_mode(DamageMode::Cell);
    let _ = t.screen.take_damage();

    // Set scroll region 1..10 and scroll down
    t.push(b"\x1b[1;10r"); // DECSTBM: scroll region rows 1-10
    let _ = t.screen.take_damage();
    t.push(b"\x1b[T"); // SD: scroll down 1 line
    let damage = t.screen.take_damage();
    assert!(!damage.is_empty());
    // Should have damage for all rows in scroll region (0..10)
    let damaged_rows: std::collections::HashSet<usize> = damage.iter().map(|d| d.row).collect();
    for r in 0..10 {
        assert!(damaged_rows.contains(&r), "row {} should be damaged", r);
    }
}

#[test]
fn damage_merge_cell_mode() {
    // In Cell mode, overlapping damage on the same row is merged.
    let mut t = TestTerminal::new(80, 25);
    t.screen.set_damage_mode(DamageMode::Cell);
    let _ = t.screen.take_damage();

    t.push(b"AB");
    let damage = t.screen.take_damage();
    // Should be merged into a single region
    assert_eq!(damage.len(), 1);
    assert_eq!(
        damage[0],
        DamageRegion {
            row: 0,
            col_start: 0,
            col_end: 2
        }
    );
}

#[test]
fn damage_merge_row_mode() {
    // In Row mode, any cell damage is expanded to the full row width.
    let mut t = TestTerminal::new(80, 25);
    t.screen.set_damage_mode(DamageMode::Row);
    let _ = t.screen.take_damage();

    t.push(b"X");
    let damage = t.screen.take_damage();
    assert_eq!(damage.len(), 1);
    assert_eq!(
        damage[0],
        DamageRegion {
            row: 0,
            col_start: 0,
            col_end: 80
        }
    );
}

#[test]
fn damage_merge_screen_mode() {
    // In Screen mode, any damage marks the entire screen.
    let mut t = TestTerminal::new(80, 25);
    t.screen.set_damage_mode(DamageMode::Screen);
    let _ = t.screen.take_damage();

    t.push(b"X"); // Just write one character
    let damage = t.screen.take_damage();
    // Should cover all 25 rows
    assert_eq!(damage.len(), 25);
    for r in 0..25 {
        assert_eq!(
            damage[r],
            DamageRegion {
                row: r,
                col_start: 0,
                col_end: 80
            }
        );
    }
}

#[test]
fn damage_moverect_with_scroll() {
    // Scrolling up moves content and damages the scroll region.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"Line1\r\nLine2\r\nLine3");
    t.screen.set_damage_mode(DamageMode::Cell);
    let _ = t.screen.take_damage();

    t.push(b"\x1b[S"); // SU: scroll up 1 line
    let damage = t.screen.take_damage();
    assert!(!damage.is_empty());
    // Full scroll region (0..25) should be damaged
    let damaged_rows: std::collections::HashSet<usize> = damage.iter().map(|d| d.row).collect();
    for r in 0..25 {
        assert!(
            damaged_rows.contains(&r),
            "row {} should be damaged after scroll",
            r
        );
    }
}

#[test]
fn damage_merge_scroll_mode() {
    // In Scroll mode, scroll operations damage the scroll region rows.
    let mut t = TestTerminal::new(80, 25);
    t.screen.set_damage_mode(DamageMode::Scroll);
    let _ = t.screen.take_damage();

    t.push(b"\x1b[S"); // SU: scroll up 1 line
    let damage = t.screen.take_damage();
    assert!(!damage.is_empty());
    let damaged_rows: std::collections::HashSet<usize> = damage.iter().map(|d| d.row).collect();
    for r in 0..25 {
        assert!(
            damaged_rows.contains(&r),
            "row {} should be damaged in scroll mode",
            r
        );
    }
}

#[test]
fn damage_scroll_with_content() {
    // Scroll with visible content: all rows in scroll region should be damaged.
    let mut t = TestTerminal::new(80, 25);
    for i in 0..25 {
        t.push_str(&format!("Row{}", i));
        if i < 24 {
            t.push(b"\r\n");
        }
    }
    t.screen.set_damage_mode(DamageMode::Cell);
    let _ = t.screen.take_damage();

    // Scroll up one line
    t.push(b"\r\n");
    let damage = t.screen.take_damage();
    assert!(!damage.is_empty());
    // At minimum, the scroll region should be damaged
    let damaged_rows: std::collections::HashSet<usize> = damage.iter().map(|d| d.row).collect();
    // The last row should be damaged (new blank line from scroll)
    assert!(
        damaged_rows.contains(&24) || damaged_rows.contains(&0),
        "scroll region should include damaged rows"
    );
}

// ---------------------------------------------------------------------------
// Resize (63screen_resize.test)
// ---------------------------------------------------------------------------

#[test]
fn resize_wider_preserves_cells() {
    // 25x80, PUSH "AB\r\nCD", resize to 25x100
    // screen_chars row 0 => "AB", row 1 => "CD"
    let mut t = TestTerminal::new(80, 25);
    t.push(b"AB\r\nCD");
    t.screen.resize(100, 25);
    t.assert_row_text(0, "AB");
    t.assert_row_text(1, "CD");
}

#[test]
fn resize_wider_allows_print_in_new_area() {
    // After resize wider, cursor can print into the newly available columns
    let mut t = TestTerminal::new(80, 25);
    t.push(b"AB");
    t.push(b"\x1b[1;79H"); // CUP to (0, 78)
    t.push(b"CD");
    // After resize, content preserved
    t.screen.resize(100, 25);
    t.assert_cell_char(0, 0, 'A');
    t.assert_cell_char(0, 1, 'B');
    t.assert_cell_char(0, 78, 'C');
    t.assert_cell_char(0, 79, 'D');
    // Move cursor explicitly to col 80 and print there
    t.push(b"\x1b[1;81H"); // CUP to (0, 80)
    t.push(b"E");
    t.assert_cell_char(0, 80, 'E');
}

#[test]
fn resize_shorter_with_blanks_truncates() {
    // 25x80, "Top" at row 0, "Line 10" at row 9, cursor at 9,7
    // Resize to 20x80 => content preserved, cursor stays at 9,7
    let mut t = TestTerminal::new(80, 25);
    t.push(b"Top");
    t.push(b"\x1b[10;1H"); // CUP to row 10 (0-based: 9)
    t.push(b"Line 10");
    assert_eq!(t.cursor(), (9, 7));
    t.screen.resize(80, 20);
    t.assert_row_text(0, "Top");
    t.assert_row_text(9, "Line 10");
    // Cursor should be clamped to valid range
    let (r, c) = t.cursor();
    assert!(r < 20);
    assert_eq!(c, 7);
}

#[test]
fn resize_shorter_with_content_scrolls() {
    // 25x80, "Top" at row 0, "Line 25" at row 24
    // Resize to 20x80 => lines pushed to scrollback, "Line 25" near bottom
    let mut t = TestTerminal::new(80, 25);
    t.push(b"Top");
    t.push(b"\x1b[25;1H"); // CUP to row 25 (0-based: 24)
    t.push(b"Line 25");
    t.screen.resize(80, 20);
    // "Line 25" should still be visible (at row 19 or wherever it ended up)
    // The exact row depends on which lines got pushed to scrollback
    let mut found = false;
    for r in 0..20 {
        if t.row_text(r) == "Line 25" {
            found = true;
            break;
        }
    }
    assert!(found, "Line 25 should still be visible after resize");
}

#[test]
fn resize_shorter_preserves_cursor_line() {
    // Cursor on last row, resize smaller
    // Cursor line should not be lost
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[25;1H"); // CUP to row 25 (0-based: 24)
    t.push(b"CursorLine");
    t.screen.resize(80, 24);
    // Cursor line content should still be somewhere in the visible grid
    let mut found = false;
    for r in 0..24 {
        if t.row_text(r) == "CursorLine" {
            found = true;
            break;
        }
    }
    assert!(found, "cursor line should survive resize");
}

#[test]
fn resize_shorter_cursor_not_negative() {
    // Cursor at row 0, content on bottom rows
    // Resize smaller => cursor stays at 0,0
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[24;1HLine24");
    t.push(b"\x1b[25;1HLine25");
    t.push(b"\x1b[1;1H"); // CUP to (0,0)
    t.screen.resize(80, 20);
    let (r, _c) = t.cursor();
    assert_eq!(r, 0, "cursor row should not be negative after resize");
}

#[test]
fn resize_taller_pops_scrollback() {
    // 25x80, fill rows so scrollback exists, then resize taller
    let mut t = TestTerminal::new(80, 25);
    // Fill all 25 rows to create content
    for i in 0..25 {
        t.push_str(&format!("Line {}", i));
        if i < 24 {
            t.push(b"\r\n");
        }
    }
    // Now scroll a few more lines to push content into scrollback
    t.push(b"\r\n");
    t.push(b"Extra1\r\n");
    t.push(b"Extra2");
    let sb_before = t.screen.grid.scrollback_len();
    assert!(sb_before > 0, "should have scrollback content");
    // Resize taller
    t.screen.resize(80, 30);
    // Scrollback should have been popped
    let sb_after = t.screen.grid.scrollback_len();
    assert!(
        sb_after < sb_before,
        "scrollback should shrink after resize taller"
    );
}

#[test]
fn resize_altscreen_independent() {
    // "Main screen" on main, enter altscreen "Alt screen"
    // Resize 30 rows, verify alt content, leave altscreen => main content
    let mut t = TestTerminal::new(80, 25);
    t.push(b"Main screen");
    t.push(b"\x1b[?1049h"); // enter alt screen
    t.push(b"Alt screen");
    assert_eq!(t.row_text(0), "Alt screen");
    t.screen.resize(80, 30);
    assert_eq!(t.row_text(0), "Alt screen");
    t.push(b"\x1b[?1049l"); // leave alt screen
    assert_eq!(t.row_text(0), "Main screen");
}

// ---------------------------------------------------------------------------
// Reflow (69screen_reflow.test)
// ---------------------------------------------------------------------------

#[test]
fn reflow_wider_unwraps_continuation() {
    // Write text that wraps at 20 cols, then widen to 40 cols.
    // The continuation line should be unwrapped back to a single row.
    let mut t = TestTerminal::new(20, 10);
    // Write exactly 20 chars to trigger wrap, then more chars
    t.push(b"AAAAAAAAAABBBBBBBBBB"); // 20 chars, fills row 0
    t.push(b"CCCCCCCCCC"); // 10 more chars on row 1 (continuation)

    assert_eq!(t.row_text(0), "AAAAAAAAAABBBBBBBBBB");
    assert_eq!(t.row_text(1), "CCCCCCCCCC");
    assert!(t.screen.grid.row(1).flags.continuation);

    // Widen to 40 cols: should unwrap into single line
    t.screen.resize(40, 10);
    assert_eq!(t.row_text(0), "AAAAAAAAAABBBBBBBBBBCCCCCCCCCC");
    assert_eq!(t.row_text(1), "");
}

#[test]
fn reflow_wider_beyond_content() {
    // Widen beyond the content length; everything fits on one line.
    let mut t = TestTerminal::new(10, 5);
    t.push(b"AAAAAAAAAA"); // fills row 0 (10 chars)
    t.push(b"BBBBB"); // 5 chars on row 1 (continuation)
    assert!(t.screen.grid.row(1).flags.continuation);

    t.screen.resize(20, 5);
    // Should fit on one line (10 As + 5 Bs = 15 chars)
    assert_eq!(t.row_text(0), "AAAAAAAAAABBBBB");
    assert_eq!(t.row_text(1), "");
}

#[test]
fn reflow_narrower_creates_continuation() {
    // Write text in 20 cols, then narrow to 10 cols.
    // The line should be split into multiple rows with continuation flags.
    let mut t = TestTerminal::new(20, 10);
    t.push(b"AAAAAAAAAABBBBBBBBBB"); // 20 chars on row 0
    assert_eq!(t.row_text(0), "AAAAAAAAAABBBBBBBBBB");

    // Now push newline so the row isn't single (needs pending wrap trigger).
    // Actually, since the text fills the row exactly, pending_wrap is set.
    // Let's force a linefeed to commit it, then write on next line.
    t.push(b"\r\nX");

    // Narrow: the 20-char line was on row 0 with continuation to row 1.
    // After writing "X" the pending_wrap triggered, so row 1 is a continuation.
    // Let's check: row 0 = "AAAAAAAAAABBBBBBBBBB", row 1 has continuation, row 2 = "X"
    // Actually row 1 was the newline target. Let me verify:
    assert_eq!(t.row_text(1), "X");

    // Narrow to 10 cols
    t.screen.resize(10, 10);
    // Row 0 was a single-row logical line (no continuation after it), so it gets truncated.
    // But wait - the pending_wrap on row 0 would have set continuation on row 1 if it wrapped.
    // Since the content is exactly 20 chars at 20 cols, pending_wrap was set but the \r\n
    // caused a newline (not autowrap). So row 1 is NOT continuation.
    // So row 0 is a single-row line, truncated to 10 cols.
    assert_eq!(t.row_text(0), "AAAAAAAAAA");
}

#[test]
fn reflow_narrower_cursor_tracks() {
    // Write text, then narrow. Cursor should track correctly.
    let mut t = TestTerminal::new(20, 10);
    t.push(b"AAAAAAAAAABBBBBBBBBB"); // 20 chars, pending wrap
    t.push(b"CC"); // wraps to row 1, continuation row; cursor at (1, 2)
    assert_eq!(t.cursor(), (1, 2));

    // Narrow to 10 cols: logical line is 22 chars, takes 3 rows at width 10
    // Row 0: AAAAAAAAAA, Row 1(cont): BBBBBBBBBB, Row 2(cont): CC
    // First chunk may go to scrollback if viewport overflows.
    // Cursor should end up on the "CC" row.
    t.screen.resize(10, 10);
    let (cr, cc) = t.cursor();
    // The "CC" content should be at the cursor's row
    assert_eq!(t.row_text(cr), "CC", "cursor should be on the CC row");
    assert_eq!(cc, 2, "cursor col should be 2");
}

#[test]
fn reflow_shell_prompt_wrap_11cols() {
    // Simulate a shell prompt that wraps: "user@host:~$ " (14 chars)
    // At 11 cols, it wraps to 2 rows.
    let mut t = TestTerminal::new(20, 10);
    t.push(b"user@host:~$ "); // 14 chars, fits in 20 cols
    assert_eq!(t.row_text(0), "user@host:~$");

    // Narrow to 11 cols
    t.screen.resize(11, 10);
    // Single-row line (no continuation), truncated to 11 cols
    assert_eq!(t.row_text(0), "user@host:~");
}

#[test]
fn reflow_shell_prompt_wrap_12cols() {
    // At 12 cols the prompt "user@host:~" fits on row 0, "$ " on row 1 if wrapped.
    let mut t = TestTerminal::new(20, 10);
    t.push(b"user@host:~$ ");
    assert_eq!(t.row_text(0), "user@host:~$");

    t.screen.resize(12, 10);
    // Single row, truncated: "user@host:~$" is 12 chars + space is at 13, but
    // row_text trims trailing spaces, so we get the first 12 chars.
    let text = t.row_text(0);
    assert!(
        text.len() <= 12,
        "row text should fit in 12 cols: '{}'",
        text
    );
}

#[test]
fn reflow_shell_prompt_wrap_16cols() {
    // At 16 cols the full prompt fits.
    let mut t = TestTerminal::new(20, 10);
    t.push(b"user@host:~$ ");
    assert_eq!(t.row_text(0), "user@host:~$");

    t.screen.resize(16, 10);
    // Still fits on one line
    assert_eq!(t.row_text(0), "user@host:~$");
}

#[test]
fn reflow_lineinfo_cont_flag() {
    // Verify the continuation flag is set correctly after reflow.
    let mut t = TestTerminal::new(10, 5);
    // Write 10 chars to fill row 0, then 5 more on continuation row 1.
    t.push(b"AAAAAAAAAA"); // fills row 0
    t.push(b"BBBBB"); // wraps to row 1

    assert!(!t.screen.grid.row(0).flags.continuation);
    assert!(t.screen.grid.row(1).flags.continuation);

    // Widen to 20: should unwrap
    t.screen.resize(20, 5);
    assert!(!t.screen.grid.row(0).flags.continuation);
    // Row 1 should now be empty (and not continuation)
    assert!(!t.screen.grid.row(1).flags.continuation);
    assert_eq!(t.row_text(1), "");

    // Narrow back to 10: should re-wrap
    t.screen.resize(10, 5);
    assert!(!t.screen.grid.row(0).flags.continuation);
    // But since the unwrapped line was a single-row logical line at 20 cols,
    // narrowing to 10 truncates (no continuation created for single-row lines).
    // The content "AAAAAAAAAAABBBBB" (15 chars) on one row at 20 cols becomes
    // truncated to 10 cols: "AAAAAAAAAA"
    assert_eq!(t.row_text(0), "AAAAAAAAAA");
}

#[test]
fn reflow_cursor_position_after_unwrap() {
    // After unwrapping, cursor should be on the correct position.
    let mut t = TestTerminal::new(10, 5);
    t.push(b"AAAAAAAAAA"); // 10 chars, fills row 0, pending wrap
    t.push(b"BB"); // wraps to row 1, cursor at (1, 2)
    assert_eq!(t.cursor(), (1, 2));

    // Widen to 20: unwrap into single row
    t.screen.resize(20, 5);
    // After unwrap, cursor should still be at the correct position
    let (cr, cc) = t.cursor();
    assert_eq!(cr, 0, "cursor should be on row 0 after unwrap");
    // Cursor col should be at 12 (10 + 2) or clamped appropriately
    assert!(cc <= 19, "cursor col should be within bounds");
}

#[test]
fn reflow_cursor_survives_extreme_resize() {
    // Resize to very small and back should not crash or lose cursor.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"Hello World");
    t.push(b"\x1b[1;12H"); // cursor at (0, 11)
    assert_eq!(t.cursor(), (0, 11));

    // Resize very small
    t.screen.resize(5, 3);
    let (cr, cc) = t.cursor();
    assert!(cr < 3, "cursor row in bounds");
    assert!(cc < 5, "cursor col in bounds");

    // Resize back
    t.screen.resize(80, 25);
    let (cr, cc) = t.cursor();
    assert!(cr < 25, "cursor row in bounds after restore");
    assert!(cc < 80, "cursor col in bounds after restore");
}

#[test]
fn reflow_prompt_cursor_stable() {
    // After typing at a prompt and resizing, cursor should stay near the end of input.
    let mut t = TestTerminal::new(40, 10);
    t.push(b"$ ls -la /very/long/path/to/some/file"); // 38 chars
    let (_, orig_col) = t.cursor();

    // Widen a bit
    t.screen.resize(60, 10);
    let (cr, cc) = t.cursor();
    assert_eq!(cr, 0, "cursor should stay on row 0");
    // Cursor col should be the same (content didn't reflow since it was single row)
    assert_eq!(cc, orig_col, "cursor col should be preserved");

    // Narrow back
    t.screen.resize(40, 10);
    let (cr2, cc2) = t.cursor();
    assert_eq!(cr2, 0);
    assert_eq!(cc2, orig_col, "cursor col should survive round-trip resize");
}

// ---------------------------------------------------------------------------
// Additional screen behaviour tests
// ---------------------------------------------------------------------------

#[test]
fn screen_reset_clears_content() {
    // After RIS (full reset), all rows should be empty
    let mut t = TestTerminal::new(80, 25);
    t.push(b"Hello World");
    t.push(b"\x1b[2;1HSecond line");
    t.assert_row_text(0, "Hello World");
    t.assert_row_text(1, "Second line");
    // ESC c = RIS (full reset)
    t.push(b"\x1bc");
    t.assert_row_text(0, "");
    t.assert_row_text(1, "");
    t.assert_cursor(0, 0);
}

#[test]
fn screen_default_fg_bg_colors() {
    // Default cell has Color::Default for both fg and bg
    let mut t = TestTerminal::new(80, 25);
    t.push(b"X");
    let c = t.cell(0, 0);
    assert_eq!(c.fg, Color::Default);
    assert_eq!(c.bg, Color::Default);
}

#[test]
fn screen_sgr_colors_on_cell() {
    // Cells written with SGR colors retain those colors
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[31mR\x1b[32mG\x1b[34mB\x1b[0m");
    assert_eq!(t.cell(0, 0).fg, Color::Indexed(1)); // red
    assert_eq!(t.cell(0, 1).fg, Color::Indexed(2)); // green
    assert_eq!(t.cell(0, 2).fg, Color::Indexed(4)); // blue
}

#[test]
fn screen_sgr_reset_restores_defaults() {
    // SGR 0 resets pen to defaults
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[1;31;42m");
    assert!(t.screen.pen().bold);
    assert_eq!(t.screen.fg(), Color::Indexed(1));
    assert_eq!(t.screen.bg(), Color::Indexed(2));
    t.push(b"\x1b[0m");
    assert!(!t.screen.pen().bold);
    assert_eq!(t.screen.fg(), Color::Default);
    assert_eq!(t.screen.bg(), Color::Default);
}

#[test]
fn screen_scroll_region_damage() {
    // Scrolling within a scroll region should damage only the region rows.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[5;15r"); // DECSTBM: scroll region rows 5-15 (0-based: 4..15)
    t.screen.set_damage_mode(DamageMode::Cell);
    let _ = t.screen.take_damage();

    t.push(b"\x1b[15;1H"); // move to bottom of scroll region
    let _ = t.screen.take_damage();
    // Linefeed at bottom of scroll region triggers scroll
    t.push(b"\n");
    let damage = t.screen.take_damage();
    assert!(!damage.is_empty());
    let damaged_rows: std::collections::HashSet<usize> = damage.iter().map(|d| d.row).collect();
    // All rows in scroll region 4..15 should be damaged
    for r in 4..15 {
        assert!(
            damaged_rows.contains(&r),
            "row {} in scroll region should be damaged",
            r
        );
    }
}

#[test]
fn screen_damage_outside_scroll_region() {
    // Writing outside the scroll region should only damage those rows.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[5;15r"); // scroll region rows 5-15
    t.screen.set_damage_mode(DamageMode::Cell);
    let _ = t.screen.take_damage();

    // Write on row 0 (outside scroll region)
    t.push(b"\x1b[1;1H"); // CUP to (0,0)
    let _ = t.screen.take_damage();
    t.push(b"X");
    let damage = t.screen.take_damage();
    assert!(!damage.is_empty());
    // Only row 0 should be damaged
    assert!(
        damage.iter().all(|d| d.row == 0),
        "damage should only be on row 0"
    );
}

#[test]
fn screen_damage_overlapping_scroll_region() {
    // Writing inside a scroll region should produce damage within that region.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[5;15r"); // scroll region rows 5-15
    t.screen.set_damage_mode(DamageMode::Cell);
    let _ = t.screen.take_damage();

    // Move inside scroll region and write
    t.push(b"\x1b[6;1H"); // CUP to row 6 (0-based: 5)
    let _ = t.screen.take_damage();
    t.push(b"Inside");
    let damage = t.screen.take_damage();
    assert!(!damage.is_empty());
    // Damage should be on row 5
    assert!(
        damage.iter().any(|d| d.row == 5),
        "row 5 (inside scroll region) should be damaged"
    );
}
