#!/usr/bin/env bash
set -euo pipefail

# Run the complete forgecad-runtime unit suite against freshly built sibling
# Workers.  A plain `cargo test -p forgecad-runtime` only builds the desktop
# workspace; the Geometry/Render Worker manifests live outside that workspace,
# so an old executable in target/debug can be selected and rejected by the
# Runtime's strict protocol/cohort gate.  This harness makes the test cohort
# explicit without adding any production fallback or changing Worker policy.

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

TEMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/forgecad-runtime-workers.XXXXXX")"
trap 'rm -rf "$TEMP_ROOT"' EXIT
TARGET_DIR="${FORGECAD_RUNTIME_TEST_TARGET:-$TEMP_ROOT/cargo-target}"

mkdir -p "$TARGET_DIR"

build_worker() {
  local manifest="$1"
  local binary="$2"
  CARGO_TARGET_DIR="$TARGET_DIR" \
    "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo build \
    --manifest-path "$PROJECT_ROOT/$manifest" \
    --bin "$binary" --offline
}

build_worker apps/geometry-worker/Cargo.toml forgecad-geometry-worker
build_worker apps/render-worker/Cargo.toml forgecad-render-worker

test -x "$TARGET_DIR/debug/forgecad-geometry-worker"
test -x "$TARGET_DIR/debug/forgecad-render-worker"

# The Runtime resolves only these fixed sibling names beside the test binary.
# Keep the final command free of the test-only fallback features so this gate
# exercises the same isolated Worker transport as a normal source build.
CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo test \
  --manifest-path "$PROJECT_ROOT/apps/desktop/src-tauri/Cargo.toml" \
  -p forgecad-runtime --lib --offline "$@"
