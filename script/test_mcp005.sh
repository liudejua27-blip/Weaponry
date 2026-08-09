#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_ROOT="$(mktemp -d /tmp/forgecad-mcp005.XXXXXX)"
trap 'rm -rf "$TEMP_ROOT"' EXIT
TARGET_DIR="$TEMP_ROOT/cargo-target"

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo test \
  --manifest-path "$PROJECT_ROOT/apps/desktop/src-tauri/Cargo.toml" \
  -p forgecad-runtime --lib reference_import --offline

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo test \
  --manifest-path "$PROJECT_ROOT/apps/desktop/src-tauri/Cargo.toml" \
  -p forgecad-mcp --bin forgecad-mcp mcp004_write_tools_are_explicit_and_confirmation_bound --offline

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo build \
  --manifest-path "$PROJECT_ROOT/apps/desktop/src-tauri/Cargo.toml" \
  -p forgecad-mcp --bin forgecad-mcp \
  -p forgecad-runtime --bin forgecad-runtime --offline

if [[ -n "${FORGECAD_MCP005_REFERENCE:-}" ]]; then
  python3 "$PROJECT_ROOT/scripts/probe_mcp005_reference.py" --execute \
    --reference "$FORGECAD_MCP005_REFERENCE" \
    --runtime-command "$TARGET_DIR/debug/forgecad-runtime" \
    --mcp-command "$TARGET_DIR/debug/forgecad-mcp"
else
  printf '%s\n' '{"status":"NOT_RUN","reason":"Set FORGECAD_MCP005_REFERENCE to run the real Codex CLI attachment probe."}'
fi
