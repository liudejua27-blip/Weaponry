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
  # Use LaunchServices so macOS keeps the GUI process alive. The Rust desktop
  # supervisor resolves the local provider key file and authored Module Pack
  # itself, because `open -n` intentionally does not preserve shell exports.
  /usr/bin/open -n "$APP_BUNDLE"
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
