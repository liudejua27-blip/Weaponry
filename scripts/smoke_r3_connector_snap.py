#!/usr/bin/env python3
from __future__ import annotations

import json
import math
import sqlite3
import tempfile
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any

from forgecad_agent.domain.concepts.connector_snapping import (
    connector_alignment_error,
    snap_child_transform,
)
from forgecad_agent.application.combined_glb import read_glb
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
from smoke_r2_module_registry import _manifest, _minimal_glb, _register


def main() -> int:
    math_successes = _run_math_corpus()
    with tempfile.TemporaryDirectory(prefix="forgecad_r3_connector_snap_") as temporary_directory:
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
                idempotency_key="r3-snap-project",
            )
            project_id = project["project_id"]
            _register_fixture_modules(base_url)
            graph = _base_graph(project_id)
            validated = _json_request(
                base_url,
                f"/api/v1/module-graphs/{graph['graph_id']}/validate",
                method="POST",
                body={"client_request_id": "r3-snap-graph", "graph": graph, "persist": True},
                idempotency_key="r3-snap-graph",
            )
            _assert(validated["valid"] is True, "connector snap base graph did not validate")
            bound = _json_request(
                base_url,
                f"/api/v1/projects/{project_id}/versions",
                method="POST",
                body={
                    "client_request_id": "r3-snap-bind",
                    "parent_version_id": project["current_version_id"],
                    "summary": "绑定 Connector 自动吸附基线。",
                    "spec": project["current_spec"],
                    "module_graph_id": graph["graph_id"],
                },
                idempotency_key="r3-snap-bind",
            )
            version_2 = bound["current_version_id"]

            root_preview, root_confirmed = _replace_module(
                base_url,
                project_id=project_id,
                version_id=version_2,
                change_set_id="change_snap_root",
                node_id="node_core",
                module_id="module_core_shell_02",
            )
            root_nodes = _nodes(root_preview["preview_graph"])
            _assert_vector(root_nodes["node_core"]["transform"]["position"], [5, 10, 0])
            _assert_vector(root_nodes["node_front"]["transform"]["position"], [80, 18, 0])
            _assert_vector(root_nodes["node_grip"]["transform"]["position"], [17, -39, 0])
            _assert_vector(root_nodes["node_top"]["transform"]["position"], [80, 33, 0])
            version_3 = root_confirmed["project"]["current_version_id"]

            front_preview, front_confirmed = _replace_module(
                base_url,
                project_id=project_id,
                version_id=version_3,
                change_set_id="change_snap_front",
                node_id="node_front",
                module_id="module_front_shell_02",
            )
            front_nodes = _nodes(front_preview["preview_graph"])
            _assert_vector(front_nodes["node_front"]["transform"]["position"], [84, 16, 0])
            _assert_vector(front_nodes["node_top"]["transform"]["position"], [86, 33, 0])
            front_edge = next(
                edge
                for edge in front_preview["preview_graph"]["edges"]
                if edge["edge_id"] == "edge_core_front"
            )
            _assert(
                front_edge["to_connector_id"] == "connector_front_02_core",
                "replacement connector ID was not remapped before snapping",
            )
            version_4 = front_confirmed["project"]["current_version_id"]
            front_replay = _json_request(
                base_url,
                "/api/v1/change-sets/change_snap_front:confirm",
                method="POST",
                idempotency_key="r3-confirm-change_snap_front",
            )
            _assert(
                front_replay["project"]["current_version_id"] == version_4,
                "connector snap confirmation replay was not idempotent",
            )
            mirror_preview, mirror_confirmed = _set_mirror(
                base_url,
                project_id=project_id,
                version_id=version_4,
                node_id="node_top",
                mirror_axis="x",
            )
            mirror_nodes = _nodes(mirror_preview["preview_graph"])
            _assert(mirror_nodes["node_top"]["mirror_axis"] == "x", "mirror state missing")
            _assert_vector(mirror_nodes["node_top"]["transform"]["position"], [86, 33, 0])
            version_5 = mirror_confirmed["project"]["current_version_id"]
            mirror_export = _json_request(
                base_url,
                f"/api/v1/versions/{version_5}/exports",
                method="POST",
                body={
                    "client_request_id": "r3-snap-mirror-export",
                    "profile": "game_asset",
                    "include_modules": True,
                    "include_quality_report": False,
                },
                idempotency_key="r3-snap-mirror-export",
            )
            exported_top = next(
                item for item in mirror_export["manifest"]["modules"] if item["node_id"] == "node_top"
            )
            _assert(exported_top["mirror_axis"] == "x", "export manifest lost mirror state")
            with urllib.request.urlopen(
                f"{base_url}/api/v1/exports/{mirror_export['export_id']}/combined.glb",
                timeout=10,
            ) as response:
                combined_document, _ = read_glb(response.read())
            top_wrapper = next(
                node
                for node in combined_document["nodes"]
                if node.get("extras", {}).get("forgecad_node_id") == "node_top"
            )
            _assert_vector(top_wrapper["translation"], [0.086, 0.033, 0])
            _assert(top_wrapper["scale"][0] == -1, "combined GLB lost X mirror scale")
            repaired_version, connector_snap_verified = _misalign_and_repair_connector(
                base_url,
                project_id=project_id,
                version_id=version_5,
            )
            version_8 = _connect_cycle_constraint(
                base_url,
                project_id=project_id,
                version_id=repaired_version,
            )
            cycle_conflict_rejected = _assert_cycle_conflict_rejected(
                base_url,
                project_id=project_id,
                version_id=version_8,
            )
            locked_descendant_rejected = _assert_locked_descendant_rejected(base_url)
            timeline = _json_request(
                base_url,
                f"/api/v1/projects/{project_id}/change-sets",
                method="GET",
            )
            _assert(len(timeline["items"]) == 7, "ChangeSet timeline count mismatch")
            timeline_operations = [
                operation["op"]
                for item in timeline["items"]
                for operation in item["change_set"]["operations"]
            ]
            _assert("replace_module" in timeline_operations, "timeline lost replacement")
            _assert("set_mirror" in timeline_operations, "timeline lost mirror")
            timeline_query_evidence = _assert_timeline_query_features(
                base_url,
                project_id,
                timeline,
            )
        finally:
            _stop_agent(process)

        with sqlite3.connect(library_root / "library.db") as connection:
            graph_count = connection.execute("SELECT COUNT(*) FROM module_graphs").fetchone()[0]
            change_set_count = connection.execute(
                "SELECT COUNT(*) FROM design_change_sets"
            ).fetchone()[0]
            diagnostic_count = connection.execute(
                "SELECT COUNT(*) FROM design_change_sets WHERE diagnostic_json IS NOT NULL"
            ).fetchone()[0]
        _assert(graph_count == 8, "connector snap graph/version count mismatch")
        _assert(change_set_count == 8, "connector snap ChangeSet count mismatch")
        _assert(diagnostic_count == 2, "rejected ChangeSet diagnostics were not persisted")

        restart_port = _free_port()
        restart_url = f"http://127.0.0.1:{restart_port}"
        restarted = _start_agent(library_root, restart_port)
        try:
            _wait_for_health(restart_url, restarted)
            restored_version = _json_request(
                restart_url,
                f"/api/v1/versions/{version_8}",
                method="GET",
            )
            restored_graph = _json_request(
                restart_url,
                f"/api/v1/module-graphs/{restored_version['module_graph_id']}",
                method="GET",
            )
            restored_nodes = _nodes(restored_graph["graph"])
            _assert_vector(restored_nodes["node_front"]["transform"]["position"], [84, 16, 0])
            _assert_vector(restored_nodes["node_top"]["transform"]["position"], [86, 33, 0])
            _assert(restored_nodes["node_top"]["mirror_axis"] == "x", "restart lost mirror")
            _assert_vector(restored_nodes["node_grip"]["transform"]["position"], [17, -39, 0])
            restored_timeline = _json_request(
                restart_url,
                f"/api/v1/projects/{project_id}/change-sets",
                method="GET",
            )
            _assert(len(restored_timeline["items"]) == 7, "restart lost ChangeSet timeline")
            restored_rejected = _timeline_request(
                restart_url,
                project_id,
                status="rejected",
            )
            _assert(len(restored_rejected["items"]) == 1, "restart lost rejected filter")
            _assert(
                restored_rejected["items"][0]["diagnostic"]["code"]
                == "CHANGE_SET_INVALID",
                "restart lost rejected diagnostic",
            )
        finally:
            _stop_agent(restarted)

        print(
            json.dumps(
                {
                    "ok": True,
                    "math_cases": 100,
                    "math_successes": math_successes,
                    "synthetic_success_rate": math_successes / 100,
                    "root_replacement_relocated_children": True,
                    "child_replacement_relocated_descendants": True,
                    "connector_remap_verified": True,
                    "mirror_version_verified": True,
                    "mirror_export_verified": True,
                    "connector_snap_action_verified": connector_snap_verified,
                    "combined_transform_verified": True,
                    "idempotent_replay": True,
                    "cycle_conflict_rejected": cycle_conflict_rejected,
                    "locked_descendant_rejected": locked_descendant_rejected,
                    "restart_restored": True,
                    "timeline_restored": True,
                    "timeline_query": timeline_query_evidence,
                    "diagnostic_count": diagnostic_count,
                    "final_version_id": version_8,
                    "graph_count": graph_count,
                },
                ensure_ascii=False,
                indent=2,
            )
        )
    return 0


def _run_math_corpus() -> int:
    successes = 0
    for index in range(100):
        parent = Transform(
            position=[index * 0.7 - 20, index * -0.31 + 8, index * 0.13],
            rotation=[
                math.sin(index * 0.17) * 0.55,
                math.cos(index * 0.11) * 0.48,
                math.sin(index * 0.07) * 0.62,
            ],
            scale=[
                0.82 + (index % 7) * 0.05,
                0.88 + (index % 5) * 0.04,
                0.9 + (index % 3) * 0.06,
            ],
        )
        parent_connector = Transform(
            position=[18 + index * 0.09, -7 + index * 0.03, (index % 9) - 4],
            rotation=[0.11 * math.sin(index), -0.08 * math.cos(index), 0.04],
            scale=[1, 1, 1],
        )
        child_connector = Transform(
            position=[-12 - index * 0.04, 3 - index * 0.02, (index % 5) - 2],
            rotation=[-0.07, 0.05 * math.sin(index * 0.3), -0.03],
            scale=[1, 1, 1],
        )
        snapped = snap_child_transform(
            parent_transform=parent,
            parent_connector=parent_connector,
            child_scale=[
                0.86 + (index % 4) * 0.07,
                0.9 + (index % 6) * 0.03,
                0.94 + (index % 5) * 0.025,
            ],
            child_connector=child_connector,
            parent_mirror_axis=("x", "y", "z", "none")[index % 4],
            child_mirror_axis=("none", "z", "x", "y")[index % 4],
        )
        distance_mm, rotation_degrees = connector_alignment_error(
            first_transform=parent,
            first_connector=parent_connector,
            second_transform=snapped,
            second_connector=child_connector,
            first_mirror_axis=("x", "y", "z", "none")[index % 4],
            second_mirror_axis=("none", "z", "x", "y")[index % 4],
        )
        if distance_mm <= 1e-7 and rotation_degrees <= 1e-5:
            successes += 1
    _assert(successes >= 95, f"synthetic connector success rate below 95%: {successes}%")
    return successes


def _register_fixture_modules(base_url: str) -> None:
    fixtures = [
        (
            "module_core_shell_01",
            "asset_snap_core_01",
            "core_shell",
            [
                _connector("connector_core_01_front", "core.front", "shell_mount", [40, 5, 0]),
                _connector("connector_core_01_grip", "core.grip", "grip_mount", [10, -20, 0]),
            ],
        ),
        (
            "module_core_shell_02",
            "asset_snap_core_02",
            "core_shell",
            [
                _connector("connector_core_02_front", "core.front", "shell_mount", [45, 8, 0]),
                _connector("connector_core_02_grip", "core.grip", "grip_mount", [12, -24, 0]),
            ],
        ),
        (
            "module_front_shell_01",
            "asset_snap_front_01",
            "front_shell",
            [
                _connector("connector_front_01_core", "front.core", "shell_mount", [-30, 0, 0]),
                _connector("connector_front_01_top", "front.top", "top_mount", [0, 10, 0]),
            ],
        ),
        (
            "module_front_shell_02",
            "asset_snap_front_02",
            "front_shell",
            [
                _connector("connector_front_02_core", "front.core", "shell_mount", [-34, 2, 0]),
                _connector("connector_front_02_top", "front.top", "top_mount", [2, 12, 0]),
            ],
        ),
        (
            "module_grip_shell_01",
            "asset_snap_grip_01",
            "grip_shell",
            [
                _connector("connector_grip_core", "grip.core", "grip_mount", [0, 25, 0]),
                _connector(
                    "connector_grip_constraint",
                    "grip.constraint",
                    "constraint_mount",
                    [60, 65, 0],
                ),
            ],
        ),
        (
            "module_top_accessory_01",
            "asset_snap_top_01",
            "top_accessory",
            [
                _connector("connector_top_front", "top.front", "top_mount", [0, -5, 0]),
                _connector(
                    "connector_top_constraint",
                    "top.constraint",
                    "constraint_mount",
                    [0, 0, 0],
                ),
            ],
        ),
    ]
    for module_id, asset_id, category, connectors in fixtures:
        payload = _minimal_glb(module_id)
        _register(
            base_url,
            _manifest(
                module_id=module_id,
                asset_id=asset_id,
                category=category,
                payload=payload,
                connectors=connectors,
            ),
            payload,
            f"packs/weapon-concept/{module_id}.glb",
            f"r3-snap-register-{module_id}",
        )


def _connector(
    connector_id: str,
    slot: str,
    connector_type: str,
    position: list[float],
) -> dict[str, Any]:
    return {
        "connector_id": connector_id,
        "slot": slot,
        "connector_type": connector_type,
        "transform": {
            "position": position,
            "rotation": [0, 0, 0],
            "scale": [1, 1, 1],
        },
        "scale_range": [0.8, 1.2],
        "exclusive": True,
    }


def _base_graph(project_id: str) -> dict[str, Any]:
    return {
        "schema_version": "ModuleGraph@1",
        "graph_id": "mg_r3_connector_snap_base",
        "project_id": project_id,
        "root_node_id": "node_core",
        "nodes": [
            _node("node_core", "module_core_shell_01", [5, 10, 0]),
            _node("node_front", "module_front_shell_01", [75, 15, 0]),
            _node("node_grip", "module_grip_shell_01", [15, -35, 0]),
            _node("node_top", "module_top_accessory_01", [75, 30, 0]),
        ],
        "edges": [
            _edge(
                "edge_core_front",
                "node_core",
                "connector_core_01_front",
                "node_front",
                "connector_front_01_core",
            ),
            _edge(
                "edge_core_grip",
                "node_core",
                "connector_core_01_grip",
                "node_grip",
                "connector_grip_core",
            ),
            _edge(
                "edge_front_top",
                "node_front",
                "connector_front_01_top",
                "node_top",
                "connector_top_front",
            ),
        ],
    }


def _replace_module(
    base_url: str,
    *,
    project_id: str,
    version_id: str,
    change_set_id: str,
    node_id: str,
    module_id: str,
) -> tuple[dict[str, Any], dict[str, Any]]:
    change_set = {
        "schema_version": "DesignChangeSet@1",
        "change_set_id": change_set_id,
        "project_id": project_id,
        "base_version_id": version_id,
        "summary": f"Replace {node_id} and auto-snap its subtree.",
        "operations": [
            {
                "operation_id": f"op_{change_set_id}",
                "op": "replace_module",
                "node_id": node_id,
                "module_id": module_id,
            }
        ],
        "protected_node_ids": [],
        "status": "proposed",
    }
    _json_request(
        base_url,
        f"/api/v1/versions/{version_id}/change-sets",
        method="POST",
        body={"client_request_id": f"r3-propose-{change_set_id}", "change_set": change_set},
        idempotency_key=f"r3-propose-{change_set_id}",
    )
    preview = _json_request(
        base_url,
        f"/api/v1/change-sets/{change_set_id}:preview",
        method="POST",
        idempotency_key=f"r3-preview-{change_set_id}",
    )
    confirmed = _json_request(
        base_url,
        f"/api/v1/change-sets/{change_set_id}:confirm",
        method="POST",
        idempotency_key=f"r3-confirm-{change_set_id}",
    )
    return preview, confirmed


def _connect_cycle_constraint(
    base_url: str,
    *,
    project_id: str,
    version_id: str,
) -> str:
    change_set_id = "change_snap_add_cycle"
    change_set = {
        "schema_version": "DesignChangeSet@1",
        "change_set_id": change_set_id,
        "project_id": project_id,
        "base_version_id": version_id,
        "summary": "Add an extra constraint edge for snap conflict testing.",
        "operations": [
            {
                "operation_id": "op_change_snap_add_cycle",
                "op": "connect",
                "edge_id": "edge_cycle_constraint",
                "from_node_id": "node_grip",
                "from_connector_id": "connector_grip_constraint",
                "to_node_id": "node_top",
                "to_connector_id": "connector_top_constraint",
            }
        ],
        "protected_node_ids": [],
        "status": "proposed",
    }
    _json_request(
        base_url,
        f"/api/v1/versions/{version_id}/change-sets",
        method="POST",
        body={"client_request_id": "r3-propose-cycle", "change_set": change_set},
        idempotency_key="r3-propose-cycle",
    )
    _json_request(
        base_url,
        f"/api/v1/change-sets/{change_set_id}:preview",
        method="POST",
        idempotency_key="r3-preview-cycle",
    )
    confirmed = _json_request(
        base_url,
        f"/api/v1/change-sets/{change_set_id}:confirm",
        method="POST",
        idempotency_key="r3-confirm-cycle",
    )
    return str(confirmed["project"]["current_version_id"])


def _set_mirror(
    base_url: str,
    *,
    project_id: str,
    version_id: str,
    node_id: str,
    mirror_axis: str,
) -> tuple[dict[str, Any], dict[str, Any]]:
    change_set_id = "change_snap_mirror_top"
    change_set = {
        "schema_version": "DesignChangeSet@1",
        "change_set_id": change_set_id,
        "project_id": project_id,
        "base_version_id": version_id,
        "summary": f"Mirror {node_id} on {mirror_axis}.",
        "operations": [
            {
                "operation_id": "op_change_snap_mirror_top",
                "op": "set_mirror",
                "node_id": node_id,
                "mirror_axis": mirror_axis,
            }
        ],
        "protected_node_ids": [],
        "status": "proposed",
    }
    _json_request(
        base_url,
        f"/api/v1/versions/{version_id}/change-sets",
        method="POST",
        body={"client_request_id": "r3-propose-mirror", "change_set": change_set},
        idempotency_key="r3-propose-mirror",
    )
    preview = _json_request(
        base_url,
        f"/api/v1/change-sets/{change_set_id}:preview",
        method="POST",
        idempotency_key="r3-preview-mirror",
    )
    confirmed = _json_request(
        base_url,
        f"/api/v1/change-sets/{change_set_id}:confirm",
        method="POST",
        idempotency_key="r3-confirm-mirror",
    )
    return preview, confirmed


def _misalign_and_repair_connector(
    base_url: str,
    *,
    project_id: str,
    version_id: str,
) -> tuple[str, bool]:
    """Exercise the user-facing connector-snap action after a manual transform."""
    misaligned_id = "change_snap_misalign_grip"
    misaligned = {
        "schema_version": "DesignChangeSet@1",
        "change_set_id": misaligned_id,
        "project_id": project_id,
        "base_version_id": version_id,
        "summary": "Move grip away from its Connector for repair testing.",
        "operations": [
            {
                "operation_id": "op_change_snap_misalign_grip",
                "op": "set_transform",
                "node_id": "node_grip",
                "transform": {
                    "position": [31, -39, 0],
                    "rotation": [0, 0, 0],
                    "scale": [1, 1, 1],
                },
            }
        ],
        "protected_node_ids": ["node_core"],
        "status": "proposed",
    }
    _json_request(
        base_url,
        f"/api/v1/versions/{version_id}/change-sets",
        method="POST",
        body={"client_request_id": "r3-propose-misalign-grip", "change_set": misaligned},
        idempotency_key="r3-propose-misalign-grip",
    )
    _json_request(
        base_url,
        f"/api/v1/change-sets/{misaligned_id}:preview",
        method="POST",
        idempotency_key="r3-preview-misalign-grip",
    )
    misaligned_confirmed = _json_request(
        base_url,
        f"/api/v1/change-sets/{misaligned_id}:confirm",
        method="POST",
        idempotency_key="r3-confirm-misalign-grip",
    )
    misaligned_version = str(misaligned_confirmed["project"]["current_version_id"])
    proposed = _json_request(
        base_url,
        f"/api/v1/versions/{misaligned_version}/change-sets:connector-snap",
        method="POST",
        body={"client_request_id": "r3-connector-snap-grip", "node_id": "node_grip"},
        idempotency_key="r3-connector-snap-grip",
    )
    _assert(
        proposed["operations"][0]["op"] == "set_transform"
        and proposed["operations"][0]["node_id"] == "node_grip",
        "connector snap did not create a grip transform ChangeSet",
    )
    snap_id = proposed["change_set_id"]
    preview = _json_request(
        base_url,
        f"/api/v1/change-sets/{snap_id}:preview",
        method="POST",
        idempotency_key="r3-preview-connector-snap-grip",
    )
    _assert_vector(_nodes(preview["preview_graph"])["node_grip"]["transform"]["position"], [17, -39, 0])
    confirmed = _json_request(
        base_url,
        f"/api/v1/change-sets/{snap_id}:confirm",
        method="POST",
        idempotency_key="r3-confirm-connector-snap-grip",
    )
    return str(confirmed["project"]["current_version_id"]), True


def _assert_cycle_conflict_rejected(
    base_url: str,
    *,
    project_id: str,
    version_id: str,
) -> bool:
    change_set_id = "change_snap_cycle_conflict"
    change_set = {
        "schema_version": "DesignChangeSet@1",
        "change_set_id": change_set_id,
        "project_id": project_id,
        "base_version_id": version_id,
        "summary": "Replace root while an incompatible extra constraint is present.",
        "operations": [
            {
                "operation_id": "op_change_snap_cycle_conflict",
                "op": "replace_module",
                "node_id": "node_core",
                "module_id": "module_core_shell_01",
            }
        ],
        "protected_node_ids": [],
        "status": "proposed",
    }
    _json_request(
        base_url,
        f"/api/v1/versions/{version_id}/change-sets",
        method="POST",
        body={"client_request_id": "r3-propose-cycle-conflict", "change_set": change_set},
        idempotency_key="r3-propose-cycle-conflict",
    )
    status, body = _json_request_allow_error(
        base_url,
        f"/api/v1/change-sets/{change_set_id}:preview",
        method="POST",
        idempotency_key="r3-preview-cycle-conflict",
    )
    _assert(status == 400, f"incompatible cycle preview returned {status}")
    _assert(
        body["error"]["code"] == "CHANGE_SET_INVALID"
        and "Connector snap conflict" in body["error"]["message"],
        "incompatible cycle did not return a structured snap conflict",
    )
    return True


def _assert_timeline_query_features(
    base_url: str,
    project_id: str,
    full_timeline: dict[str, Any],
) -> dict[str, Any]:
    expected_ids = [
        item["change_set"]["change_set_id"] for item in full_timeline["items"]
    ]
    collected_ids: list[str] = []
    cursor = None
    page_count = 0
    while True:
        page = _timeline_request(base_url, project_id, cursor=cursor, limit=2)
        page_count += 1
        collected_ids.extend(
            item["change_set"]["change_set_id"] for item in page["items"]
        )
        cursor = page.get("next_cursor")
        if not cursor:
            break
    _assert(collected_ids == expected_ids, "timeline cursor pages changed order or lost rows")
    _assert(len(set(collected_ids)) == len(collected_ids), "timeline cursor duplicated rows")
    _assert(
        page_count == math.ceil(len(expected_ids) / 2),
        "timeline limit=2 returned an unexpected page count",
    )

    rejected = _timeline_request(base_url, project_id, status="rejected")
    _assert(len(rejected["items"]) == 1, "rejected status filter mismatch")
    rejected_item = rejected["items"][0]
    _assert(
        rejected_item["change_set"]["change_set_id"] == "change_snap_cycle_conflict",
        "rejected filter returned the wrong ChangeSet",
    )
    diagnostic = rejected_item.get("diagnostic")
    _assert(diagnostic is not None, "rejected ChangeSet lost diagnostic")
    _assert(diagnostic["code"] == "CHANGE_SET_INVALID", "diagnostic code mismatch")
    _assert(diagnostic["stage"] == "preview", "diagnostic stage mismatch")
    _assert(
        diagnostic["operation_ids"] == ["op_change_snap_cycle_conflict"],
        "diagnostic operation context mismatch",
    )
    _assert("node_core" in diagnostic["node_ids"], "diagnostic node context mismatch")

    searched = _timeline_request(base_url, project_id, q="cycle_conflict")
    _assert(len(searched["items"]) == 1, "timeline search mismatch")
    mirrors = _timeline_request(base_url, project_id, operation="set_mirror")
    _assert(len(mirrors["items"]) == 1, "operation filter mismatch")

    confirmed_page = _timeline_request(base_url, project_id, status="confirmed", limit=2)
    _assert(confirmed_page.get("next_cursor"), "confirmed filter did not return a cursor")
    mismatch_status, mismatch_body = _json_request_allow_error(
        base_url,
        f"/api/v1/projects/{project_id}/change-sets?"
        + urllib.parse.urlencode(
            {
                "status": "rejected",
                "cursor": confirmed_page["next_cursor"],
            }
        ),
        method="GET",
    )
    _assert(mismatch_status == 400, "cursor/filter mismatch was accepted")
    _assert(
        mismatch_body["error"]["code"] == "INVALID_CURSOR",
        "cursor/filter mismatch returned the wrong code",
    )
    invalid_status, invalid_body = _json_request_allow_error(
        base_url,
        f"/api/v1/projects/{project_id}/change-sets?cursor=not-a-cursor",
        method="GET",
    )
    _assert(invalid_status == 400, "invalid cursor was accepted")
    _assert(
        invalid_body["error"]["code"] == "INVALID_CURSOR",
        "invalid cursor returned the wrong code",
    )
    return {
        "page_count": page_count,
        "ordered_ids": collected_ids,
        "search_verified": True,
        "status_filter_verified": True,
        "operation_filter_verified": True,
        "cursor_filter_binding_verified": True,
        "rejected_diagnostic_verified": True,
    }


def _timeline_request(
    base_url: str,
    project_id: str,
    **parameters: Any,
) -> dict[str, Any]:
    query = urllib.parse.urlencode(
        {key: value for key, value in parameters.items() if value is not None}
    )
    suffix = f"?{query}" if query else ""
    return _json_request(
        base_url,
        f"/api/v1/projects/{project_id}/change-sets{suffix}",
        method="GET",
    )


def _assert_locked_descendant_rejected(base_url: str) -> bool:
    project_body = _create_body()
    project_body["client_request_id"] = "r3-snap-locked-project"
    project_body["name"] = "Connector locked descendant fixture"
    project = _json_request(
        base_url,
        "/api/v1/projects",
        method="POST",
        body=project_body,
        idempotency_key="r3-snap-locked-project",
    )
    graph = _base_graph(project["project_id"])
    graph["graph_id"] = "mg_r3_connector_locked"
    next(node for node in graph["nodes"] if node["node_id"] == "node_grip")["locked"] = True
    _json_request(
        base_url,
        f"/api/v1/module-graphs/{graph['graph_id']}/validate",
        method="POST",
        body={"client_request_id": "r3-snap-locked-graph", "graph": graph, "persist": True},
        idempotency_key="r3-snap-locked-graph",
    )
    bound = _json_request(
        base_url,
        f"/api/v1/projects/{project['project_id']}/versions",
        method="POST",
        body={
            "client_request_id": "r3-snap-locked-bind",
            "parent_version_id": project["current_version_id"],
            "summary": "Bind locked descendant fixture.",
            "spec": project["current_spec"],
            "module_graph_id": graph["graph_id"],
        },
        idempotency_key="r3-snap-locked-bind",
    )
    change_set_id = "change_snap_locked_descendant"
    change_set = {
        "schema_version": "DesignChangeSet@1",
        "change_set_id": change_set_id,
        "project_id": project["project_id"],
        "base_version_id": bound["current_version_id"],
        "summary": "Parent replacement must not move a locked descendant.",
        "operations": [
            {
                "operation_id": "op_change_snap_locked_descendant",
                "op": "replace_module",
                "node_id": "node_core",
                "module_id": "module_core_shell_02",
            }
        ],
        "protected_node_ids": ["node_grip"],
        "status": "proposed",
    }
    _json_request(
        base_url,
        f"/api/v1/versions/{bound['current_version_id']}/change-sets",
        method="POST",
        body={"client_request_id": "r3-propose-locked-descendant", "change_set": change_set},
        idempotency_key="r3-propose-locked-descendant",
    )
    status, body = _json_request_allow_error(
        base_url,
        f"/api/v1/change-sets/{change_set_id}:preview",
        method="POST",
        idempotency_key="r3-preview-locked-descendant",
    )
    _assert(status == 400, f"locked descendant preview returned {status}")
    _assert(
        body["error"]["code"] == "CHANGE_SET_INVALID"
        and "Locked ModuleGraph node cannot be repositioned" in body["error"]["message"],
        "parent replacement moved a locked descendant",
    )
    return True


def _node(node_id: str, module_id: str, position: list[float]) -> dict[str, Any]:
    return {
        "node_id": node_id,
        "module_id": module_id,
        "transform": {"position": position, "rotation": [0, 0, 0], "scale": [1, 1, 1]},
        "mirror_axis": "none",
        "locked": False,
        "visible": True,
    }


def _edge(
    edge_id: str,
    from_node_id: str,
    from_connector_id: str,
    to_node_id: str,
    to_connector_id: str,
) -> dict[str, Any]:
    return {
        "edge_id": edge_id,
        "from_node_id": from_node_id,
        "from_connector_id": from_connector_id,
        "to_node_id": to_node_id,
        "to_connector_id": to_connector_id,
        "status": "connected",
    }


def _nodes(graph: dict[str, Any]) -> dict[str, dict[str, Any]]:
    return {node["node_id"]: node for node in graph["nodes"]}


def _assert_vector(actual: list[float], expected: list[float]) -> None:
    _assert(
        all(abs(a - b) <= 1e-7 for a, b in zip(actual, expected)),
        f"vector mismatch: {actual} != {expected}",
    )


if __name__ == "__main__":
    raise SystemExit(main())
