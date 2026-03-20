# Contributing to emux

Thanks for your interest in contributing to emux! Whether you're fixing a bug, adding a feature, improving docs, or just writing tests -- every contribution helps.

Please note that this project has a [Code of Conduct](CODE_OF_CONDUCT.md). By participating, you are expected to uphold it.

## Getting Started

### Prerequisites

- **Rust** stable toolchain (2024 edition) -- see [`rust-toolchain.toml`](rust-toolchain.toml)
- **Platform:** macOS, Linux, WSL, or Windows with ConPTY support

### Clone and build

```sh
git clone https://github.com/IISweetHeartII/emux.git
cd emux
make setup          # install pre-commit hook (fmt + clippy)
cargo build --workspace
```

### Run it

```sh
cargo run --release
```

---

## Running Tests

emux has **1,409 automated tests** (1,379 unit/integration + 30 E2E).
Every PR must pass all of them.

### Quick check (before every commit)

```sh
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
```

### Full test (before PR or release)

```sh
# Run everything including E2E tests that need a real PTY
./scripts/full-test.sh
```

Or step by step:

```sh
# 1. Format
cargo fmt --all -- --check

# 2. Lint (zero warnings required)
cargo clippy --workspace -- -D warnings

# 3. Unit + integration tests (1,379 tests)
cargo test --workspace

# 4. E2E tests — spawns real emux binary in a PTY (30 tests)
#    Must run single-threaded (PTY resource contention)
cargo test --workspace -- --ignored --test-threads=1

# 5. Release build
cargo build --release

# 6. Benchmark compilation
cargo bench --workspace --no-run

# 7. Install and verify
cargo install --path bins/emux --force
emux --version
```

### Test types

| Type | Location | How to run | Count |
|------|----------|------------|-------|
| Unit tests | `src/**/*.rs` (`#[cfg(test)]`) | `cargo test -p <crate>` | ~800 |
| Integration | `tests/*.rs` | `cargo test -p <crate> --test <name>` | ~500 |
| Golden/snapshot | `crates/emux-term/tests/golden/` | `cargo test -p emux-term -- golden` | 45 |
| Stress tests | `crates/emux-vt/tests/stress.rs` | `cargo test -p emux-vt --test stress` | 21 |
| E2E binary | `bins/emux/tests/e2e.rs` | `cargo test -p emux -- --ignored --test-threads=1` | 21 |
| Agent IPC E2E | `bins/emux/tests/agent_ipc.rs` | same as above | 4 |
| Session lifecycle | `bins/emux/tests/session.rs` | same as above | 1 |
| Daemon agent E2E | `crates/emux-daemon/tests/agent_e2e.rs` | `cargo test -p emux-daemon --test agent_e2e` | 9 |
| Fuzz targets | `fuzz/` | `cargo +nightly fuzz run fuzz_parser` | 3,993 corpus |
| Benchmarks | `benches/` | `cargo bench -p emux-vt` | 1 |

### Per-crate test commands

```sh
cargo test -p emux-vt           # VT parser (conformance + stress)
cargo test -p emux-term         # Terminal state (golden, screen, input)
cargo test -p emux-pty          # PTY spawn
cargo test -p emux-mux          # Pane/tab layout
cargo test -p emux-config       # Configuration loading
cargo test -p emux-daemon       # Daemon server + agent IPC
cargo test -p emux-ipc          # IPC protocol codec
cargo test -p emux-render       # Rendering + status bar
cargo test -p emux              # CLI + E2E binary tests
```

### Platform-specific testing

| Check | macOS | Linux/WSL | Windows |
|-------|-------|-----------|---------|
| `cargo test --workspace` | Yes | Yes | Yes |
| `-- --ignored --test-threads=1` | Yes | Yes | Skipped (`#[cfg(unix)]`) |
| Fuzz (`cargo +nightly fuzz run`) | Yes | Yes | Not supported |
| Agent IPC socket | Unix socket | Unix socket | TCP loopback |

**Windows users:** The E2E tests are Unix-only (`#[cfg(unix)]`). On Windows, run
the standard test suite and manually verify `emux.exe --version` and `emux.exe --help`.

**WSL users:** Full test suite works identically to Linux/macOS.

---

## Development Workflow

We follow a **TDD (test-driven development)** approach:

1. **Write the test first.** Place it in a `#[cfg(test)] mod tests` block in the relevant module, or in the crate's `tests/` directory for integration tests.
2. **Watch it fail.** Run `cargo test -p <crate> -- <test_name>` and confirm the failure.
3. **Implement.** Write the minimum code to make the test pass.
4. **Refactor.** Clean up while keeping all tests green.

---

## Code Standards

### Before submitting a PR

```sh
# Format
cargo fmt --all

# Lint (must be warning-free)
cargo clippy --workspace -- -D warnings

# Test
cargo test --workspace
```

### Style guidelines

- **Clippy clean.** No warnings. Run clippy before every commit.
- **Formatted.** Use default `rustfmt` settings via `cargo fmt --all`.
- **No unsafe unless justified.** If `unsafe` is needed, add a `// SAFETY:` comment explaining the invariants.
- **Document public APIs.** All `pub` items should have a `///` doc comment.
- **Error handling.** Use `Result` with descriptive error types. Avoid `.unwrap()` in library code (tests are fine).
- **No external code copy-paste.** All code must be original or properly vendored with license compliance.

---

## Commit Conventions

We use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>
```

**Types:**

| Type       | When to use                              |
|------------|------------------------------------------|
| `feat`     | A new feature                            |
| `fix`      | A bug fix                                |
| `docs`     | Documentation only changes               |
| `refactor` | Code change that neither fixes nor adds  |
| `perf`     | Performance improvement                  |
| `test`     | Adding or updating tests                 |
| `chore`    | Miscellaneous changes (deps, etc.)       |
| `ci`       | CI/CD configuration changes              |
| `build`    | Build system or tooling changes          |
| `style`    | Code style (formatting, whitespace)      |
| `revert`   | Reverting a previous commit              |

**Scope** is optional but encouraged -- use the crate name (e.g., `feat(vt): add DA2 response`).

**Breaking changes:** Add `!` after the type/scope (e.g., `feat(ipc)!: change message format`).

**Skip release:** Add `[skip release]` to the commit message to prevent auto-release on merge to main.

---

## Branch Naming

Use the following prefixes:

| Prefix      | Purpose                  |
|-------------|--------------------------|
| `feat/`     | New features             |
| `fix/`      | Bug fixes                |
| `docs/`     | Documentation            |
| `refactor/` | Code refactoring         |
| `perf/`     | Performance improvements |
| `test/`     | Adding/updating tests    |
| `ci/`       | CI/CD changes            |
| `chore/`    | Miscellaneous            |

Examples: `feat/sixel-support`, `fix/utf8-reflow-crash`, `docs/ipc-protocol`.

---

## PR Process

1. **Fork** the repo and create a feature branch from `main`.
2. **Make your changes** with tests.
3. **Ensure all checks pass:**
   ```sh
   cargo fmt --all -- --check
   cargo clippy --workspace -- -D warnings
   cargo test --workspace
   ```
4. **Open a PR** against `main` with a clear description of what changed and why.
5. One approval required for merge.

Keep PRs focused -- one feature or fix per PR. Smaller PRs get reviewed faster.

---

## Labels

Issues and PRs are automatically labeled based on file paths. You can also apply labels manually.

### Type labels

| Label                | Description              |
|----------------------|--------------------------|
| `type: bug`          | Bug reports              |
| `type: feature`      | New feature requests     |
| `type: enhancement`  | Improvements to existing |
| `type: documentation`| Docs changes             |
| `type: refactor`     | Code refactoring         |
| `type: performance`  | Performance improvements |
| `type: security`     | Security-related changes |

### Area labels

| Label              | Scope              |
|--------------------|---------------------|
| `area: emux-vt`    | VT parser           |
| `area: emux-term`  | Terminal state       |
| `area: emux-pty`   | PTY integration      |
| `area: emux-mux`   | Multiplexer          |
| `area: emux-config`| Configuration        |
| `area: emux-daemon`| Session daemon       |
| `area: emux-ipc`   | IPC protocol         |
| `area: emux-render` | Rendering           |
| `area: cli`         | CLI binary           |
| `area: infrastructure` | CI/CD, tooling  |
| `area: testing`     | Tests, fuzzing       |

### Status labels

| Label                  | Meaning                    |
|------------------------|----------------------------|
| `status: needs triage` | Awaiting maintainer review |
| `good first issue`     | Good for new contributors  |
| `help wanted`          | Community help welcome     |

---

## Architecture Overview

See the [Architecture section in README.md](README.md#architecture) for the full crate map and dependency flow. Each crate can be compiled and tested in isolation, so you can contribute to a specific layer without understanding the full stack.

---

## Good First Issues

New to the project? Here are areas where contributions are always welcome:

- **VT sequence coverage** -- find a terminal escape sequence we don't handle and add parser support + tests
- **Config options** -- expose a new setting in `emux-config` with TOML support
- **Keybinding additions** -- add a new action to the keybinding system
- **Test coverage** -- find an untested code path and write a test for it
- **Platform fixes** -- improve Windows ConPTY or WSL compatibility
- **Documentation** -- improve doc comments on public APIs

Look for issues labeled `good first issue` if any are available.

---

## Questions?

- **Discussions:** [GitHub Discussions](https://github.com/IISweetHeartII/emux/discussions)
- **Issues:** [GitHub Issues](https://github.com/IISweetHeartII/emux/issues)
- **Security:** See [SECURITY.md](SECURITY.md) for vulnerability reporting

We're happy to help you get started.
