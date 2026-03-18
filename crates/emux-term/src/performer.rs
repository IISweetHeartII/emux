//! VT parser action performer -- bridges emux-vt events to screen mutations.
//!
//! This module implements [`emux_vt::Performer`] for [`Screen`], translating
//! parsed VT actions (`Print`, `Execute`, `CsiDispatch`, `EscDispatch`,
//! `OscDispatch`) into concrete screen state changes.
//!
//! Supported sequence families:
//! - **C0 controls**: BS, HT, LF/VT/FF, CR, SO/SI.
//! - **CSI**: cursor movement (CUU/CUD/CUF/CUB/CUP/HVP), erase (ED/EL/ECH),
//!   scroll (SU/SD), insert/delete (ICH/DCH/IL/DL), SGR, modes (SM/RM),
//!   margins (DECSTBM/DECSLRM), and more.
//! - **ESC**: IND, NEL, RI, HTS, DECSC/DECRC, RIS, DECALN, charset designation.
//! - **OSC**: window title (0/2), working directory (7).

use emux_vt::{Action, Charset, Performer as VtPerformer};

use crate::color::Color;
use crate::grid::UnderlineStyle;
use crate::modes::MouseMode;
use crate::screen::{ClearTabStop, EraseDisplay, EraseLine, Screen};

/// Terminal Screen implements `VtPerformer` to handle parsed VT sequences.
impl VtPerformer for Screen {
    fn perform(&mut self, action: Action) {
        match action {
            Action::Print(c) => self.write_char(c),
            Action::Execute(byte) => self.execute_c0(byte),
            Action::CsiDispatch {
                params,
                intermediates,
                action,
                ignore,
            } => {
                if !ignore {
                    self.handle_csi(&params, &intermediates, action);
                }
            }
            Action::EscDispatch {
                intermediates,
                byte,
                ignore,
            } => {
                if !ignore {
                    self.handle_esc(&intermediates, byte);
                }
            }
            Action::OscDispatch(parts) => {
                self.handle_osc(&parts);
            }
            // DCS, APC handled later
            _ => {}
        }
    }
}

impl Screen {
    // ── C0 control character handling ──────────────────────────────────

    /// Execute a C0 control character.
    pub fn execute_c0(&mut self, byte: u8) {
        match byte {
            // BEL (0x07): bell — no-op for now
            0x08 => {
                // BS: backspace
                self.backspace();
            }
            0x09 => {
                // HT: horizontal tab
                self.tab();
            }
            0x0A..=0x0C => {
                // LF, VT, FF: linefeed
                self.linefeed();
            }
            0x0D => {
                // CR: carriage return
                self.carriage_return();
            }
            0x0E => {
                // SO: activate G1 charset
                self.active_charset = 1;
            }
            0x0F => {
                // SI: activate G0 charset
                self.active_charset = 0;
            }
            _ => {} // Ignore other C0 controls
        }
    }

    // ── CSI dispatch ───────────────────────────────────────────────────

    /// Handle a CSI (Control Sequence Introducer) dispatch.
    pub fn handle_csi(&mut self, params: &emux_vt::Params, intermediates: &[u8], action: u8) {
        let ps = params.finished();
        let subflags = params.finished_subparam_flags();
        let private = intermediates.contains(&b'?');

        match action {
            b'A' => {
                // CUU: cursor up
                self.cursor_up(params.get_or(0, 1) as usize);
            }
            b'B' => {
                // CUD: cursor down
                self.cursor_down(params.get_or(0, 1) as usize);
            }
            b'C' => {
                // CUF: cursor forward (right)
                self.cursor_right(params.get_or(0, 1) as usize);
            }
            b'D' => {
                // CUB: cursor backward (left)
                self.cursor_left(params.get_or(0, 1) as usize);
            }
            b'E' => {
                // CNL: cursor next line
                self.cursor_down(params.get_or(0, 1) as usize);
                self.carriage_return();
            }
            b'F' => {
                // CPL: cursor previous line
                self.cursor_up(params.get_or(0, 1) as usize);
                self.carriage_return();
            }
            b'G' => {
                // CHA: cursor character absolute (1-based column)
                self.cursor_position(self.cursor.row + 1, params.get_or(0, 1) as usize);
            }
            b'H' | b'f' => {
                // CUP / HVP: cursor position (1-based row, col)
                let row = params.get_or(0, 1) as usize;
                let col = params.get_or(1, 1) as usize;
                self.cursor_position(row, col);
            }
            b'J' => {
                let mode = params.get_or(0, 0);
                if private {
                    // DECSED: selective erase in display (skips protected cells)
                    match mode {
                        0 => self.selective_erase_display(EraseDisplay::Below),
                        1 => self.selective_erase_display(EraseDisplay::Above),
                        2 => self.selective_erase_display(EraseDisplay::All),
                        3 => self.selective_erase_display(EraseDisplay::Scrollback),
                        _ => {}
                    }
                } else {
                    // ED: erase in display
                    match mode {
                        0 => self.erase_display(EraseDisplay::Below),
                        1 => self.erase_display(EraseDisplay::Above),
                        2 => self.erase_display(EraseDisplay::All),
                        3 => self.erase_display(EraseDisplay::Scrollback),
                        _ => {}
                    }
                }
            }
            b'K' => {
                let mode = params.get_or(0, 0);
                if private {
                    // DECSEL: selective erase in line (skips protected cells)
                    match mode {
                        0 => self.selective_erase_line(EraseLine::ToRight),
                        1 => self.selective_erase_line(EraseLine::ToLeft),
                        2 => self.selective_erase_line(EraseLine::All),
                        _ => {}
                    }
                } else {
                    // EL: erase in line
                    match mode {
                        0 => self.erase_line(EraseLine::ToRight),
                        1 => self.erase_line(EraseLine::ToLeft),
                        2 => self.erase_line(EraseLine::All),
                        _ => {}
                    }
                }
            }
            b'L' => {
                // IL: insert lines
                self.insert_lines(params.get_or(0, 1) as usize);
            }
            b'M' => {
                // DL: delete lines
                self.delete_lines(params.get_or(0, 1) as usize);
            }
            b'P' => {
                // DCH: delete characters
                self.delete_chars(params.get_or(0, 1) as usize);
            }
            b'S' => {
                // SU: scroll up
                self.scroll_up(params.get_or(0, 1) as usize);
            }
            b'T' => {
                // SD: scroll down
                self.scroll_down(params.get_or(0, 1) as usize);
            }
            b'X' => {
                // ECH: erase characters
                self.erase_chars(params.get_or(0, 1) as usize);
            }
            b'I' => {
                // CHT: cursor horizontal tab (forward N tab stops)
                self.tab_forward(params.get_or(0, 1) as usize);
            }
            b'Z' => {
                // CBT: cursor backward tab (backward N tab stops)
                self.tab_backward(params.get_or(0, 1) as usize);
            }
            b'@' => {
                // ICH: insert characters
                self.insert_chars(params.get_or(0, 1) as usize);
            }
            b'`' => {
                // HPA: horizontal position absolute (same as CHA)
                self.cursor_position(self.cursor.row + 1, params.get_or(0, 1) as usize);
            }
            b'a' => {
                // HPR: horizontal position relative (same as CUF)
                self.cursor_right(params.get_or(0, 1) as usize);
            }
            b'b' => {
                // REP: repeat last printed character
                let count = params.get_or(0, 1) as usize;
                if let Some(c) = self.last_printed_char() {
                    for _ in 0..count {
                        self.write_char(c);
                    }
                }
            }
            b'd' => {
                // VPA: vertical line position absolute (1-based row)
                self.cursor_position(params.get_or(0, 1) as usize, self.cursor.col + 1);
            }
            b'e' => {
                // VPR: vertical position relative
                self.cursor_down(params.get_or(0, 1) as usize);
            }
            b'g' => {
                // TBC: tab clear
                let mode = params.get_or(0, 0);
                match mode {
                    0 => self.clear_tab_stop(ClearTabStop::Current),
                    3 => self.clear_tab_stop(ClearTabStop::All),
                    _ => {}
                }
            }
            b'm' => {
                // SGR: select graphic rendition
                self.handle_sgr(&ps, &subflags);
            }
            // DSR (b'n'): device status report — no-op (requires response channel)
            b'u' => {
                // SCORC / DECRC: restore cursor
                self.restore_cursor();
            }
            b'h' => {
                // SM / DECSET: set mode
                self.set_modes(&ps, private, true);
            }
            b'l' => {
                // RM / DECRST: reset mode
                self.set_modes(&ps, private, false);
            }
            b'p' => {
                if intermediates.contains(&b'!') {
                    // DECSTR: soft terminal reset
                    self.soft_reset();
                }
            }
            b'q' => {
                if intermediates.contains(&b' ') {
                    // DECSCUSR: set cursor style
                    let style = params.get_or(0, 0);
                    self.set_cursor_style(style);
                } else if intermediates.contains(&b'"') {
                    // DECSCA: set character protection attribute
                    let p = params.get_or(0, 0);
                    match p {
                        1 => self.pen.protected = true,
                        0 | 2 => self.pen.protected = false,
                        _ => {}
                    }
                }
            }
            b'r' => {
                if private {
                    // CSI ? r = XTRESTORE: restore DEC private modes
                    self.restore_dec_modes();
                } else {
                    // DECSTBM: set top and bottom margins (1-based)
                    let top = params.get_or(0, 1) as usize;
                    let bottom = params.get_or(1, self.rows() as u16) as usize;
                    self.set_scroll_region(top, bottom);
                }
            }
            b's' => {
                if private {
                    // CSI ? s = XTSAVE: save DEC private modes
                    self.save_dec_modes();
                } else if self.modes.left_right_margin {
                    // DECSLRM: set left and right margins
                    let left = params.get_or(0, 1) as usize;
                    let right = params.get_or(1, self.cols() as u16) as usize;
                    self.set_left_right_margin(left, right);
                } else {
                    // SCOSC / DECSC: save cursor
                    self.save_cursor();
                }
            }
            b'x' => {
                if intermediates.contains(&b'$') {
                    // DECFRA: fill rectangular area
                    // CSI Pc;Pt;Pl;Pb;Pr $ x
                    let ch_code = params.get_or(0, 0);
                    let ch = if (32..=126).contains(&ch_code) {
                        ch_code as u8 as char
                    } else {
                        ' '
                    };
                    let top = params.get_or(1, 1) as usize;
                    let left = params.get_or(2, 1) as usize;
                    let bottom = params.get_or(3, self.rows() as u16) as usize;
                    let right = params.get_or(4, self.cols() as u16) as usize;
                    self.fill_rect(ch, top, left, bottom, right);
                }
            }
            b'v' => {
                if intermediates.contains(&b'$') {
                    // DECCRA: copy rectangular area
                    // CSI Pts;Pls;Pbs;Prs;Pp;Ptd;Pld;Ppd $ v
                    // We ignore page params (Pp, Ppd)
                    let src_top = params.get_or(0, 1) as usize;
                    let src_left = params.get_or(1, 1) as usize;
                    let src_bottom = params.get_or(2, self.rows() as u16) as usize;
                    let src_right = params.get_or(3, self.cols() as u16) as usize;
                    let _src_page = params.get_or(4, 1);
                    let dst_top = params.get_or(5, 1) as usize;
                    let dst_left = params.get_or(6, 1) as usize;
                    self.copy_rect(src_top, src_left, src_bottom, src_right, dst_top, dst_left);
                }
            }
            b'{' => {
                if intermediates.contains(&b'$') {
                    // DECSERA: selective erase rectangular area
                    // CSI Pt;Pl;Pb;Pr $ {
                    let top = params.get_or(0, 1) as usize;
                    let left = params.get_or(1, 1) as usize;
                    let bottom = params.get_or(2, self.rows() as u16) as usize;
                    let right = params.get_or(3, self.cols() as u16) as usize;
                    self.selective_erase_rect(top, left, bottom, right);
                } else if intermediates.contains(&b'#') {
                    // XTPUSHSGR: save SGR attributes
                    self.push_sgr();
                }
            }
            b'}' => {
                if intermediates.contains(&b'#') {
                    // XTPOPSGR: restore SGR attributes
                    self.pop_sgr();
                }
            }
            _ => {} // Unhandled CSI sequence
        }
    }

    // ── SGR (Select Graphic Rendition) ─────────────────────────────────

    /// Handle SGR parameter sequence.
    fn handle_sgr(&mut self, params: &[u16], subflags: &[bool]) {
        if params.is_empty() {
            self.reset_pen();
            return;
        }

        let mut i = 0;
        while i < params.len() {
            match params[i] {
                0 => self.reset_pen(),
                1 => self.pen.bold = true,
                3 => self.pen.italic = true,
                4 => {
                    // Check if next param is a colon-separated subparam
                    if i + 1 < params.len() && subflags.get(i + 1).copied().unwrap_or(false) {
                        // 4:N underline style subparam
                        let style = params[i + 1];
                        self.pen.underline = match style {
                            0 => UnderlineStyle::None,
                            1 => UnderlineStyle::Single,
                            2 => UnderlineStyle::Double,
                            3 => UnderlineStyle::Curly,
                            _ => UnderlineStyle::Single,
                        };
                        i += 2;
                        continue;
                    } else {
                        self.pen.underline = UnderlineStyle::Single;
                    }
                }
                5 => self.pen.blink = true,
                7 => self.pen.reverse = true,
                8 => self.pen.invisible = true,
                9 => self.pen.strikethrough = true,
                21 => self.pen.underline = UnderlineStyle::Double,
                22 => self.pen.bold = false,
                23 => self.pen.italic = false,
                24 => self.pen.underline = UnderlineStyle::None,
                25 => self.pen.blink = false,
                27 => self.pen.reverse = false,
                28 => self.pen.invisible = false,
                29 => self.pen.strikethrough = false,
                // Standard foreground colors (30-37)
                30..=37 => {
                    let idx = (params[i] - 30) as u8;
                    self.fg = Color::Indexed(idx);
                }
                // Extended foreground color
                38 => {
                    i += 1;
                    i = self.parse_extended_color(params, i, true);
                    continue;
                }
                // Default foreground
                39 => self.fg = Color::Default,
                // Standard background colors (40-47)
                40..=47 => self.bg = Color::Indexed((params[i] - 40) as u8),
                // Extended background color
                48 => {
                    i += 1;
                    i = self.parse_extended_color(params, i, false);
                    continue;
                }
                // Default background
                49 => self.bg = Color::Default,
                // Bright foreground colors (90-97)
                90..=97 => self.fg = Color::Indexed((params[i] - 90 + 8) as u8),
                // Bright background colors (100-107)
                100..=107 => self.bg = Color::Indexed((params[i] - 100 + 8) as u8),
                _ => {} // Unhandled SGR parameter
            }
            i += 1;
        }
    }

    /// Parse extended color sequence (;5;N for indexed or ;2;R;G;B for RGB).
    fn parse_extended_color(&mut self, params: &[u16], mut i: usize, is_fg: bool) -> usize {
        if i >= params.len() {
            return i;
        }
        match params[i] {
            5 => {
                // Indexed color: 38;5;N or 48;5;N
                i += 1;
                if i < params.len() {
                    let color = Color::Indexed(params[i] as u8);
                    if is_fg {
                        self.fg = color;
                    } else {
                        self.bg = color;
                    }
                    i += 1;
                }
            }
            2 => {
                // RGB color: 38;2;R;G;B or 48;2;R;G;B
                i += 1;
                if i + 2 < params.len() {
                    let color =
                        Color::Rgb(params[i] as u8, params[i + 1] as u8, params[i + 2] as u8);
                    if is_fg {
                        self.fg = color;
                    } else {
                        self.bg = color;
                    }
                    i += 3;
                }
            }
            _ => {
                i += 1;
            }
        }
        i
    }

    /// Reset pen (drawing attributes) to defaults.
    /// Note: DECSCA protection is independent of SGR and is NOT reset by SGR 0.
    fn reset_pen(&mut self) {
        let protected = self.pen.protected;
        self.pen = crate::grid::CellAttrs::default();
        self.pen.protected = protected;
        self.fg = Color::Default;
        self.bg = Color::Default;
    }

    // ── ESC dispatch ───────────────────────────────────────────────────

    /// Handle an ESC (escape) sequence dispatch.
    pub fn handle_esc(&mut self, intermediates: &[u8], byte: u8) {
        match (intermediates.first(), byte) {
            (None, b'D') => {
                // IND: index (move cursor down, scrolling if needed)
                self.index();
            }
            (None, b'E') => {
                // NEL: next line
                self.carriage_return();
                self.linefeed();
            }
            (None, b'H') => {
                // HTS: set horizontal tab stop
                self.set_tab_stop();
            }
            (None, b'M') => {
                // RI: reverse index
                self.reverse_index();
            }
            (None, b'7') => {
                // DECSC: save cursor
                self.save_cursor();
            }
            (None, b'8') => {
                // DECRC: restore cursor
                self.restore_cursor();
            }
            (None, b'=') => {
                // DECKPAM: application keypad mode
                self.modes.application_keypad = true;
            }
            (None, b'>') => {
                // DECKPNM: normal keypad mode
                self.modes.application_keypad = false;
            }
            (None, b'c') => {
                // RIS: full reset
                self.reset();
            }
            (Some(b'('), designator) => {
                // Designate G0 character set
                self.charset_g0 = charset_from_designator(designator);
            }
            (Some(b')'), designator) => {
                // Designate G1 character set
                self.charset_g1 = charset_from_designator(designator);
            }
            (Some(b'#'), b'8') => {
                // DECALN: screen alignment test
                self.decaln();
            }
            _ => {} // Unhandled ESC sequence
        }
    }

    // ── OSC dispatch ───────────────────────────────────────────────────

    /// Handle an OSC (Operating System Command) sequence.
    pub fn handle_osc(&mut self, parts: &[Vec<u8>]) {
        if parts.is_empty() {
            return;
        }
        let cmd = std::str::from_utf8(&parts[0]).unwrap_or("");
        match cmd {
            "0" | "2" => {
                // Set window title
                if parts.len() > 1 {
                    self.title = String::from_utf8_lossy(&parts[1]).to_string();
                }
            }
            "7" => {
                // Set working directory (OSC 7)
                if parts.len() > 1 {
                    let dir = String::from_utf8_lossy(&parts[1]).to_string();
                    self.working_directory = Some(dir);
                }
            }
            "4" => {
                // OSC 4: color palette set/query
                // Format: OSC 4;N;spec ST  or  OSC 4;N;? ST
                if parts.len() >= 3 {
                    let idx_str = std::str::from_utf8(&parts[1]).unwrap_or("");
                    let spec = std::str::from_utf8(&parts[2]).unwrap_or("");
                    if let Ok(idx) = idx_str.parse::<u16>()
                        && idx < 256 {
                            if spec == "?" {
                                // Query: respond with current color
                                let (r, g, b) = self.palette_color(idx as u8);
                                let response = format!(
                                    "\x1b]4;{};rgb:{:02x}{:02x}/{:02x}{:02x}/{:02x}{:02x}\x1b\\",
                                    idx, r, r, g, g, b, b
                                );
                                self.response_buf.extend_from_slice(response.as_bytes());
                            } else if let Some(rgb) = parse_color_spec(spec) {
                                self.set_palette_color(idx as u8, rgb.0, rgb.1, rgb.2);
                            }
                        }
                }
            }
            "8" => {
                // OSC 8: hyperlinks
                // Format: OSC 8;params;uri ST (start) or OSC 8;; ST (end)
                if parts.len() >= 3 {
                    let uri = std::str::from_utf8(&parts[2]).unwrap_or("");
                    if uri.is_empty() {
                        self.hyperlink = None;
                    } else {
                        self.hyperlink = Some(uri.to_string());
                    }
                } else if parts.len() == 2 {
                    // OSC 8; ST (missing URI => end hyperlink)
                    self.hyperlink = None;
                }
            }
            // "133": Semantic prompt markers (OSC 133;X)
            // A = prompt start, B = command start, C = output start,
            // D;exitcode = command finished.
            // For now, just accept these without crashing.
            _ => {} // Unhandled OSC
        }
    }

    // ── Mode setting ───────────────────────────────────────────────────

    /// Set or reset terminal modes.
    fn set_modes(&mut self, params: &[u16], private: bool, enable: bool) {
        for &p in params {
            if private {
                self.set_private_mode(p, enable);
            } else {
                self.set_ansi_mode(p, enable);
            }
        }
    }

    /// Set or reset an ANSI mode.
    fn set_ansi_mode(&mut self, mode: u16, enable: bool) {
        match mode {
            4 => self.modes.insert = enable,   // IRM: insert/replace mode
            20 => self.modes.newline = enable,  // LNM: linefeed/newline mode
            _ => {}
        }
    }

    /// Set or reset a DEC private mode.
    fn set_private_mode(&mut self, mode: u16, enable: bool) {
        match mode {
            // DECCKM (1): cursor keys mode
            1 => {
                self.modes.application_cursor_keys = enable;
            }
            3 => {
                // DECCOLM: 132/80 column mode
                if !self.modes.allow_deccolm {
                    return;
                }
                let new_cols = if enable { 132 } else { 80 };
                let rows = self.rows();
                self.resize(new_cols, rows);
                // Clear screen
                self.erase_display(EraseDisplay::All);
                // Reset cursor
                self.cursor.row = 0;
                self.cursor.col = 0;
                self.clear_pending_wrap();
                // Reset scroll margins
                self.scroll_top = 0;
                self.scroll_bottom = rows;
            }
            6 => {
                // DECOM: origin mode
                self.modes.origin = enable;
                // Moving to home on origin mode change
                self.cursor_position(1, 1);
            }
            7 => {
                // DECAWM: autowrap mode
                self.modes.autowrap = enable;
            }
            9 => {
                // X10 mouse tracking
                self.modes.mouse_tracking =
                    if enable { MouseMode::X10 } else { MouseMode::None };
            }
            // Start blinking cursor (12) — handled by display layer
            45 => {
                // Reverse wrap mode
                self.modes.reverse_wrap = enable;
            }
            40 => {
                // Allow/disallow 80/132 column switching
                self.modes.allow_deccolm = enable;
            }
            69 => {
                // DECLRMM: left/right margin mode
                self.modes.left_right_margin = enable;
                if !enable {
                    // Reset left/right margins to full width
                    self.scroll_left = 0;
                    self.scroll_right = self.cols();
                }
            }
            25 => {
                // DECTCEM: cursor visibility
                self.cursor.visible = enable;
            }
            1000 => {
                // Normal mouse tracking
                self.modes.mouse_tracking =
                    if enable { MouseMode::Normal } else { MouseMode::None };
            }
            1002 => {
                // Button-event mouse tracking
                self.modes.mouse_tracking =
                    if enable { MouseMode::ButtonEvent } else { MouseMode::None };
            }
            1003 => {
                // Any-event mouse tracking
                self.modes.mouse_tracking =
                    if enable { MouseMode::AnyEvent } else { MouseMode::None };
            }
            1004 => {
                // Focus tracking
                self.modes.focus_tracking = enable;
            }
            1006 => {
                // SGR extended mouse encoding
                self.modes.mouse_sgr = enable;
            }
            47 | 1047 => {
                // Alternate screen buffer (no cursor save/restore)
                if enable {
                    self.enter_alt_screen(false);
                } else {
                    self.leave_alt_screen(false);
                }
            }
            1048 => {
                // Save/restore cursor (same as DECSC/DECRC but via DEC private mode)
                if enable {
                    self.save_cursor();
                } else {
                    self.restore_cursor();
                }
            }
            1049 => {
                // Alternate screen buffer with cursor save/restore (DECSC/DECRC)
                if enable {
                    self.enter_alt_screen(true);
                } else {
                    self.leave_alt_screen(true);
                }
            }
            2004 => {
                // Bracketed paste mode
                self.modes.bracketed_paste = enable;
            }
            _ => {} // Unhandled private mode
        }
    }

    // ── Last printed char for REP ──────────────────────────────────────

    /// Get the last printed character (for REP sequence).
    fn last_printed_char(&self) -> Option<char> {
        self.last_char
    }
}

/// Map a charset designator byte to a Charset.
fn charset_from_designator(byte: u8) -> Charset {
    match byte {
        b'0' => Charset::DecSpecialGraphics,
        b'A' => Charset::Uk,
        _ => Charset::Ascii, // B and others -> ASCII
    }
}

/// Parse a color specification string in various formats:
/// - `rgb:RR/GG/BB` or `rgb:RRRR/GGGG/BBBB`
/// - `#RGB` (4-bit per channel)
/// - `#RRGGBB` (8-bit per channel)
fn parse_color_spec(spec: &str) -> Option<(u8, u8, u8)> {
    if let Some(rest) = spec.strip_prefix("rgb:") {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() == 3 {
            let r = u16::from_str_radix(parts[0], 16).ok()?;
            let g = u16::from_str_radix(parts[1], 16).ok()?;
            let b = u16::from_str_radix(parts[2], 16).ok()?;
            // If 4-digit hex (e.g., "RRRR"), scale to 8-bit
            if parts[0].len() <= 2 {
                return Some((r as u8, g as u8, b as u8));
            } else {
                return Some(((r >> 8) as u8, (g >> 8) as u8, (b >> 8) as u8));
            }
        }
    } else if let Some(rest) = spec.strip_prefix('#') {
        match rest.len() {
            3 => {
                // #RGB -> expand each nibble
                let r = u8::from_str_radix(&rest[0..1], 16).ok()?;
                let g = u8::from_str_radix(&rest[1..2], 16).ok()?;
                let b = u8::from_str_radix(&rest[2..3], 16).ok()?;
                return Some((r * 17, g * 17, b * 17));
            }
            6 => {
                let r = u8::from_str_radix(&rest[0..2], 16).ok()?;
                let g = u8::from_str_radix(&rest[2..4], 16).ok()?;
                let b = u8::from_str_radix(&rest[4..6], 16).ok()?;
                return Some((r, g, b));
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use emux_vt::Parser;

    /// Helper: feed raw bytes through the parser into a screen.
    fn feed(screen: &mut Screen, data: &[u8]) {
        let mut parser = Parser::new();
        parser.advance(screen, data);
    }

    #[test]
    fn print_characters() {
        let mut s = Screen::new(80, 25);
        feed(&mut s, b"Hello");
        assert_eq!(s.row_text(0), "Hello");
        assert_eq!(s.cursor.col, 5);
    }

    #[test]
    fn cursor_movement_csi() {
        let mut s = Screen::new(80, 25);
        feed(&mut s, b"\x1b[5;10H");
        assert_eq!(s.cursor.row, 4);
        assert_eq!(s.cursor.col, 9);
    }

    #[test]
    fn cursor_up_down() {
        let mut s = Screen::new(80, 25);
        feed(&mut s, b"\x1b[10;1H");
        feed(&mut s, b"\x1b[3A");
        assert_eq!(s.cursor.row, 6);
        feed(&mut s, b"\x1b[5B");
        assert_eq!(s.cursor.row, 11);
    }

    #[test]
    fn erase_display_below() {
        let mut s = Screen::new(10, 3);
        feed(&mut s, b"ABCDEFGHIJ");
        feed(&mut s, b"\x1b[1;6H");
        feed(&mut s, b"\x1b[0J");
        assert_eq!(s.grid.cell(0, 4).c, 'E');
        assert_eq!(s.grid.cell(0, 5).c, ' ');
    }

    #[test]
    fn sgr_bold_and_color() {
        let mut s = Screen::new(80, 25);
        feed(&mut s, b"\x1b[1;31mX");
        assert!(s.pen.bold);
        assert_eq!(s.fg, Color::Indexed(1));
        assert!(s.grid.cell(0, 0).attrs.bold);
        assert_eq!(s.grid.cell(0, 0).fg, Color::Indexed(1));
    }

    #[test]
    fn sgr_reset() {
        let mut s = Screen::new(80, 25);
        feed(&mut s, b"\x1b[1;31m");
        assert!(s.pen.bold);
        feed(&mut s, b"\x1b[0m");
        assert!(!s.pen.bold);
        assert_eq!(s.fg, Color::Default);
    }

    #[test]
    fn sgr_256_color() {
        let mut s = Screen::new(80, 25);
        feed(&mut s, b"\x1b[38;5;42m");
        assert_eq!(s.fg, Color::Indexed(42));
    }

    #[test]
    fn sgr_rgb_color() {
        let mut s = Screen::new(80, 25);
        feed(&mut s, b"\x1b[38;2;100;150;200m");
        assert_eq!(s.fg, Color::Rgb(100, 150, 200));
    }

    #[test]
    fn tab_stop() {
        let mut s = Screen::new(80, 25);
        feed(&mut s, b"\t");
        assert_eq!(s.cursor.col, 8);
    }

    #[test]
    fn carriage_return_linefeed() {
        let mut s = Screen::new(80, 25);
        feed(&mut s, b"ABC\r\nDEF");
        assert_eq!(s.row_text(0), "ABC");
        assert_eq!(s.row_text(1), "DEF");
    }

    #[test]
    fn esc_save_restore_cursor() {
        let mut s = Screen::new(80, 25);
        feed(&mut s, b"\x1b[5;10H");
        feed(&mut s, b"\x1b7");
        feed(&mut s, b"\x1b[1;1H");
        feed(&mut s, b"\x1b8");
        assert_eq!(s.cursor.row, 4);
        assert_eq!(s.cursor.col, 9);
    }

    #[test]
    fn esc_reverse_index() {
        let mut s = Screen::new(80, 25);
        s.cursor.row = 0;
        feed(&mut s, b"\x1bM");
        assert_eq!(s.cursor.row, 0);
    }

    #[test]
    fn osc_set_title() {
        let mut s = Screen::new(80, 25);
        feed(&mut s, b"\x1b]2;My Title\x07");
        assert_eq!(s.title, "My Title");
    }

    #[test]
    fn scroll_region() {
        let mut s = Screen::new(80, 25);
        feed(&mut s, b"\x1b[5;20r");
        assert_eq!(s.scroll_top, 4);
        assert_eq!(s.scroll_bottom, 20);
    }

    #[test]
    fn insert_delete_lines() {
        let mut s = Screen::new(10, 5);
        feed(&mut s, b"line 0\r\n");
        feed(&mut s, b"line 1\r\n");
        feed(&mut s, b"line 2\r\n");
        feed(&mut s, b"\x1b[2;1H");
        feed(&mut s, b"\x1b[1L");
        assert_eq!(s.row_text(1), "");
        assert_eq!(s.row_text(2), "line 1");
    }

    #[test]
    fn dec_special_graphics_charset() {
        let mut s = Screen::new(80, 25);
        feed(&mut s, b"\x1b(0");
        feed(&mut s, b"q");
        assert_eq!(s.grid.cell(0, 0).c, '\u{2500}');
        feed(&mut s, b"\x1b(B");
        feed(&mut s, b"q");
        assert_eq!(s.grid.cell(0, 1).c, 'q');
    }

    #[test]
    fn backspace() {
        let mut s = Screen::new(80, 25);
        feed(&mut s, b"AB\x08C");
        assert_eq!(s.grid.cell(0, 0).c, 'A');
        assert_eq!(s.grid.cell(0, 1).c, 'C');
    }

    // ── DECCKM (mode 1): application cursor keys ─────────────────────

    #[test]
    fn decckm_set_via_csi() {
        // CSI ?1h sets application_cursor_keys = true
        let mut s = Screen::new(80, 25);
        assert!(!s.modes.application_cursor_keys);
        feed(&mut s, b"\x1b[?1h");
        assert!(s.modes.application_cursor_keys);
    }

    #[test]
    fn decckm_reset_via_csi() {
        // CSI ?1l resets application_cursor_keys = false
        let mut s = Screen::new(80, 25);
        feed(&mut s, b"\x1b[?1h");
        assert!(s.modes.application_cursor_keys);
        feed(&mut s, b"\x1b[?1l");
        assert!(!s.modes.application_cursor_keys);
    }

    #[test]
    fn decckm_set_via_handle_esc_method() {
        // Directly call set_private_mode(1, true/false)
        let mut s = Screen::new(80, 25);
        assert!(!s.modes.application_cursor_keys);
        s.set_private_mode(1, true);
        assert!(s.modes.application_cursor_keys);
        s.set_private_mode(1, false);
        assert!(!s.modes.application_cursor_keys);
    }

    // ── DECKPAM / DECKPNM: application keypad mode ──────────────────

    #[test]
    fn deckpam_esc_equals() {
        // ESC = enables application keypad mode
        let mut s = Screen::new(80, 25);
        assert!(!s.modes.application_keypad);
        feed(&mut s, b"\x1b=");
        assert!(s.modes.application_keypad);
    }

    #[test]
    fn deckpnm_esc_greater_than() {
        // ESC > disables application keypad mode
        let mut s = Screen::new(80, 25);
        feed(&mut s, b"\x1b=");
        assert!(s.modes.application_keypad);
        feed(&mut s, b"\x1b>");
        assert!(!s.modes.application_keypad);
    }

    #[test]
    fn deckpam_deckpnm_toggle() {
        // Toggle application keypad mode back and forth
        let mut s = Screen::new(80, 25);
        assert!(!s.modes.application_keypad);
        feed(&mut s, b"\x1b=");
        assert!(s.modes.application_keypad);
        feed(&mut s, b"\x1b>");
        assert!(!s.modes.application_keypad);
        feed(&mut s, b"\x1b=");
        assert!(s.modes.application_keypad);
    }

    #[test]
    fn deckpam_via_handle_esc_method() {
        // Directly call handle_esc for ESC = and ESC >
        let mut s = Screen::new(80, 25);
        assert!(!s.modes.application_keypad);
        s.handle_esc(&[], b'=');
        assert!(s.modes.application_keypad);
        s.handle_esc(&[], b'>');
        assert!(!s.modes.application_keypad);
    }

    // ── Bracketed paste mode via CSI ─────────────────────────────────

    #[test]
    fn bracketed_paste_mode_set_reset() {
        // CSI ?2004h enables, CSI ?2004l disables
        let mut s = Screen::new(80, 25);
        assert!(!s.modes.bracketed_paste);
        feed(&mut s, b"\x1b[?2004h");
        assert!(s.modes.bracketed_paste);
        feed(&mut s, b"\x1b[?2004l");
        assert!(!s.modes.bracketed_paste);
    }

    // ── Focus tracking mode (1004) ───────────────────────────────────

    #[test]
    fn focus_tracking_mode_set_reset() {
        // CSI ?1004h enables, CSI ?1004l disables
        let mut s = Screen::new(80, 25);
        assert!(!s.modes.focus_tracking);
        feed(&mut s, b"\x1b[?1004h");
        assert!(s.modes.focus_tracking);
        feed(&mut s, b"\x1b[?1004l");
        assert!(!s.modes.focus_tracking);
    }

    #[test]
    fn focus_tracking_survives_soft_reset() {
        // DECSTR (CSI ! p) should reset focus tracking
        let mut s = Screen::new(80, 25);
        feed(&mut s, b"\x1b[?1004h");
        assert!(s.modes.focus_tracking);
        feed(&mut s, b"\x1b[!p");
        // After soft reset, modes return to default
        assert!(!s.modes.focus_tracking);
    }

    // ── Newline mode (LNM) ──────────────────────────────────────────

    #[test]
    fn newline_mode_set_reset() {
        // CSI 20h enables LNM, CSI 20l disables
        let mut s = Screen::new(80, 25);
        assert!(!s.modes.newline);
        feed(&mut s, b"\x1b[20h");
        assert!(s.modes.newline);
        feed(&mut s, b"\x1b[20l");
        assert!(!s.modes.newline);
    }

    // ── Insert mode (IRM) ───────────────────────────────────────────

    #[test]
    fn insert_mode_set_reset() {
        // CSI 4h enables insert mode, CSI 4l disables
        let mut s = Screen::new(80, 25);
        assert!(!s.modes.insert);
        feed(&mut s, b"\x1b[4h");
        assert!(s.modes.insert);
        feed(&mut s, b"\x1b[4l");
        assert!(!s.modes.insert);
    }

    // ── DECCKM end-to-end with input encoding ───────────────────────

    #[test]
    fn decckm_affects_cursor_key_encoding() {
        use crate::input::{Key, Modifiers, encode_key};

        let mut s = Screen::new(80, 25);

        // Normal mode: arrow keys use CSI
        let up = encode_key(Key::Up, Modifiers::none(), s.modes.application_cursor_keys, false, false, false);
        assert_eq!(up, b"\x1b[A");

        // Enable DECCKM
        feed(&mut s, b"\x1b[?1h");
        let up = encode_key(Key::Up, Modifiers::none(), s.modes.application_cursor_keys, false, false, false);
        assert_eq!(up, b"\x1bOA");

        // Disable DECCKM
        feed(&mut s, b"\x1b[?1l");
        let up = encode_key(Key::Up, Modifiers::none(), s.modes.application_cursor_keys, false, false, false);
        assert_eq!(up, b"\x1b[A");
    }

    // ── DECKPAM end-to-end with keypad encoding ─────────────────────

    #[test]
    fn deckpam_affects_keypad_encoding() {
        use crate::input::{KeypadKey, encode_keypad};

        let mut s = Screen::new(80, 25);

        // Numeric mode (default)
        let num0 = encode_keypad(KeypadKey::Num0, s.modes.application_keypad);
        assert_eq!(num0, b"0");

        // Enable DECKPAM
        feed(&mut s, b"\x1b=");
        let num0 = encode_keypad(KeypadKey::Num0, s.modes.application_keypad);
        assert_eq!(num0, b"\x1bOp");

        // Disable (DECKPNM)
        feed(&mut s, b"\x1b>");
        let num0 = encode_keypad(KeypadKey::Num0, s.modes.application_keypad);
        assert_eq!(num0, b"0");
    }

    // ── RIS resets all modes ────────────────────────────────────────

    #[test]
    fn ris_resets_all_modes() {
        let mut s = Screen::new(80, 25);
        feed(&mut s, b"\x1b[?1h");       // DECCKM
        feed(&mut s, b"\x1b=");           // DECKPAM
        feed(&mut s, b"\x1b[?1004h");     // focus tracking
        feed(&mut s, b"\x1b[?2004h");     // bracketed paste

        assert!(s.modes.application_cursor_keys);
        assert!(s.modes.application_keypad);
        assert!(s.modes.focus_tracking);
        assert!(s.modes.bracketed_paste);

        // Full reset
        feed(&mut s, b"\x1bc");

        assert!(!s.modes.application_cursor_keys);
        assert!(!s.modes.application_keypad);
        assert!(!s.modes.focus_tracking);
        assert!(!s.modes.bracketed_paste);
    }
}
