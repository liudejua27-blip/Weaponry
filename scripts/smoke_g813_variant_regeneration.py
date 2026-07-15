#!/usr/bin/env python3
"""Gate the bounded, preview-only appearance rotation for direction blockouts."""

from __future__ import annotations

import json
import tempfile
from pathlib import Path

from pydantic import ValidationError

from forgecad_agent.application.agent_kernel import AgentKernelIdempotencyConflict, AgentKernelService
from forgecad_agent.application.agent_models import BuildAgentBlockoutRequest, SegmentAgentBlockoutRequest
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
    return DeterministicMechanicalPlanner().plan_complete_concept(
        brief=brief,
        pack=domain_pack_for_message(brief),
        project_id="prj_agent_asset_smoke",
    )


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad-g813-rotation-") as raw:
        factory = SQLiteConnectionFactory(Path(raw) / "library.db")
        SQLiteMigrationRunner(factory, ROOT / "migrations").run()
        _seed_project(factory)
        kernel = AgentKernelService(factory)

        for brief_index, brief in enumerate(BRIEFS, start=1):
            plan = _plan(brief)
            allowed = set(list_blockout_variants(plan.domain_pack_id))
            for direction_index, direction in enumerate(plan.directions, start=1):
                variants = []
                for variation_index in range(3):
                    key = f"g813-build-{brief_index}-{direction_index}-{variation_index}"
                    built = kernel.build_blockout(
                        BuildAgentBlockoutRequest(
                            client_request_id=key,
                            plan=plan,
                            direction_id=direction.direction_id,
                            variation_index=variation_index,
                        ),
                        key,
                    )
                    assert built.variation_index == variation_index
                    assert built.variant_id in allowed
                    variants.append(built.variant_id)
                    segmented = kernel.segment_blockout(
                        SegmentAgentBlockoutRequest(
                            client_request_id=f"g813-segment-{brief_index}-{direction_index}-{variation_index}",
                            plan=plan,
                            direction_id=direction.direction_id,
                            variant_id=built.variant_id,
                            variation_index=built.variation_index,
                            artifact_id=built.artifact_id,
                        ),
                        f"g813-segment-{brief_index}-{direction_index}-{variation_index}",
                    )
                    assert segmented.variant_id == built.variant_id
                    assert segmented.variation_index == variation_index
                    assert segmented.assembly_graph == built.assembly_graph
                    with SQLiteUnitOfWork(factory) as unit:
                        candidate = unit.agent_assets.get_candidate(segmented.artifact_id)
                        assert candidate is not None
                        payload = json.loads(candidate["candidate_json"])
                        assert payload["variant_id"] == built.variant_id
                        assert payload["variation_index"] == variation_index
                assert len(set(variants)) == 3, "a direction must rotate through three distinct visual variants"

        plan = _plan(BRIEFS[0])
        direction_id = plan.directions[0].direction_id
        first = kernel.build_blockout(
            BuildAgentBlockoutRequest(
                client_request_id="g813-idempotency-first",
                plan=plan,
                direction_id=direction_id,
                variation_index=0,
            ),
            "g813-idempotency",
        )
        assert kernel.build_blockout(
            BuildAgentBlockoutRequest(
                client_request_id="g813-idempotency-first",
                plan=plan,
                direction_id=direction_id,
                variation_index=0,
            ),
            "g813-idempotency",
        ) == first
        try:
            kernel.build_blockout(
                BuildAgentBlockoutRequest(
                    client_request_id="g813-idempotency-conflict",
                    plan=plan,
                    direction_id=direction_id,
                    variation_index=1,
                ),
                "g813-idempotency",
            )
        except AgentKernelIdempotencyConflict:
            pass
        else:
            raise AssertionError("idempotency key cannot select a different preview variation")

        try:
            BuildAgentBlockoutRequest(
                client_request_id="g813-invalid-index",
                plan=plan,
                direction_id=direction_id,
                variation_index=3,
            )
        except ValidationError:
            pass
        else:
            raise AssertionError("variation index outside 0..2 must be rejected by the contract")

    print("G813 variant regeneration smoke passed: four packs rotate three preview-only visual variants consistently")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
