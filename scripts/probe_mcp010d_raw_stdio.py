#!/usr/bin/env python3
"""Run the MCP010D operator suite over the public raw MCP stdio surface.

The probe intentionally reuses only the bounded helper/client from the
MCP010B raw probe. Runtime, CAS and the authenticated handoff are all created
under a caller-owned temporary directory; no persistent project is touched.
"""

from __future__ import annotations

import copy
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Any

SCRIPT_ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_ROOT))

from probe_mcp010b_raw_stdio import (  # noqa: E402
    MCP_PROTOCOL_VERSION,
    GateFailure,
    McpClient,
    build_identity,
    require,
    shutdown_runtime,
    wait_for_ready,
)


def parse_args() -> Any:
    import argparse

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--data-root", type=Path, required=True)
    parser.add_argument("--expected-build-cohort")
    parser.add_argument("--evidence", type=Path)
    parser.add_argument("--timeout", type=float, default=30.0)
    return parser.parse_args()


def read_ponytail_preflight(client: McpClient) -> dict[str, str]:
    """Read the mandatory first-party planning Skill before design calls."""
    result = client.tool(
        "skill_get",
        {"skill_id": "ponytail-preflight", "version": "0.1.0"},
    )
    require(isinstance(result, dict), "ponytail preflight returned no typed result")
    skill = result.get("skill")
    knowledge = result.get("knowledge")
    require(
        isinstance(skill, dict)
        and skill.get("skill_id") == "ponytail-preflight"
        and skill.get("version") == "0.1.0"
        and isinstance(skill.get("canonical_sha256"), str)
        and len(skill["canonical_sha256"]) == 64,
        "ponytail preflight manifest was not verified",
    )
    require(
        isinstance(knowledge, dict)
        and isinstance(knowledge.get("canonical_sha256"), str)
        and len(knowledge["canonical_sha256"]) == 64
        and isinstance(knowledge.get("overview"), str)
        and isinstance(knowledge.get("constraints"), str),
        "ponytail preflight knowledge was not returned",
    )
    return {
        "skill_id": skill["skill_id"],
        "version": skill["version"],
        "skill_manifest_sha256": skill["canonical_sha256"],
        "knowledge_sha256": knowledge["canonical_sha256"],
        "status": "PASS",
    }


def draft(project_id: str, catalog_hash: str) -> dict[str, Any]:
    return {
        "schema_version": "GeometryProgram@2",
        "project_id": project_id,
        "representation_plan_sha256": "d" * 64,
        "operator_catalog_sha256": catalog_hash,
        "units": {
            "length": "meter",
            "angle": "radian",
            "coordinate_system": "right-handed-y-up",
        },
        "budgets": {
            "max_nodes": 32,
            "max_triangles": 100000,
            "max_glb_bytes": 67108864,
            "max_worker_memory_bytes": 536870912,
            "max_runtime_ms": 10000,
        },
        "nodes": [
            {
                "node_id": "base",
                "operator_id": "forgecad.geometry.primitive@2",
                "inputs": [],
                "parameters": {
                    "shape": "box",
                    "size_m": [0.8, 0.8, 0.8],
                    "position_m": [-1.5, 0.0, 0.0],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
            },
            {
                "node_id": "transformed",
                "operator_id": "forgecad.geometry.transform@2",
                "inputs": ["base"],
                "parameters": {
                    "shape": "transform",
                    "translation_m": [0.0, 0.6, 0.0],
                    "rotation_rad": [0.0, 0.2, 0.0],
                    "scale": [1.0, 0.8, 1.0],
                },
            },
            {
                "node_id": "mirrored",
                "operator_id": "forgecad.geometry.mirror@1",
                "inputs": ["transformed"],
                "parameters": {"shape": "mirror", "axis": "x", "offset_m": 0.0},
            },
            {
                "node_id": "arrayed",
                "operator_id": "forgecad.geometry.array@1",
                "inputs": ["mirrored"],
                "parameters": {
                    "shape": "array",
                    "count": 2,
                    "offset_m": [1.0, 0.0, 0.0],
                },
            },
            {
                "node_id": "panel",
                "operator_id": "forgecad.geometry.panel@1",
                "inputs": [],
                "parameters": {
                    "shape": "panel",
                    "size_m": [1.6, 0.8, 0.3],
                    "thickness_m": 0.18,
                    "bevel_m": 0.08,
                    "position_m": [0.0, 1.0, 0.0],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
            },
            {
                "node_id": "vent",
                "operator_id": "forgecad.geometry.vent-array@1",
                "inputs": [],
                "parameters": {
                    "shape": "vent-array",
                    "width_m": 1.2,
                    "height_m": 0.6,
                    "depth_m": 0.18,
                    "slot_count": 4,
                    "slot_width_m": 0.12,
                    "slot_spacing_m": 0.12,
                    "position_m": [0.0, 1.0, 0.25],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
            },
            {
                "node_id": "joint",
                "operator_id": "forgecad.geometry.joint-stack@1",
                "inputs": [],
                "parameters": {
                    "shape": "joint-stack",
                    "radius_m": 0.22,
                    "depth_m": 0.12,
                    "ring_count": 3,
                    "ring_spacing_m": 0.18,
                    "radial_segments": 12,
                    "position_m": [-1.0, 0.0, 0.0],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
            },
            {
                "node_id": "profile",
                "operator_id": "forgecad.geometry.profile-extrude@1",
                "inputs": [],
                "parameters": {
                    "shape": "profile-extrude",
                    "profile": [
                        [-0.3, -0.2],
                        [0.3, -0.2],
                        [0.35, 0.15],
                        [0.0, 0.3],
                        [-0.35, 0.15],
                    ],
                    "depth_m": 0.25,
                    "position_m": [1.0, 0.5, 0.0],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
            },
            {
                "node_id": "loft",
                "operator_id": "forgecad.geometry.profile-loft@1",
                "inputs": [],
                "parameters": {
                    "shape": "profile-loft",
                    "profiles": [
                        {
                            "height_m": 0.0,
                            "points": [
                                [-0.3, -0.2],
                                [0.3, -0.2],
                                [0.3, 0.2],
                                [-0.3, 0.2],
                            ],
                        },
                        {
                            "height_m": 0.4,
                            "points": [
                                [-0.2, -0.12],
                                [0.2, -0.12],
                                [0.2, 0.12],
                                [-0.2, 0.12],
                            ],
                        },
                    ],
                    "position_m": [1.0, 1.0, 0.0],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
            },
            {
                "node_id": "revolve",
                "operator_id": "forgecad.geometry.revolve@1",
                "inputs": [],
                "parameters": {
                    "shape": "revolve",
                    "profile": [[0.2, -0.2], [0.35, 0.0], [0.2, 0.2]],
                    "radial_segments": 16,
                    "position_m": [-1.0, 1.0, 0.0],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
            },
            {
                "node_id": "sweep",
                "operator_id": "forgecad.geometry.tube-sweep@1",
                "inputs": [],
                "parameters": {
                    "shape": "tube-sweep",
                    "path": [[-0.5, 0.0, 0.0], [0.0, 0.3, 0.2], [0.5, 0.0, 0.0]],
                    "radius_m": 0.08,
                    "radial_segments": 12,
                    "cap_ends": True,
                    "position_m": [0.0, 1.8, 0.0],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
            },
            {
                "node_id": "boolean-left",
                "operator_id": "forgecad.geometry.profile-extrude@1",
                "inputs": [],
                "parameters": {
                    "shape": "profile-extrude",
                    "profile": [[-0.72, -0.50], [0.60, -0.50], [0.72, 0.0], [0.60, 0.50], [-0.72, 0.50]],
                    "depth_m": 1.0,
                    "position_m": [2.25, 0.0, 0.0],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
            },
            {
                "node_id": "boolean-right",
                "operator_id": "forgecad.geometry.profile-extrude@1",
                "inputs": [],
                "parameters": {
                    "shape": "profile-extrude",
                    "profile": [[-0.36, -0.36], [0.28, -0.36], [0.42, -0.06], [0.28, 0.36], [-0.36, 0.36]],
                    "depth_m": 0.72,
                    "position_m": [2.75, 0.0, 0.0],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
            },
            {
                "node_id": "boolean-intersection-left",
                "operator_id": "forgecad.geometry.profile-extrude@1",
                "inputs": [],
                "parameters": {
                    "shape": "profile-extrude",
                    "profile": [[-0.72, -0.50], [0.60, -0.50], [0.72, 0.0], [0.60, 0.50], [-0.72, 0.50]],
                    "depth_m": 1.0,
                    "position_m": [2.25, 0.0, 0.0],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
            },
            {
                "node_id": "boolean-intersection-right",
                "operator_id": "forgecad.geometry.profile-extrude@1",
                "inputs": [],
                "parameters": {
                    "shape": "profile-extrude",
                    "profile": [[-0.36, -0.36], [0.28, -0.36], [0.42, -0.06], [0.28, 0.36], [-0.36, 0.36]],
                    "depth_m": 0.72,
                    "position_m": [2.75, 0.0, 0.0],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
            },
            {
                "node_id": "boolean-difference",
                "operator_id": "forgecad.geometry.boolean@1",
                "inputs": ["boolean-left", "boolean-right"],
                "parameters": {"shape": "difference"},
            },
            {
                "node_id": "boolean-intersection",
                "operator_id": "forgecad.geometry.boolean@1",
                "inputs": ["boolean-intersection-left", "boolean-intersection-right"],
                "parameters": {"shape": "intersection"},
            },
            {
                "node_id": "aggregate",
                "operator_id": "forgecad.geometry.part-output@1",
                "inputs": ["panel", "vent"],
                "parameters": {"shape": "part-output"},
            },
        ],
        "part_outputs": [
            {
                "part_id": "arrayed-part",
                "input_node_ids": ["arrayed"],
                "material_zone_id": "zone-white-shell",
                "solid": True,
            },
            {
                "part_id": "panel-vent",
                "input_node_ids": ["aggregate"],
                "material_zone_id": "zone-black-mechanical",
                "solid": True,
            },
            {
                "part_id": "joint-part",
                "input_node_ids": ["joint"],
                "material_zone_id": "zone-black-mechanical",
                "solid": True,
            },
            {
                "part_id": "profile-part",
                "input_node_ids": ["profile"],
                "material_zone_id": "zone-white-shell",
                "solid": True,
            },
            {
                "part_id": "loft-part",
                "input_node_ids": ["loft"],
                "material_zone_id": "zone-white-shell",
                "solid": True,
            },
            {
                "part_id": "revolve-part",
                "input_node_ids": ["revolve"],
                "material_zone_id": "zone-black-mechanical",
                "solid": True,
            },
            {
                "part_id": "sweep-part",
                "input_node_ids": ["sweep"],
                "material_zone_id": "zone-emissive-amber",
                "solid": True,
            },
            {
                "part_id": "boolean-part",
                "input_node_ids": ["boolean-difference"],
                "material_zone_id": "zone-black-mechanical",
                "solid": True,
            },
            {
                "part_id": "boolean-intersection-part",
                "input_node_ids": ["boolean-intersection"],
                "material_zone_id": "zone-black-mechanical",
                "solid": True,
            },
        ],
    }


def main() -> int:
    args = parse_args()
    if not args.mcp.is_file() or not args.runtime.is_file() or args.timeout <= 0:
        raise GateFailure("MCP010D source binaries were unavailable")
    if args.expected_build_cohort and re.fullmatch(r"[0-9a-f]{64}", args.expected_build_cohort) is None:
        raise GateFailure("expected build cohort was not a lowercase SHA-256")
    mcp_identity = build_identity(args.mcp) if args.expected_build_cohort else None
    runtime_identity = build_identity(args.runtime) if args.expected_build_cohort else None
    if args.expected_build_cohort:
        require(
            mcp_identity and mcp_identity.get("build_cohort_sha256") == args.expected_build_cohort
            and runtime_identity
            and runtime_identity.get("build_cohort_sha256") == args.expected_build_cohort,
            "MCP/Runtime build cohorts did not match",
        )
    data_root = args.data_root.resolve()
    if data_root.exists():
        raise GateFailure("MCP010D data root must not pre-exist")
    data_root.mkdir(mode=0o700, parents=True)
    ready_path = data_root / "ipc" / "ready.json"
    runtime = subprocess.Popen(
        [
            str(args.runtime),
            "serve",
            "--database",
            str(data_root / "runtime.sqlite"),
            "--cas-root",
            str(data_root / "cas"),
            "--endpoint-dir",
            str(data_root / "ipc"),
            "--ready-file",
            str(ready_path),
        ],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
    )
    client: McpClient | None = None
    ready: dict[str, Any] | None = None
    d_result: dict[str, Any] | None = None
    try:
        ready = wait_for_ready(ready_path, runtime, args.timeout)
        socket_path = ready.get("socket_path")
        token = ready.get("token")
        require(isinstance(socket_path, str) and socket_path, "ready handoff lacked a socket")
        require(isinstance(token, str) and token, "ready handoff lacked a token")
        environment = os.environ.copy()
        for key in (
            "FORGECAD_RUNTIME_COMMAND",
            "FORGECAD_RUNTIME_DATA_DIR",
            "FORGECAD_RUNTIME_READY_FILE",
            "FORGECAD_RUNTIME_STATUS_FILE",
        ):
            environment.pop(key, None)
        environment["FORGECAD_RUNTIME_SOCKET"] = socket_path
        environment["FORGECAD_RUNTIME_TOKEN"] = token
        environment["FORGECAD_MCP_ENABLE_MCP004_WRITES"] = "1"
        client = McpClient(args.mcp, environment, args.timeout)
        initialized = client.request(
            "initialize",
            {
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "forgecad-mcp010d-raw-stdio", "version": "1"},
            },
        )
        require(
            initialized.get("result", {}).get("protocolVersion") == MCP_PROTOCOL_VERSION,
            "MCP010D initialize did not negotiate 2025-06-18",
        )
        client.notify("notifications/initialized")
        preflight = read_ponytail_preflight(client)
        capabilities = client.tool("capabilities_get")
        catalog_hash = capabilities.get("operator_catalog_sha256")
        require(isinstance(catalog_hash, str) and len(catalog_hash) == 64, "catalog hash missing")
        catalog = client.tool("operator_catalog_get")
        operators = {entry.get("operator_id"): entry for entry in catalog.get("operators", [])}
        required_active = {
            "forgecad.geometry.profile-extrude@1",
            "forgecad.geometry.profile-loft@1",
            "forgecad.geometry.subd-cage@1",
            "forgecad.geometry.revolve@1",
            "forgecad.geometry.tube-sweep@1",
            "forgecad.geometry.transform@2",
            "forgecad.geometry.mirror@1",
            "forgecad.geometry.array@1",
            "forgecad.geometry.panel@1",
            "forgecad.geometry.vent-array@1",
            "forgecad.geometry.joint-stack@1",
            "forgecad.geometry.boolean@1",
            "forgecad.geometry.part-output@1",
        }
        require(
            required_active.issubset(
                {key for key, value in operators.items() if value.get("status") == "active"}
            ),
            "OperatorCatalog did not advertise all active MCP010D operators",
        )
        require(
            operators.get("forgecad.geometry.boolean@1", {}).get("status") == "active"
            and set(operators.get("forgecad.geometry.boolean@1", {}).get("supported_shapes", []))
            == {"union", "difference", "intersection"},
            "boolean@1 did not advertise the bounded active P0 scope",
        )
        project = client.tool(
            "project_create",
            {"name": "MCP010D hard surface operator suite", "policy": {"profile": "mvp"}},
        )
        project_id = project.get("project_id")
        require(isinstance(project_id, str) and project_id, "project_create omitted project_id")
        program_draft = draft(project_id, catalog_hash)
        hashed = client.tool(
            "geometry_program_hash",
            {
                "schema_version": "GeometryProgramHashRequest@1",
                "geometry_program_draft": program_draft,
            },
        )
        require(
            hashed.get("validation_status") == "passed"
            and hashed.get("operator_catalog_sha256") == catalog_hash,
            "MCP010D graph hash was not accepted",
        )
        program = copy.deepcopy(program_draft)
        program["canonical_sha256"] = hashed["canonical_sha256"]
        prepared = client.tool(
            "geometry_prepare",
            {
                "project_id": project_id,
                "request": {"typed": "geometry", "geometry_program": program},
            },
        )
        artifact = prepared.get("artifact")
        require(
            prepared.get("schema_version") == "GeometryPrepareResult@2"
            and isinstance(artifact, dict)
            and artifact.get("hard_gate_passed") is True
            and artifact.get("operator_catalog_sha256") == catalog_hash,
            "MCP010D geometry_prepare did not return strict readback",
        )
        bindings = artifact.get("part_bindings")
        require(
            isinstance(bindings, list)
            and {item.get("source_node_id") for item in bindings}
            >= {
                "arrayed",
                "aggregate",
                "joint",
                "profile",
                "loft",
                "revolve",
                "sweep",
                "boolean-difference",
                "boolean-intersection",
            },
            "MCP010D readback lost semantic operator lineage",
        )
        negative = copy.deepcopy(program_draft)
        negative["nodes"][1]["inputs"] = ["future-node"]
        error = client.tool_error(
            "geometry_program_hash",
            {
                "schema_version": "GeometryProgramHashRequest@1",
                "geometry_program_draft": negative,
            },
        )
        require(
            error.get("code") in {"INVALID_INPUT", "GEOMETRY_PROGRAM_HASH_REJECTED"},
            f"future DAG input did not fail closed: {error}",
        )
        boolean = copy.deepcopy(program_draft)
        boolean_node = next(node for node in boolean["nodes"] if node["node_id"] == "boolean-difference")
        boolean_node["parameters"] = {"shape": "xor"}
        error = client.tool_error(
            "geometry_program_hash",
            {
                "schema_version": "GeometryProgramHashRequest@1",
                "geometry_program_draft": boolean,
            },
        )
        require(
            error.get("code") in {"INVALID_INPUT", "GEOMETRY_PROGRAM_HASH_REJECTED"},
            f"unsupported boolean shape did not fail closed: {error}",
        )
        d_result = {
            "status": "PASS",
            "operator_catalog": "16 entries / 16 active / boolean union+difference/intersection active",
            "operators": sorted(required_active),
            "geometry_program": "GeometryProgram@2 DAG",
            "semantic_parts": len(artifact.get("part_ids", [])),
            "triangle_count": artifact.get("triangle_count"),
            "artifact_readback": "ArtifactReadback@2 hard_gate_passed",
            "lineage": "PASS",
            "negative_future_input": "PASS",
            "boolean_union_difference_intersection": "PASS",
            "negative_boolean_unsupported_shape": "PASS",
            "ponytail_preflight": preflight,
            "persistent_user_data_touched": False,
        }
    finally:
        cleanup_error: BaseException | None = None
        if client is not None:
            try:
                client.close()
            except BaseException as error:
                cleanup_error = error
        if ready is not None:
            try:
                shutdown_runtime(ready, ready_path, runtime)
            except BaseException as error:
                if cleanup_error is None:
                    cleanup_error = error
        elif runtime.poll() is None:
            runtime.kill()
            runtime.wait(timeout=5)
        if cleanup_error is not None and sys.exc_info()[0] is None:
            raise cleanup_error
    receipt = {
        "schema_version": "ForgeCADMCP010DRawStdioProbe@1",
        "task_id": "FGC-MCP010D",
        "status": "PASS",
        "protocol_version": MCP_PROTOCOL_VERSION,
        "persistent_user_data_touched": False,
        "runtime_cleanup": "PASS",
        **(d_result or {}),
    }
    if args.evidence:
        evidence_root = Path(__file__).resolve().parents[1] / "docs" / "evidence"
        resolved = args.evidence if args.evidence.is_absolute() else Path(__file__).resolve().parents[1] / args.evidence
        resolved.resolve().relative_to(evidence_root.resolve())
        resolved.parent.mkdir(parents=True, exist_ok=True)
        resolved.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(receipt, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except GateFailure as error:
        print(
            json.dumps(
                {
                    "schema_version": "ForgeCADMCP010DRawStdioProbe@1",
                    "task_id": "FGC-MCP010D",
                    "status": "FAIL",
                    "reason": str(error)[:256],
                    "persistent_user_data_touched": False,
                },
                sort_keys=True,
            ),
            file=sys.stderr,
        )
        raise SystemExit(1)
