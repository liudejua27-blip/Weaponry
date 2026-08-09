#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PACKAGE_ROOT="${1:-${HOME}/Applications/ForgeCAD Runtime Dev.app}"

if [[ ! -d "$PACKAGE_ROOT" ]]; then
  printf '%s\n' "package layout NOT_RUN: ForgeCAD Runtime Dev.app is not installed" >&2
  exit 2
fi

exec python3 "$PROJECT_ROOT/scripts/probe_mcp010a_dev_app.py" \
  --app "$PACKAGE_ROOT" \
  --verify-only
