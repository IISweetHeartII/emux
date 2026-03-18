//! Integration tests: Screen + Input encoding.
//!
//! These tests verify that the Screen's mode state (set via CSI sequences)
//! correctly influences the input encoding functions. This bridges the
//! VT parser → Screen → input encoding pipeline.

mod common;
use common::TestTerminal;

use emux_term::input::{encode_key, encode_paste, Key, Modifiers};

// ---------------------------------------------------------------------------
// 1. Screen + Input: application cursor keys via CSI ?1h / ?1l
// ---------------------------------------------------------------------------

#[test]
fn screen_decckm_enables_application_cursor_keys() {
    // Send CSI ?1h (DECCKM set) to the screen, then verify encode_key
    // uses the application_cursor_keys mode flag from screen.modes.
    let mut t = TestTerminal::new(80, 25);

    // Default: application_cursor_keys is off
    assert!(!t.screen.modes.application_cursor_keys);
    let up = encode_key(
        Key::Up,
        Modifiers::none(),
        t.screen.modes.application_cursor_keys,
        false,
        false,
        false,
    );
    assert_eq!(up, b"\x1b[A", "normal mode: Up should be ESC [ A");

    // Enable DECCKM via CSI sequence
    t.push(b"\x1b[?1h");
    assert!(
        t.screen.modes.application_cursor_keys,
        "CSI ?1h should enable application_cursor_keys"
    );

    // Now encode_key should produce application cursor sequence
    let up_app = encode_key(
        Key::Up,
        Modifiers::none(),
        t.screen.modes.application_cursor_keys,
        false,
        false,
        false,
    );
    assert_eq!(up_app, b"\x1bOA", "app mode: Up should be ESC O A");

    let down_app = encode_key(
        Key::Down,
        Modifiers::none(),
        t.screen.modes.application_cursor_keys,
        false,
        false,
        false,
    );
    assert_eq!(down_app, b"\x1bOB", "app mode: Down should be ESC O B");

    let right_app = encode_key(
        Key::Right,
        Modifiers::none(),
        t.screen.modes.application_cursor_keys,
        false,
        false,
        false,
    );
    assert_eq!(right_app, b"\x1bOC", "app mode: Right should be ESC O C");

    let left_app = encode_key(
        Key::Left,
        Modifiers::none(),
        t.screen.modes.application_cursor_keys,
        false,
        false,
        false,
    );
    assert_eq!(left_app, b"\x1bOD", "app mode: Left should be ESC O D");
}

#[test]
fn screen_decckm_disable_restores_normal_cursor_keys() {
    let mut t = TestTerminal::new(80, 25);

    // Enable then disable
    t.push(b"\x1b[?1h");
    assert!(t.screen.modes.application_cursor_keys);
    t.push(b"\x1b[?1l");
    assert!(
        !t.screen.modes.application_cursor_keys,
        "CSI ?1l should disable application_cursor_keys"
    );

    let up = encode_key(
        Key::Up,
        Modifiers::none(),
        t.screen.modes.application_cursor_keys,
        false,
        false,
        false,
    );
    assert_eq!(up, b"\x1b[A", "after DECCKM reset, Up should be ESC [ A");
}

#[test]
fn screen_decckm_modified_keys_still_use_csi_in_app_mode() {
    // Even in application cursor mode, modified keys use CSI format
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[?1h");

    let shift_up = encode_key(
        Key::Up,
        Modifiers::shift(),
        t.screen.modes.application_cursor_keys,
        false,
        false,
        false,
    );
    assert_eq!(
        shift_up, b"\x1b[1;2A",
        "Shift+Up in app mode should still use CSI"
    );
}

// ---------------------------------------------------------------------------
// 2. Screen + Input: newline mode via CSI 20h / 20l (LNM)
// ---------------------------------------------------------------------------

#[test]
fn screen_newline_mode_affects_enter_encoding() {
    let mut t = TestTerminal::new(80, 25);

    // Default: newline mode off, Enter sends CR only
    assert!(!t.screen.modes.newline);
    let enter_cr = encode_key(
        Key::Enter,
        Modifiers::none(),
        false,
        false,
        t.screen.modes.newline,
        false,
    );
    assert_eq!(enter_cr, b"\x0d", "LNM off: Enter should send CR");

    // Enable LNM
    t.push(b"\x1b[20h");
    assert!(t.screen.modes.newline, "CSI 20h should enable newline mode");

    let enter_crlf = encode_key(
        Key::Enter,
        Modifiers::none(),
        false,
        false,
        t.screen.modes.newline,
        false,
    );
    assert_eq!(enter_crlf, b"\x0d\x0a", "LNM on: Enter should send CR+LF");

    // Disable LNM
    t.push(b"\x1b[20l");
    assert!(
        !t.screen.modes.newline,
        "CSI 20l should disable newline mode"
    );

    let enter_cr_again = encode_key(
        Key::Enter,
        Modifiers::none(),
        false,
        false,
        t.screen.modes.newline,
        false,
    );
    assert_eq!(
        enter_cr_again, b"\x0d",
        "after LNM reset, Enter should send CR"
    );
}

// ---------------------------------------------------------------------------
// 3. Screen + Paste: bracketed paste mode via CSI ?2004h / ?2004l
// ---------------------------------------------------------------------------

#[test]
fn screen_bracketed_paste_mode_wraps_paste() {
    let mut t = TestTerminal::new(80, 25);

    // Default: bracketed paste off
    assert!(!t.screen.modes.bracketed_paste);
    let paste_off = encode_paste("hello", t.screen.modes.bracketed_paste);
    assert_eq!(paste_off, b"hello", "without bracketed paste, text is raw");

    // Enable bracketed paste
    t.push(b"\x1b[?2004h");
    assert!(
        t.screen.modes.bracketed_paste,
        "CSI ?2004h should enable bracketed paste"
    );

    let paste_on = encode_paste("hello", t.screen.modes.bracketed_paste);
    assert_eq!(
        paste_on, b"\x1b[200~hello\x1b[201~",
        "with bracketed paste, text should be wrapped"
    );
}

#[test]
fn screen_bracketed_paste_disable_stops_wrapping() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[?2004h");
    t.push(b"\x1b[?2004l");
    assert!(
        !t.screen.modes.bracketed_paste,
        "CSI ?2004l should disable bracketed paste"
    );

    let paste = encode_paste("test", t.screen.modes.bracketed_paste);
    assert_eq!(paste, b"test", "after disable, paste should not be wrapped");
}

#[test]
fn screen_bracketed_paste_multiline_content() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[?2004h");

    let text = "line1\nline2\nline3";
    let paste = encode_paste(text, t.screen.modes.bracketed_paste);
    assert_eq!(
        paste,
        b"\x1b[200~line1\nline2\nline3\x1b[201~",
        "multiline paste should be wrapped"
    );
}

#[test]
fn screen_bracketed_paste_empty_string() {
    let mut t = TestTerminal::new(80, 25);
    t.push(b"\x1b[?2004h");

    let paste = encode_paste("", t.screen.modes.bracketed_paste);
    assert_eq!(
        paste, b"\x1b[200~\x1b[201~",
        "empty paste with bracketed mode should still produce brackets"
    );
}

// ---------------------------------------------------------------------------
// 4. Screen + Input: combined mode state pipeline
// ---------------------------------------------------------------------------

#[test]
fn screen_multiple_modes_coexist() {
    // Enable multiple modes at once and verify they all work independently
    let mut t = TestTerminal::new(80, 25);

    // Enable DECCKM, LNM, and bracketed paste together
    t.push(b"\x1b[?1h");    // DECCKM
    t.push(b"\x1b[20h");    // LNM
    t.push(b"\x1b[?2004h"); // bracketed paste

    assert!(t.screen.modes.application_cursor_keys);
    assert!(t.screen.modes.newline);
    assert!(t.screen.modes.bracketed_paste);

    // Verify each encoding uses the correct mode flag
    let up = encode_key(
        Key::Up,
        Modifiers::none(),
        t.screen.modes.application_cursor_keys,
        false,
        t.screen.modes.newline,
        false,
    );
    assert_eq!(up, b"\x1bOA", "DECCKM on: app cursor");

    let enter = encode_key(
        Key::Enter,
        Modifiers::none(),
        t.screen.modes.application_cursor_keys,
        false,
        t.screen.modes.newline,
        false,
    );
    assert_eq!(enter, b"\x0d\x0a", "LNM on: CR+LF");

    let paste = encode_paste("data", t.screen.modes.bracketed_paste);
    assert_eq!(paste, b"\x1b[200~data\x1b[201~", "bracketed paste on");
}

#[test]
fn screen_ris_resets_all_modes() {
    // RIS (ESC c) should reset all modes to defaults
    let mut t = TestTerminal::new(80, 25);

    // Enable various modes
    t.push(b"\x1b[?1h");    // DECCKM
    t.push(b"\x1b[?2004h"); // bracketed paste
    t.push(b"\x1b[20h");    // LNM

    assert!(t.screen.modes.application_cursor_keys);
    assert!(t.screen.modes.bracketed_paste);
    assert!(t.screen.modes.newline);

    // Full reset
    t.push(b"\x1bc");

    assert!(
        !t.screen.modes.application_cursor_keys,
        "RIS should reset DECCKM"
    );
    assert!(
        !t.screen.modes.bracketed_paste,
        "RIS should reset bracketed paste"
    );
    assert!(
        !t.screen.modes.newline,
        "RIS should reset LNM"
    );

    // Verify encoding reflects the reset
    let up = encode_key(
        Key::Up,
        Modifiers::none(),
        t.screen.modes.application_cursor_keys,
        false,
        false,
        false,
    );
    assert_eq!(up, b"\x1b[A", "after RIS, cursor keys should be normal");
}

// ---------------------------------------------------------------------------
// 5. Screen + Input: focus tracking mode via CSI ?1004h / ?1004l
// ---------------------------------------------------------------------------

#[test]
fn screen_focus_tracking_mode_toggle() {
    use emux_term::input::encode_focus;

    let mut t = TestTerminal::new(80, 25);
    assert!(!t.screen.modes.focus_tracking);

    // Focus events produce nothing when disabled
    let focus_in = encode_focus(true, t.screen.modes.focus_tracking);
    assert_eq!(focus_in, b"", "focus in with tracking off should be empty");

    // Enable focus tracking
    t.push(b"\x1b[?1004h");
    assert!(
        t.screen.modes.focus_tracking,
        "CSI ?1004h should enable focus tracking"
    );

    let focus_in = encode_focus(true, t.screen.modes.focus_tracking);
    assert_eq!(focus_in, b"\x1b[I", "focus in with tracking on");

    let focus_out = encode_focus(false, t.screen.modes.focus_tracking);
    assert_eq!(focus_out, b"\x1b[O", "focus out with tracking on");

    // Disable
    t.push(b"\x1b[?1004l");
    assert!(!t.screen.modes.focus_tracking);

    let focus_in = encode_focus(true, t.screen.modes.focus_tracking);
    assert_eq!(focus_in, b"", "after disable, focus events should be empty");
}
