use emux_config::Config;
use emux_mux::tab::FocusDirection;
use emux_mux::{PaneId, SplitDirection};
use emux_pty::Pty;
use emux_term::search;
use emux_term::selection::{Selection, SelectionMode, SelectionPoint};

use crate::app::{App, BorderDrag, CopyModeState, ExitReason, InputMode, MouseSelection};
use crate::keybindings::ParsedBindings;

// ---------------------------------------------------------------------------
// Command — every state transition expressed as data
// ---------------------------------------------------------------------------

pub(crate) enum Command {
    // ── Session structure ───────────────────────────────────────────
    SplitPane(SplitDirection),
    CloseActivePane,
    NewTab,
    CloseTab,
    NextTab,
    PrevTab,
    FocusDirection(FocusDirection),
    ToggleFullscreen,
    ToggleFloat,

    // ── Viewport ────────────────────────────────────────────────────
    ScrollUp(usize),
    ScrollDown(usize),

    // ── Mode transitions ────────────────────────────────────────────
    EnterSearchMode,
    ExitSearchMode,
    EnterCopyMode,
    ExitCopyMode,

    // ── Search ──────────────────────────────────────────────────────
    SearchAppendChar(char),
    SearchDeleteChar,
    SearchNextMatch,
    SearchPrevMatch,

    // ── Copy mode ───────────────────────────────────────────────────
    CopyMoveLeft,
    CopyMoveDown,
    CopyMoveUp,
    CopyMoveRight,
    CopyStartOfLine,
    CopyEndOfLine,
    CopyGotoTop,
    CopyGotoBottom,
    CopyHalfPageUp,
    CopyHalfPageDown,
    CopyToggleSelection,
    CopyYank,

    // ── I/O signals ─────────────────────────────────────────────────
    ForwardKeyToPty(Vec<u8>),
    ForwardPaste {
        data: Vec<u8>,
    },
    ForwardFocus(bool),
    Quit,
    Detach,

    // ── Mouse ───────────────────────────────────────────────────────
    FocusPane(PaneId),
    StartBorderDrag(BorderDrag),
    ContinueBorderDrag {
        col: u16,
        row: u16,
    },
    EndBorderDrag,
    StartMouseSelection {
        pane_id: PaneId,
        point: SelectionPoint,
    },
    ExtendMouseSelection(SelectionPoint),
    FinalizeMouseSelection,
    ForwardMouseToPty {
        pane_id: PaneId,
        bytes: Vec<u8>,
    },
    ScrollPaneUp {
        pane_id: PaneId,
        amount: usize,
    },
    ScrollPaneDown {
        pane_id: PaneId,
        amount: usize,
    },

    // ── System ──────────────────────────────────────────────────────
    Resize {
        cols: u16,
        rows: u16,
    },
    ReloadConfig(Box<Config>),
}

// ---------------------------------------------------------------------------
// DispatchResult — describes side-effects the event loop must perform
// ---------------------------------------------------------------------------

#[allow(dead_code)] // Fields read by tests and future optimizations
pub(crate) struct DispatchResult {
    /// Render needed after this command.
    pub needs_render: bool,
    /// Force a full clear before rendering (e.g. resize).
    pub force_clear: bool,
    /// If set, the event loop should exit with this reason.
    pub exit: Option<ExitReason>,
    /// Deferred PTY writes: (pane_id, bytes).
    pub pty_writes: Vec<(PaneId, Vec<u8>)>,
    /// A new pane needs to be spawned: (pane_id, cols, rows).
    pub spawn_pane: Option<(PaneId, usize, usize)>,
    /// Text yanked in copy mode — send as OSC 52.
    pub yanked_text: Option<String>,
    /// Text from finalized mouse selection — send as OSC 52.
    pub clipboard_text: Option<String>,
}

impl DispatchResult {
    fn none() -> Self {
        Self {
            needs_render: false,
            force_clear: false,
            exit: None,
            pty_writes: Vec::new(),
            spawn_pane: None,
            yanked_text: None,
            clipboard_text: None,
        }
    }

    fn render() -> Self {
        Self {
            needs_render: true,
            ..Self::none()
        }
    }

    fn render_clear() -> Self {
        Self {
            needs_render: true,
            force_clear: true,
            ..Self::none()
        }
    }
}

// ---------------------------------------------------------------------------
// dispatch — pure state transition, no I/O
// ---------------------------------------------------------------------------

pub(crate) fn dispatch<P: Pty>(app: &mut App<P>, cmd: Command) -> DispatchResult {
    match cmd {
        // ── Session structure ───────────────────────────────────────
        Command::SplitPane(direction) => dispatch_split_pane(app, direction),
        Command::CloseActivePane => dispatch_close_active_pane(app),
        Command::NewTab => dispatch_new_tab(app),
        Command::CloseTab => {
            let idx = app.session.active_tab_index();
            let pane_ids = app.session.active_tab().pane_ids();
            if app.session.close_tab(idx) {
                for id in pane_ids {
                    app.panes.remove(&id);
                }
            }
            mark_all_dirty(app);
            DispatchResult::render()
        }
        Command::NextTab => {
            app.session.next_tab();
            mark_all_dirty(app);
            DispatchResult::render()
        }
        Command::PrevTab => {
            app.session.prev_tab();
            mark_all_dirty(app);
            DispatchResult::render()
        }
        Command::FocusDirection(dir) => {
            app.session.active_tab_mut().focus_direction(dir);
            mark_all_dirty(app);
            DispatchResult::render()
        }
        Command::ToggleFullscreen => {
            app.session.active_tab_mut().toggle_fullscreen();
            mark_all_dirty(app);
            DispatchResult::render()
        }
        Command::ToggleFloat => {
            app.session.active_tab_mut().toggle_floating_panes();
            mark_all_dirty(app);
            DispatchResult::render()
        }

        // ── Viewport ────────────────────────────────────────────────
        Command::ScrollUp(amount) => {
            if let Some(active_id) = app.session.active_tab().active_pane_id() {
                if let Some(ps) = app.panes.get_mut(&active_id) {
                    ps.screen.scroll_viewport_up(amount.max(1));
                    ps.damage.mark_all();
                }
            }
            DispatchResult::render()
        }
        Command::ScrollDown(amount) => {
            if let Some(active_id) = app.session.active_tab().active_pane_id() {
                if let Some(ps) = app.panes.get_mut(&active_id) {
                    ps.screen.scroll_viewport_down(amount.max(1));
                    ps.damage.mark_all();
                }
            }
            DispatchResult::render()
        }

        // ── Mode transitions ────────────────────────────────────────
        Command::EnterSearchMode => {
            app.input_mode = InputMode::Search;
            app.search_query.clear();
            app.search_state = search::SearchState::default();
            app.search_direction_active = false;
            DispatchResult::render()
        }
        Command::ExitSearchMode => {
            app.input_mode = InputMode::Normal;
            DispatchResult::render()
        }
        Command::EnterCopyMode => {
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
            DispatchResult::render()
        }
        Command::ExitCopyMode => {
            app.input_mode = InputMode::Normal;
            app.copy_mode = None;
            mark_all_dirty(app);
            DispatchResult::render()
        }

        // ── Search ──────────────────────────────────────────────────
        Command::SearchAppendChar(_)
        | Command::SearchDeleteChar
        | Command::SearchNextMatch
        | Command::SearchPrevMatch => dispatch_search(app, cmd),

        // ── Copy mode ───────────────────────────────────────────────
        Command::CopyMoveLeft
        | Command::CopyMoveDown
        | Command::CopyMoveUp
        | Command::CopyMoveRight
        | Command::CopyStartOfLine
        | Command::CopyEndOfLine
        | Command::CopyGotoTop
        | Command::CopyGotoBottom
        | Command::CopyHalfPageUp
        | Command::CopyHalfPageDown
        | Command::CopyToggleSelection
        | Command::CopyYank => dispatch_copy_mode(app, cmd),

        // ── I/O signals ─────────────────────────────────────────────
        Command::ForwardKeyToPty(bytes) => {
            let mut result = DispatchResult::render();
            if let Some(active_id) = app.session.active_tab().active_pane_id() {
                if let Some(ps) = app.panes.get_mut(&active_id) {
                    ps.screen.scroll_viewport_reset();
                }
                if !bytes.is_empty() {
                    result.pty_writes.push((active_id, bytes));
                }
            }
            result
        }
        Command::ForwardPaste { data } => {
            let mut result = DispatchResult::render();
            if let Some(active_id) = app.session.active_tab().active_pane_id() {
                if !data.is_empty() {
                    result.pty_writes.push((active_id, data));
                }
            }
            mark_all_dirty(app);
            result
        }
        Command::ForwardFocus(gained) => {
            let mut result = DispatchResult::none();
            if let Some(active_id) = app.session.active_tab().active_pane_id() {
                if let Some(ps) = app.panes.get(&active_id) {
                    let bytes =
                        emux_term::input::encode_focus(gained, ps.screen.modes.focus_tracking);
                    if !bytes.is_empty() {
                        result.pty_writes.push((active_id, bytes));
                    }
                }
            }
            result
        }
        Command::Quit => DispatchResult {
            exit: Some(ExitReason::Quit),
            ..DispatchResult::none()
        },
        Command::Detach => DispatchResult {
            exit: Some(ExitReason::Detach),
            ..DispatchResult::none()
        },

        // ── Mouse ───────────────────────────────────────────────────
        Command::FocusPane(_)
        | Command::StartBorderDrag(_)
        | Command::ContinueBorderDrag { .. }
        | Command::EndBorderDrag
        | Command::StartMouseSelection { .. }
        | Command::ExtendMouseSelection(_)
        | Command::FinalizeMouseSelection
        | Command::ForwardMouseToPty { .. }
        | Command::ScrollPaneUp { .. }
        | Command::ScrollPaneDown { .. } => dispatch_mouse(app, cmd),

        // ── System ──────────────────────────────────────────────────
        Command::Resize { cols, rows } => {
            let nc = cols as usize;
            let nr = (rows as usize).saturating_sub(1);
            app.session.resize(nc, nr);

            // Resize each pane's Screen + damage tracker.
            let positions = app.session.active_tab().compute_positions();
            for (id, pos) in &positions {
                if let Some(ps) = app.panes.get_mut(id) {
                    ps.screen.resize(pos.cols, pos.rows);
                    ps.damage.resize(pos.rows);
                }
            }

            // Clamp copy mode cursor.
            if let Some(ref mut cm) = app.copy_mode {
                if let Some(active_id) = app.session.active_tab().active_pane_id() {
                    if let Some(ps) = app.panes.get(&active_id) {
                        cm.row = cm.row.min(ps.screen.rows().saturating_sub(1));
                        cm.col = cm.col.min(ps.screen.cols().saturating_sub(1));
                    }
                }
            }

            DispatchResult::render_clear()
        }
        Command::ReloadConfig(new_config) => {
            app.bindings = ParsedBindings::from_config(&new_config.keys);
            app.config = *new_config;
            mark_all_dirty(app);
            DispatchResult::render_clear()
        }
    }
}

// ---------------------------------------------------------------------------
// Sub-dispatchers
// ---------------------------------------------------------------------------

fn dispatch_search<P: Pty>(app: &mut App<P>, cmd: Command) -> DispatchResult {
    match cmd {
        Command::SearchAppendChar(c) => {
            app.search_query.push(c);
            run_search(app);
            app.search_direction_active = true;
            DispatchResult::render()
        }
        Command::SearchDeleteChar => {
            app.search_query.pop();
            run_search(app);
            DispatchResult::render()
        }
        Command::SearchNextMatch => {
            app.search_state.current =
                search::next_match_index(app.search_state.current, app.search_state.matches.len());
            DispatchResult::render()
        }
        Command::SearchPrevMatch => {
            app.search_state.current =
                search::prev_match_index(app.search_state.current, app.search_state.matches.len());
            DispatchResult::render()
        }
        _ => unreachable!(),
    }
}

fn dispatch_copy_mode<P: Pty>(app: &mut App<P>, cmd: Command) -> DispatchResult {
    match cmd {
        Command::CopyMoveLeft => copy_move(app, |cm, _| cm.col = cm.col.saturating_sub(1)),
        Command::CopyMoveDown => copy_move(app, |cm, ps| {
            cm.row = (cm.row + 1).min(ps.screen.rows().saturating_sub(1));
        }),
        Command::CopyMoveUp => copy_move(app, |cm, _| cm.row = cm.row.saturating_sub(1)),
        Command::CopyMoveRight => copy_move(app, |cm, ps| {
            cm.col = (cm.col + 1).min(ps.screen.cols().saturating_sub(1));
        }),
        Command::CopyStartOfLine => copy_move(app, |cm, _| cm.col = 0),
        Command::CopyEndOfLine => copy_move(app, |cm, ps| {
            cm.col = ps.screen.cols().saturating_sub(1);
        }),
        Command::CopyGotoTop => copy_move(app, |cm, _| cm.row = 0),
        Command::CopyGotoBottom => copy_move(app, |cm, ps| {
            cm.row = ps.screen.rows().saturating_sub(1);
        }),
        Command::CopyHalfPageUp => copy_move(app, |cm, ps| {
            let half = ps.screen.rows() / 2;
            cm.row = cm.row.saturating_sub(half);
        }),
        Command::CopyHalfPageDown => copy_move(app, |cm, ps| {
            let half = ps.screen.rows() / 2;
            cm.row = (cm.row + half).min(ps.screen.rows().saturating_sub(1));
        }),
        Command::CopyToggleSelection => {
            if let Some(ref mut cm) = app.copy_mode {
                if cm.selection.is_some() {
                    cm.selection = None;
                } else {
                    let point = SelectionPoint::new(cm.scrollback_len + cm.row, cm.col);
                    cm.selection = Some(Selection::start(point, SelectionMode::Normal));
                }
            }
            mark_active_dirty(app);
            DispatchResult::render()
        }
        Command::CopyYank => {
            let mut result = DispatchResult::render();
            if let Some(ref mut cm) = app.copy_mode {
                if let Some(ref mut sel) = cm.selection {
                    sel.finalize();
                    if let Some(active_id) = app.session.active_tab().active_pane_id() {
                        if let Some(ps) = app.panes.get(&active_id) {
                            let text = sel.get_text(&ps.screen.grid);
                            if !text.is_empty() {
                                result.yanked_text = Some(text);
                            }
                        }
                    }
                }
            }
            app.input_mode = InputMode::Normal;
            app.copy_mode = None;
            mark_all_dirty(app);
            result
        }
        _ => unreachable!(),
    }
}

fn dispatch_mouse<P: Pty>(app: &mut App<P>, cmd: Command) -> DispatchResult {
    match cmd {
        Command::FocusPane(pane_id) => {
            let current_active = app.session.active_tab().active_pane_id();
            if current_active != Some(pane_id) {
                app.session.active_tab_mut().focus_pane(pane_id);
                mark_all_dirty(app);
            }
            DispatchResult::render()
        }
        Command::StartBorderDrag(drag) => {
            app.border_drag = Some(drag);
            DispatchResult::none()
        }
        Command::ContinueBorderDrag { col, row } => {
            if let Some(drag) = app.border_drag {
                let tab = app.session.active_tab_mut();
                if drag.vertical {
                    let delta = col as i32 - drag.last_col as i32;
                    if delta != 0 {
                        tab.resize_pane(drag.pane_id, emux_mux::ResizeDirection::Right, delta);
                    }
                } else {
                    let delta = row as i32 - drag.last_row as i32;
                    if delta != 0 {
                        tab.resize_pane(drag.pane_id, emux_mux::ResizeDirection::Down, delta);
                    }
                }
                app.border_drag = Some(BorderDrag {
                    last_col: col,
                    last_row: row,
                    ..drag
                });
                mark_all_dirty(app);
            }
            DispatchResult::render()
        }
        Command::EndBorderDrag => {
            app.border_drag = None;
            let mut result = DispatchResult::render();
            result.force_clear = true;
            result
        }
        Command::StartMouseSelection { pane_id, point } => {
            app.mouse_selection = Some(MouseSelection {
                pane_id,
                selection: Selection::start(point, SelectionMode::Normal),
            });
            DispatchResult::render()
        }
        Command::ExtendMouseSelection(point) => {
            if let Some(ref mut ms) = app.mouse_selection {
                ms.selection.extend(point);
            }
            mark_all_dirty(app);
            DispatchResult::render()
        }
        Command::FinalizeMouseSelection => {
            let mut result = DispatchResult::render();
            if let Some(ref mut ms) = app.mouse_selection {
                ms.selection.finalize();
                if let Some(ps) = app.panes.get(&ms.pane_id) {
                    let text = ms.selection.get_text(&ps.screen.grid);
                    if !text.is_empty() {
                        result.clipboard_text = Some(text);
                    }
                }
            }
            app.mouse_selection = None;
            mark_all_dirty(app);
            result
        }
        Command::ForwardMouseToPty { pane_id, bytes } => {
            let mut result = DispatchResult::none();
            if !bytes.is_empty() {
                result.pty_writes.push((pane_id, bytes));
            }
            result
        }
        Command::ScrollPaneUp { pane_id, amount } => {
            if let Some(ps) = app.panes.get_mut(&pane_id) {
                ps.screen.scroll_viewport_up(amount);
                ps.damage.mark_all();
            }
            DispatchResult::render()
        }
        Command::ScrollPaneDown { pane_id, amount } => {
            if let Some(ps) = app.panes.get_mut(&pane_id) {
                ps.screen.scroll_viewport_down(amount);
                ps.damage.mark_all();
            }
            DispatchResult::render()
        }
        _ => unreachable!(),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn mark_all_dirty<P: Pty>(app: &mut App<P>) {
    for ps in app.panes.values_mut() {
        ps.damage.mark_all();
    }
}

fn mark_active_dirty<P: Pty>(app: &mut App<P>) {
    if let Some(active_id) = app.session.active_tab().active_pane_id() {
        if let Some(ps) = app.panes.get_mut(&active_id) {
            ps.damage.mark_all();
        }
    }
}

/// Copy mode cursor movement helper. Applies a closure to modify the cursor,
/// then updates the selection if active.
fn copy_move<P: Pty, F>(app: &mut App<P>, f: F) -> DispatchResult
where
    F: FnOnce(&mut CopyModeState, &crate::app::PaneState<P>),
{
    if let Some(active_id) = app.session.active_tab().active_pane_id() {
        if let Some(ref mut cm) = app.copy_mode {
            if let Some(ps) = app.panes.get(&active_id) {
                f(cm, ps);
                // Update selection endpoint.
                if let Some(ref mut sel) = cm.selection {
                    let abs_row = cm.scrollback_len + cm.row;
                    sel.extend(SelectionPoint::new(abs_row, cm.col));
                }
            }
        }
        if let Some(ps) = app.panes.get_mut(&active_id) {
            ps.damage.mark_all();
        }
    }
    DispatchResult::render()
}

/// Execute search over the active pane's viewport.
fn run_search<P: Pty>(app: &mut App<P>) {
    if app.search_query.is_empty() {
        app.search_state = search::SearchState::default();
        return;
    }

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

/// Dispatch SplitPane: modifies session layout, returns spawn request.
fn dispatch_split_pane<P: Pty>(app: &mut App<P>, direction: SplitDirection) -> DispatchResult {
    let tab = app.session.active_tab_mut();
    let mut result = DispatchResult::render();
    if let Some(new_id) = tab.split_pane(direction) {
        let positions = tab.compute_positions();
        let (pcols, prows) = positions
            .iter()
            .find(|(id, _)| *id == new_id)
            .map(|(_, p)| (p.cols, p.rows))
            .unwrap_or((80, 24));
        result.spawn_pane = Some((new_id, pcols, prows));
    }
    mark_all_dirty(app);
    result
}

/// Dispatch CloseActivePane: removes pane, or signals quit if last pane.
fn dispatch_close_active_pane<P: Pty>(app: &mut App<P>) -> DispatchResult {
    let tab = app.session.active_tab_mut();
    if let Some(active_id) = tab.active_pane_id() {
        if tab.pane_count() > 1 {
            tab.close_pane(active_id);
            app.panes.remove(&active_id);
            mark_all_dirty(app);
            // Signal that PTY sizes need syncing (force_clear).
            return DispatchResult::render_clear();
        }
    }
    DispatchResult::render()
}

/// Dispatch NewTab: creates tab in session, returns spawn request.
fn dispatch_new_tab<P: Pty>(app: &mut App<P>) -> DispatchResult {
    let size = app.session.size();
    let _tab_id = app
        .session
        .new_tab(format!("Tab {}", app.session.tab_count()));
    let new_pane_id = app.session.active_tab().pane_ids()[0];
    let mut result = DispatchResult::render();
    result.spawn_pane = Some((new_pane_id, size.cols, size.rows));
    mark_all_dirty(app);
    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::InputMode;
    use crate::app::testing::{MockPty, test_app};
    use emux_mux::SplitDirection;
    use emux_mux::tab::FocusDirection;

    #[test]
    fn dispatch_enter_search_mode() {
        let mut app = test_app(80, 24);
        let result = dispatch(&mut app, Command::EnterSearchMode);
        assert_eq!(app.input_mode, InputMode::Search);
        assert!(result.needs_render);
    }

    #[test]
    fn dispatch_exit_search_mode() {
        let mut app = test_app(80, 24);
        app.input_mode = InputMode::Search;
        let result = dispatch(&mut app, Command::ExitSearchMode);
        assert_eq!(app.input_mode, InputMode::Normal);
        assert!(result.needs_render);
    }

    #[test]
    fn dispatch_search_append_char() {
        let mut app = test_app(80, 24);
        app.input_mode = InputMode::Search;
        dispatch(&mut app, Command::SearchAppendChar('a'));
        assert_eq!(app.search_query, "a");
        assert!(app.search_direction_active);
        dispatch(&mut app, Command::SearchAppendChar('b'));
        assert_eq!(app.search_query, "ab");
    }

    #[test]
    fn dispatch_search_delete_char() {
        let mut app = test_app(80, 24);
        app.search_query = "abc".to_string();
        dispatch(&mut app, Command::SearchDeleteChar);
        assert_eq!(app.search_query, "ab");
    }

    #[test]
    fn dispatch_enter_copy_mode() {
        let mut app = test_app(80, 24);
        let result = dispatch(&mut app, Command::EnterCopyMode);
        assert_eq!(app.input_mode, InputMode::Copy);
        assert!(app.copy_mode.is_some());
        assert!(result.needs_render);
    }

    #[test]
    fn dispatch_exit_copy_mode() {
        let mut app = test_app(80, 24);
        dispatch(&mut app, Command::EnterCopyMode);
        dispatch(&mut app, Command::ExitCopyMode);
        assert_eq!(app.input_mode, InputMode::Normal);
        assert!(app.copy_mode.is_none());
    }

    #[test]
    fn dispatch_copy_move_down() {
        let mut app = test_app(80, 24);
        dispatch(&mut app, Command::EnterCopyMode);
        dispatch(&mut app, Command::CopyMoveDown);
        let cm = app.copy_mode.as_ref().unwrap();
        assert_eq!(cm.row, 1);
    }

    #[test]
    fn dispatch_copy_move_right() {
        let mut app = test_app(80, 24);
        dispatch(&mut app, Command::EnterCopyMode);
        dispatch(&mut app, Command::CopyMoveRight);
        let cm = app.copy_mode.as_ref().unwrap();
        assert_eq!(cm.col, 1);
    }

    #[test]
    fn dispatch_copy_goto_top_bottom() {
        let mut app = test_app(80, 24);
        dispatch(&mut app, Command::EnterCopyMode);
        dispatch(&mut app, Command::CopyGotoBottom);
        assert_eq!(app.copy_mode.as_ref().unwrap().row, 23);
        dispatch(&mut app, Command::CopyGotoTop);
        assert_eq!(app.copy_mode.as_ref().unwrap().row, 0);
    }

    #[test]
    fn dispatch_copy_start_end_of_line() {
        let mut app = test_app(80, 24);
        dispatch(&mut app, Command::EnterCopyMode);
        dispatch(&mut app, Command::CopyEndOfLine);
        assert_eq!(app.copy_mode.as_ref().unwrap().col, 79);
        dispatch(&mut app, Command::CopyStartOfLine);
        assert_eq!(app.copy_mode.as_ref().unwrap().col, 0);
    }

    #[test]
    fn dispatch_copy_toggle_selection() {
        let mut app = test_app(80, 24);
        dispatch(&mut app, Command::EnterCopyMode);
        assert!(app.copy_mode.as_ref().unwrap().selection.is_none());
        dispatch(&mut app, Command::CopyToggleSelection);
        assert!(app.copy_mode.as_ref().unwrap().selection.is_some());
        dispatch(&mut app, Command::CopyToggleSelection);
        assert!(app.copy_mode.as_ref().unwrap().selection.is_none());
    }

    #[test]
    fn dispatch_split_pane_returns_spawn() {
        let mut app = test_app(80, 24);
        let result = dispatch(&mut app, Command::SplitPane(SplitDirection::Horizontal));
        assert!(result.spawn_pane.is_some());
        assert!(result.needs_render);
    }

    #[test]
    fn dispatch_new_tab_returns_spawn() {
        let mut app = test_app(80, 24);
        let initial_tabs = app.session.tab_count();
        let result = dispatch(&mut app, Command::NewTab);
        assert!(result.spawn_pane.is_some());
        assert_eq!(app.session.tab_count(), initial_tabs + 1);
    }

    #[test]
    fn dispatch_next_prev_tab() {
        let mut app = test_app(80, 24);
        // Create a second tab.
        let result = dispatch(&mut app, Command::NewTab);
        if let Some((pane_id, cols, rows)) = result.spawn_pane {
            let mut screen = emux_term::Screen::new(cols, rows);
            screen.set_damage_mode(emux_term::DamageMode::Row);
            app.panes.insert(
                pane_id,
                crate::app::PaneState {
                    pty: MockPty::new(),
                    parser: emux_vt::Parser::new(),
                    screen,
                    damage: emux_render::damage::DamageTracker::new(rows),
                },
            );
        }

        let idx0 = app.session.active_tab_index();
        dispatch(&mut app, Command::PrevTab);
        let idx1 = app.session.active_tab_index();
        assert_ne!(idx0, idx1);
    }

    #[test]
    fn dispatch_scroll_up_down() {
        let mut app = test_app(80, 24);
        let result = dispatch(&mut app, Command::ScrollUp(5));
        assert!(result.needs_render);
        let result = dispatch(&mut app, Command::ScrollDown(5));
        assert!(result.needs_render);
    }

    #[test]
    fn dispatch_forward_key_produces_pty_write() {
        let mut app = test_app(80, 24);
        let result = dispatch(&mut app, Command::ForwardKeyToPty(vec![b'x']));
        assert_eq!(result.pty_writes.len(), 1);
        assert_eq!(result.pty_writes[0].1, vec![b'x']);
    }

    #[test]
    fn dispatch_quit() {
        let mut app = test_app(80, 24);
        let result = dispatch(&mut app, Command::Quit);
        assert!(matches!(result.exit, Some(ExitReason::Quit)));
    }

    #[test]
    fn dispatch_detach() {
        let mut app = test_app(80, 24);
        let result = dispatch(&mut app, Command::Detach);
        assert!(matches!(result.exit, Some(ExitReason::Detach)));
    }

    #[test]
    fn dispatch_resize() {
        let mut app = test_app(80, 24);
        let result = dispatch(
            &mut app,
            Command::Resize {
                cols: 120,
                rows: 40,
            },
        );
        assert!(result.force_clear);
        assert!(result.needs_render);
    }

    #[test]
    fn dispatch_focus_direction() {
        let mut app = test_app(80, 24);
        let result = dispatch(&mut app, Command::FocusDirection(FocusDirection::Up));
        assert!(result.needs_render);
    }

    #[test]
    fn dispatch_close_active_pane_single_pane() {
        let mut app = test_app(80, 24);
        let result = dispatch(&mut app, Command::CloseActivePane);
        assert_eq!(app.panes.len(), 1);
        assert!(result.needs_render);
    }

    #[test]
    fn dispatch_forward_paste() {
        let mut app = test_app(80, 24);
        let result = dispatch(
            &mut app,
            Command::ForwardPaste {
                data: b"hello".to_vec(),
            },
        );
        assert_eq!(result.pty_writes.len(), 1);
        assert_eq!(result.pty_writes[0].1, b"hello");
    }

    #[test]
    fn dispatch_reload_config() {
        let mut app = test_app(80, 24);
        let config = emux_config::Config::default();
        let result = dispatch(&mut app, Command::ReloadConfig(Box::new(config)));
        assert!(result.force_clear);
        assert!(result.needs_render);
    }
}
