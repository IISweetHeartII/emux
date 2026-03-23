use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use emux_mux::SplitDirection;
use emux_mux::tab::FocusDirection;

use crate::app::InputMode;
use crate::command::Command;

// ---------------------------------------------------------------------------
// Parsed keybinding cache
// ---------------------------------------------------------------------------

/// Pre-parsed keybindings so we don't re-parse strings on every key event.
pub(crate) struct ParsedBindings {
    pub(crate) split_down: Option<(KeyModifiers, KeyCode)>,
    pub(crate) split_right: Option<(KeyModifiers, KeyCode)>,
    pub(crate) close_pane: Option<(KeyModifiers, KeyCode)>,
    pub(crate) focus_up: Option<(KeyModifiers, KeyCode)>,
    pub(crate) focus_down: Option<(KeyModifiers, KeyCode)>,
    pub(crate) focus_left: Option<(KeyModifiers, KeyCode)>,
    pub(crate) focus_right: Option<(KeyModifiers, KeyCode)>,
    pub(crate) new_tab: Option<(KeyModifiers, KeyCode)>,
    pub(crate) close_tab: Option<(KeyModifiers, KeyCode)>,
    pub(crate) next_tab: Option<(KeyModifiers, KeyCode)>,
    pub(crate) prev_tab: Option<(KeyModifiers, KeyCode)>,
    pub(crate) detach: Option<(KeyModifiers, KeyCode)>,
    pub(crate) search: Option<(KeyModifiers, KeyCode)>,
    pub(crate) toggle_fullscreen: Option<(KeyModifiers, KeyCode)>,
    pub(crate) toggle_float: Option<(KeyModifiers, KeyCode)>,
    pub(crate) scroll_up: Option<(KeyModifiers, KeyCode)>,
    pub(crate) scroll_down: Option<(KeyModifiers, KeyCode)>,
    pub(crate) copy_mode: Option<(KeyModifiers, KeyCode)>,
}

impl ParsedBindings {
    pub(crate) fn from_config(keys: &emux_config::KeyBindings) -> Self {
        Self {
            split_down: parse_keybinding(&keys.split_down),
            split_right: parse_keybinding(&keys.split_right),
            close_pane: parse_keybinding(&keys.close_pane),
            focus_up: parse_keybinding(&keys.focus_up),
            focus_down: parse_keybinding(&keys.focus_down),
            focus_left: parse_keybinding(&keys.focus_left),
            focus_right: parse_keybinding(&keys.focus_right),
            new_tab: parse_keybinding(&keys.new_tab),
            close_tab: parse_keybinding(&keys.close_tab),
            next_tab: parse_keybinding(&keys.next_tab),
            prev_tab: parse_keybinding(&keys.prev_tab),
            detach: parse_keybinding(&keys.detach),
            search: parse_keybinding(&keys.search),
            toggle_fullscreen: parse_keybinding(&keys.toggle_fullscreen),
            toggle_float: parse_keybinding(&keys.toggle_float),
            scroll_up: parse_keybinding(&keys.scroll_up),
            scroll_down: parse_keybinding(&keys.scroll_down),
            copy_mode: parse_keybinding(&keys.copy_mode),
        }
    }
}

/// Parse a keybinding string like `"Leader+D"` into `(KeyModifiers, KeyCode)`.
///
/// "Leader" is treated as Ctrl+Shift. Additional modifiers (Ctrl, Shift, Alt)
/// can be combined. The final segment is the key name:
///   - Single character → `KeyCode::Char(c)`
///   - Special names: Up, Down, Left, Right, Tab, Enter, Esc, Backspace, etc.
///   - `F1`..`F12` → `KeyCode::F(n)`
pub(crate) fn parse_keybinding(binding: &str) -> Option<(KeyModifiers, KeyCode)> {
    let parts: Vec<&str> = binding.split('+').collect();
    if parts.is_empty() {
        return None;
    }

    let mut mods = KeyModifiers::empty();
    let mut key_part: Option<&str> = None;

    for part in &parts {
        let normalized = part.trim();
        match normalized.to_lowercase().as_str() {
            "leader" => {
                mods |= KeyModifiers::CONTROL | KeyModifiers::SHIFT;
            }
            "ctrl" | "control" => {
                mods |= KeyModifiers::CONTROL;
            }
            "shift" => {
                mods |= KeyModifiers::SHIFT;
            }
            "alt" | "meta" | "opt" | "option" => {
                mods |= KeyModifiers::ALT;
            }
            _ => {
                key_part = Some(normalized);
            }
        }
    }

    let key_str = key_part?;
    let code = match key_str.to_lowercase().as_str() {
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "tab" => KeyCode::Tab,
        "enter" | "return" => KeyCode::Enter,
        "esc" | "escape" => KeyCode::Esc,
        "backspace" => KeyCode::Backspace,
        "delete" | "del" => KeyCode::Delete,
        "insert" | "ins" => KeyCode::Insert,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        s if s.starts_with('f') && s.len() > 1 => {
            if let Ok(n) = s[1..].parse::<u8>() {
                KeyCode::F(n)
            } else {
                return None;
            }
        }
        _ => {
            let chars: Vec<char> = key_str.chars().collect();
            if chars.len() == 1 {
                KeyCode::Char(chars[0])
            } else {
                return None;
            }
        }
    };

    Some((mods, code))
}

/// Check whether a key event matches a parsed binding.
pub(crate) fn matches_binding(key: &KeyEvent, binding: &Option<(KeyModifiers, KeyCode)>) -> bool {
    let Some((bind_mods, bind_code)) = binding else {
        return false;
    };
    if !key.modifiers.contains(*bind_mods) {
        return false;
    }
    match (bind_code, &key.code) {
        (KeyCode::Char(bc), KeyCode::Char(kc)) => bc.eq_ignore_ascii_case(kc),
        _ => bind_code == &key.code,
    }
}

// ---------------------------------------------------------------------------
// Context for resolve_keybinding (read-only state needed for decisions)
// ---------------------------------------------------------------------------

/// Minimal read-only context needed by resolve_keybinding to make decisions
/// without access to the full App.
pub(crate) struct ResolveContext<'a> {
    pub input_mode: InputMode,
    pub bindings: &'a ParsedBindings,
    pub daemon_mode: bool,
    pub search_query: &'a str,
    pub search_direction_active: bool,
    /// Key-to-bytes translation for ForwardKeyToPty (pre-computed by caller).
    pub forward_bytes: Vec<u8>,
}

// ---------------------------------------------------------------------------
// Pure resolve: KeyEvent → Command (no mutation)
// ---------------------------------------------------------------------------

/// Pure function: given a key event and read-only context, returns a Command.
/// This function never mutates App — the caller feeds the Command to dispatch().
pub(crate) fn resolve_keybinding(key: &KeyEvent, ctx: &ResolveContext) -> Command {
    match ctx.input_mode {
        InputMode::Copy => resolve_copy_key(key),
        InputMode::Search => resolve_search_key(key, ctx),
        InputMode::Normal => resolve_normal_key(key, ctx),
    }
}

fn resolve_normal_key(key: &KeyEvent, ctx: &ResolveContext) -> Command {
    let bindings = ctx.bindings;

    if matches_binding(key, &bindings.detach) {
        return if ctx.daemon_mode {
            Command::Detach
        } else {
            Command::Quit
        };
    }
    if matches_binding(key, &bindings.search) {
        return Command::EnterSearchMode;
    }
    if matches_binding(key, &bindings.split_down) {
        return Command::SplitPane(SplitDirection::Horizontal);
    }
    if matches_binding(key, &bindings.split_right) {
        return Command::SplitPane(SplitDirection::Vertical);
    }
    if matches_binding(key, &bindings.close_pane) {
        return Command::CloseActivePane;
    }
    if matches_binding(key, &bindings.focus_up) {
        return Command::FocusDirection(FocusDirection::Up);
    }
    if matches_binding(key, &bindings.focus_down) {
        return Command::FocusDirection(FocusDirection::Down);
    }
    if matches_binding(key, &bindings.focus_left) {
        return Command::FocusDirection(FocusDirection::Left);
    }
    if matches_binding(key, &bindings.focus_right) {
        return Command::FocusDirection(FocusDirection::Right);
    }
    if matches_binding(key, &bindings.new_tab) {
        return Command::NewTab;
    }
    if matches_binding(key, &bindings.next_tab) {
        return Command::NextTab;
    }
    if matches_binding(key, &bindings.prev_tab) {
        return Command::PrevTab;
    }
    if matches_binding(key, &bindings.close_tab) {
        return Command::CloseTab;
    }
    if matches_binding(key, &bindings.toggle_fullscreen) {
        return Command::ToggleFullscreen;
    }
    if matches_binding(key, &bindings.toggle_float) {
        return Command::ToggleFloat;
    }
    if matches_binding(key, &bindings.scroll_up) {
        return Command::ScrollUp(0); // 0 = half-page (dispatch computes)
    }
    if matches_binding(key, &bindings.scroll_down) {
        return Command::ScrollDown(0);
    }
    if matches_binding(key, &bindings.copy_mode) {
        return Command::EnterCopyMode;
    }

    // No binding matched — forward to PTY.
    Command::ForwardKeyToPty(ctx.forward_bytes.clone())
}

fn resolve_search_key(key: &KeyEvent, ctx: &ResolveContext) -> Command {
    match key.code {
        KeyCode::Esc | KeyCode::Enter => Command::ExitSearchMode,
        KeyCode::Backspace => Command::SearchDeleteChar,
        KeyCode::Char('n')
            if key.modifiers.is_empty()
                && !ctx.search_query.is_empty()
                && ctx.search_direction_active =>
        {
            Command::SearchNextMatch
        }
        KeyCode::Char('N')
            if key.modifiers.contains(KeyModifiers::SHIFT)
                && !ctx.search_query.is_empty()
                && ctx.search_direction_active =>
        {
            Command::SearchPrevMatch
        }
        KeyCode::Char(c) => Command::SearchAppendChar(c),
        _ => Command::ExitSearchMode, // Ignore unknown keys, stay in search
    }
}

fn resolve_copy_key(key: &KeyEvent) -> Command {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => Command::ExitCopyMode,
        KeyCode::Char('h') | KeyCode::Left => Command::CopyMoveLeft,
        KeyCode::Char('j') | KeyCode::Down => Command::CopyMoveDown,
        KeyCode::Char('k') | KeyCode::Up => Command::CopyMoveUp,
        KeyCode::Char('l') | KeyCode::Right => Command::CopyMoveRight,
        KeyCode::Char('0') => Command::CopyStartOfLine,
        KeyCode::Char('$') => Command::CopyEndOfLine,
        KeyCode::Char('g') => Command::CopyGotoTop,
        KeyCode::Char('G') => Command::CopyGotoBottom,
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Command::CopyHalfPageUp
        }
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Command::CopyHalfPageDown
        }
        KeyCode::Char('v') => Command::CopyToggleSelection,
        KeyCode::Char('y') => Command::CopyYank,
        _ => Command::ExitCopyMode, // Unknown key exits copy mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn default_bindings() -> ParsedBindings {
        ParsedBindings::from_config(&emux_config::KeyBindings::default())
    }

    fn ctx_normal(bindings: &ParsedBindings) -> ResolveContext {
        ResolveContext {
            input_mode: InputMode::Normal,
            bindings,
            daemon_mode: false,
            search_query: "",
            search_direction_active: false,
            forward_bytes: vec![b'x'],
        }
    }

    fn ctx_search<'a>(
        bindings: &'a ParsedBindings,
        query: &'a str,
        direction_active: bool,
    ) -> ResolveContext<'a> {
        ResolveContext {
            input_mode: InputMode::Search,
            bindings,
            daemon_mode: false,
            search_query: query,
            search_direction_active: direction_active,
            forward_bytes: Vec::new(),
        }
    }

    fn ctx_copy(bindings: &ParsedBindings) -> ResolveContext {
        ResolveContext {
            input_mode: InputMode::Copy,
            bindings,
            daemon_mode: false,
            search_query: "",
            search_direction_active: false,
            forward_bytes: Vec::new(),
        }
    }

    // ── Parsing tests (existing) ────────────────────────────────────

    #[test]
    fn parse_leader_d() {
        let (mods, code) = parse_keybinding("Leader+D").unwrap();
        assert!(mods.contains(KeyModifiers::CONTROL));
        assert!(mods.contains(KeyModifiers::SHIFT));
        assert_eq!(code, KeyCode::Char('D'));
    }

    #[test]
    fn parse_ctrl_q() {
        let (mods, code) = parse_keybinding("Ctrl+Q").unwrap();
        assert!(mods.contains(KeyModifiers::CONTROL));
        assert!(!mods.contains(KeyModifiers::SHIFT));
        assert_eq!(code, KeyCode::Char('Q'));
    }

    #[test]
    fn parse_alt_tab() {
        let (mods, code) = parse_keybinding("Alt+Tab").unwrap();
        assert!(mods.contains(KeyModifiers::ALT));
        assert_eq!(code, KeyCode::Tab);
    }

    #[test]
    fn parse_f_keys() {
        let (_, code) = parse_keybinding("F1").unwrap();
        assert_eq!(code, KeyCode::F(1));
        let (_, code) = parse_keybinding("F12").unwrap();
        assert_eq!(code, KeyCode::F(12));
    }

    #[test]
    fn parse_special_keys() {
        assert_eq!(parse_keybinding("Up").unwrap().1, KeyCode::Up);
        assert_eq!(parse_keybinding("PageUp").unwrap().1, KeyCode::PageUp);
        assert_eq!(parse_keybinding("Enter").unwrap().1, KeyCode::Enter);
        assert_eq!(parse_keybinding("Backspace").unwrap().1, KeyCode::Backspace);
        assert_eq!(parse_keybinding("Esc").unwrap().1, KeyCode::Esc);
    }

    #[test]
    fn parse_leader_pageup() {
        let (mods, code) = parse_keybinding("Leader+PageUp").unwrap();
        assert!(mods.contains(KeyModifiers::CONTROL));
        assert!(mods.contains(KeyModifiers::SHIFT));
        assert_eq!(code, KeyCode::PageUp);
    }

    #[test]
    fn parse_invalid_returns_none() {
        assert!(parse_keybinding("").is_none());
        assert!(parse_keybinding("Leader+").is_none());
    }

    #[test]
    fn matches_binding_case_insensitive() {
        let binding = Some((
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            KeyCode::Char('D'),
        ));
        let key = KeyEvent::new(
            KeyCode::Char('d'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert!(matches_binding(&key, &binding));
    }

    #[test]
    fn matches_binding_none_returns_false() {
        let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty());
        assert!(!matches_binding(&key, &None));
    }

    #[test]
    fn matches_binding_wrong_key_returns_false() {
        let binding = Some((KeyModifiers::CONTROL, KeyCode::Char('q')));
        let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL);
        assert!(!matches_binding(&key, &binding));
    }

    #[test]
    fn matches_binding_missing_modifier_returns_false() {
        let binding = Some((KeyModifiers::CONTROL, KeyCode::Char('q')));
        let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::empty());
        assert!(!matches_binding(&key, &binding));
    }

    // ── resolve_keybinding tests (new) ──────────────────────────────

    #[test]
    fn resolve_detach_in_daemon_mode() {
        let bindings = default_bindings();
        let mut ctx = ctx_normal(&bindings);
        ctx.daemon_mode = true;
        // Default detach is Leader+Q
        let key = KeyEvent::new(
            KeyCode::Char('Q'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert!(matches!(resolve_keybinding(&key, &ctx), Command::Detach));
    }

    #[test]
    fn resolve_detach_in_standalone_quits() {
        let bindings = default_bindings();
        let ctx = ctx_normal(&bindings);
        // Default detach is Leader+Q — standalone mode returns Quit
        let key = KeyEvent::new(
            KeyCode::Char('Q'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert!(matches!(resolve_keybinding(&key, &ctx), Command::Quit));
    }

    #[test]
    fn resolve_split_down() {
        let bindings = default_bindings();
        let ctx = ctx_normal(&bindings);
        // Default split_down is Leader+D
        let key = KeyEvent::new(
            KeyCode::Char('D'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert!(matches!(
            resolve_keybinding(&key, &ctx),
            Command::SplitPane(SplitDirection::Horizontal)
        ));
    }

    #[test]
    fn resolve_unmatched_forwards_to_pty() {
        let bindings = default_bindings();
        let ctx = ctx_normal(&bindings);
        let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty());
        match resolve_keybinding(&key, &ctx) {
            Command::ForwardKeyToPty(bytes) => assert_eq!(bytes, vec![b'x']),
            other => panic!(
                "expected ForwardKeyToPty, got {:?}",
                std::mem::discriminant(&other)
            ),
        }
    }

    #[test]
    fn resolve_search_esc_exits() {
        let bindings = default_bindings();
        let ctx = ctx_search(&bindings, "test", true);
        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::empty());
        assert!(matches!(
            resolve_keybinding(&key, &ctx),
            Command::ExitSearchMode
        ));
    }

    #[test]
    fn resolve_search_char_appends() {
        let bindings = default_bindings();
        let ctx = ctx_search(&bindings, "", false);
        let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty());
        assert!(matches!(
            resolve_keybinding(&key, &ctx),
            Command::SearchAppendChar('a')
        ));
    }

    #[test]
    fn resolve_search_backspace_deletes() {
        let bindings = default_bindings();
        let ctx = ctx_search(&bindings, "ab", true);
        let key = KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty());
        assert!(matches!(
            resolve_keybinding(&key, &ctx),
            Command::SearchDeleteChar
        ));
    }

    #[test]
    fn resolve_search_n_next_match() {
        let bindings = default_bindings();
        let ctx = ctx_search(&bindings, "test", true);
        let key = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty());
        assert!(matches!(
            resolve_keybinding(&key, &ctx),
            Command::SearchNextMatch
        ));
    }

    #[test]
    fn resolve_search_shift_n_prev_match() {
        let bindings = default_bindings();
        let ctx = ctx_search(&bindings, "test", true);
        let key = KeyEvent::new(KeyCode::Char('N'), KeyModifiers::SHIFT);
        assert!(matches!(
            resolve_keybinding(&key, &ctx),
            Command::SearchPrevMatch
        ));
    }

    #[test]
    fn resolve_copy_hjkl() {
        let bindings = default_bindings();
        let ctx = ctx_copy(&bindings);

        let h = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::empty());
        assert!(matches!(
            resolve_keybinding(&h, &ctx),
            Command::CopyMoveLeft
        ));

        let j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
        assert!(matches!(
            resolve_keybinding(&j, &ctx),
            Command::CopyMoveDown
        ));

        let k = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::empty());
        assert!(matches!(resolve_keybinding(&k, &ctx), Command::CopyMoveUp));

        let l = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::empty());
        assert!(matches!(
            resolve_keybinding(&l, &ctx),
            Command::CopyMoveRight
        ));
    }

    #[test]
    fn resolve_copy_v_toggles_selection() {
        let bindings = default_bindings();
        let ctx = ctx_copy(&bindings);
        let key = KeyEvent::new(KeyCode::Char('v'), KeyModifiers::empty());
        assert!(matches!(
            resolve_keybinding(&key, &ctx),
            Command::CopyToggleSelection
        ));
    }

    #[test]
    fn resolve_copy_y_yanks() {
        let bindings = default_bindings();
        let ctx = ctx_copy(&bindings);
        let key = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::empty());
        assert!(matches!(resolve_keybinding(&key, &ctx), Command::CopyYank));
    }

    #[test]
    fn resolve_copy_esc_exits() {
        let bindings = default_bindings();
        let ctx = ctx_copy(&bindings);
        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::empty());
        assert!(matches!(
            resolve_keybinding(&key, &ctx),
            Command::ExitCopyMode
        ));
    }
}
