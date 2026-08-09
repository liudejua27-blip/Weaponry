#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_ROOT="$(mktemp -d /tmp/forgecad-mcp007.XXXXXX)"
trap 'rm -rf "$TEMP_ROOT"' EXIT
TARGET_DIR="$TEMP_ROOT/cargo-target"

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo test \
  --manifest-path "$PROJECT_ROOT/apps/geometry-worker/Cargo.toml" --offline

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo test \
  --manifest-path "$PROJECT_ROOT/apps/desktop/src-tauri/Cargo.toml" \
  -p forgecad-runtime geometry_candidate --offline

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo test \
  --manifest-path "$PROJECT_ROOT/apps/desktop/src-tauri/Cargo.toml" \
  -p forgecad-mcp authenticated_ipc_opt_in_exposes_mcp004_prepare_without_enabling_in_process_runtime --offline

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo build \
  --manifest-path "$PROJECT_ROOT/apps/geometry-worker/Cargo.toml" --offline

python3 "$PROJECT_ROOT/scripts/check_mcp007_geometry.py" \
  --worker "$TARGET_DIR/debug/forgecad-geometry-worker"

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo build \
  --manifest-path "$PROJECT_ROOT/apps/desktop/src-tauri/Cargo.toml" \
  -p forgecad-mcp --bin forgecad-mcp \
  -p forgecad-runtime --bin forgecad-runtime --offline

if [[ -n "${FORGECAD_MCP007_REFERENCE:-}" && "${FORGECAD_MCP007_CODEX_E2E:-0}" == "1" ]]; then
  python3 "$PROJECT_ROOT/scripts/probe_mcp007_codex_cli.py" --execute \
    --reference "$FORGECAD_MCP007_REFERENCE" \
    --runtime-command "$TARGET_DIR/debug/forgecad-runtime" \
    --mcp-command "$TARGET_DIR/debug/forgecad-mcp"
else
  printf '%s\n' '{"status":"NOT_RUN","reason":"Set FORGECAD_MCP007_REFERENCE and FORGECAD_MCP007_CODEX_E2E=1 to run the real Codex geometry probe."}'
fi
