#!/usr/bin/env python3
"""Build the fail-closed E005 zero-provider baseline without generating assets."""

from __future__ import annotations

import copy
import json
import os
import socket
import subprocess
import tempfile
import time
from collections import Counter
from pathlib import Path
from typing import Any

from urllib.error import URLError
from urllib.request import urlopen

from validate_e005_unseen_task_set import (
    canonical_sha256,
    schema_registry,
    validate_distribution_report,
    validate_provider_run_authorization,
    validate_run_receipt,
    validate_schema,
    validate_structural_difference_matrix,
    validate_task_set,
)


ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "packages" / "concept-spec" / "fixtures"
TASK_SET_PATH = FIXTURE_ROOT / "e005-unseen-mechanical-hard-surface-task-set.json"
SOURCE_MANIFEST_PATH = FIXTURE_ROOT / "e005-author-source-manifest-not-authorized.json"
HARNESS_SOURCE_PATH = FIXTURE_ROOT / "e005-harness-sensor-pod-source.json"
HUMAN_REVIEW_PATH = FIXTURE_ROOT / "e005-human-review-not-run.json"
PROVIDER_AUTHORIZATION_PATH = (
    FIXTURE_ROOT / "e005-provider-run-authorization-not-authorized.json"
)
STRUCTURAL_MATRIX_PATH = (
    FIXTURE_ROOT / "e005-structural-difference-matrix-not-run.json"
)
RUST_MANIFEST = ROOT / "apps" / "desktop" / "src-tauri" / "Cargo.toml"


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def validate_source_manifest(
    manifest: dict[str, Any], task_set: dict[str, Any], task_set_sha256: str
) -> None:
    schemas, registry = schema_registry()
    validate_schema(schemas, registry, "e005-author-source-manifest-v1.schema.json", manifest)
    if manifest["task_set_sha256"] != task_set_sha256:
        raise ValueError("E005_SOURCE_MANIFEST_TASK_SET_HASH_MISMATCH")
    task_ids = [task["task_id"] for task in task_set["tasks"]]
    manifest_ids = [entry["task_id"] for entry in manifest["entries"]]
    if len(manifest_ids) != len(set(manifest_ids)):
        raise ValueError("E005_SOURCE_MANIFEST_TASK_DUPLICATE")
    if set(manifest_ids) != set(task_ids):
        raise ValueError("E005_SOURCE_MANIFEST_TASK_COVERAGE_INVALID")
    if manifest_ids != task_ids:
        raise ValueError("E005_SOURCE_MANIFEST_ORDER_INVALID")
    for entry in manifest["entries"]:
        if entry["source_status"] == "authored":
            actual_hash = canonical_sha256(entry["source_program"])
            if entry["source_program_sha256"] != actual_hash:
                raise ValueError(f"E005_SOURCE_PROGRAM_HASH_MISMATCH:{entry['task_id']}")


def not_run_receipt(
    entry: dict[str, Any], task: dict[str, Any], task_set_sha256: str
) -> dict[str, Any]:
    request = {
        "task_id": entry["task_id"],
        "task_payload_sha256": canonical_sha256(task),
        "author_source_mode": "missing",
        "failure_code": entry["failure_code"],
    }
    return {
        "schema_version": "E005RunReceipt@1",
        "run_id": f"run_{entry['task_id']}_not_run",
        "task_set_sha256": task_set_sha256,
        "task_id": entry["task_id"],
        "status": "not_run",
        "run_mode": "offline_deterministic",
        "distribution_eligible": False,
        "author_source_mode": "missing",
        "task_payload_sha256": canonical_sha256(task),
        "request_sha256": canonical_sha256(request),
        "authoring_count": 0,
        "patch_count": 0,
        "network_provider_calls": 0,
        "billable_cost_microusd": 0,
        "failure_codes": [entry["failure_code"]],
        "human_review_status": "not_run",
    }


def build_zero_provider_baseline(
    task_set: dict[str, Any],
    manifest: dict[str, Any],
    human_review_bundle: dict[str, Any],
    structural_matrix: dict[str, Any],
    provider_authorization: dict[str, Any],
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    task_set_sha256 = canonical_sha256(task_set)
    validate_source_manifest(manifest, task_set, task_set_sha256)
    tasks_by_id = {task["task_id"]: task for task in task_set["tasks"]}
    if validate_provider_run_authorization(
        provider_authorization,
        task_set_sha256=task_set_sha256,
    ):
        raise ValueError("E005_ZERO_BASELINE_PROVIDER_UNEXPECTEDLY_AUTHORIZED")
    receipts: list[dict[str, Any]] = []
    for entry in manifest["entries"]:
        if entry["source_status"] != "unavailable":
            raise ValueError(
                "E005_AUTHORED_SOURCE_REQUIRES_VP204_RUNTIME_HARNESS:"
                f"{entry['task_id']}"
            )
        receipt = not_run_receipt(entry, tasks_by_id[entry["task_id"]], task_set_sha256)
        validate_run_receipt(
            receipt, task_set_sha256=task_set_sha256, tasks_by_id=tasks_by_id
        )
        receipts.append(receipt)

    structural_summary = validate_structural_difference_matrix(
        structural_matrix,
        task_set_sha256=task_set_sha256,
        tasks_by_id=tasks_by_id,
        receipts=receipts,
    )

    failure_histogram = Counter(
        code for receipt in receipts for code in receipt["failure_codes"]
    )
    report = {
        "schema_version": "E005DistributionReport@1",
        "report_id": "e005_zero_provider_baseline_v1",
        "task_set_sha256": task_set_sha256,
        "provider_authorization_sha256": canonical_sha256(provider_authorization),
        "total_receipt_count": len(receipts),
        "run_count": 0,
        "not_run_count": len(receipts),
        "first_pass_success_count": 0,
        "patched_success_count": 0,
        "failed_count": 0,
        "cancelled_count": 0,
        "human_review_complete_count": 0,
        "human_review_receipt_count": 0,
        "human_review_bundle_sha256": canonical_sha256(human_review_bundle),
        "independent_reviewers_per_task_minimum": 0,
        "first_pass_human_quality_count": 0,
        "within_one_patch_human_quality_count": 0,
        "lineage_complete_count": 0,
        **structural_summary,
        "structural_matrix_sha256": canonical_sha256(structural_matrix),
        "formal_eligible": False,
        "failure_histogram": dict(sorted(failure_histogram.items())),
        "receipts_sha256": canonical_sha256(receipts),
    }
    validate_distribution_report(
        report,
        task_set_sha256=task_set_sha256,
        receipts=receipts,
        human_review_bundle=human_review_bundle,
        structural_matrix=structural_matrix,
        tasks_by_id=tasks_by_id,
        provider_authorization=provider_authorization,
    )
    return receipts, report


def free_loopback_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
        listener.bind(("127.0.0.1", 0))
        return int(listener.getsockname()[1])


def wait_for_health(base_url: str, process: subprocess.Popen[bytes]) -> None:
    deadline = time.monotonic() + 20.0
    while time.monotonic() < deadline:
        if process.poll() is not None:
            output = process.communicate(timeout=1)[0].decode("utf-8", errors="replace")
            raise AssertionError(f"E005_SIDECAR_EXITED:{output[-3000:]}")
        try:
            with urlopen(f"{base_url}/api/health", timeout=1.0) as response:
                payload = json.loads(response.read().decode("utf-8"))
            if payload.get("status") == "ok" and payload.get("mode") == "restricted_geometry_executor":
                return
        except (OSError, URLError, json.JSONDecodeError):
            pass
        time.sleep(0.05)
    raise AssertionError("E005_SIDECAR_HEALTH_TIMEOUT")


def assert_no_persistence(root: Path) -> None:
    forbidden = {"library.db", "library.db-shm", "library.db-wal", "forgecad.db", "forgecad.sqlite", "objects", "object_store"}
    violations = [path for path in root.rglob("*") if path.name in forbidden]
    if violations:
        raise AssertionError(f"E005_SIDECAR_PERSISTENCE_VIOLATION:{violations}")


def run_live_sidecar_receipt(task_set: dict[str, Any]) -> dict[str, Any]:
    capability = "e" * 64
    port = free_loopback_port()
    base_url = f"http://127.0.0.1:{port}"
    python = ROOT / ".venv" / "bin" / "python"
    with tempfile.TemporaryDirectory(prefix="forgecad-e005-sidecar-") as directory:
        temporary_root = Path(directory)
        temporary_tmp = temporary_root / "tmp"
        temporary_cache = temporary_root / "pycache"
        temporary_tmp.mkdir()
        temporary_cache.mkdir()
        sidecar_environment = {
            "PATH": os.pathsep.join([str(python.parent), "/usr/bin", "/bin"]),
            "PYTHONPATH": str(ROOT / "apps" / "agent"),
            "PYTHONUNBUFFERED": "1",
            "PYTHONDONTWRITEBYTECODE": "1",
            "PYTHONPYCACHEPREFIX": str(temporary_cache),
            "TMPDIR": str(temporary_tmp),
            "FORGECAD_RESTRICTED_GEOMETRY_CAPABILITY_TOKEN": capability,
        }
        process = subprocess.Popen(
            [
                str(python),
                "-m",
                "uvicorn",
                "wushen_agent.main:create_app",
                "--factory",
                "--host",
                "127.0.0.1",
                "--port",
                str(port),
                "--log-level",
                "warning",
                "--no-access-log",
            ],
            cwd=ROOT,
            env=sidecar_environment,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        cargo_output = ""
        try:
            wait_for_health(base_url, process)
            rust_environment = dict(os.environ)
            rust_environment["FORGECAD_E005_SIDECAR_ENDPOINT"] = base_url
            rust_environment["FORGECAD_E005_SIDECAR_CAPABILITY"] = capability
            result = subprocess.run(
                [
                    str(ROOT / "script" / "with_rust_toolchain.sh"),
                    "cargo",
                    "test",
                    "--manifest-path",
                    str(RUST_MANIFEST),
                    "-p",
                    "wushen-forge-desktop",
                    "app_server_bridge::tests::e005_offline_receipt_runs_through_live_restricted_sidecar",
                    "--offline",
                    "--",
                    "--ignored",
                    "--exact",
                    "--nocapture",
                ],
                cwd=ROOT,
                env=rust_environment,
                capture_output=True,
                text=True,
                timeout=240,
            )
            cargo_output = result.stdout + result.stderr
            if result.returncode != 0:
                raise AssertionError(f"E005_LIVE_RUST_TEST_FAILED:{cargo_output[-5000:]}")
        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
            sidecar_output = process.communicate(timeout=1)[0].decode("utf-8", errors="replace")
        if process.returncode not in {0, -15}:
            raise AssertionError(f"E005_SIDECAR_STOP_FAILED:{sidecar_output[-3000:]}")
        assert_no_persistence(temporary_root)

    prefix = "E005_RUN_RECEIPT_JSON="
    receipt_lines = [line for line in cargo_output.splitlines() if line.startswith(prefix)]
    if len(receipt_lines) != 1:
        raise AssertionError("E005_LIVE_RECEIPT_OUTPUT_MISSING")
    receipt = json.loads(receipt_lines[0][len(prefix):])
    tasks_by_id = {task["task_id"]: task for task in task_set["tasks"]}
    validate_run_receipt(
        receipt,
        task_set_sha256=canonical_sha256(task_set),
        tasks_by_id=tasks_by_id,
    )
    if receipt["distribution_eligible"] or receipt["human_review_status"] != "not_run":
        raise AssertionError("E005_OFFLINE_RECEIPT_FORMAL_LEAK")
    return receipt


def self_test(task_set: dict[str, Any], manifest: dict[str, Any]) -> None:
    task_set_sha256 = canonical_sha256(task_set)
    duplicate = copy.deepcopy(manifest)
    duplicate["entries"][1]["task_id"] = duplicate["entries"][0]["task_id"]
    try:
        validate_source_manifest(duplicate, task_set, task_set_sha256)
    except ValueError as error:
        if "E005_SOURCE_MANIFEST_TASK_DUPLICATE" not in str(error):
            raise
    else:
        raise AssertionError("E005 duplicate source task self-test did not fail")

    injected = copy.deepcopy(manifest)
    injected["entries"][0]["source_program_sha256"] = "0" * 64
    injected["entries"][0]["source_program"] = load_json(
        FIXTURE_ROOT / "forge-visual-geometry-v2-bracket.json"
    )
    try:
        validate_source_manifest(injected, task_set, task_set_sha256)
    except ValueError as error:
        if "E005_SCHEMA_INVALID" not in str(error):
            raise
    else:
        raise AssertionError("E005 unavailable source injection self-test did not fail")


def one_source_harness_manifest(
    task_set: dict[str, Any], baseline_manifest: dict[str, Any], source: dict[str, Any]
) -> dict[str, Any]:
    manifest = copy.deepcopy(baseline_manifest)
    manifest["manifest_id"] = "e005_one_source_harness_v1"
    entry = manifest["entries"][0]
    if entry["task_id"] != "e005_enclosure_sensor_pod":
        raise AssertionError("E005_HARNESS_SOURCE_TASK_ORDER_CHANGED")
    entry["authoring_mode"] = "frozen_offline_fixture"
    entry["source_status"] = "authored"
    entry.pop("failure_code")
    entry["source_program_sha256"] = canonical_sha256(source)
    entry["source_program"] = source
    validate_source_manifest(manifest, task_set, canonical_sha256(task_set))
    return manifest


def main() -> int:
    task_set = load_json(TASK_SET_PATH)
    manifest = load_json(SOURCE_MANIFEST_PATH)
    harness_source = load_json(HARNESS_SOURCE_PATH)
    human_review_bundle = load_json(HUMAN_REVIEW_PATH)
    structural_matrix = load_json(STRUCTURAL_MATRIX_PATH)
    provider_authorization = load_json(PROVIDER_AUTHORIZATION_PATH)
    contract = validate_task_set(task_set)
    receipts, report = build_zero_provider_baseline(
        task_set,
        manifest,
        human_review_bundle,
        structural_matrix,
        provider_authorization,
    )
    authored_manifest = one_source_harness_manifest(task_set, manifest, harness_source)
    authored_probe = run_live_sidecar_receipt(task_set)
    self_test(task_set, manifest)
    print(
        json.dumps(
            {
                "status": "not_run_baseline_pass",
                "task_set_sha256": contract["task_set_sha256"],
                "source_manifest_sha256": canonical_sha256(manifest),
                "receipt_count": len(receipts),
                "run_count": report["run_count"],
                "not_run_count": report["not_run_count"],
                "network_provider_calls": sum(
                    receipt["network_provider_calls"] for receipt in receipts
                ),
                "billable_cost_microusd": sum(
                    receipt["billable_cost_microusd"] for receipt in receipts
                ),
                "failure_histogram": report["failure_histogram"],
                "receipts_sha256": report["receipts_sha256"],
                "formal_eligible": report["formal_eligible"],
                "offline_authored_probe": authored_probe,
                "offline_authored_manifest_sha256": canonical_sha256(authored_manifest),
            },
            ensure_ascii=False,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
