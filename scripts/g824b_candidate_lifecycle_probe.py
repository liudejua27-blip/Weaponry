#!/usr/bin/env python3
"""Run a candidate to a staging GLB, then pause before authoritative promotion."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path

from g824a_provenance_glb import build_and_readback_glb


ROOT = Path(__file__).resolve().parents[1]


def _child(command: list[str], env: dict[str, str] | None = None) -> dict:
    completed = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True, check=True, timeout=60)
    return json.loads(completed.stdout.strip().splitlines()[-1])


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--candidate", choices=("python", "wasm"), required=True)
    parser.add_argument("--python-site", type=Path, required=True)
    parser.add_argument("--wasm-module", type=Path, required=True)
    parser.add_argument("--started-marker", type=Path, required=True)
    parser.add_argument("--ready-marker", type=Path, required=True)
    parser.add_argument("--staging-glb", type=Path, required=True)
    args = parser.parse_args()
    args.started_marker.parent.mkdir(parents=True, exist_ok=True)
    args.started_marker.write_text(json.dumps({"pid": os.getpid(), "state": "candidate_started"}), encoding="utf-8")
    if args.candidate == "python":
        env = dict(os.environ)
        env["PYTHONPATH"] = os.pathsep.join([str(args.python_site), str(ROOT / "apps/agent")])
        payload = _child([sys.executable, str(ROOT / "scripts/g824a_manifold_python_evidence.py")], env)
    else:
        payload = _child([
            "node",
            str(ROOT / "scripts/g824a_manifold_wasm_evidence.mjs"),
            args.wasm_module.resolve().as_uri(),
        ])
    result = build_and_readback_glb(payload["fixtures"]["vehicle_window_subtract"], staging_output=args.staging_glb)
    args.ready_marker.write_text(json.dumps({
        "pid": os.getpid(),
        "state": "candidate_ready_before_promotion",
        "glb_sha256": result["glb_sha256"],
    }, sort_keys=True), encoding="utf-8")
    while True:
        time.sleep(0.05)


if __name__ == "__main__":
    raise SystemExit(main())
