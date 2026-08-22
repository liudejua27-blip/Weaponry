#!/usr/bin/env python3
"""Focused source gate for the versioned multi-loop profile loft contract.

JSON Schema covers the closed field shape and scalar bounds.  This checker
adds the cross-station topology invariants that JSON Schema cannot express:
stable station/component/hole IDs, globally unique hole IDs per section,
loop winding, containment, and the bounded G0/Manifold continuity policy.
"""

from __future__ import annotations

import copy
import hashlib
import json
import math
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
SCHEMAS = ROOT / "packages" / "forgecad-contracts" / "schemas"
FIXTURES = ROOT / "packages" / "forgecad-contracts" / "fixtures" / "multi-loop-profile-loft-p1"
MIRRORS = [
    ROOT / "packages" / "forgecad-skills" / "bundles" / "hard-surface-detail" / "0.2.0" / "schemas" / "geometry-program-v2.schema.json",
    ROOT / "packages" / "forgecad-skills" / "bundles" / "primitive-blockout" / "0.2.0" / "schemas" / "geometry-program-v2.schema.json",
    ROOT / "packages" / "forgecad-skills" / "bundles" / "uv-pbr" / "0.2.0" / "schemas" / "geometry-program-v2.schema.json",
]

sys.path.insert(0, str(ROOT / "scripts"))
from check_agentic_contracts import is_valid  # noqa: E402


def fail(message: str) -> None:
    raise SystemExit(f"Multi-loop profile loft P1 violation: {message}")


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def load(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        fail(f"cannot load {path.relative_to(ROOT)}: {exc}")


def canonical_hash(value: Any) -> str:
    encoded = json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


Point = tuple[float, float]


def signed_area(points: list[Point]) -> float:
    return 0.5 * sum(
        points[index][0] * points[(index + 1) % len(points)][1]
        - points[(index + 1) % len(points)][0] * points[index][1]
        for index in range(len(points))
    )


def orientation(a: Point, b: Point, c: Point) -> float:
    return (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])


def on_segment(point: Point, start: Point, end: Point) -> bool:
    return (
        abs(orientation(start, end, point)) <= 1e-9
        and min(start[0], end[0]) - 1e-9 <= point[0] <= max(start[0], end[0]) + 1e-9
        and min(start[1], end[1]) - 1e-9 <= point[1] <= max(start[1], end[1]) + 1e-9
    )


def segments_intersect(a: Point, b: Point, c: Point, d: Point) -> bool:
    ab_c = orientation(a, b, c)
    ab_d = orientation(a, b, d)
    cd_a = orientation(c, d, a)
    cd_b = orientation(c, d, b)
    if (ab_c > 1e-9 and ab_d < -1e-9 or ab_c < -1e-9 and ab_d > 1e-9) and (
        cd_a > 1e-9 and cd_b < -1e-9 or cd_a < -1e-9 and cd_b > 1e-9
    ):
        return True
    return any(
        abs(value) <= 1e-9 and on_segment(point, start, end)
        for value, point, start, end in (
            (ab_c, c, a, b),
            (ab_d, d, a, b),
            (cd_a, a, c, d),
            (cd_b, b, c, d),
        )
    )


def self_crossing(points: list[Point]) -> bool:
    for index, start in enumerate(points):
        end = points[(index + 1) % len(points)]
        for other in range(index + 1, len(points)):
            if {index, (index + 1) % len(points)} & {other, (other + 1) % len(points)}:
                continue
            other_end = points[(other + 1) % len(points)]
            if segments_intersect(start, end, points[other], other_end):
                return True
    return False


def on_boundary(point: Point, polygon: list[Point]) -> bool:
    return any(
        on_segment(point, polygon[index], polygon[(index + 1) % len(polygon)])
        for index in range(len(polygon))
    )


def inside_strict(point: Point, polygon: list[Point]) -> bool:
    if on_boundary(point, polygon):
        return False
    inside = False
    for index, start in enumerate(polygon):
        end = polygon[(index + 1) % len(polygon)]
        if (start[1] > point[1]) != (end[1] > point[1]):
            x_at_y = (end[0] - start[0]) * (point[1] - start[1]) / (end[1] - start[1]) + start[0]
            if point[0] < x_at_y:
                inside = not inside
    return inside


def polygon_boundaries_touch(left: list[Point], right: list[Point]) -> bool:
    return any(
        segments_intersect(left[index], left[(index + 1) % len(left)], right[other], right[(other + 1) % len(right)])
        for index in range(len(left))
        for other in range(len(right))
    )


def points(value: Any, label: str) -> list[Point]:
    require(isinstance(value, dict), f"{label} must be an object")
    raw = value.get("points")
    require(isinstance(raw, list) and 3 <= len(raw) <= 64, f"{label} points are out of bounds")
    result: list[Point] = []
    for point in raw:
        require(
            isinstance(point, list)
            and len(point) == 2
            and all(isinstance(item, (int, float)) and not isinstance(item, bool) and math.isfinite(item) for item in point)
            and all(-5 <= item <= 5 for item in point),
            f"{label} contains an invalid point",
        )
        result.append((float(point[0]), float(point[1])))
    corners = value.get("corner_indices")
    if corners is not None:
        require(
            isinstance(corners, list)
            and len(corners) == len(set(corners))
            and all(isinstance(item, int) and not isinstance(item, bool) and 0 <= item < len(result) for item in corners),
            f"{label} corner_indices are invalid",
        )
    return result


def topology_is_valid(request: dict[str, Any]) -> bool:
    plan = request["cross_section_plan"]
    stations = plan["stations"]
    expected_components: set[str] | None = None
    expected_holes: dict[str, set[str]] | None = None
    previous_m: float | None = None
    station_ids: set[str] = set()
    for station in stations:
        station_id = station["station_id"]
        require(station_id not in station_ids, "station IDs are not unique")
        station_ids.add(station_id)
        require(previous_m is None or station["station_m"] > previous_m, f"station order drifted at {station_id}")
        previous_m = station["station_m"]
        components = station["components"]
        component_ids = {component["component_id"] for component in components}
        require(len(component_ids) == len(components), f"duplicate component IDs at {station_id}")
        if expected_components is None:
            expected_components = component_ids
        require(component_ids == expected_components, "component ID set drifted between stations")

        parsed: list[tuple[str, list[Point], list[list[Point]]]] = []
        hole_owner: dict[str, str] = {}
        current_holes: dict[str, set[str]] = {}
        for component in components:
            component_id = component["component_id"]
            outer = points(component["outer"], f"{component_id}.outer")
            require(signed_area(outer) > 1e-9, f"{component_id}.outer is not CCW")
            require(not self_crossing(outer), f"{component_id}.outer self-crosses")
            holes = component["holes"]
            hole_ids: set[str] = set()
            parsed_holes: list[list[Point]] = []
            for hole in holes:
                hole_id = hole["hole_id"]
                require(hole_id not in hole_owner, "hole IDs must be globally unique across components")
                hole_owner[hole_id] = component_id
                hole_ids.add(hole_id)
                hole_points = points(hole, f"{component_id}.{hole_id}")
                require(signed_area(hole_points) < -1e-9, f"{hole_id} is not CW")
                require(not self_crossing(hole_points), f"{hole_id} self-crosses")
                require(all(inside_strict(point, outer) for point in hole_points), f"{hole_id} is not strictly inside its outer")
                for previous in parsed_holes:
                    require(not polygon_boundaries_touch(hole_points, previous), "holes overlap or touch")
                    require(not inside_strict(hole_points[0], previous) and not inside_strict(previous[0], hole_points), "holes contain one another")
                parsed_holes.append(hole_points)
            current_holes[component_id] = hole_ids
            parsed.append((component_id, outer, parsed_holes))

        for left_index, left_value in enumerate(parsed):
            for right_value in parsed[left_index + 1 :]:
                left_id, left_outer, left_holes = left_value
                right_id, right_outer, right_holes = right_value
                require(
                    not any(
                        polygon_boundaries_touch(left_loop, right_loop)
                        for left_loop in [left_outer, *left_holes]
                        for right_loop in [right_outer, *right_holes]
                    ),
                    f"components overlap or touch ({left_id},{right_id})",
                )
                left_in_right = all(inside_strict(point, right_outer) for point in left_outer)
                right_in_left = all(inside_strict(point, left_outer) for point in right_outer)
                left_in_right_hole = any(all(inside_strict(point, hole) for point in left_outer) for hole in right_holes)
                right_in_left_hole = any(all(inside_strict(point, hole) for point in right_outer) for hole in left_holes)
                require(not left_in_right or left_in_right_hole, "component is nested outside a containing hole")
                require(not right_in_left or right_in_left_hole, "component is nested outside a containing hole")

        if expected_holes is None:
            expected_holes = current_holes
        require(current_holes == expected_holes, "hole ID set drifted between stations")
    return True


def continuity_is_valid(request: dict[str, Any]) -> bool:
    policy = request["continuity_policy"]
    return (
        policy["surface_continuity"] == "g0-only"
        and policy["profile_point_correspondence"] == "canonical-phase-arc-length"
        and policy["station_interpolation"] in {"linear", "catmull-rom-position-only"}
        and 0 <= policy["interpolation_rings"] <= 16
        and 4 <= policy["resample_points"] <= 64
        and policy["endpoint_caps"] == "closed-solid-boolean"
        and policy["hole_policy"] == "manifold-difference"
        and policy["shared_boundary_policy"] == "none"
    )


def mutate(request: dict[str, Any], mutation: str) -> dict[str, Any]:
    value = copy.deepcopy(request)
    stations = value["cross_section_plan"]["stations"]
    if mutation == "duplicate_station_id":
        stations[1]["station_id"] = stations[0]["station_id"]
    elif mutation == "drop_component":
        stations[1]["components"].pop()
    elif mutation == "drop_hole":
        stations[1]["components"][0]["holes"].pop()
    elif mutation == "duplicate_cross_component_hole":
        stations[0]["components"][1]["holes"] = [{
            "hole_id": "hole-a",
            "points": [[-0.05, -0.04], [-0.05, 0.04], [0.05, 0.04], [0.05, -0.04]],
        }]
    elif mutation == "rename_hole_id":
        stations[1]["components"][0]["holes"][1]["hole_id"] = "hole-renamed"
    elif mutation == "reverse_hole":
        stations[0]["components"][0]["holes"][0]["points"].reverse()
    elif mutation == "cross_outer":
        stations[0]["components"][0]["outer"]["points"] = [[-0.9, -0.6], [0.9, 0.6], [0.9, -0.6], [-0.9, 0.6]]
    elif mutation == "touch_outer":
        stations[0]["components"][0]["holes"][0]["points"] = [[-0.9, -0.2], [-0.9, 0.2], [-0.6, 0.2], [-0.6, -0.2]]
    elif mutation in {"g1_policy", "g2_policy"}:
        value["continuity_policy"]["surface_continuity"] = "g1-required" if mutation == "g1_policy" else "g2-required"
    elif mutation == "endpoint_cap_alias":
        value["continuity_policy"]["endpoint_caps"] = "ear-clipped-planar"
    elif mutation == "hole_policy_alias":
        value["continuity_policy"]["hole_policy"] = "reject"
    elif mutation == "invalid_interpolation":
        value["continuity_policy"]["station_interpolation"] = "spline"
    elif mutation == "script_field":
        value["script"] = "forbidden"
    elif mutation == "url_field":
        value["url"] = "https://forbidden.invalid"
    elif mutation == "path_field":
        value["path"] = "/forbidden/file"
    elif mutation == "network_field":
        value["network"] = {"host": "forbidden.invalid"}
    elif mutation == "rings_overflow":
        value["continuity_policy"]["interpolation_rings"] = 17
    else:
        fail(f"unknown negative mutation {mutation}")
    return value


def rebind_hashes(request: dict[str, Any]) -> None:
    plan = request["cross_section_plan"]
    plan["canonical_sha256"] = ""
    plan["canonical_sha256"] = canonical_hash(plan)
    binding = copy.deepcopy(request)
    binding.pop("input_sha256")
    request["input_sha256"] = canonical_hash(binding)


def main() -> None:
    request_schema = load(SCHEMAS / "multi-loop-profile-loft-request-v1.schema.json")
    program_schema = load(SCHEMAS / "multi-loop-profile-loft-program-v1.schema.json")
    geometry_schema = load(SCHEMAS / "geometry-program-v2.schema.json")
    manifest = load(ROOT / "packages" / "forgecad-contracts" / "manifest.json")
    positive = load(FIXTURES / "positive" / "multi-loop-profile-loft.json")
    negative = load(FIXTURES / "negative" / "cases.json")
    registry = {SCHEMAS / filename: load(SCHEMAS / filename) for filename in manifest["schemas"]}
    registry_by_id = {schema["$id"]: schema for schema in registry.values() if isinstance(schema.get("$id"), str)}
    registry_by_id.update({f"https://forgecad.local/contracts/{path.name}": schema for path, schema in registry.items()})

    require(is_valid(request_schema, positive, registry_by_id), "positive request fixture is schema-invalid")
    require(topology_is_valid(positive), "positive topology is invalid")
    require(continuity_is_valid(positive), "positive continuity policy is invalid")
    plan = copy.deepcopy(positive["cross_section_plan"])
    declared_plan_hash = plan["canonical_sha256"]
    plan["canonical_sha256"] = ""
    require(canonical_hash(plan) == declared_plan_hash, "CrossSectionPlan canonical hash is stale")
    binding = copy.deepcopy(positive)
    declared_input_hash = binding.pop("input_sha256")
    require(canonical_hash(binding) == declared_input_hash, "request input hash is stale")

    for case in negative["cases"]:
        mutated = mutate(positive, case["mutation"])
        rebind_hashes(mutated)
        schema_valid = is_valid(request_schema, mutated, registry_by_id)
        try:
            semantic_valid = schema_valid and topology_is_valid(mutated) and continuity_is_valid(mutated)
        except SystemExit:
            semantic_valid = False
        require(not semantic_valid, f"negative case unexpectedly accepted: {case['id']}")

    operators = geometry_schema["$defs"]["geometry_node"]["properties"]["operator_id"]["enum"]
    parameter_refs = {
        item.get("$ref")
        for item in geometry_schema["$defs"]["geometry_node"]["properties"]["parameters"]["oneOf"]
    }
    require("forgecad.geometry.multi-loop-profile-loft@1" in operators, "GeometryProgram@2 omits operator")
    require("#/$defs/multi_loop_profile_loft_parameters" in parameter_refs, "GeometryProgram@2 omits parameters")
    require(
        program_schema["properties"]["lowered_operator_id"]["const"] == "forgecad.geometry.multi-loop-profile-loft@1",
        "Program@1 is not bound to the fixed operator",
    )
    require(
        {"multi-loop-profile-loft-request-v1.schema.json", "multi-loop-profile-loft-program-v1.schema.json"}
        <= set(manifest["schemas"]),
        "contract manifest omits MultiLoopProfileLoft schemas",
    )
    main_bytes = (SCHEMAS / "geometry-program-v2.schema.json").read_bytes()
    for mirror in MIRRORS:
        require(mirror.read_bytes() == main_bytes, f"GeometryProgram@2 mirror drifted: {mirror.relative_to(ROOT)}")
    protocol = (ROOT / "apps" / "desktop" / "src-tauri" / "crates" / "forgecad-worker-protocol" / "src" / "lib.rs").read_text(encoding="utf-8")
    require("forgecad.geometry.multi-loop-profile-loft@1" in protocol, "Worker protocol catalog omits operator")
    for path in (
        ROOT / "apps" / "desktop" / "src-tauri" / "crates" / "forgecad-runtime" / "src" / "skill_registry.rs",
        ROOT / "apps" / "desktop" / "src-tauri" / "crates" / "forgecad-runtime" / "src" / "agentic_action.rs",
        ROOT / "apps" / "desktop" / "src-tauri" / "crates" / "forgecad-mcp" / "src" / "agentic_action_tools.rs",
    ):
        require("forgecad.geometry.multi-loop-profile-loft@1" in path.read_text(encoding="utf-8"), f"allowlist omits operator: {path.relative_to(ROOT)}")
    for path in (
        ROOT / "packages" / "forgecad-skills" / "bundles" / "hard-surface-detail" / "0.2.0" / "manifest.json",
        ROOT / "packages" / "forgecad-skills" / "bundles" / "hard-surface-detail" / "0.2.0" / "operators.lock",
        ROOT / "packages" / "forgecad-skills" / "registry.json",
    ):
        require("forgecad.geometry.multi-loop-profile-loft@1" not in path.read_text(encoding="utf-8"), f"active Skill unexpectedly activated operator: {path.relative_to(ROOT)}")
    print(f"Multi-loop profile loft P1 source gate PASS: topology/hash/closed-fields/{len(negative['cases'])} negative cases/catalog/mirror/allowlist")


if __name__ == "__main__":
    main()
