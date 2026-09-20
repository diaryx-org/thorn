#!/usr/bin/env bash
#
# Run the SvgEditor package tests (macOS host): drive a real drawing through
# the Rust binding from Swift.
#
# The Swift package references the FFI symbols, so the test binary must link
# the Rust staticlib. We force-load it, the same way a consuming app does.
#
# Usage: scripts/test-swift.sh [extra `swift test` args…]
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "▸ Building svg-editor-ffi (host) staticlib…"
cargo build -p svg-editor-ffi --manifest-path "$ROOT/Cargo.toml" >/dev/null
STATIC="$ROOT/target/debug/libsvg_editor_ffi.a"
[ -f "$STATIC" ] || { echo "missing $STATIC"; exit 1; }

echo "▸ swift test (SvgEditor)…"
# The package manifest lives at the repo root (see Package.swift).
swift test --package-path "$ROOT" \
  -Xlinker -force_load -Xlinker "$STATIC" "$@"
