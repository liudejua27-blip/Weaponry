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

HARNESS_MODE="qualification"
if [[ "${1:-}" == "--architecture-fast" ]]; then
  HARNESS_MODE="architecture-fast"
  shift
fi
HARNESS_STARTED_AT="$(date +%s)"

BUILD_COHORT="$(python3 "$PROJECT_ROOT/scripts/compute_build_cohort.py")"
if [[ ! "$BUILD_COHORT" =~ ^[0-9a-f]{64}$ ]]; then
  printf '%s\n' "same-cohort harness refused an invalid source cohort" >&2
  exit 1
fi

# The default path is always new.  A caller may provide a path to preserve the
# target for inspection, but it must not already exist: reusing Cargo output is
# precisely how stale sibling Workers previously entered this gate.
TEMP_ROOT=""
if [[ -n "${FORGECAD_RUNTIME_TEST_TARGET:-}" ]]; then
  TARGET_DIR="$FORGECAD_RUNTIME_TEST_TARGET"
  if [[ -e "$TARGET_DIR" || -L "$TARGET_DIR" ]]; then
    printf '%s\n' "same-cohort harness refused a non-fresh target: $TARGET_DIR" >&2
    exit 2
  fi
else
  TEMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/forgecad-runtime-workers.XXXXXX")"
  TARGET_DIR="$TEMP_ROOT/cargo-target"
  trap 'rm -rf "$TEMP_ROOT"' EXIT
fi

mkdir -p "$TARGET_DIR"

build_worker() {
  local manifest="$1"
  local binary="$2"
  FORGECAD_BUILD_COHORT_SHA256="$BUILD_COHORT" CARGO_TARGET_DIR="$TARGET_DIR" \
    "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo build \
    --manifest-path "$PROJECT_ROOT/$manifest" \
    --bin "$binary" --offline
}

build_worker apps/geometry-worker/Cargo.toml forgecad-geometry-worker
build_worker apps/high-worker/Cargo.toml forgecad-high-worker
build_worker apps/render-worker/Cargo.toml forgecad-render-worker

test -x "$TARGET_DIR/debug/forgecad-geometry-worker"
test -x "$TARGET_DIR/debug/forgecad-high-worker"
test -x "$TARGET_DIR/debug/forgecad-render-worker"

# Build the Runtime executable as an authenticated identity witness in the
# same target.  The test binary is compiled with the same environment below;
# this separate witness makes a missing/changed compile-time cohort visible
# before the full suite starts.
FORGECAD_BUILD_COHORT_SHA256="$BUILD_COHORT" CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo build \
  --manifest-path "$PROJECT_ROOT/apps/desktop/src-tauri/Cargo.toml" \
  -p forgecad-runtime --bin forgecad-runtime --offline

assert_identity() {
  local binary="$1"
  local expected_component="$2"
  local expected_schema="$3"
  local identity
  identity="$("$binary" --build-identity)"
  EXPECTED_BUILD_COHORT="$BUILD_COHORT" EXPECTED_COMPONENT="$expected_component" \
    EXPECTED_SCHEMA="$expected_schema" \
    ACTUAL_IDENTITY="$identity" python3 -c '
import json
import os

try:
    identity = json.loads(os.environ["ACTUAL_IDENTITY"])
except (KeyError, json.JSONDecodeError) as error:
    raise SystemExit(f"same-cohort harness refused invalid build identity: {error}")

if identity.get("schema_version") != os.environ["EXPECTED_SCHEMA"]:
    raise SystemExit("same-cohort harness refused an unexpected build identity schema")
if identity.get("component") != os.environ["EXPECTED_COMPONENT"]:
    raise SystemExit("same-cohort harness refused an unexpected build identity component")
if identity.get("build_cohort_sha256") != os.environ["EXPECTED_BUILD_COHORT"]:
    raise SystemExit("same-cohort harness refused a build identity cohort mismatch")
'
}

assert_identity \
  "$TARGET_DIR/debug/forgecad-geometry-worker" \
  forgecad-geometry-worker ForgeCADDevBuildIdentity@1
assert_identity \
  "$TARGET_DIR/debug/forgecad-high-worker" \
  forgecad-high-worker ForgeCADHighWorkerBuildIdentity@1
assert_identity \
  "$TARGET_DIR/debug/forgecad-render-worker" \
  forgecad-render-worker ForgeCADDevBuildIdentity@1
assert_identity \
  "$TARGET_DIR/debug/forgecad-runtime" \
  forgecad-runtime ForgeCADDevBuildIdentity@1

CURRENT_SOURCE_COHORT="$(python3 "$PROJECT_ROOT/scripts/compute_build_cohort.py")"
if [[ "$CURRENT_SOURCE_COHORT" != "$BUILD_COHORT" ]]; then
  printf '%s\n' "same-cohort harness refused source drift during build" >&2
  printf '%s\n' "expected=$BUILD_COHORT actual=$CURRENT_SOURCE_COHORT" >&2
  exit 1
fi

if [[ "$HARNESS_MODE" == "architecture-fast" ]]; then
  # The fast lane proves architecture ownership and the current knife-native
  # durable slice.  It deliberately does not execute animation, GLB, 2K bake,
  # visual or historical-database qualification.  Those remain in the default
  # full lane below and in the explicit ignored-test policy.
  FAST_LOG="$TARGET_DIR/runtime-architecture-fast.log"
  BASELINE_CHECK_JSON="$TARGET_DIR/runtime-architecture-baseline-check.json"
  python3 "$PROJECT_ROOT/scripts/check_weaponry_runtime_baseline.py" \
    | tee "$BASELINE_CHECK_JSON" | tee -a "$FAST_LOG"

  run_fast_step() {
    local label="$1"
    shift
    printf '%s\n' "WPN_ARCH_FAST_STEP=$label" | tee -a "$FAST_LOG"
    set +e
    FORGECAD_BUILD_COHORT_SHA256="$BUILD_COHORT" CARGO_TARGET_DIR="$TARGET_DIR" \
      "$@" 2>&1 | tee -a "$FAST_LOG"
    local status=${PIPESTATUS[0]}
    set -e
    if (( status != 0 )); then
      printf '%s\n' "WPN_ARCH_FAST_STEP_RESULT=$label:FAIL" | tee -a "$FAST_LOG"
      return "$status"
    fi
    printf '%s\n' "WPN_ARCH_FAST_STEP_RESULT=$label:PASS" | tee -a "$FAST_LOG"
  }

  CARGO="$PROJECT_ROOT/script/with_rust_toolchain.sh"
  MANIFEST="$PROJECT_ROOT/apps/desktop/src-tauri/Cargo.toml"
  run_fast_step contracts-domain-map \
    "$CARGO" cargo test --manifest-path "$MANIFEST" \
    -p forgecad-contracts --lib weaponry_domain_map --offline
  run_fast_step mcp-active-schema-closure \
    python3 "$PROJECT_ROOT/scripts/check_weaponry_mcp_schema_closure.py"
  run_fast_step store-repository-boundaries \
    "$CARGO" cargo test --manifest-path "$MANIFEST" \
    -p forgecad-store --lib repository_boundaries --offline
  run_fast_step runtime-service-boundaries \
    "$CARGO" cargo test --manifest-path "$MANIFEST" \
    -p forgecad-runtime --lib runtime_services --offline
  run_fast_step runtime-domain-router \
    "$CARGO" cargo test --manifest-path "$MANIFEST" \
    -p forgecad-runtime --lib runtime_operation_router --offline
  run_fast_step runtime-timeout-contract \
    "$CARGO" cargo test --manifest-path "$MANIFEST" \
    -p forgecad-runtime --lib request_timeout_covers_fixed_worker_and_transaction_budgets --offline
  run_fast_step runtime-authoring-transaction \
    "$CARGO" cargo test --manifest-path "$MANIFEST" \
    -p forgecad-runtime --lib authoring_mesh_transaction --offline
  run_fast_step runtime-knife-curve-graph \
    "$CARGO" cargo test --manifest-path "$MANIFEST" \
    -p forgecad-runtime --lib knife_curve_modifier_graph --offline
  run_fast_step runtime-knife-evaluated-mesh \
    "$CARGO" cargo test --manifest-path "$MANIFEST" \
    -p forgecad-runtime --lib knife_curve_evaluated_mesh --offline
  run_fast_step mcp-default-domain-router \
    "$CARGO" cargo test --manifest-path "$MANIFEST" \
    -p forgecad-mcp --bin forgecad-mcp domain_router --offline
  run_fast_step mcp-default-knife-profile \
    "$CARGO" cargo test --manifest-path "$MANIFEST" \
    -p forgecad-mcp --bin forgecad-mcp knife_tool_profile --offline
  run_fast_step mcp-default-active-schema \
    "$CARGO" cargo test --manifest-path "$MANIFEST" \
    -p forgecad-mcp --bin forgecad-mcp active_schema --offline
  run_fast_step mcp-compat-domain-router \
    "$CARGO" cargo test --manifest-path "$MANIFEST" \
    -p forgecad-mcp --bin forgecad-mcp --no-default-features \
    --features legacy-compatibility-registry domain_router --offline

  CURRENT_SOURCE_COHORT="$(python3 "$PROJECT_ROOT/scripts/compute_build_cohort.py")"
  if [[ "$CURRENT_SOURCE_COHORT" != "$BUILD_COHORT" ]]; then
    printf '%s\n' "same-cohort fast harness refused source drift during test execution" >&2
    printf '%s\n' "expected=$BUILD_COHORT actual=$CURRENT_SOURCE_COHORT" >&2
    exit 1
  fi

  HARNESS_FINISHED_AT="$(date +%s)"
  HARNESS_SECONDS="$((HARNESS_FINISHED_AT - HARNESS_STARTED_AT))"
  # The architecture lane is the frequent migration feedback loop.  Fifteen
  # minutes is a hard contract, not a documentation target; anything slower
  # belongs in the qualification lane below.
  FAST_MAX_SECONDS=900
  FAST_RECEIPT="$TARGET_DIR/weaponry-architecture-fast-receipt.json"
  BUILD_COHORT="$BUILD_COHORT" FAST_LOG="$FAST_LOG" \
    BASELINE_CHECK_JSON="$BASELINE_CHECK_JSON" HARNESS_SECONDS="$HARNESS_SECONDS" \
    FAST_MAX_SECONDS="$FAST_MAX_SECONDS" FAST_RECEIPT="$FAST_RECEIPT" python3 - <<'PY'
import json
import os
import re
from pathlib import Path

log_path = Path(os.environ["FAST_LOG"])
text = log_path.read_text(encoding="utf-8")
duration_seconds = int(os.environ["HARNESS_SECONDS"])
duration_budget_seconds = int(os.environ["FAST_MAX_SECONDS"])
summaries = re.findall(
    r"test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored;",
    text,
)
steps = re.findall(r"^WPN_ARCH_FAST_STEP_RESULT=([^:]+):(PASS|FAIL)$", text, re.MULTILINE)
receipt = {
    "schema_version": "WeaponryArchitectureFastGateReceipt@1",
    "task_id": "WPN-ARCH-BASELINE-FAST-003",
    "status": (
        "PASS"
        if duration_seconds <= duration_budget_seconds
        else "FAIL_DURATION_BUDGET"
    ),
    "build_cohort_sha256": os.environ["BUILD_COHORT"],
    "four_build_identities_verified": True,
    "source_drift_detected": False,
    "timeout_contract_seconds": 180,
    "ignored_test_policy": json.loads(
        Path(os.environ["BASELINE_CHECK_JSON"]).read_text(encoding="utf-8")
    ),
    "steps": [{"name": name, "status": status} for name, status in steps],
    "test_summary": {
        "passed": sum(int(row[1]) for row in summaries),
        "failed": sum(int(row[2]) for row in summaries),
        "ignored": sum(int(row[3]) for row in summaries),
        "result_lines": len(summaries),
    },
    "duration_seconds": duration_seconds,
    "duration_budget_seconds": duration_budget_seconds,
    "qualification_lane_preserved": "script/test_runtime_workers.sh",
    "qualification_lane_status": "PRESERVED_NOT_EXECUTED_BY_FAST",
    "ignored_tests_executed_by_fast": 0,
    "ignored_tests_execution_claim": "NOT_PROVEN",
    "quality_claim": "architecture-regression-only; no visual or commercial promotion",
}
Path(os.environ["FAST_RECEIPT"]).write_text(
    json.dumps(receipt, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
    encoding="utf-8",
)
print(json.dumps(receipt, ensure_ascii=False, separators=(",", ":"), sort_keys=True))
PY

  printf '%s\n' "SAME_COHORT_ARCHITECTURE_FAST_SECONDS=$HARNESS_SECONDS"
  printf '%s\n' "SAME_COHORT_ARCHITECTURE_FAST_BUDGET_SECONDS=$FAST_MAX_SECONDS"
  printf '%s\n' "SAME_COHORT_BUILD_COHORT_SHA256=$BUILD_COHORT"
  printf '%s\n' "SAME_COHORT_TARGET_DIR=$TARGET_DIR"
  printf '%s\n' "SAME_COHORT_ARCHITECTURE_FAST_LOG=$FAST_LOG"
  printf '%s\n' "SAME_COHORT_ARCHITECTURE_FAST_RECEIPT=$FAST_RECEIPT"
  if (( HARNESS_SECONDS > FAST_MAX_SECONDS )); then
    printf '%s\n' "SAME_COHORT_ARCHITECTURE_FAST=FAIL_DURATION_BUDGET"
    exit 3
  fi
  printf '%s\n' "SAME_COHORT_ARCHITECTURE_FAST=PASS"
  exit 0
fi

# The Runtime resolves only these fixed sibling names beside the test binary.
# Keep the final command free of the test-only fallback features so this gate
# exercises the same isolated Worker transport as a normal source build.
TEST_LOG="$TARGET_DIR/runtime-test.log"
set +e
FORGECAD_BUILD_COHORT_SHA256="$BUILD_COHORT" CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo test \
  --manifest-path "$PROJECT_ROOT/apps/desktop/src-tauri/Cargo.toml" \
  -p forgecad-runtime --lib --offline "$@" 2>&1 | tee "$TEST_LOG"
TEST_STATUS=${PIPESTATUS[0]}
set -e

CURRENT_SOURCE_COHORT="$(python3 "$PROJECT_ROOT/scripts/compute_build_cohort.py")"
if [[ "$CURRENT_SOURCE_COHORT" != "$BUILD_COHORT" ]]; then
  printf '%s\n' "same-cohort harness refused source drift during test execution" >&2
  printf '%s\n' "expected=$BUILD_COHORT actual=$CURRENT_SOURCE_COHORT" >&2
  exit 1
fi

TEST_SUMMARY="$(rg '^test result:' "$TEST_LOG" | tail -1 || true)"
if [[ -n "$TEST_SUMMARY" ]]; then
  printf '%s\n' "SAME_COHORT_RUNTIME_TEST_SUMMARY=$TEST_SUMMARY"
else
  printf '%s\n' "SAME_COHORT_RUNTIME_TEST_SUMMARY=UNAVAILABLE"
fi
printf '%s\n' "SAME_COHORT_BUILD_COHORT_SHA256=$BUILD_COHORT"
printf '%s\n' "SAME_COHORT_TARGET_DIR=$TARGET_DIR"
printf '%s\n' "SAME_COHORT_RUNTIME_TEST_LOG=$TEST_LOG"

if (( TEST_STATUS != 0 )); then
  printf '%s\n' "SAME_COHORT_RUNTIME_SUITE=FAIL"
  exit "$TEST_STATUS"
fi

printf '%s\n' "SAME_COHORT_RUNTIME_SUITE=PASS"
