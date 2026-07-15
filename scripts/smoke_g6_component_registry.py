#!/usr/bin/env python3
"""Smoke Agent component save -> list -> preview replacement -> confirm."""

from __future__ import annotations

import tempfile
import base64
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
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteMigrationRunner
from smoke_g6_asset_editing import _seed_project

ROOT = Path(__file__).resolve().parents[1]


def _compatible_connector_pair(version):
    """Find two distinct parts that expose the same supported connector kind.

    Mechanical plans are intentionally allowed to use different stable internal
    roles.  This smoke exercises the component registry contract, not one
    particular planner vocabulary, so it must derive its fixture from the
    persisted assembly graph.
    """

    graph_by_part = {
        item["part_id"]: item
        for item in version.assembly_graph["parts"]
        if isinstance(item, dict) and isinstance(item.get("part_id"), str)
    }
    part_by_id = {item.part_id: item for item in version.parts}
    for source_part_id, source_graph in graph_by_part.items():
        for target_part_id, target_graph in graph_by_part.items():
            if target_part_id == source_part_id:
                continue
            for source_connector in source_graph.get("connectors", []):
                for target_connector in target_graph.get("connectors", []):
                    if source_connector.get("kind") == target_connector.get("kind"):
                        return (
                            part_by_id[source_part_id],
                            source_graph,
                            source_connector,
                            part_by_id[target_part_id],
                            target_graph,
                            target_connector,
                        )
    raise AssertionError("the committed assembly needs two compatible connectors")


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad-agent-components-") as raw:
        factory = SQLiteConnectionFactory(Path(raw) / "library.db")
        applied = SQLiteMigrationRunner(factory, ROOT / "migrations").run()
        assert "0021" in applied, applied
        _seed_project(factory)
        kernel = AgentKernelService(factory)
        assets = AgentAssetEditingService(factory)
        thread = kernel.create_thread(CreateAgentThreadRequest(client_request_id="component-thread", project_id="prj_agent_asset_smoke", title="组件 smoke"), "component-thread")
        turn = kernel.start_turn(thread.thread_id, StartAgentTurnRequest(client_request_id="component-turn", message="设计一台三关节机械臂"), "component-turn")
        plan = MechanicalConceptPlan.model_validate(next(item.payload["result"] for item in turn.items if item.item_type == "tool_result"))
        direction_id = plan.directions[0].direction_id
        built = kernel.build_blockout(BuildAgentBlockoutRequest(client_request_id="component-build", plan=plan, direction_id=direction_id), "component-build")
        segmented = kernel.segment_blockout(SegmentAgentBlockoutRequest(client_request_id="component-segment", plan=plan, direction_id=direction_id, artifact_id=built.artifact_id), "component-segment")
        version = assets.commit_blockout(CommitAgentBlockoutRequest(client_request_id="component-commit", artifact_id=segmented.artifact_id), "component-commit")
        assert assets.quality(version.asset_version_id).status == "passed"
        source_part = version.parts[0]
        component = assets.save_component(version.asset_version_id, SaveAgentComponentRequest(client_request_id="component-save", part_id=source_part.part_id, display_name="可替换部件"), "component-save")
        assert component.role == source_part.role and component.status == "active"
        listed = assets.list_components(version.project_id, domain_pack_id=version.domain_pack_id, role=source_part.role)
        assert any(item.component_id == component.component_id for item in listed)
        target = source_part
        change_set = assets.propose_change_set(version.asset_version_id, ProposeAgentAssetChangeSetRequest(client_request_id="component-replace-propose", summary="替换连杆", operations=[AgentPartEditOperation(operation_id="op_replace", op="replace_part", part_id=target.part_id, replacement_component_id=component.component_id)]), "component-replace-propose")
        preview = assets.preview_change_set(change_set.change_set_id, "component-replace-preview")
        assert preview.preview and next(item for item in preview.preview.parts if item.part_id == target.part_id).provenance == "agent_component"
        confirmed = assets.confirm_change_set(change_set.change_set_id, "component-replace-confirm")
        assert confirmed.asset_version.version_no == 2
        assert assets.quality(confirmed.asset_version.asset_version_id).status == "passed"
        snap_source, source_graph, source_connector, snap_target, target_graph, target_connector = _compatible_connector_pair(confirmed.asset_version)
        snap_set = assets.propose_change_set(confirmed.asset_version.asset_version_id, ProposeAgentAssetChangeSetRequest(client_request_id="component-snap-propose", summary="连接器对齐", operations=[AgentPartEditOperation(operation_id="op_snap", op="snap_part_to_connector", part_id=snap_source.part_id, connector_id=source_connector["connector_id"], target_part_id=snap_target.part_id, target_connector_id=target_connector["connector_id"])]), "component-snap-propose")
        snap_preview = assets.preview_change_set(snap_set.change_set_id, "component-snap-preview")
        assert snap_preview.preview
        snapped = assets.confirm_change_set(snap_set.change_set_id, "component-snap-confirm")
        assert snapped.asset_version.version_no == 3
        try:
            assets.propose_change_set(snapped.asset_version.asset_version_id, ProposeAgentAssetChangeSetRequest(client_request_id="component-bad-snap", summary="无效连接器", operations=[AgentPartEditOperation(operation_id="op_bad_snap", op="snap_part_to_connector", part_id=snap_source.part_id, connector_id="connector_missing", target_part_id=snap_target.part_id, target_connector_id=target_connector["connector_id"])]), "component-bad-snap")
        except AgentAssetError as exc:
            assert exc.code == "CONNECTOR_NOT_FOUND"
        else:
            raise AssertionError("unknown connectors must be rejected")
        exported = assets.export_glb(snapped.asset_version.asset_version_id)
        assert base64.b64decode(exported.glb_base64).startswith(b"glTF")
        assert exported.triangle_count > 0 and all(value > 0 for value in exported.bounds_mm)
    print("G6 component registry smoke passed: save, list, replacement preview, confirm")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
