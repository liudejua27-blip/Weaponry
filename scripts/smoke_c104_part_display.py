#!/usr/bin/env python3
"""C104 smoke: server-owned part locks, visibility, isolation and version cleanup."""

from __future__ import annotations

import asyncio
import json
import os
import tempfile
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
NOW = "2026-07-13T00:00:00+00:00"
PROJECT_ID = "prj_c104_part_display"
PART_BODY = "part_c104_body"
PART_WING = "part_c104_wing"


def _canonical(value: object) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


async def _request(
    app: Any,
    method: str,
    path: str,
    *,
    payload: dict[str, object] | None = None,
    headers: dict[str, str] | None = None,
) -> tuple[int, dict[str, str], dict[str, object]]:
    body = _canonical(payload).encode("utf-8") if payload is not None else b""
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


def _error_code(payload: dict[str, object]) -> str:
    error = payload.get("error")
    return str(error.get("code")) if isinstance(error, dict) else ""


def _seed_project(factory: Any) -> None:
    connection = factory.connect()
    try:
        connection.execute(
            """
            INSERT INTO domain_profiles(profile_id, domain_type, schema_version, pack_id, display_name, profile_json, profile_sha256, status, created_at, updated_at)
            VALUES ('profile_c104', 'weapon_concept', 'DesignDomainProfile@1', 'pack_weapon_concept', 'C104 fixture', '{}', ?, 'active', ?, ?)
            """,
            ("0" * 64, NOW, NOW),
        )
        connection.execute(
            """
            INSERT INTO projects(project_id, profile_id, domain_type, name, status, current_version_id, created_at, updated_at)
            VALUES (?, 'profile_c104', 'weapon_concept', 'C104 part display', 'active', NULL, ?, ?)
            """,
            (PROJECT_ID, NOW, NOW),
        )
        connection.commit()
    finally:
        connection.close()


def _parts(part_ids: tuple[str, str] = (PART_BODY, PART_WING)) -> list[dict[str, object]]:
    body, wing = part_ids
    return [
        {
            "part_id": body,
            "role": "primary_body",
            "parent_part_id": None,
            "position_mm": [0, 0, 0],
            "size_mm": [120, 40, 36],
            "material_zone_ids": ["zone_body"],
            "editable_parameters": ["transform.scale.x"],
            "locked": False,
            "provenance": "agent_generated",
        },
        {
            "part_id": wing,
            "role": "side_panel",
            "parent_part_id": body,
            "position_mm": [0, 10, 30],
            "size_mm": [50, 10, 18],
            "material_zone_ids": ["zone_wing"],
            "editable_parameters": ["transform.scale.x"],
            "locked": False,
            "provenance": "agent_generated",
        },
    ]


def _add_version(unit: Any, *, asset_version_id: str, version_no: int, parent: str | None, part_ids: tuple[str, str] = (PART_BODY, PART_WING)) -> str:
    body, wing = part_ids
    graph_id = f"mg_{asset_version_id.removeprefix('assetver_')}"
    graph = {
        "schema_version": "AssemblyGraph@1",
        "graph_id": graph_id,
        "parts": [
            {"part_id": body, "parent_part_id": None, "material_zones": ["zone_body"], "connectors": [], "joints": []},
            {"part_id": wing, "parent_part_id": body, "material_zones": ["zone_wing"], "connectors": [], "joints": []},
        ],
        "connections": [],
    }
    unit.agent_assets.add_version(
        asset_version_id=asset_version_id,
        project_id=PROJECT_ID,
        parent_asset_version_id=parent,
        version_no=version_no,
        status="committed",
        summary=f"C104 v{version_no}",
        stage="editable_asset",
        plan_id="plan_c104",
        direction_id="direction_c104",
        domain_pack_id="pack_vehicle_concept",
        artifact_id=f"artifact_{asset_version_id}",
        parts_json=_canonical(_parts(part_ids)),
        shape_program_json=_canonical(
            {
                "schema_version": "ShapeProgram@1",
                "program_id": f"shape_{asset_version_id}",
                "units": "millimeter",
                "seed": version_no,
                "triangle_budget": 1000,
                "parameters": [],
                "operations": [
                    {
                        "operation_id": f"op_{asset_version_id}",
                        "op": "box",
                        "inputs": [],
                        "args": {
                            "position": [0, 0, 0],
                            "size": [100, 40, 30],
                            "part_role": "primary_body",
                        },
                    }
                ],
                "outputs": [
                    {
                        "output_id": f"output_{asset_version_id}",
                        "operation_id": f"op_{asset_version_id}",
                        "kind": "mesh",
                        "part_role": "primary_body",
                    }
                ],
                "non_functional_only": True,
            }
        ),
        assembly_graph_json=_canonical(graph),
        material_bindings_json="{}",
        created_at=NOW,
    )
    return graph_id


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad-c104-") as raw:
        root = Path(raw)
        os.environ["WUSHEN_LIBRARY_ROOT"] = str(root / "library")
        os.environ["WUSHEN_MIGRATIONS_DIR"] = str(ROOT / "migrations")
        os.environ["WUSHEN_LOCAL_WORKER_ENABLED"] = "0"
        os.environ["FORGECAD_CONCEPT_WORKER_ENABLED"] = "0"

        from forgecad_agent.application.agent_asset_editing import AgentAssetEditingService, AgentAssetError
        from forgecad_agent.application.agent_models import AgentPartEditOperation, ProposeAgentAssetChangeSetRequest
        from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteUnitOfWork
        from wushen_agent.main import create_app

        app = create_app()
        factory = SQLiteConnectionFactory(root / "library" / "library.db")
        _seed_project(factory)
        with SQLiteUnitOfWork(factory) as unit:
            graph_id = _add_version(unit, asset_version_id="assetver_c104_v1", version_no=1, parent=None)
            unit.agent_assets.set_head(project_id=PROJECT_ID, asset_version_id="assetver_c104_v1", updated_at=NOW)
            unit.active_designs.create_agent_snapshot(
                project_id=PROJECT_ID,
                asset_version_id="assetver_c104_v1",
                assembly_graph_id=graph_id,
                updated_at=NOW,
            )

        status, headers, snapshot = asyncio.run(_request(app, "GET", f"/api/v1/projects/{PROJECT_ID}/active-design"))
        assert status == 200 and headers["etag"] == 'W/"active-design-1"'
        assert snapshot["part_display"]["locked_part_ids"] == []

        lock = {"client_request_id": "c104-lock", "snapshot_revision": 1, "action": "lock", "part_id": PART_WING}
        lock_headers = {"Idempotency-Key": "c104-lock-key", "If-Match": 'W/"active-design-1"'}
        status, headers, locked = asyncio.run(_request(app, "POST", f"/api/v1/projects/{PROJECT_ID}/active-design:part-display", payload=lock, headers=lock_headers))
        assert status == 200 and headers["etag"] == 'W/"active-design-2"'
        assert locked["part_display"]["locked_part_ids"] == [PART_WING]
        status, replay_headers, replay = asyncio.run(_request(app, "POST", f"/api/v1/projects/{PROJECT_ID}/active-design:part-display", payload=lock, headers=lock_headers))
        assert status == 200 and replay_headers["etag"] == 'W/"active-design-2"' and replay == locked

        assets = AgentAssetEditingService(factory)
        try:
            assets.propose_change_set(
                "assetver_c104_v1",
                ProposeAgentAssetChangeSetRequest(
                    client_request_id="c104-locked-edit-request",
                    summary="attempt locked edit",
                    operations=[AgentPartEditOperation(operation_id="op_c104_locked", op="set_part_parameter", part_id=PART_WING, path="transform.scale.x", value=1.2)],
                ),
                "c104-locked-edit",
            )
            raise AssertionError("locked Agent part unexpectedly accepted a ChangeSet")
        except AgentAssetError as exc:
            assert exc.code == "PART_PROTECTED"

        select = {"client_request_id": "c104-select", "snapshot_revision": 2, "selected_part_id": PART_WING, "selected_material_zone_id": "zone_wing"}
        status, _, selected = asyncio.run(_request(app, "POST", f"/api/v1/projects/{PROJECT_ID}/active-design:select", payload=select, headers={"Idempotency-Key": "c104-select-key", "If-Match": 'W/"active-design-2"'}))
        assert status == 200 and selected["selected_part_id"] == PART_WING
        hide = {"client_request_id": "c104-hide", "snapshot_revision": 3, "action": "hide", "part_id": PART_WING}
        status, _, hidden = asyncio.run(_request(app, "POST", f"/api/v1/projects/{PROJECT_ID}/active-design:part-display", payload=hide, headers={"Idempotency-Key": "c104-hide-key", "If-Match": 'W/"active-design-3"'}))
        assert status == 200 and hidden["selected_part_id"] is None and hidden["part_display"]["hidden_part_ids"] == [PART_WING]
        blocked_select = {"client_request_id": "c104-hidden-select", "snapshot_revision": 4, "selected_part_id": PART_WING}
        status, _, blocked = asyncio.run(_request(app, "POST", f"/api/v1/projects/{PROJECT_ID}/active-design:select", payload=blocked_select, headers={"Idempotency-Key": "c104-hidden-select-key", "If-Match": 'W/"active-design-4"'}))
        assert status == 409 and _error_code(blocked) == "ACTIVE_DESIGN_INVALID"

        isolate = {"client_request_id": "c104-isolate", "snapshot_revision": 4, "action": "isolate", "part_id": PART_BODY}
        status, _, isolated = asyncio.run(_request(app, "POST", f"/api/v1/projects/{PROJECT_ID}/active-design:part-display", payload=isolate, headers={"Idempotency-Key": "c104-isolate-key", "If-Match": 'W/"active-design-4"'}))
        assert status == 200 and isolated["part_display"]["isolated_part_id"] == PART_BODY
        show_all = {"client_request_id": "c104-show-all", "snapshot_revision": 5, "action": "show_all"}
        status, _, visible = asyncio.run(_request(app, "POST", f"/api/v1/projects/{PROJECT_ID}/active-design:part-display", payload=show_all, headers={"Idempotency-Key": "c104-show-all-key", "If-Match": 'W/"active-design-5"'}))
        assert status == 200 and visible["part_display"]["hidden_part_ids"] == [] and visible["part_display"]["isolated_part_id"] is None

        with SQLiteUnitOfWork(factory) as unit:
            graph_id_v2 = _add_version(unit, asset_version_id="assetver_c104_v2", version_no=2, parent="assetver_c104_v1")
            unit.agent_assets.supersede("assetver_c104_v1")
            unit.agent_assets.set_head(project_id=PROJECT_ID, asset_version_id="assetver_c104_v2", updated_at=NOW)
            advanced = unit.active_designs.advance_agent_snapshot(
                project_id=PROJECT_ID,
                expected_revision=6,
                asset_version_id="assetver_c104_v2",
                assembly_graph_id=graph_id_v2,
                updated_at=NOW,
            )
            assert advanced.part_display is not None and advanced.part_display.asset_version_id == "assetver_c104_v2"
            assert advanced.part_display.locked_part_ids == [PART_WING]
        status, _, restored = asyncio.run(_request(app, "GET", f"/api/v1/projects/{PROJECT_ID}/active-design"))
        assert status == 200 and restored["part_display"]["asset_version_id"] == "assetver_c104_v2"
        assert restored["part_display"]["locked_part_ids"] == [PART_WING]

    print("C104 part display smoke passed: lock protection, hide/isolate, idempotency, selection guard and version-state normalization")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
