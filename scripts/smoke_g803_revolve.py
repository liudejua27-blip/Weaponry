#!/usr/bin/env python3
"""Smoke the controlled profile -> revolve ShapeProgram path."""

import json
import math
import struct

from forgecad_agent.application.geometry_worker import (
    build_glb_from_shape_program,
    compile_production_concept_shape_program,
    read_shape_program_glb,
)
from forgecad_agent.application.shape_program import ShapeProgramValidationError, validate_shape_program
from forgecad_agent.application.visual_texture_sets import geometry_artifact_profile_manifest


def revolve_program(points: list[list[float]], suffix: str, angle: float = math.pi * 2) -> dict:
    return {
        "schema_version": "ShapeProgram@1",
        "program_id": f"shape_g803_{suffix}",
        "units": "millimeter",
        "seed": 803,
        "triangle_budget": 100000,
        "parameters": [],
        "operations": [
            {"operation_id": "op_profile", "op": "profile", "inputs": [], "args": {"points": points}},
            {"operation_id": "op_revolve", "op": "revolve", "inputs": ["op_profile"], "args": {"position": [0, 500, 0], "angle": angle, "part_role": f"{suffix}_body"}},
        ],
        "outputs": [{"output_id": "output_body", "operation_id": "op_revolve", "kind": "mesh", "part_role": f"{suffix}_body"}],
        "non_functional_only": True,
    }


def _glb_document_and_binary(payload: bytes) -> tuple[dict, bytes]:
    offset = 12
    document: dict | None = None
    binary = b""
    while offset + 8 <= len(payload):
        length, kind = struct.unpack_from("<II", payload, offset)
        offset += 8
        chunk = payload[offset:offset + length]
        if kind == 0x4E4F534A:
            document = json.loads(chunk.rstrip(b" \x00").decode("utf-8"))
        elif kind == 0x004E4942:
            binary = chunk
        offset += length
    assert isinstance(document, dict) and binary
    return document, binary


def _accessor_values(document: dict, binary: bytes, accessor_index: int) -> list[tuple[float | int, ...]]:
    accessor = document["accessors"][accessor_index]
    view = document["bufferViews"][accessor["bufferView"]]
    component = {5123: ("H", 2), 5126: ("f", 4)}[accessor["componentType"]]
    components = {"SCALAR": 1, "VEC3": 3}[accessor["type"]]
    element_size = component[1] * components
    stride = int(view.get("byteStride", element_size))
    offset = int(view.get("byteOffset", 0)) + int(accessor.get("byteOffset", 0))
    return [
        struct.unpack_from(f"<{components}{component[0]}", binary, offset + index * stride)
        for index in range(int(accessor["count"]))
    ]


def assert_closed_outward_winding(payload: bytes) -> None:
    document, binary = _glb_document_and_binary(payload)
    primitive = document["meshes"][0]["primitives"][0]
    positions = _accessor_values(document, binary, primitive["attributes"]["POSITION"])
    normals = _accessor_values(document, binary, primitive["attributes"]["NORMAL"])
    indices = [int(item[0]) for item in _accessor_values(document, binary, primitive["indices"])]
    signed_volume = 0.0
    for offset in range(0, len(indices), 3):
        i0, i1, i2 = indices[offset:offset + 3]
        first, second, third = (positions[i0], positions[i1], positions[i2])
        edge_a = tuple(second[axis] - first[axis] for axis in range(3))
        edge_b = tuple(third[axis] - first[axis] for axis in range(3))
        face_normal = (
            edge_a[1] * edge_b[2] - edge_a[2] * edge_b[1],
            edge_a[2] * edge_b[0] - edge_a[0] * edge_b[2],
            edge_a[0] * edge_b[1] - edge_a[1] * edge_b[0],
        )
        assert sum(value * value for value in face_normal) > 1e-12
        declared = tuple(sum(normals[index][axis] for index in (i0, i1, i2)) for axis in range(3))
        assert sum(face_normal[axis] * declared[axis] for axis in range(3)) > 0
        signed_volume += (
            first[0] * (second[1] * third[2] - second[2] * third[1])
            + first[1] * (second[2] * third[0] - second[0] * third[2])
            + first[2] * (second[0] * third[1] - second[1] * third[0])
        ) / 6
    assert signed_volume > 1e-9


def main() -> int:
    program = revolve_program([[0, -450], [180, -360], [220, 0], [180, 360], [0, 450]], "joint")
    validate_shape_program(program)
    payload, bounds, triangles = build_glb_from_shape_program(program)
    assert payload[:4] == b"glTF"
    # Each axis endpoint is one fan triangle per strip, rather than a
    # degenerate quad half. The two non-axis strips still emit two each.
    assert triangles == 16 * (1 + 2 + 2 + 1)
    assert all(value > 0 for value in bounds), bounds
    assert read_shape_program_glb(payload) == (triangles, bounds)
    assert build_glb_from_shape_program(program)[0] == payload

    production = compile_production_concept_shape_program(program)
    production_segments = int(geometry_artifact_profile_manifest("production_concept")["radial_segments"])
    assert production.readback.triangle_count == production_segments * (1 + 2 + 2 + 1)
    assert len(production.readback.surface_provenance) == 1
    surface = production.readback.surface_provenance[0]
    assert surface.closed is True
    assert surface.boundary_edge_count == 0
    assert surface.non_manifold_edge_count == 0
    assert surface.degenerate_triangle_count == 0
    assert surface.uv_degenerate_triangle_count == 0
    assert surface.tangent_fallback_triangle_count == 0
    assert_closed_outward_winding(production.glb_bytes)

    partial = revolve_program([[0, -200], [140, -120], [140, 120], [0, 200]], "partial", math.pi)
    validate_shape_program(partial)
    _, _, partial_triangles = build_glb_from_shape_program(partial)
    assert partial_triangles == 15 * (1 + 2 + 1)

    invalid = revolve_program([[-10, 0], [100, 0], [100, 100]], "negative_radius")
    try:
        validate_shape_program(invalid)
    except ShapeProgramValidationError as error:
        assert "REVOLVE_RADIUS" in str(error)
    else:
        raise AssertionError("negative revolve radius must be rejected")
    print("G803 ShapeProgram smoke passed: revolve topology, partial angle and deterministic GLB readback")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
