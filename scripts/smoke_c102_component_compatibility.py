#!/usr/bin/env python3
"""C102 smoke: explainable Agent component replacement eligibility."""

from __future__ import annotations

import asyncio
import copy
import hashlib
import json
import tempfile
from pathlib import Path

from forgecad_agent.application import geometry_worker
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
from smoke_q003_compile_readback_quality import _legacy_v1_glb


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
        plan_result = next(item.payload["result"] for item in turn.items if item.item_type == "tool_result" and item.payload.get("tool_name") == "plan_complete_concept")
        plan = MechanicalConceptPlan.model_validate(plan_result["plan"])
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

        # A database row marked passed is not sufficient.  A real pre-v2
        # compile report must be reclassified by the same current-contract
        # boundary used by quality GET/replay before component eligibility.
        current_compiled = geometry_worker.compile_shape_program(version.shape_program)
        legacy_glb = _legacy_v1_glb(current_compiled.glb_bytes)
        legacy_facts = geometry_worker.read_shape_program_glb_facts(legacy_glb)
        assert hashlib.sha256(legacy_glb).hexdigest() != current_compiled.readback.glb_sha256
        assert passed_report.compile_readback is not None
        legacy_readback = passed_report.compile_readback.model_dump(mode="json")
        legacy_readback.update({
            "glb_sha256": hashlib.sha256(legacy_glb).hexdigest(),
            "glb_byte_size": len(legacy_glb),
            "visual_texture_sets": copy.deepcopy(legacy_facts.visual_texture_sets),
        })
        for texture_set in legacy_readback["visual_texture_sets"]:
            assert texture_set["visual_texture_set_id"].endswith("_builtin")
            assert not texture_set["visual_texture_set_id"].endswith("_builtin_v2")
            assert not texture_set["visual_texture_set_id"].endswith("_builtin_v3")
            texture_set.pop("texture_material_id")
        pre_v2_report = passed_report.model_dump(mode="json")
        pre_v2_report.update({
            "quality_report_id": "quality_zzzz_c102_pre_v2",
            "status": "passed",
            "triangle_count": legacy_facts.triangle_count,
            "bounds_mm": legacy_facts.bounds_mm,
            "compile_readback": legacy_readback,
            "findings": [],
        })
        with SQLiteUnitOfWork(factory) as unit:
            unit.agent_assets.add_quality_report(
                quality_report_id=pre_v2_report["quality_report_id"],
                project_id=version.project_id,
                asset_version_id=version.asset_version_id,
                report_json=json.dumps(pre_v2_report, ensure_ascii=False, sort_keys=True),
                status="passed",
                created_at=passed_report.checked_at,
            )
        stale_candidates = assets.list_component_candidates(version.asset_version_id, part_id=target.part_id)
        assert stale_candidates[0].compatibility.eligible is False
        assert stale_candidates[0].compatibility.source_quality_status == "unavailable"
        assert "source_quality_unavailable" in stale_candidates[0].compatibility.reason_codes
        _assert_rejected(assets, version.asset_version_id, target.part_id, component.component_id)

        # A newer current-contract report restores eligibility; the stale row
        # remains historical and is never silently promoted.
        assert assets.quality(version.asset_version_id).status == "passed"
        restored = assets.list_component_candidates(version.asset_version_id, part_id=target.part_id)
        assert restored[0].compatibility.eligible is True

        original_readback = geometry_worker.read_shape_program_glb_facts
        geometry_worker.read_shape_program_glb_facts = lambda _payload: (_ for _ in ()).throw(
            ValueError("c102 damaged GLB")
        )
        try:
            compile_failure = assets.quality(version.asset_version_id)
        finally:
            geometry_worker.read_shape_program_glb_facts = original_readback
        assert compile_failure.status == "unavailable"
        assert compile_failure.evidence_source == "compile_failure"
        failed_readback_candidates = assets.list_component_candidates(
            version.asset_version_id,
            part_id=target.part_id,
        )
        assert failed_readback_candidates[0].compatibility.eligible is False
        assert failed_readback_candidates[0].compatibility.source_quality_status == "unavailable"
        _assert_rejected(assets, version.asset_version_id, target.part_id, component.component_id)
        assert assets.quality(version.asset_version_id).status == "passed"

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

        # Confirmation is a fresh trust boundary.  The immutable preview may
        # remain valid geometry, but its replacement source can lose current
        # quality between preview and the permanent write.
        with SQLiteUnitOfWork(factory) as unit:
            unit.connection.execute(
                """
                UPDATE agent_components
                SET status = 'active', domain_pack_id = ?, role = ?
                WHERE component_id = ?
                """,
                (version.domain_pack_id, target.role, component.component_id),
            )
        previewed = assets.preview_change_set(change_set.change_set_id, "c102-quality-toctou-preview")
        assert previewed.status == "previewed"
        assert previewed.preview is not None
        immutable_preview = previewed.preview.model_dump(mode="json")

        original_readback = geometry_worker.read_shape_program_glb_facts
        geometry_worker.read_shape_program_glb_facts = lambda _payload: (_ for _ in ()).throw(
            ValueError("c102 post-preview damaged GLB")
        )
        try:
            post_preview_failure = assets.quality(version.asset_version_id)
        finally:
            geometry_worker.read_shape_program_glb_facts = original_readback
        assert post_preview_failure.status == "unavailable"
        assert post_preview_failure.evidence_source == "compile_failure"

        confirm_key = "c102-quality-toctou-confirm"
        confirm_scope = f"POST /api/v1/agent/change-sets/{change_set.change_set_id}:confirm"
        with SQLiteUnitOfWork(factory) as unit:
            snapshot_before = unit.active_designs.get_snapshot(version.project_id)
            assert snapshot_before is not None
            snapshot_before_json = snapshot_before.model_dump(mode="json")
            head_before = unit.agent_assets.get_head(version.project_id)
            assert head_before is not None
            head_before_id = str(head_before["asset_version_id"])
            version_ids_before = tuple(
                row["asset_version_id"]
                for row in unit.connection.execute(
                    "SELECT asset_version_id FROM agent_asset_versions WHERE project_id = ? ORDER BY asset_version_id",
                    (version.project_id,),
                ).fetchall()
            )
            change_before = unit.agent_assets.get_change_set(change_set.change_set_id)
            assert change_before is not None
            preview_json_before = str(change_before["preview_json"])
            assert json.loads(preview_json_before) == immutable_preview

        try:
            assets.confirm_change_set(change_set.change_set_id, confirm_key)
        except AgentAssetError as exc:
            assert exc.code == "COMPONENT_QUALITY_NOT_READY", exc.code
            assert exc.status_code == 409
        else:
            raise AssertionError("confirm must recheck component source quality after preview")

        with SQLiteUnitOfWork(factory) as unit:
            snapshot_after = unit.active_designs.get_snapshot(version.project_id)
            assert snapshot_after is not None
            assert snapshot_after.model_dump(mode="json") == snapshot_before_json
            head_after = unit.agent_assets.get_head(version.project_id)
            assert head_after is not None
            assert str(head_after["asset_version_id"]) == head_before_id
            version_ids_after = tuple(
                row["asset_version_id"]
                for row in unit.connection.execute(
                    "SELECT asset_version_id FROM agent_asset_versions WHERE project_id = ? ORDER BY asset_version_id",
                    (version.project_id,),
                ).fetchall()
            )
            assert version_ids_after == version_ids_before
            change_after = unit.agent_assets.get_change_set(change_set.change_set_id)
            assert change_after is not None
            assert change_after["status"] == "previewed"
            assert str(change_after["preview_json"]) == preview_json_before
            assert change_after["resulting_asset_version_id"] is None
            assert unit.idempotency.get(confirm_scope, confirm_key) is None

        # A failed attempt does not consume its idempotency key.  Once a new
        # current report restores trust, that same key can confirm exactly the
        # stored preview, and a later replay returns the completed response.
        assert assets.quality(version.asset_version_id).status == "passed"
        confirmed = assets.confirm_change_set(change_set.change_set_id, confirm_key)
        assert confirmed.change_set.status == "confirmed"
        with SQLiteUnitOfWork(factory) as unit:
            unit.connection.execute(
                "UPDATE agent_components SET status = 'disabled' WHERE component_id = ?",
                (component.component_id,),
            )
        replayed = assets.confirm_change_set(change_set.change_set_id, confirm_key)
        assert replayed.asset_version.asset_version_id == confirmed.asset_version.asset_version_id
    print("C102 component compatibility smoke passed: quality, domain, role, activation, connector-preservation and confirm-time TOCTOU boundary")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
