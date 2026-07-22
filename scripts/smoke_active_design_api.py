#!/usr/bin/env python3
"""S003 API smoke for the server-owned ActiveDesignSnapshot boundary."""

from __future__ import annotations

import asyncio
import hashlib
import json
import os
import tempfile
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
NOW = "2026-07-13T00:00:00+00:00"


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
    request_messages = [{"type": "http.request", "body": body, "more_body": False}]
    response_messages: list[dict[str, Any]] = []

    async def receive() -> dict[str, Any]:
        if request_messages:
            return request_messages.pop(0)
        return {"type": "http.disconnect"}

    async def send(message: dict[str, Any]) -> None:
        response_messages.append(message)

    raw_headers = [(b"host", b"testserver")]
    if payload is not None:
        raw_headers.append((b"content-type", b"application/json"))
    for key, value in (headers or {}).items():
        raw_headers.append((key.lower().encode("latin-1"), value.encode("latin-1")))
    await app(
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
    start = next(
        message
        for message in response_messages
        if message["type"] == "http.response.start"
    )
    response_body = b"".join(
        message.get("body", b"")
        for message in response_messages
        if message["type"] == "http.response.body"
    )
    response_headers = {
        key.decode("latin-1").lower(): value.decode("latin-1")
        for key, value in start.get("headers", [])
    }
    return (
        int(start["status"]),
        response_headers,
        json.loads(response_body.decode("utf-8")),
    )


def _seed_project(factory: Any, project_id: str) -> None:
    connection = factory.connect()
    try:
        connection.execute(
            """
            INSERT OR IGNORE INTO domain_profiles(
              profile_id, domain_type, schema_version, pack_id, display_name,
              profile_json, profile_sha256, status, created_at, updated_at
            ) VALUES ('profile_s003', 'weapon_concept', 'DesignDomainProfile@1',
                      'pack_weapon_concept', 'S003 fixture', '{}', ?, 'active', ?, ?)
            """,
            ("0" * 64, NOW, NOW),
        )
        connection.execute(
            """
            INSERT INTO projects(
              project_id, profile_id, domain_type, name, status,
              current_version_id, created_at, updated_at
            ) VALUES (?, 'profile_s003', 'weapon_concept', ?, 'active', NULL, ?, ?)
            """,
            (project_id, f"S003 {project_id}", NOW, NOW),
        )
        connection.commit()
    finally:
        connection.close()


def _seed_agent_head(
    factory: Any, project_id: str, asset_version_id: str, graph_id: str, part_id: str
) -> None:
    from forgecad_agent.infrastructure.db import SQLiteUnitOfWork

    graph = {
        "schema_version": "AssemblyGraph@1",
        "graph_id": graph_id,
        "parts": [{"part_id": part_id}],
        "connections": [],
    }
    with SQLiteUnitOfWork(factory) as unit:
        unit.agent_assets.add_version(
            asset_version_id=asset_version_id,
            project_id=project_id,
            parent_asset_version_id=None,
            version_no=1,
            status="committed",
            summary="S003 fixture asset",
            stage="editable_asset",
            plan_id="plan_s003",
            direction_id="direction_s003",
            domain_pack_id="pack_vehicle_concept",
            artifact_id=f"artifact_{asset_version_id}",
            parts_json=_canonical([{"part_id": part_id}]),
            shape_program_json='{"schema_version":"ShapeProgram@1","operations":[]}',
            assembly_graph_json=_canonical(graph),
            material_bindings_json="{}",
            created_at=NOW,
        )
        unit.agent_assets.set_head(
            project_id=project_id, asset_version_id=asset_version_id, updated_at=NOW
        )


def _seed_legacy_current(factory: Any, project_id: str) -> tuple[str, str]:
    graph_id = "mg_s003_legacy"
    version_id = "ver_s003_legacy"
    connection = factory.connect()
    try:
        connection.execute(
            """
            INSERT INTO module_graphs(
              graph_id, project_id, version_id, root_node_id, schema_version,
              graph_json, graph_sha256, validation_status, created_at, updated_at
            ) VALUES (?, ?, NULL, 'node_root', 'ModuleGraph@1', ?, ?, 'valid', ?, ?)
            """,
            (
                graph_id,
                project_id,
                '{"schema_version":"ModuleGraph@1"}',
                "1" * 64,
                NOW,
                NOW,
            ),
        )
        connection.execute(
            """
            INSERT INTO project_versions(
              version_id, project_id, parent_version_id, version_no, status,
              summary, spec_schema_version, spec_json, spec_sha256,
              module_graph_id, change_set_id, created_at
            ) VALUES (?, ?, NULL, 1, 'committed', 'S003 legacy fixture', 'WeaponConceptSpec@1', '{}', ?, ?, NULL, ?)
            """,
            (version_id, project_id, "2" * 64, graph_id, NOW),
        )
        connection.execute(
            "UPDATE projects SET current_version_id = ?, updated_at = ? WHERE project_id = ?",
            (version_id, NOW, project_id),
        )
        connection.commit()
    finally:
        connection.close()
    return version_id, graph_id


def _legacy_hash(factory: Any, project_id: str) -> str:
    connection = factory.connect()
    try:
        payload = {
            table: [
                dict(row)
                for row in connection.execute(
                    f"SELECT * FROM {table} WHERE project_id = ?", (project_id,)
                )
            ]
            for table in ("projects", "project_versions", "module_graphs")
        }
    finally:
        connection.close()
    return hashlib.sha256(_canonical(payload).encode("utf-8")).hexdigest()


def _error_code(response: dict[str, object]) -> str:
    return str(
        ((response.get("error") or {}) if isinstance(response, dict) else {}).get(
            "code"
        )
    )


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad-s003-api-") as raw:
        root = Path(raw)
        os.environ["WUSHEN_LIBRARY_ROOT"] = str(root / "library")
        os.environ["WUSHEN_MIGRATIONS_DIR"] = str(ROOT / "migrations")
        os.environ["WUSHEN_LOCAL_WORKER_ENABLED"] = "0"
        os.environ["FORGECAD_CONCEPT_WORKER_ENABLED"] = "0"
        os.environ["FORGECAD_TEST_ONLY_LEGACY_AGENT_LIFECYCLE"] = "1"
        os.environ["FORGECAD_TEST_ONLY_LEGACY_PRODUCT_CORE"] = "1"
        os.environ["FORGECAD_K001_PACKAGED_PROBE"] = "1"

        from forgecad_agent.infrastructure.db import SQLiteConnectionFactory
        from wushen_agent.main import create_test_only_legacy_product_core_app

        app = create_test_only_legacy_product_core_app()
        factory = SQLiteConnectionFactory(root / "library" / "library.db")
        _seed_project(factory, "prj_s003_agent")
        _seed_project(factory, "prj_s003_other")
        _seed_project(factory, "prj_s003_legacy")
        _seed_agent_head(
            factory,
            "prj_s003_agent",
            "assetver_s003_agent",
            "mg_s003_agent",
            "part_s003_body",
        )
        _seed_agent_head(
            factory,
            "prj_s003_other",
            "assetver_s003_other",
            "mg_s003_other",
            "part_s003_other",
        )
        legacy_version_id, legacy_graph_id = _seed_legacy_current(
            factory, "prj_s003_legacy"
        )
        legacy_before = _legacy_hash(factory, "prj_s003_legacy")

        status, headers, snapshot = asyncio.run(
            _request(app, "GET", "/api/v1/projects/prj_s003_agent/active-design")
        )
        assert status == 200 and headers["etag"] == 'W/"active-design-1"'
        assert snapshot["active_design"]["asset_version_id"] == "assetver_s003_agent"
        status, headers, repeat = asyncio.run(
            _request(app, "GET", "/api/v1/projects/prj_s003_agent/active-design")
        )
        assert (
            status == 200
            and headers["etag"] == 'W/"active-design-1"'
            and repeat == snapshot
        )

        selection = {
            "client_request_id": "s003-select-1",
            "snapshot_revision": 1,
            "selected_part_id": "part_s003_body",
        }
        selection_headers = {"Idempotency-Key": "s003-select-key"}
        status, headers, selected = asyncio.run(
            _request(
                app,
                "POST",
                "/api/v1/projects/prj_s003_agent/active-design:select",
                payload=selection,
                headers=selection_headers,
            )
        )
        assert status == 200 and headers["etag"] == 'W/"active-design-2"'
        assert (
            selected["selected_part_id"] == "part_s003_body"
            and selected["revision"] == 2
        )
        status, headers, replay = asyncio.run(
            _request(
                app,
                "POST",
                "/api/v1/projects/prj_s003_agent/active-design:select",
                payload=selection,
                headers=selection_headers,
            )
        )
        assert (
            status == 200
            and headers["etag"] == 'W/"active-design-2"'
            and replay == selected
        )

        stale_request = {
            "client_request_id": "s003-select-stale",
            "snapshot_revision": 1,
            "selected_part_id": None,
        }
        status, _, error = asyncio.run(
            _request(
                app,
                "POST",
                "/api/v1/projects/prj_s003_agent/active-design:select",
                payload=stale_request,
                headers={"Idempotency-Key": "s003-stale-key"},
            )
        )
        assert status == 409 and _error_code(error) == "ACTIVE_DESIGN_STALE"

        etag_request = {
            "client_request_id": "s003-select-etag",
            "selected_part_id": None,
        }
        status, headers, cleared = asyncio.run(
            _request(
                app,
                "POST",
                "/api/v1/projects/prj_s003_agent/active-design:select",
                payload=etag_request,
                headers={
                    "Idempotency-Key": "s003-etag-key",
                    "If-Match": 'W/"active-design-2"',
                },
            )
        )
        assert (
            status == 200
            and headers["etag"] == 'W/"active-design-3"'
            and cleared["selected_part_id"] is None
        )

        cross_project_request = {
            "client_request_id": "s003-cross-project",
            "snapshot_revision": 3,
            "selected_part_id": "part_s003_other",
        }
        status, _, error = asyncio.run(
            _request(
                app,
                "POST",
                "/api/v1/projects/prj_s003_agent/active-design:select",
                payload=cross_project_request,
                headers={"Idempotency-Key": "s003-cross-key"},
            )
        )
        assert status == 409 and _error_code(error) == "ACTIVE_DESIGN_INVALID"

        status, headers, legacy = asyncio.run(
            _request(app, "GET", "/api/v1/projects/prj_s003_legacy/active-design")
        )
        assert status == 200 and headers["etag"] == 'W/"active-design-1"'
        assert legacy["active_design"] == {
            "source": "legacy_concept_read_only",
            "project_id": "prj_s003_legacy",
            "legacy_version_id": legacy_version_id,
            "module_graph_id": legacy_graph_id,
        }
        status, _, error = asyncio.run(
            _request(
                app,
                "POST",
                "/api/v1/projects/prj_s003_legacy/active-design:select",
                payload={
                    "client_request_id": "s003-legacy-select",
                    "snapshot_revision": 1,
                    "selected_part_id": None,
                },
                headers={"Idempotency-Key": "s003-legacy-select-key"},
            )
        )
        assert status == 409 and _error_code(error) == "ACTIVE_DESIGN_LEGACY_READ_ONLY"

        conversion_request = {
            "client_request_id": "s003-convert",
            "snapshot_revision": 1,
        }
        conversion_headers = {"Idempotency-Key": "s003-convert-key"}
        status, headers, conversion = asyncio.run(
            _request(
                app,
                "POST",
                "/api/v1/projects/prj_s003_legacy/active-design:convert-legacy",
                payload=conversion_request,
                headers=conversion_headers,
            )
        )
        assert status == 200 and headers["etag"] == 'W/"active-design-1"'
        assert (
            conversion["status"] == "ready_for_agent_rebuild"
            and conversion["source"] == legacy["active_design"]
        )
        status, _, conversion_replay = asyncio.run(
            _request(
                app,
                "POST",
                "/api/v1/projects/prj_s003_legacy/active-design:convert-legacy",
                payload=conversion_request,
                headers=conversion_headers,
            )
        )
        assert status == 200 and conversion_replay == conversion
        assert _legacy_hash(factory, "prj_s003_legacy") == legacy_before

        status, _, error = asyncio.run(
            _request(
                app,
                "POST",
                "/api/v1/projects/prj_s003_agent/active-design:convert-legacy",
                payload={
                    "client_request_id": "s003-not-legacy",
                    "snapshot_revision": 3,
                },
                headers={"Idempotency-Key": "s003-not-legacy-key"},
            )
        )
        assert status == 409 and _error_code(error) == "ACTIVE_DESIGN_NOT_LEGACY"

    print("S003 ActiveDesignSnapshot API/idempotency/ETag/legacy smoke passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
