//! Rendering primitives: text output, cursor drawing, and damage tracking.

pub mod cursor;
pub mod damage;
pub mod statusbar;
pub mod text;

use std::io::{self, Write};

use crossterm::{QueueableCommand, cursor as ct_cursor, style};
use emux_term::Screen;

use crate::cursor::cursor_style;
use crate::damage::DamageTracker;
use crate::text::render_row;

/// Terminal renderer with damage tracking for efficient redraws.
pub struct Renderer {
    damage: DamageTracker,
    last_cols: usize,
    last_rows: usize,
}

impl Renderer {
    /// Create a new renderer for a terminal of the given size.
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            damage: DamageTracker::new(rows),
            last_cols: cols,
            last_rows: rows,
        }
    }

    /// Render the screen to the given writer, only updating dirty rows.
    pub fn render<W: Write>(&mut self, writer: &mut W, screen: &Screen) -> io::Result<()> {
        let cols = screen.cols();
        let rows = screen.rows();

        // Detect size changes
        if cols != self.last_cols || rows != self.last_rows {
            self.resize(cols, rows);
        }

        if !self.damage.needs_redraw() {
            return Ok(());
        }

        // Hide cursor during rendering
        writer.queue(ct_cursor::Hide)?;

        let dirty = self.damage.dirty_rows();
        for row in dirty {
            if row >= rows {
                continue;
            }

            // Move to start of this row
            writer.queue(ct_cursor::MoveTo(0, row as u16))?;

            // Get the row cells from the grid
            let grid_row = screen.grid.row(row);
            let spans = render_row(&grid_row.cells, cols);

            for (content_style, text) in spans {
                writer.queue(style::ResetColor)?;
                writer.queue(style::SetStyle(content_style))?;
                writer.queue(style::Print(&text))?;
            }
        }

        // Reset style
        writer.queue(style::ResetColor)?;
        writer.queue(style::SetAttribute(style::Attribute::Reset))?;

        // Position cursor
        let cursor = &screen.cursor;
        writer.queue(ct_cursor::MoveTo(cursor.col as u16, cursor.row as u16))?;

        // Show/hide cursor and set shape
        if cursor.visible {
            writer.queue(ct_cursor::Show)?;
            writer.queue(cursor_style(cursor.shape))?;
        } else {
            writer.queue(ct_cursor::Hide)?;
        }

        writer.flush()?;
        self.damage.clear();

        Ok(())
    }

    /// Force a full redraw on the next render call.
    pub fn force_redraw(&mut self) {
        self.damage.mark_all();
    }

    /// Resize the renderer to match new terminal dimensions.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.last_cols = cols;
        self.last_rows = rows;
        self.damage.resize(rows);
    }
}
