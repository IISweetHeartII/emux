use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use emux_mux::{PaneId, PanePosition};
use emux_term::selection::SelectionPoint;

use crate::app::BorderDrag;
use crate::command::Command;

// ---------------------------------------------------------------------------
// Per-pane mouse tracking info (extracted from Screen for purity)
// ---------------------------------------------------------------------------

/// Minimal mouse-tracking info extracted from a pane's Screen.
pub(crate) struct PaneMouseInfo {
    pub pane_id: PaneId,
    pub mouse_tracking: emux_term::MouseMode,
    pub mouse_sgr: bool,
    pub scrollback_len: usize,
    pub viewport_offset: usize,
}

// ---------------------------------------------------------------------------
// Pure geometry helpers
// ---------------------------------------------------------------------------

/// Find which pane contains the given terminal coordinates.
pub(crate) fn pane_at_position(
    positions: &[(PaneId, PanePosition)],
    col: u16,
    row: u16,
) -> Option<PaneId> {
    let c = col as usize;
    let r = row as usize;
    for &(id, ref pos) in positions {
        if c >= pos.col && c < pos.col + pos.cols && r >= pos.row && r < pos.row + pos.rows {
            return Some(id);
        }
    }
    None
}

/// Detect if a mouse click is on a pane border. Returns a BorderDrag if so.
pub(crate) fn detect_border_click(
    positions: &[(PaneId, PanePosition)],
    col: u16,
    row: u16,
) -> Option<BorderDrag> {
    let c = col as usize;
    let r = row as usize;

    for &(id, ref pos) in positions {
        let right_edge = pos.col + pos.cols;
        if right_edge > 0 && c == right_edge - 1 && r >= pos.row && r < pos.row + pos.rows {
            let has_neighbor = positions
                .iter()
                .any(|(nid, np)| *nid != id && np.col == right_edge);
            if has_neighbor {
                return Some(BorderDrag {
                    pane_id: id,
                    vertical: true,
                    last_col: col,
                    last_row: row,
                });
            }
        }

        let bottom_edge = pos.row + pos.rows;
        if bottom_edge > 0 && r == bottom_edge - 1 && c >= pos.col && c < pos.col + pos.cols {
            let has_neighbor = positions
                .iter()
                .any(|(nid, np)| *nid != id && np.row == bottom_edge);
            if has_neighbor {
                return Some(BorderDrag {
                    pane_id: id,
                    vertical: false,
                    last_col: col,
                    last_row: row,
                });
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// resolve_mouse — pure function: MouseEvent → Vec<Command>
// ---------------------------------------------------------------------------

/// Context for mouse resolution.
pub(crate) struct MouseContext<'a> {
    pub positions: &'a [(PaneId, PanePosition)],
    pub active_pane_id: Option<PaneId>,
    pub border_drag: Option<&'a BorderDrag>,
    pub has_mouse_selection: bool,
    pub mouse_selection_pane: Option<PaneId>,
    pub pane_mouse_info: &'a [PaneMouseInfo],
}

/// Pure function: maps a mouse event + context to a list of Commands.
pub(crate) fn resolve_mouse(event: &MouseEvent, ctx: &MouseContext) -> Vec<Command> {
    let col = event.column;
    let row = event.row;
    let mut cmds = Vec::new();

    match event.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if let Some(drag) = detect_border_click(ctx.positions, col, row) {
                cmds.push(Command::StartBorderDrag(drag));
            } else if let Some(pane_id) = pane_at_position(ctx.positions, col, row) {
                // Focus pane if not already active.
                if ctx.active_pane_id != Some(pane_id) {
                    cmds.push(Command::FocusPane(pane_id));
                }

                let is_tracking = ctx
                    .pane_mouse_info
                    .iter()
                    .find(|p| p.pane_id == pane_id)
                    .map(|p| p.mouse_tracking != emux_term::MouseMode::None)
                    .unwrap_or(false);

                if !is_tracking {
                    // Start text selection.
                    if let Some((_, ppos)) = ctx.positions.iter().find(|(id, _)| *id == pane_id) {
                        if let Some(info) =
                            ctx.pane_mouse_info.iter().find(|p| p.pane_id == pane_id)
                        {
                            let local_col = (col as usize).saturating_sub(ppos.col);
                            let local_row = (row as usize).saturating_sub(ppos.row);
                            let abs_row = info.scrollback_len + local_row
                                - info.viewport_offset.min(info.scrollback_len + local_row);
                            let point = SelectionPoint::new(abs_row, local_col);
                            cmds.push(Command::StartMouseSelection { pane_id, point });
                        }
                    }
                } else {
                    push_forward_mouse(ctx, col, row, event, &mut cmds);
                }
            }
        }
        MouseEventKind::ScrollUp => {
            if let Some(pane_id) = pane_at_position(ctx.positions, col, row) {
                let is_tracking = ctx
                    .pane_mouse_info
                    .iter()
                    .find(|p| p.pane_id == pane_id)
                    .map(|p| p.mouse_tracking != emux_term::MouseMode::None)
                    .unwrap_or(false);

                if is_tracking {
                    push_forward_mouse(ctx, col, row, event, &mut cmds);
                } else {
                    cmds.push(Command::ScrollPaneUp { pane_id, amount: 3 });
                }
            }
        }
        MouseEventKind::ScrollDown => {
            if let Some(pane_id) = pane_at_position(ctx.positions, col, row) {
                let is_tracking = ctx
                    .pane_mouse_info
                    .iter()
                    .find(|p| p.pane_id == pane_id)
                    .map(|p| p.mouse_tracking != emux_term::MouseMode::None)
                    .unwrap_or(false);

                if is_tracking {
                    push_forward_mouse(ctx, col, row, event, &mut cmds);
                } else {
                    cmds.push(Command::ScrollPaneDown { pane_id, amount: 3 });
                }
            }
        }
        MouseEventKind::Down(MouseButton::Right) | MouseEventKind::Down(MouseButton::Middle) => {
            push_forward_mouse(ctx, col, row, event, &mut cmds);
        }
        MouseEventKind::Up(_) => {
            if ctx.border_drag.is_some() {
                cmds.push(Command::EndBorderDrag);
            } else if ctx.has_mouse_selection {
                cmds.push(Command::FinalizeMouseSelection);
            } else {
                push_forward_mouse(ctx, col, row, event, &mut cmds);
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if ctx.border_drag.is_some() {
                cmds.push(Command::ContinueBorderDrag { col, row });
            } else if ctx.has_mouse_selection {
                if let Some(sel_pane) = ctx.mouse_selection_pane {
                    if let Some((_, ppos)) = ctx.positions.iter().find(|(id, _)| *id == sel_pane) {
                        if let Some(info) =
                            ctx.pane_mouse_info.iter().find(|p| p.pane_id == sel_pane)
                        {
                            let local_col = (col as usize).saturating_sub(ppos.col);
                            let local_row = (row as usize).saturating_sub(ppos.row);
                            let abs_row = info.scrollback_len + local_row
                                - info.viewport_offset.min(info.scrollback_len + local_row);
                            let point = SelectionPoint::new(abs_row, local_col);
                            cmds.push(Command::ExtendMouseSelection(point));
                        }
                    }
                }
            } else {
                push_forward_mouse(ctx, col, row, event, &mut cmds);
            }
        }
        MouseEventKind::Drag(_) => {
            push_forward_mouse(ctx, col, row, event, &mut cmds);
        }
        MouseEventKind::Moved => {
            if let Some(pane_id) = pane_at_position(ctx.positions, col, row) {
                let is_any_event = ctx
                    .pane_mouse_info
                    .iter()
                    .find(|p| p.pane_id == pane_id)
                    .map(|p| p.mouse_tracking == emux_term::MouseMode::AnyEvent)
                    .unwrap_or(false);
                if is_any_event {
                    push_forward_mouse(ctx, col, row, event, &mut cmds);
                }
            }
        }
        _ => {}
    }

    cmds
}

// ---------------------------------------------------------------------------
// Helper: encode mouse event and push ForwardMouseToPty command
// ---------------------------------------------------------------------------

fn push_forward_mouse(
    ctx: &MouseContext,
    col: u16,
    row: u16,
    event: &MouseEvent,
    cmds: &mut Vec<Command>,
) {
    let Some(pane_id) = pane_at_position(ctx.positions, col, row) else {
        return;
    };
    let Some(info) = ctx.pane_mouse_info.iter().find(|p| p.pane_id == pane_id) else {
        return;
    };
    if info.mouse_tracking == emux_term::MouseMode::None {
        return;
    }
    let Some((_, ppos)) = ctx.positions.iter().find(|(id, _)| *id == pane_id) else {
        return;
    };
    let local_col = (col as usize).saturating_sub(ppos.col) as u16;
    let local_row = (row as usize).saturating_sub(ppos.row) as u16;

    let encoding = if info.mouse_sgr {
        emux_term::input::MouseEncoding::Sgr
    } else {
        emux_term::input::MouseEncoding::Normal
    };

    let mouse_ev = match event.kind {
        MouseEventKind::Down(button) => emux_term::input::MouseEvent::Press {
            button: crossterm_button_to_u8(button),
            col: local_col,
            row: local_row,
        },
        MouseEventKind::Up(_) => emux_term::input::MouseEvent::Release {
            col: local_col,
            row: local_row,
        },
        MouseEventKind::Drag(button) => emux_term::input::MouseEvent::Drag {
            button: crossterm_button_to_u8(button),
            col: local_col,
            row: local_row,
        },
        MouseEventKind::Moved => emux_term::input::MouseEvent::Drag {
            button: 3,
            col: local_col,
            row: local_row,
        },
        MouseEventKind::ScrollUp => emux_term::input::MouseEvent::ScrollUp {
            col: local_col,
            row: local_row,
        },
        MouseEventKind::ScrollDown => emux_term::input::MouseEvent::ScrollDown {
            col: local_col,
            row: local_row,
        },
        _ => return,
    };

    let bytes = emux_term::input::encode_mouse(mouse_ev, encoding);
    if !bytes.is_empty() {
        cmds.push(Command::ForwardMouseToPty { pane_id, bytes });
    }
}

fn crossterm_button_to_u8(button: MouseButton) -> u8 {
    match button {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use emux_mux::PanePosition;

    fn single_pane_positions() -> Vec<(PaneId, PanePosition)> {
        vec![(
            0,
            PanePosition {
                row: 0,
                col: 0,
                rows: 24,
                cols: 80,
            },
        )]
    }

    fn split_pane_positions() -> Vec<(PaneId, PanePosition)> {
        vec![
            (
                0,
                PanePosition {
                    row: 0,
                    col: 0,
                    rows: 24,
                    cols: 40,
                },
            ),
            (
                1,
                PanePosition {
                    row: 0,
                    col: 40,
                    rows: 24,
                    cols: 40,
                },
            ),
        ]
    }

    fn hsplit_pane_positions() -> Vec<(PaneId, PanePosition)> {
        vec![
            (
                0,
                PanePosition {
                    row: 0,
                    col: 0,
                    rows: 12,
                    cols: 80,
                },
            ),
            (
                1,
                PanePosition {
                    row: 12,
                    col: 0,
                    rows: 12,
                    cols: 80,
                },
            ),
        ]
    }

    // ── pane_at_position ────────────────────────────────────────────

    #[test]
    fn pane_at_position_single_pane() {
        let positions = single_pane_positions();
        assert_eq!(pane_at_position(&positions, 0, 0), Some(0));
        assert_eq!(pane_at_position(&positions, 40, 12), Some(0));
        assert_eq!(pane_at_position(&positions, 79, 23), Some(0));
    }

    #[test]
    fn pane_at_position_split_panes() {
        let positions = split_pane_positions();
        assert_eq!(pane_at_position(&positions, 0, 0), Some(0));
        assert_eq!(pane_at_position(&positions, 39, 10), Some(0));
        assert_eq!(pane_at_position(&positions, 40, 0), Some(1));
        assert_eq!(pane_at_position(&positions, 79, 23), Some(1));
    }

    #[test]
    fn pane_at_position_misses_status_bar() {
        let positions = single_pane_positions();
        // Row 24 is outside the pane (status bar area).
        assert_eq!(pane_at_position(&positions, 0, 24), None);
        // Col 80 is outside.
        assert_eq!(pane_at_position(&positions, 80, 0), None);
    }

    // ── detect_border_click ─────────────────────────────────────────

    #[test]
    fn detect_border_click_vertical() {
        let positions = split_pane_positions();
        // Col 39 is the last column of pane 0, and pane 1 starts at col 40.
        let drag = detect_border_click(&positions, 39, 12);
        assert!(drag.is_some());
        let drag = drag.unwrap();
        assert_eq!(drag.pane_id, 0);
        assert!(drag.vertical);
    }

    #[test]
    fn detect_border_click_horizontal() {
        let positions = hsplit_pane_positions();
        // Row 11 is the last row of pane 0, pane 1 starts at row 12.
        let drag = detect_border_click(&positions, 40, 11);
        assert!(drag.is_some());
        let drag = drag.unwrap();
        assert_eq!(drag.pane_id, 0);
        assert!(!drag.vertical);
    }

    #[test]
    fn detect_border_click_no_border() {
        let positions = split_pane_positions();
        // Col 20 is well inside pane 0.
        assert!(detect_border_click(&positions, 20, 12).is_none());
    }

    // ── resolve_mouse ───────────────────────────────────────────────

    fn mouse_ctx<'a>(
        positions: &'a [(PaneId, PanePosition)],
        pane_info: &'a [PaneMouseInfo],
    ) -> MouseContext<'a> {
        MouseContext {
            positions,
            active_pane_id: Some(0),
            border_drag: None,
            has_mouse_selection: false,
            mouse_selection_pane: None,
            pane_mouse_info: pane_info,
        }
    }

    fn no_tracking_info(pane_id: PaneId) -> PaneMouseInfo {
        PaneMouseInfo {
            pane_id,
            mouse_tracking: emux_term::MouseMode::None,
            mouse_sgr: false,
            scrollback_len: 0,
            viewport_offset: 0,
        }
    }

    #[test]
    fn resolve_click_focuses_other_pane() {
        let positions = split_pane_positions();
        let info = vec![no_tracking_info(0), no_tracking_info(1)];
        let ctx = mouse_ctx(&positions, &info);
        let event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 50,
            row: 10,
            modifiers: crossterm::event::KeyModifiers::empty(),
        };
        let cmds = resolve_mouse(&event, &ctx);
        assert!(cmds.iter().any(|c| matches!(c, Command::FocusPane(1))));
    }

    #[test]
    fn resolve_click_starts_selection() {
        let positions = single_pane_positions();
        let info = vec![no_tracking_info(0)];
        let ctx = mouse_ctx(&positions, &info);
        let event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 10,
            row: 5,
            modifiers: crossterm::event::KeyModifiers::empty(),
        };
        let cmds = resolve_mouse(&event, &ctx);
        assert!(
            cmds.iter()
                .any(|c| matches!(c, Command::StartMouseSelection { pane_id: 0, .. }))
        );
    }

    #[test]
    fn resolve_scroll_up_no_tracking() {
        let positions = single_pane_positions();
        let info = vec![no_tracking_info(0)];
        let ctx = mouse_ctx(&positions, &info);
        let event = MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 10,
            row: 5,
            modifiers: crossterm::event::KeyModifiers::empty(),
        };
        let cmds = resolve_mouse(&event, &ctx);
        assert!(cmds.iter().any(|c| matches!(
            c,
            Command::ScrollPaneUp {
                pane_id: 0,
                amount: 3
            }
        )));
    }

    #[test]
    fn resolve_mouse_up_finalizes_selection() {
        let positions = single_pane_positions();
        let info = vec![no_tracking_info(0)];
        let mut ctx = mouse_ctx(&positions, &info);
        ctx.has_mouse_selection = true;
        ctx.mouse_selection_pane = Some(0);
        let event = MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: 20,
            row: 8,
            modifiers: crossterm::event::KeyModifiers::empty(),
        };
        let cmds = resolve_mouse(&event, &ctx);
        assert!(
            cmds.iter()
                .any(|c| matches!(c, Command::FinalizeMouseSelection))
        );
    }

    #[test]
    fn resolve_border_drag_start() {
        let positions = split_pane_positions();
        let info = vec![no_tracking_info(0), no_tracking_info(1)];
        let ctx = mouse_ctx(&positions, &info);
        let event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 39,
            row: 12,
            modifiers: crossterm::event::KeyModifiers::empty(),
        };
        let cmds = resolve_mouse(&event, &ctx);
        assert!(
            cmds.iter()
                .any(|c| matches!(c, Command::StartBorderDrag(_)))
        );
    }

    #[test]
    fn resolve_drag_extends_selection() {
        let positions = single_pane_positions();
        let info = vec![no_tracking_info(0)];
        let mut ctx = mouse_ctx(&positions, &info);
        ctx.has_mouse_selection = true;
        ctx.mouse_selection_pane = Some(0);
        let event = MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: 30,
            row: 10,
            modifiers: crossterm::event::KeyModifiers::empty(),
        };
        let cmds = resolve_mouse(&event, &ctx);
        assert!(
            cmds.iter()
                .any(|c| matches!(c, Command::ExtendMouseSelection(_)))
        );
    }
}
