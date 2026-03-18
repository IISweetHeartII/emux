use std::io::Write;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use emux_mux::SplitDirection;
use emux_mux::tab::FocusDirection;
use emux_term::search;

use crate::AppError;
use crate::app::{Action, App, InputMode};
use crate::operations::{close_active_pane, new_tab, split_pane};

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
///
/// Examples:
///   "Leader+D"       → (CONTROL | SHIFT, Char('D'))
///   "Leader+Shift+D" → (CONTROL | SHIFT, Char('D'))  (Shift already in Leader)
///   "Ctrl+Tab"       → (CONTROL, Tab)
///   "Ctrl+Q"         → (CONTROL, Char('q'))
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
            // Single character or bracket/punctuation.
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
/// For `Char` bindings the comparison is case-insensitive so that
/// `Leader+D` matches both `Char('D')` and `Char('d')` when Shift is held.
pub(crate) fn matches_binding(key: &KeyEvent, binding: &Option<(KeyModifiers, KeyCode)>) -> bool {
    let Some((bind_mods, bind_code)) = binding else {
        return false;
    };
    if !key.modifiers.contains(*bind_mods) {
        return false;
    }
    match (bind_code, &key.code) {
        (KeyCode::Char(bc), KeyCode::Char(kc)) => {
            bc.eq_ignore_ascii_case(kc)
        }
        _ => bind_code == &key.code,
    }
}

// ---------------------------------------------------------------------------
// Keybinding handling — Leader is Ctrl+Shift
// ---------------------------------------------------------------------------

/// Returns an Action indicating what happened.
pub(crate) fn handle_keybinding<W: Write>(
    app: &mut App,
    key: &KeyEvent,
    _stdout: &mut W,
    _cols: u16,
    _rows: u16,
) -> Result<Action, AppError> {
    // ── Search mode ────────────────────────────────────────────────
    // While in search mode, keys are interpreted as search input rather
    // than normal keybindings.
    if app.input_mode == InputMode::Search {
        return handle_search_key(app, key);
    }

    let bindings = &app.bindings;

    // Detach / quit
    if matches_binding(key, &bindings.detach) {
        if app.daemon_mode {
            return Ok(Action::Detach);
        } else {
            return Ok(Action::Quit);
        }
    }

    // Enter search mode
    if matches_binding(key, &bindings.search) {
        app.input_mode = InputMode::Search;
        app.search_query.clear();
        app.search_state = search::SearchState::default();
        return Ok(Action::Consumed);
    }

    // Split down (horizontal split — panes stacked top/bottom)
    if matches_binding(key, &bindings.split_down) {
        split_pane(app, SplitDirection::Horizontal)?;
        return Ok(Action::Consumed);
    }
    // Split right (vertical split — panes side by side)
    if matches_binding(key, &bindings.split_right) {
        split_pane(app, SplitDirection::Vertical)?;
        return Ok(Action::Consumed);
    }
    // Close active pane
    if matches_binding(key, &bindings.close_pane) {
        close_active_pane(app);
        return Ok(Action::Consumed);
    }
    // Focus navigation
    if matches_binding(key, &bindings.focus_up) {
        app.session.active_tab_mut().focus_direction(FocusDirection::Up);
        return Ok(Action::Consumed);
    }
    if matches_binding(key, &bindings.focus_down) {
        app.session.active_tab_mut().focus_direction(FocusDirection::Down);
        return Ok(Action::Consumed);
    }
    if matches_binding(key, &bindings.focus_left) {
        app.session.active_tab_mut().focus_direction(FocusDirection::Left);
        return Ok(Action::Consumed);
    }
    if matches_binding(key, &bindings.focus_right) {
        app.session.active_tab_mut().focus_direction(FocusDirection::Right);
        return Ok(Action::Consumed);
    }
    // New tab
    if matches_binding(key, &bindings.new_tab) {
        new_tab(app)?;
        return Ok(Action::Consumed);
    }
    // Next / prev tab
    if matches_binding(key, &bindings.next_tab) {
        app.session.next_tab();
        return Ok(Action::Consumed);
    }
    if matches_binding(key, &bindings.prev_tab) {
        app.session.prev_tab();
        return Ok(Action::Consumed);
    }
    // Close tab
    if matches_binding(key, &bindings.close_tab) {
        let idx = app.session.active_tab_index();
        let pane_ids = app.session.active_tab().pane_ids();
        if app.session.close_tab(idx) {
            for id in pane_ids {
                app.panes.remove(&id);
            }
        }
        return Ok(Action::Consumed);
    }
    // Toggle fullscreen
    if matches_binding(key, &bindings.toggle_fullscreen) {
        app.session.active_tab_mut().toggle_fullscreen();
        return Ok(Action::Consumed);
    }
    // Toggle floating panes
    if matches_binding(key, &bindings.toggle_float) {
        app.session.active_tab_mut().toggle_floating_panes();
        return Ok(Action::Consumed);
    }

    Ok(Action::Forward)
}

/// Handle a key event while in search mode.
fn handle_search_key(app: &mut App, key: &KeyEvent) -> Result<Action, AppError> {
    match key.code {
        KeyCode::Esc | KeyCode::Enter => {
            // Exit search mode.
            app.input_mode = InputMode::Normal;
            return Ok(Action::Consumed);
        }
        KeyCode::Backspace => {
            app.search_query.pop();
            run_search(app);
            return Ok(Action::Consumed);
        }
        KeyCode::Char('n') if key.modifiers.is_empty() && !app.search_query.is_empty() && app.search_direction_active => {
            // Next match.
            app.search_state.current =
                search::next_match_index(app.search_state.current, app.search_state.matches.len());
            return Ok(Action::Consumed);
        }
        KeyCode::Char('N') if key.modifiers.contains(KeyModifiers::SHIFT) && !app.search_query.is_empty() && app.search_direction_active => {
            // Previous match.
            app.search_state.current =
                search::prev_match_index(app.search_state.current, app.search_state.matches.len());
            return Ok(Action::Consumed);
        }
        KeyCode::Char(c) => {
            app.search_query.push(c);
            run_search(app);
            // Once the user has typed something, n/N navigate matches.
            app.search_direction_active = true;
            return Ok(Action::Consumed);
        }
        _ => {}
    }
    Ok(Action::Consumed)
}

/// Execute a search over the active pane's viewport text and update
/// `app.search_state` with the results.
fn run_search(app: &mut App) {
    if app.search_query.is_empty() {
        app.search_state = search::SearchState::default();
        return;
    }

    // Collect row texts from the active pane's screen.
    let texts: Vec<String> = if let Some(active_id) = app.session.active_tab().active_pane_id()
        && let Some(ps) = app.panes.get(&active_id) {
            (0..ps.screen.rows())
                .map(|r| ps.screen.row_text(r))
                .collect()
        } else {
            return;
        };

    let matches = search::find_all_matches(&texts, &app.search_query, false);
    let current = if matches.is_empty() { None } else { Some(0) };
    app.search_state = search::SearchState {
        query: app.search_query.clone(),
        matches,
        current,
        case_sensitive: false,
        regex: false,
    };
}
