//! emux-vt: VT terminal escape sequence parser
//!
//! A standalone VT parser with zero dependencies on other emux crates.
//! Implements the Paul Williams VT state machine for parsing escape sequences.

mod parser;
mod params;
mod csi;
mod osc;
mod dcs;
mod charsets;

pub use parser::{Parser, Action, Performer, Intermediates};
pub use params::Params;
pub use csi::CsiParam;
pub use osc::OscAction;
pub use dcs::DcsAction;
pub use charsets::Charset;
