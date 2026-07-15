#!/usr/bin/env bash
set -euo pipefail

# Build-machine-only freezer for the current macOS arm64 sidecar. The output
# becomes Tauri's target-suffixed externalBin input; it contains no Provider
# configuration and does not make a network/API request at runtime.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PYTHON_BIN="${FORGECAD_SIDECAR_PYTHON:-$ROOT_DIR/.venv/bin/python}"
TARGET_TRIPLE="${FORGECAD_SIDECAR_TARGET:-aarch64-apple-darwin}"
if [[ "$TARGET_TRIPLE" != "aarch64-apple-darwin" ]]; then
  echo "P002 currently supports only aarch64-apple-darwin; got $TARGET_TRIPLE" >&2
  exit 2
fi
if [[ ! -x "$PYTHON_BIN" ]]; then
  echo "Python build runtime not found: $PYTHON_BIN" >&2
  exit 2
fi

SIDECAR_NAME="wushen-agent-$TARGET_TRIPLE"
BINARY_DIR="$ROOT_DIR/apps/desktop/src-tauri/binaries"
BUILD_ROOT="$ROOT_DIR/apps/desktop/src-tauri/target/packaged-sidecar-build"
mkdir -p "$BINARY_DIR" "$BUILD_ROOT"

"$PYTHON_BIN" -m PyInstaller \
  --noconfirm \
  --clean \
  --onefile \
  --name "$SIDECAR_NAME" \
  --paths "$ROOT_DIR/apps/agent" \
  --add-data "$ROOT_DIR/migrations:migrations" \
  --add-data "$ROOT_DIR/packages/concept-spec:packages/concept-spec" \
  --collect-all fastapi \
  --collect-all starlette \
  --collect-all pydantic \
  --collect-all jsonschema \
  --collect-all manifold3d \
  --hidden-import manifold3d \
  --collect-all numpy \
  --hidden-import numpy._core._exceptions \
  --collect-all wushen_agent \
  --collect-all forgecad_agent \
  --distpath "$BINARY_DIR" \
  --workpath "$BUILD_ROOT/work" \
  --specpath "$BUILD_ROOT/spec" \
  "$ROOT_DIR/apps/agent/wushen_agent/sidecar_entry.py"

chmod u+x "$BINARY_DIR/$SIDECAR_NAME"
"$PYTHON_BIN" "$ROOT_DIR/scripts/packaged_sidecar_preflight.py" --require-ready
