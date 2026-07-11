#!/usr/bin/env python3
from __future__ import annotations

import json
import sqlite3
import tempfile
from pathlib import Path

from smoke_r2_concept_projects import (
    _assert,
    _create_body,
    _free_port,
    _json_request,
    _start_agent,
    _stop_agent,
    _wait_for_health,
)
from smoke_r2_module_registry import (
    _connector,
    _graph,
    _manifest,
    _minimal_glb,
    _register,
)


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad_r4_change_api_") as temporary_directory:
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
                idempotency_key="r4-change-project",
            )
            project_id = project["project_id"]
            version_1 = project["current_version_id"]
            _register_modules(base_url)
            graph = _graph(project_id)
            graph_result = _json_request(
                base_url,
                f"/api/v1/module-graphs/{graph['graph_id']}/validate",
                method="POST",
                body={
                    "client_request_id": "r4-change-graph",
                    "graph": graph,
                    "persist": True,
                },
                idempotency_key="r4-change-graph",
            )
            _assert(graph_result["valid"] is True, "Change Planner base graph invalid")
            bound = _json_request(
                base_url,
                f"/api/v1/projects/{project_id}/versions",
                method="POST",
                body={
                    "client_request_id": "r4-change-bind",
                    "parent_version_id": version_1,
                    "summary": "绑定 Change Planner 基准 Graph。",
                    "spec": project["current_spec"],
                    "module_graph_id": graph["graph_id"],
                },
                idempotency_key="r4-change-bind",
            )
            version_2 = bound["current_version_id"]
            plan_body = {
                "client_request_id": "r4-change-plan",
                "instruction": (
                    "将选中前部替换为候选模块，整体长度调整为 218 mm，"
                    "细节密度调整为 84%。"
                ),
                "generator": "deterministic_rules",
                "selected_node_id": "node_front",
                "selected_module_id": "module_front_shell_02",
            }
            planned = _json_request(
                base_url,
                f"/api/v1/versions/{version_2}/change-sets:plan",
                method="POST",
                body=plan_body,
                idempotency_key="r4-change-plan",
            )
            _assert(
                planned["change_set"]["status"] == "proposed"
                and [item["op"] for item in planned["change_set"]["operations"]]
                == ["replace_module", "set_parameter", "set_style"],
                "deterministic Change Planner operations mismatch",
            )
            _assert(
                planned["planner_provenance"]["generator"] == "deterministic_rules"
                and planned["planner_provenance"]["fallback_used"] is False
                and len(planned["planner_provenance"]["input_sha256"]) == 64
                and len(planned["planner_provenance"]["output_sha256"]) == 64,
                "Change Planner provenance mismatch",
            )
            _assert(planned["job_id"].startswith("job_"), "Change Planner job missing")
            replay = _json_request(
                base_url,
                f"/api/v1/versions/{version_2}/change-sets:plan",
                method="POST",
                body=plan_body,
                idempotency_key="r4-change-plan",
            )
            _assert(
                replay["change_set"]["change_set_id"]
                == planned["change_set"]["change_set_id"],
                "Change Planner idempotency replay mismatch",
            )
            job = _json_request(
                base_url, f"/api/v1/jobs/{planned['job_id']}", method="GET"
            )
            _assert(
                job["type"] == "concept_change_plan"
                and len(job["events"]) == 3
                and job["outputs"]["operation_count"] == 3,
                "Change Planner JobEvent trace mismatch",
            )

            change_set_id = planned["change_set"]["change_set_id"]
            preview = _json_request(
                base_url,
                f"/api/v1/change-sets/{change_set_id}:preview",
                method="POST",
                idempotency_key="r4-change-preview",
            )
            preview_nodes = {
                item["node_id"]: item for item in preview["preview_graph"]["nodes"]
            }
            _assert(
                preview["change_set"]["status"] == "previewed"
                and preview["preview_spec"]["proportions"]["overall_length_mm"] == 218.0
                and preview["preview_spec"]["style"]["detail_density"] == 0.84
                and preview_nodes["node_front"]["module_id"]
                == "module_front_shell_02",
                "Change Planner ghost preview mismatch",
            )
            _assert(
                bound["current_version_id"] == version_2,
                "ghost preview unexpectedly changed the current version",
            )
            confirmed = _json_request(
                base_url,
                f"/api/v1/change-sets/{change_set_id}:confirm",
                method="POST",
                idempotency_key="r4-change-confirm",
            )
            version_3 = confirmed["project"]["current_version_id"]
            _assert(
                confirmed["change_set"]["status"] == "confirmed"
                and version_3 != version_2
                and confirmed["project"]["current_spec"]["proportions"][
                    "overall_length_mm"
                ]
                == 218.0,
                "Change Planner confirmation did not create the child version",
            )

            discard_plan = _json_request(
                base_url,
                f"/api/v1/versions/{version_3}/change-sets:plan",
                method="POST",
                body={
                    "client_request_id": "r4-change-discard-plan",
                    "instruction": "主体高度调整为 51 mm。",
                    "generator": "deterministic_rules",
                },
                idempotency_key="r4-change-discard-plan",
            )
            discard_id = discard_plan["change_set"]["change_set_id"]
            _json_request(
                base_url,
                f"/api/v1/change-sets/{discard_id}:preview",
                method="POST",
                idempotency_key="r4-change-discard-preview",
            )
            rejected = _json_request(
                base_url,
                f"/api/v1/change-sets/{discard_id}:reject",
                method="POST",
                idempotency_key="r4-change-discard",
            )
            _assert(rejected["status"] == "rejected", "ghost preview was not rejected")
            current_after_reject = _json_request(
                base_url, f"/api/v1/projects/{project_id}", method="GET"
            )
            _assert(
                current_after_reject["current_version_id"] == version_3,
                "rejecting ghost preview changed the current version",
            )
            timeline = _json_request(
                base_url,
                f"/api/v1/projects/{project_id}/change-sets",
                method="GET",
            )
            _assert(
                len(timeline["items"]) == 2
                and all(item["actor_type"] == "planner" for item in timeline["items"])
                and all(item["planner_instruction"] for item in timeline["items"])
                and all(item["planner_provenance"] for item in timeline["items"])
                and timeline["items"][0]["diagnostic"]["code"]
                == "CHANGE_SET_DISCARDED",
                "Change Planner timeline provenance mismatch",
            )
        except Exception as exc:
            _stop_agent(process)
            agent_output = process.stdout.read() if process.stdout else ""
            raise RuntimeError(
                f"Change Planner API smoke failed: {exc}\nAgent output:\n{agent_output}"
            ) from exc
        finally:
            if process.poll() is None:
                _stop_agent(process)

        restart_port = _free_port()
        restart_url = f"http://127.0.0.1:{restart_port}"
        restart_process = _start_agent(library_root, restart_port)
        try:
            _wait_for_health(restart_url, restart_process)
            restored = _json_request(
                restart_url,
                f"/api/v1/projects/{project_id}/change-sets",
                method="GET",
            )
            _assert(
                len(restored["items"]) == 2
                and restored["items"][0]["planner_job_id"]
                == discard_plan["job_id"]
                and restored["items"][1]["planner_job_id"] == planned["job_id"],
                "Change Planner provenance did not survive restart",
            )
        finally:
            _stop_agent(restart_process)

        database_path = library_root / "library.db"
        with sqlite3.connect(database_path) as connection:
            planner_rows = connection.execute(
                """
                SELECT COUNT(*), COUNT(planner_provenance_json), COUNT(planner_job_id)
                FROM design_change_sets
                WHERE actor_type = 'planner'
                """
            ).fetchone()
            version_count = connection.execute(
                "SELECT COUNT(*) FROM project_versions"
            ).fetchone()[0]
        _assert(planner_rows == (2, 2, 2), "Change Planner DB provenance mismatch")
        _assert(version_count == 3, "ghost preview created an unexpected version")
        print(
            json.dumps(
                {
                    "ok": True,
                    "project_id": project_id,
                    "base_version_id": version_2,
                    "confirmed_version_id": version_3,
                    "confirmed_change_set_id": change_set_id,
                    "discarded_change_set_id": discard_id,
                    "planner_rows": planner_rows[0],
                    "version_count": version_count,
                    "restart_provenance_verified": True,
                },
                ensure_ascii=False,
                indent=2,
            )
        )
    return 0


def _register_modules(base_url: str) -> None:
    modules = (
        (
            "module_core_shell_01",
            "asset_core_shell_01",
            "core_shell",
            "connector_core_front",
            "core.front",
            "core-shell-01.glb",
        ),
        (
            "module_front_shell_01",
            "asset_front_shell_01",
            "front_shell",
            "connector_front_core",
            "front.core",
            "front-shell-01.glb",
        ),
        (
            "module_front_shell_02",
            "asset_front_shell_02",
            "front_shell",
            "connector_front_02_core",
            "front.core",
            "front-shell-02.glb",
        ),
    )
    for index, (
        module_id,
        asset_id,
        category,
        connector_id,
        slot,
        filename,
    ) in enumerate(modules, start=1):
        payload = _minimal_glb(module_id)
        _register(
            base_url,
            _manifest(
                module_id=module_id,
                asset_id=asset_id,
                category=category,
                payload=payload,
                connectors=[_connector(connector_id, slot, "shell_mount")],
            ),
            payload,
            f"packs/weapon-concept/{filename}",
            f"r4-change-register-{index}",
        )


if __name__ == "__main__":
    raise SystemExit(main())
