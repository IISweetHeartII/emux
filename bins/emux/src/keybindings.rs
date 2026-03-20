use std::io::Write;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use emux_mux::tab::FocusDirection;
use emux_mux::{PaneId, SplitDirection};
use emux_term::search;
use emux_term::selection::{Selection, SelectionMode, SelectionPoint};

use crate::AppError;
use crate::app::{Action, App, CopyModeState, InputMode};
use crate::operations::{close_active_pane, mark_all_dirty, new_tab, split_pane};

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
        (KeyCode::Char(bc), KeyCode::Char(kc)) => bc.eq_ignore_ascii_case(kc),
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
    // ── Copy mode ─────────────────────────────────────────────────
    if app.input_mode == InputMode::Copy {
        return handle_copy_key(app, key);
    }

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
        app.session
            .active_tab_mut()
            .focus_direction(FocusDirection::Up);
        mark_all_dirty(app);
        return Ok(Action::Consumed);
    }
    if matches_binding(key, &bindings.focus_down) {
        app.session
            .active_tab_mut()
            .focus_direction(FocusDirection::Down);
        mark_all_dirty(app);
        return Ok(Action::Consumed);
    }
    if matches_binding(key, &bindings.focus_left) {
        app.session
            .active_tab_mut()
            .focus_direction(FocusDirection::Left);
        mark_all_dirty(app);
        return Ok(Action::Consumed);
    }
    if matches_binding(key, &bindings.focus_right) {
        app.session
            .active_tab_mut()
            .focus_direction(FocusDirection::Right);
        mark_all_dirty(app);
        return Ok(Action::Consumed);
    }
    // New tab
    if matches_binding(key, &bindings.new_tab) {
        new_tab(app)?;
        mark_all_dirty(app);
        return Ok(Action::Consumed);
    }
    // Next / prev tab
    if matches_binding(key, &bindings.next_tab) {
        app.session.next_tab();
        mark_all_dirty(app);
        return Ok(Action::Consumed);
    }
    if matches_binding(key, &bindings.prev_tab) {
        app.session.prev_tab();
        mark_all_dirty(app);
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
        mark_all_dirty(app);
        return Ok(Action::Consumed);
    }
    // Toggle fullscreen
    if matches_binding(key, &bindings.toggle_fullscreen) {
        app.session.active_tab_mut().toggle_fullscreen();
        mark_all_dirty(app);
        return Ok(Action::Consumed);
    }
    // Toggle floating panes
    if matches_binding(key, &bindings.toggle_float) {
        app.session.active_tab_mut().toggle_floating_panes();
        mark_all_dirty(app);
        return Ok(Action::Consumed);
    }
    // Scroll viewport up (view history)
    if matches_binding(key, &bindings.scroll_up) {
        if let Some(active_id) = app.session.active_tab().active_pane_id() {
            if let Some(ps) = app.panes.get_mut(&active_id) {
                let half_page = ps.screen.rows() / 2;
                ps.screen.scroll_viewport_up(half_page.max(1));
                ps.damage.mark_all();
            }
        }
        return Ok(Action::Consumed);
    }
    // Scroll viewport down (back toward live output)
    if matches_binding(key, &bindings.scroll_down) {
        if let Some(active_id) = app.session.active_tab().active_pane_id() {
            if let Some(ps) = app.panes.get_mut(&active_id) {
                let half_page = ps.screen.rows() / 2;
                ps.screen.scroll_viewport_down(half_page.max(1));
                ps.damage.mark_all();
            }
        }
        return Ok(Action::Consumed);
    }
    // Enter copy mode
    if matches_binding(key, &bindings.copy_mode) {
        if let Some(active_id) = app.session.active_tab().active_pane_id() {
            if let Some(ps) = app.panes.get_mut(&active_id) {
                app.input_mode = InputMode::Copy;
                app.copy_mode = Some(CopyModeState {
                    row: ps.screen.cursor.row,
                    col: ps.screen.cursor.col,
                    scrollback_len: ps.screen.grid.scrollback_len(),
                    selection: None,
                });
                ps.damage.mark_all();
            }
        }
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
        KeyCode::Char('n')
            if key.modifiers.is_empty()
                && !app.search_query.is_empty()
                && app.search_direction_active =>
        {
            // Next match.
            app.search_state.current =
                search::next_match_index(app.search_state.current, app.search_state.matches.len());
            return Ok(Action::Consumed);
        }
        KeyCode::Char('N')
            if key.modifiers.contains(KeyModifiers::SHIFT)
                && !app.search_query.is_empty()
                && app.search_direction_active =>
        {
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
        && let Some(ps) = app.panes.get(&active_id)
    {
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

// ---------------------------------------------------------------------------
// Copy mode — vi-style navigation and text selection
// ---------------------------------------------------------------------------

/// Handle a key event while in copy mode.
fn handle_copy_key(app: &mut App, key: &KeyEvent) -> Result<Action, AppError> {
    let active_id = app.session.active_tab().active_pane_id();
    let Some(active_id) = active_id else {
        return Ok(Action::Consumed);
    };

    match key.code {
        // Exit copy mode
        KeyCode::Esc | KeyCode::Char('q') => {
            app.input_mode = InputMode::Normal;
            app.copy_mode = None;
            if let Some(ps) = app.panes.get_mut(&active_id) {
                ps.damage.mark_all();
            }
        }
        // Movement: h/j/k/l and arrows
        KeyCode::Char('h') | KeyCode::Left => {
            if let Some(ref mut cm) = app.copy_mode {
                cm.col = cm.col.saturating_sub(1);
                update_copy_selection(cm);
                mark_active_dirty(app, active_id);
            }
        }
        KeyCode::Char('j') | KeyCode::Down => {
            if let Some(ref mut cm) = app.copy_mode {
                if let Some(ps) = app.panes.get(&active_id) {
                    cm.row = (cm.row + 1).min(ps.screen.rows().saturating_sub(1));
                }
                update_copy_selection(cm);
                mark_active_dirty(app, active_id);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if let Some(ref mut cm) = app.copy_mode {
                cm.row = cm.row.saturating_sub(1);
                update_copy_selection(cm);
                mark_active_dirty(app, active_id);
            }
        }
        KeyCode::Char('l') | KeyCode::Right => {
            if let Some(ref mut cm) = app.copy_mode {
                if let Some(ps) = app.panes.get(&active_id) {
                    cm.col = (cm.col + 1).min(ps.screen.cols().saturating_sub(1));
                }
                update_copy_selection(cm);
                mark_active_dirty(app, active_id);
            }
        }
        // Start of line
        KeyCode::Char('0') => {
            if let Some(ref mut cm) = app.copy_mode {
                cm.col = 0;
                update_copy_selection(cm);
                mark_active_dirty(app, active_id);
            }
        }
        // End of line
        KeyCode::Char('$') => {
            if let Some(ref mut cm) = app.copy_mode {
                if let Some(ps) = app.panes.get(&active_id) {
                    cm.col = ps.screen.cols().saturating_sub(1);
                }
                update_copy_selection(cm);
                mark_active_dirty(app, active_id);
            }
        }
        // Top of screen
        KeyCode::Char('g') => {
            if let Some(ref mut cm) = app.copy_mode {
                cm.row = 0;
                update_copy_selection(cm);
                mark_active_dirty(app, active_id);
            }
        }
        // Bottom of screen
        KeyCode::Char('G') => {
            if let Some(ref mut cm) = app.copy_mode {
                if let Some(ps) = app.panes.get(&active_id) {
                    cm.row = ps.screen.rows().saturating_sub(1);
                }
                update_copy_selection(cm);
                mark_active_dirty(app, active_id);
            }
        }
        // Half-page up/down
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if let Some(ref mut cm) = app.copy_mode {
                if let Some(ps) = app.panes.get(&active_id) {
                    let half = ps.screen.rows() / 2;
                    cm.row = cm.row.saturating_sub(half);
                }
                update_copy_selection(cm);
                mark_active_dirty(app, active_id);
            }
        }
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if let Some(ref mut cm) = app.copy_mode {
                if let Some(ps) = app.panes.get(&active_id) {
                    let half = ps.screen.rows() / 2;
                    cm.row = (cm.row + half).min(ps.screen.rows().saturating_sub(1));
                }
                update_copy_selection(cm);
                mark_active_dirty(app, active_id);
            }
        }
        // Toggle selection (v = normal, V = line — both use normal for simplicity)
        KeyCode::Char('v') => {
            if let Some(ref mut cm) = app.copy_mode {
                if cm.selection.is_some() {
                    cm.selection = None;
                } else {
                    let point = SelectionPoint::new(cm.scrollback_len + cm.row, cm.col);
                    cm.selection = Some(Selection::start(point, SelectionMode::Normal));
                }
                mark_active_dirty(app, active_id);
            }
        }
        // Yank (copy) selected text
        KeyCode::Char('y') => {
            let mut yanked_text = String::new();
            if let Some(ref mut cm) = app.copy_mode {
                if let Some(ref mut sel) = cm.selection {
                    sel.finalize();
                    if let Some(ps) = app.panes.get(&active_id) {
                        yanked_text = sel.get_text(&ps.screen.grid);
                    }
                }
            }
            // Store yanked text for OSC 52 output by the event loop.
            if !yanked_text.is_empty() {
                app.yanked_text = Some(yanked_text);
            }
            // Exit copy mode after yank.
            app.input_mode = InputMode::Normal;
            app.copy_mode = None;
            if let Some(ps) = app.panes.get_mut(&active_id) {
                ps.damage.mark_all();
            }
        }
        _ => {}
    }
    Ok(Action::Consumed)
}

/// Update the copy mode selection endpoint to follow the cursor.
fn update_copy_selection(cm: &mut CopyModeState) {
    if let Some(ref mut sel) = cm.selection {
        let abs_row = cm.scrollback_len + cm.row;
        sel.extend(SelectionPoint::new(abs_row, cm.col));
    }
}

fn mark_active_dirty(app: &mut App, active_id: PaneId) {
    if let Some(ps) = app.panes.get_mut(&active_id) {
        ps.damage.mark_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
        // 'Q' preserves the case from the binding string.
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
        // Shift+Ctrl+d should match Leader+D
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
}
