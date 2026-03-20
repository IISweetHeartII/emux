use std::collections::HashMap;
use std::io::{self, Read as _, Write, stdout};
use std::time::{Duration, Instant};

use crossterm::{
    ExecutableCommand,
    event::{
        self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event, MouseButton, MouseEvent, MouseEventKind,
    },
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use emux_config::ConfigWatcher;
use emux_mux::{PaneId, PanePosition, Session};
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
    stdout.execute(EnableMouseCapture)?;

    let result = run_event_loop(&mut stdout, cols, rows, true, session_name);

    // Clean up agent IPC socket.
    #[cfg(unix)]
    {
        let agent_sock =
            crate::daemon::socket_dir().join(format!("emux-agent-{session_name}.sock"));
        let _ = std::fs::remove_file(&agent_sock);
    }

    let _ = stdout.execute(DisableMouseCapture);
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
    emux_log!(
        "event loop starting: {}x{}, daemon={}",
        init_cols,
        init_rows,
        daemon_mode
    );
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
        copy_mode: None,
        yanked_text: None,
        border_drag: None,
    };

    // ---- Agent IPC socket (Unix only) ----
    // Bind BEFORE initial render so the socket is available immediately.
    #[cfg(unix)]
    let agent_socket = {
        let path = crate::daemon::socket_dir().join(format!("emux-agent-{_session_name}.sock"));
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
        match std::os::unix::net::UnixListener::bind(&path) {
            Ok(listener) => {
                let _ = listener.set_nonblocking(true);
                emux_log!("agent IPC socket bound at {:?}", path);
                Some((listener, path))
            }
            Err(e) => {
                emux_log!("failed to bind agent IPC socket: {}", e);
                None
            }
        }
    };
    #[cfg(unix)]
    let mut agent_clients: Vec<std::os::unix::net::UnixStream> = Vec::new();

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
                            // New output arrived — snap viewport back to bottom.
                            ps.screen.scroll_viewport_reset();
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
                && !ps.pty.is_alive()
            {
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
                if let Err(e) = emux_daemon::persistence::save_session(&app.session, path) {
                    emux_log!("session save error: {}", e);
                }
            }
            last_auto_save = Instant::now();
            session_dirty = false;
        }

        // ---- Agent IPC: accept connections and process commands ----
        #[cfg(unix)]
        {
            process_agent_ipc(&mut app, &agent_socket, &mut agent_clients);
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
                                if let Err(e) =
                                    emux_daemon::persistence::save_session(&app.session, path)
                                {
                                    emux_log!("session save error: {}", e);
                                }
                            }
                            return Ok(ExitReason::Quit);
                        }
                        Action::Detach => {
                            if let Some(ref path) = auto_save_path {
                                if let Err(e) =
                                    emux_daemon::persistence::save_session(&app.session, path)
                                {
                                    emux_log!("session save error: {}", e);
                                }
                            }
                            return Ok(ExitReason::Detach);
                        }
                        Action::Consumed => {
                            session_dirty = true;
                        }
                        Action::Forward => {
                            // Not a keybinding — forward to active pane's PTY.
                            if let Some(active_id) = app.session.active_tab().active_pane_id()
                                && let Some(ps) = app.panes.get_mut(&active_id)
                            {
                                // User is typing — snap viewport back to live output.
                                ps.screen.scroll_viewport_reset();
                                let bytes = translate_key(key_event, &ps.screen);
                                if !bytes.is_empty()
                                    && let Err(e) = pty_write_all(&mut ps.pty, &bytes)
                                {
                                    emux_log!("PTY write error: {}", e);
                                }
                            }
                        }
                    }
                    // Always render immediately on input for responsiveness.
                    // Only mark the active pane dirty (cursor move, etc.).
                    // Structural changes (split, tab switch) do a full mark
                    // inside their operation handlers.
                    if let Some(active_id) = app.session.active_tab().active_pane_id() {
                        if let Some(ps) = app.panes.get_mut(&active_id) {
                            ps.damage.mark_all();
                        }
                    }
                    render_all(stdout, &mut app, cols, rows, false)?;
                    last_render = Instant::now();
                    // If text was yanked in copy mode, send OSC 52 to host terminal.
                    if let Some(text) = app.yanked_text.take() {
                        let osc = emux_term::selection::osc52_clipboard(&text);
                        stdout.write_all(&osc)?;
                        stdout.flush()?;
                    }
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

                    // Clamp copy mode cursor to new pane dimensions.
                    if let Some(ref mut cm) = app.copy_mode {
                        if let Some(active_id) = app.session.active_tab().active_pane_id() {
                            if let Some(ps) = app.panes.get(&active_id) {
                                cm.row = cm.row.min(ps.screen.rows().saturating_sub(1));
                                cm.col = cm.col.min(ps.screen.cols().saturating_sub(1));
                            }
                        }
                    }

                    // Force clear on resize to avoid artifacts.
                    render_all(stdout, &mut app, cols, rows, true)?;
                    last_render = Instant::now();
                }
                Event::Paste(text) => {
                    if let Some(active_id) = app.session.active_tab().active_pane_id()
                        && let Some(ps) = app.panes.get_mut(&active_id)
                    {
                        let bytes =
                            emux_term::input::encode_paste(&text, ps.screen.modes.bracketed_paste);
                        if !bytes.is_empty()
                            && let Err(e) = pty_write_all(&mut ps.pty, &bytes)
                        {
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
                        && let Some(ps) = app.panes.get_mut(&active_id)
                    {
                        let bytes =
                            emux_term::input::encode_focus(true, ps.screen.modes.focus_tracking);
                        if !bytes.is_empty()
                            && let Err(e) = pty_write_all(&mut ps.pty, &bytes)
                        {
                            emux_log!("PTY write error: {}", e);
                        }
                    }
                }
                Event::FocusLost => {
                    if let Some(active_id) = app.session.active_tab().active_pane_id()
                        && let Some(ps) = app.panes.get_mut(&active_id)
                    {
                        let bytes =
                            emux_term::input::encode_focus(false, ps.screen.modes.focus_tracking);
                        if !bytes.is_empty()
                            && let Err(e) = pty_write_all(&mut ps.pty, &bytes)
                        {
                            emux_log!("PTY write error: {}", e);
                        }
                    }
                }
                Event::Mouse(mouse_event) => {
                    handle_mouse(&mut app, mouse_event, stdout, cols, rows)?;
                    render_all(stdout, &mut app, cols, rows, false)?;
                    last_render = Instant::now();
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Mouse handling
// ---------------------------------------------------------------------------

/// Find which pane contains the given terminal coordinates.
fn pane_at_position(positions: &[(PaneId, PanePosition)], col: u16, row: u16) -> Option<PaneId> {
    let c = col as usize;
    let r = row as usize;
    for &(id, ref pos) in positions {
        if c >= pos.col && c < pos.col + pos.cols && r >= pos.row && r < pos.row + pos.rows {
            return Some(id);
        }
    }
    None
}

/// Detect if a mouse click is on a pane border. Returns a BorderDrag if so.
fn detect_border_click(
    positions: &[(PaneId, PanePosition)],
    col: u16,
    row: u16,
) -> Option<crate::app::BorderDrag> {
    let c = col as usize;
    let r = row as usize;

    for &(id, ref pos) in positions {
        // Vertical border: the last column of this pane (right_edge - 1).
        let right_edge = pos.col + pos.cols;
        if right_edge > 0 && c == right_edge - 1 && r >= pos.row && r < pos.row + pos.rows {
            // Check that there is another pane to the right.
            let has_neighbor = positions
                .iter()
                .any(|(nid, np)| *nid != id && np.col == right_edge);
            if has_neighbor {
                return Some(crate::app::BorderDrag {
                    pane_id: id,
                    vertical: true,
                    last_col: col,
                    last_row: row,
                });
            }
        }

        // Horizontal border: the last row of this pane (bottom_edge - 1).
        let bottom_edge = pos.row + pos.rows;
        if bottom_edge > 0 && r == bottom_edge - 1 && c >= pos.col && c < pos.col + pos.cols {
            let has_neighbor = positions
                .iter()
                .any(|(nid, np)| *nid != id && np.row == bottom_edge);
            if has_neighbor {
                return Some(crate::app::BorderDrag {
                    pane_id: id,
                    vertical: false,
                    last_col: col,
                    last_row: row,
                });
            }
        }
    }
    None
}

fn handle_mouse<W: Write>(
    app: &mut App,
    event: MouseEvent,
    _stdout: &mut W,
    _cols: u16,
    _rows: u16,
) -> Result<(), AppError> {
    let positions = app.session.active_tab().compute_positions();
    let col = event.column;
    let row = event.row;

    match event.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            // Check if clicking on a pane border for resize drag.
            if let Some(drag) = detect_border_click(&positions, col, row) {
                app.border_drag = Some(drag);
            } else {
                // Click to focus pane, then forward if tracking.
                if let Some(pane_id) = pane_at_position(&positions, col, row) {
                    let current_active = app.session.active_tab().active_pane_id();
                    if current_active != Some(pane_id) {
                        app.session.active_tab_mut().focus_pane(pane_id);
                        for ps in app.panes.values_mut() {
                            ps.damage.mark_all();
                        }
                    }
                }
                forward_mouse_event_if_tracking(app, &positions, col, row, &event);
            }
        }
        MouseEventKind::ScrollUp => {
            if let Some(pane_id) = pane_at_position(&positions, col, row) {
                // Check if the child program is tracking mouse events.
                if let Some(ps) = app.panes.get_mut(&pane_id) {
                    if ps.screen.modes.mouse_tracking != emux_term::MouseMode::None {
                        // Forward scroll to PTY as mouse event.
                        let pos = positions.iter().find(|(id, _)| *id == pane_id);
                        if let Some((_, ppos)) = pos {
                            let local_col = (col as usize).saturating_sub(ppos.col) as u16;
                            let local_row = (row as usize).saturating_sub(ppos.row) as u16;
                            let encoding = if ps.screen.modes.mouse_sgr {
                                emux_term::input::MouseEncoding::Sgr
                            } else {
                                emux_term::input::MouseEncoding::Normal
                            };
                            let bytes = emux_term::input::encode_mouse(
                                emux_term::input::MouseEvent::ScrollUp {
                                    col: local_col,
                                    row: local_row,
                                },
                                encoding,
                            );
                            if let Err(e) = pty_write_all(&mut ps.pty, &bytes) {
                                emux_log!("mouse PTY write error: {}", e);
                            }
                        }
                    } else {
                        // No mouse tracking — scroll viewport up (scrollback).
                        ps.screen.scroll_viewport_up(3);
                        ps.damage.mark_all();
                    }
                }
            }
        }
        MouseEventKind::ScrollDown => {
            if let Some(pane_id) = pane_at_position(&positions, col, row) {
                if let Some(ps) = app.panes.get_mut(&pane_id) {
                    if ps.screen.modes.mouse_tracking != emux_term::MouseMode::None {
                        let pos = positions.iter().find(|(id, _)| *id == pane_id);
                        if let Some((_, ppos)) = pos {
                            let local_col = (col as usize).saturating_sub(ppos.col) as u16;
                            let local_row = (row as usize).saturating_sub(ppos.row) as u16;
                            let encoding = if ps.screen.modes.mouse_sgr {
                                emux_term::input::MouseEncoding::Sgr
                            } else {
                                emux_term::input::MouseEncoding::Normal
                            };
                            let bytes = emux_term::input::encode_mouse(
                                emux_term::input::MouseEvent::ScrollDown {
                                    col: local_col,
                                    row: local_row,
                                },
                                encoding,
                            );
                            if let Err(e) = pty_write_all(&mut ps.pty, &bytes) {
                                emux_log!("mouse PTY write error: {}", e);
                            }
                        }
                    } else {
                        ps.screen.scroll_viewport_down(3);
                        ps.damage.mark_all();
                    }
                }
            }
        }
        MouseEventKind::Down(MouseButton::Right) | MouseEventKind::Down(MouseButton::Middle) => {
            forward_mouse_event_if_tracking(app, &positions, col, row, &event);
        }
        MouseEventKind::Up(_) => {
            if app.border_drag.is_some() {
                // End border drag — sync PTY sizes to new layout.
                app.border_drag = None;
                crate::operations::sync_pty_sizes(app);
                for ps in app.panes.values_mut() {
                    ps.damage.mark_all();
                }
            } else {
                forward_mouse_event_if_tracking(app, &positions, col, row, &event);
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if let Some(drag) = app.border_drag {
                // Continue border drag — resize the pane.
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
                app.border_drag = Some(crate::app::BorderDrag {
                    last_col: col,
                    last_row: row,
                    ..drag
                });
                for ps in app.panes.values_mut() {
                    ps.damage.mark_all();
                }
            } else {
                forward_mouse_event_if_tracking(app, &positions, col, row, &event);
            }
        }
        MouseEventKind::Drag(button) => {
            forward_mouse_event_if_tracking(app, &positions, col, row, &event);
            let _ = button;
        }
        MouseEventKind::Moved => {
            // Only forward if any-event tracking is active.
            if let Some(pane_id) = pane_at_position(&positions, col, row) {
                if let Some(ps) = app.panes.get(&pane_id) {
                    if ps.screen.modes.mouse_tracking == emux_term::MouseMode::AnyEvent {
                        forward_mouse_event_if_tracking(app, &positions, col, row, &event);
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// Forward a mouse event to the PTY if the pane has mouse tracking enabled.
fn forward_mouse_event_if_tracking(
    app: &mut App,
    positions: &[(PaneId, PanePosition)],
    col: u16,
    row: u16,
    event: &MouseEvent,
) {
    let Some(pane_id) = pane_at_position(positions, col, row) else {
        return;
    };
    let Some(ps) = app.panes.get_mut(&pane_id) else {
        return;
    };
    if ps.screen.modes.mouse_tracking == emux_term::MouseMode::None {
        return;
    }
    let pos = positions.iter().find(|(id, _)| *id == pane_id);
    let Some((_, ppos)) = pos else { return };
    let local_col = (col as usize).saturating_sub(ppos.col) as u16;
    let local_row = (row as usize).saturating_sub(ppos.row) as u16;

    let encoding = if ps.screen.modes.mouse_sgr {
        emux_term::input::MouseEncoding::Sgr
    } else {
        emux_term::input::MouseEncoding::Normal
    };

    let mouse_ev = match event.kind {
        MouseEventKind::Down(button) => {
            let b = crossterm_button_to_u8(button);
            emux_term::input::MouseEvent::Press {
                button: b,
                col: local_col,
                row: local_row,
            }
        }
        MouseEventKind::Up(_) => emux_term::input::MouseEvent::Release {
            col: local_col,
            row: local_row,
        },
        MouseEventKind::Drag(button) => {
            let b = crossterm_button_to_u8(button);
            emux_term::input::MouseEvent::Drag {
                button: b,
                col: local_col,
                row: local_row,
            }
        }
        MouseEventKind::Moved => emux_term::input::MouseEvent::Drag {
            button: 3, // no button
            col: local_col,
            row: local_row,
        },
        _ => return,
    };

    let bytes = emux_term::input::encode_mouse(mouse_ev, encoding);
    if !bytes.is_empty() {
        let _ = pty_write_all(&mut ps.pty, &bytes);
    }
}

fn crossterm_button_to_u8(button: MouseButton) -> u8 {
    match button {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
    }
}

// ---------------------------------------------------------------------------
// Agent IPC — real PTY/Screen access for AI agents
// ---------------------------------------------------------------------------

/// Process agent IPC connections: accept new clients, read commands,
/// handle them with real PTY/Screen state, send responses.
#[cfg(unix)]
fn process_agent_ipc(
    app: &mut App,
    socket: &Option<(std::os::unix::net::UnixListener, std::path::PathBuf)>,
    clients: &mut Vec<std::os::unix::net::UnixStream>,
) {
    let Some((listener, _)) = socket else {
        return;
    };

    // Accept new connections (non-blocking).
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = stream.set_nonblocking(true);
                clients.push(stream);
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => break,
            Err(_) => break,
        }
    }

    // Process one message per client per frame (non-blocking).
    let mut to_remove = Vec::new();
    for (i, stream) in clients.iter_mut().enumerate() {
        // Try to read a length-prefixed message.
        let mut len_buf = [0u8; 4];
        match stream.read_exact(&mut len_buf) {
            Ok(()) => {}
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => continue,
            Err(_) => {
                to_remove.push(i);
                continue;
            }
        }
        let len = u32::from_be_bytes(len_buf) as usize;
        if len > 1_000_000 {
            to_remove.push(i);
            continue;
        }
        let mut payload = vec![0u8; len];
        // Switch to blocking briefly to read the full payload.
        let _ = stream.set_nonblocking(false);
        let read_ok = stream.read_exact(&mut payload).is_ok();
        let _ = stream.set_nonblocking(true);
        if !read_ok {
            to_remove.push(i);
            continue;
        }

        let msg: emux_ipc::ClientMessage = match emux_ipc::codec::decode(&payload) {
            Ok(m) => m,
            Err(_) => {
                to_remove.push(i);
                continue;
            }
        };

        let response = handle_agent_message(app, msg);

        // Send response.
        let _ = stream.set_nonblocking(false);
        let _ = emux_ipc::codec::write_message(stream, &response);
        let _ = stream.set_nonblocking(true);

        // Each connection handles one request-response, then we close it
        // (same as the IPC protocol spec).
        to_remove.push(i);
    }

    // Remove processed/dead clients (reverse order to preserve indices).
    to_remove.sort_unstable();
    to_remove.dedup();
    for i in to_remove.into_iter().rev() {
        clients.remove(i);
    }
}

/// Handle a single agent IPC message using the event loop's real PTY/Screen.
#[cfg(unix)]
fn handle_agent_message(app: &mut App, msg: emux_ipc::ClientMessage) -> emux_ipc::ServerMessage {
    use emux_ipc::{ClientMessage, ServerMessage};

    match msg {
        ClientMessage::Ping => ServerMessage::Pong,
        ClientMessage::GetVersion => ServerMessage::Version {
            version: emux_ipc::PROTOCOL_VERSION,
        },
        ClientMessage::ListPanes => {
            let tab = app.session.active_tab();
            let positions = tab.compute_positions();
            let active = tab.active_pane_id();
            let panes = positions
                .iter()
                .map(|(id, pos)| {
                    let pane = tab.pane(*id);
                    emux_ipc::PaneEntry {
                        id: *id,
                        title: pane.map(|p| p.title().to_owned()).unwrap_or_default(),
                        cols: pos.cols as u16,
                        rows: pos.rows as u16,
                        active: active == Some(*id),
                        has_notification: pane.map(|p| p.has_notification()).unwrap_or(false),
                    }
                })
                .collect();
            ServerMessage::PaneList { panes }
        }
        ClientMessage::GetPaneInfo { pane_id } => {
            let tab = app.session.active_tab();
            if let Some(pane) = tab.pane(pane_id) {
                let positions = tab.compute_positions();
                let (cols, rows) = positions
                    .iter()
                    .find(|(id, _)| *id == pane_id)
                    .map(|(_, p)| (p.cols as u16, p.rows as u16))
                    .unwrap_or((0, 0));
                ServerMessage::PaneInfo {
                    pane: emux_ipc::PaneEntry {
                        id: pane_id,
                        title: pane.title().to_owned(),
                        cols,
                        rows,
                        active: tab.active_pane_id() == Some(pane_id),
                        has_notification: pane.has_notification(),
                    },
                }
            } else {
                ServerMessage::Error {
                    message: format!("pane {pane_id} not found"),
                }
            }
        }
        ClientMessage::CapturePane { pane_id } => {
            if let Some(ps) = app.panes.get(&pane_id) {
                let content = (0..ps.screen.rows())
                    .map(|r| ps.screen.row_text(r))
                    .collect::<Vec<_>>()
                    .join("\n");
                ServerMessage::PaneCaptured { pane_id, content }
            } else {
                ServerMessage::Error {
                    message: format!("pane {pane_id} not found"),
                }
            }
        }
        ClientMessage::SendKeys { pane_id, keys } => {
            if let Some(ps) = app.panes.get_mut(&pane_id) {
                if let Err(e) = pty_write_all(&mut ps.pty, keys.as_bytes()) {
                    ServerMessage::Error {
                        message: format!("PTY write error: {e}"),
                    }
                } else {
                    ServerMessage::Ack
                }
            } else {
                ServerMessage::Error {
                    message: format!("pane {pane_id} not found"),
                }
            }
        }
        ClientMessage::SplitPane { direction, .. } => {
            let dir = match direction {
                emux_ipc::SplitDirection::Horizontal => emux_mux::SplitDirection::Horizontal,
                emux_ipc::SplitDirection::Vertical => emux_mux::SplitDirection::Vertical,
            };
            match app.session.active_tab_mut().split_pane(dir) {
                Some(new_id) => {
                    let positions = app.session.active_tab().compute_positions();
                    let (pcols, prows) = positions
                        .iter()
                        .find(|(id, _)| *id == new_id)
                        .map(|(_, p)| (p.cols, p.rows))
                        .unwrap_or((80, 24));
                    match spawn_pane_state(pcols, prows) {
                        Ok(ps) => {
                            app.panes.insert(new_id, ps);
                            crate::operations::sync_pty_sizes(app);
                            ServerMessage::SpawnResult { pane_id: new_id }
                        }
                        Err(e) => ServerMessage::Error {
                            message: format!("spawn error: {e}"),
                        },
                    }
                }
                None => ServerMessage::Error {
                    message: "cannot split pane".into(),
                },
            }
        }
        ClientMessage::SetPaneTitle { pane_id, title } => {
            if let Some(pane) = app.session.active_tab_mut().pane_mut(pane_id) {
                pane.set_title(title);
                ServerMessage::Ack
            } else {
                ServerMessage::Error {
                    message: format!("pane {pane_id} not found"),
                }
            }
        }
        ClientMessage::ResizePane {
            pane_id,
            cols,
            rows,
        } => {
            let tab = app.session.active_tab_mut();
            if tab.pane(pane_id).is_none() {
                return ServerMessage::Error {
                    message: format!("pane {pane_id} not found"),
                };
            }
            let positions = tab.compute_positions();
            if let Some((_, pos)) = positions.iter().find(|(id, _)| *id == pane_id) {
                let dc = cols as i32 - pos.cols as i32;
                let dr = rows as i32 - pos.rows as i32;
                if dc != 0 {
                    tab.resize_pane(pane_id, emux_mux::ResizeDirection::Right, dc);
                }
                if dr != 0 {
                    tab.resize_pane(pane_id, emux_mux::ResizeDirection::Down, dr);
                }
                crate::operations::sync_pty_sizes(app);
                ServerMessage::Ack
            } else {
                ServerMessage::Error {
                    message: format!("pane {pane_id} not found"),
                }
            }
        }
        _ => ServerMessage::Ack,
    }
}
