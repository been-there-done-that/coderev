#!/usr/bin/env bash
set -euo pipefail

PREFIX=${PREFIX:-/usr/local}
BIN_DIR="$PREFIX/bin"
BIN_NAME="coderev"

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo is required (install Rust)" >&2
  exit 1
fi

mkdir -p "$BIN_DIR"

cargo build --release
install -m 0755 "target/release/$BIN_NAME" "$BIN_DIR/$BIN_NAME"

echo "Installed $BIN_NAME to $BIN_DIR/$BIN_NAME"
