#!/usr/bin/env python3
"""Compile the draft C111 fixture for local pixel-level visual iteration.

This helper deliberately skips the frozen Inventory assertions used by the
formal PV002 gate.  It writes only temporary GLBs and a readback summary; it
does not create product state, formal evidence, or Provider traffic.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

from forgecad_agent.application.restricted_geometry_executor import (
    RestrictedGeometryExecutor,
)
from forgecad_agent.application.geometry_worker import compile_shape_program
from smoke_c111a_golden_surface_asset import _compile, _rust_dump


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output-prefix", required=True)
    args = parser.parse_args()

    prefix = Path(args.output_prefix).expanduser().resolve()
    prefix.parent.mkdir(parents=True, exist_ok=True)
    payload = _rust_dump()
    executor = RestrictedGeometryExecutor(environment={})
    profiles: dict[str, object] = {}

    for profile_id in ("interactive_preview", "production_concept"):
        try:
            glb, readback = _compile(executor, payload, profile_id)
        except AssertionError as boundary_error:
            # The process boundary intentionally returns only a stable error
            # code.  Re-run the same draft input in-process so a developer can
            # see the exact geometry rejection without weakening that boundary.
            fixture = payload["forge_visual_program_fixture"]
            try:
                compile_shape_program(
                    fixture["lowering"]["shape_program"],
                    artifact_profile_id=profile_id,
                    surface_adornment_programs=fixture[
                        "surface_adornment_programs"
                    ],
                )
            except Exception as compile_error:
                raise RuntimeError(
                    f"{boundary_error}; direct compile: {compile_error}"
                ) from compile_error
            raise
        path = prefix.with_name(f"{prefix.name}-{profile_id}.glb")
        path.write_bytes(glb)
        profiles[profile_id] = {
            "path": str(path),
            "bytes": len(glb),
            "glb_sha256": hashlib.sha256(glb).hexdigest(),
            "triangle_count": readback.get("triangle_count"),
            "primitive_count": readback.get("primitive_count"),
            "material_count": readback.get("material_count"),
        }

    print(
        json.dumps(
            {
                "schema_version": "C111VisualIterationCompile@1",
                "formal_eligible": False,
                "provider_calls": 0,
                "shape_program_sha256": payload["shape_program_sha256"],
                "forge_visual_program_sha256": payload[
                    "forge_visual_program_fixture"
                ]["lowering"]["source_program_sha256"],
                "operation_count": len(
                    payload["candidate"]["expanded_shape_program"]["operations"]
                ),
                "output_count": len(
                    payload["candidate"]["expanded_shape_program"]["outputs"]
                ),
                "profiles": profiles,
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
