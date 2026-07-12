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

export PATH="$HOME/.cargo/bin:/opt/homebrew/opt/rustup/bin:$PATH"
export WUSHEN_AGENT_RUNTIME_MODE="local-dev-python"
export WUSHEN_REPO_ROOT="$ROOT_DIR"

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
