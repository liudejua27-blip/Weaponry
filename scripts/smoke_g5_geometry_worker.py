#!/usr/bin/env python3
"""Smoke the lightweight ShapeProgram -> GLB blockout worker for all four packs."""

from __future__ import annotations

import struct

from forgecad_agent.application.domain_packs import domain_pack_for_message
from forgecad_agent.application.geometry_worker import build_blockout
from forgecad_agent.application.mechanical_planner import DeterministicMechanicalPlanner


BRIEFS = (
    "设计一个未来概念道具",
    "设计一辆冰原探索车",
    "设计一架垂直起降飞行器",
    "设计一台三关节机械臂",
)


def main() -> int:
    planner = DeterministicMechanicalPlanner()
    results = []
    for brief in BRIEFS:
        pack = domain_pack_for_message(brief)
        plan = planner.plan_complete_concept(brief=brief, pack=pack, project_id="prj_g5_smoke")
        assert len(plan.directions) == 1, f"{pack.pack_id} must produce one V003 synthesis direction"
        for direction in plan.directions:
            result = build_blockout(plan, direction.direction_id)
            assert result.glb_bytes[:4] == b"glTF"
            assert struct.unpack_from("<I", result.glb_bytes, 4)[0] == 2
            assert result.triangle_count > 0
            operations = result.shape_program["operations"]
            if pack.pack_id in {"pack_vehicle_concept", "pack_aircraft_concept", "pack_robotic_arm_concept"}:
                assert any(item.get("op") == "cylinder" for item in operations), f"{pack.pack_id} should exercise cylinder geometry"
            assert len(result.assembly_graph["parts"]) >= 3
            assert result.assembly_graph["root_part_id"] in {part["part_id"] for part in result.assembly_graph["parts"]}
            assert len(result.assembly_graph["connections"]) == len(result.assembly_graph["parts"]) - 1
            if pack.pack_id == "pack_robotic_arm_concept":
                assert sum(len(part["joints"]) for part in result.assembly_graph["parts"]) == len(result.assembly_graph["parts"]) - 1
            repeat = build_blockout(plan, direction.direction_id)
            assert result.topology_hash == repeat.topology_hash
            assert result.glb_bytes == repeat.glb_bytes
            results.append((pack.pack_id, direction.direction_id, result.triangle_count, len(result.glb_bytes)))
    assert len(results) == len(BRIEFS)
    print(f"G5 geometry worker smoke passed: {len(results)} blockouts, deterministic GLB, AssemblyGraph summaries")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
