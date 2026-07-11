#!/usr/bin/env python3
"""Run a real Blender starter Pack through the isolated Concept workbench API chain."""

from __future__ import annotations

import argparse
import hashlib
import json
import tempfile
import urllib.request
from pathlib import Path

from concept_module_pack import import_module_pack, validate_module_pack
from smoke_r2_concept_projects import (
    _assert,
    _create_body,
    _free_port,
    _json_request,
    _start_agent,
    _stop_agent,
    _wait_for_health,
)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Validate a three-module editable Blender Pack in an isolated ForgeCAD workbench."
    )
    parser.add_argument("--pack-root", required=True, type=Path)
    parser.add_argument(
        "--combined-output",
        type=Path,
        help="optional new absolute path outside committed Module Packs for DCC evidence",
    )
    args = parser.parse_args()
    pack = validate_module_pack(args.pack_root)
    _assert(
        {module.manifest.module_id for module in pack.modules}
        == {
            "module_core_shell_01",
            "module_front_shell_01",
            "module_front_shell_02",
        },
        "Blender starter Pack must contain exactly core/front01/front02",
    )

    with tempfile.TemporaryDirectory(prefix="forgecad_blender_starter_workbench_") as temporary:
        library_root = Path(temporary) / "ForgeCADLibrary"
        port = _free_port()
        base_url = f"http://127.0.0.1:{port}"
        process = _start_agent(library_root, port)
        try:
            _wait_for_health(base_url, process)
            imported = import_module_pack(pack, base_url)
            _assert(len(imported) == 3, "Blender starter Pack import count mismatch")
            project = _json_request(
                base_url,
                "/api/v1/projects",
                method="POST",
                body=_create_body(),
                idempotency_key="blender-starter-workbench-project",
            )
            graph = _graph(project["project_id"])
            validated = _json_request(
                base_url,
                f"/api/v1/module-graphs/{graph['graph_id']}/validate",
                method="POST",
                body={
                    "client_request_id": "blender-starter-workbench-graph",
                    "graph": graph,
                    "persist": True,
                },
                idempotency_key="blender-starter-workbench-graph",
            )
            _assert(validated["valid"] is True, "Blender starter Graph was rejected")
            bound = _json_request(
                base_url,
                f"/api/v1/projects/{project['project_id']}/versions",
                method="POST",
                body={
                    "client_request_id": "blender-starter-workbench-bind",
                    "parent_version_id": project["current_version_id"],
                    "summary": "绑定 visual-v2 Blender core/front starter。",
                    "spec": project["current_spec"],
                    "module_graph_id": graph["graph_id"],
                },
                idempotency_key="blender-starter-workbench-bind",
            )
            base_version_id = bound["current_version_id"]
            change_set = _replacement_change_set(project["project_id"], base_version_id)
            proposed = _json_request(
                base_url,
                f"/api/v1/versions/{base_version_id}/change-sets",
                method="POST",
                body={
                    "client_request_id": "blender-starter-workbench-propose",
                    "change_set": change_set,
                },
                idempotency_key="blender-starter-workbench-propose",
            )
            _assert(proposed["status"] == "proposed", "replacement was not proposed")
            preview = _json_request(
                base_url,
                f"/api/v1/change-sets/{change_set['change_set_id']}:preview",
                method="POST",
                idempotency_key="blender-starter-workbench-preview",
            )
            front = next(
                node for node in preview["preview_graph"]["nodes"] if node["node_id"] == "node_front"
            )
            _assert(
                front["module_id"] == "module_front_shell_02",
                "preview did not replace the Blender front shell",
            )
            confirmed = _json_request(
                base_url,
                f"/api/v1/change-sets/{change_set['change_set_id']}:confirm",
                method="POST",
                idempotency_key="blender-starter-workbench-confirm",
            )
            version_id = confirmed["project"]["current_version_id"]
            _assert(version_id != base_version_id, "confirmed replacement did not create a child Version")
            quality = _json_request(
                base_url,
                f"/api/v1/versions/{version_id}/quality-runs:inspect",
                method="POST",
                body={
                    "client_request_id": "blender-starter-workbench-quality",
                    "ruleset_version": "weapon-concept-geometry/1.3",
                },
                idempotency_key="blender-starter-workbench-quality",
            )
            _assert(
                quality["report"]["status"] in {"passed", "warning"},
                f"Blender starter quality inspection failed: {quality['report']['status']}",
            )
            exported = _json_request(
                base_url,
                f"/api/v1/versions/{version_id}/exports",
                method="POST",
                body={
                    "client_request_id": "blender-starter-workbench-export",
                    "profile": "game_asset",
                    "include_modules": True,
                    "include_combined_glb": True,
                    "include_combined_obj": False,
                    "include_render_png": False,
                    "include_turntable_video": False,
                    "include_quality_report": True,
                },
                idempotency_key="blender-starter-workbench-export",
            )
            combined_payload = _download_combined_glb(base_url, exported["export_id"])
            _assert(
                hashlib.sha256(combined_payload).hexdigest() == exported["combined_glb_sha256"],
                "combined GLB download hash mismatch",
            )
            _assert(combined_payload[:4] == b"glTF", "combined GLB header mismatch")
            combined_output = _persist_combined_glb(args.combined_output, combined_payload)
        finally:
            _stop_agent(process)

        restart_port = _free_port()
        restart_url = f"http://127.0.0.1:{restart_port}"
        restarted = _start_agent(library_root, restart_port)
        try:
            _wait_for_health(restart_url, restarted)
            restored = _json_request(
                restart_url,
                f"/api/v1/projects/{project['project_id']}",
                method="GET",
            )
            _assert(restored["current_version_id"] == version_id, "restart lost confirmed Blender Version")
            restored_modules = _json_request(restart_url, "/api/v1/module-assets", method="GET")
            _assert(len(restored_modules["items"]) == 3, "restart lost Blender starter modules")
        finally:
            _stop_agent(restarted)

    print(
        json.dumps(
            {
                "ok": True,
                "pack_id": pack.manifest.pack_id,
                "module_count": 3,
                "replacement": "module_front_shell_01 -> module_front_shell_02",
                "quality_status": quality["report"]["status"],
                "quality_finding_count": len(quality["report"]["findings"]),
                "combined_glb_sha256": exported["combined_glb_sha256"],
                "combined_glb_byte_size": exported["combined_glb_byte_size"],
                "combined_glb_output": str(combined_output) if combined_output else None,
                "restart_restored": True,
            },
            ensure_ascii=False,
            indent=2,
        )
    )
    return 0


def _graph(project_id: str) -> dict[str, object]:
    return {
        "schema_version": "ModuleGraph@1",
        "graph_id": "mg_blender_starter_visual_v2",
        "project_id": project_id,
        "root_node_id": "node_core",
        "nodes": [
            {
                "node_id": "node_core",
                "module_id": "module_core_shell_01",
                "transform": _transform([0, 0, 0]),
                "mirror_axis": "none",
                "locked": True,
                "visible": True,
            },
            {
                "node_id": "node_front",
                "module_id": "module_front_shell_01",
                "transform": _transform([-50, 0, 0]),
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
                "to_connector_id": "connector_front_01_core",
                "status": "connected",
            }
        ],
    }


def _replacement_change_set(project_id: str, version_id: str) -> dict[str, object]:
    return {
        "schema_version": "DesignChangeSet@1",
        "change_set_id": "change_blender_starter_front_replace",
        "project_id": project_id,
        "base_version_id": version_id,
        "summary": "替换 visual-v2 前部外壳，保持非功能性概念装配接口。",
        "operations": [
            {
                "operation_id": "op_blender_starter_front_replace",
                "op": "replace_module",
                "node_id": "node_front",
                "module_id": "module_front_shell_02",
            }
        ],
        "protected_node_ids": ["node_core"],
        "status": "proposed",
    }


def _transform(position: list[float]) -> dict[str, object]:
    return {
        "position": position,
        "rotation": [0, 0, 0],
        "scale": [1, 1, 1],
    }


def _download_combined_glb(base_url: str, export_id: str) -> bytes:
    with urllib.request.urlopen(
        f"{base_url}/api/v1/exports/{export_id}/combined.glb", timeout=20
    ) as response:
        return response.read()


def _persist_combined_glb(output: Path | None, payload: bytes) -> Path | None:
    if output is None:
        return None
    expanded = output.expanduser()
    if not expanded.is_absolute() or expanded.suffix != ".glb":
        raise ValueError("--combined-output must be an absolute .glb path")
    resolved = expanded.resolve()
    repository_root = Path(__file__).resolve().parents[1]
    committed_pack_root = repository_root / "assets" / "module-packs"
    if resolved.is_relative_to(committed_pack_root):
        raise ValueError("--combined-output cannot target committed Module Pack assets")
    if resolved.exists():
        raise ValueError("--combined-output must not overwrite an existing artifact")
    resolved.parent.mkdir(parents=True, exist_ok=True)
    resolved.write_bytes(payload)
    return resolved


if __name__ == "__main__":
    raise SystemExit(main())
