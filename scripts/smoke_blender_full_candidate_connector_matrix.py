#!/usr/bin/env python3
"""Exercise Connector replacements and mirrors on the Blender visual candidate.

This script deliberately creates an isolated Library and labels its result as
unclassified technical evidence.  It cannot promote a visual candidate to a
formal asset: that still requires the independent FormalModuleReview flow.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import tempfile
import urllib.request
from pathlib import Path
from typing import Any

from concept_module_pack import import_module_pack, validate_module_pack
from forgecad_agent.application.combined_glb import read_glb
from forgecad_agent.domain.concepts.connector_snapping import connector_alignment_error
from forgecad_agent.domain.concepts.models import Transform
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
from smoke_r3_module_pack_tooling import _reference_graph


EXPECTED_MODULE_IDS = {
    "module_armor_panel_01",
    "module_core_shell_01",
    "module_front_shell_01",
    "module_front_shell_02",
    "module_grip_shell_01",
    "module_lower_structure_01",
    "module_rear_shell_01",
    "module_side_accessory_01",
    "module_storage_visual_01",
    "module_top_accessory_01",
}


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Run an unclassified Connector matrix for a Blender visual candidate."
    )
    parser.add_argument("--pack-root", type=Path, required=True)
    args = parser.parse_args()
    pack = validate_module_pack(args.pack_root)
    _assert(
        {module.manifest.module_id for module in pack.modules} == EXPECTED_MODULE_IDS,
        "candidate Pack must contain the ten stable module IDs",
    )
    manifests = {module.manifest.module_id: module.manifest for module in pack.modules}

    with tempfile.TemporaryDirectory(
        prefix="forgecad_blender_candidate_connector_"
    ) as temporary:
        library_root = Path(temporary) / "ForgeCADLibrary"
        port = _free_port()
        base_url = f"http://127.0.0.1:{port}"
        process = _start_agent(library_root, port)
        try:
            _wait_for_health(base_url, process)
            _assert(
                len(import_module_pack(pack, base_url)) == 10,
                "candidate Pack import count mismatch",
            )
            project = _json_request(
                base_url,
                "/api/v1/projects",
                method="POST",
                body=_create_body(),
                idempotency_key="blender-candidate-connector-project",
            )
            graph = _reference_graph(project["project_id"])
            graph["graph_id"] = "mg_blender_candidate_connector_matrix_v1"
            validated = _json_request(
                base_url,
                f"/api/v1/module-graphs/{graph['graph_id']}/validate",
                method="POST",
                body={
                    "client_request_id": "blender-candidate-connector-graph",
                    "graph": graph,
                    "persist": True,
                },
                idempotency_key="blender-candidate-connector-graph",
            )
            _assert(validated["valid"] is True, "candidate graph was rejected")
            bound = _json_request(
                base_url,
                f"/api/v1/projects/{project['project_id']}/versions",
                method="POST",
                body={
                    "client_request_id": "blender-candidate-connector-bind",
                    "parent_version_id": project["current_version_id"],
                    "summary": "十模块 Blender 候选 Connector 替换与镜像技术演练。",
                    "spec": project["current_spec"],
                    "module_graph_id": graph["graph_id"],
                },
                idempotency_key="blender-candidate-connector-bind",
            )
            version_id = str(bound["current_version_id"])

            version_id, forward_preview = _replace_front(
                base_url,
                project_id=str(project["project_id"]),
                version_id=version_id,
                change_set_id="change_blender_candidate_front_01_to_02",
                target_module_id="module_front_shell_02",
                expected_connector_id="connector_front_02_core",
            )
            _assert_graph_alignment(forward_preview["preview_graph"], manifests)
            version_id, reverse_preview = _replace_front(
                base_url,
                project_id=str(project["project_id"]),
                version_id=version_id,
                change_set_id="change_blender_candidate_front_02_to_01",
                target_module_id="module_front_shell_01",
                expected_connector_id="connector_front_01_core",
            )
            _assert_graph_alignment(reverse_preview["preview_graph"], manifests)

            mirror_cases: list[str] = []
            for node_id in (
                "node_front",
                "node_rear",
                "node_grip",
                "node_top",
                "node_side",
                "node_lower",
                "node_storage",
                "node_armor",
            ):
                version_id, preview = _set_mirror(
                    base_url,
                    project_id=str(project["project_id"]),
                    version_id=version_id,
                    node_id=node_id,
                )
                preview_nodes = _nodes(preview["preview_graph"])
                _assert(
                    preview_nodes[node_id]["mirror_axis"] == "x",
                    f"{node_id} mirror state was not previewed",
                )
                _assert_graph_alignment(preview["preview_graph"], manifests)
                mirror_cases.append(node_id)
            locked_root_rejected = _assert_locked_root_rejected(
                base_url,
                project_id=str(project["project_id"]),
                version_id=version_id,
            )

            quality = _json_request(
                base_url,
                f"/api/v1/versions/{version_id}/quality-runs:inspect",
                method="POST",
                body={
                    "client_request_id": "blender-candidate-connector-quality",
                    "ruleset_version": "weapon-concept-geometry/1.3",
                },
                idempotency_key="blender-candidate-connector-quality",
            )
            _assert(
                quality["report"]["status"] in {"passed", "warning"},
                "candidate quality inspection failed after Connector operations",
            )
            exported = _json_request(
                base_url,
                f"/api/v1/versions/{version_id}/exports",
                method="POST",
                body={
                    "client_request_id": "blender-candidate-connector-export",
                    "profile": "game_asset",
                    "include_modules": True,
                    "include_combined_glb": True,
                    "include_quality_report": True,
                },
                idempotency_key="blender-candidate-connector-export",
            )
            _assert_combined_mirrors(
                base_url, str(exported["export_id"]), exported, set(mirror_cases)
            )
        finally:
            _stop_agent(process)

        restart_port = _free_port()
        restart_url = f"http://127.0.0.1:{restart_port}"
        restarted = _start_agent(library_root, restart_port)
        try:
            _wait_for_health(restart_url, restarted)
            version = _json_request(
                restart_url, f"/api/v1/versions/{version_id}", method="GET"
            )
            restored_graph = _json_request(
                restart_url,
                f"/api/v1/module-graphs/{version['module_graph_id']}",
                method="GET",
            )["graph"]
            _assert_graph_alignment(restored_graph, manifests)
            _assert(
                {
                    node["node_id"]
                    for node in restored_graph["nodes"]
                    if node["mirror_axis"] == "x"
                }
                == set(mirror_cases),
                "restart did not preserve candidate mirror states",
            )
            modules = _json_request(restart_url, "/api/v1/module-assets", method="GET")
            _assert(len(modules["items"]) == 10, "restart lost candidate modules")
        finally:
            _stop_agent(restarted)

    print(
        json.dumps(
            {
                "ok": True,
                "evidence_class": "unclassified",
                "formal_asset_evidence_eligible": False,
                "module_count": 10,
                "eligible_replacement_cases": 2,
                "replacement_successes": 2,
                "mirror_cases": mirror_cases,
                "mirror_successes": len(mirror_cases),
                "locked_root_rejected": locked_root_rejected,
                "connector_alignment_verified": True,
                "quality_status": quality["report"]["status"],
                "combined_glb_mirror_verified": True,
                "restart_restored": True,
            },
            ensure_ascii=False,
            indent=2,
        )
    )
    return 0


def _replace_front(
    base_url: str,
    *,
    project_id: str,
    version_id: str,
    change_set_id: str,
    target_module_id: str,
    expected_connector_id: str,
) -> tuple[str, dict[str, Any]]:
    change_set = _change_set(
        change_set_id,
        project_id,
        version_id,
        {
            "operation_id": f"op_{change_set_id}",
            "op": "replace_module",
            "node_id": "node_front",
            "module_id": target_module_id,
        },
    )
    _propose(base_url, version_id, change_set_id, change_set)
    preview = _json_request(
        base_url,
        f"/api/v1/change-sets/{change_set_id}:preview",
        method="POST",
        idempotency_key=f"preview-{change_set_id}",
    )
    nodes = _nodes(preview["preview_graph"])
    _assert(nodes["node_front"]["module_id"] == target_module_id, "front replacement failed")
    edge = next(
        item
        for item in preview["preview_graph"]["edges"]
        if item["edge_id"] == "edge_core_front"
    )
    _assert(
        edge["to_connector_id"] == expected_connector_id,
        "replacement did not remap the front Connector",
    )
    confirmed = _json_request(
        base_url,
        f"/api/v1/change-sets/{change_set_id}:confirm",
        method="POST",
        idempotency_key=f"confirm-{change_set_id}",
    )
    return str(confirmed["project"]["current_version_id"]), preview


def _set_mirror(
    base_url: str, *, project_id: str, version_id: str, node_id: str
) -> tuple[str, dict[str, Any]]:
    change_set_id = f"change_blender_candidate_mirror_{node_id.removeprefix('node_')}"
    change_set = _change_set(
        change_set_id,
        project_id,
        version_id,
        {
            "operation_id": f"op_{change_set_id}",
            "op": "set_mirror",
            "node_id": node_id,
            "mirror_axis": "x",
        },
    )
    _propose(base_url, version_id, change_set_id, change_set)
    preview = _json_request(
        base_url,
        f"/api/v1/change-sets/{change_set_id}:preview",
        method="POST",
        idempotency_key=f"preview-{change_set_id}",
    )
    confirmed = _json_request(
        base_url,
        f"/api/v1/change-sets/{change_set_id}:confirm",
        method="POST",
        idempotency_key=f"confirm-{change_set_id}",
    )
    return str(confirmed["project"]["current_version_id"]), preview


def _change_set(
    change_set_id: str,
    project_id: str,
    version_id: str,
    operation: dict[str, str],
) -> dict[str, Any]:
    return {
        "schema_version": "DesignChangeSet@1",
        "change_set_id": change_set_id,
        "project_id": project_id,
        "base_version_id": version_id,
        "summary": "Blender visual candidate Connector technical matrix.",
        "operations": [operation],
        "protected_node_ids": [],
        "status": "proposed",
    }


def _assert_locked_root_rejected(
    base_url: str, *, project_id: str, version_id: str
) -> bool:
    change_set_id = "change_blender_candidate_mirror_locked_core"
    _propose(
        base_url,
        version_id,
        change_set_id,
        _change_set(
            change_set_id,
            project_id,
            version_id,
            {
                "operation_id": f"op_{change_set_id}",
                "op": "set_mirror",
                "node_id": "node_core",
                "mirror_axis": "x",
            },
        ),
    )
    status, response = _json_request_allow_error(
        base_url,
        f"/api/v1/change-sets/{change_set_id}:preview",
        method="POST",
        idempotency_key=f"preview-{change_set_id}",
    )
    _assert(status == 400, f"locked core mirror preview returned {status}")
    _assert(
        response["error"]["code"] == "CHANGE_SET_INVALID"
        and "Locked ModuleGraph node cannot be changed" in response["error"]["message"],
        "locked core mirror returned the wrong diagnostic",
    )
    return True


def _propose(
    base_url: str,
    version_id: str,
    change_set_id: str,
    change_set: dict[str, Any],
) -> None:
    _json_request(
        base_url,
        f"/api/v1/versions/{version_id}/change-sets",
        method="POST",
        body={"client_request_id": f"propose-{change_set_id}", "change_set": change_set},
        idempotency_key=f"propose-{change_set_id}",
    )


def _assert_graph_alignment(graph: dict[str, Any], manifests: dict[str, Any]) -> None:
    nodes = _nodes(graph)
    for edge in graph["edges"]:
        first = nodes[edge["from_node_id"]]
        second = nodes[edge["to_node_id"]]
        first_connector = _connector(
            manifests[first["module_id"]], edge["from_connector_id"]
        )
        second_connector = _connector(
            manifests[second["module_id"]], edge["to_connector_id"]
        )
        distance_mm, rotation_degrees = connector_alignment_error(
            first_transform=Transform.model_validate(first["transform"]),
            first_connector=first_connector.transform,
            second_transform=Transform.model_validate(second["transform"]),
            second_connector=second_connector.transform,
            first_mirror_axis=first["mirror_axis"],
            second_mirror_axis=second["mirror_axis"],
        )
        _assert(
            distance_mm <= 1e-6 and rotation_degrees <= 1e-5,
            (
                f"{edge['edge_id']} was not snapped: "
                f"{distance_mm:.8f} mm / {rotation_degrees:.8f} deg"
            ),
        )


def _connector(manifest: Any, connector_id: str) -> Any:
    return next(
        connector
        for connector in manifest.connectors
        if connector.connector_id == connector_id
    )


def _assert_combined_mirrors(
    base_url: str,
    export_id: str,
    exported: dict[str, Any],
    mirrored_node_ids: set[str],
) -> None:
    with urllib.request.urlopen(
        f"{base_url}/api/v1/exports/{export_id}/combined.glb", timeout=20
    ) as response:
        payload = response.read()
    _assert(
        hashlib.sha256(payload).hexdigest() == exported["combined_glb_sha256"],
        "combined GLB download hash mismatch",
    )
    document, _ = read_glb(payload)
    wrappers = {
        str(node["extras"]["forgecad_node_id"]): node
        for node in document["nodes"]
        if "forgecad_node_id" in node.get("extras", {})
    }
    _assert(len(wrappers) == 9, "combined GLB wrapper count mismatch")
    for node_id, wrapper in wrappers.items():
        expected_x_scale = -1 if node_id in mirrored_node_ids else 1
        actual_x_scale = wrapper.get("scale", [1, 1, 1])[0]
        _assert(
            actual_x_scale == expected_x_scale,
            f"combined GLB mirror state mismatch for {node_id}",
        )


def _nodes(graph: dict[str, Any]) -> dict[str, dict[str, Any]]:
    return {str(node["node_id"]): node for node in graph["nodes"]}


if __name__ == "__main__":
    raise SystemExit(main())
