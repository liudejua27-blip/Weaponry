#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_ROOT="$(mktemp -d /tmp/forgecad-mcp009.XXXXXX)"
trap 'rm -rf "$TEMP_ROOT"' EXIT
TARGET_DIR="$TEMP_ROOT/cargo-target"

python3 "$PROJECT_ROOT/scripts/check_forgecad_contracts.py"

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo build \
  --manifest-path "$PROJECT_ROOT/apps/geometry-worker/Cargo.toml" \
  --bin forgecad-geometry-worker --offline

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo test \
  --manifest-path "$PROJECT_ROOT/apps/desktop/src-tauri/Cargo.toml" \
  -p forgecad-runtime --lib --offline -- --test-threads=1

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo test \
  --manifest-path "$PROJECT_ROOT/apps/desktop/src-tauri/Cargo.toml" \
  -p forgecad-mcp --offline -- --test-threads=1

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo build \
  --manifest-path "$PROJECT_ROOT/apps/desktop/src-tauri/Cargo.toml" \
  -p forgecad-mcp --bin forgecad-mcp \
  -p forgecad-runtime --bin forgecad-runtime --offline

printf '%s\n' '{"status":"PASS","scope":"MCP009 functional core","evidence":["stable Part change prepare","approval-bound immutable confirm","version diff","restore/diagnostic regression","CAS-backed mvp-glb export"]}'
if [[ -n "${FORGECAD_MCP009_REFERENCE:-}" && "${FORGECAD_MCP009_CODEX_E2E:-0}" == "1" ]]; then
  python3 "$PROJECT_ROOT/scripts/probe_mcp009_codex_cli.py" --execute \
    --reference "$FORGECAD_MCP009_REFERENCE" \
    --runtime-command "$TARGET_DIR/debug/forgecad-runtime" \
    --mcp-command "$TARGET_DIR/debug/forgecad-mcp"
else
  printf '%s\n' '{"status":"NOT_RUN","scope":"real Codex MCP008/MCP009 appearance/export host slice","reason":"Set FORGECAD_MCP009_REFERENCE and FORGECAD_MCP009_CODEX_E2E=1 to run the isolated Codex CLI probe."}'
fi
printf '%s\n' '{"status":"NOT_RUN","scope":"human visual score and packaged signing","reason":"MVP functional core is local/development-only; packaged release and user visual gate remain MCP013."}'
