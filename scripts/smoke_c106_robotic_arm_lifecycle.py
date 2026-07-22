#!/usr/bin/env python3
"""Run the C106 robotic-arm production lifecycle without a second state path.

The Rust bridge test owns Version/Snapshot/CAS/confirm/reject/navigation and
restart assertions.  The app-server test owns deterministic one-root V003
selection, while the existing production gate runs the same expanded C106
ShapePrograms through the capability-gated Python geometry executor.  This
wrapper only aggregates those real gates; it never reimplements lifecycle
state or supplies a Provider credential.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
RUST = ROOT / "script" / "with_rust_toolchain.sh"
MANIFEST = ROOT / "apps" / "desktop" / "src-tauri" / "Cargo.toml"
SCHEMA_VERSION = "C106RoboticArmLifecycleGate@1"


class GateFailure(RuntimeError):
    pass


def run(
    label: str,
    command: list[str],
    *,
    environment: dict[str, str] | None = None,
) -> str:
    try:
        completed = subprocess.run(
            command,
            cwd=ROOT,
            text=True,
            capture_output=True,
            timeout=900,
            check=False,
            env=environment,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise GateFailure(f"C106_LIFECYCLE_{label}_UNAVAILABLE") from exc
    if completed.returncode:
        detail = f"{completed.stdout}\n{completed.stderr}"[-5000:]
        raise GateFailure(f"C106_LIFECYCLE_{label}_FAILED: {detail}")
    if "cargo" in command[:2] and "test result: ok. 1 passed" not in completed.stdout:
        raise GateFailure(f"C106_LIFECYCLE_{label}_MISSING")
    if label == "RESTRICTED_EXECUTOR_CRASH_test" and "1 passed" not in completed.stdout:
        raise GateFailure(f"C106_LIFECYCLE_{label}_MISSING")
    return completed.stdout


def measured_lifecycle_evidence(path: Path) -> dict[str, Any]:
    try:
        evidence = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise GateFailure("C106_LIFECYCLE_MEASURED_EVIDENCE_MISSING") from exc
    required_sha256 = (
        "preview_glb_sha256",
        "production_glb_sha256",
        "export_glb_sha256",
    )
    has_valid_hashes = isinstance(evidence, dict) and all(
        isinstance(evidence.get(key), str)
        and len(evidence[key]) == 64
        and all(character in "0123456789abcdef" for character in evidence[key])
        for key in required_sha256
    )
    if (
        not isinstance(evidence, dict)
        or evidence.get("schema_version") != "C106LifecycleMeasuredEvidence@1"
        or evidence.get("measurement_source") != "FakeDeepSeekClient.records"
        or evidence.get("provider_policy") != "offline_deny_on_call"
        or evidence.get("measured_provider_calls") != 0
        or evidence.get("restart_readback") is not True
        or evidence.get("transient_cancel_reject_zero_persistent_writes") is not True
        or evidence.get("selected_root_recipe_id") != "recipe_c106_arm_service_display"
        or evidence.get("initial_asset_version_no") != 1
        or not isinstance(evidence.get("initial_snapshot_revision"), int)
        or evidence["initial_snapshot_revision"] < 1
        or not isinstance(evidence.get("v003_preview_triangle_count"), int)
        or evidence["v003_preview_triangle_count"] < 1
        or evidence.get("material_zone_count") != 19
        or not isinstance(evidence.get("a005_confirmed_asset_version_id"), str)
        or not isinstance(evidence.get("restart_asset_version_id"), str)
        or not isinstance(evidence.get("export_triangle_count"), int)
        or evidence["export_triangle_count"] < 1
        or not has_valid_hashes
    ):
        raise GateFailure("C106_LIFECYCLE_MEASURED_EVIDENCE_INVALID")
    return evidence


def cargo(package: str, test_name: str) -> list[str]:
    return [
        str(RUST),
        "cargo",
        "test",
        "--manifest-path",
        str(MANIFEST),
        "-p",
        package,
        test_name,
        "--offline",
    ]


def main(*, skip_production: bool = False) -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad-c106-lifecycle-evidence-") as directory:
        evidence_path = Path(directory) / "measured-evidence.json"
        environment = dict(os.environ)
        environment["FORGECAD_C106_LIFECYCLE_EVIDENCE_FILE"] = str(evidence_path)
        run(
            "RUST_LIFECYCLE_test",
            cargo(
                "wushen-forge-desktop",
                "c106_robotic_arm_single_result_uses_the_existing_atomic_lifecycle",
            ),
            environment=environment,
        )
        lifecycle_evidence = measured_lifecycle_evidence(evidence_path)
    run(
        "RUST_SINGLE_ROOT_test",
        cargo(
            "forgecad-app-server",
            "c106_arm_style_routes_one_deterministic_root_without_changing_c105_catalogs",
        ),
    )
    run(
        "RUST_CANCEL_LATE_test",
        cargo(
            "forgecad-app-server",
            "in_flight_cancel_discards_late_geometry_and_never_promotes_state",
        ),
    )
    python_environment = dict(os.environ)
    python_environment["PYTHONPATH"] = str(ROOT / "apps" / "agent")
    run(
        "RESTRICTED_EXECUTOR_CRASH_test",
        [
            str(ROOT / ".venv" / "bin" / "python"),
            "-m",
            "pytest",
            "-q",
            "apps/agent/tests/test_k003_restricted_geometry_executor.py::test_disposable_worker_timeout_crash_and_late_result_tombstone",
        ],
        environment=python_environment,
    )
    if not skip_production:
        run(
            "RESTRICTED_GEOMETRY",
            ["npm", "run", "agent:c106-robotic-arm-production-gate"],
        )
    print(
        json.dumps(
            {
                "schema_version": SCHEMA_VERSION,
                "status": "pass",
                "measured_provider_calls": lifecycle_evidence["measured_provider_calls"],
                "provider_measurement": lifecycle_evidence,
                "golden_path_steps": [
                    "single_result_preview",
                    "a005_preview_confirm",
                    "undo_redo",
                    "restart_recovery",
                    "glb_export",
                ],
                "negative_evidence": [
                    "transient_cancel_reject_zero_persistent_writes",
                    "in_flight_cancel_discards_late_geometry_and_never_promotes_state",
                    "restricted_executor_timeout_crash_and_late_result_tombstone",
                ],
                "python_capability": "restricted_geometry_only",
                "formal_eligible": False,
                "production_gate": "skipped_by_aggregate" if skip_production else "passed",
            },
            ensure_ascii=False,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Run the C106 robotic-arm lifecycle gate.")
    parser.add_argument(
        "--skip-production",
        action="store_true",
        help="run lifecycle/state regressions only; reserved for the aggregate C106 gate",
    )
    arguments = parser.parse_args()
    try:
        raise SystemExit(main(skip_production=arguments.skip_production))
    except GateFailure as exc:
        print(
            json.dumps(
                {
                    "schema_version": SCHEMA_VERSION,
                    "status": "fail",
                    "error": str(exc),
                    "formal_eligible": False,
                },
                ensure_ascii=False,
                sort_keys=True,
            ),
            file=sys.stderr,
        )
        raise SystemExit(1) from None
