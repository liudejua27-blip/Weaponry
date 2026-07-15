#!/usr/bin/env python3
"""G810 smoke: generated four-domain parts declare only concrete scale bindings."""

from __future__ import annotations

from collections import Counter
from typing import Any

from forgecad_agent.application.domain_packs import domain_pack_for_message
from forgecad_agent.application.geometry_worker import build_blockout, segment_blockout
from forgecad_agent.application.mechanical_planner import DeterministicMechanicalPlanner


BRIEFS = (
    "设计一个未来概念道具",
    "设计一辆冰原探索车",
    "设计一架垂直起降飞行器",
    "设计一台三关节机械臂",
)
EXPECTED_PATHS = ["transform.scale.x", "transform.scale.y", "transform.scale.z"]


def _shape_operations_by_role(program: dict[str, Any]) -> dict[str, list[dict[str, Any]]]:
    result: dict[str, list[dict[str, Any]]] = {}
    for operation in program["operations"]:
        args = operation.get("args", {})
        role = args.get("part_role") if isinstance(args, dict) else None
        if isinstance(role, str):
            result.setdefault(role, []).append(operation)
    return result


def main() -> int:
    planner = DeterministicMechanicalPlanner()
    total_declared_parts = 0
    for brief in BRIEFS:
        pack = domain_pack_for_message(brief)
        plan = planner.plan_complete_concept(brief=brief, pack=pack, project_id="prj_g810_generated_bindings")
        direction_id = plan.directions[0].direction_id
        first = segment_blockout(plan, direction_id)
        assert first == segment_blockout(plan, direction_id), "generated declarations must be deterministic"
        result = build_blockout(plan, direction_id)
        role_counts = Counter(str(item["role"]) for item in first)
        operations_by_role = _shape_operations_by_role(result.shape_program)
        declared = [item for item in first if item["editable_parameter_bindings"]]
        assert declared, f"{pack.pack_id} must include at least one concrete bounded part"
        total_declared_parts += len(declared)

        for part in first:
            bindings = part["editable_parameter_bindings"]
            operations = operations_by_role[str(part["role"])]
            if bindings:
                # A declaration is safe only when this Part maps to exactly one
                # size-based output.  It must never silently edit paired parts.
                assert role_counts[str(part["role"])] == 1
                assert len(operations) == 1
                operation = operations[0]
                assert operation["op"] in {"box", "wedge"}
                assert all(
                    abs(float(left) - float(right)) < 1e-6
                    for left, right in zip(operation["args"]["size"], part["size_mm"])
                )
                assert [binding["path"] for binding in bindings] == EXPECTED_PATHS
                assert part["editable_parameters"][:3] == EXPECTED_PATHS
                assert all(binding["schema_version"] == "EditableParameterBinding@1" for binding in bindings)
                assert all(binding["unit"] == "ratio" for binding in bindings)
                assert all(binding["default"] == 1.0 and binding["min"] == 0.6 and binding["max"] == 1.4 and binding["step"] == 0.1 for binding in bindings)
                assert len({binding["parameter_id"] for binding in bindings}) == 3
            else:
                # Repeated roles and non-size primitives intentionally keep no
                # false per-part controls under the current ChangeSet adapter.
                assert role_counts[str(part["role"])] > 1 or operations[0]["op"] not in {"box", "wedge"}

    assert total_declared_parts >= 4
    print("G810 generated parameter binding smoke passed: four-domain deterministic parts, concrete ShapeProgram mapping and bounded declarations")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
