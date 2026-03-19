# Configuration Reference

emux works perfectly with zero configuration. Every option has a sensible default. You only need a config file to override what you want to change.

---

## Config file location

```
~/.config/emux/config.toml
```

If the file does not exist, all defaults are used. Create it only when you want to override something.

---

## Deep merge behavior

emux uses deep merging. You only specify the fields you want to change -- everything else keeps its default value. For example, this config only changes the cursor shape; all other options (theme, keybindings, font size, etc.) remain at their defaults:

```toml
cursor_shape = "bar"
```

This applies to nested sections too. To change just the background color without touching the rest of the theme:

```toml
[theme]
background = "#1E1E2E"
```

---

## Hot-reload

emux watches `~/.config/emux/config.toml` for changes. When you save the file, the new configuration is applied automatically -- no restart needed.

---

## General options

| Option             | Type              | Default      | Description                                                      |
|--------------------|-------------------|--------------|------------------------------------------------------------------|
| `font_size`        | `float`           | `14.0`       | Font size in points.                                             |
| `font_family`      | `string` or unset | unset        | Font family name. When unset, the terminal's default is used.    |
| `scrollback_limit` | `integer`         | `10000`      | Maximum number of lines kept in the scrollback buffer per pane.  |
| `tab_width`        | `integer`         | `8`          | Number of columns per tab stop.                                  |
| `cursor_shape`     | `string`          | `"block"`    | Cursor shape: `"block"`, `"underline"`, or `"bar"`.              |
| `cursor_blink`     | `boolean`         | `true`       | Whether the cursor blinks.                                       |
| `bold_is_bright`   | `boolean`         | `false`      | Render bold text using bright ANSI colors instead of bold weight.|

---

## `[theme]` section

The default theme is One Dark. All color values are hex strings.

| Option            | Type         | Default      | Description                                        |
|-------------------|--------------|--------------|----------------------------------------------------|
| `background`      | `string`     | `"#282C34"`  | Terminal background color.                         |
| `foreground`      | `string`     | `"#ABB2BF"`  | Default text color.                                |
| `cursor`          | `string`     | `"#528BFF"`  | Cursor color.                                      |
| `selection_bg`    | `string`     | `"#3E4451"`  | Background color for selected text.                |
| `colors`          | `[string; 16]` | One Dark palette | ANSI color palette. Indices 0-7 are normal colors, 8-15 are bright. |
| `statusbar_bg`    | `string`     | `"#080808"`  | Status bar background color.                       |
| `accent`          | `string`     | `"#00AFFF"`  | Active tab accent color in the status bar.         |
| `border_active`   | `string`     | `"#00AFFF"`  | Active pane border color.                          |
| `border_inactive` | `string`     | `"#303030"`  | Inactive pane border color.                        |
| `powerline`       | `boolean`    | `true`       | Use Powerline-style separators in the status bar.  |

### Default ANSI palette (One Dark)

| Index | Name           | Default     |
|-------|----------------|-------------|
| 0     | Black          | `#1D1F21`   |
| 1     | Red            | `#CC6666`   |
| 2     | Green          | `#B5BD68`   |
| 3     | Yellow         | `#F0C674`   |
| 4     | Blue           | `#81A2BE`   |
| 5     | Magenta        | `#B294BB`   |
| 6     | Cyan           | `#8ABEB7`   |
| 7     | White          | `#C5C8C6`   |
| 8     | Bright Black   | `#666666`   |
| 9     | Bright Red     | `#D54E53`   |
| 10    | Bright Green   | `#B9CA4A`   |
| 11    | Bright Yellow  | `#E7C547`   |
| 12    | Bright Blue    | `#7AA6DA`   |
| 13    | Bright Magenta | `#C397D8`   |
| 14    | Bright Cyan    | `#70C0B1`   |
| 15    | Bright White   | `#EAEAEA`   |

---

## `[keys]` section

All keybindings are remappable. See [KEYBINDINGS.md](KEYBINDINGS.md) for the full modifier syntax and key name reference.

| Option             | Default          | Action                              |
|--------------------|------------------|-------------------------------------|
| `split_down`       | `"Leader+D"`     | Split pane downward.                |
| `split_right`      | `"Leader+R"`     | Split pane to the right.            |
| `close_pane`       | `"Leader+X"`     | Close the focused pane.             |
| `focus_up`         | `"Leader+Up"`    | Move focus to pane above.           |
| `focus_down`       | `"Leader+Down"`  | Move focus to pane below.           |
| `focus_left`       | `"Leader+Left"`  | Move focus to pane on the left.     |
| `focus_right`      | `"Leader+Right"` | Move focus to pane on the right.    |
| `new_tab`          | `"Leader+T"`     | Open a new tab.                     |
| `close_tab`        | `"Leader+W"`     | Close the current tab.              |
| `next_tab`         | `"Leader+N"`     | Switch to the next tab.             |
| `prev_tab`         | `"Leader+P"`     | Switch to the previous tab.         |
| `detach`           | `"Leader+Q"`     | Detach from the session.            |
| `search`           | `"Leader+/"`     | Open scrollback search.             |
| `toggle_fullscreen`| `"Leader+F"`     | Toggle fullscreen for focused pane. |
| `toggle_float`     | `"Leader+G"`     | Toggle floating mode for pane.      |
| `copy_mode`        | `"Leader+["`     | Enter copy mode.                    |

---

## Example configs

### Minimal -- just change the cursor

```toml
cursor_shape = "bar"
cursor_blink = false
```

### Full config

```toml
font_size = 13.0
font_family = "JetBrains Mono"
scrollback_limit = 50000
tab_width = 4
cursor_shape = "block"
cursor_blink = true
bold_is_bright = false

[theme]
background = "#282C34"
foreground = "#ABB2BF"
cursor = "#528BFF"
selection_bg = "#3E4451"
statusbar_bg = "#080808"
accent = "#00AFFF"
border_active = "#00AFFF"
border_inactive = "#303030"
powerline = true
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

### Vim user

```toml
cursor_shape = "block"
cursor_blink = false
bold_is_bright = true

[keys]
# Use HJKL for pane navigation
focus_left = "Leader+H"
focus_down = "Leader+J"
focus_up = "Leader+K"
focus_right = "Leader+L"
```

### Dev workflow -- large scrollback, Catppuccin Mocha theme

```toml
scrollback_limit = 100000
font_size = 12.0
font_family = "Fira Code"

[theme]
background = "#1E1E2E"
foreground = "#CDD6F4"
cursor = "#F5E0DC"
selection_bg = "#45475A"
statusbar_bg = "#11111B"
accent = "#89B4FA"
border_active = "#89B4FA"
border_inactive = "#313244"
powerline = true
colors = [
    "#45475A", "#F38BA8", "#A6E3A1", "#F9E2AF",
    "#89B4FA", "#F5C2E7", "#94E2D5", "#BAC2DE",
    "#585B70", "#F38BA8", "#A6E3A1", "#F9E2AF",
    "#89B4FA", "#F5C2E7", "#94E2D5", "#A6ADC8",
]
```

---

## See also

- [GETTING_STARTED.md](GETTING_STARTED.md) -- Quick start guide
- [KEYBINDINGS.md](KEYBINDINGS.md) -- Keybinding guide with modifier syntax
- [WINDOWS.md](WINDOWS.md) -- Windows-specific configuration notes
