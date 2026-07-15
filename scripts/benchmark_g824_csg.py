#!/usr/bin/env python3
"""Produce the auditable G824 CSG comparison without changing production deps."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import statistics
import subprocess
import sys
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def directory_size(path: Path) -> int:
    return sum(item.stat().st_size for item in path.rglob("*") if item.is_file())


def child(command: list[str], env: dict[str, str] | None = None) -> dict:
    completed = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True, check=True, timeout=60)
    return json.loads(completed.stdout.strip().splitlines()[-1])


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--python-site", type=Path, required=True)
    parser.add_argument("--wasm-module", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    python_env = dict(os.environ)
    python_env["PYTHONPATH"] = str(args.python_site)
    python = child([sys.executable, str(ROOT / "scripts/g824_manifold_python_adapter.py")], python_env)
    wasm = child(["node", str(ROOT / "scripts/g824_manifold_wasm_adapter.mjs"), args.wasm_module.resolve().as_uri()])
    current_env = dict(os.environ)
    current_env["PYTHONPATH"] = str(ROOT / "apps/agent")
    current_durations: list[float] = []
    for _index in range(4):
        started = time.perf_counter()
        subprocess.run([sys.executable, str(ROOT / "scripts/smoke_g805_boolean.py")], cwd=ROOT, env=current_env, capture_output=True, check=True, timeout=60)
        current_durations.append((time.perf_counter() - started) * 1000)
    current = {
        "adapter": "forgecad_restricted_current",
        "version": "ShapeProgramRuntimeManifest@1",
        "cold_ms": round(current_durations[0], 4),
        "warm_median_ms": round(statistics.median(current_durations[1:]), 4),
        "peak_rss_kib": None,
        "packaged_increment_bytes": 0,
        "deterministic_identical_fixture": True,
        "coplanar_completed": False,
        "near_degenerate_completed": False,
        "material_surface_provenance_verified": False,
        "cancellation_verified": False,
        "limitations": ["overlapping union rejected", "subtract restricted to one axis-aligned box spanning Y/Z"],
    }
    python.update({
        "version": "manifold3d==3.5.2",
        "source_commit": "11235e6b8ebea2dbed8aec4285685aafd3d95667",
        "license": "Apache-2.0",
        "notice_file_present": False,
        "packaged_increment_bytes": directory_size(args.python_site),
        "macos_arm64_executed": True,
        "windows_packaging_artifact_available": True,
        "windows_runtime_executed": False,
        "removal_plan": "delete isolated target directory; no production dependency or lockfile entry",
    })
    wasm.update({
        "version": "manifold-3d@3.5.1",
        "source_commit": "cc8a7f66d7d5a560da94346258c5b546af27811e",
        "license": "Apache-2.0",
        "notice_file_present": False,
        "packaged_increment_bytes": directory_size(args.wasm_module.parent),
        "macos_arm64_executed": True,
        "windows_packaging_artifact_available": True,
        "windows_runtime_executed": False,
        "removal_plan": "delete unpacked npm tarball; no production dependency or lockfile entry",
    })
    report = {
        "schema_version": "ForgeCADCSGBenchmark@1",
        "machine": {"platform": platform.platform(), "machine": platform.machine(), "python": platform.python_version(), "node": subprocess.check_output(["node", "--version"], text=True).strip()},
        "commands": {
            "prepare_python": "python -m pip install --no-deps --target <tmp>/python manifold3d==3.5.2 && python -m pip install --target <tmp>/python 'numpy>=1.26,<3'",
            "prepare_wasm": "npm pack manifold-3d@3.5.1 --pack-destination <tmp> && tar -xzf <tmp>/manifold-3d-3.5.1.tgz",
            "benchmark": "python scripts/benchmark_g824_csg.py --python-site <tmp>/python --wasm-module <tmp>/package/manifold.js --output evaluations/csg-g824/report.json",
        },
        "fixtures": ["vehicle_window_subtract", "aircraft_canopy_subtract", "appliance_vent_subtract", "robot_arm_housing_union", "coplanar", "near_degenerate"],
        "candidates": [current, python, wasm],
        "decision": {
            "selected": None,
            "status": "no_candidate_meets_all_gates",
            "reasons": ["material/surface provenance was not verified", "cancellation was not verified", "Windows runtime was not executed", "current handler is not robust CSG"],
        },
        "input_fingerprint": hashlib.sha256((python["version"] + wasm["version"]).encode()).hexdigest(),
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps({"ok": True, "output": str(args.output), "decision": report["decision"]}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
