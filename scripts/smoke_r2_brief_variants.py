#!/usr/bin/env python3
from __future__ import annotations

import json
import sqlite3
import tempfile
import urllib.request
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
    with tempfile.TemporaryDirectory(prefix="forgecad_r2_briefs_") as temporary_directory:
        library_root = Path(temporary_directory) / "ForgeCADLibrary"
        port = _free_port()
        base_url = f"http://127.0.0.1:{port}"
        process = _start_agent(library_root, port)
        try:
            _wait_for_health(base_url, process)
            provider_settings = _json_request(
                base_url, "/api/provider-settings", method="GET"
            )
            concept_planner_settings = [
                item
                for item in provider_settings["providers"]
                if item["provider_id"] == "deterministic_concept_rules"
            ]
            _assert(
                len(concept_planner_settings) == 1
                and concept_planner_settings[0]["status"] == "configured",
                "Concept Planner provider settings missing",
            )
            project = _json_request(
                base_url,
                "/api/v1/projects",
                method="POST",
                body=_create_body(),
                idempotency_key="r2-brief-project",
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
                "r2-brief-register-core",
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
                "r2-brief-register-front",
            )
            graph = _graph(project_id)
            _json_request(
                base_url,
                f"/api/v1/module-graphs/{graph['graph_id']}/validate",
                method="POST",
                body={"client_request_id": "r2-brief-graph", "graph": graph, "persist": True},
                idempotency_key="r2-brief-graph",
            )
            bound = _json_request(
                base_url,
                f"/api/v1/projects/{project_id}/versions",
                method="POST",
                body={
                    "client_request_id": "r2-brief-bind-graph",
                    "parent_version_id": version_1,
                    "summary": "绑定 Variant 基准 ModuleGraph。",
                    "spec": project["current_spec"],
                    "module_graph_id": graph["graph_id"],
                },
                idempotency_key="r2-brief-bind-graph",
            )

            brief_body = {
                "client_request_id": "r2-brief-interpret",
                "source_text": "寒地巡逻 S1：紧凑、工业、石墨灰、精密细节，提供三种非功能展示比例方案。",
                "reference_asset_ids": [],
                "generator": "deterministic_rules",
            }
            brief = _json_request(
                base_url,
                f"/api/v1/projects/{project_id}/brief:interpret",
                method="POST",
                body=brief_body,
                idempotency_key="r2-brief-interpret",
            )
            _assert(brief["status"] == "interpreted", "brief status mismatch")
            _assert(brief["job_id"].startswith("job_"), "brief job id missing")
            _assert(
                brief["interpreted_spec"]["proportions"]["overall_length_mm"] == 207.0,
                "brief compact proportion was not interpreted",
            )
            _assert(
                brief["interpreted_spec"]["style"]["detail_density"] == 0.82,
                "brief detail density was not interpreted",
            )
            _assert(
                brief["planner_provenance"]["generator"] == "deterministic_rules"
                and brief["planner_provenance"]["provider_id"]
                == "deterministic_concept_rules"
                and brief["planner_provenance"]["fallback_used"] is False,
                "brief planner provenance mismatch",
            )
            _assert(
                len(brief["planner_provenance"]["input_sha256"]) == 64
                and len(brief["planner_provenance"]["output_sha256"]) == 64,
                "brief planner hashes missing",
            )
            brief_replay = _json_request(
                base_url,
                f"/api/v1/projects/{project_id}/brief:interpret",
                method="POST",
                body=brief_body,
                idempotency_key="r2-brief-interpret",
            )
            _assert(brief_replay["brief_id"] == brief["brief_id"], "brief replay mismatch")
            brief_job = _json_request(
                base_url,
                f"/api/v1/jobs/{brief['job_id']}",
                method="GET",
            )
            _assert(brief_job["status"] == "succeeded", "brief job status mismatch")
            _assert(len(brief_job["events"]) == 2, "brief JobEvent count mismatch")

            variants = _json_request(
                base_url,
                f"/api/v1/projects/{project_id}/variants",
                method="POST",
                body={
                    "client_request_id": "r2-variants-generate",
                    "brief_id": brief["brief_id"],
                    "count": 3,
                    "generator": "deterministic_rules",
                },
                idempotency_key="r2-variants-generate",
            )
            _assert(len(variants["items"]) == 3, "variant count mismatch")
            _assert(variants["job_id"].startswith("job_"), "variant job id missing")
            graph_ids = {item["module_graph"]["graph_id"] for item in variants["items"]}
            scales = [item["module_graph"]["nodes"][1]["transform"]["scale"][0] for item in variants["items"]]
            scale_vectors = [
                item["module_graph"]["nodes"][1]["transform"]["scale"]
                for item in variants["items"]
            ]
            _assert(len(graph_ids) == 3, "variant graph ids were not unique")
            _assert(scales == [0.9, 1.02, 1.1], "A/B/C proportions were not distinct")
            _assert(
                scale_vectors == [[0.9, 0.9, 0.9], [1.02, 1.08, 1.08], [1.1, 1.04, 1.04]],
                "A/B/C structural scale vectors mismatch",
            )
            registered_module_ids = {"module_core_shell_01", "module_front_shell_01"}
            _assert(
                all(
                    item["recommended_module_ids"]
                    and set(item["recommended_module_ids"]) <= registered_module_ids
                    and item["rationale"]
                    and item["planner_provenance"]["generator"]
                    == "deterministic_rules"
                    for item in variants["items"]
                ),
                "variant registry recommendations or provenance missing",
            )
            _assert(all(item["status"] == "proposed" for item in variants["items"]), "variant proposal status mismatch")
            variant_events = _json_request(
                base_url,
                f"/api/v1/jobs/{variants['job_id']}/events.json",
                method="GET",
            )
            _assert(len(variant_events["items"]) == 3, "variant JobEvent count mismatch")
            _assert(
                all(event["schema_version"] == "JobEvent@2" for event in variant_events["items"]),
                "variant events did not use JobEvent@2",
            )
            resumed_events = _json_request(
                base_url,
                f"/api/v1/jobs/{variants['job_id']}/events.json?after={variant_events['items'][0]['event_id']}",
                method="GET",
            )
            _assert(len(resumed_events["items"]) == 2, "event cursor replay mismatch")
            with urllib.request.urlopen(
                f"{base_url}/api/v1/jobs/{variants['job_id']}/events",
                timeout=10,
            ) as response:
                sse_payload = response.read().decode("utf-8")
            _assert("event: concept.job.event" in sse_payload, "Concept SSE event type missing")
            _assert("JobEvent@2" in sse_payload, "Concept SSE payload schema missing")

            selected_id = variants["items"][1]["variant_id"]
            selected = _json_request(
                base_url,
                f"/api/v1/projects/{project_id}/variants/{selected_id}:select",
                method="POST",
                body={"client_request_id": "r2-select-variant-b"},
                idempotency_key="r2-select-variant-b",
            )
            _assert(selected["rank"] == 2 and selected["status"] == "selected", "variant B selection mismatch")
            listed = _json_request(
                base_url,
                f"/api/v1/projects/{project_id}/variants?brief_id={brief['brief_id']}",
                method="GET",
            )
            statuses = [item["status"] for item in listed["items"]]
            _assert(statuses == ["rejected", "selected", "rejected"], "variant selection statuses mismatch")
            confirmed_brief = _json_request(
                base_url,
                f"/api/v1/projects/{project_id}/briefs/{brief['brief_id']}",
                method="GET",
            )
            _assert(confirmed_brief["status"] == "confirmed", "brief was not confirmed by selection")
        finally:
            _stop_agent(process)

        database_path = library_root / "library.db"
        with sqlite3.connect(database_path) as connection:
            connection.row_factory = sqlite3.Row
            brief_count = connection.execute("SELECT COUNT(*) FROM design_briefs").fetchone()[0]
            variant_count = connection.execute("SELECT COUNT(*) FROM design_variants").fetchone()[0]
            selected_count = connection.execute(
                "SELECT COUNT(*) FROM design_variants WHERE status = 'selected'"
            ).fetchone()[0]
            job_count = connection.execute("SELECT COUNT(*) FROM concept_jobs").fetchone()[0]
            event_count = connection.execute("SELECT COUNT(*) FROM concept_job_events").fetchone()[0]
            stored_brief = connection.execute(
                "SELECT planner_provenance_json FROM design_briefs WHERE brief_id = ?",
                (brief["brief_id"],),
            ).fetchone()
            stored_variants = connection.execute(
                """
                SELECT recommended_module_ids_json, rationale_json, planner_provenance_json
                FROM design_variants
                WHERE brief_id = ?
                ORDER BY rank
                """,
                (brief["brief_id"],),
            ).fetchall()
        _assert(brief_count == 1, "brief table count mismatch")
        _assert(variant_count == 3, "variant table count mismatch")
        _assert(selected_count == 1, "selected variant count mismatch")
        _assert(job_count == 3, "concept job count mismatch")
        _assert(event_count == 8, "concept JobEvent count mismatch")
        _assert(
            json.loads(stored_brief["planner_provenance_json"])["provider_id"]
            == "deterministic_concept_rules",
            "brief provenance was not persisted",
        )
        _assert(
            len(stored_variants) == 3
            and all(json.loads(row["recommended_module_ids_json"]) for row in stored_variants)
            and all(json.loads(row["rationale_json"]) for row in stored_variants)
            and all(
                json.loads(row["planner_provenance_json"])["generator"]
                == "deterministic_rules"
                for row in stored_variants
            ),
            "variant planner metadata was not persisted",
        )

        restart_port = _free_port()
        restart_url = f"http://127.0.0.1:{restart_port}"
        restarted_process = _start_agent(library_root, restart_port)
        try:
            _wait_for_health(restart_url, restarted_process)
            restored = _json_request(
                restart_url,
                f"/api/v1/projects/{project_id}/variants?brief_id={brief['brief_id']}",
                method="GET",
            )
            _assert(
                [item["status"] for item in restored["items"]]
                == ["rejected", "selected", "rejected"],
                "restart did not restore variant selection",
            )
            _assert(
                all(item["planner_provenance"]["input_sha256"] for item in restored["items"]),
                "restart did not restore planner provenance",
            )
            restored_job = _json_request(
                restart_url,
                f"/api/v1/jobs/{variants['job_id']}",
                method="GET",
            )
            _assert(len(restored_job["events"]) == 3, "restart did not restore Concept Job events")
        finally:
            _stop_agent(restarted_process)

        print(
            json.dumps(
                {
                    "ok": True,
                    "project_id": project_id,
                    "brief_id": brief["brief_id"],
                    "variant_count": variant_count,
                    "selected_variant_id": selected_id,
                    "scales": scales,
                    "generator": "deterministic_rules",
                    "interpreted_overall_length_mm": 207.0,
                    "planner_provenance_persisted": True,
                    "provider_settings_visible": True,
                    "job_count": job_count,
                    "event_count": event_count,
                },
                ensure_ascii=False,
                indent=2,
            )
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
