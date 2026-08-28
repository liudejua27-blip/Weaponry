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
BUILD_COHORT="$(python3 "$ROOT_DIR/scripts/compute_build_cohort.py")"
if [[ ! "$BUILD_COHORT" =~ ^[0-9a-f]{64}$ ]]; then
  printf '%s\n' "source cohort calculator returned an invalid build cohort" >&2
  exit 1
fi

FORGECAD_BUILD_COHORT_SHA256="$BUILD_COHORT" CARGO_TARGET_DIR="$TARGET_DIR" \
  "$ROOT_DIR/script/with_rust_toolchain.sh" cargo build \
  --manifest-path "$ROOT_DIR/apps/desktop/src-tauri/Cargo.toml" \
  -p forgecad-mcp --bin forgecad-mcp \
  -p forgecad-runtime --bin forgecad-runtime \
  --offline

FORGECAD_BUILD_COHORT_SHA256="$BUILD_COHORT" CARGO_TARGET_DIR="$TARGET_DIR" \
  "$ROOT_DIR/script/with_rust_toolchain.sh" cargo build \
  --manifest-path "$ROOT_DIR/apps/geometry-worker/Cargo.toml" \
  --bin forgecad-geometry-worker \
  --offline

FORGECAD_BUILD_COHORT_SHA256="$BUILD_COHORT" CARGO_TARGET_DIR="$TARGET_DIR" \
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
  FORGECAD_BUILD_COHORT_SHA256="$BUILD_COHORT" \
  FORGECAD_RUNTIME_COMMAND="$TARGET_DIR/debug/forgecad-runtime" \
  "$TARGET_DIR/debug/forgecad-mcp" serve --stdio
