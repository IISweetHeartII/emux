//! Integration tests for the emux main binary's key translation logic.
//!
//! The `translate_key` function in main.rs is a thin wrapper that maps
//! crossterm `KeyCode`/`KeyModifiers` to `emux_term::input::Key`/`Modifiers`
//! and then calls `encode_key`. We test the same encoding path here using
//! the public `emux_term::input` API directly, which exercises the exact
//! code path used by the event loop.

use emux_term::input::{encode_key, Key, Modifiers};

// ---------------------------------------------------------------------------
// Basic character keys
// ---------------------------------------------------------------------------

#[test]
fn translate_key_char_a() {
    let bytes = encode_key(Key::Char('a'), Modifiers::none(), false, false, false, false);
    assert_eq!(bytes, b"a");
}

#[test]
fn translate_key_char_z() {
    let bytes = encode_key(Key::Char('z'), Modifiers::none(), false, false, false, false);
    assert_eq!(bytes, b"z");
}

#[test]
fn translate_key_char_space() {
    let bytes = encode_key(Key::Char(' '), Modifiers::none(), false, false, false, false);
    assert_eq!(bytes, b" ");
}

#[test]
fn translate_key_char_unicode() {
    let bytes = encode_key(Key::Char('\u{4e16}'), Modifiers::none(), false, false, false, false);
    let expected = "\u{4e16}".as_bytes().to_vec();
    assert_eq!(bytes, expected);
}

// ---------------------------------------------------------------------------
// Enter / newline mode
// ---------------------------------------------------------------------------

#[test]
fn translate_key_enter() {
    let bytes = encode_key(Key::Enter, Modifiers::none(), false, false, false, false);
    assert_eq!(bytes, b"\r");
}

#[test]
fn translate_key_enter_newline_mode() {
    let bytes = encode_key(Key::Enter, Modifiers::none(), false, false, true, false);
    assert_eq!(bytes, b"\r\n");
}

// ---------------------------------------------------------------------------
// Arrow keys — normal mode
// ---------------------------------------------------------------------------

#[test]
fn translate_key_arrow_up() {
    let bytes = encode_key(Key::Up, Modifiers::none(), false, false, false, false);
    assert_eq!(bytes, b"\x1b[A");
}

#[test]
fn translate_key_arrow_down() {
    let bytes = encode_key(Key::Down, Modifiers::none(), false, false, false, false);
    assert_eq!(bytes, b"\x1b[B");
}

#[test]
fn translate_key_arrow_right() {
    let bytes = encode_key(Key::Right, Modifiers::none(), false, false, false, false);
    assert_eq!(bytes, b"\x1b[C");
}

#[test]
fn translate_key_arrow_left() {
    let bytes = encode_key(Key::Left, Modifiers::none(), false, false, false, false);
    assert_eq!(bytes, b"\x1b[D");
}

// ---------------------------------------------------------------------------
// Arrow keys — application cursor mode (DECCKM)
// ---------------------------------------------------------------------------

#[test]
fn translate_key_arrow_up_app_cursor() {
    let bytes = encode_key(Key::Up, Modifiers::none(), true, false, false, false);
    assert_eq!(bytes, b"\x1bOA");
}

#[test]
fn translate_key_arrow_down_app_cursor() {
    let bytes = encode_key(Key::Down, Modifiers::none(), true, false, false, false);
    assert_eq!(bytes, b"\x1bOB");
}

// ---------------------------------------------------------------------------
// Ctrl + letter (C0 control codes)
// ---------------------------------------------------------------------------

#[test]
fn translate_key_ctrl_c() {
    let bytes = encode_key(Key::Char('c'), Modifiers::ctrl(), false, false, false, false);
    assert_eq!(bytes, vec![0x03]); // ETX
}

#[test]
fn translate_key_ctrl_a() {
    let bytes = encode_key(Key::Char('a'), Modifiers::ctrl(), false, false, false, false);
    assert_eq!(bytes, vec![0x01]); // SOH
}

#[test]
fn translate_key_ctrl_z() {
    let bytes = encode_key(Key::Char('z'), Modifiers::ctrl(), false, false, false, false);
    assert_eq!(bytes, vec![0x1A]); // SUB
}

#[test]
fn translate_key_ctrl_d() {
    let bytes = encode_key(Key::Char('d'), Modifiers::ctrl(), false, false, false, false);
    assert_eq!(bytes, vec![0x04]); // EOT
}

// ---------------------------------------------------------------------------
// Alt + char
// ---------------------------------------------------------------------------

#[test]
fn translate_key_alt_a() {
    let bytes = encode_key(Key::Char('a'), Modifiers::alt(), false, false, false, false);
    assert_eq!(bytes, vec![0x1B, b'a']);
}

#[test]
fn translate_key_alt_x() {
    let bytes = encode_key(Key::Char('x'), Modifiers::alt(), false, false, false, false);
    assert_eq!(bytes, vec![0x1B, b'x']);
}

// ---------------------------------------------------------------------------
// Ctrl+Alt + char
// ---------------------------------------------------------------------------

#[test]
fn translate_key_ctrl_alt_a() {
    let mods = Modifiers { shift: false, alt: true, ctrl: true };
    let bytes = encode_key(Key::Char('a'), mods, false, false, false, false);
    assert_eq!(bytes, vec![0x1B, 0x01]);
}

// ---------------------------------------------------------------------------
// Special keys
// ---------------------------------------------------------------------------

#[test]
fn translate_key_escape() {
    let bytes = encode_key(Key::Escape, Modifiers::none(), false, false, false, false);
    assert_eq!(bytes, b"\x1b");
}

#[test]
fn translate_key_tab() {
    let bytes = encode_key(Key::Tab, Modifiers::none(), false, false, false, false);
    assert_eq!(bytes, b"\x09");
}

#[test]
fn translate_key_shift_tab() {
    let bytes = encode_key(Key::Tab, Modifiers::shift(), false, false, false, false);
    assert_eq!(bytes, b"\x1b[Z");
}

#[test]
fn translate_key_backspace() {
    let bytes = encode_key(Key::Backspace, Modifiers::none(), false, false, false, false);
    assert_eq!(bytes, vec![0x7F]);
}

#[test]
fn translate_key_ctrl_backspace() {
    let bytes = encode_key(Key::Backspace, Modifiers::ctrl(), false, false, false, false);
    assert_eq!(bytes, vec![0x08]);
}

#[test]
fn translate_key_alt_backspace() {
    let bytes = encode_key(Key::Backspace, Modifiers::alt(), false, false, false, false);
    assert_eq!(bytes, vec![0x1B, 0x7F]);
}

// ---------------------------------------------------------------------------
// Navigation keys
// ---------------------------------------------------------------------------

#[test]
fn translate_key_home() {
    let bytes = encode_key(Key::Home, Modifiers::none(), false, false, false, false);
    assert_eq!(bytes, b"\x1b[H");
}

#[test]
fn translate_key_end() {
    let bytes = encode_key(Key::End, Modifiers::none(), false, false, false, false);
    assert_eq!(bytes, b"\x1b[F");
}

#[test]
fn translate_key_page_up() {
    let bytes = encode_key(Key::PageUp, Modifiers::none(), false, false, false, false);
    assert_eq!(bytes, b"\x1b[5~");
}

#[test]
fn translate_key_page_down() {
    let bytes = encode_key(Key::PageDown, Modifiers::none(), false, false, false, false);
    assert_eq!(bytes, b"\x1b[6~");
}

#[test]
fn translate_key_insert() {
    let bytes = encode_key(Key::Insert, Modifiers::none(), false, false, false, false);
    assert_eq!(bytes, b"\x1b[2~");
}

#[test]
fn translate_key_delete() {
    let bytes = encode_key(Key::Delete, Modifiers::none(), false, false, false, false);
    assert_eq!(bytes, b"\x1b[3~");
}

// ---------------------------------------------------------------------------
// Modified arrow keys (xterm-style modifier parameters)
// ---------------------------------------------------------------------------

#[test]
fn translate_key_shift_up() {
    let bytes = encode_key(Key::Up, Modifiers::shift(), false, false, false, false);
    assert_eq!(bytes, b"\x1b[1;2A");
}

#[test]
fn translate_key_ctrl_right() {
    let bytes = encode_key(Key::Right, Modifiers::ctrl(), false, false, false, false);
    assert_eq!(bytes, b"\x1b[1;5C");
}

#[test]
fn translate_key_alt_left() {
    let bytes = encode_key(Key::Left, Modifiers::alt(), false, false, false, false);
    assert_eq!(bytes, b"\x1b[1;3D");
}

// ---------------------------------------------------------------------------
// Function keys
// ---------------------------------------------------------------------------

#[test]
fn translate_key_f1() {
    let bytes = encode_key(Key::F(1), Modifiers::none(), false, false, false, false);
    assert_eq!(bytes, b"\x1bOP");
}

#[test]
fn translate_key_f2() {
    let bytes = encode_key(Key::F(2), Modifiers::none(), false, false, false, false);
    assert_eq!(bytes, b"\x1bOQ");
}

#[test]
fn translate_key_f3() {
    let bytes = encode_key(Key::F(3), Modifiers::none(), false, false, false, false);
    assert_eq!(bytes, b"\x1bOR");
}

#[test]
fn translate_key_f4() {
    let bytes = encode_key(Key::F(4), Modifiers::none(), false, false, false, false);
    assert_eq!(bytes, b"\x1bOS");
}

#[test]
fn translate_key_f5() {
    let bytes = encode_key(Key::F(5), Modifiers::none(), false, false, false, false);
    assert_eq!(bytes, b"\x1b[15~");
}

#[test]
fn translate_key_f12() {
    let bytes = encode_key(Key::F(12), Modifiers::none(), false, false, false, false);
    assert_eq!(bytes, b"\x1b[24~");
}

#[test]
fn translate_key_shift_f1() {
    let bytes = encode_key(Key::F(1), Modifiers::shift(), false, false, false, false);
    assert_eq!(bytes, b"\x1b[1;2P");
}

// ---------------------------------------------------------------------------
// Modified navigation keys
// ---------------------------------------------------------------------------

#[test]
fn translate_key_ctrl_delete() {
    let bytes = encode_key(Key::Delete, Modifiers::ctrl(), false, false, false, false);
    assert_eq!(bytes, b"\x1b[3;5~");
}

#[test]
fn translate_key_shift_page_up() {
    let bytes = encode_key(Key::PageUp, Modifiers::shift(), false, false, false, false);
    assert_eq!(bytes, b"\x1b[5;2~");
}

// ---------------------------------------------------------------------------
// Home/End in application cursor mode
// ---------------------------------------------------------------------------

#[test]
fn translate_key_home_app_cursor() {
    let bytes = encode_key(Key::Home, Modifiers::none(), true, false, false, false);
    assert_eq!(bytes, b"\x1bOH");
}

#[test]
fn translate_key_end_app_cursor() {
    let bytes = encode_key(Key::End, Modifiers::none(), true, false, false, false);
    assert_eq!(bytes, b"\x1bOF");
}

// ---------------------------------------------------------------------------
// Search integration tests
// ---------------------------------------------------------------------------

#[test]
fn search_finds_matches_in_screen_text() {
    // Verify that the search module can find matches in text that would come
    // from Screen::row_text() — this is the path used by the search keybinding.
    use emux_term::search::{find_all_matches, SearchState, next_match_index, prev_match_index};

    let rows = vec![
        "$ cargo build".into(),
        "   Compiling emux v0.1.0".into(),
        "    Finished dev profile".into(),
        "$ cargo test".into(),
        "running 42 tests".into(),
    ];

    let matches = find_all_matches(&rows, "cargo", false);
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].row, 0);
    assert_eq!(matches[1].row, 3);

    // Verify navigation.
    let mut state = SearchState {
        query: "cargo".into(),
        matches: matches.clone(),
        current: Some(0),
        case_sensitive: false,
        regex: false,
    };

    state.current = next_match_index(state.current, state.matches.len());
    assert_eq!(state.current, Some(1));

    state.current = next_match_index(state.current, state.matches.len());
    assert_eq!(state.current, Some(0)); // wraps

    state.current = prev_match_index(state.current, state.matches.len());
    assert_eq!(state.current, Some(1)); // wraps backward
}

#[test]
fn search_empty_query_clears_state() {
    use emux_term::search::{find_all_matches, SearchState};

    let rows = vec!["hello world".into()];
    let matches = find_all_matches(&rows, "", false);
    assert!(matches.is_empty());

    let state = SearchState::default();
    assert!(state.query.is_empty());
    assert!(state.matches.is_empty());
    assert_eq!(state.current, None);
}

#[test]
fn search_keybinding_parses_correctly() {
    // Verify that the default search keybinding "Leader+/" is a valid binding.
    let keys = emux_config::KeyBindings::default();
    assert_eq!(keys.search, "Leader+/");
}
