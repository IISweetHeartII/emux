# emux

A terminal multiplexer written in Rust.

## Build & Test Commands

- Build: `cargo build --workspace`
- Test all: `cargo test --workspace`
- Single test: `cargo test -p emux-vt -- test_name`
- E2E tests (needs real PTY): `cargo test --workspace -- --ignored --test-threads=1`
- Full test suite: `./scripts/full-test.sh`
- Quick check (no E2E): `./scripts/full-test.sh --quick`
- Lint: `cargo clippy --workspace -- -D warnings`
- Format: `cargo fmt --all`
- Bench: `cargo bench -p emux-vt`

## Testing

### Stress tests
Located in `crates/emux-vt/tests/stress.rs`. Deterministic tests that feed large
or pathological inputs (1 MB random data, extreme CSI params, rapid state
transitions, malformed UTF-8) to verify the parser never panics and always
recovers.

### Golden tests
Located in `crates/emux-term/tests/golden_tests.rs`. 45 snapshot tests derived
from Alacritty's ref test suite. Each test replays a recorded byte stream
through `Parser` + `Screen` and compares the resulting grid via `insta` snapshots.
Test data lives in `crates/emux-term/tests/golden/ref/<name>/`.

### Fuzz testing
Located in `fuzz/` (separate Cargo project, not part of the workspace).
Two fuzz targets using `cargo-fuzz` / libFuzzer:
- `fuzz_parser` -- raw bytes into the VT parser
- `fuzz_terminal` -- raw bytes through Parser + Screen

Run with:
```
cargo +nightly fuzz run fuzz_parser
cargo +nightly fuzz run fuzz_terminal
```
Seed corpus lives in `fuzz/corpus/`.

## Project Structure

Cargo workspace with crates under `crates/` and the main binary under `bins/emux`.

| Crate | Status | Purpose |
|-------|--------|---------|
| emux-vt | done | VT escape sequence parser (state machine, CSI/OSC/DCS) |
| emux-term | done | Terminal state: grid, screen, cursor, input encoding, SGR |
| emux-pty | done | PTY allocation and I/O (Unix via nix, Windows stub) |
| emux-mux | done | Session, tab, pane, layout engine, floating panes, swap layouts |
| emux-config | done | Configuration loading (TOML), theme, keybindings, defaults |
| emux-daemon | done | Session daemon (server, client, persistence) |
| emux-ipc | done | Client-daemon IPC protocol (length-prefixed JSON codec) |
| emux-render | done | Rendering / drawing layer (crossterm, damage tracking, status bar) |

## CI / Windows Notes

- **`deny.toml`**: cargo-deny 0.16+ changed the config schema — `"warn"`/`"allow"` are no longer valid for `[advisories]` fields. Valid values: `"deny"`, `"all"`, `"workspace"`, `"transitive"`, `"none"`. Always check `cargo deny --version` before editing.
- **Windows daemon tests**: Several `emux-daemon` tests (snapshot, multi-client session sharing) are `#[cfg_attr(windows, ignore)]` due to flaky port file / temp path races on Windows CI. These are fully covered on Unix/macOS runners.
- **Windows dead code**: Unix-only fields used behind `#[cfg(unix)]` need `#[cfg_attr(not(unix), allow(dead_code))]` to avoid `-D warnings` failures on Windows.

## Conventions

- **TDD**: write tests first, then implement.
- **No external code copy-paste**: all code must be original or properly vendored with license compliance.
- **License**: MIT.
