//! DCS (Device Control String) types.

/// Actions from parsed DCS sequences.
#[derive(Debug, Clone, PartialEq)]
pub enum DcsAction {
    /// Sixel image data.
    Sixel(Vec<u8>),
    /// XTGETTCAP terminal capability query.
    XtGetTcap(Vec<String>),
    /// DECRQSS (Request Status String).
    Decrqss(Vec<u8>),
    /// Unknown DCS.
    Unknown(Vec<u8>),
}
