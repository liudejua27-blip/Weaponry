#!/usr/bin/env python3
"""Run an explicit, diagnostic-only Codex CLI MCP004 write probe.

The probe starts one local Runtime writer with a short-lived authenticated IPC
endpoint, then asks an authenticated Codex CLI turn to create a project,
prepare a bounded non-visual candidate, confirm it, restore it and confirm a
path-free diagnostic export. It never uploads an image, runs a
geometry/render worker, writes a production file, or changes the user Codex
configuration. Without ``--execute`` it is a no-op.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any


WRITE_SEQUENCE = (
    "project_create",
    "candidate_prepare",
    "candidate_confirm",
    "restore_prepare",
    "restore_confirm",
    "export_prepare",
    "export_confirm",
)
WRITE_TOOLS = set(WRITE_SEQUENCE)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--execute",
        action="store_true",
        help="Actually invoke Runtime and authenticated Codex CLI; otherwise no process starts.",
    )
    parser.add_argument(
        "--runtime-command",
        default="forgecad-runtime",
        help="Runtime launcher binary (default: forgecad-runtime).",
    )
    parser.add_argument(
        "--mcp-command",
        default="forgecad-mcp",
        help="MCP stdio binary (default: forgecad-mcp).",
    )
    parser.add_argument(
        "--codex-command",
        default="codex",
        help="Codex CLI binary (default: codex).",
    )
    parser.add_argument(
        "--viewer-command",
        default=None,
        help="Optional ForgeCAD Viewer binary; with --viewer-read-model it verifies the same Runtime read model after Codex writes.",
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=240.0,
        help="Maximum Codex model-turn runtime in seconds (default: 240).",
    )
    return parser.parse_args()


def config_override(command: str) -> str:
    command_literal = json.dumps(command, ensure_ascii=False)
    return (
        "mcp_servers.forgecad={"
        f"command={command_literal},"
        'args=["serve","--stdio"],'
        'env_vars=["FORGECAD_RUNTIME_SOCKET","FORGECAD_RUNTIME_TOKEN","FORGECAD_MCP_ENABLE_MCP004_WRITES"],'
        "enabled=true,required=true,"
        "startup_timeout_sec=20,"
        "tool_timeout_sec=60,"
        'default_tools_approval_mode="writes"'
        "}"
    )


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
    return [grouped[key] for key in order]


def unrelated_side_effects(items: list[dict[str, Any]]) -> bool:
    normalized_types = {
        str(item.get("type", "")).replace("_", "").lower() for item in items
    }
    return bool(
        normalized_types
        & {
            "commandexecution",
            "filechange",
            "mcpresourcewrite",
            "mcptoolcallwrite",
        }
    )


def call_mcp_read_state(mcp_command: str, environment: dict[str, str]) -> dict[str, Any]:
    process = subprocess.Popen(
        [mcp_command, "serve", "--stdio"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=environment,
    )
    next_id = 1

    def request(method: str, params: dict[str, Any] | None = None) -> dict[str, Any]:
        nonlocal next_id
        identifier = next_id
        next_id += 1
        payload: dict[str, Any] = {"jsonrpc": "2.0", "id": identifier, "method": method}
        if params is not None:
            payload["params"] = params
        assert process.stdin is not None
        process.stdin.write(json.dumps(payload) + "\n")
        process.stdin.flush()
        assert process.stdout is not None
        for line in process.stdout:
            response = json.loads(line)
            if response.get("id") == identifier:
                return response
        raise RuntimeError("MCP ended before a read-state response")

    try:
        initialized = request(
            "initialize",
            {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "mcp004-codex-cli-probe", "version": "1"},
            },
        )
        if "error" in initialized:
            raise RuntimeError("MCP initialize failed")
        assert process.stdin is not None
        process.stdin.write(
            json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n"
        )
        process.stdin.flush()
        projects = request("tools/call", {"name": "project_list", "arguments": {}})
        versions = request(
            "tools/call",
            {"name": "version_list", "arguments": {"project_id": _project_id(projects)}},
        )
        if "error" in projects or "error" in versions:
            raise RuntimeError("MCP read-state call failed")
        return {
            "project_count": len(projects["result"]["structuredContent"]),
            "version_count": len(versions["result"]["structuredContent"]),
        }
    finally:
        if process.stdin is not None:
            process.stdin.close()
        process.wait(timeout=20)


def call_viewer_read_model(viewer_command: str, environment: dict[str, str]) -> dict[str, Any]:
    completed = subprocess.run(
        [viewer_command, "--viewer-read-model"],
        env=environment,
        text=True,
        capture_output=True,
        timeout=20,
        check=False,
    )
    try:
        model = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        return {"status": "ERROR", "detail": type(error).__name__, "exit_code": completed.returncode}
    if not isinstance(model, dict):
        return {"status": "ERROR", "detail": "viewer model is not an object", "exit_code": completed.returncode}
    model["exit_code"] = completed.returncode
    return model


def _project_id(projects_response: dict[str, Any]) -> str:
    projects = projects_response["result"]["structuredContent"]
    if not projects:
        raise RuntimeError("Runtime returned no Codex-created project")
    return str(projects[0]["project_id"])


def prompt_for_mvp() -> str:
    return """Perform a bounded ForgeCAD MCP004 diagnostic acceptance using only the forgecad MCP server.

Do not use shell, filesystem, images, geometry, render, external MCP servers, or any other tool. This Runtime starts empty. Create a project and use only the bounded non-visual typed diagnostic candidate path; it is not a 3D model and must not be described as one.

Execute these calls sequentially and wait for each structured result. Do not retry a failed write with different arguments. The approval_receipt_id values below are approval-context IDs; Runtime must mint and return the final receipt ID itself.
1. Call project_create with name="Codex MVP diagnostic project" and policy={"profile":"mvp"}. Save project_id.
2. Call candidate_prepare with that project_id and request={"typed":"diagnostic","label":"codex-cli"}. Save all returned candidate fields.
3. Call candidate_confirm using the returned candidate_id, base_version_id, prepared_object_id, prepared_object_sha256 and quality_report_id. Use approval_receipt_id="codex-cli-candidate-approval", approval_summary="Confirm diagnostic MCP004 candidate from Codex CLI", approval_session_id="codex-cli-mcp004-session", approval_expires_at="9999999999", idempotency_key="codex-cli-candidate-confirm".
4. Call restore_prepare with the same project_id, source_version_id equal to the version_id returned by candidate_confirm, base_version_id equal to that version_id, and request={"typed":"diagnostic","host":"codex-cli"}.
5. Call restore_confirm using the restore candidate's returned candidate_id, source_version_id, base_version_id, prepared object fields and quality_report_id. Use approval_receipt_id="codex-cli-restore-approval", approval_summary="Confirm diagnostic MCP004 restore from Codex CLI", approval_session_id="codex-cli-mcp004-session", approval_expires_at="9999999999", idempotency_key="codex-cli-restore-confirm".
6. Call export_prepare for the restored version with format="manifest-json", profile="diagnostic", and request={"target":"cas-only"}.
7. Call export_confirm using the returned manifest export_id and version_id. Use approval_receipt_id="codex-cli-export-approval", approval_summary="Confirm diagnostic MCP004 export from Codex CLI", approval_session_id="codex-cli-mcp004-session", approval_expires_at="9999999999", idempotency_key="codex-cli-export-confirm".
8. Call project_list and version_list for the project.

Return a short summary only after all calls complete. Do not claim image import, geometry, render, visual quality, GLB or production export.
"""


def not_run_receipt(reason: str) -> dict[str, Any]:
    return {
        "status": "NOT_RUN",
        "reason": reason,
        "scope": "diagnostic Codex CLI MCP004 write probe",
        "image_upload": False,
        "three_d_generation": False,
    }


def main() -> int:
    args = parse_args()
    if not args.execute:
        print(json.dumps(not_run_receipt("Pass --execute to start the explicit local Runtime and Codex CLI probe.")))
        return 2

    environment = os.environ.copy()
    environment.pop("CODEX_MCP_PROTOCOL_VERSION", None)
    environment["FORGECAD_MCP_ENABLE_MCP004_WRITES"] = "1"
    codex_calls: list[dict[str, Any]] = []
    runtime_process: subprocess.Popen[str] | None = None

    with tempfile.TemporaryDirectory(prefix="f4-") as temporary:
        root = Path(temporary)
        ready_path = root / "ready.json"
        runtime_process = subprocess.Popen(
            [
                args.runtime_command,
                "serve",
                "--database",
                str(root / "runtime.db"),
                "--cas-root",
                str(root / "cas"),
                "--endpoint-dir",
                str(root / "ipc"),
                "--ready-file",
                str(ready_path),
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=environment,
        )
        try:
            deadline = time.monotonic() + 30
            while not ready_path.exists() and time.monotonic() < deadline:
                if runtime_process.poll() is not None:
                    break
                time.sleep(0.05)
            if not ready_path.exists():
                receipt = not_run_receipt("Runtime launcher did not publish a ready handoff.")
                print(json.dumps(receipt, separators=(",", ":")))
                return 3

            ready = json.loads(ready_path.read_text(encoding="utf-8"))
            if args.viewer_command:
                # The standalone diagnostic launcher receives ready.json at an
                # explicit path; mirror that protected handoff into the
                # normal data-root location consumed by the Viewer helper.
                viewer_handoff = root / "ipc" / "ready.json"
                viewer_handoff.write_text(ready_path.read_text(encoding="utf-8"), encoding="utf-8")
            environment.update(
                {
                    "FORGECAD_RUNTIME_SOCKET": str(ready["socket_path"]),
                    "FORGECAD_RUNTIME_TOKEN": str(ready["token"]),
                }
            )
            with tempfile.TemporaryDirectory(prefix="f4-codex-") as workspace:
                command = [
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
                    prompt_for_mvp(),
                ]
                try:
                    completed = subprocess.run(
                        command,
                        env=environment,
                        text=True,
                        capture_output=True,
                        timeout=args.timeout,
                        check=False,
                    )
                except subprocess.TimeoutExpired:
                    receipt = not_run_receipt("Codex CLI diagnostic write turn timed out.")
                    receipt.update({"codex_exit_code": None, "mcp_tool_calls": []})
                    print(json.dumps(receipt, separators=(",", ":")))
                    return 3

            items = event_items(completed.stdout)
            codex_calls = mcp_calls(items)
            observed_writes = [call.get("tool") for call in codex_calls if call.get("tool") in WRITE_TOOLS]
            valid_servers = all(call.get("server") == "forgecad" for call in codex_calls)
            completed_writes = all(
                call.get("status") == "completed"
                for call in codex_calls
                if call.get("tool") in WRITE_TOOLS
            )
            viewer_model = {"status": "NOT_RUN", "reason": "No Viewer command was supplied."}
            if args.viewer_command:
                viewer_environment = environment.copy()
                viewer_environment["FORGECAD_RUNTIME_DATA_DIR"] = str(root)
                try:
                    viewer_model = call_viewer_read_model(args.viewer_command, viewer_environment)
                except (OSError, subprocess.SubprocessError) as error:
                    viewer_model = {"status": "ERROR", "detail": type(error).__name__}
            try:
                post_state = call_mcp_read_state(args.mcp_command, environment)
            except (AssertionError, RuntimeError, KeyError, IndexError, subprocess.SubprocessError) as error:
                post_state = {"error": type(error).__name__}
            status = (
                "PASS"
                if completed.returncode == 0
                and observed_writes == list(WRITE_SEQUENCE)
                and completed_writes
                and valid_servers
                and not unrelated_side_effects(items)
                and viewer_model.get("status") in {"Ready", "NOT_RUN"}
                and post_state.get("project_count") == 1
                and post_state.get("version_count") == 2
                else "BLOCKED"
            )
            receipt = {
                "status": status,
                "mode": "diagnostic-codex-cli-mcp004-write",
                "codex_exit_code": completed.returncode,
                "mcp_tool_calls": codex_calls,
                "expected_write_sequence": list(WRITE_SEQUENCE),
                "observed_write_sequence": observed_writes,
                "unrelated_side_effects": unrelated_side_effects(items),
                "viewer_read_model": viewer_model,
                "post_runtime_state": post_state,
                "image_upload": False,
                "three_d_generation": False,
                "production_file_export": False,
                "reason": None if status == "PASS" else "Codex did not complete the exact bounded MCP004 write sequence without unrelated side effects.",
            }
            print(json.dumps(receipt, ensure_ascii=False, separators=(",", ":")))
            return 0 if status == "PASS" else 3
        finally:
            if runtime_process.poll() is None:
                runtime_process.terminate()
                try:
                    runtime_process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    runtime_process.kill()
                    runtime_process.wait(timeout=5)


if __name__ == "__main__":
    raise SystemExit(main())
