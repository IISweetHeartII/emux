# IPC Protocol Reference

Complete reference for the emux inter-process communication protocol.

## Protocol overview

emux uses a length-prefixed JSON protocol over Unix domain sockets (TCP on
Windows). Each connection carries a single request-response exchange.

- **Transport**: Unix stream socket (macOS/Linux), TCP loopback (Windows)
- **Framing**: 4-byte big-endian length prefix + JSON payload
- **Encoding**: UTF-8 JSON
- **Protocol version**: 1

## Socket location

| Platform | Path |
|----------|------|
| Unix/macOS | `/tmp/emux-sockets/emux-<name>.sock` |
| Windows | TCP port in `/tmp/emux-sockets/emux-<name>.port` |

Where `<name>` is the session name.

## Message format

Every message (client and server) is framed as:

```
[4 bytes: big-endian u32 length][N bytes: JSON payload]
```

Example: sending `{"Ping"}` (6 bytes):

```
\x00\x00\x00\x06{"Ping"}
```

## ClientMessage types

### Ping

Health check.

```json
"Ping"
```

Response: `"Pong"`

### GetVersion

Get the protocol version.

```json
"GetVersion"
```

Response:

```json
{"Version": {"version": 1}}
```

### Resize

Resize the active pane.

```json
{"Resize": {"cols": 120, "rows": 40}}
```

Response: `"Ack"`

### Detach

Detach the current client from the session.

```json
"Detach"
```

Response: `"Ack"`

### ListSessions

List all active sessions.

```json
"ListSessions"
```

Response:

```json
{
  "SessionList": {
    "sessions": [
      {
        "name": "work",
        "tabs": 2,
        "panes": 3,
        "cols": 120,
        "rows": 40
      }
    ]
  }
}
```

### KillSession

Terminate a session by name.

```json
{"KillSession": {"name": "work"}}
```

Response: `"Ack"`

### SpawnPane

Create a new pane with an optional split direction.

```json
{"SpawnPane": {"direction": "horizontal"}}
```

Response:

```json
{"SpawnResult": {"pane_id": 3}}
```

### KillPane

Kill a specific pane.

```json
{"KillPane": {"pane_id": 2}}
```

Response: `"Ack"`

### FocusPane

Set focus to a specific pane.

```json
{"FocusPane": {"pane_id": 1}}
```

Response: `"Ack"`

### KeyInput

Send raw key data to the focused pane.

```json
{"KeyInput": {"data": [104, 105, 10]}}
```

Response: none (fire-and-forget render cycle)

### SplitPane

Split the focused pane. Returns the new pane's ID.

```json
{"SplitPane": {"direction": "Vertical", "size": 50}}
```

`direction`: `"Horizontal"` (top/bottom) or `"Vertical"` (left/right).
`size`: optional, percentage or fixed columns/rows.

Response:

```json
{"SpawnResult": {"pane_id": 4}}
```

### CapturePane

Capture the visible text content of a pane.

```json
{"CapturePane": {"pane_id": 2}}
```

Response:

```json
{
  "PaneCaptured": {
    "pane_id": 2,
    "content": "$ cargo test\n   Compiling emux v0.1.0\n   Finished ...\n"
  }
}
```

### SendKeys

Send text or keystrokes to a specific pane.

```json
{"SendKeys": {"pane_id": 2, "keys": "cargo build\n"}}
```

Response: `"Ack"`

### ListPanes

List all panes in the active tab.

```json
"ListPanes"
```

Response:

```json
{
  "PaneList": {
    "panes": [
      {
        "id": 0,
        "title": "editor",
        "cols": 80,
        "rows": 24,
        "active": true,
        "has_notification": false
      },
      {
        "id": 1,
        "title": "tests",
        "cols": 80,
        "rows": 12,
        "active": false,
        "has_notification": true
      }
    ]
  }
}
```

### GetPaneInfo

Get detailed info about a specific pane.

```json
{"GetPaneInfo": {"pane_id": 1}}
```

Response:

```json
{
  "PaneInfo": {
    "pane": {
      "id": 1,
      "title": "tests",
      "cols": 80,
      "rows": 12,
      "active": false,
      "has_notification": true
    }
  }
}
```

### ResizePane

Resize a specific pane.

```json
{"ResizePane": {"pane_id": 1, "cols": 100, "rows": 30}}
```

Response: `"Ack"`

### SetPaneTitle

Set a pane's display title.

```json
{"SetPaneTitle": {"pane_id": 1, "title": "build output"}}
```

Response: `"Ack"`

## ServerMessage types

| Message | When |
|---------|------|
| `Pong` | Response to `Ping` |
| `Version { version }` | Response to `GetVersion` |
| `Render { pane_id, content }` | Pushed when pane content changes |
| `SpawnResult { pane_id }` | Response to `SpawnPane` / `SplitPane` |
| `Ack` | Generic success |
| `Error { message }` | Something went wrong |
| `SessionList { sessions }` | Response to `ListSessions` |
| `PaneCaptured { pane_id, content }` | Response to `CapturePane` |
| `PaneList { panes }` | Response to `ListPanes` |
| `PaneInfo { pane }` | Response to `GetPaneInfo` |

## Data structures

### SessionEntry

```json
{
  "name": "string",
  "tabs": 0,
  "panes": 0,
  "cols": 0,
  "rows": 0
}
```

### PaneEntry

```json
{
  "id": 0,
  "title": "string",
  "cols": 0,
  "rows": 0,
  "active": false,
  "has_notification": false
}
```

## Example: Python client

```python
import socket
import struct
import json

def emux_request(session_name, message):
    """Send a request to emux and return the response."""
    sock_path = f"/tmp/emux-sockets/emux-{session_name}.sock"
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.connect(sock_path)

    # Send
    payload = json.dumps(message).encode("utf-8")
    s.sendall(struct.pack(">I", len(payload)) + payload)

    # Receive
    length_bytes = s.recv(4)
    length = struct.unpack(">I", length_bytes)[0]
    data = b""
    while len(data) < length:
        data += s.recv(length - len(data))
    s.close()

    return json.loads(data)

# List all panes
panes = emux_request("work", "ListPanes")
print(json.dumps(panes, indent=2))

# Capture pane 0
captured = emux_request("work", {"CapturePane": {"pane_id": 0}})
print(captured["PaneCaptured"]["content"])
```

## Example: bash with socat

```bash
# Ping
echo -ne '\x00\x00\x00\x06"Ping"' | \
  socat - UNIX-CONNECT:/tmp/emux-sockets/emux-work.sock | \
  tail -c +5

# List panes
MSG='"ListPanes"'
LEN=$(printf '%08x' ${#MSG})
echo -ne "\\x${LEN:0:2}\\x${LEN:2:2}\\x${LEN:4:2}\\x${LEN:6:2}${MSG}" | \
  socat - UNIX-CONNECT:/tmp/emux-sockets/emux-work.sock | \
  tail -c +5 | jq .

# Send keys to pane 2
MSG='{"SendKeys":{"pane_id":2,"keys":"ls -la\n"}}'
LEN=$(printf '%08x' ${#MSG})
echo -ne "\\x${LEN:0:2}\\x${LEN:2:2}\\x${LEN:4:2}\\x${LEN:6:2}${MSG}" | \
  socat - UNIX-CONNECT:/tmp/emux-sockets/emux-work.sock | \
  tail -c +5
```
