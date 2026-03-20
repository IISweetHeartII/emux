<div align="center">

# emux

**A modern terminal multiplexer built in Rust -- zero config, session persistence, and 1,379 tests.**

[![CI](https://github.com/IISweetHeartII/emux/actions/workflows/ci.yml/badge.svg)](https://github.com/IISweetHeartII/emux/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/IISweetHeartII/emux/graph/badge.svg)](https://codecov.io/gh/IISweetHeartII/emux)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Crates.io](https://img.shields.io/crates/v/emux.svg)](https://crates.io/crates/emux)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-orange.svg)](Cargo.toml)

<!-- ![emux demo](doc/demo.gif) -->

[Install](#installation) · [Quick Start](#quick-start) · [Keybindings](#keybindings) · [Config](#configuration) · [Docs](#documentation) · [Contributing](CONTRIBUTING.md)

</div>

---

## Why emux?

Terminal multiplexers haven't changed much in decades. tmux requires cryptic configs and a prefix key from the 1980s. Zellij improved the UX but added weight. Neither was built with modern testing practices.

**emux** takes a different approach:

- **Zero config.** Sensible defaults, One Dark theme, intuitive keybindings. Works perfectly out of the box.
- **Thoroughly tested.** 1,379 tests, 45 golden snapshot tests, 3,993 fuzz corpus files. The VT parser has been fuzz-tested to handle any byte sequence without panicking.
- **Cross-platform.** macOS, Linux, WSL, and Windows (ConPTY) from a single codebase.
- **Session persistence.** Daemon mode keeps sessions alive after disconnect. Detach, go home, reattach.
- **Scriptable.** IPC socket API with length-prefixed JSON -- perfect for automation and AI agent integration.
- **AI-native.** Built-in Claude Code agent protocol, OSC notification support, and IPC API for AI tool orchestration.
- **Clipboard that works.** Transparent OSC 52 clipboard passthrough -- copy/paste works with mouse, keyboard, and across SSH.

---

## Installation

### Quick install (recommended)

**macOS / Linux / WSL:**

```sh
curl -fsSL https://raw.githubusercontent.com/IISweetHeartII/emux/main/install.sh | sh
```

**Windows (PowerShell):**

```powershell
irm https://raw.githubusercontent.com/IISweetHeartII/emux/main/install.ps1 | iex
```

### Cargo install

```sh
cargo install emux
```

### Homebrew (macOS / Linux)

```sh
brew tap IISweetHeartII/tap
brew install emux
```

### Prebuilt binaries

Download from [GitHub Releases](https://github.com/IISweetHeartII/emux/releases/latest). Binaries are available for:

| Platform        | Architecture |
|-----------------|-------------|
| Linux (glibc)   | x86_64      |
| Linux (musl)    | x86_64      |
| Linux (glibc)   | aarch64     |
| macOS           | x86_64      |
| macOS           | aarch64     |
| Windows         | x86_64      |
| Windows         | aarch64     |

### From source

```sh
git clone https://github.com/IISweetHeartII/emux.git
cd emux
cargo build --release
# Binary: target/release/emux
```

### Requirements

- A terminal with 256-color support
- Rust 1.85+ (only if building from source)

---

## Quick Start

```sh
# Start emux (creates a session or attaches to an existing one)
emux

# Start a named session
emux new work

# List active sessions
emux ls

# Attach to a session
emux attach work

# Kill a session
emux kill work
```

Once inside emux, the **leader key** is `Ctrl+Shift`. All keybindings start with the leader followed by a key.

```
Leader + D       Split pane down
Leader + R       Split pane right
Leader + X       Close pane
Leader + T       New tab
Leader + N       Next tab
Leader + Q       Detach (session stays alive)
```

That's it. You're multiplexing.

---

## Keybindings

The leader key is **Ctrl+Shift**. All bindings are remappable in your [config file](#configuration).

### Panes

| Action             | Keybinding         |
|--------------------|---------------------|
| Split down         | `Leader + D`        |
| Split right        | `Leader + R`        |
| Close pane         | `Leader + X`        |
| Focus up           | `Leader + Up`       |
| Focus down         | `Leader + Down`     |
| Focus left         | `Leader + Left`     |
| Focus right        | `Leader + Right`    |
| Toggle fullscreen  | `Leader + F`        |
| Toggle floating    | `Leader + G`        |

### Tabs

| Action             | Keybinding         |
|--------------------|---------------------|
| New tab            | `Leader + T`        |
| Close tab          | `Leader + W`        |
| Next tab           | `Leader + N`        |
| Previous tab       | `Leader + P`        |

### Session

| Action             | Keybinding         |
|--------------------|---------------------|
| Detach             | `Leader + Q`        |
| Scrollback search  | `Leader + /`        |
| Copy mode          | `Leader + [`        |

---

## Features

### Splits, Tabs, and Floating Panes

Horizontal and vertical splits with a full tiling layout engine. Tabs for workspace organization. Floating panes that overlay the tiled layout.

```sh
Leader + D    # Split the current pane horizontally (top/bottom)
Leader + R    # Split vertically (side by side)
Leader + G    # Toggle floating pane layer
Leader + F    # Fullscreen the active pane
```

### Session Persistence

emux runs a lightweight daemon that keeps sessions alive after you disconnect. Close your terminal, SSH back in, and pick up where you left off.

```sh
emux new dev           # Start a named session with daemon
Leader + Q             # Detach (session keeps running)
emux ls                # List active sessions
emux attach dev        # Reattach
emux kill dev          # Terminate the session
```

### Scrollback Search

Search through scrollback history with text or regex patterns.

```sh
Leader + /    # Enter search mode
```

### Copy Mode

Enter copy mode to select and copy text. Supports OSC 52 for clipboard integration across SSH sessions.

```sh
Leader + [    # Enter copy mode
```

### IPC Protocol

Script emux from external tools via a Unix socket with length-prefixed JSON messages. Spawn panes, kill panes, resize, and query session state programmatically.

```sh
# The daemon listens on /tmp/emux-sockets/emux-<name>.sock
# Protocol: 4-byte big-endian length prefix + JSON payload
```

Available IPC commands: `Ping`, `GetVersion`, `Resize`, `Detach`, `ListSessions`, `KillSession`, `SpawnPane`, `KillPane`, `FocusPane`, `KeyInput`.

### AI Agent Integration

The IPC protocol is designed for AI tool orchestration. AI agents (such as Claude Code) can programmatically split panes, send keystrokes, capture pane contents, and list running panes -- enabling fully automated terminal workflows. Supported agent commands include `SplitPane`, `CapturePane`, `SendKeys`, and `ListPanes`. OSC 9/99/777 notifications alert agents when long-running tasks complete.

### Project-Aware Workspaces

emux automatically detects your project's git root and sets the working directory accordingly. The status bar displays the current branch name, so you always know which repository context you're in.

### Status Bar

A Powerline-style status bar shows the session name, open tabs, OSC notifications, current time, and hostname. Fully themeable via `config.toml` with `accent`, `border_active`, `border_inactive`, `statusbar_bg`, and `powerline` color options.

### Cross-Platform

- **macOS** -- native PTY via `forkpty`
- **Linux** -- native PTY via `forkpty`
- **WSL** -- works seamlessly under Windows Subsystem for Linux
- **Windows** -- ConPTY support for native Windows terminals

### Damage-Tracked Rendering

Only changed cells are redrawn each frame. Combined with release-mode optimizations (`opt-level = "s"`, LTO, symbol stripping), emux stays responsive even with many panes open.

---

## Configuration

emux looks for a config file at `~/.config/emux/config.toml`. If it doesn't exist, sensible defaults are used. You only need to override what you want to change.

### Example config

```toml
# ~/.config/emux/config.toml

scrollback_limit = 10000
cursor_shape = "block"
cursor_blink = true
bold_is_bright = false
font_size = 14.0

[theme]
background = "#282C34"
foreground = "#ABB2BF"
cursor = "#528BFF"
selection_bg = "#3E4451"
colors = [
    "#1D1F21", "#CC6666", "#B5BD68", "#F0C674",
    "#81A2BE", "#B294BB", "#8ABEB7", "#C5C8C6",
    "#666666", "#D54E53", "#B9CA4A", "#E7C547",
    "#7AA6DA", "#C397D8", "#70C0B1", "#EAEAEA",
]

[keys]
split_down = "Leader+D"
split_right = "Leader+R"
close_pane = "Leader+X"
focus_up = "Leader+Up"
focus_down = "Leader+Down"
focus_left = "Leader+Left"
focus_right = "Leader+Right"
new_tab = "Leader+T"
close_tab = "Leader+W"
next_tab = "Leader+N"
prev_tab = "Leader+P"
detach = "Leader+Q"
search = "Leader+/"
toggle_fullscreen = "Leader+F"
toggle_float = "Leader+G"
copy_mode = "Leader+["
```

### Key binding syntax

Bindings are strings of modifiers joined by `+`. The **Leader** modifier maps to `Ctrl+Shift`.

| Modifier  | Aliases                    |
|-----------|----------------------------|
| Leader    | Ctrl+Shift                 |
| Ctrl      | Control                    |
| Shift     | --                         |
| Alt       | Meta, Opt, Option          |

Key names: single characters (`D`, `/`, `[`), arrow keys (`Up`, `Down`, `Left`, `Right`), and special keys (`Tab`, `Enter`, `Esc`, `Backspace`, `Delete`, `Home`, `End`, `PageUp`, `PageDown`, `F1`-`F12`).

---

## Architecture

emux is a Cargo workspace with 8 focused crates, each with a single responsibility:

```
crates/
  emux-vt        VT escape sequence parser (CSI, OSC, DCS, ESC, UTF-8)
  emux-term      Terminal state engine (grid, cursor, scrollback, reflow, SGR)
  emux-pty       PTY integration (Unix forkpty + Windows ConPTY)
  emux-mux       Multiplexer (sessions, tabs, panes, layouts, floating panes)
  emux-config    Configuration system (TOML, themes, keybindings)
  emux-daemon    Session daemon (server, client, persistence)
  emux-ipc       IPC protocol (length-prefixed JSON codec)
  emux-render    TUI renderer (crossterm, damage tracking, status bar)
bins/
  emux           CLI binary -- ties everything together
```

**Dependency flow:**

```
emux-vt -> emux-term -> emux-pty -> emux-mux -> emux-render
                                        |
                    emux-config  emux-ipc  emux-daemon
```

Each crate can be compiled and tested in isolation, making it straightforward to contribute to a specific layer without understanding the full stack.

---

## Testing

emux ships with **1,379 tests**, **3,993 fuzz corpus files**, and **45 golden snapshot tests**.

```sh
# Run all tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p emux-vt
cargo test -p emux-term
cargo test -p emux-mux

# Run golden snapshot tests (uses insta)
cargo test -p emux-term -- golden

# Run benchmarks
cargo bench -p emux-vt

# Run fuzz tests (requires nightly + cargo-fuzz)
cargo +nightly fuzz run fuzz_parser
cargo +nightly fuzz run fuzz_terminal
```

### Test categories

| Type             | Location                          | What it covers                                    |
|------------------|-----------------------------------|---------------------------------------------------|
| Unit tests       | `src/**/*.rs` (`#[cfg(test)]`)    | Core logic for each crate                         |
| Integration      | `tests/*.rs`                      | Cross-module behavior (reflow, input encoding)    |
| Golden/snapshot  | `crates/emux-term/tests/golden/`  | 45 tests replaying recorded VT byte streams       |
| Stress tests     | `crates/emux-vt/tests/stress.rs`  | 1 MB random data, malformed UTF-8, extreme params |
| Fuzz targets     | `fuzz/`                           | libFuzzer targets for parser and terminal         |
| Benchmarks       | `benches/`                        | VT parse throughput                               |

---

## Comparison

| Feature              | emux       | tmux       | Zellij     | screen     |
|----------------------|:----------:|:----------:|:----------:|:----------:|
| Language             | Rust       | C          | Rust       | C          |
| Zero config          | Yes        | No         | Partial    | No         |
| Splits and tabs      | Yes        | Yes        | Yes        | Limited    |
| Session persistence  | Yes        | Yes        | Yes        | Yes        |
| Floating panes       | Yes        | No         | Yes        | No         |
| Swap layouts         | Yes        | No         | Yes        | No         |
| Scrollback search    | Yes        | Yes        | Yes        | Yes        |
| Reflow on resize     | Yes        | No         | Yes        | No         |
| IPC / scriptable     | Yes        | Yes        | Yes        | No         |
| Cross-platform       | Yes        | Unix       | Unix       | Unix       |
| Config format        | TOML       | Custom     | KDL        | Custom     |
| Automated tests      | 1,379      | ~0         | ~400       | ~0         |
| AI agent protocol    | Yes        | No         | No         | No         |
| OSC 52 clipboard     | Yes        | Partial    | No         | No         |
| Fuzz tested          | Yes        | No         | No         | No         |
| Synchronized panes   | Yes        | Yes        | No         | No         |
| Shell integration    | Yes        | No         | No         | No         |
| Session recording    | Yes        | No         | No         | No         |
| Smart selection      | Yes        | Plugin     | No         | No         |

---

## Performance Targets

| Metric                   | Target                   |
|--------------------------|--------------------------|
| Input-to-pixel latency   | < 8 ms (120 fps)         |
| VT parse throughput      | > 500 MB/s (achieved: 598 MB/s) |
| Memory per pane          | < 5 MB (10k scrollback)  |
| Cold start               | < 50 ms                  |
| Release binary size      | < 2 MB                   |

---

## Documentation

| Guide | Description |
|-------|-------------|
| [Getting Started](doc/GETTING_STARTED.md) | Installation, first session, basic navigation |
| [Configuration](doc/CONFIGURATION.md) | Full config reference with examples |
| [Keybindings](doc/KEYBINDINGS.md) | All keybindings, remapping, modifier syntax |
| [AI Integration](doc/AI_INTEGRATION.md) | Claude Code, agent teams, OSC notifications |
| [IPC Protocol](doc/IPC_PROTOCOL.md) | Complete API reference for automation |
| [Layout Templates](doc/LAYOUT_TEMPLATES.md) | .emux.toml project layouts |
| [Shell Integration](doc/SHELL_INTEGRATION.md) | OSC 133, hint mode, smart selection |
| [Session Recording](doc/RECORDING.md) | Record and replay terminal sessions |
| [Windows](doc/WINDOWS.md) | Windows-specific setup and troubleshooting |

---

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for build instructions, testing guidelines, coding standards, and the PR process.

---

## Community

- [GitHub Discussions](https://github.com/IISweetHeartII/emux/discussions) -- Questions, ideas, show & tell
- [GitHub Issues](https://github.com/IISweetHeartII/emux/issues) -- Bug reports, feature requests
- [Code of Conduct](CODE_OF_CONDUCT.md)
- [Security Policy](SECURITY.md)

---

## License

[MIT](LICENSE) -- free for personal and commercial use.
