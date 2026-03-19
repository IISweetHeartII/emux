//! Multiplexer: sessions, windows, panes, and layout management.

pub mod domain;
pub mod layout;
pub mod layout_template;
pub mod pane;
pub mod project;
pub mod search;
pub mod session;
pub mod swap_config;
pub mod tab;
pub mod window;

pub use domain::{Domain, DomainParseError};
pub use project::ProjectInfo;
pub use layout::{LayoutEngine, LayoutNode, PanePosition, SplitDirection};
pub use layout_template::{LayoutTemplate, PaneTemplate, SplitDir};
pub use pane::{Pane, PaneConstraints, PaneId, PaneSize};
pub use search::{GlobalSearchResult, SearchLineResult, search_lines, search_session, search_text};
pub use session::{Session, SessionId};
pub use swap_config::{parse_swap_layout_toml, LayoutParseError};
pub use tab::{FloatingPane, FocusDirection, ResizeDirection, SwapLayout, Tab, TabId};
pub use window::{Window, WindowId};
