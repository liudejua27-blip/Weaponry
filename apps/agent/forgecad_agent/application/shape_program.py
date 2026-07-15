from __future__ import annotations

import json
import math
import warnings
from typing import Annotated, Any, Mapping

from jsonschema import Draft202012Validator, RefResolver
from pydantic import AfterValidator

from forgecad_agent.runtime_paths import runtime_resource_root
from forgecad_agent.application.shape_program_runtime import (
    assert_declared_runtime_operations,
    assert_schema_consumes_runtime_manifest,
)

warnings.simplefilter("ignore", DeprecationWarning)


SCHEMA_ROOT = runtime_resource_root() / "packages" / "concept-spec" / "schemas"
_SCHEMA = json.loads((SCHEMA_ROOT / "shape-program.schema.json").read_text(encoding="utf-8"))
_COMMON = json.loads((SCHEMA_ROOT / "common.schema.json").read_text(encoding="utf-8"))
_PROFILE_SKETCH = json.loads((SCHEMA_ROOT / "profile-sketch.schema.json").read_text(encoding="utf-8"))
_PROFILE_SECTION_SET = json.loads((SCHEMA_ROOT / "profile-section-set.schema.json").read_text(encoding="utf-8"))
_VALIDATOR = Draft202012Validator(
    _SCHEMA,
    resolver=RefResolver.from_schema(
        _SCHEMA,
        store={
            _SCHEMA["$id"]: _SCHEMA,
            _COMMON["$id"]: _COMMON,
            _PROFILE_SKETCH["$id"]: _PROFILE_SKETCH,
            _PROFILE_SECTION_SET["$id"]: _PROFILE_SECTION_SET,
        },
    ),
)


class ShapeProgramValidationError(ValueError):
    pass


def validate_shape_program(program: Mapping[str, Any]) -> dict[str, Any]:
    """Validate a JSON ShapeProgram without executing user code or file access."""

    candidate = dict(program)
    # Unknown names are a runtime compatibility failure, not an opportunity
    # for a downstream worker to skip an unrecognised node.
    assert_schema_consumes_runtime_manifest()
    assert_declared_runtime_operations(candidate)
    errors = sorted(_VALIDATOR.iter_errors(candidate), key=lambda error: list(error.path))
    if errors:
        location = ".".join(str(part) for part in errors[0].path) or "$"
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_SCHEMA_INVALID at {location}: {errors[0].message}")
    _assert_finite(candidate)
    _assert_semantics(candidate)
    return candidate


def _validate_shape_program_pydantic_input(value: dict[str, Any]) -> dict[str, Any]:
    """Reuse the manifest-aware JSON Schema boundary from Pydantic API models."""

    return validate_shape_program(value)


# Agent response/version Pydantic models consume this alias.  It deliberately
# keeps the portable JSON Schema contract as the full document validator while
# ensuring Pydantic cannot accept a different operation allow-list.
ShapeProgramPayload = Annotated[dict[str, Any], AfterValidator(_validate_shape_program_pydantic_input)]


def validate_asset_geometry_payload(value: Mapping[str, Any]) -> dict[str, Any]:
    """Validate the geometry payload persisted by an Agent asset version.

    An imported GLB is deliberately a sealed, non-editable reference rather
    than a ShapeProgram.  It needs a narrow persisted representation so the
    G819 ShapeProgram boundary does not reinterpret foreign binary evidence as
    executable geometry.  Every other payload is an editable ShapeProgram and
    must pass the manifest-aware validator above.
    """

    candidate = dict(value)
    if candidate.get("schema_version") != "ExternalGLBReference@1":
        return validate_shape_program(candidate)
    expected = {"schema_version", "source_sha256", "editable", "reason"}
    if set(candidate) != expected:
        raise ShapeProgramValidationError("EXTERNAL_GLB_REFERENCE_SCHEMA_INVALID")
    source_sha256 = candidate.get("source_sha256")
    if not isinstance(source_sha256, str) or len(source_sha256) != 64 or any(char not in "0123456789abcdef" for char in source_sha256):
        raise ShapeProgramValidationError("EXTERNAL_GLB_REFERENCE_SHA256_INVALID")
    if candidate.get("editable") is not False:
        raise ShapeProgramValidationError("EXTERNAL_GLB_REFERENCE_EDITABLE_FORBIDDEN")
    reason = candidate.get("reason")
    if not isinstance(reason, str) or not reason.strip() or len(reason) > 500:
        raise ShapeProgramValidationError("EXTERNAL_GLB_REFERENCE_REASON_INVALID")
    return candidate


def _validate_asset_geometry_pydantic_input(value: dict[str, Any]) -> dict[str, Any]:
    return validate_asset_geometry_payload(value)


# Only AgentAssetVersion may persist this union.  Build, segmentation and
# editable response contracts continue to use ShapeProgramPayload exclusively.
AgentAssetGeometryPayload = Annotated[dict[str, Any], AfterValidator(_validate_asset_geometry_pydantic_input)]


def _assert_finite(value: Any, path: str = "$") -> None:
    if isinstance(value, float) and not math.isfinite(value):
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_NON_FINITE at {path}")
    if isinstance(value, Mapping):
        for key, child in value.items():
            _assert_finite(child, f"{path}.{key}")
    elif isinstance(value, list):
        for index, child in enumerate(value):
            _assert_finite(child, f"{path}[{index}]")


def _assert_semantics(program: Mapping[str, Any]) -> None:
    _assert_profile_input_provenance(program.get("profile_inputs", []))
    profile_inputs = {
        item["input_id"]: item
        for item in program.get("profile_inputs", [])
        if isinstance(item, Mapping)
    }
    parameters = program["parameters"]
    parameter_ids = {item["parameter_id"] for item in parameters}
    if len(parameter_ids) != len(parameters):
        raise ShapeProgramValidationError("SHAPE_PROGRAM_DUPLICATE_PARAMETER")
    for item in parameters:
        if item["min"] > item["max"] or not item["min"] <= item["default"] <= item["max"]:
            raise ShapeProgramValidationError(f"SHAPE_PROGRAM_PARAMETER_RANGE: {item['parameter_id']}")

    operations = program["operations"]
    operation_ids = {item["operation_id"] for item in operations}
    if len(operation_ids) != len(operations):
        raise ShapeProgramValidationError("SHAPE_PROGRAM_DUPLICATE_OPERATION")
    seen: set[str] = set()
    operation_by_id: dict[str, Mapping[str, Any]] = {}
    for operation in operations:
        for input_id in operation["inputs"]:
            if input_id not in seen:
                raise ShapeProgramValidationError(
                    f"SHAPE_PROGRAM_FORWARD_OR_MISSING_REFERENCE: {operation['operation_id']} -> {input_id}"
                )
        parameter_id = operation["args"].get("parameter_id")
        if parameter_id is not None and parameter_id not in parameter_ids:
            raise ShapeProgramValidationError(f"SHAPE_PROGRAM_UNKNOWN_PARAMETER: {parameter_id}")
        op_name = operation["op"]
        if op_name == "profile":
            _assert_profile_source(operation, profile_inputs)
        elif op_name == "extrude":
            if len(operation["inputs"]) != 1 or operation["inputs"][0] not in operation_by_id:
                raise ShapeProgramValidationError(f"SHAPE_PROGRAM_EXTRUDE_PROFILE_INPUT: {operation['operation_id']}")
            if operation_by_id[operation["inputs"][0]]["op"] != "profile":
                raise ShapeProgramValidationError(f"SHAPE_PROGRAM_EXTRUDE_REQUIRES_PROFILE: {operation['operation_id']}")
            _assert_extrude_profile(operation_by_id[operation["inputs"][0]], operation, profile_inputs)
        elif op_name == "revolve":
            if len(operation["inputs"]) != 1 or operation["inputs"][0] not in operation_by_id:
                raise ShapeProgramValidationError(f"SHAPE_PROGRAM_REVOLVE_PROFILE_INPUT: {operation['operation_id']}")
            if operation_by_id[operation["inputs"][0]]["op"] != "profile":
                raise ShapeProgramValidationError(f"SHAPE_PROGRAM_REVOLVE_REQUIRES_PROFILE: {operation['operation_id']}")
            _assert_revolve_profile(operation_by_id[operation["inputs"][0]], operation, profile_inputs)
        elif op_name == "loft":
            _assert_loft(operation, profile_inputs)
        elif op_name == "sweep":
            _assert_sweep(operation, profile_inputs)
        elif op_name == "mirror":
            _assert_transform_reference(operation, operation_by_id, "MIRROR")
            _assert_non_zero_axis(operation, "MIRROR")
        elif op_name == "array":
            _assert_transform_reference(operation, operation_by_id, "ARRAY")
            _assert_non_zero_axis(operation, "ARRAY")
            if int(operation["args"].get("count", 0)) < 2 or float(operation["args"].get("spacing", 0)) <= 0:
                raise ShapeProgramValidationError(f"SHAPE_PROGRAM_ARRAY_BUDGET: {operation['operation_id']}")
        elif op_name == "radial_array":
            _assert_transform_reference(operation, operation_by_id, "RADIAL_ARRAY")
            _assert_non_zero_axis(operation, "RADIAL_ARRAY")
            if int(operation["args"].get("count", 0)) < 2 or float(operation["args"].get("radius", 0)) <= 0:
                raise ShapeProgramValidationError(f"SHAPE_PROGRAM_RADIAL_ARRAY_BUDGET: {operation['operation_id']}")
            angle = float(operation["args"].get("angle", 2 * math.pi))
            if not 0 < angle <= 2 * math.pi:
                raise ShapeProgramValidationError(f"SHAPE_PROGRAM_RADIAL_ARRAY_ANGLE: {operation['operation_id']}")
        elif op_name == "bevel_approx":
            _assert_bevel_reference(operation, operation_by_id)
        elif op_name == "surface_panel":
            _assert_surface_panel_reference(operation, operation_by_id)
        elif op_name in {"union", "subtract"}:
            _assert_boolean_references(operation, operation_by_id, op_name.upper())
        seen.add(operation["operation_id"])
        operation_by_id[operation["operation_id"]] = operation
    output_ids = {item["output_id"] for item in program["outputs"]}
    if len(output_ids) != len(program["outputs"]):
        raise ShapeProgramValidationError("SHAPE_PROGRAM_DUPLICATE_OUTPUT")
    for output in program["outputs"]:
        if output["operation_id"] not in seen:
            raise ShapeProgramValidationError(f"SHAPE_PROGRAM_OUTPUT_REFERENCE_MISSING: {output['operation_id']}")

    serialized = json.dumps(program, ensure_ascii=False, sort_keys=True).lower()
    for forbidden in ("python", "javascript", "exec(", "subprocess", "os.system", "http://", "https://", "../"):
        if forbidden in serialized:
            raise ShapeProgramValidationError(f"SHAPE_PROGRAM_UNSAFE_TOKEN: {forbidden}")


def _assert_profile_input_provenance(profile_inputs: Any) -> None:
    from forgecad_agent.application.profile_contracts import canonical_profile_payload

    if not isinstance(profile_inputs, list):
        raise ShapeProgramValidationError("SHAPE_PROGRAM_PROFILE_INPUTS_INVALID")
    input_ids: set[str] = set()
    for item in profile_inputs:
        input_id = item["input_id"]
        if input_id in input_ids:
            raise ShapeProgramValidationError("SHAPE_PROGRAM_DUPLICATE_PROFILE_INPUT")
        input_ids.add(input_id)
        normalized, _canonical, digest = canonical_profile_payload(item["canonical_payload"])
        schema_version = normalized["schema_version"]
        expected_kind = "profile_sketch" if schema_version == "ProfileSketch@1" else "profile_section_set"
        if item["input_kind"] != expected_kind or item["contract_version"] != schema_version:
            raise ShapeProgramValidationError(f"SHAPE_PROGRAM_PROFILE_INPUT_KIND_MISMATCH: {input_id}")
        if item["canonical_payload"] != normalized:
            raise ShapeProgramValidationError(f"SHAPE_PROGRAM_PROFILE_INPUT_NOT_CANONICAL: {input_id}")
        if item["input_sha256"] != digest:
            raise ShapeProgramValidationError(f"SHAPE_PROGRAM_PROFILE_INPUT_HASH_MISMATCH: {input_id}")


def _assert_profile_points(operation: Mapping[str, Any]) -> None:
    points = operation["args"].get("points")
    if not isinstance(points, list) or len(points) < 3:
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_PROFILE_POINTS: {operation['operation_id']}")
    if any(not isinstance(point, list) or len(point) != 2 for point in points):
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_PROFILE_POINT_DIMENSION: {operation['operation_id']}")
    area = 0.0
    for index, point in enumerate(points):
        next_point = points[(index + 1) % len(points)]
        area += float(point[0]) * float(next_point[1]) - float(next_point[0]) * float(point[1])
    if abs(area) < 1e-6:
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_PROFILE_DEGENERATE: {operation['operation_id']}")


def _assert_profile_source(operation: Mapping[str, Any], profile_inputs: Mapping[str, Mapping[str, Any]]) -> None:
    args = operation["args"]
    has_points = "points" in args
    has_reference = "profile_input_id" in args
    if has_points == has_reference:
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_PROFILE_SOURCE_EXACTLY_ONE: {operation['operation_id']}")
    if has_points:
        _assert_profile_points(operation)
        if any(name in args for name in ("profile_scale", "cap_start", "cap_end", "radial_segments")):
            raise ShapeProgramValidationError(f"SHAPE_PROGRAM_LEGACY_PROFILE_ARGUMENTS: {operation['operation_id']}")
        return
    input_id = args["profile_input_id"]
    profile_input = profile_inputs.get(input_id)
    if profile_input is None or profile_input["input_kind"] != "profile_sketch":
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_PROFILE_INPUT_MISSING: {operation['operation_id']} -> {input_id}")
    scale = args.get("profile_scale")
    if not isinstance(scale, list) or len(scale) != 2 or any(float(value) <= 0 for value in scale):
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_PROFILE_SCALE_INVALID: {operation['operation_id']}")


def _assert_extrude_profile(
    profile: Mapping[str, Any],
    operation: Mapping[str, Any],
    profile_inputs: Mapping[str, Mapping[str, Any]],
) -> None:
    profile_input_id = profile["args"].get("profile_input_id")
    if profile_input_id is None:
        return
    payload = profile_inputs[profile_input_id]["canonical_payload"]
    cap_start = operation["args"].get("cap_start", True)
    cap_end = operation["args"].get("cap_end", True)
    if not payload["closed"] and (cap_start or cap_end):
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_OPEN_EXTRUDE_CAP_FORBIDDEN: {operation['operation_id']}")
    if "radial_segments" in operation["args"]:
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_EXTRUDE_RADIAL_SEGMENTS_FORBIDDEN: {operation['operation_id']}")


def _assert_revolve_profile(
    profile: Mapping[str, Any],
    operation: Mapping[str, Any],
    profile_inputs: Mapping[str, Mapping[str, Any]],
) -> None:
    profile_input_id = profile["args"].get("profile_input_id")
    if profile_input_id is None:
        points = profile["args"].get("points", [])
    else:
        from forgecad_agent.application.profile_contracts import resample_profile_sketch

        payload = profile_inputs[profile_input_id]["canonical_payload"]
        if payload["holes"]:
            raise ShapeProgramValidationError(f"SHAPE_PROGRAM_REVOLVE_HOLES_FORBIDDEN: {operation['operation_id']}")
        _normalized, sampled, _holes = resample_profile_sketch(payload)
        scale = profile["args"]["profile_scale"]
        points = [[point[0] * scale[0], point[1] * scale[1]] for point in sampled]
    if any(float(point[0]) < 0 for point in points) or not any(float(point[0]) > 0 for point in points):
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_REVOLVE_RADIUS: {operation['operation_id']}")
    angle = float(operation["args"].get("angle", 2 * math.pi))
    if not 0 < angle <= 2 * math.pi:
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_REVOLVE_ANGLE: {operation['operation_id']}")
    if profile_input_id is not None and math.isclose(angle, 2 * math.pi, abs_tol=1e-9) and (
        operation["args"].get("cap_start", False) or operation["args"].get("cap_end", False)
    ):
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_FULL_REVOLVE_SEAM_CAP_FORBIDDEN: {operation['operation_id']}")


def _assert_loft(
    operation: Mapping[str, Any],
    profile_inputs: Mapping[str, Mapping[str, Any]],
) -> None:
    if operation["inputs"]:
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_LOFT_INPUTS_FORBIDDEN: {operation['operation_id']}")
    args = operation["args"]
    input_id = args.get("section_set_input_id")
    profile_input = profile_inputs.get(input_id)
    if profile_input is None or profile_input["input_kind"] != "profile_section_set":
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_LOFT_SECTION_SET_MISSING: {operation['operation_id']}")
    scale = args.get("cross_section_scale")
    if not isinstance(scale, list) or len(scale) != 2 or any(float(value) <= 0 for value in scale):
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_LOFT_SCALE_INVALID: {operation['operation_id']}")
    if float(args.get("axis_length", 0)) <= 0:
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_LOFT_AXIS_LENGTH_INVALID: {operation['operation_id']}")
    if args.get("continuity", "linear") != "linear":
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_LOFT_CONTINUITY_UNSUPPORTED: {operation['operation_id']}")
    section_set = profile_input["canonical_payload"]
    profiles = {item["sketch_id"]: item for item in section_set["profiles"]}
    if any(profiles[item["profile_sketch_id"]]["holes"] for item in section_set["sections"]):
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_LOFT_HOLES_FORBIDDEN: {operation['operation_id']}")
    twists = [float(item["twist_degrees"]) for item in section_set["sections"]]
    if any(abs(right - left) > 45 for left, right in zip(twists, twists[1:])):
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_LOFT_TWIST_FLIP_RISK: {operation['operation_id']}")
    from forgecad_agent.application.profile_contracts import resample_profile_section_set

    _normalized, sampled_sections = resample_profile_section_set(section_set)
    scale_u, scale_v = (float(value) for value in scale)
    rings: list[list[tuple[float, float]]] = []
    for section in sampled_sections:
        angle = math.radians(float(section["twist_degrees"]))
        cosine, sine = math.cos(angle), math.sin(angle)
        section_scale = float(section["scale"])
        rings.append([
            (
                (float(point[0]) * scale_u * cosine - float(point[1]) * scale_v * sine) * section_scale,
                (float(point[0]) * scale_u * sine + float(point[1]) * scale_v * cosine) * section_scale,
            )
            for point in section["points"]
        ])
    for left, right in zip(rings, rings[1:]):
        for factor in (0.25, 0.5, 0.75):
            intermediate = [
                (a[0] + (b[0] - a[0]) * factor, a[1] + (b[1] - a[1]) * factor)
                for a, b in zip(left, right)
            ]
            if _polygon_self_intersects(intermediate):
                raise ShapeProgramValidationError(f"SHAPE_PROGRAM_LOFT_SPAN_SELF_INTERSECTION: {operation['operation_id']}")


def _polygon_self_intersects(points: list[tuple[float, float]]) -> bool:
    def orientation(a: tuple[float, float], b: tuple[float, float], c: tuple[float, float]) -> float:
        return (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])

    count = len(points)
    for left_index in range(count):
        left_a, left_b = points[left_index], points[(left_index + 1) % count]
        for right_index in range(left_index + 1, count):
            if right_index in {left_index, (left_index + 1) % count} or (right_index + 1) % count == left_index:
                continue
            right_a, right_b = points[right_index], points[(right_index + 1) % count]
            left_cross = orientation(left_a, left_b, right_a) * orientation(left_a, left_b, right_b)
            right_cross = orientation(right_a, right_b, left_a) * orientation(right_a, right_b, left_b)
            if left_cross < -1e-8 and right_cross < -1e-8:
                return True
    return False


def _assert_sweep(
    operation: Mapping[str, Any],
    profile_inputs: Mapping[str, Mapping[str, Any]],
) -> None:
    if operation["inputs"]:
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_SWEEP_INPUTS_FORBIDDEN: {operation['operation_id']}")
    args = operation["args"]
    input_id = args.get("profile_input_id")
    profile_input = profile_inputs.get(input_id)
    if profile_input is None or profile_input["input_kind"] != "profile_sketch":
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_SWEEP_PROFILE_MISSING: {operation['operation_id']}")
    profile = profile_input["canonical_payload"]
    if not profile["closed"] or profile["plane"] != "cross_section" or profile["holes"]:
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_SWEEP_PROFILE_UNSUPPORTED: {operation['operation_id']}")
    scale = args.get("profile_scale")
    if not isinstance(scale, list) or len(scale) != 2 or any(float(value) <= 0 for value in scale):
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_SWEEP_SCALE_INVALID: {operation['operation_id']}")
    points = args.get("path_points")
    closed = bool(args.get("path_closed", False))
    if not isinstance(points, list) or len(points) < (3 if closed else 2):
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_SWEEP_PATH_INVALID: {operation['operation_id']}")
    vectors: list[tuple[float, float, float]] = []
    minimum_visual_segment = max(float(value) for value in scale) * 1.25
    edge_count = len(points) if closed else len(points) - 1
    for index in range(edge_count):
        left, right = points[index], points[(index + 1) % len(points)]
        vector = tuple(float(right[axis]) - float(left[axis]) for axis in range(3))
        length = math.sqrt(sum(value * value for value in vector))
        if length <= 1e-6:
            raise ShapeProgramValidationError(f"SHAPE_PROGRAM_SWEEP_ZERO_LENGTH: {operation['operation_id']}")
        if length < minimum_visual_segment:
            raise ShapeProgramValidationError(f"SHAPE_PROGRAM_SWEEP_CURVATURE_RADIUS: {operation['operation_id']}")
        vectors.append(tuple(value / length for value in vector))
    turn_count = len(vectors) if closed else len(vectors) - 1
    for index in range(turn_count):
        left, right = vectors[index], vectors[(index + 1) % len(vectors)]
        dot = max(-1.0, min(1.0, sum(a * b for a, b in zip(left, right))))
        if dot < math.cos(math.radians(150)):
            raise ShapeProgramValidationError(f"SHAPE_PROGRAM_SWEEP_FRAME_FLIP: {operation['operation_id']}")
    if closed and (args.get("cap_start", False) or args.get("cap_end", False)):
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_SWEEP_CLOSED_CAP_FORBIDDEN: {operation['operation_id']}")
    if closed and abs(float(args.get("path_twist_degrees", 0))) > 1e-9:
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_SWEEP_CLOSED_TWIST_FORBIDDEN: {operation['operation_id']}")
    if _path_has_obvious_self_intersection(points, closed):
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_SWEEP_PATH_SELF_INTERSECTION: {operation['operation_id']}")


def _path_has_obvious_self_intersection(points: list[list[float]], closed: bool) -> bool:
    edge_count = len(points) if closed else len(points) - 1
    spans = [max(float(point[axis]) for point in points) - min(float(point[axis]) for point in points) for axis in range(3)]
    drop = min(range(3), key=lambda axis: spans[axis])
    keep = [axis for axis in range(3) if axis != drop]

    def cross(a: list[float], b: list[float], c: list[float]) -> float:
        return (float(b[keep[0]]) - float(a[keep[0]])) * (float(c[keep[1]]) - float(a[keep[1]])) - (float(b[keep[1]]) - float(a[keep[1]])) * (float(c[keep[0]]) - float(a[keep[0]]))

    for left_index in range(edge_count):
        for right_index in range(left_index + 1, edge_count):
            if right_index in {left_index, left_index + 1} or (closed and left_index == 0 and right_index == edge_count - 1):
                continue
            a, b = points[left_index], points[(left_index + 1) % len(points)]
            c, d = points[right_index], points[(right_index + 1) % len(points)]
            if cross(a, b, c) * cross(a, b, d) < -1e-8 and cross(c, d, a) * cross(c, d, b) < -1e-8:
                return True
    return False


def _assert_transform_reference(operation: Mapping[str, Any], operation_by_id: Mapping[str, Mapping[str, Any]], label: str) -> None:
    if len(operation["inputs"]) != 1 or operation["inputs"][0] not in operation_by_id:
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_{label}_INPUT: {operation['operation_id']}")
    if operation_by_id[operation["inputs"][0]]["op"] == "profile":
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_{label}_PROFILE_INPUT: {operation['operation_id']}")


def _assert_non_zero_axis(operation: Mapping[str, Any], label: str) -> None:
    axis = operation["args"].get("axis", [])
    if not isinstance(axis, list) or len(axis) != 3 or not any(abs(float(value)) > 1e-9 for value in axis):
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_{label}_AXIS: {operation['operation_id']}")


def _assert_boolean_references(operation: Mapping[str, Any], operation_by_id: Mapping[str, Mapping[str, Any]], label: str) -> None:
    if len(operation["inputs"]) != 2 or any(input_id not in operation_by_id for input_id in operation["inputs"]):
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_{label}_INPUT: {operation['operation_id']}")
    if any(operation_by_id[input_id]["op"] == "profile" for input_id in operation["inputs"]):
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_{label}_PROFILE_INPUT: {operation['operation_id']}")


def _assert_bevel_reference(operation: Mapping[str, Any], operation_by_id: Mapping[str, Mapping[str, Any]]) -> None:
    if len(operation["inputs"]) != 1 or operation["inputs"][0] not in operation_by_id:
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_BEVEL_INPUT: {operation['operation_id']}")
    source_op = operation_by_id[operation["inputs"][0]]["op"]
    if source_op not in {"box", "bevel_approx"}:
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_BEVEL_SOURCE: {operation['operation_id']}")
    radius = float(operation["args"].get("radius", 0))
    segments = int(operation["args"].get("segments", 1))
    if radius <= 0 or segments not in {1, 2, 3}:
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_BEVEL_ARGUMENTS: {operation['operation_id']}")


def _assert_surface_panel_reference(operation: Mapping[str, Any], operation_by_id: Mapping[str, Mapping[str, Any]]) -> None:
    if len(operation["inputs"]) != 1 or operation["inputs"][0] not in operation_by_id:
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_SURFACE_PANEL_INPUT: {operation['operation_id']}")
    source_op = operation_by_id[operation["inputs"][0]]["op"]
    if source_op not in {"box", "bevel_approx"}:
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_SURFACE_PANEL_SOURCE: {operation['operation_id']}")
    size = operation["args"].get("size")
    if size is not None and (not isinstance(size, list) or len(size) != 3 or any(float(value) <= 0 for value in size)):
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_SURFACE_PANEL_SIZE: {operation['operation_id']}")
    axis = operation["args"].get("axis", [0, 1, 0])
    if axis not in ([0, 1, 0], [0, -1, 0]):
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_SURFACE_PANEL_AXIS: {operation['operation_id']}")
    position = operation["args"].get("position")
    if position is not None and abs(float(position[1])) > 1e-9:
        raise ShapeProgramValidationError(f"SHAPE_PROGRAM_SURFACE_PANEL_OFFSET: {operation['operation_id']}")
