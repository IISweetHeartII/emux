use std::collections::HashMap;
#[allow(unused_imports)]
use std::io::{self, Write as _};
use std::time::Duration;

use emux_config::Config;
use emux_mux::{PaneId, Session};
use emux_pty::Pty;
#[cfg(unix)]
use emux_pty::{CommandBuilder, PtySize, UnixPty as NativePty};
#[cfg(windows)]
use emux_pty::{CommandBuilder, PtySize, WinPty as NativePty};
use emux_render::damage::DamageTracker;
use emux_term::search::SearchState;
use emux_term::selection::Selection;
use emux_term::{DamageMode, Screen};
use emux_vt::Parser;

use crate::AppError;
use crate::keybindings::ParsedBindings;

/// The current input mode — normal typing or a modal overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputMode {
    Normal,
    Search,
    /// Vi-style copy mode with cursor navigation and text selection.
    Copy,
}

/// State for copy mode (vi-style navigation + selection).
#[derive(Debug, Clone)]
pub(crate) struct CopyModeState {
    /// Cursor row in the viewport.
    pub row: usize,
    /// Cursor column.
    pub col: usize,
    /// Scrollback length when copy mode was entered (for absolute row computation).
    pub scrollback_len: usize,
    /// Active selection (None until 'v' is pressed).
    pub selection: Option<Selection>,
}

// ---------------------------------------------------------------------------
// Per-pane terminal state
// ---------------------------------------------------------------------------

pub(crate) struct PaneState<P: Pty = NativePty> {
    pub(crate) pty: P,
    pub(crate) parser: Parser,
    pub(crate) screen: Screen,
    pub(crate) damage: DamageTracker,
}

// ---------------------------------------------------------------------------
// Application
// ---------------------------------------------------------------------------

pub(crate) struct App<P: Pty = NativePty> {
    pub(crate) session: Session,
    pub(crate) panes: HashMap<PaneId, PaneState<P>>,
    #[allow(dead_code)]
    pub(crate) config: Config,
    pub(crate) bindings: ParsedBindings,
    /// Whether the app is running as a daemon client (true) or standalone (false).
    pub(crate) daemon_mode: bool,
    /// Current input mode (normal vs search vs copy).
    pub(crate) input_mode: InputMode,
    /// The current search query string being typed by the user.
    pub(crate) search_query: String,
    /// Search state with matches and current index.
    pub(crate) search_state: SearchState,
    /// Whether `n`/`N` navigation is active (set after first character typed).
    pub(crate) search_direction_active: bool,
    /// Copy mode state (cursor position, selection).
    pub(crate) copy_mode: Option<CopyModeState>,
    /// Active border drag state for mouse resize.
    pub(crate) border_drag: Option<BorderDrag>,
    /// Active mouse text selection state.
    pub(crate) mouse_selection: Option<MouseSelection>,
}

/// State tracking a mouse drag text selection.
#[derive(Debug, Clone)]
pub(crate) struct MouseSelection {
    /// The pane being selected in.
    pub pane_id: PaneId,
    /// The selection object tracking start/end points.
    pub selection: Selection,
}

/// State tracking a mouse drag on a pane border for resizing.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BorderDrag {
    /// The pane whose right or bottom edge is being dragged.
    pub pane_id: PaneId,
    /// Whether this is a vertical border (drag left/right) or horizontal (drag up/down).
    pub vertical: bool,
    /// Last mouse column during drag.
    pub last_col: u16,
    /// Last mouse row during drag.
    pub last_row: u16,
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
pub(crate) fn pty_write_all<W: io::Write>(pty: &mut W, mut data: &[u8]) -> io::Result<()> {
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

/// Spawn a new PTY for a pane of the given size and return a `PaneState<NativePty>`.
pub(crate) fn spawn_pane_state(cols: usize, rows: usize) -> Result<PaneState<NativePty>, AppError> {
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

// ---------------------------------------------------------------------------
// Test support: MockPty and test helpers
// ---------------------------------------------------------------------------

#[cfg(test)]
pub(crate) mod testing {
    use super::*;
    use std::collections::VecDeque;
    use std::io::{self, Read, Write};

    /// A mock PTY for unit tests. Implements Read, Write, and Pty.
    #[allow(dead_code)]
    pub(crate) struct MockPty {
        /// Data available to be read (simulates PTY output).
        pub input: VecDeque<u8>,
        /// Data written to the PTY (captures what would go to the child process).
        pub output: Vec<u8>,
        /// Whether the simulated child process is alive.
        pub alive: bool,
        /// Current PTY size.
        pub size: PtySize,
    }

    impl MockPty {
        pub fn new() -> Self {
            Self {
                input: VecDeque::new(),
                output: Vec::new(),
                alive: true,
                size: PtySize {
                    rows: 24,
                    cols: 80,
                    pixel_width: 0,
                    pixel_height: 0,
                },
            }
        }
    }

    impl Read for MockPty {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.input.is_empty() {
                return Err(io::Error::new(io::ErrorKind::WouldBlock, "no data"));
            }
            let n = buf.len().min(self.input.len());
            for b in buf.iter_mut().take(n) {
                *b = self.input.pop_front().unwrap();
            }
            Ok(n)
        }
    }

    impl Write for MockPty {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.output.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Pty for MockPty {
        fn resize(&self, _size: PtySize) -> Result<(), emux_pty::PtyError> {
            Ok(())
        }
        fn child_pid(&self) -> u32 {
            12345
        }
        fn is_alive(&self) -> bool {
            self.alive
        }
    }

    /// Create a minimal `App<MockPty>` for unit testing.
    pub(crate) fn test_app(cols: usize, rows: usize) -> App<MockPty> {
        let session = Session::new("test", cols, rows);
        let initial_pane_id: PaneId = 0;
        let mut panes: HashMap<PaneId, PaneState<MockPty>> = HashMap::new();
        let mock_pty = MockPty::new();
        let mut screen = Screen::new(cols, rows);
        screen.set_damage_mode(DamageMode::Row);
        panes.insert(
            initial_pane_id,
            PaneState {
                pty: mock_pty,
                parser: Parser::new(),
                screen,
                damage: DamageTracker::new(rows),
            },
        );
        let config = emux_config::Config::default();
        let bindings = ParsedBindings::from_config(&config.keys);
        App {
            session,
            panes,
            config,
            bindings,
            daemon_mode: false,
            input_mode: InputMode::Normal,
            search_query: String::new(),
            search_state: SearchState::default(),
            search_direction_active: false,
            copy_mode: None,
            border_drag: None,
            mouse_selection: None,
        }
    }

    #[test]
    fn mock_pty_read_write() {
        let mut pty = MockPty::new();
        pty.input.extend(b"hello");
        let mut buf = [0u8; 10];
        let n = pty.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"hello");

        pty.write_all(b"world").unwrap();
        assert_eq!(&pty.output, b"world");
    }

    #[test]
    fn mock_pty_is_alive() {
        let pty = MockPty::new();
        assert!(pty.is_alive());
    }

    #[test]
    fn test_app_creates_valid_state() {
        let app = test_app(80, 24);
        assert_eq!(app.panes.len(), 1);
        assert!(app.panes.contains_key(&0));
        assert_eq!(app.input_mode, InputMode::Normal);
        assert_eq!(app.session.name(), "test");
    }
}
