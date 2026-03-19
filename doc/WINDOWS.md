# Windows Guide

emux supports Windows 10 and later with native ConPTY, WSL, and Windows Terminal.

---

## Supported platforms

| Platform            | Status       | Notes                                      |
|---------------------|--------------|--------------------------------------------|
| Windows 10+ (ConPTY)| Supported   | Native Windows build using ConPTY API      |
| WSL 1/2             | Supported    | Runs as a native Linux binary inside WSL   |
| Windows Terminal    | Recommended  | Best rendering and color support           |
| cmd.exe / PowerShell| Works        | 256-color support required                 |

---

## Installation

### PowerShell (recommended)

```powershell
irm https://raw.githubusercontent.com/IISweetHeartII/emux/main/install.ps1 | iex
```

This downloads the correct prebuilt binary for your architecture (x86_64 or aarch64) and places it in your PATH.

### Cargo

```powershell
cargo install emux
```

Requires Rust 1.85+ and the MSVC toolchain.

### From source

```powershell
git clone https://github.com/IISweetHeartII/emux.git
cd emux
cargo build --release
# Binary: target\release\emux.exe
```

### Prebuilt binaries

Download from [GitHub Releases](https://github.com/IISweetHeartII/emux/releases/latest):

| Binary                   | Architecture |
|--------------------------|--------------|
| `emux-x86_64-pc-windows-msvc.zip`   | x86_64  |
| `emux-aarch64-pc-windows-msvc.zip`  | aarch64 |

Extract the zip and add the directory to your PATH.

---

## How the daemon works on Windows

On Unix, emux uses Unix domain sockets for IPC between the client and daemon:

```
/tmp/emux-sockets/emux-<session>.sock
```

Windows does not support Unix domain sockets in all configurations, so the Windows daemon uses **TCP loopback** (`127.0.0.1`) instead. The daemon binds to a local port and the client connects over localhost. This is functionally identical -- sessions persist, detach/attach works the same way, and the IPC protocol (length-prefixed JSON) is unchanged.

No configuration is needed. emux detects the platform and selects the correct transport automatically.

---

## Known differences from Unix

### Status bar time

The status bar clock shows **UTC** on Windows, not local time. This is a known limitation of the current implementation.

### File path hints

Smart selection and hint mode detect file paths in pane content. On Windows, this includes:

- Drive-letter paths: `C:\Users\you\project\src\main.rs`
- UNC paths: `\\server\share\file.txt`
- Forward-slash paths: `C:/Users/you/project/src/main.rs`

Unix-style paths (`/home/user/...`) are not matched on Windows since they are not valid native paths.

### Line endings

Terminal output uses CRLF on Windows. emux handles this transparently -- no configuration needed.

---

## WSL integration

If you are running emux inside WSL, it behaves exactly like the Linux build. The Unix socket daemon, forkpty, and all features work natively.

### Tips for WSL users

**Access Windows files from emux inside WSL:**

```sh
cd /mnt/c/Users/you/projects
emux new dev
```

**Clipboard:** OSC 52 clipboard passthrough works in Windows Terminal with WSL. Copy operations inside emux propagate to the Windows clipboard automatically.

**Windows Terminal as your WSL host:** Set Windows Terminal as your default terminal for the best experience. It handles 256-color, true color, and Unicode correctly.

---

## Windows Terminal recommended settings

Add these to your Windows Terminal `settings.json` for the best emux experience:

```json
{
    "profiles": {
        "defaults": {
            "colorScheme": "One Half Dark",
            "font": {
                "face": "Cascadia Code",
                "size": 12
            },
            "padding": "0",
            "scrollbarState": "hidden"
        }
    }
}
```

**Why hide the scrollbar?** emux manages its own scrollback buffer. The terminal's native scrollbar interferes with emux's rendering.

**Why zero padding?** emux draws its own borders and status bar. Terminal padding creates gaps.

---

## Troubleshooting

### "emux is not recognized as a command"

The binary is not in your PATH. Either:

1. Re-run the install script, which adds the install directory to PATH.
2. Add the directory containing `emux.exe` to your PATH manually:

```powershell
$env:PATH += ";C:\Users\you\.cargo\bin"
# Or add permanently via System > Environment Variables
```

### "ConPTY not available"

ConPTY requires Windows 10 version 1809 (October 2018 Update) or later. Check your version:

```powershell
winver
```

If you are on an older build, update Windows or use WSL instead.

### Garbled output or missing colors

- Use **Windows Terminal** instead of the legacy console host (`conhost.exe`).
- Ensure your terminal supports 256-color mode. Windows Terminal does by default.
- Check that your font supports the Unicode characters used in Powerline separators. Cascadia Code, JetBrains Mono, and Fira Code all work.

### Daemon not starting / sessions not persisting

Check if another process is using the port. The daemon binds to a localhost port:

```powershell
netstat -ano | findstr "LISTENING" | findstr "127.0.0.1"
```

If the daemon crashed, stale state may remain. Kill orphaned emux processes:

```powershell
taskkill /IM emux.exe /F
```

Then start a fresh session with `emux`.

### Firewall warnings

The Windows daemon uses TCP loopback (127.0.0.1). Some security software may flag this. The connection never leaves your machine -- it is safe to allow.

---

## See also

- [GETTING_STARTED.md](GETTING_STARTED.md) -- Quick start guide
- [CONFIGURATION.md](CONFIGURATION.md) -- Config file reference
- [KEYBINDINGS.md](KEYBINDINGS.md) -- Keybinding reference
