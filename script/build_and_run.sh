#!/usr/bin/env bash
set -euo pipefail

# Local macOS verification entrypoint for the Tauri workbench. It uses the
# development Python-Agent path; the separate packaged-sidecar native E2E
# remains tracked as unfinished FGC-P002 work.

MODE="${1:-run}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_BUNDLE="$ROOT_DIR/apps/desktop/src-tauri/target/release/bundle/macos/CAD 工作台.app"
APP_BINARY="$APP_BUNDLE/Contents/MacOS/wushen-forge-desktop"

RUSTC_BIN="$(rustup which rustc 2>/dev/null || true)"
if [[ -n "$RUSTC_BIN" && -x "$RUSTC_BIN" ]]; then
  export PATH="$(dirname "$RUSTC_BIN"):$PATH"
fi
export PATH="$HOME/.cargo/bin:/opt/homebrew/opt/rustup/bin:$PATH"
export WUSHEN_AGENT_RUNTIME_MODE="local-dev-python"
export WUSHEN_REPO_ROOT="$ROOT_DIR"

# The local workbench builds a cached Blender Pack from the current authored
# source, then stages it as self-declared original art with review still
# pending. It uses a content-addressed cache so old Library rows never shadow
# new geometry and the user's default Library is never overwritten.
VISUAL_SOURCE_FINGERPRINT="$(shasum -a 256 "$ROOT_DIR/scripts/blender/weapon_concept_starter.py" "$ROOT_DIR/scripts/stage_original_author_visual_pack.py" | shasum -a 256 | cut -c1-16)"
VISUAL_PACK_CACHE="$HOME/Library/Caches/ForgeCAD/OriginalAuthorVisualPacks/$VISUAL_SOURCE_FINGERPRINT"
ORIGINAL_AUTHOR_PACK="$VISUAL_PACK_CACHE/final-pack"
CURRENT_FORMAL_PACK="$HOME/Library/Caches/ForgeCAD/Formalization/current/final-pack"
LOCAL_TEST_MODULE_PACK="${FORGECAD_LOCAL_TEST_MODULE_PACK:-}"
LOCAL_TEST_LIBRARY_ROOT="${WUSHEN_LOCAL_TEST_LIBRARY_ROOT:-}"

prepare_local_visual_pack() {
  if [[ -f "$ORIGINAL_AUTHOR_PACK/pack.json" ]]; then
    mkdir -p "$(dirname "$CURRENT_FORMAL_PACK")"
    ln -sfn "$ORIGINAL_AUTHOR_PACK" "$CURRENT_FORMAL_PACK"
    return 0
  fi
  mkdir -p "$VISUAL_PACK_CACHE"
  "$ROOT_DIR/.venv/bin/python" "$ROOT_DIR/scripts/build_blender_starter_pack.py" \
    --module-set full_candidate --require-blender \
    --output-root "$VISUAL_PACK_CACHE/candidate-pack"
  "$ROOT_DIR/.venv/bin/python" "$ROOT_DIR/scripts/stage_original_author_visual_pack.py" \
    --candidate-root "$VISUAL_PACK_CACHE/candidate-pack" \
    --output-root "$ORIGINAL_AUTHOR_PACK"
  mkdir -p "$(dirname "$CURRENT_FORMAL_PACK")"
  ln -sfn "$ORIGINAL_AUTHOR_PACK" "$CURRENT_FORMAL_PACK"
}

if [[ -z "$LOCAL_TEST_MODULE_PACK" && "${FORGECAD_LOCAL_VISUAL_PACK:-1}" != "0" ]]; then
  prepare_local_visual_pack
  LOCAL_TEST_LIBRARY_ROOT="${LOCAL_TEST_LIBRARY_ROOT:-$HOME/Library/Caches/ForgeCAD/LocalWorkbenchLibraries/$VISUAL_SOURCE_FINGERPRINT}"
fi
if [[ -n "$LOCAL_TEST_MODULE_PACK" ]]; then
  if [[ ! -f "$LOCAL_TEST_MODULE_PACK/pack.json" ]]; then
    echo "FORGECAD_LOCAL_TEST_MODULE_PACK must point to a ModulePackManifest@1 directory" >&2
    exit 2
  fi
  # launchctl encodes a non-ASCII workspace path incorrectly for the Python
  # child on some macOS versions. Pass an ASCII-only symlink in the user cache
  # so a native Tauri candidate test is not dependent on the repository name.
  LOCAL_TEST_PACK_LINK_ROOT="$HOME/Library/Caches/ForgeCAD/LocalTestPacks"
  mkdir -p "$LOCAL_TEST_PACK_LINK_ROOT"
  LOCAL_TEST_PACK_LINK="$LOCAL_TEST_PACK_LINK_ROOT/$(printf '%s' "$LOCAL_TEST_MODULE_PACK" | shasum -a 256 | cut -c1-16)"
  ln -sfn "$LOCAL_TEST_MODULE_PACK" "$LOCAL_TEST_PACK_LINK"
  export FORGECAD_BUNDLED_MODULE_PACK="$LOCAL_TEST_PACK_LINK"
elif [[ -f "$ORIGINAL_AUTHOR_PACK/pack.json" ]]; then
  export FORGECAD_BUNDLED_MODULE_PACK="$ORIGINAL_AUTHOR_PACK"
fi
if [[ -n "$LOCAL_TEST_LIBRARY_ROOT" ]]; then
  export WUSHEN_LIBRARY_ROOT="$LOCAL_TEST_LIBRARY_ROOT"
fi

# This local-workbench verifier intentionally remains deterministic. It never
# reads a model credential or makes a provider request; live-provider checks
# are a separate, user-initiated operation with a newly issued key.

build_app() {
  (cd "$ROOT_DIR" && npm --workspace apps/desktop run tauri -- build --bundles app)
}

stop_local_app() {
  pkill -TERM -f "$APP_BINARY" >/dev/null 2>&1 || true
  # `pkill` does not emit a macOS close event, so the app-managed Python child
  # can otherwise outlive the bundle and cause the next test run to reuse old
  # code or old provider configuration on port 8000.
  if [[ "${WUSHEN_KEEP_EXISTING_AGENT:-0}" != "1" ]]; then
    pkill -TERM -f 'wushen_agent.main:create_app.*--port 8000' >/dev/null 2>&1 || true
  fi
}

launch_app() {
  if [[ ! -d "$APP_BUNDLE" ]]; then
    echo "Expected app bundle was not created: $APP_BUNDLE" >&2
    exit 1
  fi
  # Run the bundle binary directly for verification so the Rust supervisor
  # receives the same non-secret runtime paths as this shell. LaunchServices
  # drops shell variables in CI/headless sessions and can leave a GUI process
  # alive without starting its managed Agent. Credentials are never exported.
  "$APP_BINARY" >"$ROOT_DIR/.wushen-tauri.log" 2>&1 &
  TAURI_APP_PID=$!
  disown "$TAURI_APP_PID" 2>/dev/null || true
}

wait_for_agent() {
  for _ in {1..25}; do
    if curl -fsS http://127.0.0.1:8000/api/health >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  echo "local Agent did not become healthy; inspect $ROOT_DIR/.wushen-agent.log" >&2
  return 1
}

case "$MODE" in
  run)
    stop_local_app
    build_app
    launch_app
    ;;
  --verify|verify)
    stop_local_app
    build_app
    launch_app
    sleep 2
    pgrep -f "$APP_BINARY" >/dev/null
    wait_for_agent
    echo "local_tauri_app_running: true"
    echo "local_agent_healthy: true"
    echo "agent_mode: local-dev-python"
    ;;
  --debug|debug)
    stop_local_app
    build_app
    lldb -- "$APP_BINARY"
    ;;
  --logs|logs)
    stop_local_app
    build_app
    launch_app
    /usr/bin/log stream --info --style compact --predicate 'process == "wushen-forge-desktop"'
    ;;
  --telemetry|telemetry)
    stop_local_app
    build_app
    launch_app
    /usr/bin/log stream --info --style compact --predicate 'process == "wushen-forge-desktop"'
    ;;
  *)
    echo "usage: $0 [run|--verify|--debug|--logs|--telemetry]" >&2
    exit 2
    ;;
esac
