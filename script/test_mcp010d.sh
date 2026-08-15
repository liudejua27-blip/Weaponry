#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_ROOT="$(mktemp -d /tmp/forgecad-mcp010d.XXXXXX)"
trap 'rm -rf "$TEMP_ROOT"' EXIT
TARGET_DIR="$TEMP_ROOT/cargo-target"
DATA_ROOT="$TEMP_ROOT/runtime-data"

python3 "$PROJECT_ROOT/scripts/check_forgecad_contracts.py"
python3 -m py_compile "$PROJECT_ROOT/scripts/probe_mcp010d_raw_stdio.py"

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo build \
  --manifest-path "$PROJECT_ROOT/apps/geometry-worker/Cargo.toml" \
  --bin forgecad-geometry-worker --offline

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo build \
  --manifest-path "$PROJECT_ROOT/apps/desktop/src-tauri/Cargo.toml" \
  -p forgecad-mcp --bin forgecad-mcp \
  -p forgecad-runtime --bin forgecad-runtime --offline

python3 "$PROJECT_ROOT/scripts/probe_mcp010d_raw_stdio.py" \
  --mcp "$TARGET_DIR/debug/forgecad-mcp" \
  --runtime "$TARGET_DIR/debug/forgecad-runtime" \
  --data-root "$DATA_ROOT" \
  --evidence "$PROJECT_ROOT/docs/evidence/mcp010d/raw-stdio-subd-cage-20260815.json"
