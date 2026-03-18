//! Key binding definitions and mapping.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBindings {
    pub split_down: String,
    pub split_right: String,
    pub close_pane: String,
    pub focus_up: String,
    pub focus_down: String,
    pub focus_left: String,
    pub focus_right: String,
    pub new_tab: String,
    pub close_tab: String,
    pub next_tab: String,
    pub prev_tab: String,
    pub detach: String,
    pub search: String,
    pub toggle_fullscreen: String,
    pub toggle_float: String,
    pub copy_mode: String,
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
        }
    }
}
