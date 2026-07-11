#!/usr/bin/env python3
"""Prove that a clean local library reaches Brief-ready ModuleGraph state unaided."""

from __future__ import annotations

import json
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


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad_first_run_") as temporary:
        library_root = Path(temporary) / "ForgeCADLibrary"
        port = _free_port()
        base_url = f"http://127.0.0.1:{port}"
        process = _start_agent(library_root, port)
        try:
            _wait_for_health(base_url, process)
            created = _json_request(
                base_url,
                "/api/v1/projects",
                method="POST",
                body=_create_body(),
                idempotency_key="first-run-project",
            )
            initialized = _json_request(
                base_url,
                f"/api/v1/projects/{created['project_id']}:initialize-workbench",
                method="POST",
                body={"client_request_id": "first-run-initialize"},
                idempotency_key="first-run-initialize",
            )
            version_id = initialized["current_version_id"]
            version = _json_request(base_url, f"/api/v1/versions/{version_id}", method="GET")
            modules = _json_request(base_url, "/api/v1/module-assets?pack_id=pack_weapon_concept_v1", method="GET")
            _assert(len(modules["items"]) == 10, "bundled Pack did not register ten modules")
            _assert(version["module_graph_id"].startswith("mg_starter_"), "starter graph was not bound")
            graph = _json_request(base_url, f"/api/v1/module-graphs/{version['module_graph_id']}", method="GET")
            _assert(len(graph["graph"]["nodes"]) == 9, "starter graph node count mismatch")
            with urllib.request.urlopen(
                f"{base_url}/api/v1/module-assets/module_core_shell_01/thumbnail", timeout=10
            ) as response:
                thumbnail = response.read(24)
                _assert(response.headers.get_content_type() == "image/png", "thumbnail MIME mismatch")
                _assert(thumbnail.startswith(b"\x89PNG\r\n\x1a\n"), "thumbnail bytes are not PNG")
            brief = _json_request(
                base_url,
                f"/api/v1/projects/{created['project_id']}/brief:interpret",
                method="POST",
                body={
                    "client_request_id": "first-run-brief",
                    "source_text": "紧凑工业、信号红点缀的非功能概念资产",
                    "reference_asset_ids": [],
                    "generator": "deterministic_rules",
                },
                idempotency_key="first-run-brief",
            )
            variants = _json_request(
                base_url,
                f"/api/v1/projects/{created['project_id']}/variants",
                method="POST",
                body={
                    "client_request_id": "first-run-variants",
                    "brief_id": brief["brief_id"],
                    "count": 3,
                    "generator": "deterministic_rules",
                },
                idempotency_key="first-run-variants",
            )
            _assert(len(variants["items"]) == 3, "first-run Brief did not produce A/B/C variants")
            replay = _json_request(
                base_url,
                f"/api/v1/projects/{created['project_id']}:initialize-workbench",
                method="POST",
                body={"client_request_id": "first-run-initialize"},
                idempotency_key="first-run-initialize",
            )
            _assert(replay["current_version_id"] == version_id, "starter initialization was not idempotent")
            print(json.dumps({
                "ok": True,
                "module_count": len(modules["items"]),
                "node_count": len(graph["graph"]["nodes"]),
                "variant_count": len(variants["items"]),
                "thumbnail_served": True,
                "initial_version_id": created["current_version_id"],
                "workbench_version_id": version_id,
            }, ensure_ascii=False, indent=2))
        finally:
            _stop_agent(process)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
