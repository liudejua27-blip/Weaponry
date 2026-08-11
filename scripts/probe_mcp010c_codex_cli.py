#!/usr/bin/env python3
"""Run a real Codex CLI through the MCP010C visual-evidence workflow.

This is an integration probe, not an image-to-mesh claim.  Codex receives a
user-authorized reference on the setup turn and then drives the local MCP
through V2 discovery, hash/prepare, fixed nine-pass comparison, image-pass
reads, typed visual review and quality readback.  The receipt intentionally
contains no source path, prompt, token, socket or image bytes.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shlex
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
from probe_mcp007_codex_cli import (  # noqa: E402
    config_override,
    event_items,
    mcp_calls,
    structured_result,
)
from probe_mcp010b_raw_stdio import MCP_PROTOCOL_VERSION  # noqa: E402
from probe_mcp010c_raw_stdio import canonical_hash, robot_v2_program_draft  # noqa: E402


AOV_ORDER = ("beauty", "silhouette", "depth", "normal", "ao", "part-id", "material-id", "wireframe", "uv-stretch")
SETUP_SEQUENCE = ("project_create", "reference_import", "reference_get")
AUTHORING_SEQUENCE = ("capabilities_get", "runtime_status", "doctor", "operator_catalog_get", "skill_list", "geometry_program_hash", "geometry_prepare")
COMPARE_SEQUENCE = ("job_get", "candidate_get", "artifact_readback_get", "reference_compare_prepare")
RENDER_SEQUENCE = AOV_ORDER
REVIEW_SEQUENCE = ("visual_review_submit", "quality_get")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--reference", required=True, help="user-authorized PNG/JPEG path")
    parser.add_argument("--runtime-command", required=True)
    parser.add_argument("--mcp-command", required=True)
    parser.add_argument("--codex-command", default="codex")
    parser.add_argument("--evidence", type=Path, help="JSON receipt below docs/evidence")
    parser.add_argument("--timeout", type=float, default=360.0)
    parser.add_argument("--sandbox", choices=("read-only", "workspace-write"), default="workspace-write")
    parser.add_argument("--debug", action="store_true", help="print redacted Codex JSONL to stderr")
    return parser.parse_args()


def write_receipt(path: Path | None, receipt: dict[str, Any]) -> None:
    if path is None:
        return
    root = Path(__file__).resolve().parents[1]
    resolved = path if path.is_absolute() else root / path
    evidence_root = (root / "docs" / "evidence").resolve()
    try:
        resolved.resolve().relative_to(evidence_root)
    except ValueError as error:
        raise SystemExit("Codex CLI probe evidence must stay under docs/evidence") from error
    if resolved.suffix != ".json":
        raise SystemExit("Codex CLI probe evidence must be JSON")
    resolved.parent.mkdir(parents=True, exist_ok=True)
    resolved.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def build_cohort(command: str, component: str) -> str:
    result = subprocess.run([command, "--build-identity"], capture_output=True, text=True, timeout=20, check=True)
    identity = json.loads(result.stdout)
    cohort = identity.get("build_cohort_sha256") if isinstance(identity, dict) else None
    if identity.get("component") != component or not isinstance(cohort, str) or len(cohort) != 64:
        raise ValueError(f"invalid {component} build identity")
    return cohort


def wait_for_ready(path: Path, process: subprocess.Popen[str], timeout: float) -> dict[str, Any]:
    deadline = time.monotonic() + min(timeout, 30.0)
    while time.monotonic() < deadline:
        if path.is_file():
            return json.loads(path.read_text(encoding="utf-8"))
        if process.poll() is not None:
            break
        time.sleep(0.05)
    raise RuntimeError("Runtime did not publish a ready handoff")


def call_sequence(items: list[dict[str, Any]]) -> list[str]:
    return [str(call.get("tool")) for call in mcp_calls(items) if call.get("server") == "forgecad"]


def has_subsequence(actual: list[str], expected: tuple[str, ...]) -> bool:
    cursor = 0
    for name in actual:
        if cursor < len(expected) and name == expected[cursor]:
            cursor += 1
    return cursor == len(expected)


def all_completed(items: list[dict[str, Any]], expected: tuple[str, ...]) -> bool:
    calls = [call for call in mcp_calls(items) if call.get("server") == "forgecad"]
    by_tool: dict[str, list[dict[str, Any]]] = {}
    for call in calls:
        by_tool.setdefault(str(call.get("tool")), []).append(call)
    return all(any(call.get("status") == "completed" for call in by_tool.get(name, [])) for name in expected)


def render_pass_names(items: list[dict[str, Any]]) -> list[str]:
    names: list[str] = []
    for item in items:
        if item.get("type") != "mcp_tool_call" or item.get("tool") != "render_pass_get" or item.get("status") != "completed":
            continue
        arguments = item.get("arguments")
        if isinstance(arguments, dict) and isinstance(arguments.get("pass"), str):
            names.append(arguments["pass"])
    return names


def side_effect_summary(items: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Return a path/prompt-free summary of non-MCP Codex events."""
    summary: list[dict[str, Any]] = []
    for item in items:
        item_type = str(item.get("type", ""))
        if item_type not in {"command_execution", "file_change", "mcp_resource_write"}:
            continue
        command = item.get("command")
        if not isinstance(command, str):
            command = item.get("arguments")
        if isinstance(command, (dict, list)):
            command_text = json.dumps(command, sort_keys=True, ensure_ascii=False)
        elif isinstance(command, str):
            command_text = command
        else:
            command_text = ""
        normalized_command = command_text.lower()
        forbidden_tokens = (
            "rm ", "mv ", "cp ", "chmod ", "chown ", "tee ", "install ",
            "python", "node ", "git ", "curl ", "wget ", "http://", "https://",
        )
        shell_mutation = bool(re.search(r"[;&|<>]", command_text))
        read_only_skill_lookup = (
            item_type == "command_execution"
            and "skill.md" in normalized_command
            and ".codex/" in normalized_command
            and not shell_mutation
            and not any(token in normalized_command for token in forbidden_tokens)
        )
        tokens: list[str] = []
        if command_text:
            try:
                tokens = [token.rsplit("/", 1)[-1] for token in shlex.split(command_text)[:3]]
            except ValueError:
                tokens = []
        summary.append({
            "type": item_type,
            "status": item.get("status"),
            "command_token_count": len(tokens),
            "command_token_basename": tokens,
            "command_sha256": hashlib.sha256(command_text.encode("utf-8")).hexdigest() if command_text else None,
            "classification": "codex_skill_read_only" if read_only_skill_lookup else "unapproved_external_event",
        })
    return summary


def blocking_side_effects(summary: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [event for event in summary if event.get("classification") != "codex_skill_read_only"]


def run_codex_turn(options: argparse.Namespace, environment: dict[str, str], prompt_text: str, workspace_root: str, image_path: str | None = None) -> subprocess.CompletedProcess[str]:
    """Run one short Codex turn with an explicit, inspectable shell sandbox."""
    with tempfile.TemporaryDirectory(dir="/tmp", prefix="fc10c-codex-turn-") as workspace:
        command = [
            options.codex_command,
            "exec",
            "--ephemeral",
            "--ignore-user-config",
            "--json",
            "--color",
            "never",
            "--skip-git-repo-check",
            "-C",
            workspace,
            "-c",
            config_override(options.mcp_command),
            "-c",
            'mcp_servers.cloudflare-api={url="http://127.0.0.1:1",enabled=false,required=false}',
        ]
        if options.sandbox == "read-only":
            command[8:8] = ["--sandbox", "read-only"]
        else:
            command[8:8] = ["--approve-for-me"]
        if image_path:
            command.extend(["--image", image_path])
            return subprocess.run(command, input=prompt_text + "\n", env=environment, text=True, capture_output=True, timeout=options.timeout, check=False)
        command.append(prompt_text)
        return subprocess.run(command, env=environment, text=True, capture_output=True, timeout=options.timeout, check=False)


def field(result: Any, *names: str) -> Any:
    current = result
    for name in names:
        if not isinstance(current, dict):
            return None
        current = current.get(name)
    return current


def reference_dimensions(path: Path) -> tuple[int, int]:
    data = path.read_bytes()
    if not data.startswith(b"\x89PNG\r\n\x1a\n") or len(data) < 24:
        raise ValueError("the real Codex C probe currently accepts a PNG reference with a readable IHDR")
    width = int.from_bytes(data[16:20], "big")
    height = int.from_bytes(data[20:24], "big")
    if not (1 <= width <= 8192 and 1 <= height <= 8192):
        raise ValueError("reference dimensions exceed Runtime bounds")
    return width, height


def view_spec(reference_id: str, reference_sha: str, width: int, height: int) -> dict[str, Any]:
    value: dict[str, Any] = {
        "schema_version": "ReferenceViewSpec@1",
        "reference_id": reference_id,
        "reference_sha256": reference_sha,
        "view_id": "three-quarter-user-reference",
        "source_view": "three-quarter",
        # Keep the scalar representation integer-stable.  Codex's JSON
        # round-trip normalizes 0.0/1.0 to 0/1; the Runtime canonical hash is
        # type-sensitive, so the bytes hashed here must match what MCP sees.
        "image": {"width": width, "height": height, "rotation_degrees": 0, "crop": {"x": 0, "y": 0, "width": 1, "height": 1}},
        "landmarks": [],
        "regions": [],
        "canonical_sha256": "",
    }
    value["canonical_sha256"] = canonical_hash(value)
    return value


def setup_prompt(reference_path: str) -> str:
    return f"""Use only the ForgeCAD MCP server. Do not use shell, filesystem, browser, other MCP servers, or arbitrary code.

This is the first setup turn for a real MCP010C host gate. The user supplied and authorized the attached reference image. Call exactly these two ForgeCAD tools, in order, then stop:
1) project_create with name=\"MCP010C Codex visual review\" and policy={{\"profile\":\"mvp\"}}; save project_id.
2) reference_import with that project_id, source={{\"kind\":\"codex_local_file\",\"path\":{json.dumps(reference_path, ensure_ascii=False)}}}, authorization={{\"user_authorized\":true,\"declaration\":\"The user supplied and authorized this reference for local ForgeCAD modeling.\"}}; save reference_id.

Do not call reference_get or any other ForgeCAD tool in this turn. Return only project_id and reference_id, then stop. Do not claim similarity, high quality, PBR, human approval or 360-degree coverage.
"""


def reference_get_prompt(reference_id: str) -> str:
    return f"""Use only the ForgeCAD MCP server. Call exactly one tool, then stop:
reference_get with {{\"reference_id\":{json.dumps(reference_id)}}}. Verify the returned reference_id and object_sha256, but do not request or print image bytes. Do not call any other tool or claim visual quality.
"""


def authoring_prompt(project_id: str, reference_id: str) -> str:
    draft = json.dumps(robot_v2_program_draft(project_id, "<copy-exact-live-catalog-hash>"), ensure_ascii=False, separators=(",", ":"))
    return f"""Use only the ForgeCAD MCP server. Do not use shell, filesystem, browser, images, other MCP servers, or local hash code.

The project_id is {json.dumps(project_id)} and the authorized reference_id is {json.dumps(reference_id)}. Complete exactly this V2 authoring sequence in order, saving returned values between calls:
1) capabilities_get; require Runtime Ready and save operator_catalog_sha256.
2) runtime_status; require Ready.
3) doctor; require no terminal Runtime failure.
4) operator_catalog_get; require canonical_sha256 exactly equals step 1 and active forgecad.geometry.primitive@2.
5) skill_list; record the live status, but do not call an unavailable operator.
6) geometry_program_hash with {{\"schema_version\":\"GeometryProgramHashRequest@1\",\"geometry_program_draft\":<draft below>}}. Replace only the catalog placeholder with the exact hash from steps 1 and 4. Do not add canonical_sha256 before this call.
7) geometry_prepare with {{\"project_id\":{json.dumps(project_id)},\"request\":{{\"typed\":\"geometry\",\"reference_id\":{json.dumps(reference_id)},\"geometry_program\":<same draft plus only the returned canonical_sha256>}}}}. Save the complete candidate, job and artifact objects.

Do not call any other ForgeCAD tool. Do not confirm, export, or submit visual evidence in this turn. Stop after geometry_prepare and return only the hash/catalog binding and opaque IDs/counts.

Hash-free GeometryProgram@2 draft:
{draft}
"""


def compare_prompt(project_id: str, reference_id: str, candidate_id: str, job_id: str, artifact_id: str, view: dict[str, Any]) -> str:
    view_json = json.dumps(view, ensure_ascii=False, separators=(",", ":"))
    return f"""Use the ForgeCAD MCP server now. Make only these four calls, in order, then stop. Do not explain or use another tool.

Use these exact opaque values; do not rewrite them:
1) job_get with {{\"job_id\":{json.dumps(job_id)}}}.
2) candidate_get with {{\"candidate_id\":{json.dumps(candidate_id)}}}.
3) artifact_readback_get with {{\"artifact_id\":{json.dumps(artifact_id)},\"candidate_id\":{json.dumps(candidate_id)}}}.
4) reference_compare_prepare with this exact JSON object: {{\"project_id\":{json.dumps(project_id)},\"candidate_id\":{json.dumps(candidate_id)},\"reference_id\":{json.dumps(reference_id)},\"view_spec\":{view_json}}}. Copy the view_spec byte-for-byte, including canonical_sha256.

Do not call render_pass_get, review, confirm or export in this turn. Return only the two comparison CAS hashes."""


def render_prompt(render_set_hash: str) -> str:
    passes = ", ".join(json.dumps(name) for name in AOV_ORDER)
    return f"""Use the ForgeCAD MCP server now. Call render_pass_get exactly once for each of these nine passes, in this order: {passes}. For every call use the exact render_set_hash {json.dumps(render_set_hash)} and the pass enum shown. Do not call another tool, do not alter the hash, and do not copy image bytes into prose. Stop after the ninth image result and return only the nine pass names."""


def review_prompt(candidate_id: str, reference_id: str, render_set_hash: str, comparison_hash: str) -> str:
    issue = json.dumps([
        {
            "issue_id": "primitive-blockout",
            "pass": "silhouette",
            "region_id": "whole-body",
            "claim": "The primitive candidate remains a structural blockout and does not yet reproduce the reference panel, vent, cable and joint detail.",
            "confidence": 0.98,
            "visibility": "observed",
            "action": "Keep this visual evidence and request a bounded hard-surface detail revision; do not claim a likeness pass.",
        }
    ], ensure_ascii=False, separators=(",", ":"))
    return f"""Use the ForgeCAD MCP server now. Make only these two calls, in order, then stop.
1) visual_review_submit with {{\"candidate_id\":{json.dumps(candidate_id)},\"reference_id\":{json.dumps(reference_id)},\"render_set_hash\":{json.dumps(render_set_hash)},\"comparison_report_hash\":{json.dumps(comparison_hash)},\"round\":1,\"stage\":\"silhouette\",\"issues\":{issue},\"status\":\"needs_revision\"}}.
2) quality_get with {{\"candidate_id\":{json.dumps(candidate_id)},\"reference_id\":{json.dumps(reference_id)}}}.
Do not call human_visual_review_submit, candidate_confirm, export or any other tool. Return only review status, quality visual_status, hard_gate_passed and comparison metrics. Do not claim high quality or human approval."""


def base_receipt(source_sha: str, source_size: int) -> dict[str, Any]:
    return {
        "schema_version": "ForgeCADMCP010CCodexCliProbe@1",
        "task_id": "FGC-MCP010C",
        "scope": "real Codex CLI source-built fixed-render/compare/review transport",
        "source_sha256": source_sha,
        "source_size_bytes": source_size,
        "reference_path_recorded": False,
        "image_bytes_recorded": False,
        "persistent_user_data_touched": False,
        "human_review": "NOT_RUN",
        "pbr_material_pack": "NOT_RUN",
        "hq_360": "BLOCKED_REFERENCE_COVERAGE",
        "visual_quality_claim": "NOT_CLAIMED",
    }


def main() -> int:
    options = parse_args()
    source = Path(options.reference).expanduser()
    if not source.is_file() or source.is_symlink():
        receipt = base_receipt("", 0) | {"status": "BLOCKED", "reason": "reference is not a regular file"}
        write_receipt(options.evidence, receipt)
        print(json.dumps(receipt, sort_keys=True))
        return 3
    source_bytes = source.read_bytes()
    source_sha = hashlib.sha256(source_bytes).hexdigest()
    try:
        width, height = reference_dimensions(source)
    except ValueError as error:
        receipt = base_receipt(source_sha, len(source_bytes)) | {"status": "BLOCKED", "reason": str(error)}
        write_receipt(options.evidence, receipt)
        print(json.dumps(receipt, sort_keys=True))
        return 3
    if not options.execute:
        receipt = base_receipt(source_sha, len(source_bytes)) | {"status": "NOT_RUN", "reason": "Pass --execute to run the isolated local Runtime and Codex CLI."}
        write_receipt(options.evidence, receipt)
        print(json.dumps(receipt, sort_keys=True))
        return 2

    runtime_command = str(Path(options.runtime_command).expanduser().resolve())
    mcp_command = str(Path(options.mcp_command).expanduser().resolve())
    worker_command = str(Path(runtime_command).with_name("forgecad-geometry-worker"))
    if options.timeout <= 0 or not Path(runtime_command).is_file() or not Path(mcp_command).is_file() or not Path(worker_command).is_file():
        receipt = base_receipt(source_sha, len(source_bytes)) | {"status": "BLOCKED", "reason": "same-cohort source MCP, Runtime and geometry Worker binaries were unavailable"}
        write_receipt(options.evidence, receipt)
        print(json.dumps(receipt, sort_keys=True))
        return 3
    try:
        cohorts = {
            "mcp": build_cohort(mcp_command, "forgecad-mcp"),
            "runtime": build_cohort(runtime_command, "forgecad-runtime"),
            "worker": build_cohort(worker_command, "forgecad-geometry-worker"),
        }
    except (OSError, subprocess.SubprocessError, ValueError, json.JSONDecodeError) as error:
        receipt = base_receipt(source_sha, len(source_bytes)) | {"status": "BLOCKED", "reason": f"build identity unavailable: {str(error)[:240]}"}
        write_receipt(options.evidence, receipt)
        print(json.dumps(receipt, sort_keys=True))
        return 3
    if len(set(cohorts.values())) != 1:
        receipt = base_receipt(source_sha, len(source_bytes)) | {"status": "BLOCKED", "reason": "MCP, Runtime and Worker build cohorts did not match", "build_cohorts": cohorts}
        write_receipt(options.evidence, receipt)
        print(json.dumps(receipt, sort_keys=True))
        return 3

    environment = os.environ.copy()
    for key in ("CODEX_MCP_PROTOCOL_VERSION", "FORGECAD_RUNTIME_SOCKET", "FORGECAD_RUNTIME_TOKEN", "FORGECAD_RUNTIME_DATA_DIR", "FORGECAD_RUNTIME_COMMAND", "FORGECAD_RUNTIME_READY_FILE", "FORGECAD_RUNTIME_STATUS_FILE"):
        environment.pop(key, None)
    environment["FORGECAD_MCP_ENABLE_MCP004_WRITES"] = "1"
    environment["FORGECAD_ATTACHMENT_ROOTS"] = str(source.parent)

    receipt = base_receipt(source_sha, len(source_bytes)) | {"status": "BLOCKED", "build_cohorts": cohorts}
    runtime: subprocess.Popen[str] | None = None
    turn_outputs: list[subprocess.CompletedProcess[str]] = []
    try:
        with tempfile.TemporaryDirectory(dir="/tmp", prefix="fc10c-codex-") as temporary:
            root = Path(temporary)
            ready_path = root / "ipc" / "ready.json"
            runtime = subprocess.Popen(
                [runtime_command, "serve", "--database", str(root / "runtime.sqlite"), "--cas-root", str(root / "cas"), "--endpoint-dir", str(root / "ipc"), "--ready-file", str(ready_path)],
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE,
                text=True,
                encoding="utf-8",
                env=environment,
            )
            handoff = wait_for_ready(ready_path, runtime, options.timeout)
            socket_path = handoff.get("socket_path")
            token = handoff.get("token")
            if not isinstance(socket_path, str) or not isinstance(token, str):
                raise RuntimeError("ready handoff omitted authenticated endpoint")
            environment["FORGECAD_RUNTIME_SOCKET"] = socket_path
            environment["FORGECAD_RUNTIME_TOKEN"] = token

            first = run_codex_turn(options, environment, setup_prompt(str(source)), str(root), str(source))
            turn_outputs.append(first)
            first_items = event_items(first.stdout)
            project_result = structured_result(first_items, "project_create") or {}
            reference_result = structured_result(first_items, "reference_import") or {}
            reference = field(reference_result, "reference") or reference_result
            project_id = field(project_result, "project_id")
            reference_id = field(reference, "reference_id")
            reference_sha = field(reference, "object_sha256")
            if not isinstance(project_id, str) or not isinstance(reference_id, str) or not isinstance(reference_sha, str):
                raise RuntimeError("Codex setup did not return project/reference evidence")
            reference_get_result = structured_result(first_items, "reference_get")
            setup_calls = mcp_calls(first_items)
            if reference_get_result is None:
                setup_readback = run_codex_turn(options, environment, reference_get_prompt(reference_id), str(root))
                turn_outputs.append(setup_readback)
                setup_readback_items = event_items(setup_readback.stdout)
                setup_calls.extend(mcp_calls(setup_readback_items))
                reference_get_result = structured_result(setup_readback_items, "reference_get")
            reference_get = field(reference_get_result or {}, "reference") or reference_get_result or {}
            if field(reference_get, "reference_id") != reference_id or field(reference_get, "object_sha256") != reference_sha:
                raise RuntimeError("reference_get did not match reference_import")
            setup_tool_names = [str(call.get("tool")) for call in setup_calls if call.get("server") == "forgecad"]
            if not (has_subsequence(setup_tool_names, SETUP_SEQUENCE) and all(call.get("status") == "completed" for call in setup_calls)):
                raise RuntimeError("Codex setup did not complete the required MCP sequence")

            second = run_codex_turn(options, environment, authoring_prompt(project_id, reference_id), str(root))
            turn_outputs.append(second)
            second_items = event_items(second.stdout)
            catalog = structured_result(second_items, "operator_catalog_get") or {}
            capabilities = structured_result(second_items, "capabilities_get") or {}
            hashed = structured_result(second_items, "geometry_program_hash") or {}
            prepared = structured_result(second_items, "geometry_prepare") or {}
            catalog_hash = field(catalog, "canonical_sha256")
            capability_hash = field(capabilities, "operator_catalog_sha256")
            program_hash = field(hashed, "canonical_sha256")
            candidate = field(prepared, "candidate") or {}
            job = field(prepared, "job") or {}
            artifact = field(prepared, "artifact") or {}
            candidate_id = field(candidate, "candidate_id")
            job_id = field(job, "job_id")
            artifact_id = field(artifact, "artifact_id")
            if not all(isinstance(value, str) and value for value in (catalog_hash, capability_hash, program_hash, candidate_id, job_id, artifact_id)):
                raise RuntimeError("Codex authoring did not return all V2 hashes and opaque IDs")
            if catalog_hash != capability_hash or not has_subsequence(call_sequence(second_items), AUTHORING_SEQUENCE) or not all_completed(second_items, AUTHORING_SEQUENCE):
                raise RuntimeError("Codex authoring did not complete matching discovery/hash/prepare sequence")

            spec = view_spec(reference_id, reference_sha, width, height)
            third = run_codex_turn(options, environment, compare_prompt(project_id, reference_id, candidate_id, job_id, artifact_id, spec), str(root))
            turn_outputs.append(third)
            third_items = event_items(third.stdout)
            comparison = structured_result(third_items, "reference_compare_prepare") or {}
            render_set_hash = field(comparison, "render_set_object_sha256") or field(comparison, "render_set_hash")
            comparison_hash = field(comparison, "comparison_report_object_sha256") or field(comparison, "comparison_report_hash")
            render_set = field(comparison, "render_set") or {}
            metrics = field(comparison, "comparison_report", "metrics")
            actual_third = call_sequence(third_items)
            if not isinstance(render_set_hash, str) or not isinstance(comparison_hash, str):
                raise RuntimeError("Codex comparison did not return candidate-bound CAS hashes")
            if render_set.get("passes") != list(AOV_ORDER):
                raise RuntimeError("Codex comparison did not return the fixed nine AOV order")
            if not has_subsequence(actual_third, COMPARE_SEQUENCE) or not all_completed(third_items, COMPARE_SEQUENCE):
                raise RuntimeError("Codex did not complete the readback/compare sequence")

            fourth = run_codex_turn(options, environment, render_prompt(render_set_hash), str(root))
            turn_outputs.append(fourth)
            fourth_items = event_items(fourth.stdout)
            actual_fourth = call_sequence(fourth_items)
            actual_render_passes = render_pass_names(fourth_items)
            if actual_render_passes != list(RENDER_SEQUENCE) or len(actual_render_passes) != len(RENDER_SEQUENCE):
                raise RuntimeError("Codex did not complete all nine render-pass reads")

            fifth = run_codex_turn(options, environment, review_prompt(candidate_id, reference_id, render_set_hash, comparison_hash), str(root))
            turn_outputs.append(fifth)
            fifth_items = event_items(fifth.stdout)
            review = structured_result(fifth_items, "visual_review_submit") or {}
            quality = structured_result(fifth_items, "quality_get") or {}
            quality_report = field(quality, "quality_report") or quality
            review_report = field(review, "review") or review
            actual_fifth = call_sequence(fifth_items)
            if not has_subsequence(actual_fifth, REVIEW_SEQUENCE) or not all_completed(fifth_items, REVIEW_SEQUENCE):
                raise RuntimeError("Codex did not complete typed review and quality readback")
            all_items = [item for turn in turn_outputs for item in event_items(turn.stdout)]
            side_effects = side_effect_summary(all_items)
            blocking_events = blocking_side_effects(side_effects)
            expected_transport = all(turn.returncode == 0 for turn in turn_outputs) and not blocking_events
            receipt.update({
                "status": "PASS_WITH_QUALITY_TARGET_NOT_MET" if expected_transport else "BLOCKED",
                "protocol_version": MCP_PROTOCOL_VERSION,
                "codex_turn_count": len(turn_outputs),
                "codex_exit_codes": [turn.returncode for turn in turn_outputs],
                "codex_sandbox": options.sandbox,
                "unrelated_side_effects": bool(blocking_events),
                "side_effect_events": side_effects,
                "allowed_read_only_events": len(side_effects) - len(blocking_events),
                "project_id": project_id,
                "reference_id": reference_id,
                "reference_sha256": reference_sha,
                "reference_width": width,
                "reference_height": height,
                "catalog_sha256": catalog_hash,
                "program_sha256": program_hash,
                "candidate_id": candidate_id,
                "job_id": job_id,
                "artifact_id": artifact_id,
                "artifact_sha256": field(artifact, "object_sha256"),
                "part_count": len(field(artifact, "part_ids") or []),
                "triangle_count": field(artifact, "triangle_count"),
                "validator_status": field(artifact, "validator_status"),
                "render_set_hash": render_set_hash,
                "comparison_report_hash": comparison_hash,
                "aov_order": list(AOV_ORDER),
                "render_pass_calls": len(actual_render_passes),
                "render_pass_order": actual_render_passes,
                "render_pass_image_blocks": "NOT_OBSERVED_IN_SANITIZED_CLI_EVENTS",
                "comparison_metrics": metrics,
                "visual_review_status": field(review_report, "status"),
                "quality_visual_status": field(quality_report, "visual_status"),
                "quality_hard_gate_passed": field(quality_report, "hard_gate_passed"),
                "mcp_tool_calls": [call for turn in turn_outputs for call in mcp_calls(event_items(turn.stdout))],
                "expected_sequences": {"setup": list(SETUP_SEQUENCE), "authoring": list(AUTHORING_SEQUENCE), "compare": list(COMPARE_SEQUENCE), "render": list(RENDER_SEQUENCE), "review": list(REVIEW_SEQUENCE)},
                "quality_claim": "QUALITY_TARGET_NOT_MET_OR_NOT_CLAIMED",
            })
            if options.debug:
                for turn in turn_outputs:
                    print(turn.stdout, file=sys.stderr)
                    print(turn.stderr, file=sys.stderr)
    except (OSError, RuntimeError, subprocess.SubprocessError, json.JSONDecodeError) as error:
        receipt["reason"] = str(error)[:2000]
        receipt["mcp_tool_calls"] = [call for turn in turn_outputs for call in mcp_calls(event_items(turn.stdout))]
        if options.debug:
            for turn in turn_outputs:
                print(turn.stdout, file=sys.stderr)
                print(turn.stderr, file=sys.stderr)
    finally:
        if runtime is not None and runtime.poll() is None:
            runtime.terminate()
            try:
                runtime.wait(timeout=5)
            except subprocess.TimeoutExpired:
                runtime.kill()
                runtime.wait(timeout=5)
    write_receipt(options.evidence, receipt)
    print(json.dumps(receipt, sort_keys=True))
    return 0 if receipt.get("status") == "PASS_WITH_QUALITY_TARGET_NOT_MET" else 3


if __name__ == "__main__":
    raise SystemExit(main())
