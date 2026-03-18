//! Terminal mode flags (DEC private modes, ANSI modes).

use serde::{Deserialize, Serialize};

/// Mouse tracking mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MouseMode {
    #[default]
    None,
    /// Mode 9: X10 compatibility mouse reporting.
    X10,
    /// Mode 1000: Normal tracking mode.
    Normal,
    /// Mode 1002: Button-event tracking.
    ButtonEvent,
    /// Mode 1003: Any-event tracking.
    AnyEvent,
}

/// Kitty keyboard protocol flags (progressive enhancement).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct KittyKeyboardFlags {
    pub bits: u8,
}

impl KittyKeyboardFlags {
    pub fn new() -> Self {
        Self { bits: 0 }
    }

    pub fn disambiguate_escape(&self) -> bool {
        self.bits & 1 != 0
    }

    pub fn report_event_types(&self) -> bool {
        self.bits & 2 != 0
    }

    pub fn report_alternate_keys(&self) -> bool {
        self.bits & 4 != 0
    }

    pub fn report_all_keys_as_escape_codes(&self) -> bool {
        self.bits & 8 != 0
    }

    pub fn report_associated_text(&self) -> bool {
        self.bits & 16 != 0
    }
}

/// Terminal mode flags.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Modes {
    /// DECCKM (mode 1): Application cursor keys.
    pub application_cursor_keys: bool,
    /// DECKPAM/DECKPNM: Application keypad mode.
    pub application_keypad: bool,
    /// DECAWM: Auto-wrap mode.
    pub autowrap: bool,
    /// DECOM: Origin mode.
    pub origin: bool,
    /// IRM: Insert/Replace mode.
    pub insert: bool,
    /// LNM: Line feed/new line mode.
    pub newline: bool,
    /// Mode 1049: Alternate screen buffer.
    pub alt_screen: bool,
    /// Mode 2004: Bracketed paste.
    pub bracketed_paste: bool,
    /// Mode 1004: Focus tracking.
    pub focus_tracking: bool,
    /// Mouse tracking mode.
    pub mouse_tracking: MouseMode,
    /// Mode 1006: SGR extended mouse encoding.
    pub mouse_sgr: bool,
    /// Kitty keyboard protocol flags.
    pub keyboard_mode: KittyKeyboardFlags,
    /// DEC mode 40: allow 80/132 column switching (DECCOLM).
    pub allow_deccolm: bool,
    /// DEC mode 69: left/right margin mode (DECLRMM).
    pub left_right_margin: bool,
    /// DEC mode 45: reverse wrap mode.
    pub reverse_wrap: bool,
}

impl Default for Modes {
    fn default() -> Self {
        Self {
            application_cursor_keys: false,
            application_keypad: false,
            autowrap: true,
            origin: false,
            insert: false,
            newline: false,
            alt_screen: false,
            bracketed_paste: false,
            focus_tracking: false,
            mouse_tracking: MouseMode::None,
            mouse_sgr: false,
            keyboard_mode: KittyKeyboardFlags::new(),
            allow_deccolm: false,
            left_right_margin: false,
            reverse_wrap: false,
        }
    }
}
