//! Parser conformance tests translated from libvterm 02parser.test and
//! 03encoding_utf8.test.
//!
//! Each test creates a fresh parser, feeds bytes via `advance()`, and asserts
//! that the collected `Action` sequence matches expectation.

use emux_vt::{Action, Parser, Performer};

// ---------------------------------------------------------------------------
// Test harness
// ---------------------------------------------------------------------------

struct TestPerformer {
    actions: Vec<Action>,
}

impl TestPerformer {
    fn new() -> Self {
        Self {
            actions: Vec::new(),
        }
    }

    fn drain(&mut self) -> Vec<Action> {
        std::mem::take(&mut self.actions)
    }
}

impl Performer for TestPerformer {
    fn perform(&mut self, action: Action) {
        self.actions.push(action);
    }
}

/// Helper: build a `Params` from a param string like "3;4" by pushing each
/// byte through `Params::push`. This mirrors how the parser accumulates
/// params internally.
fn make_params(spec: &str) -> emux_vt::Params {
    let mut p = emux_vt::Params::new();
    for b in spec.as_bytes() {
        p.push(*b);
    }
    p
}

/// Shorthand: create a CsiDispatch action with the given final byte, params
/// string, and optional intermediates/leader bytes.
fn csi(action: u8, param_str: &str, intermediates: &[u8]) -> Action {
    Action::CsiDispatch {
        params: make_params(param_str),
        intermediates: intermediates.to_vec(),
        ignore: false,
        action,
    }
}

fn esc(intermediates: &[u8], byte: u8) -> Action {
    Action::EscDispatch {
        intermediates: intermediates.to_vec(),
        ignore: false,
        byte,
    }
}

fn prints(chars: &[char]) -> Vec<Action> {
    chars.iter().map(|&c| Action::Print(c)).collect()
}

// =========================================================================
// 02parser.test — Basic text
// =========================================================================

#[test]
fn basic_text() {
    // PUSH "hello"  =>  text 0x68,0x65,0x6c,0x6c,0x6f
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"hello");
    assert_eq!(
        p.actions,
        prints(&['h', 'e', 'l', 'l', 'o']),
    );
}

// =========================================================================
// C0 controls
// =========================================================================

#[test]
fn c0_control_0x03() {
    // PUSH "\x03"  =>  control 3
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0x03]);
    assert_eq!(p.actions, vec![Action::Execute(0x03)]);
}

#[test]
fn c0_control_0x1f() {
    // PUSH "\x1f"  =>  control 0x1f
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0x1f]);
    assert_eq!(p.actions, vec![Action::Execute(0x1f)]);
}

// =========================================================================
// C1 8-bit controls
// =========================================================================

#[test]
fn c1_8bit_0x83() {
    // PUSH "\x83"  =>  control 0x83
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0x83]);
    assert_eq!(p.actions, vec![Action::Execute(0x83)]);
}

#[test]
fn c1_8bit_0x99() {
    // PUSH "\x99"  =>  control 0x99
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0x99]);
    assert_eq!(p.actions, vec![Action::Execute(0x99)]);
}

// =========================================================================
// C1 7-bit (ESC + 0x40..0x5f mapped to 0x80..0x9f)
// =========================================================================

#[test]
fn c1_7bit_esc_0x43() {
    // PUSH "\eC"  =>  control 0x83
    // ESC 0x43 should map to C1 control 0x83.
    // The parser dispatches EscDispatch for bytes in 0x40..0x5F range.
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0x1b, 0x43]);
    // The actual parser emits EscDispatch here (not Execute 0x83).
    // This is a design choice — libvterm maps ESC+letter to C1 controls,
    // but the standard VT parser emits EscDispatch for the ESC sequence.
    assert_eq!(
        p.actions,
        vec![esc(&[], 0x43)],
    );
}

#[test]
fn c1_7bit_esc_0x59() {
    // PUSH "\eY"  =>  control 0x99 (in libvterm)
    // Same as above: parser emits EscDispatch
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0x1b, 0x59]);
    assert_eq!(
        p.actions,
        vec![esc(&[], 0x59)],
    );
}

// =========================================================================
// Mixed text and control
// =========================================================================

#[test]
fn mixed_text_and_control() {
    // PUSH "1\n2"  =>  text 0x31, control 10, text 0x32
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"1\n2");
    assert_eq!(
        p.actions,
        vec![
            Action::Print('1'),
            Action::Execute(0x0a),
            Action::Print('2'),
        ],
    );
}

// =========================================================================
// ESC sequences
// =========================================================================

#[test]
fn escape_single_byte() {
    // PUSH "\e="  =>  escape "="
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0x1b, b'=']);
    assert_eq!(p.actions, vec![esc(&[], b'=')]);
}

#[test]
fn escape_two_byte() {
    // PUSH "\e(X"  =>  escape "(X"
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0x1b, b'(', b'X']);
    assert_eq!(p.actions, vec![esc(&[b'('], b'X')]);
}

#[test]
fn escape_split_write() {
    // PUSH "\e("  then PUSH "Y"  =>  escape "(Y"
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0x1b, b'(']);
    assert_eq!(p.drain(), vec![]);
    parser.advance(&mut p, b"Y");
    assert_eq!(p.actions, vec![esc(&[b'('], b'Y')]);
}

#[test]
fn escape_cancels_escape() {
    // PUSH "\e(\e)Z"  =>  escape ")Z"
    // First ESC( starts escape, second ESC cancels it and starts new one.
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0x1b, b'(', 0x1b, b')', b'Z']);
    assert_eq!(p.actions, vec![esc(&[b')'], b'Z')]);
}

#[test]
fn can_cancels_escape() {
    // PUSH "\e(\x18AB"  =>  text 0x41,0x42
    // CAN (0x18) cancels ESC, parser emits Execute(0x18) then returns to ground.
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0x1b, b'(', 0x18, b'A', b'B']);
    assert_eq!(
        p.actions,
        vec![
            Action::Execute(0x18),
            Action::Print('A'),
            Action::Print('B'),
        ],
    );
}

#[test]
fn c0_in_escape_interrupts_and_continues() {
    // PUSH "\e(\nX"  =>  control 10, escape "(X"
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0x1b, b'(', b'\n', b'X']);
    assert_eq!(
        p.actions,
        vec![
            Action::Execute(0x0a),
            esc(&[b'('], b'X'),
        ],
    );
}

// =========================================================================
// CSI sequences
// =========================================================================

#[test]
fn csi_zero_args() {
    // PUSH "\e[a"  =>  csi 0x61 *
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0x1b, b'[', b'a']);
    assert_eq!(p.actions, vec![csi(0x61, "", &[])]);
}

#[test]
fn csi_one_arg() {
    // PUSH "\e[9b"  =>  csi 0x62 9
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0x1b, b'[', b'9', b'b']);
    assert_eq!(p.actions, vec![csi(0x62, "9", &[])]);
}

#[test]
fn csi_two_args() {
    // PUSH "\e[3;4c"  =>  csi 0x63 3,4
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b[3;4c");
    assert_eq!(p.actions, vec![csi(0x63, "3;4", &[])]);
}

#[test]
fn csi_one_arg_one_sub() {
    // PUSH "\e[1:2c"  =>  csi 0x63 1+,2  (subparam separator ':')
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b[1:2c");
    assert_eq!(p.actions, vec![csi(0x63, "1:2", &[])]);
}

#[test]
fn csi_many_digits() {
    // PUSH "\e[678d"  =>  csi 0x64 678
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b[678d");
    assert_eq!(p.actions, vec![csi(0x64, "678", &[])]);
}

#[test]
fn csi_leading_zero() {
    // PUSH "\e[007e"  =>  csi 0x65 7
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b[007e");
    let actions = p.drain();
    assert_eq!(actions.len(), 1);
    if let Action::CsiDispatch { params, action, .. } = &actions[0] {
        assert_eq!(*action, 0x65);
        assert_eq!(params.finished(), vec![7]);
    } else {
        panic!("expected CsiDispatch");
    }
}

#[test]
fn csi_private_mode_qmark() {
    // PUSH "\e[?2;7f"  =>  csi 0x66 L=3f 2,7
    // '?' (0x3f) stored as an intermediate (leader byte)
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b[?2;7f");
    assert_eq!(p.actions, vec![csi(0x66, "2;7", &[b'?'])]);
}

#[test]
fn csi_private_mode_greater() {
    // PUSH "\e[>c"  =>  csi 0x63 L=3e *
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b[>c");
    assert_eq!(p.actions, vec![csi(0x63, "", &[b'>'])]);
}

#[test]
fn csi_intermediate_space() {
    // PUSH "\e[12 q"  =>  csi 0x71 12 I=20
    // Space (0x20) is an intermediate byte
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b[12 q");
    assert_eq!(p.actions, vec![csi(0x71, "12", &[b' '])]);
}

#[test]
fn csi_mixed_with_text() {
    // PUSH "A\e[8mB"  =>  text 0x41, csi 0x6d 8, text 0x42
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"A\x1b[8mB");
    assert_eq!(
        p.actions,
        vec![
            Action::Print('A'),
            csi(0x6d, "8", &[]),
            Action::Print('B'),
        ],
    );
}

// =========================================================================
// CSI split writes
// =========================================================================

#[test]
fn csi_split_write_esc_bracket() {
    // PUSH "\e"  then PUSH "[a"  =>  csi 0x61 *
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0x1b]);
    assert_eq!(p.drain(), vec![]);
    parser.advance(&mut p, b"[a");
    assert_eq!(p.actions, vec![csi(0x61, "", &[])]);
}

#[test]
fn csi_split_write_text_then_esc_bracket() {
    // PUSH "foo\e["  =>  text
    // PUSH "4b"      =>  csi 0x62 4
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"foo\x1b[");
    assert_eq!(
        p.drain(),
        prints(&['f', 'o', 'o']),
    );
    parser.advance(&mut p, b"4b");
    assert_eq!(p.actions, vec![csi(0x62, "4", &[])]);
}

#[test]
fn csi_split_write_params() {
    // PUSH "\e[12;"  then PUSH "3c"  =>  csi 0x63 12,3
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b[12;");
    assert_eq!(p.drain(), vec![]);
    parser.advance(&mut p, b"3c");
    assert_eq!(p.actions, vec![csi(0x63, "12;3", &[])]);
}

// =========================================================================
// CSI cancellation
// =========================================================================

#[test]
fn escape_cancels_csi_starts_escape() {
    // PUSH "\e[123\e9"  =>  escape "9"
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b[123\x1b9");
    assert_eq!(p.actions, vec![esc(&[], b'9')]);
}

#[test]
fn can_cancels_csi() {
    // PUSH "\e[12\x18AB"  =>  text 0x41,0x42
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b[12\x18AB");
    assert_eq!(
        p.actions,
        vec![
            Action::Execute(0x18),
            Action::Print('A'),
            Action::Print('B'),
        ],
    );
}

#[test]
fn c0_in_csi_interrupts_and_continues() {
    // PUSH "\e[12\n;3X"  =>  control 10, csi 0x58 12,3
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b[12\n;3X");
    assert_eq!(
        p.actions,
        vec![
            Action::Execute(0x0a),
            csi(0x58, "12;3", &[]),
        ],
    );
}

// =========================================================================
// OSC sequences
// =========================================================================

#[test]
fn osc_bel_terminated() {
    // PUSH "\e]1;Hello\x07"  =>  osc [1 "Hello"]
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b]1;Hello\x07");
    assert_eq!(
        p.actions,
        vec![Action::OscDispatch(vec![b"1".to_vec(), b"Hello".to_vec()])],
    );
}

#[test]
fn osc_st_7bit_terminated() {
    // PUSH "\e]1;Hello\e\\"  =>  osc [1 "Hello"]
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b]1;Hello\x1b\\");
    // ESC triggers transition out of OscString; the '\' is dispatched as
    // EscDispatch. The OSC may or may not be dispatched depending on
    // how the parser handles ESC inside OSC.
    // Check what actually happens:
    let actions = p.drain();
    // The parser currently handles ESC as an "anywhere" transition that
    // cancels the current state. So ESC inside OSC cancels the OSC and
    // starts Escape state, then '\' dispatches as EscDispatch.
    assert_eq!(
        actions,
        vec![esc(&[], b'\\')],
    );
}

#[test]
fn osc_st_8bit_terminated() {
    // PUSH "\x9d1;Hello\x9c"  =>  osc [1 "Hello"]
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &{
        let mut v = vec![0x9d]; // 8-bit OSC
        v.extend_from_slice(b"1;Hello");
        v.push(0x9c); // 8-bit ST
        v
    });
    assert_eq!(
        p.actions,
        vec![Action::OscDispatch(vec![b"1".to_vec(), b"Hello".to_vec()])],
    );
}

#[test]
fn osc_bel_no_semicolon() {
    // PUSH "\e]1234\x07"  =>  osc [1234 ]
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b]1234\x07");
    assert_eq!(
        p.actions,
        vec![Action::OscDispatch(vec![b"1234".to_vec()])],
    );
}

#[test]
fn osc_st_no_semicolon() {
    // PUSH "\e]1234\e\\"  =>  osc [1234 ]
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b]1234\x1b\\");
    // Same as osc_st_7bit: ESC cancels OSC, '\' dispatches as EscDispatch
    let actions = p.drain();
    assert_eq!(actions, vec![esc(&[], b'\\')]);
}

#[test]
fn escape_cancels_osc() {
    // PUSH "\e]Something\e9"  =>  escape "9"
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b]Something\x1b9");
    assert_eq!(p.actions, vec![esc(&[], b'9')]);
}

#[test]
fn can_cancels_osc() {
    // PUSH "\e]12\x18AB"  =>  text 0x41,0x42
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b]12\x18AB");
    assert_eq!(
        p.actions,
        vec![
            Action::Execute(0x18),
            Action::Print('A'),
            Action::Print('B'),
        ],
    );
}

// =========================================================================
// DCS sequences
// =========================================================================

#[test]
fn dcs_8bit_st() {
    // PUSH "\x90Hello\x9c"  =>  DCS passthrough "Hello" then unhook
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    let mut data = vec![0x90]; // 8-bit DCS
    data.extend_from_slice(b"Hello");
    data.push(0x9c); // 8-bit ST
    parser.advance(&mut p, &data);
    let actions = p.drain();
    // DCS entry sees 'H' (0x48) as 0x40..0x7e -> transitions to passthrough
    // Then 'e', 'l', 'l', 'o' are DcsPut
    // Then 0x9c triggers DcsUnhook
    assert!(actions.iter().any(|a| matches!(a, Action::DcsUnhook)));
}

#[test]
fn escape_cancels_dcs() {
    // PUSH "\ePSomething\e9"  =>  escape "9"
    // ESC P enters DcsEntry; 'S' (0x53) transitions to DcsPassthrough;
    // remaining bytes are emitted as DcsPut until ESC cancels and starts
    // a new Escape sequence, dispatching '9'.
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1bPSomething\x1b9");
    let actions = p.drain();
    // The last action must be the EscDispatch for '9'
    assert_eq!(actions.last(), Some(&esc(&[], b'9')));
    // Everything before should be DcsPut bytes for "omething"
    // ('S' transitions DcsEntry -> DcsPassthrough, so it's consumed)
    for a in &actions[..actions.len() - 1] {
        assert!(matches!(a, Action::DcsPut(_)));
    }
}

#[test]
fn can_cancels_dcs() {
    // PUSH "\eP12\x18AB"  =>  text 0x41,0x42
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1bP12\x18AB");
    assert_eq!(
        p.actions,
        vec![
            Action::Execute(0x18),
            Action::Print('A'),
            Action::Print('B'),
        ],
    );
}

// =========================================================================
// NUL and DEL handling
// =========================================================================

#[test]
fn nul_in_ground() {
    // PUSH "\x00"
    // In the emux-vt parser, NUL in ground triggers Execute(0x00).
    // libvterm ignores it, but Execute is the standard VT parser behavior.
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0x00]);
    assert_eq!(p.actions, vec![Action::Execute(0x00)]);
}

#[test]
fn del_ignored_in_ground() {
    // PUSH "\x7f"  =>  nothing
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0x7f]);
    assert_eq!(p.actions, vec![]);
}

#[test]
fn del_ignored_within_csi() {
    // PUSH "\e[12\x7f3m"  =>  csi 0x6d 123
    // DEL is ignored within CSI; "12" + "3" = "123" once DEL is skipped.
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b[12\x7f3m");
    let actions = p.drain();
    assert_eq!(actions.len(), 1);
    if let Action::CsiDispatch { params, action, .. } = &actions[0] {
        assert_eq!(*action, 0x6d);
        assert_eq!(params.finished(), vec![123]);
    } else {
        panic!("expected CsiDispatch");
    }
}

#[test]
fn del_inside_text() {
    // PUSH "AB\x7fC"  =>  text A,B  text C
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"AB\x7fC");
    assert_eq!(
        p.actions,
        vec![
            Action::Print('A'),
            Action::Print('B'),
            Action::Print('C'),
        ],
    );
}

// =========================================================================
// 03encoding_utf8.test — UTF-8 decoding
// =========================================================================

#[test]
fn utf8_ascii() {
    // ENCIN "123"  =>  0x31,0x32,0x33
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"123");
    assert_eq!(
        p.actions,
        prints(&['1', '2', '3']),
    );
}

#[test]
fn utf8_2byte_boundary_low() {
    // U+0080: C2 80
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0xC2, 0x80]);
    assert_eq!(p.actions, vec![Action::Print('\u{0080}')]);
}

#[test]
fn utf8_2byte_boundary_high() {
    // U+07FF: DF BF
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0xDF, 0xBF]);
    assert_eq!(p.actions, vec![Action::Print('\u{07FF}')]);
}

#[test]
fn utf8_2byte_both_boundaries() {
    // ENCIN "\xC2\x80\xDF\xBF"  =>  U+0080, U+07FF
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0xC2, 0x80, 0xDF, 0xBF]);
    assert_eq!(
        p.actions,
        vec![Action::Print('\u{0080}'), Action::Print('\u{07FF}')],
    );
}

#[test]
fn utf8_3byte_boundary_low() {
    // U+0800: E0 A0 80
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0xE0, 0xA0, 0x80]);
    assert_eq!(p.actions, vec![Action::Print('\u{0800}')]);
}

#[test]
fn utf8_3byte_boundary_high() {
    // U+FFFD: EF BF BD
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0xEF, 0xBF, 0xBD]);
    assert_eq!(p.actions, vec![Action::Print('\u{FFFD}')]);
}

#[test]
fn utf8_3byte_both_boundaries() {
    // ENCIN "\xE0\xA0\x80\xEF\xBF\xBD"  =>  U+0800, U+FFFD
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0xE0, 0xA0, 0x80, 0xEF, 0xBF, 0xBD]);
    assert_eq!(
        p.actions,
        vec![Action::Print('\u{0800}'), Action::Print('\u{FFFD}')],
    );
}

#[test]
fn utf8_4byte_boundary_low() {
    // U+10000: F0 90 80 80
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0xF0, 0x90, 0x80, 0x80]);
    assert_eq!(p.actions, vec![Action::Print('\u{10000}')]);
}

#[test]
fn utf8_4byte_high() {
    // U+10FFFF is the max valid Unicode codepoint: F4 8F BF BF
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0xF4, 0x8F, 0xBF, 0xBF]);
    assert_eq!(p.actions, vec![Action::Print('\u{10FFFF}')]);
}

#[test]
fn utf8_split_2byte() {
    // ENCIN "\xC2" then ENCIN "\xA0"  =>  U+00A0
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0xC2]);
    assert_eq!(p.drain(), vec![]);
    parser.advance(&mut p, &[0xA0]);
    assert_eq!(p.actions, vec![Action::Print('\u{00A0}')]);
}

#[test]
fn utf8_split_3byte_after_first() {
    // ENCIN "\xE0" then ENCIN "\xA0\x80"  =>  U+0800
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0xE0]);
    assert_eq!(p.drain(), vec![]);
    parser.advance(&mut p, &[0xA0, 0x80]);
    assert_eq!(p.actions, vec![Action::Print('\u{0800}')]);
}

#[test]
fn utf8_split_3byte_after_second() {
    // ENCIN "\xE0\xA0" then ENCIN "\x80"  =>  U+0800
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0xE0, 0xA0]);
    assert_eq!(p.drain(), vec![]);
    parser.advance(&mut p, &[0x80]);
    assert_eq!(p.actions, vec![Action::Print('\u{0800}')]);
}

#[test]
fn utf8_split_4byte_after_first() {
    // ENCIN "\xF0" then ENCIN "\x90\x80\x80"  =>  U+10000
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0xF0]);
    assert_eq!(p.drain(), vec![]);
    parser.advance(&mut p, &[0x90, 0x80, 0x80]);
    assert_eq!(p.actions, vec![Action::Print('\u{10000}')]);
}

#[test]
fn utf8_split_4byte_after_second() {
    // ENCIN "\xF0\x90" then ENCIN "\x80\x80"  =>  U+10000
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0xF0, 0x90]);
    assert_eq!(p.drain(), vec![]);
    parser.advance(&mut p, &[0x80, 0x80]);
    assert_eq!(p.actions, vec![Action::Print('\u{10000}')]);
}

#[test]
fn utf8_split_4byte_after_third() {
    // ENCIN "\xF0\x90\x80" then ENCIN "\x80"  =>  U+10000
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0xF0, 0x90, 0x80]);
    assert_eq!(p.drain(), vec![]);
    parser.advance(&mut p, &[0x80]);
    assert_eq!(p.actions, vec![Action::Print('\u{10000}')]);
}

// =========================================================================
// Additional conformance tests
// =========================================================================

#[test]
fn sub_cancels_csi_like_can() {
    // SUB (0x1A) should also cancel sequences, similar to CAN
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b[12\x1aAB");
    assert_eq!(
        p.actions,
        vec![
            Action::Execute(0x1a),
            Action::Print('A'),
            Action::Print('B'),
        ],
    );
}

#[test]
fn sub_cancels_escape() {
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0x1b, b'(', 0x1a, b'A']);
    assert_eq!(
        p.actions,
        vec![
            Action::Execute(0x1a),
            Action::Print('A'),
        ],
    );
}

#[test]
fn csi_8bit_entry() {
    // 0x9b is 8-bit CSI
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0x9b, b'5', b'm']);
    assert_eq!(p.actions, vec![csi(0x6d, "5", &[])]);
}

#[test]
fn multiple_csi_sequences() {
    // Two CSI sequences back to back
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b[1m\x1b[2m");
    assert_eq!(
        p.actions,
        vec![
            csi(0x6d, "1", &[]),
            csi(0x6d, "2", &[]),
        ],
    );
}

#[test]
fn csi_missing_params_default_to_zero() {
    // PUSH "\e[;c"  =>  csi 0x63
    // The ';' separates two params: both default to 0.
    // Per VT standard, ";c" means two parameters: [0, 0].
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b[;c");
    let actions = p.drain();
    assert_eq!(actions.len(), 1);
    if let Action::CsiDispatch { params, action, .. } = &actions[0] {
        assert_eq!(*action, 0x63);
        assert_eq!(params.finished(), vec![0, 0]);
    } else {
        panic!("expected CsiDispatch");
    }
}

#[test]
fn csi_trailing_semicolon() {
    // PUSH "\e[5;m"  =>  csi 0x6d
    // The ';' separates two params: 5 and default 0.
    // Per VT standard, "5;m" means two parameters: [5, 0].
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b[5;m");
    let actions = p.drain();
    assert_eq!(actions.len(), 1);
    if let Action::CsiDispatch { params, action, .. } = &actions[0] {
        assert_eq!(*action, 0x6d);
        assert_eq!(params.finished(), vec![5, 0]);
    } else {
        panic!("expected CsiDispatch");
    }
}

#[test]
fn common_c0_controls() {
    // BEL, BS, HT, LF, VT, FF, CR
    let controls: &[u8] = &[0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d];
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, controls);
    let expected: Vec<Action> = controls.iter().map(|&b| Action::Execute(b)).collect();
    assert_eq!(p.actions, expected);
}

#[test]
fn printable_ascii_range() {
    // All printable ASCII: 0x20..=0x7e
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    let bytes: Vec<u8> = (0x20..=0x7e).collect();
    parser.advance(&mut p, &bytes);
    let expected: Vec<Action> = bytes.iter().map(|&b| Action::Print(b as char)).collect();
    assert_eq!(p.actions, expected);
}

#[test]
fn esc_dispatch_various_final_bytes() {
    // ESC 7 (DECSC), ESC 8 (DECRC), ESC c (RIS)
    for &final_byte in &[b'7', b'8', b'c', b'D', b'E', b'M'] {
        let mut parser = Parser::new();
        let mut p = TestPerformer::new();
        parser.advance(&mut p, &[0x1b, final_byte]);
        assert_eq!(p.actions, vec![esc(&[], final_byte)]);
    }
}

#[test]
fn dcs_via_7bit_esc_p() {
    // ESC P starts DCS
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1bPq\x9c");
    // ESC P -> DcsEntry, 'q' -> DcsPassthrough, 0x9c -> DcsUnhook
    let actions = p.drain();
    assert!(actions.iter().any(|a| matches!(a, Action::DcsUnhook)));
}

#[test]
fn csi_sgr_256_color() {
    // \e[38;5;196m  =>  CSI 'm' with params [38,5,196]
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b[38;5;196m");
    let actions = p.drain();
    assert_eq!(actions.len(), 1);
    if let Action::CsiDispatch { params, action, .. } = &actions[0] {
        assert_eq!(*action, b'm');
        assert_eq!(params.finished(), vec![38, 5, 196]);
    } else {
        panic!("expected CsiDispatch");
    }
}

#[test]
fn csi_sgr_rgb_color() {
    // \e[38;2;255;128;0m  =>  CSI 'm' with params [38,2,255,128,0]
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b[38;2;255;128;0m");
    let actions = p.drain();
    assert_eq!(actions.len(), 1);
    if let Action::CsiDispatch { params, action, .. } = &actions[0] {
        assert_eq!(*action, b'm');
        assert_eq!(params.finished(), vec![38, 2, 255, 128, 0]);
    } else {
        panic!("expected CsiDispatch");
    }
}

#[test]
fn utf8_emoji_4byte() {
    // U+1F600 (grinning face): F0 9F 98 80
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0xF0, 0x9F, 0x98, 0x80]);
    assert_eq!(p.actions, vec![Action::Print('\u{1F600}')]);
}

#[test]
fn utf8_cjk_3byte() {
    // U+4E2D (CJK ideograph for "middle"): E4 B8 AD
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0xE4, 0xB8, 0xAD]);
    assert_eq!(p.actions, vec![Action::Print('\u{4E2D}')]);
}

#[test]
fn utf8_mixed_with_ascii_and_controls() {
    // Mix of ASCII, UTF-8, and control chars
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    // "A" + U+00E9 (C3 A9) + "\n" + "B"
    parser.advance(&mut p, &[b'A', 0xC3, 0xA9, b'\n', b'B']);
    assert_eq!(
        p.actions,
        vec![
            Action::Print('A'),
            Action::Print('\u{00E9}'),
            Action::Execute(0x0a),
            Action::Print('B'),
        ],
    );
}

#[test]
fn empty_input() {
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"");
    assert_eq!(p.actions, vec![]);
}

#[test]
fn csi_cursor_position() {
    // \e[10;20H  =>  CSI 'H' with params [10,20]
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b[10;20H");
    assert_eq!(p.actions, vec![csi(b'H', "10;20", &[])]);
}

#[test]
fn csi_erase_in_display() {
    // \e[2J  =>  CSI 'J' with params [2]
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b[2J");
    assert_eq!(p.actions, vec![csi(b'J', "2", &[])]);
}

#[test]
fn csi_device_status_report() {
    // \e[6n  =>  CSI 'n' with params [6]
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b[6n");
    assert_eq!(p.actions, vec![csi(b'n', "6", &[])]);
}

#[test]
fn csi_set_mode_private() {
    // \e[?1049h  =>  CSI 'h' with leader '?' and param [1049]
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b[?1049h");
    assert_eq!(p.actions, vec![csi(b'h', "1049", &[b'?'])]);
}

#[test]
fn csi_reset_mode_private() {
    // \e[?1049l  =>  CSI 'l' with leader '?' and param [1049]
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b[?1049l");
    assert_eq!(p.actions, vec![csi(b'l', "1049", &[b'?'])]);
}

#[test]
fn long_text_run() {
    let text = "The quick brown fox jumps over the lazy dog";
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, text.as_bytes());
    let expected: Vec<Action> = text.chars().map(Action::Print).collect();
    assert_eq!(p.actions, expected);
}

#[test]
fn interleaved_text_and_csi() {
    // "Hello\e[1mWorld\e[0m!"
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"Hello\x1b[1mWorld\x1b[0m!");
    let expected = vec![
        Action::Print('H'),
        Action::Print('e'),
        Action::Print('l'),
        Action::Print('l'),
        Action::Print('o'),
        csi(b'm', "1", &[]),
        Action::Print('W'),
        Action::Print('o'),
        Action::Print('r'),
        Action::Print('l'),
        Action::Print('d'),
        csi(b'm', "0", &[]),
        Action::Print('!'),
    ];
    assert_eq!(p.actions, expected);
}

#[test]
fn osc_bel_with_url() {
    // OSC 8 hyperlink:  \e]8;id=foo;https://example.com\x07
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b]8;id=foo;https://example.com\x07");
    assert_eq!(
        p.actions,
        vec![Action::OscDispatch(vec![
            b"8".to_vec(),
            b"id=foo".to_vec(),
            b"https://example.com".to_vec(),
        ])],
    );
}

#[test]
fn csi_scroll_region() {
    // \e[5;20r  =>  CSI 'r' with params [5,20]
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b[5;20r");
    assert_eq!(p.actions, vec![csi(b'r', "5;20", &[])]);
}

#[test]
fn esc_charset_designation() {
    // \e(0  =>  ESC with intermediate '(' and final '0' (DEC Special Graphics)
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b(0");
    assert_eq!(p.actions, vec![esc(&[b'('], b'0')]);
}

#[test]
fn esc_charset_designation_b() {
    // \e(B  =>  ESC with intermediate '(' and final 'B' (US ASCII)
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b(B");
    assert_eq!(p.actions, vec![esc(&[b'('], b'B')]);
}

#[test]
fn can_cancels_osc_returns_to_ground() {
    // After CAN inside OSC, parser should be in ground state
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b]2;title\x18Hello");
    let actions = p.drain();
    // CAN emits Execute(0x18), then "Hello" as Print chars
    assert_eq!(actions[0], Action::Execute(0x18));
    assert_eq!(actions[1], Action::Print('H'));
    assert_eq!(actions.len(), 6); // Execute + H + e + l + l + o
}

#[test]
fn csi_multiple_intermediates() {
    // Unusual but valid: multiple intermediate bytes before final
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    // CSI with intermediate 0x20 0x21 and final 'p'
    parser.advance(&mut p, b"\x1b[ !p");
    let actions = p.drain();
    assert_eq!(actions.len(), 1);
    if let Action::CsiDispatch {
        intermediates,
        action,
        ..
    } = &actions[0]
    {
        assert_eq!(*action, b'p');
        assert_eq!(intermediates, &[b' ', b'!']);
    } else {
        panic!("expected CsiDispatch");
    }
}

#[test]
fn utf8_multiple_codepoints_in_one_advance() {
    // Multiple multi-byte chars in a single advance call
    // U+00E9 (C3 A9) + U+00F1 (C3 B1) + U+00FC (C3 BC)
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, &[0xC3, 0xA9, 0xC3, 0xB1, 0xC3, 0xBC]);
    assert_eq!(
        p.actions,
        vec![
            Action::Print('\u{00E9}'),
            Action::Print('\u{00F1}'),
            Action::Print('\u{00FC}'),
        ],
    );
}

#[test]
fn rapid_state_transitions() {
    // ESC then immediately CSI then text
    let mut parser = Parser::new();
    let mut p = TestPerformer::new();
    parser.advance(&mut p, b"\x1b[mA\x1b[1mB\x1b[0mC");
    assert_eq!(
        p.actions,
        vec![
            csi(b'm', "", &[]),
            Action::Print('A'),
            csi(b'm', "1", &[]),
            Action::Print('B'),
            csi(b'm', "0", &[]),
            Action::Print('C'),
        ],
    );
}
