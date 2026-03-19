# Keybindings

Complete reference for emux keybindings. All bindings are remappable via `~/.config/emux/config.toml`.

---

## Leader key

The **leader key** is **Ctrl+Shift**. Every default keybinding starts with the leader.

To execute a binding, hold Ctrl+Shift and press the action key. For example:

- `Leader + D` = hold Ctrl+Shift, press D
- `Leader + Up` = hold Ctrl+Shift, press Up arrow

The leader is a convenience alias. In your config, `Leader+D` is equivalent to `Ctrl+Shift+D`. You can use either form.

---

## Default bindings by category

### Panes

| Action                     | Keybinding          | Config key           |
|----------------------------|---------------------|----------------------|
| Split pane down            | `Leader + D`        | `split_down`         |
| Split pane right           | `Leader + R`        | `split_right`        |
| Close focused pane         | `Leader + X`        | `close_pane`         |
| Focus pane above           | `Leader + Up`       | `focus_up`           |
| Focus pane below           | `Leader + Down`     | `focus_down`         |
| Focus pane left            | `Leader + Left`     | `focus_left`         |
| Focus pane right           | `Leader + Right`    | `focus_right`        |
| Toggle fullscreen          | `Leader + F`        | `toggle_fullscreen`  |
| Toggle floating pane       | `Leader + G`        | `toggle_float`       |

### Tabs

| Action                     | Keybinding          | Config key           |
|----------------------------|---------------------|----------------------|
| New tab                    | `Leader + T`        | `new_tab`            |
| Close tab                  | `Leader + W`        | `close_tab`          |
| Next tab                   | `Leader + N`        | `next_tab`           |
| Previous tab               | `Leader + P`        | `prev_tab`           |

### Session

| Action                     | Keybinding          | Config key           |
|----------------------------|---------------------|----------------------|
| Detach from session        | `Leader + Q`        | `detach`             |
| Scrollback search          | `Leader + /`        | `search`             |
| Enter copy mode            | `Leader + [`        | `copy_mode`          |

---

## Remapping bindings

Override any binding in `~/.config/emux/config.toml` under the `[keys]` section:

```toml
[keys]
# Remap split down to Leader+S
split_down = "Leader+S"

# Use vim-style navigation
focus_left = "Leader+H"
focus_down = "Leader+J"
focus_up = "Leader+K"
focus_right = "Leader+L"

# Use Alt instead of Leader for tab switching
next_tab = "Alt+N"
prev_tab = "Alt+P"
```

You only need to list the bindings you want to change. Unspecified bindings keep their defaults.

---

## Modifier syntax

Bindings are strings of modifiers and a key joined by `+`.

| Modifier  | Aliases                  | Notes                                 |
|-----------|--------------------------|---------------------------------------|
| `Leader`  | --                       | Expands to `Ctrl+Shift`               |
| `Ctrl`    | `Control`                | Control key                           |
| `Shift`   | --                       | Shift key                             |
| `Alt`     | `Meta`, `Opt`, `Option`  | Alt on Linux/Windows, Option on macOS |

Multiple modifiers can be combined:

```
Ctrl+Shift+D      (same as Leader+D)
Ctrl+Alt+T
Alt+Shift+N
```

---

## Key name reference

### Letters and digits

Single uppercase characters: `A` through `Z`, `0` through `9`.

### Arrow keys

`Up`, `Down`, `Left`, `Right`

### Function keys

`F1` through `F12`

### Special keys

| Key name    | Key              |
|-------------|------------------|
| `Tab`       | Tab              |
| `Enter`     | Enter / Return   |
| `Esc`       | Escape           |
| `Backspace` | Backspace        |
| `Delete`    | Delete / Del     |
| `Home`      | Home             |
| `End`       | End              |
| `PageUp`    | Page Up          |
| `PageDown`  | Page Down        |

### Punctuation and symbols

Use the character directly: `/`, `[`, `]`, `\`, `;`, `'`, `,`, `.`, `-`, `=`, `` ` ``

---

## Tips for vim users

Map pane navigation to HJKL:

```toml
[keys]
focus_left = "Leader+H"
focus_down = "Leader+J"
focus_up = "Leader+K"
focus_right = "Leader+L"
```

The default copy mode (`Leader + [`) mirrors vim's approach -- similar to tmux's vi copy mode.

Consider `cursor_shape = "block"` and `cursor_blink = false` for a vim-like feel:

```toml
cursor_shape = "block"
cursor_blink = false
```

---

## Tips for emacs users

Use Alt-based bindings to avoid conflicts with emacs Ctrl sequences:

```toml
[keys]
split_down = "Alt+D"
split_right = "Alt+R"
close_pane = "Alt+X"
focus_up = "Alt+Up"
focus_down = "Alt+Down"
focus_left = "Alt+Left"
focus_right = "Alt+Right"
new_tab = "Alt+T"
close_tab = "Alt+W"
next_tab = "Alt+N"
prev_tab = "Alt+P"
detach = "Alt+Q"
search = "Alt+/"
toggle_fullscreen = "Alt+F"
toggle_float = "Alt+G"
copy_mode = "Alt+["
```

This keeps Ctrl+A, Ctrl+E, Ctrl+K, and other emacs-standard bindings available for your shell and editor.

---

## See also

- [CONFIGURATION.md](CONFIGURATION.md) -- Full config file reference
- [GETTING_STARTED.md](GETTING_STARTED.md) -- Quick start guide
