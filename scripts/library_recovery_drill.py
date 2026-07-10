#!/usr/bin/env python3
"""Benchmark a complete ForgeCAD Library backup and recovery drill.

The drill reuses the production backup, verification, and restore functions. It
then starts the real local Agent against the restored Library and reads back
projects, versions, module records, and every registered module GLB.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import platform
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable, TypeVar

from library_backup import (
    DATABASE_NAME,
    LibraryBackupError,
    create_backup,
    restore_backup,
    verify_backup,
)


ROOT = Path(__file__).resolve().parents[1]
REPORT_SCHEMA = "ForgeCADLibraryRecoveryDrillReport@1"
REPORT_NAME = "recovery-drill-report.json"
KNOWN_FIXTURE_GENERATOR_MARKERS = (
    "forgecad deterministic reference pack",
    "forgecad module-pack smoke",
)
EVIDENCE_CLASSES = (
    "unclassified",
    "reference_fixture",
    "representative_user_library",
    "formal_blender_10_12",
)
CAPACITY_KEYS = (
    "database_bytes",
    "reference_rows",
    "unique_object_count",
    "unique_object_bytes",
    "logical_object_bytes",
    "deduplicated_bytes",
    "backup_payload_bytes",
    "source_object_store_file_count",
    "source_object_store_bytes",
    "unreferenced_candidate_count",
    "unreferenced_candidate_bytes",
)
T = TypeVar("T")


class RecoveryDrillError(RuntimeError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


def run_recovery_drill(
    library_root: Path,
    output: Path,
    *,
    repeats: int = 1,
    evidence_class: str = "unclassified",
    baseline_report: Path | None = None,
    retain_artifacts: bool = False,
    agent_timeout_seconds: float = 20.0,
) -> dict[str, Any]:
    source_root = library_root.expanduser().resolve()
    source_database = source_root / DATABASE_NAME
    if not source_root.is_dir() or not source_database.is_file():
        raise RecoveryDrillError(
            "SOURCE_LIBRARY_NOT_FOUND",
            f"ForgeCAD Library database was not found under {source_root}.",
        )
    if not 1 <= repeats <= 10:
        raise RecoveryDrillError(
            "INVALID_REPEAT_COUNT", "Recovery drill repeats must be between 1 and 10."
        )
    if evidence_class not in EVIDENCE_CLASSES:
        raise RecoveryDrillError(
            "INVALID_EVIDENCE_CLASS",
            f"Evidence class must be one of {', '.join(EVIDENCE_CLASSES)}.",
        )
    if not 1 <= agent_timeout_seconds <= 300:
        raise RecoveryDrillError(
            "INVALID_AGENT_TIMEOUT",
            "Agent timeout must be between 1 and 300 seconds.",
        )

    destination = output.expanduser().resolve()
    if _is_within(destination, source_root):
        raise RecoveryDrillError(
            "DRILL_DESTINATION_INSIDE_LIBRARY",
            "Recovery drill output must be outside the source Library.",
        )
    if destination.exists() or destination.is_symlink():
        raise RecoveryDrillError(
            "DRILL_DESTINATION_EXISTS",
            f"Destination already exists; refusing to overwrite: {destination}.",
        )

    baseline = _load_baseline(baseline_report)
    destination.parent.mkdir(parents=True, exist_ok=True)
    staging = Path(
        tempfile.mkdtemp(prefix=f".{destination.name}.tmp-", dir=destination.parent)
    )
    started_at = _utc_now()
    started_ns = time.perf_counter_ns()
    runs: list[dict[str, Any]] = []
    try:
        for run_number in range(1, repeats + 1):
            run_root = staging / "runs" / f"run-{run_number:03d}"
            backup_root = run_root / "backup"
            restored_root = run_root / "restored-library"

            backup, backup_ms = _timed(lambda: create_backup(source_root, backup_root))
            verified, verify_ms = _timed(lambda: verify_backup(backup_root))
            restored, restore_ms = _timed(
                lambda: restore_backup(backup_root, restored_root)
            )
            readback, readback_ms = _timed(
                lambda: _agent_readback(restored_root, agent_timeout_seconds)
            )
            manifest = _read_json(backup_root / "backup-manifest.json")
            capacity = backup["capacity"]
            runs.append(
                {
                    "run": run_number,
                    "backup_id": backup["backup_id"],
                    "source_snapshot": {
                        "database_sha256": manifest["database"]["sha256"],
                        "object_set_sha256": _object_set_sha256(manifest["objects"]),
                        "schema_versions": manifest["database"]["schema_versions"],
                        "table_counts": manifest["database"]["table_counts"],
                    },
                    "capacity": capacity,
                    "duration_ms": {
                        "backup_and_internal_verify": backup_ms,
                        "independent_verify": verify_ms,
                        "restore_and_verify": restore_ms,
                        "agent_start_and_readback": readback_ms,
                        "total": round(
                            backup_ms + verify_ms + restore_ms + readback_ms, 3
                        ),
                    },
                    "throughput_bytes_per_second": {
                        "backup_payload": _throughput(
                            capacity["backup_payload_bytes"], backup_ms
                        ),
                        "independent_verify_payload": _throughput(
                            capacity["backup_payload_bytes"], verify_ms
                        ),
                        "restore_payload": _throughput(
                            capacity["backup_payload_bytes"], restore_ms
                        ),
                    },
                    "observed_directory_bytes": {
                        "backup": _directory_bytes(backup_root),
                        "restored_library": _directory_bytes(restored_root),
                    },
                    "verification": {
                        "backup": verified,
                        "restore": restored["restored_verification"],
                    },
                    "agent_readback": readback,
                    "artifact_paths": (
                        {
                            "backup": f"runs/run-{run_number:03d}/backup",
                            "restored_library": (
                                f"runs/run-{run_number:03d}/restored-library"
                            ),
                        }
                        if retain_artifacts
                        else None
                    ),
                }
            )

        _require_stable_source(runs)
        evidence = _classify_evidence(evidence_class, runs[0]["agent_readback"])
        completed_at = _utc_now()
        current_capacity = runs[0]["capacity"]
        report = {
            "schema_version": REPORT_SCHEMA,
            "drill_id": (
                f"drill_{datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ')}_"
                f"{runs[0]['source_snapshot']['database_sha256'][:12]}"
            ),
            "started_at": started_at,
            "completed_at": completed_at,
            "environment": {
                "platform": platform.platform(),
                "python": platform.python_version(),
                "agent_mode": "local_uvicorn_restored_library",
            },
            "evidence": evidence,
            "configuration": {
                "repeats": repeats,
                "artifacts_retained": retain_artifacts,
                "agent_timeout_seconds": agent_timeout_seconds,
                "source_path_recorded": False,
            },
            "summary": {
                "capacity": current_capacity,
                "duration_ms": _duration_summary(runs),
                "baseline_growth": _baseline_growth(
                    baseline, current_capacity, baseline_report
                ),
                "all_runs_verified": True,
                "all_agent_readbacks_verified": True,
                "stable_source_snapshot": True,
            },
            "runs": runs,
            "total_elapsed_ms": round(
                (time.perf_counter_ns() - started_ns) / 1_000_000, 3
            ),
            "limitations": [
                "timings are local wall-clock observations, not service-level objectives",
                "observed directory bytes are completed payload sizes, not peak disk usage",
                "evidence class is operator-declared and only known fixture generators are rejected",
                "this drill does not provide encryption, remote replication, WORM, or legal hold",
                "unreferenced candidates are measured but never deleted",
            ],
        }
        if not retain_artifacts:
            shutil.rmtree(staging / "runs", ignore_errors=True)
        _write_json(staging / REPORT_NAME, report)
        staging.rename(destination)
        return {
            "ok": True,
            "operation": "recovery_drill",
            "drill_id": report["drill_id"],
            "report_path": str(destination / REPORT_NAME),
            "evidence": evidence,
            "summary": report["summary"],
        }
    except Exception:
        shutil.rmtree(staging, ignore_errors=True)
        raise


def _agent_readback(library_root: Path, timeout_seconds: float) -> dict[str, Any]:
    port = _free_port()
    base_url = f"http://127.0.0.1:{port}"
    environment = os.environ.copy()
    environment["WUSHEN_LIBRARY_ROOT"] = str(library_root)
    environment["WUSHEN_MIGRATIONS_DIR"] = str(ROOT / "migrations")
    environment["WUSHEN_LOCAL_WORKER_ENABLED"] = "0"
    environment["FORGECAD_CONCEPT_PLANNER_PROVIDER"] = "deterministic_rules"
    process = subprocess.Popen(
        [
            sys.executable,
            "-m",
            "uvicorn",
            "wushen_agent.main:create_app",
            "--factory",
            "--host",
            "127.0.0.1",
            "--port",
            str(port),
        ],
        cwd=ROOT,
        env=environment,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    try:
        _wait_for_agent(base_url, process, timeout_seconds)
        projects = _http_json(base_url, "/api/v1/projects")
        project_items = _require_items(projects, "project list")
        version_count = 0
        current_version_count = 0
        for item in project_items:
            project_id = str(item.get("project_id", ""))
            if not project_id:
                raise RecoveryDrillError(
                    "AGENT_READBACK_INVALID", "Project list contained no project_id."
                )
            detail = _http_json(base_url, f"/api/v1/projects/{project_id}")
            versions = _require_items(
                {"items": detail.get("versions")}, f"project {project_id} versions"
            )
            version_count += len(versions)
            if detail.get("current_version_id"):
                current_version_count += 1

        modules = _http_json(base_url, "/api/v1/module-assets")
        module_items = _require_items(modules, "module list")
        module_generators: dict[str, int] = {}
        downloaded_bytes = 0
        for item in module_items:
            manifest = item.get("manifest")
            if not isinstance(manifest, dict):
                raise RecoveryDrillError(
                    "AGENT_READBACK_INVALID", "Module list contained no manifest."
                )
            module_id = str(manifest.get("module_id", ""))
            expected_sha256 = str(manifest.get("sha256", ""))
            payload, headers = _http_bytes(
                base_url, f"/api/v1/module-assets/{module_id}/file"
            )
            actual_sha256 = hashlib.sha256(payload).hexdigest()
            if actual_sha256 != expected_sha256:
                raise RecoveryDrillError(
                    "AGENT_MODULE_HASH_MISMATCH",
                    f"Restored Agent returned the wrong payload for module {module_id}.",
                )
            header_sha256 = headers.get("X-Content-SHA256")
            if header_sha256 != expected_sha256:
                raise RecoveryDrillError(
                    "AGENT_MODULE_HEADER_HASH_MISMATCH",
                    f"Restored Agent returned the wrong hash header for module {module_id}.",
                )
            generator = _glb_generator(payload)
            module_generators[generator] = module_generators.get(generator, 0) + 1
            downloaded_bytes += len(payload)
        known_fixture_modules = sum(
            count
            for generator, count in module_generators.items()
            if _is_known_fixture_generator(generator)
        )
        return {
            "status": "verified",
            "health": "ok",
            "project_count": len(project_items),
            "project_version_count": version_count,
            "projects_with_current_version": current_version_count,
            "module_count": len(module_items),
            "module_download_count": len(module_items),
            "module_download_bytes": downloaded_bytes,
            "module_hashes_verified": True,
            "module_generators": dict(sorted(module_generators.items())),
            "known_fixture_module_count": known_fixture_modules,
        }
    finally:
        _stop_process(process)


def _classify_evidence(declared_class: str, readback: dict[str, Any]) -> dict[str, Any]:
    module_count = int(readback["module_count"])
    fixture_count = int(readback["known_fixture_module_count"])
    eligible = (
        declared_class == "formal_blender_10_12"
        and 10 <= module_count <= 12
        and fixture_count == 0
    )
    if declared_class == "formal_blender_10_12" and not eligible:
        reasons: list[str] = []
        if not 10 <= module_count <= 12:
            reasons.append(f"module_count={module_count}, expected 10-12")
        if fixture_count:
            reasons.append(f"known_fixture_module_count={fixture_count}")
        raise RecoveryDrillError(
            "FORMAL_ASSET_EVIDENCE_REJECTED",
            "Formal asset evidence requirements failed: " + ", ".join(reasons) + ".",
        )
    return {
        "declared_class": declared_class,
        "formal_asset_evidence_eligible": eligible,
        "module_count": module_count,
        "known_fixture_module_count": fixture_count,
        "automated_checks": [
            "restored module count",
            "known deterministic/smoke GLB generator rejection",
            "all restored module payload hashes",
        ],
        "manual_review_still_required": declared_class == "formal_blender_10_12",
    }


def _wait_for_agent(
    base_url: str, process: subprocess.Popen[str], timeout_seconds: float
) -> None:
    deadline = time.monotonic() + timeout_seconds
    while time.monotonic() < deadline:
        if process.poll() is not None:
            output = process.stdout.read() if process.stdout else ""
            raise RecoveryDrillError(
                "RESTORED_AGENT_EXITED",
                f"Restored Agent exited before health check: {output[-2000:]}",
            )
        try:
            response = _http_json(base_url, "/api/health", timeout=1.0)
            if response.get("status") == "ok":
                return
        except RecoveryDrillError:
            time.sleep(0.1)
    raise RecoveryDrillError(
        "RESTORED_AGENT_TIMEOUT", "Restored Agent health check timed out."
    )


def _stop_process(process: subprocess.Popen[str]) -> None:
    process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)


def _http_json(base_url: str, path: str, *, timeout: float = 10.0) -> dict[str, Any]:
    payload, _ = _http_bytes(base_url, path, timeout=timeout)
    try:
        value = json.loads(payload)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise RecoveryDrillError(
            "AGENT_READBACK_INVALID", f"Agent returned invalid JSON for {path}: {exc}."
        ) from exc
    if not isinstance(value, dict):
        raise RecoveryDrillError(
            "AGENT_READBACK_INVALID", f"Agent returned a non-object for {path}."
        )
    return value


def _http_bytes(
    base_url: str, path: str, *, timeout: float = 30.0
) -> tuple[bytes, Any]:
    request = urllib.request.Request(f"{base_url}{path}", method="GET")
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            return response.read(), response.headers
    except (urllib.error.URLError, TimeoutError) as exc:
        raise RecoveryDrillError(
            "AGENT_READBACK_FAILED", f"GET {path} failed: {exc}."
        ) from exc


def _require_items(value: dict[str, Any], label: str) -> list[dict[str, Any]]:
    items = value.get("items")
    if not isinstance(items, list) or not all(isinstance(item, dict) for item in items):
        raise RecoveryDrillError(
            "AGENT_READBACK_INVALID", f"Restored Agent {label} is not an item list."
        )
    return items


def _glb_generator(payload: bytes) -> str:
    if len(payload) < 20 or payload[:4] != b"glTF":
        return "invalid_or_non_glb"
    chunk_length = int.from_bytes(payload[12:16], "little")
    chunk_type = int.from_bytes(payload[16:20], "little")
    if chunk_type != 0x4E4F534A or 20 + chunk_length > len(payload):
        return "invalid_or_non_glb"
    try:
        document = json.loads(payload[20 : 20 + chunk_length].rstrip(b" \x00"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        return "invalid_or_non_glb"
    asset = document.get("asset") if isinstance(document, dict) else None
    generator = asset.get("generator") if isinstance(asset, dict) else None
    return str(generator or "unspecified")


def _is_known_fixture_generator(generator: str) -> bool:
    normalized = generator.casefold()
    return any(marker in normalized for marker in KNOWN_FIXTURE_GENERATOR_MARKERS)


def _require_stable_source(runs: list[dict[str, Any]]) -> None:
    first = runs[0]
    fingerprint = first["source_snapshot"]
    capacity = first["capacity"]
    for run in runs[1:]:
        if run["source_snapshot"] != fingerprint or run["capacity"] != capacity:
            raise RecoveryDrillError(
                "SOURCE_CHANGED_DURING_DRILL",
                "Source Library changed between recovery drill runs; stop writers and retry.",
            )


def _duration_summary(runs: list[dict[str, Any]]) -> dict[str, Any]:
    keys = tuple(runs[0]["duration_ms"])
    return {
        key: _distribution([float(run["duration_ms"][key]) for run in runs])
        for key in keys
    }


def _distribution(values: list[float]) -> dict[str, float]:
    ordered = sorted(values)
    middle = len(ordered) // 2
    median = (
        ordered[middle]
        if len(ordered) % 2
        else (ordered[middle - 1] + ordered[middle]) / 2
    )
    p95 = ordered[max(0, math.ceil(len(ordered) * 0.95) - 1)]
    return {
        "min": round(ordered[0], 3),
        "median": round(median, 3),
        "p95": round(p95, 3),
        "max": round(ordered[-1], 3),
    }


def _baseline_growth(
    baseline: dict[str, Any] | None,
    current_capacity: dict[str, Any],
    baseline_report: Path | None,
) -> dict[str, Any] | None:
    if baseline is None or baseline_report is None:
        return None
    baseline_capacity = baseline["summary"]["capacity"]
    return {
        "baseline_report_sha256": _sha256_file(baseline_report.expanduser().resolve()),
        "capacity_delta": {
            key: int(current_capacity[key]) - int(baseline_capacity[key])
            for key in CAPACITY_KEYS
        },
    }


def _load_baseline(path: Path | None) -> dict[str, Any] | None:
    if path is None:
        return None
    resolved = path.expanduser().resolve()
    if not resolved.is_file():
        raise RecoveryDrillError(
            "BASELINE_REPORT_NOT_FOUND", f"Baseline report was not found: {resolved}."
        )
    value = _read_json(resolved)
    if value.get("schema_version") != REPORT_SCHEMA:
        raise RecoveryDrillError(
            "BASELINE_REPORT_INVALID",
            f"Baseline report must use {REPORT_SCHEMA}.",
        )
    capacity = value.get("summary", {}).get("capacity")
    if not isinstance(capacity, dict) or any(
        key not in capacity for key in CAPACITY_KEYS
    ):
        raise RecoveryDrillError(
            "BASELINE_REPORT_INVALID", "Baseline report capacity is incomplete."
        )
    return value


def _object_set_sha256(objects: Any) -> str:
    canonical = json.dumps(
        objects, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")
    return hashlib.sha256(canonical).hexdigest()


def _throughput(byte_size: int, duration_ms: float) -> int:
    return round(byte_size / max(duration_ms / 1000, 0.000001))


def _directory_bytes(root: Path) -> int:
    return sum(path.stat().st_size for path in root.rglob("*") if path.is_file())


def _timed(operation: Callable[[], T]) -> tuple[T, float]:
    started = time.perf_counter_ns()
    result = operation()
    return result, round((time.perf_counter_ns() - started) / 1_000_000, 3)


def _free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as handle:
        handle.bind(("127.0.0.1", 0))
        return int(handle.getsockname()[1])


def _is_within(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
        return True
    except ValueError:
        return False


def _read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise RecoveryDrillError(
            "REPORT_JSON_INVALID", f"Could not read JSON file {path.name}: {exc}."
        ) from exc
    if not isinstance(value, dict):
        raise RecoveryDrillError(
            "REPORT_JSON_INVALID", f"JSON file {path.name} must contain an object."
        )
    return value


def _write_json(path: Path, value: dict[str, Any]) -> None:
    path.write_text(
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _utc_now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Measure backup, verification, restore, and restored Agent readback for a "
            "ForgeCAD Library."
        )
    )
    parser.add_argument("--library-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repeats", type=int, default=1)
    parser.add_argument(
        "--evidence-class", choices=EVIDENCE_CLASSES, default="unclassified"
    )
    parser.add_argument("--baseline-report", type=Path)
    parser.add_argument("--retain-artifacts", action="store_true")
    parser.add_argument("--agent-timeout-seconds", type=float, default=20.0)
    return parser


def main() -> int:
    args = _parser().parse_args()
    try:
        result = run_recovery_drill(
            args.library_root,
            args.output,
            repeats=args.repeats,
            evidence_class=args.evidence_class,
            baseline_report=args.baseline_report,
            retain_artifacts=args.retain_artifacts,
            agent_timeout_seconds=args.agent_timeout_seconds,
        )
    except (RecoveryDrillError, LibraryBackupError) as exc:
        code = getattr(exc, "code", "RECOVERY_DRILL_FAILED")
        print(
            json.dumps(
                {"ok": False, "error": {"code": code, "message": str(exc)}},
                ensure_ascii=False,
                indent=2,
            ),
            file=sys.stderr,
        )
        return 2
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
