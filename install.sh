#!/bin/sh
# emux installer — works on macOS, Linux, and WSL.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/IISweetHeartII/emux/main/install.sh | sh
#
set -e

REPO="IISweetHeartII/emux"
INSTALL_DIR="/usr/local/bin"

# --- Detect OS ---
OS="$(uname -s)"
case "$OS" in
  Linux*)  OS="unknown-linux-gnu" ;;
  Darwin*) OS="apple-darwin" ;;
  *)       echo "Error: unsupported OS '$OS'. Use install.ps1 for Windows." >&2; exit 1 ;;
esac

# --- Detect architecture ---
ARCH="$(uname -m)"
case "$ARCH" in
  x86_64|amd64)  ARCH="x86_64" ;;
  aarch64|arm64)  ARCH="aarch64" ;;
  *)              echo "Error: unsupported architecture '$ARCH'." >&2; exit 1 ;;
esac

TARGET="${ARCH}-${OS}"

# --- Get latest version ---
echo "Fetching latest release..."
VERSION="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
  | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"//;s/".*//')"

if [ -z "$VERSION" ]; then
  echo "Error: could not determine latest version." >&2
  exit 1
fi

echo "Installing emux ${VERSION} for ${TARGET}..."

# --- Download and extract ---
URL="https://github.com/${REPO}/releases/download/${VERSION}/emux-${VERSION}-${TARGET}.tar.gz"
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

curl -fsSL "$URL" -o "${TMPDIR}/emux.tar.gz"
tar xzf "${TMPDIR}/emux.tar.gz" -C "$TMPDIR"

# --- Install ---
if [ -w "$INSTALL_DIR" ]; then
  mv "${TMPDIR}/emux" "${INSTALL_DIR}/emux"
else
  echo "Installing to ${INSTALL_DIR} (requires sudo)..."
  sudo mv "${TMPDIR}/emux" "${INSTALL_DIR}/emux"
fi

chmod +x "${INSTALL_DIR}/emux"

echo ""
echo "emux ${VERSION} installed to ${INSTALL_DIR}/emux"
echo ""
echo "Run 'emux' to get started."
