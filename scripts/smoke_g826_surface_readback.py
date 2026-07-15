#!/usr/bin/env python3
"""FGC-G826: surface completion, tangent and stable zone/face readback gate."""

from __future__ import annotations

import copy
import json
import struct
from typing import Callable

from forgecad_agent.application.geometry_worker import (
    compile_shape_program,
    read_shape_program_glb_facts,
)
from forgecad_agent.application.shape_program import ShapeProgramValidationError
from forgecad_agent.application.shape_program_runtime import UnsupportedRuntimeOperationError
from smoke_g806_bevel_surface_panel import program as bevel_panel_program
from smoke_g821_profile_solid_fidelity import (
    program_for as profile_program,
    revolve_profile,
    shell_profile,
)
from smoke_g822_loft import program as loft_program
from smoke_g823_sweep import program as sweep_program
from smoke_g825_feature_csg import boolean_program


def primitive_program() -> dict:
    operations = [
        {"operation_id": "op_box", "op": "box", "inputs": [], "args": {"position": [-900, 300, 0], "size": [500, 500, 500], "part_role": "box_shell", "zone_id": "zone_box"}},
        {"operation_id": "op_cylinder", "op": "cylinder", "inputs": [], "args": {"position": [-300, 300, 0], "radius": 220, "height": 500, "part_role": "cylinder_shell", "zone_id": "zone_cylinder"}},
        {"operation_id": "op_capsule", "op": "capsule", "inputs": [], "args": {"position": [350, 300, 0], "radius": 180, "height": 620, "part_role": "capsule_shell", "zone_id": "zone_capsule"}},
        {"operation_id": "op_wedge", "op": "wedge", "inputs": [], "args": {"position": [950, 300, 0], "size": [500, 500, 500], "part_role": "wedge_shell", "zone_id": "zone_wedge"}},
    ]
    return {
        "schema_version": "ShapeProgram@1",
        "program_id": "shape_g826_primitives",
        "units": "millimeter",
        "seed": 826,
        "triangle_budget": 100000,
        "parameters": [],
        "operations": operations,
        "outputs": [
            {"output_id": f"output_{item['op']}", "operation_id": item["operation_id"], "kind": "mesh", "part_role": item["args"]["part_role"]}
            for item in operations
        ],
        "non_functional_only": True,
    }


def transform_program() -> dict:
    return {
        "schema_version": "ShapeProgram@1",
        "program_id": "shape_g826_transform",
        "units": "millimeter",
        "seed": 826,
        "triangle_budget": 100000,
        "parameters": [],
        "operations": [
            {"operation_id": "op_source", "op": "box", "inputs": [], "args": {"position": [300, 300, 0], "size": [300, 300, 300], "part_role": "repeated_trim", "zone_id": "zone_repeated_trim"}},
            {"operation_id": "op_mirror", "op": "mirror", "inputs": ["op_source"], "args": {"axis": [1, 0, 0], "part_role": "mirrored_trim"}},
            {"operation_id": "op_array", "op": "array", "inputs": ["op_mirror"], "args": {"axis": [0, 0, 1], "count": 3, "spacing": 450, "part_role": "trim_array"}},
        ],
        "outputs": [{"output_id": "output_array", "operation_id": "op_array", "kind": "mesh", "part_role": "trim_array"}],
        "non_functional_only": True,
    }


def _glb_parts(payload: bytes) -> tuple[dict, bytearray]:
    offset = 12
    document = None
    binary = bytearray()
    while offset + 8 <= len(payload):
        length, kind = struct.unpack_from("<II", payload, offset)
        offset += 8
        chunk = payload[offset:offset + length]
        if kind == 0x4E4F534A:
            document = json.loads(chunk.rstrip(b" \x00").decode("utf-8"))
        elif kind == 0x004E4942:
            binary = bytearray(chunk)
        offset += length
    assert isinstance(document, dict) and binary
    return document, binary


def _glb_payload(document: dict, binary: bytearray) -> bytes:
    encoded = json.dumps(document, separators=(",", ":")).encode("utf-8")
    encoded += b" " * ((4 - len(encoded) % 4) % 4)
    binary.extend(b"\x00" * ((4 - len(binary) % 4) % 4))
    total = 12 + 8 + len(encoded) + 8 + len(binary)
    return (
        struct.pack("<4sII", b"glTF", 2, total)
        + struct.pack("<II", len(encoded), 0x4E4F534A)
        + encoded
        + struct.pack("<II", len(binary), 0x004E4942)
        + bytes(binary)
    )


def rewrite_glb(payload: bytes, mutate: Callable[[dict, bytearray], None]) -> bytes:
    document, binary = _glb_parts(payload)
    mutate(document, binary)
    return _glb_payload(document, binary)


def accessor_offset(document: dict, accessor_index: int) -> int:
    accessor = document["accessors"][accessor_index]
    view = document["bufferViews"][accessor["bufferView"]]
    return int(view.get("byteOffset", 0)) + int(accessor.get("byteOffset", 0))


def expect_readback_rejected(payload: bytes, text: str) -> None:
    try:
        read_shape_program_glb_facts(payload)
    except ValueError as exc:
        assert text in str(exc), exc
        return
    raise AssertionError(f"expected readback failure containing {text}")


def expect_compile_rejected(candidate: dict, text: str) -> None:
    try:
        compile_shape_program(candidate)
    except (ShapeProgramValidationError, UnsupportedRuntimeOperationError, ValueError) as exc:
        assert text in str(exc), exc
        return
    raise AssertionError(f"expected compile failure containing {text}")


def assert_surface_contract(candidate: dict, *, expected_roles: set[str] | None = None) -> bytes:
    compiled = compile_shape_program(candidate)
    repeated = compile_shape_program(candidate)
    assert compiled.glb_bytes == repeated.glb_bytes
    assert compiled.readback.glb_sha256 == repeated.readback.glb_sha256
    readback = compiled.readback
    assert readback.tangent_primitive_count == readback.normal_primitive_count == readback.uv0_primitive_count == readback.primitive_count
    assert len(readback.surface_provenance) == len(readback.material_zone_faces) == readback.primitive_count
    assert [item.primitive_id for item in readback.surface_provenance] == [item.primitive_id for item in readback.material_zone_faces]
    assert len({item.primitive_id for item in readback.surface_provenance}) == readback.primitive_count
    if expected_roles is not None:
        assert expected_roles <= {role for item in readback.surface_provenance for role in item.surface_roles}
    for surface, zone in zip(readback.surface_provenance, readback.material_zone_faces):
        assert surface.texture_ready and zone.texture_ready
        assert surface.material_zone_id == zone.material_zone_id
        assert surface.face_id_sha256 == zone.face_id_sha256
        assert surface.face_id_min == 0 and surface.face_id_max + 1 == zone.face_count
        assert surface.uv_degenerate_triangle_count == surface.tangent_fallback_triangle_count == 0
        assert 0.999 <= surface.tangent_min_length <= surface.tangent_max_length <= 1.001
        assert surface.source_operation_ids and zone.source_operation_ids
    facts = read_shape_program_glb_facts(compiled.glb_bytes)
    assert facts.tangent_primitive_count == facts.primitive_count
    assert facts.material_zone_faces == [item.model_dump() for item in readback.material_zone_faces]
    return compiled.glb_bytes


def assert_corruption_failures(payload: bytes) -> None:
    missing_tangent = rewrite_glb(
        payload,
        lambda document, _binary: document["meshes"][0]["primitives"][0]["attributes"].pop("TANGENT"),
    )
    expect_readback_rejected(missing_tangent, "missing POSITION")

    def zero_tangent(document: dict, binary: bytearray) -> None:
        primitive = document["meshes"][0]["primitives"][0]
        offset = accessor_offset(document, primitive["attributes"]["TANGENT"])
        struct.pack_into("<4f", binary, offset, 0, 0, 0, 1)

    expect_readback_rejected(rewrite_glb(payload, zero_tangent), "non-unit tangent")

    def degenerate_uv(document: dict, binary: bytearray) -> None:
        primitive = document["meshes"][0]["primitives"][0]
        offset = accessor_offset(document, primitive["attributes"]["TEXCOORD_0"])
        struct.pack_into("<6f", binary, offset, 0, 0, 0, 0, 0, 0)

    expect_readback_rejected(rewrite_glb(payload, degenerate_uv), "UV-degenerate")

    empty_zone = rewrite_glb(
        payload,
        lambda document, _binary: document["meshes"][0]["primitives"][0]["extras"].update({"forgecad_material_zone_id": ""}),
    )
    expect_readback_rejected(empty_zone, "material-zone provenance")

    def corrupt_face_id(document: dict, binary: bytearray) -> None:
        primitive = document["meshes"][0]["primitives"][0]
        offset = accessor_offset(document, primitive["attributes"]["_FORGECAD_FACE_ID"])
        struct.pack_into("<I", binary, offset, 99)

    expect_readback_rejected(rewrite_glb(payload, corrupt_face_id), "stable face provenance")


def main() -> int:
    primitive_glb = assert_surface_contract(primitive_program(), expected_roles={"surface"})
    assert_surface_contract(
        profile_program(shell_profile(sketch_id="sketch_g826_extrude"), operation="extrude", suffix="g826_extrude"),
        expected_roles={"side", "start_cap", "end_cap"},
    )
    assert_surface_contract(
        profile_program(revolve_profile(sketch_id="sketch_g826_revolve"), operation="revolve", suffix="g826_revolve", caps=False),
        expected_roles={"side", "seam"},
    )
    assert_surface_contract(loft_program("g826_loft", "x", twist=8, curved=True), expected_roles={"loft_side", "seam", "start_cap", "end_cap"})
    assert_surface_contract(sweep_program("g826_sweep", [[0, 0, 0], [500, 0, 0], [850, 260, 80]], twist=18), expected_roles={"sweep_side", "seam", "start_cap", "end_cap"})

    edge_glb = assert_surface_contract(bevel_panel_program(bevel_segments=3, radius=60, suffix="g826_edge"), expected_roles={"surface", "trim"})
    edge_readback = read_shape_program_glb_facts(edge_glb)
    finished = next(item for item in edge_readback.surface_provenance if item["edge_finish"]["mode"] == "bevel_approximation")
    assert finished["edge_finish"] == {
        "mode": "bevel_approximation",
        "edge_set": "xz_perimeter",
        "selected_edge_count": 4,
        "radius_ratio": round(60 / 700, 8),
        "subdivision_count": 3,
    }

    transform_glb = assert_surface_contract(transform_program(), expected_roles={"surface"})
    transform_facts = read_shape_program_glb_facts(transform_glb)
    assert transform_facts.primitive_count == 3
    assert len({item["part_instance_id"] for item in transform_facts.surface_provenance}) == 3
    assert {item["material_zone_id"] for item in transform_facts.surface_provenance} == {"zone_repeated_trim"}

    csg_glb = assert_surface_contract(boolean_program("subtract", suffix="g826_csg"), expected_roles={"surface", "boolean_cut"})
    csg_facts = read_shape_program_glb_facts(csg_glb)
    assert {item["material_zone_id"] for item in csg_facts.material_zone_faces} >= {"zone_shell", "zone_trim"}
    assert all(item["source_operation_ids"] for item in csg_facts.material_zone_faces)

    assert_corruption_failures(primitive_glb)

    duplicate_zone = rewrite_glb(
        edge_glb,
        lambda document, _binary: document["meshes"][0]["primitives"][1]["extras"].update({
            "forgecad_primitive_id": document["meshes"][0]["primitives"][0]["extras"]["forgecad_primitive_id"]
        }),
    )
    expect_readback_rejected(duplicate_zone, "zones overlap")

    ratio = bevel_panel_program(radius=200, suffix="g826_ratio")
    expect_compile_rejected(ratio, "EDGE_FINISH_RADIUS_RATIO_EXCEEDED")
    subdivisions = bevel_panel_program(suffix="g826_subdivisions")
    subdivisions["operations"][1]["args"]["segments"] = 4
    expect_compile_rejected(subdivisions, "SCHEMA_INVALID")
    budget = copy.deepcopy(transform_program())
    budget["program_id"] = "shape_g826_budget"
    budget["triangle_budget"] = 100
    budget["operations"][-1]["args"]["count"] = 10
    expect_compile_rejected(budget, "triangle count")

    print("G826 surface readback smoke passed: bounded edge finish, normals, UV0, tangents and stable part/zone faces survive GLB readback")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
