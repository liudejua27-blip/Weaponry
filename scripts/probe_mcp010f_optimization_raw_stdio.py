#!/usr/bin/env python3
"""Run one complete CADFit-style OptimizationJob through MCP stdio.

The fixture is intentionally synthetic and isolated.  It proves the Runtime
job, not visual likeness: a real MCP session creates a candidate, binds a
reference/camera/target, starts the asynchronous multi-fidelity search, and
reads the durable result until 32 coarse + 4 mid + 2 proposal finalists plus
one unmodified final control exist.
No candidate confirmation, version, export, or persistent user data is
allowed.
"""

from __future__ import annotations

import base64
import copy
import json
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
from probe_mcp010b_raw_stdio import (  # noqa: E402
    GateFailure,
    MCP_PROTOCOL_VERSION,
    McpClient,
    build_identity,
    shutdown_runtime,
    v2_program_draft,
    wait_for_ready,
    write_receipt,
)
from probe_mcp010c_raw_stdio import canonical_hash  # noqa: E402


COARSE_EVALUATIONS = 32
MID_TOP_K = 4
FINAL_TOP_K = 2
FINAL_CONTROLS = 1
EXPECTED_EVALUATIONS = COARSE_EVALUATIONS + MID_TOP_K + FINAL_TOP_K + FINAL_CONTROLS
TOOL_MANIFEST_SUMMARY = Path(__file__).resolve().parents[1] / "docs" / "evidence" / "mcp010f" / "source-tool-manifest-summary.json"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise GateFailure(message)


def tool_value(client: McpClient, name: str, arguments: dict[str, Any]) -> dict[str, Any]:
    value = client.tool(name, arguments)
    require(isinstance(value, dict), f"{name} did not return a typed object")
    return value


def current_tool_names() -> set[str]:
    """Read the checked-in generated manifest instead of freezing a count in a probe."""
    try:
        summary = json.loads(TOOL_MANIFEST_SUMMARY.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise GateFailure(f"current tool manifest summary unavailable: {exc}") from exc
    require(summary.get("schema_version") == "ForgeCADMcpToolManifestSummary@1", "tool manifest summary schema drifted")
    read_names = summary.get("read_names")
    write_names = summary.get("write_names")
    require(isinstance(read_names, list) and isinstance(write_names, list), "tool manifest summary names were unavailable")
    names = [*read_names, *write_names]
    require(
        len(names) == summary.get("total_count")
        and len(read_names) == summary.get("read_count")
        and len(write_names) == summary.get("write_count")
        and len(names) == len(set(names)),
        "tool manifest summary counts or uniqueness drifted",
    )
    require(all(isinstance(name, str) and name for name in names), "tool manifest summary contains an invalid tool name")
    return set(names)


def main() -> int:
    import argparse

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--data-root", type=Path, required=True)
    parser.add_argument("--evidence", type=Path)
    parser.add_argument("--expected-build-cohort")
    parser.add_argument("--timeout", type=float, default=180.0)
    args = parser.parse_args()

    require(args.mcp.is_file() and args.runtime.is_file(), "optimization probe binaries were unavailable")
    mcp_identity = build_identity(args.mcp)
    runtime_identity = build_identity(args.runtime)
    if args.expected_build_cohort:
        require(len(args.expected_build_cohort) == 64, "invalid expected build cohort")
        require(mcp_identity.get("build_cohort_sha256") == args.expected_build_cohort, "MCP cohort mismatch")
        require(runtime_identity.get("build_cohort_sha256") == args.expected_build_cohort, "Runtime cohort mismatch")

    data_root = args.data_root.absolute()
    require(not data_root.exists(), "isolated optimization data root must not pre-exist")
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
    receipt: dict[str, Any] | None = None
    try:
        ready = wait_for_ready(ready_path, runtime, args.timeout)
        socket_path = ready.get("socket_path")
        token = ready.get("token")
        require(isinstance(socket_path, str) and isinstance(token, str), "Runtime ready handoff lacked authenticated endpoint")
        environment = os.environ.copy()
        environment["FORGECAD_RUNTIME_SOCKET"] = socket_path
        environment["FORGECAD_RUNTIME_TOKEN"] = token
        environment["FORGECAD_MCP_ENABLE_MCP004_WRITES"] = "1"
        client = McpClient(args.mcp, environment, max(args.timeout, 30.0))
        initialized = client.request(
            "initialize",
            {
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "mcp010f-optimization-raw-stdio", "version": "1"},
            },
        )
        require(initialized.get("result", {}).get("protocolVersion") == MCP_PROTOCOL_VERSION, "MCP initialize failed")
        client.notify("notifications/initialized")

        listed = client.request("tools/list")
        tools = listed.get("result", {}).get("tools")
        require(isinstance(tools, list), "current MCP tool manifest was unavailable")
        listed_names = {tool.get("name") for tool in tools if isinstance(tool, dict)}
        expected_names = current_tool_names()
        require(listed_names == expected_names, "MCP tool names drifted from the generated current manifest")
        manifest_summary = json.loads(TOOL_MANIFEST_SUMMARY.read_text(encoding="utf-8"))
        preflight = tool_value(client, "skill_get", {"skill_id": "ponytail-preflight", "version": "0.1.0"})
        require(preflight.get("skill", {}).get("skill_id") == "ponytail-preflight", "Ponytail preflight did not bind")

        project = tool_value(client, "project_create", {"name": "MCP010F isolated optimization", "policy": {"profile": "mvp"}})
        project_id = project.get("project_id")
        require(isinstance(project_id, str), "project_create omitted project_id")
        reference = tool_value(
            client,
            "reference_import",
            {
                "project_id": project_id,
                "source": {
                    "kind": "inline_content",
                    "mime": "image/png",
                    "content_base64": base64.b64encode(bytes.fromhex(
                        "89504e470d0a1a0a0000000d4948445200000001000000010804000000b51c0c020000000b4944415478da6364f80f00010501012718e3660000000049454e44ae426082"
                    )).decode("ascii"),
                },
                "authorization": {"user_authorized": True, "declaration": "isolated CADFit optimization fixture"},
            },
        )
        reference_record = reference.get("reference") or {}
        reference_id = reference_record.get("reference_id")
        reference_sha256 = reference_record.get("object_sha256")
        require(isinstance(reference_id, str) and isinstance(reference_sha256, str), "reference evidence binding was incomplete")

        catalog = tool_value(client, "operator_catalog_get", {})
        catalog_sha256 = catalog.get("canonical_sha256")
        require(isinstance(catalog_sha256, str) and len(catalog_sha256) == 64, "operator catalog hash unavailable")
        draft = v2_program_draft(project_id, catalog_sha256)
        # The Runtime-owned Rig scale sink is materialized after the complete
        # Part output graph. This fixture has two shell inputs, so reserve two
        # bounded transform nodes in the derived GeometryProgram budget rather
        # than accidentally testing a source-only node budget.
        draft["budgets"]["max_nodes"] = len(draft["nodes"]) + 2
        hashed = tool_value(client, "geometry_program_hash", {"schema_version": "GeometryProgramHashRequest@1", "geometry_program_draft": draft})
        program_sha256 = hashed.get("canonical_sha256")
        require(isinstance(program_sha256, str) and len(program_sha256) == 64, "geometry program hash unavailable")
        program = copy.deepcopy(draft)
        program["canonical_sha256"] = program_sha256
        prepared = tool_value(
            client,
            "geometry_prepare",
            {"project_id": project_id, "request": {"typed": "geometry", "reference_id": reference_id, "geometry_program": program}},
        )
        candidate = prepared.get("candidate") or {}
        candidate_id = candidate.get("candidate_id")
        require(isinstance(candidate_id, str), "geometry_prepare omitted candidate_id")
        artifact = prepared.get("artifact") or {}
        persisted_program_sha256 = artifact.get("program_sha256")
        require(
            isinstance(persisted_program_sha256, str) and len(persisted_program_sha256) == 64,
            "geometry_prepare omitted the persisted program hash",
        )

        view_spec = {
            "schema_version": "ReferenceViewSpec@1",
            "reference_id": reference_id,
            "reference_sha256": reference_sha256,
            "view_id": "cadfit-isolated-view",
            "source_view": "three-quarter",
            "image": {"width": 1, "height": 1, "rotation_degrees": 0.0, "crop": {"x": 0.0, "y": 0.0, "width": 1.0, "height": 1.0}},
            "landmarks": [],
            "regions": [],
            "canonical_sha256": "",
        }
        view_spec["canonical_sha256"] = canonical_hash(view_spec)
        visual = tool_value(
            client,
            "reference_compare_prepare",
            {"project_id": project_id, "candidate_id": candidate_id, "reference_id": reference_id, "view_spec": view_spec},
        )
        camera = visual.get("camera") or {}
        camera_hash = camera.get("camera_hash")
        require(isinstance(camera_hash, str) and len(camera_hash) == 64, "comparison camera was not hash-bound")
        target = tool_value(
            client,
            "reference_mask_prepare",
            {
                "project_id": project_id,
                "reference_id": reference_id,
                "contour_points": [[0.2, 0.15], [0.8, 0.15], [0.8, 0.85], [0.2, 0.85]],
                "landmarks": [],
                "parts": [],
            },
        )
        target_sha256 = target.get("target_sha256")
        require(isinstance(target_sha256, str) and len(target_sha256) == 64, "silhouette target was not hash-bound")

        rig = {
            "schema_version": "SilhouetteRig@1",
            "rig_id": "mcp010f-optimization-rig",
            "candidate_id": candidate_id,
            "parameters": [
                {"parameter_id": "shell-width", "part_id": "shell", "semantic": "width", "value": 1.2, "min": 0.8, "max": 1.6, "step": 0.05, "unit": "meter"},
                {"parameter_id": "shell-height", "part_id": "shell", "semantic": "height", "value": 1.6, "min": 1.1, "max": 2.1, "step": 0.05, "unit": "meter"},
                {"parameter_id": "shell-depth", "part_id": "shell", "semantic": "depth", "value": 0.55, "min": 0.35, "max": 0.9, "step": 0.05, "unit": "meter"},
                {"parameter_id": "shell-scale", "part_id": "shell", "semantic": "scale", "value": 1.0, "min": 0.8, "max": 1.2, "step": 0.05, "unit": "ratio"},
            ],
            "canonical_sha256": "",
        }
        rig_draft = copy.deepcopy(rig)
        rig_draft.pop("canonical_sha256", None)
        rig_hash_result = tool_value(
            client,
            "silhouette_rig_hash",
            {
                "schema_version": "SilhouetteRigHashRequest@1",
                "project_id": project_id,
                "candidate_id": candidate_id,
                "rig_draft": rig_draft,
            },
        )
        rig_hash = rig_hash_result.get("canonical_sha256")
        require(isinstance(rig_hash, str) and len(rig_hash) == 64, "Runtime did not return the Rig hash")
        rig["canonical_sha256"] = rig_hash
        intent = {
            "schema_version": "OptimizationIntent@1",
            "intent_id": "mcp010f-optimization-intent",
            "job_id": "mcp010f-optimization-job",
            "project_id": project_id,
            "candidate_id": candidate_id,
            "reference_id": reference_id,
            "reference_sha256": reference_sha256,
            "program_sha256": persisted_program_sha256,
            "target_sha256": target_sha256,
            "camera": camera,
            "camera_hash": camera_hash,
            "part_id": "shell",
            "stage": "primary-form",
            "rig": rig,
            "fidelity": {"coarse_resolution": 128, "mid_resolution": 256, "final_resolution": 512, "coarse_evaluations": COARSE_EVALUATIONS, "mid_top_k": MID_TOP_K, "final_top_k": FINAL_TOP_K},
            "budget": {"max_evaluations": 42, "max_runtime_ms": 120000, "max_output_triangles": 250000, "max_worker_memory_bytes": 536870912},
            "objective": {"silhouette_iou": 0.35, "boundary_f1_4px": 0.25, "landmark_coverage": 0.1, "landmark_nme": 0.1, "part_region": 0.1, "program_complexity": 0.1},
            "canonical_sha256": "",
        }
        intent["canonical_sha256"] = canonical_hash(intent)
        approval = {
            "approved": True,
            "approval_receipt_id": "mcp010f-optimization-approval",
            "approval_summary": "Run isolated CADFit search and return a proposal only",
            "approval_expires_at": "9999999999",
            "approval_session_id": "mcp010f-optimization-session",
            "idempotency_key": "mcp010f-optimization-idempotency",
        }
        initial = tool_value(client, "optimization_job_prepare", {"project_id": project_id, "candidate_id": candidate_id, "intent": intent, **approval})
        require(initial.get("schema_version") == "OptimizationJobResult@1", "optimization prepare schema mismatch")

        latest = initial
        deadline = time.monotonic() + args.timeout
        while time.monotonic() < deadline:
            job = latest.get("job") or {}
            if job.get("status") in {"succeeded", "failed", "cancelled"}:
                break
            time.sleep(0.25)
            latest = tool_value(client, "optimization_job_get", {"project_id": project_id, "candidate_id": candidate_id, "job_id": intent["job_id"]})
        job = latest.get("job") or {}
        result = latest.get("result") or {}
        require(job.get("status") == "succeeded", f"optimization job did not succeed: {job.get('status')}")
        require(result.get("status") == "succeeded", "optimization result was not succeeded")
        require(result.get("evaluations_count") == EXPECTED_EVALUATIONS, "optimization did not complete the expected multi-fidelity evaluations")
        require(result.get("fidelity_counts") == {"coarse": COARSE_EVALUATIONS, "mid": MID_TOP_K, "final": FINAL_TOP_K + FINAL_CONTROLS}, "optimization fidelity counts drifted")
        evaluation_hashes = result.get("evaluation_object_sha256s")
        require(isinstance(evaluation_hashes, list) and len(evaluation_hashes) == EXPECTED_EVALUATIONS and all(isinstance(value, str) and len(value) == 64 for value in evaluation_hashes), "evaluation CAS checkpoint chain was incomplete")
        require(result.get("next_stage") == "done" and result.get("checkpoint_sequence") == EXPECTED_EVALUATIONS, "optimization checkpoint did not reach done")
        require(result.get("best_evaluation_fidelity") == "final", "optimization best-so-far escaped the highest completed fidelity")
        require(result.get("proposal_status") in {"proposed", "blocked-no-improvement"}, "optimization proposal boundary was invalid")
        require(result.get("strict_improvement") is False or result.get("proposal_status") == "proposed", "strict improvement was not tied to proposal status")
        if result.get("proposal_status") == "proposed":
            require(isinstance(result.get("proposal_program_object_sha256"), str) and len(result["proposal_program_object_sha256"]) == 64, "proposal program object hash was not separated from best-so-far")
            require(isinstance(result.get("proposal_artifact_sha256"), str) and len(result["proposal_artifact_sha256"]) == 64, "proposal artifact hash was not separated from best-so-far")

        receipt = {
            "schema_version": "ForgeCADMCP010FOptimizationRawStdioProbe@1",
            "task_id": "FGC-MCP010F",
            "status": "PASS",
            "mcp_build_cohort_sha256": mcp_identity.get("build_cohort_sha256"),
            "runtime_build_cohort_sha256": runtime_identity.get("build_cohort_sha256"),
            "expected_build_cohort_sha256": args.expected_build_cohort,
            "tool_manifest": {
                "schema_version": manifest_summary.get("schema_version"),
                "canonical_sha256": manifest_summary.get("canonical_sha256"),
                "read_count": manifest_summary.get("read_count"),
                "write_count": manifest_summary.get("write_count"),
                "total_count": manifest_summary.get("total_count"),
                "listed_count": len(listed_names),
                "exact_name_match": listed_names == expected_names,
            },
            "search_strategy": result.get("search_strategy"),
            "job_kind": "design_optimization",
            "job_status": job.get("status"),
            "result_status": result.get("status"),
            "evaluations_count": result.get("evaluations_count"),
            "fidelity_counts": result.get("fidelity_counts"),
            "checkpoint_sequence": result.get("checkpoint_sequence"),
            "next_stage": result.get("next_stage"),
            "proposal_status": result.get("proposal_status"),
            "strict_improvement": result.get("strict_improvement"),
            "best_evaluation_id": result.get("best_evaluation_id"),
            "best_evaluation_fidelity": result.get("best_evaluation_fidelity"),
            "best_program_object_sha256": result.get("best_program_object_sha256"),
            "best_artifact_sha256": result.get("best_artifact_sha256"),
            "proposal_program_object_sha256": result.get("proposal_program_object_sha256"),
            "proposal_artifact_sha256": result.get("proposal_artifact_sha256"),
            "evaluation_object_sha256s": evaluation_hashes,
            "project_id": project_id,
            "candidate_id": candidate_id,
            "reference_sha256": reference_sha256,
            "camera_hash": camera_hash,
            "target_sha256": target_sha256,
            "persistent_user_data_touched": False,
            "candidate_confirmed": False,
            "version_count": 0,
        }
        write_receipt(args.evidence, receipt)
        print(json.dumps(receipt, sort_keys=True))
        return 0
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
        if runtime.stderr is not None:
            stderr = runtime.stderr.read(8192)
            if stderr:
                print(stderr, file=sys.stderr, end="" if stderr.endswith("\n") else "\n")
        if cleanup_error is not None and sys.exc_info()[0] is None:
            raise cleanup_error


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except GateFailure as error:
        print(json.dumps({"schema_version": "ForgeCADMCP010FOptimizationRawStdioProbe@1", "task_id": "FGC-MCP010F", "status": "FAIL", "reason": str(error)}, sort_keys=True))
        raise SystemExit(1)
