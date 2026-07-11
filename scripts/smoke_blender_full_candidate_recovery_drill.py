#!/usr/bin/env python3
"""Measure a three-run recovery drill for the ten-module Blender visual candidate."""

from __future__ import annotations

import argparse
import json
import tempfile
from pathlib import Path

from concept_module_pack import import_module_pack, validate_module_pack
from library_recovery_drill import REPORT_NAME, run_recovery_drill
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
        description="Run an unclassified three-repeat recovery drill for a Blender visual candidate."
    )
    parser.add_argument("--pack-root", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    pack = validate_module_pack(args.pack_root)
    _assert(
        {module.manifest.module_id for module in pack.modules} == EXPECTED_MODULE_IDS,
        "candidate Pack must contain the ten stable module IDs",
    )

    with tempfile.TemporaryDirectory(
        prefix="forgecad_blender_candidate_recovery_"
    ) as temporary:
        library_root = Path(temporary) / "ForgeCADLibrary"
        _create_candidate_library(pack, library_root)
        run_recovery_drill(
            library_root,
            args.output,
            repeats=3,
            evidence_class="unclassified",
        )

    report = json.loads((args.output / REPORT_NAME).read_text(encoding="utf-8"))
    _assert(len(report["runs"]) == 3, "recovery drill repeat count mismatch")
    _assert(
        report["evidence"]["formal_asset_evidence_eligible"] is False,
        "candidate drill was treated as formal evidence",
    )
    for run in report["runs"]:
        readback = run["agent_readback"]
        _assert(readback["module_count"] == 10, "restored module count mismatch")
        _assert(
            readback["module_download_count"] == 10, "restored download count mismatch"
        )
        _assert(
            readback["module_hashes_verified"] is True,
            "restored module hashes mismatch",
        )
        _assert(
            readback["known_fixture_module_count"] == 0,
            "Blender candidate was classified as a known fixture",
        )

    print(
        json.dumps(
            {
                "ok": True,
                "evidence_class": report["evidence"]["declared_class"],
                "formal_asset_evidence_eligible": False,
                "module_count": 10,
                "repeats": 3,
                "report": str(args.output / REPORT_NAME),
                "stable_source_snapshot": report["summary"]["stable_source_snapshot"],
                "duration_ms": report["summary"]["duration_ms"],
            },
            ensure_ascii=False,
            indent=2,
        )
    )
    return 0


def _create_candidate_library(pack: object, library_root: Path) -> None:
    port = _free_port()
    base_url = f"http://127.0.0.1:{port}"
    process = _start_agent(library_root, port)
    try:
        _wait_for_health(base_url, process)
        imported = import_module_pack(pack, base_url)  # type: ignore[arg-type]
        _assert(len(imported) == 10, "candidate Pack import count mismatch")
        project = _json_request(
            base_url,
            "/api/v1/projects",
            method="POST",
            body=_create_body(),
            idempotency_key="blender-candidate-recovery-project",
        )
        graph = _reference_graph(project["project_id"])
        graph["graph_id"] = "mg_blender_candidate_recovery_v1"
        validated = _json_request(
            base_url,
            f"/api/v1/module-graphs/{graph['graph_id']}/validate",
            method="POST",
            body={
                "client_request_id": "blender-candidate-recovery-graph",
                "graph": graph,
                "persist": True,
            },
            idempotency_key="blender-candidate-recovery-graph",
        )
        _assert(validated["valid"] is True, "candidate recovery graph was rejected")
        _json_request(
            base_url,
            f"/api/v1/projects/{project['project_id']}/versions",
            method="POST",
            body={
                "client_request_id": "blender-candidate-recovery-bind",
                "parent_version_id": project["current_version_id"],
                "summary": "绑定十模块 Blender 候选以验证恢复链路。",
                "spec": project["current_spec"],
                "module_graph_id": graph["graph_id"],
            },
            idempotency_key="blender-candidate-recovery-bind",
        )
    finally:
        _stop_agent(process)


if __name__ == "__main__":
    raise SystemExit(main())
