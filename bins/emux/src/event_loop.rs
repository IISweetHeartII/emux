use std::collections::HashMap;
use std::io::{self, Write, stdout};
use std::time::{Duration, Instant};

use crossterm::{
    ExecutableCommand,
    event::{self, Event, EnableBracketedPaste, DisableBracketedPaste},
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use emux_config::ConfigWatcher;
use emux_mux::{PaneId, Session};
use emux_pty::PtySize;

use crate::AppError;
use crate::app::{Action, App, ExitReason, PaneState, pty_write_all, spawn_pane_state};
use crate::input::translate_key;
use crate::keybindings::{ParsedBindings, handle_keybinding};
use crate::render::render_all;

/// Run the event loop attached to a daemon session.
pub(crate) fn run_attached(session_name: &str) -> Result<(), AppError> {
    let (cols, rows) = terminal::size()?;

    terminal::enable_raw_mode()?;
    let mut stdout = stdout();
    stdout.execute(EnterAlternateScreen)?;
    stdout.execute(EnableBracketedPaste)?;

    let result = run_event_loop(&mut stdout, cols, rows, true, session_name);

    let _ = stdout.execute(DisableBracketedPaste);
    let _ = stdout.execute(LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();

    match result {
        Ok(ExitReason::Detach) => {
            println!("[detached (from session {})]", session_name);
            Ok(())
        }
        Ok(ExitReason::Quit) => Ok(()),
        Err(e) => Err(e),
    }
}

// ---------------------------------------------------------------------------
// Event loop
// ---------------------------------------------------------------------------

fn run_event_loop<W: Write>(
    stdout: &mut W,
    init_cols: u16,
    init_rows: u16,
    daemon_mode: bool,
    _session_name: &str,
) -> Result<ExitReason, AppError> {
    emux_log!("event loop starting: {}x{}, daemon={}", init_cols, init_rows, daemon_mode);
    let mut cols = init_cols;
    let mut rows = init_rows;
    let c = cols as usize;
    // Reserve 1 row at the bottom for the status bar.
    let pane_rows = (rows as usize).saturating_sub(1);

    let config = emux_config::load_config();
    let session = Session::new("main", c, pane_rows);

    // Spawn the initial pane.
    let initial_pane_id: PaneId = 0; // Tab::new always starts pane 0
    let initial_state = spawn_pane_state(c, pane_rows)?;
    let mut panes: HashMap<PaneId, PaneState> = HashMap::new();
    panes.insert(initial_pane_id, initial_state);

    let bindings = ParsedBindings::from_config(&config.keys);
    let mut app = App {
        session,
        panes,
        config,
        bindings,
        daemon_mode,
        input_mode: crate::app::InputMode::Normal,
        search_query: String::new(),
        search_state: emux_term::search::SearchState::default(),
        search_direction_active: false,
    };

    // Force an initial full draw.
    render_all(stdout, &mut app, cols, rows, true)?;

    let mut buf = [0u8; 65536];
    let mut last_render = Instant::now();
    /// Minimum interval between renders (~60 fps).
    const FRAME_BUDGET: Duration = Duration::from_millis(16);

    // Config hot-reload: poll the config file mtime every 5 seconds.
    let mut config_watcher = ConfigWatcher::for_default_path();
    let mut last_config_check = Instant::now();
    const CONFIG_CHECK_INTERVAL: Duration = Duration::from_secs(5);

    // Session auto-save: save to the default snapshot path every 30 seconds.
    let auto_save_path = emux_daemon::persistence::default_snapshot_path(_session_name);
    let mut last_auto_save = Instant::now();
    const AUTO_SAVE_INTERVAL: Duration = Duration::from_secs(30);
    let mut session_dirty = false;

    loop {
        // ---- Read from ALL pane PTYs (non-blocking) ----
        let pane_ids: Vec<PaneId> = app.panes.keys().copied().collect();
        let mut any_output = false;
        for id in &pane_ids {
            if let Some(ps) = app.panes.get_mut(id) {
                // Read in a loop until WouldBlock to drain all available data.
                loop {
                    match ps.pty.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            ps.parser.advance(&mut ps.screen, &buf[..n]);
                            // Drain per-row damage from Screen into the
                            // pane's DamageTracker so dirty rows accumulate
                            // until the next render.
                            let regions = ps.screen.take_damage();
                            for region in &regions {
                                ps.damage.mark_row(region.row);
                            }
                            any_output = true;
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => break,
                        #[cfg(unix)]
                        Err(ref e) if e.raw_os_error() == Some(libc::EIO) => break,
                        Err(_) => break,
                    }
                }
            }
        }

        // Check for dead panes (child processes that exited).
        let alive_ids: Vec<PaneId> = app.panes.keys().copied().collect();
        let mut dead: Vec<PaneId> = Vec::new();
        for id in &alive_ids {
            if let Some(ps) = app.panes.get(id)
                && !ps.pty.is_alive() {
                    dead.push(*id);
                }
        }
        for id in dead {
            app.panes.remove(&id);
            let tab = app.session.active_tab_mut();
            if tab.pane_count() > 1 {
                tab.close_pane(id);
            } else {
                // Last pane in tab died — quit.
                return Ok(ExitReason::Quit);
            }
        }
        if app.panes.is_empty() {
            return Ok(ExitReason::Quit);
        }

        // ---- Render deadline synchronization ----
        // After PTY output, render only when the frame deadline has elapsed.
        let since_last = last_render.elapsed();
        if any_output && since_last >= FRAME_BUDGET {
            render_all(stdout, &mut app, cols, rows, false)?;
            last_render = Instant::now();
        }

        // ---- Config hot-reload check ----
        if last_config_check.elapsed() >= CONFIG_CHECK_INTERVAL {
            last_config_check = Instant::now();
            if let Some(ref mut watcher) = config_watcher
                && let Some(new_config) = watcher.check()
            {
                app.bindings = ParsedBindings::from_config(&new_config.keys);
                app.config = new_config;
                // Force a full redraw so theme changes are visible.
                for ps in app.panes.values_mut() {
                    ps.damage.mark_all();
                }
                render_all(stdout, &mut app, cols, rows, true)?;
                last_render = Instant::now();
            }
        }

        // ---- Session auto-save ----
        if session_dirty && last_auto_save.elapsed() >= AUTO_SAVE_INTERVAL {
            if let Some(ref path) = auto_save_path {
                let _ = emux_daemon::persistence::save_session(&app.session, path);
            }
            last_auto_save = Instant::now();
            session_dirty = false;
        }

        // ---- Adaptive poll timeout ----
        // If we just received PTY output there is likely more coming soon, so
        // poll with zero timeout to keep draining.  Otherwise sleep up to 16ms
        // to save CPU while idle.
        let poll_timeout = if any_output {
            Duration::ZERO
        } else {
            // Sleep at most until the next frame deadline.
            FRAME_BUDGET.saturating_sub(since_last)
        };

        // ---- Poll for keyboard / resize events ----
        if event::poll(poll_timeout)? {
            match event::read()? {
                Event::Key(key_event) => {
                    match handle_keybinding(&mut app, &key_event, stdout, cols, rows)? {
                        Action::Quit => {
                            // Save before quitting.
                            if let Some(ref path) = auto_save_path {
                                let _ = emux_daemon::persistence::save_session(&app.session, path);
                            }
                            return Ok(ExitReason::Quit);
                        }
                        Action::Detach => {
                            if let Some(ref path) = auto_save_path {
                                let _ = emux_daemon::persistence::save_session(&app.session, path);
                            }
                            return Ok(ExitReason::Detach);
                        }
                        Action::Consumed => {
                            session_dirty = true;
                        }
                        Action::Forward => {
                            // Not a keybinding — forward to active pane's PTY.
                            if let Some(active_id) = app.session.active_tab().active_pane_id()
                                && let Some(ps) = app.panes.get_mut(&active_id) {
                                    let bytes = translate_key(key_event, &ps.screen);
                                    if !bytes.is_empty()
                                        && let Err(e) = pty_write_all(&mut ps.pty, &bytes) {
                                        emux_log!("PTY write error: {}", e);
                                    }
                                }
                        }
                    }
                    // Always render immediately on input for responsiveness.
                    // Mark all panes dirty so the render picks up any visual
                    // side-effects of the key (cursor move, etc.).
                    for ps in app.panes.values_mut() {
                        ps.damage.mark_all();
                    }
                    render_all(stdout, &mut app, cols, rows, false)?;
                    last_render = Instant::now();
                }
                Event::Resize(new_cols, new_rows) => {
                    cols = new_cols;
                    rows = new_rows;
                    let nc = new_cols as usize;
                    let nr = (new_rows as usize).saturating_sub(1);

                    app.session.resize(nc, nr);
                    session_dirty = true;

                    // Resize every pane's PTY + Screen to their new layout size.
                    let positions = app.session.active_tab().compute_positions();
                    for (id, pos) in &positions {
                        if let Some(ps) = app.panes.get_mut(id) {
                            ps.screen.resize(pos.cols, pos.rows);
                            ps.damage.resize(pos.rows);
                            let _ = ps.pty.resize(PtySize {
                                rows: pos.rows as u16,
                                cols: pos.cols as u16,
                                pixel_width: 0,
                                pixel_height: 0,
                            });
                        }
                    }

                    // Force clear on resize to avoid artifacts.
                    render_all(stdout, &mut app, cols, rows, true)?;
                    last_render = Instant::now();
                }
                Event::Paste(text) => {
                    if let Some(active_id) = app.session.active_tab().active_pane_id()
                        && let Some(ps) = app.panes.get_mut(&active_id) {
                            let bytes = emux_term::input::encode_paste(
                                &text,
                                ps.screen.modes.bracketed_paste,
                            );
                            if !bytes.is_empty()
                                && let Err(e) = pty_write_all(&mut ps.pty, &bytes) {
                                emux_log!("PTY write error: {}", e);
                            }
                        }
                    for ps in app.panes.values_mut() {
                        ps.damage.mark_all();
                    }
                    render_all(stdout, &mut app, cols, rows, false)?;
                    last_render = Instant::now();
                }
                Event::FocusGained => {
                    if let Some(active_id) = app.session.active_tab().active_pane_id()
                        && let Some(ps) = app.panes.get_mut(&active_id) {
                            let bytes = emux_term::input::encode_focus(true, ps.screen.modes.focus_tracking);
                            if !bytes.is_empty()
                                && let Err(e) = pty_write_all(&mut ps.pty, &bytes) {
                                emux_log!("PTY write error: {}", e);
                            }
                        }
                }
                Event::FocusLost => {
                    if let Some(active_id) = app.session.active_tab().active_pane_id()
                        && let Some(ps) = app.panes.get_mut(&active_id) {
                            let bytes = emux_term::input::encode_focus(false, ps.screen.modes.focus_tracking);
                            if !bytes.is_empty()
                                && let Err(e) = pty_write_all(&mut ps.pty, &bytes) {
                                emux_log!("PTY write error: {}", e);
                            }
                        }
                }
                _ => {}
            }
        }
    }
}
