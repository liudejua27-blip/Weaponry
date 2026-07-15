#!/usr/bin/env python3
"""C102 smoke: explainable Agent component replacement eligibility."""

from __future__ import annotations

import asyncio
import json
import tempfile
from pathlib import Path

from forgecad_agent.application.agent_asset_editing import AgentAssetEditingService, AgentAssetError
from forgecad_agent.application.agent_kernel import AgentKernelService
from forgecad_agent.application.agent_models import (
    AgentPartEditOperation,
    BuildAgentBlockoutRequest,
    CommitAgentBlockoutRequest,
    CreateAgentThreadRequest,
    ProposeAgentAssetChangeSetRequest,
    SaveAgentComponentRequest,
    SegmentAgentBlockoutRequest,
    StartAgentTurnRequest,
)
from forgecad_agent.application.mechanical_planner import MechanicalConceptPlan
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteMigrationRunner, SQLiteUnitOfWork
from fastapi import FastAPI
from forgecad_agent.api.agent_asset_routes import build_agent_asset_router
from smoke_g6_asset_editing import _seed_project


ROOT = Path(__file__).resolve().parents[1]


async def _get_json(app: FastAPI, path: str, query: str) -> tuple[int, object]:
    messages = [{"type": "http.request", "body": b"", "more_body": False}]
    response: list[dict[str, object]] = []

    async def receive() -> dict[str, object]:
        return messages.pop(0) if messages else {"type": "http.disconnect"}

    async def send(message: dict[str, object]) -> None:
        response.append(message)

    await app({
        "type": "http",
        "asgi": {"version": "3.0", "spec_version": "2.3"},
        "http_version": "1.1",
        "method": "GET",
        "scheme": "http",
        "path": path,
        "raw_path": path.encode("ascii"),
        "query_string": query.encode("ascii"),
        "headers": [(b"host", b"testserver")],
        "client": ("127.0.0.1", 12345),
        "server": ("testserver", 80),
    }, receive, send)
    start = next(message for message in response if message["type"] == "http.response.start")
    body = b"".join(message.get("body", b"") for message in response if message["type"] == "http.response.body")
    return int(start["status"]), json.loads(body.decode("utf-8"))


def _proposal(version_id: str, part_id: str, component_id: str) -> ProposeAgentAssetChangeSetRequest:
    return ProposeAgentAssetChangeSetRequest(
        client_request_id=f"c102-propose-{component_id}",
        summary="C102 组件替换验证",
        operations=[AgentPartEditOperation(
            operation_id="op_c102_replace",
            op="replace_part",
            part_id=part_id,
            replacement_component_id=component_id,
        )],
    )


def _assert_rejected(assets: AgentAssetEditingService, version_id: str, part_id: str, component_id: str) -> None:
    try:
        assets.propose_change_set(version_id, _proposal(version_id, part_id, component_id), f"c102-reject-{component_id}")
    except AgentAssetError as exc:
        assert exc.code == "COMPONENT_QUALITY_NOT_READY", exc.code
    else:
        raise AssertionError("a component without a usable source quality result must be rejected")


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad-c102-components-") as raw:
        factory = SQLiteConnectionFactory(Path(raw) / "library.db")
        applied = SQLiteMigrationRunner(factory, ROOT / "migrations").run()
        assert "0025" in applied, applied
        _seed_project(factory)
        kernel = AgentKernelService(factory)
        assets = AgentAssetEditingService(factory)
        thread = kernel.create_thread(CreateAgentThreadRequest(client_request_id="c102-thread", project_id="prj_agent_asset_smoke", title="C102 组件兼容"), "c102-thread")
        turn = kernel.start_turn(thread.thread_id, StartAgentTurnRequest(client_request_id="c102-turn", message="设计一台三关节机械臂"), "c102-turn")
        plan = MechanicalConceptPlan.model_validate(
            next(item.payload["result"] for item in turn.items if item.item_type == "tool_result" and "result" in item.payload)
        )
        direction_id = plan.directions[0].direction_id
        built = kernel.build_blockout(BuildAgentBlockoutRequest(client_request_id="c102-build", plan=plan, direction_id=direction_id), "c102-build")
        segmented = kernel.segment_blockout(SegmentAgentBlockoutRequest(client_request_id="c102-segment", plan=plan, direction_id=direction_id, artifact_id=built.artifact_id), "c102-segment")
        version = assets.commit_blockout(CommitAgentBlockoutRequest(client_request_id="c102-commit", artifact_id=segmented.artifact_id), "c102-commit")
        target = next(part for part in version.parts if part.editable_parameter_bindings)
        component = assets.save_component(
            version.asset_version_id,
            SaveAgentComponentRequest(client_request_id="c102-save", part_id=target.part_id, display_name="C102 上臂替换件"),
            "c102-save",
        )

        unavailable = assets.list_component_candidates(version.asset_version_id, part_id=target.part_id)
        assert len(unavailable) == 1
        assert unavailable[0].compatibility.eligible is False
        assert "source_quality_unavailable" in unavailable[0].compatibility.reason_codes
        _assert_rejected(assets, version.asset_version_id, target.part_id, component.component_id)

        passed_report = assets.quality(version.asset_version_id)
        assert passed_report.status == "passed"
        passed = assets.list_component_candidates(version.asset_version_id, part_id=target.part_id)
        assert passed[0].compatibility.eligible is True
        assert passed[0].compatibility.source_quality_status == "passed"
        assert {"same_domain_pack", "same_role", "target_connectors_preserved"} <= set(passed[0].compatibility.reason_codes)

        # A newest failed report must immediately revoke eligibility; no component
        # copy stores its own quality field.
        failed = passed_report.model_copy(update={"status": "failed", "findings": []})
        with SQLiteUnitOfWork(factory) as unit:
            unit.connection.execute(
                """
                UPDATE agent_asset_quality_reports
                SET status = ?, report_json = ?
                WHERE quality_report_id = (
                  SELECT quality_report_id
                  FROM agent_asset_quality_reports
                  WHERE asset_version_id = ?
                  ORDER BY created_at DESC, quality_report_id DESC
                  LIMIT 1
                )
                """,
                (failed.status, failed.model_dump_json(), version.asset_version_id),
            )
        failed_candidates = assets.list_component_candidates(version.asset_version_id, part_id=target.part_id)
        assert failed_candidates[0].compatibility.eligible is False
        assert "source_quality_failed" in failed_candidates[0].compatibility.reason_codes
        _assert_rejected(assets, version.asset_version_id, target.part_id, component.component_id)

        # A new real quality run restores only the source quality fact; then the
        # preview-first ChangeSet path remains the sole way to replace geometry.
        assert assets.quality(version.asset_version_id).status == "passed"
        eligible = assets.list_component_candidates(version.asset_version_id, part_id=target.part_id)[0]
        assert eligible.compatibility.eligible is True
        app = FastAPI()
        app.include_router(build_agent_asset_router(assets))
        status, payload = asyncio.run(_get_json(
            app,
            f"/api/v1/agent/asset-versions/{version.asset_version_id}/components:compatible",
            f"part_id={target.part_id}",
        ))
        assert status == 200 and isinstance(payload, list) and payload[0]["compatibility"]["eligible"] is True
        change_set = assets.propose_change_set(
            version.asset_version_id,
            _proposal(version.asset_version_id, target.part_id, component.component_id),
            "c102-eligible-propose",
        )
        assert change_set.status == "proposed"

        # Disabled, wrong-domain and wrong-role records are all ineligible without
        # being promoted to a preview candidate.
        with SQLiteUnitOfWork(factory) as unit:
            unit.connection.execute("UPDATE agent_components SET status = 'disabled' WHERE component_id = ?", (component.component_id,))
        assert assets.list_component_candidates(version.asset_version_id, part_id=target.part_id)[0].compatibility.eligible is False
        with SQLiteUnitOfWork(factory) as unit:
            unit.connection.execute("UPDATE agent_components SET status = 'active', domain_pack_id = 'pack_vehicle_concept' WHERE component_id = ?", (component.component_id,))
        assert "domain_pack_mismatch" in assets.list_component_candidates(version.asset_version_id, part_id=target.part_id)[0].compatibility.reason_codes
        with SQLiteUnitOfWork(factory) as unit:
            unit.connection.execute("UPDATE agent_components SET domain_pack_id = ?, role = 'role_c102_mismatch' WHERE component_id = ?", (version.domain_pack_id, component.component_id))
        assert "role_mismatch" in assets.list_component_candidates(version.asset_version_id, part_id=target.part_id)[0].compatibility.reason_codes
    print("C102 component compatibility smoke passed: quality, domain, role, activation, connector-preservation and ChangeSet boundary")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
