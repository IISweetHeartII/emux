#!/bin/bash
# Full test suite for emux — run before PR or release.
#
# Usage:
#   ./scripts/full-test.sh          # full run (includes E2E)
#   ./scripts/full-test.sh --quick  # skip E2E (for CI without PTY)
#
# Exit codes:
#   0 = all passed
#   1 = something failed

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
BOLD='\033[1m'
RESET='\033[0m'

PASS=0
FAIL=0
SKIP=0

check() {
    local name="$1"
    shift
    printf "${BOLD}[%02d] %-35s${RESET}" "$((PASS+FAIL+SKIP+1))" "$name"
    if "$@" > /tmp/emux-test-output.log 2>&1; then
        printf "${GREEN}PASS${RESET}\n"
        PASS=$((PASS+1))
    else
        printf "${RED}FAIL${RESET}\n"
        cat /tmp/emux-test-output.log
        FAIL=$((FAIL+1))
    fi
}

skip() {
    local name="$1"
    printf "${BOLD}[%02d] %-35s${RESET}SKIP\n" "$((PASS+FAIL+SKIP+1))" "$name"
    SKIP=$((SKIP+1))
}

QUICK=false
if [ "${1:-}" = "--quick" ]; then
    QUICK=true
fi

echo ""
echo "======================================"
echo "  emux full test suite"
echo "======================================"
echo ""

check "cargo fmt --check"              cargo fmt --all -- --check
check "cargo clippy -D warnings"       cargo clippy --workspace -- -D warnings
check "cargo test (unit+integration)"  cargo test --workspace

if [ "$QUICK" = false ]; then
    # Clean stale sockets before E2E tests
    rm -f "${TMPDIR:-/tmp}"/emux-sockets/emux-* 2>/dev/null || true

    check "E2E tests (--ignored)"      cargo test --workspace -- --ignored --test-threads=1
else
    skip "E2E tests (--quick mode)"
fi

check "release build"                  cargo build --release
check "bench compile"                  cargo bench --workspace --no-run
check "cargo doc"                      cargo doc --workspace --no-deps

# Verify binary
check "binary --version"               target/release/emux --version
check "binary --help"                  target/release/emux --help

# Fuzz targets (nightly only)
if rustup run nightly rustc --version > /dev/null 2>&1 && [ -d "fuzz" ]; then
    check "fuzz targets compile"       bash -c "cd fuzz && cargo +nightly check"
else
    skip "fuzz targets (no nightly)"
fi

echo ""
echo "======================================"
printf "  ${GREEN}PASS: %d${RESET}  " "$PASS"
if [ "$FAIL" -gt 0 ]; then
    printf "${RED}FAIL: %d${RESET}  " "$FAIL"
fi
if [ "$SKIP" -gt 0 ]; then
    printf "SKIP: %d  " "$SKIP"
fi
echo ""
echo "======================================"
echo ""

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
