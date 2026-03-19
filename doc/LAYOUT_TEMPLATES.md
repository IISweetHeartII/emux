# Layout Templates Guide

Layout templates let you define a pane arrangement in a `.emux.toml` file.
Drop one in your project root and emux will set up your workspace automatically
when you start a session there.

## How it works

When emux starts in a directory, it walks up the directory tree looking for a
`.emux.toml` file. If found, emux creates panes according to the template:
spawning commands, setting working directories, and applying splits.

## File location

Place `.emux.toml` in your project root:

```
my-project/
  .emux.toml      <-- emux finds this
  src/
  tests/
  Cargo.toml
```

emux searches from the current directory upward. A template in `~/projects/`
applies to all subdirectories that don't have their own `.emux.toml`.

## TOML schema

```toml
# Required: template name
name = "my-layout"

# One or more pane definitions
[[panes]]
command = "nvim ."           # Optional: command to run (default: shell)
cwd = "src"                  # Optional: working directory relative to project root
split = "horizontal"         # Optional: "horizontal" (top/bottom) or "vertical" (left/right)
                             #           Default: "horizontal"
size = 30                    # Optional: size as percentage or fixed columns/rows
title = "editor"             # Optional: pane title
```

### Field reference

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | string | required | Template name (shown in UI) |
| `panes[].command` | string | none (opens shell) | Startup command |
| `panes[].cwd` | string | none (project root) | Working directory, relative to project root |
| `panes[].split` | string | `"horizontal"` | Split direction: `"horizontal"` or `"vertical"` |
| `panes[].size` | integer | none (even split) | Percentage (0-100) or fixed columns/rows |
| `panes[].title` | string | none (derived from process) | Pane title |

The first pane is the initial pane. Each subsequent pane splits from the
previous one in the specified direction.

## Examples

### Web development

Editor on the left, dev server top-right, test runner bottom-right.

```toml
name = "web-dev"

[[panes]]
command = "nvim ."
title = "editor"

[[panes]]
command = "npm run dev"
split = "vertical"
size = 40
title = "server"

[[panes]]
command = "npm test -- --watch"
split = "horizontal"
size = 50
title = "tests"
```

### Data science

Jupyter on the left, terminal top-right, system monitor bottom-right.

```toml
name = "data-science"

[[panes]]
command = "jupyter lab"
title = "jupyter"

[[panes]]
command = "python3"
split = "vertical"
size = 40
title = "repl"

[[panes]]
command = "htop"
split = "horizontal"
size = 30
title = "monitor"
```

### DevOps -- multiple servers

```toml
name = "devops"

[[panes]]
command = "ssh web-01"
title = "web-01"

[[panes]]
command = "ssh web-02"
split = "vertical"
title = "web-02"

[[panes]]
command = "ssh db-01"
split = "horizontal"
title = "db-01"

[[panes]]
command = "ssh db-02"
split = "vertical"
title = "db-02"
```

### Rust development

Editor, cargo watch, and a shell for ad-hoc commands.

```toml
name = "rust-dev"

[[panes]]
command = "nvim ."
title = "editor"

[[panes]]
command = "cargo watch -x 'test --workspace'"
split = "horizontal"
size = 30
title = "tests"

[[panes]]
command = "cargo watch -x clippy"
split = "vertical"
size = 50
title = "clippy"
```

### AI-assisted development

Editor, AI agent, and build output.

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

[[panes]]
command = "cargo watch -x test"
split = "horizontal"
size = 30
title = "tests"
```

## Saving the current layout as a template

Use the emux CLI to export your current pane arrangement:

```bash
emux save-layout .emux.toml
```

This writes the current session's pane layout (commands, splits, sizes, titles)
to the specified file.

## Split directions

- **`horizontal`**: splits top/bottom. The new pane appears below the current one.
- **`vertical`**: splits left/right. The new pane appears to the right.

```
horizontal split:        vertical split:
+------------+           +------+-----+
|   pane 1   |           |      |     |
+------------+           |  1   |  2  |
|   pane 2   |           |      |     |
+------------+           +------+-----+
```
