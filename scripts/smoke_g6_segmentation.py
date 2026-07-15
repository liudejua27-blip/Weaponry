#!/usr/bin/env python3
"""Smoke deterministic segmentation candidates for all mechanical concept packs."""

from __future__ import annotations

from forgecad_agent.application.domain_packs import domain_pack_for_message
from forgecad_agent.application.geometry_worker import segment_blockout
from forgecad_agent.application.mechanical_planner import DeterministicMechanicalPlanner


BRIEFS = (
    "设计一个未来概念道具",
    "设计一辆冰原探索车",
    "设计一架垂直起降飞行器",
    "设计一台三关节机械臂",
)


def main() -> int:
    planner = DeterministicMechanicalPlanner()
    count = 0
    for brief in BRIEFS:
        pack = domain_pack_for_message(brief)
        plan = planner.plan_complete_concept(
            brief=brief,
            pack=pack,
            project_id="prj_g6_segmentation_smoke",
        )
        for direction in plan.directions:
            first = segment_blockout(plan, direction.direction_id)
            repeat = segment_blockout(plan, direction.direction_id)
            assert first == repeat
            assert len(first) >= 3
            assert first[0]["parent_part_id"] is None
            if plan.domain_pack_id == "pack_robotic_arm_concept":
                assert [item["parent_part_id"] for item in first[1:]] == [item["part_id"] for item in first[:-1]]
            else:
                assert all(item["parent_part_id"] == first[0]["part_id"] for item in first[1:])
            assert all(item["material_zone_ids"] for item in first)
            declared_parts = [item for item in first if item["editable_parameter_bindings"]]
            assert declared_parts, f"{plan.domain_pack_id} must expose at least one bounded editable part"
            for part in declared_parts:
                bindings = part["editable_parameter_bindings"]
                assert [item["path"] for item in bindings] == [
                    "transform.scale.x",
                    "transform.scale.y",
                    "transform.scale.z",
                ]
                assert part["editable_parameters"][:3] == [item["path"] for item in bindings]
            count += 1
    assert count == 12
    print(f"G6 segmentation smoke passed: {count} deterministic candidate graphs")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
