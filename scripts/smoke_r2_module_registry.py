#!/usr/bin/env python3
from __future__ import annotations

import base64
import hashlib
import json
import sqlite3
import struct
import tempfile
import urllib.request
from pathlib import Path
from typing import Any, Dict, List

from smoke_r2_concept_projects import (
    _assert,
    _create_body,
    _free_port,
    _json_request,
    _json_request_allow_error,
    _start_agent,
    _stop_agent,
    _wait_for_health,
)


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad_r2_modules_") as temporary_directory:
        library_root = Path(temporary_directory) / "ForgeCADLibrary"
        port = _free_port()
        base_url = f"http://127.0.0.1:{port}"
        process = _start_agent(library_root, port)
        try:
            _wait_for_health(base_url, process)
            project = _json_request(
                base_url,
                "/api/v1/projects",
                method="POST",
                body=_create_body(),
                idempotency_key="r2-module-project",
            )
            project_id = project["project_id"]

            core_payload = _minimal_glb("core_shell_01")
            front_payload = _minimal_glb("front_shell_01")
            core_manifest = _manifest(
                module_id="module_core_shell_01",
                asset_id="asset_core_shell_01",
                category="core_shell",
                payload=core_payload,
                connectors=[
                    _connector("connector_core_front", "core.front", "shell_mount"),
                    _connector("connector_core_grip", "core.grip", "grip_mount"),
                ],
            )
            front_manifest = _manifest(
                module_id="module_front_shell_01",
                asset_id="asset_front_shell_01",
                category="front_shell",
                payload=front_payload,
                connectors=[
                    _connector("connector_front_core", "front.core", "shell_mount"),
                ],
            )
            core = _register(
                base_url,
                core_manifest,
                core_payload,
                "packs/weapon-concept/core-shell-01.glb",
                "r2-register-core",
            )
            front = _register(
                base_url,
                front_manifest,
                front_payload,
                "packs/weapon-concept/front-shell-01.glb",
                "r2-register-front",
            )
            _assert(core["manifest"]["module_id"] == "module_core_shell_01", "core registration mismatch")
            _assert(front["manifest"]["module_id"] == "module_front_shell_01", "front registration mismatch")
            with urllib.request.urlopen(
                f"{base_url}/api/v1/module-assets/module_core_shell_01/file",
                timeout=10,
            ) as response:
                downloaded_core = response.read()
                _assert(response.headers.get_content_type() == "model/gltf-binary", "module MIME mismatch")
                _assert(
                    response.headers["X-Content-SHA256"] == hashlib.sha256(core_payload).hexdigest(),
                    "module response hash mismatch",
                )
            _assert(downloaded_core == core_payload, "downloaded module GLB changed")

            replay = _register(
                base_url,
                core_manifest,
                core_payload,
                "packs/weapon-concept/core-shell-01.glb",
                "r2-register-core",
            )
            _assert(replay["object_path"] == core["object_path"], "module replay object path mismatch")

            listed = _json_request(
                base_url,
                "/api/v1/module-assets?pack_id=pack_weapon_concept_v1",
                method="GET",
            )
            _assert(len(listed["items"]) == 2, "module registry list count mismatch")
            front_only = _json_request(
                base_url,
                "/api/v1/module-assets?category=front_shell",
                method="GET",
            )
            _assert(len(front_only["items"]) == 1, "module category filter mismatch")

            bad_manifest = _manifest(
                module_id="module_bad_hash",
                asset_id="asset_bad_hash",
                category="armor_panel",
                payload=core_payload,
                connectors=[],
            )
            bad_manifest["sha256"] = "0" * 64
            bad_hash_status, bad_hash = _json_request_allow_error(
                base_url,
                "/api/v1/module-assets",
                method="POST",
                body={
                    "client_request_id": "r2-bad-hash",
                    "manifest": bad_manifest,
                    "logical_path": "packs/weapon-concept/bad.glb",
                    "glb_data_base64": base64.b64encode(core_payload).decode("ascii"),
                },
                idempotency_key="r2-register-bad-hash",
            )
            _assert(
                bad_hash_status == 409 and bad_hash["error"]["code"] == "MODULE_HASH_MISMATCH",
                "module hash mismatch was not rejected",
            )

            graph = _graph(project_id)
            validated = _json_request(
                base_url,
                f"/api/v1/module-graphs/{graph['graph_id']}/validate",
                method="POST",
                body={"client_request_id": "r2-graph-valid", "graph": graph, "persist": True},
                idempotency_key="r2-graph-valid",
            )
            _assert(validated["valid"] is True, "valid graph was rejected")
            _assert(validated["persisted"] is True, "valid graph was not persisted")
            _assert(validated["job_id"].startswith("job_"), "graph validation job id missing")
            validation_job = _json_request(
                base_url,
                f"/api/v1/jobs/{validated['job_id']}",
                method="GET",
            )
            _assert(validation_job["type"] == "validate_graph", "graph job type mismatch")
            _assert(len(validation_job["events"]) == 3, "graph JobEvent count mismatch")
            graph_record = _json_request(
                base_url,
                f"/api/v1/module-graphs/{graph['graph_id']}",
                method="GET",
            )
            _assert(graph_record["graph"] == graph, "persisted graph round-trip mismatch")

            invalid_graph = _graph(project_id, graph_id="mg_missing_module")
            invalid_graph["nodes"][1]["module_id"] = "module_missing"
            invalid = _json_request(
                base_url,
                f"/api/v1/module-graphs/{invalid_graph['graph_id']}/validate",
                method="POST",
                body={
                    "client_request_id": "r2-graph-missing-module",
                    "graph": invalid_graph,
                    "persist": True,
                },
                idempotency_key="r2-graph-missing-module",
            )
            _assert(invalid["valid"] is False, "unknown module graph was accepted")
            _assert(invalid["persisted"] is False, "invalid graph was persisted")
            _assert(invalid["issues"][0]["code"] == "MODULE_NOT_FOUND", "invalid graph issue mismatch")
            _assert(invalid["job_id"].startswith("job_"), "invalid graph job id missing")
        finally:
            _stop_agent(process)

        database_path = library_root / "library.db"
        with sqlite3.connect(database_path) as connection:
            module_count = connection.execute("SELECT COUNT(*) FROM module_assets").fetchone()[0]
            connector_count = connection.execute("SELECT COUNT(*) FROM module_connectors").fetchone()[0]
            graph_count = connection.execute("SELECT COUNT(*) FROM module_graphs").fetchone()[0]
            concept_job_count = connection.execute("SELECT COUNT(*) FROM concept_jobs").fetchone()[0]
            concept_event_count = connection.execute("SELECT COUNT(*) FROM concept_job_events").fetchone()[0]
        _assert(module_count == 2, "module table count mismatch")
        _assert(connector_count == 3, "connector table count mismatch")
        _assert(graph_count == 1, "only the valid graph should be persisted")
        _assert(concept_job_count == 2, "graph validation jobs were not persisted")
        _assert(concept_event_count == 6, "graph validation events were not persisted")

        restart_port = _free_port()
        restart_url = f"http://127.0.0.1:{restart_port}"
        restarted_process = _start_agent(library_root, restart_port)
        try:
            _wait_for_health(restart_url, restarted_process)
            restored_modules = _json_request(
                restart_url,
                "/api/v1/module-assets",
                method="GET",
            )
            restored_graph = _json_request(
                restart_url,
                "/api/v1/module-graphs/mg_arctic_patrol_v1",
                method="GET",
            )
            _assert(len(restored_modules["items"]) == 2, "restart did not restore module registry")
            _assert(restored_graph["validation_status"] == "valid", "restart did not restore graph")
            restored_validation_job = _json_request(
                restart_url,
                f"/api/v1/jobs/{validated['job_id']}",
                method="GET",
            )
            _assert(len(restored_validation_job["events"]) == 3, "restart lost graph events")
        finally:
            _stop_agent(restarted_process)

        print(
            json.dumps(
                {
                    "ok": True,
                    "project_id": project_id,
                    "module_count": module_count,
                    "connector_count": connector_count,
                    "graph_count": graph_count,
                    "invalid_graph_persisted": False,
                    "concept_job_count": concept_job_count,
                    "concept_event_count": concept_event_count,
                },
                ensure_ascii=False,
                indent=2,
            )
        )
    return 0


def _register(
    base_url: str,
    manifest: Dict[str, Any],
    payload: bytes,
    logical_path: str,
    idempotency_key: str,
) -> Dict[str, Any]:
    return _json_request(
        base_url,
        "/api/v1/module-assets",
        method="POST",
        body={
            "client_request_id": idempotency_key,
            "manifest": manifest,
            "logical_path": logical_path,
            "glb_data_base64": base64.b64encode(payload).decode("ascii"),
        },
        idempotency_key=idempotency_key,
    )


def _manifest(
    *,
    module_id: str,
    asset_id: str,
    category: str,
    payload: bytes,
    connectors: List[Dict[str, Any]],
) -> Dict[str, Any]:
    return {
        "schema_version": "ModuleAssetManifest@1",
        "module_id": module_id,
        "pack_id": "pack_weapon_concept_v1",
        "category": category,
        "asset_id": asset_id,
        "sha256": hashlib.sha256(payload).hexdigest(),
        "bounds_mm": [148, 56, 42],
        "triangle_count": 12,
        "material_slots": ["primary", "secondary", "accent"],
        "connectors": connectors,
    }


def _connector(connector_id: str, slot: str, connector_type: str) -> Dict[str, Any]:
    return {
        "connector_id": connector_id,
        "slot": slot,
        "connector_type": connector_type,
        "transform": _transform(),
        "scale_range": [0.8, 1.2],
        "exclusive": True,
    }


def _graph(project_id: str, *, graph_id: str = "mg_arctic_patrol_v1") -> Dict[str, Any]:
    return {
        "schema_version": "ModuleGraph@1",
        "graph_id": graph_id,
        "project_id": project_id,
        "root_node_id": "node_core",
        "nodes": [
            {
                "node_id": "node_core",
                "module_id": "module_core_shell_01",
                "transform": _transform(),
                "mirror_axis": "none",
                "locked": True,
                "visible": True,
            },
            {
                "node_id": "node_front",
                "module_id": "module_front_shell_01",
                "transform": _transform(),
                "mirror_axis": "none",
                "locked": False,
                "visible": True,
            },
        ],
        "edges": [
            {
                "edge_id": "edge_core_front",
                "from_node_id": "node_core",
                "from_connector_id": "connector_core_front",
                "to_node_id": "node_front",
                "to_connector_id": "connector_front_core",
                "status": "connected",
            }
        ],
    }


def _transform() -> Dict[str, Any]:
    return {
        "position": [0.0, 0.0, 0.0],
        "rotation": [0.0, 0.0, 0.0],
        "scale": [1.0, 1.0, 1.0],
    }


def _minimal_glb(name: str) -> bytes:
    document = {
        "asset": {"version": "2.0", "generator": "ForgeCAD R2 fixture"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"name": name}],
    }
    json_payload = json.dumps(document, separators=(",", ":")).encode("utf-8")
    json_payload += b" " * ((4 - len(json_payload) % 4) % 4)
    total_length = 12 + 8 + len(json_payload)
    return (
        struct.pack("<4sII", b"glTF", 2, total_length)
        + struct.pack("<II", len(json_payload), 0x4E4F534A)
        + json_payload
    )


if __name__ == "__main__":
    raise SystemExit(main())
