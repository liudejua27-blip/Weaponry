#!/usr/bin/env python3
"""Focused V003 single-generation contract smoke.

This is deliberately a thin, fail-closed runner around the Rust integration
fixture.  The fixture is the oracle: this script neither recreates the V003
state machine nor accepts source-text evidence.  It first verifies that every
required contract facet is still present in the real Cargo test target, then
runs that target offline through the repository-owned Rust toolchain wrapper.
"""

from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys
import time


ROOT = Path(__file__).resolve().parents[1]
RUST_WRAPPER = ROOT / "script" / "with_rust_toolchain.sh"
MANIFEST = ROOT / "apps" / "desktop" / "src-tauri" / "Cargo.toml"
PACKAGE = "forgecad-core"
TARGET = "single_generation"
SCHEMA_VERSION = "V003SingleGenerationSmoke@1"
TIMEOUT_SECONDS = 900

# These test names identify coverage facets, rather than duplicating their
# product decisions in Python.  If a later Rust change removes a facet, the
# smoke fails before it can report a green target merely because fewer tests
# happened to run.
REQUIRED_TESTS = (
    "passing_every_hard_gate_yields_one_transient_preview_not_a_version",
    "undetermined_gate_is_never_aggregated_as_a_passing_result_or_repair",
    "repairs_are_same_intent_limited_to_two_and_cannot_escape_failed_field_scope",
    "repair_rejects_brief_or_recipe_intent_drift",
    "third_repair_is_rejected_after_the_two_repair_budget_is_consumed",
    "cancellation_has_no_preview_and_is_terminal",
)


class SmokeFailure(RuntimeError):
    pass


def cargo_command(*arguments: str) -> list[str]:
    return [
        str(RUST_WRAPPER),
        "cargo",
        "test",
        "--manifest-path",
        str(MANIFEST),
        "-p",
        PACKAGE,
        "--test",
        TARGET,
        "--offline",
        *arguments,
    ]


def run(command: list[str]) -> subprocess.CompletedProcess[str]:
    print(f"[v003-single-generation] {' '.join(command)}", flush=True)
    try:
        completed = subprocess.run(
            command,
            cwd=ROOT,
            text=True,
            capture_output=True,
            check=False,
            timeout=TIMEOUT_SECONDS,
        )
    except FileNotFoundError as exc:
        raise SmokeFailure("V003_RUST_TOOLCHAIN_MISSING") from exc
    except subprocess.TimeoutExpired as exc:
        raise SmokeFailure("V003_RUST_TEST_TIMEOUT") from exc
    if completed.stdout:
        print(completed.stdout, end="" if completed.stdout.endswith("\n") else "\n")
    if completed.stderr:
        print(completed.stderr, end="" if completed.stderr.endswith("\n") else "\n", file=sys.stderr)
    if completed.returncode != 0:
        raise SmokeFailure(f"V003_RUST_TEST_FAILED (exit {completed.returncode})")
    return completed


def listed_tests() -> set[str]:
    completed = run(cargo_command("--", "--list"))
    return {
        line.rsplit(": test", 1)[0].strip()
        for line in completed.stdout.splitlines()
        if line.rstrip().endswith(": test")
    }


def main() -> int:
    if not RUST_WRAPPER.is_file() or not os.access(RUST_WRAPPER, os.X_OK):
        raise SmokeFailure("V003_RUST_WRAPPER_MISSING")
    if not MANIFEST.is_file():
        raise SmokeFailure("V003_RUST_MANIFEST_MISSING")

    available = listed_tests()
    missing = sorted(set(REQUIRED_TESTS) - available)
    if missing:
        raise SmokeFailure(
            "V003_RUST_CONTRACT_COVERAGE_MISSING: " + ", ".join(missing)
        )
    run(cargo_command())
    print(
        json.dumps(
            {
                "schema_version": SCHEMA_VERSION,
                "status": "pass",
                "package": PACKAGE,
                "test_target": TARGET,
                "required_tests": list(REQUIRED_TESTS),
                "contract_oracle": "real_rust_integration_tests",
            },
            ensure_ascii=False,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    started = time.monotonic()
    try:
        raise SystemExit(main())
    except SmokeFailure as exc:
        print(
            json.dumps(
                {
                    "schema_version": SCHEMA_VERSION,
                    "status": "fail",
                    "error_code": str(exc).split(":", 1)[0],
                    "duration_ms": int((time.monotonic() - started) * 1000),
                },
                ensure_ascii=False,
                sort_keys=True,
            ),
            file=sys.stderr,
        )
        raise SystemExit(1) from exc
