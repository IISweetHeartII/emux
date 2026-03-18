# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
