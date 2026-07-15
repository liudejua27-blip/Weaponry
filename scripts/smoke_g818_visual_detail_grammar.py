#!/usr/bin/env python3
"""Gate the bounded showcase-only visual detail grammar and material mapping."""

from __future__ import annotations

import json
import struct
from itertools import combinations

from forgecad_agent.application.domain_packs import domain_pack_for_message
from forgecad_agent.application.geometry_worker import (
    build_blockout,
    build_glb_from_shape_program,
    read_shape_program_glb,
    segment_blockout,
)
from forgecad_agent.application.mechanical_planner import DeterministicMechanicalPlanner


BRIEFS = (
    "设计一个细节丰富的未来概念道具，只用于游戏展示",
    "设计一辆细节丰富的概念探索车",
    "设计一架细节丰富的概念飞行器",
    "设计一台细节丰富的展示型机械臂",
)
DETAIL_PREFIXES = (
    "visual_panel_",
    "visual_groove_",
    "visual_guard_",
    "visual_light_strip_",
    "visual_cable_slot_",
    "visual_vent_",
    "visual_fastener_",
)


def _glb_document(payload: bytes) -> dict:
    assert payload[:4] == b"glTF"
    json_length, chunk_type = struct.unpack_from("<II", payload, 12)
    assert chunk_type == 0x4E4F534A
    return json.loads(payload[20:20 + json_length].decode("utf-8").rstrip(" "))


def _glb_bounds_by_role(payload: bytes) -> dict[str, tuple[tuple[float, float, float], tuple[float, float, float]]]:
    """Read per-role AABBs from the POSITION accessors in the emitted GLB."""

    document = _glb_document(payload)
    bounds: dict[str, tuple[list[float], list[float]]] = {}
    for primitive in document["meshes"][0]["primitives"]:
        role = str(primitive["extras"]["forgecad_part_role"])
        accessor = document["accessors"][primitive["attributes"]["POSITION"]]
        minimum = [float(value) for value in accessor["min"]]
        maximum = [float(value) for value in accessor["max"]]
        if role not in bounds:
            bounds[role] = (minimum, maximum)
            continue
        current_min, current_max = bounds[role]
        bounds[role] = (
            [min(current_min[index], minimum[index]) for index in range(3)],
            [max(current_max[index], maximum[index]) for index in range(3)],
        )
    return {
        role: (tuple(minimum), tuple(maximum))
        for role, (minimum, maximum) in bounds.items()
    }


def _assert_aabb_overlap_and_exposure(
    bounds: dict[str, tuple[tuple[float, float, float], tuple[float, float, float]]],
    bridge_role: str,
    target_roles: tuple[str, ...],
) -> None:
    bridge_min, bridge_max = bounds[bridge_role]
    target_bounds = []
    for target_role in target_roles:
        target_min, target_max = bounds[target_role]
        target_bounds.append((target_min, target_max))
        overlap = [
            min(bridge_max[axis], target_max[axis]) - max(bridge_min[axis], target_min[axis])
            for axis in range(3)
        ]
        assert all(value > 0 for value in overlap), (bridge_role, target_role, overlap)

    def intersection_volume(
        boxes: tuple[tuple[tuple[float, float, float], tuple[float, float, float]], ...],
    ) -> float:
        lower = [max(box[0][axis] for box in boxes) for axis in range(3)]
        upper = [min(box[1][axis] for box in boxes) for axis in range(3)]
        lengths = [max(0.0, upper[axis] - lower[axis]) for axis in range(3)]
        return lengths[0] * lengths[1] * lengths[2]

    bridge_bounds = (bridge_min, bridge_max)
    bridge_volume = intersection_volume((bridge_bounds,))
    covered_volume = 0.0
    for count in range(1, len(target_bounds) + 1):
        sign = 1.0 if count % 2 else -1.0
        covered_volume += sign * sum(
            intersection_volume((bridge_bounds, *subset))
            for subset in combinations(target_bounds, count)
        )
    exposed_volume = bridge_volume - covered_volume
    assert exposed_volume > max(1e-6, bridge_volume * 1e-6), (
        bridge_role,
        "bridge AABB is covered by the union of target AABBs",
        exposed_volume,
    )


def main() -> int:
    planner = DeterministicMechanicalPlanner()
    visual_roles_by_pack: dict[str, set[str]] = {}
    plans_by_pack = {}
    for brief in BRIEFS:
        pack = domain_pack_for_message(brief)
        plan = planner.plan_complete_concept(brief=brief, pack=pack, project_id="prj_g818_smoke")
        plans_by_pack[pack.pack_id] = plan
        direction_id = plan.directions[0].direction_id
        quick = build_blockout(plan, direction_id, presentation_profile="quick_sketch")
        showcase = build_blockout(plan, direction_id, presentation_profile="showcase")
        roles = [item["args"]["part_role"] for item in showcase.shape_program["operations"]]
        categories = {prefix for prefix in DETAIL_PREFIXES if any(role.startswith(prefix) for role in roles)}
        assert len(categories) == len(DETAIL_PREFIXES), (pack.pack_id, categories)
        assert not any(role.startswith("visual_") for role in [item["args"]["part_role"] for item in quick.shape_program["operations"]])
        visual_operations = [item for item in showcase.shape_program["operations"] if item["args"]["part_role"].startswith("visual_")]
        visual_primitives = [item for item in visual_operations if item["op"] != "bevel_approx"]
        visual_edge_finishes = [item for item in visual_operations if item["op"] == "bevel_approx"]
        visual_roles_by_pack[pack.pack_id] = {item["args"]["part_role"] for item in visual_primitives}
        domain_tag = {
            "pack_future_weapon_prop": "_prop_",
            "pack_vehicle_concept": "_vehicle_",
            "pack_aircraft_concept": "_aircraft_",
            "pack_robotic_arm_concept": "_robot_",
        }[pack.pack_id]
        assert all(domain_tag in role for role in visual_roles_by_pack[pack.pack_id])
        assert 7 <= len(visual_primitives) <= 20
        assert visual_edge_finishes
        assert len(visual_edge_finishes) <= sum(item["op"] == "box" for item in visual_primitives)
        assert all(len(item["inputs"]) == 1 for item in visual_edge_finishes)
        assert {item["args"].get("material_id") for item in visual_primitives} >= {
            "mat_rubber", "mat_composite", "mat_aluminum", "mat_emissive_blue",
        }
        rebuilt_glb, rebuilt_bounds, rebuilt_triangles = build_glb_from_shape_program(showcase.shape_program)
        assert (rebuilt_bounds, rebuilt_triangles) == (showcase.bounds_mm, showcase.triangle_count)
        assert read_shape_program_glb(rebuilt_glb) == (showcase.triangle_count, showcase.bounds_mm)
        document = _glb_document(showcase.glb_bytes)
        assert len(document["materials"]) == 8
        assert len(document["images"]) == len(document["textures"]) == 40
        material_indices = {primitive["material"] for primitive in document["meshes"][0]["primitives"]}
        assert {1, 3, 6}.issubset(material_indices)
        if pack.pack_id == "pack_vehicle_concept":
            assert 7 in material_indices
        aluminum = document["materials"][1]
        automotive = document["materials"][7]
        assert aluminum["extras"]["forgecad_texture_material_id"] == "mat_aluminum"
        assert automotive["extras"]["forgecad_texture_material_id"] == "mat_automotive_paint"
        assert "KHR_materials_clearcoat" not in aluminum.get("extensions", {})
        assert automotive["extensions"]["KHR_materials_clearcoat"]["clearcoatFactor"] == 0.86
        aluminum_base = aluminum["pbrMetallicRoughness"]["baseColorTexture"]["index"]
        automotive_base = automotive["pbrMetallicRoughness"]["baseColorTexture"]["index"]
        assert aluminum_base != automotive_base
        aluminum_image = document["textures"][aluminum_base]["source"]
        automotive_image = document["textures"][automotive_base]["source"]
        aluminum_map = document["images"][aluminum_image]["extras"]["forgecad_visual_texture"]
        automotive_map = document["images"][automotive_image]["extras"]["forgecad_visual_texture"]
        assert aluminum_map["sha256"] != automotive_map["sha256"]
        parts = segment_blockout(plan, direction_id, presentation_profile="showcase")
        graph_parts = showcase.assembly_graph["parts"]
        assert [part["part_id"] for part in graph_parts] == [part["part_id"] for part in parts]
        for part in graph_parts:
            if part["role"].startswith("visual_"):
                assert part["joints"] == []
                assert part["editable_parameters"] == []
        if pack.pack_id == "pack_robotic_arm_concept":
            visual_part_ids = {part["part_id"] for part in graph_parts if part["role"].startswith("visual_")}
            assert not any(joint["target_part_id"] in visual_part_ids for part in graph_parts for joint in part["joints"])

    role_sets = list(visual_roles_by_pack.values())
    assert all(not left.intersection(right) for index, left in enumerate(role_sets) for right in role_sets[index + 1:])

    selected_variants = {
        "pack_future_weapon_prop": "compact_prop_a",
        "pack_vehicle_concept": "urban_scout_a",
        "pack_aircraft_concept": "vertical_takeoff_a",
        "pack_robotic_arm_concept": "precision_light_a",
    }
    selected_operations = {}
    selected_bounds = {}
    for pack_id, variant_id in selected_variants.items():
        plan = plans_by_pack[pack_id]
        result = build_blockout(
            plan,
            plan.directions[0].direction_id,
            variant_id=variant_id,
            presentation_profile="showcase",
        )
        selected_operations[pack_id] = {
            item["args"]["part_role"]: item
            for item in result.shape_program["operations"]
            if item["op"] not in {"profile", "bevel_approx"}
        }
        selected_bounds[pack_id] = _glb_bounds_by_role(result.glb_bytes)

    prop_ops = selected_operations["pack_future_weapon_prop"]
    assert prop_ops["prop_core"]["op"] == prop_ops["prop_grip"]["op"] == "capsule"
    assert prop_ops["visual_guard_prop_mount_collar"]["op"] == "cylinder"
    _assert_aabb_overlap_and_exposure(
        selected_bounds["pack_future_weapon_prop"],
        "visual_guard_prop_mount_collar",
        ("prop_core", "prop_grip"),
    )
    vehicle_ops = selected_operations["pack_vehicle_concept"]
    assert vehicle_ops["vehicle_cabin"]["args"]["size"][1] == 260
    assert sum(role.startswith("vehicle_hub_") for role in vehicle_ops) == 4
    assert {vehicle_ops[role]["args"]["material_id"] for role in vehicle_ops if role.startswith("vehicle_hub_")} == {"mat_aluminum"}
    for side, wheels in (
        ("left", ("vehicle_wheel_fl", "vehicle_wheel_rl")),
        ("right", ("vehicle_wheel_fr", "vehicle_wheel_rr")),
    ):
        bridge_role = f"visual_guard_vehicle_side_bridge_{side}"
        assert vehicle_ops[bridge_role]["op"] == "box"
        _assert_aabb_overlap_and_exposure(
            selected_bounds["pack_vehicle_concept"],
            bridge_role,
            ("vehicle_chassis", *wheels),
        )
    aircraft_ops = selected_operations["pack_aircraft_concept"]
    assert aircraft_ops["airframe_core"]["op"] == "capsule"
    assert {aircraft_ops[role]["op"] for role in aircraft_ops if role.startswith("lift_wing_")} == {"wedge"}
    assert {aircraft_ops[role]["args"]["height"] for role in aircraft_ops if role.startswith("lift_rotor_")} == {54}
    assert sum(role.startswith("lift_hub_") for role in aircraft_ops) == 4
    for position in ("front_left", "front_right", "rear_left", "rear_right"):
        bridge_role = f"visual_guard_aircraft_rotor_pylon_{position}"
        wing_role = f"lift_wing_{'left' if position.endswith('left') else 'right'}"
        rotor_role = f"lift_rotor_{position}"
        assert aircraft_ops[bridge_role]["op"] == "wedge"
        _assert_aabb_overlap_and_exposure(
            selected_bounds["pack_aircraft_concept"],
            bridge_role,
            (wing_role, rotor_role),
        )
        pylon_min, pylon_max = selected_bounds["pack_aircraft_concept"][bridge_role]
        wing_min, wing_max = selected_bounds["pack_aircraft_concept"][wing_role]
        wing_z_overlap = min(pylon_max[2], wing_max[2]) - max(pylon_min[2], wing_min[2])
        assert wing_z_overlap >= 0.07, (bridge_role, wing_role, wing_z_overlap)
    robot_ops = selected_operations["pack_robotic_arm_concept"]
    assert robot_ops["precision_link_1"]["op"] == robot_ops["precision_link_2"]["op"] == "capsule"
    assert robot_ops["visual_guard_robot_shoulder_bridge"]["op"] == "box"
    _assert_aabb_overlap_and_exposure(
        selected_bounds["pack_robotic_arm_concept"],
        "visual_guard_robot_shoulder_bridge",
        ("precision_joint_1", "precision_link_1"),
    )

    print("G818 visual detail grammar smoke passed: four role-whitelisted detail layouts and eight independent GLB PBR materials stay same-source")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
