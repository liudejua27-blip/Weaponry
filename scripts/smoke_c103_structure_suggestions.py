#!/usr/bin/env python3
"""C103 smoke: evidence-bound split/merge suggestions stay preview-first."""

from __future__ import annotations

import asyncio
import json
import tempfile
from pathlib import Path

from fastapi import FastAPI

from forgecad_agent.api.agent_asset_routes import build_agent_asset_router
from forgecad_agent.application.agent_asset_editing import (
    AgentAssetEditingService,
    AgentAssetError,
    _sync_agent_snapshot,
    _version_row,
)
from forgecad_agent.application.agent_models import (
    AgentAssetVersion,
    AgentPartEditOperation,
    BlockoutPartCandidate,
    ProposeAgentAssetChangeSetRequest,
)
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteMigrationRunner, SQLiteUnitOfWork
from smoke_g6_asset_editing import _seed_project


ROOT = Path(__file__).resolve().parents[1]


async def _get_json(app: FastAPI, path: str) -> tuple[int, object]:
    messages = [{"type": "http.request", "body": b"", "more_body": False}]
    response: list[dict[str, object]] = []

    async def receive() -> dict[str, object]:
        return messages.pop(0) if messages else {"type": "http.disconnect"}

    async def send(message: dict[str, object]) -> None:
        response.append(message)

    await app({
        "type": "http", "asgi": {"version": "3.0", "spec_version": "2.3"}, "http_version": "1.1",
        "method": "GET", "scheme": "http", "path": path, "raw_path": path.encode("ascii"),
        "query_string": b"", "headers": [(b"host", b"testserver")],
        "client": ("127.0.0.1", 12345), "server": ("testserver", 80),
    }, receive, send)
    start = next(message for message in response if message["type"] == "http.response.start")
    body = b"".join(message.get("body", b"") for message in response if message["type"] == "http.response.body")
    return int(start["status"]), json.loads(body.decode("utf-8"))


def _part(part_id: str, role: str, position: list[float], size: list[float], parent: str | None = None) -> BlockoutPartCandidate:
    return BlockoutPartCandidate(
        part_id=part_id,
        role=role,
        parent_part_id=parent,
        position_mm=position,
        size_mm=size,
        material_zone_ids=[f"zone_{role}"],
        editable_parameters=["transform.position", "transform.scale"],
        provenance="agent_generated",
    )


def _version(asset_version_id: str, *, parts: list[BlockoutPartCandidate], graph: dict[str, object], program: dict[str, object], version_no: int) -> AgentAssetVersion:
    return AgentAssetVersion(
        asset_version_id=asset_version_id,
        project_id="prj_agent_asset_smoke",
        parent_asset_version_id=None,
        version_no=version_no,
        status="committed",
        summary="C103 fixture",
        stage="editable_asset",
        plan_id="plan_c103",
        direction_id="direction_c103",
        domain_pack_id="pack_vehicle_concept",
        artifact_id="artifact_c103",
        parts=parts,
        shape_program=program,
        assembly_graph=graph,
        material_bindings={},
        created_at="2026-07-13T00:00:00+00:00",
    )


def _activate(factory: SQLiteConnectionFactory, version: AgentAssetVersion) -> None:
    with SQLiteUnitOfWork(factory) as unit:
        unit.agent_assets.add_version(**_version_row(version))
        unit.agent_assets.set_head(project_id=version.project_id, asset_version_id=version.asset_version_id, updated_at=version.created_at)
        _sync_agent_snapshot(unit, version=version, updated_at=version.created_at)


def _split_fixture() -> AgentAssetVersion:
    shell = _part("part_shell", "body_panel", [0, 0, 0], [100, 40, 50])
    program = {
        "schema_version": "ShapeProgram@1", "program_id": "shape_c103_split", "units": "millimeter", "seed": 7,
        "triangle_budget": 100000, "parameters": [], "non_functional_only": True,
        "operations": [
            {"operation_id": "op_shell_a", "op": "box", "inputs": [], "args": {"position": [-35, 0, 0], "size": [30, 40, 50], "part_role": "body_panel", "zone_id": "zone_body_panel"}},
            {"operation_id": "op_shell_b", "op": "box", "inputs": [], "args": {"position": [35, 0, 0], "size": [30, 40, 50], "part_role": "body_panel", "zone_id": "zone_body_panel"}},
        ],
        "outputs": [
            {"output_id": "output_shell_a", "operation_id": "op_shell_a", "kind": "mesh", "part_role": "body_panel"},
            {"output_id": "output_shell_b", "operation_id": "op_shell_b", "kind": "mesh", "part_role": "body_panel"},
        ],
    }
    graph = {
        "schema_version": "AssemblyGraph@1", "graph_id": "mg_c103_split", "concept_id": "concept_c103", "root_part_id": shell.part_id,
        "parts": [{"part_id": shell.part_id, "role": shell.role, "parent_part_id": None, "geometry_source": "shape_program", "transform": {"position": [0, 0, 0], "rotation": [0, 0, 0], "scale": [1, 1, 1]}, "connectors": [], "joints": [], "material_zones": ["zone_body_panel"], "editable_parameters": ["transform.position", "transform.scale"], "locked": False, "provenance": "agent_generated"}],
        "connections": [],
    }
    return _version("assetver_c103_split", parts=[shell], graph=graph, program=program, version_no=1)


def _merge_fixture() -> AgentAssetVersion:
    body = _part("part_body", "body_shell", [0, 0, 0], [100, 40, 50])
    trim = _part("part_trim", "trim_panel", [80, 0, 0], [30, 20, 30], body.part_id)
    program = {
        "schema_version": "ShapeProgram@1", "program_id": "shape_c103_merge", "units": "millimeter", "seed": 7,
        "triangle_budget": 100000, "parameters": [], "non_functional_only": True,
        "operations": [
            {"operation_id": "op_body", "op": "box", "inputs": [], "args": {"position": [0, 0, 0], "size": [100, 40, 50], "part_role": "body_shell", "zone_id": "zone_body_shell"}},
            {"operation_id": "op_trim", "op": "box", "inputs": [], "args": {"position": [80, 0, 0], "size": [30, 20, 30], "part_role": "trim_panel", "zone_id": "zone_trim_panel"}},
        ],
        "outputs": [
            {"output_id": "output_body", "operation_id": "op_body", "kind": "mesh", "part_role": "body_shell"},
            {"output_id": "output_trim", "operation_id": "op_trim", "kind": "mesh", "part_role": "trim_panel"},
        ],
    }
    graph = {
        "schema_version": "AssemblyGraph@1", "graph_id": "mg_c103_merge", "concept_id": "concept_c103", "root_part_id": body.part_id,
        "parts": [
            {"part_id": body.part_id, "role": body.role, "parent_part_id": None, "geometry_source": "shape_program", "transform": {"position": [0, 0, 0], "rotation": [0, 0, 0], "scale": [1, 1, 1]}, "connectors": [{"connector_id": "connector_body_to_trim", "kind": "axial_mount", "position": [80, 0, 0], "normal": [0, 1, 0]}], "joints": [], "material_zones": ["zone_body_shell"], "editable_parameters": ["transform.position", "transform.scale"], "locked": False, "provenance": "agent_generated"},
            {"part_id": trim.part_id, "role": trim.role, "parent_part_id": body.part_id, "geometry_source": "shape_program", "transform": {"position": [80, 0, 0], "rotation": [0, 0, 0], "scale": [1, 1, 1]}, "connectors": [{"connector_id": "connector_trim_mount", "kind": "axial_mount", "position": [0, 0, 0], "normal": [0, 1, 0]}], "joints": [], "material_zones": ["zone_trim_panel"], "editable_parameters": ["transform.position", "transform.scale"], "locked": False, "provenance": "agent_generated"},
        ],
        "connections": [{"connection_id": "conn_body_trim", "from_part_id": body.part_id, "from_connector_id": "connector_body_to_trim", "to_part_id": trim.part_id, "to_connector_id": "connector_trim_mount", "status": "connected"}],
    }
    return _version("assetver_c103_merge", parts=[body, trim], graph=graph, program=program, version_no=3)


def _proposal(version_id: str, operation: AgentPartEditOperation, suffix: str) -> ProposeAgentAssetChangeSetRequest:
    return ProposeAgentAssetChangeSetRequest(client_request_id=f"c103-{suffix}", summary="C103 结构建议预览", operations=[operation])


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad-c103-structure-") as raw:
        factory = SQLiteConnectionFactory(Path(raw) / "library.db")
        applied = SQLiteMigrationRunner(factory, ROOT / "migrations").run()
        assert "0020" in applied and "0023" in applied, applied
        _seed_project(factory)
        assets = AgentAssetEditingService(factory)

        split_source = _split_fixture()
        _activate(factory, split_source)
        split_candidates = assets.list_structure_suggestions(split_source.asset_version_id)
        split = next(item for item in split_candidates.suggestions if item.kind == "split_part")
        assert split.source_facts == ["independent_shape_outputs", "no_connection_or_joint", "no_child_parts"]
        try:
            assets.propose_change_set(split_source.asset_version_id, _proposal(split_source.asset_version_id, AgentPartEditOperation(operation_id="op_c103_bad", op="split_part", part_id=split.part_id, structure_suggestion_id="structure_split_part_missing"), "invalid"), "c103-invalid")
        except AgentAssetError as exc:
            assert exc.code == "STRUCTURE_SUGGESTION_NOT_AVAILABLE", exc.code
        else:
            raise AssertionError("manual split without a listed suggestion must be rejected")
        proposed = assets.propose_change_set(split_source.asset_version_id, _proposal(split_source.asset_version_id, AgentPartEditOperation(operation_id="op_c103_split", op="split_part", part_id=split.part_id, structure_suggestion_id=split.suggestion_id), "split"), "c103-split-propose")
        with SQLiteUnitOfWork(factory) as unit:
            snapshot = unit.active_designs.get_snapshot(split_source.project_id)
            assert snapshot is not None and snapshot.preview is None and snapshot.active_design.asset_version_id == split_source.asset_version_id
        preview = assets.preview_change_set(proposed.change_set_id, "c103-split-preview")
        assert preview.preview is not None and len(preview.preview.parts) == 2
        with SQLiteUnitOfWork(factory) as unit:
            snapshot = unit.active_designs.get_snapshot(split_source.project_id)
            assert snapshot is not None and snapshot.preview is not None
            assert snapshot.active_design.asset_version_id == split_source.asset_version_id
        confirmed = assets.confirm_change_set(proposed.change_set_id, "c103-split-confirm")
        assert len(confirmed.asset_version.parts) == 2 and confirmed.asset_version.parent_asset_version_id == split_source.asset_version_id
        split_report = assets.quality(confirmed.asset_version.asset_version_id)
        assert split_report.status == "passed", split_report
        assert assets.export_glb(confirmed.asset_version.asset_version_id).asset_version_id == confirmed.asset_version.asset_version_id

        merge_source = _merge_fixture()
        _activate(factory, merge_source)
        merge_candidates = assets.list_structure_suggestions(merge_source.asset_version_id)
        merge = next(item for item in merge_candidates.suggestions if item.kind == "merge_parts")
        app = FastAPI()
        app.include_router(build_agent_asset_router(assets))
        status, payload = asyncio.run(_get_json(app, f"/api/v1/agent/asset-versions/{merge_source.asset_version_id}/structure-suggestions"))
        assert status == 200 and isinstance(payload, dict) and payload["suggestions"][0]["kind"] == "merge_parts"
        merge_change = assets.propose_change_set(merge_source.asset_version_id, _proposal(merge_source.asset_version_id, AgentPartEditOperation(operation_id="op_c103_merge", op="merge_parts", part_id=merge.part_id, target_part_id=merge.target_part_id, structure_suggestion_id=merge.suggestion_id), "merge"), "c103-merge-propose")
        merge_preview = assets.preview_change_set(merge_change.change_set_id, "c103-merge-preview")
        assert merge_preview.preview is not None and len(merge_preview.preview.parts) == 1
        merge_confirmed = assets.confirm_change_set(merge_change.change_set_id, "c103-merge-confirm")
        assert len(merge_confirmed.asset_version.parts) == 1
        assert assets.quality(merge_confirmed.asset_version.asset_version_id).status == "passed"
        assert assets.export_glb(merge_confirmed.asset_version.asset_version_id).asset_version_id == merge_confirmed.asset_version.asset_version_id
    print("C103 structure suggestions smoke passed: read-only suggestions, guarded split/merge preview, confirm, quality, export and Snapshot")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
