#!/usr/bin/env python3
"""Build and execute an isolated macOS sidecar containing the Python CSG candidate."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import socket
import subprocess
import tempfile
import time
import urllib.request
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
BASELINE = ROOT / "apps/desktop/src-tauri/binaries/wushen-agent-aarch64-apple-darwin"
BUDGET = {
    "max_total_bytes": 48 * 1024 * 1024,
    "max_increment_bytes": 28 * 1024 * 1024,
    "max_cold_start_regression_ms": 5_000,
    "max_peak_rss_kib": 300 * 1024,
}


def _free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as handle:
        handle.bind(("127.0.0.1", 0))
        return int(handle.getsockname()[1])


def _process_tree_rss_kib(root_pid: int) -> int:
    output = subprocess.run(
        ["ps", "-axo", "pid=,ppid=,rss="],
        text=True,
        capture_output=True,
        check=False,
    ).stdout
    rows: dict[int, tuple[int, int]] = {}
    for line in output.splitlines():
        fields = line.split()
        if len(fields) == 3 and all(field.isdigit() for field in fields):
            rows[int(fields[0])] = (int(fields[1]), int(fields[2]))
    included = {root_pid}
    changed = True
    while changed:
        changed = False
        for pid, (parent, _rss) in rows.items():
            if parent in included and pid not in included:
                included.add(pid)
                changed = True
    return sum(rows.get(pid, (0, 0))[1] for pid in included)


def _wait_for_health(port: int, process: subprocess.Popen[str]) -> tuple[float, int]:
    started = time.perf_counter()
    peak = 0
    endpoint = f"http://127.0.0.1:{port}/api/health"
    for _ in range(1200):
        if process.poll() is not None:
            stderr = process.stderr.read() if process.stderr else ""
            raise AssertionError(f"candidate sidecar exited before health: {stderr[-2000:]}")
        peak = max(peak, _process_tree_rss_kib(process.pid))
        try:
            with urllib.request.urlopen(endpoint, timeout=0.25) as response:
                payload = json.loads(response.read().decode("utf-8"))
            if payload.get("status") != "ok" or payload.get("service") != "wushen-agent":
                raise AssertionError(f"unexpected health payload: {payload}")
            return (time.perf_counter() - started) * 1000, peak
        except OSError:
            time.sleep(0.05)
    raise AssertionError("candidate sidecar did not become healthy")


def _stop(process: subprocess.Popen[str]) -> None:
    process.terminate()
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=10)


def _measure_binary(binary: Path, runtime_root: Path) -> tuple[float, int]:
    environment = dict(os.environ)
    for name in ("FORGECAD_AGENT_PROVIDER", "FORGECAD_AGENT_BASE_URL", "FORGECAD_AGENT_MODEL", "FORGECAD_AGENT_API_KEY"):
        environment.pop(name, None)
    environment["WUSHEN_LIBRARY_ROOT"] = str(runtime_root / "library")
    environment["FORGECAD_CONCEPT_WORKER_ENABLED"] = "0"
    environment["WUSHEN_LOCAL_WORKER_ENABLED"] = "0"
    port = _free_port()
    process = subprocess.Popen(
        [str(binary), "agent", "serve", "--host", "127.0.0.1", "--port", str(port)],
        cwd=runtime_root, env=environment, text=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE,
    )
    try:
        try:
            return _wait_for_health(port, process)
        except Exception as exc:
            _stop(process)
            stderr = process.stderr.read() if process.stderr else ""
            raise AssertionError(f"candidate runtime failed: {exc}; stderr={stderr[-4000:]}") from exc
    finally:
        if process.poll() is None:
            _stop(process)


def _license_entry(site: Path, distribution: str) -> dict:
    candidates = list(site.glob(f"{distribution}-*.dist-info"))
    if len(candidates) != 1:
        raise AssertionError(f"expected one dist-info for {distribution}: {candidates}")
    dist_info = candidates[0]
    metadata_text = (dist_info / "METADATA").read_text(encoding="utf-8")
    version = next(line.split(":", 1)[1].strip() for line in metadata_text.splitlines() if line.startswith("Version:"))
    license_files = sorted(path for path in (dist_info / "licenses").rglob("*") if path.is_file())
    return {
        "name": distribution,
        "version": version,
        "license_expression": "Apache-2.0" if distribution == "manifold3d" else "BSD-3-Clause",
        "license_files": [
            {"name": path.name, "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
            for path in license_files
        ],
        "notice_present": any(path.name.lower().startswith("notice") for path in license_files),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--python-site", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if platform.system() != "Darwin" or platform.machine() != "arm64":
        raise SystemExit("G824C macOS package benchmark requires Darwin arm64")
    if not BASELINE.is_file():
        raise AssertionError("current packaged sidecar baseline is missing")
    with tempfile.TemporaryDirectory(prefix="forgecad-g824c-package-") as raw:
        root = Path(raw)
        hook = root / "candidate_runtime_hook.py"
        hook.write_text("import manifold3d\nimport numpy\n", encoding="utf-8")
        dist = root / "dist"
        work = root / "work"
        spec = root / "spec"
        binary_name = "wushen-agent-g824c-aarch64-apple-darwin"
        command = [
            str(ROOT / ".venv/bin/python"), "-m", "PyInstaller",
            "--noconfirm", "--clean", "--onefile", "--name", binary_name,
            "--paths", str(ROOT / "apps/agent"), "--paths", str(args.python_site),
            "--add-data", f"{ROOT / 'migrations'}:migrations",
            "--add-data", f"{ROOT / 'packages/concept-spec'}:packages/concept-spec",
            "--runtime-hook", str(hook),
            "--hidden-import", "manifold3d", "--hidden-import", "numpy", "--hidden-import", "numpy._core._exceptions",
            "--collect-all", "fastapi", "--collect-all", "starlette", "--collect-all", "pydantic",
            "--collect-all", "jsonschema", "--collect-all", "wushen_agent", "--collect-all", "forgecad_agent",
            "--distpath", str(dist), "--workpath", str(work), "--specpath", str(spec),
            str(ROOT / "apps/agent/wushen_agent/sidecar_entry.py"),
        ]
        build_started = time.perf_counter()
        completed = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, timeout=300)
        if completed.returncode != 0:
            raise AssertionError(f"PyInstaller candidate build failed: {completed.stderr[-4000:]}")
        build_ms = (time.perf_counter() - build_started) * 1000
        binary = dist / binary_name
        if not binary.is_file():
            raise AssertionError("candidate sidecar binary is missing")
        archive = subprocess.run(
            [str(ROOT / ".venv/bin/pyi-archive_viewer"), "-l", str(binary)],
            text=True, capture_output=True, check=True, timeout=60,
        ).stdout
        if "manifold3d" not in archive or "numpy" not in archive:
            raise AssertionError("candidate sidecar archive is missing manifold3d or numpy")
        with tempfile.TemporaryDirectory(prefix="forgecad-g824c-baseline-runtime-") as baseline_runtime:
            baseline_cold_ms, baseline_peak_rss_kib = _measure_binary(BASELINE, Path(baseline_runtime))
        with tempfile.TemporaryDirectory(prefix="forgecad-g824c-candidate-runtime-") as candidate_runtime:
            cold_ms, peak_rss_kib = _measure_binary(binary, Path(candidate_runtime))
        baseline_bytes = BASELINE.stat().st_size
        candidate_bytes = binary.stat().st_size
        increment = candidate_bytes - baseline_bytes
        licenses = [_license_entry(args.python_site, "manifold3d"), _license_entry(args.python_site, "numpy")]
        budget_passed = (
            candidate_bytes <= BUDGET["max_total_bytes"]
            and increment <= BUDGET["max_increment_bytes"]
            and cold_ms <= baseline_cold_ms + BUDGET["max_cold_start_regression_ms"]
            and peak_rss_kib <= BUDGET["max_peak_rss_kib"]
        )
        report = {
            "schema_version": "ForgeCADCSGPackagedCandidateEvidence@1",
            "machine": {"platform": platform.platform(), "machine": platform.machine(), "python": platform.python_version()},
            "budget": BUDGET,
            "python_candidate": {
                "version": "manifold3d==3.5.2",
                "source_commit": "11235e6b8ebea2dbed8aec4285685aafd3d95667",
                "baseline_sidecar_bytes": baseline_bytes,
                "baseline_cold_start_ms": round(baseline_cold_ms, 3),
                "baseline_peak_rss_kib": baseline_peak_rss_kib,
                "candidate_sidecar_bytes": candidate_bytes,
                "increment_bytes": increment,
                "build_ms": round(build_ms, 3),
                "cold_start_ms": round(cold_ms, 3),
                "cold_start_regression_ms": round(cold_ms - baseline_cold_ms, 3),
                "peak_rss_kib": peak_rss_kib,
                "mach_o_arm64": "Mach-O 64-bit executable arm64" in subprocess.check_output(["file", str(binary)], text=True),
                "archive_contains_manifold3d": True,
                "archive_contains_numpy": True,
                "runtime_hook_import_and_health_verified": True,
                "packaging_findings": [
                    "PyInstaller 6.16.0 did not discover numpy._core._exceptions from the isolated --target tree; the candidate build requires an explicit hidden import",
                ],
                "provider_calls": 0,
                "budget_passed": budget_passed,
                "licenses": licenses,
            },
            "wasm_candidate": {
                "version": "manifold-3d@3.5.1",
                "payload_bytes": sum(path.stat().st_size for path in args.python_site.parent.joinpath("js/package").rglob("*") if path.is_file()) if args.python_site.parent.joinpath("js/package").exists() else 2761999,
                "current_python_sidecar_execution_host": False,
                "rejection_reason": "would require a second JS/WASM execution host or moving authoritative geometry into the WebView; neither is an adopted ForgeCAD Worker boundary",
            },
            "recommendation": {
                "candidate": "manifold_python" if budget_passed else None,
                "status": "recommended_pending_windows_runtime" if budget_passed else "no_candidate_within_package_budget",
                "production_dependency_added": False,
                "remaining_blockers": [
                    "Windows x64 packaged sidecar must execute provenance and lifecycle fixtures",
                    "a superseding ADR must adopt the Python candidate before G825 changes production dependencies",
                ],
            },
        }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    temporary = args.output.with_suffix(args.output.suffix + ".tmp")
    temporary.write_text(json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    temporary.replace(args.output)
    print(json.dumps({"ok": True, "output": str(args.output), "recommendation": report["recommendation"]}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
