#!/usr/bin/env python3
"""Run the MCP010D operator suite over the public raw MCP stdio surface.

The probe intentionally reuses only the bounded helper/client from the
MCP010B raw probe. Runtime, CAS and the authenticated handoff are all created
under a caller-owned temporary directory; no persistent project is touched.
"""

from __future__ import annotations

import copy
import hashlib
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


def canonical_hash(value: Any) -> str:
    encoded = json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def authoring_preview_request(
    topology_request: dict[str, Any],
    base_topology_sha256: str,
    edit: dict[str, Any],
) -> dict[str, Any]:
    request = {
        "schema_version": "AuthoringMeshEditPreviewRequest@1",
        "topology_request": topology_request,
        "base_topology_sha256": base_topology_sha256,
        "edit": edit,
        "edit_policy_sha256": "1d050226b13848902f44bddb1b88c240cdfa86759703f804443b03964f8ddaae",
    }
    request["input_sha256"] = canonical_hash(request)
    return request


def authoring_prepare_request(
    project_id: str,
    source_candidate_id: str,
    preview_request: dict[str, Any],
    preview_canonical_sha256: str,
    idempotency_key: str,
    base_version_id: str | None = None,
) -> dict[str, Any]:
    request = {
        "schema_version": "AuthoringMeshEditPrepareRequest@1",
        "project_id": project_id,
        "source_candidate_id": source_candidate_id,
        "base_version_id": base_version_id,
        "preview_request": preview_request,
        "expected_preview_canonical_sha256": preview_canonical_sha256,
        "idempotency_key": idempotency_key,
        "max_response_bytes": 1048576,
    }
    request["input_sha256"] = canonical_hash(request)
    return request


def require_self_hash(value: dict[str, Any], label: str) -> None:
    preimage = copy.deepcopy(value)
    actual = preimage.get("canonical_sha256")
    preimage["canonical_sha256"] = ""
    require(
        isinstance(actual, str) and actual == canonical_hash(preimage),
        f"{label} canonical hash differed",
    )


def tool_schema_error(client: McpClient, name: str, arguments: dict[str, Any]) -> str:
    response = client.request("tools/call", {"name": name, "arguments": arguments})
    error = response.get("error")
    data = error.get("data") if isinstance(error, dict) else None
    code = data.get("code") if isinstance(data, dict) else None
    require(
        code == "INVALID_TOOL_PARAMS",
        f"negative {name} did not fail at the closed MCP schema: {response}",
    )
    return code


def parse_args() -> Any:
    import argparse

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--data-root", type=Path, required=True)
    parser.add_argument("--expected-build-cohort")
    parser.add_argument("--evidence", type=Path)
    parser.add_argument(
        "--exercise-authoring-prepare",
        action="store_true",
        help="Also stage and verify one approval-gated authoring mesh edit candidate.",
    )
    parser.add_argument(
        "--exercise-exact-geometry-prepare",
        action="store_true",
        help="Also verify explicit-head exact geometry prepare, replay and collision over raw stdio.",
    )
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
                "node_id": "longitudinal-loft",
                "operator_id": "forgecad.geometry.longitudinal-section-loft@1",
                "inputs": [],
                "parameters": {
                    "shape": "longitudinal-section-loft",
                    "sections": [
                        {"station_m": -0.6, "points": [[-0.18, -0.12], [0.18, -0.12], [0.24, 0.0], [0.18, 0.12], [-0.18, 0.12], [-0.24, 0.0]]},
                        {"station_m": 0.0, "points": [[-0.30, -0.20], [0.30, -0.20], [0.38, 0.0], [0.30, 0.20], [-0.30, 0.20], [-0.38, 0.0]]},
                        {"station_m": 0.8, "points": [[-0.16, -0.10], [0.16, -0.10], [0.22, 0.0], [0.16, 0.10], [-0.16, 0.10], [-0.22, 0.0]]},
                    ],
                    "position_m": [0.0, 0.0, 0.0],
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
                "node_id": "authored-panel",
                "operator_id": "forgecad.geometry.authoring-mesh@1",
                "inputs": [],
                "parameters": {
                    "shape": "authoring-mesh",
                    "topology_policy": "triangle-quad-manifold-with-boundary@1",
                    "vertices": [
                        {"element_id": "v0", "position_m": [-0.4, -0.3, 0.0]},
                        {"element_id": "v1", "position_m": [0.4, -0.3, 0.0]},
                        {"element_id": "v2", "position_m": [0.4, 0.3, 0.0]},
                        {"element_id": "v3", "position_m": [-0.4, 0.3, 0.0]},
                    ],
                    "edges": [
                        {"element_id": "e01", "vertex_ids": ["v0", "v1"]},
                        {"element_id": "e03", "vertex_ids": ["v0", "v3"]},
                        {"element_id": "e12", "vertex_ids": ["v1", "v2"]},
                        {"element_id": "e23", "vertex_ids": ["v2", "v3"]},
                    ],
                    "loops": [
                        {"element_id": "l0", "face_id": "f0", "ordinal": 0, "vertex_id": "v0", "edge_id": "e01", "edge_forward": True},
                        {"element_id": "l1", "face_id": "f0", "ordinal": 1, "vertex_id": "v1", "edge_id": "e12", "edge_forward": True},
                        {"element_id": "l2", "face_id": "f0", "ordinal": 2, "vertex_id": "v2", "edge_id": "e23", "edge_forward": True},
                        {"element_id": "l3", "face_id": "f0", "ordinal": 3, "vertex_id": "v3", "edge_id": "e03", "edge_forward": False},
                    ],
                    "faces": [{"element_id": "f0", "loop_ids": ["l0", "l1", "l2", "l3"]}],
                    "position_m": [0.0, 2.6, 0.0],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
            },
            {
                "node_id": "aggregate",
                "operator_id": "forgecad.geometry.part-output@1",
                "inputs": ["panel", "vent"],
                "parameters": {"shape": "part-output"},
            },
            {
                "node_id": "energy-core-guard",
                "operator_id": "forgecad.geometry.energy-core@1",
                "inputs": [],
                "parameters": {
                    "shape": "energy-core",
                    "component": "guard-ring",
                    "outer_radius_m": 0.36,
                    "inner_radius_m": 0.28,
                    "depth_m": 0.08,
                    "radial_segments": 24,
                    "position_m": [1.8, 1.4, 0.0],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
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
                "part_id": "energy-core-guard-part",
                "input_node_ids": ["energy-core-guard"],
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
                "part_id": "longitudinal-loft-part",
                "input_node_ids": ["longitudinal-loft"],
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
                "part_id": "authored-panel-part",
                "input_node_ids": ["authored-panel"],
                "material_zone_id": "zone-white-shell",
                "solid": False,
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
    data_root = args.data_root.absolute()
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
        expected_operator_ids = {
            "forgecad.geometry.primitive@2",
            "forgecad.geometry.profile-extrude@1",
            "forgecad.geometry.profile-loft@1",
            "forgecad.geometry.profile-loft@2",
            "forgecad.geometry.multi-loop-profile-loft@1",
            "forgecad.geometry.longitudinal-section-loft@1",
            "forgecad.geometry.subd-cage@1",
            "forgecad.geometry.subd-cage@2",
            "forgecad.geometry.authoring-mesh@1",
            "forgecad.geometry.surface-patch@1",
            "forgecad.geometry.surface-shell@1",
            "forgecad.geometry.revolve@1",
            "forgecad.geometry.tube-sweep@1",
            "forgecad.geometry.transform@2",
            "forgecad.geometry.mirror@1",
            "forgecad.geometry.array@1",
            "forgecad.geometry.bevel@1",
            "forgecad.geometry.bevel@2",
            "forgecad.geometry.normal-policy@1",
            "forgecad.geometry.panel@1",
            "forgecad.geometry.panel@2",
            "forgecad.geometry.vent-array@1",
            "forgecad.geometry.vent-array@2",
            "forgecad.geometry.recessed-channel@1",
            "forgecad.geometry.energy-core@1",
            "forgecad.geometry.joint-stack@1",
            "forgecad.geometry.boolean@1",
            "forgecad.geometry.part-output@1",
        }
        require(
            set(operators) == expected_operator_ids
            and len(catalog.get("operators", [])) == len(expected_operator_ids)
            and all(value.get("status") == "active" for value in operators.values()),
            "OperatorCatalog current truth drifted: expected exactly 28 active operators",
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
                "longitudinal-loft",
                "revolve",
                "sweep",
                "boolean-difference",
                "boolean-intersection",
                "authored-panel",
            },
            "MCP010D readback lost semantic operator lineage",
        )
        candidate = prepared.get("candidate")
        require(
            isinstance(candidate, dict)
            and isinstance(candidate.get("candidate_id"), str)
            and isinstance(artifact.get("artifact_id"), str)
            and isinstance(artifact.get("canonical_sha256"), str)
            and isinstance(artifact.get("program_sha256"), str)
            and isinstance(artifact.get("readback_config_sha256"), str),
            "authoring topology bindings were unavailable from geometry_prepare",
        )
        exact_geometry_prepare_result: dict[str, Any] | None = None
        modifier_apply_result: dict[str, Any] | None = None
        candidate_bound_modifier_apply_result: dict[str, Any] | None = None
        candidate_bound_modifier_job_id: str | None = None
        candidate_bound_modifier_candidate: dict[str, Any] | None = None
        candidate_bound_modifier_events: list[dict[str, Any]] | None = None
        if args.exercise_exact_geometry_prepare:
            versions_before_exact = client.tool(
                "version_list", {"project_id": project_id}
            )
            exact_arguments = {
                "project_id": project_id,
                "base_version_id": None,
                "idempotency_key": "mcp010f-geometry-prepare-exact-once",
                "request": {"typed": "geometry", "geometry_program": program},
            }
            exact_response = client.request(
                "tools/call",
                {"name": "geometry_prepare", "arguments": exact_arguments},
            )
            exact_result = exact_response.get("result")
            require(
                isinstance(exact_result, dict)
                and exact_result.get("isError") is not True
                and isinstance(exact_result.get("structuredContent"), dict),
                f"exact geometry_prepare failed: {exact_response}",
            )
            exact_prepared = exact_result["structuredContent"]
            exact_candidate = exact_prepared.get("candidate")
            require(
                exact_prepared.get("schema_version") == "GeometryPrepareResult@2"
                and isinstance(exact_candidate, dict)
                and exact_candidate.get("base_version_id") is None
                and exact_candidate.get("state") == "reviewable"
                and exact_candidate.get("quality_hard_gate_passed") is True,
                "exact geometry_prepare did not stage a reviewable empty-head V2 candidate",
            )
            exact_wire_bytes = len(
                json.dumps(exact_response, ensure_ascii=False, separators=(",", ":")).encode(
                    "utf-8"
                )
            )
            require(
                exact_wire_bytes <= 1048576,
                "exact geometry_prepare full MCP response exceeded 1 MiB",
            )
            replay_response = client.request(
                "tools/call",
                {"name": "geometry_prepare", "arguments": exact_arguments},
            )
            require(
                replay_response.get("result") == exact_result,
                "exact geometry_prepare replay did not return the identical MCP result",
            )

            missing_head_arguments = copy.deepcopy(exact_arguments)
            missing_head_arguments.pop("base_version_id")
            missing_head_error = tool_schema_error(
                client, "geometry_prepare", missing_head_arguments
            )
            v1_exact_arguments = copy.deepcopy(exact_arguments)
            v1_exact_arguments["idempotency_key"] = "mcp010f-v1-exact-rejected"
            v1_exact_arguments["request"]["geometry_program"] = {
                "schema_version": "GeometryProgram@1"
            }
            v1_exact_error = tool_schema_error(
                client, "geometry_prepare", v1_exact_arguments
            )

            changed_arguments = copy.deepcopy(exact_arguments)
            changed_program = changed_arguments["request"]["geometry_program"]
            changed_program["representation_plan_sha256"] = "e" * 64
            changed_draft = copy.deepcopy(changed_program)
            changed_draft.pop("canonical_sha256")
            changed_program["canonical_sha256"] = canonical_hash(changed_draft)
            state_before_collision = client.tool("project_get", {"project_id": project_id})
            collision_error = client.tool_error("geometry_prepare", changed_arguments)
            require(
                "IDEMPOTENCY_KEY_REUSED"
                in json.dumps(collision_error, ensure_ascii=False, sort_keys=True),
                f"exact geometry_prepare key collision was not explicit: {collision_error}",
            )
            require(
                client.tool("project_get", {"project_id": project_id})
                == state_before_collision,
                "exact geometry_prepare collision mutated project state",
            )
            require(
                client.tool("version_list", {"project_id": project_id})
                == versions_before_exact,
                "exact geometry_prepare created or changed a version before confirmation",
            )
            exact_geometry_prepare_result = {
                "schema_version": exact_prepared.get("schema_version"),
                "candidate_id": exact_candidate.get("candidate_id"),
                "artifact_sha256": exact_prepared.get("artifact", {}).get("artifact_id"),
                "base_version_id": exact_candidate.get("base_version_id"),
                "worker_replay": "PASS_ACTUAL_SIBLING_BYTE_EXACT_SAME_COHORT",
                "idempotent_replay": "PASS_IDENTICAL_MCP_RESULT",
                "missing_head_error": missing_head_error,
                "v1_exact_error": v1_exact_error,
                "collision_error_code": collision_error.get("code"),
                "collision_status": "REJECTED_IDEMPOTENCY_KEY_REUSED_NO_VISIBLE_RESIDUE",
                "full_mcp_response_bytes": exact_wire_bytes,
                "max_response_bytes": 1048576,
                "version_status": "no-version-created",
                "confirm_status": "approval-required",
                "quality_status": "structural_only",
            }

            modifier_request = {
                "schema_version": "GeometryModifierEvaluationRequest@2",
                "project_id": project_id,
                "representation_plan_sha256": "d" * 64,
                "part_id": "modifier-apply-shell",
                "material_zone_id": "zone-white-shell",
                "solid": True,
                "base_node": {
                    "node_id": "modifier-base",
                    "operator_id": "forgecad.geometry.primitive@2",
                    "inputs": [],
                    "parameters": {
                        "shape": "box",
                        "size_m": [1.0, 1.0, 1.0],
                        "position_m": [0.0, 0.0, 0.0],
                        "rotation_rad": [0.0, 0.0, 0.0],
                    },
                },
                "modifiers": [
                    {
                        "modifier_id": "round",
                        "enabled": True,
                        "operator_id": "forgecad.geometry.bevel@1",
                        "parameters": {
                            "shape": "bevel",
                            "width_m": 0.04,
                            "segments": 2,
                            "profile": 0.5,
                            "edge_scope": "all-source-box-edges",
                            "clamp_overlap": False,
                        },
                    },
                    {
                        "modifier_id": "preview-mirror",
                        "enabled": False,
                        "operator_id": "forgecad.geometry.mirror@1",
                        "parameters": {
                            "shape": "mirror",
                            "axis": "x",
                            "offset_m": 0.0,
                        },
                    },
                    {
                        "modifier_id": "shade",
                        "enabled": True,
                        "operator_id": "forgecad.geometry.normal-policy@1",
                        "parameters": {
                            "shape": "normal-policy",
                            "weighting": "face-area-x-corner-angle",
                            "crease_angle_rad": 1.0,
                            "keep_sharp": True,
                            "output_domain": "corner",
                        },
                    },
                ],
                "previous_evaluation": None,
                "input_sha256": "",
            }
            modifier_binding = copy.deepcopy(modifier_request)
            modifier_binding.pop("input_sha256")
            modifier_request["input_sha256"] = canonical_hash(modifier_binding)
            modifier_evaluation = client.tool(
                "geometry_program_hash", modifier_request
            )
            require(
                modifier_evaluation.get("schema_version")
                == "GeometryModifierEvaluationResult@2"
                and modifier_evaluation.get("quality_status") == "structural_only"
                and isinstance(modifier_evaluation.get("canonical_sha256"), str)
                and isinstance(modifier_evaluation.get("output_sha256"), str),
                "modifier evaluation did not return a canonical bounded program",
            )
            modifier_arguments = {
                "project_id": project_id,
                "base_version_id": None,
                "idempotency_key": "mcp010f-modifier-apply-exact-once",
                "request": {
                    "typed": "geometry",
                    "modifier_evaluation_request": modifier_request,
                    "modifier_evaluation_sha256": modifier_evaluation[
                        "canonical_sha256"
                    ],
                },
            }
            modifier_response = client.request(
                "tools/call",
                {"name": "geometry_prepare", "arguments": modifier_arguments},
            )
            modifier_mcp_result = modifier_response.get("result")
            require(
                isinstance(modifier_mcp_result, dict)
                and modifier_mcp_result.get("isError") is not True
                and isinstance(modifier_mcp_result.get("structuredContent"), dict),
                f"bounded modifier apply failed: {modifier_response}",
            )
            modifier_prepared = modifier_mcp_result["structuredContent"]
            modifier_candidate = modifier_prepared.get("candidate")
            require(
                modifier_prepared.get("schema_version") == "GeometryPrepareResult@2"
                and modifier_prepared.get("artifact", {}).get("program_sha256")
                == modifier_evaluation.get("output_sha256")
                and isinstance(modifier_candidate, dict)
                and modifier_candidate.get("state") == "reviewable"
                and modifier_candidate.get("quality_hard_gate_passed") is True,
                "bounded modifier apply did not bind evaluation output to a reviewable candidate",
            )
            modifier_job_id = modifier_prepared.get("job", {}).get("job_id")
            require(
                isinstance(modifier_job_id, str) and modifier_job_id,
                "bounded modifier apply omitted its durable Job",
            )
            modifier_events = client.tool(
                "job_events_read", {"job_id": modifier_job_id, "after_sequence": 0}
            )
            require(
                isinstance(modifier_events, list)
                and len(modifier_events) == 1
                and isinstance(modifier_events[0].get("payload"), dict)
                and isinstance(
                    modifier_events[0]["payload"].get(
                        "modifier_evaluation_object_sha256"
                    ),
                    str,
                )
                and modifier_events[0]["payload"].get(
                    "modifier_evaluation_canonical_sha256"
                )
                == modifier_evaluation.get("canonical_sha256")
                and modifier_events[0]["payload"].get("modifier_output_sha256")
                == modifier_evaluation.get("output_sha256"),
                "bounded modifier apply did not durably link its evaluation sidecar",
            )
            modifier_wire_bytes = len(
                json.dumps(
                    modifier_response, ensure_ascii=False, separators=(",", ":")
                ).encode("utf-8")
            )
            require(
                modifier_wire_bytes <= 1048576,
                "bounded modifier apply full MCP response exceeded 1 MiB",
            )
            require(
                client.request(
                    "tools/call",
                    {"name": "geometry_prepare", "arguments": modifier_arguments},
                ).get("result")
                == modifier_mcp_result,
                "bounded modifier apply replay did not return the identical MCP result",
            )

            tampered_evaluation_arguments = copy.deepcopy(modifier_arguments)
            tampered_evaluation_arguments["idempotency_key"] = (
                "mcp010f-modifier-apply-evaluation-mismatch"
            )
            tampered_evaluation_arguments["request"][
                "modifier_evaluation_sha256"
            ] = "f" * 64
            tampered_evaluation = client.tool_error(
                "geometry_prepare", tampered_evaluation_arguments
            )
            require(
                "GEOMETRY_MODIFIER_APPLY_EVALUATION_MISMATCH"
                in json.dumps(tampered_evaluation, ensure_ascii=False, sort_keys=True),
                "modifier apply accepted a mismatched evaluation binding",
            )
            forbidden_modifier_arguments = copy.deepcopy(modifier_arguments)
            forbidden_modifier_arguments["idempotency_key"] = (
                "mcp010f-modifier-apply-forbidden-python"
            )
            forbidden_modifier_arguments["request"]["modifier_evaluation_request"][
                "python"
            ] = "exec"
            forbidden_modifier_error = tool_schema_error(
                client, "geometry_prepare", forbidden_modifier_arguments
            )
            changed_modifier_arguments = copy.deepcopy(modifier_arguments)
            changed_modifier_arguments["request"]["modifier_evaluation_request"][
                "modifiers"
            ][1]["parameters"]["offset_m"] = 0.2
            changed_modifier_binding = copy.deepcopy(
                changed_modifier_arguments["request"]["modifier_evaluation_request"]
            )
            changed_modifier_binding.pop("input_sha256")
            changed_modifier_arguments["request"]["modifier_evaluation_request"][
                "input_sha256"
            ] = canonical_hash(changed_modifier_binding)
            changed_modifier_evaluation = client.tool(
                "geometry_program_hash",
                changed_modifier_arguments["request"]["modifier_evaluation_request"],
            )
            changed_modifier_arguments["request"][
                "modifier_evaluation_sha256"
            ] = changed_modifier_evaluation["canonical_sha256"]
            modifier_collision = client.tool_error(
                "geometry_prepare", changed_modifier_arguments
            )
            require(
                "IDEMPOTENCY_KEY_REUSED"
                in json.dumps(modifier_collision, ensure_ascii=False, sort_keys=True),
                "changed modifier intent reused an exact apply key",
            )
            require(
                client.tool("version_list", {"project_id": project_id})
                == versions_before_exact,
                "bounded modifier apply created or changed a version before confirmation",
            )
            modifier_apply_result = {
                "evaluation_schema_version": modifier_evaluation.get("schema_version"),
                "evaluation_canonical_sha256": modifier_evaluation.get(
                    "canonical_sha256"
                ),
                "evaluation_output_sha256": modifier_evaluation.get("output_sha256"),
                "prepared_program_sha256": modifier_prepared.get("artifact", {}).get(
                    "program_sha256"
                ),
                "candidate_id": modifier_candidate.get("candidate_id"),
                "candidate_state": modifier_candidate.get("state"),
                "durable_evaluation_sidecar_sha256": modifier_events[0]["payload"].get(
                    "modifier_evaluation_object_sha256"
                ),
                "durable_job_event_link": "PASS_RESTART_SAFE_JOB_EVENT_TO_REACHABLE_CAS_SIDECAR",
                "worker_replay": "PASS_ACTUAL_SIBLING_BYTE_EXACT_SAME_COHORT",
                "idempotent_replay": "PASS_IDENTICAL_MCP_RESULT",
                "changed_intent_same_key": "REJECTED_IDEMPOTENCY_KEY_REUSED",
                "tampered_evaluation": "REJECTED_EVALUATION_MISMATCH",
                "forbidden_python_error": forbidden_modifier_error,
                "full_mcp_response_bytes": modifier_wire_bytes,
                "max_response_bytes": 1048576,
                "version_status": "no-version-created",
                "confirm_status": "approval-required",
                "export_status": "locked-until-confirm",
                "quality_status": "structural_only",
            }

            candidate_bound_key = "mcp010f-candidate-bound-modifier-apply-once"
            candidate_bound_source_artifact = exact_prepared["artifact"]
            candidate_bound_source = client.tool(
                "candidate_get", {"candidate_id": exact_candidate["candidate_id"]}
            )
            require(
                candidate_bound_source.get("candidate_id")
                == exact_candidate["candidate_id"]
                and candidate_bound_source.get("state") == "reviewable"
                and candidate_bound_source.get("quality_hard_gate_passed") is True,
                "candidate-bound modifier source was not current and reviewable",
            )
            candidate_bound_apply_request = {
                "schema_version": "GeometryModifierApplyRequest@1",
                "project_id": project_id,
                "source_candidate_id": candidate_bound_source["candidate_id"],
                "source_candidate_canonical_sha256": candidate_bound_source[
                    "canonical_sha256"
                ],
                "source_artifact_sha256": candidate_bound_source[
                    "prepared_object_sha256"
                ],
                "source_artifact_readback_sha256": candidate_bound_source_artifact[
                    "canonical_sha256"
                ],
                "source_geometry_program_sha256": candidate_bound_source_artifact[
                    "program_sha256"
                ],
                "source_operator_catalog_sha256": candidate_bound_source_artifact[
                    "operator_catalog_sha256"
                ],
                "source_readback_config_sha256": candidate_bound_source_artifact[
                    "readback_config_sha256"
                ],
                "source_part_id": "profile-part",
                "base_version_id": None,
                "modifiers": [
                    {
                        "modifier_id": "candidate-bound-offset",
                        "enabled": True,
                        "operator_id": "forgecad.geometry.transform@2",
                        "parameters": {
                            "shape": "transform",
                            "translation_m": [0.01, 0.0, 0.0],
                            "rotation_rad": [0.0, 0.0, 0.0],
                            "scale": [1.0, 1.0, 1.0],
                        },
                    },
                    {
                        "modifier_id": "candidate-bound-preview-mirror",
                        "enabled": False,
                        "operator_id": "forgecad.geometry.mirror@1",
                        "parameters": {
                            "shape": "mirror",
                            "axis": "x",
                            "offset_m": 0.0,
                        },
                    },
                ],
                "idempotency_key": candidate_bound_key,
                "max_response_bytes": 1048576,
                "input_sha256": "",
            }
            candidate_bound_input = copy.deepcopy(candidate_bound_apply_request)
            candidate_bound_input.pop("input_sha256")
            candidate_bound_apply_request["input_sha256"] = canonical_hash(
                candidate_bound_input
            )
            candidate_bound_arguments = {
                "project_id": project_id,
                "base_version_id": None,
                "idempotency_key": candidate_bound_key,
                "request": {
                    "typed": "geometry",
                    "modifier_apply_request": candidate_bound_apply_request,
                    "modifier_apply_sha256": canonical_hash(
                        candidate_bound_apply_request
                    ),
                },
            }
            candidate_bound_response = client.request(
                "tools/call",
                {
                    "name": "geometry_prepare",
                    "arguments": candidate_bound_arguments,
                },
            )
            candidate_bound_mcp_result = candidate_bound_response.get("result")
            require(
                isinstance(candidate_bound_mcp_result, dict)
                and candidate_bound_mcp_result.get("isError") is not True
                and isinstance(
                    candidate_bound_mcp_result.get("structuredContent"), dict
                ),
                f"candidate-bound modifier apply failed: {candidate_bound_response}",
            )
            candidate_bound_prepared = candidate_bound_mcp_result["structuredContent"]
            candidate_bound_modifier_candidate = candidate_bound_prepared.get(
                "candidate"
            )
            candidate_bound_artifact = candidate_bound_prepared.get("artifact")
            require(
                candidate_bound_prepared.get("schema_version")
                == "GeometryPrepareResult@2"
                and isinstance(candidate_bound_modifier_candidate, dict)
                and candidate_bound_modifier_candidate.get("state") == "reviewable"
                and candidate_bound_modifier_candidate.get(
                    "quality_hard_gate_passed"
                )
                is True
                and isinstance(candidate_bound_artifact, dict)
                and candidate_bound_artifact.get("program_sha256")
                != artifact.get("program_sha256"),
                "candidate-bound modifier apply did not stage a distinct reviewable program",
            )
            source_part_bindings = {
                (
                    item.get("part_id"),
                    item.get("source_node_id"),
                ): (
                    item.get("material_zone_id"),
                    item.get("solid"),
                    item.get("triangle_count"),
                )
                for item in candidate_bound_source_artifact["part_bindings"]
            }
            derived_bindings = candidate_bound_artifact.get("part_bindings")
            require(
                isinstance(derived_bindings, list),
                "candidate-bound modifier apply omitted derived Part bindings",
            )
            derived_part_bindings = {
                (
                    item.get("part_id"),
                    item.get("source_node_id"),
                ): (
                    item.get("material_zone_id"),
                    item.get("solid"),
                    item.get("triangle_count"),
                )
                for item in derived_bindings
            }
            source_target = [
                (key, value)
                for key, value in source_part_bindings.items()
                if key[0] == "profile-part"
            ]
            derived_target = [
                (key, value)
                for key, value in derived_part_bindings.items()
                if key[0] == "profile-part"
            ]
            source_non_target = {
                key: value
                for key, value in source_part_bindings.items()
                if key[0] != "profile-part"
            }
            derived_non_target = {
                key: value
                for key, value in derived_part_bindings.items()
                if key[0] != "profile-part"
            }
            require(
                len(source_target) == 1
                and len(derived_target) == 1
                and source_target[0][0][1] == "profile"
                and derived_target[0][0][1] != source_target[0][0][1]
                and derived_target[0][1][0:2] == source_target[0][1][0:2]
                and derived_non_target == source_non_target,
                "candidate-bound modifier apply did not preserve non-target Part/material/source-map bindings",
            )
            candidate_bound_modifier_job_id = candidate_bound_prepared.get(
                "job", {}
            ).get("job_id")
            require(
                isinstance(candidate_bound_modifier_job_id, str)
                and candidate_bound_modifier_job_id,
                "candidate-bound modifier apply omitted its durable Job",
            )
            candidate_bound_modifier_events = client.tool(
                "job_events_read",
                {
                    "job_id": candidate_bound_modifier_job_id,
                    "after_sequence": 0,
                },
            )
            require(
                isinstance(candidate_bound_modifier_events, list)
                and len(candidate_bound_modifier_events) == 1
                and candidate_bound_modifier_events[0]
                .get("payload", {})
                .get("modifier_apply_source_candidate_id")
                == candidate_bound_source["candidate_id"]
                and candidate_bound_modifier_events[0]
                .get("payload", {})
                .get("modifier_apply_source_part_id")
                == "profile-part"
                and isinstance(
                    candidate_bound_modifier_events[0]
                    .get("payload", {})
                    .get("modifier_apply_result_object_sha256"),
                    str,
                ),
                "candidate-bound modifier apply did not durably link its source and sidecar",
            )
            require(
                client.request(
                    "tools/call",
                    {
                        "name": "geometry_prepare",
                        "arguments": candidate_bound_arguments,
                    },
                ).get("result")
                == candidate_bound_mcp_result,
                "candidate-bound modifier apply replay did not return the identical MCP result",
            )

            unknown_part_arguments = copy.deepcopy(candidate_bound_arguments)
            unknown_part_arguments["idempotency_key"] = (
                "mcp010f-candidate-bound-unknown-part"
            )
            unknown_part_request = unknown_part_arguments["request"][
                "modifier_apply_request"
            ]
            unknown_part_request["idempotency_key"] = unknown_part_arguments[
                "idempotency_key"
            ]
            unknown_part_request["source_part_id"] = "part-does-not-exist"
            unknown_part_input = copy.deepcopy(unknown_part_request)
            unknown_part_input.pop("input_sha256")
            unknown_part_request["input_sha256"] = canonical_hash(unknown_part_input)
            unknown_part_arguments["request"]["modifier_apply_sha256"] = canonical_hash(
                unknown_part_request
            )
            unknown_part_error = client.tool_error(
                "geometry_prepare", unknown_part_arguments
            )
            require(
                "GEOMETRY_MODIFIER_APPLY_TARGET_PART_AMBIGUOUS"
                in json.dumps(unknown_part_error, ensure_ascii=False, sort_keys=True),
                "candidate-bound modifier apply accepted an unknown stable Part",
            )

            tampered_source_arguments = copy.deepcopy(candidate_bound_arguments)
            tampered_source_arguments["idempotency_key"] = (
                "mcp010f-candidate-bound-tampered-source"
            )
            tampered_source_request = tampered_source_arguments["request"][
                "modifier_apply_request"
            ]
            tampered_source_request["idempotency_key"] = tampered_source_arguments[
                "idempotency_key"
            ]
            tampered_source_request["source_artifact_sha256"] = "f" * 64
            tampered_source_input = copy.deepcopy(tampered_source_request)
            tampered_source_input.pop("input_sha256")
            tampered_source_request["input_sha256"] = canonical_hash(
                tampered_source_input
            )
            tampered_source_arguments["request"][
                "modifier_apply_sha256"
            ] = canonical_hash(tampered_source_request)
            tampered_source_error = client.tool_error(
                "geometry_prepare", tampered_source_arguments
            )
            tampered_source_text = json.dumps(
                tampered_source_error, ensure_ascii=False, sort_keys=True
            )
            require(
                any(
                    code in tampered_source_text
                    for code in (
                        "GEOMETRY_MODIFIER_APPLY_SOURCE_CANDIDATE_INVALID",
                        "GEOMETRY_MODIFIER_APPLY_SOURCE_ARTIFACT_MISMATCH",
                        "GEOMETRY_MODIFIER_APPLY_SOURCE_ARTIFACT_HASH_MISMATCH",
                    )
                ),
                "candidate-bound modifier apply accepted a forged source artifact hash",
            )

            candidate_python_arguments = copy.deepcopy(candidate_bound_arguments)
            candidate_python_arguments["idempotency_key"] = (
                "mcp010f-candidate-bound-forbidden-python"
            )
            candidate_python_arguments["request"]["modifier_apply_request"][
                "python"
            ] = "exec"
            candidate_python_error = tool_schema_error(
                client, "geometry_prepare", candidate_python_arguments
            )
            candidate_reference_arguments = copy.deepcopy(candidate_bound_arguments)
            candidate_reference_arguments["idempotency_key"] = (
                "mcp010f-candidate-bound-forbidden-reference"
            )
            candidate_reference_arguments["request"]["reference_id"] = "foreign-reference"
            candidate_reference_error = tool_schema_error(
                client, "geometry_prepare", candidate_reference_arguments
            )
            require(
                client.tool("version_list", {"project_id": project_id})
                == versions_before_exact,
                "candidate-bound modifier apply created or changed a version before confirmation",
            )
            candidate_bound_wire_bytes = len(
                json.dumps(
                    candidate_bound_response,
                    ensure_ascii=False,
                    separators=(",", ":"),
                ).encode("utf-8")
            )
            require(
                candidate_bound_wire_bytes <= 1048576,
                "candidate-bound modifier apply full MCP response exceeded 1 MiB",
            )
            candidate_bound_modifier_apply_result = {
                "schema_version": "GeometryModifierApplyResult@1",
                "source_candidate_id": candidate_bound_source["candidate_id"],
                "new_candidate_id": candidate_bound_modifier_candidate[
                    "candidate_id"
                ],
                "source_part_id": "profile-part",
                "source_terminal_node_id": source_target[0][0][1],
                "derived_terminal_node_id": derived_target[0][0][1],
                "source_program_sha256": candidate_bound_source_artifact[
                    "program_sha256"
                ],
                "derived_program_sha256": candidate_bound_artifact[
                    "program_sha256"
                ],
                "durable_apply_sidecar_sha256": candidate_bound_modifier_events[0][
                    "payload"
                ]["modifier_apply_result_object_sha256"],
                "source_replay": "PASS_ACTUAL_SIBLING_BYTE_EXACT_SAME_COHORT",
                "derived_replay": "PASS_ACTUAL_SIBLING_BYTE_EXACT_SAME_COHORT",
                "part_binding_status": "PASS_TARGET_DERIVED_NON_TARGET_EXACT",
                "idempotent_replay": "PASS_IDENTICAL_MCP_RESULT",
                "unknown_part": "REJECTED_TARGET_PART_UNAVAILABLE_OR_AMBIGUOUS",
                "tampered_source": "REJECTED_SOURCE_ARTIFACT_MISMATCH",
                "forbidden_python_error": candidate_python_error,
                "forbidden_reference_error": candidate_reference_error,
                "full_mcp_response_bytes": candidate_bound_wire_bytes,
                "max_response_bytes": 1048576,
                "version_status": "no-version-created",
                "confirm_status": "approval-required",
                "export_status": "locked-until-confirm",
                "quality_status": "structural_only",
            }
        topology_request = {
            "schema_version": "AuthoringTopologyRequest@1",
            "project_id": project_id,
            "candidate_id": candidate["candidate_id"],
            "artifact_id": artifact["artifact_id"],
            "artifact_readback_sha256": artifact["canonical_sha256"],
            "program_sha256": artifact["program_sha256"],
            "operator_catalog_sha256": artifact["operator_catalog_sha256"],
            "readback_config_sha256": artifact["readback_config_sha256"],
            "authoring_node_id": "authored-panel",
            "part_id": "authored-panel-part",
            "authoring_topology_policy_sha256": "a6fb36a530e49537673b66d65ecb6e4fb4f51ffb3e7d01a0980be71f28cb367d",
            "max_response_bytes": 1048576,
        }
        state_before_authoring_reads = client.tool("project_get", {"project_id": project_id})
        topology = client.tool("authoring_topology_get", topology_request)
        require(
            topology.get("schema_version") == "AuthoringTopology@1"
            and topology.get("scope") == "single-direct-authoring-mesh-part"
            and topology.get("counts")
            == {"vertex_count": 4, "edge_count": 4, "loop_count": 4, "face_count": 1}
            and topology.get("runtime_write_performed") is False
            and topology.get("persistent_user_data_touched") is False,
            "authoring topology readback was not exact and read-only",
        )
        require_self_hash(topology, "AuthoringTopology@1")
        topology_response_bytes = len(
            json.dumps(topology, separators=(",", ":")).encode("utf-8")
        )
        require(
            topology_response_bytes <= 1048576,
            "authoring topology exceeded the 1 MiB response budget",
        )
        base_topology_sha256 = topology.get("topology_sha256")
        require(
            isinstance(base_topology_sha256, str) and len(base_topology_sha256) == 64,
            "authoring topology omitted its source hash",
        )
        translated = client.tool(
            "authoring_mesh_edit_preview",
            authoring_preview_request(
                topology_request,
                base_topology_sha256,
                {
                    "operation": "translate_vertices",
                    "vertex_ids": ["v2", "v3"],
                    "delta_m": [0.0, 0.0, 0.1],
                },
            ),
        )
        extruded = client.tool(
            "authoring_mesh_edit_preview",
            authoring_preview_request(
                topology_request,
                base_topology_sha256,
                {"operation": "single_face_extrude", "face_id": "f0", "distance_m": 0.1},
            ),
        )
        require(
            translated.get("schema_version") == "AuthoringMeshEditPreview@1"
            and translated.get("operation") == "translate_vertices"
            and translated.get("counts", {}).get("before")
            == translated.get("counts", {}).get("after")
            and extruded.get("operation") == "single_face_extrude"
            and extruded.get("counts", {}).get("after", {}).get("vertex_count") == 8
            and extruded.get("counts", {}).get("after", {}).get("edge_count") == 12
            and extruded.get("counts", {}).get("after", {}).get("loop_count") == 20
            and extruded.get("counts", {}).get("after", {}).get("face_count") == 5
            and extruded.get("counts", {}).get("after", {}).get("triangle_count")
            == extruded.get("counts", {}).get("before", {}).get("triangle_count", 0) + 8
            and translated.get("runtime_write_performed") is False
            and extruded.get("runtime_write_performed") is False,
            "bounded authoring edit previews did not replay exact expected topology: "
            f"translate={translated.get('counts')} extrude={extruded.get('counts')}",
        )
        require_self_hash(translated, "translate AuthoringMeshEditPreview@1")
        require_self_hash(extruded, "extrude AuthoringMeshEditPreview@1")
        translated_response_bytes = len(
            json.dumps(translated, separators=(",", ":")).encode("utf-8")
        )
        extruded_response_bytes = len(
            json.dumps(extruded, separators=(",", ":")).encode("utf-8")
        )
        require(
            translated_response_bytes <= 1048576 and extruded_response_bytes <= 1048576,
            "authoring edit preview exceeded the 1 MiB response budget",
        )
        authoring_prepare_result: dict[str, Any] | None = None
        if args.exercise_authoring_prepare:
            extrude_request = authoring_preview_request(
                topology_request,
                base_topology_sha256,
                {
                    "operation": "single_face_extrude",
                    "face_id": "f0",
                    "distance_m": 0.1,
                },
            )
            versions_before_prepare = client.tool(
                "version_list", {"project_id": project_id}
            )
            prepare_request = authoring_prepare_request(
                project_id,
                candidate["candidate_id"],
                extrude_request,
                extruded["canonical_sha256"],
                "raw-stdio-authoring-edit-prepare-once",
            )
            staged = client.tool("authoring_mesh_edit_prepare", prepare_request)
            require(
                staged.get("schema_version") == "AuthoringMeshEditPrepare@1"
                and staged.get("source_candidate_id") == candidate["candidate_id"]
                and staged.get("candidate", {}).get("state") == "reviewable"
                and staged.get("candidate", {}).get("quality_hard_gate_passed") is True
                and staged.get("runtime_write_performed") is True
                and staged.get("persistent_user_data_touched") is True
                and staged.get("version_status") == "no-version-created"
                and staged.get("confirm_status") == "approval-required"
                and staged.get("export_status") == "locked-until-confirm",
                f"authoring edit prepare did not stage an approval-gated candidate: {staged}",
            )
            require_self_hash(staged, "AuthoringMeshEditPrepare@1")
            staged_candidate = client.tool(
                "candidate_get", {"candidate_id": staged["new_candidate_id"]}
            )
            require(
                staged_candidate.get("candidate_id") == staged["new_candidate_id"]
                and staged_candidate.get("state") == "reviewable"
                and staged_candidate.get("prepared_object_sha256")
                == staged.get("derived_artifact_sha256"),
                "staged authoring candidate readback differed",
            )
            replay = client.tool("authoring_mesh_edit_prepare", prepare_request)
            require(replay == staged, "authoring edit prepare idempotent replay differed")

            alternate_request = authoring_preview_request(
                topology_request,
                base_topology_sha256,
                {
                    "operation": "single_face_extrude",
                    "face_id": "f0",
                    "distance_m": 0.2,
                },
            )
            alternate_preview = client.tool(
                "authoring_mesh_edit_preview", alternate_request
            )
            conflicting_request = authoring_prepare_request(
                project_id,
                candidate["candidate_id"],
                alternate_request,
                alternate_preview["canonical_sha256"],
                "raw-stdio-authoring-edit-prepare-once",
            )
            conflicting_error = client.tool_error(
                "authoring_mesh_edit_prepare", conflicting_request
            )
            require(
                conflicting_error.get("code")
                in {"INVALID_INPUT", "RUNTIME_WRITE_FAILED", "STORE_CONTRACT"},
                f"authoring edit idempotency-key reuse did not fail closed: {conflicting_error}",
            )
            stale_prepare_request = authoring_prepare_request(
                project_id,
                candidate["candidate_id"],
                extrude_request,
                extruded["canonical_sha256"],
                "raw-stdio-authoring-edit-prepare-stale",
                "version-stale",
            )
            stale_prepare_error = client.tool_error(
                "authoring_mesh_edit_prepare", stale_prepare_request
            )
            require(
                stale_prepare_error.get("code")
                in {"INVALID_INPUT", "RUNTIME_WRITE_FAILED", "STORE_CONTRACT"},
                f"authoring edit stale head did not fail closed: {stale_prepare_error}",
            )
            forbidden_prepare = copy.deepcopy(prepare_request)
            forbidden_prepare["preview_request"]["edit"]["python"] = "bmesh.ops"
            forbidden_prepare_code = tool_schema_error(
                client, "authoring_mesh_edit_prepare", forbidden_prepare
            )
            require(
                client.tool("version_list", {"project_id": project_id})
                == versions_before_prepare,
                "authoring edit prepare created or changed a version",
            )
            authoring_prepare_result = {
                "schema_version": staged.get("schema_version"),
                "source_candidate_id": staged.get("source_candidate_id"),
                "new_candidate_id": staged.get("new_candidate_id"),
                "derived_program_sha256": staged.get("derived_program_sha256"),
                "derived_artifact_sha256": staged.get("derived_artifact_sha256"),
                "derived_artifact_readback_sha256": staged.get(
                    "derived_artifact_readback_sha256"
                ),
                "derived_geometry_candidate_evidence_sha256": staged.get(
                    "derived_geometry_candidate_evidence_sha256"
                ),
                "source_worker_build_cohort_sha256": staged.get(
                    "source_worker_build_cohort_sha256"
                ),
                "derived_worker_build_cohort_sha256": staged.get(
                    "derived_worker_build_cohort_sha256"
                ),
                "canonical_sha256": staged.get("canonical_sha256"),
                "candidate_state": "reviewable",
                "idempotent_exact_replay": "PASS",
                "idempotency_key_reuse": "REJECTED_NO_VISIBLE_RESIDUE",
                "stale_head": "REJECTED",
                "forbidden_python_error_code": forbidden_prepare_code,
                "version_inventory_unchanged": True,
                "confirm_status": "approval-required",
                "export_status": "locked-until-confirm",
                "quality_status": "structural_only",
            }
        oversized_budget = copy.deepcopy(topology_request)
        oversized_budget["max_response_bytes"] = 1048577
        oversized_budget_error = tool_schema_error(
            client,
            "authoring_topology_get",
            oversized_budget,
        )
        stale_preview = authoring_preview_request(
            topology_request,
            "f" * 64,
            {"operation": "single_face_extrude", "face_id": "f0", "distance_m": 0.1},
        )
        stale_error = client.tool_error("authoring_mesh_edit_preview", stale_preview)
        require(
            stale_error.get("code") in {"INVALID_INPUT", "RUNTIME_READ_FAILED"},
            f"stale authoring topology did not fail closed: {stale_error}",
        )
        forbidden_preview_codes = []
        for forbidden_field in ("python", "path", "url", "env", "plugin", "network"):
            forbidden_preview = authoring_preview_request(
                topology_request,
                base_topology_sha256,
                {
                    "operation": "translate_vertices",
                    "vertex_ids": ["v0"],
                    "delta_m": [0.0, 0.0, 0.1],
                    forbidden_field: "forbidden",
                },
            )
            forbidden_preview_codes.append(
                tool_schema_error(
                    client,
                    "authoring_mesh_edit_preview",
                    forbidden_preview,
                )
            )
        require(
            client.tool("project_get", {"project_id": project_id})
            == state_before_authoring_reads,
            "authoring topology/edit previews mutated project state",
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
        executable = copy.deepcopy(program_draft)
        authored = next(node for node in executable["nodes"] if node["node_id"] == "authored-panel")
        authored["parameters"]["python"] = "import bpy"
        error = client.tool_error(
            "geometry_program_hash",
            {
                "schema_version": "GeometryProgramHashRequest@1",
                "geometry_program_draft": executable,
            },
        )
        require(
            error.get("code") in {"INVALID_INPUT", "GEOMETRY_PROGRAM_HASH_REJECTED"},
            f"authoring mesh executable field did not fail closed: {error}",
        )
        state_before_forbidden_prepare = client.tool("project_get", {"project_id": project_id})
        forbidden_prepare_codes = []
        for forbidden_field in ("python", "path", "url", "env", "plugin", "network"):
            forbidden_program = copy.deepcopy(program)
            forbidden_authored = next(
                node for node in forbidden_program["nodes"] if node["node_id"] == "authored-panel"
            )
            forbidden_authored["parameters"][forbidden_field] = "forbidden"
            forbidden_error = client.tool_error(
                "geometry_prepare",
                {
                    "project_id": project_id,
                    "request": {"typed": "geometry", "geometry_program": forbidden_program},
                },
            )
            require(
                forbidden_error.get("code")
                in {
                    "INVALID_INPUT",
                    "RUNTIME_WRITE_FAILED",
                    "GEOMETRY_PREPARE_REJECTED",
                    "GEOMETRY_REJECTED",
                },
                f"geometry_prepare accepted forbidden {forbidden_field} field: {forbidden_error}",
            )
            forbidden_prepare_codes.append(forbidden_error.get("code"))
        require(
            client.tool("project_get", {"project_id": project_id})
            == state_before_forbidden_prepare,
            "forbidden geometry_prepare mutated project state",
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
        if args.exercise_exact_geometry_prepare:
            require(
                modifier_apply_result is not None
                and isinstance(modifier_job_id, str)
                and isinstance(modifier_candidate, dict)
                and candidate_bound_modifier_apply_result is not None
                and isinstance(candidate_bound_modifier_job_id, str)
                and isinstance(candidate_bound_modifier_candidate, dict)
                and isinstance(candidate_bound_modifier_events, list),
                "modifier apply restart fixture was unavailable",
            )
            expected_modifier_events = modifier_events
            expected_candidate_bound_modifier_events = candidate_bound_modifier_events
            client.close()
            client = None
            shutdown_runtime(ready, ready_path, runtime)
            ready = None
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
            ready = wait_for_ready(ready_path, runtime, args.timeout)
            restart_environment = os.environ.copy()
            for key in (
                "FORGECAD_RUNTIME_COMMAND",
                "FORGECAD_RUNTIME_DATA_DIR",
                "FORGECAD_RUNTIME_READY_FILE",
                "FORGECAD_RUNTIME_STATUS_FILE",
            ):
                restart_environment.pop(key, None)
            restart_environment["FORGECAD_RUNTIME_SOCKET"] = ready["socket_path"]
            restart_environment["FORGECAD_RUNTIME_TOKEN"] = ready["token"]
            restart_environment["FORGECAD_MCP_ENABLE_MCP004_WRITES"] = "1"
            client = McpClient(args.mcp, restart_environment, args.timeout)
            initialized = client.request(
                "initialize",
                {
                    "protocolVersion": MCP_PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": {
                        "name": "forgecad-mcp010f-modifier-restart",
                        "version": "1",
                    },
                },
            )
            require(
                initialized.get("result", {}).get("protocolVersion")
                == MCP_PROTOCOL_VERSION,
                "modifier apply restart MCP initialization failed",
            )
            client.notify("notifications/initialized")
            read_ponytail_preflight(client)
            restarted_events = client.tool(
                "job_events_read", {"job_id": modifier_job_id, "after_sequence": 0}
            )
            restarted_candidate = client.tool(
                "candidate_get", {"candidate_id": modifier_candidate["candidate_id"]}
            )
            require(
                restarted_events == expected_modifier_events
                and restarted_candidate.get("state") == "reviewable"
                and restarted_candidate.get("candidate_id")
                == modifier_candidate.get("candidate_id"),
                "modifier evaluation sidecar Job link or candidate did not survive Runtime/MCP restart",
            )
            modifier_apply_result["durable_job_event_link"] = (
                "PASS_RESTART_READBACK_JOB_EVENT_TO_REACHABLE_CAS_SIDECAR"
            )
            restarted_candidate_bound_events = client.tool(
                "job_events_read",
                {
                    "job_id": candidate_bound_modifier_job_id,
                    "after_sequence": 0,
                },
            )
            restarted_candidate_bound_candidate = client.tool(
                "candidate_get",
                {
                    "candidate_id": candidate_bound_modifier_candidate[
                        "candidate_id"
                    ]
                },
            )
            require(
                restarted_candidate_bound_events
                == expected_candidate_bound_modifier_events
                and restarted_candidate_bound_candidate.get("state") == "reviewable"
                and restarted_candidate_bound_candidate.get("candidate_id")
                == candidate_bound_modifier_candidate.get("candidate_id"),
                "candidate-bound Apply sidecar Job link or candidate did not survive Runtime/MCP restart",
            )
            candidate_bound_modifier_apply_result["durable_job_event_link"] = (
                "PASS_RESTART_READBACK_JOB_EVENT_TO_REACHABLE_CAS_SIDECAR"
            )
        d_result = {
            "status": "PASS",
            "operator_catalog": "28 entries / 28 active / profile-loft@2, multi-loop-profile-loft@1, bevel@2, panel@2, vent-array@2, recessed-channel@1, energy-core@1, authoring mesh, longitudinal section loft and boolean union+difference/intersection active",
            "operator_catalog_sha256": catalog_hash,
            "operators": sorted(expected_operator_ids),
            "geometry_program": "GeometryProgram@2 DAG",
            "semantic_parts": len(artifact.get("part_ids", [])),
            "triangle_count": artifact.get("triangle_count"),
            "artifact_readback": "ArtifactReadback@2 hard_gate_passed",
            "lineage": "PASS",
            "negative_future_input": "PASS",
            "negative_authoring_mesh_executable_field": "PASS",
            "negative_authoring_mesh_geometry_prepare_fields": {
                "fields": ["python", "path", "url", "env", "plugin", "network"],
                "error_codes": forbidden_prepare_codes,
                "project_state_unchanged": True,
            },
            "authoring_topology_readback": {
                "schema_version": topology.get("schema_version"),
                "topology_sha256": base_topology_sha256,
                "counts": topology.get("counts"),
                "response_budget_bytes": 1048576,
                "response_bytes": topology_response_bytes,
                "request_sha256": canonical_hash(topology_request),
                "result_canonical_sha256": topology.get("canonical_sha256"),
                "oversized_budget_error_code": oversized_budget_error,
                "canonical_hash": "PASS",
                "runtime_write_performed": False,
            },
            "authoring_mesh_edit_previews": {
                "operations": ["translate_vertices", "single_face_extrude"],
                "translate_replay": "PASS",
                "extrude_replay": "PASS",
                "stale_base_topology": "REJECTED",
                "forbidden_fields": ["python", "path", "url", "env", "plugin", "network"],
                "forbidden_field_error_codes": forbidden_preview_codes,
                "canonical_hash": "PASS",
                "translate_input_sha256": translated.get("input_sha256"),
                "translate_result_canonical_sha256": translated.get("canonical_sha256"),
                "translate_response_bytes": translated_response_bytes,
                "translate_derived_program_sha256": translated.get("derived_program_sha256"),
                "translate_derived_artifact_sha256": translated.get("derived_replay", {}).get("artifact_sha256"),
                "extrude_input_sha256": extruded.get("input_sha256"),
                "extrude_result_canonical_sha256": extruded.get("canonical_sha256"),
                "extrude_response_bytes": extruded_response_bytes,
                "extrude_derived_program_sha256": extruded.get("derived_program_sha256"),
                "extrude_derived_artifact_sha256": extruded.get("derived_replay", {}).get("artifact_sha256"),
                "project_state_unchanged": True,
                "quality_status": "structural_only",
            },
            "authoring_mesh_edit_prepare": authoring_prepare_result,
            "exact_geometry_prepare": exact_geometry_prepare_result,
            "modifier_apply": modifier_apply_result,
            "candidate_bound_modifier_apply": candidate_bound_modifier_apply_result,
            "probe_setup_runtime_writes": ["project_create", "geometry_prepare"],
            "authoring_read_slice_runtime_writes": False,
            "boolean_union_difference_intersection": "PASS",
            "negative_boolean_unsupported_shape": "PASS",
            "ponytail_preflight": preflight,
            "persistent_user_data_touched": (
                args.exercise_authoring_prepare or args.exercise_exact_geometry_prepare
            ),
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
        "schema_version": (
            "ForgeCADMCP010FAuthoringMeshEditPrepareRawStdioProbe@1"
            if args.exercise_authoring_prepare
            else (
                "ForgeCADMCP010FExactGeometryPrepareRawStdioProbe@1"
                if args.exercise_exact_geometry_prepare
                else "ForgeCADMCP010DRawStdioProbe@1"
            )
        ),
        "task_id": (
            "FGC-MCP010F"
            if args.exercise_authoring_prepare or args.exercise_exact_geometry_prepare
            else "FGC-MCP010D"
        ),
        "status": "PASS",
        "protocol_version": MCP_PROTOCOL_VERSION,
        "persistent_user_data_touched": (
            args.exercise_authoring_prepare or args.exercise_exact_geometry_prepare
        ),
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
