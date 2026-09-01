#!/usr/bin/env python3
"""Persist the frozen Dragonfang r8 delivery once through Runtime/Store/CAS.

This is a narrow delivery probe, not a regression suite. It prepares the
immutable KnifeSceneProgram, proves exact replay and restart readback, then
executes one final export. The retained receipt never upgrades visual, human,
engine or commercial status.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
from probe_mcp010b_raw_stdio import (  # noqa: E402
    GateFailure,
    MCP_PROTOCOL_VERSION,
    McpClient,
    shutdown_runtime,
    wait_for_ready,
)


MAX_RESPONSE_BYTES = 1_048_576
WRITER_POLICY = "forgecad-runtime-only-state-writer@1"
REQUEST_CANONICALIZATION = "canonical-json-sha256-excluding-input-sha256@1"
NODE_CANONICAL_SHA256 = r"""
import { createHash } from 'node:crypto'
let input = ''
for await (const chunk of process.stdin) input += chunk
const value = JSON.parse(input)
function canonicalJson(current) {
  if (current === null || typeof current === 'string' || typeof current === 'boolean') return JSON.stringify(current)
  if (typeof current === 'number') {
    if (!Number.isFinite(current)) throw new Error('canonical JSON rejects non-finite numbers')
    return Object.is(current, -0) ? '0' : JSON.stringify(current)
  }
  if (Array.isArray(current)) return `[${current.map(canonicalJson).join(',')}]`
  if (typeof current === 'object') {
    return `{${Object.keys(current).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(current[key])}`).join(',')}}`
  }
  throw new Error(`canonical JSON rejects ${typeof current}`)
}
process.stdout.write(createHash('sha256').update(canonicalJson(value)).digest('hex'))
"""


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")


def sha256(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def runtime_canonical_sha256(value: Any) -> str:
    """Match the fixed Worker/serde JSON number spelling used by Runtime."""
    completed = subprocess.run(
        ["node", "--input-type=module", "-e", NODE_CANONICAL_SHA256],
        input=json.dumps(value, ensure_ascii=False, allow_nan=False),
        text=True,
        encoding="utf-8",
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    require(completed.returncode == 0, f"canonical request hashing failed: {completed.stderr.strip()}")
    digest = completed.stdout.strip()
    require(len(digest) == 64, "canonical request hashing returned an invalid SHA-256")
    return digest


def require(condition: bool, message: str) -> None:
    if not condition:
        raise GateFailure(message)


def seal_request(value: dict[str, Any]) -> dict[str, Any]:
    value["input_sha256"] = ""
    value["input_sha256"] = runtime_canonical_sha256(value)
    return value


def facade(client: McpClient, name: str, operation: str, request: dict[str, Any]) -> dict[str, Any]:
    value = client.tool(name, {"operation": operation, "request": request})
    require(isinstance(value, dict), f"{name}.{operation} returned no typed result")
    return value


def start_runtime(binary: Path, data_root: Path, endpoint_name: str, environment: dict[str, str], timeout: float):
    endpoint_dir = data_root / endpoint_name
    ready_path = endpoint_dir / "ready.json"
    process = subprocess.Popen(
        [
            str(binary),
            "serve",
            "--database",
            str(data_root / "runtime.sqlite"),
            "--cas-root",
            str(data_root / "cas"),
            "--endpoint-dir",
            str(endpoint_dir),
            "--ready-file",
            str(ready_path),
        ],
        env=environment,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
    )
    ready = wait_for_ready(ready_path, process, timeout)
    return process, ready_path, ready


def start_mcp(binary: Path, base_environment: dict[str, str], ready: dict[str, Any], timeout: float) -> McpClient:
    environment = dict(base_environment)
    environment.update(
        {
            "FORGECAD_RUNTIME_SOCKET": str(ready["socket_path"]),
            "FORGECAD_RUNTIME_TOKEN": str(ready["token"]),
            "FORGECAD_MCP_ENABLE_MCP004_WRITES": "1",
        }
    )
    client = McpClient(binary, environment, timeout)
    initialized = client.request(
        "initialize",
        {
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {"name": "dragonfang-r8-delivery-live", "version": "1"},
        },
    )
    require(
        initialized.get("result", {}).get("protocolVersion") == MCP_PROTOCOL_VERSION,
        "MCP initialize failed",
    )
    client.notify("notifications/initialized")
    tools = client.request("tools/list").get("result", {}).get("tools")
    require(isinstance(tools, list) and len(tools) == 11, "default Knife facade count drifted")
    preflight = facade(
        client,
        "weapon_preflight",
        "skill_get",
        {"skill_id": "ponytail-preflight", "version": "0.1.0"},
    )
    require(preflight.get("skill", {}).get("skill_id") == "ponytail-preflight", "preflight failed")
    return client


def common_request() -> dict[str, Any]:
    return {
        "max_response_bytes": MAX_RESPONSE_BYTES,
        "runtime_write_performed": False,
        "writer_policy": WRITER_POLICY,
        "canonicalization_policy": REQUEST_CANONICALIZATION,
        "input_sha256": "",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--program", type=Path, required=True)
    parser.add_argument("--delivery-manifest", type=Path, required=True)
    parser.add_argument("--data-root", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    parser.add_argument("--expected-build-cohort", required=True)
    parser.add_argument("--timeout", type=float, default=90.0)
    args = parser.parse_args()

    require(args.mcp.is_file() and args.runtime.is_file(), "current Runtime/MCP binaries are missing")
    require(not args.data_root.exists(), "live data root already exists; refusing to replay an old run")
    args.data_root.mkdir(parents=True)
    program_bytes = args.program.read_bytes()
    program = json.loads(program_bytes)
    manifest = json.loads(args.delivery_manifest.read_bytes())
    require(program.get("canonical_sha256") == manifest.get("program", {}).get("semantic_sha256"), "program/manifest semantic hash drifted")
    require(sha256(program_bytes) == manifest.get("program", {}).get("file_sha256"), "program file hash drifted")
    expected_glb_sha = manifest.get("delivery_glb", {}).get("sha256")
    expected_glb_bytes = manifest.get("delivery_glb", {}).get("bytes")
    require(isinstance(expected_glb_sha, str) and isinstance(expected_glb_bytes, int), "delivery GLB identity is missing")

    base_environment = dict(os.environ)
    first_runtime = first_ready_path = first_ready = first_client = None
    second_runtime = second_ready_path = second_ready = second_client = None
    try:
        first_runtime, first_ready_path, first_ready = start_runtime(
            args.runtime, args.data_root, "ipc-first", base_environment, args.timeout
        )
        first_client = start_mcp(args.mcp, base_environment, first_ready, args.timeout)
        capabilities = facade(first_client, "weapon_preflight", "capabilities_get", {})
        require(capabilities.get("build_cohort_sha256") == args.expected_build_cohort, "Runtime cohort drifted")
        project = facade(
            first_client,
            "reference_intake",
            "project_create",
            {"name": "Dragonfang r8 action-ready delivery", "policy": {"profile": "knife"}},
        )
        project_id = project.get("project_id")
        require(isinstance(project_id, str) and project_id, "project_create omitted project_id")
        prepare_request = seal_request(
            {
                "schema_version": "WeaponryThreeJsKnifeDesignPrepareRequest@1",
                "operation": "weaponry_threejs_knife_design_prepare",
                "project_id": project_id,
                "program": program,
                "idempotency_key": "dragonfang-r8-design-delivery-014",
                **common_request(),
            }
        )
        prepared = facade(
            first_client, "authoring_transaction", "weaponry_threejs_knife_design_prepare", prepare_request
        )
        replayed = facade(
            first_client, "authoring_transaction", "weaponry_threejs_knife_design_prepare", prepare_request
        )
        require(prepared.get("status") == "prepared" and prepared.get("runtime_write_performed") is True, "design prepare did not persist")
        require(replayed.get("status") == "replayed" and replayed.get("runtime_write_performed") is False, "design exact replay touched state")
        require(replayed.get("program_sha256") == prepared.get("program_sha256"), "design replay hash drifted")
        design_id = prepared.get("design_id")
        program_sha = prepared.get("program_sha256")
        program_object_sha = prepared.get("program_object_sha256")
        require(program_sha == program.get("canonical_sha256"), "Runtime program semantic hash drifted")
        require(isinstance(design_id, str) and isinstance(program_object_sha, str), "Runtime design identity is incomplete")
        first_client.close()
        first_client = None
        shutdown_runtime(first_ready, first_ready_path, first_runtime)
        first_runtime = first_ready_path = first_ready = None

        second_runtime, second_ready_path, second_ready = start_runtime(
            args.runtime, args.data_root, "ipc-reopen", base_environment, args.timeout
        )
        second_client = start_mcp(args.mcp, base_environment, second_ready, args.timeout)
        get_request = seal_request(
            {
                "schema_version": "WeaponryThreeJsKnifeDesignGetRequest@1",
                "operation": "weaponry_threejs_knife_design_get",
                "project_id": project_id,
                "design_id": design_id,
                "program_sha256": program_sha,
                "program_object_sha256": program_object_sha,
                "max_response_bytes": MAX_RESPONSE_BYTES,
                "runtime_write_performed": False,
                "persistent_user_data_touched": False,
                "writer_policy": WRITER_POLICY,
                "canonicalization_policy": REQUEST_CANONICALIZATION,
                "input_sha256": "",
            }
        )
        found = facade(second_client, "authoring_transaction", "weaponry_threejs_knife_design_get", get_request)
        require(found.get("status") == "found" and found.get("program_object_sha256") == program_object_sha, "restart get drifted")
        export_request = seal_request(
            {
                "schema_version": "WeaponryThreeJsKnifeDesignExecuteRequest@1",
                "operation": "weaponry_threejs_knife_design_execute",
                "action": "export",
                "project_id": project_id,
                "design_id": design_id,
                "program_sha256": program_sha,
                "program_object_sha256": program_object_sha,
                "idempotency_key": "dragonfang-r8-export-delivery-014",
                **common_request(),
            }
        )
        exported = facade(
            second_client, "authoring_transaction", "weaponry_threejs_knife_design_execute", export_request
        )
        require(exported.get("status") == "executed" and exported.get("worker_invoked") is True, "final export did not execute")
        require(exported.get("glb_sha256") == expected_glb_sha, "Runtime GLB differs from packaged delivery")
        require(exported.get("glb_object_sha256") == expected_glb_sha, "Runtime GLB CAS object hash drifted")
        require(exported.get("glb_bytes") == expected_glb_bytes, "Runtime GLB byte length drifted")
        require(exported.get("triangle_count") == 4598 and len(exported.get("part_ids", [])) == 13, "Runtime GLB structural summary drifted")

        receipt = {
            "schema_version": "WeaponryThreeJsR8DeliveryLiveReceipt@1",
            "task_id": "WPN-THREE-R8-PACKAGE-DELIVERY-014",
            "asset_id": "Dragonfang Kukri",
            "status": "PASS_RUNTIME_STORE_CAS_ACTION_READY_DELIVERY",
            "build_cohort_sha256": args.expected_build_cohort,
            "program_sha256": program_sha,
            "program_object_sha256": program_object_sha,
            "design_id": design_id,
            "prepare_status": "PASS",
            "exact_replay_status": "PASS_NOT_TOUCHED",
            "runtime_reopen_get_status": "PASS_FOUND_EXACT_HASH",
            "execution_id": exported.get("execution_id"),
            "worker_result_sha256": exported.get("worker_result_sha256"),
            "worker_result_object_sha256": exported.get("worker_result_object_sha256"),
            "glb_sha256": exported.get("glb_sha256"),
            "glb_object_sha256": exported.get("glb_object_sha256"),
            "glb_bytes": exported.get("glb_bytes"),
            "triangle_count": exported.get("triangle_count"),
            "part_count": len(exported.get("part_ids", [])),
            "part_ids": exported.get("part_ids"),
            "action_runtime": {
                "part_pivots": 13,
                "sockets": 3,
                "collider_intents": 2,
                "destruction_groups": 2,
                "geometry_buffers_modified": False,
            },
            "retained_data_root": f"apps/desktop/src-tauri/target/{args.data_root.name}",
            "visual_status": "NOT_APPROVED",
            "human_status": "NOT_RUN",
            "engine_status": "NOT_RUN",
            "commercial_status": "NOT_RUN",
            "historical_receipts_mutated": False,
            "canonical_sha256": "",
        }
        receipt["canonical_sha256"] = sha256(canonical_bytes(receipt))
        args.receipt.parent.mkdir(parents=True, exist_ok=True)
        args.receipt.write_text(json.dumps(receipt, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print(json.dumps(receipt, ensure_ascii=False, indent=2, sort_keys=True))
        return 0
    finally:
        if first_client is not None:
            first_client.close()
        if second_client is not None:
            second_client.close()
        if first_runtime is not None and first_ready is not None and first_ready_path is not None:
            shutdown_runtime(first_ready, first_ready_path, first_runtime)
        if second_runtime is not None and second_ready is not None and second_ready_path is not None:
            shutdown_runtime(second_ready, second_ready_path, second_runtime)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except GateFailure as error:
        print(f"WPN_THREE_R8_DELIVERY_LIVE_FAILED: {error}", file=sys.stderr)
        raise SystemExit(1)
