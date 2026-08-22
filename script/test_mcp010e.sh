#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_ROOT="$(mktemp -d /tmp/forgecad-mcp010e.XXXXXX)"
trap 'rm -rf "$TEMP_ROOT"' EXIT
TARGET_DIR="$TEMP_ROOT/cargo-target"
DATA_ROOT="$TEMP_ROOT/runtime-data"

python3 "$PROJECT_ROOT/scripts/check_forgecad_contracts.py"
python3 -m py_compile "$PROJECT_ROOT/scripts/probe_mcp010e_raw_stdio.py"

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo build \
  --manifest-path "$PROJECT_ROOT/apps/geometry-worker/Cargo.toml" \
  --bin forgecad-geometry-worker --offline

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo build \
  --manifest-path "$PROJECT_ROOT/apps/render-worker/Cargo.toml" \
  --bin forgecad-render-worker --offline

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo build \
  --manifest-path "$PROJECT_ROOT/apps/desktop/src-tauri/Cargo.toml" \
  -p forgecad-mcp --bin forgecad-mcp \
  -p forgecad-runtime --bin forgecad-runtime --offline

python3 "$PROJECT_ROOT/scripts/probe_mcp010e_raw_stdio.py" \
  --mcp "$TARGET_DIR/debug/forgecad-mcp" \
  --runtime "$TARGET_DIR/debug/forgecad-runtime" \
  --data-root "$DATA_ROOT"

python3 "$PROJECT_ROOT/scripts/probe_mcp010e_raw_stdio.py" \
  --mcp "$TARGET_DIR/debug/forgecad-mcp" \
  --runtime "$TARGET_DIR/debug/forgecad-runtime" \
  --data-root "$TEMP_ROOT/linework-data" \
  --detail \
  --geometry-variant surface-linework \
  --material-variant armor-shell-zones

python3 "$PROJECT_ROOT/scripts/probe_mcp010e_raw_stdio.py" \
  --mcp "$TARGET_DIR/debug/forgecad-mcp" \
  --runtime "$TARGET_DIR/debug/forgecad-runtime" \
  --data-root "$TEMP_ROOT/three-quarter-data" \
  --detail \
  --geometry-variant three-quarter \
  --material-variant armor-shell-zones

python3 "$PROJECT_ROOT/scripts/probe_mcp010e_raw_stdio.py" \
  --mcp "$TARGET_DIR/debug/forgecad-mcp" \
  --runtime "$TARGET_DIR/debug/forgecad-runtime" \
  --data-root "$TEMP_ROOT/fictional-energy-weapon-pack-data" \
  --appearance-pack fictional-energy-weapon
