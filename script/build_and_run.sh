#!/usr/bin/env bash
set -euo pipefail

# ForgeCAD's single-user development entrypoint. The MCP process owns stdio
# and starts one Runtime child asynchronously. Only current Rust components
# participate in this path.

if (( $# != 0 )); then
  printf '%s\n' "usage: script/build_and_run.sh" >&2
  exit 2
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_DIR="${FORGECAD_LOCAL_TARGET_DIR:-$ROOT_DIR/.forgecad-target}"

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$ROOT_DIR/script/with_rust_toolchain.sh" cargo build \
  --manifest-path "$ROOT_DIR/apps/desktop/src-tauri/Cargo.toml" \
  -p forgecad-mcp --bin forgecad-mcp \
  -p forgecad-runtime --bin forgecad-runtime \
  --offline

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$ROOT_DIR/script/with_rust_toolchain.sh" cargo build \
  --manifest-path "$ROOT_DIR/apps/render-worker/Cargo.toml" \
  --bin forgecad-render-worker \
  --offline

exec env \
  -u FORGECAD_RUNTIME_DATA_DIR \
  -u FORGECAD_RUNTIME_SOCKET \
  -u FORGECAD_RUNTIME_TOKEN \
  -u FORGECAD_RUNTIME_READY_FILE \
  -u FORGECAD_RUNTIME_STATUS_FILE \
  FORGECAD_RUNTIME_COMMAND="$TARGET_DIR/debug/forgecad-runtime" \
  "$TARGET_DIR/debug/forgecad-mcp" serve --stdio
