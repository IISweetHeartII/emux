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
