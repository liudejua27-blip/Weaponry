#!/usr/bin/env python3
"""Run the real Codex CLI through the MCP008/MCP009 appearance slice.

The prompt supplies a small, deterministic typed robot program so the probe
tests the real Codex host, MCP wire, Runtime, CAS GLB/readback, fixed render,
approval and path-free MVP GLB export.  The reference image is admitted by
the same Codex turn, but this receipt does not claim pixel-level visual
similarity or human acceptance.
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

from probe_mcp007_codex_cli import (
    canonical_json,
    config_override,
    event_items,
    geometry_program,
    mcp_calls,
    sha256_json_without_hash,
    unrelated_side_effects,
)


SEQUENCE = (
    "project_create",
    "reference_import",
    "geometry_prepare",
    "artifact_readback_get",
    "appearance_prepare",
    "artifact_readback_get",
    "quality_get",
    "candidate_confirm",
    "version_list",
    "export_prepare",
    "export_confirm",
    "version_list",
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--reference", required=True, help="user-authorized PNG/JPEG path")
    parser.add_argument("--runtime-command", default="forgecad-runtime")
    parser.add_argument("--mcp-command", default="forgecad-mcp")
    parser.add_argument("--codex-command", default="codex")
    parser.add_argument("--timeout", type=float, default=360.0)
    parser.add_argument("--debug", action="store_true", help="print local Codex diagnostics to stderr; not evidence")
    return parser.parse_args()


def appearance_program(geometry: dict[str, Any]) -> dict[str, Any]:
    zones = [
        {
            "zone_id": "zone-white-shell",
            "part_ids": [
                "head-shell",
                "chest-shell",
                "pelvis-shell",
                "arm-left",
                "arm-right",
                "thigh-left",
                "thigh-right",
            ],
            "base_color": [0.78, 0.82, 0.86, 1],
            "metallic": 0.72,
            "roughness": 0.28,
            "emissive": [0, 0, 0],
        },
        {
            "zone_id": "zone-black-mechanical",
            "part_ids": [
                "visor",
                "neck",
                "chest-core",
                "hand-left",
                "hand-right",
                "shin-left",
                "shin-right",
            ],
            "base_color": [0.03, 0.04, 0.05, 1],
            "metallic": 0.75,
            "roughness": 0.30,
            "emissive": [0, 0, 0],
        },
        {
            "zone_id": "zone-amber-emissive",
            "part_ids": ["chest-light"],
            "base_color": [0.16, 0.06, 0.01, 1],
            "metallic": 0.20,
            "roughness": 0.25,
            "emissive": [1, 0.12, 0.01],
        },
    ]
    value: dict[str, Any] = {
        "schema_version": "AppearanceProgram@1",
        "project_id": geometry["project_id"],
        "geometry_program_sha256": geometry["canonical_sha256"],
        "material_zones": zones,
    }
    value["canonical_sha256"] = sha256_json_without_hash(value)
    return value


def reference_geometry_program() -> dict[str, Any]:
    program = geometry_program()
    program["nodes"].append(
        {
            "node_id": "chest-light",
            "operator_id": "forgecad.geometry.primitive@1",
            "part_id": "chest-light",
            "parameters": {
                "shape": "cylinder",
                "size": [0.16, 0.08, 0.16],
                "position": [0, 2.08, -0.58],
                "material_zone_id": "zone-amber-emissive",
                "segments": 16,
            },
        }
    )
    program["canonical_sha256"] = sha256_json_without_hash(program)
    return program


def prompt(reference_path: str) -> str:
    geometry = reference_geometry_program()
    appearance = appearance_program(geometry)
    geometry_json = json.dumps(geometry, ensure_ascii=False, separators=(",", ":"))
    appearance_json = json.dumps(appearance, ensure_ascii=False, separators=(",", ":"))
    return f"""Use only the forgecad MCP server. Do not use shell, filesystem, browser, images, other MCP servers or arbitrary code.

This is a real host acceptance turn for the user's authorized hard-surface robot reference. The typed programs below are bounded product-owned primitives; do not describe them as pixel-level reconstruction. Execute exactly these calls in this order and wait for each structured result:
1) project_create with name=\"Codex MCP009 robot appearance\" and policy={{\"profile\":\"mvp\"}}. Save project_id.
2) reference_import with project_id=<saved project_id>, source={{\"kind\":\"codex_local_file\",\"path\":{json.dumps(reference_path, ensure_ascii=False)}}}, authorization={{\"user_authorized\":true,\"declaration\":\"The user supplied and authorized this reference for the local ForgeCAD MVP.\"}}. The returned structured_content is {{\"reference\":{{...}}}}; save reference_id from structured_content.reference.reference_id.
3) geometry_prepare with project_id=<saved project_id>, request={{\"typed\":\"geometry\",\"reference_id\":<saved reference_id>,\"geometry_program\":{geometry_json}}}. The returned structured_content is {{\"artifact\":{{...}},\"candidate\":{{...}}}}; save artifact_id and candidate_id from structured_content.artifact, plus the returned candidate object.
4) Immediately call artifact_readback_get with exact JSON arguments {{\"artifact_id\":<saved geometry artifact_id>,\"candidate_id\":<saved geometry candidate_id>}}. Do not omit either key and do not call it with an empty object.
5) appearance_prepare with project_id=<saved project_id>, request={{\"typed\":\"appearance\",\"reference_id\":<saved reference_id>,\"geometry_program\":{geometry_json},\"appearance_program\":{appearance_json}}}. Its structured_content.artifact is the NEW appearance artifact; save its artifact_id and candidate_id, not the old geometry candidate.
6) Immediately call artifact_readback_get with exact JSON arguments {{\"artifact_id\":<saved appearance artifact_id>,\"candidate_id\":<saved appearance candidate_id>}}. Do not omit either key.
7) quality_get with the appearance candidate_id and reference_id. Treat the reference comparison as limited evidence, not a visual similarity score.
8) candidate_confirm with project_id and the appearance candidate's candidate_id, prepared_object_id, prepared_object_sha256 and quality_report_id copied from that appearance candidate. Use approval_receipt_id=\"codex-mcp009-approval\", approval_summary=\"Approve bounded robot appearance candidate\", approval_session_id=\"codex-mcp009-session\", approval_expires_at=\"9999999999\", idempotency_key=\"codex-mcp009-confirm\".
9) version_list with the saved project_id and save the newly confirmed version_id.
10) export_prepare with project_id, the confirmed version_id, format=\"glb\", profile=\"mvp-glb\", request={{\"target\":\"cas-only\",\"reason\":\"Codex MCP009 host acceptance\"}}. Save export_id.
11) export_confirm with project_id, the saved export_id, the same version_id, format=\"glb\", profile=\"mvp-glb\", approval_receipt_id=\"codex-mcp009-export-approval\", approval_summary=\"Approve CAS-backed MVP GLB export\", approval_session_id=\"codex-mcp009-session\", approval_expires_at=\"9999999999\", idempotency_key=\"codex-mcp009-export\".
12) version_list again.

This is a hard protocol sequence, not a conversational checkpoint: after the first version_list you MUST continue immediately with export_prepare, export_confirm and the final version_list. Do not stop after candidate_confirm/version_list and do not emit a final answer until all twelve calls have completed. Do not retry with incomplete arguments and do not make any extra ForgeCAD call. If a required result field is missing, stop and report the failure rather than issuing an empty or duplicate call.

Return only a compact summary of the tool calls and the returned artifact/version/output hashes. Do not claim human visual acceptance, pixel similarity, packaged signing or a finished high-quality model.
"""


def blocked(reason: str, source_sha256: str, size: int) -> dict[str, Any]:
    return {
        "status": "BLOCKED",
        "reason": reason,
        "scope": "MCP008/MCP009 real Codex CLI appearance and export host slice",
        "source_sha256": source_sha256,
        "source_size_bytes": size,
        "reference_path_recorded": False,
        "image_bytes_recorded": False,
        "visual_similarity": "NOT_RUN",
        "human_review": "NOT_RUN",
    }


def main() -> int:
    args = parse_args()
    source = Path(args.reference)
    if not source.is_file() or source.is_symlink():
        print(json.dumps(blocked("reference is not a regular file", "", 0)))
        return 3
    source_bytes = source.read_bytes()
    source_sha256 = hashlib.sha256(source_bytes).hexdigest()
    if not args.execute:
        result = blocked("Pass --execute to run the local Runtime and Codex CLI.", source_sha256, len(source_bytes))
        result["status"] = "NOT_RUN"
        print(json.dumps(result, separators=(",", ":")))
        return 2

    environment = os.environ.copy()
    environment.pop("CODEX_MCP_PROTOCOL_VERSION", None)
    environment["FORGECAD_MCP_ENABLE_MCP004_WRITES"] = "1"
    environment["FORGECAD_ATTACHMENT_ROOTS"] = str(source.parent)

    with tempfile.TemporaryDirectory(dir="/tmp", prefix="fc9-") as temporary:
        root = Path(temporary)
        ready = root / "ready.json"
        runtime = subprocess.Popen(
            [
                args.runtime_command,
                "serve",
                "--database",
                str(root / "runtime.sqlite"),
                "--cas-root",
                str(root / "cas"),
                "--endpoint-dir",
                str(root / "ipc"),
                "--ready-file",
                str(ready),
            ],
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
            with tempfile.TemporaryDirectory(dir="/tmp", prefix="fc9-codex-") as workspace:
                try:
                    completed = subprocess.run(
                        [
                            args.codex_command,
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
                            config_override(args.mcp_command),
                            "-c",
                            'mcp_servers.cloudflare-api={url="http://127.0.0.1:1",enabled=false,required=false}',
                            "--image",
                            str(source),
                        ],
                        input=prompt(str(source)) + "\n",
                        env=environment,
                        text=True,
                        capture_output=True,
                        timeout=args.timeout,
                        check=False,
                    )
                except subprocess.TimeoutExpired:
                    print(json.dumps(blocked("Codex CLI timed out before the ordered host receipt completed.", source_sha256, len(source_bytes)), separators=(",", ":")))
                    return 3
            calls = mcp_calls(event_items(completed.stdout))
            if args.debug:
                import sys
                print(completed.stdout, file=sys.stderr)
                print(completed.stderr, file=sys.stderr)
            tools = [call.get("tool") for call in calls if call.get("server") == "forgecad"]
            statuses_ok = all(call.get("status") == "completed" for call in calls)
            status = "PASS" if completed.returncode == 0 and tools == list(SEQUENCE) and statuses_ok and not unrelated_side_effects(event_items(completed.stdout)) else "BLOCKED"
            receipt: dict[str, Any] = {
                "status": status,
                "mode": "codex-cli-mcp009-appearance-export",
                "codex_exit_code": completed.returncode,
                "mcp_tool_calls": calls,
                "expected_sequence": list(SEQUENCE),
                "source_sha256": source_sha256,
                "source_size_bytes": len(source_bytes),
                "reference_path_recorded": False,
                "image_bytes_recorded": False,
                "visual_similarity": "NOT_RUN",
                "human_review": "NOT_RUN",
                "packaged_signing": "NOT_RUN",
                "reason": None if status == "PASS" else "Codex did not complete the exact MCP008/MCP009 host sequence.",
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
            if args.debug and runtime.stderr is not None:
                import sys
                runtime_diagnostics = runtime.stderr.read()
                if runtime_diagnostics:
                    print(runtime_diagnostics, file=sys.stderr)


if __name__ == "__main__":
    raise SystemExit(main())
