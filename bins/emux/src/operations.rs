use emux_pty::PtySize;

use crate::app::App;

/// After a layout change, resize each pane's PTY/Screen to match its position
/// and mark all panes dirty so borders and content are fully repainted.
pub(crate) fn sync_pty_sizes(app: &mut App) {
    let positions = app.session.active_tab().compute_positions();
    for (id, pos) in &positions {
        if let Some(ps) = app.panes.get_mut(id) {
            if ps.screen.cols() != pos.cols || ps.screen.rows() != pos.rows {
                ps.screen.resize(pos.cols, pos.rows);
                ps.damage.resize(pos.rows);
                if let Err(e) = ps.pty.resize(PtySize {
                    rows: pos.rows as u16,
                    cols: pos.cols as u16,
                    pixel_width: 0,
                    pixel_height: 0,
                }) {
                    emux_log!("PTY resize error: {}", e);
                }
            }
            ps.damage.mark_all();
        }
    }
}
