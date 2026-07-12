#!/usr/bin/env bash
set -euo pipefail

# Homebrew's rustup installation may keep the active toolchain outside
# ~/.cargo/bin. Put the resolved toolchain directory on PATH so Cargo can
# always find its matching rustc (including in non-interactive shells).
RUSTC_BIN="$(rustup which rustc 2>/dev/null || true)"
CARGO_BIN="$(rustup which cargo 2>/dev/null || true)"

if [[ -n "$RUSTC_BIN" && -x "$RUSTC_BIN" ]]; then
  export PATH="$(dirname "$RUSTC_BIN"):$PATH"
fi

if [[ "${1:-}" == "cargo" && -n "$CARGO_BIN" && -x "$CARGO_BIN" ]]; then
  shift
  exec "$CARGO_BIN" "$@"
fi

exec "$@"
