# TODOS

## Security: Add Minisign signature verification to `emux upgrade`

**What:** Verify release archives against a Minisign cryptographic signature, not just SHA-256 checksums. Embed the public key in the binary.

**Why:** SHA-256 checksums protect against corruption and MITM but not a compromised release author (who controls both the binary and SHA256SUMS). Signatures require the attacker to also steal the private signing key, which is stored offline.

**Pros:** Gold-standard supply chain protection. Key can be rotated independently of GitHub credentials. Matches the security model of rustup, cargo-binstall, and other Rust ecosystem tools.

**Cons:** Requires Minisign tooling in CI, offline key management, and the `minisign-verify` crate (~200 lines of new code).

**Context:** CSO audit on 2026-03-23 rated the original no-verification vulnerability HIGH (OWASP A08, 9/10 confidence). The current SHA-256 fix is a significant improvement over zero verification. Signatures are the natural next step when the project has more users and the release process is more mature.

**Depends on:** SHA-256 verification (already implemented in `bins/emux/src/upgrade.rs`).

**Files:** `bins/emux/src/upgrade.rs`, CI release workflow

## Reliability: Atomic binary replacement on Unix

**What:** Change `download_and_replace()` to write the new binary to a temp file on the same filesystem as the target, then `fs::rename()` (which is atomic on Unix), instead of the current `fs::copy()`.

**Why:** `fs::copy` is not atomic — if interrupted mid-write (power loss, kill signal, disk full), the user is left with a partially written, corrupt binary at the install path. `fs::rename` on the same filesystem is a single atomic `rename(2)` syscall.

**Pros:** Eliminates the risk of a corrupt binary from interrupted upgrades.

**Cons:** Minimal — requires writing the temp file to the same directory as the target binary (not `/tmp`), then renaming.

**Context:** Identified by cross-model review during eng review on 2026-03-23. Pre-existing behavior, not introduced by the SHA-256 security fix.

**Depends on:** Nothing — standalone reliability improvement.

**Files:** `bins/emux/src/upgrade.rs` (lines ~220-240, the `fs::copy` + permission set block)
