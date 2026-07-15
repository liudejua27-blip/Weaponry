#!/usr/bin/env python3
"""S008 smoke: immutable Agent asset undo/redo bound to ActiveDesignSnapshot."""

from __future__ import annotations

import asyncio
import json
import os
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
NOW = "2026-07-13T00:00:00+00:00"


async def _request(
    app: object,
    method: str,
    path: str,
    *,
    payload: dict[str, object] | None = None,
    headers: dict[str, str] | None = None,
) -> tuple[int, dict[str, str], dict[str, object]]:
    body = json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8") if payload else b""
    incoming = [{"type": "http.request", "body": body, "more_body": False}]
    outgoing: list[dict[str, object]] = []

    async def receive() -> dict[str, object]:
        return incoming.pop(0) if incoming else {"type": "http.disconnect"}

    async def send(message: dict[str, object]) -> None:
        outgoing.append(message)

    raw_headers: list[tuple[bytes, bytes]] = [(b"host", b"testserver")]
    if payload is not None:
        raw_headers.append((b"content-type", b"application/json"))
    raw_headers.extend((key.lower().encode("latin-1"), value.encode("latin-1")) for key, value in (headers or {}).items())
    await app(  # type: ignore[operator]
        {
            "type": "http",
            "asgi": {"version": "3.0", "spec_version": "2.3"},
            "http_version": "1.1",
            "method": method,
            "scheme": "http",
            "path": path,
            "raw_path": path.encode("ascii"),
            "query_string": b"",
            "headers": raw_headers,
            "client": ("127.0.0.1", 12345),
            "server": ("testserver", 80),
        },
        receive,
        send,
    )
    start = next(message for message in outgoing if message["type"] == "http.response.start")
    response_body = b"".join(message.get("body", b"") for message in outgoing if message["type"] == "http.response.body")
    response_headers = {key.decode("latin-1").lower(): value.decode("latin-1") for key, value in start.get("headers", [])}  # type: ignore[union-attr]
    return int(start["status"]), response_headers, json.loads(response_body.decode("utf-8"))


def _seed_project(factory: object, project_id: str) -> None:
    connection = factory.connect()  # type: ignore[attr-defined]
    try:
        connection.execute(
            """
            INSERT INTO domain_profiles(
              profile_id, domain_type, schema_version, pack_id, display_name,
              profile_json, profile_sha256, status, created_at, updated_at
            ) VALUES ('profile_s008', 'weapon_concept', 'DesignDomainProfile@1',
                      'pack_weapon_concept', 'S008 fixture', '{}', ?, 'active', ?, ?)
            """,
            ("0" * 64, NOW, NOW),
        )
        connection.execute(
            """
            INSERT INTO projects(
              project_id, profile_id, domain_type, name, status,
              current_version_id, created_at, updated_at
            ) VALUES (?, 'profile_s008', 'weapon_concept', 'S008 navigation', 'active', NULL, ?, ?)
            """,
            (project_id, NOW, NOW),
        )
        connection.commit()
    finally:
        connection.close()


def _add_version(unit: object, *, asset_version_id: str, parent: str | None, version_no: int, status: str, summary: str) -> None:
    graph = {
        "schema_version": "AssemblyGraph@1",
        "graph_id": f"mg_{asset_version_id.removeprefix('assetver_')}",
        "parts": [{"part_id": "part_s008_body"}],
        "connections": [],
    }
    unit.agent_assets.add_version(  # type: ignore[attr-defined]
        asset_version_id=asset_version_id,
        project_id="prj_s008_navigation",
        parent_asset_version_id=parent,
        version_no=version_no,
        status=status,
        summary=summary,
        stage="editable_asset",
        plan_id="plan_s008",
        direction_id="direction_s008",
        domain_pack_id="pack_weapon_concept",
        artifact_id=f"artifact_{asset_version_id}",
        parts_json='[{"part_id":"part_s008_body","role":"primary_body","parent_part_id":null,"position_mm":[0,0,0],"size_mm":[100,40,30],"material_zone_ids":["zone_body"],"editable_parameters":[],"locked":false,"provenance":"agent_generated"}]',
        shape_program_json='{"schema_version":"ShapeProgram@1","operations":[{"id":"box_s008","op":"box","args":{"size":[100,40,30]}}]}',
        assembly_graph_json=__import__("json").dumps(graph, sort_keys=True, separators=(",", ":")),
        material_bindings_json='{}',
        created_at=NOW,
    )


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad-s008-navigation-") as raw:
        root = Path(raw)
        os.environ["WUSHEN_LIBRARY_ROOT"] = str(root / "library")
        os.environ["WUSHEN_MIGRATIONS_DIR"] = str(ROOT / "migrations")
        os.environ["WUSHEN_LOCAL_WORKER_ENABLED"] = "0"
        os.environ["FORGECAD_CONCEPT_WORKER_ENABLED"] = "0"

        from forgecad_agent.application.active_design import ActiveDesignApiError, ActiveDesignService
        from forgecad_agent.application.agent_asset_editing import AgentAssetEditingService
        from forgecad_agent.application.agent_models import NavigateActiveDesignRequest, SelectActiveDesignRequest
        from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteUnitOfWork
        from wushen_agent.main import create_app

        app = create_app()  # applies the real migration chain, including 0026
        factory = SQLiteConnectionFactory(root / "library" / "library.db")
        _seed_project(factory, "prj_s008_navigation")
        with SQLiteUnitOfWork(factory) as unit:
            _add_version(unit, asset_version_id="assetver_s008_v1", parent=None, version_no=1, status="superseded", summary="初始外观")
            _add_version(unit, asset_version_id="assetver_s008_v2", parent="assetver_s008_v1", version_no=2, status="committed", summary="加长后的外观")
            unit.agent_assets.set_head(project_id="prj_s008_navigation", asset_version_id="assetver_s008_v2", updated_at=NOW)
            unit.active_designs.create_agent_snapshot(
                project_id="prj_s008_navigation",
                asset_version_id="assetver_s008_v2",
                assembly_graph_id="mg_s008_v2",
                updated_at=NOW,
            )

        undo_request = {"client_request_id": "s008-undo", "snapshot_revision": 1}
        undo_headers = {"Idempotency-Key": "s008-undo-key", "If-Match": 'W/"active-design-1"'}
        status, response_headers, undone = asyncio.run(_request(
            app,
            "POST",
            "/api/v1/projects/prj_s008_navigation/active-design:undo",
            payload=undo_request,
            headers=undo_headers,
        ))
        assert status == 200 and response_headers["etag"] == 'W/"active-design-2"'
        undo_version_id = str(undone["active_design"]["asset_version_id"])
        assert int(undone["revision"]) == 2 and undo_version_id not in {"assetver_s008_v1", "assetver_s008_v2"}
        assert undone["selected_part_id"] is None and undone["preview"] is None and undone["quality"] is None
        status, replay_headers, replay = asyncio.run(_request(
            app,
            "POST",
            "/api/v1/projects/prj_s008_navigation/active-design:undo",
            payload=undo_request,
            headers=undo_headers,
        ))
        assert status == 200 and replay_headers["etag"] == 'W/"active-design-2"' and replay == undone

        service = ActiveDesignService(factory)
        try:
            service.navigate_asset(
                "prj_s008_navigation",
                NavigateActiveDesignRequest(client_request_id="s008-stale", snapshot_revision=1),
                expected_revision=1,
                idempotency_key="s008-stale-key",
                action="redo",
            )
            raise AssertionError("stale redo unexpectedly succeeded")
        except ActiveDesignApiError as exc:
            assert exc.code == "ACTIVE_DESIGN_STALE"
        redone = service.navigate_asset(
            "prj_s008_navigation",
            NavigateActiveDesignRequest(client_request_id="s008-redo", snapshot_revision=2),
            expected_revision=2,
            idempotency_key="s008-redo-key",
            action="redo",
        )
        redo_version_id = redone.active_design.asset_version_id  # type: ignore[union-attr]
        assert redone.revision == 3 and redo_version_id not in {"assetver_s008_v1", "assetver_s008_v2", undo_version_id}

        with SQLiteUnitOfWork(factory) as unit:
            head = unit.agent_assets.get_head("prj_s008_navigation")
            assert head is not None and str(head["asset_version_id"]) == redo_version_id
            restored = unit.agent_assets.get_version(redo_version_id)
            assert restored is not None and str(restored["parent_asset_version_id"]) == "assetver_s008_v2"
            assert str(restored["status"]) == "committed"
            assert str(unit.agent_assets.get_version("assetver_s008_v2")["status"]) == "superseded"  # type: ignore[index]
            frame = unit.agent_assets.get_navigation_frame(redo_version_id)
            assert frame is not None and str(frame["undo_target_asset_version_id"]) == "assetver_s008_v1"
            assert frame["redo_target_asset_version_id"] is None

            # A pending preview is an explicit state barrier: navigation must
            # never silently discard it.
            unit.agent_assets.add_change_set(
                change_set_id="assetcs_s008_pending",
                project_id="prj_s008_navigation",
                base_asset_version_id=redo_version_id,
                summary="S008 pending preview fixture",
                operations_json="[]",
                protected_part_ids_json="[]",
                created_at=NOW,
            )
            unit.active_designs.set_preview(
                project_id="prj_s008_navigation",
                expected_revision=3,
                change_set_id="assetcs_s008_pending",
                base_asset_version_id=redo_version_id,
                updated_at=NOW,
            )
        try:
            service.navigate_asset(
                "prj_s008_navigation",
                NavigateActiveDesignRequest(client_request_id="s008-preview-block", snapshot_revision=4),
                expected_revision=4,
                idempotency_key="s008-preview-block-key",
                action="undo",
            )
            raise AssertionError("undo unexpectedly discarded a pending preview")
        except ActiveDesignApiError as exc:
            assert exc.code == "ACTIVE_DESIGN_PREVIEW_PENDING"
        with SQLiteUnitOfWork(factory) as unit:
            unit.active_designs.set_preview(
                project_id="prj_s008_navigation",
                expected_revision=4,
                change_set_id=None,
                base_asset_version_id=None,
                updated_at=NOW,
            )

        # A quality write and a selection write both consume the same Snapshot
        # revision. Navigation with the prior revision must be stale, not
        # overwrite either write.
        assets = AgentAssetEditingService(factory)
        quality = assets.quality(redo_version_id)
        assert quality.asset_version_id == redo_version_id
        try:
            service.navigate_asset(
                "prj_s008_navigation",
                NavigateActiveDesignRequest(client_request_id="s008-quality-race", snapshot_revision=5),
                expected_revision=5,
                idempotency_key="s008-quality-race-key",
                action="undo",
            )
            raise AssertionError("navigation unexpectedly overwrote a quality write")
        except ActiveDesignApiError as exc:
            assert exc.code == "ACTIVE_DESIGN_STALE"
        after_quality_undo = service.navigate_asset(
            "prj_s008_navigation",
            NavigateActiveDesignRequest(client_request_id="s008-quality-undo", snapshot_revision=6),
            expected_revision=6,
            idempotency_key="s008-quality-undo-key",
            action="undo",
        )
        assert after_quality_undo.revision == 7 and after_quality_undo.quality is None
        after_quality_redo = service.navigate_asset(
            "prj_s008_navigation",
            NavigateActiveDesignRequest(client_request_id="s008-quality-redo", snapshot_revision=7),
            expected_revision=7,
            idempotency_key="s008-quality-redo-key",
            action="redo",
        )
        assert after_quality_redo.revision == 8
        selected = service.select_part(
            "prj_s008_navigation",
            SelectActiveDesignRequest(client_request_id="s008-select", snapshot_revision=8, selected_part_id="part_s008_body"),
            expected_revision=8,
            idempotency_key="s008-select-key",
        )
        assert selected.revision == 9 and selected.selected_part_id == "part_s008_body"
        try:
            service.navigate_asset(
                "prj_s008_navigation",
                NavigateActiveDesignRequest(client_request_id="s008-selection-race", snapshot_revision=8),
                expected_revision=8,
                idempotency_key="s008-selection-race-key",
                action="undo",
            )
            raise AssertionError("navigation unexpectedly overwrote a selection write")
        except ActiveDesignApiError as exc:
            assert exc.code == "ACTIVE_DESIGN_STALE"
        after_selection_undo = service.navigate_asset(
            "prj_s008_navigation",
            NavigateActiveDesignRequest(client_request_id="s008-selection-undo", snapshot_revision=9),
            expected_revision=9,
            idempotency_key="s008-selection-undo-key",
            action="undo",
        )
        assert (
            after_selection_undo.revision == 10
            and after_selection_undo.selected_part_id == "part_s008_body"
            and after_selection_undo.selected_material_zone_id is None
        )

    print("S008 immutable Agent undo/redo, Snapshot CAS and idempotency smoke passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
