#!/usr/bin/env bash
set -euo pipefail

# Local macOS verification entrypoint for the Tauri workbench.
# This intentionally uses the development Python Agent fallback. It is not a
# release-packaging command: the checked-in packaged sidecar is still a
# placeholder and cannot support a standalone installation.

MODE="${1:-run}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_BUNDLE="$ROOT_DIR/apps/desktop/src-tauri/target/release/bundle/macos/武神 Forge.app"
APP_BINARY="$APP_BUNDLE/Contents/MacOS/wushen-forge-desktop"

RUSTC_BIN="$(rustup which rustc 2>/dev/null || true)"
if [[ -n "$RUSTC_BIN" && -x "$RUSTC_BIN" ]]; then
  export PATH="$(dirname "$RUSTC_BIN"):$PATH"
fi
export PATH="$HOME/.cargo/bin:/opt/homebrew/opt/rustup/bin:$PATH"
export WUSHEN_AGENT_RUNTIME_MODE="local-dev-python"
export WUSHEN_REPO_ROOT="$ROOT_DIR"

# Prefer the locally formalized original-author visual Pack when this Mac has
# it available. The Agent safely falls back to the repository reference Pack
# on other development machines.
ORIGINAL_AUTHOR_PACK="$HOME/Library/Caches/ForgeCAD/Formalization/weapon-concept-v1-final-art-intake-20260711/final-pack"
if [[ -f "$ORIGINAL_AUTHOR_PACK/pack.json" ]]; then
  export FORGECAD_BUNDLED_MODULE_PACK="$ORIGINAL_AUTHOR_PACK"
fi

# Opt in to the user's locally stored DeepSeek test credential. The key never
# enters this repository, command history, logs, or the Tauri bundle.
DEEPSEEK_KEY_FILE="${FORGECAD_DEEPSEEK_API_KEY_FILE:-$HOME/Library/Application Support/ForgeCAD/deepseek-test.key}"
if [[ -r "$DEEPSEEK_KEY_FILE" ]]; then
  export FORGECAD_CONCEPT_PLANNER_PROVIDER="openai_compatible"
  export FORGECAD_CONCEPT_PLANNER_BASE_URL="https://api.deepseek.com"
  export FORGECAD_CONCEPT_PLANNER_MODEL="deepseek-v4-pro"
  export FORGECAD_CONCEPT_PLANNER_API_KEY_FILE="$DEEPSEEK_KEY_FILE"
  export FORGECAD_CONCEPT_PLANNER_RESPONSE_MODE="auto"
  export FORGECAD_CONCEPT_PLANNER_MAX_TOKENS="4096"
fi

build_app() {
  (cd "$ROOT_DIR" && npm --workspace apps/desktop run tauri -- build --bundles app)
}

stop_local_app() {
  pkill -f "$APP_BINARY" >/dev/null 2>&1 || true
}

launch_app() {
  if [[ ! -d "$APP_BUNDLE" ]]; then
    echo "Expected app bundle was not created: $APP_BUNDLE" >&2
    exit 1
  fi
  /usr/bin/open -n "$APP_BUNDLE"
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
    echo "local_tauri_app_running: true"
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
