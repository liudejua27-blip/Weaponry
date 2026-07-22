#!/usr/bin/env python3
"""FGC-M107: Snapshot-owned material-zone selection, CAS and navigation smoke."""

from __future__ import annotations

import asyncio
import json
import os
import tempfile
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
NOW = "2026-07-13T00:00:00+00:00"
PROJECT = "prj_m107_zone"


async def _request(
    app: Any,
    method: str,
    path: str,
    *,
    payload: dict[str, object],
    headers: dict[str, str],
) -> tuple[int, dict[str, str], dict[str, object]]:
    body = json.dumps(
        payload, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")
    incoming = [{"type": "http.request", "body": body, "more_body": False}]
    outgoing: list[dict[str, object]] = []

    async def receive() -> dict[str, object]:
        return incoming.pop(0) if incoming else {"type": "http.disconnect"}

    async def send(message: dict[str, object]) -> None:
        outgoing.append(message)

    raw_headers = [(b"host", b"testserver"), (b"content-type", b"application/json")]
    raw_headers.extend(
        (key.lower().encode("latin-1"), value.encode("latin-1"))
        for key, value in headers.items()
    )
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
    start = next(item for item in outgoing if item["type"] == "http.response.start")
    response_body = b"".join(
        item.get("body", b"")
        for item in outgoing
        if item["type"] == "http.response.body"
    )
    response_headers = {
        key.decode("latin-1").lower(): value.decode("latin-1")
        for key, value in start.get("headers", [])
    }  # type: ignore[union-attr]
    return (
        int(start["status"]),
        response_headers,
        json.loads(response_body.decode("utf-8")),
    )


def _seed(factory: Any) -> None:
    connection = factory.connect()
    try:
        connection.execute(
            "INSERT INTO domain_profiles(profile_id, domain_type, schema_version, pack_id, display_name, profile_json, profile_sha256, status, created_at, updated_at) VALUES ('profile_m107', 'weapon_concept', 'DesignDomainProfile@1', 'pack_vehicle_concept', 'M107 fixture', '{}', ?, 'active', ?, ?)",
            ("0" * 64, NOW, NOW),
        )
        connection.execute(
            "INSERT INTO projects(project_id, profile_id, domain_type, name, status, current_version_id, created_at, updated_at) VALUES (?, 'profile_m107', 'weapon_concept', 'M107 zone', 'active', NULL, ?, ?)",
            (PROJECT, NOW, NOW),
        )
        connection.commit()
    finally:
        connection.close()


def _add_version(
    unit: Any, asset_version_id: str, parent: str | None, version_no: int, status: str
) -> None:
    part = {
        "part_id": "part_m107_body",
        "role": "body",
        "parent_part_id": None,
        "position_mm": [0, 0, 0],
        "size_mm": [100, 40, 30],
        "material_zone_ids": ["zone_body", "zone_trim"],
        "editable_parameters": [],
        "locked": False,
        "provenance": "agent_generated",
    }
    graph = {
        "schema_version": "AssemblyGraph@1",
        "graph_id": f"mg_m107_{version_no}",
        "parts": [
            {"part_id": "part_m107_body", "material_zones": ["zone_body", "zone_trim"]}
        ],
        "connections": [],
    }
    unit.agent_assets.add_version(
        asset_version_id=asset_version_id,
        project_id=PROJECT,
        parent_asset_version_id=parent,
        version_no=version_no,
        status=status,
        summary=f"M107 v{version_no}",
        stage="editable_asset",
        plan_id="plan_m107",
        direction_id="direction_m107",
        domain_pack_id="pack_vehicle_concept",
        artifact_id=f"artifact_{asset_version_id}",
        parts_json=json.dumps([part], separators=(",", ":")),
        shape_program_json='{"schema_version":"ShapeProgram@1","operations":[]}',
        assembly_graph_json=json.dumps(graph, separators=(",", ":")),
        material_bindings_json="{}",
        created_at=NOW,
    )


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad-m107-zone-") as raw:
        root = Path(raw)
        os.environ["WUSHEN_LIBRARY_ROOT"] = str(root / "library")
        os.environ["WUSHEN_MIGRATIONS_DIR"] = str(ROOT / "migrations")
        os.environ["WUSHEN_LOCAL_WORKER_ENABLED"] = "0"
        os.environ["FORGECAD_CONCEPT_WORKER_ENABLED"] = "0"
        os.environ["FORGECAD_TEST_ONLY_LEGACY_AGENT_LIFECYCLE"] = "1"
        os.environ["FORGECAD_TEST_ONLY_LEGACY_PRODUCT_CORE"] = "1"
        os.environ["FORGECAD_K001_PACKAGED_PROBE"] = "1"
        from forgecad_agent.infrastructure.db import (
            SQLiteConnectionFactory,
            SQLiteMigrationRunner,
            SQLiteUnitOfWork,
        )
        from forgecad_agent.application.active_design import (
            ActiveDesignApiError,
            ActiveDesignService,
        )
        from forgecad_agent.application.agent_models import (
            NavigateActiveDesignRequest,
            SelectActiveDesignRequest,
        )
        from wushen_agent.main import create_test_only_legacy_product_core_app

        app = create_test_only_legacy_product_core_app()
        factory = SQLiteConnectionFactory(root / "library" / "library.db")
        _seed(factory)
        with SQLiteUnitOfWork(factory) as unit:
            _add_version(unit, "assetver_m107_v1", None, 1, "superseded")
            _add_version(unit, "assetver_m107_v2", "assetver_m107_v1", 2, "committed")
            unit.agent_assets.set_head(
                project_id=PROJECT, asset_version_id="assetver_m107_v2", updated_at=NOW
            )
            unit.active_designs.create_agent_snapshot(
                project_id=PROJECT,
                asset_version_id="assetver_m107_v2",
                assembly_graph_id="mg_m107_2",
                updated_at=NOW,
            )

        select = {
            "client_request_id": "m107-select",
            "snapshot_revision": 1,
            "selected_part_id": "part_m107_body",
            "selected_material_zone_id": "zone_trim",
        }
        status, headers, selected = asyncio.run(
            _request(
                app,
                "POST",
                f"/api/v1/projects/{PROJECT}/active-design:select",
                payload=select,
                headers={"Idempotency-Key": "m107-select-key"},
            )
        )
        assert status == 200 and headers["etag"] == 'W/"active-design-2"'
        assert (
            selected["selected_part_id"] == "part_m107_body"
            and selected["selected_material_zone_id"] == "zone_trim"
        )
        status, _, invalid = asyncio.run(
            _request(
                app,
                "POST",
                f"/api/v1/projects/{PROJECT}/active-design:select",
                payload={
                    **select,
                    "snapshot_revision": 2,
                    "selected_material_zone_id": "zone_missing",
                },
                headers={"Idempotency-Key": "m107-invalid-key"},
            )
        )
        assert status == 409 and invalid["error"]["code"] == "ACTIVE_DESIGN_INVALID"  # type: ignore[index]

        service = ActiveDesignService(factory)
        try:
            service.select_part(
                PROJECT,
                SelectActiveDesignRequest(
                    client_request_id="m107-stale",
                    snapshot_revision=1,
                    selected_part_id="part_m107_body",
                    selected_material_zone_id="zone_body",
                ),
                expected_revision=1,
                idempotency_key="m107-stale-key",
            )
            raise AssertionError("stale zone selection unexpectedly succeeded")
        except ActiveDesignApiError as exc:
            assert exc.code == "ACTIVE_DESIGN_STALE"

        reloaded = service.get_snapshot(PROJECT)
        assert reloaded.selected_material_zone_id == "zone_trim"
        undone = service.navigate_asset(
            PROJECT,
            NavigateActiveDesignRequest(
                client_request_id="m107-undo", snapshot_revision=2
            ),
            expected_revision=2,
            idempotency_key="m107-undo-key",
            action="undo",
        )
        assert (
            undone.selected_part_id == "part_m107_body"
            and undone.selected_material_zone_id == "zone_trim"
        )
        redone = service.navigate_asset(
            PROJECT,
            NavigateActiveDesignRequest(
                client_request_id="m107-redo", snapshot_revision=3
            ),
            expected_revision=3,
            idempotency_key="m107-redo-key",
            action="redo",
        )
        assert (
            redone.selected_part_id == "part_m107_body"
            and redone.selected_material_zone_id == "zone_trim"
        )
        rerun = SQLiteMigrationRunner(factory, ROOT / "migrations").run()
        assert "0030" not in rerun, (
            f"material-zone migration must be idempotent: {rerun}"
        )
    print(
        "FGC-M107 ActiveDesignSnapshot material-zone selection/CAS/restart/undo-redo smoke passed"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
