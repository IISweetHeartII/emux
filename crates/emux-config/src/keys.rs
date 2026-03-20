//! Key binding definitions and mapping.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBindings {
    /// Key binding to split the current pane downward.
    pub split_down: String,
    /// Key binding to split the current pane to the right.
    pub split_right: String,
    /// Key binding to close the focused pane.
    pub close_pane: String,
    /// Key binding to move focus to the pane above.
    pub focus_up: String,
    /// Key binding to move focus to the pane below.
    pub focus_down: String,
    /// Key binding to move focus to the pane on the left.
    pub focus_left: String,
    /// Key binding to move focus to the pane on the right.
    pub focus_right: String,
    /// Key binding to open a new tab.
    pub new_tab: String,
    /// Key binding to close the current tab.
    pub close_tab: String,
    /// Key binding to switch to the next tab.
    pub next_tab: String,
    /// Key binding to switch to the previous tab.
    pub prev_tab: String,
    /// Key binding to detach from the session.
    pub detach: String,
    /// Key binding to open the search prompt.
    pub search: String,
    /// Key binding to toggle fullscreen for the focused pane.
    pub toggle_fullscreen: String,
    /// Key binding to toggle floating mode for the focused pane.
    pub toggle_float: String,
    /// Key binding to enter copy mode.
    pub copy_mode: String,
    /// Key binding to scroll viewport up (view scrollback history).
    pub scroll_up: String,
    /// Key binding to scroll viewport down (back toward live output).
    pub scroll_down: String,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            split_down: "Leader+D".into(),
            split_right: "Leader+R".into(),
            close_pane: "Leader+X".into(),
            focus_up: "Leader+Up".into(),
            focus_down: "Leader+Down".into(),
            focus_left: "Leader+Left".into(),
            focus_right: "Leader+Right".into(),
            new_tab: "Leader+T".into(),
            close_tab: "Leader+W".into(),
            next_tab: "Leader+N".into(),
            prev_tab: "Leader+P".into(),
            detach: "Leader+Q".into(),
            search: "Leader+/".into(),
            toggle_fullscreen: "Leader+F".into(),
            toggle_float: "Leader+G".into(),
            copy_mode: "Leader+[".into(),
            scroll_up: "Leader+PageUp".into(),
            scroll_down: "Leader+PageDown".into(),
        }
    }
}
