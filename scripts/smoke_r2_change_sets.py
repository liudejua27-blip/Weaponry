#!/usr/bin/env python3
from __future__ import annotations

import json
import sqlite3
import tempfile
from pathlib import Path
from typing import Any, Dict

from forgecad_agent.domain.concepts.models import ModelQualityReport

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
from smoke_r2_module_registry import (
    _connector,
    _graph,
    _manifest,
    _minimal_glb,
    _register,
    _transform,
)


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad_r2_changes_") as temporary_directory:
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
                idempotency_key="r2-change-project",
            )
            project_id = project["project_id"]
            version_1 = project["current_version_id"]

            core_payload = _minimal_glb("core_shell_01")
            front_payload = _minimal_glb("front_shell_01")
            _register(
                base_url,
                _manifest(
                    module_id="module_core_shell_01",
                    asset_id="asset_core_shell_01",
                    category="core_shell",
                    payload=core_payload,
                    connectors=[
                        _connector("connector_core_front", "core.front", "shell_mount"),
                    ],
                ),
                core_payload,
                "packs/weapon-concept/core-shell-01.glb",
                "r2-change-register-core",
            )
            _register(
                base_url,
                _manifest(
                    module_id="module_front_shell_01",
                    asset_id="asset_front_shell_01",
                    category="front_shell",
                    payload=front_payload,
                    connectors=[
                        _connector("connector_front_core", "front.core", "shell_mount"),
                    ],
                ),
                front_payload,
                "packs/weapon-concept/front-shell-01.glb",
                "r2-change-register-front",
            )
            base_graph = _graph(project_id)
            graph_result = _json_request(
                base_url,
                f"/api/v1/module-graphs/{base_graph['graph_id']}/validate",
                method="POST",
                body={"client_request_id": "r2-change-graph", "graph": base_graph, "persist": True},
                idempotency_key="r2-change-graph",
            )
            _assert(graph_result["valid"] is True, "base graph did not validate")

            version_2_project = _json_request(
                base_url,
                f"/api/v1/projects/{project_id}/versions",
                method="POST",
                body={
                    "client_request_id": "r2-bind-graph-version",
                    "parent_version_id": version_1,
                    "summary": "绑定首个已验证 ModuleGraph。",
                    "spec": project["current_spec"],
                    "module_graph_id": base_graph["graph_id"],
                },
                idempotency_key="r2-bind-graph-version",
            )
            version_2 = version_2_project["current_version_id"]

            change_set = _change_set(project_id, version_2, "change_refine_front_01")
            proposed = _json_request(
                base_url,
                f"/api/v1/versions/{version_2}/change-sets",
                method="POST",
                body={"client_request_id": "r2-propose-change", "change_set": change_set},
                idempotency_key="r2-propose-change",
            )
            _assert(proposed["status"] == "proposed", "change set was not proposed")

            preview = _json_request(
                base_url,
                f"/api/v1/change-sets/{change_set['change_set_id']}:preview",
                method="POST",
                idempotency_key="r2-preview-change",
            )
            _assert(preview["change_set"]["status"] == "previewed", "preview status mismatch")
            _assert(preview["preview_graph"]["graph_id"] != base_graph["graph_id"], "preview reused base graph id")
            _assert(preview["preview_graph"]["nodes"][1]["transform"]["scale"][0] == 1.08, "preview transform missing")
            _assert(preview["preview_spec"]["style"]["detail_density"] == 0.76, "preview style missing")
            original_graph = _json_request(
                base_url,
                f"/api/v1/module-graphs/{base_graph['graph_id']}",
                method="GET",
            )
            _assert(original_graph["graph"]["nodes"][1]["transform"]["scale"][0] == 1.0, "preview mutated parent graph")

            confirmed = _json_request(
                base_url,
                f"/api/v1/change-sets/{change_set['change_set_id']}:confirm",
                method="POST",
                idempotency_key="r2-confirm-change",
            )
            version_3 = confirmed["project"]["current_version_id"]
            _assert(confirmed["change_set"]["status"] == "confirmed", "confirm status mismatch")
            _assert(version_3 not in {version_1, version_2}, "confirm did not create a child version")
            _assert(len(confirmed["project"]["versions"]) == 3, "confirm version history mismatch")
            _assert(confirmed["project"]["versions"][2]["parent_version_id"] == version_2, "confirm parent mismatch")
            _assert(confirmed["project"]["current_spec"]["style"]["detail_density"] == 0.76, "confirmed spec mismatch")
            confirm_replay = _json_request(
                base_url,
                f"/api/v1/change-sets/{change_set['change_set_id']}:confirm",
                method="POST",
                idempotency_key="r2-confirm-change",
            )
            _assert(confirm_replay["project"]["current_version_id"] == version_3, "confirm replay mismatch")

            quality_report = {
                "schema_version": "ModelQualityReport@1",
                "report_id": "quality_arctic_patrol_v3",
                "project_id": project_id,
                "version_id": version_3,
                "ruleset_version": "weapon-concept-quality/1.0",
                "status": "warning",
                "findings": [
                    {
                        "finding_id": "finding_symmetry_v3",
                        "check_id": "assembly.symmetry_deviation",
                        "category": "assembly",
                        "severity": "warning",
                        "status": "warning",
                        "node_ids": ["node_front"],
                        "measured_value": 0.4,
                        "threshold": 0.25,
                        "message": "前部外壳超出目标对称偏差。",
                        "suggestion": "重新吸附或接受非对称风格。",
                    }
                ],
                "created_at": "2026-07-10T12:00:00+00:00",
            }
            quality = _json_request(
                base_url,
                f"/api/v1/versions/{version_3}/quality-runs",
                method="POST",
                body={"client_request_id": "r2-quality-v3", "report": quality_report},
                idempotency_key="r2-quality-v3",
            )
            _assert(quality["report"]["status"] == "warning", "quality report status mismatch")
            _assert(quality["job_id"].startswith("job_"), "quality job id missing")
            quality_job = _json_request(
                base_url,
                f"/api/v1/jobs/{quality['job_id']}",
                method="GET",
            )
            _assert(quality_job["type"] == "quality_run", "quality job type mismatch")
            _assert(len(quality_job["events"]) == 3, "quality JobEvent count mismatch")
            quality_replay = _json_request(
                base_url,
                f"/api/v1/versions/{version_3}/quality-runs",
                method="POST",
                body={"client_request_id": "r2-quality-v3", "report": quality_report},
                idempotency_key="r2-quality-v3",
            )
            _assert(quality_replay["quality_run_id"] == quality["quality_run_id"], "quality replay mismatch")
            _assert(quality_replay["job_id"] == quality["job_id"], "quality job replay mismatch")
            quality_read = _json_request(
                base_url,
                f"/api/v1/quality-runs/{quality['quality_run_id']}",
                method="GET",
            )
            normalized_quality_report = ModelQualityReport.model_validate(
                quality_report
            ).model_dump(mode="json")
            _assert(
                quality_read["report"] == normalized_quality_report,
                "quality report round-trip mismatch",
            )

            protected = _change_set(project_id, version_3, "change_protected_core")
            protected["operations"][0]["node_id"] = "node_core"
            protected_status, protected_body = _json_request_allow_error(
                base_url,
                f"/api/v1/versions/{version_3}/change-sets",
                method="POST",
                body={"client_request_id": "r2-protected-change", "change_set": protected},
                idempotency_key="r2-protected-change",
            )
            _assert(
                protected_status == 422 and protected_body["error"]["code"] == "INVALID_REQUEST",
                "protected node change was not rejected",
            )

            locked_bypass = _change_set(project_id, version_3, "change_locked_bypass")
            locked_bypass["operations"][0]["node_id"] = "node_core"
            locked_bypass["protected_node_ids"] = []
            _json_request(
                base_url,
                f"/api/v1/versions/{version_3}/change-sets",
                method="POST",
                body={"client_request_id": "r2-locked-bypass", "change_set": locked_bypass},
                idempotency_key="r2-locked-bypass",
            )
            locked_preview_status, locked_preview_body = _json_request_allow_error(
                base_url,
                f"/api/v1/change-sets/{locked_bypass['change_set_id']}:preview",
                method="POST",
                idempotency_key="r2-locked-bypass-preview",
            )
            _assert(
                locked_preview_status == 400
                and locked_preview_body["error"]["code"] == "CHANGE_SET_INVALID",
                "service allowed a client to bypass a locked Graph node",
            )

            locked_mirror = _change_set(project_id, version_3, "change_locked_mirror")
            locked_mirror["operations"] = [
                {
                    "operation_id": "op_locked_mirror",
                    "op": "set_mirror",
                    "node_id": "node_core",
                    "mirror_axis": "x",
                }
            ]
            locked_mirror["protected_node_ids"] = []
            _json_request(
                base_url,
                f"/api/v1/versions/{version_3}/change-sets",
                method="POST",
                body={"client_request_id": "r2-locked-mirror", "change_set": locked_mirror},
                idempotency_key="r2-locked-mirror",
            )
            mirror_status, mirror_body = _json_request_allow_error(
                base_url,
                f"/api/v1/change-sets/{locked_mirror['change_set_id']}:preview",
                method="POST",
                idempotency_key="r2-locked-mirror-preview",
            )
            _assert(
                mirror_status == 400 and mirror_body["error"]["code"] == "CHANGE_SET_INVALID",
                "service allowed mirror on a locked Graph node",
            )

            stale_change = _change_set(project_id, version_3, "change_stale_front")
            _json_request(
                base_url,
                f"/api/v1/versions/{version_3}/change-sets",
                method="POST",
                body={"client_request_id": "r2-stale-propose", "change_set": stale_change},
                idempotency_key="r2-stale-propose",
            )
            _json_request(
                base_url,
                f"/api/v1/change-sets/{stale_change['change_set_id']}:preview",
                method="POST",
                idempotency_key="r2-stale-preview",
            )
            _json_request(
                base_url,
                f"/api/v1/projects/{project_id}/versions",
                method="POST",
                body={
                    "client_request_id": "r2-advance-current",
                    "parent_version_id": version_3,
                    "summary": "推进当前版本以制造 stale base。",
                    "spec": confirmed["project"]["current_spec"],
                    "module_graph_id": None,
                },
                idempotency_key="r2-advance-current",
            )
            stale_status, stale_body = _json_request_allow_error(
                base_url,
                f"/api/v1/change-sets/{stale_change['change_set_id']}:confirm",
                method="POST",
                idempotency_key="r2-stale-confirm",
            )
            _assert(
                stale_status == 409 and stale_body["error"]["code"] == "CHANGE_SET_STALE",
                "stale ChangeSet was not rejected",
            )
        finally:
            _stop_agent(process)

        database_path = library_root / "library.db"
        with sqlite3.connect(database_path) as connection:
            confirmed_row = connection.execute(
                "SELECT status, result_version_id FROM design_change_sets WHERE change_set_id = ?",
                (change_set["change_set_id"],),
            ).fetchone()
            stale_row = connection.execute(
                "SELECT status, diagnostic_json FROM design_change_sets WHERE change_set_id = ?",
                (stale_change["change_set_id"],),
            ).fetchone()
            version_count = connection.execute("SELECT COUNT(*) FROM project_versions").fetchone()[0]
            graph_count = connection.execute("SELECT COUNT(*) FROM module_graphs").fetchone()[0]
            quality_run_count = connection.execute("SELECT COUNT(*) FROM quality_runs").fetchone()[0]
            finding_count = connection.execute("SELECT COUNT(*) FROM quality_findings").fetchone()[0]
            concept_job_count = connection.execute("SELECT COUNT(*) FROM concept_jobs").fetchone()[0]
            concept_event_count = connection.execute("SELECT COUNT(*) FROM concept_job_events").fetchone()[0]
        _assert(confirmed_row == ("confirmed", version_3), "confirmed ChangeSet trace mismatch")
        _assert(stale_row[0] == "stale", "stale ChangeSet was not persisted")
        stale_diagnostic = json.loads(stale_row[1])
        _assert(
            stale_diagnostic["code"] == "CHANGE_SET_STALE"
            and stale_diagnostic["stage"] == "confirm"
            and stale_diagnostic["recoverable"] is True,
            "stale ChangeSet diagnostic was not persisted",
        )
        _assert(version_count == 4, "unexpected project version count")
        _assert(graph_count == 2, "preview should persist exactly one child graph on confirm")
        _assert(quality_run_count == 1, "quality run count mismatch")
        _assert(finding_count == 1, "quality finding count mismatch")
        _assert(concept_job_count == 2, "graph/quality jobs were not persisted")
        _assert(concept_event_count == 6, "graph/quality events were not persisted")

        print(
            json.dumps(
                {
                    "ok": True,
                    "project_id": project_id,
                    "base_version_id": version_2,
                    "confirmed_version_id": version_3,
                    "confirmed_change_set_id": change_set["change_set_id"],
                    "stale_change_set_id": stale_change["change_set_id"],
                    "stale_diagnostic_code": stale_diagnostic["code"],
                    "version_count": version_count,
                    "graph_count": graph_count,
                    "quality_run_count": quality_run_count,
                    "finding_count": finding_count,
                    "concept_job_count": concept_job_count,
                    "concept_event_count": concept_event_count,
                },
                ensure_ascii=False,
                indent=2,
            )
        )
    return 0


def _change_set(project_id: str, version_id: str, change_set_id: str) -> Dict[str, Any]:
    return {
        "schema_version": "DesignChangeSet@1",
        "change_set_id": change_set_id,
        "project_id": project_id,
        "base_version_id": version_id,
        "summary": "调整前部比例和细节密度，保护核心外壳。",
        "operations": [
            {
                "operation_id": f"op_{change_set_id}_front_scale",
                "op": "set_transform",
                "node_id": "node_front",
                "transform": {
                    **_transform(),
                    "scale": [1.08, 0.96, 1.0],
                },
            },
            {
                "operation_id": f"op_{change_set_id}_detail",
                "op": "set_style",
                "path": "style.detail_density",
                "value": 0.76,
            },
        ],
        "protected_node_ids": ["node_core"],
        "status": "proposed",
    }


if __name__ == "__main__":
    raise SystemExit(main())
