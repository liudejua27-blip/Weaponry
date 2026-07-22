#!/usr/bin/env python3
"""Development-only structural coherence gate for M108B Recipe fixtures.

This is deliberately not a visual score, formal benchmark kit, or substitute
for independent human review.  It compiles the real Rust-expanded candidates
through the restricted geometry executor and inspects the resulting GLB plus
GeometryCompileReadback facts.  A passing report always remains
``formal_eligible=false``.
"""
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import math
import struct
import sys
from collections import defaultdict
from pathlib import Path
from typing import Any, Mapping

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "apps/agent"))
sys.path.insert(0, str(ROOT / "scripts"))

from forgecad_agent.application.restricted_geometry_executor import (  # noqa: E402
    RestrictedGeometryExecutionRequest,
    RestrictedGeometryExecutor,
)
from package_component_recipes import canonical_sha256  # noqa: E402
from prepare_m108b_asset_preflight import (  # noqa: E402
    DOMAINS,
    ROOT_IDS,
    _assert_readback_matches_glb,
    _run_rust_expansion,
)


REGISTRY_PATH = ROOT / "packages/concept-spec/fixtures/production-component-recipe-registry.json"
ROOT_COUNT = 12
ROOT_SLOT_COUNT = 75


class CoherenceError(ValueError):
    """A real expanded/compiled structural fact is missing or incoherent."""


def fail(code: str, detail: str = "") -> None:
    raise CoherenceError(f"{code}:{detail}" if detail else code)


def _load_registry() -> dict[str, Any]:
    try:
        value = json.loads(REGISTRY_PATH.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise CoherenceError("M108B_STRUCTURAL_REGISTRY_INVALID") from error
    if not isinstance(value, dict) or value.get("schema_version") != "EditableComponentRecipeRegistry@1":
        fail("M108B_STRUCTURAL_REGISTRY_INVALID")
    if not isinstance(value.get("recipes"), list):
        fail("M108B_STRUCTURAL_REGISTRY_INVALID")
    return value


def _vec3(value: object, code: str) -> tuple[float, float, float]:
    if not isinstance(value, list) or len(value) != 3:
        fail(code)
    try:
        result = tuple(float(item) for item in value)
    except (TypeError, ValueError) as error:
        raise CoherenceError(code) from error
    if not all(math.isfinite(item) for item in result):
        fail(code)
    return result  # type: ignore[return-value]


def _distance(a: tuple[float, float, float], b: tuple[float, float, float]) -> float:
    return math.sqrt(sum((left - right) ** 2 for left, right in zip(a, b)))


def _valid_frame(connector: Mapping[str, Any], code: str) -> None:
    normal = _vec3(connector.get("normal"), code)
    up = _vec3(connector.get("up"), code)
    if not 0.999 <= math.sqrt(sum(item * item for item in normal)) <= 1.001:
        fail(code)
    if not 0.999 <= math.sqrt(sum(item * item for item in up)) <= 1.001:
        fail(code)
    if abs(sum(left * right for left, right in zip(normal, up))) > 1e-5:
        fail(code)


def _assert_zero_local_transform(slot: Mapping[str, Any]) -> None:
    transform = slot.get("parent_local_transform")
    position = _vec3(transform.get("position"), "M108B_STRUCTURAL_LOCAL_OFFSET_INVALID") if isinstance(transform, dict) else fail("M108B_STRUCTURAL_LOCAL_OFFSET_INVALID")
    if any(abs(item) > 1e-7 for item in position):
        fail("M108B_STRUCTURAL_LOCAL_OFFSET_INVALID", str(slot.get("slot_id")))


Matrix4 = tuple[tuple[float, float, float, float], tuple[float, float, float, float], tuple[float, float, float, float], tuple[float, float, float, float]]


def _identity_matrix() -> Matrix4:
    return (
        (1.0, 0.0, 0.0, 0.0),
        (0.0, 1.0, 0.0, 0.0),
        (0.0, 0.0, 1.0, 0.0),
        (0.0, 0.0, 0.0, 1.0),
    )


def _matrix_multiply(left: Matrix4, right: Matrix4) -> Matrix4:
    return tuple(
        tuple(sum(left[row][index] * right[index][column] for index in range(4)) for column in range(4))
        for row in range(4)
    )  # type: ignore[return-value]


def _transform_matrix(transform: Mapping[str, Any], code: str) -> Matrix4:
    position = _vec3(transform.get("position"), code)
    rotation = _vec3(transform.get("rotation"), code)
    scale = _vec3(transform.get("scale"), code)
    # M108B Recipe mesh baking is rigid-only.  Mirroring Rust's explicit
    # restriction here keeps a malformed AssemblyGraph from looking aligned
    # merely because its translation happens to match.
    if any(abs(value - 1.0) > 1e-7 for value in scale):
        fail(code)
    rx, ry, rz = rotation
    sx, cx = math.sin(rx), math.cos(rx)
    sy, cy = math.sin(ry), math.cos(ry)
    sz, cz = math.sin(rz), math.cos(rz)
    # Must stay byte-for-byte conceptually aligned with Rust transform.rs:
    # column vectors, world = parent * local, Euler XYZ (Rz * Ry * Rx).
    return (
        (cy * cz, cz * sx * sy - cx * sz, sx * sz + cx * cz * sy, position[0]),
        (cy * sz, cx * cz + sx * sy * sz, cx * sy * sz - cz * sx, position[1]),
        (-sy, cy * sx, cx * cy, position[2]),
        (0.0, 0.0, 0.0, 1.0),
    )


def _connector_matrix(connector: Mapping[str, Any], code: str) -> Matrix4:
    _valid_frame(connector, code)
    position = _vec3(connector.get("position"), code)
    normal = _vec3(connector.get("normal"), code)
    up = _vec3(connector.get("up"), code)
    right = (
        up[1] * normal[2] - up[2] * normal[1],
        up[2] * normal[0] - up[0] * normal[2],
        up[0] * normal[1] - up[1] * normal[0],
    )
    return (
        (right[0], up[0], normal[0], position[0]),
        (right[1], up[1], normal[1], position[1]),
        (right[2], up[2], normal[2], position[2]),
        (0.0, 0.0, 0.0, 1.0),
    )


def _inverse_rigid(matrix: Matrix4) -> Matrix4:
    inverse = [list(row) for row in _identity_matrix()]
    for row in range(3):
        for column in range(3):
            inverse[row][column] = matrix[column][row]
        inverse[row][3] = -sum(inverse[row][column] * matrix[column][3] for column in range(3))
    return tuple(tuple(row) for row in inverse)  # type: ignore[return-value]


def _assert_matrix_close(actual: Matrix4, expected: Matrix4, code: str, detail: str) -> None:
    # Rust writes Euler angles after an inverse decomposition, so compare the
    # reconstructed rigid matrices instead of raw Euler triples (gimbal lock
    # has multiple equivalent triples).
    if any(abs(actual[row][column] - expected[row][column]) > 1e-4 for row in range(4) for column in range(4)):
        fail(code, detail)


def _assert_exact_roots(seen_roots: set[str], domain_counts: Mapping[str, int]) -> None:
    if seen_roots != ROOT_IDS:
        fail("M108B_STRUCTURAL_ROOT_SET_INVALID")
    if set(domain_counts) != set(DOMAINS) or any(domain_counts.get(domain) != 3 for domain in DOMAINS):
        fail("M108B_STRUCTURAL_DOMAIN_COUNT_INVALID")


def _assert_distinct_domain_fingerprints(fingerprints: Mapping[str, list[str]]) -> None:
    if len(fingerprints) != 4 or any(len(values) != 3 or len(set(values)) != 3 for values in fingerprints.values()):
        fail("M108B_STRUCTURAL_DOMAIN_FINGERPRINT_CLONE")


def _parse_glb_json(glb: bytes) -> dict[str, Any]:
    if len(glb) < 20 or glb[:4] != b"glTF":
        fail("M108B_STRUCTURAL_GLB_INVALID")
    version, total = struct.unpack_from("<II", glb, 4)
    if version != 2 or total != len(glb):
        fail("M108B_STRUCTURAL_GLB_INVALID")
    chunk_length, chunk_type = struct.unpack_from("<I4s", glb, 12)
    if chunk_type != b"JSON" or 20 + chunk_length > len(glb):
        fail("M108B_STRUCTURAL_GLB_INVALID")
    try:
        value = json.loads(glb[20 : 20 + chunk_length].decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CoherenceError("M108B_STRUCTURAL_GLB_INVALID") from error
    if not isinstance(value, dict):
        fail("M108B_STRUCTURAL_GLB_INVALID")
    return value


PrimitiveBounds = tuple[tuple[float, float, float], tuple[float, float, float], str, tuple[str, ...]]


def _primitive_bounds(glb_json: Mapping[str, Any]) -> dict[str, PrimitiveBounds]:
    accessors = glb_json.get("accessors")
    meshes = glb_json.get("meshes")
    if not isinstance(accessors, list) or not isinstance(meshes, list) or len(meshes) != 1:
        fail("M108B_STRUCTURAL_GLB_PRIMITIVES_INVALID")
    primitives = meshes[0].get("primitives") if isinstance(meshes[0], dict) else None
    if not isinstance(primitives, list) or not primitives:
        fail("M108B_STRUCTURAL_GLB_PRIMITIVES_INVALID")
    result: dict[str, PrimitiveBounds] = {}
    for primitive in primitives:
        if not isinstance(primitive, dict):
            fail("M108B_STRUCTURAL_GLB_PRIMITIVES_INVALID")
        extras = primitive.get("extras")
        attributes = primitive.get("attributes")
        if not isinstance(extras, dict) or not isinstance(attributes, dict):
            fail("M108B_STRUCTURAL_GLB_PRIMITIVES_INVALID")
        primitive_id = extras.get("forgecad_primitive_id")
        feature_node = extras.get("forgecad_feature_node_id")
        position_accessor = attributes.get("POSITION")
        if not isinstance(primitive_id, str) or not isinstance(feature_node, str) or not isinstance(position_accessor, int):
            fail("M108B_STRUCTURAL_GLB_PRIMITIVES_INVALID")
        if position_accessor < 0 or position_accessor >= len(accessors) or not isinstance(accessors[position_accessor], dict):
            fail("M108B_STRUCTURAL_GLB_PRIMITIVES_INVALID")
        accessor = accessors[position_accessor]
        lower = _vec3(accessor.get("min"), "M108B_STRUCTURAL_GLB_BOUNDS_INVALID")
        upper = _vec3(accessor.get("max"), "M108B_STRUCTURAL_GLB_BOUNDS_INVALID")
        if any(low > high for low, high in zip(lower, upper)) or primitive_id in result:
            fail("M108B_STRUCTURAL_GLB_BOUNDS_INVALID")
        csg_provenance = extras.get("forgecad_csg_provenance")
        if csg_provenance is None:
            source_operations = (feature_node,)
        elif (
            isinstance(csg_provenance, dict)
            and isinstance(csg_provenance.get("source_operation_ids"), list)
            and csg_provenance["source_operation_ids"]
            and all(isinstance(item, str) and item.startswith("op_") for item in csg_provenance["source_operation_ids"])
        ):
            source_operations = tuple(csg_provenance["source_operation_ids"])
        else:
            fail("M108B_STRUCTURAL_GLB_PROVENANCE_INVALID", primitive_id)
        # glTF POSITION is metres while GeometryCompileReadback uses millimetres.
        # One feature can legally emit many primitives (array/mirror/CSG), so
        # `primitive_id` is the unique identity and feature node is provenance.
        result[primitive_id] = (tuple(item * 1000.0 for item in lower), tuple(item * 1000.0 for item in upper), feature_node, source_operations)
    return result


def _assert_primitive_provenance(
    primitives: Mapping[str, PrimitiveBounds],
    surfaces: object,
    feature_history: object,
) -> None:
    """Bind every GLB primitive to one readback surface and immutable history.

    Feature nodes are deliberately many-to-one with primitives for operations
    such as mirror/array/radial array.  The invariant is therefore a bijection
    on *primitive IDs*, plus exact per-primitive feature/source-operation facts,
    not a false one-feature/one-primitive requirement.
    """
    if not isinstance(surfaces, list) or not isinstance(feature_history, list):
        fail("M108B_STRUCTURAL_SURFACE_FACTS_INVALID")
    history_ids = [item.get("node_id") for item in feature_history if isinstance(item, dict)]
    if len(history_ids) != len(feature_history) or not history_ids or any(not isinstance(item, str) for item in history_ids) or len(set(history_ids)) != len(history_ids):
        fail("M108B_STRUCTURAL_FEATURE_HISTORY_INVALID")
    history_set = set(history_ids)
    by_primitive: dict[str, Mapping[str, Any]] = {}
    for surface in surfaces:
        if not isinstance(surface, dict) or not isinstance(surface.get("primitive_id"), str):
            fail("M108B_STRUCTURAL_SURFACE_FACTS_INVALID")
        primitive_id = surface["primitive_id"]
        if primitive_id in by_primitive:
            fail("M108B_STRUCTURAL_SURFACE_PROVENANCE_DUPLICATE", primitive_id)
        by_primitive[primitive_id] = surface
    if set(by_primitive) != set(primitives) or len(by_primitive) != len(primitives):
        fail("M108B_STRUCTURAL_GLB_READBACK_DRIFT")
    for primitive_id, (_, _, feature_node, source_operations) in primitives.items():
        surface = by_primitive[primitive_id]
        sources = surface.get("source_operation_ids")
        if (
            surface.get("feature_node_id") != feature_node
            or not isinstance(sources, list)
            or tuple(sources) != source_operations
            or feature_node not in history_set
            or any(source not in history_set for source in source_operations)
        ):
            fail("M108B_STRUCTURAL_GLB_PROVENANCE_DRIFT", primitive_id)


def _aabb_distance(left: tuple[tuple[float, float, float], tuple[float, float, float]], right: tuple[tuple[float, float, float], tuple[float, float, float]]) -> float:
    return math.sqrt(sum(max(0.0, left[0][index] - right[1][index], right[0][index] - left[1][index]) ** 2 for index in range(3)))


def _aabb_diag(bounds: tuple[tuple[float, float, float], tuple[float, float, float]]) -> float:
    return _distance(bounds[0], bounds[1])


def _normalized_structure(value: object, scale: float, key: str = "") -> object:
    """Retain actual shape relationships while removing generated identifiers."""
    ignored = {
        "operation_id", "output_id", "program_id", "input_id", "profile_input_id",
        "section_set_id", "section_id", "sketch_id", "profile_sketch_id", "source_ref", "feature_id", "primitive_id",
        "recipe_id", "recipe_sha256", "candidate_id", "changeset_id", "project_id",
        "material_id", "zone_id", "material_zone_id", "registry_sha256",
    }
    if key in ignored or key == "provenance":
        return None
    if isinstance(value, dict):
        return {name: _normalized_structure(item, scale, name) for name, item in sorted(value.items()) if name not in ignored and name != "provenance"}
    if isinstance(value, list):
        if key == "inputs":
            return ["operation_input" for _ in value]
        return [_normalized_structure(item, scale, key) for item in value]
    if isinstance(value, bool) or value is None:
        return value
    if isinstance(value, (int, float)):
        return round(float(value) / scale, 5)
    return value


def _fingerprint(candidate: Mapping[str, Any], bounds_mm: list[float]) -> str:
    program = candidate.get("expanded_shape_program")
    if not isinstance(program, dict) or not isinstance(program.get("operations"), list):
        fail("M108B_STRUCTURAL_PROGRAM_INVALID")
    scale = max(float(item) for item in bounds_mm)
    if not math.isfinite(scale) or scale <= 1.0:
        fail("M108B_STRUCTURAL_BOUNDS_INVALID")
    raw_operations = [operation for operation in program["operations"] if isinstance(operation, dict)]
    # IDs are generated independently for every candidate, but the dependency
    # graph is structural truth.  The old normalizer retained only input count,
    # so a boolean/sweep wired to a different predecessor could be reported as
    # the same candidate.  Canonical operation ordinals remove generated names
    # while retaining the actual graph edges.
    operation_ordinals = {
        operation_id: index
        for index, operation in enumerate(raw_operations)
        if isinstance((operation_id := operation.get("operation_id")), str)
    }
    operations = []
    for operation in raw_operations:
        inputs = operation.get("inputs")
        if not isinstance(inputs, list):
            fail("M108B_STRUCTURAL_PROGRAM_INVALID")
        references = [
            f"operation_ref_{operation_ordinals[input_id]}" if isinstance(input_id, str) and input_id in operation_ordinals else "unresolved_operation_ref"
            for input_id in inputs
        ]
        operations.append(_normalized_structure({"op": operation.get("op"), "args": operation.get("args"), "operation_refs": references}, scale))
    return hashlib.sha256(json.dumps(operations, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8")).hexdigest()


def _assert_root_slots(
    candidate: Mapping[str, Any],
    recipe: Mapping[str, Any],
    recipes: Mapping[str, Mapping[str, Any]],
) -> int:
    graph = candidate.get("expanded_assembly_graph")
    instances = candidate.get("component_recipe_instances")
    slots = recipe.get("child_slots")
    if not isinstance(graph, dict) or not isinstance(instances, list) or not isinstance(slots, list):
        fail("M108B_STRUCTURAL_EXPANSION_INVALID")
    root = next((item for item in instances if isinstance(item, dict) and item.get("instance_path") == "root"), None)
    parts = graph.get("parts")
    if not isinstance(root, dict) or not isinstance(parts, list):
        fail("M108B_STRUCTURAL_EXPANSION_INVALID")
    root_instance_id = root.get("instance_id")
    parts_by_instance: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for part in parts:
        if isinstance(part, dict) and isinstance(part.get("recipe_instance_id"), str):
            parts_by_instance[part["recipe_instance_id"]].append(part)
    if not isinstance(root_instance_id, str) or not parts_by_instance.get(root_instance_id):
        fail("M108B_STRUCTURAL_ROOT_PART_MISSING")
    checked = 0
    for slot in slots:
        if not isinstance(slot, dict) or slot.get("required") is not True:
            continue
        _assert_zero_local_transform(slot)
        parent_connector_id = slot.get("parent_connector_id")
        child_connector_id = slot.get("child_connector_id")
        parent_parts = [part for part in parts_by_instance[root_instance_id] if any(connector.get("connector_id") == parent_connector_id for connector in part.get("connectors", []) if isinstance(connector, dict))]
        if len(parent_parts) != 1:
            fail("M108B_STRUCTURAL_PARENT_MOUNT_MISSING", str(slot.get("slot_id")))
        parent = parent_parts[0]
        parent_connector = next(connector for connector in parent["connectors"] if connector.get("connector_id") == parent_connector_id)
        _valid_frame(parent_connector, "M108B_STRUCTURAL_PARENT_FRAME_INVALID")
        children = [item for item in instances if isinstance(item, dict) and item.get("parent_instance_id") == root_instance_id and item.get("parent_slot_id") == slot.get("slot_id")]
        if len(children) != 1 or not isinstance(children[0].get("instance_id"), str):
            fail("M108B_STRUCTURAL_CHILD_INSTANCE_INVALID", str(slot.get("slot_id")))
        child_parts = parts_by_instance.get(children[0]["instance_id"], [])
        if len(child_parts) != 1:
            fail("M108B_STRUCTURAL_CHILD_PART_INVALID", str(slot.get("slot_id")))
        child = child_parts[0]
        child_connector = next((connector for connector in child.get("connectors", []) if isinstance(connector, dict) and connector.get("connector_id") == child_connector_id), None)
        if not isinstance(child_connector, dict):
            fail("M108B_STRUCTURAL_CHILD_MOUNT_MISSING", str(slot.get("slot_id")))
        _valid_frame(child_connector, "M108B_STRUCTURAL_CHILD_FRAME_INVALID")
        child_recipe_ref = children[0].get("recipe")
        child_recipe_id = child_recipe_ref.get("recipe_id") if isinstance(child_recipe_ref, dict) else None
        child_recipe = recipes.get(child_recipe_id) if isinstance(child_recipe_id, str) else None
        if child_recipe is None or not isinstance(child_recipe.get("root_local_transform"), dict):
            fail("M108B_STRUCTURAL_CHILD_RECIPE_MISSING", str(slot.get("slot_id")))
        if not isinstance(parent.get("transform"), dict) or not isinstance(child.get("transform"), dict) or not isinstance(slot.get("parent_local_transform"), dict):
            fail("M108B_STRUCTURAL_CHILD_TRANSFORM_INVALID", str(slot.get("slot_id")))
        # Recompute the exact Rust expansion formula, including orientation:
        # parent world × parent connector × slot local × inverse(child
        # connector) × child root local.  Comparing only translation misses a
        # flipped normal or roll error that makes a visually detached part.
        expected_child = _matrix_multiply(
            _matrix_multiply(
                _matrix_multiply(
                    _matrix_multiply(
                        _transform_matrix(parent["transform"], "M108B_STRUCTURAL_PARENT_TRANSFORM_INVALID"),
                        _connector_matrix(parent_connector, "M108B_STRUCTURAL_PARENT_FRAME_INVALID"),
                    ),
                    _transform_matrix(slot["parent_local_transform"], "M108B_STRUCTURAL_LOCAL_OFFSET_INVALID"),
                ),
                _inverse_rigid(_connector_matrix(child_connector, "M108B_STRUCTURAL_CHILD_FRAME_INVALID")),
            ),
            _transform_matrix(child_recipe["root_local_transform"], "M108B_STRUCTURAL_CHILD_TRANSFORM_INVALID"),
        )
        actual_child = _transform_matrix(child["transform"], "M108B_STRUCTURAL_CHILD_TRANSFORM_INVALID")
        _assert_matrix_close(actual_child, expected_child, "M108B_STRUCTURAL_MOUNT_FRAME_DRIFT", str(slot.get("slot_id")))
        checked += 1
    return checked


def _assert_compiled_facts(candidate: Mapping[str, Any], result: Any) -> dict[str, Any]:
    graph = candidate.get("expanded_assembly_graph")
    if not isinstance(graph, dict) or not isinstance(graph.get("parts"), list) or not result.glb_base64 or not isinstance(result.readback, dict):
        fail("M108B_STRUCTURAL_COMPILE_FACTS_INVALID")
    glb = base64.b64decode(result.glb_base64)
    if hashlib.sha256(glb).hexdigest() != result.glb_sha256:
        fail("M108B_STRUCTURAL_GLB_HASH_INVALID")
    readback = _assert_readback_matches_glb(glb=glb, readback=result.readback, expected_glb_sha256=result.glb_sha256)
    bounds_mm = readback.get("bounds_mm")
    if not isinstance(bounds_mm, list) or len(bounds_mm) != 3 or any(not isinstance(item, (int, float)) or item <= 1 for item in bounds_mm):
        fail("M108B_STRUCTURAL_BOUNDS_INVALID")
    glb_primitives = _primitive_bounds(_parse_glb_json(glb))
    surfaces = readback.get("surface_provenance")
    if not isinstance(surfaces, list) or len(surfaces) != len(glb_primitives):
        fail("M108B_STRUCTURAL_SURFACE_FACTS_INVALID")
    _assert_primitive_provenance(glb_primitives, surfaces, readback.get("feature_history"))
    primitive_ops = {feature for _, (_, _, feature, _) in glb_primitives.items()}
    parts = graph["parts"]
    for part in parts:
        if not isinstance(part, dict) or not isinstance(part.get("operation_id"), str) or part["operation_id"] not in primitive_ops:
            fail("M108B_STRUCTURAL_PART_PRIMITIVE_MISSING", str(part.get("part_id") if isinstance(part, dict) else "unknown"))
    mins = [min(item[0][axis] for item in glb_primitives.values()) for axis in range(3)]
    maxes = [max(item[1][axis] for item in glb_primitives.values()) for axis in range(3)]
    glb_bounds = [maxes[index] - mins[index] for index in range(3)]
    if any(abs(glb_bounds[index] - float(bounds_mm[index])) > max(2.0, float(bounds_mm[index]) * 0.01) for index in range(3)):
        fail("M108B_STRUCTURAL_WHOLE_BOUNDS_DRIFT")
    whole_diag = math.sqrt(sum(float(item) ** 2 for item in bounds_mm))
    for primitive_id, (lower, upper, _, _) in glb_primitives.items():
        if _aabb_diag((lower, upper)) <= max(2.0, whole_diag * 0.001) or _aabb_diag((lower, upper)) > whole_diag * 1.03:
            fail("M108B_STRUCTURAL_PART_BOUNDS_OUTLIER", primitive_id)
    parts_by_id = {part.get("part_id"): part for part in parts if isinstance(part, dict) and isinstance(part.get("part_id"), str)}
    if len(parts_by_id) != len(parts):
        fail("M108B_STRUCTURAL_PART_ID_INVALID")
    # AssemblyGraph records one durable Part for each Recipe instance, while
    # one Recipe instance may emit several GLB primitives. Rust preserves the
    # instance hash in every baked operation ID (`op_<instance-suffix>_...`),
    # so aggregate those primitives before checking parent/child proximity.
    # Using only the parent's first output would compare an arm head accent to
    # the root Recipe's base primitive instead of the complete root component.
    part_bounds_by_id: dict[str, tuple[tuple[float, float, float], tuple[float, float, float]]] = {}
    for part_id in parts_by_id:
        suffix = part_id.removeprefix("part_")
        matches = [
            (lower, upper)
            for lower, upper, feature_node, _ in glb_primitives.values()
            if feature_node.startswith(f"op_{suffix}_")
        ]
        if not matches:
            fail("M108B_STRUCTURAL_PART_BOUNDS_MISSING", part_id)
        lower = tuple(min(bounds[0][axis] for bounds in matches) for axis in range(3))
        upper = tuple(max(bounds[1][axis] for bounds in matches) for axis in range(3))
        part_bounds_by_id[part_id] = (lower, upper)
    for part in parts:
        if not isinstance(part, dict) or not isinstance(part.get("parent_part_id"), str):
            continue
        parent = parts_by_id.get(part.get("parent_part_id"))
        child_bounds = part_bounds_by_id.get(part.get("part_id"))
        parent_bounds = part_bounds_by_id.get(parent.get("part_id")) if isinstance(parent, dict) else None
        if child_bounds is None or parent_bounds is None:
            fail("M108B_STRUCTURAL_PART_BOUNDS_MISSING")
        mount_budget = max(24.0, _aabb_diag(parent_bounds) * 0.12)
        if _aabb_distance(parent_bounds, child_bounds) > mount_budget:
            fail("M108B_STRUCTURAL_MOUNT_BOUNDS_OUTLIER", str(part.get("part_id")))
    return {"bounds_mm": [round(float(item), 3) for item in bounds_mm], "primitive_count": len(glb_primitives)}


def run_gate() -> dict[str, Any]:
    registry = _load_registry()
    fixture = _run_rust_expansion()
    registry_sha256 = canonical_sha256(registry)
    if fixture.get("registry_sha256") != registry_sha256:
        fail("M108B_STRUCTURAL_REGISTRY_DRIFT")
    recipes = {recipe.get("recipe_id"): recipe for recipe in registry["recipes"] if isinstance(recipe, dict) and isinstance(recipe.get("recipe_id"), str)}
    candidates = fixture.get("candidates")
    if not isinstance(candidates, list) or len(candidates) != ROOT_COUNT:
        fail("M108B_STRUCTURAL_ROOT_COUNT_INVALID")
    executor = RestrictedGeometryExecutor(environment={})
    slots = 0
    fingerprints: dict[str, list[str]] = defaultdict(list)
    seen_roots: set[str] = set()
    domain_counts: dict[str, int] = defaultdict(int)
    reports: list[dict[str, Any]] = []
    for index, candidate in enumerate(candidates):
        if not isinstance(candidate, dict):
            fail("M108B_STRUCTURAL_CANDIDATE_INVALID")
        recipe_ref = candidate.get("recipe")
        recipe_id = recipe_ref.get("recipe_id") if isinstance(recipe_ref, dict) else None
        recipe = recipes.get(recipe_id)
        if recipe is None:
            fail("M108B_STRUCTURAL_RECIPE_MISSING", str(recipe_id))
        if recipe_id not in ROOT_IDS or recipe_id in seen_roots:
            fail("M108B_STRUCTURAL_ROOT_SET_INVALID", str(recipe_id))
        seen_roots.add(recipe_id)
        slots += _assert_root_slots(candidate, recipe, recipes)
        request = RestrictedGeometryExecutionRequest.model_validate({
            "schema_version": "RestrictedGeometryExecutionRequest@1", "protocol_version": "forgecad.restricted-geometry/1",
            "execution_id": f"m108b_structural_{index}", "idempotency_key": f"m108b_structural_idem_{index}",
            "cancellation_id": f"m108b_structural_cancel_{index}", "cancellation_token": f"m108b_structural_token_{index}",
            "action": "compile_readback", "timeout_ms": 120000, "artifact_profile_id": "production_concept",
            "shape_program": candidate["expanded_shape_program"],
        })
        result = executor.execute(request)
        facts = _assert_compiled_facts(candidate, result)
        domain = recipe.get("allowed_domains", [None])[0]
        instances = candidate.get("component_recipe_instances")
        root_instance = next((item for item in instances if isinstance(item, dict) and item.get("instance_path") == "root"), None) if isinstance(instances, list) else None
        if not isinstance(domain, str) or domain not in DOMAINS or not isinstance(root_instance, dict) or root_instance.get("domain_pack_id") != domain:
            fail("M108B_STRUCTURAL_DOMAIN_INVALID", str(recipe_id))
        domain_counts[domain] += 1
        fingerprint = _fingerprint(candidate, facts["bounds_mm"])
        fingerprints[domain].append(fingerprint)
        reports.append({"recipe_id": recipe_id, "domain_pack_id": domain, "fingerprint": fingerprint, **facts})
    if slots != ROOT_SLOT_COUNT:
        fail("M108B_STRUCTURAL_ROOT_SLOT_COUNT_INVALID", str(slots))
    _assert_exact_roots(seen_roots, domain_counts)
    _assert_distinct_domain_fingerprints(fingerprints)
    return {
        "schema_version": "M108BStructuralCoherence@1",
        "purpose": "development_structural_coherence_only",
        "status": "passed",
        "formal_eligible": False,
        "human_benchmark_evidence": False,
        "provider_calls": 0,
        "registry_sha256": registry_sha256,
        "root_count": ROOT_COUNT,
        "required_root_slot_count": slots,
        "fixtures": reports,
    }


def self_test() -> None:
    try:
        _valid_frame({"normal": [1, 0, 0], "up": [1, 0, 0]}, "SELF")
    except CoherenceError:
        pass
    else:
        raise AssertionError("invalid connector frame accepted")
    try:
        _assert_zero_local_transform({"slot_id": "bad", "parent_local_transform": {"position": [1, 0, 0]}})
    except CoherenceError:
        pass
    else:
        raise AssertionError("non-zero local slot offset accepted")
    try:
        _assert_matrix_close(
            _identity_matrix(),
            _transform_matrix({"position": [0, 0, 0], "rotation": [0, 0, 0.25], "scale": [1, 1, 1]}, "SELF"),
            "SELF_MOUNT_ROTATION_DRIFT",
            "slot",
        )
    except CoherenceError:
        pass
    else:
        raise AssertionError("rotated child mount accepted")
    try:
        _assert_exact_roots(set(ROOT_IDS) - {next(iter(ROOT_IDS))}, {domain: 3 for domain in DOMAINS})
    except CoherenceError:
        pass
    else:
        raise AssertionError("substituted root fixture set accepted")
    try:
        _assert_exact_roots(set(ROOT_IDS), {domain: 3 for domain in DOMAINS[:-1]})
    except CoherenceError:
        pass
    else:
        raise AssertionError("missing domain fixture count accepted")
    try:
        _aabb_distance(((0, 0, 0), (1, 1, 1)), ((4, 0, 0), (5, 1, 1))) <= 0.1 or (_ for _ in ()).throw(CoherenceError("SELF_OUTLIER"))
    except CoherenceError:
        pass
    else:
        raise AssertionError("detached child bounds accepted")
    left = hashlib.sha256(json.dumps(_normalized_structure({"op": "loft", "args": {"axis_length": 100}}, 100), sort_keys=True).encode()).hexdigest()
    right = hashlib.sha256(json.dumps(_normalized_structure({"op": "loft", "args": {"axis_length": 250}}, 100), sort_keys=True).encode()).hexdigest()
    if left == right:
        raise AssertionError("normalized structural fingerprint ignored shape facts")
    first_clone = _normalized_structure({"operation_id": "op_a", "inputs": ["op_source_a"], "args": {"profile_input_id": "profile_a", "axis_length": 100}}, 100)
    second_clone = _normalized_structure({"operation_id": "op_b", "inputs": ["op_source_b"], "args": {"profile_input_id": "profile_b", "axis_length": 100}}, 100)
    if first_clone != second_clone:
        raise AssertionError("generated identifiers leaked into structural fingerprint")
    candidate_left = {
        "expanded_shape_program": {"operations": [
            {"operation_id": "op_left_a", "op": "box", "args": {"size": [100, 100, 100]}, "inputs": []},
            {"operation_id": "op_left_b", "op": "box", "args": {"size": [80, 80, 80]}, "inputs": []},
            {"operation_id": "op_left_c", "op": "subtract", "args": {}, "inputs": ["op_left_a", "op_left_b"]},
        ]}
    }
    candidate_rewired = {
        "expanded_shape_program": {"operations": [
            {"operation_id": "op_right_a", "op": "box", "args": {"size": [100, 100, 100]}, "inputs": []},
            {"operation_id": "op_right_b", "op": "box", "args": {"size": [80, 80, 80]}, "inputs": []},
            {"operation_id": "op_right_c", "op": "subtract", "args": {}, "inputs": ["op_right_b", "op_right_a"]},
        ]}
    }
    if _fingerprint(candidate_left, [100, 100, 100]) == _fingerprint(candidate_rewired, [100, 100, 100]):
        raise AssertionError("rewired structural dependency accepted as a clone")
    repeated_feature_primitives: dict[str, PrimitiveBounds] = {
        "primitive_a": ((0, 0, 0), (1, 1, 1), "op_array", ("op_array",)),
        "primitive_b": ((2, 0, 0), (3, 1, 1), "op_array", ("op_array",)),
    }
    repeated_feature_surfaces = [
        {"primitive_id": "primitive_a", "feature_node_id": "op_array", "source_operation_ids": ["op_array"]},
        {"primitive_id": "primitive_b", "feature_node_id": "op_array", "source_operation_ids": ["op_array"]},
    ]
    _assert_primitive_provenance(repeated_feature_primitives, repeated_feature_surfaces, [{"node_id": "op_array"}])
    try:
        _assert_primitive_provenance(
            repeated_feature_primitives,
            [
                {"primitive_id": "primitive_a", "feature_node_id": "op_array", "source_operation_ids": ["op_array"]},
                {"primitive_id": "primitive_b", "feature_node_id": "op_array", "source_operation_ids": ["op_missing"]},
            ],
            [{"node_id": "op_array"}],
        )
    except CoherenceError:
        pass
    else:
        raise AssertionError("primitive source-operation provenance drift accepted")
    try:
        _assert_primitive_provenance(repeated_feature_primitives, repeated_feature_surfaces, [{"node_id": "op_other"}])
    except CoherenceError:
        pass
    else:
        raise AssertionError("primitive feature absent from immutable history accepted")
    try:
        _assert_distinct_domain_fingerprints({"a": ["same", "same", "other"], "b": ["1", "2", "3"], "c": ["1", "2", "3"], "d": ["1", "2", "3"]})
    except CoherenceError:
        pass
    else:
        raise AssertionError("cloned domain fingerprint accepted")
    print("M108B structural coherence self-test passed: formal_eligible=false, provider_calls=0")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return 0
    print(json.dumps(run_gate(), ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
