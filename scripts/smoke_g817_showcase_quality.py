#!/usr/bin/env python3
"""Gate the bounded quick-sketch/showcase presentation profiles."""

from __future__ import annotations

import json
import tempfile
from pathlib import Path

from pydantic import ValidationError

from forgecad_agent.application.agent_kernel import AgentKernelService
from forgecad_agent.application.agent_models import BuildAgentBlockoutRequest, SegmentAgentBlockoutRequest
from forgecad_agent.application.domain_packs import domain_pack_for_message
from forgecad_agent.application.geometry_worker import build_blockout, read_shape_program_glb, segment_blockout
from forgecad_agent.application.mechanical_planner import DeterministicMechanicalPlanner
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteMigrationRunner, SQLiteUnitOfWork


ROOT = Path(__file__).resolve().parents[1]
BRIEFS = (
    "设计一个未来概念道具，细节丰富",
    "设计一辆未来探索车，细节丰富",
    "设计一架概念飞机，细节丰富",
    "设计一台机械臂，细节丰富",
)


def _plan(brief: str):
    return DeterministicMechanicalPlanner().plan_complete_concept(
        brief=brief,
        pack=domain_pack_for_message(brief),
        project_id="prj_g817_smoke",
    )


def main() -> int:
    for brief in BRIEFS:
        plan = _plan(brief)
        direction_id = plan.directions[0].direction_id
        quick = build_blockout(plan, direction_id, presentation_profile="quick_sketch")
        showcase = build_blockout(plan, direction_id, presentation_profile="showcase")
        repeated = build_blockout(plan, direction_id, presentation_profile="showcase")
        assert quick.presentation_profile == "quick_sketch"
        assert showcase.presentation_profile == "showcase"
        assert showcase.topology_hash == repeated.topology_hash and showcase.glb_bytes == repeated.glb_bytes
        assert showcase.topology_hash != quick.topology_hash
        assert showcase.triangle_count > quick.triangle_count
        assert read_shape_program_glb(showcase.glb_bytes) == (showcase.triangle_count, showcase.bounds_mm)
        showcase_roles = [item["args"]["part_role"] for item in showcase.shape_program["operations"]]
        assert any(role.startswith("visual_panel_") for role in showcase_roles)
        assert any(role.startswith("visual_groove_") for role in showcase_roles)
        quick_roles = [item["args"]["part_role"] for item in quick.shape_program["operations"]]
        assert not any(role.startswith("visual_") for role in quick_roles)
        parts = segment_blockout(plan, direction_id, presentation_profile="showcase")
        assert [part["part_id"] for part in showcase.assembly_graph["parts"]] == [part["part_id"] for part in parts]
        for part in parts:
            if part["role"].startswith("visual_"):
                assert part["editable_parameter_bindings"] == []

    with tempfile.TemporaryDirectory(prefix="forgecad-g817-quality-") as raw:
        factory = SQLiteConnectionFactory(Path(raw) / "library.db")
        SQLiteMigrationRunner(factory, ROOT / "migrations").run()
        kernel = AgentKernelService(factory)
        plan = _plan(BRIEFS[0])
        direction_id = plan.directions[0].direction_id
        build_request = BuildAgentBlockoutRequest(
            client_request_id="g817-showcase-build",
            plan=plan,
            direction_id=direction_id,
            presentation_profile="showcase",
        )
        built = kernel.build_blockout(build_request, "g817-showcase-build")
        segmented = kernel.segment_blockout(
            SegmentAgentBlockoutRequest(
                client_request_id="g817-showcase-segment",
                plan=plan,
                direction_id=direction_id,
                variant_id=built.variant_id,
                variation_index=built.variation_index,
                presentation_profile="showcase",
                artifact_id=built.artifact_id,
            ),
            "g817-showcase-segment",
        )
        assert built.presentation_profile == segmented.presentation_profile == "showcase"
        assert built.assembly_graph == segmented.assembly_graph
        with SQLiteUnitOfWork(factory) as unit:
            candidate = unit.agent_assets.get_candidate(segmented.artifact_id)
            assert candidate is not None
            assert json.loads(candidate["candidate_json"])["presentation_profile"] == "showcase"
        try:
            BuildAgentBlockoutRequest(
                client_request_id="g817-invalid-profile",
                plan=plan,
                direction_id=direction_id,
                presentation_profile="unbounded",  # type: ignore[arg-type]
            )
        except ValidationError:
            pass
        else:
            raise AssertionError("unknown presentation profile must be rejected by the contract")

    print("G817 showcase quality smoke passed: four domains keep preview, segmentation and candidate geometry profile-consistent")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
