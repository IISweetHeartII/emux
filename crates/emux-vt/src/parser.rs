//! VT state machine parser.
//!
//! Based on the Paul Williams VT parser state machine.
//! Reference: <https://vt100.net/emu/dec_ansi_parser>

use crate::params::Params;

const MAX_INTERMEDIATES: usize = 4;
const MAX_OSC_DATA: usize = 65536;

/// Fixed-size intermediate byte buffer (stack-allocated, Copy).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Intermediates {
    data: [u8; MAX_INTERMEDIATES],
    len: u8,
}

impl Default for Intermediates {
    fn default() -> Self {
        Self::new()
    }
}

impl Intermediates {
    /// Create a new empty intermediates buffer.
    #[inline]
    pub const fn new() -> Self {
        Self {
            data: [0; MAX_INTERMEDIATES],
            len: 0,
        }
    }

    /// Clear the buffer.
    #[inline]
    fn clear(&mut self) {
        self.len = 0;
    }

    /// Push a byte if there is room.
    #[inline]
    pub fn push(&mut self, byte: u8) {
        if (self.len as usize) < MAX_INTERMEDIATES {
            self.data[self.len as usize] = byte;
            self.len += 1;
        }
    }

    /// Return the contents as a slice.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.data[..self.len as usize]
    }

    /// Return the length.
    #[inline]
    pub fn len(&self) -> usize {
        self.len as usize
    }

    /// Check if empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Check if the buffer contains a specific byte.
    #[inline]
    pub fn contains(&self, byte: &u8) -> bool {
        let s = self.as_slice();
        s.contains(byte)
    }

    /// Get the first element.
    #[inline]
    pub fn first(&self) -> Option<&u8> {
        if self.len > 0 {
            Some(&self.data[0])
        } else {
            None
        }
    }
}

/// Actions emitted by the parser.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Regular printable character.
    Print(char),
    /// C0 or C1 control character.
    Execute(u8),
    /// CSI sequence dispatched.
    CsiDispatch {
        params: Params,
        intermediates: Intermediates,
        ignore: bool,
        action: u8,
    },
    /// ESC sequence dispatched.
    EscDispatch {
        intermediates: Intermediates,
        ignore: bool,
        byte: u8,
    },
    /// OSC sequence (terminated by BEL or ST).
    OscDispatch(Vec<Vec<u8>>),
    /// DCS sequence hook.
    DcsHook {
        params: Params,
        intermediates: Intermediates,
        ignore: bool,
        byte: u8,
    },
    /// DCS data byte.
    DcsPut(u8),
    /// DCS sequence unhook.
    DcsUnhook,
    /// APC sequence data.
    ApcDispatch(Vec<u8>),
}

/// Trait for receiving parsed actions.
pub trait Performer {
    fn perform(&mut self, action: Action);
}

/// VT parser states.
#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    Ground,
    Escape,
    EscapeIntermediate,
    CsiEntry,
    CsiParam,
    CsiIntermediate,
    CsiIgnore,
    OscString,
    DcsEntry,
    DcsParam,
    DcsIntermediate,
    DcsPassthrough,
    DcsIgnore,
    SosPmApcString,
    Utf8,
}

/// The VT escape sequence parser.
pub struct Parser {
    state: State,
    params: Params,
    intermediates: Intermediates,
    osc_data: Vec<u8>,
    dcs_data: Vec<u8>,
    utf8_buf: [u8; 4],
    utf8_idx: u8,
    utf8_len: u8,
}

impl Parser {
    /// Create a new parser in the ground state.
    pub fn new() -> Self {
        Self {
            state: State::Ground,
            params: Params::new(),
            intermediates: Intermediates::new(),
            osc_data: Vec::new(),
            dcs_data: Vec::new(),
            utf8_buf: [0; 4],
            utf8_idx: 0,
            utf8_len: 0,
        }
    }

    /// Feed a slice of bytes to the parser.
    pub fn advance<P: Performer>(&mut self, performer: &mut P, bytes: &[u8]) {
        let mut i = 0;
        let len = bytes.len();
        while i < len {
            // ASCII fast path: when in Ground state, scan for runs of printable
            // ASCII (0x20..=0x7e) and emit them without per-byte overhead.
            if self.state == State::Ground {
                let start = i;
                while i < len {
                    let b = bytes[i];
                    if (0x20..=0x7e).contains(&b) {
                        i += 1;
                    } else {
                        break;
                    }
                }
                // Emit all printable ASCII chars we found
                for &b in &bytes[start..i] {
                    performer.perform(Action::Print(b as char));
                }
                if i >= len {
                    break;
                }
            }
            // Process the current (non-printable-ASCII or non-Ground) byte
            self.advance_byte(performer, bytes[i]);
            i += 1;
        }
    }

    #[inline(always)]
    fn advance_byte<P: Performer>(&mut self, performer: &mut P, byte: u8) {
        // Handle UTF-8 continuation
        if self.state == State::Utf8 {
            self.utf8_buf[self.utf8_idx as usize] = byte;
            self.utf8_idx += 1;
            if self.utf8_idx == self.utf8_len {
                if let Ok(s) = std::str::from_utf8(&self.utf8_buf[..self.utf8_len as usize])
                    && let Some(c) = s.chars().next()
                {
                    performer.perform(Action::Print(c));
                }
                self.state = State::Ground;
            }
            return;
        }

        // Anywhere transitions (CAN, SUB, ESC)
        match byte {
            0x18 | 0x1a => {
                // CAN or SUB: cancel current sequence
                self.state = State::Ground;
                performer.perform(Action::Execute(byte));
                return;
            }
            0x1b => {
                // ESC: start escape sequence
                self.state = State::Escape;
                self.intermediates.clear();
                return;
            }
            _ => {}
        }

        match self.state {
            State::Ground => self.ground(performer, byte),
            State::Escape => self.escape(performer, byte),
            State::EscapeIntermediate => self.escape_intermediate(performer, byte),
            State::CsiEntry => self.csi_entry(performer, byte),
            State::CsiParam => self.csi_param(performer, byte),
            State::CsiIntermediate => self.csi_intermediate(performer, byte),
            State::CsiIgnore => self.csi_ignore(performer, byte),
            State::OscString => self.osc_string(performer, byte),
            State::DcsEntry => self.dcs_entry(performer, byte),
            State::DcsParam => self.dcs_param(performer, byte),
            State::DcsIntermediate => self.dcs_intermediate(performer, byte),
            State::DcsPassthrough => self.dcs_passthrough(performer, byte),
            State::DcsIgnore => self.dcs_ignore(performer, byte),
            State::SosPmApcString => self.sos_pm_apc_string(performer, byte),
            State::Utf8 => unreachable!(),
        }
    }

    #[inline(always)]
    fn ground<P: Performer>(&mut self, performer: &mut P, byte: u8) {
        match byte {
            0x00..=0x1f => performer.perform(Action::Execute(byte)),
            0x20..=0x7e => performer.perform(Action::Print(byte as char)),
            // DEL - ignore
            // ST - ignore in ground
            0x80..=0x8f | 0x91..=0x97 | 0x99 | 0x9a => {
                performer.perform(Action::Execute(byte));
            }
            0x90 => {
                // DCS
                self.state = State::DcsEntry;
                self.params.clear();
                self.intermediates.clear();
            }
            0x98 | 0x9e | 0x9f => {
                // SOS, PM, APC
                self.state = State::SosPmApcString;
            }
            0x9b => {
                // CSI
                self.state = State::CsiEntry;
                self.params.clear();
                self.intermediates.clear();
            }
            0x9d => {
                // OSC
                self.state = State::OscString;
                self.osc_data.clear();
            }
            0xc0..=0xdf => {
                // 2-byte UTF-8
                self.utf8_buf[0] = byte;
                self.utf8_idx = 1;
                self.utf8_len = 2;
                self.state = State::Utf8;
            }
            0xe0..=0xef => {
                // 3-byte UTF-8
                self.utf8_buf[0] = byte;
                self.utf8_idx = 1;
                self.utf8_len = 3;
                self.state = State::Utf8;
            }
            0xf0..=0xf7 => {
                // 4-byte UTF-8
                self.utf8_buf[0] = byte;
                self.utf8_idx = 1;
                self.utf8_len = 4;
                self.state = State::Utf8;
            }
            _ => {} // Invalid bytes
        }
    }

    fn escape<P: Performer>(&mut self, performer: &mut P, byte: u8) {
        match byte {
            0x00..=0x17 | 0x19 | 0x1c..=0x1f => {
                performer.perform(Action::Execute(byte));
            }
            0x20..=0x2f => {
                self.intermediates.push(byte);
                self.state = State::EscapeIntermediate;
            }
            0x30..=0x4f | 0x51..=0x57 | 0x59 | 0x5a | 0x5c | 0x60..=0x7e => {
                performer.perform(Action::EscDispatch {
                    intermediates: self.intermediates,
                    ignore: false,
                    byte,
                });
                self.state = State::Ground;
            }
            0x50 => {
                // DCS
                self.state = State::DcsEntry;
                self.params.clear();
                self.intermediates.clear();
            }
            0x58 | 0x5e | 0x5f => {
                // SOS, PM, APC
                self.state = State::SosPmApcString;
            }
            0x5b => {
                // CSI [
                self.state = State::CsiEntry;
                self.params.clear();
                self.intermediates.clear();
            }
            0x5d => {
                // OSC ]
                self.state = State::OscString;
                self.osc_data.clear();
            }
            0x7f => {} // DEL - ignore
            _ => {
                self.state = State::Ground;
            }
        }
    }

    fn escape_intermediate<P: Performer>(&mut self, performer: &mut P, byte: u8) {
        match byte {
            0x00..=0x17 | 0x19 | 0x1c..=0x1f => {
                performer.perform(Action::Execute(byte));
            }
            0x20..=0x2f => {
                self.intermediates.push(byte);
            }
            0x30..=0x7e => {
                performer.perform(Action::EscDispatch {
                    intermediates: self.intermediates,
                    ignore: false,
                    byte,
                });
                self.state = State::Ground;
            }
            0x7f => {} // DEL - ignore
            _ => {
                self.state = State::Ground;
            }
        }
    }

    #[inline(always)]
    fn csi_entry<P: Performer>(&mut self, performer: &mut P, byte: u8) {
        match byte {
            0x00..=0x17 | 0x19 | 0x1c..=0x1f => {
                performer.perform(Action::Execute(byte));
            }
            0x20..=0x2f => {
                self.intermediates.push(byte);
                self.state = State::CsiIntermediate;
            }
            0x30..=0x3b => {
                // digits, semicolon, or colon subparam separator
                self.params.push(byte);
                self.state = State::CsiParam;
            }
            0x3c..=0x3f => {
                // private mode indicator (?, >, =, <)
                self.intermediates.push(byte);
                self.state = State::CsiParam;
            }
            0x40..=0x7e => {
                performer.perform(Action::CsiDispatch {
                    params: self.params,
                    intermediates: self.intermediates,
                    ignore: false,
                    action: byte,
                });
                self.state = State::Ground;
            }
            0x7f => {} // DEL - ignore
            _ => {
                self.state = State::Ground;
            }
        }
    }

    #[inline(always)]
    fn csi_param<P: Performer>(&mut self, performer: &mut P, byte: u8) {
        match byte {
            0x00..=0x17 | 0x19 | 0x1c..=0x1f => {
                performer.perform(Action::Execute(byte));
            }
            0x20..=0x2f => {
                self.intermediates.push(byte);
                self.state = State::CsiIntermediate;
            }
            0x30..=0x3b => {
                self.params.push(byte);
            }
            0x3c..=0x3f => {
                self.state = State::CsiIgnore;
            }
            0x40..=0x7e => {
                performer.perform(Action::CsiDispatch {
                    params: self.params,
                    intermediates: self.intermediates,
                    ignore: false,
                    action: byte,
                });
                self.state = State::Ground;
            }
            0x7f => {} // DEL - ignore
            _ => {
                self.state = State::Ground;
            }
        }
    }

    fn csi_intermediate<P: Performer>(&mut self, performer: &mut P, byte: u8) {
        match byte {
            0x00..=0x17 | 0x19 | 0x1c..=0x1f => {
                performer.perform(Action::Execute(byte));
            }
            0x20..=0x2f => {
                self.intermediates.push(byte);
            }
            0x30..=0x3f => {
                self.state = State::CsiIgnore;
            }
            0x40..=0x7e => {
                performer.perform(Action::CsiDispatch {
                    params: self.params,
                    intermediates: self.intermediates,
                    ignore: false,
                    action: byte,
                });
                self.state = State::Ground;
            }
            0x7f => {} // DEL - ignore
            _ => {
                self.state = State::Ground;
            }
        }
    }

    fn csi_ignore<P: Performer>(&mut self, performer: &mut P, byte: u8) {
        match byte {
            0x00..=0x17 | 0x19 | 0x1c..=0x1f => {
                performer.perform(Action::Execute(byte));
            }
            0x40..=0x7e => {
                self.state = State::Ground;
            }
            _ => {} // ignore
        }
    }

    fn osc_string<P: Performer>(&mut self, performer: &mut P, byte: u8) {
        match byte {
            0x07 | 0x9c => {
                // BEL or ST (8-bit) terminates OSC
                let parts = self.osc_data.split(|&b| b == b';').map(<[u8]>::to_vec).collect();
                performer.perform(Action::OscDispatch(parts));
                self.state = State::Ground;
            }
            0x00..=0x06 | 0x08..=0x1f => {
                // Ignore C0 controls in OSC (except BEL)
            }
            _ => {
                if self.osc_data.len() < MAX_OSC_DATA {
                    self.osc_data.push(byte);
                }
            }
        }
    }

    fn dcs_entry<P: Performer>(&mut self, _performer: &mut P, byte: u8) {
        match byte {
            0x20..=0x2f => {
                self.intermediates.push(byte);
                self.state = State::DcsIntermediate;
            }
            0x30..=0x39 | 0x3b => {
                self.params.push(byte);
                self.state = State::DcsParam;
            }
            0x3c..=0x3f => {
                self.intermediates.push(byte);
                self.state = State::DcsParam;
            }
            0x40..=0x7e => {
                self.state = State::DcsPassthrough;
            }
            _ => {
                self.state = State::DcsIgnore;
            }
        }
    }

    fn dcs_param<P: Performer>(&mut self, performer: &mut P, byte: u8) {
        match byte {
            0x30..=0x39 | 0x3b => {
                self.params.push(byte);
            }
            0x20..=0x2f => {
                self.intermediates.push(byte);
                self.state = State::DcsIntermediate;
            }
            0x40..=0x7e => {
                performer.perform(Action::DcsHook {
                    params: self.params,
                    intermediates: self.intermediates,
                    ignore: false,
                    byte,
                });
                self.dcs_data.clear();
                self.state = State::DcsPassthrough;
            }
            0x3a | 0x3c..=0x3f => {
                self.state = State::DcsIgnore;
            }
            _ => {}
        }
    }

    fn dcs_intermediate<P: Performer>(&mut self, performer: &mut P, byte: u8) {
        match byte {
            0x20..=0x2f => {
                self.intermediates.push(byte);
            }
            0x40..=0x7e => {
                performer.perform(Action::DcsHook {
                    params: self.params,
                    intermediates: self.intermediates,
                    ignore: false,
                    byte,
                });
                self.dcs_data.clear();
                self.state = State::DcsPassthrough;
            }
            0x30..=0x3f => {
                self.state = State::DcsIgnore;
            }
            _ => {}
        }
    }

    fn dcs_passthrough<P: Performer>(&mut self, performer: &mut P, byte: u8) {
        match byte {
            0x9c => {
                // ST terminates DCS
                performer.perform(Action::DcsUnhook);
                self.state = State::Ground;
            }
            0x00..=0x17 | 0x19 | 0x1c..=0x1f | 0x20..=0x7e => {
                performer.perform(Action::DcsPut(byte));
            }
            // DEL - ignore
            _ => {}
        }
    }

    fn dcs_ignore<P: Performer>(&mut self, _performer: &mut P, byte: u8) {
        if byte == 0x9c {
            self.state = State::Ground;
        }
    }

    fn sos_pm_apc_string<P: Performer>(&mut self, _performer: &mut P, byte: u8) {
        if byte == 0x9c {
            self.state = State::Ground;
        }
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Collector(Vec<Action>);

    impl Collector {
        fn new() -> Self {
            Self(Vec::new())
        }
    }

    impl Performer for Collector {
        fn perform(&mut self, action: Action) {
            self.0.push(action);
        }
    }

    #[test]
    fn ascii_fast_path() {
        let mut parser = Parser::new();
        let mut c = Collector::new();
        parser.advance(&mut c, b"Hello World");
        let expected: Vec<Action> = "Hello World".chars().map(Action::Print).collect();
        assert_eq!(c.0, expected);
    }

    #[test]
    fn utf8_multibyte() {
        // U+4E2D is a 3-byte UTF-8 char: E4 B8 AD
        let mut parser = Parser::new();
        let mut c = Collector::new();
        parser.advance(&mut c, &[0xE4, 0xB8, 0xAD]);
        assert_eq!(c.0, vec![Action::Print('\u{4E2D}')]);
    }

    #[test]
    fn csi_basic() {
        // \x1b[1;2m => CsiDispatch with params "1;2" and final byte 'm'
        let mut parser = Parser::new();
        let mut c = Collector::new();
        parser.advance(&mut c, b"\x1b[1;2m");
        assert_eq!(c.0.len(), 1);
        match &c.0[0] {
            Action::CsiDispatch { params, action, .. } => {
                assert_eq!(*action, b'm');
                assert_eq!(params.finished(), vec![1, 2]);
            }
            other => panic!("expected CsiDispatch, got {:?}", other),
        }
    }

    #[test]
    fn esc_cancel() {
        // ESC then CAN (0x18) should return to ground state
        let mut parser = Parser::new();
        let mut c = Collector::new();
        parser.advance(&mut c, &[0x1b, 0x18, b'A']);
        // CAN emits Execute(0x18), then ground state prints 'A'
        assert_eq!(c.0[0], Action::Execute(0x18));
        assert_eq!(c.0[1], Action::Print('A'));
        assert_eq!(c.0.len(), 2);
    }

    #[test]
    fn intermediates_overflow() {
        // Push more than MAX_INTERMEDIATES (4) intermediate bytes; should not panic
        let mut parser = Parser::new();
        let mut c = Collector::new();
        // ESC followed by 6 intermediate bytes (0x20..0x25) then final byte
        let mut input = vec![0x1b];
        for i in 0..6 {
            input.push(0x20 + i);
        }
        input.push(b'Z'); // final byte
        parser.advance(&mut c, &input);
        // Should complete without panic; the dispatch should happen
        assert_eq!(c.0.len(), 1);
        match &c.0[0] {
            Action::EscDispatch { intermediates, .. } => {
                // Only first MAX_INTERMEDIATES should be stored
                assert_eq!(intermediates.len(), MAX_INTERMEDIATES);
            }
            other => panic!("expected EscDispatch, got {:?}", other),
        }
    }

    #[test]
    fn osc_size_limit() {
        // Feed >64KB of OSC data; should not panic and data should be truncated
        let mut parser = Parser::new();
        let mut c = Collector::new();
        let mut input = vec![0x1b, b']']; // start OSC
        // Push 70000 bytes of 'A'
        input.extend(std::iter::repeat(b'A').take(70_000));
        input.push(0x07); // BEL to terminate
        parser.advance(&mut c, &input);
        assert_eq!(c.0.len(), 1);
        match &c.0[0] {
            Action::OscDispatch(parts) => {
                let total: usize = parts.iter().map(|p| p.len()).sum();
                assert!(total <= MAX_OSC_DATA, "OSC data should be truncated to {MAX_OSC_DATA}, got {total}");
            }
            other => panic!("expected OscDispatch, got {:?}", other),
        }
    }
}
