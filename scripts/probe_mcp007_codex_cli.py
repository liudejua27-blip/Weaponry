#!/usr/bin/env python3
"""Run the real Codex CLI through the MCP007 geometry vertical slice.

The probe intentionally uses one small, deterministic typed hard-surface robot
program.  It exercises the real reference-byte admission, geometry compilation,
CAS GLB readback and MCP approval opt-in without recording the source path,
prompt or image bytes in the receipt.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any


SEQUENCE = ("project_create", "reference_import", "geometry_prepare", "artifact_readback_get")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--reference", required=True, help="user-authorized PNG/JPEG path")
    parser.add_argument("--runtime-command", default="forgecad-runtime")
    parser.add_argument("--mcp-command", default="forgecad-mcp")
    parser.add_argument("--codex-command", default="codex")
    parser.add_argument("--timeout", type=float, default=300.0)
    parser.add_argument("--debug", action="store_true", help="write local Codex diagnostics to stderr; never part of evidence")
    return parser.parse_args()


def event_items(stdout: str) -> list[dict[str, Any]]:
    items: list[dict[str, Any]] = []
    for line in stdout.splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        item = event.get("item")
        if isinstance(item, dict):
            items.append(item)
    return items


def mcp_calls(items: list[dict[str, Any]]) -> list[dict[str, Any]]:
    grouped: dict[str, dict[str, Any]] = {}
    order: list[str] = []
    for item in items:
        if item.get("type") != "mcp_tool_call":
            continue
        key = str(item.get("id") or f"call-{len(order)}")
        if key not in grouped:
            order.append(key)
            grouped[key] = {
                "server": item.get("server"),
                "tool": item.get("tool"),
                "status": item.get("status"),
            }
        elif item.get("status"):
            grouped[key]["status"] = item["status"]
        result = item.get("result")
        arguments = item.get("arguments")
        if isinstance(arguments, dict) and item.get("tool") == "silhouette_fit_prepare":
            grouped[key]["fit_argument_keys"] = sorted(str(field) for field in arguments.keys())
            for field in ("canonical_sha256", "target_sha256", "project_id", "candidate_id", "schema_version"):
                value = arguments.get(field)
                if isinstance(value, (str, int, float, bool)):
                    grouped[key][f"fit_argument_{field}"] = value
            camera = arguments.get("base_camera")
            rig = arguments.get("rig")
            if isinstance(camera, dict):
                grouped[key]["fit_argument_camera_hash"] = camera.get("camera_hash")
                grouped[key]["fit_argument_camera_canonical_sha256"] = camera.get("canonical_sha256")
            if isinstance(rig, dict):
                grouped[key]["fit_argument_rig_canonical_sha256"] = rig.get("canonical_sha256")
            optimizer = arguments.get("optimizer")
            grouped[key]["fit_argument_optimizer_type"] = type(optimizer).__name__
            if isinstance(optimizer, dict):
                grouped[key]["fit_argument_optimizer"] = {
                    field: optimizer.get(field)
                    for field in ("algorithm", "max_iterations", "max_evaluations", "step_fraction")
                    if field in optimizer
                }
        if isinstance(arguments, dict) and item.get("tool") == "render_pass_get":
            # Keep only the bounded render identity in receipts.  Recording
            # the exact hash/pass pair makes a failed AOV transport
            # distinguishable from a Runtime render-set rejection without
            # retaining prompts, image bytes, or arbitrary arguments.
            for field in ("render_set_hash", "pass", "candidate_id"):
                value = arguments.get(field)
                if isinstance(value, (str, int, float, bool)):
                    grouped[key][f"argument_{field}"] = value
        if isinstance(result, dict):
            # Codex JSONL has emitted both the MCP SDK snake_case spelling
            # and the wire/API camelCase spelling over time.  Treat them as
            # the same typed result; otherwise a successful tool call is
            # incorrectly recorded as completed-but-empty and later stages
            # cannot resume from its hashes.
            structured = result.get("structured_content")
            if not isinstance(structured, dict):
                structured = result.get("structuredContent")
            if isinstance(structured, dict):
                if isinstance(structured.get("code"), str):
                    grouped[key]["code"] = structured["code"]
                for field in ("message", "next_action", "retryable"):
                    value = structured.get(field)
                    if isinstance(value, (str, bool)):
                        grouped[key][field] = value
                evidence_ids = structured.get("evidence_ids")
                if isinstance(evidence_ids, list) and all(isinstance(value, str) for value in evidence_ids):
                    grouped[key]["evidence_ids"] = evidence_ids[:8]
                # Keep a compact diagnostic projection for non-artifact
                # typed responses.  Real Codex event envelopes may omit a
                # large structured payload; recording the keys and the
                # selected camera/hash fields lets a receipt distinguish
                # client truncation from a Runtime contract failure without
                # storing prompts, image bytes or full candidate data.
                grouped[key]["structured_keys"] = sorted(str(field) for field in structured.keys())
                for field in (
                    "schema_version",
                    "canonical_sha256",
                    "target_sha256",
                    "project_id",
                    "candidate_id",
                    "camera_hash",
                    "selected_camera_hash",
                ):
                    if field in structured and isinstance(structured[field], (str, int, float, bool)):
                        grouped[key][field] = structured[field]
                for field in ("selected_camera", "camera"):
                    if isinstance(structured.get(field), dict):
                        grouped[key][field] = {
                            key_name: structured[field].get(key_name)
                            for key_name in ("camera_hash", "canonical_sha256", "yaw", "pitch", "roll", "fov_degrees", "distance_m")
                            if key_name in structured[field]
                        }
                artifact = structured.get("artifact")
                if isinstance(artifact, dict):
                    grouped[key]["artifact"] = {
                        field: artifact.get(field)
                        for field in ("artifact_id", "candidate_id", "part_ids", "triangle_count", "validator_status", "mime", "size_bytes")
                        if field in artifact
                    }
    return [grouped[key] for key in order]


def unrelated_side_effects(items: list[dict[str, Any]]) -> bool:
    normalized = {str(item.get("type", "")).replace("_", "").lower() for item in items}
    return bool(normalized & {"commandexecution", "filechange", "mcpresourcewrite"})


def canonical_json(value: Any) -> bytes:
    # All probe numbers are simple integers/floats and all keys are ASCII. This
    # matches forgecad-core's sorted-key, compact JSON canonicalization.
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")


def sha256_json_without_hash(value: dict[str, Any]) -> str:
    payload = dict(value)
    payload.pop("canonical_sha256", None)
    return hashlib.sha256(canonical_json(payload)).hexdigest()


def geometry_program(project_id: str = "codex-mcp007-project") -> dict[str, Any]:
    nodes = [
        ("head-shell", "sphere", [0.9, 0.95, 0.82], [0, 3.25, 0], "zone-white-shell"),
        ("visor", "box", [0.72, 0.28, 0.12], [0, 3.22, -0.42], "zone-black-mechanical"),
        ("neck", "cylinder", [0.42, 0.55, 0.42], [0, 2.72, 0], "zone-black-mechanical"),
        ("chest-shell", "box", [1.55, 1.25, 0.62], [0, 2.05, 0], "zone-white-shell"),
        ("chest-core", "cylinder", [0.52, 0.78, 0.52], [0, 1.98, -0.22], "zone-black-mechanical"),
        ("pelvis-shell", "box", [1.12, 0.72, 0.62], [0, 1.22, 0], "zone-white-shell"),
        ("arm-left", "box", [0.38, 1.42, 0.42], [-1.05, 1.95, 0], "zone-white-shell"),
        ("arm-right", "box", [0.38, 1.42, 0.42], [1.05, 1.95, 0], "zone-white-shell"),
        ("hand-left", "box", [0.34, 0.5, 0.34], [-1.05, 1.05, 0], "zone-black-mechanical"),
        ("hand-right", "box", [0.34, 0.5, 0.34], [1.05, 1.05, 0], "zone-black-mechanical"),
        ("thigh-left", "box", [0.52, 1.35, 0.58], [-0.48, 0.32, 0], "zone-white-shell"),
        ("thigh-right", "box", [0.52, 1.35, 0.58], [0.48, 0.32, 0], "zone-white-shell"),
        ("shin-left", "box", [0.42, 1.35, 0.5], [-0.48, -0.92, 0], "zone-black-mechanical"),
        ("shin-right", "box", [0.42, 1.35, 0.5], [0.48, -0.92, 0], "zone-black-mechanical"),
    ]
    nodes_value = [
        {
            "node_id": part_id,
            "operator_id": "forgecad.geometry.primitive@1",
            "part_id": part_id,
            "parameters": {
                "shape": shape,
                "size": size,
                "position": position,
                "material_zone_id": material_zone,
                "segments": 16,
            },
        }
        for part_id, shape, size, position, material_zone in nodes
    ]
    program: dict[str, Any] = {
        "schema_version": "GeometryProgram@1",
        "project_id": project_id,
        "representation_plan_sha256": "d" * 64,
        "nodes": nodes_value,
        "budgets": {"max_nodes": 32, "max_triangles": 50000, "max_runtime_ms": 3000},
    }
    program["canonical_sha256"] = sha256_json_without_hash(program)
    return program


def config_override(command: str) -> str:
    return (
        "mcp_servers.forgecad={"
        f"command={json.dumps(command)},"
        'args=["serve","--stdio"],'
        'env_vars=["FORGECAD_RUNTIME_SOCKET","FORGECAD_RUNTIME_TOKEN","FORGECAD_MCP_ENABLE_MCP004_WRITES","FORGECAD_ATTACHMENT_ROOTS"],'
        "enabled=true,required=true,startup_timeout_sec=20,tool_timeout_sec=120,"
        'default_tools_approval_mode="writes"}'
    )


def prompt(reference_path: str) -> str:
    program = json.dumps(geometry_program(), ensure_ascii=False, separators=(",", ":"))
    return f"""Use only the forgecad MCP server. Do not use shell, browser, filesystem tools, other MCP servers, images, or arbitrary code.

Call exactly these four tools in this order and stop: project_create, reference_import, geometry_prepare, artifact_readback_get.
1) project_create with name=\"MCP007 robot geometry acceptance\" and policy={{\"profile\":\"mvp\"}}. Save project_id.
2) reference_import once with project_id=<saved project_id>, source={{\"kind\":\"codex_local_file\",\"path\":{json.dumps(reference_path, ensure_ascii=False)}}}, authorization={{\"user_authorized\":true,\"declaration\":\"The user supplied and authorized this reference for the local ForgeCAD MVP.\"}}. Save reference_id.
3) geometry_prepare once with project_id=<saved project_id> and request={{\"typed\":\"geometry\",\"reference_id\":<saved reference_id>,\"geometry_program\":{program}}}. Save the returned artifact_id and candidate_id.
4) artifact_readback_get once with the artifact_id and candidate_id returned by geometry_prepare.
Do not call capabilities_get or any other tool. Report only whether all four calls completed, the number of returned part_ids, triangle_count, validator_status and artifact_id. Do not claim rendering, similarity, PBR or a finished high-quality model.
"""


def blocked(reason: str, source_sha256: str, size: int) -> dict[str, Any]:
    return {
        "status": "BLOCKED",
        "reason": reason,
        "scope": "MCP007 real Codex geometry vertical slice",
        "source_sha256": source_sha256,
        "source_size_bytes": size,
        "reference_path_recorded": False,
        "image_bytes_recorded": False,
        "geometry": "NOT_RUN",
        "render": "NOT_RUN",
    }


def structured_result(items: list[dict[str, Any]], tool_name: str) -> dict[str, Any] | None:
    """Return the last structured result for one completed MCP call.

    Codex JSONL may retain a structured payload on a failed/retried call while
    a later lifecycle event carries the final status.  Do not let that stale
    payload hide the last successful typed result from a bounded stage.
    """
    for item in reversed(items):
        if item.get("type") != "mcp_tool_call" or item.get("tool") != tool_name:
            continue
        status = item.get("status")
        if status not in (None, "completed"):
            continue
        result = item.get("result")
        if not isinstance(result, dict):
            continue
        structured = result.get("structured_content")
        if not isinstance(structured, dict):
            structured = result.get("structuredContent")
        if isinstance(structured, dict):
            return structured
    return None


def run_turn(options: argparse.Namespace, environment: dict[str, str], prompt_text: str, workspace_root: str, image_path: str | None = None) -> subprocess.CompletedProcess[str]:
    """Run one short Codex turn against the same authenticated Runtime."""
    with tempfile.TemporaryDirectory(dir="/tmp", prefix="fc7-codex-") as workspace:
        command = [
                options.codex_command,
                "exec",
                "--ephemeral",
                "--ignore-user-config",
                "--json",
                "--color",
                "never",
                "--approve-for-me",
                "--skip-git-repo-check",
                "-C",
                workspace,
                "-c",
                config_override(options.mcp_command),
                "-c",
                'mcp_servers.cloudflare-api={url="http://127.0.0.1:1",enabled=false,required=false}',
        ]
        if image_path:
            command.extend(["--image", image_path])
            return subprocess.run(
                command,
                input=prompt_text + "\n",
                env=environment,
                text=True,
                capture_output=True,
                timeout=options.timeout,
                check=False,
            )
        command.append(prompt_text)
        return subprocess.run(
            command,
            env=environment,
            text=True,
            capture_output=True,
            timeout=options.timeout,
            check=False,
        )


def main() -> int:
    options = parse_args()
    source = Path(options.reference)
    if not source.is_file() or source.is_symlink():
        print(json.dumps(blocked("reference is not a regular file", "", 0)))
        return 3
    source_bytes = source.read_bytes()
    source_sha256 = hashlib.sha256(source_bytes).hexdigest()
    if not options.execute:
        print(json.dumps({**blocked("Pass --execute to run the local Runtime and Codex CLI.", source_sha256, len(source_bytes)), "status": "NOT_RUN"}))
        return 2

    environment = os.environ.copy()
    environment.pop("CODEX_MCP_PROTOCOL_VERSION", None)
    environment["FORGECAD_MCP_ENABLE_MCP004_WRITES"] = "1"
    environment["FORGECAD_ATTACHMENT_ROOTS"] = str(source.parent)
    with tempfile.TemporaryDirectory(dir="/tmp", prefix="fc7-") as temporary:
        root = Path(temporary)
        ready = root / "ready.json"
        runtime = subprocess.Popen(
            [options.runtime_command, "serve", "--database", str(root / "runtime.sqlite"), "--cas-root", str(root / "cas"), "--endpoint-dir", str(root / "ipc"), "--ready-file", str(ready)],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            text=True,
            env=environment,
        )
        try:
            deadline = time.monotonic() + 30
            while not ready.exists() and time.monotonic() < deadline:
                if runtime.poll() is not None:
                    break
                time.sleep(0.05)
            if not ready.exists():
                print(json.dumps(blocked("Runtime did not publish a ready handoff.", source_sha256, len(source_bytes))))
                return 3
            handoff = json.loads(ready.read_text(encoding="utf-8"))
            environment.update({"FORGECAD_RUNTIME_SOCKET": str(handoff["socket_path"]), "FORGECAD_RUNTIME_TOKEN": str(handoff["token"])})

            # Keep each turn small enough that Codex receives a stable MCP
            # tool catalog before it sees the large typed GeometryProgram.
            first = run_turn(
                options,
                environment,
                f"""Use only ForgeCAD. Call project_create exactly once with name=\"Codex MCP007 robot geometry\" and policy={{\"profile\":\"mvp\"}}. Save its project_id. Then call reference_import exactly once with that project_id, source={{\"kind\":\"codex_local_file\",\"path\":{json.dumps(str(source), ensure_ascii=False)}}}, authorization={{\"user_authorized\":true,\"declaration\":\"The user supplied and authorized this reference for the local ForgeCAD MVP.\"}}. Do not call any other tool. Return the project_id and reference_id, then stop.""",
                str(root),
                str(source),
            )
            first_items = event_items(first.stdout)
            project = structured_result(first_items, "project_create")
            project_id = project.get("project_id") if project else None
            if not isinstance(project_id, str) or not project_id:
                all_items = first_items
                calls = mcp_calls(all_items)
                receipt = {
                    "status": "BLOCKED",
                    "mode": "codex-cli-mcp007-geometry-multiturn",
                    "codex_exit_code": first.returncode,
                    "mcp_tool_calls": calls,
                    "expected_sequence": list(SEQUENCE),
                    "source_sha256": source_sha256,
                    "source_size_bytes": len(source_bytes),
                    "reference_path_recorded": False,
                    "image_bytes_recorded": False,
                    "geometry": "NOT_RUN",
                    "render": "NOT_RUN",
                    "reason": "Codex did not return a project_id from the setup turn.",
                }
                print(json.dumps(receipt, ensure_ascii=False, separators=(",", ":")))
                return 3

            program = geometry_program(project_id)
            program_json = json.dumps(program, ensure_ascii=False, separators=(",", ":"))
            second = run_turn(
                options,
                environment,
                f"""Use only ForgeCAD. Call geometry_prepare exactly once with project_id={json.dumps(project_id)} and request={{"typed":"geometry","geometry_program":{program_json}}}. The returned structured_content is {{"artifact":{{...}},"candidate":{{...}}}}; save artifact_id and candidate_id from the returned artifact object. Then immediately call artifact_readback_get exactly once with exact JSON arguments {{"artifact_id":<saved artifact_id>,"candidate_id":<saved candidate_id>}}. Do not call it with an empty object, omit either key, or invent IDs. Do not call any other tool. Stop only after both structured results.""",
                str(root),
            )
            second_items = event_items(second.stdout)
            geometry = structured_result(second_items, "geometry_prepare")
            artifact = geometry.get("artifact") if geometry else None
            artifact_id = artifact.get("artifact_id") if isinstance(artifact, dict) else None
            candidate_id = artifact.get("candidate_id") if isinstance(artifact, dict) else None
            if not isinstance(artifact_id, str) or not isinstance(candidate_id, str):
                all_items = first_items + second_items
                calls = mcp_calls(first_items) + mcp_calls(second_items)
                receipt = {
                    "status": "BLOCKED",
                    "mode": "codex-cli-mcp007-geometry-multiturn",
                    "codex_exit_code": second.returncode,
                    "mcp_tool_calls": calls,
                    "expected_sequence": list(SEQUENCE),
                    "source_sha256": source_sha256,
                    "source_size_bytes": len(source_bytes),
                    "reference_path_recorded": False,
                    "image_bytes_recorded": False,
                    "geometry": "NOT_RUN",
                    "render": "NOT_RUN",
                    "reason": "Codex did not return hash-bound geometry artifact and candidate IDs.",
                }
                print(json.dumps(receipt, ensure_ascii=False, separators=(",", ":")))
                return 3

            all_items = first_items + second_items
            # Codex's ephemeral sessions reuse item_0/item_1 IDs; parse each
            # turn independently so a later result cannot overwrite an
            # earlier tool in the sanitized receipt.
            calls = mcp_calls(first_items) + mcp_calls(second_items)
            tools = [call.get("tool") for call in calls if call.get("server") == "forgecad"]
            statuses_ok = all(call.get("status") == "completed" for call in calls)
            status = "PASS" if all(turn.returncode == 0 for turn in (first, second)) and tools == list(SEQUENCE) and statuses_ok and not unrelated_side_effects(all_items) else "BLOCKED"
            if options.debug:
                import sys
                for turn in (first, second):
                    print(turn.stdout, file=sys.stderr)
                    print(turn.stderr, file=sys.stderr)
            receipt = {
                "status": status,
                "mode": "codex-cli-mcp007-geometry-multiturn",
                "codex_exit_code": max(turn.returncode for turn in (first, second)),
                "mcp_tool_calls": calls,
                "expected_sequence": list(SEQUENCE),
                "source_sha256": source_sha256,
                "source_size_bytes": len(source_bytes),
                "reference_path_recorded": False,
                "image_bytes_recorded": False,
                "geometry": "PASS" if status == "PASS" else "NOT_RUN",
                "render": "NOT_RUN",
                "artifact": {
                    "artifact_id": artifact_id,
                    "candidate_id": candidate_id,
                    "part_count": len(artifact.get("part_ids", [])) if isinstance(artifact, dict) else None,
                    "triangle_count": artifact.get("triangle_count") if isinstance(artifact, dict) else None,
                    "validator_status": artifact.get("validator_status") if isinstance(artifact, dict) else None,
                },
                "reason": None if status == "PASS" else "Codex did not complete the exact MCP007 sequence across the two short turns.",
            }
            print(json.dumps(receipt, ensure_ascii=False, separators=(",", ":")))
            return 0 if status == "PASS" else 3
        finally:
            if runtime.poll() is None:
                runtime.terminate()
                try:
                    runtime.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    runtime.kill()
                    runtime.wait(timeout=5)


if __name__ == "__main__":
    raise SystemExit(main())
