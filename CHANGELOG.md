# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- OSC 52 transparent clipboard passthrough (copy/paste works everywhere)
- AI agent IPC protocol (SplitPane, CapturePane, SendKeys, ListPanes, etc.)
- Project-aware workspaces with git root auto-detection and branch display
- Powerline-style status bar with session, tabs, notifications, time, hostname
- OSC notification system (OSC 9/99/777) for AI agent completion alerts
- Pane border theming (active/inactive colors)
- Theme configuration: accent, border_active, border_inactive, statusbar_bg, powerline
- Synchronized panes (type in all panes at once with toggle)
- Shell integration via OSC 133 semantic zones (per-command navigation, exit code tracking)
- Smart selection and hint mode (auto-detect URLs, file paths, git SHAs, IPs, emails)
- Cross-pane search (search all panes simultaneously)
- Layout templates (.emux.toml project-specific pane layouts with startup commands)
- Session recording and replay (asciicast v2 format)

### Changed
- VT parser optimized: 242 MB/s → 598 MB/s (2.4x improvement)
- Vec allocations replaced with fixed-size arrays in parser hot path
- Dependencies updated: toml 1.0, crossterm 0.29, nix 0.31, criterion 0.8

### Fixed
- Windows: daemon uses TCP loopback instead of Unix sockets (full Windows compilation support)
- Windows: status bar time display (was showing "??:??")
- Windows: file path hint detection now matches `C:\...` and UNC paths
- Windows: nix/libc dependencies made conditional (Unix-only)
- Status bar: emoji/CJK display width calculation fixed (was counting chars, not columns)

## [0.1.0] - 2026-03-18

Initial release of emux.

### Added

- **emux-vt** -- Complete VT escape sequence parser supporting CSI, OSC, DCS, ESC, and UTF-8 sequences. State-machine architecture with fuzz-tested robustness.
- **emux-term** -- Terminal state engine with grid management, cursor tracking, scrollback buffer, content reflow on resize, SGR attribute handling, and input encoding.
- **emux-pty** -- Cross-platform PTY integration: Unix via `forkpty` (macOS, Linux, WSL) and Windows via ConPTY.
- **emux-mux** -- Multiplexer with sessions, tabs, panes, horizontal/vertical splits, floating panes, swap layouts, and fullscreen toggle.
- **emux-config** -- TOML configuration system with deep merge (override only what you need), One Dark theme, and remappable keybindings.
- **emux-daemon** -- Session daemon with Unix socket server, client connection management, session persistence across terminal disconnects.
- **emux-ipc** -- Client-daemon IPC protocol using length-prefixed JSON codec. Commands: Ping, GetVersion, Resize, Detach, ListSessions, KillSession, SpawnPane, KillPane, FocusPane, KeyInput.
- **emux-render** -- TUI renderer using crossterm with damage-tracked cell updates, pane border drawing, and status bar.
- **CLI** -- `emux`, `emux new [name]`, `emux attach [name]`, `emux ls`, `emux kill <name>`.
- **Testing** -- 1,105 automated tests, 45 golden snapshot tests (derived from Alacritty ref test suite), stress tests with 1 MB random data and malformed input, 3,993 fuzz corpus files across 2 fuzz targets.
- **Benchmarks** -- VT parser throughput benchmarks via criterion.
