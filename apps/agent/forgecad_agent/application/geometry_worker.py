from __future__ import annotations

import hashlib
import json
import math
import struct
from dataclasses import dataclass, replace
from typing import Any, Callable, Dict, List, Mapping, Sequence, Tuple

from .geometry_models import GeometryCompileReadback
from .manifold_csg import (
    DEFAULT_CSG_TIMEOUT_SECONDS,
    MANIFOLD_KERNEL_ID,
    MANIFOLD_KERNEL_VERSION,
    ManifoldCsgError,
    execute_manifold_boolean,
)
from .mechanical_planner import MechanicalConceptPlan
from .shape_program import validate_shape_program
from .shape_program_runtime import (
    MANIFEST_SCHEMA_VERSION,
    UnsupportedRuntimeOperationError,
    assert_worker_executor_coverage,
)
from .surface_layer_pbr import (
    normalize_surface_layer_lowering,
    surface_layer_lowering_sha256,
    surface_layer_material_id,
    surface_layer_visual_texture_png_bytes,
    surface_layer_visual_texture_set,
)
from .visual_intent import visual_intent_for_direction
from .visual_texture_sets import (
    GeometryArtifactProfileId,
    builtin_material_properties,
    builtin_visual_material_binding,
    builtin_visual_material_count,
    builtin_visual_texture_set_for_material_index,
    builtin_visual_texture_set_for_readback,
    geometry_artifact_profile_manifest,
    normalize_surface_adornment_program,
    surface_adornment_material_id,
    surface_adornment_program_sha256,
    surface_adornment_visual_texture_png_bytes,
    surface_adornment_visual_texture_set,
    studio_environment_manifest,
    visual_texture_png_bytes,
)


@dataclass(frozen=True)
class BoxPrimitive:
    part_role: str
    center_mm: Tuple[float, float, float]
    size_mm: Tuple[float, float, float]
    material_index: int
    primitive_kind: str = "box"
    radius_mm: float = 0.0
    height_mm: float = 0.0
    axis: Tuple[float, float, float] = (0.0, 1.0, 0.0)
    wedge_slope: float = 1.0
    profile_points: Tuple[Tuple[float, float], ...] = ()
    profile_holes: Tuple[Tuple[Tuple[float, float], ...], ...] = ()
    profile_closed: bool = True
    profile_input_id: str | None = None
    revolve_angle: float = math.pi * 2
    cap_start: bool = True
    cap_end: bool = True
    radial_segments: int = 16
    capsule_hemisphere_segments: int = 5
    smooth_normals: bool = False
    loft_rings_mm: Tuple[Tuple[Tuple[float, float, float], ...], ...] = ()
    loft_profiles: Tuple[Tuple[Tuple[float, float], ...], ...] = ()
    loft_axis: str = "x"
    loft_cap_start_normal: Tuple[float, float, float] | None = None
    loft_cap_end_normal: Tuple[float, float, float] | None = None
    sweep_path_mm: Tuple[Tuple[float, float, float], ...] = ()
    path_closed: bool = False
    path_twist_degrees: float = 0.0
    bevel_radius_mm: float = 0.0
    bevel_segments: int = 1
    material_id: str | None = None
    material_zone_id: str | None = None
    source_operation_id: str | None = None
    profile_contract: Mapping[str, Any] | None = None
    loft_cross_section_scale: Tuple[float, float] = (0.0, 0.0)
    sweep_profile_scale: Tuple[float, float] = (0.0, 0.0)
    # ShapeProgram's existing bounded ``args.rotation`` is a static mesh bake,
    # never a glTF node transform.  Keeping the frame on the primitive lets
    # G826 generate normals/tangents from the final vertices while preserving
    # the single-mesh identity-node contract.
    rotation_radians: Tuple[float, float, float] = (0.0, 0.0, 0.0)
    rotation_origin_mm: Tuple[float, float, float] | None = None


@dataclass(frozen=True)
class CsgMeshPrimitive:
    part_role: str
    feature_node_id: str
    triangles: Tuple[Mapping[str, Any], ...]
    primitive_kind: str = "csg_mesh"


GeometryPrimitive = BoxPrimitive | CsgMeshPrimitive


@dataclass(frozen=True)
class GeometryBuildResult:
    glb_bytes: bytes
    shape_program: Dict[str, Any]
    assembly_graph: Dict[str, Any]
    bounds_mm: List[float]
    triangle_count: int
    topology_hash: str
    direction_id: str
    compile_readback: GeometryCompileReadback
    variant_id: str | None = None
    presentation_profile: str = "quick_sketch"


@dataclass(frozen=True)
class GeometryCompileResult:
    glb_bytes: bytes
    readback: GeometryCompileReadback


@dataclass(frozen=True)
class ShapeProgramGlbReadbackFacts:
    triangle_count: int
    bounds_mm: List[float]
    mesh_count: int
    primitive_count: int
    material_count: int
    uv0_primitive_count: int
    normal_primitive_count: int
    tangent_primitive_count: int
    surface_provenance: List[Dict[str, Any]]
    material_zone_faces: List[Dict[str, Any]]
    visual_texture_sets: List[Dict[str, Any]]
    visual_environment: Dict[str, Any]
    artifact_profile: Dict[str, Any]
    feature_history: List[Dict[str, Any]]


class GeometryCompileReadbackError(ValueError):
    """Compilation produced bytes whose GLB facts cannot be trusted."""

    code = "GEOMETRY_READBACK_FAILED"


@dataclass(frozen=True)
class ImportedGlbInspection:
    """Safe, metadata-only inspection result for a user supplied GLB.

    Imported geometry is a reference asset until an Agent rebuilds it as a
    ShapeProgram.  This inspector deliberately accepts only self-contained
    glTF 2.0 binary files and never evaluates extensions, shaders, URLs or
    application code embedded in a document.
    """

    sha256: str
    byte_size: int
    triangle_count: int
    bounds_mm: List[float]
    mesh_count: int
    primitive_count: int
    material_count: int
    node_count: int


MAX_IMPORTED_GLB_BYTES = 32 * 1024 * 1024
MAX_IMPORTED_GLB_TRIANGLES = 250_000
MAX_CSG_DEPTH = 8
MAX_EDGE_FINISH_RADIUS_RATIO = 0.25
MAX_EDGE_FINISH_SUBDIVISIONS = 3
VISUAL_UV_REPEAT_MM = 320.0
# Keep bounded primitives lightweight while avoiding the visibly faceted
# 16-sided silhouette that dominated wheels, rotors, joints and capsule shells
# in the real M108 workbench captures.  This is one fixed runtime baseline,
# not a user-controlled tessellation parameter or a second quality mode.
RADIAL_PRIMITIVE_SEGMENTS = 24

# Executor identifiers, not another operation allow-list.  The manifest owns
# the operation -> executor mapping; this set proves the shipped Worker still
# contains each declared implementation and gives tests a deterministic way to
# simulate a missing executor.
_WORKER_EXECUTOR_IDS = frozenset({
    "primitive_box",
    "primitive_cylinder",
    "primitive_capsule",
    "primitive_wedge",
    "profile_sketch",
    "profile_extrude",
    "profile_revolve",
    "profile_loft",
    "profile_sweep",
    "mirror_transform",
    "linear_array",
    "radial_array",
    "restricted_union",
    "restricted_subtract",
    "bevel_approximation",
    "surface_panel_attachment",
})


def assert_shape_program_runtime_compatible(program: Mapping[str, Any]) -> dict[str, Any]:
    """Validate manifest/schema input and fail closed if a Worker executor is absent.

    This remains compile-free so callers can guard inputs before work starts;
    Q003 quality then consumes ``compile_shape_program`` readback facts.
    """

    candidate = validate_shape_program(program)
    assert_worker_executor_coverage(_WORKER_EXECUTOR_IDS)
    return candidate


def _runtime_operation_error(operation_id: str, op: Any, reason: str) -> UnsupportedRuntimeOperationError:
    return UnsupportedRuntimeOperationError(
        operation_id=operation_id,
        op=op if isinstance(op, str) else "<invalid-op>",
        reason=reason,
    )


BLOCKOUT_VARIANT_IDS: Dict[str, Tuple[str, ...]] = {
    "pack_future_weapon_prop": (
        "compact_prop_a", "compact_prop_b", "compact_prop_c",
        "long_profile_prop_a", "long_profile_prop_b", "long_profile_prop_c",
        "heavy_support_prop_a", "heavy_support_prop_b", "heavy_support_prop_c",
        "energy_visual_prop_a", "energy_visual_prop_b", "energy_visual_prop_c",
    ),
    "pack_vehicle_concept": (
        "urban_scout_a", "urban_scout_b", "urban_scout_c",
        "exploration_vehicle_a", "exploration_vehicle_b", "exploration_vehicle_c",
        "low_racer_a", "low_racer_b", "low_racer_c",
        "heavy_transport_a", "heavy_transport_b", "heavy_transport_c",
    ),
    "pack_aircraft_concept": (
        "vertical_takeoff_a", "vertical_takeoff_b", "vertical_takeoff_c",
        "fast_single_seat_a", "fast_single_seat_b", "fast_single_seat_c",
        "wide_body_transport_a", "wide_body_transport_b", "wide_body_transport_c",
        "uncrewed_scout_a", "uncrewed_scout_b", "uncrewed_scout_c",
    ),
    "pack_robotic_arm_concept": (
        "precision_light_a", "precision_light_b", "precision_light_c",
        "heavy_handler_a", "heavy_handler_b", "heavy_handler_c",
        "long_reach_maintenance_a", "long_reach_maintenance_b", "long_reach_maintenance_c",
        "dual_tool_service_a", "dual_tool_service_b", "dual_tool_service_c",
    ),
}


def list_blockout_variants(pack_id: str) -> List[str]:
    """Return the versioned deterministic blockout catalog for one domain pack."""
    try:
        return list(BLOCKOUT_VARIANT_IDS[pack_id])
    except KeyError as exc:
        raise ValueError(f"unknown domain pack for blockout catalog: {pack_id}") from exc


_VARIANT_FAMILY_BY_SILHOUETTE = {
    "compact": 0,
    "balanced": 1,
    "extended": 2,
    "industrial": 3,
    "organic": 3,
}


def resolve_blockout_variant(
    plan: MechanicalConceptPlan,
    direction_id: str,
    variant_id: str | None = None,
    variation_index: int = 0,
) -> str:
    """Resolve one pre-reviewed visual variant without exposing free-form geometry.

    A caller can name a catalog entry for a repeatable preview. Ordinary
    workbench calls omit it: then the direction silhouette picks one related
    three-item family and the direction ID chooses a stable starting member.
    ``variation_index`` rotates only within that three-item family. When an
    exact catalog entry is supplied it remains authoritative; the index is
    retained by API callers solely as preview provenance. The result is
    visual-only; it never carries functional, manufacturing, or engineering
    data.
    """
    direction = next((item for item in plan.directions if item.direction_id == direction_id), None)
    if direction is None:
        raise ValueError(f"direction not found: {direction_id}")
    if variation_index not in {0, 1, 2}:
        raise ValueError("variation_index must be between 0 and 2")
    variants = list_blockout_variants(plan.domain_pack_id)
    if variant_id is not None:
        if variant_id not in variants:
            raise ValueError(f"unknown blockout variant {variant_id!r} for {plan.domain_pack_id}")
        return variant_id
    mapping_payload = plan.spec.get("visual_intent_mapping") if isinstance(plan.spec, dict) else None
    intent = visual_intent_for_direction(
        mapping_payload,
        domain_pack_id=plan.domain_pack_id,
        direction_id=direction_id,
    )
    # Old, imported or malformed plans keep the G812 silhouette fallback. A
    # valid G815 mapping can only choose an existing 0..3 catalog family.
    family = intent.variant_family_index if intent is not None else _VARIANT_FAMILY_BY_SILHOUETTE.get(direction.silhouette, 1)
    family_start = family * 3
    digest = hashlib.sha256(f"{plan.plan_id}:{direction_id}".encode("utf-8")).digest()
    return variants[family_start + ((digest[0] + variation_index) % 3)]


def build_blockout(
    plan: MechanicalConceptPlan,
    direction_id: str,
    variant_id: str | None = None,
    presentation_profile: str = "quick_sketch",
) -> GeometryBuildResult:
    direction = next((item for item in plan.directions if item.direction_id == direction_id), None)
    if direction is None:
        raise ValueError(f"direction not found: {direction_id}")
    if variant_id is not None and variant_id not in BLOCKOUT_VARIANT_IDS.get(plan.domain_pack_id, ()):
        raise ValueError(f"unknown blockout variant {variant_id!r} for {plan.domain_pack_id}")
    if presentation_profile not in {"quick_sketch", "showcase"}:
        raise ValueError("presentation_profile must be quick_sketch or showcase")
    boxes = _presentation_primitives(
        _boxes_for_domain(plan.domain_pack_id, direction.silhouette, variant_id),
        plan=plan,
        direction_id=direction_id,
        presentation_profile=presentation_profile,
    )
    program = _program_for_boxes(
        plan,
        direction_id,
        boxes,
        variant_id=variant_id,
        presentation_profile=presentation_profile,
    )
    # Candidate GLB bytes must travel through the same manifest-guarded
    # compile/readback entry point as later preview and export paths.
    compiled = compile_shape_program(program)
    assembly_graph = _assembly_graph(
        plan,
        direction_id,
        boxes,
        variant_id=variant_id,
        presentation_profile=presentation_profile,
    )
    topology_hash = hashlib.sha256(_canonical_json(program).encode("utf-8")).hexdigest()
    return GeometryBuildResult(
        glb_bytes=compiled.glb_bytes,
        shape_program=program,
        assembly_graph=assembly_graph,
        bounds_mm=compiled.readback.bounds_mm,
        triangle_count=compiled.readback.triangle_count,
        topology_hash=topology_hash,
        direction_id=direction_id,
        compile_readback=compiled.readback,
        variant_id=variant_id,
        presentation_profile=presentation_profile,
    )


def _normalize_surface_adornment_programs(
    programs: Sequence[Mapping[str, object]],
) -> tuple[dict[str, object], ...]:
    """Accept only closed A005 texture-bake instructions in stable order."""

    if len(programs) > 32:
        raise ValueError("surface adornment program count exceeds the restricted geometry limit")
    normalized = tuple(normalize_surface_adornment_program(item) for item in programs)
    program_ids = [str(item["program_id"]) for item in normalized]
    zone_ids = [str(item["target_zone_id"]) for item in normalized]
    if len(program_ids) != len(set(program_ids)):
        raise ValueError("surface adornment program ids must be unique")
    if len(zone_ids) != len(set(zone_ids)):
        raise ValueError("surface adornments may target each material zone at most once")
    return tuple(sorted(
        normalized,
        key=lambda item: (str(item["program_id"]), surface_adornment_material_id(item)),
    ))


def _surface_adornments_by_zone(
    boxes: Sequence[GeometryPrimitive],
    surface_adornments: Sequence[Mapping[str, object]],
) -> dict[str, Mapping[str, object]]:
    """Bind a normalized A005 program only to an existing homogeneous zone.

    The Rust-owned caller validates selection and Snapshot ownership before it
    reaches restricted geometry.  This worker still refuses a stale/missing
    zone or a base-material mismatch rather than guessing from role, colour or
    primitive position.
    """

    if not surface_adornments:
        return {}
    material_indices_by_zone: dict[str, set[int]] = {}
    for primitive in boxes:
        if isinstance(primitive, BoxPrimitive):
            zone_id = primitive.material_zone_id
            if not isinstance(zone_id, str):
                raise ValueError("surface adornment compilation requires stable material-zone provenance")
            material_indices_by_zone.setdefault(zone_id, set()).add(
                int(primitive.material_index)
            )
        else:
            for triangle in primitive.triangles:
                zone_id = triangle.get("material_zone_id")
                material_id = triangle.get("material_id")
                if not isinstance(zone_id, str) or not isinstance(material_id, str):
                    raise ValueError("surface adornment CSG provenance is invalid")
                material_indices_by_zone.setdefault(zone_id, set()).add(
                    builtin_visual_material_binding(material_id)[0]
                )
    resolved: dict[str, Mapping[str, object]] = {}
    for program in surface_adornments:
        zone_id = str(program["target_zone_id"])
        actual_indices = material_indices_by_zone.get(zone_id)
        if not actual_indices:
            raise ValueError("surface adornment target zone is not present in the compiled geometry")
        expected_index, _ = builtin_visual_material_binding(str(program["base_material"]))
        if actual_indices != {expected_index}:
            raise ValueError("surface adornment base material does not match the target zone")
        resolved[zone_id] = program
    return resolved


def _surface_layers_by_zone(
    boxes: Sequence[GeometryPrimitive],
    lowering: Mapping[str, object] | None,
) -> dict[str, Mapping[str, object]]:
    """Bind one sealed Design Surface to its exact existing material zone.

    A retained surface layer is not a new material selector.  Its Rust
    lowering carries the reviewed A005 base-material mapping, and this worker
    proves that the selected zone exists and is homogeneous before replacing
    the zone's visual-only PBR texture set.
    """

    if lowering is None:
        return {}
    normalized = normalize_surface_layer_lowering(lowering)
    adornments = normalized["adornments"]
    if not adornments:
        raise ValueError("surface layer lowering requires a reviewed A005 binding")
    zone_ids = {str(item["target_zone_id"]) for item in adornments}
    base_materials = {str(item["base_material"]) for item in adornments}
    if len(zone_ids) != 1 or len(base_materials) != 1:
        raise ValueError("surface layer lowering must bind one exact material zone")
    zone_id = next(iter(zone_ids))
    expected_index, _ = builtin_visual_material_binding(next(iter(base_materials)))
    material_indices_by_zone: dict[str, set[int]] = {}
    for primitive in boxes:
        if isinstance(primitive, BoxPrimitive):
            if not isinstance(primitive.material_zone_id, str):
                raise ValueError("surface layer compilation requires stable material-zone provenance")
            material_indices_by_zone.setdefault(primitive.material_zone_id, set()).add(
                int(primitive.material_index)
            )
        else:
            for triangle in primitive.triangles:
                triangle_zone = triangle.get("material_zone_id")
                triangle_material = triangle.get("material_id")
                if not isinstance(triangle_zone, str) or not isinstance(triangle_material, str):
                    raise ValueError("surface layer CSG provenance is invalid")
                material_indices_by_zone.setdefault(triangle_zone, set()).add(
                    builtin_visual_material_binding(triangle_material)[0]
                )
    actual_indices = material_indices_by_zone.get(zone_id)
    if not actual_indices:
        raise ValueError("surface layer target zone is not present in the compiled geometry")
    if actual_indices != {expected_index}:
        raise ValueError("surface layer base material does not match the target zone")
    return {zone_id: normalized}


def compile_shape_program(
    program: Dict[str, Any],
    *,
    artifact_profile_id: GeometryArtifactProfileId = "interactive_preview",
    surface_adornment_programs: Sequence[Mapping[str, object]] = (),
    surface_layer_lowering: Mapping[str, object] | None = None,
    cancel_check: Callable[[], bool] | None = None,
    csg_timeout_seconds: float = DEFAULT_CSG_TIMEOUT_SECONDS,
) -> GeometryCompileResult:
    """Compile once, then bind all trusted facts to the resulting GLB readback."""
    artifact_profile = geometry_artifact_profile_manifest(artifact_profile_id)
    surface_adornments = _normalize_surface_adornment_programs(surface_adornment_programs)
    normalized_surface_layer = (
        normalize_surface_layer_lowering(surface_layer_lowering)
        if surface_layer_lowering is not None
        else None
    )
    if normalized_surface_layer is not None:
        if list(surface_adornments) != normalized_surface_layer["adornments"]:
            raise ValueError("surface layer lowering must carry the exact Rust-lowered A005 adornment list")
        # The retained five-channel bake supersedes (rather than stacks with)
        # the intermediate A005 texture set.  The exact A005 list remains in
        # the sealed lowering and GLB provenance below.
        surface_adornments = ()
    program = assert_shape_program_runtime_compatible(program)
    source_triangle_budget = int(program.get("triangle_budget", 0))
    effective_triangle_budget = min(
        source_triangle_budget * int(artifact_profile["triangle_budget_multiplier"]),
        int(artifact_profile["max_triangle_count"]),
    )
    profile_inputs = {
        item["input_id"]: item
        for item in program.get("profile_inputs", [])
        if isinstance(item, dict)
    }
    resolved: Dict[str, List[GeometryPrimitive]] = {}
    csg_depth_by_id: Dict[str, int] = {}
    feature_hash_by_id: Dict[str, str] = {}
    feature_history: List[Dict[str, Any]] = []
    for operation in program.get("operations", []):
        if not isinstance(operation, dict):
            raise _runtime_operation_error("", "", "operation must be an object")
        operation_id = str(operation.get("operation_id", ""))
        op = operation.get("op")
        args = operation.get("args", {})
        if not isinstance(args, dict):
            raise _runtime_operation_error(operation_id, op, "args must be an object")
        position = args.get("position", [0, 0, 0])
        if not isinstance(position, list) or len(position) != 3:
            raise _runtime_operation_error(operation_id, op, "position must be a three-number vector")
        role = str(args.get("part_role", "part"))
        material_id = str(args.get("material_id", "")) or None
        material_zone_id = str(args.get("zone_id", "")) or None
        material = _material_index(material_id or "")
        if op in {"mirror", "array", "radial_array", "union", "subtract"} and any(
            _primitive_has_static_rotation(item)
            for input_id in operation.get("inputs", [])
            for item in resolved.get(str(input_id), [])
        ):
            raise _runtime_operation_error(
                operation_id,
                op,
                "static rotation through mirror, array or CSG is not implemented",
            )
        if op == "box":
            size = args.get("size")
            if not isinstance(size, list) or len(size) != 3 or any(float(value) <= 0 for value in size):
                raise _runtime_operation_error(operation_id, op, "box requires a positive three-number size")
            resolved[operation_id] = [BoxPrimitive(role, tuple(float(value) for value in position), tuple(float(value) for value in size), material, material_id=material_id)]
        elif op == "cylinder":
            radius = float(args.get("radius", 0))
            height = float(args.get("height", 0))
            if radius <= 0 or height <= 0:
                raise _runtime_operation_error(operation_id, op, "cylinder requires positive radius and height")
            axis = args.get("axis", [0, 1, 0])
            resolved[operation_id] = [_cylinder(role, tuple(float(value) for value in position), radius, height, material, tuple(float(value) for value in axis), material_id=material_id)]
        elif op == "capsule":
            radius = float(args.get("radius", 0))
            height = float(args.get("height", 0))
            if radius <= 0 or height <= 0:
                raise _runtime_operation_error(operation_id, op, "capsule requires positive radius and height")
            resolved[operation_id] = [BoxPrimitive(role, tuple(float(value) for value in position), (radius * 2, height, radius * 2), material, "capsule", radius, height, tuple(float(value) for value in args.get("axis", [0, 1, 0])), material_id=material_id)]
        elif op == "wedge":
            size = args.get("size")
            if not isinstance(size, list) or len(size) != 3 or any(float(value) <= 0 for value in size):
                raise _runtime_operation_error(operation_id, op, "wedge requires a positive three-number size")
            resolved[operation_id] = [BoxPrimitive(role, tuple(float(value) for value in position), tuple(float(value) for value in size), material, "wedge", material_id=material_id)]
        elif op == "profile":
            resolved[operation_id] = []
        elif op in {"extrude", "revolve"}:
            inputs = operation.get("inputs", [])
            profile_operation = next((item for item in program.get("operations", []) if isinstance(item, dict) and item.get("operation_id") == (inputs[0] if inputs else None)), None)
            profile_args = profile_operation.get("args", {}) if isinstance(profile_operation, dict) else {}
            profile_input_id = profile_args.get("profile_input_id")
            holes: List[List[List[float]]] = []
            profile_closed = True
            if profile_input_id is not None:
                from .profile_contracts import resample_profile_sketch

                profile_input = profile_inputs.get(profile_input_id)
                if not isinstance(profile_input, dict):
                    raise _runtime_operation_error(operation_id, op, "profile input provenance is missing")
                normalized, points, holes = resample_profile_sketch(profile_input["canonical_payload"])
                profile_closed = bool(normalized["closed"])
                scale = profile_args["profile_scale"]
                points = [[float(point[0]) * float(scale[0]), float(point[1]) * float(scale[1])] for point in points]
                holes = [
                    [[float(point[0]) * float(scale[0]), float(point[1]) * float(scale[1])] for point in contour]
                    for contour in holes
                ]
            else:
                points = profile_args.get("points")
            if not isinstance(points, list) or len(points) < 2:
                raise _runtime_operation_error(operation_id, op, "profile input has no usable points")
            if op == "extrude":
                height = float(args.get("height", 0))
                if height <= 0:
                    raise _runtime_operation_error(operation_id, op, "extrude requires positive height")
                resolved[operation_id] = [BoxPrimitive(
                    role,
                    tuple(float(value) for value in position),
                    (0.0, height, 0.0),
                    material,
                    "extrude_profile" if profile_input_id is not None else "extrude",
                    height_mm=height,
                    profile_points=tuple((float(point[0]), float(point[1])) for point in points),
                    profile_holes=tuple(tuple((float(point[0]), float(point[1])) for point in contour) for contour in holes),
                    profile_closed=profile_closed,
                    profile_input_id=profile_input_id,
                    cap_start=bool(args.get("cap_start", True)),
                    cap_end=bool(args.get("cap_end", True)),
                    material_id=material_id,
                )]
            else:
                angle = float(args.get("angle", math.pi * 2))
                if angle <= 0 or angle > math.pi * 2 or any(float(point[0]) < 0 for point in points):
                    raise _runtime_operation_error(operation_id, op, "revolve requires a valid non-negative-radius profile and angle")
                resolved[operation_id] = [BoxPrimitive(
                    role,
                    tuple(float(value) for value in position),
                    (0.0, 0.0, 0.0),
                    material,
                    "revolve_profile" if profile_input_id is not None else "revolve",
                    profile_points=tuple((float(point[0]), float(point[1])) for point in points),
                    profile_closed=profile_closed,
                    profile_input_id=profile_input_id,
                    revolve_angle=angle,
                    cap_start=bool(args.get("cap_start", False)),
                    cap_end=bool(args.get("cap_end", False)),
                    radial_segments=int(args.get("radial_segments", 24 if profile_input_id is not None else 16)),
                    material_id=material_id,
                )]
        elif op == "loft":
            from .profile_contracts import resample_profile_section_set

            input_id = args.get("section_set_input_id")
            profile_input = profile_inputs.get(input_id)
            if not isinstance(profile_input, dict) or profile_input.get("input_kind") != "profile_section_set":
                raise _runtime_operation_error(operation_id, op, "loft section-set provenance is missing")
            normalized, sections = resample_profile_section_set(profile_input["canonical_payload"])
            scale = args.get("cross_section_scale")
            axis_length = float(args.get("axis_length", 0))
            if not isinstance(scale, list) or len(scale) != 2 or axis_length <= 0:
                raise _runtime_operation_error(operation_id, op, "loft requires positive cross-section scale and axis length")
            axis = normalized["main_axis"]
            rings: List[Tuple[Tuple[float, float, float], ...]] = []
            profiles: List[Tuple[Tuple[float, float], ...]] = []
            for section in sections:
                if section["holes"]:
                    raise _runtime_operation_error(operation_id, op, "loft holes are not supported")
                angle = math.radians(float(section["twist_degrees"]))
                cosine, sine = math.cos(angle), math.sin(angle)
                section_scale = float(section["scale"])
                plane: List[Tuple[float, float]] = []
                ring: List[Tuple[float, float, float]] = []
                along = float(section["position"]) * axis_length / 2
                for point in section["points"]:
                    raw_u = float(point[0]) * float(scale[0]) * section_scale
                    raw_v = float(point[1]) * float(scale[1]) * section_scale
                    u = raw_u * cosine - raw_v * sine
                    v = raw_u * sine + raw_v * cosine
                    plane.append((u, v))
                    ring.append(_loft_axis_point(axis, along, u, v))
                profiles.append(tuple(plane))
                rings.append(tuple(ring))
            cap_start = sections[0]["cap_policy"] == "start"
            cap_end = sections[-1]["cap_policy"] == "end"
            resolved[operation_id] = [BoxPrimitive(
                role,
                tuple(float(value) for value in position),
                (0.0, 0.0, 0.0),
                material,
                "loft_profile",
                profile_input_id=str(input_id),
                cap_start=cap_start,
                cap_end=cap_end,
                loft_rings_mm=tuple(rings),
                loft_profiles=tuple(profiles),
                loft_axis=axis,
                material_id=material_id,
            )]
        elif op == "sweep":
            from .profile_contracts import resample_profile_sketch

            input_id = args.get("profile_input_id")
            profile_input = profile_inputs.get(input_id)
            if not isinstance(profile_input, dict) or profile_input.get("input_kind") != "profile_sketch":
                raise _runtime_operation_error(operation_id, op, "sweep profile provenance is missing")
            normalized, points, holes = resample_profile_sketch(profile_input["canonical_payload"])
            scale = args.get("profile_scale")
            path = args.get("path_points")
            if holes or not normalized["closed"] or not isinstance(scale, list) or not isinstance(path, list):
                raise _runtime_operation_error(operation_id, op, "sweep requires one closed hole-free profile and bounded path")
            scaled = tuple((float(point[0]) * float(scale[0]), float(point[1]) * float(scale[1])) for point in points)
            resolved[operation_id] = [BoxPrimitive(
                role,
                tuple(float(value) for value in position),
                (0.0, 0.0, 0.0),
                material,
                "sweep_profile",
                profile_points=scaled,
                profile_input_id=str(input_id),
                cap_start=bool(args.get("cap_start", True)),
                cap_end=bool(args.get("cap_end", True)),
                sweep_path_mm=tuple(tuple(float(value) for value in point) for point in path),
                path_closed=bool(args.get("path_closed", False)),
                path_twist_degrees=float(args.get("path_twist_degrees", 0)),
                material_id=material_id,
            )]
        elif op == "bevel_approx":
            inputs = operation.get("inputs", [])
            source = resolved.get(inputs[0], []) if inputs else []
            if len(source) != 1 or source[0].primitive_kind not in {"box", "bevel_box"}:
                raise ValueError("bevel_approx only supports one box source")
            radius = float(args.get("radius", 0))
            segments = int(args.get("segments", 1))
            if radius <= 0 or segments not in {1, 2, 3}:
                raise ValueError("bevel_approx requires radius > 0 and 1-3 segments")
            if radius >= min(source[0].size_mm[0], source[0].size_mm[2]) / 2:
                raise ValueError("bevel_approx radius must be smaller than the source X/Z half-size")
            radius_ratio = radius / min(source[0].size_mm[0], source[0].size_mm[2])
            if radius_ratio > MAX_EDGE_FINISH_RADIUS_RATIO:
                raise ValueError(
                    "EDGE_FINISH_RADIUS_RATIO_EXCEEDED: bevel_approx radius ratio "
                    f"{radius_ratio:.6f} exceeds {MAX_EDGE_FINISH_RADIUS_RATIO}"
                )
            if segments > MAX_EDGE_FINISH_SUBDIVISIONS:
                raise ValueError("EDGE_FINISH_SUBDIVISION_BUDGET_EXCEEDED")
            source_primitive = source[0]
            resolved[operation_id] = [replace(
                source_primitive,
                part_role=str(args.get("part_role") or source_primitive.part_role),
                material_index=material if args.get("material_id") else source_primitive.material_index,
                material_id=material_id if args.get("material_id") else source_primitive.material_id,
                primitive_kind="bevel_box",
                bevel_radius_mm=radius,
                bevel_segments=segments,
                source_operation_id=operation_id,
            )]
        elif op == "surface_panel":
            inputs = operation.get("inputs", [])
            source = resolved.get(inputs[0], []) if inputs else []
            if len(source) != 1 or source[0].primitive_kind not in {"box", "bevel_box"}:
                raise ValueError("surface_panel only supports one box or bevel_approx source")
            base = source[0]
            panel_size = args.get("size")
            if panel_size is None:
                panel_size = [base.size_mm[0] * 0.6, max(1.0, min(base.size_mm[1] * 0.08, 20.0)), base.size_mm[2] * 0.6]
            if not isinstance(panel_size, list) or len(panel_size) != 3 or any(float(value) <= 0 for value in panel_size):
                raise ValueError("surface_panel requires a positive size")
            panel_size_tuple = tuple(float(value) for value in panel_size)
            if panel_size_tuple[0] > base.size_mm[0] or panel_size_tuple[2] > base.size_mm[2]:
                raise ValueError("surface_panel must fit within the source X/Z face")
            axis = args.get("axis", [0, 1, 0])
            sign = 1.0 if axis == [0, 1, 0] else -1.0
            offset = position
            panel_center = (
                base.center_mm[0] + float(offset[0]),
                base.center_mm[1] + sign * (base.size_mm[1] / 2 + panel_size_tuple[1] / 2),
                base.center_mm[2] + float(offset[2]),
            )
            if abs(float(offset[0])) + panel_size_tuple[0] / 2 > base.size_mm[0] / 2 or abs(float(offset[2])) + panel_size_tuple[2] / 2 > base.size_mm[2] / 2:
                raise ValueError("surface_panel offset places the panel outside the source X/Z face")
            panel = BoxPrimitive(
                str(args.get("part_role") or "surface_panel"),
                panel_center,
                panel_size_tuple,
                material,
                "surface_panel",
                material_id=material_id or base.material_id,
                rotation_radians=base.rotation_radians,
                rotation_origin_mm=base.rotation_origin_mm or base.center_mm,
            )
            resolved[operation_id] = [*source, panel]
        elif op in {"mirror", "array", "radial_array"}:
            inputs = operation.get("inputs", [])
            source = resolved.get(inputs[0], []) if inputs else []
            if not source:
                raise _runtime_operation_error(operation_id, op, "transform source has no exportable geometry")
            axis = _dominant_axis(args.get("axis", [0, 1, 0]))
            if op == "mirror":
                resolved[operation_id] = [_mirror_primitive(item, axis) for item in source]
            elif op == "array":
                count = int(args.get("count", 0))
                spacing = float(args.get("spacing", 0))
                resolved[operation_id] = [
                    _translate_primitive(item, axis, spacing * index)
                    for index in range(count)
                    for item in source
                ]
            else:
                count = int(args.get("count", 0))
                radius = float(args.get("radius", 0))
                angle = float(args.get("angle", math.pi * 2))
                resolved[operation_id] = [_radial_primitive(item, axis, radius, angle * index / count) for index in range(count) for item in source]
        elif op in {"union", "subtract"}:
            inputs = operation.get("inputs", [])
            first = resolved.get(inputs[0], []) if len(inputs) > 0 else []
            second = resolved.get(inputs[1], []) if len(inputs) > 1 else []
            if not first or not second:
                raise ManifoldCsgError("CSG_INPUT_MISSING", operation_id, f"{op} requires two exportable geometry inputs")
            depth = 1 + max(csg_depth_by_id.get(str(input_id), 0) for input_id in inputs)
            if depth > MAX_CSG_DEPTH:
                raise ManifoldCsgError(
                    "CSG_DEPTH_EXCEEDED",
                    operation_id,
                    f"boolean feature depth {depth} exceeds {MAX_CSG_DEPTH}",
                )
            csg_depth_by_id[operation_id] = depth
            if any(
                _box_inputs_have_near_coincident_planes(left, right)
                for left in first
                for right in second
                if isinstance(left, BoxPrimitive) and isinstance(right, BoxPrimitive)
            ):
                raise ManifoldCsgError(
                    "CSG_DEGENERATE_OUTPUT",
                    operation_id,
                    "near-coincident input planes were rejected before kernel/GLB",
                )
            triangles = execute_manifold_boolean(
                node_id=operation_id,
                operation=str(op),
                left_solids=[_primitive_csg_solid(item) for item in first],
                right_solids=[_primitive_csg_solid(item) for item in second],
                triangle_budget=effective_triangle_budget,
                cancel_check=cancel_check,
                timeout_seconds=csg_timeout_seconds,
            )
            resolved[operation_id] = [
                CsgMeshPrimitive(
                    part_role=role,
                    feature_node_id=operation_id,
                    triangles=tuple(triangles),
                )
            ]
        else:
            raise _runtime_operation_error(operation_id, op, "operation has no Worker implementation")
        if op in {"box", "cylinder", "capsule", "wedge", "extrude", "revolve", "loft", "sweep"}:
            resolved[operation_id] = _apply_static_operation_rotation(
                resolved.get(operation_id, []), args, operation_id, op,
            )
        elif _operation_has_non_identity_rotation(args):
            raise _runtime_operation_error(
                operation_id,
                op,
                "rotation is only defined for static source geometry; derived transforms fail closed",
            )
        csg_depth_by_id.setdefault(
            operation_id,
            max((csg_depth_by_id.get(str(input_id), 0) for input_id in operation.get("inputs", [])), default=0),
        )
        resolved[operation_id] = [
            replace(
                item,
                material_zone_id=item.material_zone_id or material_zone_id or f"zone_{item.part_role}",
                source_operation_id=item.source_operation_id or operation_id,
            )
            if isinstance(item, BoxPrimitive)
            else item
            for item in resolved.get(operation_id, [])
        ]
        resolved[operation_id] = _apply_artifact_profile_to_primitives(
            resolved.get(operation_id, []),
            artifact_profile_id=artifact_profile_id,
        )
        feature = _feature_node_readback(
            operation=operation,
            primitives=resolved.get(operation_id, []),
            input_hashes=[feature_hash_by_id[str(input_id)] for input_id in operation.get("inputs", [])],
            csg_depth=csg_depth_by_id[operation_id],
            artifact_profile_sha256=str(artifact_profile["profile_sha256"]),
        )
        feature_hash_by_id[operation_id] = str(feature["result_sha256"])
        feature_history.append(feature)
    boxes = [item for output in program.get("outputs", []) if isinstance(output, dict) for item in resolved.get(str(output.get("operation_id")), [])]
    if not boxes:
        raise ValueError("ShapeProgram has no exportable geometry outputs")
    expected_triangles = sum(_primitive_triangle_count(item) for item in boxes)
    if expected_triangles > effective_triangle_budget:
        raise _runtime_operation_error(
            "runtime_triangle_budget",
            "compile",
            f"compiled triangle count {expected_triangles} exceeds artifact budget {effective_triangle_budget}",
        )
    adornments_by_zone = _surface_adornments_by_zone(boxes, surface_adornments)
    surface_layers_by_zone = _surface_layers_by_zone(boxes, normalized_surface_layer)
    glb, compiler_bounds = _build_glb(
        boxes,
        feature_history=feature_history,
        artifact_profile_id=artifact_profile_id,
        surface_adornments_by_zone=adornments_by_zone,
        surface_layers_by_zone=surface_layers_by_zone,
    )
    try:
        facts = read_shape_program_glb_facts(glb)
    except (ValueError, TypeError, KeyError, IndexError, struct.error, json.JSONDecodeError) as exc:
        raise GeometryCompileReadbackError(f"GLB readback failed: {exc}") from exc
    if not facts.feature_history:
        raise GeometryCompileReadbackError("GLB immutable feature history is missing")
    if facts.triangle_count != expected_triangles:
        raise GeometryCompileReadbackError("GLB readback triangle count does not match compiled geometry")
    if any(abs(float(left) - float(right)) > 0.01 for left, right in zip(compiler_bounds, facts.bounds_mm)):
        raise GeometryCompileReadbackError("GLB readback bounds do not match compiled geometry")
    if facts.artifact_profile != artifact_profile:
        raise GeometryCompileReadbackError("GLB artifact profile does not match the requested compile profile")
    if facts.triangle_count > effective_triangle_budget:
        raise _runtime_operation_error(
            "runtime_triangle_budget",
            "compile",
            f"GLB readback triangle count {facts.triangle_count} exceeds artifact budget {effective_triangle_budget}",
        )
    operations = [item for item in program.get("operations", []) if isinstance(item, dict)]
    outputs = [item for item in program.get("outputs", []) if isinstance(item, dict)]
    readback = GeometryCompileReadback(
        schema_version="GeometryCompileReadback@2",
        runtime_manifest_version=MANIFEST_SCHEMA_VERSION,
        artifact_profile=facts.artifact_profile,
        program_id=str(program["program_id"]),
        shape_program_sha256=hashlib.sha256(_canonical_json(program).encode("utf-8")).hexdigest(),
        glb_sha256=hashlib.sha256(glb).hexdigest(),
        glb_byte_size=len(glb),
        triangle_count=facts.triangle_count,
        bounds_mm=facts.bounds_mm,
        mesh_count=facts.mesh_count,
        primitive_count=facts.primitive_count,
        material_count=facts.material_count,
        uv0_primitive_count=facts.uv0_primitive_count,
        normal_primitive_count=facts.normal_primitive_count,
        tangent_primitive_count=facts.tangent_primitive_count,
        surface_provenance=facts.surface_provenance,
        material_zone_faces=facts.material_zone_faces,
        visual_texture_sets=facts.visual_texture_sets,
        visual_environment=facts.visual_environment,
        feature_history=facts.feature_history,
        operation_ids=[str(item["operation_id"]) for item in operations],
        operation_names=[str(item["op"]) for item in operations],
        output_roles=[str(item["part_role"]) for item in outputs],
        material_ids=sorted(_material_ids_for_primitives(boxes)),
        readback_status="passed",
    )
    return GeometryCompileResult(glb_bytes=glb, readback=readback)


def build_glb_from_shape_program(program: Dict[str, Any]) -> Tuple[bytes, List[float], int]:
    """Compatibility tuple backed by the single compile/readback result."""

    compiled = compile_preview_shape_program(program)
    return compiled.glb_bytes, compiled.readback.bounds_mm, compiled.readback.triangle_count


def compile_preview_shape_program(
    program: Dict[str, Any],
    *,
    surface_adornment_programs: Sequence[Mapping[str, object]] = (),
    cancel_check: Callable[[], bool] | None = None,
    csg_timeout_seconds: float = DEFAULT_CSG_TIMEOUT_SECONDS,
) -> GeometryCompileResult:
    """Compile the responsive workbench derivative of one ShapeProgram."""

    return compile_shape_program(
        program,
        artifact_profile_id="interactive_preview",
        surface_adornment_programs=surface_adornment_programs,
        cancel_check=cancel_check,
        csg_timeout_seconds=csg_timeout_seconds,
    )


def compile_production_concept_shape_program(
    program: Dict[str, Any],
    *,
    surface_adornment_programs: Sequence[Mapping[str, object]] = (),
    cancel_check: Callable[[], bool] | None = None,
    csg_timeout_seconds: float = DEFAULT_CSG_TIMEOUT_SECONDS,
) -> GeometryCompileResult:
    """Compile the on-demand production concept derivative of one ShapeProgram."""

    return compile_shape_program(
        program,
        artifact_profile_id="production_concept",
        surface_adornment_programs=surface_adornment_programs,
        cancel_check=cancel_check,
        csg_timeout_seconds=csg_timeout_seconds,
    )


def read_shape_program_glb(payload: bytes) -> Tuple[int, List[float]]:
    """Read back the exact static GLB contract produced by this worker."""
    facts = read_shape_program_glb_facts(payload)
    return facts.triangle_count, facts.bounds_mm


def read_shape_program_glb_facts(payload: bytes) -> ShapeProgramGlbReadbackFacts:
    """Read all geometry facts used by quality and export from one GLB parse."""
    if len(payload) < 20 or len(payload) > 64 * 1024 * 1024:
        raise ValueError("GLB is outside the supported readback size")
    magic, version, declared_length = struct.unpack_from("<4sII", payload, 0)
    if magic != b"glTF" or version != 2 or declared_length != len(payload):
        raise ValueError("GLB header is invalid")
    offset = 12
    document: Dict[str, Any] | None = None
    binary = b""
    while offset + 8 <= len(payload):
        length, chunk_type = struct.unpack_from("<II", payload, offset)
        offset += 8
        end = offset + length
        if end > len(payload):
            raise ValueError("GLB chunk exceeds file length")
        if chunk_type == 0x4E4F534A and document is None:
            document = json.loads(payload[offset:end].rstrip(b" \x00").decode("utf-8"))
        elif chunk_type == 0x004E4942 and not binary:
            binary = payload[offset:end]
        offset = end
    if offset != len(payload) or document is None or not binary:
        raise ValueError("GLB chunks are incomplete")
    if document.get("asset", {}).get("version") != "2.0":
        raise ValueError("GLB asset version is not 2.0")
    accessors = document.get("accessors", [])
    views = document.get("bufferViews", [])
    if not isinstance(accessors, list) or not isinstance(views, list):
        raise ValueError("GLB accessors or bufferViews are invalid")
    triangles = 0
    primitive_count = 0
    uv0_primitive_count = 0
    normal_primitive_count = 0
    tangent_primitive_count = 0
    surface_provenance: List[Dict[str, Any]] = []
    material_zone_faces: List[Dict[str, Any]] = []
    primitive_material_bindings: List[Tuple[int, str, str]] = []
    minimum = [float("inf")] * 3
    maximum = [float("-inf")] * 3
    meshes = document.get("meshes", [])
    _validate_static_readback_scene(document, meshes)
    materials = document.get("materials", [])
    if not isinstance(materials, list):
        raise ValueError("GLB materials are invalid")
    artifact_profile = _read_geometry_artifact_profile_facts(document)
    document_extras = document.get("extras", {})
    feature_history = document_extras.get("forgecad_feature_history") if isinstance(document_extras, dict) else None
    if feature_history is None:
        # Historical G824 evidence GLBs predate immutable feature history and
        # remain readable. New Worker compilation rejects this empty result.
        feature_history = []
    elif not isinstance(feature_history, list):
        raise ValueError("GLB immutable feature history is invalid")
    for feature in feature_history:
        if (
            not isinstance(feature, dict)
            or feature.get("schema_version") != "GeometryFeatureNodeReadback@1"
            or not isinstance(feature.get("node_id"), str)
            or not isinstance(feature.get("input_node_ids"), list)
            or not isinstance(feature.get("input_hashes"), list)
            or len(feature["input_node_ids"]) != len(feature["input_hashes"])
            or any(
                not isinstance(feature.get(field), str)
                or len(feature[field]) != 64
                or any(char not in "0123456789abcdef" for char in feature[field])
                for field in (
                    "parameters_sha256",
                    "node_input_sha256",
                    "result_sha256",
                    "surface_provenance_sha256",
                )
            )
        ):
            raise ValueError("GLB immutable feature history is invalid")
    for mesh in meshes:
        for primitive in mesh.get("primitives", []):
            attributes = primitive.get("attributes", {})
            position_index = attributes.get("POSITION")
            normal_index = attributes.get("NORMAL")
            uv_index = attributes.get("TEXCOORD_0")
            tangent_index = attributes.get("TANGENT")
            face_id_index = attributes.get("_FORGECAD_FACE_ID")
            source_face_id_index = attributes.get("_FORGECAD_SOURCE_FACE_ID")
            index_index = primitive.get("indices")
            if any(value is None for value in (position_index, normal_index, uv_index, tangent_index, face_id_index, source_face_id_index, index_index)):
                raise ValueError(
                    "GLB primitive is missing POSITION, NORMAL, TEXCOORD_0, TANGENT, "
                    "stable face provenance or indices"
                )
            position_accessor = _readback_accessor_record(accessors, position_index)
            normal_accessor = _readback_accessor_record(accessors, normal_index)
            uv_accessor = _readback_accessor_record(accessors, uv_index)
            tangent_accessor = _readback_accessor_record(accessors, tangent_index)
            face_id_accessor = _readback_accessor_record(accessors, face_id_index)
            source_face_id_accessor = _readback_accessor_record(accessors, source_face_id_index)
            index_accessor = _readback_accessor_record(accessors, index_index)
            if (
                position_accessor.get("type") != "VEC3"
                or normal_accessor.get("type") != "VEC3"
                or uv_accessor.get("type") != "VEC2"
                or tangent_accessor.get("type") != "VEC4"
                or face_id_accessor.get("type") != "SCALAR"
                or source_face_id_accessor.get("type") != "SCALAR"
                or index_accessor.get("type") != "SCALAR"
            ):
                raise ValueError("GLB accessor types are invalid")
            vertex_count = position_accessor.get("count")
            if type(vertex_count) is not int or vertex_count <= 0 or any(
                accessor.get("count") != vertex_count
                for accessor in (normal_accessor, uv_accessor, tangent_accessor, face_id_accessor, source_face_id_accessor)
            ):
                raise ValueError("GLB surface attribute counts do not align")
            count = index_accessor.get("count")
            if type(count) is not int or count <= 0 or count % 3:
                raise ValueError("GLB index count is not divisible by three")
            triangles += count // 3
            primitive_count += 1
            normal_primitive_count += 1
            uv0_primitive_count += 1
            tangent_primitive_count += 1
            extras = primitive.get("extras", {})
            roles = extras.get("forgecad_surface_roles", []) if isinstance(extras, dict) else []
            ranges = extras.get("forgecad_surface_ranges", []) if isinstance(extras, dict) else []
            if not isinstance(roles, list) or any(not isinstance(role, str) or not role for role in roles):
                raise ValueError("GLB surface provenance is invalid")
            if not isinstance(ranges, list) or any(
                not isinstance(item, dict)
                or item.get("surface_role") not in roles
                or not isinstance(item.get("first_triangle"), int)
                or not isinstance(item.get("triangle_count"), int)
                or item["first_triangle"] < 0
                or item["triangle_count"] < 0
                for item in ranges
            ):
                raise ValueError("GLB surface triangle ranges are invalid")
            positive_ranges = [item for item in ranges if item["triangle_count"] > 0]
            if sum(item["triangle_count"] for item in positive_ranges) != count // 3:
                raise ValueError("GLB surface triangle ranges do not cover primitive indices")
            cursor = 0
            for item in positive_ranges:
                if item["first_triangle"] != cursor:
                    raise ValueError("GLB surface triangle ranges are not contiguous")
                cursor += item["triangle_count"]
            position_values = _readback_float_accessor(
                accessors,
                views,
                binary,
                position_index,
                3,
            )
            if any(not math.isfinite(value) for position in position_values for value in position):
                raise ValueError("GLB POSITION contains non-finite values")
            actual_lower = [min(position[axis] for position in position_values) for axis in range(3)]
            actual_upper = [max(position[axis] for position in position_values) for axis in range(3)]
            declared_lower = position_accessor.get("min")
            declared_upper = position_accessor.get("max")
            if (
                not isinstance(declared_lower, list)
                or not isinstance(declared_upper, list)
                or len(declared_lower) != 3
                or len(declared_upper) != 3
                or any(
                    not isinstance(value, (int, float)) or isinstance(value, bool) or not math.isfinite(float(value))
                    for value in [*declared_lower, *declared_upper]
                )
            ):
                raise ValueError("GLB position bounds are missing or invalid")
            if any(
                not math.isclose(float(declared_lower[axis]), actual_lower[axis], rel_tol=1e-6, abs_tol=1e-7)
                or not math.isclose(float(declared_upper[axis]), actual_upper[axis], rel_tol=1e-6, abs_tol=1e-7)
                for axis in range(3)
            ):
                raise ValueError("GLB declared position bounds do not match POSITION bytes")
            topology = _readback_primitive_topology(
                accessors,
                views,
                binary,
                position_index,
                index_index,
            )
            uv_values = _readback_float_accessor(accessors, views, binary, uv_index, 2)
            normal_values = _readback_float_accessor(accessors, views, binary, normal_index, 3)
            tangent_values = _readback_float_accessor(accessors, views, binary, tangent_index, 4)
            indices = _readback_index_accessor(accessors, views, binary, index_index)
            face_id_values = _readback_stable_face_id_accessor(accessors, views, binary, face_id_index)
            source_face_id_values = _readback_stable_face_id_accessor(
                accessors,
                views,
                binary,
                source_face_id_index,
            )
            if any(not math.isfinite(value) for item in [*uv_values, *normal_values, *tangent_values] for value in item):
                raise ValueError("GLB UV0, normal or tangent contains non-finite values")
            uv_repeat_mm = extras.get("forgecad_visual_uv_repeat_mm") if isinstance(extras, dict) else None
            if uv_repeat_mm is None:
                if any(value < -1e-6 or value > 1 + 1e-6 for uv in uv_values for value in uv):
                    raise ValueError("GLB UV0 is outside the normalized baseline")
            else:
                if not isinstance(uv_repeat_mm, (int, float)) or not math.isclose(float(uv_repeat_mm), VISUAL_UV_REPEAT_MM):
                    raise ValueError("GLB visual UV repeat metadata is invalid")
                if any(value < -1e-6 or value > 64 + 1e-6 for uv in uv_values for value in uv):
                    raise ValueError("GLB UV0 is outside the bounded visual repeat baseline")
            if any(abs(math.sqrt(sum(value * value for value in normal)) - 1) > 1e-3 for normal in normal_values):
                raise ValueError("GLB contains a non-unit normal")
            tangent_lengths = [math.sqrt(sum(value * value for value in tangent[:3])) for tangent in tangent_values]
            if any(abs(length - 1) > 1e-3 for length in tangent_lengths):
                raise ValueError("GLB contains a non-unit tangent")
            if any(abs(sum(normal[index] * tangent[index] for index in range(3))) > 1e-3 for normal, tangent in zip(normal_values, tangent_values)):
                raise ValueError("GLB tangent is not orthogonal to its normal")
            tangent_handedness = sorted({int(round(tangent[3])) for tangent in tangent_values})
            if any(value not in {-1, 1} for value in tangent_handedness):
                raise ValueError("GLB tangent handedness is invalid")
            face_ids: List[int] = []
            source_face_ids: List[int] = []
            uv_degenerate_triangle_count = 0
            for offset in range(0, len(indices), 3):
                triangle_indices = indices[offset:offset + 3]
                triangle_face_ids = {face_id_values[index] for index in triangle_indices}
                triangle_source_face_ids = {source_face_id_values[index] for index in triangle_indices}
                if len(triangle_face_ids) != 1 or len(triangle_source_face_ids) != 1:
                    raise ValueError("GLB stable face provenance is split inside a triangle")
                face_ids.append(next(iter(triangle_face_ids)))
                source_face_ids.append(next(iter(triangle_source_face_ids)))
                uv_a, uv_b, uv_c = [uv_values[index] for index in triangle_indices]
                determinant = (
                    (uv_b[0] - uv_a[0]) * (uv_c[1] - uv_a[1])
                    - (uv_b[1] - uv_a[1]) * (uv_c[0] - uv_a[0])
                )
                if abs(determinant) <= 1e-12:
                    uv_degenerate_triangle_count += 1
            if sorted(face_ids) != list(range(count // 3)):
                raise ValueError("GLB stable face ids are missing, duplicated or non-contiguous")
            if uv_degenerate_triangle_count:
                raise ValueError("GLB contains UV-degenerate triangles and is not texture ready")
            profile_input_id = extras.get("forgecad_profile_input_id")
            if profile_input_id is not None:
                if topology["degenerate_triangle_count"]:
                    raise ValueError("profile GLB contains degenerate triangles")
            csg_provenance = extras.get("forgecad_csg_provenance")
            if csg_provenance is not None and (
                not isinstance(csg_provenance, dict)
                or not isinstance(csg_provenance.get("source_operation_ids"), list)
                or not csg_provenance["source_operation_ids"]
                or not isinstance(csg_provenance.get("material_zone_id"), str)
                or not isinstance(csg_provenance.get("boolean_backside"), bool)
            ):
                raise ValueError("GLB CSG surface provenance is invalid")
            primitive_id = extras.get("forgecad_primitive_id")
            part_instance_id = extras.get("forgecad_part_instance_id")
            material_zone_id = extras.get("forgecad_material_zone_id")
            normal_mode = extras.get("forgecad_normal_mode")
            edge_finish = extras.get("forgecad_edge_finish")
            if (
                not isinstance(primitive_id, str)
                or not primitive_id.startswith("primitive_")
                or not isinstance(part_instance_id, str)
                or not part_instance_id.startswith("partface_")
                or not isinstance(material_zone_id, str)
                or not material_zone_id.startswith("zone_")
                or normal_mode not in {"split", "split_weighted"}
            ):
                raise ValueError("GLB part, primitive, normal or material-zone provenance is invalid")
            _validate_edge_finish_readback(edge_finish)
            feature_node_id = extras.get("forgecad_feature_node_id")
            source_operation_ids = (
                [str(value) for value in csg_provenance["source_operation_ids"]]
                if csg_provenance is not None
                else ([str(feature_node_id)] if isinstance(feature_node_id, str) else [])
            )
            if not source_operation_ids or any(not value.startswith("op_") for value in source_operation_ids):
                raise ValueError("GLB zone faces are missing source operation provenance")
            face_id_sha256 = hashlib.sha256(_canonical_json({
                "primitive_id": primitive_id,
                "part_instance_id": part_instance_id,
                "material_zone_id": material_zone_id,
                "face_ids": sorted(face_ids),
                "source_face_ids": sorted(source_face_ids),
            }).encode()).hexdigest()
            provenance_item = {
                "primitive_id": primitive_id,
                "part_instance_id": part_instance_id,
                "part_role": str(extras.get("forgecad_part_role", "part")),
                "profile_input_id": profile_input_id,
                "surface_roles": roles,
                "surface_ranges": ranges,
                "uv0_min": [min(value[index] for value in uv_values) for index in range(2)],
                "uv0_max": [max(value[index] for value in uv_values) for index in range(2)],
                "material_zone_id": material_zone_id,
                "source_operation_ids": source_operation_ids,
                "normal_mode": normal_mode,
                "tangent_min_length": min(tangent_lengths),
                "tangent_max_length": max(tangent_lengths),
                "tangent_handedness": tangent_handedness,
                "uv_degenerate_triangle_count": uv_degenerate_triangle_count,
                "tangent_fallback_triangle_count": 0,
                "face_id_min": min(face_ids),
                "face_id_max": max(face_ids),
                "face_id_sha256": face_id_sha256,
                "edge_finish": edge_finish,
                "texture_ready": True,
                **topology,
            }
            if feature_node_id is not None:
                provenance_item["feature_node_id"] = str(feature_node_id)
            if csg_provenance is not None:
                provenance_item.update({
                    "source_operation_ids": source_operation_ids,
                    "boolean_backside": bool(csg_provenance["boolean_backside"]),
                })
            material_index = primitive.get("material")
            if type(material_index) is not int or material_index < 0 or material_index >= len(materials):
                raise ValueError("GLB primitive material is invalid")
            material_id = extras.get("forgecad_material_id")
            if not isinstance(material_id, str) or not material_id.startswith("mat_"):
                raise ValueError("GLB primitive material identity is invalid")
            material_extras = materials[material_index].get("extras", {}) if isinstance(materials[material_index], dict) else {}
            surface_layer = (
                material_extras.get("forgecad_surface_layer_lowering")
                if isinstance(material_extras, dict)
                else None
            )
            adornment = (
                material_extras.get("forgecad_surface_adornment")
                if isinstance(material_extras, dict)
                else None
            )
            if surface_layer is not None:
                if adornment is not None:
                    raise ValueError("GLB material cannot mix retained surface layers with A005 material rows")
                normalized_layer = normalize_surface_layer_lowering(surface_layer)
                expected_zone_id = str(normalized_layer["adornments"][0]["target_zone_id"])
                expected_base_material_id = str(normalized_layer["adornments"][0]["base_material"])
                if (
                    material_id != surface_layer_material_id(normalized_layer)
                    or material_zone_id != expected_zone_id
                    or material_index < builtin_visual_material_count()
                    or material_extras.get("forgecad_surface_layer_lowering_sha256")
                    != surface_layer_lowering_sha256(normalized_layer)
                    or material_extras.get("forgecad_surface_layer_retained_layers_sha256")
                    != normalized_layer["retained_layers_sha256"]
                    or material_extras.get("forgecad_base_material_id")
                    != builtin_visual_material_binding(expected_base_material_id)[1]
                ):
                    raise ValueError("GLB retained surface layer primitive provenance is invalid")
            elif adornment is None:
                try:
                    expected_material_index, _texture_material_id = builtin_visual_material_binding(material_id)
                except ValueError as exc:
                    raise ValueError("GLB primitive material identity is outside the reviewed visual catalog") from exc
                if material_index != expected_material_index:
                    raise ValueError("GLB primitive material identity does not match its canonical texture material")
            else:
                normalized_adornment = normalize_surface_adornment_program(adornment)
                if (
                    material_id != surface_adornment_material_id(normalized_adornment)
                    or material_zone_id != normalized_adornment["target_zone_id"]
                    or material_index < builtin_visual_material_count()
                    or material_extras.get("forgecad_surface_adornment_sha256")
                    != surface_adornment_program_sha256(normalized_adornment)
                    or material_extras.get("forgecad_base_material_id")
                    != builtin_visual_material_binding(str(normalized_adornment["base_material"]))[1]
                ):
                    raise ValueError("GLB surface adornment primitive provenance is invalid")
            surface_provenance.append(provenance_item)
            material_zone_faces.append({
                "primitive_id": primitive_id,
                "part_instance_id": part_instance_id,
                "material_zone_id": material_zone_id,
                "material_id": material_id,
                "face_count": len(face_ids),
                "face_id_sha256": face_id_sha256,
                "surface_roles": roles,
                "source_operation_ids": source_operation_ids,
                "texture_ready": True,
            })
            primitive_material_bindings.append((material_index, material_id, material_zone_id))
            for axis in range(3):
                minimum[axis] = min(minimum[axis], actual_lower[axis] * 1000)
                maximum[axis] = max(maximum[axis], actual_upper[axis] * 1000)
    primitive_ids = [str(item["primitive_id"]) for item in surface_provenance]
    if len(primitive_ids) != len(set(primitive_ids)):
        raise ValueError("GLB material zones overlap because primitive identity is duplicated")
    zone_keys = [
        (str(item["primitive_id"]), str(item["material_zone_id"]), str(item["face_id_sha256"]))
        for item in material_zone_faces
    ]
    if len(zone_keys) != len(set(zone_keys)):
        raise ValueError("GLB material-zone face sets overlap")
    visual_texture_sets = _read_visual_pbr_facts(
        document=document,
        binary=binary,
        views=views,
        materials=materials,
        primitive_material_bindings=primitive_material_bindings,
        artifact_profile_id=artifact_profile["artifact_profile_id"],
    )
    visual_environment = _read_visual_environment_facts(document)
    return ShapeProgramGlbReadbackFacts(
        triangle_count=triangles,
        bounds_mm=[round(maximum[index] - minimum[index], 4) for index in range(3)],
        mesh_count=len(meshes),
        primitive_count=primitive_count,
        material_count=len(materials),
        uv0_primitive_count=uv0_primitive_count,
        normal_primitive_count=normal_primitive_count,
        tangent_primitive_count=tangent_primitive_count,
        surface_provenance=surface_provenance,
        material_zone_faces=material_zone_faces,
        visual_texture_sets=visual_texture_sets,
        visual_environment=visual_environment,
        artifact_profile=artifact_profile,
        feature_history=[dict(item) for item in feature_history],
    )


def _read_visual_pbr_facts(
    *,
    document: Mapping[str, Any],
    binary: bytes,
    views: Sequence[Any],
    materials: Sequence[Any],
    primitive_material_bindings: Sequence[Tuple[int, str, str]],
    artifact_profile_id: GeometryArtifactProfileId,
) -> List[Dict[str, Any]]:
    """Verify the exact embedded PBR payload consumed by GLB/quality/export.

    This is intentionally stricter than a presentation-side material guess:
    every bound zone must reference a GLB material with five embedded maps,
    correct channel colour spaces and hash-verified image bytes.
    """

    images = document.get("images", [])
    textures = document.get("textures", [])
    if not isinstance(images, list) or not isinstance(textures, list):
        raise ValueError("GLB visual texture resources are invalid")
    if document.get("samplers") not in (None, []):
        raise ValueError("GLB PBR sampling state does not match the fixed repeat/linear contract")
    used: Dict[Tuple[int, str], set[str]] = {}
    for material_index, material_id, zone_id in primitive_material_bindings:
        used.setdefault((material_index, material_id), set()).add(zone_id)
    if not used:
        raise ValueError("GLB has no material-zone PBR bindings")

    def number_matches(value: Any, expected: float) -> bool:
        return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(float(value)) and math.isclose(float(value), expected)

    def texture_map(texture_index: Any, role: str, expected_map: Any) -> Dict[str, Any]:
        if type(texture_index) is not int or texture_index < 0 or texture_index >= len(textures):
            raise ValueError("GLB PBR texture reference is invalid")
        texture = textures[texture_index]
        if not isinstance(texture, dict):
            raise ValueError("GLB texture entry is invalid")
        if set(texture) != {"name", "source"} or texture.get("name") != expected_map.texture_id:
            raise ValueError("GLB PBR sampling state does not match the fixed repeat/linear contract")
        image_index = texture.get("source")
        if type(image_index) is not int or image_index < 0 or image_index >= len(images):
            raise ValueError("GLB PBR texture does not embed an image")
        image = images[image_index]
        if not isinstance(image, dict) or "uri" in image or image.get("mimeType") != "image/png":
            raise ValueError("GLB PBR image must be embedded PNG data")
        if image.get("name") != expected_map.texture_id:
            raise ValueError("GLB PBR image identity does not match its built-in texture")
        view_index = image.get("bufferView")
        if type(view_index) is not int or view_index < 0 or view_index >= len(views):
            raise ValueError("GLB PBR image buffer view is invalid")
        view = views[view_index]
        if not isinstance(view, dict):
            raise ValueError("GLB PBR image view is invalid")
        buffer_index = view.get("buffer")
        offset = view.get("byteOffset", 0)
        length = view.get("byteLength")
        if type(buffer_index) is not int or buffer_index != 0:
            raise ValueError("GLB PBR image view must explicitly reference the embedded buffer")
        if "byteStride" in view:
            raise ValueError("GLB PBR image view cannot declare an accessor byteStride")
        if (
            type(offset) is not int
            or offset < 0
            or offset % 4 != 0
            or type(length) is not int
            or length <= 0
            or offset + length > len(binary)
        ):
            raise ValueError("GLB PBR image exceeds its binary buffer")
        payload = binary[offset:offset + length]
        width, height = _readback_png_dimensions(payload)
        extras = image.get("extras")
        metadata = extras.get("forgecad_visual_texture") if isinstance(extras, dict) else None
        if not isinstance(metadata, dict) or metadata.get("texture_role") != role:
            raise ValueError("GLB PBR image metadata does not match its channel")
        if metadata.get("mime_type") != "image/png" or metadata.get("sha256") != hashlib.sha256(payload).hexdigest():
            raise ValueError("GLB PBR image hash or mime metadata is invalid")
        if metadata.get("byte_size") != len(payload) or metadata.get("width") != width or metadata.get("height") != height:
            raise ValueError("GLB PBR image dimensions do not match metadata")
        expected_space = "srgb" if role in {"base_color", "emissive"} else "linear"
        if metadata.get("color_space") != expected_space:
            raise ValueError("GLB PBR image has an invalid colour space")
        if metadata.get("source") != "forgecad_builtin" or metadata.get("license") != "not_applicable":
            raise ValueError("GLB PBR image provenance is outside the built-in boundary")
        expected_metadata = expected_map.model_dump(mode="json")
        expected_payload = (
            surface_layer_visual_texture_png_bytes(
                normalized_surface_layer,
                artifact_profile_id=artifact_profile_id,
                texture_role=role,
            )
            if normalized_surface_layer is not None
            else surface_adornment_visual_texture_png_bytes(
                normalized_adornment,
                artifact_profile_id=artifact_profile_id,
                texture_role=role,
            )
            if normalized_adornment is not None
            else visual_texture_png_bytes(expected_map.texture_id)
        )
        if metadata != expected_metadata or payload != expected_payload:
            raise ValueError("GLB PBR image does not match the built-in texture truth")
        return {
            "texture_id": metadata.get("texture_id"),
            "texture_role": role,
            "mime_type": "image/png",
            "byte_size": len(payload),
            "sha256": metadata.get("sha256"),
            "color_space": metadata.get("color_space"),
            "width": width,
            "height": height,
            "source": metadata.get("source"),
            "license": metadata.get("license"),
            "fallback": metadata.get("fallback"),
            "glb_image_index": image_index,
            "glb_texture_index": texture_index,
        }

    results: List[Dict[str, Any]] = []
    visual_texture_contract_version: str | None = None
    for (material_index, material_id), zone_ids in sorted(used.items()):
        material = materials[material_index]
        if not isinstance(material, dict):
            raise ValueError("GLB material entry is invalid")
        extras = material.get("extras")
        texture_set_id = extras.get("forgecad_visual_texture_set_id") if isinstance(extras, dict) else None
        texture_material_id = extras.get("forgecad_texture_material_id") if isinstance(extras, dict) else None
        surface_layer = extras.get("forgecad_surface_layer_lowering") if isinstance(extras, dict) else None
        adornment = extras.get("forgecad_surface_adornment") if isinstance(extras, dict) else None
        normalized_adornment: dict[str, object] | None = None
        normalized_surface_layer: dict[str, object] | None = None
        if surface_layer is not None:
            if adornment is not None:
                raise ValueError("GLB material cannot mix retained surface layers with A005 material rows")
            normalized_surface_layer = normalize_surface_layer_lowering(surface_layer)
            expected_texture_set = surface_layer_visual_texture_set(
                normalized_surface_layer,
                artifact_profile_id=artifact_profile_id,
            )
            base_index, base_texture_material_id = builtin_visual_material_binding(
                str(normalized_surface_layer["adornments"][0]["base_material"])
            )
            expected_zone_id = str(normalized_surface_layer["adornments"][0]["target_zone_id"])
            if (
                material_index < builtin_visual_material_count()
                or material_id != expected_texture_set.material_id
                or texture_material_id != expected_texture_set.material_id
                or texture_set_id != expected_texture_set.visual_texture_set_id
                or extras.get("forgecad_base_material_id") != base_texture_material_id
                or extras.get("forgecad_surface_layer_lowering_sha256")
                != surface_layer_lowering_sha256(normalized_surface_layer)
                or extras.get("forgecad_surface_layer_retained_layers_sha256")
                != normalized_surface_layer["retained_layers_sha256"]
                or set(zone_ids) != {expected_zone_id}
            ):
                raise ValueError("GLB material does not match the retained surface layer PBR truth")
        elif adornment is None:
            try:
                expected_texture_set = builtin_visual_texture_set_for_readback(
                    material_index,
                    texture_set_id if isinstance(texture_set_id, str) else "",
                )
                expected_material_index, expected_texture_material_id = builtin_visual_material_binding(material_id)
            except ValueError as exc:
                raise ValueError("GLB material does not match the built-in VisualTextureSet identity") from exc
            if (
                expected_material_index != material_index
                or texture_material_id != expected_texture_material_id
                or texture_material_id != expected_texture_set.material_id
            ):
                raise ValueError("GLB material does not match the built-in VisualTextureSet identity")
        else:
            normalized_adornment = normalize_surface_adornment_program(adornment)
            expected_texture_set = surface_adornment_visual_texture_set(
                normalized_adornment,
                artifact_profile_id=artifact_profile_id,
            )
            base_index, base_texture_material_id = builtin_visual_material_binding(
                str(normalized_adornment["base_material"])
            )
            if (
                material_index < builtin_visual_material_count()
                or material_id != expected_texture_set.material_id
                or texture_material_id != expected_texture_set.material_id
                or texture_set_id != expected_texture_set.visual_texture_set_id
                or extras.get("forgecad_base_material_id") != base_texture_material_id
                or extras.get("forgecad_surface_adornment_sha256")
                != surface_adornment_program_sha256(normalized_adornment)
                or set(zone_ids) != {str(normalized_adornment["target_zone_id"])}
            ):
                raise ValueError("GLB material does not match the surface adornment PBR truth")
        if visual_texture_contract_version is None:
            visual_texture_contract_version = expected_texture_set.version
        elif expected_texture_set.version != visual_texture_contract_version:
            raise ValueError(
                "GLB materials mix incompatible built-in visual texture contract versions"
            )
        pbr = material.get("pbrMetallicRoughness")
        if not isinstance(pbr, dict):
            raise ValueError("GLB material lacks metallic-roughness PBR")

        def texture_info(
            container: Mapping[str, Any],
            field: str,
            *,
            extra_field: str | None = None,
        ) -> Mapping[str, Any]:
            info = container.get(field)
            expected_keys = {"index"} | ({extra_field} if extra_field else set())
            if not isinstance(info, dict):
                raise ValueError("GLB PBR texture reference is invalid")
            if set(info) != expected_keys:
                raise ValueError("GLB PBR sampling state does not match the fixed UV0 repeat/linear contract")
            if type(info.get("index")) is not int:
                raise ValueError("GLB PBR texture reference is invalid")
            return info

        base_color_info = texture_info(pbr, "baseColorTexture")
        metallic_roughness_info = texture_info(pbr, "metallicRoughnessTexture")
        normal_info = texture_info(material, "normalTexture", extra_field="scale")
        occlusion_info = texture_info(material, "occlusionTexture", extra_field="strength")
        emissive_info = texture_info(material, "emissiveTexture")
        refs = {
            "base_color": base_color_info["index"],
            "metallic_roughness": metallic_roughness_info["index"],
            "normal": normal_info["index"],
            "occlusion": occlusion_info["index"],
            "emissive": emissive_info["index"],
        }
        expected_maps = {item.texture_role: item for item in expected_texture_set.maps}
        maps = [
            texture_map(refs[role], role, expected_maps[role])
            for role in ("base_color", "metallic_roughness", "normal", "occlusion", "emissive")
        ]
        extensions = material.get("extensions", {})
        if not isinstance(extensions, dict):
            raise ValueError("GLB material extensions are invalid")
        allowed_extensions = {"KHR_materials_clearcoat", "KHR_materials_transmission", "KHR_materials_ior"}
        if any(key not in allowed_extensions for key in extensions):
            raise ValueError("GLB material uses an unsupported visual extension")
        if normalized_surface_layer is not None:
            base_index, _ = builtin_visual_material_binding(
                str(normalized_surface_layer["adornments"][0]["base_material"])
            )
            expected_properties = builtin_material_properties(base_index)
        elif normalized_adornment is None:
            expected_properties = builtin_material_properties(material_index)
        else:
            base_index, _ = builtin_visual_material_binding(
                str(normalized_adornment["base_material"])
            )
            expected_properties = builtin_material_properties(base_index)
        expected_base_factor = [1, 1, 1, float(expected_properties["alpha"])]
        normal_texture = material.get("normalTexture")
        occlusion_texture = material.get("occlusionTexture")
        if (
            pbr.get("baseColorFactor") != expected_base_factor
            or not number_matches(pbr.get("metallicFactor"), 1)
            or not number_matches(pbr.get("roughnessFactor"), 1)
            or not isinstance(normal_texture, dict)
            or not number_matches(normal_texture.get("scale"), float(expected_properties["normal_scale"]))
            or not isinstance(occlusion_texture, dict)
            or not number_matches(occlusion_texture.get("strength"), 1)
            or material.get("emissiveFactor") != [1, 1, 1]
        ):
            raise ValueError("GLB visual material parameters do not match the built-in PBR truth")
        expected_clearcoat = float(expected_properties["clearcoat"])
        clearcoat = extensions.get("KHR_materials_clearcoat")
        if expected_clearcoat > 0:
            clearcoat_roughness = (
                clearcoat.get("clearcoatRoughnessTexture")
                if isinstance(clearcoat, dict)
                else None
            )
            clearcoat_normal = (
                clearcoat.get("clearcoatNormalTexture")
                if isinstance(clearcoat, dict)
                else None
            )
            if (
                not isinstance(clearcoat, dict)
                or not number_matches(clearcoat.get("clearcoatFactor"), expected_clearcoat)
                or not number_matches(clearcoat.get("clearcoatRoughnessFactor"), 1)
                or not isinstance(clearcoat_roughness, dict)
                or set(clearcoat_roughness) != {"index"}
                or clearcoat_roughness.get("index")
                != refs["metallic_roughness"]
                or not isinstance(clearcoat_normal, dict)
                or set(clearcoat_normal) != {"index", "scale"}
                or clearcoat_normal.get("index") != refs["normal"]
                or not number_matches(
                    clearcoat_normal.get("scale"),
                    float(expected_properties["normal_scale"]),
                )
            ):
                raise ValueError("GLB clearcoat parameters do not match the built-in PBR truth")
        elif clearcoat is not None:
            raise ValueError("GLB clearcoat is not allowed for this built-in material")
        expected_transmission = float(expected_properties["transmission"])
        transmission = extensions.get("KHR_materials_transmission")
        ior = extensions.get("KHR_materials_ior")
        if expected_transmission > 0:
            if (
                not isinstance(transmission, dict)
                or not number_matches(transmission.get("transmissionFactor"), expected_transmission)
                or not isinstance(ior, dict)
                or not number_matches(ior.get("ior"), float(expected_properties["ior"]))
            ):
                raise ValueError("GLB transmission parameters do not match the built-in PBR truth")
        elif transmission is not None or ior is not None:
            raise ValueError("GLB transmission is not allowed for this built-in material")
        if "KHR_materials_transmission" in extensions:
            base_color_factor = pbr.get("baseColorFactor", [1, 1, 1, 1])
            if (
                "KHR_materials_ior" not in extensions
                or material.get("alphaMode", "OPAQUE") != "OPAQUE"
                or not isinstance(base_color_factor, list)
                or len(base_color_factor) != 4
                or not number_matches(base_color_factor[3], 1)
            ):
                raise ValueError("GLB transparent material compatibility metadata is incomplete")
        results.append({
            "schema_version": "VisualTextureSet@1",
            "visual_texture_set_id": texture_set_id,
            "material_id": material_id,
            "texture_material_id": texture_material_id,
            "material_index": material_index,
            "material_zone_ids": sorted(zone_ids),
            "maps": maps,
            "extensions": sorted(extensions),
            "texture_byte_size": sum(item["byte_size"] for item in maps),
            **(
                {
                    "surface_adornment": normalized_adornment,
                    "surface_adornment_sha256": surface_adornment_program_sha256(
                        normalized_adornment
                    ),
                }
                if normalized_adornment is not None
                else {}
            ),
            **(
                {
                    "surface_layer_lowering": normalized_surface_layer,
                    "surface_layer_lowering_sha256": surface_layer_lowering_sha256(
                        normalized_surface_layer
                    ),
                    "surface_layer_retained_layers_sha256": normalized_surface_layer[
                        "retained_layers_sha256"
                    ],
                }
                if normalized_surface_layer is not None
                else {}
            ),
        })
    return results


def _readback_png_dimensions(payload: bytes) -> Tuple[int, int]:
    if len(payload) < 24 or payload[:8] != b"\x89PNG\r\n\x1a\n" or payload[12:16] != b"IHDR":
        raise ValueError("GLB PBR image is not a valid PNG")
    width, height = struct.unpack(">II", payload[16:24])
    if not 0 < width <= 4096 or not 0 < height <= 4096:
        raise ValueError("GLB PBR image dimensions are outside budget")
    return width, height


def _read_visual_environment_facts(document: Mapping[str, Any]) -> Dict[str, Any]:
    extras = document.get("extras")
    environment = extras.get("forgecad_visual_environment") if isinstance(extras, dict) else None
    if not isinstance(environment, dict):
        raise ValueError("GLB visual environment is missing")
    expected = studio_environment_manifest()
    if environment != expected:
        raise ValueError("GLB visual environment does not match the fixed studio profile")
    # The raw GLB keeps the full renderer recipe so it can be audited against
    # the desktop implementation.  The stable public readback remains the
    # versioned ForgeCADVisualEnvironment@1 summary plus its hash; adding
    # renderer-internal fields must not silently widen that API contract.
    return {
        key: environment[key]
        for key in (
            "schema_version",
            "environment_id",
            "environment_kind",
            "environment_sha256",
            "source",
            "license",
            "color_workflow",
            "output_color_space",
            "tone_mapping",
            "tone_mapping_exposure",
            "contact_shadows",
            "pmrem",
        )
    }


def _read_geometry_artifact_profile_facts(
    document: Mapping[str, Any],
) -> Dict[str, Any]:
    extras = document.get("extras")
    profile = extras.get("forgecad_geometry_artifact_profile") if isinstance(extras, dict) else None
    if not isinstance(profile, dict):
        raise ValueError("GLB geometry artifact profile is missing")
    profile_id = profile.get("artifact_profile_id")
    if profile_id not in {"interactive_preview", "production_concept"}:
        raise ValueError("GLB geometry artifact profile identity is invalid")
    expected = geometry_artifact_profile_manifest(profile_id)
    if profile != expected:
        raise ValueError("GLB geometry artifact profile does not match the code-owned manifest")
    return dict(profile)


def _readback_primitive_topology(
    accessors: List[Any],
    views: List[Any],
    binary: bytes,
    position_accessor_index: int,
    index_accessor_index: int,
) -> Dict[str, Any]:
    vertices = [
        tuple(round(float(value), 8) for value in vertex)
        for vertex in _readback_float_accessor(
            accessors,
            views,
            binary,
            position_accessor_index,
            3,
        )
    ]
    index_accessor = _readback_accessor_record(accessors, index_accessor_index)
    if index_accessor.get("componentType") not in {5123, 5125}:
        raise ValueError("GLB topology readback requires uint16 or uint32 indices")
    indices = _readback_index_accessor(
        accessors,
        views,
        binary,
        index_accessor_index,
    )
    if any(index < 0 or index >= len(vertices) for index in indices):
        raise ValueError("GLB index references a missing POSITION vertex")
    edge_counts: Dict[Tuple[Tuple[float, float, float], Tuple[float, float, float]], int] = {}
    degenerate = 0
    for offset in range(0, len(indices), 3):
        triangle = [vertices[indices[offset + index]] for index in range(3)]
        if len(set(triangle)) != 3:
            degenerate += 1
            continue
        for left, right in ((triangle[0], triangle[1]), (triangle[1], triangle[2]), (triangle[2], triangle[0])):
            edge = tuple(sorted((left, right)))
            edge_counts[edge] = edge_counts.get(edge, 0) + 1
    boundary = sum(count == 1 for count in edge_counts.values())
    non_manifold = sum(count > 2 for count in edge_counts.values())
    return {
        "closed": degenerate == 0 and boundary == 0 and non_manifold == 0,
        "boundary_edge_count": boundary,
        "non_manifold_edge_count": non_manifold,
        "degenerate_triangle_count": degenerate,
    }


def _readback_accessor_record(accessors: List[Any], accessor_index: Any) -> Dict[str, Any]:
    if (
        type(accessor_index) is not int
        or accessor_index < 0
        or accessor_index >= len(accessors)
        or not isinstance(accessors[accessor_index], dict)
    ):
        raise ValueError("GLB accessor index is invalid")
    accessor = accessors[accessor_index]
    if accessor.get("sparse") is not None:
        raise ValueError("GLB sparse accessors are not supported by readback")
    return accessor


def _validate_static_readback_scene(document: Dict[str, Any], meshes: Any) -> None:
    """Freeze compiled ForgeCAD GLBs to one untransformed mesh instance."""

    scenes = document.get("scenes")
    nodes = document.get("nodes")
    active_scene = document.get("scene")
    if (
        not isinstance(meshes, list)
        or len(meshes) != 1
        or not isinstance(meshes[0], dict)
        or not isinstance(scenes, list)
        or len(scenes) != 1
        or not isinstance(scenes[0], dict)
        or type(active_scene) is not int
        or active_scene != 0
        or not isinstance(scenes[0].get("nodes"), list)
        or len(scenes[0]["nodes"]) != 1
        or type(scenes[0]["nodes"][0]) is not int
        or scenes[0]["nodes"][0] != 0
        or not isinstance(nodes, list)
        or len(nodes) != 1
        or not isinstance(nodes[0], dict)
        or type(nodes[0].get("mesh")) is not int
        or nodes[0]["mesh"] != 0
    ):
        raise ValueError("GLB must use the single-mesh static scene contract")
    if any(
        key in nodes[0]
        for key in (
            "children",
            "matrix",
            "rotation",
            "scale",
            "skin",
            "translation",
            "weights",
        )
    ) or "extensions" in nodes[0]:
        raise ValueError("GLB static scene node must be untransformed and uninstanced")


def _readback_accessor_layout(
    accessors: List[Any],
    views: List[Any],
    binary: bytes,
    accessor_index: Any,
    *,
    element_size: int,
    component_size: int,
) -> Tuple[Dict[str, Any], int, int, int]:
    """Resolve one tightly bounded accessor without reading outside its bufferView."""

    accessor = _readback_accessor_record(accessors, accessor_index)
    view_index = accessor.get("bufferView")
    if (
        type(view_index) is not int
        or view_index < 0
        or view_index >= len(views)
        or not isinstance(views[view_index], dict)
    ):
        raise ValueError("GLB accessor bufferView index is invalid")
    view = views[view_index]
    buffer_index = view.get("buffer")
    if type(buffer_index) is not int or buffer_index != 0:
        raise ValueError("GLB accessor references an unsupported buffer")

    count = accessor.get("count")
    accessor_offset = accessor.get("byteOffset", 0)
    view_offset = view.get("byteOffset", 0)
    view_length = view.get("byteLength")
    explicit_stride = view.get("byteStride")
    stride = element_size if explicit_stride is None else explicit_stride
    if type(count) is not int or count <= 0:
        raise ValueError("GLB accessor count is invalid")
    if (
        type(accessor_offset) is not int
        or accessor_offset < 0
        or type(view_offset) is not int
        or view_offset < 0
        or type(view_length) is not int
        or view_length < 0
    ):
        raise ValueError("GLB accessor buffer range is invalid")
    if explicit_stride is not None and (
        type(explicit_stride) is not int
        or explicit_stride < 4
        or explicit_stride > 252
        or explicit_stride % 4 != 0
    ):
        raise ValueError("GLB explicit byteStride is outside the glTF range")
    if (
        type(stride) is not int
        or stride < element_size
        or stride % component_size != 0
        or accessor_offset % component_size != 0
        or view_offset % component_size != 0
        or (view_offset + accessor_offset) % component_size != 0
    ):
        raise ValueError("GLB accessor byte stride or alignment is invalid")

    base = view_offset + accessor_offset
    view_end = view_offset + view_length
    end = base + (count - 1) * stride + element_size
    if view_end > len(binary):
        raise ValueError("GLB bufferView exceeds binary buffer")
    if base < view_offset or end > view_end:
        raise ValueError("GLB accessor exceeds its bufferView")
    return accessor, count, stride, base


def _readback_float_accessor(
    accessors: List[Any],
    views: List[Any],
    binary: bytes,
    accessor_index: int,
    component_count: int,
) -> List[Tuple[float, ...]]:
    accessor = _readback_accessor_record(accessors, accessor_index)
    if accessor.get("componentType") != 5126:
        raise ValueError("GLB float accessor has an unsupported component type")
    element_size = component_count * 4
    _accessor, count, stride, base = _readback_accessor_layout(
        accessors,
        views,
        binary,
        accessor_index,
        element_size=element_size,
        component_size=4,
    )
    return [
        tuple(float(value) for value in struct.unpack_from(f"<{component_count}f", binary, base + index * stride))
        for index in range(count)
    ]


def _readback_index_accessor(
    accessors: List[Any],
    views: List[Any],
    binary: bytes,
    accessor_index: int,
) -> List[int]:
    accessor = _readback_accessor_record(accessors, accessor_index)
    if accessor.get("type") != "SCALAR":
        raise ValueError("GLB integer accessor must be SCALAR")
    component_type = accessor.get("componentType")
    if type(component_type) is not int:
        raise ValueError("GLB integer accessor has an unsupported component type")
    format_code, byte_size = {5121: ("B", 1), 5123: ("H", 2), 5125: ("I", 4)}.get(
        component_type,
        (None, None),
    )
    if format_code is None or byte_size is None:
        raise ValueError("GLB integer accessor has an unsupported component type")
    _accessor, count, stride, base = _readback_accessor_layout(
        accessors,
        views,
        binary,
        accessor_index,
        element_size=byte_size,
        component_size=byte_size,
    )
    return [
        int(struct.unpack_from(f"<{format_code}", binary, base + index * stride)[0])
        for index in range(count)
    ]


def _readback_stable_face_id_accessor(
    accessors: List[Any],
    views: List[Any],
    binary: bytes,
    accessor_index: int,
) -> List[int]:
    """Read ForgeCAD's per-vertex face provenance from a standard float attribute.

    glTF restricts mesh vertex attributes to FLOAT, UNSIGNED_BYTE or
    UNSIGNED_SHORT component types.  The provenance IDs need to preserve the
    complete triangle range, so the compiler stores finite integral values in
    FLOAT accessors rather than emitting invalid UNSIGNED_INT attributes.
    """
    values = _readback_float_accessor(accessors, views, binary, accessor_index, 1)
    result: List[int] = []
    for (value,) in values:
        if not math.isfinite(value) or value < 0 or not value.is_integer():
            raise ValueError("GLB stable face provenance must contain non-negative integral floats")
        result.append(int(value))
    return result


def _validate_edge_finish_readback(value: Any) -> None:
    if not isinstance(value, dict) or set(value) != {
        "mode",
        "edge_set",
        "selected_edge_count",
        "radius_ratio",
        "subdivision_count",
    }:
        raise ValueError("GLB edge-finish provenance is invalid")
    mode = value.get("mode")
    edge_set = value.get("edge_set")
    selected = value.get("selected_edge_count")
    radius_ratio = value.get("radius_ratio")
    subdivisions = value.get("subdivision_count")
    if mode == "none":
        if edge_set != "none" or selected != 0 or radius_ratio != 0 or subdivisions != 0:
            raise ValueError("GLB empty edge finish reports geometry work")
        return
    if (
        mode != "bevel_approximation"
        or edge_set != "xz_perimeter"
        or selected != 4
        or not isinstance(radius_ratio, (int, float))
        or not 0 < float(radius_ratio) <= MAX_EDGE_FINISH_RADIUS_RATIO
        or not isinstance(subdivisions, int)
        or not 0 < subdivisions <= MAX_EDGE_FINISH_SUBDIVISIONS
    ):
        raise ValueError("GLB bevel approximation exceeds its edge-finish contract")


def inspect_imported_glb(payload: bytes) -> ImportedGlbInspection:
    """Validate a self-contained GLB before it enters the local library.

    The imported file is never executed.  The checks intentionally exclude
    external buffers/images and compressed mesh extensions because P0 keeps a
    predictable, local-only GLB path.  Bounds are reported in millimetres
    using glTF's metre convention.
    """
    if len(payload) < 20 or len(payload) > MAX_IMPORTED_GLB_BYTES:
        raise ValueError("导入 GLB 超出 20 B–32 MB 的轻量限制。")
    document, binary = _parse_glb_chunks(payload)
    if document.get("asset", {}).get("version") != "2.0":
        raise ValueError("只支持 glTF 2.0 GLB。")
    if any(extension in {"KHR_draco_mesh_compression", "EXT_meshopt_compression"} for extension in document.get("extensionsUsed", [])):
        raise ValueError("当前不支持压缩网格 GLB；请先导出为普通 glTF 2.0 GLB。")
    buffers = document.get("buffers")
    if not isinstance(buffers, list) or len(buffers) != 1 or not isinstance(buffers[0], dict):
        raise ValueError("导入 GLB 必须只包含一个内嵌二进制缓冲区。")
    if buffers[0].get("uri"):
        raise ValueError("导入 GLB 不能引用外部缓冲区。")
    if int(buffers[0].get("byteLength", -1)) > len(binary):
        raise ValueError("导入 GLB 的缓冲区长度无效。")
    images = document.get("images", [])
    if not isinstance(images, list) or any(isinstance(image, dict) and image.get("uri") for image in images):
        raise ValueError("导入 GLB 不能引用外部图片。")

    accessors = document.get("accessors")
    views = document.get("bufferViews")
    meshes = document.get("meshes")
    if not isinstance(accessors, list) or not isinstance(views, list) or not isinstance(meshes, list) or not meshes:
        raise ValueError("导入 GLB 缺少网格、访问器或缓冲视图。")
    triangle_count = 0
    primitive_count = 0
    minimum = [float("inf")] * 3
    maximum = [float("-inf")] * 3
    for mesh in meshes:
        if not isinstance(mesh, dict) or not isinstance(mesh.get("primitives"), list):
            raise ValueError("导入 GLB 的网格格式无效。")
        for primitive in mesh["primitives"]:
            if not isinstance(primitive, dict) or primitive.get("mode", 4) != 4:
                raise ValueError("当前只支持三角形 GLB 网格。")
            position_index = primitive.get("attributes", {}).get("POSITION") if isinstance(primitive.get("attributes"), dict) else None
            if not isinstance(position_index, int):
                raise ValueError("导入 GLB 网格缺少 POSITION。")
            position = _import_accessor(accessors, views, binary, position_index, label="POSITION")
            if position.get("componentType") != 5126 or position.get("type") != "VEC3" or int(position.get("count", 0)) <= 0:
                raise ValueError("导入 GLB 的 POSITION 访问器无效。")
            lower = position.get("min")
            upper = position.get("max")
            if not _finite_vec3(lower) or not _finite_vec3(upper):
                raise ValueError("导入 GLB 的 POSITION 必须包含有限 min/max。")
            for axis in range(3):
                minimum[axis] = min(minimum[axis], float(lower[axis]))
                maximum[axis] = max(maximum[axis], float(upper[axis]))
            index_index = primitive.get("indices")
            if index_index is None:
                index_count = int(position["count"])
            elif isinstance(index_index, int):
                indices = _import_accessor(accessors, views, binary, index_index, label="indices")
                if indices.get("type") != "SCALAR" or indices.get("componentType") not in {5121, 5123, 5125}:
                    raise ValueError("导入 GLB 的索引访问器无效。")
                index_count = int(indices.get("count", 0))
            else:
                raise ValueError("导入 GLB 的索引引用无效。")
            if index_count <= 0 or index_count % 3:
                raise ValueError("导入 GLB 的三角形索引数量无效。")
            triangle_count += index_count // 3
            primitive_count += 1
    if triangle_count > MAX_IMPORTED_GLB_TRIANGLES:
        raise ValueError(f"导入 GLB 超过 {MAX_IMPORTED_GLB_TRIANGLES:,} 三角形轻量预算。")
    if primitive_count == 0:
        raise ValueError("导入 GLB 没有可显示的三角形网格。")
    return ImportedGlbInspection(
        sha256=hashlib.sha256(payload).hexdigest(),
        byte_size=len(payload),
        triangle_count=triangle_count,
        bounds_mm=[round((maximum[index] - minimum[index]) * 1000, 4) for index in range(3)],
        mesh_count=len(meshes),
        primitive_count=primitive_count,
        material_count=len(document.get("materials", [])) if isinstance(document.get("materials", []), list) else 0,
        node_count=len(document.get("nodes", [])) if isinstance(document.get("nodes", []), list) else 0,
    )


def _parse_glb_chunks(payload: bytes) -> Tuple[Dict[str, Any], bytes]:
    magic, version, declared_length = struct.unpack_from("<4sII", payload, 0)
    if magic != b"glTF" or version != 2 or declared_length != len(payload):
        raise ValueError("GLB 头部无效。")
    offset = 12
    document: Dict[str, Any] | None = None
    binary = b""
    while offset + 8 <= len(payload):
        length, chunk_type = struct.unpack_from("<II", payload, offset)
        offset += 8
        end = offset + length
        if end > len(payload):
            raise ValueError("GLB 分块超出文件长度。")
        if chunk_type == 0x4E4F534A:
            if document is not None:
                raise ValueError("GLB 包含多个 JSON 分块。")
            decoded = json.loads(payload[offset:end].rstrip(b" \x00").decode("utf-8"))
            if not isinstance(decoded, dict):
                raise ValueError("GLB JSON 根对象无效。")
            document = decoded
        elif chunk_type == 0x004E4942:
            if binary:
                raise ValueError("GLB 包含多个二进制分块。")
            binary = payload[offset:end]
        offset = end
    if offset != len(payload) or document is None or not binary:
        raise ValueError("GLB 缺少 JSON 或二进制分块。")
    return document, binary


def _import_accessor(accessors: List[Any], views: List[Any], binary: bytes, accessor_index: int, *, label: str) -> Dict[str, Any]:
    if accessor_index < 0 or accessor_index >= len(accessors) or not isinstance(accessors[accessor_index], dict):
        raise ValueError(f"导入 GLB 的 {label} 访问器引用无效。")
    accessor = accessors[accessor_index]
    if accessor.get("sparse"):
        raise ValueError("当前不支持 sparse GLB 访问器。")
    view_index = accessor.get("bufferView")
    if not isinstance(view_index, int) or view_index < 0 or view_index >= len(views) or not isinstance(views[view_index], dict):
        raise ValueError(f"导入 GLB 的 {label} 缺少缓冲视图。")
    view = views[view_index]
    if view.get("buffer", 0) != 0:
        raise ValueError(f"导入 GLB 的 {label} 引用了外部缓冲区。")
    component_sizes = {5120: 1, 5121: 1, 5122: 2, 5123: 2, 5125: 4, 5126: 4}
    component_count = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}
    component_type = accessor.get("componentType")
    value_type = accessor.get("type")
    count = accessor.get("count")
    if component_type not in component_sizes or value_type not in component_count or not isinstance(count, int) or count <= 0:
        raise ValueError(f"导入 GLB 的 {label} 访问器格式无效。")
    element_size = component_sizes[component_type] * component_count[value_type]
    stride = view.get("byteStride", element_size)
    if not isinstance(stride, int) or stride < element_size:
        raise ValueError(f"导入 GLB 的 {label} 步长无效。")
    accessor_offset = accessor.get("byteOffset", 0)
    view_offset = view.get("byteOffset", 0)
    view_length = view.get("byteLength")
    if not isinstance(accessor_offset, int) or accessor_offset < 0 or not isinstance(view_offset, int) or view_offset < 0 or not isinstance(view_length, int) or view_length < 0:
        raise ValueError(f"导入 GLB 的 {label} 缓冲范围无效。")
    required = accessor_offset + (count - 1) * stride + element_size
    if required > view_length or view_offset + required > len(binary):
        raise ValueError(f"导入 GLB 的 {label} 访问器超出缓冲区。")
    return accessor


def _finite_vec3(value: Any) -> bool:
    return isinstance(value, list) and len(value) == 3 and all(isinstance(item, (int, float)) and math.isfinite(float(item)) for item in value)


def _material_index(material_id: str) -> int:
    return builtin_visual_material_binding(material_id or None)[0]


_BOUND_SCALE_PATHS: Tuple[Tuple[str, str], ...] = (
    ("transform.scale.x", "长度比例"),
    ("transform.scale.y", "高度比例"),
    ("transform.scale.z", "宽度比例"),
)


def _editable_parameter_bindings_for_part(
    *,
    part_id: str,
    primitive: BoxPrimitive,
    role_count: int,
) -> List[Dict[str, Any]]:
    """Return declarations only for one concrete generated ShapeProgram output.

    The current ChangeSet adapter resolves ShapeProgram operations by stable
    role. Repeated roles (such as paired wheels or nacelles) would update more
    than one primitive, so they deliberately remain undeclared until a future
    operation-level editing contract exists. Cylinders and capsules likewise
    remain undeclared: their current adapter has radius/height semantics, not
    three independent size axes. Boxes and wedges map directly to `args.size`.
    """
    if primitive.part_role.startswith("visual_") or role_count != 1 or primitive.primitive_kind not in {"box", "wedge"}:
        return []
    parameter_stem = part_id.removeprefix("part_")
    return [
        {
            "schema_version": "EditableParameterBinding@1",
            "parameter_id": f"editparam_{parameter_stem}_{path.rsplit('.', 1)[1]}",
            "path": path,
            "display_name": display_name,
            "unit": "ratio",
            "default": 1.0,
            "min": 0.6,
            "max": 1.4,
            "step": 0.1,
        }
        for path, display_name in _BOUND_SCALE_PATHS
    ]


def segment_blockout(
    plan: MechanicalConceptPlan,
    direction_id: str,
    variant_id: str | None = None,
    presentation_profile: str = "quick_sketch",
) -> List[Dict[str, Any]]:
    """Return deterministic part candidates for a generated blockout.

    Segmentation is deliberately a candidate operation: roles come from the
    domain pack's deterministic silhouette template, while every part remains
    editable and uncommitted until a later ChangeSet confirmation.
    """
    direction = next((item for item in plan.directions if item.direction_id == direction_id), None)
    if direction is None:
        raise ValueError(f"direction not found: {direction_id}")
    if variant_id is not None and variant_id not in BLOCKOUT_VARIANT_IDS.get(plan.domain_pack_id, ()):
        raise ValueError(f"unknown blockout variant {variant_id!r} for {plan.domain_pack_id}")
    if presentation_profile not in {"quick_sketch", "showcase"}:
        raise ValueError("presentation_profile must be quick_sketch or showcase")
    boxes = _presentation_primitives(
        _boxes_for_domain(plan.domain_pack_id, direction.silhouette, variant_id),
        plan=plan,
        direction_id=direction_id,
        presentation_profile=presentation_profile,
    )
    candidates: List[Dict[str, Any]] = []
    role_counts = {role: sum(item.part_role == role for item in boxes) for role in {item.part_role for item in boxes}}
    for index, box in enumerate(boxes):
        part_id = f"part_{index + 1}_{box.part_role}"
        is_visual_detail = box.part_role.startswith("visual_")
        parent_index = (
            index - 1
            if plan.domain_pack_id == "pack_robotic_arm_concept" and index > 0 and not is_visual_detail
            else 0
        )
        parent_part_id = None if index == 0 else f"part_{parent_index + 1}_{boxes[parent_index].part_role}"
        parameter_bindings = _editable_parameter_bindings_for_part(
            part_id=part_id,
            primitive=box,
            role_count=role_counts[box.part_role],
        )
        candidates.append(
            {
                "part_id": part_id,
                "role": box.part_role,
                "parent_part_id": parent_part_id,
                "position_mm": [round(value, 4) for value in box.center_mm],
                "size_mm": [round(value, 4) for value in box.size_mm],
                "material_zone_ids": [f"zone_{box.part_role}"],
                "editable_parameters": [
                    *(item["path"] for item in parameter_bindings),
                    *(
                        ["joint.rotation"]
                        if plan.domain_pack_id == "pack_robotic_arm_concept" and index > 0 and not is_visual_detail
                        else []
                    ),
                ],
                "editable_parameter_bindings": parameter_bindings,
                "locked": False,
                "provenance": "agent_generated",
            }
        )
    return candidates


def _boxes_for_domain(pack_id: str, silhouette: str, variant_id: str | None = None) -> List[BoxPrimitive]:
    if variant_id is not None:
        return _variant_boxes_for_domain(pack_id, variant_id)
    scale = {"compact": 0.88, "balanced": 1.0, "extended": 1.18, "industrial": 1.08, "organic": 1.0}.get(silhouette, 1.0)
    if pack_id == "pack_vehicle_concept":
        return [
            _box("body_shell", (0, 620, 0), (2200 * scale, 520 * scale, 900 * scale), 0),
            _box("cabin", (180, 1020, 0), (1100 * scale, 520 * scale, 760 * scale), 1),
            _cylinder("wheel_or_track", (-760, 340, -520), 180, 220, 2, (0, 0, 1)),
            _cylinder("wheel_or_track", (760, 340, -520), 180, 220, 2, (0, 0, 1)),
            _cylinder("wheel_or_track", (-760, 340, 520), 180, 220, 2, (0, 0, 1)),
            _cylinder("wheel_or_track", (760, 340, 520), 180, 220, 2, (0, 0, 1)),
            _box("lighting", (-1090, 700, 0), (80, 180, 420), 2),
        ]
    if pack_id == "pack_aircraft_concept":
        return [
            _box("fuselage", (0, 600, 0), (2800 * scale, 500 * scale, 520 * scale), 0),
            _box("cockpit_canopy", (-720, 850, 0), (650, 360, 460), 1),
            _box("main_wing", (160, 600, 0), (1250 * scale, 120, 2400 * scale), 1),
            _box("tail_surface", (1080, 820, 0), (520, 300, 1000), 2),
            _cylinder("nacelle", (-120, 360, -690), 160, 800, 2, (1, 0, 0)),
            _cylinder("nacelle", (-120, 360, 690), 160, 800, 2, (1, 0, 0)),
        ]
    if pack_id == "pack_robotic_arm_concept":
        return [
            _box("base", (0, 250, 0), (700 * scale, 500, 700 * scale), 0),
            _cylinder("shoulder_joint", (0, 760, 0), 260, 380, 2),
            _box("upper_link", (0, 1350, 0), (360, 1050 * scale, 360), 1),
            _cylinder("elbow_joint", (0, 1950, 0), 225, 380, 2),
            _box("forearm_link", (0, 2480, 0), (300, 900 * scale, 300), 1),
            _cylinder("wrist_joint", (0, 3000, 0), 180, 300, 2),
            _box("end_effector", (0, 3300, 0), (500, 380, 500), 0),
        ]
    return [
        _box("primary_body", (0, 620, 0), (1900 * scale, 520, 720 * scale), 0),
        _box("secondary_body", (-850, 720, 0), (600, 420, 620), 1),
        _box("mobility", (0, 240, 0), (650, 380, 520), 2),
        _box("trim", (300, 900, 380), (700, 90, 60), 2),
        _box("transparent", (350, 930, 0), (450, 180, 360), 1),
    ]


def _presentation_primitives(
    boxes: List[BoxPrimitive],
    *,
    plan: MechanicalConceptPlan,
    direction_id: str,
    presentation_profile: str,
) -> List[BoxPrimitive]:
    """Append deterministic, non-functional exterior layers for showcase mode.

    The same bounded primitive list feeds ShapeProgram, GLB, AssemblyGraph and
    segmentation.  It therefore cannot create a more detailed saved asset
    than the preview the user accepted.  These are visual shells and accents,
    never derived mechanisms, manufacturing features or material claims.
    """
    if presentation_profile == "quick_sketch":
        return list(boxes)
    intent = visual_intent_for_direction(
        plan.spec.get("visual_intent_mapping") if isinstance(plan.spec, dict) else None,
        domain_pack_id=plan.domain_pack_id,
        direction_id=direction_id,
    )
    detail_level = {"simple": 2, "medium": 3, "dense": 4}.get(
        intent.detail_density if intent else "medium",
        3,
    )
    primary = _showcase_primary_anchor(boxes, plan.domain_pack_id)
    if plan.domain_pack_id == "pack_future_weapon_prop":
        details = _future_prop_showcase_details(primary, detail_level)
    elif plan.domain_pack_id == "pack_vehicle_concept":
        details = _vehicle_showcase_details(primary, detail_level, boxes)
    elif plan.domain_pack_id == "pack_aircraft_concept":
        details = _aircraft_showcase_details(primary, detail_level, boxes)
    elif plan.domain_pack_id == "pack_robotic_arm_concept":
        details = _robotic_arm_showcase_details(primary, detail_level, boxes)
    else:
        raise ValueError(f"showcase detail grammar is not defined for {plan.domain_pack_id}")
    fairings = _showcase_connection_fairings(boxes, plan.domain_pack_id)
    return [*boxes, *fairings, *details]


_SHOWCASE_PRIMARY_ROLE_WHITELIST: Mapping[str, Tuple[str, ...]] = {
    "pack_future_weapon_prop": (
        "primary_body", "prop_core", "prop_capsule_body", "prop_front_shell",
        "prop_long_body", "prop_body_spine", "prop_heavy_body", "prop_energy_body",
        "prop_energy_handle", "prop_drone_body",
    ),
    "pack_vehicle_concept": (
        "body_shell", "vehicle_chassis", "vehicle_body", "rover_chassis", "rover_front",
        "crawler_chassis", "racer_front", "racer_delta_body", "racer_capsule_body",
        "carrier_chassis", "hauler_base", "rescue_bus_body",
    ),
    "pack_aircraft_concept": (
        "fuselage", "airframe_core", "tilt_body", "lift_fan_body", "jet_spine",
        "needle_fuselage", "interceptor_body", "cargo_fuselage", "passenger_fuselage",
        "heavy_lifter_body", "scout_flying_wing", "scout_body", "scout_capsule",
    ),
    "pack_robotic_arm_concept": (
        "base", "precision_base", "desktop_base", "rail_base", "handler_base",
        "handler_pedestal", "welding_base", "maintenance_base", "telescopic_base",
        "inspection_base", "service_mobile_base", "carousel_base", "service_pedestal",
    ),
}


def _showcase_primary_anchor(boxes: Sequence[BoxPrimitive], domain_pack_id: str) -> BoxPrimitive:
    """Resolve one reviewed primary role; never fall back to primitive order."""

    allowed_roles = _SHOWCASE_PRIMARY_ROLE_WHITELIST.get(domain_pack_id)
    if allowed_roles is None:
        raise ValueError(f"showcase anchor whitelist is not defined for {domain_pack_id}")
    matches = [item for item in boxes if item.part_role in allowed_roles]
    if len(matches) != 1:
        raise ValueError(
            f"showcase requires exactly one whitelisted primary role for {domain_pack_id}; "
            f"found {[item.part_role for item in matches]}"
        )
    return matches[0]


def _primitive_display_extent(primitive: BoxPrimitive) -> Tuple[float, float, float]:
    """Return axis-aware dimensions for deterministic visual-layer placement."""

    if primitive.primitive_kind not in {"cylinder", "capsule"}:
        return primitive.size_mm
    dimensions = [primitive.radius_mm * 2] * 3
    dominant = max(range(3), key=lambda index: abs(primitive.axis[index]))
    dimensions[dominant] = primitive.height_mm
    return (dimensions[0], dimensions[1], dimensions[2])


def _detail_thickness(width: float, height: float, depth: float) -> float:
    return max(10.0, min(24.0, min(width, height, depth) * 0.06))


def _exact_showcase_roles(
    boxes: Sequence[BoxPrimitive],
    *,
    domain_pack_id: str,
    required_roles: Sequence[str],
) -> Mapping[str, BoxPrimitive] | None:
    """Resolve one bounded showcase fixture without primitive-order fallback.

    The first role is the fixture-specific trigger.  Once it is present, a
    partial or duplicated match is rejected so a renamed catalog part cannot
    silently drop a connection layer from the GLB presented for review.
    """

    matches = {
        role: [primitive for primitive in boxes if primitive.part_role == role]
        for role in required_roles
    }
    present = {role for role, items in matches.items() if items}
    if not matches[required_roles[0]]:
        return None
    expected = set(required_roles)
    if present != expected or any(len(matches[role]) != 1 for role in required_roles):
        raise ValueError(
            f"showcase connection roles are incomplete for {domain_pack_id}; "
            f"expected {sorted(expected)}, found {sorted(present)}"
        )
    return {role: matches[role][0] for role in required_roles}


def _showcase_connection_fairings(
    boxes: Sequence[BoxPrimitive],
    domain_pack_id: str,
) -> List[BoxPrimitive]:
    """Add non-functional exterior bridges to reviewed M108 fixtures.

    The fairings only improve visual continuity between already-present parts.
    They do not add joints, mechanisms or manufacturing meaning, and they use
    the same bounded primitive/GLB/readback path as every other showcase part.
    """

    if domain_pack_id == "pack_future_weapon_prop":
        roles = _exact_showcase_roles(
            boxes,
            domain_pack_id=domain_pack_id,
            required_roles=("prop_core", "prop_grip"),
        )
        if roles is None:
            return []
        core = roles["prop_core"]
        grip = roles["prop_grip"]
        core_height = _primitive_display_extent(core)[1]
        grip_extent = _primitive_display_extent(grip)
        grip_visual_radius = max(
            grip.radius_mm,
            min(grip_extent[0], grip_extent[2]) / 2,
        )
        collar_height = 64.0
        return [
            _cylinder(
                "visual_guard_prop_mount_collar",
                (
                    grip.center_mm[0],
                    core.center_mm[1] - core_height / 2 - collar_height / 2 + 16.0,
                    grip.center_mm[2],
                ),
                grip_visual_radius * 1.15,
                collar_height,
                1,
                (0.0, 1.0, 0.0),
                material_id="mat_aluminum",
            )
        ]

    if domain_pack_id == "pack_vehicle_concept":
        wheel_roles = (
            "vehicle_wheel_fl",
            "vehicle_wheel_fr",
            "vehicle_wheel_rl",
            "vehicle_wheel_rr",
        )
        roles = _exact_showcase_roles(
            boxes,
            domain_pack_id=domain_pack_id,
            required_roles=("vehicle_chassis", *wheel_roles),
        )
        if roles is None:
            return []
        chassis = roles["vehicle_chassis"]
        depth = _primitive_display_extent(chassis)[2]
        fairings: List[BoxPrimitive] = []
        for side, front_role, rear_role in (
            ("left", "vehicle_wheel_fl", "vehicle_wheel_rl"),
            ("right", "vehicle_wheel_fr", "vehicle_wheel_rr"),
        ):
            front = roles[front_role]
            rear = roles[rear_role]
            wheel_radius = min(front.radius_mm, rear.radius_mm)
            thickness = max(64.0, depth * 0.07)
            center_z = (front.center_mm[2] + rear.center_mm[2]) / 2
            center_z -= math.copysign(thickness * 0.72, center_z)
            fairings.append(
                _box(
                    f"visual_guard_vehicle_side_bridge_{side}",
                    (
                        (front.center_mm[0] + rear.center_mm[0]) / 2,
                        (front.center_mm[1] + rear.center_mm[1]) / 2 + wheel_radius * 0.55,
                        center_z,
                    ),
                    (
                        abs(front.center_mm[0] - rear.center_mm[0]) + wheel_radius * 0.7,
                        wheel_radius * 0.38,
                        thickness,
                    ),
                    7,
                    material_id="mat_automotive_paint",
                )
            )
        return fairings

    if domain_pack_id == "pack_aircraft_concept":
        rotor_roles = (
            "lift_rotor_front_left",
            "lift_rotor_front_right",
            "lift_rotor_rear_left",
            "lift_rotor_rear_right",
        )
        roles = _exact_showcase_roles(
            boxes,
            domain_pack_id=domain_pack_id,
            required_roles=("lift_wing_left", "lift_wing_right", *rotor_roles),
        )
        if roles is not None:
            fairings = []
            for rotor_role in rotor_roles:
                side = "left" if rotor_role.endswith("left") else "right"
                wing = roles[f"lift_wing_{side}"]
                rotor = roles[rotor_role]
                wing_thickness = _primitive_display_extent(wing)[1]
                rotor_span = max(rotor.radius_mm, 150.0)
                side_sign = math.copysign(1.0, rotor.center_mm[2])
                wing_attach = (
                    wing.center_mm[0],
                    wing.center_mm[1] + wing_thickness * 0.04,
                    rotor.center_mm[2] - side_sign * rotor_span * 0.22,
                )
                rotor_attach = (
                    rotor.center_mm[0],
                    rotor.center_mm[1] - rotor.height_mm * 0.08,
                    rotor.center_mm[2],
                )
                center = tuple(
                    (wing_attach[axis] + rotor_attach[axis]) / 2
                    for axis in range(3)
                )
                relative_start = tuple(
                    wing_attach[axis] - center[axis] for axis in range(3)
                )
                relative_end = tuple(
                    rotor_attach[axis] - center[axis] for axis in range(3)
                )
                fairings.append(
                    _showcase_sweep_tube(
                        f"visual_guard_aircraft_rotor_pylon_{rotor_role.removeprefix('lift_rotor_')}",
                        center,
                        path_points=(
                            relative_start,
                            tuple(
                                (relative_start[axis] + relative_end[axis]) / 2
                                + (8.0 if axis == 1 else 0.0)
                                for axis in range(3)
                            ),
                            relative_end,
                        ),
                        profile_scale=(18.0, 42.0),
                        material=0,
                        material_id="mat_primary",
                    )
                )
            return fairings

        tilt_roles = _exact_showcase_roles(
            boxes,
            domain_pack_id=domain_pack_id,
            required_roles=("tilt_body", "tilt_pod_left", "tilt_pod_right"),
        )
        if tilt_roles is not None:
            body = tilt_roles["tilt_body"]
            fairings = []
            for side in ("left", "right"):
                pod = tilt_roles[f"tilt_pod_{side}"]
                body_extent = _primitive_display_extent(body)
                pod_extent = _primitive_display_extent(pod)
                body_edge_z = body.center_mm[2] + math.copysign(body_extent[2] / 2, pod.center_mm[2])
                pod_edge_z = pod.center_mm[2] - math.copysign(pod_extent[2] / 2, pod.center_mm[2])
                bridge_depth = abs(pod_edge_z - body_edge_z) + 80.0
                fairings.append(
                    _box(
                        f"visual_guard_aircraft_tilt_pod_bridge_{side}",
                        (
                            pod.center_mm[0],
                            (body.center_mm[1] + pod.center_mm[1]) / 2,
                            (body_edge_z + pod_edge_z) / 2,
                        ),
                        (
                            min(500.0, body_extent[0] * 0.24),
                            min(body_extent[1], pod_extent[1]) * 0.5,
                            bridge_depth,
                        ),
                        3,
                        material_id="mat_composite",
                    )
                )
            return fairings
        return []

    if domain_pack_id == "pack_robotic_arm_concept":
        roles = _exact_showcase_roles(
            boxes,
            domain_pack_id=domain_pack_id,
            required_roles=("precision_joint_1", "precision_link_1"),
        )
        if roles is not None:
            joint = roles["precision_joint_1"]
            link = roles["precision_link_1"]
            return [
                _box(
                    "visual_guard_robot_shoulder_bridge",
                    (
                        (joint.center_mm[0] + link.center_mm[0]) / 2,
                        joint.center_mm[1] + 70.0,
                        (joint.center_mm[2] + link.center_mm[2]) / 2
                        + joint.radius_mm * 0.45,
                    ),
                    (
                        abs(joint.center_mm[0] - link.center_mm[0]) + 80.0,
                        140.0,
                        min(joint.radius_mm * 2, link.radius_mm * 2) * 0.75,
                    ),
                    3,
                    material_id="mat_composite",
                )
            ]

        desktop_roles = _exact_showcase_roles(
            boxes,
            domain_pack_id=domain_pack_id,
            required_roles=("desktop_wrist", "desktop_tool"),
        )
        if desktop_roles is not None:
            wrist = desktop_roles["desktop_wrist"]
            tool = desktop_roles["desktop_tool"]
            wrist_extent = _primitive_display_extent(wrist)
            tool_extent = _primitive_display_extent(tool)
            wrist_top = wrist.center_mm[1] + wrist_extent[1] / 2
            tool_bottom = tool.center_mm[1] - tool_extent[1] / 2
            return [
                _cylinder(
                    "visual_guard_robot_wrist_bridge",
                    (
                        wrist.center_mm[0],
                        (wrist_top + tool_bottom) / 2,
                        wrist.center_mm[2],
                    ),
                    min(wrist.radius_mm * 0.82, tool_extent[0] * 0.36, tool_extent[2] * 0.36),
                    abs(tool_bottom - wrist_top) + 40.0,
                    3,
                    (0.0, 1.0, 0.0),
                    material_id="mat_aluminum",
                )
            ]

        rail_roles = _exact_showcase_roles(
            boxes,
            domain_pack_id=domain_pack_id,
            required_roles=("rail_base", "rail_carriage", "rail_pivot"),
        )
        if rail_roles is not None:
            base = rail_roles["rail_base"]
            carriage = rail_roles["rail_carriage"]
            pivot = rail_roles["rail_pivot"]
            base_extent = _primitive_display_extent(base)
            carriage_extent = _primitive_display_extent(carriage)
            pivot_extent = _primitive_display_extent(pivot)
            base_top = base.center_mm[1] + base_extent[1] / 2
            carriage_bottom = carriage.center_mm[1] - carriage_extent[1] / 2
            carriage_top = carriage.center_mm[1] + carriage_extent[1] / 2
            pivot_bottom = pivot.center_mm[1] - pivot_extent[1] / 2
            return [
                _box(
                    "visual_guard_robot_rail_bridge",
                    (
                        carriage.center_mm[0],
                        (base_top + carriage_bottom) / 2,
                        carriage.center_mm[2],
                    ),
                    (
                        carriage_extent[0] * 0.8,
                        abs(carriage_bottom - base_top) + 40.0,
                        min(base_extent[2], carriage_extent[2]) * 0.8,
                    ),
                    3,
                    material_id="mat_composite",
                ),
                _cylinder(
                    "visual_guard_robot_carriage_bridge",
                    (
                        pivot.center_mm[0],
                        (carriage_top + pivot_bottom) / 2,
                        pivot.center_mm[2],
                    ),
                    min(pivot.radius_mm * 0.82, carriage_extent[0] * 0.36, carriage_extent[2] * 0.36),
                    abs(pivot_bottom - carriage_top) + 40.0,
                    3,
                    (0.0, 1.0, 0.0),
                    material_id="mat_composite",
                ),
            ]
        return []

    raise ValueError(f"showcase connection grammar is not defined for {domain_pack_id}")


def _future_prop_showcase_details(primary: BoxPrimitive, detail_level: int) -> List[BoxPrimitive]:
    """Non-functional compact prop language with layered side surfacing.

    The fixed accents borrow hard-surface product-design cues without encoding
    a real mechanism: one shallow side cassette, a bounded sweep highlight,
    staggered visual vents and a small color badge.  They remain decorative
    display geometry and do not imply a bore, action, magazine or performance.
    """

    cx, cy, cz = primary.center_mm
    width, height, depth = _primitive_display_extent(primary)
    thickness = _detail_thickness(width, height, depth)
    side_surface = cz + depth / 2 + thickness / 2 - 6.0
    details = [
        _box(
            "visual_panel_prop_dorsal",
            (cx + width * 0.05, cy + height / 2 + thickness / 2 - 7.0, cz),
            (width * 0.47, thickness, depth * 0.28),
            3,
            material_id="mat_composite",
        ),
        _box(
            "visual_panel_prop_side",
            (cx - width * 0.1, cy - height * 0.02, side_surface),
            (width * 0.44, height * 0.34, thickness),
            3,
            material_id="mat_composite",
        ),
        _showcase_sweep_tube(
            "visual_groove_prop_flowline",
            (cx, cy, side_surface),
            path_points=(
                (-width * 0.34, height * 0.2, 0.0),
                (-width * 0.18, height * 0.13, 0.0),
                (0.0, height * 0.08, 0.0),
                (width * 0.2, height * 0.12, 0.0),
                (width * 0.33, height * 0.22, 0.0),
            ),
            profile_scale=(5.0, 7.0),
            material=1,
            material_id="mat_aluminum",
        ),
        _box(
            "visual_guard_prop_rear",
            (
                cx + width * 0.3,
                cy + height * 0.02,
                side_surface,
            ),
            (width * 0.13, height * 0.22, thickness),
            1,
            material_id="mat_aluminum",
        ),
        _box(
            "visual_light_strip_prop_side",
            (cx - width * 0.3, cy + height * 0.15, side_surface),
            (width * 0.2, max(12.0, height * 0.045), thickness),
            6,
            material_id="mat_emissive_blue",
        ),
        _box(
            "visual_cable_slot_prop_lower",
            (cx + width * 0.03, cy - height * 0.25, side_surface),
            (width * 0.3, max(12.0, height * 0.045), thickness),
            0,
            material_id="mat_rubber",
        ),
    ]
    vent_size = (
        max(44.0, width * 0.04),
        max(14.0, height * 0.045),
        thickness,
    )
    for index, (x_ratio, y_ratio) in enumerate(
        ((0.19, -0.09), (0.245, -0.055), (0.3, -0.02), (0.355, 0.015)),
        1,
    ):
        details.append(
            _wedge(
                f"visual_vent_prop_{index}",
                (cx + width * x_ratio, cy + height * y_ratio, side_surface),
                vent_size,
                0,
                material_id="mat_rubber",
            )
        )
    fastener_radius = max(8.0, min(13.0, min(width, depth) * 0.022))
    for index, x_offset in enumerate((-width * 0.2, width * 0.23), 1):
        details.append(
            _cylinder(
                f"visual_fastener_prop_{index}",
                (cx + x_offset, cy + height / 2 + thickness / 2 - 7.0, cz),
                fastener_radius,
                thickness,
                1,
                (0.0, 1.0, 0.0),
                material_id="mat_aluminum",
            )
        )
    if detail_level >= 4:
        details.append(
            _box(
                "visual_panel_prop_lower",
                (cx + width * 0.26, cy - height * 0.24, side_surface),
                (width * 0.1, max(14.0, height * 0.05), thickness),
                2,
                material_id="mat_signal_red",
            )
        )
    return details


def _vehicle_showcase_details(
    primary: BoxPrimitive,
    detail_level: int,
    boxes: Sequence[BoxPrimitive],
) -> List[BoxPrimitive]:
    """Vehicle shell language: hood/rocker layers, front lighting and top vents."""

    cx, cy, cz = primary.center_mm
    width, height, depth = _primitive_display_extent(primary)
    thickness = _detail_thickness(width, height, depth)
    anchors = _exact_showcase_roles(
        boxes,
        domain_pack_id="pack_vehicle_concept",
        required_roles=("vehicle_nose", "vehicle_cabin"),
    )
    if anchors is None:
        paint_panel = _box(
            "visual_panel_vehicle_paint",
            (cx - width * 0.1, cy + height * 0.44, cz),
            (width * 0.18, thickness, depth * 0.52),
            7,
            material_id="mat_automotive_paint",
        )
        deck_panel = _box(
            "visual_panel_vehicle_deck",
            (cx + width * 0.28, cy + height * 0.335, cz),
            (width * 0.14, thickness, depth * 0.44),
            3,
            material_id="mat_composite",
        )
    else:
        nose = anchors["vehicle_nose"]
        cabin = anchors["vehicle_cabin"]
        nose_extent = _primitive_display_extent(nose)
        cabin_extent = _primitive_display_extent(cabin)
        paint_panel = _box(
            "visual_panel_vehicle_paint",
            (
                nose.center_mm[0],
                nose.center_mm[1] + nose_extent[1] / 2 + thickness / 2 - 2.0,
                nose.center_mm[2],
            ),
            (nose_extent[0] * 0.58, thickness, nose_extent[2] * 0.7),
            7,
            material_id="mat_automotive_paint",
        )
        deck_panel = _box(
            "visual_panel_vehicle_deck",
            (
                cabin.center_mm[0] + cabin_extent[0] * 0.38,
                cabin.center_mm[1] + cabin_extent[1] / 2 + thickness / 2 - 2.0,
                cabin.center_mm[2],
            ),
            (cabin_extent[0] * 0.22, thickness, cabin_extent[2] * 0.62),
            3,
            material_id="mat_composite",
        )
    details = [
        paint_panel,
        deck_panel,
        _box("visual_groove_vehicle_rocker_left", (cx + width * 0.02, cy - height * 0.28, cz - depth / 2 - thickness / 2 + 6.0), (width * 0.62, max(14.0, height * 0.12), thickness), 0, material_id="mat_rubber"),
        _box("visual_groove_vehicle_rocker_right", (cx + width * 0.02, cy - height * 0.28, cz + depth / 2 + thickness / 2 - 6.0), (width * 0.62, max(14.0, height * 0.12), thickness), 0, material_id="mat_rubber"),
        _box(
            "visual_guard_vehicle_rear_bumper",
            (cx + width / 2 - max(18.0, thickness * 0.45), cy - height * 0.08, cz),
            (max(16.0, thickness * 0.5), max(30.0, height * 0.1), depth * 0.46),
            3,
            material_id="mat_composite",
        ),
        _box(
            "visual_light_strip_vehicle_front",
            (cx - width / 2 + max(18.0, thickness * 0.45), cy + height * 0.02, cz),
            (max(16.0, thickness * 0.5), max(18.0, height * 0.07), depth * 0.34),
            6,
            material_id="mat_emissive_blue",
        ),
        _box("visual_cable_slot_vehicle_belt", (cx - width * 0.05, cy + height * 0.12, cz + depth / 2 + thickness / 2 - 6.0), (width * 0.46, max(12.0, height * 0.055), thickness), 0, material_id="mat_rubber"),
    ]
    wheel_anchors = _exact_showcase_roles(
        boxes,
        domain_pack_id="pack_vehicle_concept",
        required_roles=(
            "vehicle_chassis",
            "vehicle_wheel_fl",
            "vehicle_wheel_fr",
            "vehicle_wheel_rl",
            "vehicle_wheel_rr",
        ),
    )
    if wheel_anchors is not None:
        for position in ("fl", "fr", "rl", "rr"):
            wheel = wheel_anchors[f"vehicle_wheel_{position}"]
            side_sign = math.copysign(1.0, wheel.center_mm[2])
            details.append(
                _showcase_sweep_tube(
                    f"visual_guard_vehicle_fender_{position}",
                    (
                        wheel.center_mm[0],
                        wheel.center_mm[1],
                        wheel.center_mm[2]
                        + side_sign * (wheel.height_mm / 2 + 4.0),
                    ),
                    path_points=(
                        (-wheel.radius_mm * 0.88, wheel.radius_mm * 0.3, 0.0),
                        (-wheel.radius_mm * 0.56, wheel.radius_mm * 0.88, 0.0),
                        (0.0, wheel.radius_mm * 1.12, 0.0),
                        (wheel.radius_mm * 0.56, wheel.radius_mm * 0.88, 0.0),
                        (wheel.radius_mm * 0.88, wheel.radius_mm * 0.3, 0.0),
                    ),
                    profile_scale=(24.0, 18.0),
                    material=7,
                    material_id="mat_automotive_paint",
                )
            )
    vent_radius = max(9.0, min(19.0, min(width, depth) * 0.028))
    for index, z_offset in enumerate((-vent_radius * 1.15, vent_radius * 1.15), 1):
        details.append(
            _wedge(
                f"visual_vent_vehicle_deck_{index}",
                (
                    cx + width * 0.28,
                    cy + height / 2 + thickness / 2 - 6.0,
                    cz + z_offset,
                ),
                (vent_radius * 2.6, thickness, vent_radius * 1.2),
                0,
                material_id="mat_rubber",
            )
        )
    fastener_radius = max(8.0, min(15.0, min(height, depth) * 0.03))
    details.append(
        _cylinder(
            "visual_fastener_vehicle_sill_center",
            (cx, cy - height * 0.18, cz + depth / 2 + thickness / 2 - 6.0),
            fastener_radius,
            thickness,
            1,
            (0.0, 0.0, 1.0),
            material_id="mat_aluminum",
        )
    )
    if detail_level >= 4:
        details.append(_box("visual_light_strip_vehicle_rear", (cx + width / 2 + thickness / 2, cy + height * 0.04, cz), (thickness, max(24.0, height * 0.18), depth * 0.38), 6, material_id="mat_emissive_blue"))
    return details


def _aircraft_showcase_details(
    primary: BoxPrimitive,
    detail_level: int,
    boxes: Sequence[BoxPrimitive],
) -> List[BoxPrimitive]:
    """Aircraft shell language: dorsal spine, paired chines and wing-tip markers."""

    cx, cy, cz = primary.center_mm
    width, height, depth = _primitive_display_extent(primary)
    thickness = _detail_thickness(width, height, depth)
    surface_inset = 20.0 if primary.primitive_kind == "loft_contract" else 8.0
    details = [
        _box("visual_panel_aircraft_dorsal", (cx + width * 0.08, cy + height / 2 + thickness / 2 - surface_inset, cz), (width * 0.36, thickness, depth * 0.18), 3, material_id="mat_composite"),
        _box("visual_groove_aircraft_belly", (cx + width * 0.12, cy - height / 2 - thickness / 2 + surface_inset, cz), (width * 0.3, thickness, depth * 0.24), 0, material_id="mat_rubber"),
        _wedge(
            "visual_guard_aircraft_chine_left",
            (
                cx - width * 0.08,
                cy - height * 0.05,
                cz - depth / 2 - thickness / 2 + surface_inset,
            ),
            (width * 0.18, max(18.0, height * 0.08), thickness),
            1,
            material_id="mat_aluminum",
        ),
        _wedge(
            "visual_guard_aircraft_chine_right",
            (
                cx - width * 0.08,
                cy - height * 0.05,
                cz + depth / 2 + thickness / 2 - surface_inset,
            ),
            (width * 0.18, max(18.0, height * 0.08), thickness),
            1,
            material_id="mat_aluminum",
        ),
        _box("visual_light_strip_aircraft_port", (cx - width * 0.28, cy + height * 0.06, cz - depth / 2 - thickness / 2 + surface_inset), (width * 0.16, max(12.0, height * 0.06), thickness), 6, material_id="mat_emissive_blue"),
        _box("visual_light_strip_aircraft_starboard", (cx - width * 0.28, cy + height * 0.06, cz + depth / 2 + thickness / 2 - surface_inset), (width * 0.16, max(12.0, height * 0.06), thickness), 6, material_id="mat_emissive_blue"),
        _box("visual_cable_slot_aircraft_spine", (cx + width * 0.18, cy + height / 2 + thickness / 2 - surface_inset, cz), (width * 0.22, thickness, max(14.0, depth * 0.05)), 0, material_id="mat_rubber"),
    ]
    lift_anchors = _exact_showcase_roles(
        boxes,
        domain_pack_id="pack_aircraft_concept",
        required_roles=("lift_wing_left", "lift_wing_right"),
    )
    if lift_anchors is not None:
        for side, sign in (("left", -1.0), ("right", 1.0)):
            wing = lift_anchors[f"lift_wing_{side}"]
            wing_extent = _primitive_display_extent(wing)
            panel_depth = wing_extent[2] * 0.14
            details.append(
                _wedge(
                    f"visual_guard_aircraft_wing_root_{side}",
                    (
                        wing.center_mm[0] - wing_extent[0] * 0.12,
                        wing.center_mm[1] + wing_extent[1] / 2 + thickness / 2 - 10.0,
                        cz + sign * (depth / 2 + panel_depth / 2 - 10.0),
                    ),
                    (wing_extent[0] * 0.22, thickness, panel_depth),
                    0,
                    material_id="mat_primary",
                )
            )
    vent_radius = max(9.0, min(18.0, min(height, depth) * 0.04))
    details.append(
        _wedge(
            "visual_vent_aircraft_tail_1",
            (cx + width / 2 + thickness / 2 - surface_inset, cy - height * 0.1, cz),
            (thickness, vent_radius * 2, vent_radius * 2),
            0,
            material_id="mat_rubber",
        )
    )
    fastener_radius = max(8.0, min(14.0, min(width, depth) * 0.025))
    details.append(_cylinder("visual_fastener_aircraft_dorsal_1", (cx - width * 0.12, cy + height / 2 + thickness / 2 - surface_inset, cz), fastener_radius, thickness, 1, (0.0, 1.0, 0.0), material_id="mat_aluminum"))
    return details


def _robotic_arm_showcase_details(
    primary: BoxPrimitive,
    detail_level: int,
    boxes: Sequence[BoxPrimitive],
) -> List[BoxPrimitive]:
    """Robot shell language: grounded base fascia, service plate and status bar."""

    cx, cy, cz = primary.center_mm
    width, height, depth = _primitive_display_extent(primary)
    thickness = _detail_thickness(width, height, depth)
    link_matches = [item for item in boxes if item.part_role == "precision_link_1"]
    if len(link_matches) > 1:
        raise ValueError("showcase robot upper-link anchor is duplicated")
    if link_matches:
        link = link_matches[0]
        link_extent = _primitive_display_extent(link)
        panel = _box(
            "visual_panel_robot_upper_link",
            (
                link.center_mm[0],
                link.center_mm[1],
                link.center_mm[2] + link.radius_mm + thickness / 2 - 8.0,
            ),
            (link.radius_mm * 0.64, link_extent[1] * 0.42, thickness),
            3,
            material_id="mat_composite",
        )
    else:
        panel = _box("visual_panel_robot_base", (cx, cy + height / 2 + thickness / 2, cz), (width * 0.58, thickness, depth * 0.58), 3, material_id="mat_composite")
    details = [
        panel,
        _box("visual_groove_robot_plinth", (cx, cy - height * 0.2, cz + depth / 2 + thickness / 2 - 6.0), (width * 0.58, max(14.0, height * 0.12), thickness), 0, material_id="mat_rubber"),
        _box(
            "visual_guard_robot_corner",
            (
                cx + width * 0.3,
                cy + height * 0.08,
                cz + depth / 2 + thickness / 2 - 6.0,
            ),
            (width * 0.16, height * 0.32, thickness),
            1,
            material_id="mat_aluminum",
        ),
        _box("visual_light_strip_robot_status", (cx - width * 0.32, cy + height * 0.08, cz + depth / 2 + thickness / 2 - 6.0), (max(18.0, width * 0.08), height * 0.42, thickness), 6, material_id="mat_emissive_blue"),
        _box("visual_cable_slot_robot_service", (cx + width * 0.08, cy + height * 0.16, cz + depth / 2 + thickness / 2 - 6.0), (width * 0.34, max(12.0, height * 0.055), thickness), 0, material_id="mat_rubber"),
    ]
    forearm_matches = [item for item in boxes if item.part_role == "precision_link_2"]
    if len(forearm_matches) > 1:
        raise ValueError("showcase robot forearm anchor is duplicated")
    if forearm_matches:
        forearm = forearm_matches[0]
        forearm_extent = _primitive_display_extent(forearm)
        details.append(
            _box(
                "visual_cable_slot_robot_forearm",
                (
                    forearm.center_mm[0],
                    forearm.center_mm[1] - forearm.radius_mm * 0.34,
                    forearm.center_mm[2] + forearm.radius_mm + thickness / 2 - 8.0,
                ),
                (forearm_extent[0] * 0.5, max(12.0, forearm.radius_mm * 0.18), thickness),
                0,
                material_id="mat_rubber",
            )
        )
    joint_anchors = _exact_showcase_roles(
        boxes,
        domain_pack_id="pack_robotic_arm_concept",
        required_roles=("precision_joint_1", "precision_joint_2", "precision_wrist"),
    )
    if joint_anchors is not None:
        for label, role in (
            ("shoulder", "precision_joint_1"),
            ("elbow", "precision_joint_2"),
            ("wrist", "precision_wrist"),
        ):
            joint = joint_anchors[role]
            cap_height = 28.0
            details.append(
                _cylinder(
                    f"visual_guard_robot_joint_cap_{label}",
                    (
                        joint.center_mm[0],
                        joint.center_mm[1],
                        joint.center_mm[2] + joint.height_mm / 2 + cap_height / 2 - 8.0,
                    ),
                    joint.radius_mm * 0.68,
                    cap_height,
                    1,
                    (0.0, 0.0, 1.0),
                    material_id="mat_aluminum",
                )
            )
        details.append(
            _showcase_sweep_tube(
                "visual_cable_robot_service_loop",
                (365.0, 835.0, 142.0),
                path_points=(
                    (-365.0, -285.0, 0.0),
                    (-315.0, -135.0, 10.0),
                    (-175.0, 5.0, 0.0),
                    (-175.0, 245.0, 0.0),
                    (95.0, 245.0, -20.0),
                    (365.0, 245.0, -32.0),
                ),
                profile_scale=(14.0, 14.0),
                material=4,
                material_id="mat_rubber",
            )
        )
    vent_radius = max(9.0, min(18.0, min(width, height) * 0.04))
    for index, x_offset in enumerate((-vent_radius * 1.5, 0.0, vent_radius * 1.5), 1):
        details.append(_cylinder(f"visual_vent_robot_base_{index}", (cx + width * 0.2 + x_offset, cy - height * 0.1, cz + depth / 2 + thickness / 2 - 6.0), vent_radius, thickness, 0, (0.0, 0.0, 1.0), material_id="mat_rubber"))
    fastener_radius = max(8.0, min(15.0, min(width, depth) * 0.025))
    for index, z_offset in enumerate((-depth * 0.22, depth * 0.22), 1):
        details.append(_cylinder(f"visual_fastener_robot_top_{index}", (cx, cy + height / 2 + thickness / 2 - 6.0, cz + z_offset), fastener_radius, thickness, 1, (0.0, 1.0, 0.0), material_id="mat_aluminum"))
    if detail_level >= 4:
        details.append(_box("visual_panel_robot_warning", (cx + width * 0.32, cy - height * 0.02, cz - depth / 2 - thickness / 2), (width * 0.16, height * 0.24, thickness), 2, material_id="mat_signal_red"))
    return details


def _box(
    role: str,
    center: Tuple[float, float, float],
    size: Tuple[float, float, float],
    material: int,
    *,
    material_id: str | None = None,
) -> BoxPrimitive:
    return BoxPrimitive(role, center, size, material, material_id=material_id)


def _wedge(
    role: str,
    center: Tuple[float, float, float],
    size: Tuple[float, float, float],
    material: int,
    *,
    material_id: str | None = None,
) -> BoxPrimitive:
    return BoxPrimitive(role, center, size, material, "wedge", material_id=material_id)


def _capsule(
    role: str,
    center: Tuple[float, float, float],
    radius: float,
    height: float,
    material: int,
    axis: Tuple[float, float, float] = (0.0, 1.0, 0.0),
) -> BoxPrimitive:
    return BoxPrimitive(role, center, (radius * 2, height, radius * 2), material, "capsule", radius, height, axis)


def _showcase_ellipse_profile(
    sketch_id: str,
    half_u: float,
    half_v: float,
    *,
    resample_count: int = 24,
) -> Dict[str, Any]:
    """Build one bounded rounded cross-section for a fixed reviewed loft.

    The four quadratic arcs are part of the built-in visual fixture, not a
    user-authored free curve.  G820/G822 still own normalization, resampling,
    winding and runtime rejection.
    """

    start = [0.0, -half_v]
    return {
        "schema_version": "ProfileSketch@1",
        "sketch_id": sketch_id,
        "version": 1,
        "plane": "cross_section",
        "closed": True,
        "winding": "counter_clockwise",
        "start": start,
        "segments": [
            {"kind": "quadratic", "control": [half_u, -half_v], "to": [half_u, 0.0]},
            {"kind": "quadratic", "control": [half_u, half_v], "to": [0.0, half_v]},
            {"kind": "quadratic", "control": [-half_u, half_v], "to": [-half_u, 0.0]},
            {"kind": "quadratic", "control": [-half_u, -half_v], "to": start},
        ],
        "holes": [],
        "normalized_bounds": {
            "min": [-half_u, -half_v],
            "max": [half_u, half_v],
        },
        "symmetry": "vertical",
        "continuity_hint": "tangent",
        "resample_count": resample_count,
        "provenance": {
            "source": "component_recipe",
            "source_ref": "m108_reviewed_showcase_loft",
        },
    }


def _showcase_airfoil_profile(
    sketch_id: str,
    half_u: float,
    half_v: float,
    *,
    resample_count: int,
) -> Dict[str, Any]:
    """Build one bounded asymmetric visual airfoil for a reviewed wing loft."""

    start = [-half_u, 0.0]
    return {
        "schema_version": "ProfileSketch@1",
        "sketch_id": sketch_id,
        "version": 1,
        "plane": "cross_section",
        "closed": True,
        "winding": "clockwise",
        "start": start,
        "segments": [
            {
                "kind": "quadratic",
                "control": [-half_u * 0.78, half_v * 0.85],
                "to": [-half_u * 0.15, half_v * 0.72],
            },
            {
                "kind": "quadratic",
                "control": [half_u * 0.55, half_v * 0.48],
                "to": [half_u, 0.0],
            },
            {
                "kind": "quadratic",
                "control": [half_u * 0.55, -half_v * 0.2],
                "to": [-half_u * 0.15, -half_v * 0.38],
            },
            {
                "kind": "quadratic",
                "control": [-half_u * 0.82, -half_v * 0.42],
                "to": start,
            },
        ],
        "holes": [],
        "normalized_bounds": {
            "min": [-half_u, -half_v * 0.42],
            "max": [half_u, half_v * 0.85],
        },
        "symmetry": "none",
        "continuity_hint": "tangent",
        "resample_count": resample_count,
        "provenance": {
            "source": "component_recipe",
            "source_ref": "m108_reviewed_showcase_airfoil",
        },
    }


def _showcase_hard_surface_profile(
    sketch_id: str,
    half_u: float,
    half_v: float,
    *,
    resample_count: int,
) -> Dict[str, Any]:
    """Build one rounded-rectangle cross-section for reviewed product shells.

    The flat crown, floor and side bands keep prop, vehicle and tool silhouettes
    from collapsing into generic cylinders while the quadratic corners preserve
    a bounded product-style edge transition. This remains a fixed M108 fixture,
    not a free profile editor or an engineering section.
    """

    start = [-half_u * 0.68, -half_v]
    return {
        "schema_version": "ProfileSketch@1",
        "sketch_id": sketch_id,
        "version": 1,
        "plane": "cross_section",
        "closed": True,
        "winding": "counter_clockwise",
        "start": start,
        "segments": [
            {"kind": "line", "to": [half_u * 0.68, -half_v]},
            {
                "kind": "quadratic",
                "control": [half_u, -half_v],
                "to": [half_u, -half_v * 0.58],
            },
            {"kind": "line", "to": [half_u, half_v * 0.52]},
            {
                "kind": "quadratic",
                "control": [half_u, half_v],
                "to": [half_u * 0.62, half_v],
            },
            {"kind": "line", "to": [-half_u * 0.62, half_v]},
            {
                "kind": "quadratic",
                "control": [-half_u, half_v],
                "to": [-half_u, half_v * 0.52],
            },
            {"kind": "line", "to": [-half_u, -half_v * 0.58]},
            {
                "kind": "quadratic",
                "control": [-half_u, -half_v],
                "to": start,
            },
        ],
        "holes": [],
        "normalized_bounds": {
            "min": [-half_u, -half_v],
            "max": [half_u, half_v],
        },
        "symmetry": "vertical",
        "continuity_hint": "tangent",
        "resample_count": resample_count,
        "provenance": {
            "source": "component_recipe",
            "source_ref": "m108_reviewed_showcase_hard_surface",
        },
    }


def _showcase_loft_shell(
    role: str,
    center: Tuple[float, float, float],
    *,
    axis_length: float,
    cross_section_scale: Tuple[float, float],
    sections: Sequence[Tuple[float, float, float]],
    material: int,
    material_id: str,
    main_axis: str = "x",
    resample_count: int = 24,
    profile_shape: str = "ellipse",
) -> BoxPrimitive:
    """Describe a fixed multi-section shell that must execute through G822."""

    if len(sections) < 3 or sections[0][0] != -1.0 or sections[-1][0] != 1.0:
        raise ValueError("showcase loft requires at least three sections spanning -1..1")
    if main_axis not in {"x", "y", "z"}:
        raise ValueError("showcase loft main axis must be x, y or z")
    if resample_count not in {16, 24}:
        raise ValueError("showcase loft resample count must use a reviewed fixed baseline")
    if profile_shape not in {"ellipse", "airfoil", "hard_surface"}:
        raise ValueError("showcase loft profile shape must use a reviewed fixed profile")
    profile_factory = {
        "ellipse": _showcase_ellipse_profile,
        "airfoil": _showcase_airfoil_profile,
        "hard_surface": _showcase_hard_surface_profile,
    }[profile_shape]
    profiles = [
        profile_factory(
            f"sketch_{role}_{index}",
            half_u,
            half_v,
            resample_count=resample_count,
        )
        for index, (_position, half_u, half_v) in enumerate(sections, 1)
    ]
    section_set = {
        "schema_version": "ProfileSectionSet@1",
        "section_set_id": f"sectionset_{role}",
        "version": 1,
        "main_axis": main_axis,
        "profiles": profiles,
        "sections": [
            {
                "section_id": f"section_{role}_{index}",
                "position": position,
                "profile_sketch_id": profiles[index - 1]["sketch_id"],
                "scale": 1.0,
                "twist_degrees": 0.0,
                "cap_policy": "start" if index == 1 else "end" if index == len(sections) else "none",
            }
            for index, (position, _half_u, _half_v) in enumerate(sections, 1)
        ],
        "resample_policy": {"mode": "uniform_count", "count": resample_count},
        "symmetry": "none" if profile_shape == "airfoil" else "vertical",
        "provenance": {
            "source": "component_recipe",
            "source_ref": f"m108_reviewed_{role}",
        },
    }
    max_u = max(item[1] for item in sections) * cross_section_scale[0]
    max_v = max(item[2] for item in sections) * cross_section_scale[1]
    display_size = {
        "x": (axis_length, max_u * 2, max_v * 2),
        "y": (max_u * 2, axis_length, max_v * 2),
        "z": (max_u * 2, max_v * 2, axis_length),
    }[main_axis]
    return BoxPrimitive(
        role,
        center,
        display_size,
        material,
        "loft_contract",
        height_mm=axis_length,
        loft_axis=main_axis,
        material_id=material_id,
        profile_contract=section_set,
        loft_cross_section_scale=cross_section_scale,
    )


def _showcase_sweep_tube(
    role: str,
    center: Tuple[float, float, float],
    *,
    path_points: Sequence[Tuple[float, float, float]],
    profile_scale: Tuple[float, float],
    material: int,
    material_id: str,
    resample_count: int = 8,
) -> BoxPrimitive:
    """Describe a reviewed visual tube that must execute through G823."""

    if not 3 <= len(path_points) <= 8:
        raise ValueError("showcase sweep requires 3-8 reviewed path points")
    if resample_count != 8:
        raise ValueError("showcase sweep uses the reviewed eight-point profile")
    if any(value <= 0 or value > 64 for value in profile_scale):
        raise ValueError("showcase sweep profile scale is outside the visual budget")
    if any(
        not math.isfinite(value)
        for point in path_points
        for value in point
    ):
        raise ValueError("showcase sweep path must be finite")
    profile = _showcase_ellipse_profile(
        f"sketch_{role}",
        1.0,
        1.0,
        resample_count=resample_count,
    )
    minimum = [
        min(point[axis] for point in path_points) - max(profile_scale)
        for axis in range(3)
    ]
    maximum = [
        max(point[axis] for point in path_points) + max(profile_scale)
        for axis in range(3)
    ]
    return BoxPrimitive(
        role,
        center,
        tuple(maximum[axis] - minimum[axis] for axis in range(3)),
        material,
        "sweep_contract",
        material_id=material_id,
        profile_contract=profile,
        sweep_profile_scale=profile_scale,
        sweep_path_mm=tuple(path_points),
        cap_start=True,
        cap_end=True,
    )


def _variant_boxes_for_domain(pack_id: str, variant_id: str) -> List[BoxPrimitive]:
    """Return an explicit, low-poly structure for the G807 diversity catalog.

    These are visual blockouts only.  The catalog intentionally varies part roles,
    primitive kinds, counts and placement so a repeated scale change cannot pass as
    a new design.  It contains no functional, manufacturing or performance data.
    """
    catalog: Dict[str, Dict[str, List[BoxPrimitive]]] = {
        "pack_future_weapon_prop": {
            "compact_prop_a": [
                _showcase_loft_shell(
                    "prop_core",
                    (0, 610, 0),
                    axis_length=1320,
                    cross_section_scale=(190, 175),
                    sections=(
                        (-1.0, 0.48, 0.58),
                        (-0.82, 0.78, 0.84),
                        (-0.45, 1.0, 0.96),
                        (0.1, 0.98, 1.0),
                        (0.55, 0.88, 0.92),
                        (0.82, 0.72, 0.76),
                        (1.0, 0.5, 0.58),
                    ),
                    material=0,
                    material_id="mat_primary",
                    profile_shape="hard_surface",
                ),
                _showcase_loft_shell(
                    "prop_front_shroud",
                    (-695, 610, 0),
                    axis_length=260,
                    cross_section_scale=(165, 155),
                    sections=(
                        (-1.0, 0.58, 0.62),
                        (-0.55, 0.88, 0.92),
                        (0.2, 1.0, 1.0),
                        (1.0, 0.8, 0.86),
                    ),
                    material=3,
                    material_id="mat_composite",
                    resample_count=16,
                    profile_shape="hard_surface",
                ),
                _cylinder(
                    "prop_front_trim",
                    (-835, 610, 0),
                    125,
                    70,
                    1,
                    (1, 0, 0),
                    material_id="mat_aluminum",
                ),
                _cylinder(
                    "prop_front_lens",
                    (-876, 610, 0),
                    88,
                    16,
                    5,
                    (1, 0, 0),
                    material_id="mat_dark_glass",
                ),
                _showcase_sweep_tube(
                    "prop_grip",
                    (-80, 320, 0),
                    path_points=(
                        (-50, 210, 0),
                        (-30, 95, 0),
                        (0, -50, 0),
                        (55, -205, 0),
                    ),
                    profile_scale=(62, 48),
                    material=1,
                    material_id="mat_rubber",
                ),
                _showcase_loft_shell(
                    "prop_lower_fore_shell",
                    (-330, 455, 0),
                    axis_length=520,
                    cross_section_scale=(88, 138),
                    sections=(
                        (-1.0, 0.45, 0.55),
                        (-0.55, 0.9, 0.9),
                        (0.35, 1.0, 1.0),
                        (1.0, 0.5, 0.65),
                    ),
                    material=3,
                    material_id="mat_composite",
                    resample_count=16,
                    profile_shape="hard_surface",
                ),
                _showcase_loft_shell(
                    "prop_rear_housing",
                    (610, 610, 0),
                    axis_length=420,
                    cross_section_scale=(170, 160),
                    sections=(
                        (-1.0, 1.0, 1.0),
                        (-0.3, 0.92, 0.96),
                        (0.5, 0.7, 0.76),
                        (1.0, 0.4, 0.52),
                    ),
                    material=0,
                    material_id="mat_primary",
                    resample_count=16,
                    profile_shape="hard_surface",
                ),
                _showcase_loft_shell(
                    "prop_sensor_housing",
                    (20, 825, 0),
                    axis_length=300,
                    cross_section_scale=(70, 58),
                    sections=(
                        (-1.0, 0.55, 0.7),
                        (-0.3, 1.0, 1.0),
                        (0.6, 0.85, 0.92),
                        (1.0, 0.55, 0.6),
                    ),
                    material=3,
                    material_id="mat_composite",
                    resample_count=16,
                    profile_shape="hard_surface",
                ),
                _cylinder(
                    "prop_sensor_glass",
                    (-137, 825, 0),
                    40,
                    18,
                    5,
                    (1, 0, 0),
                    material_id="mat_dark_glass",
                ),
                _box(
                    "prop_color_badge",
                    (360, 515, 181),
                    (110, 28, 18),
                    2,
                    material_id="mat_signal_red",
                ),
            ],
            "compact_prop_b": [_capsule("prop_capsule_body", (0, 570, 0), 300, 1500, 0, (1, 0, 0)), _box("prop_side_housing", (100, 520, -390), (680, 260, 180), 1), _box("prop_grip", (-250, 170, 0), (300, 560, 280), 1), _wedge("prop_optic", (250, 840, 0), (360, 180, 220), 2), _box("prop_panel", (520, 590, 390), (380, 80, 220), 2)],
            "compact_prop_c": [_wedge("prop_front_shell", (-500, 600, 0), (720, 420, 560), 0), _box("prop_rear_shell", (480, 600, 0), (900, 500, 600), 1), _cylinder("prop_muzzle_ring", (-900, 600, 0), 180, 240, 2, (1, 0, 0)), _box("prop_grip", (-120, 170, 0), (320, 560, 300), 1), _box("prop_back_panel", (700, 820, 0), (260, 180, 280), 2)],
            "long_profile_prop_a": [_box("prop_long_body", (0, 590, 0), (2300, 360, 420), 0), _cylinder("prop_front_ring", (-1180, 590, 0), 150, 260, 2, (1, 0, 0)), _box("prop_stock", (980, 510, 0), (560, 340, 360), 1), _wedge("prop_upper_rail", (240, 820, 0), (1100, 120, 180), 2), _box("prop_grip", (-180, 180, 0), (280, 520, 260), 1)],
            "long_profile_prop_b": [_box("prop_long_body", (0, 620, 0), (2500, 420, 500), 0), _box("prop_lower_housing", (-180, 360, 0), (760, 260, 380), 1), _cylinder("prop_front_emitter", (-1230, 620, 0), 130, 300, 2, (1, 0, 0)), _cylinder("prop_rear_emitter", (1120, 620, 0), 110, 260, 2, (1, 0, 0)), _box("prop_grip", (0, 180, 0), (300, 560, 280), 1), _wedge("prop_top_fin", (400, 900, 0), (480, 220, 180), 2)],
            "long_profile_prop_c": [_box("prop_body_spine", (0, 600, 0), (2200, 360, 480), 0), _cylinder("prop_front_ring", (-1060, 600, -170), 120, 320, 2, (1, 0, 0)), _cylinder("prop_front_ring_upper", (-1060, 600, 170), 120, 320, 2, (1, 0, 0)), _box("prop_side_housing", (260, 480, 0), (820, 240, 600), 1), _box("prop_stock", (980, 520, 0), (500, 320, 360), 1), _wedge("prop_sight", (420, 860, 0), (300, 160, 220), 2)],
            "heavy_support_prop_a": [_box("prop_heavy_body", (0, 640, 0), (2100, 620, 820), 0), _wedge("prop_front_shroud", (-880, 680, 0), (520, 620, 760), 1), _box("prop_support_cradle", (0, 260, 0), (720, 340, 620), 2), _cylinder("prop_support_left", (-420, 120, -380), 90, 480, 1), _cylinder("prop_support_right", (-420, 120, 380), 90, 480, 1), _box("prop_heat_panel", (420, 880, 0), (620, 90, 420), 2)],
            "heavy_support_prop_b": [_box("prop_heavy_body", (0, 660, 0), (2300, 560, 760), 0), _capsule("prop_core_capsule", (-300, 660, 0), 210, 900, 1, (1, 0, 0)), _box("prop_support_pedestal", (0, 220, 0), (620, 420, 620), 2), _cylinder("prop_leg_left", (500, 100, -360), 85, 520, 1), _cylinder("prop_leg_right", (500, 100, 360), 85, 520, 1), _cylinder("prop_leg_center", (680, 110, 0), 85, 560, 1), _wedge("prop_sensor", (520, 920, 0), (360, 260, 280), 2)],
            "heavy_support_prop_c": [_box("prop_heavy_body", (0, 620, 0), (1900, 700, 900), 0), _cylinder("prop_ring_module", (-780, 620, 0), 260, 260, 2, (1, 0, 0)), _box("prop_rear_housing", (780, 650, 0), (620, 560, 720), 1), _box("prop_mag_left", (-80, 180, -340), (260, 620, 240), 1), _box("prop_mag_right", (-80, 180, 340), (260, 620, 240), 1), _wedge("prop_control_panel", (300, 1030, 0), (500, 180, 360), 2)],
            "energy_visual_prop_a": [_box("prop_energy_body", (0, 620, 0), (1700, 480, 620), 0), _cylinder("prop_emitter", (-950, 620, 0), 220, 300, 2, (1, 0, 0)), _wedge("prop_fin_upper", (260, 920, 0), (520, 240, 180), 1), _wedge("prop_fin_lower", (260, 320, 0), (520, 240, 180), 1), _box("prop_glass_core", (300, 640, 0), (360, 260, 400), 2)],
            "energy_visual_prop_b": [_capsule("prop_energy_handle", (0, 260, 0), 180, 900, 1), _wedge("prop_energy_head", (0, 920, 0), (840, 520, 640), 0), _cylinder("prop_emitter_left", (-300, 1040, 0), 110, 360, 2), _cylinder("prop_emitter_right", (300, 1040, 0), 110, 360, 2), _box("prop_shield", (0, 620, 390), (780, 520, 90), 2)],
            "energy_visual_prop_c": [_box("prop_drone_body", (0, 760, 0), (1500, 420, 720), 0), _wedge("prop_fin_front", (-700, 820, 0), (500, 260, 460), 1), _wedge("prop_fin_back", (700, 820, 0), (500, 260, 460), 1), _wedge("prop_fin_left", (0, 820, -520), (520, 260, 340), 1), _wedge("prop_fin_right", (0, 820, 520), (520, 260, 340), 1), _cylinder("prop_sensor", (0, 1040, 0), 150, 240, 2), _box("prop_pod_left", (-80, 420, -420), (280, 300, 240), 2), _box("prop_pod_right", (-80, 420, 420), (280, 300, 240), 2)],
        },
        "pack_vehicle_concept": {
            "urban_scout_a": [
                _showcase_loft_shell(
                    "vehicle_chassis",
                    (0, 520, 0),
                    axis_length=2200,
                    cross_section_scale=(380, 490),
                    sections=(
                        (-1.0, 0.3, 0.48),
                        (-0.72, 0.55, 0.9),
                        (-0.2, 0.88, 1.0),
                        (0.45, 0.72, 0.96),
                        (1.0, 0.45, 0.72),
                    ),
                    material=7,
                    material_id="mat_automotive_paint",
                    profile_shape="hard_surface",
                ),
                _showcase_loft_shell(
                    "vehicle_canopy",
                    (-120, 820, 0),
                    axis_length=700,
                    cross_section_scale=(130, 280),
                    sections=(
                        (-1.0, 0.25, 0.55),
                        (-0.6, 0.75, 0.9),
                        (0.0, 0.95, 1.0),
                        (0.7, 0.65, 0.88),
                        (1.0, 0.25, 0.5),
                    ),
                    material=5,
                    material_id="mat_dark_glass",
                ),
                _cylinder("vehicle_wheel_fl", (-720, 250, -550), 250, 220, 2, (0, 0, 1), material_id="mat_rubber"),
                _cylinder("vehicle_wheel_fr", (-720, 250, 550), 250, 220, 2, (0, 0, 1), material_id="mat_rubber"),
                _cylinder("vehicle_wheel_rl", (720, 250, -550), 250, 220, 2, (0, 0, 1), material_id="mat_rubber"),
                _cylinder("vehicle_wheel_rr", (720, 250, 550), 250, 220, 2, (0, 0, 1), material_id="mat_rubber"),
                _cylinder("vehicle_hub_fl", (-720, 250, -550), 118, 246, 1, (0, 0, 1)),
                _cylinder("vehicle_hub_fr", (-720, 250, 550), 118, 246, 1, (0, 0, 1)),
                _cylinder("vehicle_hub_rl", (720, 250, -550), 118, 246, 1, (0, 0, 1)),
                _cylinder("vehicle_hub_rr", (720, 250, 550), 118, 246, 1, (0, 0, 1)),
            ],
            "urban_scout_b": [_box("vehicle_body", (0, 620, 0), (2100, 480, 880), 0), _capsule("vehicle_cabin_capsule", (180, 960, 0), 300, 1000, 1, (1, 0, 0)), _cylinder("vehicle_hover_front", (-720, 320, -400), 150, 260, 2, (0, 0, 1)), _cylinder("vehicle_hover_rear", (720, 320, 400), 150, 260, 2, (0, 0, 1)), _box("vehicle_lightbar", (-1040, 760, 0), (100, 160, 560), 2)],
            "urban_scout_c": [_box("vehicle_body", (0, 580, 0), (2200, 520, 760), 0), _wedge("vehicle_cabin", (120, 920, 0), (980, 500, 700), 1), _box("vehicle_track_left", (0, 300, -500), (1800, 260, 220), 2), _box("vehicle_track_right", (0, 300, 500), (1800, 260, 220), 2), _box("vehicle_sensor", (-700, 920, 0), (260, 220, 320), 2)],
            "exploration_vehicle_a": [_box("rover_chassis", (0, 520, 0), (2400, 360, 1000), 0), _box("rover_lab", (260, 920, 0), (900, 600, 860), 1), *[_cylinder(f"rover_wheel_{index}", (center, 300, side), 160, 200, 2, (0, 0, 1)) for index, center in enumerate((-850, -300, 300, 850), 1) for side in (-520, 520)], _wedge("rover_sensor_mast", (-600, 1120, 0), (260, 520, 260), 2)],
            "exploration_vehicle_b": [_box("rover_front", (-850, 560, 0), (700, 460, 900), 0), _box("rover_rear", (850, 560, 0), (900, 500, 980), 1), _capsule("rover_central_module", (0, 900, 0), 260, 1100, 1, (1, 0, 0)), _cylinder("rover_axle_front", (-700, 300, 0), 120, 1100, 2, (0, 0, 1)), _cylinder("rover_axle_rear", (700, 300, 0), 120, 1100, 2, (0, 0, 1)), _box("rover_roof_panel", (0, 1250, 0), (720, 90, 520), 2)],
            "exploration_vehicle_c": [_box("crawler_chassis", (0, 520, 0), (2500, 460, 1100), 0), _wedge("crawler_cabin", (-250, 960, 0), (1000, 640, 900), 1), _box("crawler_track_left", (0, 300, -620), (2200, 300, 260), 2), _box("crawler_track_right", (0, 300, 620), (2200, 300, 260), 2), _box("crawler_rear_crate", (850, 920, 0), (520, 560, 800), 1), _cylinder("crawler_mast", (350, 1420, 0), 100, 520, 2)],
            "low_racer_a": [_wedge("racer_front", (-760, 500, 0), (800, 340, 700), 0), _box("racer_cockpit", (160, 780, 0), (760, 360, 640), 1), _box("racer_rear", (820, 520, 0), (700, 420, 720), 0), _cylinder("racer_wheel_left", (-420, 260, -520), 150, 180, 2, (0, 0, 1)), _cylinder("racer_wheel_right", (-420, 260, 520), 150, 180, 2, (0, 0, 1)), _cylinder("racer_wheel_left_rear", (700, 260, -520), 150, 180, 2, (0, 0, 1)), _cylinder("racer_wheel_right_rear", (700, 260, 520), 150, 180, 2, (0, 0, 1))],
            "low_racer_b": [_box("racer_delta_body", (0, 520, 0), (2600, 360, 1000), 0), _wedge("racer_delta_nose", (-1000, 580, 0), (700, 300, 850), 1), _box("racer_canopy", (120, 840, 0), (620, 300, 520), 2), _cylinder("racer_pod_left", (550, 300, -400), 140, 420, 2, (1, 0, 0)), _cylinder("racer_pod_right", (550, 300, 400), 140, 420, 2, (1, 0, 0))],
            "low_racer_c": [_capsule("racer_capsule_body", (0, 620, 0), 340, 2200, 0, (1, 0, 0)), _box("racer_cockpit", (-120, 920, 0), (620, 240, 520), 1), _wedge("racer_wing_left", (320, 520, -680), (900, 160, 520), 2), _wedge("racer_wing_right", (320, 520, 680), (900, 160, 520), 2), _box("racer_tail", (900, 700, 0), (420, 300, 360), 1)],
            "heavy_transport_a": [_box("carrier_chassis", (0, 600, 0), (3000, 680, 1400), 0), _box("carrier_cab", (-900, 1120, 0), (760, 720, 1100), 1), _box("carrier_cargo", (720, 1050, 0), (1500, 900, 1200), 1), _cylinder("carrier_wheel_front_l", (-900, 300, -700), 220, 240, 2, (0, 0, 1)), _cylinder("carrier_wheel_front_r", (-900, 300, 700), 220, 240, 2, (0, 0, 1)), _cylinder("carrier_wheel_back_l", (850, 300, -700), 220, 240, 2, (0, 0, 1)), _cylinder("carrier_wheel_back_r", (850, 300, 700), 220, 240, 2, (0, 0, 1))],
            "heavy_transport_b": [_box("hauler_base", (0, 520, 0), (3000, 520, 1300), 0), _wedge("hauler_front", (-1000, 980, 0), (900, 760, 1100), 1), _box("hauler_cargo_left", (720, 980, -400), (1500, 760, 480), 1), _box("hauler_cargo_right", (720, 980, 400), (1500, 760, 480), 1), _cylinder("hauler_crane", (420, 1450, 0), 140, 900, 2), _box("hauler_light", (-1400, 840, 0), (120, 220, 760), 2)],
            "heavy_transport_c": [_capsule("rescue_bus_body", (0, 700, 0), 500, 2900, 0, (1, 0, 0)), _box("rescue_front", (-1050, 1100, 0), (620, 720, 1120), 1), _box("rescue_roof_module", (400, 1380, 0), (1000, 240, 980), 2), _cylinder("rescue_axle_front", (-800, 300, 0), 150, 1400, 2, (0, 0, 1)), _cylinder("rescue_axle_rear", (800, 300, 0), 150, 1400, 2, (0, 0, 1)), _box("rescue_side_panel", (0, 920, 680), (1800, 380, 100), 1)],
        },
    }
    # Aircraft and robotic-arm catalogs are kept in a second table below to keep
    # the domain-specific structures readable and reviewable in code review.
    catalog.update(_aircraft_variant_catalog())
    catalog.update(_robotic_arm_variant_catalog())
    try:
        return list(catalog[pack_id][variant_id])
    except KeyError as exc:
        raise ValueError(f"unknown blockout variant {variant_id!r} for {pack_id}") from exc


def _aircraft_variant_catalog() -> Dict[str, Dict[str, List[BoxPrimitive]]]:
    def rotor_blades(position: str, center: Tuple[float, float, float]) -> List[BoxPrimitive]:
        """Return a restrained crossed visual rotor instead of an opaque disc."""

        x, y, z = center
        return [
            _box(
                f"visual_blade_aircraft_{position}_longitudinal",
                (x, y, z),
                (300, 10, 24),
                3,
                material_id="mat_composite",
            ),
            _box(
                f"visual_blade_aircraft_{position}_transverse",
                (x, y, z),
                (24, 10, 300),
                3,
                material_id="mat_composite",
            ),
        ]

    return {
        "pack_aircraft_concept": {
            "vertical_takeoff_a": [
                _showcase_loft_shell(
                    "airframe_core",
                    (40, 650, 0),
                    axis_length=2200,
                    cross_section_scale=(190, 210),
                    sections=(
                        (-1.0, 0.18, 0.18),
                        (-0.72, 0.78, 0.78),
                        (-0.25, 1.0, 1.0),
                        (0.35, 0.92, 0.9),
                        (0.78, 0.58, 0.55),
                        (1.0, 0.22, 0.2),
                    ),
                    material=0,
                    material_id="mat_primary",
                ),
                _showcase_loft_shell(
                    "airframe_canopy",
                    (-420, 805, 0),
                    axis_length=520,
                    cross_section_scale=(105, 150),
                    sections=(
                        (-1.0, 0.25, 0.4),
                        (-0.55, 0.85, 0.9),
                        (0.2, 1.0, 1.0),
                        (1.0, 0.3, 0.45),
                    ),
                    material=5,
                    material_id="mat_dark_glass",
                ),
                _showcase_loft_shell(
                    "lift_wing_left",
                    (180, 610, -500),
                    axis_length=700,
                    cross_section_scale=(360, 32),
                    sections=(
                        (-1.0, 0.18, 0.34),
                        (-0.55, 0.5, 0.62),
                        (0.15, 0.84, 0.86),
                        (1.0, 1.0, 1.0),
                    ),
                    material=0,
                    material_id="mat_primary",
                    main_axis="z",
                    resample_count=16,
                    profile_shape="airfoil",
                ),
                _showcase_loft_shell(
                    "lift_wing_right",
                    (180, 610, 500),
                    axis_length=700,
                    cross_section_scale=(360, 32),
                    sections=(
                        (-1.0, 1.0, 1.0),
                        (-0.15, 0.84, 0.86),
                        (0.55, 0.5, 0.62),
                        (1.0, 0.18, 0.34),
                    ),
                    material=0,
                    material_id="mat_primary",
                    main_axis="z",
                    resample_count=16,
                    profile_shape="airfoil",
                ),
                _wedge("airframe_tail", (870, 760, 0), (380, 220, 130), 1),
                _cylinder("lift_rotor_front_left", (-320, 656, -820), 52, 48, 1, (0, 1, 0), material_id="mat_aluminum"),
                _cylinder("lift_rotor_front_right", (-320, 656, 820), 52, 48, 1, (0, 1, 0), material_id="mat_aluminum"),
                _cylinder("lift_rotor_rear_left", (520, 656, -820), 52, 48, 1, (0, 1, 0), material_id="mat_aluminum"),
                _cylinder("lift_rotor_rear_right", (520, 656, 820), 52, 48, 1, (0, 1, 0), material_id="mat_aluminum"),
                *rotor_blades("front_left", (-320, 684, -820)),
                *rotor_blades("front_right", (-320, 684, 820)),
                *rotor_blades("rear_left", (520, 684, -820)),
                *rotor_blades("rear_right", (520, 684, 820)),
            ],
            "vertical_takeoff_b": [_box("tilt_body", (0, 600, 0), (2200, 480, 760), 0), _wedge("tilt_nose", (-850, 700, 0), (720, 420, 720), 1), _cylinder("tilt_pod_left", (100, 450, -720), 180, 420, 2, (1, 0, 0)), _cylinder("tilt_pod_right", (100, 450, 720), 180, 420, 2, (1, 0, 0)), _box("tilt_tail", (850, 820, 0), (520, 520, 420), 1)],
            "vertical_takeoff_c": [_capsule("lift_fan_body", (0, 650, 0), 360, 2100, 0, (1, 0, 0)), _box("lift_fan_canopy", (-450, 940, 0), (620, 320, 520), 1), _cylinder("lift_fan_front", (-500, 500, 0), 250, 180, 2, (1, 0, 0)), _cylinder("lift_fan_rear", (600, 500, 0), 250, 180, 2, (1, 0, 0)), _wedge("lift_fan_tail", (820, 900, 0), (420, 360, 520), 2)],
            "fast_single_seat_a": [_wedge("jet_nose", (-1000, 600, 0), (900, 420, 520), 0), _box("jet_spine", (300, 620, 0), (1900, 360, 500), 0), _box("jet_canopy", (-360, 920, 0), (540, 300, 430), 1), _wedge("jet_delta_left", (180, 560, -820), (1400, 140, 620), 1), _wedge("jet_delta_right", (180, 560, 820), (1400, 140, 620), 1), _cylinder("jet_engine", (1050, 520, 0), 150, 500, 2, (1, 0, 0))],
            "fast_single_seat_b": [_capsule("needle_fuselage", (0, 620, 0), 260, 3000, 0, (1, 0, 0)), _box("needle_canopy", (-550, 930, 0), (520, 300, 420), 1), _wedge("needle_fin_top", (720, 920, 0), (420, 600, 220), 2), _wedge("needle_fin_left", (500, 620, -620), (900, 120, 420), 1), _wedge("needle_fin_right", (500, 620, 620), (900, 120, 420), 1)],
            "fast_single_seat_c": [_box("interceptor_body", (0, 600, 0), (2700, 420, 640), 0), _wedge("interceptor_nose", (-1100, 650, 0), (800, 360, 620), 1), _box("interceptor_canopy", (-450, 880, 0), (520, 280, 420), 2), _cylinder("interceptor_engine_left", (900, 500, -300), 140, 500, 2, (1, 0, 0)), _cylinder("interceptor_engine_right", (900, 500, 300), 140, 500, 2, (1, 0, 0)), _box("interceptor_tail", (1040, 840, 0), (500, 500, 420), 1)],
            "wide_body_transport_a": [_box("cargo_fuselage", (0, 700, 0), (3000, 800, 1500), 0), _box("cargo_cockpit", (-1150, 1250, 0), (680, 520, 1100), 1), _wedge("cargo_wing_left", (100, 650, -1200), (1800, 180, 1000), 1), _wedge("cargo_wing_right", (100, 650, 1200), (1800, 180, 1000), 1), _cylinder("cargo_engine_left", (-100, 420, -1000), 240, 600, 2, (1, 0, 0)), _cylinder("cargo_engine_right", (-100, 420, 1000), 240, 600, 2, (1, 0, 0))],
            "wide_body_transport_b": [_capsule("passenger_fuselage", (0, 720, 0), 620, 3300, 0, (1, 0, 0)), _box("passenger_cockpit", (-1250, 1250, 0), (500, 560, 1000), 1), _box("passenger_wing", (240, 620, 0), (1400, 180, 2600), 1), _cylinder("passenger_engine_left", (250, 460, -900), 220, 520, 2, (1, 0, 0)), _cylinder("passenger_engine_right", (250, 460, 900), 220, 520, 2, (1, 0, 0)), _box("passenger_tail", (1200, 1120, 0), (560, 620, 700), 2)],
            "wide_body_transport_c": [_box("heavy_lifter_body", (0, 680, 0), (3200, 720, 1700), 0), _wedge("heavy_lifter_nose", (-1350, 860, 0), (800, 640, 1500), 1), _box("heavy_lifter_wing", (200, 620, 0), (1600, 180, 3000), 1), _cylinder("heavy_lifter_engine_1", (-250, 400, -1150), 230, 620, 2, (1, 0, 0)), _cylinder("heavy_lifter_engine_2", (450, 400, -1150), 230, 620, 2, (1, 0, 0)), _cylinder("heavy_lifter_engine_3", (-250, 400, 1150), 230, 620, 2, (1, 0, 0)), _cylinder("heavy_lifter_engine_4", (450, 400, 1150), 230, 620, 2, (1, 0, 0))],
            "uncrewed_scout_a": [_wedge("scout_flying_wing", (0, 620, 0), (2400, 260, 2600), 0), _box("scout_sensor_bay", (-300, 820, 0), (620, 260, 720), 2), _cylinder("scout_pod_left", (600, 430, -700), 150, 460, 2, (1, 0, 0)), _cylinder("scout_pod_right", (600, 430, 700), 150, 460, 2, (1, 0, 0))],
            "uncrewed_scout_b": [_box("scout_body", (0, 620, 0), (2200, 480, 820), 0), _wedge("scout_nose", (-920, 720, 0), (720, 420, 760), 1), _box("scout_wing_left", (200, 620, -900), (1500, 130, 700), 1), _box("scout_wing_right", (200, 620, 900), (1500, 130, 700), 1), _cylinder("scout_tail_sensor", (780, 980, 0), 130, 360, 2)],
            "uncrewed_scout_c": [_capsule("scout_capsule", (0, 650, 0), 340, 2400, 0, (1, 0, 0)), _box("scout_upper_bay", (-240, 960, 0), (720, 260, 520), 1), _wedge("scout_fin_top", (620, 900, 0), (400, 560, 220), 2), _wedge("scout_fin_bottom", (620, 390, 0), (400, 280, 220), 2), _cylinder("scout_camera", (-860, 650, 0), 130, 240, 2, (1, 0, 0))],
        }
    }


def _robotic_arm_variant_catalog() -> Dict[str, Dict[str, List[BoxPrimitive]]]:
    def chain(prefix: str, heights: Tuple[int, ...], tool: str, material: int = 1) -> List[BoxPrimitive]:
        boxes: List[BoxPrimitive] = [_box(f"{prefix}_base", (0, 220, 0), (620, 440, 620), 0)]
        top = 440.0
        for index, height in enumerate(heights, 1):
            boxes.append(_cylinder(f"{prefix}_joint_{index}", (0, top + 130, 0), 150 + index * 20, 260, 2))
            top += 260
            boxes.append(_box(f"{prefix}_link_{index}", (0, top + height / 2, 0), (260 + index * 30, height, 260 + index * 30), material))
            top += height
        boxes.append(_wedge(tool, (0, top + 180, 0), (460, 360, 460), 2))
        return boxes

    return {
        "pack_robotic_arm_concept": {
            "precision_light_a": [
                _box("precision_base", (0, 150, 0), (700, 300, 700), 0),
                _cylinder("precision_turntable", (0, 350, 0), 210, 100, 2),
                _cylinder("precision_joint_1", (0, 550, 0), 150, 260, 2, (0, 0, 1)),
                _capsule("precision_link_1", (190, 820, 0), 115, 560, 0),
                _cylinder("precision_joint_2", (190, 1080, 0), 140, 260, 2, (0, 0, 1)),
                _capsule("precision_link_2", (460, 1080, 0), 105, 540, 1, (1, 0, 0)),
                _cylinder("precision_wrist", (730, 1080, 0), 105, 230, 2, (0, 0, 1)),
                _box("precision_tool_palm", (850, 1080, 0), (220, 190, 290), 1),
                _showcase_loft_shell(
                    "precision_gripper_upper",
                    (1010, 1170, 0),
                    axis_length=260,
                    cross_section_scale=(32, 46),
                    sections=(
                        (-1.0, 1.0, 1.0),
                        (0.45, 0.72, 0.78),
                        (1.0, 0.38, 0.52),
                    ),
                    material=4,
                    material_id="mat_rubber",
                    resample_count=16,
                    profile_shape="hard_surface",
                ),
                _showcase_loft_shell(
                    "precision_gripper_lower",
                    (1010, 990, 0),
                    axis_length=260,
                    cross_section_scale=(32, 46),
                    sections=(
                        (-1.0, 1.0, 1.0),
                        (0.45, 0.72, 0.78),
                        (1.0, 0.38, 0.52),
                    ),
                    material=4,
                    material_id="mat_rubber",
                    resample_count=16,
                    profile_shape="hard_surface",
                ),
            ],
            "precision_light_b": [_box("desktop_base", (0, 180, 0), (620, 360, 620), 0), _cylinder("desktop_turntable", (0, 500, 0), 180, 280, 2), _capsule("desktop_link", (0, 1050, 0), 120, 820, 1), _cylinder("desktop_wrist", (0, 1550, 0), 130, 260, 2), _box("desktop_tool", (0, 1880, 0), (360, 320, 260), 1)],
            "precision_light_c": [_box("rail_base", (0, 160, 0), (1000, 320, 460), 0), _box("rail_carriage", (-320, 500, 0), (360, 260, 420), 1), _cylinder("rail_pivot", (-320, 800, 0), 140, 300, 2), _box("rail_link", (-320, 1250, 0), (260, 760, 260), 1), _cylinder("rail_wrist", (-320, 1720, 0), 120, 240, 2), _wedge("rail_tool", (-320, 2000, 0), (420, 320, 360), 2)],
            "heavy_handler_a": chain("handler", (900, 760, 620), "handler_claw", 0),
            "heavy_handler_b": [_box("handler_pedestal", (0, 300, 0), (900, 600, 900), 0), _cylinder("handler_shoulder", (0, 760, 0), 280, 420, 2), _box("handler_upper", (0, 1450, 0), (520, 1100, 520), 0), _cylinder("handler_elbow", (0, 2100, 0), 240, 420, 2), _box("handler_forearm", (0, 2700, 0), (460, 920, 460), 1), _box("handler_tool_changer", (0, 3330, 0), (620, 420, 620), 2)],
            "heavy_handler_c": [_box("welding_base", (0, 260, 0), (820, 520, 820), 0), _wedge("welding_shield", (0, 760, 0), (720, 500, 740), 1), _cylinder("welding_joint", (0, 1220, 0), 220, 380, 2), _capsule("welding_arm", (0, 1900, 0), 220, 1100, 0), _cylinder("welding_wrist", (0, 2600, 0), 160, 300, 2), _box("welding_tool", (0, 3000, 0), (420, 520, 420), 2)],
            "long_reach_maintenance_a": [_box("maintenance_base", (0, 220, 0), (700, 440, 700), 0), _cylinder("maintenance_pivot", (0, 700, 0), 220, 360, 2), _box("maintenance_boom_a", (0, 1450, 0), (300, 1300, 300), 1), _box("maintenance_boom_b", (0, 2450, 0), (240, 900, 240), 1), _cylinder("maintenance_wrist", (0, 3050, 0), 150, 280, 2), _wedge("maintenance_camera", (0, 3420, 0), (400, 320, 360), 2)],
            "long_reach_maintenance_b": [_box("telescopic_base", (0, 260, 0), (760, 520, 760), 0), _cylinder("telescopic_joint", (0, 800, 0), 210, 360, 2), _capsule("telescopic_outer", (0, 1500, 0), 190, 1200, 1), _capsule("telescopic_inner", (0, 2500, 0), 140, 900, 1), _cylinder("telescopic_wrist", (0, 3180, 0), 130, 260, 2), _box("telescopic_probe", (0, 3500, 0), (320, 460, 320), 2)],
            "long_reach_maintenance_c": [_box("inspection_base", (0, 260, 0), (720, 520, 720), 0), _cylinder("inspection_turntable", (0, 760, 0), 240, 340, 2), _wedge("inspection_boom", (0, 1500, 0), (420, 1400, 360), 1), _cylinder("inspection_elbow", (0, 2350, 0), 180, 300, 2), _box("inspection_link", (0, 2880, 0), (280, 760, 280), 1), _wedge("inspection_sensor", (0, 3400, 0), (440, 360, 440), 2)],
            "dual_tool_service_a": [_box("service_mobile_base", (0, 220, 0), (900, 440, 900), 0), _cylinder("service_turret", (0, 700, 0), 240, 360, 2), _box("service_left_arm", (-420, 1450, 0), (300, 1200, 300), 1), _box("service_right_arm", (420, 1450, 0), (300, 1200, 300), 1), _wedge("service_left_tool", (-420, 2250, 0), (420, 360, 420), 2), _wedge("service_right_tool", (420, 2250, 0), (420, 360, 420), 2)],
            "dual_tool_service_b": [_box("carousel_base", (0, 240, 0), (820, 480, 820), 0), _cylinder("carousel_disk", (0, 720, 0), 320, 260, 2), _box("carousel_left_link", (-420, 1350, 0), (280, 1000, 280), 1), _capsule("carousel_right_link", (420, 1350, 0), 150, 1000, 1), _box("carousel_left_tool", (-420, 2060, 0), (360, 420, 360), 2), _box("carousel_right_tool", (420, 2060, 0), (360, 420, 360), 2)],
            "dual_tool_service_c": [_box("service_pedestal", (0, 300, 0), (840, 600, 840), 0), _wedge("service_center_guard", (0, 800, 0), (760, 360, 760), 1), _cylinder("service_left_joint", (-360, 1350, 0), 170, 320, 2), _cylinder("service_right_joint", (360, 1350, 0), 170, 320, 2), _box("service_left_link", (-360, 1860, 0), (260, 780, 260), 1), _box("service_right_link", (360, 1860, 0), (260, 780, 260), 1), _wedge("service_center_tool", (0, 2480, 0), (500, 380, 500), 2)],
        }
    }


def _cylinder(
    role: str,
    center: Tuple[float, float, float],
    radius: float,
    height: float,
    material: int,
    axis: Tuple[float, float, float] = (0.0, 1.0, 0.0),
    *,
    material_id: str | None = None,
) -> BoxPrimitive:
    return BoxPrimitive(
        role,
        center,
        (radius * 2, height, radius * 2),
        material,
        "cylinder",
        radius,
        height,
        axis,
        radial_segments=RADIAL_PRIMITIVE_SEGMENTS,
        material_id=material_id,
    )


def _apply_artifact_profile_to_primitives(
    primitives: Sequence[GeometryPrimitive],
    *,
    artifact_profile_id: GeometryArtifactProfileId,
) -> List[GeometryPrimitive]:
    profile = geometry_artifact_profile_manifest(artifact_profile_id)
    result: List[GeometryPrimitive] = []
    for primitive in primitives:
        if isinstance(primitive, CsgMeshPrimitive):
            result.append(primitive)
            continue
        updates: Dict[str, Any] = {}
        if primitive.primitive_kind in {"cylinder", "capsule"}:
            updates["radial_segments"] = int(profile["radial_segments"])
        if primitive.primitive_kind == "capsule":
            updates["capsule_hemisphere_segments"] = int(
                profile["capsule_hemisphere_segments"]
            )
        if (
            artifact_profile_id == "production_concept"
            and primitive.primitive_kind in {"revolve", "revolve_profile"}
        ):
            updates["radial_segments"] = max(
                int(primitive.radial_segments),
                int(profile["radial_segments"]),
            )
        if primitive.primitive_kind in {"loft_profile", "sweep_profile"}:
            updates["smooth_normals"] = bool(profile["smooth_loft_normals"])
        result.append(replace(primitive, **updates) if updates else primitive)
    return result


def _primitive_triangle_count(primitive: GeometryPrimitive) -> int:
    if isinstance(primitive, CsgMeshPrimitive):
        return len(primitive.triangles)
    if primitive.primitive_kind in {"box", "surface_panel"}:
        return 12
    if primitive.primitive_kind == "wedge":
        return 8
    if primitive.primitive_kind == "cylinder":
        return primitive.radial_segments * 4
    if primitive.primitive_kind == "capsule":
        return primitive.radial_segments * (
            primitive.capsule_hemisphere_segments * 4 - 2
        )
    if primitive.primitive_kind == "extrude":
        return 4 * len(primitive.profile_points) - 4
    if primitive.primitive_kind == "extrude_profile":
        edge_count = (len(primitive.profile_points) if primitive.profile_closed else len(primitive.profile_points) - 1) + sum(len(hole) for hole in primitive.profile_holes)
        cap_triangles = len(_triangulate_profile_cap(list(primitive.profile_points), [list(hole) for hole in primitive.profile_holes]))
        return edge_count * 2 + cap_triangles * int(primitive.cap_start) + cap_triangles * int(primitive.cap_end)
    if primitive.primitive_kind == "revolve":
        segments = (
            primitive.radial_segments
            if abs(primitive.revolve_angle - math.pi * 2) < 1e-6
            else max(1, primitive.radial_segments - 1)
        )
        return segments * _revolve_side_triangles_per_strip(
            primitive.profile_points, False
        )
    if primitive.primitive_kind == "revolve_profile":
        strips = primitive.radial_segments
        side = strips * _revolve_side_triangles_per_strip(primitive.profile_points, primitive.profile_closed)
        seam = max(0, len(_revolve_seam_polygon(primitive.profile_points, primitive.profile_closed)) - 2)
        return side + seam * int(primitive.cap_start) + seam * int(primitive.cap_end)
    if primitive.primitive_kind == "loft_profile":
        ring_count = len(primitive.loft_rings_mm)
        point_count = len(primitive.loft_rings_mm[0]) if ring_count else 0
        cap_count = max(0, point_count - 2)
        return max(0, ring_count - 1) * point_count * 2 + cap_count * int(primitive.cap_start) + cap_count * int(primitive.cap_end)
    if primitive.primitive_kind == "sweep_profile":
        edge_count = len(primitive.sweep_path_mm) if primitive.path_closed else max(0, len(primitive.sweep_path_mm) - 1)
        cap_count = max(0, len(primitive.profile_points) - 2)
        return edge_count * len(primitive.profile_points) * 2 + cap_count * int(primitive.cap_start and not primitive.path_closed) + cap_count * int(primitive.cap_end and not primitive.path_closed)
    if primitive.primitive_kind == "bevel_box":
        ring_points = 4 * (primitive.bevel_segments + 1)
        return 4 * ring_points
    return 64


def _surface_roles_for_primitive(primitive: BoxPrimitive) -> List[str]:
    if primitive.primitive_kind == "surface_panel":
        return ["trim"]
    if primitive.primitive_kind == "extrude_profile":
        return [
            "side",
            *(["hole_wall"] if primitive.profile_holes else []),
            *(["start_cap"] if primitive.cap_start and primitive.profile_closed else []),
            *(["end_cap"] if primitive.cap_end and primitive.profile_closed else []),
        ]
    if primitive.primitive_kind == "revolve_profile":
        full = math.isclose(primitive.revolve_angle, math.pi * 2, abs_tol=1e-9)
        return [
            "side",
            "seam",
            *(["start_cap"] if primitive.cap_start and not full else []),
            *(["end_cap"] if primitive.cap_end and not full else []),
        ]
    if primitive.primitive_kind == "loft_profile":
        return [
            "loft_side",
            "seam",
            *(["start_cap"] if primitive.cap_start else []),
            *(["end_cap"] if primitive.cap_end else []),
        ]
    if primitive.primitive_kind == "sweep_profile":
        return [
            "sweep_side",
            "seam",
            *(["start_cap"] if primitive.cap_start and not primitive.path_closed else []),
            *(["end_cap"] if primitive.cap_end and not primitive.path_closed else []),
        ]
    return ["surface"]


def _surface_ranges_for_primitive(primitive: BoxPrimitive) -> List[Dict[str, Any]]:
    if primitive.primitive_kind == "surface_panel":
        return [{"surface_role": "trim", "first_triangle": 0, "triangle_count": 12}]
    if primitive.primitive_kind == "extrude_profile":
        ranges: List[Dict[str, Any]] = []
        cursor = 0
        outer_edges = len(primitive.profile_points) if primitive.profile_closed else len(primitive.profile_points) - 1
        side_count = outer_edges * 2
        ranges.append({"surface_role": "side", "first_triangle": cursor, "triangle_count": side_count})
        cursor += side_count
        if primitive.profile_holes:
            hole_count = sum(len(hole) * 2 for hole in primitive.profile_holes)
            ranges.append({"surface_role": "hole_wall", "first_triangle": cursor, "triangle_count": hole_count})
            cursor += hole_count
        cap_count = len(_triangulate_profile_cap(list(primitive.profile_points), [list(hole) for hole in primitive.profile_holes])) if primitive.profile_closed else 0
        if primitive.cap_start and cap_count:
            ranges.append({"surface_role": "start_cap", "first_triangle": cursor, "triangle_count": cap_count})
            cursor += cap_count
        if primitive.cap_end and cap_count:
            ranges.append({"surface_role": "end_cap", "first_triangle": cursor, "triangle_count": cap_count})
        return ranges
    if primitive.primitive_kind == "revolve_profile":
        side_count = primitive.radial_segments * _revolve_side_triangles_per_strip(primitive.profile_points, primitive.profile_closed)
        ranges = [{"surface_role": "side", "first_triangle": 0, "triangle_count": side_count}]
        # The UV seam exists even for a full revolve but does not add faces.
        ranges.append({"surface_role": "seam", "first_triangle": side_count, "triangle_count": 0})
        cursor = side_count
        full = math.isclose(primitive.revolve_angle, math.pi * 2, abs_tol=1e-9)
        cap_count = max(0, len(_revolve_seam_polygon(primitive.profile_points, primitive.profile_closed)) - 2)
        if not full and primitive.cap_start and cap_count:
            ranges.append({"surface_role": "start_cap", "first_triangle": cursor, "triangle_count": cap_count})
            cursor += cap_count
        if not full and primitive.cap_end and cap_count:
            ranges.append({"surface_role": "end_cap", "first_triangle": cursor, "triangle_count": cap_count})
        return ranges
    if primitive.primitive_kind == "loft_profile":
        ring_count = len(primitive.loft_rings_mm)
        point_count = len(primitive.loft_rings_mm[0]) if ring_count else 0
        side_count = max(0, ring_count - 1) * point_count * 2
        ranges = [
            {"surface_role": "loft_side", "first_triangle": 0, "triangle_count": side_count},
            {"surface_role": "seam", "first_triangle": side_count, "triangle_count": 0},
        ]
        cursor = side_count
        cap_count = max(0, point_count - 2)
        if primitive.cap_start and cap_count:
            ranges.append({"surface_role": "start_cap", "first_triangle": cursor, "triangle_count": cap_count})
            cursor += cap_count
        if primitive.cap_end and cap_count:
            ranges.append({"surface_role": "end_cap", "first_triangle": cursor, "triangle_count": cap_count})
        return ranges
    if primitive.primitive_kind == "sweep_profile":
        edge_count = len(primitive.sweep_path_mm) if primitive.path_closed else max(0, len(primitive.sweep_path_mm) - 1)
        side_count = edge_count * len(primitive.profile_points) * 2
        ranges = [
            {"surface_role": "sweep_side", "first_triangle": 0, "triangle_count": side_count},
            {"surface_role": "seam", "first_triangle": side_count, "triangle_count": 0},
        ]
        cursor = side_count
        cap_count = max(0, len(primitive.profile_points) - 2)
        if primitive.cap_start and not primitive.path_closed and cap_count:
            ranges.append({"surface_role": "start_cap", "first_triangle": cursor, "triangle_count": cap_count})
            cursor += cap_count
        if primitive.cap_end and not primitive.path_closed and cap_count:
            ranges.append({"surface_role": "end_cap", "first_triangle": cursor, "triangle_count": cap_count})
        return ranges
    return [{"surface_role": "surface", "first_triangle": 0, "triangle_count": _primitive_triangle_count(primitive)}]


def _revolve_side_triangles_per_strip(points: Tuple[Tuple[float, float], ...], closed: bool) -> int:
    edge_count = len(points) if closed else len(points) - 1
    triangles = 0
    for index in range(edge_count):
        left = points[index][0]
        right = points[(index + 1) % len(points)][0]
        if left <= 1e-9 and right <= 1e-9:
            continue
        triangles += 1 if left <= 1e-9 or right <= 1e-9 else 2
    return triangles


def _dominant_axis(axis: Any) -> int:
    if not isinstance(axis, list) or len(axis) != 3 or not any(abs(float(value)) > 1e-9 for value in axis):
        raise ValueError("transform axis must be a non-zero 3-vector")
    return max(range(3), key=lambda index: abs(float(axis[index])))


def _loft_axis_point(axis: str, along: float, u: float, v: float) -> Tuple[float, float, float]:
    if axis == "x":
        return along, u, v
    if axis == "y":
        return u, along, v
    if axis == "z":
        return u, v, along
    raise ValueError("loft main axis is invalid")


def _translate_center(center: Tuple[float, float, float], axis: int, distance: float) -> Tuple[float, float, float]:
    translated = list(center)
    translated[axis] += distance
    return tuple(translated)


def _translate_csg_triangle(triangle: Mapping[str, Any], delta: Sequence[float]) -> Dict[str, Any]:
    return {
        **dict(triangle),
        "vertices_mm": [
            [round(float(point[index]) + float(delta[index]), 7) for index in range(3)]
            for point in triangle["vertices_mm"]
        ],
    }


def _translate_primitive(primitive: GeometryPrimitive, axis: int, distance: float) -> GeometryPrimitive:
    if isinstance(primitive, CsgMeshPrimitive):
        delta = [0.0, 0.0, 0.0]
        delta[axis] = distance
        return replace(
            primitive,
            triangles=tuple(_translate_csg_triangle(item, delta) for item in primitive.triangles),
        )
    return replace(primitive, center_mm=_translate_center(primitive.center_mm, axis, distance))


def _mirror_primitive(primitive: GeometryPrimitive, axis: int) -> GeometryPrimitive:
    if isinstance(primitive, CsgMeshPrimitive):
        mirrored = []
        for triangle in primitive.triangles:
            points = [list(point) for point in triangle["vertices_mm"]]
            for point in points:
                point[axis] = -float(point[axis])
            # Reflection flips handedness; reverse winding to keep outward normals.
            mirrored.append({**dict(triangle), "vertices_mm": [points[0], points[2], points[1]]})
        return replace(primitive, triangles=tuple(mirrored))
    center = list(primitive.center_mm)
    center[axis] = -center[axis]
    return replace(primitive, center_mm=tuple(center))


def _primitive_center(primitive: GeometryPrimitive) -> Tuple[float, float, float]:
    if isinstance(primitive, BoxPrimitive):
        return primitive.center_mm
    points = [point for triangle in primitive.triangles for point in triangle["vertices_mm"]]
    return tuple((min(float(point[axis]) for point in points) + max(float(point[axis]) for point in points)) / 2 for axis in range(3))


def _radial_primitive(primitive: GeometryPrimitive, axis: int, radius: float, angle: float) -> GeometryPrimitive:
    center = _primitive_center(primitive)
    cosine = math.cos(angle)
    sine = math.sin(angle)
    if axis == 0:
        radial_a, radial_b = center[1] + radius, center[2]
        rotated = (center[0], radial_a * cosine - radial_b * sine, radial_a * sine + radial_b * cosine)
    elif axis == 2:
        radial_a, radial_b = center[0] + radius, center[1]
        rotated = (radial_a * cosine - radial_b * sine, radial_a * sine + radial_b * cosine, center[2])
    else:
        radial_a, radial_b = center[0] + radius, center[2]
        rotated = (radial_a * cosine - radial_b * sine, center[1], radial_a * sine + radial_b * cosine)
    if isinstance(primitive, CsgMeshPrimitive):
        delta = [rotated[index] - center[index] for index in range(3)]
        return replace(primitive, triangles=tuple(_translate_csg_triangle(item, delta) for item in primitive.triangles))
    return replace(primitive, center_mm=rotated)


def _box_inputs_have_near_coincident_planes(left: BoxPrimitive, right: BoxPrimitive) -> bool:
    for axis in range(3):
        left_planes = (
            left.center_mm[axis] - left.size_mm[axis] / 2,
            left.center_mm[axis] + left.size_mm[axis] / 2,
        )
        right_planes = (
            right.center_mm[axis] - right.size_mm[axis] / 2,
            right.center_mm[axis] + right.size_mm[axis] / 2,
        )
        if any(0 < abs(a - b) < 1e-5 for a in left_planes for b in right_planes):
            return True
    return False


def _program_for_boxes(
    plan: MechanicalConceptPlan,
    direction_id: str,
    boxes: List[BoxPrimitive],
    variant_id: str | None = None,
    presentation_profile: str = "quick_sketch",
) -> Dict[str, Any]:
    operations = []
    outputs = []
    profile_inputs: List[Dict[str, Any]] = []
    for index, box in enumerate(boxes):
        op_id = f"op_{index + 1}_{box.part_role}"
        args = {
            "position": list(box.center_mm),
            "part_role": box.part_role,
            "zone_id": f"zone_{box.part_role}",
            "material_id": _visual_material_id_for_primitive(box),
        }
        inputs: List[str] = []
        if box.primitive_kind == "cylinder":
            op_name = "cylinder"
            args.update({"radius": box.radius_mm, "height": box.height_mm, "axis": list(box.axis)})
        elif box.primitive_kind == "capsule":
            op_name = "capsule"
            args.update({"radius": box.radius_mm, "height": box.height_mm, "axis": list(box.axis)})
        elif box.primitive_kind == "wedge":
            op_name = "wedge"
            args["size"] = list(box.size_mm)
        elif box.primitive_kind == "loft_contract":
            if box.profile_contract is None or any(value <= 0 for value in box.loft_cross_section_scale):
                raise ValueError(f"showcase loft contract is incomplete for {box.part_role}")
            from .profile_contracts import canonical_profile_payload

            canonical, _canonical_json, digest = canonical_profile_payload(box.profile_contract)
            profile_input_id = f"profileinput_{index + 1}_{box.part_role}"
            profile_inputs.append({
                "input_id": profile_input_id,
                "input_kind": "profile_section_set",
                "contract_version": "ProfileSectionSet@1",
                "input_sha256": digest,
                "canonical_payload": canonical,
            })
            op_name = "loft"
            args.update({
                "section_set_input_id": profile_input_id,
                "cross_section_scale": list(box.loft_cross_section_scale),
                "axis_length": box.height_mm,
                "continuity": "linear",
            })
        elif box.primitive_kind == "sweep_contract":
            if (
                box.profile_contract is None
                or any(value <= 0 for value in box.sweep_profile_scale)
                or len(box.sweep_path_mm) < 3
            ):
                raise ValueError(f"showcase sweep contract is incomplete for {box.part_role}")
            from .profile_contracts import canonical_profile_payload

            canonical, _canonical_json, digest = canonical_profile_payload(
                box.profile_contract
            )
            profile_input_id = f"profileinput_{index + 1}_{box.part_role}"
            profile_inputs.append({
                "input_id": profile_input_id,
                "input_kind": "profile_sketch",
                "contract_version": "ProfileSketch@1",
                "input_sha256": digest,
                "canonical_payload": canonical,
            })
            op_name = "sweep"
            args.update({
                "profile_input_id": profile_input_id,
                "profile_scale": list(box.sweep_profile_scale),
                "path_points": [list(point) for point in box.sweep_path_mm],
                "path_closed": False,
                "path_twist_degrees": 0.0,
                "cap_start": box.cap_start,
                "cap_end": box.cap_end,
            })
        elif box.primitive_kind == "revolve":
            op_name = "revolve"
            args.update({"angle": box.revolve_angle, "points": [list(point) for point in box.profile_points]})
            profile_op_id = f"{op_id}_profile"
            operations.append({"operation_id": profile_op_id, "op": "profile", "inputs": [], "args": {"points": [list(point) for point in box.profile_points]}})
            inputs = [profile_op_id]
        else:
            op_name = "box"
            args["size"] = list(box.size_mm)
        operations.append({"operation_id": op_id, "op": op_name, "inputs": inputs, "args": args})
        output_operation_id = op_id
        if presentation_profile == "showcase" and op_name == "box":
            edge_operation_id = f"{op_id}_edge"
            edge_radius = round(min(box.size_mm[0], box.size_mm[2]) * 0.08, 4)
            operations.append({
                "operation_id": edge_operation_id,
                "op": "bevel_approx",
                "inputs": [op_id],
                "args": {
                    "position": list(box.center_mm),
                    "part_role": box.part_role,
                    "zone_id": f"zone_{box.part_role}",
                    "material_id": _visual_material_id_for_primitive(box),
                    "radius": edge_radius,
                    "segments": 3,
                },
            })
            output_operation_id = edge_operation_id
        outputs.append(
            {
                "output_id": f"output_{index + 1}_{box.part_role}",
                "operation_id": output_operation_id,
                "kind": "mesh",
                "part_role": box.part_role,
            }
        )
    program = {
        "schema_version": "ShapeProgram@1",
        "program_id": f"shape_{plan.domain_pack_id.removeprefix('pack_')}_{direction_id}{('_' + variant_id) if variant_id else ''}{'_showcase' if presentation_profile == 'showcase' else ''}",
        "units": "millimeter",
        "seed": 7,
        "triangle_budget": 100000,
        "parameters": [],
        "operations": operations,
        "outputs": outputs,
        "non_functional_only": True,
    }
    if profile_inputs:
        program["profile_inputs"] = profile_inputs
    return program


def _assembly_graph(
    plan: MechanicalConceptPlan,
    direction_id: str,
    boxes: List[BoxPrimitive],
    variant_id: str | None = None,
    presentation_profile: str = "quick_sketch",
) -> Dict[str, Any]:
    parts = []
    connections = []
    for index, box in enumerate(boxes):
        part_id = f"part_{index + 1}_{box.part_role}"
        is_visual_detail = box.part_role.startswith("visual_")
        parent_index = index - 1 if plan.domain_pack_id == "pack_robotic_arm_concept" and index > 0 and not is_visual_detail else 0
        parent_part_id = None if index == 0 else f"part_{parent_index + 1}_{boxes[parent_index].part_role}"
        mount_connector_id = f"connector_{part_id}_mount"
        connectors = [
            {
                "connector_id": mount_connector_id,
                "kind": "surface_mount" if index == 0 or is_visual_detail else "axial_mount",
                "position": [0, 0, 0],
                "normal": [0, 1, 0],
            }
        ]
        joints = []
        if plan.domain_pack_id == "pack_robotic_arm_concept" and index > 0 and not is_visual_detail:
            joints.append(
                {
                    "joint_id": f"joint_{boxes[parent_index].part_role}_{box.part_role}",
                    "kind": "revolute",
                    "target_part_id": part_id,
                    "axis": [0, 0, 1],
                    "min_value": -1.5708,
                    "max_value": 1.5708,
                }
            )
        parts.append(
            {
                "part_id": part_id,
                "role": box.part_role,
                "parent_part_id": parent_part_id,
                "geometry_source": "shape_program",
                "transform": {"position": list(box.center_mm), "rotation": [0, 0, 0], "scale": [1, 1, 1]},
                "connectors": connectors,
                "joints": joints,
                "material_zones": [f"zone_{box.part_role}"],
                "editable_parameters": [] if is_visual_detail else ["transform.position", "transform.scale", *( ["joint.rotation"] if joints else [] )],
                "locked": False,
                "provenance": "agent_generated",
            }
        )
        if index > 0:
            parent_part_id = parts[parent_index]["part_id"]
            parent_center = boxes[parent_index].center_mm
            child_center = box.center_mm
            parent_connector_id = f"connector_{parent_part_id}_to_{part_id}"
            parts[parent_index]["connectors"].append(
                {
                    "connector_id": parent_connector_id,
                    "kind": "axial_mount",
                    "position": [round(child_center[axis] - parent_center[axis], 4) for axis in range(3)],
                    "normal": [0, 1, 0],
                }
            )
            connections.append(
                {
                    "connection_id": f"conn_{parent_part_id}_{part_id}",
                    "from_part_id": parent_part_id,
                    "from_connector_id": parent_connector_id,
                    "to_part_id": part_id,
                    "to_connector_id": mount_connector_id,
                    "status": "connected",
                }
            )
    return {
        "schema_version": "AssemblyGraph@1",
        "graph_id": f"mg_{plan.plan_id.removeprefix('plan_')}_{direction_id}{('_' + variant_id) if variant_id else ''}{'_showcase' if presentation_profile == 'showcase' else ''}",
        "concept_id": str(plan.spec.get("concept_id", "asset_agent_plan")),
        "root_part_id": parts[0]["part_id"],
        "parts": parts,
        "connections": connections,
    }


def _material_id_for_index(index: int) -> str:
    return {
        0: "mat_primary",
        1: "mat_aluminum",
        2: "mat_signal_red",
        3: "mat_composite",
        4: "mat_rubber",
        5: "mat_dark_glass",
        6: "mat_emissive_blue",
        7: "mat_automotive_paint",
    }.get(index, "mat_primary")


def _visual_material_id_for_primitive(primitive: BoxPrimitive) -> str:
    """Preserve authored material intent and apply reviewed visual role aliases.

    The variant catalog predates explicit material ids, so its numeric material
    field must not disappear when converted into ShapeProgram.  A small fixed
    role table improves visual readability without exposing an engineering
    material inference path or any user-controlled executable behavior.
    """
    if primitive.material_id is not None:
        return primitive.material_id
    role = primitive.part_role.lower()
    if any(token in role for token in ("wheel", "track", "tire", "grip", "foot")):
        return "mat_rubber"
    if any(token in role for token in ("canopy", "cockpit", "glass", "window", "transparent")):
        return "mat_dark_glass"
    if any(token in role for token in ("light", "lamp", "emissive")):
        return "mat_emissive_blue"
    if any(token in role for token in ("joint", "rotor", "nacelle", "ring", "pivot", "wrist", "turntable")):
        return "mat_aluminum"
    return _material_id_for_index(primitive.material_index)


def _primitive_csg_solid(primitive: GeometryPrimitive) -> Dict[str, Any]:
    if isinstance(primitive, CsgMeshPrimitive):
        return {"triangles": [dict(item) for item in primitive.triangles]}
    positions, _normals, _uvs, indices, _lower, _upper = _primitive_geometry(primitive)
    vertices = [
        [round(float(value) * 1000, 7) for value in struct.unpack_from("<3f", positions, offset)]
        for offset in range(0, len(positions), 12)
    ]
    index_values = list(struct.unpack(f"<{len(indices) // 2}H", indices))
    ranges = _surface_ranges_for_primitive(primitive)

    def surface_role(face_id: int) -> str:
        for item in ranges:
            first = int(item["first_triangle"])
            if first <= face_id < first + int(item["triangle_count"]):
                return str(item["surface_role"])
        return "surface"

    triangles = []
    for face_id, offset in enumerate(range(0, len(index_values), 3)):
        triangles.append({
            "vertices_mm": [vertices[index_values[offset + index]] for index in range(3)],
            "source_operation_id": primitive.source_operation_id or "op_source",
            "source_part_role": primitive.part_role,
            "material_id": primitive.material_id or _material_id_for_index(primitive.material_index),
            "material_zone_id": primitive.material_zone_id or f"zone_{primitive.part_role}",
            "source_face_id": face_id,
            "boolean_backside": False,
            "surface_role": surface_role(face_id),
        })
    return {"triangles": triangles}


def _solid_is_closed(solid: Mapping[str, Any]) -> bool:
    edges: Dict[Tuple[Tuple[float, float, float], Tuple[float, float, float]], int] = {}
    for triangle in solid.get("triangles", []):
        points = [tuple(round(float(value), 6) for value in point) for point in triangle["vertices_mm"]]
        if len(set(points)) != 3:
            return False
        for left, right in ((points[0], points[1]), (points[1], points[2]), (points[2], points[0])):
            edge = tuple(sorted((left, right)))
            edges[edge] = edges.get(edge, 0) + 1
    return bool(edges) and all(count == 2 for count in edges.values())


def _material_ids_for_primitives(primitives: Sequence[GeometryPrimitive]) -> set[str]:
    result: set[str] = set()
    for primitive in primitives:
        if isinstance(primitive, BoxPrimitive):
            result.add(primitive.material_id or _material_id_for_index(primitive.material_index))
        else:
            result.update(str(item["material_id"]) for item in primitive.triangles)
    return result


def _operation_has_non_identity_rotation(args: Mapping[str, Any]) -> bool:
    rotation = args.get("rotation", [0.0, 0.0, 0.0])
    if not isinstance(rotation, list) or len(rotation) != 3:
        return True
    try:
        return any(abs(float(value)) > 1e-12 for value in rotation)
    except (TypeError, ValueError):
        return True


def _apply_static_operation_rotation(
    primitives: Sequence[GeometryPrimitive],
    args: Mapping[str, Any],
    operation_id: str,
    op: Any,
) -> List[GeometryPrimitive]:
    rotation = args.get("rotation", [0.0, 0.0, 0.0])
    if not isinstance(rotation, list) or len(rotation) != 3:
        raise _runtime_operation_error(operation_id, op, "rotation must be a three-number vector")
    try:
        radians = tuple(float(value) for value in rotation)
    except (TypeError, ValueError) as exc:
        raise _runtime_operation_error(operation_id, op, "rotation must be a three-number vector") from exc
    if not all(math.isfinite(value) and abs(value) <= math.pi + 1e-9 for value in radians):
        raise _runtime_operation_error(operation_id, op, "rotation must be finite and within one Euler turn")
    return [
        replace(item, rotation_radians=radians, rotation_origin_mm=item.center_mm)
        if isinstance(item, BoxPrimitive) else item
        for item in primitives
    ]


def _primitive_has_static_rotation(primitive: GeometryPrimitive) -> bool:
    return isinstance(primitive, BoxPrimitive) and any(
        abs(value) > 1e-12 for value in primitive.rotation_radians
    )


def _rotation_matrix_xyz(rotation: Tuple[float, float, float]) -> Tuple[Tuple[float, float, float], ...]:
    rx, ry, rz = rotation
    sx, cx = math.sin(rx), math.cos(rx)
    sy, cy = math.sin(ry), math.cos(ry)
    sz, cz = math.sin(rz), math.cos(rz)
    return (
        (cy * cz, cz * sx * sy - cx * sz, sx * sz + cx * cz * sy),
        (cy * sz, cx * cz + sx * sy * sz, cx * sy * sz - cz * sx),
        (-sy, cy * sx, cx * cy),
    )


def _apply_static_rotation_to_geometry(
    primitive: BoxPrimitive,
    positions: bytes,
    normals: bytes,
    lower: List[float],
    upper: List[float],
) -> Tuple[bytes, bytes, List[float], List[float]]:
    if not _primitive_has_static_rotation(primitive):
        return positions, normals, lower, upper
    matrix = _rotation_matrix_xyz(primitive.rotation_radians)
    origin = primitive.rotation_origin_mm or primitive.center_mm
    origin_m = tuple(float(value) / 1000 for value in origin)
    position_values = list(struct.unpack(f"<{len(positions) // 4}f", positions))
    normal_values = list(struct.unpack(f"<{len(normals) // 4}f", normals))

    def rotate(vector: Tuple[float, float, float]) -> Tuple[float, float, float]:
        return tuple(sum(matrix[row][column] * vector[column] for column in range(3)) for row in range(3))

    for offset in range(0, len(position_values), 3):
        relative = tuple(position_values[offset + axis] - origin_m[axis] for axis in range(3))
        rotated = rotate(relative)
        for axis in range(3):
            position_values[offset + axis] = origin_m[axis] + rotated[axis]
    for offset in range(0, len(normal_values), 3):
        rotated = rotate(tuple(normal_values[offset + axis] for axis in range(3)))
        length = math.sqrt(sum(value * value for value in rotated))
        if length <= 1e-12 or not math.isfinite(length):
            raise ValueError("STATIC_ROTATION_NORMAL_INVALID")
        for axis in range(3):
            normal_values[offset + axis] = rotated[axis] / length
    return (
        struct.pack(f"<{len(position_values)}f", *position_values),
        struct.pack(f"<{len(normal_values)}f", *normal_values),
        [min(position_values[axis::3]) for axis in range(3)],
        [max(position_values[axis::3]) for axis in range(3)],
    )


def _feature_node_readback(
    *,
    operation: Mapping[str, Any],
    primitives: Sequence[GeometryPrimitive],
    input_hashes: Sequence[str],
    csg_depth: int,
    artifact_profile_sha256: str,
) -> Dict[str, Any]:
    operation_id = str(operation["operation_id"])
    op = str(operation["op"])
    parameters_sha256 = hashlib.sha256(_canonical_json(operation.get("args", {})).encode()).hexdigest()
    kernel_id = MANIFOLD_KERNEL_ID if op in {"union", "subtract"} else "forgecad_builtin"
    kernel_version = MANIFOLD_KERNEL_VERSION if kernel_id == MANIFOLD_KERNEL_ID else "ShapeProgramRuntimeManifest@1"
    node_input = {
        "node_id": operation_id,
        "operation": op,
        "input_node_ids": list(operation.get("inputs", [])),
        "input_hashes": list(input_hashes),
        "parameters_sha256": parameters_sha256,
        "runtime_manifest_version": MANIFEST_SCHEMA_VERSION,
        "artifact_profile_sha256": artifact_profile_sha256,
        "kernel_id": kernel_id,
        "kernel_version": kernel_version,
    }
    node_input_sha256 = hashlib.sha256(_canonical_json(node_input).encode()).hexdigest()
    solids = [_primitive_csg_solid(item) for item in primitives]
    geometry_payload = [solid["triangles"] for solid in solids]
    result_sha256 = hashlib.sha256(_canonical_json({"node_input_sha256": node_input_sha256, "geometry": geometry_payload}).encode()).hexdigest()
    provenance = [
        {
            key: value
            for key, value in triangle.items()
            if key != "vertices_mm"
        }
        for solid in solids
        for triangle in solid["triangles"]
    ]
    material_ids = sorted({str(item["material_id"]) for item in provenance})
    material_zone_ids = sorted({str(item["material_zone_id"]) for item in provenance})
    surface_roles = sorted({str(item["surface_role"]) for item in provenance})
    return {
        "schema_version": "GeometryFeatureNodeReadback@1",
        "node_id": operation_id,
        "operation": op,
        "input_node_ids": list(operation.get("inputs", [])),
        "input_hashes": list(input_hashes),
        "parameters_sha256": parameters_sha256,
        "node_input_sha256": node_input_sha256,
        "result_sha256": result_sha256,
        "surface_provenance_sha256": hashlib.sha256(_canonical_json(provenance).encode()).hexdigest(),
        "runtime_manifest_version": MANIFEST_SCHEMA_VERSION,
        "kernel_id": kernel_id,
        "kernel_version": kernel_version,
        "csg_depth": csg_depth,
        "result_triangle_count": len(provenance),
        "result_closed": (
            True
            if primitives and all(isinstance(item, CsgMeshPrimitive) for item in primitives)
            else (all(_solid_is_closed(item) for item in solids) if solids else False)
        ),
        "material_ids": material_ids,
        "material_zone_ids": material_zone_ids,
        "surface_roles": surface_roles,
    }


def _csg_glb_payloads(primitive: CsgMeshPrimitive) -> List[Dict[str, Any]]:
    grouped: Dict[Tuple[Any, ...], List[Mapping[str, Any]]] = {}
    for triangle in primitive.triangles:
        key = (
            str(triangle["source_operation_id"]),
            str(triangle["material_id"]),
            str(triangle["material_zone_id"]),
            str(triangle["surface_role"]),
            bool(triangle["boolean_backside"]),
        )
        grouped.setdefault(key, []).append(triangle)
    payloads = []
    for key in sorted(grouped):
        source_operation_id, material_id, zone_id, surface_role, backside = key
        positions: List[float] = []
        normals: List[float] = []
        uvs: List[float] = []
        indices: List[int] = []
        source_face_ids: List[int] = []
        for triangle in grouped[key]:
            points = [[float(value) / 1000 for value in point] for point in triangle["vertices_mm"]]
            ab = [points[1][index] - points[0][index] for index in range(3)]
            ac = [points[2][index] - points[0][index] for index in range(3)]
            normal = [
                ab[1] * ac[2] - ab[2] * ac[1],
                ab[2] * ac[0] - ab[0] * ac[2],
                ab[0] * ac[1] - ab[1] * ac[0],
            ]
            length = math.sqrt(sum(value * value for value in normal))
            if length <= 1e-12:
                raise ManifoldCsgError("CSG_DEGENERATE_OUTPUT", primitive.feature_node_id, "degenerate triangle rejected before GLB")
            normal = [value / length for value in normal]
            base = len(positions) // 3
            for point in points:
                positions.extend(point)
                normals.extend(normal)
            uvs.extend((0, 0, 1, 0, 0, 1))
            indices.extend((base, base + 1, base + 2))
            source_face_ids.append(int(triangle["source_face_id"]))
        axes = [[positions[offset + axis] for offset in range(0, len(positions), 3)] for axis in range(3)]
        index_component = 5125 if len(positions) // 3 > 65535 else 5123
        index_format = "I" if index_component == 5125 else "H"
        payloads.append({
            "positions": struct.pack(f"<{len(positions)}f", *positions),
            "normals": struct.pack(f"<{len(normals)}f", *normals),
            "uvs": struct.pack(f"<{len(uvs)}f", *uvs),
            "indices": struct.pack(f"<{len(indices)}{index_format}", *indices),
            "index_component": index_component,
            "minimum": [min(values) for values in axes],
            "maximum": [max(values) for values in axes],
            "material_index": _material_index(material_id),
            "extras": {
                "forgecad_part_role": primitive.part_role,
                "forgecad_material_id": material_id,
                "forgecad_profile_input_id": None,
                "forgecad_feature_node_id": primitive.feature_node_id,
                "forgecad_surface_roles": [surface_role],
                "forgecad_surface_ranges": [{"surface_role": surface_role, "first_triangle": 0, "triangle_count": len(indices) // 3}],
                "forgecad_material_zone_id": zone_id,
                "forgecad_normal_mode": "split",
                "forgecad_edge_finish": {
                    "mode": "none",
                    "edge_set": "none",
                    "selected_edge_count": 0,
                    "radius_ratio": 0,
                    "subdivision_count": 0,
                },
                "forgecad_source_face_ids": source_face_ids,
                "forgecad_csg_provenance": {
                    "source_operation_ids": [source_operation_id],
                    "material_zone_id": zone_id,
                    "boolean_backside": backside,
                    "source_face_ids": sorted(set(source_face_ids)),
                },
            },
        })
    return payloads


def _glb_payloads_for_primitive(primitive: GeometryPrimitive) -> List[Dict[str, Any]]:
    if isinstance(primitive, CsgMeshPrimitive):
        return _csg_glb_payloads(primitive)
    positions, normals, uvs, indices, lower, upper = _primitive_geometry(primitive)
    positions, normals, lower, upper = _apply_static_rotation_to_geometry(
        primitive, positions, normals, lower, upper,
    )
    weighted_kinds = {
        "cylinder",
        "capsule",
        "revolve",
        "revolve_profile",
        "sweep_profile",
        "bevel_box",
        *(["loft_profile"] if primitive.smooth_normals else []),
    }
    if primitive.primitive_kind == "bevel_box":
        edge_finish = {
            "mode": "bevel_approximation",
            "edge_set": "xz_perimeter",
            "selected_edge_count": 4,
            "radius_ratio": round(
                primitive.bevel_radius_mm / min(primitive.size_mm[0], primitive.size_mm[2]),
                8,
            ),
            "subdivision_count": primitive.bevel_segments,
        }
    else:
        edge_finish = {
            "mode": "none",
            "edge_set": "none",
            "selected_edge_count": 0,
            "radius_ratio": 0,
            "subdivision_count": 0,
        }
    return [{
        "positions": positions,
        "normals": normals,
        "uvs": uvs,
        "indices": indices,
        "index_component": 5123,
        "minimum": lower,
        "maximum": upper,
        "material_index": primitive.material_index,
        "extras": {
            "forgecad_part_role": primitive.part_role,
            "forgecad_material_id": primitive.material_id or _material_id_for_index(primitive.material_index),
            "forgecad_profile_input_id": primitive.profile_input_id,
            "forgecad_feature_node_id": primitive.source_operation_id,
            "forgecad_surface_roles": _surface_roles_for_primitive(primitive),
            "forgecad_surface_ranges": _surface_ranges_for_primitive(primitive),
            "forgecad_material_zone_id": primitive.material_zone_id,
            "forgecad_normal_mode": "split_weighted" if primitive.primitive_kind in weighted_kinds else "split",
            "forgecad_visual_uv_repeat_mm": VISUAL_UV_REPEAT_MM if primitive.primitive_kind in {
                "box", "surface_panel", "wedge", "cylinder", "capsule", "bevel_box",
                "loft_profile", "sweep_profile",
            } else None,
            "forgecad_edge_finish": edge_finish,
            "forgecad_source_face_ids": list(range(_primitive_triangle_count(primitive))),
        },
    }]


def _surface_complete_payload(
    payload: Mapping[str, Any],
    *,
    part_instance_id: str,
    primitive_id: str,
) -> Dict[str, Any]:
    """Split triangles and attach tangent + stable face ids before GLB write.

    Face ids are vertex attributes, so a later vertex/index reorder preserves
    the face-to-part/zone contract.  Vertices are deliberately split per face
    to prevent an optimizer from merging different provenance identities.
    """

    positions = list(struct.unpack(f"<{len(payload['positions']) // 4}f", payload["positions"]))
    normals = list(struct.unpack(f"<{len(payload['normals']) // 4}f", payload["normals"]))
    uvs = list(struct.unpack(f"<{len(payload['uvs']) // 4}f", payload["uvs"]))
    index_component = int(payload["index_component"])
    index_format, index_size = {5123: ("H", 2), 5125: ("I", 4)}.get(index_component, (None, None))
    if index_format is None or index_size is None:
        raise ValueError("surface completion only supports uint16/uint32 indices")
    indices = list(struct.unpack(f"<{len(payload['indices']) // index_size}{index_format}", payload["indices"]))
    if len(indices) % 3:
        raise ValueError("surface completion requires triangle indices")
    extras = dict(payload["extras"])
    source_face_ids = extras.pop("forgecad_source_face_ids", None)
    triangle_count = len(indices) // 3
    if not isinstance(source_face_ids, list) or len(source_face_ids) != triangle_count:
        raise ValueError("surface completion source-face provenance does not align")
    if not isinstance(extras.get("forgecad_material_zone_id"), str):
        raise ValueError("surface completion requires a stable material zone")

    out_positions: List[float] = []
    out_normals: List[float] = []
    out_uvs: List[float] = []
    out_tangents: List[float] = []
    out_face_ids: List[int] = []
    out_source_face_ids: List[int] = []
    out_indices: List[int] = []

    def vector(values: Sequence[float], index: int, width: int) -> Tuple[float, ...]:
        return tuple(float(value) for value in values[index * width:(index + 1) * width])

    for face_id, offset in enumerate(range(0, len(indices), 3)):
        vertex_ids = indices[offset:offset + 3]
        points = [vector(positions, index, 3) for index in vertex_ids]
        triangle_uvs = [vector(uvs, index, 2) for index in vertex_ids]
        edge_one = tuple(points[1][axis] - points[0][axis] for axis in range(3))
        edge_two = tuple(points[2][axis] - points[0][axis] for axis in range(3))
        du_one = triangle_uvs[1][0] - triangle_uvs[0][0]
        dv_one = triangle_uvs[1][1] - triangle_uvs[0][1]
        du_two = triangle_uvs[2][0] - triangle_uvs[0][0]
        dv_two = triangle_uvs[2][1] - triangle_uvs[0][1]
        determinant = du_one * dv_two - dv_one * du_two
        if abs(determinant) <= 1e-12:
            raise ValueError("SURFACE_UV_DEGENERATE: tangent basis cannot be derived")
        inverse = 1.0 / determinant
        raw_tangent = tuple(inverse * (dv_two * edge_one[axis] - dv_one * edge_two[axis]) for axis in range(3))
        raw_bitangent = tuple(inverse * (-du_two * edge_one[axis] + du_one * edge_two[axis]) for axis in range(3))
        for local_index, vertex_id in enumerate(vertex_ids):
            normal = vector(normals, vertex_id, 3)
            normal_length = math.sqrt(sum(value * value for value in normal))
            if normal_length <= 1e-12:
                raise ValueError("SURFACE_NORMAL_DEGENERATE: tangent basis cannot be derived")
            normal = tuple(value / normal_length for value in normal)
            projection = sum(normal[axis] * raw_tangent[axis] for axis in range(3))
            tangent = tuple(raw_tangent[axis] - normal[axis] * projection for axis in range(3))
            tangent_length = math.sqrt(sum(value * value for value in tangent))
            if tangent_length <= 1e-12:
                tangent = (
                    raw_bitangent[1] * normal[2] - raw_bitangent[2] * normal[1],
                    raw_bitangent[2] * normal[0] - raw_bitangent[0] * normal[2],
                    raw_bitangent[0] * normal[1] - raw_bitangent[1] * normal[0],
                )
                tangent_length = math.sqrt(sum(value * value for value in tangent))
            if tangent_length <= 1e-12:
                raise ValueError("SURFACE_TANGENT_DEGENERATE: tangent basis cannot be derived")
            tangent = tuple(value / tangent_length for value in tangent)
            cross_normal_tangent = (
                normal[1] * tangent[2] - normal[2] * tangent[1],
                normal[2] * tangent[0] - normal[0] * tangent[2],
                normal[0] * tangent[1] - normal[1] * tangent[0],
            )
            handedness = -1.0 if sum(cross_normal_tangent[axis] * raw_bitangent[axis] for axis in range(3)) < 0 else 1.0
            out_positions.extend(points[local_index])
            out_normals.extend(normal)
            out_uvs.extend(triangle_uvs[local_index])
            out_tangents.extend((*tangent, handedness))
            out_face_ids.append(face_id)
            out_source_face_ids.append(int(source_face_ids[face_id]))
            out_indices.append(len(out_indices))

    completed_index_component = 5125 if len(out_positions) // 3 > 65535 else 5123
    completed_index_format = "I" if completed_index_component == 5125 else "H"
    extras.update({
        "forgecad_primitive_id": primitive_id,
        "forgecad_part_instance_id": part_instance_id,
    })
    return {
        **dict(payload),
        "positions": struct.pack(f"<{len(out_positions)}f", *out_positions),
        "normals": struct.pack(f"<{len(out_normals)}f", *out_normals),
        "uvs": struct.pack(f"<{len(out_uvs)}f", *out_uvs),
        "tangents": struct.pack(f"<{len(out_tangents)}f", *out_tangents),
        # Standard glTF mesh attributes cannot use UNSIGNED_INT.  Float keeps
        # the bounded per-primitive IDs exact while remaining a valid custom
        # vertex attribute component type.
        "face_ids": struct.pack(f"<{len(out_face_ids)}f", *out_face_ids),
        "source_face_ids": struct.pack(f"<{len(out_source_face_ids)}f", *out_source_face_ids),
        "indices": struct.pack(f"<{len(out_indices)}{completed_index_format}", *out_indices),
        "index_component": completed_index_component,
        "extras": extras,
    }


def _build_glb(
    boxes: Sequence[GeometryPrimitive],
    *,
    feature_history: Sequence[Mapping[str, Any]],
    artifact_profile_id: GeometryArtifactProfileId,
    surface_adornments_by_zone: Mapping[str, Mapping[str, object]],
    surface_layers_by_zone: Mapping[str, Mapping[str, object]],
) -> Tuple[bytes, List[float]]:
    binary = bytearray()
    views: List[Dict[str, Any]] = []
    accessors: List[Dict[str, Any]] = []
    primitives: List[Dict[str, Any]] = []
    used_material_indices: set[int] = set()
    minimum = [float("inf")] * 3
    maximum = [float("-inf")] * 3
    sorted_surface_adornments = tuple(sorted(
        surface_adornments_by_zone.values(),
        key=lambda item: (str(item["program_id"]), surface_adornment_material_id(item)),
    ))
    adornment_index_by_zone = {
        str(program["target_zone_id"]): builtin_visual_material_count() + index
        for index, program in enumerate(sorted_surface_adornments)
    }
    sorted_surface_layers = tuple(sorted(
        surface_layers_by_zone.values(),
        key=lambda item: surface_layer_material_id(item),
    ))
    surface_layer_index_by_zone = {
        str(layer["adornments"][0]["target_zone_id"]): (
            builtin_visual_material_count() + len(sorted_surface_adornments) + index
        )
        for index, layer in enumerate(sorted_surface_layers)
    }
    for box_index, box in enumerate(boxes):
        part_role = box.part_role
        part_instance_id = f"partface_{part_role}_{box_index:04d}"
        for payload_index, raw_payload in enumerate(_glb_payloads_for_primitive(box)):
            zone_id = raw_payload.get("extras", {}).get("forgecad_material_zone_id")
            adornment = (
                surface_adornments_by_zone.get(zone_id)
                if isinstance(zone_id, str)
                else None
            )
            surface_layer = (
                surface_layers_by_zone.get(zone_id)
                if isinstance(zone_id, str)
                else None
            )
            if surface_layer is not None:
                raw_payload = dict(raw_payload)
                raw_payload["material_index"] = surface_layer_index_by_zone[zone_id]
                extras = dict(raw_payload["extras"])
                base_material_id = str(surface_layer["adornments"][0]["base_material"])
                extras["forgecad_base_material_id"] = extras["forgecad_material_id"]
                if extras["forgecad_base_material_id"] != base_material_id:
                    raise ValueError("surface layer target zone lost its exact base material")
                extras["forgecad_material_id"] = surface_layer_material_id(surface_layer)
                extras["forgecad_surface_layer_lowering"] = dict(surface_layer)
                extras["forgecad_surface_layer_lowering_sha256"] = surface_layer_lowering_sha256(surface_layer)
                extras["forgecad_surface_layer_retained_layers_sha256"] = surface_layer["retained_layers_sha256"]
                raw_payload["extras"] = extras
            elif adornment is not None:
                raw_payload = dict(raw_payload)
                raw_payload["material_index"] = adornment_index_by_zone[zone_id]
                extras = dict(raw_payload["extras"])
                extras["forgecad_base_material_id"] = extras["forgecad_material_id"]
                extras["forgecad_material_id"] = surface_adornment_material_id(adornment)
                extras["forgecad_surface_adornment"] = dict(adornment)
                extras["forgecad_surface_adornment_sha256"] = surface_adornment_program_sha256(adornment)
                raw_payload["extras"] = extras
            payload = _surface_complete_payload(
                raw_payload,
                part_instance_id=part_instance_id,
                primitive_id=f"primitive_{box_index:04d}_{payload_index:03d}",
            )
            positions = payload["positions"]
            normals = payload["normals"]
            uvs = payload["uvs"]
            tangents = payload["tangents"]
            face_ids = payload["face_ids"]
            source_face_ids = payload["source_face_ids"]
            indices = payload["indices"]
            box_min = payload["minimum"]
            box_max = payload["maximum"]
            for axis in range(3):
                minimum[axis] = min(minimum[axis], box_min[axis])
                maximum[axis] = max(maximum[axis], box_max[axis])
            p = _add_accessor(binary, views, accessors, positions, 5126, len(positions) // 12, "VEC3", box_min, box_max)
            n = _add_accessor(binary, views, accessors, normals, 5126, len(normals) // 12, "VEC3")
            uv = _add_accessor(binary, views, accessors, uvs, 5126, len(uvs) // 8, "VEC2")
            tangent = _add_accessor(binary, views, accessors, tangents, 5126, len(tangents) // 16, "VEC4")
            face_id = _add_accessor(binary, views, accessors, face_ids, 5126, len(face_ids) // 4, "SCALAR")
            source_face_id = _add_accessor(binary, views, accessors, source_face_ids, 5126, len(source_face_ids) // 4, "SCALAR")
            index_component = int(payload["index_component"])
            index_size = 4 if index_component == 5125 else 2
            ix = _add_accessor(binary, views, accessors, indices, index_component, len(indices) // index_size, "SCALAR", target=34963)
            primitives.append({
                "attributes": {
                    "POSITION": p,
                    "NORMAL": n,
                    "TEXCOORD_0": uv,
                    "TANGENT": tangent,
                    "_FORGECAD_FACE_ID": face_id,
                    "_FORGECAD_SOURCE_FACE_ID": source_face_id,
                },
                "indices": ix,
                "material": payload["material_index"],
                "mode": 4,
                "extras": payload["extras"],
            })
            used_material_indices.add(int(payload["material_index"]))
    visual_resources = _append_visual_pbr_resources(
        binary,
        views,
        artifact_profile_id=artifact_profile_id,
        used_material_indices=used_material_indices,
        surface_adornments=sorted_surface_adornments,
        surface_layers=sorted_surface_layers,
    )
    document: Dict[str, Any] = {
        "asset": {"version": "2.0", "generator": "ForgeCAD ShapeProgram surface-complete/1"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"name": "FORGECAD_BLOCKOUT", "mesh": 0}],
        "meshes": [{"name": "FORGECAD_SHAPE_PROGRAM_MESH", "primitives": primitives}],
        "materials": visual_resources["materials"],
        "images": visual_resources["images"],
        "textures": visual_resources["textures"],
        "buffers": [{"byteLength": len(binary)}],
        "bufferViews": views,
        "accessors": accessors,
        "extras": {
            "forgecad_feature_history": list(feature_history),
            "forgecad_visual_environment": studio_environment_manifest(),
            "forgecad_geometry_artifact_profile": geometry_artifact_profile_manifest(
                artifact_profile_id
            ),
        },
    }
    if visual_resources["extensions_used"]:
        document["extensionsUsed"] = visual_resources["extensions_used"]
    json_chunk = json.dumps(document, separators=(",", ":")).encode("utf-8")
    json_chunk += b" " * ((4 - len(json_chunk) % 4) % 4)
    binary.extend(b"\x00" * ((4 - len(binary) % 4) % 4))
    total_length = 12 + 8 + len(json_chunk) + 8 + len(binary)
    payload = (
        struct.pack("<4sII", b"glTF", 2, total_length)
        + struct.pack("<II", len(json_chunk), 0x4E4F534A)
        + json_chunk
        + struct.pack("<II", len(binary), 0x004E4942)
        + bytes(binary)
    )
    return payload, [round((maximum[i] - minimum[i]) * 1000, 4) for i in range(3)]


def _append_visual_pbr_resources(
    binary: bytearray,
    views: List[Dict[str, Any]],
    *,
    artifact_profile_id: GeometryArtifactProfileId,
    used_material_indices: set[int],
    surface_adornments: Sequence[Mapping[str, object]],
    surface_layers: Sequence[Mapping[str, object]],
) -> Dict[str, Any]:
    """Embed complete visual-only PBR maps in the same GLB as geometry.

    Geometry emits a fixed, bounded material-index table.  A user-facing
    material id may alias an index (for example ``mat_graphite`` uses the
    primary visual set); primitive extras preserve that exact id while this
    table remains the single texture byte source.
    """

    images: List[Dict[str, Any]] = []
    textures: List[Dict[str, Any]] = []
    materials: List[Dict[str, Any]] = []
    extensions_used: set[str] = set()
    for material_index in range(builtin_visual_material_count()):
        properties = builtin_material_properties(material_index)
        if (
            artifact_profile_id == "production_concept"
            and material_index not in used_material_indices
        ):
            materials.append({
                "name": f"MAT_UNUSED_{str(properties['material_id']).removeprefix('mat_')}",
                "pbrMetallicRoughness": {
                    "baseColorFactor": [1, 1, 1, 1],
                    "metallicFactor": 0,
                    "roughnessFactor": 1,
                },
                "extras": {
                    "forgecad_visual_only": True,
                    "forgecad_unused_material_placeholder": True,
                },
            })
            continue
        texture_set = builtin_visual_texture_set_for_material_index(
            material_index,
            artifact_profile_id=artifact_profile_id,
        )
        texture_by_role: Dict[str, int] = {}
        for texture_map in texture_set.maps:
            payload = visual_texture_png_bytes(texture_map.texture_id)
            if hashlib.sha256(payload).hexdigest() != texture_map.sha256 or len(payload) != texture_map.byte_size:
                raise ValueError("built-in visual texture bytes do not match their manifest")
            view_index = _add_binary_view(binary, views, payload)
            image_index = len(images)
            images.append({
                "name": texture_map.texture_id,
                "bufferView": view_index,
                "mimeType": texture_map.mime_type,
                "extras": {"forgecad_visual_texture": texture_map.model_dump(mode="json")},
            })
            texture_index = len(textures)
            textures.append({"name": texture_map.texture_id, "source": image_index})
            texture_by_role[texture_map.texture_role] = texture_index
        pbr: Dict[str, Any] = {
            "baseColorFactor": [1, 1, 1, float(properties["alpha"])],
            "metallicFactor": 1,
            "roughnessFactor": 1,
            "baseColorTexture": {"index": texture_by_role["base_color"]},
            "metallicRoughnessTexture": {"index": texture_by_role["metallic_roughness"]},
        }
        material: Dict[str, Any] = {
            "name": f"MAT_{texture_set.material_id.removeprefix('mat_')}",
            "pbrMetallicRoughness": pbr,
            "normalTexture": {"index": texture_by_role["normal"], "scale": float(properties["normal_scale"])},
            "occlusionTexture": {"index": texture_by_role["occlusion"], "strength": 1},
            "emissiveTexture": {"index": texture_by_role["emissive"]},
            "emissiveFactor": [1, 1, 1],
            "extras": {
                "forgecad_visual_texture_set_id": texture_set.visual_texture_set_id,
                "forgecad_texture_material_id": texture_set.material_id,
                "forgecad_visual_only": True,
            },
        }
        if float(properties["alpha"]) < 1:
            material["alphaMode"] = "BLEND"
        extensions: Dict[str, Any] = {}
        if float(properties["clearcoat"]) > 0:
            extensions["KHR_materials_clearcoat"] = {
                "clearcoatFactor": float(properties["clearcoat"]),
                "clearcoatRoughnessFactor": 1,
                "clearcoatRoughnessTexture": {
                    "index": texture_by_role["metallic_roughness"],
                },
                "clearcoatNormalTexture": {
                    "index": texture_by_role["normal"],
                    "scale": float(properties["normal_scale"]),
                },
            }
        if float(properties["transmission"]) > 0:
            extensions["KHR_materials_transmission"] = {
                "transmissionFactor": float(properties["transmission"]),
            }
            extensions["KHR_materials_ior"] = {"ior": float(properties["ior"])}
        if extensions:
            material["extensions"] = extensions
            extensions_used.update(extensions)
        materials.append(material)
    for adornment_index, adornment in enumerate(surface_adornments):
        material_index = builtin_visual_material_count() + adornment_index
        if material_index not in used_material_indices:
            # The writer must never serialize speculative A005 material rows:
            # only an actual primitive binding may append a dynamic texture set.
            continue
        texture_set = surface_adornment_visual_texture_set(
            adornment,
            artifact_profile_id=artifact_profile_id,
        )
        base_index, texture_base_material_id = builtin_visual_material_binding(
            str(adornment["base_material"])
        )
        properties = builtin_material_properties(base_index)
        texture_by_role: Dict[str, int] = {}
        for texture_map in texture_set.maps:
            payload = surface_adornment_visual_texture_png_bytes(
                adornment,
                artifact_profile_id=artifact_profile_id,
                texture_role=texture_map.texture_role,
            )
            if hashlib.sha256(payload).hexdigest() != texture_map.sha256 or len(payload) != texture_map.byte_size:
                raise ValueError("surface adornment texture bytes do not match their manifest")
            view_index = _add_binary_view(binary, views, payload)
            image_index = len(images)
            images.append({
                "name": texture_map.texture_id,
                "bufferView": view_index,
                "mimeType": texture_map.mime_type,
                "extras": {"forgecad_visual_texture": texture_map.model_dump(mode="json")},
            })
            texture_index = len(textures)
            textures.append({"name": texture_map.texture_id, "source": image_index})
            texture_by_role[texture_map.texture_role] = texture_index
        pbr: Dict[str, Any] = {
            "baseColorFactor": [1, 1, 1, float(properties["alpha"])],
            "metallicFactor": 1,
            "roughnessFactor": 1,
            "baseColorTexture": {"index": texture_by_role["base_color"]},
            "metallicRoughnessTexture": {"index": texture_by_role["metallic_roughness"]},
        }
        material = {
            "name": f"MAT_{texture_set.material_id.removeprefix('mat_')}",
            "pbrMetallicRoughness": pbr,
            "normalTexture": {"index": texture_by_role["normal"], "scale": float(properties["normal_scale"])},
            "occlusionTexture": {"index": texture_by_role["occlusion"], "strength": 1},
            "emissiveTexture": {"index": texture_by_role["emissive"]},
            "emissiveFactor": [1, 1, 1],
            "extras": {
                "forgecad_visual_texture_set_id": texture_set.visual_texture_set_id,
                "forgecad_texture_material_id": texture_set.material_id,
                "forgecad_base_material_id": texture_base_material_id,
                "forgecad_surface_adornment": dict(adornment),
                "forgecad_surface_adornment_sha256": surface_adornment_program_sha256(adornment),
                "forgecad_visual_only": True,
            },
        }
        extensions: Dict[str, Any] = {}
        if float(properties["clearcoat"]) > 0:
            extensions["KHR_materials_clearcoat"] = {
                "clearcoatFactor": float(properties["clearcoat"]),
                "clearcoatRoughnessFactor": 1,
                "clearcoatRoughnessTexture": {"index": texture_by_role["metallic_roughness"]},
                "clearcoatNormalTexture": {
                    "index": texture_by_role["normal"],
                    "scale": float(properties["normal_scale"]),
                },
            }
        if float(properties["transmission"]) > 0:
            extensions["KHR_materials_transmission"] = {
                "transmissionFactor": float(properties["transmission"]),
            }
            extensions["KHR_materials_ior"] = {"ior": float(properties["ior"])}
        if extensions:
            material["extensions"] = extensions
            extensions_used.update(extensions)
        materials.append(material)
    for surface_layer_index, lowering in enumerate(surface_layers):
        material_index = (
            builtin_visual_material_count()
            + len(surface_adornments)
            + surface_layer_index
        )
        if material_index not in used_material_indices:
            continue
        normalized_layer = normalize_surface_layer_lowering(lowering)
        texture_set = surface_layer_visual_texture_set(
            normalized_layer,
            artifact_profile_id=artifact_profile_id,
        )
        base_material_id = str(normalized_layer["adornments"][0]["base_material"])
        base_index, texture_base_material_id = builtin_visual_material_binding(base_material_id)
        properties = builtin_material_properties(base_index)
        texture_by_role: Dict[str, int] = {}
        for texture_map in texture_set.maps:
            payload = surface_layer_visual_texture_png_bytes(
                normalized_layer,
                artifact_profile_id=artifact_profile_id,
                texture_role=texture_map.texture_role,
            )
            if hashlib.sha256(payload).hexdigest() != texture_map.sha256 or len(payload) != texture_map.byte_size:
                raise ValueError("surface layer texture bytes do not match their manifest")
            view_index = _add_binary_view(binary, views, payload)
            image_index = len(images)
            images.append({
                "name": texture_map.texture_id,
                "bufferView": view_index,
                "mimeType": texture_map.mime_type,
                "extras": {"forgecad_visual_texture": texture_map.model_dump(mode="json")},
            })
            texture_index = len(textures)
            textures.append({"name": texture_map.texture_id, "source": image_index})
            texture_by_role[texture_map.texture_role] = texture_index
        pbr: Dict[str, Any] = {
            "baseColorFactor": [1, 1, 1, float(properties["alpha"])],
            "metallicFactor": 1,
            "roughnessFactor": 1,
            "baseColorTexture": {"index": texture_by_role["base_color"]},
            "metallicRoughnessTexture": {"index": texture_by_role["metallic_roughness"]},
        }
        material = {
            "name": f"MAT_{texture_set.material_id.removeprefix('mat_')}",
            "pbrMetallicRoughness": pbr,
            "normalTexture": {"index": texture_by_role["normal"], "scale": float(properties["normal_scale"])},
            "occlusionTexture": {"index": texture_by_role["occlusion"], "strength": 1},
            "emissiveTexture": {"index": texture_by_role["emissive"]},
            "emissiveFactor": [1, 1, 1],
            "extras": {
                "forgecad_visual_texture_set_id": texture_set.visual_texture_set_id,
                "forgecad_texture_material_id": texture_set.material_id,
                "forgecad_base_material_id": texture_base_material_id,
                "forgecad_surface_layer_lowering": dict(normalized_layer),
                "forgecad_surface_layer_lowering_sha256": surface_layer_lowering_sha256(normalized_layer),
                "forgecad_surface_layer_retained_layers_sha256": normalized_layer["retained_layers_sha256"],
                "forgecad_visual_only": True,
            },
        }
        extensions: Dict[str, Any] = {}
        if float(properties["clearcoat"]) > 0:
            extensions["KHR_materials_clearcoat"] = {
                "clearcoatFactor": float(properties["clearcoat"]),
                "clearcoatRoughnessFactor": 1,
                "clearcoatRoughnessTexture": {"index": texture_by_role["metallic_roughness"]},
                "clearcoatNormalTexture": {
                    "index": texture_by_role["normal"],
                    "scale": float(properties["normal_scale"]),
                },
            }
        if float(properties["transmission"]) > 0:
            extensions["KHR_materials_transmission"] = {"transmissionFactor": float(properties["transmission"])}
            extensions["KHR_materials_ior"] = {"ior": float(properties["ior"])}
        if extensions:
            material["extensions"] = extensions
            extensions_used.update(extensions)
        materials.append(material)
    return {
        "images": images,
        "textures": textures,
        "materials": materials,
        "extensions_used": sorted(extensions_used),
    }


def _box_geometry(box: BoxPrimitive) -> Tuple[bytes, bytes, bytes, bytes, List[float], List[float]]:
    cx, cy, cz = (value / 1000 for value in box.center_mm)
    hx, hy, hz = (value / 2000 for value in box.size_mm)
    uv_unit = VISUAL_UV_REPEAT_MM / 1000
    faces = (
        ((1, 0, 0), ((hx, -hy, -hz), (hx, -hy, hz), (hx, hy, hz), (hx, hy, -hz)), 2 * hz / uv_unit, 2 * hy / uv_unit),
        ((-1, 0, 0), ((-hx, -hy, hz), (-hx, -hy, -hz), (-hx, hy, -hz), (-hx, hy, hz)), 2 * hz / uv_unit, 2 * hy / uv_unit),
        ((0, 1, 0), ((-hx, hy, -hz), (hx, hy, -hz), (hx, hy, hz), (-hx, hy, hz)), 2 * hx / uv_unit, 2 * hz / uv_unit),
        ((0, -1, 0), ((-hx, -hy, hz), (hx, -hy, hz), (hx, -hy, -hz), (-hx, -hy, -hz)), 2 * hx / uv_unit, 2 * hz / uv_unit),
        ((0, 0, 1), ((hx, -hy, hz), (-hx, -hy, hz), (-hx, hy, hz), (hx, hy, hz)), 2 * hx / uv_unit, 2 * hy / uv_unit),
        ((0, 0, -1), ((-hx, -hy, -hz), (hx, -hy, -hz), (hx, hy, -hz), (-hx, hy, -hz)), 2 * hx / uv_unit, 2 * hy / uv_unit),
    )
    positions: List[float] = []
    normals: List[float] = []
    uvs: List[float] = []
    indices: List[int] = []
    for face_index, (normal, vertices, repeat_u, repeat_v) in enumerate(faces):
        base = face_index * 4
        for x, y, z in vertices:
            positions.extend((cx + x, cy + y, cz + z))
            normals.extend(normal)
        uvs.extend((0, 0, repeat_u, 0, repeat_u, repeat_v, 0, repeat_v))
        indices.extend((base, base + 2, base + 1, base, base + 3, base + 2))
    return (
        struct.pack(f"<{len(positions)}f", *positions),
        struct.pack(f"<{len(normals)}f", *normals),
        struct.pack(f"<{len(uvs)}f", *uvs),
        struct.pack(f"<{len(indices)}H", *indices),
        [cx - hx, cy - hy, cz - hz],
        [cx + hx, cy + hy, cz + hz],
    )


def _primitive_geometry(primitive: BoxPrimitive) -> Tuple[bytes, bytes, bytes, bytes, List[float], List[float]]:
    if primitive.primitive_kind == "cylinder":
        return _cylinder_geometry(primitive)
    if primitive.primitive_kind == "capsule":
        return _capsule_geometry(primitive)
    if primitive.primitive_kind == "wedge":
        return _wedge_geometry(primitive)
    if primitive.primitive_kind in {"extrude", "extrude_profile"}:
        return _extrude_geometry(primitive)
    if primitive.primitive_kind in {"revolve", "revolve_profile"}:
        return _revolve_geometry(primitive)
    if primitive.primitive_kind == "loft_profile":
        return _loft_geometry(primitive)
    if primitive.primitive_kind == "sweep_profile":
        return _sweep_geometry(primitive)
    if primitive.primitive_kind == "bevel_box":
        return _bevel_box_geometry(primitive)
    return _box_geometry(primitive)


def _sweep_geometry(sweep: BoxPrimitive) -> Tuple[bytes, bytes, bytes, bytes, List[float], List[float]]:
    # Keep the path in the source operation's local frame.  The loft emitted
    # below retains ``sweep.center_mm`` and _loft_geometry applies that origin
    # once while writing vertices.  Adding it here as well would translate a
    # sweep twice, and a subsequent static rotation would no longer agree with
    # the Rust-baked Recipe connector frame.
    path = list(sweep.sweep_path_mm)
    profile = list(sweep.profile_points)
    if len(path) < (3 if sweep.path_closed else 2) or len(profile) < 8:
        raise ValueError("sweep path or profile is below its minimum budget")

    def normalize(vector: Tuple[float, float, float]) -> Tuple[float, float, float]:
        length = math.sqrt(sum(value * value for value in vector))
        if length <= 1e-9:
            raise ValueError("sweep frame contains a zero-length vector")
        return tuple(value / length for value in vector)

    def cross(left: Tuple[float, float, float], right: Tuple[float, float, float]) -> Tuple[float, float, float]:
        return (
            left[1] * right[2] - left[2] * right[1],
            left[2] * right[0] - left[0] * right[2],
            left[0] * right[1] - left[1] * right[0],
        )

    segments = [normalize(tuple(path[(index + 1) % len(path)][axis] - path[index][axis] for axis in range(3))) for index in range(len(path) if sweep.path_closed else len(path) - 1)]
    tangents: List[Tuple[float, float, float]] = []
    for index in range(len(path)):
        if not sweep.path_closed and index == 0:
            tangent = segments[0]
        elif not sweep.path_closed and index == len(path) - 1:
            tangent = segments[-1]
        else:
            previous = segments[index - 1]
            following = segments[index % len(segments)]
            tangent = normalize(tuple(previous[axis] + following[axis] for axis in range(3)))
        tangents.append(tangent)
    reference = min(((1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0)), key=lambda axis: abs(sum(axis[i] * tangents[0][i] for i in range(3))))
    normal = normalize(cross(tangents[0], reference))
    frames: List[Tuple[Tuple[float, float, float], Tuple[float, float, float]]] = []
    for index, tangent in enumerate(tangents):
        if index:
            previous = tangents[index - 1]
            rotation_axis_raw = cross(previous, tangent)
            sine = math.sqrt(sum(value * value for value in rotation_axis_raw))
            cosine = max(-1.0, min(1.0, sum(previous[axis] * tangent[axis] for axis in range(3))))
            if sine > 1e-9:
                rotation_axis = tuple(value / sine for value in rotation_axis_raw)
                axis_cross_normal = cross(rotation_axis, normal)
                axis_dot_normal = sum(rotation_axis[axis] * normal[axis] for axis in range(3))
                normal = normalize(tuple(
                    normal[axis] * cosine
                    + axis_cross_normal[axis] * sine
                    + rotation_axis[axis] * axis_dot_normal * (1 - cosine)
                    for axis in range(3)
                ))
        binormal = normalize(cross(tangent, normal))
        fraction = index / max(1, len(path) - 1)
        twist = math.radians(sweep.path_twist_degrees) * fraction
        cosine, sine = math.cos(twist), math.sin(twist)
        twisted_normal = tuple(normal[axis] * cosine + binormal[axis] * sine for axis in range(3))
        twisted_binormal = tuple(-normal[axis] * sine + binormal[axis] * cosine for axis in range(3))
        frames.append((twisted_normal, twisted_binormal))
    rings: List[Tuple[Tuple[float, float, float], ...]] = []
    for center, (normal, binormal) in zip(path, frames):
        rings.append(tuple(tuple(center[axis] + normal[axis] * u + binormal[axis] * v for axis in range(3)) for u, v in profile))
    profiles = [tuple(profile) for _ in rings]
    if sweep.path_closed:
        rings.append(rings[0])
        profiles.append(profiles[0])
    loft = replace(
        sweep,
        primitive_kind="loft_profile",
        loft_rings_mm=tuple(rings),
        loft_profiles=tuple(profiles),
        loft_axis="x",
        loft_cap_start_normal=tuple(-value for value in tangents[0]),
        loft_cap_end_normal=tangents[-1],
        cap_start=sweep.cap_start and not sweep.path_closed,
        cap_end=sweep.cap_end and not sweep.path_closed,
    )
    return _loft_geometry(loft)


def _loft_geometry(loft: BoxPrimitive) -> Tuple[bytes, bytes, bytes, bytes, List[float], List[float]]:
    """Build a deterministic linear loft from validated, uniformly sampled rings."""

    rings = [list(ring) for ring in loft.loft_rings_mm]
    profiles = [list(profile) for profile in loft.loft_profiles]
    maximum_rings = 33 if loft.sweep_path_mm else 12
    if not 2 <= len(rings) <= maximum_rings or len(profiles) != len(rings):
        raise ValueError("loft/sweep requires aligned section rings within its point budget")
    point_count = len(rings[0])
    if point_count < 8 or any(len(ring) != point_count for ring in rings):
        raise ValueError("loft rings must share one uniform sample count")
    positions: List[float] = []
    normals: List[float] = []
    uvs: List[float] = []
    indices: List[int] = []
    center = tuple(value / 1000 for value in loft.center_mm)
    ring_centers = [
        tuple(sum(point[axis] for point in ring) / point_count for axis in range(3))
        for ring in rings
    ]
    longitudinal_repeats = [0.0]
    for index in range(1, len(ring_centers)):
        longitudinal_repeats.append(
            longitudinal_repeats[-1]
            + math.dist(ring_centers[index - 1], ring_centers[index]) / VISUAL_UV_REPEAT_MM
        )
    perimeter_repeats: List[List[float]] = []
    for ring in rings:
        cumulative = [0.0]
        for index in range(point_count):
            cumulative.append(
                cumulative[-1]
                + math.dist(ring[index], ring[(index + 1) % point_count]) / VISUAL_UV_REPEAT_MM
            )
        perimeter_repeats.append(cumulative)

    def vector(left: Tuple[float, float, float], right: Tuple[float, float, float]) -> Tuple[float, float, float]:
        return right[0] - left[0], right[1] - left[1], right[2] - left[2]

    def cross(left: Tuple[float, float, float], right: Tuple[float, float, float]) -> Tuple[float, float, float]:
        return (
            left[1] * right[2] - left[2] * right[1],
            left[2] * right[0] - left[0] * right[2],
            left[0] * right[1] - left[1] * right[0],
        )

    def normalized(value: Tuple[float, float, float]) -> Tuple[float, float, float]:
        length = math.sqrt(sum(item * item for item in value))
        if length <= 1e-12:
            raise ValueError("loft contains a zero-area face")
        return tuple(item / length for item in value)

    smooth_normals: List[List[Tuple[float, float, float]]] = []
    if loft.smooth_normals:
        for ring_index, ring in enumerate(rings):
            previous_ring = rings[max(0, ring_index - 1)]
            following_ring = rings[min(len(rings) - 1, ring_index + 1)]
            ring_normals: List[Tuple[float, float, float]] = []
            for point_index, point in enumerate(ring):
                previous_point = ring[(point_index - 1) % point_count]
                following_point = ring[(point_index + 1) % point_count]
                perimeter_tangent = vector(previous_point, following_point)
                longitudinal_tangent = vector(
                    previous_ring[point_index],
                    following_ring[point_index],
                )
                candidate = cross(longitudinal_tangent, perimeter_tangent)
                length = math.sqrt(sum(item * item for item in candidate))
                if length <= 1e-12:
                    candidate = vector(ring_centers[ring_index], point)
                    length = math.sqrt(sum(item * item for item in candidate))
                if length <= 1e-12:
                    raise ValueError("loft contains a degenerate smooth-normal sample")
                candidate = tuple(item / length for item in candidate)
                outward = vector(ring_centers[ring_index], point)
                if sum(candidate[axis] * outward[axis] for axis in range(3)) < 0:
                    candidate = tuple(-item for item in candidate)
                ring_normals.append(candidate)
            smooth_normals.append(ring_normals)

    def append(point: Tuple[float, float, float], normal: Tuple[float, float, float], uv: Tuple[float, float]) -> int:
        positions.extend(tuple(center[index] + point[index] / 1000 for index in range(3)))
        normals.extend(normal)
        uvs.extend(uv)
        return len(positions) // 3 - 1

    for span in range(len(rings) - 1):
        v0, v1 = longitudinal_repeats[span], longitudinal_repeats[span + 1]
        centerline = tuple(
            (sum(point[axis] for point in rings[span]) / point_count + sum(point[axis] for point in rings[span + 1]) / point_count) / 2
            for axis in range(3)
        )
        for index in range(point_count):
            following = (index + 1) % point_count
            quad = [rings[span][index], rings[span + 1][index], rings[span + 1][following], rings[span][following]]
            normal_refs = [
                (span, index),
                (span + 1, index),
                (span + 1, following),
                (span, following),
            ]
            face_normal = normalized(cross(vector(quad[0], quad[1]), vector(quad[0], quad[2])))
            midpoint = tuple(sum(point[axis] for point in quad) / 4 for axis in range(3))
            outward = tuple(midpoint[axis] - centerline[axis] for axis in range(3))
            if sum(face_normal[axis] * outward[axis] for axis in range(3)) < 0:
                quad = [quad[0], quad[3], quad[2], quad[1]]
                normal_refs = [
                    normal_refs[0],
                    normal_refs[3],
                    normal_refs[2],
                    normal_refs[1],
                ]
                face_normal = tuple(-value for value in face_normal)
            current_u0, current_u1 = perimeter_repeats[span][index:index + 2]
            next_u0, next_u1 = perimeter_repeats[span + 1][index:index + 2]
            base = len(positions) // 3
            for vertex_offset, (point, uv) in enumerate(zip(
                quad,
                (
                    (current_u0, v0),
                    (next_u0, v1),
                    (next_u1, v1),
                    (current_u1, v0),
                ),
            )):
                normal = (
                    smooth_normals[normal_refs[vertex_offset][0]][normal_refs[vertex_offset][1]]
                    if loft.smooth_normals
                    else face_normal
                )
                append(point, normal, uv)
            indices.extend((base, base + 1, base + 2, base, base + 2, base + 3))

    axis_normal = {
        "x": (1.0, 0.0, 0.0),
        "y": (0.0, 1.0, 0.0),
        "z": (0.0, 0.0, 1.0),
    }[loft.loft_axis]

    def add_cap(section_index: int, outward: Tuple[float, float, float]) -> None:
        plane = profiles[section_index]
        ring = rings[section_index]
        point_lookup = {point: ring[index] for index, point in enumerate(plane)}
        min_u = min(point[0] for point in plane)
        min_v = min(point[1] for point in plane)
        for triangle in _triangulate_profile_cap(plane, []):
            points = [point_lookup[point] for point in triangle]
            normal = normalized(cross(vector(points[0], points[1]), vector(points[0], points[2])))
            if sum(normal[index] * outward[index] for index in range(3)) < 0:
                points.reverse()
            base = len(positions) // 3
            for point_2d, point_3d in zip(triangle if points[0] == point_lookup[triangle[0]] else reversed(triangle), points):
                append(
                    point_3d,
                    outward,
                    (
                        (point_2d[0] - min_u) / VISUAL_UV_REPEAT_MM,
                        (point_2d[1] - min_v) / VISUAL_UV_REPEAT_MM,
                    ),
                )
            indices.extend((base, base + 1, base + 2))

    if loft.cap_start:
        add_cap(0, loft.loft_cap_start_normal or tuple(-value for value in axis_normal))
    if loft.cap_end:
        add_cap(-1, loft.loft_cap_end_normal or axis_normal)
    if len(positions) // 3 > 65535:
        raise ValueError("loft vertex budget exceeds uint16 GLB indices")
    minimum = [min(positions[index::3]) for index in range(3)]
    maximum = [max(positions[index::3]) for index in range(3)]
    return (
        struct.pack(f"<{len(positions)}f", *positions),
        struct.pack(f"<{len(normals)}f", *normals),
        struct.pack(f"<{len(uvs)}f", *uvs),
        struct.pack(f"<{len(indices)}H", *indices),
        minimum,
        maximum,
    )


def _bevel_box_geometry(bevel: BoxPrimitive) -> Tuple[bytes, bytes, bytes, bytes, List[float], List[float]]:
    """Build a low-poly rounded-rectangle prism as a bevel approximation.

    The operation intentionally rounds only the X/Z perimeter and keeps the
    two Y faces planar.  This is a visual concept primitive, not a general
    edge-aware mesh bevel or a manufacturing fillet.
    """
    cx, cy, cz = (value / 1000 for value in bevel.center_mm)
    hx, hy, hz = (value / 2000 for value in bevel.size_mm)
    radius = min(bevel.bevel_radius_mm / 1000, hx, hz)
    segments = max(1, min(3, int(bevel.bevel_segments)))
    points: List[Tuple[float, float]] = []
    corner_centers = ((hx - radius, hz - radius), (-hx + radius, hz - radius), (-hx + radius, -hz + radius), (hx - radius, -hz + radius))
    corner_starts = (0.0, math.pi / 2, math.pi, math.pi * 1.5)
    for (corner_x, corner_z), start in zip(corner_centers, corner_starts):
        for step in range(segments + 1):
            angle = start + (math.pi / 2) * step / segments
            points.append((corner_x + radius * math.cos(angle), corner_z + radius * math.sin(angle)))
    positions: List[float] = []
    normals: List[float] = []
    uvs: List[float] = []
    indices: List[int] = []

    def append_vertex(x: float, y: float, z: float, normal: Tuple[float, float, float], uv: Tuple[float, float]) -> int:
        index = len(positions) // 3
        positions.extend((cx + x, cy + y, cz + z))
        normals.extend(normal)
        uvs.extend(uv)
        return index

    ring_count = len(points)
    uv_unit = VISUAL_UV_REPEAT_MM / 1000
    perimeter_offsets = [0.0]
    for index in range(ring_count):
        perimeter_offsets.append(perimeter_offsets[-1] + math.dist(points[index], points[(index + 1) % ring_count]))
    for index in range(ring_count):
        next_index = (index + 1) % ring_count
        x0, z0 = points[index]
        x1, z1 = points[next_index]
        dx, dz = x1 - x0, z1 - z0
        normal_length = math.sqrt(dx * dx + dz * dz) or 1.0
        normal = (dz / normal_length, 0.0, -dx / normal_length)
        base = len(positions) // 3
        u0, u1 = perimeter_offsets[index] / uv_unit, perimeter_offsets[index + 1] / uv_unit
        repeat_v = 2 * hy / uv_unit
        append_vertex(x0, -hy, z0, normal, (u0, 0.0))
        append_vertex(x1, -hy, z1, normal, (u1, 0.0))
        append_vertex(x1, hy, z1, normal, (u1, repeat_v))
        append_vertex(x0, hy, z0, normal, (u0, repeat_v))
        indices.extend((base, base + 2, base + 1, base, base + 3, base + 2))

    for cap_y, cap_normal in ((-hy, (0.0, -1.0, 0.0)), (hy, (0.0, 1.0, 0.0))):
        center = append_vertex(0.0, cap_y, 0.0, cap_normal, (hx / uv_unit, hz / uv_unit))
        for index in range(ring_count):
            next_index = (index + 1) % ring_count
            first_point = points[index]
            second_point = points[next_index]
            first = append_vertex(
                first_point[0], cap_y, first_point[1], cap_normal,
                ((first_point[0] + hx) / uv_unit, (first_point[1] + hz) / uv_unit),
            )
            second = append_vertex(
                second_point[0], cap_y, second_point[1], cap_normal,
                ((second_point[0] + hx) / uv_unit, (second_point[1] + hz) / uv_unit),
            )
            if cap_normal[1] > 0:
                indices.extend((center, second, first))
            else:
                indices.extend((center, first, second))
    minimum = [min(positions[index::3]) for index in range(3)]
    maximum = [max(positions[index::3]) for index in range(3)]
    return (
        struct.pack(f"<{len(positions)}f", *positions),
        struct.pack(f"<{len(normals)}f", *normals),
        struct.pack(f"<{len(uvs)}f", *uvs),
        struct.pack(f"<{len(indices)}H", *indices),
        minimum,
        maximum,
    )


def _capsule_geometry(capsule: BoxPrimitive) -> Tuple[bytes, bytes, bytes, bytes, List[float], List[float]]:
    segments = capsule.radial_segments
    hemisphere_segments = capsule.capsule_hemisphere_segments
    cx, cy, cz = (value / 1000 for value in capsule.center_mm)
    radius = min(capsule.radius_mm / 1000, capsule.height_mm / 2000)
    half_straight = max(0.0, capsule.height_mm / 2000 - radius)
    axis = capsule.axis
    dominant = max(range(3), key=lambda index: abs(axis[index]))
    sign = -1.0 if axis[dominant] < 0 else 1.0
    positions: List[float] = []
    normals: List[float] = []
    uvs: List[float] = []
    indices: List[int] = []

    def map_local(local_x: float, local_y: float, local_z: float) -> Tuple[float, float, float]:
        if dominant == 0:
            return (local_y * sign, -local_x * sign, local_z)
        if dominant == 2:
            return (local_x, -local_z * sign, local_y * sign)
        return (local_x, local_y * sign, local_z * sign)

    profile: List[Tuple[float, float, Tuple[float, float]]] = []
    for index in range(hemisphere_segments + 1):
        theta = (math.pi / 2) * index / hemisphere_segments
        profile.append((half_straight + radius * math.cos(theta), radius * math.sin(theta), (math.sin(theta), math.cos(theta))))
    for index in range(1, hemisphere_segments + 1):
        theta = (math.pi / 2) + (math.pi / 2) * index / hemisphere_segments
        profile.append((-half_straight + radius * math.cos(theta), radius * math.sin(theta), (math.sin(theta), math.cos(theta))))

    uv_unit = VISUAL_UV_REPEAT_MM / 1000
    profile_top = profile[0][0]
    for ring_index, (local_y, ring_radius, normal_yz) in enumerate(profile):
        for segment in range(segments):
            angle = 2 * math.pi * segment / segments
            local_x = ring_radius * math.cos(angle)
            local_z = ring_radius * math.sin(angle)
            mapped = map_local(local_x, local_y, local_z)
            positions.extend((cx + mapped[0], cy + mapped[1], cz + mapped[2]))
            normal = map_local(normal_yz[0] * math.cos(angle), normal_yz[1], normal_yz[0] * math.sin(angle))
            normals.extend(normal)
            uvs.extend(((2 * math.pi * radius * segment / segments) / uv_unit, (profile_top - local_y) / uv_unit))
    for ring in range(len(profile) - 1):
        for segment in range(segments):
            next_segment = (segment + 1) % segments
            base = ring * segments + segment
            upper = (ring + 1) * segments + segment
            if ring > 0:
                indices.extend((base, ring * segments + next_segment, upper))
            if ring < len(profile) - 2:
                indices.extend((ring * segments + next_segment, (ring + 1) * segments + next_segment, upper))
    minimum = [min(positions[index::3]) for index in range(3)]
    maximum = [max(positions[index::3]) for index in range(3)]
    return (struct.pack(f"<{len(positions)}f", *positions), struct.pack(f"<{len(normals)}f", *normals), struct.pack(f"<{len(uvs)}f", *uvs), struct.pack(f"<{len(indices)}H", *indices), minimum, maximum)


def _wedge_geometry(wedge: BoxPrimitive) -> Tuple[bytes, bytes, bytes, bytes, List[float], List[float]]:
    cx, cy, cz = (value / 1000 for value in wedge.center_mm)
    hx, hy, hz = (value / 2000 for value in wedge.size_mm)
    vertices = ((-hx, -hy, -hz), (hx, -hy, -hz), (hx, -hy, hz), (-hx, -hy, hz), (-hx, hy, -hz), (-hx, hy, hz))
    faces = ((0, 1, 2, 3), (0, 3, 5, 4), (1, 4, 5, 2), (0, 4, 1), (3, 2, 5))
    positions: List[float] = []
    normals: List[float] = []
    uvs: List[float] = []
    indices: List[int] = []
    for face in faces:
        a, b, c = (vertices[index] for index in face[:3])
        ab = tuple(b[index] - a[index] for index in range(3))
        ac = tuple(c[index] - a[index] for index in range(3))
        normal = (ab[1] * ac[2] - ab[2] * ac[1], ab[2] * ac[0] - ab[0] * ac[2], ab[0] * ac[1] - ab[1] * ac[0])
        length = math.sqrt(sum(value * value for value in normal)) or 1.0
        normal = tuple(value / length for value in normal)
        base = len(positions) // 3
        dominant_axis = max(range(3), key=lambda axis: abs(normal[axis]))
        uv_axes = [axis for axis in range(3) if axis != dominant_axis]
        uv_min = [min(vertices[vertex_index][axis] for vertex_index in face) for axis in uv_axes]
        for vertex_index in face:
            vertex = vertices[vertex_index]
            positions.extend((cx + vertex[0], cy + vertex[1], cz + vertex[2]))
            normals.extend(normal)
            uvs.extend(tuple(
                (vertex[axis] - uv_min[index]) / (VISUAL_UV_REPEAT_MM / 1000)
                for index, axis in enumerate(uv_axes)
            ))
        if len(face) == 3:
            indices.extend((base, base + 1, base + 2))
        else:
            indices.extend((base, base + 1, base + 2, base, base + 2, base + 3))
    minimum = [min(positions[index::3]) for index in range(3)]
    maximum = [max(positions[index::3]) for index in range(3)]
    return (struct.pack(f"<{len(positions)}f", *positions), struct.pack(f"<{len(normals)}f", *normals), struct.pack(f"<{len(uvs)}f", *uvs), struct.pack(f"<{len(indices)}H", *indices), minimum, maximum)


def _polygon_area_2d(points: List[Tuple[float, float]]) -> float:
    return 0.5 * sum(
        points[index][0] * points[(index + 1) % len(points)][1]
        - points[(index + 1) % len(points)][0] * points[index][1]
        for index in range(len(points))
    )


def _point_in_triangle_2d(point: Tuple[float, float], a: Tuple[float, float], b: Tuple[float, float], c: Tuple[float, float]) -> bool:
    def cross(p: Tuple[float, float], q: Tuple[float, float], r: Tuple[float, float]) -> float:
        return (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0])

    values = (cross(a, b, point), cross(b, c, point), cross(c, a, point))
    return all(value >= -1e-8 for value in values)


def _segments_cross_2d(a: Tuple[float, float], b: Tuple[float, float], c: Tuple[float, float], d: Tuple[float, float]) -> bool:
    def cross(p: Tuple[float, float], q: Tuple[float, float], r: Tuple[float, float]) -> float:
        return (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0])

    ab_c, ab_d = cross(a, b, c), cross(a, b, d)
    cd_a, cd_b = cross(c, d, a), cross(c, d, b)
    return ((ab_c > 1e-8 and ab_d < -1e-8) or (ab_c < -1e-8 and ab_d > 1e-8)) and (
        (cd_a > 1e-8 and cd_b < -1e-8) or (cd_a < -1e-8 and cd_b > 1e-8)
    )


def _point_in_polygon_2d(point: Tuple[float, float], polygon: List[Tuple[float, float]]) -> bool:
    inside = False
    previous = polygon[-1]
    for current in polygon:
        if (current[1] > point[1]) != (previous[1] > point[1]):
            crossing = (previous[0] - current[0]) * (point[1] - current[1]) / (previous[1] - current[1]) + current[0]
            if point[0] < crossing:
                inside = not inside
        previous = current
    return inside


def _bridge_profile_holes(
    outer: List[Tuple[float, float]],
    holes: List[List[Tuple[float, float]]],
) -> List[Tuple[float, float]]:
    merged = list(outer)
    remaining = [list(hole) for hole in holes]
    for hole_index, hole in enumerate(remaining):
        hi = max(range(len(hole)), key=lambda index: (hole[index][0], -hole[index][1]))
        anchor = hole[hi]
        boundaries = [merged, hole, *remaining[hole_index + 1 :]]

        def visible(outer_index: int) -> bool:
            target = merged[outer_index]
            midpoint = ((anchor[0] + target[0]) / 2, (anchor[1] + target[1]) / 2)
            if not _point_in_polygon_2d(midpoint, outer):
                return False
            if any(_point_in_polygon_2d(midpoint, candidate) for candidate in remaining):
                return False
            for boundary in boundaries:
                for index, start in enumerate(boundary):
                    end = boundary[(index + 1) % len(boundary)]
                    if start in {anchor, target} or end in {anchor, target}:
                        continue
                    if _segments_cross_2d(anchor, target, start, end):
                        return False
            return True

        candidates = sorted(range(len(merged)), key=lambda index: math.dist(anchor, merged[index]))
        try:
            oi = next(index for index in candidates if visible(index))
        except StopIteration as exc:
            raise ValueError("profile hole cannot be connected to outer contour") from exc
        hole_path = hole[hi:] + hole[: hi + 1]
        merged = merged[: oi + 1] + hole_path + [merged[oi]] + merged[oi + 1 :]
    return merged


def _ear_clip_polygon(points: List[Tuple[float, float]]) -> List[Tuple[int, int, int]]:
    if len(points) < 3:
        return []
    if _polygon_area_2d(points) < 0:
        points.reverse()
    remaining = list(range(len(points)))
    triangles: List[Tuple[int, int, int]] = []
    guard = len(points) * len(points) * 2
    while len(remaining) > 3 and guard > 0:
        guard -= 1
        ear_found = False
        for cursor, current in enumerate(remaining):
            previous = remaining[cursor - 1]
            following = remaining[(cursor + 1) % len(remaining)]
            a, b, c = points[previous], points[current], points[following]
            cross = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
            if cross <= 1e-9:
                continue
            blocked = False
            for other in remaining:
                if other in {previous, current, following} or points[other] in {a, b, c}:
                    continue
                if _point_in_triangle_2d(points[other], a, b, c):
                    blocked = True
                    break
            if blocked:
                continue
            triangles.append((previous, current, following))
            del remaining[cursor]
            ear_found = True
            break
        if not ear_found:
            # Bridged hole polygons contain duplicate bridge endpoints. Remove
            # only a truly collinear/duplicate vertex and retry; never invent
            # triangles for an unresolved topology.
            removed = False
            for cursor, current in enumerate(remaining):
                previous = remaining[cursor - 1]
                following = remaining[(cursor + 1) % len(remaining)]
                a, b, c = points[previous], points[current], points[following]
                cross = abs((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]))
                if b == a or b == c or cross <= 1e-9:
                    del remaining[cursor]
                    removed = True
                    break
            if not removed:
                raise ValueError("profile cap triangulation failed")
    if len(remaining) == 3:
        triangles.append(tuple(remaining))
    return triangles


def _triangulate_profile_cap(
    outer: List[Tuple[float, float]],
    holes: List[List[Tuple[float, float]]],
) -> List[Tuple[Tuple[float, float], Tuple[float, float], Tuple[float, float]]]:
    outer = list(outer)
    if _polygon_area_2d(outer) < 0:
        outer.reverse()
    normalized_holes: List[List[Tuple[float, float]]] = []
    for hole in holes:
        contour = list(hole)
        if _polygon_area_2d(contour) > 0:
            contour.reverse()
        normalized_holes.append(contour)
    merged = _bridge_profile_holes(outer, normalized_holes) if normalized_holes else outer
    return [(merged[a], merged[b], merged[c]) for a, b, c in _ear_clip_polygon(merged)]


def _extrude_geometry(extrude: BoxPrimitive) -> Tuple[bytes, bytes, bytes, bytes, List[float], List[float]]:
    """Build a deterministic prism/ribbon from a validated 2D X/Z profile."""
    if extrude.primitive_kind == "extrude_profile":
        return _extrude_contract_geometry(extrude)
    points = list(extrude.profile_points)
    if len(points) < 3 or extrude.height_mm <= 0:
        raise ValueError("extrude requires a non-degenerate profile and positive height")
    area = sum(points[index][0] * points[(index + 1) % len(points)][1] - points[(index + 1) % len(points)][0] * points[index][1] for index in range(len(points)))
    if area < 0:
        points.reverse()
    cx, cy, cz = (value / 1000 for value in extrude.center_mm)
    half_height = extrude.height_mm / 2000
    positions: List[float] = []
    normals: List[float] = []
    uvs: List[float] = []
    indices: List[int] = []
    min_x, max_x = min(point[0] for point in points), max(point[0] for point in points)
    min_z, max_z = min(point[1] for point in points), max(point[1] for point in points)
    span_x, span_z = max(max_x - min_x, 1e-9), max(max_z - min_z, 1e-9)

    def add_face(vertices: List[Tuple[float, float, float]], normal: Tuple[float, float, float]) -> None:
        base = len(positions) // 3
        for index, vertex in enumerate(vertices):
            positions.extend((cx + vertex[0] / 1000, cy + vertex[1] / 1000, cz + vertex[2] / 1000))
            normals.extend(normal)
            if normal[1] != 0:
                uvs.extend(((vertex[0] - min_x) / span_x, (vertex[2] - min_z) / span_z))
            elif len(vertices) == 4:
                uvs.extend(((0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0))[index])
            else:
                raise ValueError("legacy extrude side UV requires a quad")
        for index in range(1, len(vertices) - 1):
            indices.extend((base, base + index, base + index + 1))

    top = [(x, half_height * 1000, z) for x, z in points]
    bottom = [(x, -half_height * 1000, z) for x, z in points]
    add_face(top, (0, 1, 0))
    add_face(list(reversed(bottom)), (0, -1, 0))
    for index, (x0, z0) in enumerate(points):
        x1, z1 = points[(index + 1) % len(points)]
        dx = x1 - x0
        dz = z1 - z0
        length = math.sqrt(dx * dx + dz * dz) or 1.0
        normal = (dz / length, 0, -dx / length)
        add_face([(x0, -half_height * 1000, z0), (x1, -half_height * 1000, z1), (x1, half_height * 1000, z1), (x0, half_height * 1000, z0)], normal)
    minimum = [min(positions[index::3]) for index in range(3)]
    maximum = [max(positions[index::3]) for index in range(3)]
    return (struct.pack(f"<{len(positions)}f", *positions), struct.pack(f"<{len(normals)}f", *normals), struct.pack(f"<{len(uvs)}f", *uvs), struct.pack(f"<{len(indices)}H", *indices), minimum, maximum)


def _extrude_contract_geometry(extrude: BoxPrimitive) -> Tuple[bytes, bytes, bytes, bytes, List[float], List[float]]:
    outer = list(extrude.profile_points)
    holes = [list(hole) for hole in extrude.profile_holes]
    if len(outer) < 2 or extrude.height_mm <= 0:
        raise ValueError("contract extrude requires a sampled profile and positive height")
    cx, cy, cz = (value / 1000 for value in extrude.center_mm)
    half_height = extrude.height_mm / 2000
    positions: List[float] = []
    normals: List[float] = []
    uvs: List[float] = []
    indices: List[int] = []
    all_points = [*outer, *(point for contour in holes for point in contour)]
    min_x, max_x = min(point[0] for point in all_points), max(point[0] for point in all_points)
    min_z, max_z = min(point[1] for point in all_points), max(point[1] for point in all_points)
    span_x, span_z = max(max_x - min_x, 1e-9), max(max_z - min_z, 1e-9)

    def append(point: Tuple[float, float], y: float, normal: Tuple[float, float, float], uv: Tuple[float, float]) -> int:
        positions.extend((cx + point[0] / 1000, cy + y, cz + point[1] / 1000))
        normals.extend(normal)
        uvs.extend(uv)
        return len(positions) // 3 - 1

    def add_walls(contour: List[Tuple[float, float]], closed: bool) -> None:
        edge_count = len(contour) if closed else len(contour) - 1
        lengths = [math.dist(contour[index], contour[(index + 1) % len(contour)]) for index in range(edge_count)]
        perimeter = sum(lengths) or 1.0
        distance = 0.0
        for index in range(edge_count):
            current = contour[index]
            following = contour[(index + 1) % len(contour)]
            dx, dz = following[0] - current[0], following[1] - current[1]
            length = math.sqrt(dx * dx + dz * dz) or 1.0
            normal = (dz / length, 0.0, -dx / length)
            u0, u1 = distance / perimeter, (distance + lengths[index]) / perimeter
            base = len(positions) // 3
            append(current, -half_height, normal, (u0, 0.0))
            append(following, -half_height, normal, (u1, 0.0))
            append(following, half_height, normal, (u1, 1.0))
            append(current, half_height, normal, (u0, 1.0))
            indices.extend((base, base + 1, base + 2, base, base + 2, base + 3))
            distance += lengths[index]

    add_walls(outer, extrude.profile_closed)
    for hole in holes:
        add_walls(hole, True)

    if extrude.profile_closed and (extrude.cap_start or extrude.cap_end):
        cap_triangles = _triangulate_profile_cap(outer, holes)
        if extrude.cap_start:
            for triangle in cap_triangles:
                uv_values = [((point[0] - min_x) / span_x, (point[1] - min_z) / span_z) for point in triangle]
                base = len(positions) // 3
                for point, uv in zip(triangle, uv_values):
                    append(point, -half_height, (0.0, -1.0, 0.0), uv)
                indices.extend((base, base + 1, base + 2))
        if extrude.cap_end:
            for triangle in cap_triangles:
                uv_values = [((point[0] - min_x) / span_x, (point[1] - min_z) / span_z) for point in triangle]
                base = len(positions) // 3
                for point, uv in zip(reversed(triangle), reversed(uv_values)):
                    append(point, half_height, (0.0, 1.0, 0.0), uv)
                indices.extend((base, base + 1, base + 2))
    minimum = [min(positions[index::3]) for index in range(3)]
    maximum = [max(positions[index::3]) for index in range(3)]
    if len(positions) // 3 > 65535:
        raise ValueError("contract extrude vertex budget exceeds uint16 GLB indices")
    return (
        struct.pack(f"<{len(positions)}f", *positions),
        struct.pack(f"<{len(normals)}f", *normals),
        struct.pack(f"<{len(uvs)}f", *uvs),
        struct.pack(f"<{len(indices)}H", *indices),
        minimum,
        maximum,
    )


def _revolve_geometry(revolve: BoxPrimitive) -> Tuple[bytes, bytes, bytes, bytes, List[float], List[float]]:
    """Revolve a validated radius/height profile around the local Y axis."""
    if revolve.primitive_kind == "revolve_profile":
        return _revolve_contract_geometry(revolve)
    points = list(revolve.profile_points)
    if len(points) < 3 or any(radius < 0 for radius, _ in points):
        raise ValueError("revolve requires a non-negative radius profile")
    segments = revolve.radial_segments
    cx, cy, cz = (value / 1000 for value in revolve.center_mm)
    angle = revolve.revolve_angle
    positions: List[float] = []
    normals: List[float] = []
    uvs: List[float] = []
    indices: List[int] = []
    min_height = min(point[1] for point in points)
    height_span = max(max(point[1] for point in points) - min_height, 1e-9)

    def profile_normal(index: int) -> Tuple[float, float]:
        previous = points[index - 1] if index > 0 else points[index]
        following = points[index + 1] if index + 1 < len(points) else points[index]
        dr, dy = following[0] - previous[0], following[1] - previous[1]
        length = math.sqrt(dr * dr + dy * dy)
        return (dy / length, -dr / length) if length > 1e-9 else (0.0, 1.0)

    def pole_normal(index: int) -> Tuple[float, float, float]:
        """Return the stable axial normal for a shared endpoint pole.

        A pole cannot retain the radial normal from every former ring vertex.
        Its adjacent profile slope determines whether it is the lower or upper
        cap of the visual surface; choosing that axial direction keeps the
        shared normal outward for the fan triangles.
        """

        _radial, vertical = profile_normal(index)
        return (0.0, 1.0 if vertical >= 0 else -1.0, 0.0)
    # An axis point is a single geometric pole, not a ring of coincident
    # vertices.  A repeated pole ring makes each adjacent quad emit one
    # zero-area triangle and turns an otherwise closed full revolve into a
    # topology failure at readback.  Keep non-axis vertices as before, but
    # route every ring of an axis profile point to its one pole.
    #
    # This legacy inline-points path deliberately retains its existing
    # partial-angle ring count.  The ProfileSketch contract path below owns
    # partial-end seam/cap semantics.
    vertex_indices: List[List[int]] = []
    pole_indices: Dict[int, int] = {}
    for segment in range(segments):
        theta = angle * segment / segments
        cos_theta = math.cos(theta)
        sin_theta = math.sin(theta)
        ring_indices: List[int] = []
        for point_index, (radius, height) in enumerate(points):
            if radius <= 1e-9 and point_index in pole_indices:
                ring_indices.append(pole_indices[point_index])
                continue
            positions.extend((cx + radius * cos_theta / 1000, cy + height / 1000, cz + radius * sin_theta / 1000))
            if radius <= 1e-9:
                normals.extend(pole_normal(point_index))
            else:
                radial_normal, vertical_normal = profile_normal(point_index)
                normals.extend((
                    cos_theta * radial_normal,
                    vertical_normal,
                    sin_theta * radial_normal,
                ))
            # A pole has no circumferential direction. Pinning it to the seam
            # keeps one vertex while preserving non-degenerate UV fan faces.
            uvs.extend((0.0 if radius <= 1e-9 else segment / segments, (height - min_height) / height_span))
            vertex_index = len(positions) // 3 - 1
            ring_indices.append(vertex_index)
            if radius <= 1e-9:
                pole_indices[point_index] = vertex_index
        vertex_indices.append(ring_indices)
    profile_count = len(points)
    for segment in range(segments):
        next_segment = (segment + 1) % segments if abs(angle - math.pi * 2) < 1e-6 else segment + 1
        if next_segment >= segments:
            break
        for point_index in range(profile_count - 1):
            base = vertex_indices[segment][point_index]
            next_base = vertex_indices[next_segment][point_index]
            current_next = vertex_indices[segment][point_index + 1]
            next_next = vertex_indices[next_segment][point_index + 1]
            radius, next_radius = points[point_index][0], points[point_index + 1][0]
            if radius <= 1e-9 and next_radius <= 1e-9:
                continue
            if radius <= 1e-9:
                indices.extend((base, current_next, next_next))
            elif next_radius <= 1e-9:
                indices.extend((base, current_next, next_base))
            else:
                indices.extend((base, next_next, next_base, base, current_next, next_next))
    minimum = [min(positions[index::3]) for index in range(3)]
    maximum = [max(positions[index::3]) for index in range(3)]
    return (struct.pack(f"<{len(positions)}f", *positions), struct.pack(f"<{len(normals)}f", *normals), struct.pack(f"<{len(uvs)}f", *uvs), struct.pack(f"<{len(indices)}H", *indices), minimum, maximum)


def _revolve_seam_polygon(points: Tuple[Tuple[float, float], ...], closed: bool = False) -> List[Tuple[float, float]]:
    polygon = list(points)
    if not closed:
        if polygon[0][0] > 1e-9:
            polygon.insert(0, (0.0, polygon[0][1]))
        if polygon[-1][0] > 1e-9:
            polygon.append((0.0, polygon[-1][1]))
    cleaned: List[Tuple[float, float]] = []
    for point in polygon:
        if not cleaned or math.dist(point, cleaned[-1]) > 1e-9:
            cleaned.append(point)
    if len(cleaned) > 2 and _polygon_area_2d(cleaned) < 0:
        cleaned.reverse()
    return cleaned


def _revolve_contract_geometry(revolve: BoxPrimitive) -> Tuple[bytes, bytes, bytes, bytes, List[float], List[float]]:
    points = list(revolve.profile_points)
    if len(points) < 2 or any(radius < 0 for radius, _height in points):
        raise ValueError("contract revolve requires a sampled non-negative-radius profile")
    full = math.isclose(revolve.revolve_angle, math.pi * 2, abs_tol=1e-9)
    segments = revolve.radial_segments
    ring_count = segments if full else segments + 1
    cx, cy, cz = (value / 1000 for value in revolve.center_mm)
    positions: List[float] = []
    normals: List[float] = []
    uvs: List[float] = []
    indices: List[int] = []
    heights = [height for _radius, height in points]
    min_height, max_height = min(heights), max(heights)
    height_span = max(max_height - min_height, 1e-9)

    def profile_normal(index: int) -> Tuple[float, float]:
        previous = points[index - 1] if index > 0 else (points[-1] if revolve.profile_closed else points[index])
        following = points[(index + 1) % len(points)] if index + 1 < len(points) or revolve.profile_closed else points[index]
        dr, dy = following[0] - previous[0], following[1] - previous[1]
        length = math.sqrt(dr * dr + dy * dy) or 1.0
        return dy / length, -dr / length

    for ring in range(ring_count):
        theta = revolve.revolve_angle * ring / segments
        cosine, sine = math.cos(theta), math.sin(theta)
        for point_index, (radius, height) in enumerate(points):
            positions.extend((cx + radius * cosine / 1000, cy + height / 1000, cz + radius * sine / 1000))
            radial_normal, vertical_normal = profile_normal(point_index)
            normals.extend((radial_normal * cosine, vertical_normal, radial_normal * sine))
            uvs.extend((ring / segments, (height - min_height) / height_span))
    profile_edges = len(points) if revolve.profile_closed else len(points) - 1
    for ring in range(segments):
        following_ring = (ring + 1) % ring_count
        for point_index in range(profile_edges):
            following_point = (point_index + 1) % len(points)
            base = ring * len(points) + point_index
            right = following_ring * len(points) + point_index
            current_next = ring * len(points) + following_point
            right_next = following_ring * len(points) + following_point
            radius, next_radius = points[point_index][0], points[following_point][0]
            if radius <= 1e-9 and next_radius <= 1e-9:
                continue
            if radius <= 1e-9:
                indices.extend((base, right_next, current_next))
            elif next_radius <= 1e-9:
                indices.extend((base, right, current_next))
            else:
                indices.extend((base, right, right_next, base, right_next, current_next))

    if not full and (revolve.cap_start or revolve.cap_end):
        polygon = _revolve_seam_polygon(revolve.profile_points, revolve.profile_closed)
        triangles = _triangulate_profile_cap(polygon, [])
        max_radius = max(point[0] for point in polygon) or 1.0

        def append_seam(point: Tuple[float, float], theta: float, normal: Tuple[float, float, float]) -> int:
            radius, height = point
            positions.extend((cx + radius * math.cos(theta) / 1000, cy + height / 1000, cz + radius * math.sin(theta) / 1000))
            normals.extend(normal)
            uvs.extend((radius / max_radius, (height - min_height) / height_span))
            return len(positions) // 3 - 1

        if revolve.cap_start:
            for triangle in triangles:
                base = len(positions) // 3
                for point in reversed(triangle):
                    append_seam(point, 0.0, (0.0, 0.0, -1.0))
                indices.extend((base, base + 1, base + 2))
        if revolve.cap_end:
            for triangle in triangles:
                theta = revolve.revolve_angle
                normal = (-math.sin(theta), 0.0, math.cos(theta))
                base = len(positions) // 3
                for point in triangle:
                    append_seam(point, theta, normal)
                indices.extend((base, base + 1, base + 2))
    minimum = [min(positions[index::3]) for index in range(3)]
    maximum = [max(positions[index::3]) for index in range(3)]
    if len(positions) // 3 > 65535:
        raise ValueError("contract revolve vertex budget exceeds uint16 GLB indices")
    return (
        struct.pack(f"<{len(positions)}f", *positions),
        struct.pack(f"<{len(normals)}f", *normals),
        struct.pack(f"<{len(uvs)}f", *uvs),
        struct.pack(f"<{len(indices)}H", *indices),
        minimum,
        maximum,
    )


def _cylinder_geometry(cylinder: BoxPrimitive) -> Tuple[bytes, bytes, bytes, bytes, List[float], List[float]]:
    segments = cylinder.radial_segments
    cx, cy, cz = (value / 1000 for value in cylinder.center_mm)
    radius = cylinder.radius_mm / 1000
    half_height = cylinder.height_mm / 2000
    axis = cylinder.axis
    dominant = max(range(3), key=lambda index: abs(axis[index]))
    sign = -1.0 if axis[dominant] < 0 else 1.0
    positions: List[float] = []
    normals: List[float] = []
    uvs: List[float] = []
    indices: List[int] = []

    def map_local(local_x: float, local_y: float, local_z: float) -> Tuple[float, float, float]:
        if dominant == 0:
            return (local_y * sign, -local_x * sign, local_z)
        if dominant == 2:
            return (local_x, -local_z * sign, local_y * sign)
        return (local_x, local_y * sign, local_z * sign)

    def append_vertex(local: Tuple[float, float, float], local_normal: Tuple[float, float, float], uv: Tuple[float, float]) -> int:
        mapped = map_local(*local)
        mapped_normal = map_local(*local_normal)
        positions.extend((cx + mapped[0], cy + mapped[1], cz + mapped[2]))
        normals.extend(mapped_normal)
        uvs.extend(uv)
        return len(positions) // 3 - 1

    for segment in range(segments):
        a0 = 2 * math.pi * segment / segments
        a1 = 2 * math.pi * (segment + 1) / segments
        x0, z0 = radius * math.cos(a0), radius * math.sin(a0)
        x1, z1 = radius * math.cos(a1), radius * math.sin(a1)
        uv_unit = VISUAL_UV_REPEAT_MM / 1000
        u0, u1 = radius * a0 / uv_unit, radius * a1 / uv_unit
        repeat_v = 2 * half_height / uv_unit
        base = len(positions) // 3
        append_vertex((x0, -half_height, z0), (math.cos(a0), 0, math.sin(a0)), (u0, 0))
        append_vertex((x1, -half_height, z1), (math.cos(a1), 0, math.sin(a1)), (u1, 0))
        append_vertex((x1, half_height, z1), (math.cos(a1), 0, math.sin(a1)), (u1, repeat_v))
        append_vertex((x0, half_height, z0), (math.cos(a0), 0, math.sin(a0)), (u0, repeat_v))
        indices.extend((base, base + 2, base + 1, base, base + 3, base + 2))
        bottom_center = append_vertex((0, -half_height, 0), (0, -1, 0), (radius / uv_unit, radius / uv_unit))
        bottom0 = append_vertex((x0, -half_height, z0), (0, -1, 0), ((x0 + radius) / uv_unit, (z0 + radius) / uv_unit))
        bottom1 = append_vertex((x1, -half_height, z1), (0, -1, 0), ((x1 + radius) / uv_unit, (z1 + radius) / uv_unit))
        indices.extend((bottom_center, bottom0, bottom1))
        top_center = append_vertex((0, half_height, 0), (0, 1, 0), (radius / uv_unit, radius / uv_unit))
        top0 = append_vertex((x0, half_height, z0), (0, 1, 0), ((x0 + radius) / uv_unit, (z0 + radius) / uv_unit))
        top1 = append_vertex((x1, half_height, z1), (0, 1, 0), ((x1 + radius) / uv_unit, (z1 + radius) / uv_unit))
        indices.extend((top_center, top1, top0))
    minimum = [min(positions[index::3]) for index in range(3)]
    maximum = [max(positions[index::3]) for index in range(3)]
    return (
        struct.pack(f"<{len(positions)}f", *positions),
        struct.pack(f"<{len(normals)}f", *normals),
        struct.pack(f"<{len(uvs)}f", *uvs),
        struct.pack(f"<{len(indices)}H", *indices),
        minimum,
        maximum,
    )


def _add_binary_view(binary: bytearray, views: List[Dict[str, Any]], payload: bytes) -> int:
    """Append a self-contained non-geometry buffer view (for embedded images)."""

    binary.extend(b"\x00" * ((4 - len(binary) % 4) % 4))
    offset = len(binary)
    binary.extend(payload)
    views.append({"buffer": 0, "byteOffset": offset, "byteLength": len(payload)})
    return len(views) - 1


def _add_accessor(binary: bytearray, views: List[Dict[str, Any]], accessors: List[Dict[str, Any]], payload: bytes, component_type: int, count: int, value_type: str, minimum: List[float] = None, maximum: List[float] = None, *, target: int = 34962) -> int:
    binary.extend(b"\x00" * ((4 - len(binary) % 4) % 4))
    offset = len(binary)
    binary.extend(payload)
    view_index = len(views)
    views.append({"buffer": 0, "byteOffset": offset, "byteLength": len(payload), "target": target})
    accessor: Dict[str, Any] = {"bufferView": view_index, "componentType": component_type, "count": count, "type": value_type}
    if minimum is not None:
        accessor["min"] = minimum
    if maximum is not None:
        accessor["max"] = maximum
    accessors.append(accessor)
    return len(accessors) - 1


def _canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
