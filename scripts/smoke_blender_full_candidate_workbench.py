#!/usr/bin/env python3
"""Run a ten-module editable Blender candidate Pack through the isolated Concept chain."""

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
from smoke_r3_module_pack_tooling import _reference_graph


EXPECTED_MODULE_IDS = {
    "module_core_shell_01",
    "module_front_shell_01",
    "module_front_shell_02",
    "module_rear_shell_01",
    "module_grip_shell_01",
    "module_top_accessory_01",
    "module_side_accessory_01",
    "module_lower_structure_01",
    "module_storage_visual_01",
    "module_armor_panel_01",
}


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Validate a ten-module Blender visual candidate in an isolated ForgeCAD workbench."
    )
    parser.add_argument("--pack-root", required=True, type=Path)
    parser.add_argument("--combined-output", type=Path)
    args = parser.parse_args()
    pack = validate_module_pack(args.pack_root)
    _assert(
        {module.manifest.module_id for module in pack.modules} == EXPECTED_MODULE_IDS,
        "full candidate Pack must contain the ten stable module IDs",
    )

    with tempfile.TemporaryDirectory(
        prefix="forgecad_blender_full_candidate_"
    ) as temporary:
        library_root = Path(temporary) / "ForgeCADLibrary"
        port = _free_port()
        base_url = f"http://127.0.0.1:{port}"
        process = _start_agent(library_root, port)
        try:
            _wait_for_health(base_url, process)
            imported = import_module_pack(pack, base_url)
            _assert(len(imported) == 10, "full candidate import count mismatch")
            project = _json_request(
                base_url,
                "/api/v1/projects",
                method="POST",
                body=_create_body(),
                idempotency_key="blender-full-candidate-project",
            )
            graph = _reference_graph(project["project_id"])
            graph["graph_id"] = "mg_blender_full_candidate_visual_v1"
            validated = _json_request(
                base_url,
                f"/api/v1/module-graphs/{graph['graph_id']}/validate",
                method="POST",
                body={
                    "client_request_id": "blender-full-candidate-graph",
                    "graph": graph,
                    "persist": True,
                },
                idempotency_key="blender-full-candidate-graph",
            )
            _assert(validated["valid"] is True, "full candidate graph was rejected")
            bound = _json_request(
                base_url,
                f"/api/v1/projects/{project['project_id']}/versions",
                method="POST",
                body={
                    "client_request_id": "blender-full-candidate-bind",
                    "parent_version_id": project["current_version_id"],
                    "summary": "绑定十模块 Blender 视觉候选包，保持非功能性概念装配范围。",
                    "spec": project["current_spec"],
                    "module_graph_id": graph["graph_id"],
                },
                idempotency_key="blender-full-candidate-bind",
            )
            version_id = bound["current_version_id"]
            quality = _json_request(
                base_url,
                f"/api/v1/versions/{version_id}/quality-runs:inspect",
                method="POST",
                body={
                    "client_request_id": "blender-full-candidate-quality",
                    "ruleset_version": "weapon-concept-geometry/1.3",
                },
                idempotency_key="blender-full-candidate-quality",
            )
            _assert(
                quality["report"]["status"] in {"passed", "warning"},
                "full candidate quality inspection failed",
            )
            exported = _json_request(
                base_url,
                f"/api/v1/versions/{version_id}/exports",
                method="POST",
                body={
                    "client_request_id": "blender-full-candidate-export",
                    "profile": "game_asset",
                    "include_modules": True,
                    "include_combined_glb": True,
                    "include_combined_obj": False,
                    "include_render_png": False,
                    "include_turntable_video": False,
                    "include_quality_report": True,
                },
                idempotency_key="blender-full-candidate-export",
            )
            combined_payload = _download_combined_glb(base_url, exported["export_id"])
            _assert(
                hashlib.sha256(combined_payload).hexdigest()
                == exported["combined_glb_sha256"],
                "combined GLB download hash mismatch",
            )
            _assert(combined_payload[:4] == b"glTF", "combined GLB header mismatch")
            combined_output = _persist_combined_glb(
                args.combined_output, combined_payload
            )
        finally:
            _stop_agent(process)

        restart_port = _free_port()
        restart_url = f"http://127.0.0.1:{restart_port}"
        restarted = _start_agent(library_root, restart_port)
        try:
            _wait_for_health(restart_url, restarted)
            restored = _json_request(
                restart_url, f"/api/v1/projects/{project['project_id']}", method="GET"
            )
            _assert(
                restored["current_version_id"] == version_id,
                "restart lost candidate Version",
            )
            restored_modules = _json_request(
                restart_url, "/api/v1/module-assets", method="GET"
            )
            _assert(
                len(restored_modules["items"]) == 10, "restart lost candidate modules"
            )
            restored_graph = _json_request(
                restart_url, f"/api/v1/module-graphs/{graph['graph_id']}", method="GET"
            )
            _assert(
                len(restored_graph["graph"]["nodes"]) == 9, "restart lost full graph"
            )
        finally:
            _stop_agent(restarted)

    print(
        json.dumps(
            {
                "ok": True,
                "pack_id": pack.manifest.pack_id,
                "module_count": 10,
                "graph_node_count": 9,
                "quality_status": quality["report"]["status"],
                "quality_finding_count": len(quality["report"]["findings"]),
                "quality_finding_checks": [
                    finding["check_id"] for finding in quality["report"]["findings"]
                ],
                "quality_findings": [
                    {
                        "check_id": finding["check_id"],
                        "node_ids": finding["node_ids"],
                        "message": finding["message"],
                    }
                    for finding in quality["report"]["findings"]
                ],
                "combined_glb_sha256": exported["combined_glb_sha256"],
                "combined_glb_byte_size": exported["combined_glb_byte_size"],
                "combined_glb_output": str(combined_output)
                if combined_output
                else None,
                "restart_restored": True,
            },
            ensure_ascii=False,
            indent=2,
        )
    )
    return 0


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
    committed_pack_root = (
        Path(__file__).resolve().parents[1] / "assets" / "module-packs"
    )
    if resolved.is_relative_to(committed_pack_root):
        raise ValueError("--combined-output cannot target committed Module Pack assets")
    if resolved.exists():
        raise ValueError("--combined-output must not overwrite an existing artifact")
    resolved.parent.mkdir(parents=True, exist_ok=True)
    resolved.write_bytes(payload)
    return resolved


if __name__ == "__main__":
    raise SystemExit(main())
