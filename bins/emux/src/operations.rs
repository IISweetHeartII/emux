use emux_mux::SplitDirection;
use emux_pty::PtySize;

use crate::AppError;
use crate::app::{App, spawn_pane_state};

pub(crate) fn split_pane(app: &mut App, direction: SplitDirection) -> Result<(), AppError> {
    let tab = app.session.active_tab_mut();
    if let Some(new_id) = tab.split_pane(direction) {
        // Compute the size the layout assigned to this new pane.
        let positions = tab.compute_positions();
        let (pcols, prows) = positions
            .iter()
            .find(|(id, _)| *id == new_id)
            .map(|(_, p)| (p.cols, p.rows))
            .unwrap_or((80, 24));

        let ps = spawn_pane_state(pcols, prows)?;
        app.panes.insert(new_id, ps);

        // Also resize existing panes to match new layout.
        sync_pty_sizes(app);
    }
    Ok(())
}

pub(crate) fn close_active_pane(app: &mut App) {
    let tab = app.session.active_tab_mut();
    if let Some(active_id) = tab.active_pane_id()
        && tab.pane_count() > 1 {
            tab.close_pane(active_id);
            app.panes.remove(&active_id);
            sync_pty_sizes(app);
        }
}

pub(crate) fn new_tab(app: &mut App) -> Result<(), AppError> {
    let size = app.session.size();
    let _tab_id = app.session.new_tab(format!("Tab {}", app.session.tab_count()));

    // The new tab has pane 0 by default, but since PaneIds are per-tab and
    // we have a global HashMap, we need to know the actual id. The Tab always
    // starts with pane 0 as its first pane_id.
    let new_pane_id = app.session.active_tab().pane_ids()[0];
    let ps = spawn_pane_state(size.cols, size.rows)?;
    app.panes.insert(new_pane_id, ps);
    Ok(())
}

/// After a layout change, resize each pane's PTY/Screen to match its position.
pub(crate) fn sync_pty_sizes(app: &mut App) {
    let positions = app.session.active_tab().compute_positions();
    for (id, pos) in &positions {
        if let Some(ps) = app.panes.get_mut(id)
            && (ps.screen.cols() != pos.cols || ps.screen.rows() != pos.rows) {
                ps.screen.resize(pos.cols, pos.rows);
                ps.damage.resize(pos.rows);
                let _ = ps.pty.resize(PtySize {
                    rows: pos.rows as u16,
                    cols: pos.cols as u16,
                    pixel_width: 0,
                    pixel_height: 0,
                });
            }
    }
}
