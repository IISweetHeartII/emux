#![no_main]
use libfuzzer_sys::fuzz_target;
use emux_vt::{Action, Parser, Performer};

struct NullPerformer;
impl Performer for NullPerformer {
    fn perform(&mut self, _action: Action) {}
}

fuzz_target!(|data: &[u8]| {
    let mut parser = Parser::new();
    let mut performer = NullPerformer;
    parser.advance(&mut performer, data);
});
