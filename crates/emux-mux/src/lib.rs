//! Multiplexer: sessions, windows, panes, and layout management.

pub mod domain;
pub mod layout;
pub mod pane;
pub mod session;
pub mod swap_config;
pub mod tab;
pub mod window;

pub use domain::{Domain, DomainParseError};
pub use layout::{LayoutEngine, LayoutNode, PanePosition, SplitDirection};
pub use pane::{Pane, PaneConstraints, PaneId, PaneSize};
pub use session::{Session, SessionId};
pub use swap_config::{parse_swap_layout_toml, LayoutParseError};
pub use tab::{FloatingPane, FocusDirection, ResizeDirection, SwapLayout, Tab, TabId};
pub use window::{Window, WindowId};
