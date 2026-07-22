"""M108B-02 static ShapeProgram rotation evidence.

The Rust Recipe engine owns connector frames and writes only existing
``args.rotation`` values.  These tests exercise the other half of that
contract: Python bakes final vertices/normals, not glTF node transforms.
"""

from __future__ import annotations

import copy
import hashlib
import json
import math
import struct
from pathlib import Path
from types import SimpleNamespace

import pytest

from forgecad_agent.application import geometry_worker
from forgecad_agent.application.geometry_worker import compile_shape_program, read_shape_program_glb_facts
from forgecad_agent.application.shape_program import (
    ShapeProgramValidationError,
    validate_shape_program,
)


ROOT = Path(__file__).resolve().parents[3]
REGISTRY = ROOT / "packages/concept-spec/fixtures/editable-component-recipe-registry.json"


def _program(operations: list[dict], outputs: list[dict]) -> dict:
    return {
        "schema_version": "ShapeProgram@1",
        "program_id": "shape_static_rotation_test",
        "units": "millimeter",
        "seed": 17,
        "triangle_budget": 10_000,
        "parameters": [],
        "profile_inputs": [],
        "operations": operations,
        "outputs": outputs,
        "non_functional_only": True,
    }


def _output(operation_id: str, role: str) -> dict:
    return {
        "output_id": f"output_{operation_id.removeprefix('op_')}",
        "operation_id": operation_id,
        "kind": "mesh",
        "part_role": role,
    }


def _compile_rotation(program: dict) -> None:
    first = compile_shape_program(program, artifact_profile_id="production_concept")
    second = compile_shape_program(copy.deepcopy(program), artifact_profile_id="production_concept")
    assert hashlib.sha256(first.glb_bytes).hexdigest() == hashlib.sha256(second.glb_bytes).hexdigest()
    assert first.readback.triangle_count > 0
    assert first.readback.normal_primitive_count == first.readback.primitive_count
    assert first.readback.tangent_primitive_count == first.readback.primitive_count
    facts = read_shape_program_glb_facts(first.glb_bytes)
    assert facts.mesh_count == 1
    assert facts.normal_primitive_count == facts.tangent_primitive_count == facts.primitive_count


def _glb_document(glb: bytes) -> dict:
    _magic, _version, _length = struct.unpack("<4sII", glb[:12])
    json_length, _json_type = struct.unpack("<II", glb[12:20])
    return json.loads(glb[20:20 + json_length].decode("utf-8"))


def test_static_rotation_bakes_box_wedge_cylinder_capsule_bevel_and_surface_panel() -> None:
    rotation = [0.0, 0.0, math.pi / 2]
    operations = [
        {"operation_id": "op_box", "op": "box", "inputs": [], "args": {"position": [0, 0, 0], "rotation": rotation, "size": [100, 200, 300], "part_role": "box_form", "material_id": "mat_aluminum", "zone_id": "zone_box"}},
        {"operation_id": "op_wedge", "op": "wedge", "inputs": [], "args": {"position": [500, 0, 0], "rotation": rotation, "size": [100, 200, 300], "part_role": "wedge_form", "material_id": "mat_aluminum", "zone_id": "zone_wedge"}},
        {"operation_id": "op_cylinder", "op": "cylinder", "inputs": [], "args": {"position": [1000, 0, 0], "rotation": rotation, "radius": 80, "height": 260, "axis": [0, 1, 0], "part_role": "cylinder_form", "material_id": "mat_aluminum", "zone_id": "zone_cylinder"}},
        {"operation_id": "op_capsule", "op": "capsule", "inputs": [], "args": {"position": [1500, 0, 0], "rotation": rotation, "radius": 70, "height": 280, "axis": [0, 1, 0], "part_role": "capsule_form", "material_id": "mat_aluminum", "zone_id": "zone_capsule"}},
        {"operation_id": "op_bevel_source", "op": "box", "inputs": [], "args": {"position": [2000, 0, 0], "rotation": rotation, "size": [200, 120, 180], "part_role": "bevel_form", "material_id": "mat_aluminum", "zone_id": "zone_bevel"}},
        {"operation_id": "op_bevel", "op": "bevel_approx", "inputs": ["op_bevel_source"], "args": {"radius": 20, "segments": 2, "part_role": "bevel_form", "material_id": "mat_aluminum", "zone_id": "zone_bevel"}},
        {"operation_id": "op_panel_source", "op": "box", "inputs": [], "args": {"position": [2500, 0, 0], "rotation": rotation, "size": [220, 120, 180], "part_role": "panel_form", "material_id": "mat_aluminum", "zone_id": "zone_panel"}},
        {"operation_id": "op_panel", "op": "surface_panel", "inputs": ["op_panel_source"], "args": {"position": [20, 0, 0], "size": [80, 10, 80], "axis": [0, 1, 0], "part_role": "panel_trim", "material_id": "mat_signal_red", "zone_id": "zone_panel_trim"}},
    ]
    program = _program(operations, [_output("op_box", "box_form"), _output("op_wedge", "wedge_form"), _output("op_cylinder", "cylinder_form"), _output("op_capsule", "capsule_form"), _output("op_bevel", "bevel_form"), _output("op_panel", "panel_trim")])
    _compile_rotation(program)
    # 90 degrees swaps the unrotated box X/Y world extents.
    rotated = compile_shape_program(_program([operations[0]], [_output("op_box", "box_form")]), artifact_profile_id="production_concept")
    assert rotated.readback.bounds_mm == pytest.approx([200.0, 100.0, 300.0], abs=0.02)


def test_surface_panel_offset_and_bevel_inherit_the_source_static_frame() -> None:
    rotation = (0.0, 0.0, math.pi / 2)
    base = geometry_worker.BoxPrimitive(
        "shell", (0.0, 0.0, 0.0), (220.0, 120.0, 180.0), 0,
        rotation_radians=rotation, rotation_origin_mm=(0.0, 0.0, 0.0),
    )
    panel = geometry_worker.BoxPrimitive(
        "trim", (20.0, 65.0, 0.0), (80.0, 10.0, 80.0), 0,
        "surface_panel", rotation_radians=base.rotation_radians,
        rotation_origin_mm=base.rotation_origin_mm,
    )
    payload = geometry_worker._glb_payloads_for_primitive(panel)[0]
    positions = struct.unpack(f"<{len(payload['positions']) // 4}f", payload["positions"])
    centroid = [sum(positions[axis::3]) / (len(positions) // 3) * 1000 for axis in range(3)]
    assert centroid == pytest.approx([-65.0, 20.0, 0.0], abs=0.02)

    bevel = geometry_worker.BoxPrimitive(
        "bevel", (0.0, 0.0, 0.0), (200.0, 120.0, 180.0), 0,
        "bevel_box", bevel_radius_mm=20.0, bevel_segments=2,
        rotation_radians=rotation, rotation_origin_mm=(0.0, 0.0, 0.0),
    )
    bevel_payload = geometry_worker._glb_payloads_for_primitive(bevel)[0]
    bounds = [
        (bevel_payload["maximum"][axis] - bevel_payload["minimum"][axis]) * 1000
        for axis in range(3)
    ]
    assert bounds == pytest.approx([120.0, 200.0, 180.0], abs=0.02)


def test_static_rotation_bakes_profile_extrude_and_revolve() -> None:
    rotation = [0.35, -0.45, 0.7]
    operations = [
        {"operation_id": "op_profile_extrude", "op": "profile", "inputs": [], "args": {"points": [[-80, -60], [80, -60], [80, 60], [-80, 60]]}},
        {"operation_id": "op_extrude", "op": "extrude", "inputs": ["op_profile_extrude"], "args": {"position": [-300, 0, 0], "rotation": rotation, "height": 180, "part_role": "extrude_form", "material_id": "mat_aluminum", "zone_id": "zone_extrude"}},
        # The triangular profile avoids the known v1 zero-area seam in a
        # rectangular partial revolve while still exercising final rotation.
        {"operation_id": "op_profile_revolve", "op": "profile", "inputs": [], "args": {"points": [[40, -90], [100, 0], [40, 90]]}},
        {"operation_id": "op_revolve", "op": "revolve", "inputs": ["op_profile_revolve"], "args": {"position": [300, 0, 0], "rotation": rotation, "angle": 6.0, "part_role": "revolve_form", "material_id": "mat_aluminum", "zone_id": "zone_revolve"}},
    ]
    _compile_rotation(_program(operations, [_output("op_extrude", "extrude_form"), _output("op_revolve", "revolve_form")]))


def test_static_rotation_bakes_recipe_sweep_and_existing_loft_contract() -> None:
    registry = json.loads(REGISTRY.read_text(encoding="utf-8"))
    sweep_recipe = next(recipe for recipe in registry["recipes"] if recipe["recipe_id"] == "recipe_vehicle_body_shell")
    sweep = copy.deepcopy(sweep_recipe["shape_program_template"])
    sweep["program_id"] = "shape_static_recipe_sweep"
    sweep["operations"][0]["args"]["rotation"] = [0.2, 0.4, -0.6]
    _compile_rotation(sweep)

    box = next(item for item in geometry_worker._boxes_for_domain("pack_vehicle_concept", "balanced", "urban_scout_a") if item.primitive_kind == "loft_contract")
    loft = geometry_worker._program_for_boxes(SimpleNamespace(domain_pack_id="pack_vehicle_concept"), "rotation_loft", [box])
    loft["operations"][0]["args"]["rotation"] = [-0.3, 0.2, 0.5]
    _compile_rotation(loft)


def test_static_rotation_sweep_applies_source_position_once() -> None:
    """A swept local path must follow the same source origin as every other op.

    This guards the M108B Rust bake contract: Rust writes the world-space
    source ``position`` and Python rotates the resulting mesh around that
    origin.  Omitting the position while building the sweep path would leave
    a rotated sweep at the local origin even though its AssemblyGraph reports
    a translated frame.
    """
    registry = json.loads(REGISTRY.read_text(encoding="utf-8"))
    template = next(
        recipe["shape_program_template"]
        for recipe in registry["recipes"]
        if recipe["recipe_id"] == "recipe_vehicle_body_shell"
    )
    local = copy.deepcopy(template)
    local["program_id"] = "shape_static_sweep_local"
    local["operations"][0]["args"]["rotation"] = [0.25, -0.35, 0.45]
    translated = copy.deepcopy(local)
    translated["program_id"] = "shape_static_sweep_translated"
    translation = [730.0, -410.0, 260.0]
    translated["operations"][0]["args"]["position"] = translation

    local_result = compile_shape_program(local, artifact_profile_id="production_concept")
    translated_result = compile_shape_program(translated, artifact_profile_id="production_concept")
    local_glb = _glb_document(local_result.glb_bytes)
    translated_glb = _glb_document(translated_result.glb_bytes)
    local_min = local_glb["accessors"][0]["min"]
    translated_min = translated_glb["accessors"][0]["min"]
    # The exact accessor index is deliberately not trusted for content, but
    # the first POSITION accessor is stable in this bounded one-output fixture.
    assert [
        (translated_min[index] - local_min[index]) * 1000
        for index in range(3)
    ] == pytest.approx(translation, abs=0.02)


def test_static_rotation_rejects_nan_and_unsupported_csg_propagation() -> None:
    invalid = _program([
        {"operation_id": "op_nan", "op": "box", "inputs": [], "args": {"position": [0, 0, 0], "rotation": [math.nan, 0, 0], "size": [10, 10, 10], "part_role": "nan_form", "material_id": "mat_aluminum", "zone_id": "zone_nan"}},
    ], [_output("op_nan", "nan_form")])
    with pytest.raises(ShapeProgramValidationError, match="SHAPE_PROGRAM_NON_FINITE"):
        validate_shape_program(invalid)

    csg = _program([
        {"operation_id": "op_left", "op": "box", "inputs": [], "args": {"position": [0, 0, 0], "rotation": [0, 0, math.pi / 2], "size": [100, 100, 100], "part_role": "left_form", "material_id": "mat_aluminum", "zone_id": "zone_left"}},
        {"operation_id": "op_right", "op": "box", "inputs": [], "args": {"position": [20, 0, 0], "size": [100, 100, 100], "part_role": "right_form", "material_id": "mat_aluminum", "zone_id": "zone_right"}},
        {"operation_id": "op_union", "op": "union", "inputs": ["op_left", "op_right"], "args": {"part_role": "union_form", "material_id": "mat_aluminum", "zone_id": "zone_union"}},
    ], [_output("op_union", "union_form")])
    with pytest.raises(Exception, match="static rotation through mirror, array or CSG"):
        compile_shape_program(csg, artifact_profile_id="production_concept")


def test_readback_rejects_glb_node_transform_tampering() -> None:
    program = _program([
        {"operation_id": "op_box", "op": "box", "inputs": [], "args": {"position": [0, 0, 0], "rotation": [0, 0, math.pi / 2], "size": [100, 200, 300], "part_role": "box_form", "material_id": "mat_aluminum", "zone_id": "zone_box"}},
    ], [_output("op_box", "box_form")])
    glb = compile_shape_program(program, artifact_profile_id="production_concept").glb_bytes
    _magic, _version, _length = struct.unpack("<4sII", glb[:12])
    json_length, json_type = struct.unpack("<II", glb[12:20])
    document = json.loads(glb[20:20 + json_length].decode("utf-8"))
    document["nodes"][0]["rotation"] = [0, 0, 0, 1]
    json_chunk = json.dumps(document, separators=(",", ":")).encode("utf-8")
    json_chunk += b" " * ((4 - len(json_chunk) % 4) % 4)
    binary_offset = 20 + json_length
    binary_length, binary_type = struct.unpack("<II", glb[binary_offset:binary_offset + 8])
    binary = glb[binary_offset + 8:binary_offset + 8 + binary_length]
    tampered = struct.pack("<4sII", b"glTF", 2, 12 + 8 + len(json_chunk) + 8 + len(binary)) + struct.pack("<II", len(json_chunk), json_type) + json_chunk + struct.pack("<II", len(binary), binary_type) + binary
    with pytest.raises(ValueError, match="untransformed and uninstanced"):
        read_shape_program_glb_facts(tampered)
