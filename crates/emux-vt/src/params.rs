//! CSI/DCS parameter accumulation and parsing.

/// Maximum number of parameters in a CSI/DCS sequence.
const MAX_PARAMS: usize = 32;

/// Accumulated parameters from a CSI or DCS sequence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Params {
    values: [u16; MAX_PARAMS],
    /// Tracks which separators were colons (subparam) vs semicolons.
    /// `subparam_flags[i]` is true if parameter `i+1` was separated from
    /// parameter `i` by a colon (':') rather than a semicolon (';').
    subparam_flags: [bool; MAX_PARAMS],
    len: u8,
    current: u32,
    has_current: bool,
    trailing_sep: bool,
    /// Whether the next pushed value is a subparam (colon-separated).
    pending_colon: bool,
}

impl Params {
    /// Create a new empty parameter accumulator.
    #[inline]
    pub fn new() -> Self {
        Self {
            values: [0; MAX_PARAMS],
            subparam_flags: [false; MAX_PARAMS],
            len: 0,
            current: 0,
            has_current: false,
            trailing_sep: false,
            pending_colon: false,
        }
    }

    /// Reset all accumulated parameters.
    #[inline]
    pub fn clear(&mut self) {
        self.len = 0;
        self.current = 0;
        self.has_current = false;
        self.trailing_sep = false;
        self.pending_colon = false;
    }

    /// Feed a single byte (digit or separator) into the accumulator.
    #[inline]
    pub fn push(&mut self, byte: u8) {
        match byte {
            b'0'..=b'9' => {
                self.has_current = true;
                self.trailing_sep = false;
                self.current = self
                    .current
                    .saturating_mul(10)
                    .saturating_add((byte - b'0') as u32);
            }
            b';' => {
                if (self.len as usize) < MAX_PARAMS {
                    let val = if self.has_current {
                        self.current.min(u16::MAX as u32) as u16
                    } else {
                        0
                    };
                    let idx = self.len as usize;
                    self.values[idx] = val;
                    self.subparam_flags[idx] = self.pending_colon;
                    self.len += 1;
                }
                self.current = 0;
                self.has_current = false;
                self.trailing_sep = true;
                self.pending_colon = false;
            }
            b':' => {
                // Subparameter separator
                if (self.len as usize) < MAX_PARAMS {
                    let val = if self.has_current {
                        self.current.min(u16::MAX as u32) as u16
                    } else {
                        0
                    };
                    let idx = self.len as usize;
                    self.values[idx] = val;
                    self.subparam_flags[idx] = self.pending_colon;
                    self.len += 1;
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
        let mut result = self.values[..self.len as usize].to_vec();
        if (self.len as usize) < MAX_PARAMS {
            if self.has_current {
                result.push(self.current.min(u16::MAX as u32) as u16);
            } else if self.trailing_sep {
                result.push(0);
            }
        }
        result
    }

    /// Get parameter at index with a default value.
    #[inline]
    pub fn get(&self, index: usize, default: u16) -> u16 {
        let total = self.finished_len();
        if index >= total {
            return default;
        }
        if index < self.len as usize {
            self.values[index]
        } else {
            // It's the pending value
            if self.has_current {
                self.current.min(u16::MAX as u32) as u16
            } else {
                0 // trailing separator default
            }
        }
    }

    /// Get parameter treating 0 as default (standard CSI behavior).
    #[inline]
    pub fn get_or(&self, index: usize, default: u16) -> u16 {
        let val = self.get(index, 0);
        if val == 0 { default } else { val }
    }

    /// Number of parameters (including the pending one).
    #[inline]
    pub fn len(&self) -> usize {
        self.finished_len()
    }

    /// Compute the finished length without allocating.
    #[inline]
    fn finished_len(&self) -> usize {
        let base = self.len as usize;
        if base < MAX_PARAMS && (self.has_current || self.trailing_sep) {
            base + 1
        } else {
            base
        }
    }

    /// Return finished subparam flags. `flags[i]` is true if parameter `i` was
    /// separated from the previous parameter by a colon (`:`) rather than a
    /// semicolon (`;`). The first parameter always has flag `false`.
    pub fn finished_subparam_flags(&self) -> Vec<bool> {
        let mut result = self.subparam_flags[..self.len as usize].to_vec();
        // The pending value needs a flag too
        if self.has_current && result.len() < MAX_PARAMS {
            result.push(self.pending_colon);
        } else if self.trailing_sep && !self.has_current && result.len() < MAX_PARAMS {
            result.push(false);
        }
        result
    }

    /// Returns true if no parameters have been accumulated.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.finished_len() == 0
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
