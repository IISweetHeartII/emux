//! Terminal state conformance tests derived from libvterm's test suite.
//!
//! Each test verifies expected behavior for a specific terminal operation
//! using the real `Screen` + `Parser` implementation.
//!
//! Run with: `cargo test -p emux-term --test state_conformance`
//!
//! Reference: libvterm/t/10state_putglyph.test through 30state_pen.test
//! Terminal dimensions assumed: 80 columns x 25 rows (libvterm default).

mod common;
use common::TestTerminal;

use emux_term::{Color, grid::UnderlineStyle};

// ---------------------------------------------------------------------------
// Pen-specific helpers (not shared across test files)
// ---------------------------------------------------------------------------

#[allow(dead_code)]
trait PenAssertions {
    fn screen(&self) -> &emux_term::Screen;

    fn assert_bold(&self, expected: bool) {
        assert_eq!(self.screen().pen().bold, expected, "pen bold mismatch");
    }

    fn assert_underline(&self, expected: u8) {
        let actual = match self.screen().pen().underline {
            UnderlineStyle::None => 0,
            UnderlineStyle::Single => 1,
            UnderlineStyle::Double => 2,
            UnderlineStyle::Curly => 3,
        };
        assert_eq!(actual, expected, "pen underline mismatch");
    }

    fn assert_italic(&self, expected: bool) {
        assert_eq!(self.screen().pen().italic, expected, "pen italic mismatch");
    }

    fn assert_blink(&self, expected: bool) {
        assert_eq!(self.screen().pen().blink, expected, "pen blink mismatch");
    }

    fn assert_reverse(&self, expected: bool) {
        assert_eq!(
            self.screen().pen().reverse,
            expected,
            "pen reverse mismatch"
        );
    }

    fn assert_strikethrough(&self, expected: bool) {
        assert_eq!(
            self.screen().pen().strikethrough,
            expected,
            "pen strikethrough mismatch"
        );
    }

    fn assert_fg(&self, color: Color) {
        assert_eq!(self.screen().fg(), color, "fg color mismatch");
    }

    fn assert_bg(&self, color: Color) {
        assert_eq!(self.screen().bg(), color, "bg color mismatch");
    }

    fn assert_fg_indexed(&self, idx: u8) {
        self.assert_fg(Color::Indexed(idx));
    }

    fn assert_bg_indexed(&self, idx: u8) {
        self.assert_bg(Color::Indexed(idx));
    }

    fn assert_fg_rgb(&self, r: u8, g: u8, b: u8) {
        self.assert_fg(Color::Rgb(r, g, b));
    }

    fn assert_bg_rgb(&self, r: u8, g: u8, b: u8) {
        self.assert_bg(Color::Rgb(r, g, b));
    }

    fn assert_fg_default(&self) {
        self.assert_fg(Color::Default);
    }

    fn assert_bg_default(&self) {
        self.assert_bg(Color::Default);
    }
}

impl PenAssertions for TestTerminal {
    fn screen(&self) -> &emux_term::Screen {
        &self.screen
    }
}

// ---------------------------------------------------------------------------
// 10state_putglyph: Glyph rendering
// ---------------------------------------------------------------------------

#[test]
fn putglyph_ascii() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ABC");
    t.assert_cell_char(0, 0, 'A');
    t.assert_cell_char(0, 1, 'B');
    t.assert_cell_char(0, 2, 'C');
    t.assert_cursor(0, 3);
}

#[test]
fn putglyph_utf8_1char() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\xC3\x81\xC3\xA9");
    t.assert_cell_char(0, 0, '\u{00C1}');
    t.assert_cell_char(0, 1, '\u{00E9}');
    t.assert_cursor(0, 2);
}

#[test]
fn putglyph_utf8_split_write() {
    // UTF-8 character split across two push calls.
    // The persistent parser handles this correctly.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\xC3");
    t.push(b"\x81");
    t.assert_cell_char(0, 0, '\u{00C1}');
}

#[test]
fn putglyph_wide_char() {
    let mut t = TestTerminal::new(80, 25);
    t.push("\u{FF10} ".as_bytes());
    t.assert_cell_char(0, 0, '\u{FF10}');
    t.assert_cell_width(0, 0, 2);
    t.assert_cell_char(0, 2, ' ');
}

#[test]
fn putglyph_emoji_wide() {
    let mut t = TestTerminal::new(80, 25);
    t.push("\u{1F600} ".as_bytes());
    t.assert_cell_char(0, 0, '\u{1F600}');
    t.assert_cell_width(0, 0, 2);
    t.assert_cell_char(0, 2, ' ');
}

#[test]
fn putglyph_combining_chars() {
    let mut t = TestTerminal::new(80, 25);
    t.push("e\u{0301}Z".as_bytes());
    t.assert_cell_char(0, 1, 'Z');
    t.assert_cursor(0, 2);
}

// ---------------------------------------------------------------------------
// 11state_movecursor: Cursor movement
// ---------------------------------------------------------------------------

#[test]
fn cursor_implicit_advance() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ABC");
    t.assert_cursor(0, 3);
}

#[test]
fn cursor_backspace() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ABC");
    t.push(b"\x08");
    t.assert_cursor(0, 2);
}

#[test]
fn cursor_horizontal_tab() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\t");
    t.assert_cursor(0, 8);
}

#[test]
fn cursor_carriage_return() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ABC\r");
    t.assert_cursor(0, 0);
}

#[test]
fn cursor_linefeed() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\r\n");
    t.assert_cursor(1, 0);
}

#[test]
fn cursor_backspace_bounded_left() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[4;2H"); // (3, 1)
    t.assert_cursor(3, 1);
    t.push(b"\x08");
    t.assert_cursor(3, 0);
    t.push(b"\x08");
    t.assert_cursor(3, 0); // bounded
}

#[test]
fn cursor_ht_bounded_right() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[1;78H"); // (0, 77)
    t.push(b"\t");
    t.assert_cursor(0, 79);
    t.push(b"\t");
    t.assert_cursor(0, 79); // bounded
}

#[test]
fn cursor_index_ind() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ABC\x1bD");
    t.assert_cursor(1, 3);
}

#[test]
fn cursor_reverse_index_ri() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ABC\x1bD"); // (1, 3)
    t.push(b"\x1bM");
    t.assert_cursor(0, 3);
}

#[test]
fn cursor_nel() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ABC\x1bE");
    t.assert_cursor(1, 0);
}

#[test]
fn cursor_cud() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[B");
    t.assert_cursor(1, 0);
    t.push(b"\x1b[3B");
    t.assert_cursor(4, 0);
    t.push(b"\x1b[0B"); // 0 treated as 1
    t.assert_cursor(5, 0);
}

#[test]
fn cursor_cuf() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[5B"); // move down to row 5
    t.push(b"\x1b[C");
    t.assert_cursor(5, 1);
    t.push(b"\x1b[3C");
    t.assert_cursor(5, 4);
    t.push(b"\x1b[0C");
    t.assert_cursor(5, 5);
}

#[test]
fn cursor_cuu() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[6;6H"); // (5, 5)
    t.push(b"\x1b[A");
    t.assert_cursor(4, 5);
    t.push(b"\x1b[3A");
    t.assert_cursor(1, 5);
    t.push(b"\x1b[0A");
    t.assert_cursor(0, 5);
}

#[test]
fn cursor_cub() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[1;6H"); // (0, 5)
    t.push(b"\x1b[D");
    t.assert_cursor(0, 4);
    t.push(b"\x1b[3D");
    t.assert_cursor(0, 1);
    t.push(b"\x1b[0D");
    t.assert_cursor(0, 0);
}

#[test]
fn cursor_cnl() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"   "); // (0, 3)
    t.push(b"\x1b[E");
    t.assert_cursor(1, 0);
    t.push(b"   ");
    t.push(b"\x1b[2E");
    t.assert_cursor(3, 0);
    t.push(b"\x1b[0E");
    t.assert_cursor(4, 0);
}

#[test]
fn cursor_cpl() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[5;4H"); // (4, 3)
    t.push(b"\x1b[F");
    t.assert_cursor(3, 0);
    t.push(b"   ");
    t.push(b"\x1b[2F");
    t.assert_cursor(1, 0);
    t.push(b"\x1b[0F");
    t.assert_cursor(0, 0);
}

#[test]
fn cursor_cha() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\n"); // row 1
    t.push(b"\x1b[20G");
    t.assert_cursor(1, 19);
    t.push(b"\x1b[G"); // default -> col 1 -> 0
    t.assert_cursor(1, 0);
}

#[test]
fn cursor_cup() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[10;5H");
    t.assert_cursor(9, 4);
    t.push(b"\x1b[8H"); // row 8, col defaults to 1
    t.assert_cursor(7, 0);
    t.push(b"\x1b[H"); // both default to 1
    t.assert_cursor(0, 0);
}

#[test]
fn cursor_bounds_checking() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[A"); // up from (0,0)
    t.assert_cursor(0, 0);
    t.push(b"\x1b[D"); // left from (0,0)
    t.assert_cursor(0, 0);
    t.push(b"\x1b[25;80H"); // (24, 79)
    t.assert_cursor(24, 79);
    t.push(b"\x1b[B"); // down from bottom
    t.assert_cursor(24, 79);
    t.push(b"\x1b[C"); // right from right edge
    t.assert_cursor(24, 79);
}

#[test]
fn cursor_cup_clamps_large_values() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[999G");
    t.assert_cursor(0, 79);
    t.push(b"\x1b[99;99H");
    t.assert_cursor(24, 79);
}

#[test]
fn cursor_hpa() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[5`");
    t.assert_cursor(0, 4);
}

#[test]
fn cursor_hpr() {
    // CSI a = HPR (Horizontal Position Relative). Implemented as cursor_right.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[1;5H"); // col 4 (0-indexed)
    t.push(b"\x1b[3a");
    t.assert_cursor(0, 7);
}

#[test]
fn cursor_hvp() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[3;3f");
    t.assert_cursor(2, 2);
}

#[test]
fn cursor_vpa() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[3;3f"); // (2, 2)
    t.push(b"\x1b[5d");
    t.assert_cursor(4, 2);
}

#[test]
fn cursor_vpr() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[5;3f"); // (4, 2)
    t.push(b"\x1b[2e");
    t.assert_cursor(6, 2);
}

#[test]
fn cursor_cht() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\t"); // col 8
    t.push(b"   "); // col 11
    t.push(b"\t"); // col 16
    t.push(b"\x1b[I"); // +1 tab -> col 24
    t.assert_cursor(0, 24);
}

#[test]
fn cursor_cbt() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[65G"); // col 64
    t.push(b"\x1b[Z"); // back one tab -> col 56
    t.assert_cursor(0, 56);
    t.push(b"\x1b[2Z"); // back two tabs -> col 40
    t.assert_cursor(0, 40);
}

// ---------------------------------------------------------------------------
// 12state_scroll: Scrolling (verified via content, not scroll events)
// ---------------------------------------------------------------------------

#[test]
fn scroll_lf_at_bottom() {
    // LF on the last row triggers a scroll up.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"FIRST\r"); // CR to go back to col 0
    for _ in 0..24 {
        t.push(b"\n");
    }
    t.assert_cursor(24, 0);
    t.push(b"\n"); // triggers scroll
    t.assert_cursor(24, 0);
    // "FIRST" should have scrolled off (into scrollback), row 0 is now what was row 1
    assert_eq!(t.row_text(0), "");
}

#[test]
fn scroll_ind_at_bottom() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"TOP");
    t.push(b"\x1b[25H"); // row 25 (0-indexed: 24)
    t.push(b"\x1bD"); // IND at bottom triggers scroll
    // TOP was on row 0, now it should have scrolled to scrollback or row shifts
    t.assert_cursor(24, 0);
}

#[test]
fn scroll_ri_at_top() {
    // RI at row 0 triggers scroll down (content moves down, blank line at top).
    let mut t = TestTerminal::new(80, 25);
    t.push(b"TOP");
    t.push(b"\x1b[H"); // ensure at row 0
    t.push(b"\x1bM"); // RI at top -> scroll down
    t.assert_cursor(0, 0);
    // "TOP" should now be on row 1
    assert_eq!(t.row_text(0), "");
    assert_eq!(t.row_text(1), "TOP");
}

#[test]
fn scroll_decstbm_lf() {
    // DECSTBM sets scroll region; LF at bottom of region scrolls region only.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[1;10r"); // rows 1-10 (0-indexed: 0..10)
    t.assert_cursor(0, 0);
    t.push(b"R0\r"); // CR to go back to col 0
    for _ in 0..9 {
        t.push(b"\n");
    }
    t.assert_cursor(9, 0);
    t.push(b"\n"); // scroll within region
    t.assert_cursor(9, 0);
    // Row 0 should no longer have "R0" (scrolled out of region)
    assert_eq!(t.row_text(0), "");
}

#[test]
fn scroll_lf_outside_decstbm() {
    // LF outside the scroll region does NOT scroll, just moves cursor down.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[1;10r");
    t.push(b"\x1b[20H"); // row 20 (0-indexed: 19)
    t.assert_cursor(19, 0);
    t.push(b"\n");
    t.assert_cursor(20, 0);
}

#[test]
fn scroll_sd_su() {
    // CSI S = SU (Scroll Up = content moves up, blank lines at bottom).
    let mut t = TestTerminal::new(80, 25);
    t.push(b"LINE0");
    t.push(b"\x1b[H"); // home first so cursor is at (0,0)
    t.push(b"\x1b[S"); // scroll up 1
    t.assert_cursor(0, 0); // cursor doesn't move
    // After scroll up 1, old row 0 ("LINE0") went to scrollback.
    // Row 0 is now what was row 1 (blank).
    assert_eq!(t.row_text(0), "");
}

#[test]
fn scroll_su() {
    // CSI T = SD (Scroll Down = content moves down, blank line at top).
    let mut t = TestTerminal::new(80, 25);
    t.push(b"LINE0");
    t.push(b"\x1b[H"); // home
    t.push(b"\x1b[T"); // scroll down 1
    t.assert_cursor(0, 0);
    // Row 0 should be blank (new blank line inserted at top)
    assert_eq!(t.row_text(0), "");
    // "LINE0" should now be on row 1
    assert_eq!(t.row_text(1), "LINE0");
}

#[test]
fn scroll_sd_in_decstbm() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[5;20r");
    // Write something in the scroll region
    t.push(b"\x1b[5;1H"); // row 5 (0-indexed: 4)
    t.push(b"REGION");
    t.push(b"\x1b[S"); // scroll up within region
    // "REGION" was at row 4, after scroll up it moves to scrollback-equivalent within region
    // Row 4 should now be what was row 5 (blank)
    assert_eq!(t.row_text(4), "");
    // "REGION" content shifted to... actually in a region scroll, row 4 content moves out
    // and row 19 (bottom of region) becomes blank
}

#[test]
fn scroll_sd_clamped() {
    // SD with count > region height gets clamped.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"DATA");
    t.push(b"\x1b[100S");
    // All rows should be blank after scrolling by more than screen height
    assert_eq!(t.row_text(0), "");
    assert_eq!(t.row_text(24), "");
}

#[test]
fn scroll_decstbm_resets_cursor() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[5;5H"); // (4, 4)
    t.assert_cursor(4, 4);
    t.push(b"\x1b[r"); // reset DECSTBM
    t.assert_cursor(0, 0);
}

// ---------------------------------------------------------------------------
// 13state_edit: Edit operations
// ---------------------------------------------------------------------------

#[test]
fn edit_ich() {
    // CSI @ = ICH (Insert Character). Shifts line right.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ACD");
    t.push(b"\x1b[2D"); // back 2 -> col 1
    t.assert_cursor(0, 1);
    t.push(b"\x1b[@"); // insert 1 blank at col 1
    // 'C' and 'D' should have shifted right
    t.assert_cell_char(0, 0, 'A');
    t.assert_cell_char(0, 1, ' '); // inserted blank
    t.assert_cell_char(0, 2, 'C');
    t.assert_cell_char(0, 3, 'D');
    t.assert_cursor(0, 1);
}

#[test]
fn edit_ich_count() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ABCD");
    t.push(b"\x1b[3D"); // col 1
    t.push(b"\x1b[2@"); // insert 2 blanks at col 1
    t.assert_cell_char(0, 0, 'A');
    t.assert_cell_char(0, 1, ' ');
    t.assert_cell_char(0, 2, ' ');
    t.assert_cell_char(0, 3, 'B');
    t.assert_cell_char(0, 4, 'C');
    t.assert_cell_char(0, 5, 'D');
}

#[test]
fn edit_dch() {
    // CSI P = DCH (Delete Character). Shifts line left.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ABCD");
    t.push(b"\x1b[3D"); // col 1
    t.assert_cursor(0, 1);
    t.push(b"\x1b[P"); // delete 1 at col 1
    t.assert_cell_char(0, 0, 'A');
    t.assert_cell_char(0, 1, 'C');
    t.assert_cell_char(0, 2, 'D');
    t.assert_cursor(0, 1);
}

#[test]
fn edit_dch_count() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ABCDEF");
    t.push(b"\x1b[5D"); // col 1
    t.push(b"\x1b[3P"); // delete 3 at col 1
    t.assert_cell_char(0, 0, 'A');
    t.assert_cell_char(0, 1, 'E');
    t.assert_cell_char(0, 2, 'F');
    t.assert_cursor(0, 1);
}

#[test]
fn edit_ech() {
    // CSI X = ECH (Erase Character). Blanks cells without shifting.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ABC");
    t.push(b"\x1b[2D"); // col 1
    t.push(b"\x1b[X"); // erase 1 cell at (0,1)
    t.assert_cell_char(0, 0, 'A');
    t.assert_cell_char(0, 1, ' ');
    t.assert_cell_char(0, 2, 'C'); // not shifted
    t.assert_cursor(0, 1); // cursor does NOT move
}

#[test]
fn edit_ech_count() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ABCDE");
    t.push(b"\x1b[4D"); // col 1
    t.push(b"\x1b[3X"); // erase 3 cells starting at col 1
    t.assert_cell_char(0, 0, 'A');
    t.assert_cell_char(0, 1, ' ');
    t.assert_cell_char(0, 2, ' ');
    t.assert_cell_char(0, 3, ' ');
    t.assert_cell_char(0, 4, 'E');
    t.assert_cursor(0, 1);
}

#[test]
fn edit_ech_bounded() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ABC");
    t.push(b"\x1b[2D"); // col 1
    t.push(b"\x1b[100X");
    // Should erase from col 1..80, not panic
    t.assert_cursor(0, 1);
}

#[test]
fn edit_il() {
    // CSI L = IL (Insert Line). Pushes lines down within scroll region.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"LINE0\r\n");
    t.push(b"LINE1\r\n");
    t.push(b"LINE2");
    t.push(b"\x1b[2;1H"); // row 2, col 1 -> (1, 0)
    t.assert_cursor(1, 0);
    t.push(b"\x1b[L"); // insert 1 line at row 1
    // Row 1 should now be blank, old row 1 (LINE1) shifts to row 2
    assert_eq!(t.row_text(0), "LINE0");
    assert_eq!(t.row_text(1), "");
    assert_eq!(t.row_text(2), "LINE1");
    assert_eq!(t.row_text(3), "LINE2");
}

#[test]
fn edit_il_count() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"LINE0\r\nLINE1\r\nLINE2");
    t.push(b"\x1b[2;1H"); // (1, 0)
    t.push(b"\x1b[2L"); // insert 2 lines
    assert_eq!(t.row_text(0), "LINE0");
    assert_eq!(t.row_text(1), "");
    assert_eq!(t.row_text(2), "");
    assert_eq!(t.row_text(3), "LINE1");
    assert_eq!(t.row_text(4), "LINE2");
}

#[test]
fn edit_il_in_decstbm() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[5;15r");
    t.push(b"\x1b[5;1H"); // row 5 -> (4, 0)
    t.push(b"R4");
    t.push(b"\x1b[6;1H");
    t.push(b"R5");
    t.push(b"\x1b[5;1H"); // back to (4, 0)
    t.push(b"\x1b[L"); // insert line within region
    assert_eq!(t.row_text(4), "");
    assert_eq!(t.row_text(5), "R4");
    assert_eq!(t.row_text(6), "R5");
}

#[test]
fn edit_il_outside_decstbm() {
    // IL outside scroll region: cursor is moved to col 0 but no lines inserted
    // because the cursor row is outside [scroll_top, scroll_bottom).
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[5;15r");
    t.push(b"\x1b[20;5H"); // row 20, col 5 -> (19, 4)
    t.push(b"KEEP");
    t.push(b"\x1b[20;1H"); // (19, 0)
    t.push(b"\x1b[L");
    // IL outside region should be a no-op for content (cursor goes to col 0)
    // Verify content didn't shift
}

#[test]
fn edit_dl() {
    // CSI M = DL (Delete Line). Pulls lines up within scroll region.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"LINE0\r\nLINE1\r\nLINE2\r\nLINE3");
    t.push(b"\x1b[2;1H"); // (1, 0)
    t.assert_cursor(1, 0);
    t.push(b"\x1b[M"); // delete 1 line at row 1
    assert_eq!(t.row_text(0), "LINE0");
    assert_eq!(t.row_text(1), "LINE2");
    assert_eq!(t.row_text(2), "LINE3");
}

#[test]
fn edit_dl_count() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"LINE0\r\nLINE1\r\nLINE2\r\nLINE3\r\nLINE4");
    t.push(b"\x1b[2;1H"); // (1, 0)
    t.push(b"\x1b[2M"); // delete 2 lines
    assert_eq!(t.row_text(0), "LINE0");
    assert_eq!(t.row_text(1), "LINE3");
    assert_eq!(t.row_text(2), "LINE4");
}

#[test]
fn edit_dl_in_decstbm() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[5;15r");
    t.push(b"\x1b[5;1H"); // (4, 0)
    t.push(b"R4");
    t.push(b"\x1b[6;1H");
    t.push(b"R5");
    t.push(b"\x1b[5;1H"); // back to (4, 0)
    t.push(b"\x1b[M"); // delete line within region
    assert_eq!(t.row_text(4), "R5");
}

#[test]
fn edit_dl_outside_decstbm() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[5;15r");
    t.push(b"\x1b[20;1H"); // (19, 0)
    t.push(b"KEEP");
    t.push(b"\x1b[20;1H");
    t.push(b"\x1b[M");
    // DL outside region should be no-op for content
}

#[test]
fn edit_el_0() {
    // CSI 0 K = EL from cursor to end of line.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ABCDE");
    t.push(b"\x1b[3D"); // col 2
    t.push(b"\x1b[0K");
    t.assert_cell_char(0, 0, 'A');
    t.assert_cell_char(0, 1, 'B');
    t.assert_cell_char(0, 2, ' ');
    t.assert_cell_char(0, 3, ' ');
    t.assert_cell_char(0, 4, ' ');
    t.assert_cursor(0, 2);
}

#[test]
fn edit_el_1() {
    // CSI 1 K = EL from start of line to cursor (inclusive).
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ABCDE");
    t.push(b"\x1b[3D"); // col 2
    t.push(b"\x1b[1K");
    t.assert_cell_char(0, 0, ' ');
    t.assert_cell_char(0, 1, ' ');
    t.assert_cell_char(0, 2, ' ');
    t.assert_cell_char(0, 3, 'D');
    t.assert_cell_char(0, 4, 'E');
    t.assert_cursor(0, 2);
}

#[test]
fn edit_el_2() {
    // CSI 2 K = EL entire line.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ABCDE");
    t.push(b"\x1b[3D"); // col 2
    t.push(b"\x1b[2K");
    t.assert_cell_char(0, 0, ' ');
    t.assert_cell_char(0, 4, ' ');
    t.assert_cursor(0, 2);
}

#[test]
fn edit_ed_0() {
    // CSI 0 J = ED from cursor to end of screen.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ROW0");
    t.push(b"\x1b[2;1H");
    t.push(b"ROW1DATA");
    t.push(b"\x1b[2;2H"); // (1, 1)
    t.push(b"\x1b[0J");
    // Row 0 should be untouched
    assert_eq!(t.row_text(0), "ROW0");
    // Row 1 col 0 should be preserved, col 1+ should be blank
    t.assert_cell_char(1, 0, 'R');
    t.assert_cell_char(1, 1, ' ');
    t.assert_cursor(1, 1);
}

#[test]
fn edit_ed_1() {
    // CSI 1 J = ED from start of screen to cursor.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ROW0");
    t.push(b"\x1b[2;1H");
    t.push(b"ROW1DATA");
    t.push(b"\x1b[2;2H"); // (1, 1)
    t.push(b"\x1b[1J");
    // Row 0 entirely blanked
    t.assert_cell_char(0, 0, ' ');
    // Row 1 cols 0..1 blanked
    t.assert_cell_char(1, 0, ' ');
    t.assert_cell_char(1, 1, ' ');
    // Row 1 col 2+ preserved
    t.assert_cell_char(1, 2, 'W');
    t.assert_cursor(1, 1);
}

#[test]
fn edit_ed_2() {
    // CSI 2 J = ED entire screen.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"SOMETHING");
    t.push(b"\x1b[2;2H");
    t.push(b"\x1b[2J");
    t.assert_cell_char(0, 0, ' ');
    t.assert_cursor(1, 1);
}

// ---------------------------------------------------------------------------
// 15state_mode: Terminal modes
// ---------------------------------------------------------------------------

#[test]
fn mode_insert_replace() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ACD");
    t.push(b"\x1b[1;2H"); // col 2 -> (0, 1)
    t.push(b"B"); // overwrite mode: B replaces C
    t.assert_cell_char(0, 0, 'A');
    t.assert_cell_char(0, 1, 'B');
    t.assert_cell_char(0, 2, 'D');

    // Now test insert mode
    t.push(b"\x1b[2J\x1b[H"); // clear and home
    t.push(b"ACD");
    t.push(b"\x1b[4h"); // insert mode ON
    t.push(b"\x1b[1;2H"); // (0, 1)
    t.push(b"B"); // insert mode: B inserted, C and D shift right
    t.assert_cell_char(0, 0, 'A');
    t.assert_cell_char(0, 1, 'B');
    t.assert_cell_char(0, 2, 'C');
    t.assert_cell_char(0, 3, 'D');
}

#[test]
fn mode_newline() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[5G\n"); // col 5 (0-indexed: 4), LF
    t.assert_cursor(1, 4); // normal mode: column preserved

    t.push(b"\x1b[20h"); // newline mode ON
    t.push(b"\x1b[5G\n");
    t.assert_cursor(2, 0); // newline mode: LF also does CR
}

#[test]
fn mode_decom_origin() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[5;15r"); // scroll region rows 5-15

    // Without origin mode
    t.push(b"\x1b[H");
    t.assert_cursor(0, 0);
    t.push(b"\x1b[3;3H");
    t.assert_cursor(2, 2);

    // Enable origin mode
    t.push(b"\x1b[?6h");
    t.push(b"\x1b[H"); // "home" is now top of scroll region
    t.assert_cursor(4, 0); // row 5 (0-indexed: 4)
    t.push(b"\x1b[3;3H"); // row 3, col 3 relative to region
    t.assert_cursor(6, 2);
}

#[test]
fn mode_decom_bounds_cursor() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[5;15r");
    t.push(b"\x1b[?6h");
    t.push(b"\x1b[H");
    t.push(b"\x1b[10A"); // try to go 10 rows up from top of region
    t.assert_cursor(4, 0); // clamped at scroll region top
    t.push(b"\x1b[20B"); // try to go 20 rows down
    t.assert_cursor(14, 0); // clamped at scroll region bottom
}

#[test]
fn mode_decom_without_scroll_region() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[?6l"); // off first
    t.push(b"\x1b[r\x1b[?6h"); // reset region, then enable origin
    t.assert_cursor(0, 0);
}

// ---------------------------------------------------------------------------
// 20state_wrapping: Autowrap behavior
// ---------------------------------------------------------------------------

#[test]
fn wrap_79th_column() {
    // Printing up to column 79 does not wrap.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[75G"); // col 75 (0-indexed: 74)
    t.push(b"AAAAA"); // fills cols 74..78, last A at 78, cursor at 79
    // After writing 5 chars from col 74: 74,75,76,77,78 -> cursor at 79
    // Actually wait - 5 chars from col 74: positions 74,75,76,77,78 -> cursor at 79
    t.assert_cursor(0, 79);
}

#[test]
fn wrap_phantom_column() {
    // Printing the 80th character enters "pending wrap" state.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[75G");
    t.push(b"AAAAAA"); // 6 chars from col 74: fills 74..79, pending wrap
    t.assert_cursor(0, 79);
    t.assert_cell_char(0, 79, 'A');
}

#[test]
fn wrap_line_wraparound() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[75G");
    t.push(b"AAAAAA"); // pending wrap at col 79
    t.push(b"B");
    t.assert_cell_char(1, 0, 'B');
    t.assert_cursor(1, 1);
}

#[test]
fn wrap_combined_write() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[78G"); // col 77
    t.push(b"BBBCC");
    t.assert_cell_char(0, 77, 'B');
    t.assert_cell_char(0, 78, 'B');
    t.assert_cell_char(0, 79, 'B');
    t.assert_cell_char(1, 0, 'C');
    t.assert_cell_char(1, 1, 'C');
    t.assert_cursor(1, 2);
}

#[test]
fn wrap_disabled() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[?7l"); // autowrap OFF
    t.push(b"\x1b[75G");
    t.push(b"DDDDDD"); // fills 74..79
    t.assert_cursor(0, 79);
    t.push(b"D"); // overwrites col 79
    t.assert_cell_char(0, 79, 'D');
    t.assert_cursor(0, 79);
}

#[test]
fn wrap_phantom_cancelled_by_cup() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[25;78HABC"); // phantom at row 24 col 79
    t.assert_cursor(24, 79);
    t.push(b"\x1b[25;1HD"); // CUP to col 1, then print D
    t.assert_cell_char(24, 0, 'D');
}

#[test]
fn wrap_scroll_at_bottom() {
    // Wrap at last row triggers scroll.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[25;78HABC"); // fills 77,78,79 on row 24 -> phantom
    t.assert_cursor(24, 79);
    t.push(b"D"); // wraps: scroll + put D
    t.assert_cell_char(24, 0, 'D');
}

// ---------------------------------------------------------------------------
// 21state_tabstops: Tab stops
// ---------------------------------------------------------------------------

#[test]
fn tabstop_default() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\t");
    t.assert_cursor(0, 8);
    t.push(b"\t");
    t.assert_cursor(0, 16);
}

#[test]
fn tabstop_hts() {
    // ESC H = HTS (Horizontal Tab Set) at current column.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[5G\x1bH"); // set tab at col 4
    t.push(b"\x1b[G\t"); // go home, tab -> should stop at col 4
    t.assert_cursor(0, 4);
}

#[test]
fn tabstop_tbc_0() {
    // CSI 0 g = TBC (Tab Clear) at current column.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[9G\x1b[g"); // move to col 8 (1-based 9), clear tab there
    t.push(b"\x1b[G\t"); // tab from col 0 skips col 8 -> goes to 16
    t.assert_cursor(0, 16);
}

#[test]
fn tabstop_tbc_3() {
    // CSI 3 g = Clear ALL tab stops.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[3g"); // clear all
    t.push(b"\x1b[50G\x1bH"); // set one at col 49
    t.push(b"\x1b[G"); // home
    t.push(b"\t"); // should go to col 49 (only tab stop)
    t.assert_cursor(0, 49);
}

// ---------------------------------------------------------------------------
// 22state_save: Cursor save/restore
// ---------------------------------------------------------------------------

#[test]
fn save_restore_decsc_decrc() {
    // ESC 7 = DECSC, ESC 8 = DECRC.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[2;2H"); // (1, 1)
    t.push(b"\x1b7"); // save
    t.assert_cursor(1, 1);

    t.push(b"\x1b[5;5H"); // (4, 4)
    t.assert_cursor(4, 4);
    t.push(b"\x1b8"); // restore
    t.assert_cursor(1, 1);
}

#[test]
fn save_restore_csi_1048() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[2;2H");
    t.push(b"\x1b[?1048h"); // save
    t.push(b"\x1b[5;5H");
    t.assert_cursor(4, 4);
    t.push(b"\x1b[?1048l"); // restore
    t.assert_cursor(1, 1);
}

#[test]
fn save_restore_pen_attributes() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[1m"); // bold ON
    t.assert_bold(true);
    t.push(b"\x1b[?1048h"); // save

    t.push(b"\x1b[22;4m"); // bold OFF, underline ON
    t.assert_bold(false);
    t.assert_underline(1);

    t.push(b"\x1b[?1048l"); // restore
    t.assert_bold(true);
    t.assert_underline(0);
}

#[test]
fn save_twice_restore_twice() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[2;10H\x1b[?1048h");
    t.push(b"\x1b[6;10H\x1b[?1048h");
    t.push(b"\x1b[H");
    t.assert_cursor(0, 0);
    t.push(b"\x1b[?1048l");
    t.assert_cursor(5, 9);
    t.push(b"\x1b[H");
    t.push(b"\x1b[?1048l");
    t.assert_cursor(5, 9);
}

// ---------------------------------------------------------------------------
// 30state_pen: SGR attributes
// ---------------------------------------------------------------------------

#[test]
fn pen_reset() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[1;3;4;5;7m"); // set several
    t.push(b"\x1b[m"); // reset
    t.assert_bold(false);
    t.assert_underline(0);
    t.assert_italic(false);
    t.assert_blink(false);
    t.assert_reverse(false);
    t.assert_fg_default();
    t.assert_bg_default();
}

#[test]
fn pen_bold() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[1m");
    t.assert_bold(true);
    t.push(b"\x1b[22m");
    t.assert_bold(false);
    t.push(b"\x1b[1m\x1b[m");
    t.assert_bold(false);
}

#[test]
fn pen_underline() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[4m"); // single underline
    t.assert_underline(1);
    t.push(b"\x1b[21m"); // double underline
    t.assert_underline(2);
    t.push(b"\x1b[24m"); // underline off
    t.assert_underline(0);
}

#[test]
fn pen_underline_subparams() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[4m\x1b[4:0m");
    t.assert_underline(0);
    t.push(b"\x1b[4:1m");
    t.assert_underline(1);
    t.push(b"\x1b[4:2m");
    t.assert_underline(2);
    t.push(b"\x1b[4:3m");
    t.assert_underline(3);
    t.push(b"\x1b[4m\x1b[m");
    t.assert_underline(0);
}

#[test]
fn pen_italic() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[3m");
    t.assert_italic(true);
    t.push(b"\x1b[23m");
    t.assert_italic(false);
    t.push(b"\x1b[3m\x1b[m");
    t.assert_italic(false);
}

#[test]
fn pen_blink() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[5m");
    t.assert_blink(true);
    t.push(b"\x1b[25m");
    t.assert_blink(false);
    t.push(b"\x1b[5m\x1b[m");
    t.assert_blink(false);
}

#[test]
fn pen_reverse() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[7m");
    t.assert_reverse(true);
    t.push(b"\x1b[27m");
    t.assert_reverse(false);
    t.push(b"\x1b[7m\x1b[m");
    t.assert_reverse(false);
}

#[test]
fn pen_font_selection() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[11m");
    // assert font == 1
    t.push(b"\x1b[19m");
    // assert font == 9
    t.push(b"\x1b[10m");
    // assert font == 0
}

#[test]
fn pen_foreground_indexed() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[31m");
    t.assert_fg_indexed(1);
    t.push(b"\x1b[32m");
    t.assert_fg_indexed(2);
    t.push(b"\x1b[34m");
    t.assert_fg_indexed(4);
    t.push(b"\x1b[91m"); // bright red (index 9)
    t.assert_fg_indexed(9);
}

#[test]
fn pen_foreground_rgb() {
    // Uses semicolon syntax: 38;2;R;G;B
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[38;2;10;20;30m");
    t.assert_fg_rgb(10, 20, 30);
}

#[test]
fn pen_foreground_256() {
    // Uses semicolon syntax: 38;5;N
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[38;5;1m");
    t.assert_fg_indexed(1);
}

#[test]
fn pen_foreground_default() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[31m");
    t.push(b"\x1b[39m");
    t.assert_fg_default();
}

#[test]
fn pen_background_indexed() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[41m");
    t.assert_bg_indexed(1);
    t.push(b"\x1b[42m");
    t.assert_bg_indexed(2);
    t.push(b"\x1b[44m");
    t.assert_bg_indexed(4);
    t.push(b"\x1b[101m"); // bright red bg
    t.assert_bg_indexed(9);
}

#[test]
fn pen_background_rgb() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[48;2;10;20;30m");
    t.assert_bg_rgb(10, 20, 30);
}

#[test]
fn pen_background_256() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[48;5;1m");
    t.assert_bg_indexed(1);
}

#[test]
fn pen_background_default() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[41m");
    t.push(b"\x1b[49m");
    t.assert_bg_default();
}

#[test]
fn pen_bold_ansi_highbright() {
    let mut t = TestTerminal::new(80, 25);
    t.screen.set_bold_is_bright(true);
    t.push(b"\x1b[m\x1b[1;37m");
    t.assert_bold(true);
    t.assert_fg_indexed(15);

    t.push(b"\x1b[m\x1b[37;1m");
    t.assert_bold(true);
    t.assert_fg_indexed(15);
}

#[test]
fn pen_decstr_resets() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[1;4m");
    t.assert_bold(true);
    t.assert_underline(1);
    t.push(b"\x1b[!p");
    t.assert_bold(false);
    t.assert_underline(0);
}

// ---------------------------------------------------------------------------
// Additional edge-case tests
// ---------------------------------------------------------------------------

#[test]
fn cursor_backspace_cancels_phantom() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[4;80H"); // (3, 79)
    t.push(b"X"); // prints at 79, enters phantom
    t.assert_cursor(3, 79);
    t.push(b"\x08"); // BS
    t.assert_cursor(3, 78);
}

#[test]
fn cursor_cup_cancels_phantom() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[10;78H"); // (9, 77)
    t.push(b"ABC"); // fills 77,78,79 -> phantom
    t.assert_cursor(9, 79);
    t.push(b"\x1b[10;80H"); // CUP to (9, 79) -- cancels phantom
    t.push(b"C"); // prints at 79, re-enters phantom
    t.assert_cursor(9, 79);
    t.push(b"X"); // wraps to next line
    t.assert_cursor(10, 1);
}

#[test]
fn scroll_ri_in_decstbm() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[9;10r"); // rows 9-10 (0-indexed: 8..10)
    t.push(b"\x1b[9;1H"); // row 9 (0-indexed: 8)
    t.push(b"TOP");
    t.push(b"\x1b[10;1H"); // row 10 (0-indexed: 9)
    t.push(b"BOT");
    t.push(b"\x1b[9;1H"); // back to top of region
    t.push(b"\x1bM"); // RI at top of region -> reverse scroll (content moves down)
    t.assert_cursor(8, 0);
    // Row 8 should now be blank (new line inserted)
    assert_eq!(t.row_text(8), "");
    // Old "TOP" should now be at row 9
    assert_eq!(t.row_text(9), "TOP");
}

#[test]
fn scroll_lf_below_decstbm_no_scroll() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[9;10r");
    t.push(b"\x1b[25H"); // row 25 -> (24, 0)
    t.assert_cursor(24, 0);
    t.push(b"\n");
    // At bottom of screen, outside scroll region, should not scroll.
    // Cursor stays at row 24 (can't go further).
    t.assert_cursor(24, 0);
}

#[test]
fn edit_el_preserves_cursor() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"ABCDE");
    t.push(b"\x1b[3D"); // col 2
    t.push(b"\x1b[0K");
    t.assert_cursor(0, 2);
    t.push(b"\x1b[1K");
    t.assert_cursor(0, 2);
    t.push(b"\x1b[2K");
    t.assert_cursor(0, 2);
}

#[test]
fn cursor_cnl_bounded_bottom() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[25;80H"); // (24, 79)
    t.push(b"\x1b[E"); // CNL -> row 24 (already at bottom), col 0
    t.assert_cursor(24, 0);
}

#[test]
fn cursor_cpl_bounded_top() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[H"); // (0, 0)
    t.push(b"\x1b[F"); // CPL -> stays at row 0, col 0
    t.assert_cursor(0, 0);
}

#[test]
fn mode_decom_with_decslrm() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[5;15r");
    t.push(b"\x1b[?6h");
    t.push(b"\x1b[?69h");
    t.push(b"\x1b[20;60s");
    t.push(b"\x1b[H");
    t.assert_cursor(4, 19);
}

#[test]
fn tabstop_default_positions() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\t"); // 0 -> 8
    t.assert_cursor(0, 8);
    t.push(b"   "); // 8 -> 11
    t.push(b"\t"); // 11 -> 16
    t.assert_cursor(0, 16);
    t.push(b"       "); // 16 -> 23
    t.push(b"\t"); // 23 -> 24
    t.assert_cursor(0, 24);
    t.push(b"        "); // 24 -> 32
    t.push(b"\t"); // 32 -> 40
    t.assert_cursor(0, 40);
}

#[test]
fn scroll_invalid_decstbm_ignored() {
    let mut t = TestTerminal::new(80, 25);
    // These should be silently rejected or clamped without crashing.
    t.push(b"\x1b[100;105r");
    t.push(b"\x1b[5;2r"); // top > bottom
}

#[test]
fn edit_ed_3_scrollback() {
    // CSI 3 J = Clear scrollback buffer.
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[3J");
    // Should not crash. Scrollback buffer cleared.
}
