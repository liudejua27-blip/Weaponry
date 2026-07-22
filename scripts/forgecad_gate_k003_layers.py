#!/usr/bin/env python3
"""Run the five K003 operational gate layers in order, without rebuilding.

This is an integration runner only.  It does not replace any layer's source
of truth and never invokes the legacy K001/K002 gate chains.  A failed layer
stops the sequence, while the bounded report and artifact manifest remain
available for diagnosis.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time
from typing import Any, Mapping, Sequence


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_ARTIFACT_DIR = ROOT / "output" / "k003-layered-gate"
REPORT_SCHEMA = "ForgeCADK003LayeredGateReport@1"
MANIFEST_SCHEMA = "ForgeCADK003ArtifactManifest@1"
LAYER_ORDER = ("host", "rust_core", "rust_python_contract", "packaged", "workbench")
LAYER_REPORT_SCHEMAS = {
    "host": "ForgeCADHostPreflightReport@1",
    "rust_core": "ForgeCADRustCoreGateReport@1",
    "rust_python_contract": "ForgeCADRustPythonContractGateReport@1",
    "packaged": "ForgeCADK003PackagedSmoke@1",
    "workbench": "ForgeCADWorkbenchE2EGateReport@1",
}
LAYER_CONTRACT_FIELDS: dict[str, Mapping[str, Any]] = {
    "host": {"phase": "host_preflight", "subsystem": "host"},
    "rust_core": {"layer": 2, "phase": "gate", "subsystem": "forgecad-core"},
    "rust_python_contract": {"phase": "gate.rust_python_contract", "subsystem": "rust_python_boundary"},
    "packaged": {},
    "workbench": {"phase": "workbench_e2e", "subsystem": "desktop_workbench"},
}
EXIT_CODES = {
    "host": 21,
    "rust_core": 22,
    "rust_python_contract": 23,
    "packaged": 24,
    "workbench": 25,
    "internal": 30,
}
PASS_STATUSES = {"pass", "passed", "ok", "pass_with_warnings"}
FAIL_STATUSES = {"fail", "failed", "blocked", "hard_fail", "warning_fail", "not_run", "error"}
SOURCE_EXCLUDED_PREFIXES = ("output/", "build/", "target/")
STABLE_ERROR_CODES = {
    "schema": "LAYER_SCHEMA_MISMATCH",
    "status_exit": "LAYER_STATUS_EXIT_MISMATCH",
    "nonzero_pass": "LAYER_NONZERO_WITHOUT_FAILURE_REPORT",
    "invalid": "LAYER_REPORT_INVALID",
    "timeout": "LAYER_TIMEOUT",
    "unavailable": "LAYER_COMMAND_UNAVAILABLE",
    "app_missing": "PACKAGED_APP_MISSING",
    "sidecar_missing": "PACKAGED_SIDECAR_MISSING",
    "source_changed": "SOURCE_CHANGED_DURING_GATE",
    "internal": "RUNNER_INTERNAL_ERROR",
    "workbench_coverage": "WORKBENCH_COVERAGE_INCOMPLETE",
}


def _sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _sha256_file(path: Path) -> str | None:
    if not path.is_file():
        return None
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _path_for_report(path: Path) -> str:
    """Return a stable, non-sensitive identifier for a report path."""

    resolved = path.resolve()
    try:
        return str(resolved.relative_to(ROOT))
    except ValueError:
        basename = "".join(
            character if character.isascii() and (character.isalnum() or character in "._-") else "-"
            for character in resolved.name
        ).strip(".-")[:48] or "artifact"
        path_hash = _sha256_bytes(str(resolved).encode("utf-8"))[:12]
        return f"external-artifact/{basename}-{path_hash}"


def _git_bytes(*args: str) -> bytes:
    completed = subprocess.run(
        ["git", *args],
        cwd=ROOT,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if completed.returncode != 0:
        raise RuntimeError("git source fingerprint unavailable")
    return completed.stdout


def _source_path(path: str) -> bool:
    normalized = path.replace("\\", "/")
    return normalized and not normalized.startswith(SOURCE_EXCLUDED_PREFIXES)


def _source_fingerprint() -> dict[str, Any]:
    """Hash tracked state plus every non-generated untracked source file.

    Status, staged diff, unstaged diff and untracked file bytes are separate
    inputs so a gate cannot miss a staged edit or an untracked file being
    added, changed or removed while the gate is running.
    """

    status_raw = _git_bytes("status", "--porcelain=v1", "-z", "--untracked-files=all")
    status_records = []
    for record in status_raw.decode("utf-8", errors="surrogateescape").split("\0"):
        if not record:
            continue
        path = record[3:] if len(record) >= 3 else ""
        if _source_path(path):
            status_records.append(record)
    untracked_raw = _git_bytes("ls-files", "--others", "--exclude-standard", "-z")
    untracked = []
    for path in untracked_raw.decode("utf-8", errors="surrogateescape").split("\0"):
        if not path or not _source_path(path):
            continue
        candidate = ROOT / path
        if candidate.is_file():
            untracked.append({"path": path, "sha256": _sha256_file(candidate), "size": candidate.stat().st_size})
        else:
            untracked.append({"path": path, "missing": True})
    payload = {
        "head": _git_bytes("rev-parse", "HEAD").decode("ascii", errors="replace").strip(),
        "status": sorted(status_records),
        "staged_diff_sha256": _sha256_bytes(_git_bytes("diff", "--binary", "--cached", "--", ".", ":!output", ":!build", ":!target")),
        "unstaged_diff_sha256": _sha256_bytes(_git_bytes("diff", "--binary", "--", ".", ":!output", ":!build", ":!target")),
        "untracked": sorted(untracked, key=lambda item: item["path"]),
    }
    encoded = json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return {"sha256": _sha256_bytes(encoded), **payload}


def _status_from_report(report: Mapping[str, Any]) -> tuple[bool, bool]:
    """Return (is_pass, is_failure) using the layer's public status fields."""

    values = [report.get("status"), report.get("result"), report.get("run_status")]
    is_pass = report.get("ok") is True or any(value in PASS_STATUSES for value in values)
    is_failure = report.get("ok") is False or any(value in FAIL_STATUSES for value in values)
    return is_pass, is_failure


def _validate_layer_report(layer: str, report: Mapping[str, Any], child_returncode: int) -> str | None:
    if report.get("schema_version") != LAYER_REPORT_SCHEMAS[layer]:
        return STABLE_ERROR_CODES["schema"]
    for field, expected in LAYER_CONTRACT_FIELDS[layer].items():
        if report.get(field) != expected:
            return STABLE_ERROR_CODES["invalid"]
    is_pass, is_failure = _status_from_report(report)
    if is_pass and is_failure:
        return STABLE_ERROR_CODES["status_exit"]
    if child_returncode != 0 and is_pass:
        return STABLE_ERROR_CODES["nonzero_pass"]
    reported_exit = report.get("exit_code")
    if reported_exit is not None and reported_exit != child_returncode:
        return STABLE_ERROR_CODES["status_exit"]
    if child_returncode == 0 and (not is_pass or is_failure):
        return STABLE_ERROR_CODES["status_exit"]
    if child_returncode != 0 and not is_failure:
        return STABLE_ERROR_CODES["invalid"]
    return None


def _bounded(value: str, limit: int = 320) -> str:
    value = value.replace("\x00", " ").strip()
    return value if len(value) <= limit else value[: limit - 1].rstrip() + "…"


def _parse_last_json(output: str) -> dict[str, Any] | None:
    decoder = json.JSONDecoder()
    for index in (i for i, char in enumerate(output) if char == "{"):
        try:
            value, end = decoder.raw_decode(output[index:])
        except json.JSONDecodeError:
            continue
        if end == len(output[index:].rstrip()) and isinstance(value, dict):
            return value
    return None


def _run_layer(
    layer: str,
    command: Sequence[str],
    *,
    env: dict[str, str],
    timeout_seconds: float,
    report_path: Path,
) -> dict[str, Any]:
    started = time.monotonic()
    synthetic = False
    child_returncode: int | None = None
    try:
        completed = subprocess.run(
            list(command),
            cwd=ROOT,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=timeout_seconds,
            check=False,
        )
        child_returncode = completed.returncode
    except subprocess.TimeoutExpired:
        synthetic = True
        report = {
            "schema_version": LAYER_REPORT_SCHEMAS[layer],
            "layer": layer,
            **LAYER_CONTRACT_FIELDS[layer],
            "status": "failed",
            "stable_error_code": "LAYER_TIMEOUT",
            "exit_code": EXIT_CODES[layer],
        }
    except OSError:
        synthetic = True
        report = {
            "schema_version": LAYER_REPORT_SCHEMAS[layer],
            "layer": layer,
            **LAYER_CONTRACT_FIELDS[layer],
            "status": "failed",
            "stable_error_code": "LAYER_COMMAND_UNAVAILABLE",
            "exit_code": EXIT_CODES[layer],
        }
    else:
        report = _parse_last_json(completed.stdout + "\n" + completed.stderr)
        if report is None:
            report = {
                "schema_version": LAYER_REPORT_SCHEMAS[layer],
                "layer": layer,
                **LAYER_CONTRACT_FIELDS[layer],
                "status": "failed",
                "stable_error_code": "LAYER_REPORT_INVALID",
                "exit_code": completed.returncode,
            }
        else:
            validation_error = _validate_layer_report(layer, report, completed.returncode)
            if validation_error:
                report = dict(report)
                report["status"] = "failed"
                report["ok"] = False
                report["stable_error_code"] = validation_error
                report["exit_code"] = completed.returncode
        report.setdefault("layer", layer)
        report.setdefault("exit_code", completed.returncode)
        report.setdefault("stable_error_code", None if completed.returncode == 0 else "LAYER_FAILED")
    report["child_returncode"] = child_returncode
    if synthetic:
        report["synthetic_failure"] = True
    report["duration_ms"] = int((time.monotonic() - started) * 1000)
    report["report_file"] = _path_for_report(report_path)
    report_path.write_text(json.dumps(report, ensure_ascii=False, sort_keys=True, indent=2) + "\n")
    return report


def _packaged_artifact_paths() -> dict[str, Path]:
    return {
        "sidecar": ROOT / "apps/desktop/src-tauri/binaries/wushen-agent-aarch64-apple-darwin",
        "app_bundle": ROOT / "apps/desktop/src-tauri/target/release/bundle/macos/CAD 工作台.app",
        "app_executable": ROOT / "apps/desktop/src-tauri/target/release/bundle/macos/CAD 工作台.app/Contents/MacOS/wushen-forge-desktop",
    }


def _artifact_failure_code(path_key: str) -> str:
    return STABLE_ERROR_CODES["sidecar_missing" if path_key == "sidecar" else "app_missing"]


def _require_packaged_artifacts() -> dict[str, dict[str, str]]:
    bindings: dict[str, dict[str, str]] = {}
    paths = _packaged_artifact_paths()
    for key in ("sidecar", "app_executable"):
        path = paths[key]
        if not path.is_file():
            raise GateFailure(_artifact_failure_code(key))
        digest = _sha256_file(path)
        if not digest:
            raise GateFailure(_artifact_failure_code(key))
        bindings[key] = {"path": _path_for_report(path), "sha256": digest}
    return bindings


def _bind_packaged_artifacts(report: dict[str, Any]) -> dict[str, Any]:
    bindings = _require_packaged_artifacts()
    bound = dict(report)
    bound["artifacts"] = bindings
    return bound


class GateFailure(RuntimeError):
    def __init__(self, stable_error_code: str) -> None:
        super().__init__(stable_error_code)
        self.stable_error_code = stable_error_code


def _artifact_manifest(artifact_dir: Path, reports: dict[str, dict[str, Any]], *, source: dict[str, Any] | None = None) -> dict[str, Any]:
    packaged = reports.get("packaged", {})
    artifacts = packaged.get("artifacts")
    if not isinstance(artifacts, dict) or not all(
        isinstance(artifacts.get(key), dict) and isinstance(artifacts[key].get("sha256"), str) and artifacts[key]["sha256"]
        for key in ("sidecar", "app_executable")
    ):
        # Keep the manifest fail-closed even when the host layer fails before
        # the packaged report exists; the aggregate will carry the stable
        # artifact error once the packaged layer is reached.
        artifacts = {key: {"path": _path_for_report(path), "sha256": _sha256_file(path)} for key, path in _packaged_artifact_paths().items() if key in {"sidecar", "app_executable"}}
    report_schemas: dict[str, str | None] = {}
    contract_versions: list[str] = []
    for layer, report in reports.items():
        report_schemas[layer] = report.get("schema_version")
        for contract in report.get("contracts", []):
            if isinstance(contract, dict) and isinstance(contract.get("version"), str):
                contract_versions.append(contract["version"])
    return {
        "schema_version": MANIFEST_SCHEMA,
        "source": source or _source_fingerprint(),
        "artifacts": artifacts,
        "reports": report_schemas,
        "expected_report_schemas": LAYER_REPORT_SCHEMAS,
        "contract_versions": sorted(set(contract_versions)),
        "expected_contract_versions": [
            "MaterialCatalogRustPythonContract@1",
            "ShapeProgramPersistenceRustPythonContract@1",
            "RestrictedGeometryRustPythonContract@1",
            "ForgeCADGlbSurfaceProvenanceRustPythonContract@1",
        ],
        "report_directory": _path_for_report(artifact_dir),
    }


def _is_layer_passed(report: Mapping[str, Any]) -> bool:
    is_pass, is_failure = _status_from_report(report)
    return is_pass and not is_failure


def _run_auxiliary_json(
    command: Sequence[str],
    *,
    env: dict[str, str],
    timeout_seconds: float,
    expected_schema: str,
) -> dict[str, Any]:
    try:
        completed = subprocess.run(
            list(command), cwd=ROOT, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            text=True, timeout=timeout_seconds, check=False,
        )
    except subprocess.TimeoutExpired:
        return {"schema_version": expected_schema, "ok": False, "exit_code": EXIT_CODES["workbench"], "stable_error_code": STABLE_ERROR_CODES["timeout"]}
    except OSError:
        return {"schema_version": expected_schema, "ok": False, "exit_code": EXIT_CODES["workbench"], "stable_error_code": STABLE_ERROR_CODES["unavailable"]}
    report = _parse_last_json(completed.stdout + "\n" + completed.stderr)
    if report is None or report.get("schema_version") != expected_schema:
        return {"schema_version": expected_schema, "ok": False, "exit_code": completed.returncode, "stable_error_code": STABLE_ERROR_CODES["invalid"]}
    report = dict(report)
    if report.get("exit_code") is not None and report["exit_code"] != completed.returncode:
        report.update({"ok": False, "stable_error_code": STABLE_ERROR_CODES["status_exit"], "exit_code": completed.returncode})
    elif completed.returncode == 0 and report.get("ok") is not True:
        report.update({"ok": False, "stable_error_code": STABLE_ERROR_CODES["status_exit"], "exit_code": completed.returncode})
    elif completed.returncode != 0 and report.get("ok") is True:
        report.update({"ok": False, "stable_error_code": STABLE_ERROR_CODES["nonzero_pass"], "exit_code": completed.returncode})
    return report


def validate_workbench_coverage(t002: Mapping[str, Any], m108: Mapping[str, Any]) -> dict[str, Any]:
    facets = t002.get("facets")
    t002_ok = (
        t002.get("schema_version") == LAYER_REPORT_SCHEMAS["workbench"]
        and t002.get("ok") is True
        and t002.get("scenario_count") == t002.get("expected_scenario_count") == 14
        and isinstance(facets, dict)
        and all(isinstance(facets.get(name), dict) and facets[name].get("status") == "passed" for name in ("browser", "renderer", "quality", "export"))
    )
    m108_ok = (
        m108.get("schema_version") == "M108WorkbenchRendererSelfTest@1"
        and m108.get("phase") == "self_test"
        and m108.get("subsystem") == "workbench_renderer_route_lifecycle"
        and m108.get("ok") is True
        and m108.get("repeat_count") == 3
    )
    return {
        "schema_version": "ForgeCADK003WorkbenchCoverage@1",
        "t002": {"schema_version": t002.get("schema_version"), "scenario_count": t002.get("scenario_count"), "ok": t002_ok},
        "m108_renderer": {"schema_version": m108.get("schema_version"), "repeat_count": m108.get("repeat_count"), "ok": m108_ok},
        "ok": t002_ok and m108_ok,
        "stable_error_code": None if t002_ok and m108_ok else STABLE_ERROR_CODES["workbench_coverage"],
    }


def _run_workbench_layer(*, env: dict[str, str], timeout_seconds: float, artifact_dir: Path) -> dict[str, Any]:
    t002 = _run_layer(
        "workbench", ["node", "scripts/smoke_workbench_e2e_scenarios.mjs"], env=env,
        timeout_seconds=timeout_seconds, report_path=artifact_dir / "workbench-t002.json",
    )
    m108 = _run_auxiliary_json(
        ["node", "scripts/smoke_m108_workbench_renderer.self-test.mjs"], env=env,
        timeout_seconds=timeout_seconds, expected_schema="M108WorkbenchRendererSelfTest@1",
    )
    coverage = validate_workbench_coverage(t002, m108)
    combined = dict(t002)
    combined["m108_renderer"] = m108
    combined["coverage"] = coverage
    combined["status"] = "passed" if coverage["ok"] else "failed"
    combined["ok"] = coverage["ok"]
    combined["exit_code"] = 0 if coverage["ok"] else EXIT_CODES["workbench"]
    combined["stable_error_code"] = None if coverage["ok"] else coverage["stable_error_code"] or m108.get("stable_error_code")
    combined["report_file"] = _path_for_report(artifact_dir / "workbench.json")
    (artifact_dir / "workbench-m108.json").write_text(json.dumps(m108, ensure_ascii=False, sort_keys=True, indent=2) + "\n")
    (artifact_dir / "workbench.json").write_text(json.dumps(combined, ensure_ascii=False, sort_keys=True, indent=2) + "\n")
    return combined


def _write_aggregate(
    artifact_dir: Path,
    reports: dict[str, dict[str, Any]],
    *,
    status: str,
    stable_error_code: str | None,
    exit_code: int,
    source_before: dict[str, Any],
    source_after: dict[str, Any],
) -> int:
    source_changed = source_before.get("sha256") != source_after.get("sha256")
    source = {"before": source_before, "after": source_after, "changed": source_changed}
    if source_changed:
        status, stable_error_code, exit_code = "failed", STABLE_ERROR_CODES["source_changed"], EXIT_CODES["internal"]
    manifest = _artifact_manifest(artifact_dir, reports, source=source)
    manifest_path = artifact_dir / "manifest.json"
    manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, sort_keys=True, indent=2) + "\n")
    aggregate = {
        "schema_version": REPORT_SCHEMA,
        "gate_id": "FGC-GATE-K003-LAYERS",
        "phase": "k003_layered_gate",
        "subsystem": "layer_aggregator",
        "status": status,
        "stable_error_code": stable_error_code,
        "exit_code": exit_code,
        "layer_order": list(LAYER_ORDER),
        "completed_layers": list(reports),
        "reports": {name: {"schema_version": item.get("schema_version"), "status": item.get("status", item.get("result")), "stable_error_code": item.get("stable_error_code")} for name, item in reports.items()},
        "source_changed": source_changed,
        "manifest": _path_for_report(manifest_path),
    }
    (artifact_dir / "report.json").write_text(json.dumps(aggregate, ensure_ascii=False, sort_keys=True, indent=2) + "\n")
    print(json.dumps(aggregate, ensure_ascii=False, sort_keys=True, separators=(",", ":")))
    return exit_code


def run_gate(artifact_dir: Path, timeout_seconds: float) -> int:
    if not artifact_dir.is_absolute():
        artifact_dir = ROOT / artifact_dir
    artifact_dir.mkdir(parents=True, exist_ok=True)
    host_library_dir = artifact_dir / "host-library"
    host_library_dir.mkdir(parents=True, exist_ok=True)
    source_before = _source_fingerprint()
    base_env = dict(os.environ)
    base_env["PYTHONPATH"] = f"scripts{os.pathsep}{base_env.get('PYTHONPATH', '')}".rstrip(os.pathsep)
    paths = _packaged_artifact_paths()
    commands: dict[str, list[str]] = {
        "host": [sys.executable, "scripts/forgecad_gate_host_preflight.py", "--workspace", str(ROOT), "--library", str(host_library_dir), "--dynamic-port", "--sidecar", str(paths["sidecar"]), "--app-artifact", str(paths["app_bundle"])],
        "rust_core": [sys.executable, "scripts/forgecad_gate_rust_core.py"],
        "rust_python_contract": [sys.executable, "scripts/forgecad_gate_rust_python_contract.py"],
        "packaged": ["npm", "run", "desktop:k003-packaged-native-smoke"],
    }
    reports: dict[str, dict[str, Any]] = {}
    for layer in ("host", "rust_core", "rust_python_contract", "packaged"):
        layer_env = dict(base_env)
        if layer == "packaged":
            try:
                _require_packaged_artifacts()
            except GateFailure as failure:
                reports[layer] = {"schema_version": LAYER_REPORT_SCHEMAS[layer], "status": "failed", "ok": False, "exit_code": EXIT_CODES[layer], "stable_error_code": failure.stable_error_code}
                break
            packaged_dir = artifact_dir / "packaged"
            packaged_dir.mkdir(parents=True, exist_ok=True)
            layer_env["FORGECAD_K003_PACKAGED_SMOKE_ARTIFACT_DIR"] = str(packaged_dir)
        report = _run_layer(layer, commands[layer], env=layer_env, timeout_seconds=timeout_seconds, report_path=artifact_dir / f"{layer}.json")
        if layer == "packaged" and _is_layer_passed(report):
            try:
                report = _bind_packaged_artifacts(report)
                (artifact_dir / "packaged.json").write_text(json.dumps(report, ensure_ascii=False, sort_keys=True, indent=2) + "\n")
            except GateFailure as failure:
                report = {**report, "status": "failed", "ok": False, "exit_code": EXIT_CODES[layer], "stable_error_code": failure.stable_error_code}
        reports[layer] = report
        if not _is_layer_passed(report):
            break
    if len(reports) == 4 and all(_is_layer_passed(item) for item in reports.values()):
        reports["workbench"] = _run_workbench_layer(env=base_env, timeout_seconds=timeout_seconds, artifact_dir=artifact_dir)
    source_after = _source_fingerprint()
    failed_layer = next((name for name, item in reports.items() if not _is_layer_passed(item)), None)
    return _write_aggregate(
        artifact_dir, reports,
        status="passed" if failed_layer is None else "failed",
        stable_error_code=None if failed_layer is None else reports[failed_layer].get("stable_error_code") or f"{failed_layer.upper()}_FAILED",
        exit_code=0 if failed_layer is None else EXIT_CODES[failed_layer],
        source_before=source_before, source_after=source_after,
    )


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifact-dir", type=Path, default=DEFAULT_ARTIFACT_DIR)
    parser.add_argument("--timeout-seconds", type=float, default=1800)
    args = parser.parse_args(argv)
    artifact_dir = args.artifact_dir if args.artifact_dir.is_absolute() else ROOT / args.artifact_dir
    try:
        return run_gate(args.artifact_dir, args.timeout_seconds)
    except Exception:
        try:
            artifact_dir.mkdir(parents=True, exist_ok=True)
            report = {
                "schema_version": REPORT_SCHEMA,
                "gate_id": "FGC-GATE-K003-LAYERS",
                "phase": "k003_layered_gate",
                "subsystem": "layer_aggregator",
                "status": "failed",
                "stable_error_code": STABLE_ERROR_CODES["internal"],
                "exit_code": EXIT_CODES["internal"],
                "layer_order": list(LAYER_ORDER),
                "completed_layers": [],
                "reports": {},
                "manifest": _path_for_report(artifact_dir / "manifest.json"),
            }
            manifest = {
                "schema_version": MANIFEST_SCHEMA,
                "source": {"status": "unavailable"},
                "artifacts": {},
                "reports": {},
                "expected_report_schemas": LAYER_REPORT_SCHEMAS,
                "report_directory": _path_for_report(artifact_dir),
            }
            (artifact_dir / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, sort_keys=True, indent=2) + "\n")
            (artifact_dir / "report.json").write_text(json.dumps(report, ensure_ascii=False, sort_keys=True, indent=2) + "\n")
            print(json.dumps(report, ensure_ascii=False, sort_keys=True, separators=(",", ":")))
        except Exception:
            # The process exit is still stable even if the filesystem itself is unavailable.
            pass
        return EXIT_CODES["internal"]


if __name__ == "__main__":
    raise SystemExit(main())
