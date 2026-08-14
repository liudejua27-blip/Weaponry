#!/usr/bin/env python3
"""Exercise an explicit MCP010B GeometryProgram@2 stdio path in isolation.

This probe deliberately uses only the public MCP JSON-RPC surface after either
source binaries or a packaged Dev.app cohort have been built. Its Runtime
database, CAS and ready handoff live below a caller-owned temporary directory.
No reference image, user path or persistent ForgeCAD data is involved.
"""

from __future__ import annotations

import argparse
import copy
import json
import os
import re
import selectors
import socket
import subprocess
import sys
import time
from pathlib import Path
from typing import Any


MAX_RESPONSE_BYTES = 8 * 1024 * 1024
MCP_PROTOCOL_VERSION = "2025-06-18"


class GateFailure(RuntimeError):
    """A compact, non-path-bearing failure used by the isolated gate."""


class McpClient:
    def __init__(self, command: Path, environment: dict[str, str], timeout: float) -> None:
        self._timeout = timeout
        self._next_id = 1
        self._process = subprocess.Popen(
            [str(command), "serve", "--stdio"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            env=environment,
            bufsize=1,
        )
        if self._process.stdin is None or self._process.stdout is None:
            raise GateFailure("MCP stdio pipes were unavailable")
        self._selector = selectors.DefaultSelector()
        self._selector.register(self._process.stdout, selectors.EVENT_READ)

    def request(self, method: str, params: dict[str, Any] | None = None) -> dict[str, Any]:
        identifier = self._next_id
        self._next_id += 1
        request: dict[str, Any] = {"jsonrpc": "2.0", "id": identifier, "method": method}
        if params is not None:
            request["params"] = params
        assert self._process.stdin is not None
        self._process.stdin.write(json.dumps(request, separators=(",", ":")) + "\n")
        self._process.stdin.flush()
        deadline = time.monotonic() + self._timeout
        while time.monotonic() < deadline:
            events = self._selector.select(max(0.0, deadline - time.monotonic()))
            if not events:
                break
            assert self._process.stdout is not None
            line = self._process.stdout.readline(MAX_RESPONSE_BYTES + 1)
            if not line:
                break
            if len(line.encode("utf-8")) > MAX_RESPONSE_BYTES:
                raise GateFailure("MCP response exceeded the bounded probe limit")
            try:
                response = json.loads(line)
            except json.JSONDecodeError as error:
                raise GateFailure("MCP emitted invalid JSON-RPC") from error
            if response.get("id") == identifier:
                if not isinstance(response, dict):
                    raise GateFailure("MCP response was not an object")
                return response
        raise GateFailure(f"MCP response timed out for {method}")

    def notify(self, method: str) -> None:
        assert self._process.stdin is not None
        self._process.stdin.write(
            json.dumps({"jsonrpc": "2.0", "method": method}, separators=(",", ":")) + "\n"
        )
        self._process.stdin.flush()

    def tool(self, name: str, arguments: dict[str, Any] | None = None) -> Any:
        response = self.request(
            "tools/call", {"name": name, "arguments": arguments or {}}
        )
        if "error" in response:
            raise GateFailure(f"MCP protocol error for {name}: {response.get('error')}")
        result = response.get("result")
        if not isinstance(result, dict):
            raise GateFailure(f"MCP tool failed for {name}: malformed result")
        if result.get("isError"):
            error_payload = result.get("structuredContent") or result.get("content") or "untyped error"
            rendered = json.dumps(error_payload, ensure_ascii=False, sort_keys=True)
            raise GateFailure(f"MCP tool failed for {name}: {rendered[:2048]}")
        return result.get("structuredContent")

    def tool_error(self, name: str, arguments: dict[str, Any]) -> dict[str, Any]:
        response = self.request("tools/call", {"name": name, "arguments": arguments})
        if "error" in response:
            raise GateFailure(f"MCP protocol error for negative {name}")
        result = response.get("result")
        if not isinstance(result, dict) or result.get("isError") is not True:
            raise GateFailure(f"negative {name} unexpectedly succeeded")
        structured = result.get("structuredContent")
        if not isinstance(structured, dict):
            raise GateFailure(f"negative {name} did not return a typed error")
        return structured

    def resource_json(self, uri: str) -> dict[str, Any]:
        response = self.request("resources/read", {"uri": uri})
        if "error" in response:
            raise GateFailure("operator catalog resource request failed")
        contents = response.get("result", {}).get("contents")
        if not isinstance(contents, list) or len(contents) != 1:
            raise GateFailure("operator catalog resource had an invalid content envelope")
        text = contents[0].get("text") if isinstance(contents[0], dict) else None
        if not isinstance(text, str) or len(text.encode("utf-8")) > MAX_RESPONSE_BYTES:
            raise GateFailure("operator catalog resource was not bounded JSON text")
        try:
            value = json.loads(text)
        except json.JSONDecodeError as error:
            raise GateFailure("operator catalog resource was invalid JSON") from error
        if not isinstance(value, dict):
            raise GateFailure("operator catalog resource was not an object")
        return value

    def close(self) -> None:
        if self._process.stdin is not None and not self._process.stdin.closed:
            self._process.stdin.close()
        try:
            self._process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self._process.kill()
            self._process.wait(timeout=5)
            raise GateFailure("MCP did not stop after stdio EOF")
        if self._process.returncode != 0:
            raise GateFailure("MCP exited unexpectedly during isolated cleanup")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--data-root", type=Path, required=True)
    parser.add_argument(
        "--expected-build-cohort",
        help="Optional 64-character cohort that both explicit MCP and Runtime binaries must report.",
    )
    parser.add_argument(
        "--evidence",
        type=Path,
        help="Optional JSON receipt path beneath docs/evidence.",
    )
    parser.add_argument("--timeout", type=float, default=20.0)
    return parser.parse_args()


def build_identity(path: Path) -> dict[str, Any]:
    completed = subprocess.run(
        [str(path), "--build-identity"],
        text=True,
        encoding="utf-8",
        capture_output=True,
        timeout=20,
        check=True,
    )
    try:
        value = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise GateFailure("explicit binary did not return a JSON build identity") from error
    if not isinstance(value, dict):
        raise GateFailure("explicit binary build identity was not an object")
    return value


def write_receipt(path: Path | None, receipt: dict[str, Any]) -> None:
    if path is None:
        return
    resolved = path if path.is_absolute() else Path(__file__).resolve().parents[1] / path
    evidence_root = Path(__file__).resolve().parents[1] / "docs" / "evidence"
    try:
        resolved.resolve().relative_to(evidence_root.resolve())
    except ValueError as error:
        raise GateFailure("probe evidence must stay under docs/evidence") from error
    if resolved.suffix != ".json":
        raise GateFailure("probe evidence must be a JSON file")
    resolved.parent.mkdir(parents=True, exist_ok=True)
    resolved.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def v2_program_draft(project_id: str, catalog_hash: str) -> dict[str, Any]:
    value: dict[str, Any] = {
        "schema_version": "GeometryProgram@2",
        "project_id": project_id,
        "representation_plan_sha256": "a" * 64,
        "operator_catalog_sha256": catalog_hash,
        "units": {
            "length": "meter",
            "angle": "radian",
            "coordinate_system": "right-handed-y-up",
        },
        "budgets": {
            "max_nodes": 4,
            "max_triangles": 10000,
            "max_glb_bytes": 1048576,
            "max_worker_memory_bytes": 536870912,
            "max_runtime_ms": 10000,
        },
        "nodes": [
            {
                "node_id": "shell",
                "operator_id": "forgecad.geometry.primitive@2",
                "inputs": [],
                "parameters": {
                    "shape": "box",
                    "size_m": [1.2, 1.6, 0.55],
                    "position_m": [0.0, 1.7, 0.0],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
            },
            {
                "node_id": "joint",
                "operator_id": "forgecad.geometry.primitive@2",
                "inputs": [],
                "parameters": {
                    "shape": "cylinder",
                    "radius_m": 0.3,
                    "height_m": 0.8,
                    "radial_segments": 16,
                    "position_m": [0.0, 0.55, 0.0],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
            },
            {
                "node_id": "sensor",
                "operator_id": "forgecad.geometry.primitive@2",
                "inputs": [],
                "parameters": {
                    "shape": "ellipsoid",
                    "radii_m": [0.25, 0.35, 0.2],
                    "longitude_segments": 16,
                    "latitude_segments": 8,
                    "position_m": [0.0, 2.65, 0.0],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
            },
            {
                "node_id": "accent",
                "operator_id": "forgecad.geometry.primitive@2",
                "inputs": [],
                "parameters": {
                    "shape": "sphere",
                    "radius_m": 0.12,
                    "longitude_segments": 16,
                    "latitude_segments": 8,
                    "position_m": [0.0, 1.7, -0.36],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
            },
        ],
        "part_outputs": [
            {
                "part_id": "shell",
                "input_node_ids": ["shell", "accent"],
                "material_zone_id": "zone-white-shell",
                "solid": True,
            },
            {
                "part_id": "joint",
                "input_node_ids": ["joint"],
                "material_zone_id": "zone-black-mechanical",
                "solid": True,
            },
            {
                "part_id": "sensor",
                "input_node_ids": ["sensor"],
                "material_zone_id": "zone-emissive-amber",
                "solid": True,
            },
        ],
    }
    return value


def wait_for_ready(path: Path, process: subprocess.Popen[str], timeout: float) -> dict[str, Any]:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if path.is_file():
            try:
                value = json.loads(path.read_text(encoding="utf-8"))
            except (OSError, json.JSONDecodeError):
                value = None
            if isinstance(value, dict) and value.get("status") == "ready":
                return value
        if process.poll() is not None:
            raise GateFailure("isolated Runtime exited before publishing its ready handoff")
        time.sleep(0.02)
    raise GateFailure("isolated Runtime did not publish its ready handoff")


def shutdown_runtime(ready: dict[str, Any], ready_path: Path, process: subprocess.Popen[str]) -> None:
    endpoint = ready.get("socket_path")
    token = ready.get("token")
    if not isinstance(endpoint, str) or not endpoint or not isinstance(token, str) or not token:
        raise GateFailure("isolated Runtime handoff lacked authenticated endpoint data")

    connection = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    connection.settimeout(2.0)
    buffered = b""

    def exchange(payload: dict[str, Any]) -> dict[str, Any]:
        nonlocal buffered
        connection.sendall(json.dumps(payload, separators=(",", ":")).encode("utf-8") + b"\n")
        while b"\n" not in buffered:
            chunk = connection.recv(65536)
            if not chunk:
                raise GateFailure("isolated Runtime closed before cleanup response")
            buffered += chunk
            if len(buffered) > MAX_RESPONSE_BYTES:
                raise GateFailure("isolated Runtime cleanup response exceeded limit")
        line, buffered = buffered.split(b"\n", 1)
        try:
            value = json.loads(line)
        except json.JSONDecodeError as error:
            raise GateFailure("isolated Runtime cleanup response was invalid JSON") from error
        if not isinstance(value, dict):
            raise GateFailure("isolated Runtime cleanup response was not an object")
        return value

    try:
        connection.connect(endpoint)
        authenticated = exchange(
            {"version": 1, "token": token, "method": "authenticate", "payload": None}
        )
        if authenticated.get("ok") is not True:
            raise GateFailure("isolated Runtime cleanup authentication failed")
        stopped = exchange(
            {"version": 1, "token": None, "method": "runtime_shutdown", "payload": None}
        )
        if stopped.get("ok") is not True:
            raise GateFailure("isolated Runtime cleanup was rejected")
    finally:
        connection.close()

    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)
        raise GateFailure("isolated Runtime did not stop after authenticated shutdown")
    if process.returncode != 0:
        raise GateFailure("isolated Runtime exited unexpectedly during cleanup")
    if ready_path.exists():
        raise GateFailure("isolated Runtime did not remove its ready handoff")


def require(condition: bool, message: str) -> None:
    if not condition:
        raise GateFailure(message)


def require_invalid_input(error: dict[str, Any], message: str) -> None:
    require(
        error.get("schema_version") == "RuntimeError@1"
        and error.get("code") == "INVALID_INPUT"
        and error.get("retryable") is False,
        message,
    )


def main() -> int:
    args = parse_args()
    if args.timeout <= 0 or not args.mcp.is_file() or not args.runtime.is_file():
        raise GateFailure("source MCP010B binaries were unavailable")
    expected_cohort = args.expected_build_cohort
    if expected_cohort is not None and re.fullmatch(r"[0-9a-f]{64}", expected_cohort) is None:
        raise GateFailure("expected build cohort was not a lowercase SHA-256")
    mcp_identity = build_identity(args.mcp) if expected_cohort is not None else None
    runtime_identity = build_identity(args.runtime) if expected_cohort is not None else None
    if expected_cohort is not None:
        require(
            mcp_identity is not None
            and mcp_identity.get("build_cohort_sha256") == expected_cohort
            and runtime_identity is not None
            and runtime_identity.get("build_cohort_sha256") == expected_cohort,
            "explicit MCP/Runtime binaries did not match the expected build cohort",
        )
    data_root = args.data_root.resolve()
    if data_root.exists():
        raise GateFailure("isolated MCP010B data root must not pre-exist")
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
                "clientInfo": {"name": "forgecad-mcp010b-raw-stdio", "version": "1"},
            },
        )
        require(
            initialized.get("result", {}).get("protocolVersion") == MCP_PROTOCOL_VERSION,
            "MCP010B source stdio initialize did not negotiate Codex compatibility",
        )
        client.notify("notifications/initialized")

        tools_response = client.request("tools/list")
        tools = tools_response.get("result", {}).get("tools")
        require(isinstance(tools, list), "MCP tools/list did not return tools")
        hash_tool = next(
            (tool for tool in tools if isinstance(tool, dict) and tool.get("name") == "geometry_program_hash"),
            None,
        )
        require(
            isinstance(hash_tool, dict)
            and hash_tool.get("annotations", {}).get("readOnlyHint") is True,
            "geometry_program_hash was not exposed as a read-only MCP tool",
        )
        catalog_tool = next(
            (tool for tool in tools if isinstance(tool, dict) and tool.get("name") == "operator_catalog_get"),
            None,
        )
        require(
            isinstance(catalog_tool, dict)
            and catalog_tool.get("annotations", {}).get("readOnlyHint") is True,
            "operator_catalog_get was not exposed as a read-only MCP tool",
        )

        skills = client.tool("skill_list")
        require(
            isinstance(skills, dict)
            and skills.get("schema_version") == "SkillListResult@1"
            and isinstance(skills.get("skills"), list)
            and len(skills["skills"]) == 11,
            "current source/package did not expose the eleven first-party Skill manifests",
        )
        primitive_skill = next(
            (
                skill
                for skill in skills["skills"]
                if isinstance(skill, dict)
                and skill.get("skill_id") == "primitive-blockout"
                and skill.get("version") == "0.2.0"
            ),
            None,
        )
        require(
            isinstance(primitive_skill, dict)
            and primitive_skill.get("execution_availability") == "active"
            and primitive_skill.get("missing_operator_ids") == [],
            "primitive-blockout@0.2.0 was not truthfully reported as active",
        )

        capabilities = client.tool("capabilities_get")
        require(isinstance(capabilities, dict), "capabilities_get did not return an object")
        capability_hash = capabilities.get("operator_catalog_sha256")
        require(
            isinstance(capability_hash, str) and len(capability_hash) == 64,
            "capabilities_get omitted the V2 operator catalog hash",
        )
        catalog_from_tool = client.tool("operator_catalog_get")
        require(isinstance(catalog_from_tool, dict), "operator_catalog_get did not return an object")
        catalog = client.resource_json("forgecad://operators/catalog")
        require(catalog.get("schema_version") == "OperatorCatalog@1", "catalog schema was incorrect")
        require(
            catalog == catalog_from_tool and catalog.get("canonical_sha256") == capability_hash,
            "capabilities/catalog tool/resource hash mismatch",
        )
        operators = catalog.get("operators")
        require(
            isinstance(operators, list)
            and len(operators) == 13
            and isinstance(operators[0], dict)
            and operators[0].get("operator_id") == "forgecad.geometry.primitive@2"
            and operators[0].get("status") == "active",
            "catalog did not expose the current MCP010D operator set with primitive@2 first",
        )

        project = client.tool(
            "project_create",
            {"name": "MCP010B raw stdio isolated V2 geometry", "policy": {"profile": "mvp"}},
        )
        require(isinstance(project, dict), "project_create did not return an object")
        project_id = project.get("project_id")
        require(isinstance(project_id, str) and project_id, "project_create omitted project_id")

        draft = v2_program_draft(project_id, capability_hash)
        hash_result = client.tool(
            "geometry_program_hash",
            {
                "schema_version": "GeometryProgramHashRequest@1",
                "geometry_program_draft": draft,
            },
        )
        require(
            isinstance(hash_result, dict)
            and hash_result.get("schema_version") == "GeometryProgramHashResult@1"
            and hash_result.get("geometry_program_schema_version") == "GeometryProgram@2"
            and hash_result.get("operator_catalog_sha256") == capability_hash
            and hash_result.get("validation_status") == "passed",
            "geometry_program_hash did not return a strict V2 hash receipt",
        )
        canonical_sha256 = hash_result.get("canonical_sha256")
        require(
            isinstance(canonical_sha256, str) and len(canonical_sha256) == 64,
            "geometry_program_hash omitted a canonical SHA-256",
        )
        program = dict(draft)
        program["canonical_sha256"] = canonical_sha256
        prepared = client.tool(
            "geometry_prepare",
            {
                "project_id": project_id,
                "request": {"typed": "geometry", "geometry_program": program},
            },
        )
        require(
            isinstance(prepared, dict) and prepared.get("schema_version") == "GeometryPrepareResult@2",
            "geometry_prepare did not return GeometryPrepareResult@2",
        )
        artifact = prepared.get("artifact")
        candidate = prepared.get("candidate")
        require(isinstance(artifact, dict) and isinstance(candidate, dict), "V2 prepare omitted artifact/candidate")
        require(
            prepared.get("operator_catalog", {}).get("canonical_sha256") == capability_hash,
            "V2 prepare catalog hash did not match capability hash",
        )
        require(
            artifact.get("schema_version") == "ArtifactReadback@2"
            and artifact.get("operator_catalog_sha256") == capability_hash
            and artifact.get("hard_gate_passed") is True
            and artifact.get("validator_status") == "passed",
            "V2 prepare did not return a passing strict ArtifactReadback@2",
        )
        integrity = artifact.get("integrity")
        require(
            isinstance(integrity, dict)
            and all(
                integrity.get(key) == 0
                for key in (
                    "invalid_index_count",
                    "non_finite_count",
                    "degenerate_triangle_count",
                    "boundary_edge_count",
                    "non_manifold_edge_count",
                    "winding_error_count",
                    "uv_non_finite_count",
                    "zero_area_uv_triangle_count",
                    "tangent_non_finite_count",
                    "tangent_orthogonality_error_count",
                    "tangent_handedness_error_count",
                    "metadata_mismatch_count",
                    "external_uri_count",
                )
            )
            and all(integrity.get(key) == 1 for key in ("part_coverage", "source_coverage", "material_zone_coverage")),
            "strict GLB readback reported a non-zero integrity failure",
        )
        part_bindings = artifact.get("part_bindings")
        shell_bindings = (
            [
                binding
                for binding in part_bindings
                if isinstance(binding, dict) and binding.get("part_id") == "shell"
            ]
            if isinstance(part_bindings, list)
            else []
        )
        require(
            [binding.get("source_node_id") for binding in shell_bindings] == ["shell", "accent"]
            and all(binding.get("material_zone_id") == "zone-white-shell" for binding in shell_bindings)
            and set(artifact.get("source_node_ids", [])) == {"shell", "accent", "joint", "sensor"},
            "V2 semantic Part sink did not preserve ordered per-source lineage and coverage",
        )
        artifact_id = artifact.get("artifact_id")
        candidate_id = candidate.get("candidate_id")
        require(
            isinstance(artifact_id, str) and len(artifact_id) == 64 and isinstance(candidate_id, str),
            "V2 prepare omitted artifact/candidate binding ids",
        )
        reread = client.tool(
            "artifact_readback_get", {"artifact_id": artifact_id, "candidate_id": candidate_id}
        )
        require(
            isinstance(reread, dict)
            and reread.get("schema_version") == "ArtifactReadback@2"
            and reread.get("artifact_id") == artifact_id
            and reread.get("candidate_id") == candidate_id
            and reread.get("canonical_sha256") == artifact.get("canonical_sha256"),
            "candidate-bound ArtifactReadback@2 did not round-trip",
        )

        binding_error = client.tool_error(
            "artifact_readback_get",
            {"artifact_id": artifact_id, "candidate_id": "candidate-not-bound"},
        )
        require(
            binding_error.get("schema_version") == "RuntimeError@1",
            "candidate/artifact mismatch did not fail through the typed MCP error path",
        )
        prefilled_hash_error = client.tool_error(
            "geometry_program_hash",
            {
                "schema_version": "GeometryProgramHashRequest@1",
                "geometry_program_draft": program,
            },
        )
        require(
            prefilled_hash_error.get("schema_version") == "RuntimeError@1",
            "geometry_program_hash accepted a draft with a prefilled canonical hash",
        )
        empty_part_sink_draft = copy.deepcopy(draft)
        empty_part_sink_draft["part_outputs"][0]["input_node_ids"] = []
        empty_part_sink_error = client.tool_error(
            "geometry_program_hash",
            {
                "schema_version": "GeometryProgramHashRequest@1",
                "geometry_program_draft": empty_part_sink_draft,
            },
        )
        require_invalid_input(
            empty_part_sink_error,
            "geometry_program_hash accepted an empty semantic Part sink",
        )
        duplicate_part_sink_draft = copy.deepcopy(draft)
        duplicate_part_sink_draft["part_outputs"][0]["input_node_ids"] = ["shell", "shell"]
        duplicate_part_sink_error = client.tool_error(
            "geometry_program_hash",
            {
                "schema_version": "GeometryProgramHashRequest@1",
                "geometry_program_draft": duplicate_part_sink_draft,
            },
        )
        require_invalid_input(
            duplicate_part_sink_error,
            "geometry_program_hash accepted duplicate semantic Part sink inputs",
        )
        reused_part_sink_draft = copy.deepcopy(draft)
        reused_part_sink_draft["part_outputs"][1]["input_node_ids"] = ["joint", "shell"]
        reused_part_sink_error = client.tool_error(
            "geometry_program_hash",
            {
                "schema_version": "GeometryProgramHashRequest@1",
                "geometry_program_draft": reused_part_sink_draft,
            },
        )
        require_invalid_input(
            reused_part_sink_error,
            "geometry_program_hash accepted a source assigned to multiple semantic Parts",
        )
        unknown_part_sink_draft = copy.deepcopy(draft)
        unknown_part_sink_draft["part_outputs"][0]["input_node_ids"] = ["missing-node"]
        unknown_part_sink_error = client.tool_error(
            "geometry_program_hash",
            {
                "schema_version": "GeometryProgramHashRequest@1",
                "geometry_program_draft": unknown_part_sink_draft,
            },
        )
        require_invalid_input(
            unknown_part_sink_error,
            "geometry_program_hash accepted a semantic Part sink input outside the node graph",
        )
        bad_draft = dict(draft)
        bad_draft["operator_catalog_sha256"] = "0" * 64
        catalog_hash_error = client.tool_error(
            "geometry_program_hash",
            {
                "schema_version": "GeometryProgramHashRequest@1",
                "geometry_program_draft": bad_draft,
            },
        )
        require(
            catalog_hash_error.get("schema_version") == "RuntimeError@1",
            "geometry_program_hash did not reject an unknown catalog",
        )
        bad_program = dict(program)
        bad_program["operator_catalog_sha256"] = "0" * 64
        catalog_error = client.tool_error(
            "geometry_prepare",
            {
                "project_id": project_id,
                "request": {"typed": "geometry", "geometry_program": bad_program},
            },
        )
        require(
            catalog_error.get("schema_version") == "RuntimeError@1",
            "catalog-mismatch GeometryProgram@2 did not fail closed",
        )
        cross_project_program = dict(program)
        cross_project_program["project_id"] = "project-not-target"
        project_binding_error = client.tool_error(
            "geometry_prepare",
            {
                "project_id": project_id,
                "request": {"typed": "geometry", "geometry_program": cross_project_program},
            },
        )
        require(
            project_binding_error.get("schema_version") == "RuntimeError@1",
            "GeometryProgram project binding did not fail closed",
        )
    finally:
        cleanup_error: BaseException | None = None
        # Close stdio before the direct authenticated cleanup request so the
        # isolated probe has no live adapter left when it shuts Runtime down.
        if client is not None:
            try:
                client.close()
            except BaseException as error:
                cleanup_error = error
        if ready is not None:
            try:
                shutdown_runtime(ready, ready_path, runtime)
            except BaseException as error:  # retain the original gate failure when present
                if cleanup_error is None:
                    cleanup_error = error
        elif runtime.poll() is None:
            runtime.kill()
            runtime.wait(timeout=5)
        if cleanup_error is not None and sys.exc_info()[0] is None:
            raise cleanup_error

    receipt = {
        "schema_version": "ForgeCADMCP010BRawStdioProbe@1",
        "task_id": "FGC-MCP010B",
        "status": "PASS",
        "protocol_version": MCP_PROTOCOL_VERSION,
        "operator_catalog_hash_match": True,
        "operator_catalog_tool_resource_match": "PASS",
        "skill_registry_count": 11,
        "primitive_blockout_skill": "active",
        "geometry_program_hash": "PASS",
        "geometry_program": "GeometryProgram@2",
        "prepare_result": "GeometryPrepareResult@2",
        "artifact_readback": "ArtifactReadback@2",
        "candidate_bound_readback": "PASS",
        "semantic_part_sink_multi_input": "PASS",
        "negative_empty_part_sink_input_ids": "PASS",
        "negative_duplicate_part_sink_input_ids": "PASS",
        "negative_reused_part_sink_input_ids": "PASS",
        "negative_unknown_part_sink_input_ids": "PASS",
        "negative_catalog_mismatch": "PASS",
        "negative_prefilled_hash": "PASS",
        "negative_project_binding": "PASS",
        "negative_candidate_artifact_mismatch": "PASS",
        "expected_build_cohort_sha256": expected_cohort,
        "mcp_build_cohort_sha256": (
            mcp_identity.get("build_cohort_sha256") if mcp_identity is not None else None
        ),
        "runtime_build_cohort_sha256": (
            runtime_identity.get("build_cohort_sha256") if runtime_identity is not None else None
        ),
        "persistent_user_data_touched": False,
        "runtime_cleanup": "PASS",
    }
    write_receipt(args.evidence, receipt)
    print(json.dumps(receipt, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except GateFailure as error:
        print(
            json.dumps(
                {
                    "schema_version": "ForgeCADMCP010BRawStdioProbe@1",
                    "task_id": "FGC-MCP010B",
                    "status": "FAIL",
                    "reason": str(error)[:256],
                    "persistent_user_data_touched": False,
                },
                sort_keys=True,
            ),
            file=sys.stderr,
        )
        raise SystemExit(1)
