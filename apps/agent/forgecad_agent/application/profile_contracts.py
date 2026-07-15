from __future__ import annotations

import hashlib
import json
import math
import warnings
from typing import Annotated, Any, Literal, Mapping, Union

from jsonschema import Draft202012Validator, RefResolver
from pydantic import AfterValidator, BaseModel, ConfigDict, Field, model_validator

from forgecad_agent.runtime_paths import runtime_resource_root


warnings.simplefilter("ignore", DeprecationWarning)

SCHEMA_ROOT = runtime_resource_root() / "packages" / "concept-spec" / "schemas"
_PROFILE_SCHEMA = json.loads((SCHEMA_ROOT / "profile-sketch.schema.json").read_text(encoding="utf-8"))
_SECTION_SET_SCHEMA = json.loads((SCHEMA_ROOT / "profile-section-set.schema.json").read_text(encoding="utf-8"))
_PROFILE_VALIDATOR = Draft202012Validator(_PROFILE_SCHEMA)
_SECTION_SET_VALIDATOR = Draft202012Validator(
    _SECTION_SET_SCHEMA,
    resolver=RefResolver.from_schema(
        _SECTION_SET_SCHEMA,
        store={_PROFILE_SCHEMA["$id"]: _PROFILE_SCHEMA},
    ),
)

Point2 = tuple[float, float]
_EPSILON = 1e-9
_MAX_TOTAL_SEGMENTS = 128


class ProfileContractValidationError(ValueError):
    pass


class _StrictProfileModel(BaseModel):
    model_config = ConfigDict(extra="forbid", allow_inf_nan=False)


class LineSegment(_StrictProfileModel):
    kind: Literal["line"]
    to: tuple[float, float]


class QuadraticSegment(_StrictProfileModel):
    kind: Literal["quadratic"]
    control: tuple[float, float]
    to: tuple[float, float]


class CubicSegment(_StrictProfileModel):
    kind: Literal["cubic"]
    control_1: tuple[float, float]
    control_2: tuple[float, float]
    to: tuple[float, float]


ProfileSegment = Union[LineSegment, QuadraticSegment, CubicSegment]


class ProfileBounds(_StrictProfileModel):
    min: tuple[float, float]
    max: tuple[float, float]


class ProfileProvenance(_StrictProfileModel):
    source: Literal["agent", "svg_editor", "component_recipe", "reference_rebuild"]
    source_ref: str | None = Field(default=None, pattern=r"^[a-z][a-z0-9_\-]{1,119}$")


class ProfileHole(_StrictProfileModel):
    hole_id: str = Field(pattern=r"^hole_[a-z0-9_\-]+$")
    winding: Literal["clockwise", "counter_clockwise"]
    start: tuple[float, float]
    segments: list[ProfileSegment] = Field(min_length=3, max_length=64)


class ProfileSketch(_StrictProfileModel):
    schema_version: Literal["ProfileSketch@1"] = "ProfileSketch@1"
    sketch_id: str = Field(pattern=r"^sketch_[a-z0-9_\-]+$")
    version: Literal[1] = 1
    plane: Literal["front", "side", "top", "cross_section"]
    closed: bool
    winding: Literal["open", "clockwise", "counter_clockwise"]
    start: tuple[float, float]
    segments: list[ProfileSegment] = Field(min_length=1, max_length=64)
    holes: list[ProfileHole] = Field(default_factory=list, max_length=8)
    normalized_bounds: ProfileBounds
    symmetry: Literal["none", "horizontal", "vertical", "radial"] = "none"
    continuity_hint: Literal["linear", "tangent", "smooth"] = "linear"
    resample_count: int = Field(ge=8, le=256)
    provenance: ProfileProvenance

    @model_validator(mode="after")
    def validate_geometry(self) -> "ProfileSketch":
        _assert_profile_semantics(self)
        return self


class ProfileSection(_StrictProfileModel):
    section_id: str = Field(pattern=r"^section_[a-z0-9_\-]+$")
    position: float = Field(ge=-1, le=1)
    profile_sketch_id: str = Field(pattern=r"^sketch_[a-z0-9_\-]+$")
    scale: float = Field(ge=0.25, le=4)
    twist_degrees: float = Field(ge=-45, le=45)
    cap_policy: Literal["none", "start", "end"]


class ProfileResamplePolicy(_StrictProfileModel):
    mode: Literal["uniform_count"] = "uniform_count"
    count: int = Field(ge=8, le=256)


class ProfileSectionSet(_StrictProfileModel):
    schema_version: Literal["ProfileSectionSet@1"] = "ProfileSectionSet@1"
    section_set_id: str = Field(pattern=r"^sectionset_[a-z0-9_\-]+$")
    version: Literal[1] = 1
    main_axis: Literal["x", "y", "z"]
    profiles: list[ProfileSketch] = Field(min_length=1, max_length=12)
    sections: list[ProfileSection] = Field(min_length=2, max_length=12)
    resample_policy: ProfileResamplePolicy
    symmetry: Literal["none", "horizontal", "vertical", "radial"] = "none"
    provenance: ProfileProvenance

    @model_validator(mode="after")
    def validate_sections(self) -> "ProfileSectionSet":
        _assert_section_set_semantics(self)
        return self


def validate_profile_sketch(value: Mapping[str, Any]) -> dict[str, Any]:
    candidate = dict(value)
    _raise_schema_error(_PROFILE_VALIDATOR, candidate, "PROFILE_SKETCH_SCHEMA_INVALID")
    try:
        model = ProfileSketch.model_validate(candidate)
    except Exception as exc:
        if isinstance(exc, ProfileContractValidationError):
            raise
        raise ProfileContractValidationError(f"PROFILE_SKETCH_INVALID: {exc}") from exc
    return model.model_dump(mode="json", exclude_none=True)


def validate_profile_section_set(value: Mapping[str, Any]) -> dict[str, Any]:
    candidate = dict(value)
    _raise_schema_error(_SECTION_SET_VALIDATOR, candidate, "PROFILE_SECTION_SET_SCHEMA_INVALID")
    try:
        model = ProfileSectionSet.model_validate(candidate)
    except Exception as exc:
        if isinstance(exc, ProfileContractValidationError):
            raise
        raise ProfileContractValidationError(f"PROFILE_SECTION_SET_INVALID: {exc}") from exc
    return model.model_dump(mode="json", exclude_none=True)


def normalize_profile_sketch(value: Mapping[str, Any]) -> dict[str, Any]:
    model = ProfileSketch.model_validate(validate_profile_sketch(value))
    payload = model.model_dump(mode="json", exclude_none=True)
    if model.closed and model.winding == "clockwise":
        payload["start"], payload["segments"] = _reverse_contour(model.start, model.segments)
        payload["winding"] = "counter_clockwise"
    normalized_holes: list[dict[str, Any]] = []
    for hole in model.holes:
        item = hole.model_dump(mode="json")
        if hole.winding == "counter_clockwise":
            item["start"], item["segments"] = _reverse_contour(hole.start, hole.segments)
            item["winding"] = "clockwise"
        normalized_holes.append(item)
    payload["holes"] = sorted(normalized_holes, key=lambda item: item["hole_id"])
    return _normalize_numbers(payload)


def normalize_profile_section_set(value: Mapping[str, Any]) -> dict[str, Any]:
    model = ProfileSectionSet.model_validate(validate_profile_section_set(value))
    payload = model.model_dump(mode="json", exclude_none=True)
    payload["profiles"] = sorted(
        (normalize_profile_sketch(profile.model_dump(mode="json", exclude_none=True)) for profile in model.profiles),
        key=lambda item: item["sketch_id"],
    )
    return _normalize_numbers(payload)


def canonical_profile_payload(value: Mapping[str, Any]) -> tuple[dict[str, Any], str, str]:
    schema_version = value.get("schema_version")
    if schema_version == "ProfileSketch@1":
        normalized = normalize_profile_sketch(value)
    elif schema_version == "ProfileSectionSet@1":
        normalized = normalize_profile_section_set(value)
    else:
        raise ProfileContractValidationError("PROFILE_CONTRACT_VERSION_UNSUPPORTED")
    canonical = json.dumps(normalized, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)
    return normalized, canonical, hashlib.sha256(canonical.encode("utf-8")).hexdigest()


def resample_profile_contour(value: Mapping[str, Any]) -> list[list[float]]:
    model = ProfileSketch.model_validate(validate_profile_sketch(value))
    points = _dense_contour(model.start, model.segments, model.closed)
    return [[x, y] for x, y in _resample_polyline(points, model.resample_count, model.closed)]


def resample_profile_sketch(value: Mapping[str, Any]) -> tuple[dict[str, Any], list[list[float]], list[list[list[float]]]]:
    """Return canonical contract plus deterministic outer/hole samples.

    Closed contours deliberately omit the repeated closing point.  Geometry
    executors close the ring explicitly, so the same sample count can be used
    by Extrude, Revolve, Loft and Sweep without seam-dependent duplicates.
    """

    normalized = normalize_profile_sketch(value)
    model = ProfileSketch.model_validate(normalized)
    outer = _resample_polyline(_dense_contour(model.start, model.segments, model.closed), model.resample_count, model.closed)
    holes = [
        _resample_polyline(_dense_contour(hole.start, hole.segments, True), model.resample_count, True)
        for hole in model.holes
    ]
    return (
        normalized,
        [[x, y] for x, y in outer],
        [[[x, y] for x, y in contour] for contour in holes],
    )


def resample_profile_section_set(
    value: Mapping[str, Any],
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    """Return canonical ordered sections with one fixed sample seam.

    Each section keeps the contract's position, scale, twist and cap policy.
    The referenced cross-section is sampled at the shared uniform count; no
    executor is allowed to reorder or independently resample a ring.
    """

    normalized = normalize_profile_section_set(value)
    model = ProfileSectionSet.model_validate(normalized)
    profiles = {profile.sketch_id: profile for profile in model.profiles}
    sections: list[dict[str, Any]] = []
    for section in model.sections:
        profile = profiles[section.profile_sketch_id]
        _canonical, outer, holes = resample_profile_sketch(profile.model_dump(mode="json", exclude_none=True))
        sections.append({
            **section.model_dump(mode="json"),
            "points": outer,
            "holes": holes,
        })
    return normalized, sections


def _validate_profile_payload(value: dict[str, Any]) -> dict[str, Any]:
    return validate_profile_sketch(value)


def _validate_section_set_payload(value: dict[str, Any]) -> dict[str, Any]:
    return validate_profile_section_set(value)


ProfileSketchPayload = Annotated[dict[str, Any], AfterValidator(_validate_profile_payload)]
ProfileSectionSetPayload = Annotated[dict[str, Any], AfterValidator(_validate_section_set_payload)]


def _raise_schema_error(validator: Draft202012Validator, candidate: Mapping[str, Any], code: str) -> None:
    errors = sorted(validator.iter_errors(candidate), key=lambda error: list(error.path))
    if errors:
        location = ".".join(str(part) for part in errors[0].path) or "$"
        raise ProfileContractValidationError(f"{code} at {location}: {errors[0].message}")


def _assert_profile_semantics(profile: ProfileSketch) -> None:
    _assert_points_normalized(profile)
    total_segments = len(profile.segments) + sum(len(hole.segments) for hole in profile.holes)
    if total_segments > _MAX_TOTAL_SEGMENTS:
        raise ProfileContractValidationError("PROFILE_SEGMENT_BUDGET_EXCEEDED")
    if profile.closed:
        if profile.winding == "open" or len(profile.segments) < 3 or not _same_point(profile.segments[-1].to, profile.start):
            raise ProfileContractValidationError("PROFILE_CLOSED_CONTOUR_INVALID")
    elif profile.winding != "open" or _same_point(profile.segments[-1].to, profile.start) or profile.holes:
        raise ProfileContractValidationError("PROFILE_OPEN_CONTOUR_INVALID")

    outer = _dense_contour(profile.start, profile.segments, profile.closed)
    _assert_curve_not_degenerate(profile.start, profile.segments, "PROFILE_OUTER_DEGENERATE")
    _assert_no_self_intersection(outer, profile.closed, "PROFILE_OUTER_SELF_INTERSECTION")
    if profile.closed:
        area = _signed_area(outer[:-1])
        if abs(area) <= _EPSILON:
            raise ProfileContractValidationError("PROFILE_OUTER_DEGENERATE")
        expected = "counter_clockwise" if area > 0 else "clockwise"
        if profile.winding != expected:
            raise ProfileContractValidationError("PROFILE_OUTER_WINDING_MISMATCH")

    expected_bounds = _control_bounds(profile)
    actual = profile.normalized_bounds
    if not all(math.isclose(left, right, abs_tol=1e-9) for left, right in zip((*expected_bounds[0], *expected_bounds[1]), (*actual.min, *actual.max))):
        raise ProfileContractValidationError("PROFILE_NORMALIZED_BOUNDS_MISMATCH")
    if actual.max[0] - actual.min[0] <= _EPSILON or actual.max[1] - actual.min[1] <= _EPSILON:
        raise ProfileContractValidationError("PROFILE_NORMALIZED_BOUNDS_DEGENERATE")

    hole_ids: set[str] = set()
    hole_polylines: list[list[Point2]] = []
    for hole in profile.holes:
        if hole.hole_id in hole_ids:
            raise ProfileContractValidationError("PROFILE_DUPLICATE_HOLE")
        hole_ids.add(hole.hole_id)
        if not _same_point(hole.segments[-1].to, hole.start):
            raise ProfileContractValidationError("PROFILE_HOLE_NOT_CLOSED")
        _assert_curve_not_degenerate(hole.start, hole.segments, "PROFILE_HOLE_DEGENERATE")
        polyline = _dense_contour(hole.start, hole.segments, True)
        _assert_no_self_intersection(polyline, True, "PROFILE_HOLE_SELF_INTERSECTION")
        area = _signed_area(polyline[:-1])
        expected = "counter_clockwise" if area > 0 else "clockwise"
        if abs(area) <= _EPSILON or hole.winding != expected:
            raise ProfileContractValidationError("PROFILE_HOLE_WINDING_MISMATCH")
        if not _point_in_polygon(polyline[0], outer[:-1]) or _polylines_intersect(polyline, outer):
            raise ProfileContractValidationError("PROFILE_HOLE_OUTSIDE_OUTER")
        for other in hole_polylines:
            if _polylines_intersect(polyline, other) or _point_in_polygon(polyline[0], other[:-1]) or _point_in_polygon(other[0], polyline[:-1]):
                raise ProfileContractValidationError("PROFILE_HOLES_OVERLAP")
        hole_polylines.append(polyline)


def _assert_section_set_semantics(section_set: ProfileSectionSet) -> None:
    profile_by_id = {profile.sketch_id: profile for profile in section_set.profiles}
    if len(profile_by_id) != len(section_set.profiles):
        raise ProfileContractValidationError("PROFILE_SECTION_SET_DUPLICATE_PROFILE")
    section_ids = [section.section_id for section in section_set.sections]
    if len(section_ids) != len(set(section_ids)):
        raise ProfileContractValidationError("PROFILE_SECTION_SET_DUPLICATE_SECTION")
    positions = [section.position for section in section_set.sections]
    if any(right - left <= _EPSILON for left, right in zip(positions, positions[1:])):
        raise ProfileContractValidationError("PROFILE_SECTION_SET_ORDER_INVALID")
    for index, section in enumerate(section_set.sections):
        profile = profile_by_id.get(section.profile_sketch_id)
        if profile is None:
            raise ProfileContractValidationError("PROFILE_SECTION_SET_PROFILE_MISSING")
        if not profile.closed or profile.plane != "cross_section":
            raise ProfileContractValidationError("PROFILE_SECTION_SET_REQUIRES_CLOSED_CROSS_SECTION")
        if profile.resample_count != section_set.resample_policy.count:
            raise ProfileContractValidationError("PROFILE_SECTION_SET_RESAMPLE_MISMATCH")
        allowed_cap = {"none"}
        if index == 0:
            allowed_cap.add("start")
        if index == len(section_set.sections) - 1:
            allowed_cap.add("end")
        if section.cap_policy not in allowed_cap:
            raise ProfileContractValidationError("PROFILE_SECTION_SET_CAP_POSITION_INVALID")


def _assert_points_normalized(profile: ProfileSketch) -> None:
    points = [profile.start]
    for segment in profile.segments:
        points.extend(_segment_control_points(segment))
    for hole in profile.holes:
        points.append(hole.start)
        for segment in hole.segments:
            points.extend(_segment_control_points(segment))
    if any(not math.isfinite(value) or value < -1 or value > 1 for point in points for value in point):
        raise ProfileContractValidationError("PROFILE_POINT_OUT_OF_NORMALIZED_BOUNDS")


def _control_bounds(profile: ProfileSketch) -> tuple[Point2, Point2]:
    points: list[Point2] = [profile.start]
    for segment in profile.segments:
        points.extend(_segment_control_points(segment))
    for hole in profile.holes:
        points.append(hole.start)
        for segment in hole.segments:
            points.extend(_segment_control_points(segment))
    return (min(point[0] for point in points), min(point[1] for point in points)), (max(point[0] for point in points), max(point[1] for point in points))


def _segment_control_points(segment: ProfileSegment) -> list[Point2]:
    if isinstance(segment, LineSegment):
        return [segment.to]
    if isinstance(segment, QuadraticSegment):
        return [segment.control, segment.to]
    return [segment.control_1, segment.control_2, segment.to]


def _assert_curve_not_degenerate(start: Point2, segments: list[ProfileSegment], code: str) -> None:
    current = start
    total = 0.0
    for segment in segments:
        samples = [_evaluate_segment(current, segment, step / 16) for step in range(1, 17)]
        previous = current
        for point in samples:
            total += math.dist(previous, point)
            previous = point
        current = segment.to
    if total <= _EPSILON:
        raise ProfileContractValidationError(code)


def _dense_contour(start: Point2, segments: list[ProfileSegment], closed: bool) -> list[Point2]:
    points: list[Point2] = [start]
    current = start
    for segment in segments:
        points.extend(_evaluate_segment(current, segment, step / 16) for step in range(1, 17))
        current = segment.to
    if closed and not _same_point(points[-1], points[0]):
        points.append(points[0])
    return points


def _evaluate_segment(start: Point2, segment: ProfileSegment, t: float) -> Point2:
    u = 1 - t
    if isinstance(segment, LineSegment):
        return (u * start[0] + t * segment.to[0], u * start[1] + t * segment.to[1])
    if isinstance(segment, QuadraticSegment):
        return (
            u * u * start[0] + 2 * u * t * segment.control[0] + t * t * segment.to[0],
            u * u * start[1] + 2 * u * t * segment.control[1] + t * t * segment.to[1],
        )
    return (
        u**3 * start[0] + 3 * u * u * t * segment.control_1[0] + 3 * u * t * t * segment.control_2[0] + t**3 * segment.to[0],
        u**3 * start[1] + 3 * u * u * t * segment.control_1[1] + 3 * u * t * t * segment.control_2[1] + t**3 * segment.to[1],
    )


def _reverse_contour(start: Point2, segments: list[ProfileSegment]) -> tuple[list[float], list[dict[str, Any]]]:
    starts: list[Point2] = []
    current = start
    for segment in segments:
        starts.append(current)
        current = segment.to
    reversed_segments: list[dict[str, Any]] = []
    for original_start, segment in reversed(list(zip(starts, segments))):
        if isinstance(segment, LineSegment):
            reversed_segments.append({"kind": "line", "to": list(original_start)})
        elif isinstance(segment, QuadraticSegment):
            reversed_segments.append({"kind": "quadratic", "control": list(segment.control), "to": list(original_start)})
        else:
            reversed_segments.append({
                "kind": "cubic",
                "control_1": list(segment.control_2),
                "control_2": list(segment.control_1),
                "to": list(original_start),
            })
    return list(start), reversed_segments


def _signed_area(points: list[Point2]) -> float:
    return 0.5 * sum(points[index][0] * points[(index + 1) % len(points)][1] - points[(index + 1) % len(points)][0] * points[index][1] for index in range(len(points)))


def _same_point(left: Point2, right: Point2) -> bool:
    return math.dist(left, right) <= _EPSILON


def _assert_no_self_intersection(points: list[Point2], closed: bool, code: str) -> None:
    edges = list(zip(points, points[1:]))
    for left_index, left in enumerate(edges):
        for right_index in range(left_index + 1, len(edges)):
            if right_index in {left_index, left_index + 1}:
                continue
            if closed and left_index == 0 and right_index == len(edges) - 1:
                continue
            if _segments_intersect(*left, *edges[right_index]):
                raise ProfileContractValidationError(code)


def _segments_intersect(a: Point2, b: Point2, c: Point2, d: Point2) -> bool:
    def orient(p: Point2, q: Point2, r: Point2) -> float:
        return (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0])

    def on_segment(p: Point2, q: Point2, r: Point2) -> bool:
        return (
            min(p[0], r[0]) - _EPSILON <= q[0] <= max(p[0], r[0]) + _EPSILON
            and min(p[1], r[1]) - _EPSILON <= q[1] <= max(p[1], r[1]) + _EPSILON
        )

    ab_c, ab_d, cd_a, cd_b = orient(a, b, c), orient(a, b, d), orient(c, d, a), orient(c, d, b)
    if ((ab_c > _EPSILON and ab_d < -_EPSILON) or (ab_c < -_EPSILON and ab_d > _EPSILON)) and ((cd_a > _EPSILON and cd_b < -_EPSILON) or (cd_a < -_EPSILON and cd_b > _EPSILON)):
        return True
    return (
        (abs(ab_c) <= _EPSILON and on_segment(a, c, b))
        or (abs(ab_d) <= _EPSILON and on_segment(a, d, b))
        or (abs(cd_a) <= _EPSILON and on_segment(c, a, d))
        or (abs(cd_b) <= _EPSILON and on_segment(c, b, d))
    )


def _polylines_intersect(left: list[Point2], right: list[Point2]) -> bool:
    return any(_segments_intersect(a, b, c, d) for a, b in zip(left, left[1:]) for c, d in zip(right, right[1:]))


def _point_in_polygon(point: Point2, polygon: list[Point2]) -> bool:
    inside = False
    j = len(polygon) - 1
    for i, current in enumerate(polygon):
        previous = polygon[j]
        if (current[1] > point[1]) != (previous[1] > point[1]):
            crossing = (previous[0] - current[0]) * (point[1] - current[1]) / (previous[1] - current[1]) + current[0]
            if point[0] < crossing:
                inside = not inside
        j = i
    return inside


def _resample_polyline(points: list[Point2], count: int, closed: bool) -> list[Point2]:
    distances = [0.0]
    for left, right in zip(points, points[1:]):
        distances.append(distances[-1] + math.dist(left, right))
    total = distances[-1]
    if total <= _EPSILON:
        raise ProfileContractValidationError("PROFILE_RESAMPLE_DEGENERATE")
    denominator = count if closed else count - 1
    targets = [total * index / denominator for index in range(count)]
    result: list[Point2] = []
    edge = 0
    for target in targets:
        while edge + 1 < len(distances) and distances[edge + 1] < target:
            edge += 1
        span = distances[edge + 1] - distances[edge]
        ratio = 0.0 if span <= _EPSILON else (target - distances[edge]) / span
        result.append((
            points[edge][0] + (points[edge + 1][0] - points[edge][0]) * ratio,
            points[edge][1] + (points[edge + 1][1] - points[edge][1]) * ratio,
        ))
    return result


def _normalize_numbers(value: Any) -> Any:
    if isinstance(value, float):
        rounded = round(value, 12)
        return 0.0 if rounded == 0 else rounded
    if isinstance(value, list):
        return [_normalize_numbers(item) for item in value]
    if isinstance(value, dict):
        return {key: _normalize_numbers(item) for key, item in value.items()}
    return value
