#!/usr/bin/env python3
"""Gate the bounded showcase-only visual detail grammar and material mapping."""

from __future__ import annotations

import json
import struct

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


def main() -> int:
    planner = DeterministicMechanicalPlanner()
    for brief in BRIEFS:
        pack = domain_pack_for_message(brief)
        plan = planner.plan_complete_concept(brief=brief, pack=pack, project_id="prj_g818_smoke")
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
        assert len(document["materials"]) == 7
        material_indices = {primitive["material"] for primitive in document["meshes"][0]["primitives"]}
        assert {0, 1, 3, 6}.issubset(material_indices)
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

    print("G818 visual detail grammar smoke passed: seven bounded visual categories and seven GLB PBR materials across four concept domains")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
