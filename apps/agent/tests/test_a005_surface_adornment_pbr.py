"""Focused A005 deterministic surface-adornment PBR/readback coverage."""

from __future__ import annotations

import copy
import hashlib
import json
import struct
import zlib

import pytest

from forgecad_agent.application.geometry_worker import compile_shape_program
from forgecad_agent.application.restricted_geometry_executor import (
    RestrictedGeometryExecutionRequest,
    RestrictedGeometryExecutor,
)
from forgecad_agent.application.visual_texture_sets import (
    builtin_visual_material_count,
    surface_adornment_texture_cache_facts,
)


def _program() -> dict:
    operations = [
        {
            "operation_id": "op_shell",
            "op": "box",
            "inputs": [],
            "args": {
                "position": [0, 250, 0],
                "size": [600, 360, 400],
                "part_role": "shell",
                "material_id": "mat_primary",
                "zone_id": "zone_shell",
            },
        },
        {
            "operation_id": "op_trim",
            "op": "box",
            "inputs": [],
            "args": {
                "position": [800, 250, 0],
                "size": [360, 240, 280],
                "part_role": "trim",
                "material_id": "mat_aluminum",
                "zone_id": "zone_trim",
            },
        },
    ]
    return {
        "schema_version": "ShapeProgram@1",
        "program_id": "shape_a005_surface_adornment",
        "units": "millimeter",
        "seed": 5005,
        "triangle_budget": 20_000,
        "parameters": [],
        "operations": operations,
        "outputs": [
            {
                "output_id": f"output_{item['operation_id'].removeprefix('op_')}",
                "operation_id": item["operation_id"],
                "kind": "mesh",
                "part_role": item["args"]["part_role"],
            }
            for item in operations
        ],
        "non_functional_only": True,
    }


def _adornment(
    *,
    program_id: str = "adorn_shell",
    target_part_id: str = "part_shell",
    target_zone_id: str = "zone_shell",
    base_material: str = "mat_primary",
    kind: str = "normal_relief",
    motif: str = "parallel_groove",
    intensity: str = "balanced",
    coverage: str = "full_zone",
    seed: int = 5005,
) -> dict:
    return {
        "schema_version": "SurfaceAdornmentProgram@1",
        "program_id": program_id,
        "target_part_id": target_part_id,
        "target_zone_id": target_zone_id,
        "kind": kind,
        "motif": motif,
        "intensity": intensity,
        "coverage": coverage,
        "seed": seed,
        "base_material": base_material,
        "execution": "texture_bake",
        "skill_id": "skill_surface_finish",
        "skill_version": 1,
        "skill_sha256": "a" * 64,
        "generator": "a005_v1",
        "non_functional_only": True,
    }


def _glb_document(payload: bytes) -> tuple[dict, bytes]:
    offset = 12
    document = None
    binary = b""
    while offset + 8 <= len(payload):
        length, chunk_type = struct.unpack_from("<II", payload, offset)
        offset += 8
        chunk = payload[offset:offset + length]
        if chunk_type == 0x4E4F534A:
            document = json.loads(chunk.rstrip(b" \x00").decode("utf-8"))
        elif chunk_type == 0x004E4942:
            binary = chunk
        offset += length
    assert isinstance(document, dict)
    return document, binary


def _png_pixels(payload: bytes) -> tuple[int, int, tuple[tuple[int, int, int], ...]]:
    assert payload[:8] == b"\x89PNG\r\n\x1a\n"
    width, height = struct.unpack(">II", payload[16:24])
    offset = 8
    encoded = bytearray()
    while offset < len(payload):
        length = struct.unpack(">I", payload[offset:offset + 4])[0]
        chunk_type = payload[offset + 4:offset + 8]
        chunk = payload[offset + 8:offset + 8 + length]
        if chunk_type == b"IDAT":
            encoded.extend(chunk)
        offset += 12 + length
    raw = zlib.decompress(bytes(encoded))
    rows = [raw[row * (width * 3 + 1) + 1:(row + 1) * (width * 3 + 1)] for row in range(height)]
    return width, height, tuple(
        tuple(row[offset:offset + 3])
        for row in rows
        for offset in range(0, width * 3, 3)
    )


@pytest.mark.parametrize(
    ("kind", "motif", "coverage"),
    [
        ("normal_relief", "parallel_groove", "full_zone"),
        ("pattern", "chevron_relief", "center_band"),
        ("flowline", "double_flowline", "symmetric_pair"),
        ("micro_surface", "hex_microgrid", "edge_band"),
    ],
)
@pytest.mark.parametrize(
    ("profile", "extent"),
    [("interactive_preview", 128), ("production_concept", 1024)],
)
def test_a005_adornments_are_deterministic_complete_pbr_and_read_back(
    kind: str,
    motif: str,
    coverage: str,
    profile: str,
    extent: int,
) -> None:
    adornment = _adornment(kind=kind, motif=motif, coverage=coverage)
    first = compile_shape_program(
        _program(), artifact_profile_id=profile, surface_adornment_programs=[adornment]
    )
    second = compile_shape_program(
        copy.deepcopy(_program()),
        artifact_profile_id=profile,
        surface_adornment_programs=[copy.deepcopy(adornment)],
    )
    assert first.glb_bytes == second.glb_bytes
    assert first.readback.model_dump(mode="json") == second.readback.model_dump(mode="json")
    assert builtin_visual_material_count() == 8
    assert first.readback.material_count == 9
    texture_set = next(item for item in first.readback.visual_texture_sets if item.surface_adornment)
    assert texture_set.material_index == 8
    assert texture_set.surface_adornment == adornment
    assert texture_set.surface_adornment_sha256 == hashlib.sha256(
        json.dumps(adornment, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode()
    ).hexdigest()
    assert {item.texture_role for item in texture_set.maps} == {
        "base_color", "metallic_roughness", "normal", "occlusion", "emissive"
    }
    assert {(item.width, item.height) for item in texture_set.maps} == {(extent, extent)}
    document, binary = _glb_document(first.glb_bytes)
    dynamic = document["materials"][8]
    assert dynamic["extras"]["forgecad_surface_adornment"] == adornment
    assert dynamic["extras"]["forgecad_surface_adornment_sha256"] == texture_set.surface_adornment_sha256
    primitive = next(
        primitive
        for primitive in document["meshes"][0]["primitives"]
        if primitive["extras"]["forgecad_material_zone_id"] == "zone_shell"
    )
    assert primitive["material"] == 8
    assert primitive["extras"]["forgecad_base_material_id"] == "mat_primary"
    assert primitive["extras"]["forgecad_material_id"] == texture_set.material_id
    texture_by_name = {item["name"]: item for item in document["textures"]}
    image_by_index = document["images"]
    payloads = {}
    for item in texture_set.maps:
        image = image_by_index[texture_by_name[item.texture_id]["source"]]
        view = document["bufferViews"][image["bufferView"]]
        payloads[item.texture_role] = binary[view.get("byteOffset", 0):view.get("byteOffset", 0) + view["byteLength"]]
    _width, _height, normal_pixels = _png_pixels(payloads["normal"])
    _width, _height, roughness_pixels = _png_pixels(payloads["metallic_roughness"])
    assert len({pixel[:2] for pixel in normal_pixels}) > 1
    assert len({pixel[1] for pixel in roughness_pixels}) > 1


def test_a005_rejects_zone_material_and_schema_violations_without_extra_materials() -> None:
    mismatch = _adornment(base_material="mat_aluminum")
    with pytest.raises(ValueError, match="base material"):
        compile_shape_program(_program(), surface_adornment_programs=[mismatch])
    missing_zone = _adornment(target_zone_id="zone_missing")
    with pytest.raises(ValueError, match="target zone"):
        compile_shape_program(_program(), surface_adornment_programs=[missing_zone])
    invalid = _adornment()
    invalid["execution"] = "shell"
    with pytest.raises(ValueError, match="execution"):
        compile_shape_program(_program(), surface_adornment_programs=[invalid])
    plain = compile_shape_program(_program())
    assert plain.readback.material_count == builtin_visual_material_count()


def test_a005_appends_only_used_materials_in_stable_program_order() -> None:
    shell = _adornment(program_id="adorn_z_shell", seed=11)
    trim = _adornment(
        program_id="adorn_a_trim",
        target_part_id="part_trim",
        target_zone_id="zone_trim",
        base_material="mat_aluminum",
        kind="flowline",
        motif="double_flowline",
        seed=12,
    )
    first = compile_shape_program(
        _program(), surface_adornment_programs=[shell, trim]
    )
    second = compile_shape_program(
        _program(), surface_adornment_programs=[trim, shell]
    )
    assert first.glb_bytes == second.glb_bytes
    dynamic = [
        item for item in first.readback.visual_texture_sets if item.surface_adornment
    ]
    assert first.readback.material_count == builtin_visual_material_count() + 2
    assert [item.surface_adornment["program_id"] for item in dynamic] == [
        "adorn_a_trim", "adorn_z_shell",
    ]


def test_a005_restricted_geometry_request_accepts_only_bounded_compile_inputs() -> None:
    request = RestrictedGeometryExecutionRequest(
        execution_id="a005-exec",
        idempotency_key="a005-idempotency",
        cancellation_id="a005-cancel",
        cancellation_token="a005-token",
        action="compile_readback",
        artifact_profile_id="interactive_preview",
        shape_program=_program(),
        surface_adornment_programs=[_adornment()],
    )
    assert request.surface_adornment_programs[0]["program_id"] == "adorn_shell"
    executed = RestrictedGeometryExecutor(environment={}).execute(request)
    dynamic = next(
        item
        for item in executed.readback["visual_texture_sets"]
        if item["surface_adornment"]
    )
    assert dynamic["surface_adornment"]["target_part_id"] == "part_shell"
    assert dynamic["surface_adornment"]["target_zone_id"] == "zone_shell"
    invalid = _adornment()
    invalid["skill_sha256"] = "not-a-hash"
    with pytest.raises(ValueError, match="skill hash"):
        RestrictedGeometryExecutionRequest(
            execution_id="a005-exec-invalid",
            idempotency_key="a005-idempotency-invalid",
            cancellation_id="a005-cancel-invalid",
            cancellation_token="a005-token-invalid",
            action="compile_readback",
            artifact_profile_id="interactive_preview",
            shape_program=_program(),
            surface_adornment_programs=[invalid],
        )
    assert surface_adornment_texture_cache_facts()["entry_count"] <= 32
