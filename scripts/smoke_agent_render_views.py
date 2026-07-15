#!/usr/bin/env python3
"""R002/R003 smoke for Snapshot-bound Agent standard and exploded PNG rendering."""

from __future__ import annotations

import asyncio
import base64
import hashlib
import io
import json
import os
import struct
import tempfile
import zipfile
import zlib
from pathlib import Path
from typing import Any
from urllib.parse import urlsplit

from jsonschema import Draft202012Validator

from smoke_active_design_api import _error_code, _seed_legacy_current, _seed_project


ROOT = Path(__file__).resolve().parents[1]
NOW = "2026-07-13T00:00:00+00:00"


def _canonical(value: object) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


async def _request(
    app: Any,
    method: str,
    url: str,
    *,
    payload: dict[str, object] | None = None,
    headers: dict[str, str] | None = None,
) -> tuple[int, dict[str, str], dict[str, object]]:
    parsed = urlsplit(url)
    body = _canonical(payload).encode("utf-8") if payload is not None else b""
    messages = [{"type": "http.request", "body": body, "more_body": False}]
    responses: list[dict[str, Any]] = []

    async def receive() -> dict[str, Any]:
        return messages.pop(0) if messages else {"type": "http.disconnect"}

    async def send(message: dict[str, Any]) -> None:
        responses.append(message)

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
            "path": parsed.path,
            "raw_path": parsed.path.encode("ascii"),
            "query_string": parsed.query.encode("ascii"),
            "headers": raw_headers,
            "client": ("127.0.0.1", 12345),
            "server": ("testserver", 80),
        },
        receive,
        send,
    )
    start = next(item for item in responses if item["type"] == "http.response.start")
    response_body = b"".join(item.get("body", b"") for item in responses if item["type"] == "http.response.body")
    response_headers = {key.decode("latin-1").lower(): value.decode("latin-1") for key, value in start.get("headers", [])}
    return int(start["status"]), response_headers, json.loads(response_body.decode("utf-8"))


async def _binary_request(app: Any, url: str) -> tuple[int, dict[str, str], bytes]:
    parsed = urlsplit(url)
    messages = [{"type": "http.request", "body": b"", "more_body": False}]
    responses: list[dict[str, Any]] = []

    async def receive() -> dict[str, Any]:
        return messages.pop(0) if messages else {"type": "http.disconnect"}

    async def send(message: dict[str, Any]) -> None:
        responses.append(message)

    await app(
        {
            "type": "http",
            "asgi": {"version": "3.0", "spec_version": "2.3"},
            "http_version": "1.1",
            "method": "GET",
            "scheme": "http",
            "path": parsed.path,
            "raw_path": parsed.path.encode("ascii"),
            "query_string": parsed.query.encode("ascii"),
            "headers": [(b"host", b"testserver")],
            "client": ("127.0.0.1", 12345),
            "server": ("testserver", 80),
        },
        receive,
        send,
    )
    start = next(item for item in responses if item["type"] == "http.response.start")
    response_body = b"".join(item.get("body", b"") for item in responses if item["type"] == "http.response.body")
    response_headers = {key.decode("latin-1").lower(): value.decode("latin-1") for key, value in start.get("headers", [])}
    return int(start["status"]), response_headers, response_body


def _seed_render_agent(factory: Any, project_id: str) -> str:
    from forgecad_agent.infrastructure.db import SQLiteUnitOfWork

    asset_version_id = "assetver_r002_agent"
    part_id = "part_r002_body"
    cabin_id = "part_r002_cabin"
    graph = {
        "schema_version": "AssemblyGraph@1",
        "graph_id": "mg_r002_agent",
        "root_part_id": part_id,
        "parts": [
            {"part_id": part_id, "role": "body", "parent_part_id": None},
            {"part_id": cabin_id, "role": "cabin", "parent_part_id": part_id},
        ],
        "connections": [{"connection_id": "conn_r002_body_cabin", "from_part_id": part_id, "to_part_id": cabin_id}],
    }
    program = {
        "schema_version": "ShapeProgram@1",
        "program_id": "shape_r002_agent",
        "units": "millimeter",
        "seed": 2,
        "triangle_budget": 100000,
        "parameters": [],
        "operations": [
            {
                "operation_id": "op_r002_body",
                "op": "box",
                "inputs": [],
                "args": {"part_role": "body", "size": [120, 60, 80], "position": [0, 0, 0], "rotation": [0, 0, 0], "material_id": "mat_graphite"},
            },
            {
                "operation_id": "op_r002_cabin",
                "op": "box",
                "inputs": [],
                "args": {"part_role": "cabin", "size": [70, 40, 50], "position": [38, 46, 0], "rotation": [0, 0, 0], "material_id": "mat_aluminum"},
            },
        ],
        "outputs": [
            {"output_id": "output_r002_body", "operation_id": "op_r002_body", "kind": "mesh", "part_role": "body"},
            {"output_id": "output_r002_cabin", "operation_id": "op_r002_cabin", "kind": "mesh", "part_role": "cabin"},
        ],
        "non_functional_only": True,
    }
    parts = [
        {
            "part_id": part_id,
            "role": "body",
            "parent_part_id": None,
            "position_mm": [0, 0, 0],
            "size_mm": [120, 60, 80],
            "material_zone_ids": ["zone_body"],
            "editable_parameters": ["scale"],
            "locked": False,
            "provenance": "agent_generated",
        },
        {
            "part_id": cabin_id,
            "role": "cabin",
            "parent_part_id": part_id,
            "position_mm": [38, 46, 0],
            "size_mm": [70, 40, 50],
            "material_zone_ids": ["zone_cabin"],
            "editable_parameters": ["scale"],
            "locked": False,
            "provenance": "agent_generated",
        },
    ]
    with SQLiteUnitOfWork(factory) as unit:
        unit.agent_assets.add_version(
            asset_version_id=asset_version_id,
            project_id=project_id,
            parent_asset_version_id=None,
            version_no=1,
            status="committed",
            summary="R002 render fixture",
            stage="editable_asset",
            plan_id="plan_r002",
            direction_id="direction_r002",
            domain_pack_id="pack_vehicle_concept",
            artifact_id="artifact_r002_agent",
            parts_json=_canonical(parts),
            shape_program_json=_canonical(program),
            assembly_graph_json=_canonical(graph),
            material_bindings_json="{}",
            created_at=NOW,
        )
        unit.agent_assets.set_head(project_id=project_id, asset_version_id=asset_version_id, updated_at=NOW)
    return asset_version_id


def _assert_png(payload: bytes, width: int, height: int) -> str:
    assert payload[:8] == b"\x89PNG\r\n\x1a\n"
    assert payload[12:16] == b"IHDR"
    actual_width, actual_height, bit_depth, color_type, compression, filtering, interlace = struct.unpack(">IIBBBBB", payload[16:29])
    assert (actual_width, actual_height) == (width, height)
    assert (bit_depth, color_type, compression, filtering, interlace) == (8, 6, 0, 0, 0)
    position = 8
    compressed = []
    while position + 12 <= len(payload):
        chunk_length = struct.unpack(">I", payload[position : position + 4])[0]
        kind = payload[position + 4 : position + 8]
        if kind == b"IDAT":
            compressed.append(payload[position + 8 : position + 8 + chunk_length])
        position += 12 + chunk_length
        if kind == b"IEND":
            break
    decoded = zlib.decompress(b"".join(compressed))
    stride = width * 4
    alphas = [decoded[row * (stride + 1) + 1 + column * 4 + 3] for row in range(height) for column in range(width)]
    assert any(alpha == 0 for alpha in alphas) and any(alpha > 0 for alpha in alphas)
    return hashlib.sha256(payload).hexdigest()


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad-r002-render-views-") as raw:
        root = Path(raw)
        os.environ["WUSHEN_LIBRARY_ROOT"] = str(root / "library")
        os.environ["WUSHEN_MIGRATIONS_DIR"] = str(ROOT / "migrations")
        os.environ["WUSHEN_LOCAL_WORKER_ENABLED"] = "0"
        os.environ["FORGECAD_CONCEPT_WORKER_ENABLED"] = "0"

        from forgecad_agent.infrastructure.db import SQLiteConnectionFactory
        from wushen_agent.main import create_app

        app = create_app()
        factory = SQLiteConnectionFactory(root / "library" / "library.db")
        _seed_project(factory, "prj_r002_agent")
        _seed_project(factory, "prj_r002_legacy")
        asset_version_id = _seed_render_agent(factory, "prj_r002_agent")
        _seed_legacy_current(factory, "prj_r002_legacy")

        status, _, snapshot = asyncio.run(_request(app, "GET", "/api/v1/projects/prj_r002_agent/active-design"))
        assert status == 200 and snapshot["active_design"]["asset_version_id"] == asset_version_id

        url = f"/api/v1/agent/asset-versions/{asset_version_id}:render?width=128&height=128"
        status, _, first = asyncio.run(_request(app, "GET", url))
        assert status == 200, (status, first)
        assert first["asset_version_id"] == asset_version_id
        assert first["renderer_id"] == "forgecad-agent-software-raster@1"
        assert [item["view_id"] for item in first["views"]] == ["iso", "front", "side", "top", "exploded_iso"]
        assert first["exploded_view_available"] is True and first["exploded_unavailable_reason"] is None
        first_hashes = []
        for item in first["views"]:
            payload = base64.b64decode(item["png_base64"], validate=True)
            digest = _assert_png(payload, 128, 128)
            assert digest == item["sha256"] and item["byte_size"] == len(payload)
            assert item["readback_status"] == "passed"
            assert item["background_mode"] == "transparent"
            first_hashes.append(digest)
        exploded = first["views"][-1]
        assert exploded["presentation_mode"] == "exploded"
        assert exploded["camera_view"] == "iso"
        assert exploded["part_ids"] == ["part_r002_body", "part_r002_cabin"]
        assert first["render_set_sha256"] == hashlib.sha256(_canonical({
            "schema_version": "AgentAssetRenderSet@1",
            "asset_version_id": asset_version_id,
            "renderer_id": "forgecad-agent-software-raster@1",
            "width": 128,
            "height": 128,
            "views": [
                {
                    "view_id": item["view_id"],
                    "sha256": digest,
                    "presentation_mode": item["presentation_mode"],
                    "background_mode": item["background_mode"],
                    "part_ids": item["part_ids"],
                }
                for item, digest in zip(first["views"], first_hashes)
            ],
            "exploded_view_available": True,
            "exploded_unavailable_reason": None,
        }).encode("utf-8")).hexdigest()

        from forgecad_agent.application.agent_asset_editing import AgentAssetEditingService
        from forgecad_agent.application.agent_rendering import ExplodedPartOffset, render_agent_views

        source_glb = base64.b64decode(
            AgentAssetEditingService(factory).export_glb(asset_version_id).glb_base64,
            validate=True,
        )
        unsafe_candidate = render_agent_views(
            source_glb,
            width=128,
            height=128,
            exploded_parts=(ExplodedPartOffset(part_id="part_r002_body", offset=(0.0, 0.0, 0.0)),),
        )
        assert "exploded_iso" not in unsafe_candidate.views
        assert unsafe_candidate.exploded_part_ids == ()
        assert unsafe_candidate.exploded_unavailable_reason is not None

        status, _, second = asyncio.run(_request(app, "GET", url))
        assert status == 200
        assert second["render_set_sha256"] == first["render_set_sha256"]
        assert [item["sha256"] for item in second["views"]] == first_hashes

        package_url = (
            f"/api/v1/agent/asset-versions/{asset_version_id}:render-package?width=128&height=128"
            f"&render_set_sha256={first['render_set_sha256']}"
        )
        status, headers, package = asyncio.run(_binary_request(app, package_url))
        assert status == 200
        assert headers["content-type"].startswith("application/zip")
        assert headers["cache-control"] == "no-store"
        assert headers["x-forgecad-render-set-sha256"] == first["render_set_sha256"]
        assert headers["content-disposition"] == f'attachment; filename="{asset_version_id}-concept-views.zip"'
        with zipfile.ZipFile(io.BytesIO(package), "r") as bundle:
            expected_names = ["manifest.json", "iso.png", "front.png", "side.png", "top.png", "exploded_iso.png"]
            assert bundle.namelist() == expected_names
            assert all(item.date_time == (1980, 1, 1, 0, 0, 0) for item in bundle.infolist())
            manifest = json.loads(bundle.read("manifest.json"))
            package_schema = json.loads((ROOT / "packages/concept-spec/schemas/agent-asset-render-package.schema.json").read_text(encoding="utf-8"))
            Draft202012Validator(package_schema).validate(manifest)
            assert manifest["schema_version"] == "AgentAssetRenderPackage@1"
            assert manifest["package_kind"] == "concept_view_png_bundle"
            assert manifest["asset_version_id"] == asset_version_id
            assert manifest["render_set_sha256"] == first["render_set_sha256"]
            assert manifest["non_engineering_notice"] == "concept_views_only_not_engineering_or_manufacturing_data"
            assert [item["file_name"] for item in manifest["views"]] == expected_names[1:]
            assert manifest["views"][-1]["part_ids"] == ["part_r002_body", "part_r002_cabin"]
            for item in manifest["views"]:
                payload = bundle.read(item["file_name"])
                assert hashlib.sha256(payload).hexdigest() == item["sha256"]
                assert len(payload) == item["byte_size"]
                _assert_png(payload, item["width"], item["height"])
        repeat_status, repeat_headers, repeat_package = asyncio.run(_binary_request(app, package_url))
        assert repeat_status == 200 and repeat_headers["x-forgecad-render-set-sha256"] == first["render_set_sha256"]
        assert repeat_package == package

        stale_package_url = package_url.replace(first["render_set_sha256"], "0" * 64)
        status, _, stale_package = asyncio.run(_request(app, "GET", stale_package_url))
        assert status == 409 and _error_code(stale_package) == "RENDER_SET_STALE"
        status, _, missing_fingerprint = asyncio.run(_request(app, "GET", f"/api/v1/agent/asset-versions/{asset_version_id}:render-package?width=128&height=128"))
        assert status == 422

        status, _, legacy_error = asyncio.run(_request(app, "GET", "/api/v1/agent/asset-versions/assetver_missing:render?width=128&height=128"))
        assert status == 404 and _error_code(legacy_error) == "ASSET_VERSION_NOT_FOUND"
        status, _, invalid = asyncio.run(_request(app, "GET", f"/api/v1/agent/asset-versions/{asset_version_id}:render?width=32&height=128"))
        assert status == 422

    print("R002/R003/R004 Agent PNG provenance, transparent alpha, deterministic exploded views and ZIP package smoke passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
