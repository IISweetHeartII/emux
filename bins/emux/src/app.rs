use std::collections::HashMap;
use std::io;
use std::time::Duration;

use emux_config::Config;
use emux_mux::{PaneId, Session};
#[cfg(unix)]
use emux_pty::{CommandBuilder, PtySize, UnixPty as NativePty};
#[cfg(windows)]
use emux_pty::{CommandBuilder, PtySize, WinPty as NativePty};
use emux_render::damage::DamageTracker;
use emux_term::search::SearchState;
use emux_term::{DamageMode, Screen};
use emux_vt::Parser;

use crate::AppError;
use crate::keybindings::ParsedBindings;

// ---------------------------------------------------------------------------
// Keybinding action result
// ---------------------------------------------------------------------------

pub(crate) enum Action {
    /// Key was consumed by a binding (no further processing needed).
    Consumed,
    /// Key was NOT consumed — forward to PTY.
    Forward,
    /// Quit emux (standalone mode).
    Quit,
    /// Detach from daemon (session stays alive).
    Detach,
}

/// The current input mode — normal typing or a modal overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputMode {
    Normal,
    Search,
}

// ---------------------------------------------------------------------------
// Per-pane terminal state
// ---------------------------------------------------------------------------

pub(crate) struct PaneState {
    pub(crate) pty: NativePty,
    pub(crate) parser: Parser,
    pub(crate) screen: Screen,
    pub(crate) damage: DamageTracker,
}

// ---------------------------------------------------------------------------
// Application
// ---------------------------------------------------------------------------

pub(crate) struct App {
    pub(crate) session: Session,
    pub(crate) panes: HashMap<PaneId, PaneState>,
    #[allow(dead_code)]
    pub(crate) config: Config,
    pub(crate) bindings: ParsedBindings,
    /// Whether the app is running as a daemon client (true) or standalone (false).
    pub(crate) daemon_mode: bool,
    /// Current input mode (normal vs search).
    pub(crate) input_mode: InputMode,
    /// The current search query string being typed by the user.
    pub(crate) search_query: String,
    /// Search state with matches and current index.
    pub(crate) search_state: SearchState,
    /// Whether `n`/`N` navigation is active (set after first character typed).
    pub(crate) search_direction_active: bool,
}

// ---------------------------------------------------------------------------
// Exit reason
// ---------------------------------------------------------------------------

pub(crate) enum ExitReason {
    Quit,
    Detach,
}

/// Set a file descriptor to non-blocking mode (Unix only).
///
/// On Windows this is not needed: the PTY implementation uses its own
/// threaded I/O model and does not expose a raw file descriptor.
#[cfg(unix)]
pub(crate) fn set_nonblocking(fd: i32) {
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }
}

/// Write all bytes to a non-blocking PTY, retrying on WouldBlock with
/// exponential backoff (10μs → 20 → 40 … capped at 1000μs).  Resets to
/// the minimum delay after every successful write so bulk transfers stay fast.
pub(crate) fn pty_write_all(pty: &mut NativePty, mut data: &[u8]) -> io::Result<()> {
    let mut backoff_us = 10u64;
    while !data.is_empty() {
        match pty.write(data) {
            Ok(0) => return Err(io::Error::new(io::ErrorKind::WriteZero, "write zero")),
            Ok(n) => {
                data = &data[n..];
                backoff_us = 10;
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_micros(backoff_us));
                backoff_us = (backoff_us * 2).min(1000);
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// Spawn a new PTY for a pane of the given size and return a `PaneState`.
pub(crate) fn spawn_pane_state(cols: usize, rows: usize) -> Result<PaneState, AppError> {
    let size = PtySize {
        rows: rows as u16,
        cols: cols as u16,
        pixel_width: 0,
        pixel_height: 0,
    };
    let mut cmd = CommandBuilder::default_shell();
    let pid = std::process::id().to_string();
    cmd.env("EMUX", &pid);
    let pty = NativePty::spawn(&cmd, size)?;
    #[cfg(unix)]
    set_nonblocking(pty.master_raw_fd());
    let mut screen = Screen::new(cols, rows);
    // Use Row-level damage mode so we can efficiently skip clean rows.
    screen.set_damage_mode(DamageMode::Row);
    let damage = DamageTracker::new(rows);
    Ok(PaneState {
        pty,
        parser: Parser::new(),
        screen,
        damage,
    })
}
