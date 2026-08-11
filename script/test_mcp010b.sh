#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_ROOT="$(mktemp -d /tmp/forgecad-mcp010b.XXXXXX)"
trap 'rm -rf "$TEMP_ROOT"' EXIT
TARGET_DIR="$TEMP_ROOT/cargo-target"
DATA_ROOT="$TEMP_ROOT/runtime-data"

python3 "$PROJECT_ROOT/scripts/check_forgecad_contracts.py"

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo build \
  --manifest-path "$PROJECT_ROOT/apps/geometry-worker/Cargo.toml" \
  --bin forgecad-geometry-worker --offline

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo test \
  --manifest-path "$PROJECT_ROOT/apps/desktop/src-tauri/Cargo.toml" \
  -p forgecad-runtime --lib geometry_worker::isolated_tests --offline -- \
  --ignored --test-threads=1

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo build \
  --manifest-path "$PROJECT_ROOT/apps/desktop/src-tauri/Cargo.toml" \
  -p forgecad-mcp --bin forgecad-mcp \
  -p forgecad-runtime --bin forgecad-runtime --offline

python3 "$PROJECT_ROOT/scripts/probe_mcp010b_raw_stdio.py" \
  --mcp "$TARGET_DIR/debug/forgecad-mcp" \
  --runtime "$TARGET_DIR/debug/forgecad-runtime" \
  --data-root "$DATA_ROOT"
