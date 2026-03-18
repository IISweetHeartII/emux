//! Input/key encoding conformance tests translated from libvterm and tmux.
//!
//! Sources:
//!   - libvterm/t/25state_input.test
//!   - tmux/regress/input-keys.sh

use emux_term::input::*;

fn no_mods() -> Modifiers { Modifiers::none() }
fn ctrl() -> Modifiers { Modifiers::ctrl() }
fn alt() -> Modifiers { Modifiers::alt() }
fn shift() -> Modifiers { Modifiers::shift() }
fn ctrl_alt() -> Modifiers { Modifiers { ctrl: true, alt: true, ..Default::default() } }

// ---------------------------------------------------------------------------
// Unmodified ASCII (libvterm 25state_input.test)
// ---------------------------------------------------------------------------

#[test]
fn input_ascii_uppercase_a() {
    // libvterm: INCHAR 0 41 => output "A"
    assert_eq!(encode_key(Key::Char('A'), no_mods(), false, false, false, false), b"A");
}

#[test]
fn input_ascii_lowercase_a() {
    // libvterm: INCHAR 0 61 => output "a"
    assert_eq!(encode_key(Key::Char('a'), no_mods(), false, false, false, false), b"a");
}

// ---------------------------------------------------------------------------
// Ctrl modifier (libvterm 25state_input.test)
// ---------------------------------------------------------------------------

#[test]
fn input_ctrl_uppercase_a() {
    // libvterm: INCHAR C 41 => output "\x1b[65;5u"
    // Ctrl + uppercase A uses CSI u encoding
    assert_eq!(
        encode_key(Key::Char('A'), ctrl(), false, false, false, false),
        b"\x1b[65;5u"
    );
}

#[test]
fn input_ctrl_lowercase_a() {
    // libvterm: INCHAR C 61 => output "\x01"
    assert_eq!(
        encode_key(Key::Char('a'), ctrl(), false, false, false, false),
        b"\x01"
    );
}

#[test]
fn input_ctrl_lowercase_letters() {
    // tmux: C-a => ^A, C-b => ^B, ... C-z => ^Z
    for (i, ch) in ('a'..='z').enumerate() {
        let expected = vec![(i as u8) + 1];
        assert_eq!(
            encode_key(Key::Char(ch), ctrl(), false, false, false, false),
            expected,
            "Ctrl+{ch}"
        );
    }
}

// ---------------------------------------------------------------------------
// Alt modifier (libvterm 25state_input.test)
// ---------------------------------------------------------------------------

#[test]
fn input_alt_uppercase_a() {
    // libvterm: INCHAR A 41 => output "\x1bA"
    assert_eq!(
        encode_key(Key::Char('A'), alt(), false, false, false, false),
        b"\x1bA"
    );
}

#[test]
fn input_alt_lowercase_a() {
    // libvterm: INCHAR A 61 => output "\x1ba"
    assert_eq!(
        encode_key(Key::Char('a'), alt(), false, false, false, false),
        b"\x1ba"
    );
}

#[test]
fn input_alt_lowercase_letters() {
    // tmux: M-a => ^[a, M-b => ^[b, ... M-z => ^[z
    for ch in 'a'..='z' {
        let expected = vec![0x1B, ch as u8];
        assert_eq!(
            encode_key(Key::Char(ch), alt(), false, false, false, false),
            expected,
            "Alt+{ch}"
        );
    }
}

#[test]
fn input_alt_uppercase_letters() {
    // tmux: M-A => ^[A, M-B => ^[B, ... M-Z => ^[Z
    for ch in 'A'..='Z' {
        let expected = vec![0x1B, ch as u8];
        assert_eq!(
            encode_key(Key::Char(ch), alt(), false, false, false, false),
            expected,
            "Alt+{ch}"
        );
    }
}

// ---------------------------------------------------------------------------
// Ctrl+Alt modifier (libvterm 25state_input.test)
// ---------------------------------------------------------------------------

#[test]
fn input_ctrl_alt_uppercase_a() {
    // libvterm: INCHAR CA 41 => output "\x1b[65;7u"
    assert_eq!(
        encode_key(Key::Char('A'), ctrl_alt(), false, false, false, false),
        b"\x1b[65;7u"
    );
}

#[test]
fn input_ctrl_alt_lowercase_a() {
    // libvterm: INCHAR CA 61 => output "\x1b\x01"
    assert_eq!(
        encode_key(Key::Char('a'), ctrl_alt(), false, false, false, false),
        b"\x1b\x01"
    );
}

#[test]
fn input_meta_ctrl_letters_tmux() {
    // tmux: M-C-a => ^[^A, M-C-b => ^[^B, ... M-C-z => ^[^Z
    for (i, ch) in ('a'..='z').enumerate() {
        let expected = vec![0x1B, (i as u8) + 1];
        assert_eq!(
            encode_key(Key::Char(ch), ctrl_alt(), false, false, false, false),
            expected,
            "Ctrl+Alt+{ch}"
        );
    }
}

// ---------------------------------------------------------------------------
// Special keys: Ctrl-I (Tab), Space (libvterm 25state_input.test)
// ---------------------------------------------------------------------------

#[test]
fn input_ctrl_i_disambiguation() {
    // libvterm: Ctrl-I is special because 0x09 = Tab
    // INCHAR C 49 => "\x1b[73;5u" (disambiguated via CSI u)
    // INCHAR C 69 => "\x1b[105;5u"
    // The last param (disambiguate=true) enables CSI u encoding for ambiguous keys.
    assert_eq!(
        encode_key(Key::Char('I'), ctrl(), false, false, false, true),
        b"\x1b[73;5u"
    );
    assert_eq!(
        encode_key(Key::Char('i'), ctrl(), false, false, false, true),
        b"\x1b[105;5u"
    );
}

#[test]
fn input_space_with_modifiers() {
    // libvterm: Space (0x20) special handling
    assert_eq!(encode_key(Key::Char(' '), no_mods(), false, false, false, false), b" ");
    assert_eq!(encode_key(Key::Char(' '), shift(), false, false, false, false), b"\x1b[32;2u");
    assert_eq!(encode_key(Key::Char(' '), ctrl(), false, false, false, false), b"\x00");
    assert_eq!(encode_key(Key::Char(' '), Modifiers { shift: true, ctrl: true, ..Default::default() }, false, false, false, false), b"\x1b[32;6u");
    assert_eq!(encode_key(Key::Char(' '), alt(), false, false, false, false), b"\x1b ");
    assert_eq!(encode_key(Key::Char(' '), ctrl_alt(), false, false, false, false), b"\x1b\x00");
}

// ---------------------------------------------------------------------------
// Cursor keys (libvterm 25state_input.test)
// ---------------------------------------------------------------------------

#[test]
fn input_cursor_up_normal_mode() {
    // libvterm: INKEY 0 Up => "\x1b[A"
    assert_eq!(encode_key(Key::Up, no_mods(), false, false, false, false), b"\x1b[A");
}

#[test]
fn input_cursor_keys_with_shift() {
    // libvterm: INKEY S Up => "\x1b[1;2A"
    assert_eq!(encode_key(Key::Up, shift(), false, false, false, false), b"\x1b[1;2A");
}

#[test]
fn input_cursor_keys_with_ctrl() {
    // libvterm: INKEY C Up => "\x1b[1;5A"
    assert_eq!(encode_key(Key::Up, ctrl(), false, false, false, false), b"\x1b[1;5A");
}

#[test]
fn input_cursor_keys_with_alt() {
    // libvterm: INKEY A Up => "\x1b[1;3A"
    assert_eq!(encode_key(Key::Up, alt(), false, false, false, false), b"\x1b[1;3A");
}

#[test]
fn input_cursor_keys_all_modifiers() {
    // Shift+Ctrl = 6, Shift+Alt = 4, Ctrl+Alt = 7, Shift+Ctrl+Alt = 8
    let sc = Modifiers { shift: true, ctrl: true, ..Default::default() };
    let sa = Modifiers { shift: true, alt: true, ..Default::default() };
    let ca = Modifiers { ctrl: true, alt: true, ..Default::default() };
    let sca = Modifiers { shift: true, alt: true, ctrl: true };

    assert_eq!(encode_key(Key::Up, sc, false, false, false, false), b"\x1b[1;6A");
    assert_eq!(encode_key(Key::Up, sa, false, false, false, false), b"\x1b[1;4A");
    assert_eq!(encode_key(Key::Up, ca, false, false, false, false), b"\x1b[1;7A");
    assert_eq!(encode_key(Key::Up, sca, false, false, false, false), b"\x1b[1;8A");
}

#[test]
fn input_cursor_keys_application_mode() {
    // After CSI ?1h (DECCKM): INKEY 0 Up => "\x1bOA"
    assert_eq!(encode_key(Key::Up, no_mods(), true, false, false, false), b"\x1bOA");
    assert_eq!(encode_key(Key::Down, no_mods(), true, false, false, false), b"\x1bOB");
    assert_eq!(encode_key(Key::Right, no_mods(), true, false, false, false), b"\x1bOC");
    assert_eq!(encode_key(Key::Left, no_mods(), true, false, false, false), b"\x1bOD");
    // Modified keys still use CSI even in app mode
    assert_eq!(encode_key(Key::Up, shift(), true, false, false, false), b"\x1b[1;2A");
}

#[test]
fn input_arrow_keys_tmux() {
    // tmux: Up => ^[[A, Down => ^[[B, Right => ^[[C, Left => ^[[D
    assert_eq!(encode_key(Key::Up, no_mods(), false, false, false, false), b"\x1b[A");
    assert_eq!(encode_key(Key::Down, no_mods(), false, false, false, false), b"\x1b[B");
    assert_eq!(encode_key(Key::Right, no_mods(), false, false, false, false), b"\x1b[C");
    assert_eq!(encode_key(Key::Left, no_mods(), false, false, false, false), b"\x1b[D");
}

// ---------------------------------------------------------------------------
// Function keys (libvterm 25state_input.test, tmux input-keys.sh)
// ---------------------------------------------------------------------------

#[test]
fn input_f1_unmodified() {
    // libvterm: INKEY 0 F1 => "\x1bOP" (SS3 P)
    assert_eq!(encode_key(Key::F(1), no_mods(), false, false, false, false), b"\x1bOP");
}

#[test]
fn input_f1_modified() {
    // libvterm: INKEY S F1 => "\x1b[1;2P"
    assert_eq!(encode_key(Key::F(1), shift(), false, false, false, false), b"\x1b[1;2P");
    assert_eq!(encode_key(Key::F(1), alt(), false, false, false, false), b"\x1b[1;3P");
    assert_eq!(encode_key(Key::F(1), ctrl(), false, false, false, false), b"\x1b[1;5P");
}

#[test]
fn input_f2_f4_unmodified() {
    // tmux: F2 => ^[OQ, F3 => ^[OR, F4 => ^[OS
    assert_eq!(encode_key(Key::F(2), no_mods(), false, false, false, false), b"\x1bOQ");
    assert_eq!(encode_key(Key::F(3), no_mods(), false, false, false, false), b"\x1bOR");
    assert_eq!(encode_key(Key::F(4), no_mods(), false, false, false, false), b"\x1bOS");
}

#[test]
fn input_f5_f12_unmodified() {
    // tmux: F5 => ^[[15~, F6 => ^[[17~, F7 => ^[[18~, F8 => ^[[19~
    // F9 => ^[[20~, F10 => ^[[21~, F11 => ^[[23~, F12 => ^[[24~
    assert_eq!(encode_key(Key::F(5), no_mods(), false, false, false, false), b"\x1b[15~");
    assert_eq!(encode_key(Key::F(6), no_mods(), false, false, false, false), b"\x1b[17~");
    assert_eq!(encode_key(Key::F(7), no_mods(), false, false, false, false), b"\x1b[18~");
    assert_eq!(encode_key(Key::F(8), no_mods(), false, false, false, false), b"\x1b[19~");
    assert_eq!(encode_key(Key::F(9), no_mods(), false, false, false, false), b"\x1b[20~");
    assert_eq!(encode_key(Key::F(10), no_mods(), false, false, false, false), b"\x1b[21~");
    assert_eq!(encode_key(Key::F(11), no_mods(), false, false, false, false), b"\x1b[23~");
    assert_eq!(encode_key(Key::F(12), no_mods(), false, false, false, false), b"\x1b[24~");
}

#[test]
fn input_function_keys_extended_modifiers() {
    // S-F1 => ^[[1;2P, C-F1 => ^[[1;5P
    assert_eq!(encode_key(Key::F(1), shift(), false, false, false, false), b"\x1b[1;2P");
    assert_eq!(encode_key(Key::F(1), ctrl(), false, false, false, false), b"\x1b[1;5P");
    // S-F5 => ^[[15;2~, C-F5 => ^[[15;5~
    assert_eq!(encode_key(Key::F(5), shift(), false, false, false, false), b"\x1b[15;2~");
    assert_eq!(encode_key(Key::F(5), ctrl(), false, false, false, false), b"\x1b[15;5~");
}

// ---------------------------------------------------------------------------
// Tab / Enter / Backspace (libvterm 25state_input.test, tmux)
// ---------------------------------------------------------------------------

#[test]
fn input_tab_and_shift_tab() {
    // INKEY 0 Tab => "\x09"
    assert_eq!(encode_key(Key::Tab, no_mods(), false, false, false, false), b"\x09");
    // INKEY S Tab => "\x1b[Z" (reverse tab / backtab)
    assert_eq!(encode_key(Key::Tab, shift(), false, false, false, false), b"\x1b[Z");
    // INKEY A Tab => "\x1b\x09"
    assert_eq!(encode_key(Key::Tab, alt(), false, false, false, false), b"\x1b\x09");
}

#[test]
fn input_enter_linefeed_mode() {
    // libvterm: INKEY 0 Enter => "\x0d" (CR)
    assert_eq!(encode_key(Key::Enter, no_mods(), false, false, false, false), b"\x0d");
}

#[test]
fn input_enter_newline_mode() {
    // After CSI 20h (LNM): INKEY 0 Enter => "\x0d\x0a" (CR+LF)
    assert_eq!(encode_key(Key::Enter, no_mods(), false, false, true, false), b"\x0d\x0a");
}

#[test]
fn input_backspace_tmux() {
    // tmux: BSpace => ^? (0x7F)
    assert_eq!(encode_key(Key::Backspace, no_mods(), false, false, false, false), b"\x7f");
    // M-BSpace => ^[^?
    assert_eq!(encode_key(Key::Backspace, alt(), false, false, false, false), b"\x1b\x7f");
}

// ---------------------------------------------------------------------------
// Keypad (libvterm 25state_input.test, tmux input-keys.sh)
// ---------------------------------------------------------------------------

#[test]
fn input_keypad_numeric_mode() {
    use emux_term::input::{encode_keypad, KeypadKey};
    // DECKPNM (default): KP0 => '0'
    assert_eq!(encode_keypad(KeypadKey::Num0, false), b"0");
    assert_eq!(encode_keypad(KeypadKey::Num1, false), b"1");
    assert_eq!(encode_keypad(KeypadKey::Num9, false), b"9");
    assert_eq!(encode_keypad(KeypadKey::Plus, false), b"+");
    assert_eq!(encode_keypad(KeypadKey::Minus, false), b"-");
    assert_eq!(encode_keypad(KeypadKey::Decimal, false), b".");
    assert_eq!(encode_keypad(KeypadKey::Enter, false), b"\x0d");
}

#[test]
fn input_keypad_application_mode() {
    use emux_term::input::{encode_keypad, KeypadKey};
    // After ESC = (DECKPAM): KP0 => "\x1bOp"
    assert_eq!(encode_keypad(KeypadKey::Num0, true), b"\x1bOp");
    assert_eq!(encode_keypad(KeypadKey::Num1, true), b"\x1bOq");
    assert_eq!(encode_keypad(KeypadKey::Num9, true), b"\x1bOy");
    assert_eq!(encode_keypad(KeypadKey::Enter, true), b"\x1bOM");
    assert_eq!(encode_keypad(KeypadKey::Minus, true), b"\x1bOm");
}

#[test]
fn input_keypad_operators_tmux() {
    use emux_term::input::{encode_keypad, KeypadKey};
    // tmux: KP* => '*', KP+ => '+', etc.
    assert_eq!(encode_keypad(KeypadKey::Star, false), b"*");
    assert_eq!(encode_keypad(KeypadKey::Plus, false), b"+");
    assert_eq!(encode_keypad(KeypadKey::Minus, false), b"-");
    assert_eq!(encode_keypad(KeypadKey::Slash, false), b"/");
    assert_eq!(encode_keypad(KeypadKey::Equal, false), b"=");
}

// ---------------------------------------------------------------------------
// Navigation keys (tmux input-keys.sh)
// ---------------------------------------------------------------------------

#[test]
fn input_insert_delete() {
    // tmux: IC/Insert => ^[[2~, DC/Delete => ^[[3~
    assert_eq!(encode_key(Key::Insert, no_mods(), false, false, false, false), b"\x1b[2~");
    assert_eq!(encode_key(Key::Delete, no_mods(), false, false, false, false), b"\x1b[3~");
}

#[test]
fn input_home_end() {
    // Home => ^[[H, End => ^[[F (xterm-style)
    assert_eq!(encode_key(Key::Home, no_mods(), false, false, false, false), b"\x1b[H");
    assert_eq!(encode_key(Key::End, no_mods(), false, false, false, false), b"\x1b[F");
    // S-Home => ^[[1;2H, C-Home => ^[[1;5H
    assert_eq!(encode_key(Key::Home, shift(), false, false, false, false), b"\x1b[1;2H");
    assert_eq!(encode_key(Key::Home, ctrl(), false, false, false, false), b"\x1b[1;5H");
}

#[test]
fn input_page_up_down() {
    // PPage/PageUp => ^[[5~, NPage/PageDown => ^[[6~
    assert_eq!(encode_key(Key::PageUp, no_mods(), false, false, false, false), b"\x1b[5~");
    assert_eq!(encode_key(Key::PageDown, no_mods(), false, false, false, false), b"\x1b[6~");
}

#[test]
fn input_navigation_extended_modifiers() {
    // Modifier encoding: 2=S, 3=M, 4=SM, 5=C, 6=SC, 7=CM, 8=SCM
    let sc = Modifiers { shift: true, ctrl: true, ..Default::default() };
    assert_eq!(encode_key(Key::Insert, shift(), false, false, false, false), b"\x1b[2;2~");
    assert_eq!(encode_key(Key::Delete, ctrl(), false, false, false, false), b"\x1b[3;5~");
    assert_eq!(encode_key(Key::PageUp, alt(), false, false, false, false), b"\x1b[5;3~");
    assert_eq!(encode_key(Key::PageDown, sc, false, false, false, false), b"\x1b[6;6~");
}

// ---------------------------------------------------------------------------
// Bracketed paste (libvterm 25state_input.test)
// ---------------------------------------------------------------------------

#[test]
fn input_bracketed_paste_off() {
    use emux_term::input::encode_paste;
    // Without bracketed paste, text is sent as-is
    assert_eq!(encode_paste("hello", false), b"hello");
}

#[test]
fn input_bracketed_paste_on() {
    use emux_term::input::encode_paste;
    // With bracketed paste, text is wrapped in ESC[200~ ... ESC[201~
    assert_eq!(
        encode_paste("hello", true),
        b"\x1b[200~hello\x1b[201~"
    );
}

#[test]
fn input_bracketed_paste_empty_string() {
    use emux_term::input::encode_paste;
    // Empty paste with bracketed mode still wraps
    assert_eq!(encode_paste("", true), b"\x1b[200~\x1b[201~");
    // Empty paste without bracketed mode is empty
    assert_eq!(encode_paste("", false), b"");
}

#[test]
fn input_bracketed_paste_multiline() {
    use emux_term::input::encode_paste;
    // Multi-line text is wrapped correctly with bracketed paste
    let text = "line1\nline2\nline3";
    assert_eq!(
        encode_paste(text, true),
        b"\x1b[200~line1\nline2\nline3\x1b[201~"
    );
    // Without bracketed paste, pass through as-is
    assert_eq!(encode_paste(text, false), text.as_bytes());
}

#[test]
fn input_bracketed_paste_with_escape_chars() {
    use emux_term::input::encode_paste;
    // Text containing ESC sequences should be wrapped verbatim (no escaping)
    let text = "\x1b[31mred\x1b[0m";
    assert_eq!(
        encode_paste(text, true),
        b"\x1b[200~\x1b[31mred\x1b[0m\x1b[201~"
    );
    assert_eq!(encode_paste(text, false), text.as_bytes());
}

// ---------------------------------------------------------------------------
// Focus reporting (libvterm 25state_input.test)
// ---------------------------------------------------------------------------

#[test]
fn input_focus_reporting_disabled() {
    use emux_term::input::encode_focus;
    // Focus reporting disabled: no output
    assert_eq!(encode_focus(true, false), b"");
    assert_eq!(encode_focus(false, false), b"");
}

#[test]
fn input_focus_reporting_enabled() {
    use emux_term::input::encode_focus;
    // Focus reporting enabled: CSI I for focus in, CSI O for focus out
    assert_eq!(encode_focus(true, true), b"\x1b[I");
    assert_eq!(encode_focus(false, true), b"\x1b[O");
}

// ---------------------------------------------------------------------------
// Mouse encoding (derived from common xterm protocols)
// ---------------------------------------------------------------------------

#[test]
fn input_mouse_normal_mode_click() {
    // X10 mouse: CSI M + button+32 + col+33 + row+33
    // Button 0 click at (0,0) => "\x1b[M" 0x20 0x21 0x21
    let result = encode_mouse(
        MouseEvent::Press { button: 0, col: 0, row: 0 },
        MouseEncoding::Normal,
    );
    assert_eq!(result, b"\x1b[M\x20\x21\x21");
}

#[test]
fn input_mouse_sgr_mode_press() {
    // SGR mouse: CSI < button ; col ; row M
    // Button 0 press at col 9, row 4 => "\x1b[<0;10;5M"
    let result = encode_mouse(
        MouseEvent::Press { button: 0, col: 9, row: 4 },
        MouseEncoding::Sgr,
    );
    assert_eq!(result, b"\x1b[<0;10;5M");
}

#[test]
fn input_mouse_sgr_mode_release() {
    // SGR mouse release: CSI < button ; col ; row m (lowercase m)
    // Button 0 release at col 9, row 4 => "\x1b[<0;10;5m"
    let result = encode_mouse(
        MouseEvent::Release { col: 9, row: 4 },
        MouseEncoding::Sgr,
    );
    assert_eq!(result, b"\x1b[<0;10;5m");
}

#[test]
fn input_mouse_scroll_wheel() {
    // Scroll up = button 64, scroll down = button 65
    // SGR: "\x1b[<64;col+1;row+1M" for scroll up
    let result = encode_mouse(
        MouseEvent::ScrollUp { col: 5, row: 10 },
        MouseEncoding::Sgr,
    );
    assert_eq!(result, b"\x1b[<64;6;11M");

    let result = encode_mouse(
        MouseEvent::ScrollDown { col: 5, row: 10 },
        MouseEncoding::Sgr,
    );
    assert_eq!(result, b"\x1b[<65;6;11M");
}

#[test]
fn input_mouse_drag_tracking() {
    // Drag with button 0 held: button = 32 (motion) + 0
    // SGR: "\x1b[<32;col+1;row+1M"
    let result = encode_mouse(
        MouseEvent::Drag { button: 0, col: 15, row: 20 },
        MouseEncoding::Sgr,
    );
    assert_eq!(result, b"\x1b[<32;16;21M");
}

// ---------------------------------------------------------------------------
// Additional mouse encoding tests
// ---------------------------------------------------------------------------

#[test]
fn input_mouse_normal_mode_release() {
    // X10/Normal release: button byte = 3 + 32 = 35
    let result = encode_mouse(
        MouseEvent::Release { col: 5, row: 10 },
        MouseEncoding::Normal,
    );
    assert_eq!(result, vec![0x1B, b'[', b'M', 35, 5 + 33, 10 + 33]);
}

#[test]
fn input_mouse_normal_mode_right_click() {
    // Button 2 (right) press at (3, 7): button byte = 2 + 32 = 34
    let result = encode_mouse(
        MouseEvent::Press { button: 2, col: 3, row: 7 },
        MouseEncoding::Normal,
    );
    assert_eq!(result, vec![0x1B, b'[', b'M', 34, 3 + 33, 7 + 33]);
}

#[test]
fn input_mouse_normal_mode_middle_click() {
    // Button 1 (middle) press at (0, 0): button byte = 1 + 32 = 33
    let result = encode_mouse(
        MouseEvent::Press { button: 1, col: 0, row: 0 },
        MouseEncoding::Normal,
    );
    assert_eq!(result, vec![0x1B, b'[', b'M', 33, 0 + 33, 0 + 33]);
}

#[test]
fn input_mouse_normal_mode_scroll_up() {
    // Scroll up: button byte = 64 + 32 = 96
    let result = encode_mouse(
        MouseEvent::ScrollUp { col: 10, row: 5 },
        MouseEncoding::Normal,
    );
    assert_eq!(result, vec![0x1B, b'[', b'M', 96, 10 + 33, 5 + 33]);
}

#[test]
fn input_mouse_normal_mode_scroll_down() {
    // Scroll down: button byte = 65 + 32 = 97
    let result = encode_mouse(
        MouseEvent::ScrollDown { col: 10, row: 5 },
        MouseEncoding::Normal,
    );
    assert_eq!(result, vec![0x1B, b'[', b'M', 97, 10 + 33, 5 + 33]);
}

#[test]
fn input_mouse_normal_mode_drag() {
    // Drag button 0: button byte = 0 + 32 + 32 = 64
    let result = encode_mouse(
        MouseEvent::Drag { button: 0, col: 20, row: 15 },
        MouseEncoding::Normal,
    );
    assert_eq!(result, vec![0x1B, b'[', b'M', 64, 20 + 33, 15 + 33]);
}

#[test]
fn input_mouse_sgr_mode_right_click() {
    // SGR: button 2 press at (10, 20) => "\x1b[<2;11;21M"
    let result = encode_mouse(
        MouseEvent::Press { button: 2, col: 10, row: 20 },
        MouseEncoding::Sgr,
    );
    assert_eq!(result, b"\x1b[<2;11;21M");
}

#[test]
fn input_mouse_sgr_mode_drag() {
    // SGR drag: button + 32 for motion flag
    // Drag button 1 at (5, 3) => "\x1b[<33;6;4M"
    let result = encode_mouse(
        MouseEvent::Drag { button: 1, col: 5, row: 3 },
        MouseEncoding::Sgr,
    );
    assert_eq!(result, b"\x1b[<33;6;4M");
}

#[test]
fn input_mouse_sgr_large_coordinates() {
    // SGR supports coordinates > 223 (unlike X10 normal mode).
    // Press at col 300, row 200 => "\x1b[<0;301;201M"
    let result = encode_mouse(
        MouseEvent::Press { button: 0, col: 300, row: 200 },
        MouseEncoding::Sgr,
    );
    assert_eq!(result, b"\x1b[<0;301;201M");
}

#[test]
fn input_mouse_normal_mode_origin() {
    // Verify encoding at the origin (0,0) in both modes.
    let normal = encode_mouse(
        MouseEvent::Press { button: 0, col: 0, row: 0 },
        MouseEncoding::Normal,
    );
    assert_eq!(normal, b"\x1b[M\x20\x21\x21");

    let sgr = encode_mouse(
        MouseEvent::Press { button: 0, col: 0, row: 0 },
        MouseEncoding::Sgr,
    );
    assert_eq!(sgr, b"\x1b[<0;1;1M");
}

// ---------------------------------------------------------------------------
// Focus reporting — edge cases
// ---------------------------------------------------------------------------

#[test]
fn input_focus_in_enabled_produces_csi_i() {
    use emux_term::input::encode_focus;
    assert_eq!(encode_focus(true, true), b"\x1b[I");
}

#[test]
fn input_focus_out_enabled_produces_csi_o() {
    use emux_term::input::encode_focus;
    assert_eq!(encode_focus(false, true), b"\x1b[O");
}

#[test]
fn input_focus_in_disabled_produces_empty() {
    use emux_term::input::encode_focus;
    assert_eq!(encode_focus(true, false), Vec::<u8>::new());
}

#[test]
fn input_focus_out_disabled_produces_empty() {
    use emux_term::input::encode_focus;
    assert_eq!(encode_focus(false, false), Vec::<u8>::new());
}

// ---------------------------------------------------------------------------
// Home/End in application cursor mode
// ---------------------------------------------------------------------------

#[test]
fn input_home_end_application_cursor_mode() {
    // In application cursor mode, Home/End use SS3 encoding
    assert_eq!(encode_key(Key::Home, no_mods(), true, false, false, false), b"\x1bOH");
    assert_eq!(encode_key(Key::End, no_mods(), true, false, false, false), b"\x1bOF");
    // Modified Home/End still use CSI even in app cursor mode
    assert_eq!(encode_key(Key::Home, shift(), true, false, false, false), b"\x1b[1;2H");
    assert_eq!(encode_key(Key::End, ctrl(), true, false, false, false), b"\x1b[1;5F");
}

// ---------------------------------------------------------------------------
// Escape key encoding
// ---------------------------------------------------------------------------

#[test]
fn input_escape_key() {
    assert_eq!(encode_key(Key::Escape, no_mods(), false, false, false, false), b"\x1b");
}

#[test]
fn input_escape_key_with_modifiers() {
    // Modifiers on escape are currently ignored (raw ESC)
    assert_eq!(encode_key(Key::Escape, alt(), false, false, false, false), b"\x1b");
    assert_eq!(encode_key(Key::Escape, ctrl(), false, false, false, false), b"\x1b");
}

// ---------------------------------------------------------------------------
// Ctrl+Backspace encoding
// ---------------------------------------------------------------------------

#[test]
fn input_ctrl_backspace() {
    // Ctrl+Backspace => 0x08 (BS)
    assert_eq!(encode_key(Key::Backspace, ctrl(), false, false, false, false), b"\x08");
}

// ---------------------------------------------------------------------------
// Function key F13+ out of range returns empty
// ---------------------------------------------------------------------------

#[test]
fn input_function_key_out_of_range() {
    assert_eq!(encode_key(Key::F(13), no_mods(), false, false, false, false), b"");
    assert_eq!(encode_key(Key::F(0), no_mods(), false, false, false, false), b"");
}

// ---------------------------------------------------------------------------
// All keypad keys in application mode
// ---------------------------------------------------------------------------

#[test]
fn input_keypad_all_keys_application_mode() {
    use emux_term::input::{encode_keypad, KeypadKey};
    assert_eq!(encode_keypad(KeypadKey::Num2, true), b"\x1bOr");
    assert_eq!(encode_keypad(KeypadKey::Num3, true), b"\x1bOs");
    assert_eq!(encode_keypad(KeypadKey::Num4, true), b"\x1bOt");
    assert_eq!(encode_keypad(KeypadKey::Num5, true), b"\x1bOu");
    assert_eq!(encode_keypad(KeypadKey::Num6, true), b"\x1bOv");
    assert_eq!(encode_keypad(KeypadKey::Num7, true), b"\x1bOw");
    assert_eq!(encode_keypad(KeypadKey::Num8, true), b"\x1bOx");
    assert_eq!(encode_keypad(KeypadKey::Decimal, true), b"\x1bOn");
    assert_eq!(encode_keypad(KeypadKey::Separator, true), b"\x1bOl");
    assert_eq!(encode_keypad(KeypadKey::Star, true), b"\x1bOj");
    assert_eq!(encode_keypad(KeypadKey::Plus, true), b"\x1bOk");
    assert_eq!(encode_keypad(KeypadKey::Slash, true), b"\x1bOo");
    assert_eq!(encode_keypad(KeypadKey::Equal, true), b"\x1bOX");
}

// ---------------------------------------------------------------------------
// Mouse mode 1006 (SGR) integration via screen modes
// ---------------------------------------------------------------------------

#[test]
fn mouse_sgr_mode_flag_default_off() {
    use emux_term::Screen;
    let s = Screen::new(80, 24);
    assert!(!s.modes.mouse_sgr);
}

#[test]
fn mouse_sgr_mode_enabled_via_csi() {
    use emux_term::Screen;
    use emux_vt::Parser;
    let mut s = Screen::new(80, 24);
    let mut p = Parser::new();
    // CSI ? 1006 h — enable SGR mouse encoding
    p.advance(&mut s, b"\x1b[?1006h");
    assert!(s.modes.mouse_sgr);
    // CSI ? 1006 l — disable SGR mouse encoding
    p.advance(&mut s, b"\x1b[?1006l");
    assert!(!s.modes.mouse_sgr);
}

#[test]
fn mouse_tracking_modes_set_via_csi() {
    use emux_term::Screen;
    use emux_term::modes::MouseMode;
    use emux_vt::Parser;
    let mut s = Screen::new(80, 24);
    let mut p = Parser::new();

    // Mode 9 — X10
    p.advance(&mut s, b"\x1b[?9h");
    assert_eq!(s.modes.mouse_tracking, MouseMode::X10);
    p.advance(&mut s, b"\x1b[?9l");
    assert_eq!(s.modes.mouse_tracking, MouseMode::None);

    // Mode 1000 — Normal
    p.advance(&mut s, b"\x1b[?1000h");
    assert_eq!(s.modes.mouse_tracking, MouseMode::Normal);

    // Mode 1002 — ButtonEvent
    p.advance(&mut s, b"\x1b[?1002h");
    assert_eq!(s.modes.mouse_tracking, MouseMode::ButtonEvent);

    // Mode 1003 — AnyEvent
    p.advance(&mut s, b"\x1b[?1003h");
    assert_eq!(s.modes.mouse_tracking, MouseMode::AnyEvent);
    p.advance(&mut s, b"\x1b[?1003l");
    assert_eq!(s.modes.mouse_tracking, MouseMode::None);
}

// ---------------------------------------------------------------------------
// E2E mouse event routing (requires a running terminal — ignored)
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn e2e_mouse_click_focuses_pane() {
    // This would test that clicking on a non-active pane focuses it.
    // Requires a full App setup with multiple panes.
}

#[test]
#[ignore]
fn e2e_mouse_scroll_without_tracking() {
    // This would test that scroll events trigger scrollback when
    // no mouse tracking is enabled.
}

#[test]
#[ignore]
fn e2e_mouse_event_forwarded_to_pty() {
    // This would test that mouse events are encoded and written to the
    // PTY when the child program has enabled mouse tracking.
}
