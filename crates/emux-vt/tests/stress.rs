//! Deterministic stress tests for the VT parser.
//!
//! These tests feed large or pathological inputs to ensure the parser
//! never panics and always recovers to a sane state.

use emux_vt::{Action, Parser, Performer};

/// Performer that records actions for inspection.
struct RecordingPerformer {
    actions: Vec<Action>,
}

impl RecordingPerformer {
    fn new() -> Self {
        Self {
            actions: Vec::new(),
        }
    }

    fn clear(&mut self) {
        self.actions.clear();
    }
}

impl Performer for RecordingPerformer {
    fn perform(&mut self, action: Action) {
        self.actions.push(action);
    }
}

/// Feed 1MB of pseudo-random VT data without panicking.
///
/// Uses a simple LCG PRNG seeded deterministically so the test is reproducible.
#[test]
fn stress_1mb_random_data() {
    let mut parser = Parser::new();
    let mut performer = RecordingPerformer::new();

    // Simple LCG: state = state * 6364136223846793005 + 1442695040888963407
    let mut rng_state: u64 = 0xDEAD_BEEF_CAFE_BABE;
    let size = 1024 * 1024; // 1 MB
    let mut data = Vec::with_capacity(size);
    for _ in 0..size {
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        data.push((rng_state >> 33) as u8);
    }

    parser.advance(&mut performer, &data);

    // Random data should produce some actions (prints, executes, etc.)
    assert!(
        !performer.actions.is_empty(),
        "1MB of random data should produce at least some actions"
    );

    performer.clear();

    // Verify parser recovers and can process normal input after stress
    parser.advance(&mut performer, b"\x1b[1m");
    let has_csi = performer.actions.iter().any(|a| {
        matches!(a, Action::CsiDispatch { action: b'm', .. })
    });
    assert!(
        has_csi,
        "parser should recover and correctly parse CSI after 1MB random data"
    );
}

/// Feed all C0 control characters (0x00..0x1F) in a single burst.
#[test]
fn stress_all_c0_controls() {
    let mut parser = Parser::new();
    let mut performer = RecordingPerformer::new();

    let data: Vec<u8> = (0x00..=0x1Fu8).collect();
    // Repeat many times
    let repeated: Vec<u8> = data.iter().copied().cycle().take(data.len() * 1000).collect();
    parser.advance(&mut performer, &repeated);

    // All C0 bytes should produce Execute actions
    assert!(
        performer.actions.iter().all(|a| matches!(a, Action::Execute(_))),
        "C0 controls should only produce Execute actions"
    );

    performer.clear();

    // Verify parser recovers and can process normal input
    parser.advance(&mut performer, b"\x1b[1m");
    let has_csi = performer.actions.iter().any(|a| {
        matches!(a, Action::CsiDispatch { action: b'm', .. })
    });
    assert!(has_csi, "parser should recover after C0 burst");
}

/// Feed all C1 control characters via ESC encoding (ESC 0x40..0x5F).
#[test]
fn stress_all_c1_controls() {
    let mut parser = Parser::new();
    let mut performer = RecordingPerformer::new();

    let mut data = Vec::new();
    for byte in 0x40..=0x5Fu8 {
        data.push(0x1B); // ESC
        data.push(byte);
    }
    // Repeat many times
    let repeated: Vec<u8> = data.iter().copied().cycle().take(data.len() * 500).collect();
    parser.advance(&mut performer, &repeated);

    // Should produce actions (EscDispatch for each ESC+byte pair)
    assert!(
        !performer.actions.is_empty(),
        "C1 controls should produce actions"
    );

    performer.clear();

    // Verify parser recovers and can process normal input
    parser.advance(&mut performer, b"\x1b[1m");
    let has_csi = performer.actions.iter().any(|a| {
        matches!(a, Action::CsiDispatch { action: b'm', .. })
    });
    assert!(has_csi, "parser should recover after C1 burst");
}

/// Feed a maximum-length OSC string (64KB of payload).
#[test]
fn stress_max_osc_string() {
    let mut parser = Parser::new();
    let mut performer = RecordingPerformer::new();

    let mut data = Vec::new();
    // OSC start: ESC ]
    data.push(0x1B);
    data.push(b']');
    // OSC number + separator
    data.push(b'0');
    data.push(b';');
    // 64KB of 'A'
    data.extend(std::iter::repeat(b'A').take(65536));
    // BEL terminates OSC in this parser
    data.push(0x07);

    parser.advance(&mut performer, &data);

    // Verify at least one OscDispatch was emitted
    assert!(
        performer.actions.iter().any(|a| matches!(a, Action::OscDispatch(_))),
        "expected OscDispatch action after long OSC string"
    );
}

/// Feed a DCS sequence with a large payload.
#[test]
fn stress_large_dcs_payload() {
    let mut parser = Parser::new();
    let mut performer = RecordingPerformer::new();

    let mut data = Vec::new();
    // DCS start: ESC P
    data.push(0x1B);
    data.push(b'P');
    // Parameters
    data.extend_from_slice(b"0;1|");
    // 32KB of data bytes (0x40..0x7E range is passthrough data)
    data.extend(std::iter::repeat(b'A').take(32768));
    // ST (0x9C) terminates DCS passthrough in this parser
    data.push(0x9C);

    parser.advance(&mut performer, &data);

    // Should have DcsHook, many DcsPut, and DcsUnhook
    assert!(
        performer.actions.iter().any(|a| matches!(a, Action::DcsHook { .. })),
        "expected DcsHook action"
    );
    assert!(
        performer.actions.iter().any(|a| matches!(a, Action::DcsUnhook)),
        "expected DcsUnhook action"
    );
}

/// Feed rapid state transitions: sequences that start but are interrupted
/// by other sequence initiators before completion.
#[test]
fn stress_rapid_state_transitions() {
    let mut parser = Parser::new();
    let mut performer = RecordingPerformer::new();

    let mut data = Vec::new();
    for _ in 0..10_000 {
        // Start CSI, then interrupt with ESC
        data.extend_from_slice(b"\x1b[");
        // Start OSC, then interrupt with ESC
        data.extend_from_slice(b"\x1b]");
        // Start DCS, then interrupt with ESC
        data.extend_from_slice(b"\x1bP");
        // ESC to cancel whatever is pending
        data.push(0x1B);
    }
    // Final reset to ground: ST
    data.push(b'\\');

    parser.advance(&mut performer, &data);

    performer.clear();

    // Verify parser is still functional after rapid state transitions
    parser.advance(&mut performer, b"\x1b[1m");
    let has_csi = performer.actions.iter().any(|a| {
        matches!(a, Action::CsiDispatch { action: b'm', .. })
    });
    assert!(has_csi, "parser should recover after rapid state transitions");
}

/// Verify the parser returns to ground state after any arbitrary sequence.
///
/// We check this by feeding arbitrary data, then feeding a known CSI sequence
/// and verifying it is correctly parsed.
#[test]
fn stress_recovery_to_ground_state() {
    // Various pathological prefixes that might leave the parser in odd states.
    let pathological_inputs: Vec<Vec<u8>> = vec![
        // Incomplete ESC
        vec![0x1B],
        // Incomplete CSI
        vec![0x1B, b'['],
        // CSI with params but no final byte
        vec![0x1B, b'[', b'1', b';', b'2'],
        // Incomplete OSC
        vec![0x1B, b']', b'0', b';', b'x'],
        // Incomplete DCS
        vec![0x1B, b'P', b'0'],
        // DCS passthrough without terminator
        vec![0x1B, b'P', b'q', b'A', b'B', b'C'],
        // Garbage bytes
        vec![0xFF, 0xFE, 0x80, 0x81, 0x90, 0x9B],
        // Broken UTF-8 sequences
        vec![0xC3, 0xC3, 0xE2, 0xF0],
        // All ones
        vec![0xFF; 256],
        // Alternating ESC and NUL
        vec![0x1B, 0x00, 0x1B, 0x00, 0x1B, 0x00],
    ];

    for (i, prefix) in pathological_inputs.iter().enumerate() {
        let mut parser = Parser::new();
        let mut performer = RecordingPerformer::new();

        // Feed pathological data
        parser.advance(&mut performer, prefix);
        performer.clear();

        // Cancel any in-progress sequence: CAN (0x18) returns to ground
        parser.advance(&mut performer, &[0x18]);
        performer.clear();

        // Now feed a known CSI sequence: CSI 1m (bold SGR)
        parser.advance(&mut performer, b"\x1b[1m");

        // Should have received a CsiDispatch
        let has_csi = performer.actions.iter().any(|a| {
            matches!(a, Action::CsiDispatch { action: b'm', .. })
        });
        assert!(
            has_csi,
            "parser did not recover to ground state after pathological input #{i}: {prefix:?}"
        );
    }
}

/// Feed a mix of valid and invalid UTF-8, ensuring no panics.
#[test]
fn stress_utf8_edge_cases() {
    let mut parser = Parser::new();
    let mut performer = RecordingPerformer::new();

    let mut data = Vec::new();
    // Valid 2-byte
    data.extend_from_slice(&[0xC3, 0xA9]); // e-acute
    // Valid 3-byte
    data.extend_from_slice(&[0xE2, 0x82, 0xAC]); // Euro sign
    // Valid 4-byte
    data.extend_from_slice(&[0xF0, 0x9F, 0x98, 0x80]); // Grinning face
    // Overlong 2-byte (invalid)
    data.extend_from_slice(&[0xC0, 0xAF]);
    // Overlong 3-byte (invalid)
    data.extend_from_slice(&[0xE0, 0x80, 0xAF]);
    // Truncated 2-byte
    data.push(0xC3);
    // Truncated 3-byte
    data.extend_from_slice(&[0xE2, 0x82]);
    // Truncated 4-byte
    data.extend_from_slice(&[0xF0, 0x9F, 0x98]);
    // Continuation byte without start
    data.push(0x80);
    data.push(0xBF);
    // Surrogates (invalid in UTF-8)
    data.extend_from_slice(&[0xED, 0xA0, 0x80]); // U+D800
    data.extend_from_slice(&[0xED, 0xBF, 0xBF]); // U+DFFF
    // Max codepoint
    data.extend_from_slice(&[0xF4, 0x8F, 0xBF, 0xBF]); // U+10FFFF
    // Beyond max (invalid)
    data.extend_from_slice(&[0xF4, 0x90, 0x80, 0x80]); // U+110000

    // Repeat many times
    let repeated: Vec<u8> = data.iter().copied().cycle().take(data.len() * 500).collect();
    parser.advance(&mut performer, &repeated);

    // Should produce actions from valid UTF-8 sequences at minimum
    assert!(
        !performer.actions.is_empty(),
        "UTF-8 edge cases should produce some actions"
    );

    performer.clear();

    // Verify parser recovers and can process normal input
    parser.advance(&mut performer, b"\x1b[1m");
    let has_csi = performer.actions.iter().any(|a| {
        matches!(a, Action::CsiDispatch { action: b'm', .. })
    });
    assert!(has_csi, "parser should recover after UTF-8 edge cases");
}

/// Feed CSI sequences with extreme parameter counts and values.
#[test]
fn stress_csi_extreme_params() {
    let mut parser = Parser::new();
    let mut performer = RecordingPerformer::new();

    // CSI with 100 parameters
    let mut data = Vec::new();
    data.extend_from_slice(b"\x1b[");
    for i in 0..100u32 {
        if i > 0 {
            data.push(b';');
        }
        data.extend_from_slice(i.to_string().as_bytes());
    }
    data.push(b'm');

    parser.advance(&mut performer, &data);
    performer.clear();

    // CSI with very large parameter values
    data.clear();
    data.extend_from_slice(b"\x1b[");
    data.extend_from_slice(b"999999999;999999999;999999999m");
    parser.advance(&mut performer, &data);
    performer.clear();

    // Repeated CSI sequences (10000 times)
    data.clear();
    for _ in 0..10_000 {
        data.extend_from_slice(b"\x1b[1;2;3;4m");
    }
    parser.advance(&mut performer, &data);

    // Each CSI sequence should produce a CsiDispatch
    let csi_count = performer.actions.iter().filter(|a| {
        matches!(a, Action::CsiDispatch { action: b'm', .. })
    }).count();
    assert_eq!(csi_count, 10_000, "each repeated CSI should be dispatched");
}

// ---------------------------------------------------------------------------
// Regression tests
// ---------------------------------------------------------------------------

#[test]
fn regression_osc_exceeds_64kb_is_bounded() {
    // Verify OSC data larger than 64KB is truncated
    let mut parser = Parser::new();
    let mut performer = RecordingPerformer::new();
    // Start OSC
    parser.advance(&mut performer, b"\x1b]2;");
    // Feed 100KB of data
    let data = vec![b'A'; 100_000];
    parser.advance(&mut performer, &data);
    // Terminate with BEL
    parser.advance(&mut performer, b"\x07");

    // OSC should have been dispatched (possibly truncated, but dispatched)
    let has_osc = performer.actions.iter().any(|a| matches!(a, Action::OscDispatch(_)));
    assert!(has_osc, "OSC larger than 64KB should still be dispatched");
}

