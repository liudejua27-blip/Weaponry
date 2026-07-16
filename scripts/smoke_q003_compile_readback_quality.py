#!/usr/bin/env python3
"""Gate quality facts against the compiled GLB readback, never an estimate."""

from __future__ import annotations

import base64
import copy
import hashlib
import json
import struct
import tempfile
from pathlib import Path

from forgecad_agent.application import geometry_worker
from forgecad_agent.application.agent_asset_editing import (
    AgentAssetEditingService,
    AgentAssetError,
    _quality_request_hash,
)
from forgecad_agent.application.agent_kernel import AgentKernelService
from forgecad_agent.application.agent_models import (
    AgentAssetQualityReport,
    BuildAgentBlockoutRequest,
    CommitAgentBlockoutRequest,
    CreateAgentThreadRequest,
    SegmentAgentBlockoutRequest,
    StartAgentTurnRequest,
)
from forgecad_agent.application.mechanical_planner import MechanicalConceptPlan
from forgecad_agent.application.visual_texture_sets import (
    legacy_builtin_visual_texture_sets,
    visual_texture_png_bytes,
)
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteMigrationRunner, SQLiteUnitOfWork


ROOT = Path(__file__).resolve().parents[1]
CASES = (
    ("future", "设计一个虚构非功能的未来概念道具", "pack_future_weapon_prop"),
    ("vehicle", "设计一辆城市探索汽车", "pack_vehicle_concept"),
    ("aircraft", "设计一架垂直起降概念飞机", "pack_aircraft_concept"),
    ("robotic", "设计一台三关节机械臂", "pack_robotic_arm_concept"),
)
PBR_TEXTURE_FIELDS = {
    "base_color": ("pbrMetallicRoughness", "baseColorTexture"),
    "metallic_roughness": ("pbrMetallicRoughness", "metallicRoughnessTexture"),
    "normal": (None, "normalTexture"),
    "occlusion": (None, "occlusionTexture"),
    "emissive": (None, "emissiveTexture"),
}


def _glb_parts(payload: bytes) -> tuple[dict, bytearray]:
    offset = 12
    document = None
    binary = bytearray()
    while offset + 8 <= len(payload):
        length, kind = struct.unpack_from("<II", payload, offset)
        offset += 8
        chunk = payload[offset:offset + length]
        if kind == 0x4E4F534A:
            document = json.loads(chunk.rstrip(b" \x00").decode("utf-8"))
        elif kind == 0x004E4942:
            binary = bytearray(chunk)
        offset += length
    assert isinstance(document, dict) and binary
    return document, binary


def _glb_payload(document: dict, binary: bytearray) -> bytes:
    encoded = json.dumps(document, separators=(",", ":")).encode("utf-8")
    encoded += b" " * ((4 - len(encoded) % 4) % 4)
    binary.extend(b"\x00" * ((4 - len(binary) % 4) % 4))
    total = 12 + 8 + len(encoded) + 8 + len(binary)
    return (
        struct.pack("<4sII", b"glTF", 2, total)
        + struct.pack("<II", len(encoded), 0x4E4F534A)
        + encoded
        + struct.pack("<II", len(binary), 0x004E4942)
        + bytes(binary)
    )


def _material_texture_reference(document: dict, material_index: int, role: str) -> dict:
    parent_field, texture_field = PBR_TEXTURE_FIELDS[role]
    material = document["materials"][material_index]
    parent = material[parent_field] if parent_field is not None else material
    reference = parent[texture_field]
    assert isinstance(reference, dict) and type(reference.get("index")) is int
    return reference


def _legacy_v1_glb(payload: bytes) -> bytes:
    """Rewrite one current compiler GLB to the immutable pre-v2 texture bytes."""

    document, binary = _glb_parts(payload)
    for material_index, texture_set in enumerate(legacy_builtin_visual_texture_sets()):
        material = document["materials"][material_index]
        material["extras"]["forgecad_visual_texture_set_id"] = texture_set.visual_texture_set_id
        for texture_map in texture_set.maps:
            texture_index = int(
                _material_texture_reference(
                    document,
                    material_index,
                    texture_map.texture_role,
                )["index"]
            )
            texture = document["textures"][texture_index]
            image = document["images"][int(texture["source"])]
            view = document["bufferViews"][int(image["bufferView"])]
            texture_payload = visual_texture_png_bytes(texture_map.texture_id)
            binary.extend(b"\x00" * ((4 - len(binary) % 4) % 4))
            view["byteOffset"] = len(binary)
            view["byteLength"] = len(texture_payload)
            binary.extend(texture_payload)
            texture["name"] = texture_map.texture_id
            image["name"] = texture_map.texture_id
            image["extras"]["forgecad_visual_texture"] = texture_map.model_dump(mode="json")
    binary.extend(b"\x00" * ((4 - len(binary) % 4) % 4))
    document["buffers"][0]["byteLength"] = len(binary)
    return _glb_payload(document, binary)


def _seed(factory: SQLiteConnectionFactory) -> None:
    now = "2026-07-15T00:00:00+00:00"
    with factory.connect() as connection:
        connection.execute(
            """INSERT INTO domain_profiles(profile_id, domain_type, schema_version, pack_id, display_name, profile_json, profile_sha256, status, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)""",
            ("profile_weapon_concept_v1", "weapon_concept", "DesignDomainProfile@1", "weapon-concept-v1-reference", "Q003", "{}", "0" * 64, "active", now, now),
        )
        for suffix, _, _ in CASES:
            connection.execute(
                """INSERT INTO projects(project_id, profile_id, domain_type, name, status, current_version_id, created_at, updated_at)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?)""",
                (f"prj_q003_{suffix}", "profile_weapon_concept_v1", "weapon_concept", f"Q003 {suffix}", "active", None, now, now),
            )


def _make_asset(factory: SQLiteConnectionFactory, suffix: str, message: str, expected_pack: str):
    kernel = AgentKernelService(factory)
    assets = AgentAssetEditingService(factory)
    thread = kernel.create_thread(
        CreateAgentThreadRequest(client_request_id=f"q003-thread-{suffix}", project_id=f"prj_q003_{suffix}", title=f"Q003 {suffix}"),
        f"q003-thread-{suffix}",
    )
    turn = kernel.start_turn(
        thread.thread_id,
        StartAgentTurnRequest(client_request_id=f"q003-turn-{suffix}", message=message),
        f"q003-turn-{suffix}",
    )
    plan_result = next(item.payload["result"] for item in turn.items if item.item_type == "tool_result" and item.payload.get("tool_name") == "plan_complete_concept")
    plan = MechanicalConceptPlan.model_validate(plan_result["plan"])
    assert plan.domain_pack_id == expected_pack, plan.domain_pack_id
    direction_id = plan.directions[0].direction_id
    built = kernel.build_blockout(
        BuildAgentBlockoutRequest(client_request_id=f"q003-build-{suffix}", plan=plan, direction_id=direction_id),
        f"q003-build-{suffix}",
    )
    segmented = kernel.segment_blockout(
        SegmentAgentBlockoutRequest(client_request_id=f"q003-segment-{suffix}", plan=plan, direction_id=direction_id, artifact_id=built.artifact_id),
        f"q003-segment-{suffix}",
    )
    return assets, assets.commit_blockout(
        CommitAgentBlockoutRequest(client_request_id=f"q003-commit-{suffix}", artifact_id=segmented.artifact_id),
        f"q003-commit-{suffix}",
    )


def _revision(factory: SQLiteConnectionFactory, project_id: str) -> int:
    with SQLiteUnitOfWork(factory) as unit:
        snapshot = unit.active_designs.get_snapshot(project_id)
        assert snapshot is not None
        return snapshot.revision


def _assert_error(action, code: str) -> None:
    try:
        action()
    except AgentAssetError as exc:
        assert exc.code == code, exc
        return
    raise AssertionError(f"expected {code}")


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad-q003-") as raw:
        factory = SQLiteConnectionFactory(Path(raw) / "library.db")
        SQLiteMigrationRunner(factory, ROOT / "migrations").run()
        _seed(factory)
        versions = []

        for suffix, message, pack in CASES:
            assets, version = _make_asset(factory, suffix, message, pack)
            compiled = geometry_worker.compile_shape_program(version.shape_program)
            revision = _revision(factory, version.project_id)
            report = assets.quality(
                version.asset_version_id,
                expected_revision=revision,
                idempotency_key=f"q003-quality-{suffix}",
            )
            assert report.evidence_source == "geometry_compile_readback"
            assert report.compile_readback == compiled.readback
            assert report.triangle_count == compiled.readback.triangle_count
            assert report.bounds_mm == compiled.readback.bounds_mm

            exported = assets.export_glb(version.asset_version_id)
            exported_glb = base64.b64decode(exported.glb_base64, validate=True)
            assert exported.triangle_count == report.triangle_count
            assert exported.bounds_mm == report.bounds_mm
            assert hashlib.sha256(exported_glb).hexdigest() == report.compile_readback.glb_sha256

            # A process restart must replay the immutable report before compiling.
            restarted = AgentAssetEditingService(factory)
            original_compile = __import__(
                "forgecad_agent.application.agent_asset_editing", fromlist=["compile_shape_program"]
            ).compile_shape_program
            editing_module = __import__("forgecad_agent.application.agent_asset_editing", fromlist=["compile_shape_program"])
            editing_module.compile_shape_program = lambda _program: (_ for _ in ()).throw(AssertionError("replay recompiled geometry"))
            try:
                assert restarted.quality(
                    version.asset_version_id,
                    expected_revision=revision,
                    idempotency_key=f"q003-quality-{suffix}",
                ) == report
            finally:
                editing_module.compile_shape_program = original_compile
            versions.append((assets, version, report))

        # A real pre-v2 readback remains parseable for history, but it must not
        # survive restart as passed evidence for today's v2 compiler/export.
        assets, version, current = versions[0]
        current_compiled = geometry_worker.compile_shape_program(version.shape_program)
        legacy_glb = _legacy_v1_glb(current_compiled.glb_bytes)
        legacy_facts = geometry_worker.read_shape_program_glb_facts(legacy_glb)
        legacy_glb_sha256 = hashlib.sha256(legacy_glb).hexdigest()
        assert legacy_glb_sha256 != current_compiled.readback.glb_sha256
        legacy_readback = current.compile_readback.model_dump(mode="json")
        legacy_readback.update({
            "glb_sha256": legacy_glb_sha256,
            "glb_byte_size": len(legacy_glb),
            "visual_texture_sets": copy.deepcopy(legacy_facts.visual_texture_sets),
        })
        assert all(
            item["visual_texture_set_id"].endswith("_builtin")
            and not item["visual_texture_set_id"].endswith("_builtin_v2")
            and not item["visual_texture_set_id"].endswith("_builtin_v3")
            for item in legacy_readback["visual_texture_sets"]
        )
        # This field did not exist when the v1 quality report was persisted.
        for item in legacy_readback["visual_texture_sets"]:
            item.pop("texture_material_id")
        pre_v2_payload = current.model_dump(mode="json")
        pre_v2_payload.update({
            "quality_report_id": "quality_q003_pre_v2",
            "status": "passed",
            "triangle_count": legacy_facts.triangle_count,
            "bounds_mm": legacy_facts.bounds_mm,
            "evidence_source": "geometry_compile_readback",
            "compile_readback": legacy_readback,
            "findings": [],
            "checked_at": "2026-07-15T12:00:00+00:00",
        })
        pre_v2_json = json.dumps(pre_v2_payload, ensure_ascii=False, sort_keys=True)
        replay_key = "q003-pre-v2-replay"
        with SQLiteUnitOfWork(factory) as unit:
            snapshot = unit.active_designs.get_snapshot(version.project_id)
            assert snapshot is not None
            replay_revision = snapshot.revision
            unit.agent_assets.add_quality_report(
                quality_report_id=pre_v2_payload["quality_report_id"],
                project_id=version.project_id,
                asset_version_id=version.asset_version_id,
                report_json=pre_v2_json,
                status="passed",
                created_at=pre_v2_payload["checked_at"],
            )
            unit.active_designs.set_quality(
                project_id=version.project_id,
                expected_revision=replay_revision,
                quality_report_id=pre_v2_payload["quality_report_id"],
                asset_version_id=version.asset_version_id,
                updated_at=pre_v2_payload["checked_at"],
            )
            unit.idempotency.add(
                scope=f"POST /api/v1/agent/asset-versions/{version.asset_version_id}:quality",
                key=replay_key,
                request_hash=_quality_request_hash(version.asset_version_id, replay_revision),
                response_json=pre_v2_json,
                created_at=pre_v2_payload["checked_at"],
            )

        restarted = AgentAssetEditingService(factory)
        restarted_snapshot_revision = _revision(factory, version.project_id)
        assert restarted_snapshot_revision == replay_revision + 1
        current_export = restarted.export_glb(version.asset_version_id)
        current_export_sha256 = hashlib.sha256(
            base64.b64decode(current_export.glb_base64, validate=True)
        ).hexdigest()
        assert current_export_sha256 == current_compiled.readback.glb_sha256
        assert current_export_sha256 != legacy_glb_sha256
        isolated_pre_v2 = restarted.get_quality_report(pre_v2_payload["quality_report_id"])
        assert isolated_pre_v2.status == "unavailable"
        assert isolated_pre_v2.evidence_source == "stale_compile_readback"
        assert isolated_pre_v2.triangle_count == 0 and isolated_pre_v2.bounds_mm is None
        assert isolated_pre_v2.compile_readback is None
        assert isolated_pre_v2.findings[0].check_id == "stale_geometry_compile_readback"
        replayed_pre_v2 = restarted.quality(
            version.asset_version_id,
            expected_revision=replay_revision,
            idempotency_key=replay_key,
        )
        assert replayed_pre_v2 == isolated_pre_v2
        assert assets.get_quality_report(current.quality_report_id) == current

        # A pre-Q003 estimate remains readable only as unavailable evidence and
        # cannot replace a current readback-backed report.
        legacy = AgentAssetQualityReport(
            quality_report_id="quality_q003_legacy",
            asset_version_id=version.asset_version_id,
            status="passed",
            triangle_count=999999,
            findings=[],
            checked_at="2026-07-14T00:00:00+00:00",
        )
        with SQLiteUnitOfWork(factory) as unit:
            unit.agent_assets.add_quality_report(
                quality_report_id=legacy.quality_report_id,
                project_id=version.project_id,
                asset_version_id=version.asset_version_id,
                report_json=json.dumps(legacy.model_dump(mode="json"), ensure_ascii=False, sort_keys=True),
                status=legacy.status,
                created_at=legacy.checked_at,
            )
        isolated = assets.get_quality_report(legacy.quality_report_id)
        assert isolated.status == "unavailable" and isolated.triangle_count == 0
        assert isolated.evidence_source == "legacy_estimate"

        # Damaged GLB readback produces explicit unavailable quality and blocks export.
        assets, version, _ = versions[1]
        original_readback = geometry_worker.read_shape_program_glb_facts
        geometry_worker.read_shape_program_glb_facts = lambda _payload: (_ for _ in ()).throw(ValueError("damaged GLB"))
        try:
            failure = assets.quality(
                version.asset_version_id,
                expected_revision=_revision(factory, version.project_id),
                idempotency_key="q003-damaged-readback",
            )
            assert failure.status == "unavailable" and failure.evidence_source == "compile_failure"
            assert failure.triangle_count == 0 and failure.bounds_mm is None and failure.compile_readback is None
            _assert_error(lambda: assets.export_glb(version.asset_version_id), "GEOMETRY_READBACK_FAILED")
        finally:
            geometry_worker.read_shape_program_glb_facts = original_readback

        # Persisted unknown operations remain fail-closed and write no report.
        assets, version, _ = versions[-1]
        unknown = json.loads(json.dumps(version.shape_program))
        unknown["operations"][0]["op"] = "pivot"
        with factory.connect() as connection:
            before = connection.execute("SELECT COUNT(*) FROM agent_asset_quality_reports").fetchone()[0]
            connection.execute(
                "UPDATE agent_asset_versions SET shape_program_json = ? WHERE asset_version_id = ?",
                (json.dumps(unknown, ensure_ascii=False, sort_keys=True), version.asset_version_id),
            )
        _assert_error(lambda: assets.quality(version.asset_version_id), "UNSUPPORTED_RUNTIME_OPERATION")
        with factory.connect() as connection:
            after = connection.execute("SELECT COUNT(*) FROM agent_asset_quality_reports").fetchone()[0]
        assert after == before

    print("Q003 compile/readback quality smoke passed: four domains share immutable GLB facts; failures and legacy estimates are isolated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
