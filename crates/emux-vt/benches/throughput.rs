use criterion::{Criterion, black_box, criterion_group, criterion_main};
use emux_vt::{Action, Parser, Performer};

struct TestPerformer;

impl Performer for TestPerformer {
    fn perform(&mut self, _action: Action) {}
}

/// Build a buffer that mixes plain ASCII text with common escape sequences.
fn build_mixed_input(size: usize) -> Vec<u8> {
    let fragments: &[&[u8]] = &[
        b"hello world ",
        b"\x1b[31m",        // SGR: set foreground red
        b"colored text ",
        b"\x1b[0m",         // SGR: reset
        b"\x1b[10;20H",     // CUP: move cursor
        b"abcdefghij\r\n",  // text + CR LF
        b"\x1b[2J",         // ED:  clear screen
        b"\x1b[?25l",       // DECRST: hide cursor
        b"the quick brown fox jumps over the lazy dog ",
        b"\x1b[?25h",       // DECSET: show cursor
    ];

    let mut buf = Vec::with_capacity(size);
    let mut i = 0;
    while buf.len() < size {
        buf.extend_from_slice(fragments[i % fragments.len()]);
        i += 1;
    }
    buf.truncate(size);
    buf
}

fn bench_parser_throughput(c: &mut Criterion) {
    let input = build_mixed_input(256 * 1024); // 256 KiB

    c.bench_function("parse_256k_mixed", |b| {
        b.iter(|| {
            let mut parser = Parser::new();
            let mut performer = TestPerformer;
            parser.advance(&mut performer, black_box(&input));
        });
    });
}

criterion_group!(benches, bench_parser_throughput);
criterion_main!(benches);
