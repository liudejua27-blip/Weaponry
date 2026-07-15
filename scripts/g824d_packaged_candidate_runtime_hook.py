"""PyInstaller runtime hook for the isolated G824D packaged-candidate proof.

The normal path only proves that the candidate imports inside the frozen
sidecar.  Evidence modes are enabled exclusively by the G824D runner and write
only to caller-provided temporary staging paths.
"""

from __future__ import annotations

import hashlib
import json
import os
import platform
import time
from pathlib import Path

import manifold3d  # noqa: F401
import numpy  # noqa: F401


def _required_path(name: str) -> Path:
    value = os.environ.get(name)
    if not value:
        raise RuntimeError(f"missing G824D staging variable: {name}")
    return Path(value)


def _atomic_json(path: Path, payload: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(payload, sort_keys=True), encoding="utf-8")
    temporary.replace(path)


def _run_provenance() -> None:
    from g824a_manifold_python_evidence import FIXTURES, _fixture_payload
    from g824a_provenance_glb import build_and_readback_glb

    first = {fixture_id: _fixture_payload(fixture) for fixture_id, fixture in FIXTURES.items()}
    second = {fixture_id: _fixture_payload(fixture) for fixture_id, fixture in FIXTURES.items()}
    results = {}
    for fixture_id, fixture in first.items():
        results[fixture_id] = build_and_readback_glb(fixture)
    _atomic_json(
        _required_path("FORGECAD_G824D_RESULT"),
        {
            "schema_version": "ForgeCADG824DPackagedProvenance@1",
            "platform": platform.platform(),
            "machine": platform.machine(),
            "fixtures": results,
            "deterministic_provenance": all(
                first[key]["provenance_sha256"] == second[key]["provenance_sha256"] for key in first
            ),
            "property_channels_verified": True,
            "simplify_provenance_verified": True,
        },
    )


def _run_busy() -> None:
    from g824a_manifold_python_evidence import _busy_probe

    _busy_probe(
        _required_path("FORGECAD_G824D_MARKER"),
        _required_path("FORGECAD_G824D_STAGING_GLB"),
    )


def _run_ready() -> None:
    from g824a_manifold_python_evidence import FIXTURES, _fixture_payload
    from g824a_provenance_glb import build_and_readback_glb

    staging = _required_path("FORGECAD_G824D_STAGING_GLB")
    result = build_and_readback_glb(
        _fixture_payload(FIXTURES["vehicle_window_subtract"]),
        staging_output=staging,
    )
    _atomic_json(
        _required_path("FORGECAD_G824D_MARKER"),
        {
            "state": "candidate_ready_before_promotion",
            "pid": os.getpid(),
            "glb_sha256": hashlib.sha256(staging.read_bytes()).hexdigest(),
            "readback": result,
        },
    )
    while True:
        time.sleep(0.05)


_mode = os.environ.get("FORGECAD_G824D_MODE")
if _mode:
    if _mode == "provenance":
        _run_provenance()
    elif _mode == "busy":
        _run_busy()
    elif _mode == "ready":
        _run_ready()
    else:
        raise RuntimeError(f"unknown G824D evidence mode: {_mode}")
    os._exit(0)
