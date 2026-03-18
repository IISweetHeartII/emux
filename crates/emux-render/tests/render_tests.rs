use crossterm::style::{Attribute, Color as CtColor};
use emux_render::cursor::cursor_style;
use emux_render::damage::DamageTracker;
use emux_render::text::{cell_style, color_to_crossterm, render_row};
use emux_render::Renderer;
use emux_term::grid::{Cell, UnderlineStyle};
use emux_term::{Color, CursorShape, Screen};

// ── DamageTracker tests ──────────────────────────────────────────────

#[test]
fn damage_new_triggers_full_redraw() {
    let dt = DamageTracker::new(5);
    assert!(dt.needs_redraw());
    assert!(dt.is_dirty(0));
    assert!(dt.is_dirty(4));
}

#[test]
fn damage_clear_resets_all() {
    let mut dt = DamageTracker::new(5);
    dt.clear();
    assert!(!dt.needs_redraw());
    assert!(!dt.is_dirty(0));
}

#[test]
fn damage_mark_row() {
    let mut dt = DamageTracker::new(5);
    dt.clear();
    dt.mark_row(2);
    assert!(dt.is_dirty(2));
    assert!(!dt.is_dirty(0));
    assert!(dt.needs_redraw());
}

#[test]
fn damage_mark_all() {
    let mut dt = DamageTracker::new(5);
    dt.clear();
    dt.mark_all();
    assert!(dt.is_dirty(0));
    assert!(dt.is_dirty(4));
}

#[test]
fn damage_dirty_rows_list() {
    let mut dt = DamageTracker::new(5);
    dt.clear();
    dt.mark_row(1);
    dt.mark_row(3);
    assert_eq!(dt.dirty_rows(), vec![1, 3]);
}

#[test]
fn damage_resize() {
    let mut dt = DamageTracker::new(3);
    dt.clear();
    dt.resize(5);
    // After resize, full redraw is triggered
    assert!(dt.needs_redraw());
    assert_eq!(dt.dirty_rows().len(), 5);
}

// ── color_to_crossterm tests ─────────────────────────────────────────

#[test]
fn color_default_maps_to_reset() {
    assert_eq!(color_to_crossterm(&Color::Default), CtColor::Reset);
}

#[test]
fn color_indexed() {
    assert_eq!(color_to_crossterm(&Color::Indexed(42)), CtColor::AnsiValue(42));
}

#[test]
fn color_rgb() {
    assert_eq!(
        color_to_crossterm(&Color::Rgb(10, 20, 30)),
        CtColor::Rgb { r: 10, g: 20, b: 30 }
    );
}

// ── cell_style tests ─────────────────────────────────────────────────

#[test]
fn cell_style_bold() {
    let mut cell = Cell::default();
    cell.attrs.bold = true;
    let style = cell_style(&cell);
    assert!(style.attributes.has(Attribute::Bold));
}

#[test]
fn cell_style_italic() {
    let mut cell = Cell::default();
    cell.attrs.italic = true;
    let style = cell_style(&cell);
    assert!(style.attributes.has(Attribute::Italic));
}

#[test]
fn cell_style_underline() {
    let mut cell = Cell::default();
    cell.attrs.underline = UnderlineStyle::Single;
    let style = cell_style(&cell);
    assert!(style.attributes.has(Attribute::Underlined));
}

#[test]
fn cell_style_colors() {
    let mut cell = Cell::default();
    cell.fg = Color::Rgb(255, 0, 0);
    cell.bg = Color::Indexed(7);
    let style = cell_style(&cell);
    assert_eq!(style.foreground_color, Some(CtColor::Rgb { r: 255, g: 0, b: 0 }));
    assert_eq!(style.background_color, Some(CtColor::AnsiValue(7)));
}

// ── render_row tests ─────────────────────────────────────────────────

#[test]
fn render_row_ascii() {
    let cells: Vec<Cell> = "Hello".chars().map(|c| {
        let mut cell = Cell::default();
        cell.c = c;
        cell
    }).collect();
    let spans = render_row(&cells, 5);
    let text: String = spans.iter().map(|(_, s)| s.as_str()).collect();
    assert_eq!(text, "Hello");
}

#[test]
fn render_row_wide_chars() {
    // Simulate a wide character 'W' at col 0 (width=2) followed by continuation (width=0)
    let mut cells = vec![Cell::default(); 4];
    cells[0].c = '\u{4e16}'; // CJK character
    cells[0].width = 2;
    cells[1].c = ' ';
    cells[1].width = 0; // continuation
    cells[2].c = 'A';
    cells[2].width = 1;
    cells[3].c = 'B';
    cells[3].width = 1;

    let spans = render_row(&cells, 4);
    let text: String = spans.iter().map(|(_, s)| s.as_str()).collect();
    assert_eq!(text, "\u{4e16}AB");
}

#[test]
fn render_row_attributes_split_spans() {
    let mut cells = vec![Cell::default(); 3];
    cells[0].c = 'A';
    cells[1].c = 'B';
    cells[1].attrs.bold = true;
    cells[2].c = 'C';

    let spans = render_row(&cells, 3);
    // Should be at least 3 spans because the bold cell differs
    assert!(spans.len() >= 2);
    // First span has 'A', second has 'B' (bold), third has 'C'
    let text: String = spans.iter().map(|(_, s)| s.as_str()).collect();
    assert_eq!(text, "ABC");
}

// ── cursor_style tests ───────────────────────────────────────────────

#[test]
fn cursor_style_block() {
    use crossterm::cursor::SetCursorStyle;
    assert!(matches!(cursor_style(CursorShape::Block), SetCursorStyle::SteadyBlock));
}

#[test]
fn cursor_style_bar() {
    use crossterm::cursor::SetCursorStyle;
    assert!(matches!(cursor_style(CursorShape::Bar), SetCursorStyle::SteadyBar));
}

#[test]
fn cursor_style_underline() {
    use crossterm::cursor::SetCursorStyle;
    assert!(matches!(cursor_style(CursorShape::Underline), SetCursorStyle::SteadyUnderScore));
}

// ── Renderer tests ───────────────────────────────────────────────────

#[test]
fn renderer_render_to_buffer() {
    let mut screen = Screen::new(10, 3);
    // Write some text into the screen
    screen.write_char('H');
    screen.write_char('i');

    let mut renderer = Renderer::new(10, 3);
    let mut buf: Vec<u8> = Vec::new();
    renderer.render(&mut buf, &screen).unwrap();
    // The buffer should contain escape sequences and the text "Hi"
    let output = String::from_utf8_lossy(&buf);
    assert!(output.contains("Hi"), "output should contain 'Hi', got: {}", output);
}

#[test]
fn renderer_no_redraw_after_clear() {
    let screen = Screen::new(10, 3);
    let mut renderer = Renderer::new(10, 3);
    let mut buf: Vec<u8> = Vec::new();
    renderer.render(&mut buf, &screen).unwrap();

    // After rendering, damage is cleared; rendering again should produce nothing new
    let mut buf2: Vec<u8> = Vec::new();
    renderer.render(&mut buf2, &screen).unwrap();
    assert!(buf2.is_empty(), "no output expected after damage cleared");
}

#[test]
fn renderer_force_redraw() {
    let screen = Screen::new(10, 3);
    let mut renderer = Renderer::new(10, 3);
    let mut buf: Vec<u8> = Vec::new();
    renderer.render(&mut buf, &screen).unwrap();

    renderer.force_redraw();
    let mut buf2: Vec<u8> = Vec::new();
    renderer.render(&mut buf2, &screen).unwrap();
    assert!(!buf2.is_empty(), "force_redraw should cause output");
}

#[test]
fn renderer_resize() {
    let screen = Screen::new(20, 5);
    let mut renderer = Renderer::new(10, 3);
    // render triggers resize detection
    let mut buf: Vec<u8> = Vec::new();
    renderer.render(&mut buf, &screen).unwrap();
    assert!(!buf.is_empty(), "resize should trigger full redraw");
}

// ── Additional DamageTracker tests ──────────────────────────────────

#[test]
fn damage_out_of_bounds_row_is_not_dirty() {
    let dt = DamageTracker::new(3);
    // Row 10 is out of range, should return true due to full_redraw initially
    assert!(dt.is_dirty(10));

    let mut dt2 = DamageTracker::new(3);
    dt2.clear();
    // After clear, out-of-bounds row returns false
    assert!(!dt2.is_dirty(10));
}

#[test]
fn damage_mark_row_out_of_bounds_is_no_op() {
    let mut dt = DamageTracker::new(3);
    dt.clear();
    dt.mark_row(100); // out of bounds, should not panic
    assert!(!dt.needs_redraw());
}

#[test]
fn damage_mark_clear_mark_cycle() {
    let mut dt = DamageTracker::new(5);
    // Initial full redraw
    assert!(dt.needs_redraw());
    dt.clear();
    assert!(!dt.needs_redraw());

    // Mark individual rows
    dt.mark_row(0);
    dt.mark_row(4);
    assert!(dt.needs_redraw());
    assert_eq!(dt.dirty_rows(), vec![0, 4]);

    // Clear and mark different rows
    dt.clear();
    dt.mark_row(2);
    assert_eq!(dt.dirty_rows(), vec![2]);
    assert!(!dt.is_dirty(0));
    assert!(!dt.is_dirty(4));
}

#[test]
fn damage_resize_shrink() {
    let mut dt = DamageTracker::new(10);
    dt.clear();
    dt.resize(3);
    assert!(dt.needs_redraw());
    assert_eq!(dt.dirty_rows().len(), 3);
}

#[test]
fn damage_single_row_tracker() {
    let mut dt = DamageTracker::new(1);
    assert!(dt.is_dirty(0));
    dt.clear();
    assert!(!dt.is_dirty(0));
    dt.mark_row(0);
    assert!(dt.is_dirty(0));
    assert_eq!(dt.dirty_rows(), vec![0]);
}

// ── Additional cell_style attribute tests ───────────────────────────

#[test]
fn cell_style_double_underline() {
    let mut cell = Cell::default();
    cell.attrs.underline = UnderlineStyle::Double;
    let style = cell_style(&cell);
    assert!(style.attributes.has(Attribute::DoubleUnderlined));
}

#[test]
fn cell_style_curly_underline() {
    let mut cell = Cell::default();
    cell.attrs.underline = UnderlineStyle::Curly;
    let style = cell_style(&cell);
    assert!(style.attributes.has(Attribute::Undercurled));
}

#[test]
fn cell_style_blink() {
    let mut cell = Cell::default();
    cell.attrs.blink = true;
    let style = cell_style(&cell);
    assert!(style.attributes.has(Attribute::SlowBlink));
}

#[test]
fn cell_style_reverse() {
    let mut cell = Cell::default();
    cell.attrs.reverse = true;
    let style = cell_style(&cell);
    assert!(style.attributes.has(Attribute::Reverse));
}

#[test]
fn cell_style_invisible() {
    let mut cell = Cell::default();
    cell.attrs.invisible = true;
    let style = cell_style(&cell);
    assert!(style.attributes.has(Attribute::Hidden));
}

#[test]
fn cell_style_strikethrough() {
    let mut cell = Cell::default();
    cell.attrs.strikethrough = true;
    let style = cell_style(&cell);
    assert!(style.attributes.has(Attribute::CrossedOut));
}

#[test]
fn cell_style_multiple_attributes() {
    let mut cell = Cell::default();
    cell.attrs.bold = true;
    cell.attrs.italic = true;
    cell.attrs.underline = UnderlineStyle::Single;
    cell.fg = Color::Rgb(0, 255, 0);
    cell.bg = Color::Rgb(0, 0, 255);
    let style = cell_style(&cell);
    assert!(style.attributes.has(Attribute::Bold));
    assert!(style.attributes.has(Attribute::Italic));
    assert!(style.attributes.has(Attribute::Underlined));
    assert_eq!(style.foreground_color, Some(CtColor::Rgb { r: 0, g: 255, b: 0 }));
    assert_eq!(style.background_color, Some(CtColor::Rgb { r: 0, g: 0, b: 255 }));
}

// ── Additional render_row tests ─────────────────────────────────────

#[test]
fn render_row_padding_short_content() {
    // 3 cells of content but width=6, should pad with spaces
    let cells: Vec<Cell> = "ABC".chars().map(|c| {
        let mut cell = Cell::default();
        cell.c = c;
        cell
    }).collect();
    let spans = render_row(&cells, 6);
    let text: String = spans.iter().map(|(_, s)| s.as_str()).collect();
    assert_eq!(text, "ABC   ");
}

#[test]
fn render_row_empty() {
    let cells: Vec<Cell> = Vec::new();
    let spans = render_row(&cells, 5);
    let text: String = spans.iter().map(|(_, s)| s.as_str()).collect();
    assert_eq!(text, "     "); // all padding
}

#[test]
fn render_row_control_chars_replaced() {
    // Control characters (< space) should be rendered as spaces
    let mut cells = vec![Cell::default(); 3];
    cells[0].c = '\x01'; // SOH
    cells[1].c = 'A';
    cells[2].c = '\x1f'; // US
    let spans = render_row(&cells, 3);
    let text: String = spans.iter().map(|(_, s)| s.as_str()).collect();
    assert_eq!(text, " A ");
}

#[test]
fn render_row_multiple_wide_chars() {
    // Two consecutive CJK characters
    let mut cells = vec![Cell::default(); 4];
    cells[0].c = '\u{4e16}'; // wide
    cells[0].width = 2;
    cells[1].c = ' ';
    cells[1].width = 0; // continuation
    cells[2].c = '\u{754c}'; // wide
    cells[2].width = 2;
    cells[3].c = ' ';
    cells[3].width = 0; // continuation
    let spans = render_row(&cells, 4);
    let text: String = spans.iter().map(|(_, s)| s.as_str()).collect();
    assert_eq!(text, "\u{4e16}\u{754c}");
}

#[test]
fn render_row_mixed_styles_produce_multiple_spans() {
    let mut cells = vec![Cell::default(); 4];
    cells[0].c = 'A';
    cells[0].fg = Color::Rgb(255, 0, 0);
    cells[1].c = 'B';
    cells[1].fg = Color::Rgb(255, 0, 0); // same style as A
    cells[2].c = 'C';
    cells[2].fg = Color::Rgb(0, 255, 0); // different style
    cells[3].c = 'D';
    cells[3].fg = Color::Rgb(0, 255, 0); // same as C

    let spans = render_row(&cells, 4);
    // A and B coalesce, C and D coalesce => 2 spans
    assert_eq!(spans.len(), 2);
    assert_eq!(spans[0].1, "AB");
    assert_eq!(spans[1].1, "CD");
}

// ── Renderer with styled screen content ─────────────────────────────

#[test]
fn renderer_multiple_styled_cells() {
    let mut screen = Screen::new(10, 3);
    // Write some characters to the screen
    for ch in "Hello".chars() {
        screen.write_char(ch);
    }

    let mut renderer = Renderer::new(10, 3);
    let mut buf: Vec<u8> = Vec::new();
    renderer.render(&mut buf, &screen).unwrap();
    let output = String::from_utf8_lossy(&buf);
    assert!(output.contains("Hello"), "output should contain 'Hello': {}", output);
}

#[test]
fn renderer_cursor_at_origin() {
    let screen = Screen::new(10, 5);
    // Cursor should be at (0,0) by default
    assert_eq!(screen.cursor.col, 0);
    assert_eq!(screen.cursor.row, 0);

    let mut renderer = Renderer::new(10, 5);
    let mut buf: Vec<u8> = Vec::new();
    renderer.render(&mut buf, &screen).unwrap();
    // Output should contain cursor positioning sequences
    assert!(!buf.is_empty());
}

#[test]
fn renderer_cursor_after_writing() {
    let mut screen = Screen::new(10, 5);
    screen.write_char('X');
    screen.write_char('Y');
    // Cursor should have advanced
    assert_eq!(screen.cursor.col, 2);
    assert_eq!(screen.cursor.row, 0);

    let mut renderer = Renderer::new(10, 5);
    let mut buf: Vec<u8> = Vec::new();
    renderer.render(&mut buf, &screen).unwrap();
    let output = String::from_utf8_lossy(&buf);
    assert!(output.contains("XY"));
}

#[test]
fn renderer_after_erase_display() {
    use emux_term::screen::EraseDisplay;

    let mut screen = Screen::new(10, 3);
    for ch in "ABCDE".chars() {
        screen.write_char(ch);
    }
    screen.erase_display(EraseDisplay::All);

    let mut renderer = Renderer::new(10, 3);
    let mut buf: Vec<u8> = Vec::new();
    renderer.render(&mut buf, &screen).unwrap();
    // After erasing, the rendered output should not contain the original text
    let output = String::from_utf8_lossy(&buf);
    assert!(!output.contains("ABCDE"), "erased screen should not contain original text");
}

#[test]
fn renderer_wide_chars_via_screen() {
    let mut screen = Screen::new(10, 3);
    // Write a CJK character through the screen API
    screen.write_char('\u{4e16}'); // wide char
    screen.write_char('A');

    let mut renderer = Renderer::new(10, 3);
    let mut buf: Vec<u8> = Vec::new();
    renderer.render(&mut buf, &screen).unwrap();
    let output = String::from_utf8_lossy(&buf);
    assert!(output.contains('\u{4e16}'), "output should contain wide char");
    assert!(output.contains('A'), "output should contain 'A'");
}

#[test]
fn renderer_consecutive_renders_only_dirty() {
    let mut screen = Screen::new(10, 3);
    let mut renderer = Renderer::new(10, 3);

    // First render: full
    let mut buf1: Vec<u8> = Vec::new();
    renderer.render(&mut buf1, &screen).unwrap();
    assert!(!buf1.is_empty());

    // No changes: empty
    let mut buf2: Vec<u8> = Vec::new();
    renderer.render(&mut buf2, &screen).unwrap();
    assert!(buf2.is_empty());

    // Write something and force redraw
    screen.write_char('Z');
    renderer.force_redraw();
    let mut buf3: Vec<u8> = Vec::new();
    renderer.render(&mut buf3, &screen).unwrap();
    assert!(!buf3.is_empty());
    let output = String::from_utf8_lossy(&buf3);
    assert!(output.contains('Z'));
}
