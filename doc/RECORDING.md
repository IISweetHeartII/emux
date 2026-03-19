# Session Recording Guide

emux records terminal sessions in asciicast v2 format -- the same format used
by asciinema. Recordings capture all terminal output, input, and resize events
with precise timing.

## What gets recorded

- **Output events** (`"o"`): everything the terminal displays (command output,
  prompts, escape sequences)
- **Input events** (`"i"`): keystrokes sent to the terminal
- **Resize events** (`"r"`): terminal dimension changes

## Starting a recording

```bash
# Record the current session
emux record my-session.cast

# Record with a title
emux record --title "Deploy walkthrough" deploy.cast
```

The recording starts immediately and captures all pane output until stopped.

## Stopping a recording

```bash
# Stop recording (or exit the session)
emux record --stop
```

The recorder flushes all buffered events and closes the file.

## Replaying a recording

Recordings are standard asciicast v2 files. Play them with asciinema:

```bash
# Install asciinema if needed
pip install asciinema

# Play back a recording
asciinema play my-session.cast

# Play at 2x speed
asciinema play -s 2 my-session.cast
```

## File format

Asciicast v2 is newline-delimited JSON.

**Line 1 -- Header:**

```json
{
  "version": 2,
  "width": 80,
  "height": 24,
  "timestamp": 1710864000,
  "title": "Deploy walkthrough"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `version` | integer | Always `2` |
| `width` | integer | Terminal width in columns |
| `height` | integer | Terminal height in rows |
| `timestamp` | integer | Unix epoch when recording started |
| `title` | string | Optional recording title |

**Subsequent lines -- Events:**

```json
[0.5, "o", "$ cargo build\r\n"]
[1.2, "o", "   Compiling emux v0.1.0\r\n"]
[3.0, "i", "q"]
[3.5, "r", "120x40"]
```

Each event is a JSON array: `[time, type, data]`

| Field | Type | Values |
|-------|------|--------|
| `time` | float | Seconds since recording start |
| `type` | string | `"o"` (output), `"i"` (input), `"r"` (resize) |
| `data` | string | The payload (text for o/i, `"COLSxROWS"` for r) |

## Compatibility with asciinema.org

Recordings are fully compatible with asciinema:

```bash
# Upload to asciinema.org
asciinema upload my-session.cast

# Or embed in documentation
# https://asciinema.org/a/<id>
```

The files also work with any tool that reads asciicast v2: `asciinema-player`
(web embed), `svg-term-cli` (SVG export), etc.

## Use cases

**Debugging**: record a session where a bug occurs. Share the `.cast` file
with teammates who can replay it exactly as it happened.

**Demos**: record a polished walkthrough of a feature. Upload to asciinema.org
or embed in docs with `asciinema-player`.

**Auditing**: record production sessions for compliance. The timestamp and
input/output separation make it easy to review what happened and when.

**Onboarding**: record setup procedures or common workflows. New team members
replay them step by step.

## Tips

- **Keep sessions focused.** Record one task per file. A 5-minute recording of
  a deploy is more useful than an hour-long session with unrelated work mixed in.

- **Name recordings descriptively.** Use names like `deploy-v2.3.cast` or
  `debug-memory-leak.cast`, not `recording1.cast`.

- **Use titles.** The `--title` flag embeds a description in the file header,
  which asciinema displays during playback.

- **Recording size.** Files are plain text (NDJSON). A typical 10-minute session
  produces a few hundred KB. Long sessions with heavy output (build logs, test
  suites) can be larger -- consider recording only the relevant portion.

- **Input privacy.** Input events are recorded. If you type passwords or tokens,
  they will be in the file. Either avoid entering secrets during recording or
  strip input events before sharing:

  ```bash
  # Remove input events from a recording
  head -1 session.cast > clean.cast
  tail -n +2 session.cast | grep -v '^\[.*,"i",' >> clean.cast
  ```
