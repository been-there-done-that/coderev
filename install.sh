#!/usr/bin/env bash
set -euo pipefail

# Get OS and Architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

# Map ARCH for GitHub Assets
case "$ARCH" in
    x86_64)  TARGET_ARCH="x86_64" ;;
    arm64|aarch64) TARGET_ARCH="aarch64" ;;
    *) echo "error: unsupported architecture $ARCH" >&2; exit 1 ;;
esac

# Map OS for GitHub Assets
case "$OS" in
    Darwin) TARGET_OS="apple-darwin" ;;
    Linux)  TARGET_OS="unknown-linux-gnu" ;;
    *) echo "error: unsupported OS $OS" >&2; exit 1 ;;
esac

TARGET="${TARGET_ARCH}-${TARGET_OS}"

# Get Latest version from GitHub API
VERSION=$(curl -s https://api.github.com/repos/been-there-done-that/coderev/releases/latest | grep -oE '"tag_name": "v[^"]+"' | head -n 1 | cut -d'"' -f4)

if [ -z "$VERSION" ]; then
    echo "error: could not find latest release version" >&2
    exit 1
fi

ASSET_NAME="coderev-${TARGET}.tar.gz"
DOWNLOAD_URL="https://github.com/been-there-done-that/coderev/releases/download/${VERSION}/${ASSET_NAME}"

PREFIX=${PREFIX:-/usr/local}
BIN_DIR="$PREFIX/bin"
BIN_NAME="coderev"

echo "Downloading $BIN_NAME $VERSION for $TARGET..."
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

if ! curl -L "$DOWNLOAD_URL" -o "$TMP_DIR/$ASSET_NAME"; then
    echo "error: failed to download $DOWNLOAD_URL" >&2
    exit 1
fi

tar -xzf "$TMP_DIR/$ASSET_NAME" -C "$TMP_DIR"

mkdir -p "$BIN_DIR"
install -m 0755 "$TMP_DIR/$BIN_NAME" "$BIN_DIR/$BIN_NAME"

echo "Successfully installed $BIN_NAME to $BIN_DIR/$BIN_NAME"
