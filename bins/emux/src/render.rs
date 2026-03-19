use std::io::{self, Write};

use crossterm::{QueueableCommand, cursor as ct_cursor, style, terminal};
use emux_mux::{PaneId, PanePosition};
use emux_render::damage::DamageTracker;
use emux_render::text::render_row;
use emux_term::Screen;

use crate::app::{App, InputMode};

// ---------------------------------------------------------------------------
// Rendering — draws ALL visible panes with borders + status bar
// ---------------------------------------------------------------------------

pub(crate) fn render_all<W: Write>(
    writer: &mut W,
    app: &mut App,
    total_cols: u16,
    total_rows: u16,
    force_clear: bool,
) -> io::Result<()> {
    let tc = total_cols as usize;
    let tr = total_rows as usize;
    let pane_area_rows = tr.saturating_sub(1); // last row is status bar

    // When force_clear is set (resize, initial draw) we must repaint
    // everything. Mark all pane damage trackers accordingly.
    if force_clear {
        for ps in app.panes.values_mut() {
            ps.damage.mark_all();
        }
    }

    // Check if any pane actually needs a redraw. If nothing is dirty and we
    // are not forced, skip the entire render pass.
    let any_dirty = force_clear || app.panes.values().any(|ps| ps.damage.needs_redraw());
    if !any_dirty {
        return Ok(());
    }

    writer.queue(ct_cursor::Hide)?;
    if force_clear {
        writer.queue(terminal::Clear(terminal::ClearType::All))?;
    }

    let positions = app.session.active_tab().compute_positions();
    let active_id = app.session.active_tab().active_pane_id();

    for &(pane_id, ref pos) in &positions {
        if let Some(ps) = app.panes.get(&pane_id) {
            render_pane_region(
                writer,
                &ps.screen,
                &ps.damage,
                pos,
                tc,
                pane_area_rows,
                force_clear,
            )?;
        }
    }

    // Draw borders between panes (if more than one pane).
    if positions.len() > 1 {
        draw_borders(writer, &positions, active_id, tc, pane_area_rows)?;
    }

    // Draw status bar on the last row (or search bar if in search mode).
    if app.input_mode == InputMode::Search {
        draw_search_bar(writer, app, total_cols, total_rows)?;
    } else {
        draw_status_bar(writer, app, total_cols, total_rows)?;
    }

    // Position cursor at the active pane's cursor location.
    if let Some(aid) = active_id
        && let Some(ps) = app.panes.get(&aid)
        && let Some((_, apos)) = positions.iter().find(|(id, _)| *id == aid)
    {
        let cx = apos.col as u16 + ps.screen.cursor.col as u16;
        let cy = apos.row as u16 + ps.screen.cursor.row as u16;
        writer.queue(ct_cursor::MoveTo(cx, cy))?;
        if ps.screen.cursor.visible {
            writer.queue(ct_cursor::Show)?;
        }
    }

    writer.flush()?;

    // Clear damage flags now that everything dirty has been repainted.
    for ps in app.panes.values_mut() {
        ps.damage.clear();
    }

    Ok(())
}

/// Render one pane's screen content into a region of the terminal.
/// Only rows marked dirty in the `DamageTracker` are redrawn unless
/// `force_all` is set (initial paint / resize).
pub(crate) fn render_pane_region<W: Write>(
    writer: &mut W,
    screen: &Screen,
    damage: &DamageTracker,
    pos: &PanePosition,
    _total_cols: usize,
    _pane_area_rows: usize,
    force_all: bool,
) -> io::Result<()> {
    let draw_rows = pos.rows.min(screen.rows());
    let draw_cols = pos.cols.min(screen.cols());
    let mut prev_style: Option<style::ContentStyle> = None;

    for r in 0..draw_rows {
        // Skip clean rows when not doing a full repaint.
        if !force_all && !damage.is_dirty(r) {
            continue;
        }
        writer.queue(ct_cursor::MoveTo(pos.col as u16, (pos.row + r) as u16))?;
        let grid_row = screen.grid.row(r);
        let spans = render_row(&grid_row.cells, draw_cols);
        for (content_style, text) in spans {
            if prev_style.as_ref() != Some(&content_style) {
                writer.queue(style::SetStyle(content_style))?;
                prev_style = Some(content_style);
            }
            writer.queue(style::Print(&text))?;
        }
    }

    // Reset style after drawing the pane.
    writer.queue(style::ResetColor)?;
    writer.queue(style::SetAttribute(style::Attribute::Reset))?;

    Ok(())
}

/// Draw separator borders between panes. Active pane border is highlighted.
pub(crate) fn draw_borders<W: Write>(
    writer: &mut W,
    positions: &[(PaneId, PanePosition)],
    active_id: Option<PaneId>,
    total_cols: usize,
    pane_area_rows: usize,
) -> io::Result<()> {
    // We look for vertical boundaries (right edge of a pane where another pane
    // starts) and horizontal boundaries (bottom edge).
    // For simplicity we draw on the *last column/row* of a pane, overwriting the
    // content there with a border character. This is the simplest approach that
    // doesn't require subtracting border space from the layout.

    // Collect vertical edges: where one pane's right edge == another pane's left edge.
    for &(id_a, ref pa) in positions {
        let right_edge = pa.col + pa.cols;
        if right_edge >= total_cols {
            continue;
        }
        // Check if there is a pane starting at right_edge in the same row range.
        for &(id_b, ref pb) in positions {
            if id_a == id_b {
                continue;
            }
            if pb.col == right_edge {
                // Draw vertical border line.
                let row_start = pa.row.max(pb.row);
                let row_end = (pa.row + pa.rows).min(pb.row + pb.rows).min(pane_area_rows);
                let is_active_border = active_id == Some(id_a) || active_id == Some(id_b);

                if is_active_border {
                    writer.queue(style::SetForegroundColor(style::Color::Cyan))?;
                } else {
                    writer.queue(style::SetForegroundColor(style::Color::DarkGrey))?;
                }
                for row in row_start..row_end {
                    writer.queue(ct_cursor::MoveTo(
                        right_edge.saturating_sub(1) as u16,
                        row as u16,
                    ))?;
                    writer.queue(style::Print("\u{2502}"))?; // │
                }
                writer.queue(style::ResetColor)?;
            }
        }
    }

    // Collect horizontal edges.
    for &(id_a, ref pa) in positions {
        let bottom_edge = pa.row + pa.rows;
        if bottom_edge >= pane_area_rows {
            continue;
        }
        for &(id_b, ref pb) in positions {
            if id_a == id_b {
                continue;
            }
            if pb.row == bottom_edge {
                let col_start = pa.col.max(pb.col);
                let col_end = (pa.col + pa.cols).min(pb.col + pb.cols).min(total_cols);
                let is_active_border = active_id == Some(id_a) || active_id == Some(id_b);

                if is_active_border {
                    writer.queue(style::SetForegroundColor(style::Color::Cyan))?;
                } else {
                    writer.queue(style::SetForegroundColor(style::Color::DarkGrey))?;
                }
                writer.queue(ct_cursor::MoveTo(
                    col_start as u16,
                    bottom_edge.saturating_sub(1) as u16,
                ))?;
                let line: String = "\u{2500}".repeat(col_end - col_start); // ─
                writer.queue(style::Print(&line))?;
                writer.queue(style::ResetColor)?;
            }
        }
    }

    Ok(())
}

/// Draw a status bar at the bottom of the terminal.
pub(crate) fn draw_status_bar<W: Write>(
    writer: &mut W,
    app: &App,
    total_cols: u16,
    total_rows: u16,
) -> io::Result<()> {
    let bar_row = total_rows.saturating_sub(1);
    writer.queue(ct_cursor::MoveTo(0, bar_row))?;
    writer.queue(style::SetForegroundColor(style::Color::Black))?;
    writer.queue(style::SetBackgroundColor(style::Color::White))?;

    let session_name = app.session.name();
    let tab_count = app.session.tab_count();
    let active_idx = app.session.active_tab_index();

    let mut left = format!(" [{}] ", session_name);
    for i in 0..tab_count {
        let name = app.session.tab(i).map(|t| t.name()).unwrap_or("?");
        if i == active_idx {
            left.push_str(&format!("{}* ", name));
        } else {
            left.push_str(&format!("{} ", name));
        }
        if i + 1 < tab_count {
            left.push_str("| ");
        }
    }

    let pane_count = app.panes.len();
    let right = format!(
        "{} pane{} | emux 0.1.0 ",
        pane_count,
        if pane_count == 1 { "" } else { "s" }
    );

    // Pad to full width with right-aligned info.
    let tc = total_cols as usize;
    let bar = if left.len() + right.len() <= tc {
        let padding = tc - left.len() - right.len();
        format!("{}{}{}", left, " ".repeat(padding), right)
    } else if left.len() < tc {
        let mut s = left;
        s.push_str(&" ".repeat(tc - s.len()));
        s
    } else {
        let mut s = left;
        s.truncate(tc);
        s
    };

    writer.queue(style::Print(&bar))?;
    writer.queue(style::ResetColor)?;

    Ok(())
}

/// Draw a search prompt bar at the bottom of the terminal (replaces status bar).
pub(crate) fn draw_search_bar<W: Write>(
    writer: &mut W,
    app: &App,
    total_cols: u16,
    total_rows: u16,
) -> io::Result<()> {
    let bar_row = total_rows.saturating_sub(1);
    writer.queue(ct_cursor::MoveTo(0, bar_row))?;
    writer.queue(style::SetForegroundColor(style::Color::Black))?;
    writer.queue(style::SetBackgroundColor(style::Color::Yellow))?;

    let tc = total_cols as usize;
    let match_count = app.search_state.matches.len();
    let current = app.search_state.current.map(|i| i + 1).unwrap_or(0);

    let left = format!(" /{}_ ", app.search_query);
    let right = if app.search_query.is_empty() {
        " type to search, Esc to cancel ".to_string()
    } else {
        format!(" {}/{} matches ", current, match_count)
    };

    let bar = if left.len() + right.len() <= tc {
        let padding = tc - left.len() - right.len();
        format!("{}{}{}", left, " ".repeat(padding), right)
    } else if left.len() < tc {
        let mut s = left;
        s.push_str(&" ".repeat(tc - s.len()));
        s
    } else {
        let mut s = left;
        s.truncate(tc);
        s
    };

    writer.queue(style::Print(&bar))?;
    writer.queue(style::ResetColor)?;

    Ok(())
}
