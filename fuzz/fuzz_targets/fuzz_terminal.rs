#![no_main]
use libfuzzer_sys::fuzz_target;
use emux_vt::{Parser, Performer as VtPerformer};
use emux_term::Screen;

fuzz_target!(|data: &[u8]| {
    let mut screen = Screen::new(80, 24);
    let mut parser = Parser::new();
    parser.advance(&mut screen, data);
    // Just verify it doesn't panic
    let _ = screen.grid.row_text(0);
});
