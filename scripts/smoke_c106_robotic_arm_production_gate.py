#!/usr/bin/env python3
"""C106 automated evidence gate for the three robotic-arm production recipes.

This gate consumes the Rust-owned C106 dump and compiles exactly those three
read-only candidates through the capability-gated RestrictedGeometryExecutor
DTO. The returned bytes are then bound to Rust Core state; the Rust helper is
never launched from inside the disposable geometry worker. It is not an M108B gate, does
not contact a Provider, and never emits a visual score or a formal-human-review
claim.
"""

from __future__ import annotations

import argparse
import base64
import copy
from collections import Counter
from dataclasses import asdict
import hashlib
import json
import math
from pathlib import Path
import re
import struct
import subprocess
import sys
import tempfile
from typing import Any, Callable, Mapping, NoReturn, TypeVar

from prepare_m108b_asset_preflight import (
    PreflightError,
    _g826_checks,
    _m108a_checks,
    _q003_checks,
)


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "apps" / "desktop" / "src-tauri" / "Cargo.toml"
RUST_WRAPPER = ROOT / "script" / "with_rust_toolchain.sh"
PACKAGE = "forgecad-core"
DUMP_BINARY = "c106_robotic_arm_recipe_dump"
BINDING_BINARY = "c106_production_evidence_bind"
DUMP_SCHEMA = "C106RoboticArmRecipeExpansion@1"
REPORT_SCHEMA = "C106RoboticArmProductionGate@1"
ARTIFACT_SUMMARY_SCHEMA = "C106RoboticArmProductionArtifacts@1"
DOMAIN = "pack_robotic_arm_concept"
ROOT_IDS = {
    "recipe_c106_arm_desktop_assistant",
    "recipe_c106_arm_gallery_industrial",
    "recipe_c106_arm_service_display",
}
REQUIRED_ROLES = Counter(
    {
        "base_form": 1,
        "turntable": 1,
        "joint_housing": 3,
        "link_armor": 2,
        "cable_harness": 1,
        "end_effector_form": 1,
        "surface_trim": 1,
    }
)
PBR_ROLES = {
    "base_color",
    "metallic_roughness",
    "normal",
    "occlusion",
    "emissive",
}
SURFACE_PROVENANCE_FIELDS = (
    "primitive_id",
    "part_instance_id",
    "part_role",
    "profile_input_id",
    "surface_roles",
    "surface_ranges",
    "uv0_min",
    "uv0_max",
    "closed",
    "boundary_edge_count",
    "non_manifold_edge_count",
    "degenerate_triangle_count",
    "feature_node_id",
    "source_operation_ids",
    "material_zone_id",
    "boolean_backside",
    "normal_mode",
    "tangent_min_length",
    "tangent_max_length",
    "tangent_handedness",
    "uv_degenerate_triangle_count",
    "tangent_fallback_triangle_count",
    "face_id_min",
    "face_id_max",
    "face_id_sha256",
    "edge_finish",
    "texture_ready",
)
VISUAL_TEXTURE_SET_FIELDS = (
    "schema_version",
    "visual_texture_set_id",
    "material_id",
    "texture_material_id",
    "material_index",
    "material_zone_ids",
    "maps",
    "extensions",
    "texture_byte_size",
    "surface_adornment",
    "surface_adornment_sha256",
    "surface_layer_lowering",
    "surface_layer_lowering_sha256",
    "surface_layer_retained_layers_sha256",
)
VISUAL_TEXTURE_MAP_FIELDS = (
    "texture_id",
    "texture_role",
    "mime_type",
    "byte_size",
    "sha256",
    "color_space",
    "width",
    "height",
    "source",
    "license",
    "fallback",
    "glb_image_index",
    "glb_texture_index",
)
OPERATION_ROLES_BY_PART_ROLE = {
    "base_form": {"base_form"},
    "turntable": {"secondary_form"},
    "joint_housing": {"secondary_form"},
    "link_armor": {"upper_link_form"},
    "cable_harness": {"visual_detail"},
    "end_effector_form": {"end_effector_form"},
    "surface_trim": {"trim"},
}
AUTOMATED_GATES = {
    "m108a": "FGC-M108A",
    "q003": "FGC-Q003",
    "g826": "FGC-G826",
}


class GateFailure(AssertionError):
    pass


T = TypeVar("T")


class ProductionExecutionTrace:
    """Count Provider calls at the execution-node boundary and deny every one."""

    def __init__(self) -> None:
        self.measured_provider_calls = 0
        self.nodes: list[dict[str, str]] = []

    def execute(
        self,
        node_id: str,
        authority: str,
        callback: Callable[[], T],
    ) -> T:
        self.nodes.append({"node_id": node_id, "authority": authority})
        if authority == "provider":
            self.measured_provider_calls += 1
            fail("C106_PROVIDER_CALL_FORBIDDEN", node_id)
        return callback()

    def measurement(self) -> dict[str, Any]:
        provider_nodes = [node for node in self.nodes if node["authority"] == "provider"]
        if provider_nodes or self.measured_provider_calls != 0:
            fail("C106_PROVIDER_CALL_FORBIDDEN")
        required = {
            "rust_recipe_dump",
            "restricted_geometry_compile_0",
            "restricted_geometry_compile_1",
            "restricted_geometry_compile_2",
            "rust_product_state_bind",
        }
        observed = {node["node_id"] for node in self.nodes}
        if not required <= observed:
            fail("C106_PROVIDER_MEASUREMENT_INCOMPLETE", ",".join(sorted(required - observed)))
        return {
            "schema_version": "C106ProviderCallMeasurement@1",
            "policy": "deny_on_call",
            "measurement_source": "execution_node_trace",
            "measured_provider_calls": self.measured_provider_calls,
            "provider_nodes": provider_nodes,
            "execution_nodes": list(self.nodes),
        }


def fail(code: str, detail: str = "") -> NoReturn:
    raise GateFailure(f"{code}: {detail}" if detail else code)


def canonical_sha256(value: object) -> str:
    return hashlib.sha256(
        json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")
    ).hexdigest()


def artifact_directory_for_name(name: str) -> Path:
    """Resolve an opt-in artifact directory beneath the repository output root.

    The Gate's default remains read-only.  Artifact extraction is deliberately
    restricted to one simple basename so a local visual-capture caller cannot
    redirect production GLB bytes into arbitrary paths.
    """

    if not re.fullmatch(r"[a-z0-9][a-z0-9_-]{0,63}", name):
        fail("C106_ARTIFACT_DIRECTORY_INVALID", name)
    return ROOT / "output" / name


def write_production_artifacts(
    artifact_directory_name: str,
    reports: list[Mapping[str, Any]],
    compiled: Mapping[str, Mapping[str, Any]],
) -> Path:
    destination = artifact_directory_for_name(artifact_directory_name)
    if destination.exists():
        fail("C106_ARTIFACT_DIRECTORY_EXISTS", artifact_directory_name)
    destination.mkdir(parents=True)

    artifact_entries: list[dict[str, Any]] = []
    try:
        for report in sorted(reports, key=lambda item: str(item["recipe_id"])):
            recipe_id = str(report["recipe_id"])
            glb = compiled[recipe_id].get("glb")
            readback = compiled[recipe_id].get("readback")
            if not isinstance(glb, bytes) or not isinstance(readback, Mapping):
                fail("C106_ARTIFACT_PAYLOAD_INVALID", recipe_id)
            file_name = f"{recipe_id.removeprefix('recipe_')}.glb"
            (destination / file_name).write_bytes(glb)
            artifact_entries.append(
                {
                    "recipe_id": recipe_id,
                    "file_name": file_name,
                    "glb_sha256": report["glb_sha256"],
                    "triangle_count": readback["triangle_count"],
                    "primitive_count": readback["primitive_count"],
                    "material_count": readback["material_count"],
                    "artifact_profile": readback["artifact_profile"],
                }
            )
        (destination / "readback-summary.json").write_text(
            json.dumps(
                {
                    "schema_version": ARTIFACT_SUMMARY_SCHEMA,
                    "status": "pass",
                    "formal_eligible": False,
                    "artifacts": artifact_entries,
                },
                ensure_ascii=False,
                indent=2,
                sort_keys=True,
            )
            + "\n",
            encoding="utf-8",
        )
    except BaseException:
        # The caller only receives a complete, independently verified set.
        # Do not leave a partially-written visual capture behind on failure.
        for path in destination.iterdir():
            path.unlink()
        destination.rmdir()
        raise
    return destination


def run_rust_dump() -> dict[str, Any]:
    command = [
        str(RUST_WRAPPER),
        "cargo",
        "run",
        "--quiet",
        "--manifest-path",
        str(MANIFEST),
        "-p",
        PACKAGE,
        "--bin",
        DUMP_BINARY,
        "--offline",
    ]
    completed = subprocess.run(
        command, cwd=ROOT, text=True, capture_output=True, timeout=600, check=False
    )
    if completed.returncode != 0:
        fail("C106_RUST_DUMP_FAILED", completed.stderr[-1600:])
    try:
        value = json.loads(completed.stdout)
    except json.JSONDecodeError as exc:
        raise GateFailure("C106_RUST_DUMP_INVALID_JSON") from exc
    if not isinstance(value, dict) or value.get("schema_version") != DUMP_SCHEMA:
        fail("C106_RUST_DUMP_SCHEMA_INVALID")
    return value


def mapping(value: object, code: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        fail(code)
    return value


def list_value(value: object, code: str) -> list[Any]:
    if not isinstance(value, list):
        fail(code)
    return value


def glb_document(payload: bytes) -> Mapping[str, Any]:
    offset = 12
    document: object = None
    while offset + 8 <= len(payload):
        length, kind = struct.unpack_from("<II", payload, offset)
        offset += 8
        chunk = payload[offset : offset + length]
        offset += length
        if kind == 0x4E4F534A:
            document = json.loads(chunk.rstrip(b" \x00").decode("utf-8"))
    return mapping(document, "C106_SPATIAL_GLB_DOCUMENT_INVALID")


def aabb_volume(bounds: tuple[list[float], list[float]]) -> float:
    return math.prod(max(0.0, bounds[1][axis] - bounds[0][axis]) for axis in range(3))


def aabb_intersection_volume(
    left: tuple[list[float], list[float]], right: tuple[list[float], list[float]]
) -> float:
    overlap = (
        [max(left[0][axis], right[0][axis]) for axis in range(3)],
        [min(left[1][axis], right[1][axis]) for axis in range(3)],
    )
    return aabb_volume(overlap)


def assert_spatial_bounds(
    part_bounds: Mapping[str, tuple[list[float], list[float]]],
) -> dict[str, Any]:
    if len(part_bounds) != 10:
        fail("C106_SPATIAL_PART_COVERAGE_INVALID", str(len(part_bounds)))
    volumes = {part_id: aabb_volume(bounds) for part_id, bounds in part_bounds.items()}
    if any(volume <= 1e-6 for volume in volumes.values()):
        fail("C106_SPATIAL_PART_VOLUME_INVALID")
    overall = (
        [min(bounds[0][axis] for bounds in part_bounds.values()) for axis in range(3)],
        [max(bounds[1][axis] for bounds in part_bounds.values()) for axis in range(3)],
    )
    extents = [overall[1][axis] - overall[0][axis] for axis in range(3)]
    longest_axis = max(range(3), key=extents.__getitem__)
    if extents[longest_axis] <= 1.0:
        fail("C106_SPATIAL_ASSEMBLY_COLLAPSED")
    centroids = {
        part_id: [(bounds[0][axis] + bounds[1][axis]) / 2.0 for axis in range(3)]
        for part_id, bounds in part_bounds.items()
    }
    centroid_span = max(value[longest_axis] for value in centroids.values()) - min(
        value[longest_axis] for value in centroids.values()
    )
    centroid_span_ratio = centroid_span / extents[longest_axis]
    if centroid_span_ratio < 0.55:
        fail("C106_SPATIAL_ASSEMBLY_PILED", f"centroid_span_ratio={centroid_span_ratio:.4f}")
    diagonal = math.sqrt(sum(extent * extent for extent in extents))
    minimum_centroid_separation = min(
        math.dist(left, right)
        for index, left in enumerate(centroids.values())
        for right in list(centroids.values())[index + 1 :]
    )
    if diagonal <= 0 or minimum_centroid_separation / diagonal < 0.01:
        fail("C106_SPATIAL_PART_CENTROIDS_COLLAPSED")
    maximum_overlap_ratio = 0.0
    for index, (left_id, left) in enumerate(part_bounds.items()):
        for right_id, right in list(part_bounds.items())[index + 1 :]:
            intersection = aabb_intersection_volume(left, right)
            if intersection <= 0:
                continue
            ratio = intersection / min(volumes[left_id], volumes[right_id])
            maximum_overlap_ratio = max(maximum_overlap_ratio, ratio)
            # A bounded connector/joint envelope may intersect adjacent armor,
            # but one semantic Part must never be almost entirely swallowed by
            # another. This catches the common "ten roles at one origin" bug
            # without rejecting deliberate joint/link seating.
            if ratio > 0.72:
                fail(
                    "C106_SPATIAL_PART_CONTAINMENT_INVALID",
                    f"{left_id}:{right_id}:{ratio:.4f}",
                )
    return {
        "part_aabb_count": len(part_bounds),
        "assembly_bounds": [round(value, 6) for value in extents],
        "centroid_span_ratio": round(centroid_span_ratio, 6),
        "minimum_centroid_separation": round(minimum_centroid_separation, 6),
        "maximum_overlap_ratio": round(maximum_overlap_ratio, 6),
        "containment_limit": 0.72,
    }


def assert_spatial_integrity(
    candidate: Mapping[str, Any], graph: Mapping[str, Any], glb: bytes
) -> dict[str, Any]:
    parts = list_value(graph.get("parts"), "C106_SPATIAL_PARTS_INVALID")
    prefix_owner: dict[str, str] = {}
    for item in parts:
        part = mapping(item, "C106_SPATIAL_PART_INVALID")
        part_id = part.get("part_id")
        instance_id = part.get("recipe_instance_id")
        if not isinstance(part_id, str) or not isinstance(instance_id, str):
            fail("C106_SPATIAL_PART_INVALID")
        prefix_owner[f"op_{instance_id.removeprefix('recipeinst_')}_"] = part_id
    document = glb_document(glb)
    accessors = list_value(document.get("accessors"), "C106_SPATIAL_ACCESSORS_INVALID")
    meshes = list_value(document.get("meshes"), "C106_SPATIAL_MESHES_INVALID")
    part_bounds: dict[str, tuple[list[float], list[float]]] = {}
    for mesh_value in meshes:
        mesh = mapping(mesh_value, "C106_SPATIAL_MESH_INVALID")
        for primitive_value in list_value(mesh.get("primitives"), "C106_SPATIAL_PRIMITIVES_INVALID"):
            primitive = mapping(primitive_value, "C106_SPATIAL_PRIMITIVE_INVALID")
            extras = mapping(primitive.get("extras"), "C106_SPATIAL_PROVENANCE_INVALID")
            feature_node_id = extras.get("forgecad_feature_node_id")
            owners = [part_id for prefix, part_id in prefix_owner.items() if isinstance(feature_node_id, str) and feature_node_id.startswith(prefix)]
            if len(owners) != 1:
                fail("C106_SPATIAL_PROVENANCE_INVALID", str(feature_node_id))
            attributes = mapping(primitive.get("attributes"), "C106_SPATIAL_POSITION_INVALID")
            position_index = attributes.get("POSITION")
            if not isinstance(position_index, int) or not 0 <= position_index < len(accessors):
                fail("C106_SPATIAL_POSITION_INVALID")
            accessor = mapping(accessors[position_index], "C106_SPATIAL_POSITION_INVALID")
            minimum = accessor.get("min")
            maximum = accessor.get("max")
            if (
                not isinstance(minimum, list)
                or not isinstance(maximum, list)
                or len(minimum) != 3
                or len(maximum) != 3
                or any(not isinstance(value, (int, float)) or not math.isfinite(float(value)) for value in [*minimum, *maximum])
            ):
                fail("C106_SPATIAL_POSITION_INVALID")
            bounds = ([float(value) for value in minimum], [float(value) for value in maximum])
            owner = owners[0]
            existing = part_bounds.get(owner)
            if existing is None:
                part_bounds[owner] = bounds
            else:
                part_bounds[owner] = (
                    [min(existing[0][axis], bounds[0][axis]) for axis in range(3)],
                    [max(existing[1][axis], bounds[1][axis]) for axis in range(3)],
                )
    return assert_spatial_bounds(part_bounds)


def canonical_surface_provenance(value: object, code: str) -> list[dict[str, Any]]:
    """Make GLB provenance comparison independent of JSON field/list ordering.

    Primitive ordering follows GLB mesh packing, while role/source-operation
    collections are identity sets once their own records carry explicit IDs or
    triangle ranges.  The canonical form deliberately retains every declared
    provenance field, including face hashes, topology and source operations.
    """

    records: list[dict[str, Any]] = []
    primitive_ids: set[str] = set()
    for raw_item in list_value(value, code):
        item = mapping(raw_item, code)
        unknown = set(item) - set(SURFACE_PROVENANCE_FIELDS)
        if unknown:
            fail(code, f"unknown_fields={sorted(unknown)}")
        primitive_id = item.get("primitive_id")
        if not isinstance(primitive_id, str) or primitive_id in primitive_ids:
            fail(code, "primitive identity")
        primitive_ids.add(primitive_id)
        normalized = {field: item.get(field) for field in SURFACE_PROVENANCE_FIELDS}
        for field in ("surface_roles", "source_operation_ids", "tangent_handedness"):
            values = normalized[field]
            if not isinstance(values, list) or len(values) != len(set(values)):
                fail(code, field)
            normalized[field] = sorted(values)
        ranges = normalized["surface_ranges"]
        if not isinstance(ranges, list) or any(not isinstance(item, Mapping) for item in ranges):
            fail(code, "surface_ranges")
        normalized["surface_ranges"] = sorted(
            (dict(item) for item in ranges),
            key=lambda item: (
                str(item.get("surface_role")),
                int(item.get("first_triangle", -1)),
                int(item.get("triangle_count", -1)),
            ),
        )
        records.append(normalized)
    return sorted(records, key=lambda item: str(item["primitive_id"]))


def assert_surface_provenance_matches(readback: object, glb_facts: object) -> None:
    if canonical_surface_provenance(readback, "C106_READBACK_SURFACE_INVALID") != canonical_surface_provenance(
        glb_facts, "C106_GLB_SURFACE_INVALID"
    ):
        fail("C106_GLB_FACT_DRIFT", "surface_provenance")


def canonical_visual_texture_sets(value: object, code: str) -> list[dict[str, Any]]:
    """Compare PBR evidence by identity rather than GLB packing/list order.

    The resulting representation retains every public texture-set/map field;
    it only canonicalizes collections which the typed contract defines as
    identity sets (sets, zones, extensions and map roles), plus absent optional
    adornment fields represented as ``null`` by Pydantic.
    """

    sets: list[dict[str, Any]] = []
    set_ids: set[str] = set()
    for raw_set in list_value(value, code):
        texture_set = mapping(raw_set, code)
        unknown = set(texture_set) - set(VISUAL_TEXTURE_SET_FIELDS)
        if unknown:
            fail(code, f"unknown_set_fields={sorted(unknown)}")
        texture_set_id = texture_set.get("visual_texture_set_id")
        if not isinstance(texture_set_id, str) or texture_set_id in set_ids:
            fail(code, "texture set identity")
        set_ids.add(texture_set_id)
        normalized = {field: texture_set.get(field) for field in VISUAL_TEXTURE_SET_FIELDS}
        for field in ("material_zone_ids", "extensions"):
            values = normalized[field]
            if not isinstance(values, list) or len(values) != len(set(values)):
                fail(code, field)
            normalized[field] = sorted(values)
        maps_by_role: dict[str, dict[str, Any]] = {}
        for raw_map in list_value(normalized["maps"], code):
            texture_map = mapping(raw_map, code)
            unknown_map = set(texture_map) - set(VISUAL_TEXTURE_MAP_FIELDS)
            if unknown_map:
                fail(code, f"unknown_map_fields={sorted(unknown_map)}")
            role = texture_map.get("texture_role")
            if not isinstance(role, str) or role in maps_by_role:
                fail(code, "texture map role")
            maps_by_role[role] = {
                field: texture_map.get(field) for field in VISUAL_TEXTURE_MAP_FIELDS
            }
        if set(maps_by_role) != PBR_ROLES:
            fail(code, "texture map coverage")
        normalized["maps"] = [maps_by_role[role] for role in sorted(maps_by_role)]
        sets.append(normalized)
    return sorted(
        sets,
        key=lambda item: (
            str(item["visual_texture_set_id"]),
            str(item["material_id"]),
            int(item["material_index"]),
        ),
    )


def assert_visual_texture_sets_match(readback: object, glb_facts: object) -> None:
    if canonical_visual_texture_sets(readback, "C106_READBACK_TEXTURE_INVALID") != canonical_visual_texture_sets(
        glb_facts, "C106_GLB_TEXTURE_INVALID"
    ):
        fail("C106_GLB_FACT_DRIFT", "visual_texture_sets")


def candidate_domain(candidate: Mapping[str, Any]) -> str:
    instances = list_value(candidate.get("component_recipe_instances"), "C106_INSTANCE_INVALID")
    domains = {
        item.get("domain_pack_id")
        for item in instances
        if isinstance(item, Mapping) and isinstance(item.get("domain_pack_id"), str)
    }
    if domains != {DOMAIN}:
        fail("C106_DOMAIN_INVALID", repr(sorted(domains)))
    return DOMAIN


def assert_candidate(candidate: Mapping[str, Any]) -> tuple[str, Mapping[str, Any], list[Any]]:
    recipe = mapping(candidate.get("recipe"), "C106_RECIPE_INVALID")
    recipe_id = recipe.get("recipe_id")
    if not isinstance(recipe_id, str) or recipe_id not in ROOT_IDS:
        fail("C106_ROOT_ID_INVALID", str(recipe_id))
    candidate_domain(candidate)
    graph = mapping(candidate.get("expanded_assembly_graph"), "C106_ASSEMBLY_GRAPH_INVALID")
    parts = list_value(graph.get("parts"), "C106_PARTS_INVALID")
    connections = list_value(graph.get("connections"), "C106_CONNECTIONS_INVALID")
    if len(parts) != 10:
        fail("C106_PART_COUNT_INVALID", f"{recipe_id}:{len(parts)}")
    if len(connections) != 9:
        fail("C106_CONNECTION_COUNT_INVALID", f"{recipe_id}:{len(connections)}")

    part_ids: set[str] = set()
    instance_ids: set[str] = set()
    connectors_by_part: dict[str, set[str]] = {}
    roles: list[str] = []
    zone_ids: set[str] = set()
    for item in parts:
        part = mapping(item, "C106_PART_INVALID")
        part_id = part.get("part_id")
        instance_id = part.get("recipe_instance_id")
        role = part.get("role")
        if not isinstance(part_id, str) or part_id in part_ids:
            fail("C106_PART_ID_INVALID", recipe_id)
        part_ids.add(part_id)
        if not isinstance(instance_id, str) or instance_id in instance_ids:
            fail("C106_PART_INSTANCE_INVALID", recipe_id)
        instance_ids.add(instance_id)
        if not isinstance(role, str):
            fail("C106_PART_ROLE_INVALID", recipe_id)
        roles.append(role)
        pivot = mapping(part.get("pivot"), "C106_PART_PIVOT_INVALID")
        if not all(isinstance(pivot.get(field), list) and len(pivot[field]) == 3 for field in ("position", "normal", "up")):
            fail("C106_PART_PIVOT_INVALID", recipe_id)
        part_connectors = list_value(part.get("connectors"), "C106_PART_CONNECTORS_INVALID")
        part_connector_ids: set[str] = set()
        for connector_value in part_connectors:
            connector = mapping(connector_value, "C106_CONNECTOR_INVALID")
            connector_id = connector.get("connector_id")
            if not isinstance(connector_id, str) or connector_id in part_connector_ids:
                fail("C106_CONNECTOR_ID_INVALID", recipe_id)
            part_connector_ids.add(connector_id)
        connectors_by_part[part_id] = part_connector_ids
        material_zones = list_value(part.get("material_zone_ids"), "C106_PART_ZONE_INVALID")
        if not material_zones or any(not isinstance(zone, str) for zone in material_zones):
            fail("C106_PART_ZONE_INVALID", recipe_id)
        zone_ids.update(material_zones)
    if Counter(roles) != REQUIRED_ROLES:
        fail("C106_REQUIRED_ROLES_INVALID", f"{recipe_id}:{dict(Counter(roles))}")
    # The MVP visual-quality increment keeps the same ten semantic Parts but
    # gives their blue paint, metal trim, rubber channels and signal accents
    # explicit immutable Material Zones instead of flattening them into one
    # grey zone per Part.  The service plinth owns three additional zones.
    if len(zone_ids) > 19:
        fail("C106_ZONE_COUNT_INVALID", f"{recipe_id}:{len(zone_ids)}")
    if sum(len(item) for item in connectors_by_part.values()) < 18:
        fail("C106_CONNECTOR_COUNT_INVALID", recipe_id)
    connection_ids: set[str] = set()
    for item in connections:
        connection = mapping(item, "C106_CONNECTION_INVALID")
        connection_id = connection.get("connection_id")
        if not isinstance(connection_id, str) or connection_id in connection_ids:
            fail("C106_CONNECTION_ID_INVALID", recipe_id)
        connection_ids.add(connection_id)
        if connection.get("status") != "connected":
            fail("C106_CONNECTION_STATUS_INVALID", recipe_id)
        from_part = connection.get("from_part_id")
        to_part = connection.get("to_part_id")
        if from_part not in part_ids or to_part not in part_ids:
            fail("C106_CONNECTION_REFERENCE_INVALID", f"{recipe_id}:part")
        if connection.get("from_connector_id") not in connectors_by_part[from_part]:
            fail("C106_CONNECTION_REFERENCE_INVALID", f"{recipe_id}:from_connector")
        if connection.get("to_connector_id") not in connectors_by_part[to_part]:
            fail("C106_CONNECTION_REFERENCE_INVALID", f"{recipe_id}:to_connector")
    return recipe_id, graph, parts


def assert_readback(
    *,
    candidate: Mapping[str, Any],
    graph: Mapping[str, Any],
    glb: bytes,
    glb_sha256: str,
    result_readback: Mapping[str, Any],
    expected_shape_sha256: str,
) -> dict[str, Any]:
    from forgecad_agent.application.geometry_models import GeometryCompileReadback
    from forgecad_agent.application.geometry_worker import read_shape_program_glb_facts

    typed_readback = {
        field: value
        for field, value in result_readback.items()
        if field in GeometryCompileReadback.model_fields
    }
    readback = GeometryCompileReadback.model_validate(typed_readback).model_dump(mode="json")
    if readback.get("schema_version") != "GeometryCompileReadback@2":
        fail("C106_READBACK_SCHEMA_INVALID")
    assert_readback_identity(readback, glb_sha256)
    if hashlib.sha256(glb).hexdigest() != glb_sha256:
        fail("C106_GLB_READBACK_HASH_DRIFT")
    triangles = int(readback.get("triangle_count", 0))
    recipe_id = str(mapping(candidate.get("recipe"), "C106_RECIPE_INVALID").get("recipe_id"))
    # C107 keeps the ShapeProgram at the existing 48-output cap.  The focused
    # service-display derivative alone expands root-local maintenance hardware
    # into 32 radial fasteners and 24 compact signal pods.  Its two compact
    # siblings intentionally retain their smaller real geometry budget, so a
    # service-only minimum must not reject those independently reviewed roots.
    triangle_min, triangle_max = (
        (80_000, 150_000)
        if recipe_id == "recipe_c106_arm_service_display"
        else (5_000, 40_000)
    )
    if not triangle_min <= triangles <= triangle_max:
        fail("C106_TRIANGLE_BUDGET_INVALID", str(triangles))
    primitives = int(readback.get("primitive_count", 0))
    # The bounded 48-output contract intentionally expands two root-local
    # visual arrays into 32 top-deck fasteners and 24 front signal pods.  This
    # keeps production-visible hardware detail without broadening the
    # ShapeProgram operation set or output count.
    if not 10 <= primitives <= 128:
        fail("C106_PRIMITIVE_BUDGET_INVALID", str(primitives))
    if any(int(readback.get(field, 0)) != primitives for field in ("uv0_primitive_count", "normal_primitive_count", "tangent_primitive_count")):
        fail("C106_VERTEX_FACTS_INVALID")

    parts_by_id = {
        part["part_id"]: {
            "part_id": part["part_id"],
            "recipe_instance_id": part["recipe_instance_id"],
            "zones": set(part["material_zone_ids"]),
            "role": part.get("role"),
            "operation_id": part.get("operation_id"),
            "output_id": part.get("output_id"),
        }
        for part in list_value(graph.get("parts"), "C106_PARTS_INVALID")
        if (
            isinstance(part, Mapping)
            and isinstance(part.get("part_id"), str)
            and isinstance(part.get("recipe_instance_id"), str)
            and isinstance(part.get("operation_id"), str)
        )
    }
    if len(parts_by_id) != 10:
        fail("C106_PART_OWNERSHIP_INVALID")

    # The expanded graph exposes one canonical operation per semantic Part, but
    # a C106 recipe instance can contribute several render operations (for
    # example, an arm link body plus its armor panels).  Provenance is therefore
    # joined by the Rust-owned recipe-instance digest in the operation ID, then
    # constrained by the operation render role and that Part's declared zones.
    operation_owner: dict[str, Mapping[str, Any]] = {}
    operations = list_value(
        mapping(candidate.get("expanded_shape_program"), "C106_PROGRAM_INVALID").get("operations"),
        "C106_PROGRAM_INVALID",
    )
    for operation_value in operations:
        operation = mapping(operation_value, "C106_OPERATION_OWNERSHIP_INVALID")
        operation_id = operation.get("operation_id")
        arguments = mapping(operation.get("args"), "C106_OPERATION_OWNERSHIP_INVALID")
        if not isinstance(operation_id, str):
            fail("C106_OPERATION_OWNERSHIP_INVALID")
        owners = [
            part
            for part in parts_by_id.values()
            if operation_id.startswith(f"op_{part['recipe_instance_id'].removeprefix('recipeinst_')}_")
        ]
        if len(owners) != 1:
            fail("C106_OPERATION_OWNERSHIP_INVALID", operation_id)
        owner = owners[0]
        zone_id = arguments.get("zone_id")
        operation_role = arguments.get("part_role")
        if zone_id is None and operation_role is None:
            continue
        if not isinstance(zone_id, str) or not isinstance(operation_role, str):
            fail("C106_OPERATION_OWNERSHIP_INVALID", operation_id)
        if zone_id not in owner["zones"]:
            fail("C106_OPERATION_OWNERSHIP_INVALID", operation_id)
        allowed_roles = OPERATION_ROLES_BY_PART_ROLE.get(owner["role"])
        if allowed_roles is None or operation_role not in allowed_roles:
            fail("C106_OPERATION_OWNERSHIP_INVALID", operation_id)
        operation_owner[operation_id] = owner
    if {part["operation_id"] for part in parts_by_id.values()} - set(operation_owner):
        fail("C106_OPERATION_OWNERSHIP_INVALID")

    surfaces = list_value(readback.get("surface_provenance"), "C106_SURFACE_FACTS_INVALID")
    zones = list_value(readback.get("material_zone_faces"), "C106_ZONE_FACE_FACTS_INVALID")
    if len(surfaces) != primitives or len(zones) != primitives:
        fail("C106_FACE_MAPPING_COUNT_INVALID")
    by_primitive = {item.get("primitive_id"): item for item in zones if isinstance(item, Mapping)}
    if len(by_primitive) != primitives:
        fail("C106_ZONE_FACE_FACTS_INVALID")
    observed_part_ids: set[str] = set()
    for surface_value in surfaces:
        surface = mapping(surface_value, "C106_SURFACE_FACTS_INVALID")
        primitive_id = surface.get("primitive_id")
        zone = by_primitive.get(primitive_id)
        if not isinstance(primitive_id, str) or not isinstance(zone, Mapping):
            fail("C106_FACE_MAPPING_INVALID")
        if surface.get("closed") is not True or any(int(surface.get(field, 1)) != 0 for field in ("boundary_edge_count", "non_manifold_edge_count", "degenerate_triangle_count", "uv_degenerate_triangle_count", "tangent_fallback_triangle_count")):
            fail("C106_TOPOLOGY_INVALID", primitive_id)
        source_operations = surface.get("source_operation_ids")
        if not isinstance(source_operations, list):
            fail("C106_FACE_PART_ZONE_INVALID", primitive_id)
        matched_parts = {
            operation_owner[operation]["part_id"]
            for operation in source_operations
            if isinstance(operation, str) and operation in operation_owner
        }
        if len(matched_parts) != 1:
            fail("C106_FACE_PART_ZONE_INVALID", primitive_id)
        expected_part = parts_by_id[matched_parts.pop()]
        feature_node_id = surface.get("feature_node_id")
        if not isinstance(feature_node_id, str) or operation_owner.get(feature_node_id) != expected_part:
            fail("C106_FACE_PART_ZONE_INVALID", primitive_id)
        zone_id = surface.get("material_zone_id")
        if zone_id not in expected_part["zones"]:
            fail("C106_FACE_PART_ZONE_INVALID", primitive_id)
        if zone.get("material_zone_id") != zone_id or zone.get("face_id_sha256") != surface.get("face_id_sha256"):
            fail("C106_FACE_PART_ZONE_DRIFT", primitive_id)
        observed_part_ids.add(expected_part["part_id"])
    if observed_part_ids != set(parts_by_id):
        fail("C106_FACE_PART_COVERAGE_INVALID")
    texture_sets = list_value(readback.get("visual_texture_sets"), "C106_PBR_MISSING")
    if not texture_sets:
        fail("C106_PBR_MISSING")
    for texture_set_value in texture_sets:
        texture_set = mapping(texture_set_value, "C106_PBR_INVALID")
        if not isinstance(texture_set.get("visual_texture_set_id"), str) or not texture_set["visual_texture_set_id"].endswith("_builtin_v4"):
            fail("C106_PBR_INVALID")
        maps = list_value(texture_set.get("maps"), "C106_PBR_INVALID")
        if len(maps) != 5 or {item.get("texture_role") for item in maps if isinstance(item, Mapping)} != PBR_ROLES:
            fail("C106_PBR_INVALID")
        if any(not isinstance(item, Mapping) or item.get("width") != 1024 or item.get("height") != 1024 for item in maps):
            fail("C106_PBR_INVALID")
    facts = asdict(read_shape_program_glb_facts(glb))
    for field in ("triangle_count", "bounds_mm", "mesh_count", "primitive_count", "material_count", "uv0_primitive_count", "normal_primitive_count", "tangent_primitive_count", "surface_provenance", "material_zone_faces", "visual_texture_sets", "feature_history", "artifact_profile"):
        if field == "surface_provenance":
            assert_surface_provenance_matches(readback.get(field), facts.get(field))
            continue
        if field == "visual_texture_sets":
            assert_visual_texture_sets_match(readback.get(field), facts.get(field))
            continue
        if facts.get(field) != readback.get(field):
            fail("C106_GLB_FACT_DRIFT", field)
    if readback.get("shape_program_sha256") != expected_shape_sha256:
        fail("C106_SHAPE_PROGRAM_HASH_DRIFT")
    return readback


def assert_readback_identity(readback: Mapping[str, Any], expected_glb_sha256: str) -> None:
    if readback.get("glb_sha256") != expected_glb_sha256:
        fail("C106_GLB_READBACK_HASH_DRIFT")
    profile = readback.get("artifact_profile")
    if not isinstance(profile, Mapping) or profile.get("artifact_profile_id") != "production_concept":
        fail("C106_PROFILE_INVALID")


def bind_product_state(fixtures: list[dict[str, Any]]) -> dict[str, Mapping[str, Any]]:
    payload = {
        "schema_version": "C106ProductionEvidenceBindingInput@1",
        "fixtures": fixtures,
    }
    with tempfile.TemporaryDirectory(prefix="forgecad-c106-evidence-input-") as raw:
        input_path = Path(raw) / "input.json"
        input_path.write_text(
            json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":")),
            encoding="utf-8",
        )
        command = [
            str(RUST_WRAPPER),
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(MANIFEST),
            "-p",
            PACKAGE,
            "--bin",
            BINDING_BINARY,
            "--offline",
            "--",
            str(input_path),
        ]
        completed = subprocess.run(
            command, cwd=ROOT, text=True, capture_output=True, timeout=600, check=False
        )
    if completed.returncode != 0:
        fail("C106_PRODUCT_STATE_BINDING_FAILED", completed.stderr[-2400:])
    try:
        output = json.loads(completed.stdout)
    except json.JSONDecodeError as exc:
        raise GateFailure("C106_PRODUCT_STATE_BINDING_INVALID_JSON") from exc
    if not isinstance(output, dict) or output.get("schema_version") != "C106ProductionEvidenceBinding@1":
        fail("C106_PRODUCT_STATE_BINDING_SCHEMA_INVALID")
    bindings = list_value(output.get("bindings"), "C106_PRODUCT_STATE_BINDINGS_INVALID")
    by_recipe: dict[str, Mapping[str, Any]] = {}
    for binding_value in bindings:
        binding = mapping(binding_value, "C106_PRODUCT_STATE_BINDING_INVALID")
        recipe_id = binding.get("recipe_id")
        if not isinstance(recipe_id, str) or recipe_id in by_recipe:
            fail("C106_PRODUCT_STATE_BINDING_INVALID")
        by_recipe[recipe_id] = binding
    if set(by_recipe) != ROOT_IDS:
        fail("C106_PRODUCT_STATE_BINDINGS_INVALID")
    return by_recipe


def assert_product_state_binding(
    *,
    binding: Mapping[str, Any],
    recipe_id: str,
    candidate_sha256: str,
    glb_sha256: str,
    readback_sha256: str,
) -> None:
    asset_version_id = binding.get("asset_version_id")
    mismatches: list[str] = []
    for field, expected in (
        ("recipe_id", recipe_id),
        ("candidate_sha256", candidate_sha256),
        ("production_glb_sha256", glb_sha256),
        ("readback_sha256", readback_sha256),
    ):
        if binding.get(field) != expected:
            actual = binding.get(field)
            mismatches.append(
                f"{field}:{str(expected)[:12]}!={str(actual)[:12]}"
            )
    if not isinstance(asset_version_id, str):
        mismatches.append("asset_version_id:type")
    else:
        for field in (
            "snapshot_asset_version_id",
            "quality_asset_version_id",
            "export_source_version_id",
        ):
            if binding.get(field) != asset_version_id:
                mismatches.append(f"{field}:drift")
    if binding.get("restart_readback") is not True:
        mismatches.append("restart_readback:false")
    for field in ("quality_report_id", "artifact_id", "project_id"):
        if not isinstance(binding.get(field), str):
            mismatches.append(f"{field}:type")
    if int(binding.get("snapshot_revision", 0)) < 2:
        mismatches.append("snapshot_revision:lt2")
    if mismatches:
        fail("C106_PRODUCT_STATE_IDENTITY_DRIFT", f"{recipe_id}:{','.join(mismatches)}")


def independent_gate_evidence(
    *,
    recipe_id: str,
    candidate_sha256: str,
    glb_sha256: str,
    readback_sha256: str,
    readback: Mapping[str, Any],
    binding: Mapping[str, Any],
    spatial_checks: Mapping[str, Any],
    provider_measurement: Mapping[str, Any],
) -> dict[str, Any]:
    try:
        check_sets = {
            "m108a": _m108a_checks(readback),
            "q003": _q003_checks(readback),
            "g826": _g826_checks(readback),
        }
    except PreflightError as error:
        fail("C106_AUTOMATED_GATE_FAILED", str(error))
    check_sets["q003"].update(
        {
            "quality_report_id": binding["quality_report_id"],
            "quality_asset_version_id": binding["quality_asset_version_id"],
            "export_source_version_id": binding["export_source_version_id"],
            "production_glb_sha256": binding["production_glb_sha256"],
        }
    )
    check_sets["g826"]["spatial_integrity"] = dict(spatial_checks)
    reports: dict[str, Any] = {"readback_sha256": readback_sha256}
    report_hashes: set[str] = set()
    execution_ids: set[str] = set()
    for gate_key, gate_id in AUTOMATED_GATES.items():
        execution_id = f"c106-{recipe_id.removeprefix('recipe_c106_')}-{gate_key}-{glb_sha256[:12]}"
        report = {
            "schema_version": "C106AutomatedGateEvidence@1",
            "gate_id": gate_id,
            "status": "passed",
            "execution_id": execution_id,
            "candidate_sha256": candidate_sha256,
            "source_glb_sha256": glb_sha256,
            "readback_sha256": readback_sha256,
            "project_id": binding["project_id"],
            "asset_version_id": binding["asset_version_id"],
            "quality_report_id": binding["quality_report_id"],
            "export_source_version_id": binding["export_source_version_id"],
            "checks": check_sets[gate_key],
            "measured_provider_calls": provider_measurement["measured_provider_calls"],
            "provider_measurement": dict(provider_measurement),
            "formal_eligible": False,
        }
        report_hash = canonical_sha256(report)
        reports[gate_key] = {"report": report, "report_sha256": report_hash}
        report_hashes.add(report_hash)
        execution_ids.add(execution_id)
    if len(report_hashes) != 3 or len(execution_ids) != 3:
        fail("C106_AUTOMATED_GATE_EVIDENCE_REUSED", recipe_id)
    return reports


def compile_candidate(
    index: int,
    candidate: Mapping[str, Any],
    graph: Mapping[str, Any],
    shape_program_seal: Mapping[str, Any],
    executor: Any,
    trace: ProductionExecutionTrace,
) -> tuple[dict[str, Any], Mapping[str, Any], bytes, dict[str, Any]]:
    from forgecad_agent.application.restricted_geometry_executor import (
        RestrictedGeometryExecutionRequest,
    )

    canonical_shape_program = shape_program_seal.get("shape_program_canonical_json")
    expected_shape_sha256 = shape_program_seal.get("shape_program_sha256")
    if (
        shape_program_seal.get("recipe_id")
        != mapping(candidate.get("recipe"), "C106_RECIPE_INVALID").get("recipe_id")
        or not isinstance(canonical_shape_program, str)
        or not isinstance(expected_shape_sha256, str)
        or len(expected_shape_sha256) != 64
    ):
        fail("C106_SHAPE_PROGRAM_SEAL_INVALID", str(index))
    request = RestrictedGeometryExecutionRequest.model_validate(
        {
            "schema_version": "RestrictedGeometryExecutionRequest@1",
            "protocol_version": "forgecad.restricted-geometry/1",
            "execution_id": f"exec_c106_production_{index}",
            "idempotency_key": f"idem_c106_production_{index}",
            "cancellation_id": f"cancel_c106_production_{index}",
            "cancellation_token": f"token_c106_production_{index}",
            "action": "compile_readback",
            "timeout_ms": 120_000,
            "artifact_profile_id": "production_concept",
            "shape_program": candidate.get("expanded_shape_program"),
            "shape_program_canonical_json": canonical_shape_program,
            "shape_program_sha256": expected_shape_sha256,
        }
    )
    result = trace.execute(
        f"restricted_geometry_compile_{index}",
        "restricted_geometry_executor",
        lambda: executor.execute(request),
    )
    if result.readback is None or result.glb_base64 is None:
        fail("C106_RESTRICTED_GEOMETRY_RESULT_INVALID", str(index))
    try:
        glb = base64.b64decode(result.glb_base64, validate=True)
    except ValueError as error:
        fail("C106_RESTRICTED_GEOMETRY_GLB_INVALID", str(error))
    result_readback = result.readback
    glb_sha256 = result.glb_sha256
    if not isinstance(glb_sha256, str):
        fail("C106_RESTRICTED_GEOMETRY_HASH_INVALID", str(index))
    readback = assert_readback(
        candidate=candidate,
        graph=graph,
        glb=glb,
        glb_sha256=glb_sha256,
        result_readback=mapping(result_readback, "C106_READBACK_INVALID"),
        expected_shape_sha256=expected_shape_sha256,
    )
    spatial_checks = assert_spatial_integrity(candidate, graph, glb)
    return {
        "recipe_id": mapping(candidate.get("recipe"), "C106_RECIPE_INVALID")["recipe_id"],
        "candidate_sha256": candidate.get("candidate_sha256"),
        "shape_program_sha256": readback["shape_program_sha256"],
        "glb_sha256": glb_sha256,
        "triangle_count": readback["triangle_count"],
        "primitive_count": readback["primitive_count"],
    }, readback, glb, spatial_checks


def expect_failure(code: str, callback: Any) -> None:
    try:
        callback()
    except GateFailure as error:
        if str(error).startswith(code):
            return
        raise
    fail("C106_NEGATIVE_ACCEPTED", code)


def assert_distinct_artifacts(
    root_ids: set[str],
    candidate_hashes: set[str],
    semantic_hashes: set[str],
    glb_hashes: set[str],
) -> None:
    if root_ids != ROOT_IDS or len(candidate_hashes) != 3 or len(semantic_hashes) != 3 or len(glb_hashes) != 3:
        fail("C106_DISTINCT_ROOT_OR_ARTIFACT_INVALID")


def run_negative_probes(
    candidate: Mapping[str, Any], readback: Mapping[str, Any], expected_glb_sha256: str
) -> None:
    missing_role = copy.deepcopy(candidate)
    missing_role["expanded_assembly_graph"]["parts"][0]["role"] = "surface_trim"
    expect_failure("C106_REQUIRED_ROLES_INVALID", lambda: assert_candidate(missing_role))
    expect_failure(
        "C106_DISTINCT_ROOT_OR_ARTIFACT_INVALID",
        lambda: assert_distinct_artifacts(ROOT_IDS, {"a" * 64}, {"b" * 64}, {"c" * 64}),
    )
    preview_readback = copy.deepcopy(dict(readback))
    preview_readback["artifact_profile"]["artifact_profile_id"] = "interactive_preview"
    expect_failure("C106_PROFILE_INVALID", lambda: assert_readback_identity(preview_readback, expected_glb_sha256))
    tampered = copy.deepcopy(dict(readback))
    tampered["glb_sha256"] = "0" * 64
    expect_failure("C106_GLB_READBACK_HASH_DRIFT", lambda: assert_readback_identity(tampered, expected_glb_sha256))


def self_test_candidate() -> dict[str, Any]:
    roles = [
        "base_form", "turntable", "joint_housing", "joint_housing", "joint_housing",
        "link_armor", "link_armor", "cable_harness", "end_effector_form", "surface_trim",
    ]
    parts: list[dict[str, Any]] = []
    for index, role in enumerate(roles):
        parts.append(
            {
                "part_id": f"part_{index}",
                "recipe_instance_id": f"recipeinst_{index}",
                "operation_id": f"op_{index}",
                "output_id": f"output_{index}",
                "role": role,
                "pivot": {"position": [0, 0, 0], "normal": [1, 0, 0], "up": [0, 0, 1]},
                "connectors": [
                    {"connector_id": f"connector_{slot}"}
                    for slot in (range(9) if index == 0 else (0,))
                ],
                "material_zone_ids": [f"zone_{index % 5}"],
            }
        )
    return {
        "recipe": {"recipe_id": "recipe_c106_arm_desktop_assistant"},
        "candidate_sha256": "a" * 64,
        "component_recipe_instances": [{"domain_pack_id": DOMAIN}],
        "expanded_shape_program": {"schema_version": "ShapeProgram@1", "operations": []},
        "expanded_assembly_graph": {
            "parts": parts,
            "connections": [
                {
                    "connection_id": f"connection_{index}",
                    "status": "connected",
                    "from_part_id": "part_0",
                    "to_part_id": f"part_{index + 1}",
                    "from_connector_id": f"connector_{index}",
                    "to_connector_id": "connector_0",
                }
                for index in range(9)
            ],
        },
    }


def self_test() -> int:
    candidate = self_test_candidate()
    assert_candidate(candidate)
    separated_bounds = {
        f"part_{index}": ([float(index), 0.0, 0.0], [float(index) + 0.2, 0.2, 0.2])
        for index in range(10)
    }
    assert_spatial_bounds(separated_bounds)
    piled_bounds = {
        f"part_{index}": ([0.0, 0.0, 0.0], [0.2, 0.2, 0.2])
        for index in range(10)
    }
    expect_failure(
        "C106_SPATIAL_ASSEMBLY_COLLAPSED",
        lambda: assert_spatial_bounds(piled_bounds),
    )
    assert_distinct_artifacts(ROOT_IDS, {"a" * 64, "b" * 64, "c" * 64}, {"d" * 64, "e" * 64, "f" * 64}, {"1" * 64, "2" * 64, "3" * 64})
    provenance = {
        "primitive_id": "primitive_self_test",
        "part_instance_id": "partface_self_test",
        "part_role": "visual_detail",
        "profile_input_id": None,
        "surface_roles": ["side", "top"],
        "surface_ranges": [
            {"surface_role": "top", "first_triangle": 8, "triangle_count": 4},
            {"surface_role": "side", "first_triangle": 0, "triangle_count": 8},
        ],
        "uv0_min": [0.0, 0.0],
        "uv0_max": [1.0, 1.0],
        "closed": True,
        "boundary_edge_count": 0,
        "non_manifold_edge_count": 0,
        "degenerate_triangle_count": 0,
        "feature_node_id": "op_self_test",
        "source_operation_ids": ["op_self_test_b", "op_self_test_a"],
        "material_zone_id": "zone_self_test",
        "boolean_backside": None,
        "normal_mode": "split",
        "tangent_min_length": 1.0,
        "tangent_max_length": 1.0,
        "tangent_handedness": [1, -1],
        "uv_degenerate_triangle_count": 0,
        "tangent_fallback_triangle_count": 0,
        "face_id_min": 0,
        "face_id_max": 11,
        "face_id_sha256": "f" * 64,
        "edge_finish": {"mode": "none", "edge_set": "none", "selected_edge_count": 0, "radius_ratio": 0, "subdivision_count": 0},
        "texture_ready": True,
    }
    reordered_provenance = copy.deepcopy(provenance)
    reordered_provenance["surface_roles"].reverse()
    reordered_provenance["surface_ranges"].reverse()
    reordered_provenance["source_operation_ids"].reverse()
    reordered_provenance["tangent_handedness"].reverse()
    assert_surface_provenance_matches([provenance], [reordered_provenance])
    changed_provenance = copy.deepcopy(provenance)
    changed_provenance["face_id_sha256"] = "0" * 64
    expect_failure(
        "C106_GLB_FACT_DRIFT",
        lambda: assert_surface_provenance_matches([provenance], [changed_provenance]),
    )
    texture_set = {
        "schema_version": "VisualTextureSet@1",
        "visual_texture_set_id": "vtexset_self_test_builtin_v4",
        "material_id": "mat_self_test",
        "texture_material_id": "mat_self_test",
        "material_index": 3,
        "material_zone_ids": ["zone_self_test_b", "zone_self_test_a"],
        "maps": [
            {
                "texture_id": f"vtex_self_test_{role}",
                "texture_role": role,
                "mime_type": "image/png",
                "byte_size": 100 + index,
                "sha256": f"{index:x}" * 64,
                "color_space": "srgb" if role in {"base_color", "emissive"} else "linear",
                "width": 1024,
                "height": 1024,
                "source": "forgecad_builtin",
                "license": "not_applicable",
                "fallback": "none",
                "glb_image_index": index,
                "glb_texture_index": index,
            }
            for index, role in enumerate(sorted(PBR_ROLES))
        ],
        "extensions": ["KHR_materials_ior", "KHR_materials_clearcoat"],
        "texture_byte_size": sum(100 + index for index in range(5)),
        "surface_adornment": None,
        "surface_adornment_sha256": None,
    }
    reordered_texture_set = copy.deepcopy(texture_set)
    reordered_texture_set["material_zone_ids"].reverse()
    reordered_texture_set["maps"].reverse()
    reordered_texture_set["extensions"].reverse()
    reordered_texture_set.pop("surface_adornment")
    reordered_texture_set.pop("surface_adornment_sha256")
    assert_visual_texture_sets_match([texture_set], [reordered_texture_set])
    changed_texture_set = copy.deepcopy(texture_set)
    changed_texture_set["maps"][0]["sha256"] = "f" * 64
    expect_failure(
        "C106_GLB_FACT_DRIFT",
        lambda: assert_visual_texture_sets_match([texture_set], [changed_texture_set]),
    )
    readback = {
        "glb_sha256": "f" * 64,
        "artifact_profile": {"artifact_profile_id": "production_concept"},
    }
    run_negative_probes(candidate, readback, "f" * 64)
    assert artifact_directory_for_name("c106_artifact_self_test") == ROOT / "output" / "c106_artifact_self_test"
    expect_failure(
        "C106_ARTIFACT_DIRECTORY_INVALID",
        lambda: artifact_directory_for_name("../outside-output"),
    )
    print(json.dumps({"schema_version": REPORT_SCHEMA, "status": "pass", "mode": "self_test", "formal_eligible": False, "measured_provider_calls": 0}, ensure_ascii=False, sort_keys=True))
    return 0


def main(*, self_test_only: bool = False, artifact_directory_name: str | None = None) -> int:
    if self_test_only:
        return self_test()
    if not MANIFEST.is_file() or not RUST_WRAPPER.is_file():
        fail("C106_RUST_INPUT_MISSING")
    from forgecad_agent.application.restricted_geometry_executor import (
        RestrictedGeometryExecutor,
    )

    trace = ProductionExecutionTrace()
    dump = trace.execute("rust_recipe_dump", "rust_core", run_rust_dump)
    candidates = list_value(dump.get("candidates"), "C106_CANDIDATES_INVALID")
    if len(candidates) != 3:
        fail("C106_ROOT_COUNT_INVALID", str(len(candidates)))
    raw_seals = list_value(
        dump.get("shape_program_seals"), "C106_SHAPE_PROGRAM_SEALS_INVALID"
    )
    seals_by_recipe = {
        seal.get("recipe_id"): seal
        for value in raw_seals
        if isinstance(value, Mapping)
        for seal in [mapping(value, "C106_SHAPE_PROGRAM_SEAL_INVALID")]
        if isinstance(seal.get("recipe_id"), str)
    }
    if len(raw_seals) != 3 or set(seals_by_recipe) != ROOT_IDS:
        fail("C106_SHAPE_PROGRAM_SEALS_INVALID")
    reports: list[dict[str, Any]] = []
    root_ids: set[str] = set()
    semantic_hashes: set[str] = set()
    candidate_hashes: set[str] = set()
    glb_hashes: set[str] = set()
    first_candidate: Mapping[str, Any] | None = None
    first_readback: Mapping[str, Any] | None = None
    first_glb_sha256: str | None = None
    compiled: dict[str, dict[str, Any]] = {}
    binding_inputs: list[dict[str, Any]] = []
    executor = RestrictedGeometryExecutor(environment={})
    for index, raw_candidate in enumerate(candidates):
        candidate = mapping(raw_candidate, "C106_CANDIDATE_INVALID")
        recipe_id, graph, _ = assert_candidate(candidate)
        root_ids.add(recipe_id)
        candidate_hash = candidate.get("candidate_sha256")
        if not isinstance(candidate_hash, str) or len(candidate_hash) != 64:
            fail("C106_CANDIDATE_HASH_INVALID", recipe_id)
        candidate_hashes.add(candidate_hash)
        seal = seals_by_recipe.get(recipe_id)
        if seal is None:
            fail("C106_SHAPE_PROGRAM_SEAL_INVALID", recipe_id)
        sealed_sha256 = seal.get("shape_program_sha256")
        if not isinstance(sealed_sha256, str):
            fail("C106_SHAPE_PROGRAM_SEAL_INVALID", recipe_id)
        semantic_hashes.add(sealed_sha256)
        report, readback, glb, spatial_checks = compile_candidate(
            index, candidate, graph, seal, executor, trace
        )
        if report["glb_sha256"] in glb_hashes:
            fail("C106_GLB_CLONE_INVALID", recipe_id)
        glb_hashes.add(report["glb_sha256"])
        compiled[recipe_id] = {
            "report": report,
            "readback": readback,
            "glb": glb,
            "spatial_checks": spatial_checks,
        }
        binding_inputs.append(
            {
                "recipe_id": recipe_id,
                "candidate": candidate,
                # Preserve the exact cross-language semantic hash scope. The
                # Rust binder hashes these bytes before typed deserialization,
                # then also requires the parsed Value to equal `candidate`.
                "candidate_canonical_json": json.dumps(
                    {**candidate, "candidate_sha256": ""},
                    ensure_ascii=False,
                    sort_keys=True,
                    separators=(",", ":"),
                    allow_nan=False,
                ),
                "shape_program_canonical_json": seal["shape_program_canonical_json"],
                "shape_program_sha256": sealed_sha256,
                "production_glb_base64": base64.b64encode(glb).decode("ascii"),
                "readback": readback,
            }
        )
        if first_candidate is None:
            first_candidate = candidate
            first_readback = readback
            first_glb_sha256 = report["glb_sha256"]
    assert_distinct_artifacts(root_ids, candidate_hashes, semantic_hashes, glb_hashes)
    bindings = trace.execute(
        "rust_product_state_bind",
        "rust_core",
        lambda: bind_product_state(binding_inputs),
    )
    provider_measurement = trace.measurement()
    version_ids: set[str] = set()
    quality_ids: set[str] = set()
    for recipe_id in sorted(ROOT_IDS):
        result = compiled[recipe_id]
        report = result["report"]
        readback = result["readback"]
        binding = bindings[recipe_id]
        # Rust Core owns the canonical product-state hash. Its number
        # normalization is intentionally authoritative over Python's JSON
        # encoder for values such as integral floats.
        readback_sha256 = binding.get("readback_sha256")
        if (
            not isinstance(readback_sha256, str)
            or len(readback_sha256) != 64
            or any(character not in "0123456789abcdef" for character in readback_sha256)
        ):
            fail("C106_PRODUCT_STATE_READBACK_HASH_INVALID", recipe_id)
        assert_product_state_binding(
            binding=binding,
            recipe_id=recipe_id,
            candidate_sha256=report["candidate_sha256"],
            glb_sha256=report["glb_sha256"],
            readback_sha256=readback_sha256,
        )
        version_ids.add(str(binding["asset_version_id"]))
        quality_ids.add(str(binding["quality_report_id"]))
        report["product_state_binding"] = dict(binding)
        report["gate_evidence"] = independent_gate_evidence(
            recipe_id=recipe_id,
            candidate_sha256=report["candidate_sha256"],
            glb_sha256=report["glb_sha256"],
            readback_sha256=readback_sha256,
            readback=readback,
            binding=binding,
            spatial_checks=result["spatial_checks"],
            provider_measurement=provider_measurement,
        )
        reports.append(report)
    if len(version_ids) != 3 or len(quality_ids) != 3:
        fail("C106_PRODUCT_STATE_IDENTITY_REUSED")
    if first_candidate is None or first_readback is None or first_glb_sha256 is None:
        fail("C106_NEGATIVE_FIXTURE_MISSING")
    run_negative_probes(first_candidate, first_readback, first_glb_sha256)
    artifact_directory = None
    if artifact_directory_name is not None:
        artifact_directory = write_production_artifacts(
            artifact_directory_name,
            reports,
            compiled,
        )
    print(
        json.dumps(
            {
                "schema_version": REPORT_SCHEMA,
                "status": "pass",
                "formal_eligible": False,
                "human_benchmark_evidence": False,
                "measured_provider_calls": provider_measurement["measured_provider_calls"],
                "provider_measurement": provider_measurement,
                "roots": reports,
                "artifact_directory": (
                    str(artifact_directory.relative_to(ROOT))
                    if artifact_directory is not None
                    else None
                ),
            },
            ensure_ascii=False,
            sort_keys=True,
        ),
        flush=True,
    )
    return 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Run the C106 robotic-arm automated production gate.")
    parser.add_argument("--self-test", action="store_true", help="test the gate's negative checks without Rust or geometry execution")
    parser.add_argument(
        "--artifact-dir",
        help="optional safe output basename for three verified production GLBs plus readback-summary.json",
    )
    arguments = parser.parse_args()
    try:
        raise SystemExit(
            main(
                self_test_only=arguments.self_test,
                artifact_directory_name=arguments.artifact_dir,
            )
        )
    except GateFailure as error:
        print(json.dumps({"schema_version": REPORT_SCHEMA, "status": "fail", "error": str(error), "formal_eligible": False}, ensure_ascii=False, sort_keys=True), file=sys.stderr)
        raise SystemExit(1) from error
