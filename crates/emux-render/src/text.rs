//! Text rendering: cell-to-terminal-output conversion.

use crossterm::style::{Attribute, Color as CtColor, ContentStyle};
use emux_term::grid::{Cell, UnderlineStyle};
use emux_term::Color;

/// Convert an `emux_term::Color` to a crossterm `Color`.
pub fn color_to_crossterm(color: &Color) -> CtColor {
    match color {
        Color::Default => CtColor::Reset,
        Color::Indexed(idx) => CtColor::AnsiValue(*idx),
        Color::Rgb(r, g, b) => CtColor::Rgb { r: *r, g: *g, b: *b },
    }
}

/// Build a `ContentStyle` from a cell's attributes and colors.
pub fn cell_style(cell: &Cell) -> ContentStyle {
    let mut style = ContentStyle::new();
    style.foreground_color = Some(color_to_crossterm(&cell.fg));
    style.background_color = Some(color_to_crossterm(&cell.bg));

    if cell.attrs.bold {
        style.attributes.set(Attribute::Bold);
    }
    if cell.attrs.italic {
        style.attributes.set(Attribute::Italic);
    }
    match cell.attrs.underline {
        UnderlineStyle::None => {}
        UnderlineStyle::Single => {
            style.attributes.set(Attribute::Underlined);
        }
        UnderlineStyle::Double => {
            style.attributes.set(Attribute::DoubleUnderlined);
        }
        UnderlineStyle::Curly => {
            style.attributes.set(Attribute::Undercurled);
        }
    }
    if cell.attrs.blink {
        style.attributes.set(Attribute::SlowBlink);
    }
    if cell.attrs.reverse {
        style.attributes.set(Attribute::Reverse);
    }
    if cell.attrs.invisible {
        style.attributes.set(Attribute::Hidden);
    }
    if cell.attrs.strikethrough {
        style.attributes.set(Attribute::CrossedOut);
    }

    style
}

/// Convert a row of cells into a sequence of styled text spans.
///
/// Adjacent cells with the same style are coalesced into a single span.
/// Wide-char continuation cells (width == 0) are skipped.  The output
/// is padded with spaces to exactly `width` columns.
pub fn render_row(cells: &[Cell], width: usize) -> Vec<(ContentStyle, String)> {
    let mut spans: Vec<(ContentStyle, String)> = Vec::new();
    let mut col = 0;

    for cell in cells.iter().take(width) {
        // Skip continuation cells for wide characters
        if cell.width == 0 {
            col += 1;
            continue;
        }

        let style = cell_style(cell);
        let ch = if cell.c < ' ' { ' ' } else { cell.c };

        if let Some(last) = spans.last_mut() {
            if last.0 == style {
                last.1.push(ch);
            } else {
                spans.push((style, ch.to_string()));
            }
        } else {
            spans.push((style, ch.to_string()));
        }

        col += cell.width as usize;
    }

    // Pad to the full width if needed
    while col < width {
        let style = ContentStyle::new();
        if let Some(last) = spans.last_mut() {
            if last.0 == style {
                last.1.push(' ');
            } else {
                spans.push((style, " ".to_string()));
            }
        } else {
            spans.push((style, " ".to_string()));
        }
        col += 1;
    }

    spans
}
