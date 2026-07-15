#!/usr/bin/env python3
"""Gate the bounded visual-variant path from Agent direction to saved candidate."""

from __future__ import annotations

import json
import tempfile
from pathlib import Path

from forgecad_agent.application.agent_asset_editing import AgentAssetEditingService
from forgecad_agent.application.agent_kernel import (
    AgentKernelError,
    AgentKernelIdempotencyConflict,
    AgentKernelService,
)
from forgecad_agent.application.agent_models import (
    BuildAgentBlockoutRequest,
    CommitAgentBlockoutRequest,
    SegmentAgentBlockoutRequest,
)
from forgecad_agent.application.domain_packs import domain_pack_for_message
from forgecad_agent.application.geometry_worker import list_blockout_variants
from forgecad_agent.application.mechanical_planner import DeterministicMechanicalPlanner
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteMigrationRunner, SQLiteUnitOfWork
from smoke_g6_asset_editing import _seed_project


ROOT = Path(__file__).resolve().parents[1]
BRIEFS = (
    "设计一个未来概念道具",
    "设计一辆冰原探索车",
    "设计一架垂直起降飞行器",
    "设计一台三关节机械臂",
)


def _plan(brief: str):
    pack = domain_pack_for_message(brief)
    return DeterministicMechanicalPlanner().plan_complete_concept(
        brief=brief,
        pack=pack,
        project_id="prj_agent_asset_smoke",
    )


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad-g812-variants-") as raw:
        factory = SQLiteConnectionFactory(Path(raw) / "library.db")
        SQLiteMigrationRunner(factory, ROOT / "migrations").run()
        _seed_project(factory)
        kernel = AgentKernelService(factory)
        assets = AgentAssetEditingService(factory)
        committed = None

        for brief_index, brief in enumerate(BRIEFS, start=1):
            plan = _plan(brief)
            allowed = set(list_blockout_variants(plan.domain_pack_id))
            for direction_index, direction in enumerate(plan.directions, start=1):
                key = f"g812-build-{brief_index}-{direction_index}"
                built = kernel.build_blockout(
                    BuildAgentBlockoutRequest(
                        client_request_id=key,
                        plan=plan,
                        direction_id=direction.direction_id,
                    ),
                    key,
                )
                assert built.variant_id in allowed
                assert built.shape_program["program_id"].endswith(f"_{built.variant_id}")
                assert built.assembly_graph["graph_id"].endswith(f"_{built.variant_id}")
                replay = kernel.build_blockout(
                    BuildAgentBlockoutRequest(client_request_id=key, plan=plan, direction_id=direction.direction_id),
                    key,
                )
                assert replay == built
                segment_key = f"g812-segment-{brief_index}-{direction_index}"
                segmented = kernel.segment_blockout(
                    SegmentAgentBlockoutRequest(
                        client_request_id=segment_key,
                        plan=plan,
                        direction_id=direction.direction_id,
                        variant_id=built.variant_id,
                        artifact_id=built.artifact_id,
                    ),
                    segment_key,
                )
                assert segmented.variant_id == built.variant_id
                assert segmented.assembly_graph == built.assembly_graph
                assert len(segmented.parts) == len(built.assembly_graph["parts"])
                with SQLiteUnitOfWork(factory) as unit:
                    candidate = unit.agent_assets.get_candidate(segmented.artifact_id)
                    assert candidate is not None
                    candidate_payload = json.loads(candidate["candidate_json"])
                    assert candidate_payload["variant_id"] == built.variant_id
                    assert json.loads(candidate["shape_program_json"])["program_id"] == built.shape_program["program_id"]
                committed = committed or (segmented, built)

        assert committed is not None
        segmented, built = committed
        version = assets.commit_blockout(
            CommitAgentBlockoutRequest(
                client_request_id="g812-commit",
                artifact_id=segmented.artifact_id,
                project_id="prj_agent_asset_smoke",
            ),
            "g812-commit",
        )
        assert version.shape_program["program_id"] == built.shape_program["program_id"]
        assert version.assembly_graph["graph_id"] == built.assembly_graph["graph_id"]

        plan = _plan(BRIEFS[0])
        wrong_variant = list_blockout_variants("pack_vehicle_concept")[0]
        try:
            kernel.build_blockout(
                BuildAgentBlockoutRequest(
                    client_request_id="g812-wrong-pack",
                    plan=plan,
                    direction_id=plan.directions[0].direction_id,
                    variant_id=wrong_variant,
                ),
                "g812-wrong-pack",
            )
        except AgentKernelError as exc:
            assert exc.code == "BLOCKOUT_INVALID"
        else:
            raise AssertionError("cross-pack variant must be rejected")

        first_variant, second_variant = list_blockout_variants(plan.domain_pack_id)[:2]
        kernel.build_blockout(
            BuildAgentBlockoutRequest(
                client_request_id="g812-idempotency-first",
                plan=plan,
                direction_id=plan.directions[0].direction_id,
                variant_id=first_variant,
            ),
            "g812-idempotency",
        )
        try:
            kernel.build_blockout(
                BuildAgentBlockoutRequest(
                    client_request_id="g812-idempotency-second",
                    plan=plan,
                    direction_id=plan.directions[0].direction_id,
                    variant_id=second_variant,
                ),
                "g812-idempotency",
            )
        except AgentKernelIdempotencyConflict:
            pass
        else:
            raise AssertionError("idempotency key cannot select a different visual variant")

    print("G812 direction variant smoke passed: four packs, build/segment/candidate consistency and rejection boundaries")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
