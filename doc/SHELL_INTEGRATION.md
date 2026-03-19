# Shell Integration Guide

Shell integration connects your shell to emux via escape sequences, enabling
per-command navigation, exit code tracking, and background task alerts.

## What shell integration provides

### Per-command navigation

Jump between prompts in scrollback. Instead of scrolling through hundreds of
lines of build output, press a key to land on the next or previous prompt.

### Exit code tracking

emux knows whether the last command succeeded or failed. The status bar can show
the exit code, and pane borders can change color on failure.

### Alert when done

When a pane is in the background (not focused) and a command finishes, emux
marks it with a notification indicator. Useful for long-running builds or test
suites -- you work in another pane and get alerted when the background task
completes.

## How it works

The shell emits OSC 133 escape sequences at specific points in the
prompt-command-output cycle. emux records these as `ShellMark` entries tied to
scrollback rows, which it uses for navigation and status tracking.

The sequence:

```
1. Shell displays prompt     --> emits OSC 133;A  (prompt start)
2. User types a command
3. User presses Enter        --> emits OSC 133;B  (command start)
4. Command produces output   --> emits OSC 133;C  (output start)
5. Command finishes          --> emits OSC 133;D;N (finished, exit code N)
```

## Setup

### bash

Add to `~/.bashrc`:

```bash
if [[ -n "$EMUX" ]]; then
  PS0='\e]133;C\a'

  __emux_prompt_start() {
    printf '\e]133;D;%s\a' "$__emux_last_exit"
    printf '\e]133;A\a'
  }

  __emux_command_start() {
    printf '\e]133;B\a'
  }

  __emux_save_exit() {
    __emux_last_exit=$?
  }

  __emux_last_exit=0
  PROMPT_COMMAND="__emux_save_exit;__emux_prompt_start;${PROMPT_COMMAND}"
  trap '__emux_command_start' DEBUG
fi
```

### zsh

Add to `~/.zshrc`:

```zsh
if [[ -n "$EMUX" ]]; then
  __emux_precmd() {
    local exit_code=$?
    print -Pn '\e]133;D;%s\a' "$exit_code"
    print -Pn '\e]133;A\a'
  }

  __emux_preexec() {
    print -Pn '\e]133;B\a'
    print -Pn '\e]133;C\a'
  }

  precmd_functions+=(__emux_precmd)
  preexec_functions+=(__emux_preexec)
fi
```

### fish

Add to `~/.config/fish/config.fish`:

```fish
if set -q EMUX
  function __emux_prompt --on-event fish_prompt
    printf '\e]133;D;%s\a' $__emux_last_status
    printf '\e]133;A\a'
  end

  function __emux_preexec --on-event fish_preexec
    printf '\e]133;B\a'
    printf '\e]133;C\a'
  end

  function __emux_postexec --on-event fish_postexec
    set -g __emux_last_status $status
  end

  set -g __emux_last_status 0
end
```

## Escape sequence reference

| Sequence | Name | Meaning |
|----------|------|---------|
| `\e]133;A\a` | Prompt start | Shell is displaying a prompt |
| `\e]133;B\a` | Command start | User pressed Enter, command is about to run |
| `\e]133;C\a` | Output start | Command output begins |
| `\e]133;D;<N>\a` | Command finished | Command exited with code `<N>` |

These follow the [FinalTerm/iTerm2 shell integration](https://iterm2.com/documentation-escape-codes.html)
specification used by iTerm2, VS Code terminal, WezTerm, and others.

## Navigation keybindings

| Key | Action |
|-----|--------|
| `Ctrl-Shift-Up` | Jump to previous prompt |
| `Ctrl-Shift-Down` | Jump to next prompt |

These keybindings move the viewport so the target prompt is at the top of the
screen. They only work when shell integration is active (marks exist in
scrollback).

## Smart selection / hint mode

emux scans the visible terminal content for common patterns and lets you
select them quickly without reaching for the mouse.

### Detected patterns

| Pattern | Examples |
|---------|----------|
| URLs | `https://github.com/user/repo`, `ftp://files.example.com/data` |
| File paths | `/usr/local/bin/emux`, `./src/main.rs`, `~/projects/emux` |
| Git SHAs | `abc1234`, `abc1234567890abcdef1234567890abcdef1234` |
| IPv4 addresses | `192.168.1.100`, `10.0.0.1` |
| Email addresses | `user@example.com` |

### How to enter hint mode

Press the hint mode keybinding (default: `Ctrl-Shift-H`). emux highlights all
detected patterns on screen and overlays a single-character label (a-z, then
A-Z) on each one.

Type the label character to select that hint. The matched text is copied to the
clipboard. Press `Escape` to cancel.

### How it works internally

1. emux converts each visible row to text.
2. Regex patterns run in priority order: URLs > emails > IPs > file paths > git SHAs.
3. Overlapping matches are suppressed (a URL containing an IP won't also match as an IP).
4. Up to 52 matches get single-character labels (a-z, A-Z).

### Example

Terminal shows:

```
$ git log --oneline
abc1234 Fix build
def5678 Add feature
$ cat /etc/hosts
127.0.0.1 localhost
```

Press `Ctrl-Shift-H`. emux highlights:

```
[a] abc1234   [b] def5678   [c] /etc/hosts   [d] 127.0.0.1
```

Press `a` to copy `abc1234` to the clipboard.
