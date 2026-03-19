# Getting Started with emux

A quick start guide to get you multiplexing in under a minute.

---

## Installation

Pick the method that suits your platform:

### macOS / Linux / WSL (recommended)

```sh
curl -fsSL https://raw.githubusercontent.com/IISweetHeartII/emux/main/install.sh | sh
```

### Homebrew

```sh
brew tap IISweetHeartII/tap
brew install emux
```

### Cargo

```sh
cargo install emux
```

### Windows (PowerShell)

```powershell
irm https://raw.githubusercontent.com/IISweetHeartII/emux/main/install.ps1 | iex
```

### From source

```sh
git clone https://github.com/IISweetHeartII/emux.git
cd emux
cargo build --release
# Binary is at target/release/emux
```

Requires Rust 1.85+ when building from source.

---

## First session

```sh
emux
```

That's it. You're inside a multiplexed terminal session with the One Dark theme, a status bar, and all defaults applied. No config file needed.

---

## The leader key

All emux keybindings start with the **leader key**: **Ctrl+Shift**.

You press and hold Ctrl+Shift, then press the action key. For example, `Leader + D` means hold Ctrl+Shift, then press D.

The leader is remappable in your config file. See [CONFIGURATION.md](CONFIGURATION.md) for details.

---

## Essential keybindings

### Panes

| Action              | Keybinding          |
|---------------------|---------------------|
| Split down          | `Leader + D`        |
| Split right         | `Leader + R`        |
| Close pane          | `Leader + X`        |
| Focus up            | `Leader + Up`       |
| Focus down          | `Leader + Down`     |
| Focus left          | `Leader + Left`     |
| Focus right         | `Leader + Right`    |
| Toggle fullscreen   | `Leader + F`        |
| Toggle floating     | `Leader + G`        |

### Tabs

| Action              | Keybinding          |
|---------------------|---------------------|
| New tab             | `Leader + T`        |
| Close tab           | `Leader + W`        |
| Next tab            | `Leader + N`        |
| Previous tab        | `Leader + P`        |

### Session

| Action              | Keybinding          |
|---------------------|---------------------|
| Detach              | `Leader + Q`        |
| Scrollback search   | `Leader + /`        |
| Copy mode           | `Leader + [`        |

For the full keybinding reference, see [KEYBINDINGS.md](KEYBINDINGS.md).

---

## Basic navigation

### Splitting panes

```
Leader + D    Split the current pane horizontally (new pane appears below)
Leader + R    Split the current pane vertically (new pane appears to the right)
```

Move between panes with the arrow keys while holding the leader.

### Tabs

```
Leader + T    Open a new tab
Leader + N    Switch to the next tab
Leader + P    Switch to the previous tab
```

### Floating panes

```
Leader + G    Toggle a floating pane that overlays the tiled layout
```

### Fullscreen

```
Leader + F    Fullscreen the active pane (press again to restore)
```

---

## Session lifecycle

emux sessions persist in the background via a daemon. The typical workflow:

### 1. Create a named session

```sh
emux new work
```

Or just run `emux` for an unnamed session.

### 2. Work

Split panes, open tabs, run commands -- everything stays in the session.

### 3. Detach

```
Leader + Q
```

The session keeps running in the background. You can close your terminal or disconnect SSH.

### 4. List sessions

```sh
emux ls
```

Shows all active sessions.

### 5. Reattach

```sh
emux attach work
```

Pick up exactly where you left off.

### 6. Kill a session

```sh
emux kill work
```

Terminates the session and all its panes.

---

## What's next

- [CONFIGURATION.md](CONFIGURATION.md) -- Config file reference, themes, and example configs
- [KEYBINDINGS.md](KEYBINDINGS.md) -- Complete keybinding guide with remapping instructions
- [WINDOWS.md](WINDOWS.md) -- Windows-specific setup and tips
