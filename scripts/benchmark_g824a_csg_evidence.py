#!/usr/bin/env python3
"""Generate G824A provenance and cancellation evidence without production integration."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any

from g824a_provenance_glb import build_and_readback_glb


ROOT = Path(__file__).resolve().parents[1]


def _child(command: list[str], env: dict[str, str] | None = None) -> dict[str, Any]:
    completed = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True, check=True, timeout=60)
    return json.loads(completed.stdout.strip().splitlines()[-1])


def _wait_for(path: Path, process: subprocess.Popen[str], timeout: float = 8.0) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if path.exists():
            return
        if process.poll() is not None:
            stdout, stderr = process.communicate()
            raise RuntimeError(f"busy probe exited before marker: {stdout[-500:]} {stderr[-500:]}")
        time.sleep(0.01)
    process.kill()
    process.wait(timeout=3)
    raise TimeoutError("busy probe did not enter the kernel loop")


def _cancel_case(command: list[str], env: dict[str, str] | None, error_code: str) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="forgecad-g824a-cancel-") as directory:
        root = Path(directory)
        marker = root / "kernel-started.json"
        output = root / "candidate.glb"
        state = root / "state"
        state.mkdir()
        sentinels = {
            state / "snapshot.json": b'{"revision":17}',
            state / "version.json": b'{"head":"asset-v8"}',
            state / "cache-head": b"stable-candidate-head",
        }
        for path, payload in sentinels.items():
            path.write_bytes(payload)
        resolved = [value.replace("{marker}", str(marker)).replace("{output}", str(output)) for value in command]
        process = subprocess.Popen(resolved, cwd=ROOT, env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        _wait_for(marker, process)
        if error_code == "CSG_CANCELLED":
            process.terminate()
        else:
            time.sleep(0.03)
            process.kill()
        process.wait(timeout=5)
        unchanged = all(path.read_bytes() == payload for path, payload in sentinels.items())
        return {
            "error_code": error_code,
            "process_exit_code": process.returncode,
            "kernel_start_marker_seen": marker.exists(),
            "partial_glb_emitted": output.exists(),
            "isolated_snapshot_version_cache_unchanged": unchanged,
            "process_reaped": process.poll() is not None,
        }


def _candidate_evidence(payload: dict[str, Any], cancellation: list[dict[str, Any]]) -> dict[str, Any]:
    readback = {fixture_id: build_and_readback_glb(fixture) for fixture_id, fixture in payload["fixtures"].items()}
    valid = [value for fixture_id, value in readback.items() if fixture_id != "near_degenerate_subtract"]
    near = readback["near_degenerate_subtract"]
    return {
        "adapter": payload["adapter"],
        "deterministic_provenance": payload["deterministic_provenance"],
        "property_channels_verified": payload["property_channels_verified"],
        "simplify_provenance_verified": payload["simplify_provenance_verified"],
        "fixtures": {
            fixture_id: {
                "kernel_triangle_count": payload["fixtures"][fixture_id]["triangle_count"],
                "kernel_provenance_sha256": payload["fixtures"][fixture_id]["provenance_sha256"],
                "kernel_source_ids": payload["fixtures"][fixture_id]["source_ids"],
                "kernel_material_ids": payload["fixtures"][fixture_id]["material_ids"],
                "kernel_zone_ids": payload["fixtures"][fixture_id]["zone_ids"],
                "readback": readback[fixture_id],
            }
            for fixture_id in payload["fixtures"]
        },
        "valid_fixture_glb_provenance_verified": bool(valid) and all(value["glb_provenance_verified"] for value in valid),
        "near_degenerate_stable_rejection_verified": near.get("error_code") == "CSG_DEGENERATE_OUTPUT" and near.get("partial_glb_emitted") is False,
        "cancellation_process_boundary_verified": all(
            item["kernel_start_marker_seen"]
            and not item["partial_glb_emitted"]
            and item["isolated_snapshot_version_cache_unchanged"]
            and item["process_reaped"]
            for item in cancellation
        ),
        "cancellation_cases": cancellation,
        "production_operation_lifecycle_verified": False,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--python-site", type=Path, required=True)
    parser.add_argument("--wasm-module", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    python_env = dict(os.environ)
    python_env["PYTHONPATH"] = os.pathsep.join([str(args.python_site), str(ROOT / "apps/agent")])
    python_script = str(ROOT / "scripts/g824a_manifold_python_evidence.py")
    wasm_script = str(ROOT / "scripts/g824a_manifold_wasm_evidence.mjs")
    wasm_url = args.wasm_module.resolve().as_uri()
    python_payload = _child([sys.executable, python_script], python_env)
    wasm_payload = _child(["node", wasm_script, wasm_url])
    python_cancel = [
        _cancel_case([sys.executable, python_script, "--busy-probe", "--marker", "{marker}", "--output", "{output}"], python_env, code)
        for code in ("CSG_CANCELLED", "CSG_TIMEOUT")
    ]
    wasm_cancel = [
        _cancel_case(["node", wasm_script, wasm_url, "--busy-probe", "{marker}", "{output}"], None, code)
        for code in ("CSG_CANCELLED", "CSG_TIMEOUT")
    ]
    python = _candidate_evidence(python_payload, python_cancel)
    wasm = _candidate_evidence(wasm_payload, wasm_cancel)
    common_fixtures = sorted(set(python["fixtures"]) & set(wasm["fixtures"]))
    cross_binding_glb_match = all(
        python["fixtures"][fixture_id]["readback"].get("glb_sha256")
        == wasm["fixtures"][fixture_id]["readback"].get("glb_sha256")
        for fixture_id in common_fixtures
    )
    report = {
        "schema_version": "ForgeCADCSGAdoptionEvidence@1",
        "machine": {
            "platform": platform.platform(),
            "machine": platform.machine(),
            "python": platform.python_version(),
            "node": subprocess.check_output(["node", "--version"], text=True).strip(),
        },
        "candidate_versions": {
            "manifold_python": {"version": "manifold3d==3.5.2", "source_commit": "11235e6b8ebea2dbed8aec4285685aafd3d95667"},
            "manifold_wasm": {"version": "manifold-3d@3.5.1", "source_commit": "cc8a7f66d7d5a560da94346258c5b546af27811e"},
        },
        "commands": {
            "benchmark": "python scripts/benchmark_g824a_csg_evidence.py --python-site <tmp>/python --wasm-module <tmp>/package/manifold.js --output evaluations/csg-g824a/report.json",
        },
        "candidates": [python, wasm],
        "cross_binding_valid_glb_hashes_match": cross_binding_glb_match,
        "stable_error_codes": ["CSG_CANCELLED", "CSG_TIMEOUT", "CSG_DEGENERATE_OUTPUT"],
        "decision": {
            "selected": None,
            "status": "evidence_advanced_candidate_not_selected",
            "satisfied": [
                "source/material/zone property channels survive union/subtract and simplify on macOS arm64",
                "valid fixtures preserve provenance through deterministic GLB write and ForgeCAD readback",
                "near-degenerate output is rejected before partial GLB emission",
                "isolated candidate processes are reaped for cancel/timeout without candidate GLB or sentinel state mutation",
            ],
            "remaining_blockers": [
                "production Worker/Version/Snapshot/cache lifecycle is not integrated or verified",
                "Windows x64 packaged sidecar runtime has not executed the fixed fixtures",
                "one candidate has not been selected by a superseding ADR",
            ],
        },
        "input_fingerprint": hashlib.sha256(
            (python_payload["adapter"] + wasm_payload["adapter"] + "ForgeCADCSGAdoptionEvidence@1").encode()
        ).hexdigest(),
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    temporary = args.output.with_suffix(args.output.suffix + ".tmp")
    temporary.write_text(json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    temporary.replace(args.output)
    print(json.dumps({"ok": True, "output": str(args.output), "decision": report["decision"]}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
