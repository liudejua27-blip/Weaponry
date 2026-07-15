#!/usr/bin/env python3
"""Smoke safe external GLB import -> immutable reference -> verified re-export."""

from __future__ import annotations

import base64
import json
import struct
import tempfile
from pathlib import Path

from forgecad_agent.application.agent_asset_editing import AgentAssetEditingService, AgentAssetError
from forgecad_agent.application.agent_kernel import AgentKernelService
from forgecad_agent.application.agent_models import (
    AgentPartEditOperation,
    BuildAgentBlockoutRequest,
    CreateAgentThreadRequest,
    ImportAgentGlbRequest,
    ProposeAgentAssetChangeSetRequest,
    StartAgentTurnRequest,
)
from forgecad_agent.application.mechanical_planner import MechanicalConceptPlan
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteMigrationRunner
from forgecad_agent.infrastructure.storage.content_addressed_store import ContentAddressedStore
from smoke_g6_asset_editing import _seed_project


ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad-agent-import-") as raw:
        root = Path(raw)
        factory = SQLiteConnectionFactory(root / "library.db")
        applied = SQLiteMigrationRunner(factory, ROOT / "migrations").run()
        assert "0022" in applied, applied
        _seed_project(factory)
        kernel = AgentKernelService(factory)
        assets = AgentAssetEditingService(factory, ContentAddressedStore(root))
        thread = kernel.create_thread(
            CreateAgentThreadRequest(client_request_id="import-thread", project_id="prj_agent_asset_smoke", title="外部 GLB 导入 smoke"),
            "import-thread",
        )
        turn = kernel.start_turn(
            thread.thread_id,
            StartAgentTurnRequest(client_request_id="import-turn", message="设计一辆适合冰原探索的未来车辆"),
            "import-turn",
        )
        plan = MechanicalConceptPlan.model_validate(next(item.payload["result"] for item in turn.items if item.item_type == "tool_result"))
        built = kernel.build_blockout(
            BuildAgentBlockoutRequest(client_request_id="import-build", plan=plan, direction_id=plan.directions[0].direction_id),
            "import-build",
        )
        imported = assets.import_glb(
            ImportAgentGlbRequest(
                client_request_id="import-glb",
                project_id="prj_agent_asset_smoke",
                domain_pack_id=plan.domain_pack_id,
                file_name="ice-scout.glb",
                glb_base64=built.glb_base64,
                summary="导入冰原探索车参考模型",
            ),
            "import-glb",
        )
        assert imported.asset_version.shape_program["schema_version"] == "ExternalGLBReference@1"
        assert imported.asset_version.parts[0].provenance == "imported_glb"
        assert imported.inspection.triangle_count == built.triangle_count
        assert imported.inspection.mesh_count >= 1
        exported = assets.export_glb(imported.asset_version.asset_version_id)
        assert base64.b64decode(exported.glb_base64) == base64.b64decode(built.glb_base64)
        report = assets.quality(imported.asset_version.asset_version_id)
        assert report.status == "warning" and report.triangle_count == built.triangle_count
        try:
            assets.propose_change_set(
                imported.asset_version.asset_version_id,
                ProposeAgentAssetChangeSetRequest(
                    client_request_id="import-edit-denied",
                    summary="不允许直接修改参考模型",
                    operations=[
                        AgentPartEditOperation(
                            operation_id="op_import_edit",
                            op="set_part_parameter",
                            part_id=imported.asset_version.parts[0].part_id,
                            path="transform.scale.x",
                            value=1.2,
                        )
                    ],
                ),
                "import-edit-denied",
            )
        except AgentAssetError as exc:
            assert exc.code == "EXTERNAL_REFERENCE_NOT_EDITABLE"
        else:
            raise AssertionError("external GLB references must not become editable ShapePrograms")
        try:
            assets.import_glb(
                ImportAgentGlbRequest(
                    client_request_id="import-invalid",
                    project_id="prj_agent_asset_smoke",
                    domain_pack_id=plan.domain_pack_id,
                    file_name="bad.glb",
                    glb_base64=base64.b64encode(b"not-a-real-glb-content").decode("ascii"),
                ),
                "import-invalid",
            )
        except AgentAssetError as exc:
            assert exc.code == "GLB_IMPORT_REJECTED"
        else:
            raise AssertionError("invalid GLB must be rejected before object storage")
        external_buffer_glb = _mutate_glb_json(
            base64.b64decode(built.glb_base64),
            lambda document: document["buffers"][0].update({"uri": "outside.bin"}),
        )
        try:
            assets.import_glb(
                ImportAgentGlbRequest(
                    client_request_id="import-external-buffer",
                    project_id="prj_agent_asset_smoke",
                    domain_pack_id=plan.domain_pack_id,
                    file_name="external-buffer.glb",
                    glb_base64=base64.b64encode(external_buffer_glb).decode("ascii"),
                ),
                "import-external-buffer",
            )
        except AgentAssetError as exc:
            assert exc.code == "GLB_IMPORT_REJECTED"
        else:
            raise AssertionError("external GLB buffers must be rejected")
    print("G7 external GLB import smoke passed: inspect, immutable reference, re-export, edit denial")
    return 0


def _mutate_glb_json(payload: bytes, mutate) -> bytes:
    _, _, declared_length = struct.unpack_from("<4sII", payload, 0)
    assert declared_length == len(payload)
    offset = 12
    json_length, json_type = struct.unpack_from("<II", payload, offset)
    assert json_type == 0x4E4F534A
    offset += 8
    document = json.loads(payload[offset:offset + json_length].rstrip(b" \x00").decode("utf-8"))
    mutate(document)
    offset += json_length
    binary_length, binary_type = struct.unpack_from("<II", payload, offset)
    assert binary_type == 0x004E4942
    offset += 8
    binary = payload[offset:offset + binary_length]
    encoded = json.dumps(document, separators=(",", ":")).encode("utf-8")
    encoded += b" " * ((4 - len(encoded) % 4) % 4)
    binary += b"\x00" * ((4 - len(binary) % 4) % 4)
    total = 12 + 8 + len(encoded) + 8 + len(binary)
    return (
        struct.pack("<4sII", b"glTF", 2, total)
        + struct.pack("<II", len(encoded), 0x4E4F534A)
        + encoded
        + struct.pack("<II", len(binary), 0x004E4942)
        + binary
    )


if __name__ == "__main__":
    raise SystemExit(main())
