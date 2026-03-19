//! emux-vt: VT terminal escape sequence parser
//!
//! A standalone VT parser with zero dependencies on other emux crates.
//! Implements the Paul Williams VT state machine for parsing escape sequences.

mod charsets;
mod csi;
mod dcs;
mod osc;
mod params;
mod parser;

pub use charsets::Charset;
pub use csi::CsiParam;
pub use dcs::DcsAction;
pub use osc::OscAction;
pub use params::Params;
pub use parser::{Action, Intermediates, Parser, Performer};
