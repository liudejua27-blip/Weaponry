#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_ROOT="$(mktemp -d /tmp/forgecad-mcp006.XXXXXX)"
trap 'rm -rf "$TEMP_ROOT"' EXIT
TARGET_DIR="$TEMP_ROOT/cargo-target"

python3 "$PROJECT_ROOT/scripts/materialize_mcp006_bundles.py"
python3 "$PROJECT_ROOT/scripts/check_mcp006_skills.py"

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo test \
  --manifest-path "$PROJECT_ROOT/apps/desktop/src-tauri/Cargo.toml" \
  -p forgecad-runtime --lib skill_registry --offline

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo test \
  --manifest-path "$PROJECT_ROOT/apps/desktop/src-tauri/Cargo.toml" \
  -p forgecad-mcp --bin forgecad-mcp skill_registry_resources_are_read_only_and_unknown_capabilities_are_typed --offline

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo build \
  --manifest-path "$PROJECT_ROOT/apps/desktop/src-tauri/Cargo.toml" \
  -p forgecad-mcp --bin forgecad-mcp \
  -p forgecad-runtime --bin forgecad-runtime --offline

if [[ "${FORGECAD_MCP006_CODEX_E2E:-0}" == "1" ]]; then
  env -u FORGECAD_MCP_ENABLE_MCP004_WRITES -u FORGECAD_RUNTIME_DATA_DIR \
    python3 "$PROJECT_ROOT/scripts/probe_mcp006_codex_cli.py" --execute \
      --runtime-command "$TARGET_DIR/debug/forgecad-runtime" \
      --mcp-command "$TARGET_DIR/debug/forgecad-mcp"
else
  printf '%s\n' '{"status":"NOT_RUN","reason":"Set FORGECAD_MCP006_CODEX_E2E=1 to run the real Codex CLI Skill registry probe."}'
fi
