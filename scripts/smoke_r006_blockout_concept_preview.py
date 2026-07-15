#!/usr/bin/env python3
"""R006 smoke: ephemeral, same-source software concept images for all packs."""

from __future__ import annotations

import base64
import json
import tempfile
from pathlib import Path

from jsonschema import Draft202012Validator

from forgecad_agent.application.agent_kernel import AgentKernelService
from forgecad_agent.application.agent_models import (
    BuildAgentBlockoutRequest,
    RenderAgentBlockoutConceptPreviewRequest,
)
from forgecad_agent.application.domain_packs import domain_pack_for_message
from forgecad_agent.application.mechanical_planner import DeterministicMechanicalPlanner
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteMigrationRunner


ROOT = Path(__file__).resolve().parents[1]
SCHEMA = ROOT / "packages" / "concept-spec" / "schemas" / "blockout-concept-preview.schema.json"
BRIEFS = (
    "设计一个非功能性的未来概念道具，用于游戏展示。",
    "设计一辆紧凑的未来探索汽车，完整外观展示。",
    "设计一架紧凑的未来概念飞机，完整外观展示。",
    "设计一台三关节维护机械臂概念模型。",
)
WRITE_TABLES = (
    "idempotency_records",
    "agent_blockout_candidates",
    "agent_asset_versions",
    "agent_asset_heads",
    "active_design_snapshots",
    "agent_asset_quality_reports",
    "export_packages_v2",
)


def _count(factory: SQLiteConnectionFactory, table: str) -> int:
    with factory.connect() as connection:
        return int(connection.execute(f"SELECT COUNT(*) FROM {table}").fetchone()[0])


def _plan(brief: str):
    return DeterministicMechanicalPlanner().plan_complete_concept(
        brief=brief,
        pack=domain_pack_for_message(brief),
        project_id="prj_r006_ephemeral",
    )


def main() -> int:
    validator = Draft202012Validator(json.loads(SCHEMA.read_text(encoding="utf-8")))
    with tempfile.TemporaryDirectory(prefix="forgecad-r006-preview-") as raw:
        factory = SQLiteConnectionFactory(Path(raw) / "library.db")
        SQLiteMigrationRunner(factory, ROOT / "migrations").run()
        service = AgentKernelService(factory)
        before = {table: _count(factory, table) for table in WRITE_TABLES}

        for index, brief in enumerate(BRIEFS, start=1):
            plan = _plan(brief)
            direction = plan.directions[0]
            request = RenderAgentBlockoutConceptPreviewRequest(
                client_request_id=f"r006-preview-{index}",
                plan=plan,
                direction_id=direction.direction_id,
            )
            preview = service.render_blockout_concept_preview(request)
            replay = service.render_blockout_concept_preview(request)
            validator.validate(preview.model_dump(mode="json"))
            assert preview == replay, "the same ephemeral context must render deterministically"
            assert preview.domain_pack_id == plan.domain_pack_id
            assert preview.direction_id == direction.direction_id
            assert preview.width == 320 and preview.height == 240
            assert base64.b64decode(preview.png_base64, validate=True).startswith(b"\x89PNG\r\n\x1a\n")

            built = service.build_blockout(
                BuildAgentBlockoutRequest(
                    client_request_id=f"r006-build-{index}",
                    plan=plan,
                    direction_id=direction.direction_id,
                ),
                f"r006-build-{index}",
            )
            assert preview.variant_id == built.variant_id
            assert preview.topology_hash == built.topology_hash

        # The comparison builds above intentionally add idempotency rows only;
        # capture the preview-only write boundary using a fresh service call.
        isolated_before = {table: _count(factory, table) for table in WRITE_TABLES}
        plan = _plan(BRIEFS[0])
        service.render_blockout_concept_preview(
            RenderAgentBlockoutConceptPreviewRequest(
                client_request_id="r006-no-write",
                plan=plan,
                direction_id=plan.directions[1].direction_id,
                variation_index=1,
            )
        )
        assert {table: _count(factory, table) for table in WRITE_TABLES} == isolated_before
        assert _count(factory, "agent_blockout_candidates") == before["agent_blockout_candidates"]
        assert _count(factory, "agent_asset_versions") == before["agent_asset_versions"]

    print("R006 blockout concept preview smoke passed: four packs, deterministic same-source PNGs, no preview writes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
