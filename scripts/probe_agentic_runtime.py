#!/usr/bin/env python3
"""Run the first Agentic Design Runtime observe/plan read-model probe.

The probe creates only an isolated temporary project, then verifies the
mandatory Ponytail preflight followed by the Agentic read tools.  It never
imports a reference, compiles geometry, confirms a candidate, exports a
version, or writes the user's Runtime data.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Any

SCRIPT_ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_ROOT))

from probe_mcp010b_raw_stdio import (  # noqa: E402
    GateFailure,
    MCP_PROTOCOL_VERSION,
    McpClient,
    shutdown_runtime,
    wait_for_ready,
    v2_program_draft,
)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise GateFailure(message)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--data-root", type=Path, required=True)
    parser.add_argument("--evidence", type=Path)
    parser.add_argument("--timeout", type=float, default=60.0)
    return parser.parse_args()


def write_evidence(path: Path | None, value: dict[str, Any]) -> None:
    if path is None:
        return
    root = Path(__file__).resolve().parents[1]
    resolved = path if path.is_absolute() else root / path
    evidence_root = (root / "docs" / "evidence").resolve()
    resolved.resolve().relative_to(evidence_root)
    resolved.parent.mkdir(parents=True, exist_ok=True)
    resolved.write_text(
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
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


def main() -> int:
    args = parse_args()
    require(args.timeout > 0, "timeout must be positive")
    require(args.mcp.is_file() and args.runtime.is_file(), "MCP/Runtime binaries are unavailable")
    require(not args.data_root.exists(), "data root must not pre-exist")

    args.data_root.mkdir(mode=0o700, parents=True)
    ready_path = args.data_root / "ipc" / "ready.json"
    environment = os.environ.copy()
    for key in (
        "FORGECAD_RUNTIME_SOCKET",
        "FORGECAD_RUNTIME_TOKEN",
        "FORGECAD_RUNTIME_DATA_DIR",
        "FORGECAD_RUNTIME_COMMAND",
    ):
        environment.pop(key, None)
    environment["FORGECAD_MCP_ENABLE_MCP004_WRITES"] = "1"

    runtime: subprocess.Popen[str] | None = None
    client: McpClient | None = None
    ready: dict[str, Any] | None = None
    project_id: str | None = None
    receipt: dict[str, Any] = {
        "schema_version": "ForgeCADAgenticRuntimeProbe@1",
        "task_id": "FGC-MCP010F",
        "status": "BLOCKED",
        "persistent_user_data_touched": False,
    }
    cleanup_error: BaseException | None = None

    try:
        runtime = subprocess.Popen(
            [
                str(args.runtime),
                "serve",
                "--database",
                str(args.data_root / "runtime.sqlite"),
                "--cas-root",
                str(args.data_root / "cas"),
                "--endpoint-dir",
                str(args.data_root / "ipc"),
                "--ready-file",
                str(ready_path),
            ],
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            text=True,
        )
        ready = wait_for_ready(ready_path, runtime, args.timeout)
        socket_path = ready.get("socket_path")
        token = ready.get("token")
        require(isinstance(socket_path, str) and isinstance(token, str), "Runtime handoff is incomplete")
        environment["FORGECAD_RUNTIME_SOCKET"] = socket_path
        environment["FORGECAD_RUNTIME_TOKEN"] = token

        client = McpClient(args.mcp, environment, args.timeout)
        initialized = client.request(
            "initialize",
            {
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "forgecad-agentic-runtime-probe", "version": "1"},
            },
        )
        require(
            initialized.get("result", {}).get("protocolVersion") == MCP_PROTOCOL_VERSION,
            "MCP initialize failed",
        )
        client.notify("notifications/initialized")

        preflight = client.tool(
            "skill_get",
            {"skill_id": "ponytail-preflight", "version": "0.1.0"},
        )
        require(isinstance(preflight, dict), "ponytail preflight returned no typed result")
        skill = preflight.get("skill")
        require(
            isinstance(skill, dict)
            and skill.get("skill_id") == "ponytail-preflight"
            and skill.get("version") == "0.1.0",
            "ponytail preflight was not returned",
        )

        listed = client.request("tools/list")
        names = {
            item.get("name")
            for item in listed.get("result", {}).get("tools", [])
            if isinstance(item, dict)
        }
        required_tools = {
            "scene_observe_get",
            "design_stage_plan_get",
            "session_create_or_resume",
            "session_get",
            "checkpoint_prepare",
            "checkpoint_get",
            "checkpoint_restore_prepare",
        }
        require(required_tools.issubset(names), "Agentic observe/plan tools are not both exposed")

        project = client.tool(
            "project_create",
            {"name": "Agentic Runtime isolated observe probe", "policy": {"profile": "mvp"}},
        )
        project_id = project.get("project_id") if isinstance(project, dict) else None
        require(isinstance(project_id, str) and project_id, "project_create omitted project_id")

        observe = client.tool("scene_observe_get", {"project_id": project_id})
        plan = client.tool("design_stage_plan_get", {"project_id": project_id})
        require(isinstance(observe, dict), "scene_observe_get returned no typed projection")
        require(isinstance(plan, dict), "design_stage_plan_get returned no typed projection")
        require(
            observe.get("schema_version") == "AgenticSceneObserveResult@1"
            and isinstance(observe.get("semantic_scene_graph"), dict)
            and observe["semantic_scene_graph"].get("schema_version") == "SemanticSceneGraph@1"
            and isinstance(observe.get("model_understanding_bundle"), dict)
            and observe["model_understanding_bundle"].get("schema_version") == "ModelUnderstandingBundle@1",
            "scene_observe_get did not return the complete Agentic projection",
        )
        require(
            plan.get("schema_version") == "DesignStagePlan@1",
            "design_stage_plan_get did not return DesignStagePlan@1",
        )
        stage = plan.get("current_stage") or plan.get("stage")
        require(stage == "reference-canvas", "empty project did not remain at reference-canvas")
        allowed = plan.get("allowed_actions")
        locked = plan.get("locked_actions", plan.get("blocked_actions"))
        require(isinstance(allowed, list) and isinstance(locked, list), "stage plan omitted action locks")
        require(
            not any(action in {"appearance_prepare", "candidate_confirm", "export_confirm"} for action in allowed),
            "empty reference stage unlocked a forbidden action",
        )

        reference_bytes = bytes.fromhex(
            "89504e470d0a1a0a0000000d4948445200000001000000010804000000b51c0c020000000b4944415478da6364f80f00010501012718e3660000000049454e44ae426082"
        )
        reference_result = client.tool(
            "reference_import",
            {
                "project_id": project_id,
                "source": {
                    "kind": "inline_content",
                    "mime": "image/png",
                    "content_base64": base64.b64encode(reference_bytes).decode("ascii"),
                },
                "authorization": {
                    "user_authorized": True,
                    "declaration": "User authorized this isolated reference for Runtime verification.",
                },
            },
        )
        reference = reference_result.get("reference") if isinstance(reference_result, dict) else None
        require(isinstance(reference, dict), "reference_import omitted reference evidence")
        reference_id = reference.get("reference_id")
        reference_sha256 = reference.get("object_sha256")
        require(
            isinstance(reference_id, str) and isinstance(reference_sha256, str),
            "reference evidence binding was incomplete",
        )

        capabilities = client.tool("capabilities_get")
        catalog_sha256 = capabilities.get("operator_catalog_sha256") if isinstance(capabilities, dict) else None
        require(isinstance(catalog_sha256, str) and len(catalog_sha256) == 64, "operator catalog hash unavailable")
        draft = v2_program_draft(project_id, catalog_sha256)
        hashed = client.tool(
            "geometry_program_hash",
            {"schema_version": "GeometryProgramHashRequest@1", "geometry_program_draft": draft},
        )
        program_sha256 = hashed.get("canonical_sha256") if isinstance(hashed, dict) else None
        require(isinstance(program_sha256, str) and len(program_sha256) == 64, "geometry program hash unavailable")
        program = dict(draft)
        program["canonical_sha256"] = program_sha256
        prepared = client.tool(
            "geometry_prepare",
            {
                "project_id": project_id,
                "request": {
                    "typed": "geometry",
                    "reference_id": reference_id,
                    "geometry_program": program,
                },
            },
        )
        candidate = prepared.get("candidate") if isinstance(prepared, dict) else None
        require(isinstance(candidate, dict), "geometry_prepare omitted candidate")
        candidate_id = candidate.get("candidate_id")
        candidate_state_sha256 = candidate.get("canonical_sha256")
        artifact_sha256 = candidate.get("prepared_object_sha256") or candidate.get("manifest_hash")
        require(
            isinstance(candidate_id, str)
            and isinstance(candidate_state_sha256, str)
            and len(candidate_state_sha256) == 64
            and isinstance(artifact_sha256, str)
            and len(artifact_sha256) == 64,
            "candidate state/artifact binding was incomplete",
        )

        observed = client.tool("scene_observe_get", {"project_id": project_id, "candidate_id": candidate_id})
        require(
            isinstance(observed, dict)
            and observed.get("candidate_id") == candidate_id
            and observed.get("canonical_sha256")
            and len(observed["canonical_sha256"]) == 64,
            "candidate-bound Agentic observation was incomplete",
        )
        candidate_plan = client.tool(
            "design_stage_plan_get",
            {"project_id": project_id, "candidate_id": candidate_id},
        )
        require(
            isinstance(candidate_plan, dict)
            and candidate_plan.get("schema_version") == "DesignStagePlan@1"
            and candidate_plan.get("project_id") == project_id
            and candidate_plan.get("candidate_id") == candidate_id,
            "candidate-bound DesignStagePlan was incomplete",
        )
        # Durable session/checkpoint evidence must remain stable across a
        # rebuilt read-only projection. The candidate state hash is one of the
        # Runtime-owned observation bindings and does not depend on ephemeral
        # projection fields such as the critic envelope.
        evidence_sha256 = candidate_state_sha256
        camera_hash = canonical_hash(
            {
                "schema_version": "CameraCalibrationRef@1",
                "status": "unknown",
                "reference_id": reference_id,
                "reference_sha256": reference_sha256,
                "reason": "isolated probe has no rendered camera evidence",
            }
        )
        session_result = client.tool(
            "session_create_or_resume",
            {
                "session_id": "session-agentic-probe",
                "project_id": project_id,
                "candidate_id": candidate_id,
                "idempotency_key": "session-agentic-probe-create",
                "reference_id": reference_id,
                "design_spec_id": "design-spec-agentic-probe",
                "reference_canvas_id": "reference-canvas-agentic-probe",
                "camera_hash": camera_hash,
                "evidence_sha256": evidence_sha256,
                "approved": True,
                "approval_receipt_id": "agentic-session-probe-approval",
                "approval_summary": "Create isolated Agentic Runtime session",
                "approval_expires_at": "9999999999",
            },
        )
        session = session_result.get("session") if isinstance(session_result, dict) else None
        require(
            isinstance(session, dict)
            and session.get("schema_version") == "DesignSession@1"
            and session_result.get("durable") is True
            and session.get("project_id") == project_id
            and session.get("candidate_id") == candidate_id,
            "session_create_or_resume did not return a durable bound session",
        )

        checkpoint_result = client.tool(
            "checkpoint_prepare",
            {
                "session_id": session["session_id"],
                "project_id": project_id,
                "candidate_id": candidate_id,
                "visual_state": "fail",
                "evidence_sha256": evidence_sha256,
                "stage": session["current_stage"],
                "checkpoint_type": "stage-fail",
                "candidate_state_sha256": candidate_state_sha256,
                "artifact_sha256": artifact_sha256,
                "reference_id": reference_id,
                "reference_sha256": reference_sha256,
                "camera_hash": camera_hash,
                "idempotency_key": "agentic-probe-failed-checkpoint",
                "approved": True,
                "approval_receipt_id": "agentic-checkpoint-probe-approval",
                "approval_summary": "Persist isolated failed visual checkpoint",
                "approval_expires_at": "9999999999",
            },
        )
        checkpoint = checkpoint_result.get("checkpoint") if isinstance(checkpoint_result, dict) else None
        require(
            isinstance(checkpoint, dict)
            and checkpoint.get("schema_version") == "DesignCheckpoint@1"
            and checkpoint.get("immutable") is True
            and checkpoint.get("runtime_write") is False
            and checkpoint_result.get("durable") is True,
            "checkpoint_prepare did not return an immutable durable checkpoint",
        )
        checkpoint_id = checkpoint.get("checkpoint_id")
        checkpoint_sha256 = checkpoint.get("canonical_sha256")
        require(isinstance(checkpoint_id, str) and isinstance(checkpoint_sha256, str), "checkpoint binding was incomplete")
        checkpoint_read = client.tool(
            "checkpoint_get",
            {
                "checkpoint_id": checkpoint_id,
                "session_id": session["session_id"],
                "project_id": project_id,
                "candidate_id": candidate_id,
            },
        )
        require(
            isinstance(checkpoint_read, dict)
            and checkpoint_read.get("checkpoint", {}).get("canonical_sha256") == checkpoint_sha256,
            "checkpoint_get did not round-trip the immutable checkpoint",
        )
        restore_result = client.tool(
            "checkpoint_restore_prepare",
            {
                "checkpoint_id": checkpoint_id,
                "checkpoint_sha256": checkpoint_sha256,
                "session_id": session["session_id"],
                "project_id": project_id,
                "candidate_id": candidate_id,
                "visual_state": "fail",
                "idempotency_key": "agentic-probe-restore-intent",
                "approved": True,
                "approval_receipt_id": "agentic-restore-probe-approval",
                "approval_summary": "Prepare isolated bounded restore intent",
                "approval_expires_at": "9999999999",
            },
        )
        require(
            isinstance(restore_result, dict)
            and restore_result.get("durable") is False
            and restore_result.get("runtime_confirm_allowed") is False
            and restore_result.get("intent", {}).get("schema_version") == "RepairIntent@1",
            "restore prepare did not remain bounded and approval-gated",
        )

        session_read = client.tool(
            "session_get",
            {"session_id": session["session_id"], "project_id": project_id, "candidate_id": candidate_id},
        )
        require(
            isinstance(session_read, dict)
            and session_read.get("session", {}).get("current_checkpoint_id") == checkpoint_id,
            "session_get did not expose the checkpoint pointer",
        )

        first_ready = ready
        client.close()
        client = None
        require(first_ready is not None and runtime is not None, "Runtime restart state was incomplete")
        shutdown_runtime(first_ready, ready_path, runtime)
        runtime = None
        ready = None
        for key in ("FORGECAD_RUNTIME_SOCKET", "FORGECAD_RUNTIME_TOKEN"):
            environment.pop(key, None)
        runtime = subprocess.Popen(
            [
                str(args.runtime),
                "serve",
                "--database",
                str(args.data_root / "runtime.sqlite"),
                "--cas-root",
                str(args.data_root / "cas"),
                "--endpoint-dir",
                str(args.data_root / "ipc"),
                "--ready-file",
                str(ready_path),
            ],
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            text=True,
        )
        ready = wait_for_ready(ready_path, runtime, args.timeout)
        socket_path = ready.get("socket_path")
        token = ready.get("token")
        require(isinstance(socket_path, str) and isinstance(token, str), "Runtime restart handoff is incomplete")
        environment["FORGECAD_RUNTIME_SOCKET"] = socket_path
        environment["FORGECAD_RUNTIME_TOKEN"] = token
        client = McpClient(args.mcp, environment, args.timeout)
        initialized = client.request(
            "initialize",
            {
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "forgecad-agentic-runtime-restart-probe", "version": "1"},
            },
        )
        require(initialized.get("result", {}).get("protocolVersion") == MCP_PROTOCOL_VERSION, "MCP restart initialize failed")
        client.notify("notifications/initialized")
        restart_preflight = client.tool(
            "skill_get",
            {"skill_id": "ponytail-preflight", "version": "0.1.0"},
        )
        require(
            isinstance(restart_preflight, dict)
            and restart_preflight.get("skill", {}).get("skill_id") == "ponytail-preflight",
            "restart MCP session did not perform Ponytail preflight",
        )
        restarted_session = client.tool(
            "session_get",
            {"session_id": session["session_id"], "project_id": project_id, "candidate_id": candidate_id},
        )
        restarted_checkpoint = client.tool(
            "checkpoint_get",
            {
                "checkpoint_id": checkpoint_id,
                "session_id": session["session_id"],
                "project_id": project_id,
                "candidate_id": candidate_id,
            },
        )
        require(
            restarted_session.get("session", {}).get("canonical_sha256") == session_read.get("session", {}).get("canonical_sha256")
            and restarted_checkpoint.get("checkpoint", {}).get("canonical_sha256") == checkpoint_sha256,
            "Runtime restart changed durable session/checkpoint hashes",
        )
        receipt.update(
            {
                "status": "PASS",
                "protocol_version": MCP_PROTOCOL_VERSION,
                "preflight": {
                    "skill_id": skill.get("skill_id"),
                    "version": skill.get("version"),
                    "status": "PASS",
                },
                "tool_manifest": {
                    "scene_observe_get": "read-only",
                    "design_stage_plan_get": "read-only",
                    "candidate_stage_plan_get": "read-only candidate-bound projection",
                    "session_create_or_resume": "approved Runtime write",
                    "session_get": "read-only durable lookup",
                    "checkpoint_prepare": "approved Runtime write",
                    "checkpoint_get": "read-only durable lookup",
                    "checkpoint_restore_prepare": "approved CAS-only intent",
                },
                "project_id": project_id,
                "candidate_id": candidate_id,
                "reference_id": reference_id,
                "session_id": session["session_id"],
                "checkpoint_id": checkpoint_id,
                "session_canonical_sha256": session_read["session"]["canonical_sha256"],
                "checkpoint_canonical_sha256": checkpoint_sha256,
                "restore_intent_object_sha256": restore_result.get("intent_object_sha256"),
                "durable_records": {
                    "session": restarted_session.get("session"),
                    "checkpoint": restarted_checkpoint.get("checkpoint"),
                    "repair_intent": restore_result.get("intent"),
                },
                "projection_records": {
                    "scene_observe": observed,
                    "stage_plan": candidate_plan,
                },
                "restart_readback": True,
                "scene_observe_schema": observe.get("schema_version"),
                "stage_plan_schema": plan.get("schema_version"),
                "stage": stage,
                "allowed_action_count": len(allowed),
                "locked_action_count": len(locked),
                "empty_reference_stage_fail_closed": True,
                "runtime_write_scope": "isolated temporary project only",
            }
        )
    except BaseException as error:
        receipt["reason"] = str(error)[:2048]
        raise
    finally:
        if client is not None:
            try:
                client.close()
            except BaseException as error:
                cleanup_error = error
        if ready is not None and runtime is not None:
            try:
                shutdown_runtime(ready, ready_path, runtime)
            except BaseException as error:
                if cleanup_error is None:
                    cleanup_error = error
        elif runtime is not None and runtime.poll() is None:
            runtime.kill()
            runtime.wait(timeout=5)
        write_evidence(args.evidence, receipt)
        if cleanup_error is not None and sys.exc_info()[0] is None:
            raise cleanup_error

    print(json.dumps(receipt, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (GateFailure, OSError, ValueError) as error:
        print(
            json.dumps(
                {
                    "schema_version": "ForgeCADAgenticRuntimeProbe@1",
                    "task_id": "FGC-MCP010F",
                    "status": "BLOCKED",
                    "reason": str(error)[:2048],
                    "persistent_user_data_touched": False,
                },
                ensure_ascii=False,
                sort_keys=True,
            ),
            file=sys.stderr,
        )
        raise SystemExit(1)
