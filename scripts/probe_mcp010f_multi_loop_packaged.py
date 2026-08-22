#!/usr/bin/env python3
"""Verify packaged multi-loop-profile-loft@1 through read-only MCP tools."""

from __future__ import annotations

import argparse
import json
import os
import tempfile
import time
from pathlib import Path
from typing import Any

from probe_mcp010a_dev_app import (
    DEFAULT_APP,
    McpClient,
    shutdown_isolated_runtime,
    verify_app,
    write_receipt,
)


ROOT = Path(__file__).resolve().parents[1]
FIXTURE = (
    ROOT
    / "packages"
    / "forgecad-contracts"
    / "fixtures"
    / "multi-loop-profile-loft-p1"
    / "positive"
    / "multi-loop-profile-loft.json"
)
OPERATOR_ID = "forgecad.geometry.multi-loop-profile-loft@1"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", type=Path, default=DEFAULT_APP)
    parser.add_argument("--evidence", type=Path)
    parser.add_argument("--timeout", type=float, default=20.0)
    return parser.parse_args()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def main() -> int:
    args = parse_args()
    manifest, paths = verify_app(args.app)
    cohort = str(manifest["build_cohort_sha256"])
    request: dict[str, Any] = json.loads(FIXTURE.read_text(encoding="utf-8"))

    environment = os.environ.copy()
    for key in (
        "FORGECAD_RUNTIME_COMMAND",
        "FORGECAD_RUNTIME_SOCKET",
        "FORGECAD_RUNTIME_TOKEN",
        "FORGECAD_RUNTIME_READY_FILE",
        "FORGECAD_RUNTIME_STATUS_FILE",
        "FORGECAD_MCP_ENABLE_MCP004_WRITES",
    ):
        environment.pop(key, None)

    with tempfile.TemporaryDirectory(prefix="forgecad-mcp010f-multi-loop-") as temporary:
        runtime_data = Path(temporary) / "runtime-data"
        environment["FORGECAD_RUNTIME_DATA_DIR"] = str(runtime_data)
        client = McpClient(paths["forgecad-mcp"], environment, args.timeout)
        try:
            initialized = client.request(
                "initialize",
                {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": {
                        "name": "forgecad-mcp010f-multi-loop-packaged-probe",
                        "version": "1",
                    },
                },
            )
            require(
                initialized.get("result", {}).get("protocolVersion") == "2025-06-18",
                "packaged multi-loop probe initialize failed",
            )
            client.notify("notifications/initialized")

            state = "Starting"
            deadline = time.monotonic() + args.timeout
            while time.monotonic() < deadline:
                status = client.tool("runtime_status")
                state = status.get("state") if isinstance(status, dict) else None
                if state == "Ready":
                    break
                require(
                    state not in {"Degraded", "Busy"},
                    f"packaged sibling Runtime failed: {state}",
                )
                time.sleep(0.1)
            require(state == "Ready", "packaged sibling Runtime did not become Ready")

            capabilities = client.tool("capabilities_get")
            require(isinstance(capabilities, dict), "capabilities_get was not an object")
            require(capabilities.get("build_cohort_match") is True, "cohort binding failed")
            require(
                capabilities.get("build_cohort_sha256") == cohort,
                "Runtime cohort differs from app manifest",
            )
            require(
                capabilities.get("mcp_build_cohort_sha256") == cohort,
                "MCP cohort differs from app manifest",
            )

            preflight = client.tool(
                "skill_get", {"skill_id": "ponytail-preflight", "version": "0.1.0"}
            )
            require(isinstance(preflight, dict), "preflight was not an object")
            skill = preflight.get("skill")
            knowledge = preflight.get("knowledge")
            require(
                isinstance(skill, dict)
                and skill.get("skill_id") == "ponytail-preflight"
                and skill.get("version") == "0.1.0"
                and isinstance(skill.get("canonical_sha256"), str)
                and len(skill["canonical_sha256"]) == 64
                and isinstance(knowledge, dict)
                and isinstance(knowledge.get("canonical_sha256"), str)
                and len(knowledge["canonical_sha256"]) == 64,
                "packaged ponytail preflight was not verified",
            )

            projects_before = client.tool("project_list")
            require(projects_before == [], "isolated Runtime was not empty before probe")

            catalog = client.tool("operator_catalog_get")
            require(isinstance(catalog, dict), "operator catalog was not an object")
            catalog_hash = catalog.get("canonical_sha256")
            require(
                isinstance(catalog_hash, str) and len(catalog_hash) == 64,
                "operator catalog hash was invalid",
            )
            require(
                catalog_hash == capabilities.get("operator_catalog_sha256"),
                "capabilities and catalog hashes differ",
            )
            operators = catalog.get("operators")
            require(isinstance(operators, list), "operator catalog entries were missing")
            operator = next(
                (
                    entry
                    for entry in operators
                    if isinstance(entry, dict) and entry.get("operator_id") == OPERATOR_ID
                ),
                None,
            )
            require(isinstance(operator, dict), "multi-loop operator is absent from package")
            require(operator.get("status") == "active", "multi-loop operator is not active")
            require(
                operator.get("parameter_schema") == "MultiLoopProfileLoftParameters@1",
                "multi-loop parameter schema drifted",
            )
            require(
                operator.get("part_output_required") is True,
                "multi-loop Part output requirement drifted",
            )

            result = client.tool("geometry_program_hash", request)
            require(isinstance(result, dict), "multi-loop expansion was not an object")
            require(
                result.get("schema_version") == "MultiLoopProfileLoftProgram@1",
                "multi-loop program schema drifted",
            )
            require(result.get("operator_id") == OPERATOR_ID, "operator binding drifted")
            require(result.get("lowered_operator_id") == OPERATOR_ID, "lowered operator drifted")
            require(result.get("validator_status") == "passed", "validator did not pass")
            require(result.get("quality_status") == "structural_only", "quality claim drifted")
            require(result.get("runtime_write_performed") is False, "read-only route reported a write")
            require(result.get("user_approval_required") is True, "approval boundary drifted")
            require(
                isinstance(result.get("canonical_sha256"), str)
                and len(result["canonical_sha256"]) == 64,
                "program canonical hash was invalid",
            )
            geometry = result.get("geometry_program")
            require(isinstance(geometry, dict), "lowered GeometryProgram@2 was missing")
            require(geometry.get("operator_catalog_sha256") == catalog_hash, "catalog binding drifted")
            nodes = geometry.get("nodes")
            require(
                isinstance(nodes, list)
                and len(nodes) == 1
                and isinstance(nodes[0], dict)
                and nodes[0].get("operator_id") == OPERATOR_ID,
                "lowered geometry node binding drifted",
            )
            source_map = result.get("source_map")
            require(isinstance(source_map, dict), "multi-loop source map was missing")
            require(
                source_map.get("station_ids") == ["station-front", "station-rear"],
                "station lineage order drifted",
            )
            require(
                source_map.get("component_ids") == ["island-a", "shell-core"],
                "component lineage order drifted",
            )
            require(
                source_map.get("hole_ids") == ["hole-a", "hole-b"],
                "hole lineage order drifted",
            )
            require(
                source_map.get("realized_surface_continuity") == "g0-only",
                "realized continuity claim drifted",
            )
            policy = result.get("continuity_policy")
            require(isinstance(policy, dict), "continuity policy was missing")
            require(
                policy.get("endpoint_caps") == "closed-solid-boolean"
                and policy.get("hole_policy") == "manifold-difference",
                "cap or hole policy drifted",
            )

            repeated = client.tool("geometry_program_hash", request)
            require(repeated == result, "repeated read-only expansion was not deterministic")

            projects_after = client.tool("project_list")
            require(projects_after == projects_before, "read-only probe created persistent projects")
        finally:
            try:
                shutdown_isolated_runtime(runtime_data)
            finally:
                client.close()

    receipt = {
        "schema_version": "ForgeCADMCP010FPackagedMultiLoopProfileLoftProbe@1",
        "task_id": "FGC-MCP010F",
        "status": "PASS",
        "build_cohort_sha256": cohort,
        "component_cohort_match": True,
        "runtime_state": "Ready",
        "ponytail_preflight": "PASS",
        "operator_catalog_sha256": catalog_hash,
        "operator_id": OPERATOR_ID,
        "operator_status": "active",
        "operator_parameter_schema": operator["parameter_schema"],
        "part_output_required": operator["part_output_required"],
        "multi_loop_read_only_expansion": "PASS",
        "repeat_expansion_determinism": "PASS",
        "program_schema_version": result["schema_version"],
        "program_canonical_sha256": result["canonical_sha256"],
        "cross_section_plan_sha256": result["cross_section_plan_sha256"],
        "source_map": {
            "station_ids": source_map["station_ids"],
            "component_ids": source_map["component_ids"],
            "hole_ids": source_map["hole_ids"],
            "realized_surface_continuity": source_map["realized_surface_continuity"],
        },
        "endpoint_caps": policy["endpoint_caps"],
        "hole_policy": policy["hole_policy"],
        "quality_status": result["quality_status"],
        "runtime_write_performed": result["runtime_write_performed"],
        "projects_before": 0,
        "projects_after": 0,
        "candidate_created": False,
        "job_created": False,
        "version_created": False,
        "persistent_user_data_touched": False,
        "allowed_mcp_calls": [
            "runtime_status",
            "capabilities_get",
            "skill_get",
            "project_list",
            "operator_catalog_get",
            "geometry_program_hash",
        ],
        "authenticated_runtime_shutdown": "PASS",
        "source_revision": manifest.get("source_revision"),
        "source_worktree_dirty": manifest.get("source_worktree_dirty"),
        "glb_readback_gate": "NOT_RUN",
        "genus_gate": "NOT_RUN",
        "manifold_readback_gate": "NOT_RUN",
        "lineage_readback_gate": "NOT_RUN",
        "visual_reference_gate": "NOT_RUN",
        "pbr_gate": "NOT_RUN",
        "human_review_gate": "NOT_RUN",
        "hq_360_gate": "BLOCKED_REFERENCE_COVERAGE",
        "codex_desktop_restart_gate": "NOT_RUN",
    }
    write_receipt(args.evidence, receipt)
    print(json.dumps(receipt, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
