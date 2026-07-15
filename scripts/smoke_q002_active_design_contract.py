#!/usr/bin/env python3
"""Q002 smoke: Snapshot bootstrap compatibility and idempotent quality CAS."""

from __future__ import annotations

import asyncio
import json
import os
import tempfile
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
NOW = "2026-07-13T00:00:00+00:00"


async def request(
    app: object,
    method: str,
    path: str,
    *,
    headers: dict[str, str] | None = None,
) -> tuple[int, dict[str, str], dict[str, Any]]:
    outgoing: list[dict[str, Any]] = []

    async def receive() -> dict[str, Any]:
        return {"type": "http.disconnect"}

    async def send(message: dict[str, Any]) -> None:
        outgoing.append(message)

    raw_headers = [(b"host", b"testserver")]
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
    body = b"".join(message.get("body", b"") for message in outgoing if message["type"] == "http.response.body")
    response_headers = {key.decode("latin-1").lower(): value.decode("latin-1") for key, value in start.get("headers", [])}
    try:
        decoded_body = json.loads(body.decode("utf-8")) if body else {}
    except json.JSONDecodeError:
        # Starlette's CORS preflight body is the plain-text `OK` response.
        decoded_body = {}
    return int(start["status"]), response_headers, decoded_body


def seed_project(factory: Any, project_id: str) -> None:
    connection = factory.connect()
    try:
        connection.execute(
            """
            INSERT INTO domain_profiles(
              profile_id, domain_type, schema_version, pack_id, display_name,
              profile_json, profile_sha256, status, created_at, updated_at
            ) VALUES (?, 'weapon_concept', 'DesignDomainProfile@1', 'pack_future_weapon_prop',
                      'Q002 fixture', '{}', ?, 'active', ?, ?)
            """,
            (f"profile_{project_id}", "0" * 64, NOW, NOW),
        )
        connection.execute(
            """
            INSERT INTO projects(project_id, profile_id, domain_type, name, status, current_version_id, created_at, updated_at)
            VALUES (?, ?, 'weapon_concept', ?, 'active', NULL, ?, ?)
            """,
            (project_id, f"profile_{project_id}", project_id, NOW, NOW),
        )
        connection.commit()
    finally:
        connection.close()


def seed_agent_head(factory: Any, project_id: str, asset_version_id: str) -> None:
    from forgecad_agent.infrastructure.db import SQLiteUnitOfWork

    graph_id = f"mg_{project_id}"
    graph = {
        "schema_version": "AssemblyGraph@1",
        "graph_id": graph_id,
        "parts": [{
            "part_id": "part_q002_body",
            "role": "primary_body",
            "transform": {"position": [0, 0, 0], "rotation": [0, 0, 0], "scale": [1, 1, 1]},
            "connectors": [],
            "joints": [],
        }],
        "connections": [],
    }
    with SQLiteUnitOfWork(factory) as unit:
        unit.agent_assets.add_version(
            asset_version_id=asset_version_id,
            project_id=project_id,
            parent_asset_version_id=None,
            version_no=1,
            status="committed",
            summary="Q002 active asset",
            stage="editable_asset",
            plan_id="plan_q002",
            direction_id="direction_q002",
            domain_pack_id="pack_future_weapon_prop",
            artifact_id="artifact_q002",
            parts_json='[{"part_id":"part_q002_body","role":"primary_body","parent_part_id":null,"position_mm":[0,0,0],"size_mm":[100,40,30],"material_zone_ids":["zone_body"],"editable_parameters":[],"locked":false,"provenance":"agent_generated"}]',
            shape_program_json='{"schema_version":"ShapeProgram@1","program_id":"shape_q002","units":"millimeter","seed":2,"triangle_budget":1000,"parameters":[],"operations":[{"operation_id":"op_box_q002","op":"box","inputs":[],"args":{"position":[0,0,0],"size":[100,40,30],"part_role":"primary_body"}}],"outputs":[{"output_id":"output_q002","operation_id":"op_box_q002","kind":"mesh","part_role":"primary_body"}],"non_functional_only":true}',
            assembly_graph_json=json.dumps(graph, sort_keys=True, separators=(",", ":")),
            material_bindings_json="{}",
            created_at=NOW,
        )
        unit.agent_assets.set_head(project_id=project_id, asset_version_id=asset_version_id, updated_at=NOW)


def seed_legacy_current(factory: Any, project_id: str) -> None:
    connection = factory.connect()
    try:
        graph_id = f"mg_{project_id}"
        version_id = f"ver_{project_id}"
        connection.execute(
            """
            INSERT INTO module_graphs(graph_id, project_id, version_id, root_node_id, schema_version,
                                      graph_json, graph_sha256, validation_status, created_at, updated_at)
            VALUES (?, ?, NULL, 'node_root', 'ModuleGraph@1', ?, ?, 'valid', ?, ?)
            """,
            (graph_id, project_id, '{"schema_version":"ModuleGraph@1"}', "1" * 64, NOW, NOW),
        )
        connection.execute(
            """
            INSERT INTO project_versions(version_id, project_id, parent_version_id, version_no, status,
                                         summary, spec_schema_version, spec_json, spec_sha256,
                                         module_graph_id, change_set_id, created_at)
            VALUES (?, ?, NULL, 1, 'committed', 'Q002 legacy fixture', 'WeaponConceptSpec@1', '{}', ?, ?, NULL, ?)
            """,
            (version_id, project_id, "2" * 64, graph_id, NOW),
        )
        connection.execute("UPDATE projects SET current_version_id = ? WHERE project_id = ?", (version_id, project_id))
        connection.commit()
    finally:
        connection.close()


def error_code(body: dict[str, Any]) -> str:
    return str((body.get("error") or {}).get("code"))


def count_rows(factory: Any, table: str, project_id: str) -> int:
    connection = factory.connect()
    try:
        return int(connection.execute(f"SELECT COUNT(*) FROM {table} WHERE project_id = ?", (project_id,)).fetchone()[0])
    finally:
        connection.close()


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad-q002-") as raw:
        root = Path(raw)
        os.environ["WUSHEN_LIBRARY_ROOT"] = str(root / "library")
        os.environ["WUSHEN_MIGRATIONS_DIR"] = str(ROOT / "migrations")
        os.environ["WUSHEN_LOCAL_WORKER_ENABLED"] = "0"
        os.environ["FORGECAD_CONCEPT_WORKER_ENABLED"] = "0"

        from forgecad_agent.infrastructure.db import SQLiteConnectionFactory
        from wushen_agent.main import create_app

        app = create_app()
        factory = SQLiteConnectionFactory(root / "library" / "library.db")
        seed_project(factory, "prj_q002_empty")
        seed_project(factory, "prj_q002_agent")
        seed_project(factory, "prj_q002_legacy")
        seed_agent_head(factory, "prj_q002_agent", "assetver_q002_agent")
        seed_legacy_current(factory, "prj_q002_legacy")

        status, _, body = asyncio.run(request(app, "GET", "/api/v1/projects/prj_q002_empty/active-design"))
        assert status == 404 and error_code(body) == "ACTIVE_DESIGN_NOT_FOUND"
        assert count_rows(factory, "active_design_snapshots", "prj_q002_empty") == 0

        status, headers, agent_snapshot = asyncio.run(request(app, "GET", "/api/v1/projects/prj_q002_agent/active-design"))
        assert status == 200 and headers["etag"] == 'W/"active-design-1"' and headers["cache-control"] == "no-store"
        assert agent_snapshot["active_design"]["asset_version_id"] == "assetver_q002_agent"
        assert count_rows(factory, "active_design_snapshots", "prj_q002_agent") == 1
        status, cors_headers, _ = asyncio.run(request(
            app,
            "GET",
            "/api/v1/projects/prj_q002_agent/active-design",
            headers={"Origin": "http://127.0.0.1:5173"},
        ))
        assert status == 200 and "etag" in cors_headers["access-control-expose-headers"].lower()
        status, cors_headers, _ = asyncio.run(request(
            app,
            "OPTIONS",
            quality_path := "/api/v1/agent/asset-versions/assetver_q002_agent:quality",
            headers={
                "Origin": "http://127.0.0.1:5173",
                "Access-Control-Request-Method": "POST",
                "Access-Control-Request-Headers": "if-match,idempotency-key",
            },
        ))
        assert status == 200 and "if-match" in cors_headers["access-control-allow-headers"].lower()
        status, repeat_headers, repeated = asyncio.run(request(app, "GET", "/api/v1/projects/prj_q002_agent/active-design"))
        assert status == 200 and repeat_headers["etag"] == headers["etag"] and repeated == agent_snapshot
        assert count_rows(factory, "active_design_snapshots", "prj_q002_agent") == 1

        status, navigation_headers, navigation = asyncio.run(request(app, "GET", "/api/v1/projects/prj_q002_agent/active-design:navigation"))
        assert status == 200 and navigation_headers["cache-control"] == "no-store" and "etag" not in navigation_headers
        assert navigation["active_asset_version_id"] == "assetver_q002_agent"

        status, legacy_headers, legacy_snapshot = asyncio.run(request(app, "GET", "/api/v1/projects/prj_q002_legacy/active-design"))
        assert status == 200 and legacy_headers["cache-control"] == "no-store"
        assert legacy_snapshot["active_design"]["source"] == "legacy_concept_read_only"
        assert count_rows(factory, "active_design_snapshots", "prj_q002_legacy") == 1

        status, _, body = asyncio.run(request(app, "POST", quality_path, headers={"If-Match": headers["etag"]}))
        assert status == 400 and error_code(body) == "IDEMPOTENCY_KEY_REQUIRED"
        status, _, body = asyncio.run(request(app, "POST", quality_path, headers={"Idempotency-Key": "q002-missing-revision"}))
        assert status == 400 and error_code(body) == "ACTIVE_DESIGN_REVISION_REQUIRED"

        quality_headers = {"If-Match": headers["etag"], "Idempotency-Key": "q002-quality"}
        status, _, quality = asyncio.run(request(app, "POST", quality_path, headers=quality_headers))
        assert status == 200 and quality["asset_version_id"] == "assetver_q002_agent"
        assert count_rows(factory, "agent_asset_quality_reports", "prj_q002_agent") == 1
        status, _, replay = asyncio.run(request(app, "POST", quality_path, headers=quality_headers))
        assert status == 200 and replay == quality
        assert count_rows(factory, "agent_asset_quality_reports", "prj_q002_agent") == 1

        status, _, body = asyncio.run(request(app, "POST", quality_path, headers={"If-Match": 'W/"active-design-2"', "Idempotency-Key": "q002-quality"}))
        assert status == 409 and error_code(body) == "IDEMPOTENCY_CONFLICT"
        status, _, body = asyncio.run(request(app, "POST", quality_path, headers={"If-Match": headers["etag"], "Idempotency-Key": "q002-stale"}))
        assert status == 409 and error_code(body) == "ACTIVE_DESIGN_STALE"
        assert count_rows(factory, "agent_asset_quality_reports", "prj_q002_agent") == 1

    print("Q002 active-design contract smoke passed: bootstrap compatibility, no-store navigation, quality replay/conflict and stale CAS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
