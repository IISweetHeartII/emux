//! CSI (Control Sequence Introducer) parameter types.

/// Represents a parsed CSI parameter value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CsiParam(pub u16);

impl CsiParam {
    /// Return the raw parameter value.
    pub fn value(self) -> u16 {
        self.0
    }

    /// Get value with a default if zero (many CSI params treat 0 as default).
    pub fn value_or(self, default: u16) -> u16 {
        if self.0 == 0 { default } else { self.0 }
    }
}
