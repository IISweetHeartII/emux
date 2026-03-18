//! VT state machine parser.
//!
//! Based on the Paul Williams VT parser state machine.
//! Reference: <https://vt100.net/emu/dec_ansi_parser>

use crate::params::Params;

const MAX_INTERMEDIATES: usize = 4;
const MAX_OSC_DATA: usize = 65536;

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
        intermediates: Vec<u8>,
        ignore: bool,
        action: u8,
    },
    /// ESC sequence dispatched.
    EscDispatch {
        intermediates: Vec<u8>,
        ignore: bool,
        byte: u8,
    },
    /// OSC sequence (terminated by BEL or ST).
    OscDispatch(Vec<Vec<u8>>),
    /// DCS sequence hook.
    DcsHook {
        params: Params,
        intermediates: Vec<u8>,
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
    intermediates: Vec<u8>,
    osc_data: Vec<u8>,
    dcs_data: Vec<u8>,
    utf8_buf: [u8; 4],
    utf8_idx: u8,
    utf8_len: u8,
}

impl Parser {
    pub fn new() -> Self {
        Self {
            state: State::Ground,
            params: Params::new(),
            intermediates: Vec::new(),
            osc_data: Vec::new(),
            dcs_data: Vec::new(),
            utf8_buf: [0; 4],
            utf8_idx: 0,
            utf8_len: 0,
        }
    }

    /// Feed a slice of bytes to the parser.
    pub fn advance<P: Performer>(&mut self, performer: &mut P, bytes: &[u8]) {
        for &byte in bytes {
            self.advance_byte(performer, byte);
        }
    }

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
                if self.intermediates.len() < MAX_INTERMEDIATES {
                    self.intermediates.push(byte);
                }
                self.state = State::EscapeIntermediate;
            }
            0x30..=0x4f | 0x51..=0x57 | 0x59 | 0x5a | 0x5c | 0x60..=0x7e => {
                performer.perform(Action::EscDispatch {
                    intermediates: self.intermediates.clone(),
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
                if self.intermediates.len() < MAX_INTERMEDIATES { self.intermediates.push(byte); }
            }
            0x30..=0x7e => {
                performer.perform(Action::EscDispatch {
                    intermediates: self.intermediates.clone(),
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

    fn csi_entry<P: Performer>(&mut self, performer: &mut P, byte: u8) {
        match byte {
            0x00..=0x17 | 0x19 | 0x1c..=0x1f => {
                performer.perform(Action::Execute(byte));
            }
            0x20..=0x2f => {
                if self.intermediates.len() < MAX_INTERMEDIATES { self.intermediates.push(byte); }
                self.state = State::CsiIntermediate;
            }
            0x30..=0x39 | 0x3b => {
                self.params.push(byte);
                self.state = State::CsiParam;
            }
            0x3a => {
                // subparam separator
                self.params.push(byte);
                self.state = State::CsiParam;
            }
            0x3c..=0x3f => {
                // private mode indicator (?, >, =, <)
                if self.intermediates.len() < MAX_INTERMEDIATES { self.intermediates.push(byte); }
                self.state = State::CsiParam;
            }
            0x40..=0x7e => {
                performer.perform(Action::CsiDispatch {
                    params: self.params.clone(),
                    intermediates: self.intermediates.clone(),
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

    fn csi_param<P: Performer>(&mut self, performer: &mut P, byte: u8) {
        match byte {
            0x00..=0x17 | 0x19 | 0x1c..=0x1f => {
                performer.perform(Action::Execute(byte));
            }
            0x20..=0x2f => {
                if self.intermediates.len() < MAX_INTERMEDIATES { self.intermediates.push(byte); }
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
                    params: self.params.clone(),
                    intermediates: self.intermediates.clone(),
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
                if self.intermediates.len() < MAX_INTERMEDIATES { self.intermediates.push(byte); }
            }
            0x30..=0x3f => {
                self.state = State::CsiIgnore;
            }
            0x40..=0x7e => {
                performer.perform(Action::CsiDispatch {
                    params: self.params.clone(),
                    intermediates: self.intermediates.clone(),
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
                if self.intermediates.len() < MAX_INTERMEDIATES { self.intermediates.push(byte); }
                self.state = State::DcsIntermediate;
            }
            0x30..=0x39 | 0x3b => {
                self.params.push(byte);
                self.state = State::DcsParam;
            }
            0x3c..=0x3f => {
                if self.intermediates.len() < MAX_INTERMEDIATES { self.intermediates.push(byte); }
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
                if self.intermediates.len() < MAX_INTERMEDIATES { self.intermediates.push(byte); }
                self.state = State::DcsIntermediate;
            }
            0x40..=0x7e => {
                performer.perform(Action::DcsHook {
                    params: self.params.clone(),
                    intermediates: self.intermediates.clone(),
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
                if self.intermediates.len() < MAX_INTERMEDIATES { self.intermediates.push(byte); }
            }
            0x40..=0x7e => {
                performer.perform(Action::DcsHook {
                    params: self.params.clone(),
                    intermediates: self.intermediates.clone(),
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
