//! Golden tests derived from Alacritty's ref test suite.
//!
//! Each test corresponds to a recording + expected grid snapshot in
//! `tests/golden/ref/<name>/`. The pattern is:
//!   1. Read `size.json` for terminal dimensions `{columns, screen_lines}`.
//!   2. Read `config.json` for options like `{history_size}`.
//!   3. Read `alacritty.recording` (raw byte stream).
//!   4. Feed the recording through `Parser` + `Screen`.
//!   5. Snapshot the resulting grid with `insta` for comparison.

use std::fs;
use std::path::{Path, PathBuf};

use emux_term::Screen;
use emux_vt::Parser;
use serde_json::Value;

/// Directory containing all golden ref test data.
fn ref_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/ref")
}

/// Size descriptor from `size.json`.
struct TermSize {
    columns: usize,
    screen_lines: usize,
}

/// Config from `config.json`.
struct RefConfig {
    history_size: usize,
}

/// Read the size.json file.
fn read_size(dir: &Path) -> TermSize {
    let data = fs::read_to_string(dir.join("size.json"))
        .unwrap_or_else(|e| panic!("Failed to read size.json in {}: {e}", dir.display()));
    let v: Value = serde_json::from_str(&data).unwrap();
    TermSize {
        columns: v["columns"].as_u64().unwrap() as usize,
        screen_lines: v["screen_lines"].as_u64().unwrap() as usize,
    }
}

/// Read the config.json file.
fn read_config(dir: &Path) -> RefConfig {
    let data = fs::read_to_string(dir.join("config.json"))
        .unwrap_or_else(|e| panic!("Failed to read config.json in {}: {e}", dir.display()));
    let v: Value = serde_json::from_str(&data).unwrap();
    RefConfig {
        history_size: v.get("history_size").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
    }
}

/// Render the screen grid as a human-readable string for snapshot comparison.
///
/// Format: each row is on its own line, with cells rendered as their character.
/// Trailing spaces on each row are trimmed. Empty trailing rows are preserved
/// to maintain the exact grid dimensions.
///
/// Attributes are encoded inline when non-default:
///   `[bold]`, `[italic]`, `[fg:N]`, `[bg:N]`, `[fg:r,g,b]`, `[underline]`, etc.
/// This keeps the snapshot human-readable while capturing attribute state.
fn render_grid(screen: &Screen) -> String {
    use emux_term::grid::UnderlineStyle;
    use emux_term::{CellAttrs, Color};

    let mut lines = Vec::with_capacity(screen.rows());
    for row_idx in 0..screen.rows() {
        let mut line = String::new();
        let mut last_non_space = 0;
        // First pass: find the last non-default cell
        for col in (0..screen.cols()).rev() {
            let cell = screen.grid.cell(row_idx, col);
            if cell.c != ' '
                || cell.fg != Color::Default
                || cell.bg != Color::Default
                || cell.attrs != CellAttrs::default()
            {
                last_non_space = col + 1;
                break;
            }
        }

        for col in 0..last_non_space {
            let cell = screen.grid.cell(row_idx, col);

            // Skip continuation cells for wide characters
            if cell.width == 0 {
                continue;
            }

            // Emit attribute markers if non-default
            let mut attrs = Vec::new();
            if cell.attrs.bold {
                attrs.push("bold".to_string());
            }
            if cell.attrs.italic {
                attrs.push("italic".to_string());
            }
            match cell.attrs.underline {
                UnderlineStyle::Single => attrs.push("underline".to_string()),
                UnderlineStyle::Double => attrs.push("double-underline".to_string()),
                UnderlineStyle::Curly => attrs.push("curly-underline".to_string()),
                UnderlineStyle::None => {}
            }
            if cell.attrs.blink {
                attrs.push("blink".to_string());
            }
            if cell.attrs.reverse {
                attrs.push("reverse".to_string());
            }
            if cell.attrs.invisible {
                attrs.push("invisible".to_string());
            }
            if cell.attrs.strikethrough {
                attrs.push("strike".to_string());
            }
            match cell.fg {
                Color::Default => {}
                Color::Indexed(n) => attrs.push(format!("fg:{n}")),
                Color::Rgb(r, g, b) => attrs.push(format!("fg:{r},{g},{b}")),
            }
            match cell.bg {
                Color::Default => {}
                Color::Indexed(n) => attrs.push(format!("bg:{n}")),
                Color::Rgb(r, g, b) => attrs.push(format!("bg:{r},{g},{b}")),
            }

            if !attrs.is_empty() {
                line.push('[');
                line.push_str(&attrs.join(","));
                line.push(']');
            }

            line.push(cell.c);
        }

        lines.push(line);
    }

    // Trim trailing empty lines but keep at least one
    while lines.len() > 1 && lines.last().map_or(false, |l| l.is_empty()) {
        lines.pop();
    }

    lines.join("\n")
}

/// Run a golden test: read recording data, feed through parser, return rendered grid.
fn run_golden_test(name: &str) -> String {
    let dir = ref_dir().join(name);
    assert!(
        dir.exists(),
        "Golden test directory not found: {}",
        dir.display()
    );

    let size = read_size(&dir);
    let config = read_config(&dir);

    let recording = fs::read(dir.join("alacritty.recording"))
        .unwrap_or_else(|e| panic!("Failed to read recording for {name}: {e}"));

    let mut screen = Screen::new(size.columns, size.screen_lines);
    screen.grid.set_scrollback_limit(config.history_size);

    let mut parser = Parser::new();
    parser.advance(&mut screen, &recording);

    render_grid(&screen)
}

// ── Macro to generate golden tests ──────────────────────────────────────────

macro_rules! golden_tests {
    ($($name:ident),* $(,)?) => {
        $(
            #[test]
            fn $name() {
                let result = run_golden_test(stringify!($name));
                insta::assert_snapshot!(result);
            }
        )*
    };
}

// All 45 golden tests from Alacritty's ref suite.
golden_tests! {
    alt_reset,
    clear_underline,
    colored_reset,
    colored_underline,
    csi_rep,
    decaln_reset,
    deccolm_reset,
    delete_chars_reset,
    delete_lines,
    erase_chars_reset,
    erase_in_line,
    fish_cc,
    grid_reset,
    history,
    hyperlinks,
    indexed_256_colors,
    insert_blank_reset,
    issue_855,
    ll,
    newline_with_cursor_beyond_scroll_region,
    origin_goto,
    region_scroll_down,
    row_reset,
    saved_cursor,
    saved_cursor_alt,
    scroll_in_region_up_preserves_history,
    scroll_up_reset,
    selective_erasure,
    sgr,
    tab_rendering,
    tmux_git_log,
    tmux_htop,
    underline,
    vim_24bitcolors_bce,
    vim_large_window_scroll,
    vim_simple_edit,
    vttest_cursor_movement_1,
    vttest_insert,
    vttest_origin_mode_1,
    vttest_origin_mode_2,
    vttest_scroll,
    vttest_tab_clear_set,
    wrapline_alt_toggle,
    zerowidth,
    zsh_tab_completion,
}
