#!/usr/bin/env python3
"""Focused M109A LOD gate for the robotic-arm service-display golden path.

The production LOD0 is covered by the C106 production gate.  This companion
gate proves that the exact same Rust-expanded ShapeProgram still produces a
bounded, deterministic interactive LOD1 and that no second product/version
truth is introduced.
"""

from __future__ import annotations

import base64
import hashlib
import json
from typing import Any, Mapping

from smoke_c106_robotic_arm_production_gate import run_rust_dump

from forgecad_agent.application.restricted_geometry_executor import (
    RestrictedGeometryExecutionRequest,
    RestrictedGeometryExecutor,
)
from forgecad_agent.application.visual_texture_sets import (
    geometry_artifact_profile_manifest,
)


ROOT_RECIPE_ID = "recipe_c106_arm_service_display"


def _mapping(value: object, label: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise AssertionError(label)
    return value


def _service_fixture() -> tuple[Mapping[str, Any], Mapping[str, Any]]:
    dump = run_rust_dump()
    candidates = dump.get("candidates")
    seals = dump.get("shape_program_seals")
    if not isinstance(candidates, list) or not isinstance(seals, list):
        raise AssertionError("M109A_RUST_DUMP_INVALID")
    candidate = next(
        (
            _mapping(item, "M109A_CANDIDATE_INVALID")
            for item in candidates
            if isinstance(item, Mapping)
            and isinstance(item.get("recipe"), Mapping)
            and item["recipe"].get("recipe_id") == ROOT_RECIPE_ID
        ),
        None,
    )
    seal = next(
        (
            _mapping(item, "M109A_SEAL_INVALID")
            for item in seals
            if isinstance(item, Mapping) and item.get("recipe_id") == ROOT_RECIPE_ID
        ),
        None,
    )
    if candidate is None or seal is None:
        raise AssertionError("M109A_SERVICE_FIXTURE_MISSING")
    return candidate, seal


def _compile(
    executor: RestrictedGeometryExecutor,
    candidate: Mapping[str, Any],
    seal: Mapping[str, Any],
    suffix: str,
) -> tuple[bytes, Mapping[str, Any]]:
    request = RestrictedGeometryExecutionRequest.model_validate(
        {
            "schema_version": "RestrictedGeometryExecutionRequest@1",
            "protocol_version": "forgecad.restricted-geometry/1",
            "execution_id": f"exec_m109a_preview_{suffix}",
            "idempotency_key": f"idem_m109a_preview_{suffix}",
            "cancellation_id": f"cancel_m109a_preview_{suffix}",
            "cancellation_token": f"token_m109a_preview_{suffix}",
            "action": "compile_readback",
            "timeout_ms": 120_000,
            "artifact_profile_id": "interactive_preview",
            "shape_program": candidate["expanded_shape_program"],
            "shape_program_canonical_json": seal["shape_program_canonical_json"],
            "shape_program_sha256": seal["shape_program_sha256"],
        }
    )
    result = executor.execute(request)
    if result.glb_base64 is None or result.readback is None:
        raise AssertionError("M109A_PREVIEW_COMPILE_INVALID")
    return base64.b64decode(result.glb_base64, validate=True), result.readback


def main() -> int:
    candidate, seal = _service_fixture()
    graph = _mapping(candidate.get("expanded_assembly_graph"), "M109A_GRAPH_INVALID")
    program = _mapping(candidate.get("expanded_shape_program"), "M109A_PROGRAM_INVALID")
    if len(graph.get("parts", [])) != 10 or len(graph.get("connections", [])) != 9:
        raise AssertionError("M109A_ASSEMBLY_IDENTITY_DRIFT")
    if len(program.get("outputs", [])) != 48:
        raise AssertionError("M109A_OUTPUT_IDENTITY_DRIFT")

    preview_profile = geometry_artifact_profile_manifest("interactive_preview")
    production_profile = geometry_artifact_profile_manifest("production_concept")
    if (
        preview_profile["delivery"] != "interactive"
        or preview_profile["texture_width"] != 128
        or production_profile["delivery"] != "on_demand"
        or production_profile["texture_width"] != 1024
        or production_profile["radial_segments"] != 64
        or production_profile["capsule_hemisphere_segments"] != 14
    ):
        raise AssertionError("M109A_PROFILE_CONTRACT_INVALID")

    executor = RestrictedGeometryExecutor(environment={})
    first_glb, first = _compile(executor, candidate, seal, "first")
    second_glb, second = _compile(executor, candidate, seal, "second")
    if first_glb != second_glb or first != second:
        raise AssertionError("M109A_PREVIEW_NON_DETERMINISTIC")
    if hashlib.sha256(first_glb).hexdigest() != first.get("glb_sha256"):
        raise AssertionError("M109A_PREVIEW_HASH_INVALID")
    if first.get("shape_program_sha256") != seal.get("shape_program_sha256"):
        raise AssertionError("M109A_PREVIEW_LINEAGE_DRIFT")
    profile = _mapping(first.get("artifact_profile"), "M109A_PREVIEW_PROFILE_MISSING")
    if profile != preview_profile:
        raise AssertionError("M109A_PREVIEW_PROFILE_DRIFT")
    triangle_count = int(first.get("triangle_count", 0))
    if not 15_000 <= triangle_count <= 35_000:
        raise AssertionError(f"M109A_PREVIEW_TRIANGLE_BUDGET_INVALID:{triangle_count}")
    texture_extents = {
        (int(item["width"]), int(item["height"]))
        for texture_set in first.get("visual_texture_sets", [])
        for item in texture_set.get("maps", [])
    }
    if texture_extents != {(128, 128)}:
        raise AssertionError(f"M109A_PREVIEW_TEXTURE_INVALID:{texture_extents}")
    print(
        json.dumps(
            {
                "schema_version": "M109AArmPreviewLodGate@1",
                "status": "pass",
                "root_recipe_id": ROOT_RECIPE_ID,
                "lod": "LOD1",
                "triangle_count": triangle_count,
                "primitive_count": first["primitive_count"],
                "texture_extent": [128, 128],
                "glb_byte_size": len(first_glb),
                "glb_sha256": first["glb_sha256"],
                "shape_program_sha256": first["shape_program_sha256"],
                "parts": 10,
                "connections": 9,
                "outputs": 48,
                "measured_provider_calls": 0,
                "formal_eligible": False,
            },
            ensure_ascii=False,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
