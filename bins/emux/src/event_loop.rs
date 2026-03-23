use std::collections::HashMap;
use std::io::{self, Read as _, Write, stdout};
use std::time::{Duration, Instant};

use crossterm::{
    ExecutableCommand,
    event::{
        self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event,
    },
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use emux_config::ConfigWatcher;
use emux_mux::{PaneId, Session};
use emux_pty::PtySize;

use crate::AppError;
use crate::app::{App, ExitReason, PaneState, pty_write_all, spawn_pane_state};
use crate::command::{self, Command, DispatchResult};
use crate::input::translate_key;
use crate::keybindings::{ParsedBindings, ResolveContext, resolve_keybinding};
use crate::mouse::{MouseContext, PaneMouseInfo, resolve_mouse};
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
    let pane_rows = (rows as usize).saturating_sub(1);

    let config = emux_config::load_config();
    let session = Session::new("main", c, pane_rows);

    let initial_pane_id: PaneId = 0;
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
        border_drag: None,
        mouse_selection: None,
    };

    // ---- Agent IPC socket (Unix only) ----
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
    const FRAME_BUDGET: Duration = Duration::from_millis(16);

    let mut config_watcher = ConfigWatcher::for_default_path();
    let mut last_config_check = Instant::now();
    const CONFIG_CHECK_INTERVAL: Duration = Duration::from_secs(5);

    let auto_save_path = emux_daemon::persistence::default_snapshot_path(_session_name);
    let mut last_auto_save = Instant::now();
    const AUTO_SAVE_INTERVAL: Duration = Duration::from_secs(30);
    let mut session_dirty = false;

    loop {
        // ---- 1. Drain PTY reads ----
        let any_output = drain_pty_reads(&mut app, &mut buf);

        // ---- 2. Handle dead panes ----
        if handle_dead_panes(&mut app) {
            return Ok(ExitReason::Quit);
        }

        // ---- 3. Render if frame budget elapsed ----
        let since_last = last_render.elapsed();
        if any_output && since_last >= FRAME_BUDGET {
            render_all(stdout, &mut app, cols, rows, false)?;
            last_render = Instant::now();
        }

        // ---- 4. Config hot-reload ----
        if last_config_check.elapsed() >= CONFIG_CHECK_INTERVAL {
            last_config_check = Instant::now();
            if let Some(ref mut watcher) = config_watcher
                && let Some(new_config) = watcher.check()
            {
                let result =
                    command::dispatch(&mut app, Command::ReloadConfig(Box::new(new_config)));
                execute_side_effects(stdout, &mut app, &result)?;
                render_all(stdout, &mut app, cols, rows, true)?;
                last_render = Instant::now();
            }
        }

        // ---- 5. Session auto-save ----
        if session_dirty && last_auto_save.elapsed() >= AUTO_SAVE_INTERVAL {
            if let Some(ref path) = auto_save_path {
                if let Err(e) = emux_daemon::persistence::save_session(&app.session, path) {
                    emux_log!("session save error: {}", e);
                }
            }
            last_auto_save = Instant::now();
            session_dirty = false;
        }

        // ---- 6. Agent IPC ----
        #[cfg(unix)]
        {
            process_agent_ipc(&mut app, &agent_socket, &mut agent_clients);
        }

        // ---- 7. Adaptive poll timeout ----
        let poll_timeout = if any_output {
            Duration::ZERO
        } else {
            FRAME_BUDGET.saturating_sub(since_last)
        };

        // ---- 8. Poll and dispatch events ----
        if event::poll(poll_timeout)? {
            let cmds = match event::read()? {
                Event::Key(key_event) => {
                    let forward_bytes =
                        if let Some(active_id) = app.session.active_tab().active_pane_id() {
                            app.panes
                                .get(&active_id)
                                .map(|ps| translate_key(key_event, &ps.screen))
                                .unwrap_or_default()
                        } else {
                            Vec::new()
                        };

                    let ctx = ResolveContext {
                        input_mode: app.input_mode,
                        bindings: &app.bindings,
                        daemon_mode: app.daemon_mode,
                        search_query: &app.search_query,
                        search_direction_active: app.search_direction_active,
                        forward_bytes,
                    };

                    let cmd = resolve_keybinding(&key_event, &ctx);

                    // Compute half-page for scroll commands.
                    let cmd = match cmd {
                        Command::ScrollUp(0) | Command::ScrollDown(0) => {
                            let half = app
                                .session
                                .active_tab()
                                .active_pane_id()
                                .and_then(|id| app.panes.get(&id))
                                .map(|ps| ps.screen.rows() / 2)
                                .unwrap_or(1)
                                .max(1);
                            match cmd {
                                Command::ScrollUp(_) => Command::ScrollUp(half),
                                _ => Command::ScrollDown(half),
                            }
                        }
                        other => other,
                    };

                    vec![cmd]
                }
                Event::Mouse(mouse_event) => {
                    let positions = app.session.active_tab().compute_positions();
                    let pane_info: Vec<PaneMouseInfo> = positions
                        .iter()
                        .filter_map(|(id, _)| {
                            app.panes.get(id).map(|ps| PaneMouseInfo {
                                pane_id: *id,
                                mouse_tracking: ps.screen.modes.mouse_tracking,
                                mouse_sgr: ps.screen.modes.mouse_sgr,
                                scrollback_len: ps.screen.grid.scrollback_len(),
                                viewport_offset: ps.screen.viewport_offset(),
                            })
                        })
                        .collect();

                    let ctx = MouseContext {
                        positions: &positions,
                        active_pane_id: app.session.active_tab().active_pane_id(),
                        border_drag: app.border_drag.as_ref(),
                        has_mouse_selection: app.mouse_selection.is_some(),
                        mouse_selection_pane: app.mouse_selection.as_ref().map(|ms| ms.pane_id),
                        pane_mouse_info: &pane_info,
                    };

                    resolve_mouse(&mouse_event, &ctx)
                }
                Event::Resize(new_cols, new_rows) => {
                    cols = new_cols;
                    rows = new_rows;
                    vec![Command::Resize {
                        cols: new_cols,
                        rows: new_rows,
                    }]
                }
                Event::Paste(text) => {
                    let data = if let Some(active_id) = app.session.active_tab().active_pane_id() {
                        app.panes
                            .get(&active_id)
                            .map(|ps| {
                                emux_term::input::encode_paste(
                                    &text,
                                    ps.screen.modes.bracketed_paste,
                                )
                            })
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };
                    vec![Command::ForwardPaste { data }]
                }
                Event::FocusGained => vec![Command::ForwardFocus(true)],
                Event::FocusLost => vec![Command::ForwardFocus(false)],
            };

            for cmd in cmds {
                let result = command::dispatch(&mut app, cmd);

                // Save session before exiting.
                if result.exit.is_some() {
                    if let Some(ref path) = auto_save_path {
                        if let Err(e) = emux_daemon::persistence::save_session(&app.session, path) {
                            emux_log!("session save error: {}", e);
                        }
                    }
                }

                execute_side_effects(stdout, &mut app, &result)?;

                if let Some(exit) = result.exit {
                    return Ok(exit);
                }

                session_dirty = true;
            }

            // Mark active pane dirty and render after any input.
            if let Some(active_id) = app.session.active_tab().active_pane_id() {
                if let Some(ps) = app.panes.get_mut(&active_id) {
                    ps.damage.mark_all();
                }
            }
            render_all(stdout, &mut app, cols, rows, false)?;
            last_render = Instant::now();
        }
    }
}

// ---------------------------------------------------------------------------
// Extracted helpers
// ---------------------------------------------------------------------------

/// Drain all available data from all pane PTYs. Returns true if any output was read.
fn drain_pty_reads(app: &mut App, buf: &mut [u8]) -> bool {
    let pane_ids: Vec<PaneId> = app.panes.keys().copied().collect();
    let mut any_output = false;
    for id in &pane_ids {
        if let Some(ps) = app.panes.get_mut(id) {
            loop {
                match ps.pty.read(buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        ps.parser.advance(&mut ps.screen, &buf[..n]);
                        ps.screen.scroll_viewport_reset();
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
    any_output
}

/// Check for dead panes and remove them. Returns true if all panes are dead (should quit).
fn handle_dead_panes(app: &mut App) -> bool {
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
            return true;
        }
    }
    app.panes.is_empty()
}

/// Execute the side-effects described in a DispatchResult.
fn execute_side_effects<W: Write>(
    stdout: &mut W,
    app: &mut App,
    result: &DispatchResult,
) -> Result<(), AppError> {
    // Spawn new pane if requested.
    if let Some((pane_id, pcols, prows)) = result.spawn_pane {
        let ps = spawn_pane_state(pcols, prows)?;
        app.panes.insert(pane_id, ps);
        crate::operations::sync_pty_sizes(app);
    }

    // PTY resize sync on EndBorderDrag (signaled via force_clear + EndBorderDrag).
    if result.force_clear {
        crate::operations::sync_pty_sizes(app);
    }

    // PTY writes.
    for (pane_id, bytes) in &result.pty_writes {
        if let Some(ps) = app.panes.get_mut(pane_id) {
            if let Err(e) = pty_write_all(&mut ps.pty, bytes) {
                emux_log!("PTY write error: {}", e);
            }
        }
    }

    // Resize PTYs after Resize command.
    if result.force_clear {
        let positions = app.session.active_tab().compute_positions();
        for (id, pos) in &positions {
            if let Some(ps) = app.panes.get_mut(id) {
                let _ = ps.pty.resize(PtySize {
                    rows: pos.rows as u16,
                    cols: pos.cols as u16,
                    pixel_width: 0,
                    pixel_height: 0,
                });
            }
        }
    }

    // Send OSC 52 clipboard for yanked text.
    if let Some(ref text) = result.yanked_text {
        let osc = emux_term::selection::osc52_clipboard(text);
        stdout.write_all(&osc)?;
        stdout.flush()?;
    }

    // Send OSC 52 clipboard for mouse selection.
    if let Some(ref text) = result.clipboard_text {
        let osc = emux_term::selection::osc52_clipboard(text);
        stdout.write_all(&osc)?;
        stdout.flush()?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Agent IPC — real PTY/Screen access for AI agents
// ---------------------------------------------------------------------------

#[cfg(unix)]
fn process_agent_ipc(
    app: &mut App,
    socket: &Option<(std::os::unix::net::UnixListener, std::path::PathBuf)>,
    clients: &mut Vec<std::os::unix::net::UnixStream>,
) {
    let Some((listener, _)) = socket else {
        return;
    };

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

    let mut to_remove = Vec::new();
    for (i, stream) in clients.iter_mut().enumerate() {
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
        // Use a short read timeout instead of fully blocking to prevent deadlock.
        let _ = stream.set_nonblocking(false);
        let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
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

        let _ = stream.set_nonblocking(false);
        let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
        if let Err(e) = emux_ipc::codec::write_message(stream, &response) {
            emux_log!("agent IPC write error: {}", e);
        }
        let _ = stream.set_nonblocking(true);

        to_remove.push(i);
    }

    to_remove.sort_unstable();
    to_remove.dedup();
    for i in to_remove.into_iter().rev() {
        clients.remove(i);
    }
}

// ---------------------------------------------------------------------------
// PaneAccess impl for standalone mode (used by agent IPC)
// ---------------------------------------------------------------------------

/// PaneAccess backed by App's pane HashMap — holds only the panes,
/// not the session, to allow split borrows.
struct StandalonePaneAccess<'a> {
    panes: &'a mut HashMap<PaneId, PaneState>,
}

impl crate::ipc_handler::PaneAccess for StandalonePaneAccess<'_> {
    fn capture_pane(&mut self, pane_id: PaneId) -> Option<String> {
        let ps = self.panes.get(&pane_id)?;
        let mut content = String::new();
        for r in 0..ps.screen.rows() {
            if r > 0 {
                content.push('\n');
            }
            content.push_str(&ps.screen.row_text(r));
        }
        Some(content)
    }

    fn send_keys(&mut self, pane_id: PaneId, data: &[u8]) -> Result<(), String> {
        if let Some(ps) = self.panes.get_mut(&pane_id) {
            pty_write_all(&mut ps.pty, data).map_err(|e| format!("write error: {e}"))
        } else {
            Err(format!("pane {pane_id} not found"))
        }
    }

    fn remove_pane(&mut self, pane_id: PaneId) {
        self.panes.remove(&pane_id);
    }

    fn spawn_pane(&mut self, pane_id: PaneId, cols: usize, rows: usize) -> Result<(), String> {
        match spawn_pane_state(cols, rows) {
            Ok(ps) => {
                self.panes.insert(pane_id, ps);
                Ok(())
            }
            Err(e) => Err(format!("spawn error: {e}")),
        }
    }
}

/// Handle an agent IPC message using the shared handler.
#[cfg(unix)]
fn handle_agent_message(app: &mut App, msg: emux_ipc::ClientMessage) -> emux_ipc::ServerMessage {
    // Split borrow: session and panes are disjoint fields of App.
    let mut access = StandalonePaneAccess {
        panes: &mut app.panes,
    };
    let result = crate::ipc_handler::handle_ipc(&mut app.session, &mut access, msg);
    if result.sync_sizes {
        crate::operations::sync_pty_sizes(app);
    }
    result.response
}
