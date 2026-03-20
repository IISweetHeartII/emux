use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use emux_term::Screen;

pub(crate) fn translate_key(event: KeyEvent, screen: &Screen) -> Vec<u8> {
    use emux_term::input::{Key, Modifiers, encode_key};

    let mods = Modifiers {
        shift: event.modifiers.contains(KeyModifiers::SHIFT),
        alt: event.modifiers.contains(KeyModifiers::ALT),
        ctrl: event.modifiers.contains(KeyModifiers::CONTROL),
    };

    let key = match event.code {
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Enter => Key::Enter,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Tab => Key::Tab,
        KeyCode::Esc => Key::Escape,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Insert => Key::Insert,
        KeyCode::Delete => Key::Delete,
        KeyCode::F(n) => Key::F(n),
        _ => return vec![],
    };

    let app_cursor = screen.modes.application_cursor_keys;
    let app_keypad = screen.modes.application_keypad;
    let newline_mode = screen.modes.newline;

    encode_key(key, mods, app_cursor, app_keypad, newline_mode, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_screen() -> Screen {
        Screen::new(80, 24)
    }

    #[test]
    fn translate_printable_char() {
        let event = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty());
        let bytes = translate_key(event, &default_screen());
        assert_eq!(bytes, b"a");
    }

    #[test]
    fn translate_enter() {
        let event = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
        let bytes = translate_key(event, &default_screen());
        assert_eq!(bytes, b"\r");
    }

    #[test]
    fn translate_ctrl_c() {
        let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        let bytes = translate_key(event, &default_screen());
        assert_eq!(bytes, &[0x03]); // ETX
    }

    #[test]
    fn translate_escape() {
        let event = KeyEvent::new(KeyCode::Esc, KeyModifiers::empty());
        let bytes = translate_key(event, &default_screen());
        assert_eq!(bytes, &[0x1b]);
    }

    #[test]
    fn translate_arrow_up() {
        let event = KeyEvent::new(KeyCode::Up, KeyModifiers::empty());
        let bytes = translate_key(event, &default_screen());
        assert_eq!(bytes, b"\x1b[A");
    }

    #[test]
    fn translate_unknown_key_returns_empty() {
        let event = KeyEvent::new(KeyCode::CapsLock, KeyModifiers::empty());
        let bytes = translate_key(event, &default_screen());
        assert!(bytes.is_empty());
    }

    #[test]
    fn translate_tab() {
        let event = KeyEvent::new(KeyCode::Tab, KeyModifiers::empty());
        let bytes = translate_key(event, &default_screen());
        assert_eq!(bytes, b"\t");
    }

    #[test]
    fn translate_f1() {
        let event = KeyEvent::new(KeyCode::F(1), KeyModifiers::empty());
        let bytes = translate_key(event, &default_screen());
        assert!(!bytes.is_empty());
        // F1 = ESC O P or ESC [ 1 1 ~
        assert!(bytes.starts_with(b"\x1b"));
    }
}
