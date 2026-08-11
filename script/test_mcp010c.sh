#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_ROOT="$(mktemp -d /tmp/forgecad-mcp010c.XXXXXX)"
trap 'rm -rf "$TEMP_ROOT"' EXIT
TARGET_DIR="$TEMP_ROOT/cargo-target"
DATA_ROOT="$TEMP_ROOT/runtime-data"
EXPORT_DATA_ROOT="$TEMP_ROOT/export-restart-runtime-data"

python3 "$PROJECT_ROOT/scripts/check_forgecad_contracts.py"
python3 -m py_compile "$PROJECT_ROOT/scripts/probe_mcp010c_raw_stdio.py"

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo test \
  --manifest-path "$PROJECT_ROOT/apps/geometry-worker/Cargo.toml" \
  --lib c_fixed_perspective_renderer_emits_deterministic_nine_aov_set --offline

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo build \
  --manifest-path "$PROJECT_ROOT/apps/geometry-worker/Cargo.toml" \
  --bin forgecad-geometry-worker --offline

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo test \
  --manifest-path "$PROJECT_ROOT/apps/desktop/src-tauri/Cargo.toml" \
  -p forgecad-runtime c_fixed_renderer_persists_nine_aovs_and_review_chain --offline

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo test \
  --manifest-path "$PROJECT_ROOT/apps/desktop/src-tauri/Cargo.toml" \
  -p forgecad-runtime v2_runtime_output_validators_fail_closed_on_mutated_receipts --offline

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo test \
  --manifest-path "$PROJECT_ROOT/apps/desktop/src-tauri/Cargo.toml" \
  -p forgecad-mcp --offline

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo build \
  --manifest-path "$PROJECT_ROOT/apps/desktop/src-tauri/Cargo.toml" \
  -p forgecad-mcp --bin forgecad-mcp \
  -p forgecad-runtime --bin forgecad-runtime --offline

python3 "$PROJECT_ROOT/scripts/probe_mcp010c_raw_stdio.py" \
  --mcp "$TARGET_DIR/debug/forgecad-mcp" \
  --runtime "$TARGET_DIR/debug/forgecad-runtime" \
  --data-root "$DATA_ROOT" \
  --determinism-repeats 5

python3 "$PROJECT_ROOT/scripts/probe_mcp010c_raw_stdio.py" \
  --mcp "$TARGET_DIR/debug/forgecad-mcp" \
  --runtime "$TARGET_DIR/debug/forgecad-runtime" \
  --data-root "$EXPORT_DATA_ROOT" \
  --export-restart \
  --determinism-repeats 2

printf '%s\n' "MCP010C fixed renderer, nine-AOV, comparison and review gate PASS"
