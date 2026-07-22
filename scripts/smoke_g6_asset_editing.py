#!/usr/bin/env python3
"""Smoke the general mechanical Agent candidate -> version -> edit loop."""

from __future__ import annotations

import asyncio
import base64
import hashlib
import json
import tempfile
from pathlib import Path

from fastapi import FastAPI

from forgecad_agent.api.agent_asset_routes import build_agent_asset_router
from forgecad_agent.application.agent_asset_editing import AgentAssetEditingService, AgentAssetError
from forgecad_agent.application.agent_kernel import AgentKernelService
from forgecad_agent.application.geometry_worker import (
    compile_shape_program,
    read_shape_program_glb_facts,
)
from forgecad_agent.application.agent_models import (
    AgentPartEditOperation,
    BuildAgentBlockoutRequest,
    CommitAgentBlockoutRequest,
    CreateAgentThreadRequest,
    ProposeAgentAssetChangeSetRequest,
    SegmentAgentBlockoutRequest,
    StartAgentTurnRequest,
)
from forgecad_agent.application.mechanical_planner import MechanicalConceptPlan
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteMigrationRunner, SQLiteUnitOfWork


ROOT = Path(__file__).resolve().parents[1]


async def _get_binary(
    app: FastAPI,
    path: str,
) -> tuple[int, dict[str, str], bytes]:
    messages = [{"type": "http.request", "body": b"", "more_body": False}]
    response: list[dict[str, object]] = []

    async def receive() -> dict[str, object]:
        return messages.pop(0) if messages else {"type": "http.disconnect"}

    async def send(message: dict[str, object]) -> None:
        response.append(message)

    await app(
        {
            "type": "http",
            "asgi": {"version": "3.0", "spec_version": "2.3"},
            "http_version": "1.1",
            "method": "GET",
            "scheme": "http",
            "path": path,
            "raw_path": path.encode("ascii"),
            "query_string": b"",
            "headers": [(b"host", b"testserver")],
            "client": ("127.0.0.1", 12345),
            "server": ("testserver", 80),
        },
        receive,
        send,
    )
    start = next(item for item in response if item["type"] == "http.response.start")
    headers = {
        key.decode("latin-1").lower(): value.decode("latin-1")
        for key, value in start.get("headers", [])
    }
    body = b"".join(
        item.get("body", b"")
        for item in response
        if item["type"] == "http.response.body"
    )
    return int(start["status"]), headers, body


def _seed_project(factory: SQLiteConnectionFactory) -> None:
    now = "2026-07-12T00:00:00+00:00"
    connection = factory.connect()
    connection.execute(
        """
        INSERT INTO domain_profiles(
          profile_id, domain_type, schema_version, pack_id, display_name,
          profile_json, profile_sha256, status, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            "profile_weapon_concept_v1",
            "weapon_concept",
            "DesignDomainProfile@1",
            "weapon-concept-v1-reference",
            "Smoke Fixture",
            "{}",
            "0" * 64,
            "active",
            now,
            now,
        ),
    )
    connection.execute(
        """
        INSERT INTO projects(
          project_id, profile_id, domain_type, name, status,
          current_version_id, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        """,
        ("prj_agent_asset_smoke", "profile_weapon_concept_v1", "weapon_concept", "Agent asset smoke", "active", None, now, now),
    )
    connection.commit()
    connection.close()


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad-agent-assets-") as raw:
        root = Path(raw)
        factory = SQLiteConnectionFactory(root / "library.db")
        applied = SQLiteMigrationRunner(factory, ROOT / "migrations").run()
        assert "0020" in applied and "0025" in applied and "0026" in applied, applied
        _seed_project(factory)
        kernel = AgentKernelService(factory)
        assets = AgentAssetEditingService(factory)

        thread = kernel.create_thread(
            CreateAgentThreadRequest(
                client_request_id="g6-thread",
                project_id="prj_agent_asset_smoke",
                title="机械臂资产编辑 smoke",
            ),
            "g6-thread",
        )
        turn = kernel.start_turn(
            thread.thread_id,
            StartAgentTurnRequest(client_request_id="g6-turn", message="设计一台三关节机械臂"),
            "g6-turn",
        )
        payload = next(
            item.payload["result"]
            for item in turn.items
            if item.item_type == "tool_result" and item.payload.get("tool_name") == "plan_complete_concept"
        )
        plan = MechanicalConceptPlan.model_validate(payload["plan"])
        direction_id = plan.directions[0].direction_id
        built = kernel.build_blockout(
            BuildAgentBlockoutRequest(client_request_id="g6-build", plan=plan, direction_id=direction_id),
            "g6-build",
        )
        segmented = kernel.segment_blockout(
            SegmentAgentBlockoutRequest(
                client_request_id="g6-segment",
                plan=plan,
                direction_id=direction_id,
                artifact_id=built.artifact_id,
            ),
            "g6-segment",
        )
        version = assets.commit_blockout(
            CommitAgentBlockoutRequest(client_request_id="g6-commit", artifact_id=segmented.artifact_id),
            "g6-commit",
        )
        with SQLiteUnitOfWork(factory) as unit:
            snapshot = unit.active_designs.get_snapshot("prj_agent_asset_smoke")
            assert snapshot is not None
            assert snapshot.active_design.asset_version_id == version.asset_version_id
            assert snapshot.export.source_version_id == version.asset_version_id
        base_export = assets.export_glb(version.asset_version_id)
        base_glb_sha256 = hashlib.sha256(base64.b64decode(base_export.glb_base64)).hexdigest()
        part = next(item for item in version.parts if item.editable_parameter_bindings)
        joint_index, joint_part = next(
            (index, item)
            for index, item in enumerate(version.parts)
            if "joint.rotation" in item.editable_parameters
        )
        descendant = version.parts[joint_index + 1]
        change_set = assets.propose_change_set(
            version.asset_version_id,
            ProposeAgentAssetChangeSetRequest(
                client_request_id="g6-propose",
                summary="缩短一段连杆并换成拉丝铝视觉材质",
                operations=[
                    AgentPartEditOperation(
                        operation_id="op_shorten_link",
                        op="set_part_parameter",
                        part_id=part.part_id,
                        path="transform.scale.y",
                        value=0.8,
                    ),
                    AgentPartEditOperation(
                        operation_id="op_aluminum_link",
                        op="apply_material_preset",
                        part_id=part.part_id,
                        material_id="mat_aluminum",
                        material_zone_id=part.material_zone_ids[0],
                    ),
                    AgentPartEditOperation(
                        operation_id="op_joint_pose",
                        op="set_joint_pose",
                        part_id=joint_part.part_id,
                        transform={"rotation": [0, 0, 0.26]},
                    ),
                ],
            ),
            "g6-propose",
        )
        app = FastAPI()
        app.include_router(build_agent_asset_router(assets))
        preview_glb_path = (
            f"/api/v1/agent/change-sets/{change_set.change_set_id}:preview.glb"
        )
        status, _, body = asyncio.run(_get_binary(app, preview_glb_path))
        assert status == 409
        assert json.loads(body)["error"]["code"] == "ASSET_CHANGE_SET_NOT_PREVIEWED"

        preview = assets.preview_change_set(change_set.change_set_id, "g6-preview")
        assert preview.status == "previewed" and preview.preview is not None
        with SQLiteUnitOfWork(factory) as unit:
            snapshot = unit.active_designs.get_snapshot("prj_agent_asset_smoke")
            assert snapshot is not None and snapshot.preview is not None
            assert snapshot.preview.change_set_id == change_set.change_set_id
            assert snapshot.preview.base_asset_version_id == version.asset_version_id
            stored_change = unit.agent_assets.get_change_set(change_set.change_set_id)
            assert stored_change is not None and stored_change["preview_json"]
            stored_preview_json = str(stored_change["preview_json"])
            assert '"glb_base64"' not in stored_preview_json
        preview_descendant = next(item for item in preview.preview.parts if item.part_id == descendant.part_id)
        assert preview_descendant.position_mm != descendant.position_mm, "joint preview must reposition descendants"

        status, headers, preview_glb = asyncio.run(_get_binary(app, preview_glb_path))
        assert status == 200
        assert headers["content-type"] == "model/gltf-binary"
        assert headers["cache-control"] == "no-store"
        preview_glb_sha256 = hashlib.sha256(preview_glb).hexdigest()
        assert headers["x-forgecad-preview-glb-sha256"] == preview_glb_sha256
        assert headers["x-forgecad-base-asset-version-id"] == version.asset_version_id
        assert int(headers["x-forgecad-preview-triangle-count"]) > 0
        assert preview_glb_sha256 != base_glb_sha256
        assert base64.b64encode(preview_glb).decode("ascii") not in stored_preview_json
        preview_glb_facts = read_shape_program_glb_facts(preview_glb)
        target_zone_id = part.material_zone_ids[0]
        assert any(
            zone["material_zone_id"] == target_zone_id
            and zone["material_id"] == "mat_aluminum"
            for zone in preview_glb_facts.material_zone_faces
        ), "preview GLB readback must bind the requested material to the selected Material Zone"
        assert any(
            texture_set["material_id"] == "mat_aluminum"
            and target_zone_id in texture_set["material_zone_ids"]
            and texture_set["visual_texture_set_id"].endswith("_builtin_v3")
            for texture_set in preview_glb_facts.visual_texture_sets
        ), "preview GLB must carry the current embedded five-channel PBR texture set"

        with SQLiteUnitOfWork(factory) as unit:
            snapshot = unit.active_designs.get_snapshot("prj_agent_asset_smoke")
            assert snapshot is not None and snapshot.preview is not None
            unit.active_designs.set_preview(
                project_id="prj_agent_asset_smoke",
                expected_revision=snapshot.revision,
                change_set_id=None,
                base_asset_version_id=None,
                updated_at="2026-07-12T00:00:01+00:00",
            )
        status, _, body = asyncio.run(_get_binary(app, preview_glb_path))
        assert status == 409
        assert json.loads(body)["error"]["code"] == "ACTIVE_DESIGN_PREVIEW_STALE"
        with SQLiteUnitOfWork(factory) as unit:
            snapshot = unit.active_designs.get_snapshot("prj_agent_asset_smoke")
            assert snapshot is not None and snapshot.preview is None
            unit.active_designs.set_preview(
                project_id="prj_agent_asset_smoke",
                expected_revision=snapshot.revision,
                change_set_id=change_set.change_set_id,
                base_asset_version_id=version.asset_version_id,
                updated_at="2026-07-12T00:00:02+00:00",
            )

        confirmed = assets.confirm_change_set(change_set.change_set_id, "g6-confirm")
        assert confirmed.asset_version.version_no == 2
        assert confirmed.asset_version.stage == "editable_asset"
        assert confirmed.asset_version.material_bindings
        assert confirmed.asset_version.material_bindings.get(f"{part.part_id}:{part.material_zone_ids[0]}") == "mat_aluminum"
        assert assets.get_version(confirmed.asset_version.asset_version_id).version_no == 2
        confirmed_export = assets.export_glb(confirmed.asset_version.asset_version_id)
        confirmed_glb_sha256 = hashlib.sha256(base64.b64decode(confirmed_export.glb_base64)).hexdigest()
        assert confirmed_glb_sha256 != base_glb_sha256, "material ChangeSet must produce a different compiled GLB"
        confirmed_readback = compile_shape_program(confirmed.asset_version.shape_program).readback
        assert any(
            zone.material_zone_id == target_zone_id and zone.material_id == "mat_aluminum"
            for zone in confirmed_readback.material_zone_faces
        ), "confirmed GLB readback must bind the requested material to the selected Material Zone"
        assert any(
            texture_set.material_id == "mat_aluminum"
            and target_zone_id in texture_set.material_zone_ids
            and texture_set.visual_texture_set_id.endswith("_builtin_v3")
            for texture_set in confirmed_readback.visual_texture_sets
        ), "confirmed GLB must carry the current embedded five-channel PBR texture set"
        with SQLiteUnitOfWork(factory) as unit:
            snapshot = unit.active_designs.get_snapshot("prj_agent_asset_smoke")
            head = unit.agent_assets.get_head("prj_agent_asset_smoke")
            assert snapshot is not None and head is not None
            assert snapshot.active_design.asset_version_id == confirmed.asset_version.asset_version_id
            assert head["asset_version_id"] == confirmed.asset_version.asset_version_id
            assert snapshot.export.source_version_id == confirmed.asset_version.asset_version_id
            assert snapshot.preview is None
        report = assets.quality(confirmed.asset_version.asset_version_id)
        assert report.status == "passed", report
        assert report.triangle_count > 0
        with SQLiteUnitOfWork(factory) as unit:
            snapshot = unit.active_designs.get_snapshot("prj_agent_asset_smoke")
            stored_report = unit.agent_assets.get_quality_report(report.quality_report_id)
            assert snapshot is not None and snapshot.quality is not None
            assert snapshot.quality.quality_report_id == report.quality_report_id
            assert snapshot.quality.asset_version_id == confirmed.asset_version.asset_version_id
            assert stored_report is not None and stored_report["asset_version_id"] == confirmed.asset_version.asset_version_id
        assert assets.get_quality_report(report.quality_report_id) == report
        overlap_target = next(item for item in confirmed.asset_version.parts if item.part_id != descendant.part_id)
        overlap_change = assets.propose_change_set(
            confirmed.asset_version.asset_version_id,
            ProposeAgentAssetChangeSetRequest(
                client_request_id="g6-overlap-propose",
                summary="制造一个可检测的概念重叠",
                operations=[
                    AgentPartEditOperation(
                        operation_id="op_overlap_descendant",
                        op="set_part_transform",
                        part_id=descendant.part_id,
                        transform={
                            "position": list(overlap_target.position_mm),
                            "rotation": [0, 0, 0],
                            "scale": [1, 1, 1],
                        },
                    )
                ],
            ),
            "g6-overlap-propose",
        )
        assets.preview_change_set(overlap_change.change_set_id, "g6-overlap-preview")
        overlap_confirmed = assets.confirm_change_set(overlap_change.change_set_id, "g6-overlap-confirm")
        try:
            assets.export_glb(confirmed.asset_version.asset_version_id)
            raise AssertionError("stale Agent asset export unexpectedly succeeded")
        except AgentAssetError as exc:
            assert exc.code == "ACTIVE_DESIGN_STALE"
        exported_current = assets.export_glb(overlap_confirmed.asset_version.asset_version_id)
        assert exported_current.asset_version_id == overlap_confirmed.asset_version.asset_version_id
        overlap_report = assets.quality(overlap_confirmed.asset_version.asset_version_id)
        assert overlap_report.status == "warning"
        assert any(item.check_id == "concept_aabb_overlap" for item in overlap_report.findings)
        try:
            assets.propose_change_set(
                overlap_confirmed.asset_version.asset_version_id,
                ProposeAgentAssetChangeSetRequest(
                    client_request_id="g6-invalid",
                    summary="越出声明范围的参数",
                    operations=[
                        AgentPartEditOperation(
                            operation_id="op_invalid",
                            op="set_part_parameter",
                            part_id=part.part_id,
                            path="transform.scale.x",
                            value=1.5,
                        )
                    ],
                ),
                "g6-invalid",
            )
        except AgentAssetError as exc:
            assert exc.code == "PARAMETER_OUT_OF_RANGE"
        else:
            raise AssertionError("out-of-range Agent asset parameters must be rejected")

        try:
            assets.propose_change_set(
                overlap_confirmed.asset_version.asset_version_id,
                ProposeAgentAssetChangeSetRequest(
                    client_request_id="g6-invalid-zone",
                    summary="不存在的材质区",
                    operations=[
                        AgentPartEditOperation(
                            operation_id="op_invalid_zone",
                            op="apply_material_preset",
                            part_id=part.part_id,
                            material_id="mat_aluminum",
                            material_zone_id="zone_not_registered",
                        )
                    ],
                ),
                "g6-invalid-zone",
            )
        except AgentAssetError as exc:
            assert exc.code == "MATERIAL_ZONE_NOT_FOUND"
        else:
            raise AssertionError("unregistered material zones must be rejected")

        try:
            assets.propose_change_set(
                overlap_confirmed.asset_version.asset_version_id,
                ProposeAgentAssetChangeSetRequest(
                    client_request_id="g6-invalid-material-domain",
                    summary="领域不兼容的视觉材质",
                    operations=[
                        AgentPartEditOperation(
                            operation_id="op_invalid_material_domain",
                            op="apply_material_preset",
                            part_id=part.part_id,
                            material_id="mat_automotive_paint",
                            material_zone_id=part.material_zone_ids[0],
                        )
                    ],
                ),
                "g6-invalid-material-domain",
            )
        except AgentAssetError as exc:
            assert exc.code == "MATERIAL_DOMAIN_INCOMPATIBLE"
        else:
            raise AssertionError("domain-incompatible material presets must be rejected")

    print("G6 asset editing smoke passed: binary PBR preview, guarded material domain, confirm, version 2")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
