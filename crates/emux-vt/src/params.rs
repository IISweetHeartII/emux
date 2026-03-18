//! CSI/DCS parameter accumulation and parsing.

/// Maximum number of parameters in a CSI/DCS sequence.
const MAX_PARAMS: usize = 32;

/// Accumulated parameters from a CSI or DCS sequence.
#[derive(Debug, Clone, PartialEq)]
pub struct Params {
    values: Vec<u16>,
    current: u32,
    has_current: bool,
    trailing_sep: bool,
    /// Tracks which separators were colons (subparam) vs semicolons.
    /// `subparam_flags[i]` is true if parameter `i+1` was separated from
    /// parameter `i` by a colon (':') rather than a semicolon (';').
    subparam_flags: Vec<bool>,
    /// Whether the next pushed value is a subparam (colon-separated).
    pending_colon: bool,
}

impl Params {
    pub fn new() -> Self {
        Self {
            values: Vec::with_capacity(8),
            current: 0,
            has_current: false,
            trailing_sep: false,
            subparam_flags: Vec::with_capacity(8),
            pending_colon: false,
        }
    }

    pub fn clear(&mut self) {
        self.values.clear();
        self.current = 0;
        self.has_current = false;
        self.trailing_sep = false;
        self.subparam_flags.clear();
        self.pending_colon = false;
    }

    pub fn push(&mut self, byte: u8) {
        match byte {
            b'0'..=b'9' => {
                self.has_current = true;
                self.trailing_sep = false;
                self.current = self.current.saturating_mul(10).saturating_add((byte - b'0') as u32);
            }
            b';' => {
                if self.values.len() < MAX_PARAMS {
                    let val = if self.has_current {
                        self.current.min(u16::MAX as u32) as u16
                    } else {
                        0
                    };
                    self.values.push(val);
                    self.subparam_flags.push(self.pending_colon);
                }
                self.current = 0;
                self.has_current = false;
                self.trailing_sep = true;
                self.pending_colon = false;
            }
            b':' => {
                // Subparameter separator
                if self.values.len() < MAX_PARAMS {
                    let val = if self.has_current {
                        self.current.min(u16::MAX as u32) as u16
                    } else {
                        0
                    };
                    self.values.push(val);
                    self.subparam_flags.push(self.pending_colon);
                }
                self.current = 0;
                self.has_current = false;
                self.pending_colon = true;
            }
            _ => {}
        }
    }

    /// Finalize and return all parameter values.
    pub fn finished(&self) -> Vec<u16> {
        let mut result = self.values.clone();
        if self.has_current && result.len() < MAX_PARAMS {
            result.push(self.current.min(u16::MAX as u32) as u16);
        } else if self.trailing_sep && !self.has_current && result.len() < MAX_PARAMS {
            result.push(0);
        }
        result
    }

    /// Get parameter at index with a default value.
    pub fn get(&self, index: usize, default: u16) -> u16 {
        let finished = self.finished();
        finished.get(index).copied().unwrap_or(default)
    }

    /// Get parameter treating 0 as default (standard CSI behavior).
    pub fn get_or(&self, index: usize, default: u16) -> u16 {
        let val = self.get(index, 0);
        if val == 0 { default } else { val }
    }

    /// Number of parameters (including the pending one).
    pub fn len(&self) -> usize {
        self.finished().len()
    }

    /// Return finished subparam flags. `flags[i]` is true if parameter `i` was
    /// separated from the previous parameter by a colon (`:`) rather than a
    /// semicolon (`;`). The first parameter always has flag `false`.
    pub fn finished_subparam_flags(&self) -> Vec<bool> {
        let mut result = self.subparam_flags.clone();
        // The pending value needs a flag too
        if self.has_current && result.len() < MAX_PARAMS {
            result.push(self.pending_colon);
        } else if self.trailing_sep && !self.has_current && result.len() < MAX_PARAMS {
            result.push(false);
        }
        result
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for Params {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_params() {
        let p = Params::new();
        assert!(p.is_empty());
        assert_eq!(p.get(0, 1), 1);
    }

    #[test]
    fn single_param() {
        let mut p = Params::new();
        for b in b"42" {
            p.push(*b);
        }
        assert_eq!(p.get(0, 0), 42);
    }

    #[test]
    fn multiple_params() {
        let mut p = Params::new();
        for b in b"1;2;3" {
            p.push(*b);
        }
        assert_eq!(p.finished(), vec![1, 2, 3]);
    }

    #[test]
    fn empty_param_defaults_to_zero() {
        let mut p = Params::new();
        for b in b";2;" {
            p.push(*b);
        }
        assert_eq!(p.finished(), vec![0, 2, 0]);
    }

    #[test]
    fn overflow_clamped() {
        let mut p = Params::new();
        for b in b"999999999" {
            p.push(*b);
        }
        assert_eq!(p.get(0, 0), u16::MAX);
    }

    #[test]
    fn max_params_limit() {
        let mut p = Params::new();
        let input = "0;".repeat(40);
        for b in input.as_bytes() {
            p.push(*b);
        }
        assert!(p.len() <= 32);
    }
}
