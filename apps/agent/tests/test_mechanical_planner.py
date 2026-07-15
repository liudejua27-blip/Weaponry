from forgecad_agent.application.domain_packs import domain_pack_by_id
from forgecad_agent.application.mechanical_planner import DeterministicMechanicalPlanner


def test_deterministic_planner_emits_three_complete_directions_for_each_pack():
    planner = DeterministicMechanicalPlanner()
    for pack_id in (
        "pack_future_weapon_prop",
        "pack_vehicle_concept",
        "pack_aircraft_concept",
        "pack_robotic_arm_concept",
    ):
        plan = planner.plan_complete_concept(
            brief="用于游戏美术的非功能性概念模型",
            pack=domain_pack_by_id(pack_id),
            project_id="prj_unit_test",
        )
        assert plan.domain_pack_id == pack_id
        assert len(plan.directions) == 3
        assert all(direction.primary_part_roles for direction in plan.directions)
        assert plan.shape_program_ready is False
