# AI Integration Guide

emux is built with first-class support for AI coding agents. Its IPC protocol
exposes pane management primitives that let agents spawn, observe, and control
terminal panes programmatically -- exactly what tools like Claude Code need to
orchestrate multi-step workflows.

## Why emux for AI workflows

- **Structured IPC**: length-prefixed JSON over a Unix socket. No screen-scraping.
- **Agent-oriented commands**: `SplitPane`, `CapturePane`, `SendKeys`, `ListPanes`,
  `GetPaneInfo`, `ResizePane`, `SetPaneTitle` -- purpose-built for tool use.
- **Notification hooks**: OSC 9/99/777 sequences let processes signal task
  completion. emux surfaces these as tab indicators and status bar counts.
- **Deterministic capture**: `CapturePane` returns the visible text of any pane,
  giving agents reliable observation without terminal emulator quirks.

## Claude Code integration

### Running Claude Code in emux panes

Start a session, then launch Claude Code in a pane:

```bash
emux new-session -s work
# Inside emux, open a split and run claude
emux split-pane -v -- claude
```

Or define it in a `.emux.toml` layout (see `LAYOUT_TEMPLATES.md`):

```toml
name = "ai-dev"

[[panes]]
command = "nvim ."
title = "editor"

[[panes]]
command = "claude"
split = "vertical"
size = 50
title = "claude"
```

### Agent team support via IPC

Multiple agents can coordinate through the emux IPC socket. Each agent connects
to the socket, discovers panes with `ListPanes`, and operates on them
independently.

The socket lives at:

| Platform | Location |
|----------|----------|
| Unix/macOS | `/tmp/emux-sockets/emux-<session>.sock` |
| Windows | TCP port listed in `/tmp/emux-sockets/emux-<session>.port` |

### How the agent protocol works

The core agent commands and their responses:

| Command | Purpose | Response |
|---------|---------|----------|
| `SplitPane` | Create a new pane | `SpawnResult { pane_id }` |
| `CapturePane` | Read visible text from a pane | `PaneCaptured { pane_id, content }` |
| `SendKeys` | Type text/keys into a pane | `Ack` |
| `ListPanes` | Enumerate all panes | `PaneList { panes }` |
| `GetPaneInfo` | Get details about one pane | `PaneInfo { pane }` |
| `ResizePane` | Resize a specific pane | `Ack` |
| `SetPaneTitle` | Label a pane | `Ack` |

Each `PaneEntry` in a `PaneList` response contains:

```json
{
  "id": 2,
  "title": "claude",
  "cols": 80,
  "rows": 24,
  "active": false,
  "has_notification": true
}
```

### Example: scripting a Claude Code workflow

Python script that creates a pane, runs a command, waits, then reads the output:

```python
import socket
import struct
import json
import time

SOCK = "/tmp/emux-sockets/emux-work.sock"

def send(msg):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.connect(SOCK)
    payload = json.dumps(msg).encode()
    s.sendall(struct.pack(">I", len(payload)) + payload)
    length = struct.unpack(">I", s.recv(4))[0]
    resp = json.loads(s.recv(length))
    s.close()
    return resp

# Split a new pane to the right
result = send({"SplitPane": {"direction": "Vertical", "size": 50}})
pane_id = result["SpawnResult"]["pane_id"]

# Run tests in the new pane
send({"SendKeys": {"pane_id": pane_id, "keys": "cargo test\n"}})

# Wait for tests to finish, then capture output
time.sleep(10)
output = send({"CapturePane": {"pane_id": pane_id}})
print(output["PaneCaptured"]["content"])
```

## OSC notifications: signaling task completion

AI agents (or any process) can emit OSC escape sequences to notify emux that
a task is done. This is useful for long-running builds, test suites, or agent
steps.

### Supported OSC sequences

| Sequence | Origin | Format |
|----------|--------|--------|
| OSC 9 | iTerm2 | `\e]9;<body>\a` |
| OSC 99 | kitty | `\e]99;i=<id>:<body>\a` |
| OSC 777 | rxvt-unicode | `\e]777;notify;<title>;<body>\a` |

### How notifications appear

- **Tab indicator**: the pane's tab shows an unread marker.
- **Status bar**: a notification count badge appears.
- **IPC**: `ListPanes` and `GetPaneInfo` include `has_notification: true`.

### Emitting a notification from a script

```bash
# Simple notification (OSC 9)
printf '\e]9;Build complete\a'

# With title (OSC 777)
printf '\e]777;notify;CI;All tests passed\a'
```

An AI agent can emit these after completing a step so that the user (or a
supervising agent) knows to check the pane.

## IPC socket basics for tool authors

1. Connect to the Unix socket at `/tmp/emux-sockets/emux-<session>.sock`.
2. Send a 4-byte big-endian length prefix followed by a JSON-encoded `ClientMessage`.
3. Read a 4-byte big-endian length prefix followed by a JSON-encoded `ServerMessage`.
4. Each connection handles one request-response pair.

See `IPC_PROTOCOL.md` for the full message reference.

## Compatible tools

emux works with any AI coding tool that can run in a terminal:

- **Claude Code** -- Anthropic's CLI agent
- **OpenCode** -- open-source coding agent
- **Aider** -- AI pair programming in the terminal
- **Goose** -- Block's AI agent
- **Amp** -- Sourcegraph's AI dev tool
- **Cursor Agent** -- Cursor's terminal mode

Any tool that reads/writes stdin/stdout works in an emux pane. Tools that
can speak the IPC protocol get full pane orchestration.
