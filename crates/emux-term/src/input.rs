//! Input encoding: convert key/mouse events to VT escape sequences.
//!
//! This module translates high-level [`Key`] and [`MouseEvent`] values into
//! the byte sequences that a PTY child process expects.  It handles:
//!
//! - Ctrl/Alt/Shift modifiers (xterm-style modifier parameters).
//! - Application cursor mode (DECCKM) vs normal cursor mode.
//! - Newline mode (LNM) for Enter.
//! - Function keys F1-F12 (SS3 for F1-F4, CSI tilde for F5-F12).
//! - Mouse encoding in Normal (X10) and SGR (1006) modes.
//!
//! Use [`encode_key`] for keyboard input and [`encode_mouse`] for mouse events.

/// A key event to encode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Enter,
    Tab,
    Backspace,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    F(u8), // F1-F24
}

/// Modifier key state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    /// Shift key is pressed.
    pub shift: bool,
    /// Alt (Meta) key is pressed.
    pub alt: bool,
    /// Ctrl key is pressed.
    pub ctrl: bool,
}

impl Modifiers {
    /// Create a `Modifiers` with no modifier keys pressed.
    pub fn none() -> Self {
        Self::default()
    }

    /// Create a `Modifiers` with only Ctrl pressed.
    pub fn ctrl() -> Self {
        Self { ctrl: true, ..Default::default() }
    }

    /// Create a `Modifiers` with only Alt pressed.
    pub fn alt() -> Self {
        Self { alt: true, ..Default::default() }
    }

    /// Create a `Modifiers` with only Shift pressed.
    pub fn shift() -> Self {
        Self { shift: true, ..Default::default() }
    }

    /// Compute the xterm modifier parameter value: 1 + shift*1 + alt*2 + ctrl*4.
    fn param(self) -> u8 {
        let mut v = 1u8;
        if self.shift { v += 1; }
        if self.alt { v += 2; }
        if self.ctrl { v += 4; }
        v
    }

    fn any(self) -> bool {
        self.shift || self.alt || self.ctrl
    }
}

/// Mouse event to encode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseEvent {
    Press { button: u8, col: u16, row: u16 },
    Release { col: u16, row: u16 },
    Drag { button: u8, col: u16, row: u16 },
    ScrollUp { col: u16, row: u16 },
    ScrollDown { col: u16, row: u16 },
}

/// Mouse encoding mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseEncoding {
    /// X10 / Normal mode (CSI M + bytes).
    Normal,
    /// SGR mode (1006).
    Sgr,
}

/// Encode a key press into bytes to send to the PTY.
///
/// Parameters:
/// - `app_cursor`: DECCKM (application cursor keys) is active.
/// - `app_keypad`: DECKPAM (application keypad) is active.
/// - `newline_mode`: LNM (newline mode) is active.
/// - `disambiguate`: when true, use CSI u encoding for keys that would
///   otherwise be ambiguous with C0 control codes (e.g. Ctrl+I vs Tab).
pub fn encode_key(
    key: Key,
    mods: Modifiers,
    app_cursor: bool,
    _app_keypad: bool,
    newline_mode: bool,
    disambiguate: bool,
) -> Vec<u8> {
    match key {
        Key::Char(ch) => encode_char(ch, mods, disambiguate),
        Key::Enter => encode_enter(mods, newline_mode),
        Key::Tab => encode_tab(mods),
        Key::Backspace => encode_backspace(mods),
        Key::Escape => encode_escape(mods),
        Key::Up => encode_cursor(b'A', mods, app_cursor),
        Key::Down => encode_cursor(b'B', mods, app_cursor),
        Key::Left => encode_cursor(b'D', mods, app_cursor),
        Key::Right => encode_cursor(b'C', mods, app_cursor),
        Key::Home => encode_home_end(b'H', mods, app_cursor),
        Key::End => encode_home_end(b'F', mods, app_cursor),
        Key::PageUp => encode_tilde_key(5, mods),
        Key::PageDown => encode_tilde_key(6, mods),
        Key::Insert => encode_tilde_key(2, mods),
        Key::Delete => encode_tilde_key(3, mods),
        Key::F(n) => encode_function_key(n, mods),
    }
}

fn encode_char(ch: char, mods: Modifiers, disambiguate: bool) -> Vec<u8> {
    // Special handling for space
    if ch == ' ' {
        return encode_space(mods);
    }

    if mods.ctrl && !mods.alt && ch.is_ascii_lowercase() {
        // In disambiguate mode, Ctrl+i (which produces 0x09 = Tab) uses CSI u
        if disambiguate && ch == 'i' {
            let cp = ch as u32;
            let m = mods.param();
            return format!("\x1b[{cp};{m}u").into_bytes();
        }
        // Ctrl + lowercase letter => C0 control code
        let code = (ch as u8) - b'a' + 1;
        vec![code]
    } else if mods.ctrl && !mods.alt && ch.is_ascii_uppercase() {
        // Ctrl + uppercase letter => CSI u encoding
        let cp = ch as u32;
        let m = mods.param();
        format!("\x1b[{cp};{m}u").into_bytes()
    } else if mods.ctrl && mods.alt && ch.is_ascii_lowercase() {
        // Ctrl+Alt + lowercase => ESC + C0
        let code = (ch as u8) - b'a' + 1;
        vec![0x1B, code]
    } else if mods.ctrl && mods.alt && ch.is_ascii_uppercase() {
        // Ctrl+Alt + uppercase => CSI u encoding
        let cp = ch as u32;
        let m = mods.param();
        format!("\x1b[{cp};{m}u").into_bytes()
    } else if mods.alt && !mods.ctrl && !mods.shift {
        // Alt + char => ESC + char
        let mut buf = vec![0x1B];
        let mut tmp = [0u8; 4];
        buf.extend_from_slice(ch.encode_utf8(&mut tmp).as_bytes());
        buf
    } else if mods.any() && !mods.alt {
        // Other modifier combos for non-letter chars -> try CSI u
        if mods.shift && !mods.ctrl {
            // Shift alone on a letter is just the uppercase letter
            if ch.is_ascii_alphabetic() {
                let mut tmp = [0u8; 4];
                let s = ch.encode_utf8(&mut tmp);
                return s.as_bytes().to_vec();
            }
        }
        // For anything else with modifiers, literal char
        let mut tmp = [0u8; 4];
        let s = ch.encode_utf8(&mut tmp);
        s.as_bytes().to_vec()
    } else {
        // Unmodified or unsupported modifier combo: literal char
        let mut tmp = [0u8; 4];
        let s = ch.encode_utf8(&mut tmp);
        s.as_bytes().to_vec()
    }
}

fn encode_space(mods: Modifiers) -> Vec<u8> {
    if !mods.any() {
        return vec![b' '];
    }
    if mods.ctrl && !mods.alt && !mods.shift {
        return vec![0x00];
    }
    if mods.alt && !mods.ctrl && !mods.shift {
        return vec![0x1B, b' '];
    }
    if mods.ctrl && mods.alt && !mods.shift {
        return vec![0x1B, 0x00];
    }
    // Any other modifier combo with space -> CSI u
    let m = mods.param();
    format!("\x1b[32;{m}u").into_bytes()
}

fn encode_enter(_mods: Modifiers, newline_mode: bool) -> Vec<u8> {
    if newline_mode {
        vec![0x0D, 0x0A]
    } else {
        vec![0x0D]
    }
}

fn encode_tab(mods: Modifiers) -> Vec<u8> {
    if mods.shift && !mods.ctrl && !mods.alt {
        // Shift+Tab => reverse tab
        b"\x1b[Z".to_vec()
    } else if mods.alt && !mods.ctrl && !mods.shift {
        vec![0x1B, 0x09]
    } else {
        vec![0x09]
    }
}

fn encode_backspace(mods: Modifiers) -> Vec<u8> {
    if mods.ctrl && !mods.alt {
        vec![0x08]
    } else if mods.alt && !mods.ctrl {
        vec![0x1B, 0x7F]
    } else {
        vec![0x7F]
    }
}

fn encode_escape(_mods: Modifiers) -> Vec<u8> {
    vec![0x1B]
}

fn encode_cursor(suffix: u8, mods: Modifiers, app_cursor: bool) -> Vec<u8> {
    if mods.any() {
        // Modified cursor keys always use CSI format
        let m = mods.param();
        format!("\x1b[1;{}{}", m, suffix as char).into_bytes()
    } else if app_cursor {
        vec![0x1B, b'O', suffix]
    } else {
        vec![0x1B, b'[', suffix]
    }
}

fn encode_home_end(suffix: u8, mods: Modifiers, app_cursor: bool) -> Vec<u8> {
    encode_cursor(suffix, mods, app_cursor)
}

fn encode_tilde_key(code: u8, mods: Modifiers) -> Vec<u8> {
    if mods.any() {
        let m = mods.param();
        format!("\x1b[{code};{m}~").into_bytes()
    } else {
        format!("\x1b[{code}~").into_bytes()
    }
}

fn encode_function_key(n: u8, mods: Modifiers) -> Vec<u8> {
    match n {
        1..=4 => {
            let suffix = match n {
                1 => b'P',
                2 => b'Q',
                3 => b'R',
                4 => b'S',
                _ => unreachable!(),
            };
            if mods.any() {
                let m = mods.param();
                format!("\x1b[1;{}{}", m, suffix as char).into_bytes()
            } else {
                vec![0x1B, b'O', suffix]
            }
        }
        5..=12 => {
            let code = match n {
                5 => 15,
                6 => 17,
                7 => 18,
                8 => 19,
                9 => 20,
                10 => 21,
                11 => 23,
                12 => 24,
                _ => unreachable!(),
            };
            encode_tilde_key(code, mods)
        }
        _ => Vec::new(),
    }
}

/// Encode a mouse event.
pub fn encode_mouse(event: MouseEvent, mode: MouseEncoding) -> Vec<u8> {
    match mode {
        MouseEncoding::Normal => encode_mouse_normal(event),
        MouseEncoding::Sgr => encode_mouse_sgr(event),
    }
}

fn encode_mouse_normal(event: MouseEvent) -> Vec<u8> {
    let (button_byte, col, row) = match event {
        MouseEvent::Press { button, col, row } => (button + 32, col, row),
        MouseEvent::Release { col, row } => (3 + 32, col, row),
        MouseEvent::Drag { button, col, row } => (button + 32 + 32, col, row),
        MouseEvent::ScrollUp { col, row } => (64 + 32, col, row),
        MouseEvent::ScrollDown { col, row } => (65 + 32, col, row),
    };
    vec![0x1B, b'[', b'M', button_byte, (col as u8) + 33, (row as u8) + 33]
}

/// Keypad key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeypadKey {
    Num0, Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9,
    Decimal, Separator, Star, Plus, Minus, Slash, Enter, Equal,
}

/// Encode a keypad key press.
pub fn encode_keypad(key: KeypadKey, app_mode: bool) -> Vec<u8> {
    if app_mode {
        let suffix = match key {
            KeypadKey::Num0 => b'p',
            KeypadKey::Num1 => b'q',
            KeypadKey::Num2 => b'r',
            KeypadKey::Num3 => b's',
            KeypadKey::Num4 => b't',
            KeypadKey::Num5 => b'u',
            KeypadKey::Num6 => b'v',
            KeypadKey::Num7 => b'w',
            KeypadKey::Num8 => b'x',
            KeypadKey::Num9 => b'y',
            KeypadKey::Decimal => b'n',
            KeypadKey::Separator => b'l',
            KeypadKey::Star => return b"\x1bOj".to_vec(),
            KeypadKey::Plus => return b"\x1bOk".to_vec(),
            KeypadKey::Minus => b'm',
            KeypadKey::Slash => return b"\x1bOo".to_vec(),
            KeypadKey::Enter => b'M',
            KeypadKey::Equal => b'X',
        };
        vec![0x1B, b'O', suffix]
    } else {
        let ch = match key {
            KeypadKey::Num0 => '0',
            KeypadKey::Num1 => '1',
            KeypadKey::Num2 => '2',
            KeypadKey::Num3 => '3',
            KeypadKey::Num4 => '4',
            KeypadKey::Num5 => '5',
            KeypadKey::Num6 => '6',
            KeypadKey::Num7 => '7',
            KeypadKey::Num8 => '8',
            KeypadKey::Num9 => '9',
            KeypadKey::Decimal => '.',
            KeypadKey::Separator => ',',
            KeypadKey::Star => '*',
            KeypadKey::Plus => '+',
            KeypadKey::Minus => '-',
            KeypadKey::Slash => '/',
            KeypadKey::Enter => return vec![0x0D],
            KeypadKey::Equal => '=',
        };
        vec![ch as u8]
    }
}

/// Encode pasted text, wrapping in bracketed paste markers if enabled.
pub fn encode_paste(text: &str, bracketed: bool) -> Vec<u8> {
    if bracketed {
        let mut buf = Vec::with_capacity(text.len() + 12);
        buf.extend_from_slice(b"\x1b[200~");
        buf.extend_from_slice(text.as_bytes());
        buf.extend_from_slice(b"\x1b[201~");
        buf
    } else {
        text.as_bytes().to_vec()
    }
}

/// Encode a focus event.
pub fn encode_focus(focused: bool, reporting_enabled: bool) -> Vec<u8> {
    if reporting_enabled {
        if focused {
            b"\x1b[I".to_vec()
        } else {
            b"\x1b[O".to_vec()
        }
    } else {
        Vec::new()
    }
}

fn encode_mouse_sgr(event: MouseEvent) -> Vec<u8> {
    match event {
        MouseEvent::Press { button, col, row } => {
            format!("\x1b[<{};{};{}M", button, col + 1, row + 1).into_bytes()
        }
        MouseEvent::Release { col, row } => {
            format!("\x1b[<0;{};{}m", col + 1, row + 1).into_bytes()
        }
        MouseEvent::Drag { button, col, row } => {
            format!("\x1b[<{};{};{}M", button + 32, col + 1, row + 1).into_bytes()
        }
        MouseEvent::ScrollUp { col, row } => {
            format!("\x1b[<64;{};{}M", col + 1, row + 1).into_bytes()
        }
        MouseEvent::ScrollDown { col, row } => {
            format!("\x1b[<65;{};{}M", col + 1, row + 1).into_bytes()
        }
    }
}
