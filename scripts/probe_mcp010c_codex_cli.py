#!/usr/bin/env python3
"""Run a real Codex CLI through the MCP010C visual-evidence workflow.

This is an integration probe, not an image-to-mesh claim.  Codex receives a
user-authorized reference on the setup turn and then drives the local MCP
through V2 discovery, one canonical Agentic scene observation, hash/prepare,
fixed nine-pass comparison, image-pass reads, typed visual review and quality
readback.  The receipt intentionally contains no source path, prompt, token,
socket or image bytes.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import re
import shlex
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
from probe_mcp007_codex_cli import (  # noqa: E402
    config_override as base_config_override,
    event_items,
    mcp_calls,
    structured_result,
)
from probe_mcp010b_raw_stdio import MCP_PROTOCOL_VERSION  # noqa: E402
from probe_mcp010c_raw_stdio import canonical_hash, robot_v2_program_draft  # noqa: E402
from probe_mcp010e_raw_stdio import robot_detail_program_draft  # noqa: E402


AOV_ORDER = ("beauty", "silhouette", "depth", "normal", "ao", "part-id", "material-id", "wireframe", "uv-stretch")
METRIC_CRITERIA = {
    "silhouette_iou": ("min", "silhouette_iou_min", "VISIBLE_SILHOUETTE_IOU_MIN"),
    "boundary_f1_4px": ("min", "boundary_f1_4px_min", "VISIBLE_BOUNDARY_F1_MIN"),
    "bbox_edge_error": ("max", "bbox_edge_error_max", "VISIBLE_BBOX_EDGE_ERROR_MAX"),
    "centroid_error": ("max", "centroid_error_max", "VISIBLE_CENTROID_ERROR_MAX"),
    "landmark_coverage": ("min", "landmark_coverage_min", "VISIBLE_LANDMARK_COVERAGE_MIN"),
    "landmark_nme": ("max", "landmark_nme_max", "VISIBLE_LANDMARK_NME_MAX"),
    "region_median_iou": ("min", "region_median_iou_min", "VISIBLE_REGION_MEDIAN_IOU_MIN"),
    "critical_region_min_iou": ("min", "critical_region_min_iou_min", "VISIBLE_CRITICAL_REGION_IOU_MIN"),
}
SETUP_SEQUENCE = ("skill_get", "project_create", "reference_import", "reference_get")
AUTHORING_SEQUENCE = ("capabilities_get", "runtime_status", "doctor", "operator_catalog_get", "skill_list", "geometry_program_hash", "geometry_prepare")
COMPARE_SEQUENCE = ("job_get", "candidate_get", "artifact_readback_get", "reference_compare_prepare")
RENDER_SEQUENCE = AOV_ORDER
REVIEW_SEQUENCE = ("visual_review_submit", "quality_get")
SILHOUETTE_TARGET_SEQUENCE = ("reference_mask_prepare",)
CADFIT_COARSE_EVALUATIONS = 32
CADFIT_MID_TOP_K = 4
CADFIT_FINAL_TOP_K = 2
CADFIT_EXPECTED_EVALUATIONS = CADFIT_COARSE_EVALUATIONS + CADFIT_MID_TOP_K + CADFIT_FINAL_TOP_K + 1
CADFIT_TERMINAL_STATUSES = {"succeeded", "failed", "cancelled"}
# Keep the visual turn as one bounded observe -> decide -> prepare surface.
# scene_observe_get is deliberately inside the same ephemeral Codex turn as
# target/camera/Rig, so the model does not stitch a design decision from
# unrelated fragmented reads or silently re-observe after a state change.
SILHOUETTE_SEQUENCE = ("silhouette_target_get", "scene_observe_get", "camera_fit_prepare", "silhouette_rig_hash")


class BoundaryOnlyComplete(RuntimeError):
    """Internal control flow for the evidence-only boundary route."""


def config_override(command: str, approval_mode: str = "writes") -> str:
    """Build the MCP override, with an explicit isolated-probe opt-in.

    Product defaults keep write approval enabled.  The opt-in is only used by
    this probe against a temporary Runtime/CAS and the prompts never request
    candidate confirmation or export.
    """
    if approval_mode not in {"writes", "approve"}:
        raise ValueError(f"unsupported isolated probe approval mode: {approval_mode}")
    value = base_config_override(command)
    return value.replace(
        'default_tools_approval_mode="writes"',
        f'default_tools_approval_mode="{approval_mode}"',
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--reference", required=True, help="user-authorized PNG/JPEG path")
    parser.add_argument("--runtime-command", required=True)
    parser.add_argument("--mcp-command", required=True)
    parser.add_argument("--codex-command", default="codex")
    parser.add_argument(
        "--auto-approve-isolated-writes",
        action="store_true",
        help="For this probe's temporary Runtime only, approve setup writes; candidate confirm/export are never requested.",
    )
    parser.add_argument("--evidence", type=Path, help="JSON receipt below docs/evidence")
    parser.add_argument(
        "--intake",
        type=Path,
        help="Optional sanitized ForgeCADCodexReferenceInventory/robot intake JSON with normalized landmarks and visible regions.",
    )
    parser.add_argument(
        "--geometry-route",
        choices=("primitive", "detail"),
        default="primitive",
        help="Author a primitive V2 blockout (default) or the bounded MCP010D hard-surface detail fixture.",
    )
    parser.add_argument(
        "--geometry-variant",
        default="surface-linework",
        help="Detail-route variant passed to the first-party MCP010D fixture; ignored for primitive route.",
    )
    parser.add_argument(
        "--material-variant",
        default="armor-shell-zones",
        choices=("default-zones", "surface-zones", "armor-shell-zones"),
        help="Detail-route material-zone recipe metadata; appearance writes remain outside this C probe.",
    )
    parser.add_argument(
        "--silhouette-first",
        action="store_true",
        help="Run the Runtime-owned reference mask, camera fit and bounded SilhouetteRig proposal before comparison.",
    )
    parser.add_argument(
        "--cadfit-optimization",
        action="store_true",
        help="After the bounded silhouette fit, run a real Codex-driven asynchronous one-Part CADFit OptimizationJob; it records a proposal only and never confirms or exports.",
    )
    parser.add_argument("--timeout", type=float, default=360.0)
    parser.add_argument("--sandbox", choices=("read-only", "workspace-write"), default="workspace-write")
    parser.add_argument(
        "--viewer-executable",
        type=Path,
        help="Optional source-built ForgeCAD Viewer executable; bind its read model to this live Codex candidate.",
    )
    parser.add_argument("--debug", action="store_true", help="print redacted Codex JSONL to stderr")
    parser.add_argument(
        "--boundary-only",
        action="store_true",
        help="stop after candidate-bound boundary_error_get and persist its typed Part summary; skip AOV/review calls",
    )
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


def runtime_visible_view_policy(repo_root: Path) -> dict[str, Any]:
    """Read the threshold policy from the source used to build this cohort.

    QualityReport@2 intentionally remains a closed evidence contract and does
    not duplicate the product threshold table. The real-Codex receipt must
    still make the policy used for its metric interpretation explicit, so bind
    the parsed values to the exact Runtime source hash rather than repeating a
    second hand-maintained table in the probe.
    """
    runtime_source = repo_root / "apps/desktop/src-tauri/crates/forgecad-runtime/src/lib.rs"
    source_bytes = runtime_source.read_bytes()
    source = source_bytes.decode("utf-8")
    thresholds: dict[str, float] = {}
    for _metric, (_direction, threshold_name, constant_name) in METRIC_CRITERIA.items():
        match = re.search(
            rf"const {re.escape(constant_name)}: f64 = ([0-9]+(?:\.[0-9]+)?);",
            source,
        )
        if match is None:
            raise RuntimeError(f"Runtime visible-view threshold is missing: {constant_name}")
        thresholds[threshold_name] = float(match.group(1))
    source_sha256 = hashlib.sha256(source_bytes).hexdigest()
    return {
        "authority": "forgecad-runtime-source",
        "source_path": "apps/desktop/src-tauri/crates/forgecad-runtime/src/lib.rs",
        "source_sha256": source_sha256,
        "revision": f"FORGECAD_VISIBLE_VIEW_POLICY@1:{source_sha256}",
        "thresholds": thresholds,
    }


def metric_gate_results(metrics: Any, thresholds: dict[str, float]) -> dict[str, str]:
    if not isinstance(metrics, dict):
        raise RuntimeError("comparison did not return a metric object")
    results: dict[str, str] = {}
    for metric_name, (direction, threshold_name, _constant_name) in METRIC_CRITERIA.items():
        measured = metrics.get(metric_name)
        threshold = thresholds.get(threshold_name)
        if not isinstance(measured, (int, float)) or not math.isfinite(float(measured)):
            raise RuntimeError(f"comparison metric is invalid: {metric_name}")
        if threshold is None:
            raise RuntimeError(f"Runtime threshold is missing: {threshold_name}")
        results[metric_name] = (
            "PASS"
            if (float(measured) >= threshold if direction == "min" else float(measured) <= threshold)
            else "FAIL"
        )
    return results


def complete_transport_receipt_fields(
    metrics: Any,
    typed_evidence_bindings: dict[str, Any],
    quality_visual_status: Any,
    quality_hard_gate_passed: Any,
    visual_review_status: Any,
    camera_binding_status: str,
    policy: dict[str, Any],
) -> dict[str, Any]:
    """Return explicit truth-binding fields for a completed transport.

    These fields describe evidence completeness only. They deliberately do
    not turn a failed visual candidate, an unrun human/PBR gate, or an absent
    export/restart proof into a pass.
    """
    render_artifacts = typed_evidence_bindings.get("render_pass_artifacts")
    comparison_bindings = typed_evidence_bindings.get("comparison_bindings") or {}
    mask_binding = typed_evidence_bindings.get("mask_binding") or {}
    visual_review = typed_evidence_bindings.get("visual_review") or {}
    quality_readback = typed_evidence_bindings.get("quality_readback") or {}
    thresholds = policy["thresholds"]
    return {
        "thresholds": thresholds,
        "threshold_revision": policy["revision"],
        "threshold_source_sha256": policy["source_sha256"],
        "metric_gate_results": metric_gate_results(metrics, thresholds),
        "receipt_completeness": {
            "artifact_readback_integrity_counters": "PASS_ARTIFACT_READBACK_HASH_AND_VALIDATOR",
            "camera_binding": camera_binding_status,
            "candidate_confirm": "NOT_RUN_EXPLICIT",
            "candidate_state": "PASS_CANDIDATE_BOUND_READBACK",
            "comparison_canonical_vs_object_hashes": "PASS"
            if comparison_bindings.get("comparison_report", {}).get("binding_status") == "PASS"
            else "MISSING",
            "export": "NOT_RUN_EXPLICIT",
            "mask_sha256_and_revision": "PASS"
            if mask_binding.get("binding_status") == "PASS"
            else "MISSING",
            "metric_revision": "ReferenceComparisonReport@1",
            "per_aov_hashes_and_dimensions": "PASS_9_TYPED_RENDER_SET_PASSES"
            if isinstance(render_artifacts, list)
            and len(render_artifacts) == len(AOV_ORDER)
            and all(item.get("binding_status") == "PASS_TYPED_RENDER_SET_PASS_ARTIFACT" for item in render_artifacts)
            else "MISSING",
            "render_canonical_vs_object_hashes": "PASS"
            if comparison_bindings.get("render_set", {}).get("binding_status") == "PASS"
            else "MISSING",
            "restart_hash": "NOT_RUN_EXPLICIT",
            "status": "COMPLETE_TRANSPORT_WITH_QUALITY_TARGET_NOT_MET"
            if quality_visual_status == "QUALITY_TARGET_NOT_MET" and quality_hard_gate_passed is False
            else "INCOMPLETE_TRUTH_BINDING",
            "structured_thresholds": "PASS_RUNTIME_SOURCE_POLICY",
            "threshold_revision": "PASS_RUNTIME_SOURCE_SHA256",
            "visual_review_receipt_hash": "PASS"
            if visual_review.get("binding_status") == "PASS"
            and isinstance(quality_readback.get("object_sha256"), str)
            else "MISSING",
        },
        "downstream_status": {
            "candidate_confirm": "NOT_RUN",
            "export": "NOT_RUN",
            "restart_hash": "NOT_RUN",
            "packaged_reference_visual_e2e": "NOT_RUN",
            "viewer_accessibility_e2e": "NOT_RUN",
        },
        "quality_status_derivation": {
            "metric_gate_results": "RUNTIME_POLICY_BOUND",
            "quality_visual_status": quality_visual_status,
            "quality_hard_gate_passed": quality_hard_gate_passed,
            "visual_review_status": visual_review_status,
        },
    }


def read_bound_viewer_projection(
    executable: Path,
    data_root: Path,
    expected_cohort: str,
    project_id: str,
    candidate_id: str,
    artifact_id: str,
    artifact_sha256: str,
    reference_id: str,
    reference_sha256: str,
    render_set_hash: str,
    comparison_report_hash: str,
) -> dict[str, Any]:
    """Read and verify the packaged Viewer projection while Runtime is live.

    The Viewer remains read-only.  This check is deliberately performed before
    the Codex probe shuts down its isolated Runtime so a structural projection
    cannot be mistaken for a projection over a different candidate or cohort.
    """
    executable = executable.expanduser().resolve()
    if not executable.is_file() or not os.access(executable, os.X_OK):
        raise RuntimeError("packaged Viewer executable is missing or not executable")
    environment = os.environ.copy()
    environment["FORGECAD_RUNTIME_DATA_DIR"] = str(data_root)
    identity = subprocess.run(
        [str(executable), "--build-identity"],
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
        env=environment,
    )
    if identity.returncode != 0:
        raise RuntimeError("packaged Viewer build identity command failed")
    try:
        identity_value = json.loads(identity.stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError(f"packaged Viewer build identity was not JSON: {error}") from error
    viewer_cohort = identity_value.get("build_cohort_sha256") if isinstance(identity_value, dict) else None
    if (
        not isinstance(identity_value, dict)
        or identity_value.get("schema_version") != "ForgeCADDevBuildIdentity@1"
        or identity_value.get("component") != "forgecad-viewer"
        or not isinstance(viewer_cohort, str)
        or len(viewer_cohort) != 64
        or viewer_cohort != expected_cohort
    ):
        raise RuntimeError("packaged Viewer build cohort did not match the live Runtime cohort")

    projection_process = subprocess.run(
        [str(executable), "--viewer-read-model"],
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
        env=environment,
    )
    if projection_process.returncode != 0:
        raise RuntimeError("packaged Viewer read-model command failed")
    try:
        projection = json.loads(projection_process.stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError(f"packaged Viewer read model was not JSON: {error}") from error
    if (
        not isinstance(projection, dict)
        or projection.get("schema_version") != "ForgeCADViewerReadModel@1"
        or projection.get("status") != "Ready"
        or projection.get("retryable") is not False
    ):
        raise RuntimeError("packaged Viewer did not return a Ready read model")

    matching_project: dict[str, Any] | None = None
    matching_candidate: dict[str, Any] | None = None
    project_values = projection.get("projects")
    if isinstance(project_values, list):
        for project_view in project_values:
            if not isinstance(project_view, dict):
                continue
            project = project_view.get("project") or {}
            if project.get("project_id") != project_id:
                continue
            matching_project = project_view
            for candidate_view in project_view.get("candidates") or []:
                if not isinstance(candidate_view, dict):
                    continue
                candidate = candidate_view.get("candidate") or {}
                if candidate.get("candidate_id") == candidate_id:
                    matching_candidate = candidate_view
                    break
            break
    if matching_project is None or matching_candidate is None:
        raise RuntimeError("packaged Viewer read model did not contain the exact project/candidate")

    candidate = matching_candidate.get("candidate") or {}
    artifact = matching_candidate.get("artifact") or {}
    quality_raw = matching_candidate.get("quality") or {}
    quality = quality_raw.get("quality_report") if isinstance(quality_raw, dict) else None
    quality = quality if isinstance(quality, dict) else quality_raw
    reference_raw = matching_candidate.get("reference") or {}
    reference = reference_raw.get("reference") if isinstance(reference_raw, dict) else None
    reference = reference if isinstance(reference, dict) else reference_raw
    binding_pairs = {
        "candidate_id": (candidate.get("candidate_id"), candidate_id),
        "candidate_project_id": (candidate.get("project_id"), project_id),
        "candidate_manifest_hash": (candidate.get("manifest_hash"), artifact_id),
        "artifact_id": (artifact.get("artifact_id"), artifact_id),
        "artifact_candidate_id": (artifact.get("candidate_id"), candidate_id),
        "artifact_object_sha256": (artifact.get("object_sha256"), artifact_sha256),
        "quality_candidate_id": (quality.get("candidate_id"), candidate_id),
        "quality_artifact_sha256": (quality.get("artifact_sha256"), artifact_sha256),
        "quality_reference_id": (quality.get("reference_id"), reference_id),
        "quality_reference_sha256": (quality.get("reference_sha256"), reference_sha256),
        "quality_render_set_hash": (quality.get("render_set_hash"), render_set_hash),
        "quality_comparison_report_hash": (quality.get("comparison_report_hash"), comparison_report_hash),
        "reference_id": (reference.get("reference_id"), reference_id),
        "reference_project_id": (reference.get("project_id"), project_id),
        "reference_object_sha256": (reference.get("object_sha256"), reference_sha256),
    }
    mismatches = [name for name, (actual, expected) in binding_pairs.items() if actual != expected]
    if mismatches:
        raise RuntimeError(f"packaged Viewer lineage binding mismatch: {', '.join(mismatches)}")
    return {
        "status": "PASS_CURRENT_COHORT_BOUND_READ_MODEL",
        "schema_version": projection["schema_version"],
        "build_cohort_sha256": viewer_cohort,
        "project_id": project_id,
        "candidate_id": candidate_id,
        "artifact_id": artifact_id,
        "artifact_sha256": artifact_sha256,
        "reference_id": reference_id,
        "reference_sha256": reference_sha256,
        "render_set_hash": render_set_hash,
        "comparison_report_hash": comparison_report_hash,
        "quality_visual_status": quality.get("visual_status"),
        "quality_hard_gate_passed": quality.get("hard_gate_passed"),
        "binding": "PASS_EXACT_PROJECT_CANDIDATE_ARTIFACT_REFERENCE_RENDERSET_COMPARISON",
        "project_count": len(project_values) if isinstance(project_values, list) else 0,
        "candidate_count": len(matching_project.get("candidates") or []),
        "ui_e2e": "NOT_RUN",
        "persistent_user_data_touched": False,
    }


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


def completed_tool_sequence(items: list[dict[str, Any]]) -> list[str]:
    """Return only successfully completed ForgeCAD calls for a bounded stage.

    A real Codex may emit a failed, protected human-review attempt before it
    follows the recovery prompt.  Failed calls are retained in the receipt,
    but they must not make a later, exact typed review sequence look out of
    order.  The caller still rejects any *successful* human review call.
    """
    return [
        str(call.get("tool"))
        for call in mcp_calls(items)
        if call.get("server") == "forgecad" and call.get("status") == "completed"
    ]


def review_stage_completed(items: list[dict[str, Any]]) -> bool:
    completed = completed_tool_sequence(items)
    return has_subsequence(completed, REVIEW_SEQUENCE) and all_completed(items, REVIEW_SEQUENCE)


def successful_human_review(items: list[dict[str, Any]]) -> bool:
    return any(
        call.get("server") == "forgecad"
        and call.get("tool") == "human_visual_review_submit"
        and call.get("status") == "completed"
        for call in mcp_calls(items)
    )


def run_required_codex_turn(
    options: argparse.Namespace,
    environment: dict[str, str],
    prompt_text: str,
    workspace_root: str,
    expected: tuple[str, ...],
    turn_outputs: list[subprocess.CompletedProcess[str]],
    label: str,
    max_attempts: int = 3,
) -> list[dict[str, Any]]:
    """Run one bounded MCP stage with resumable Codex retries.

    Codex turns are intentionally short, but a real host can still stop after
    a partial tool sequence (or lose a fresh MCP adapter during reconnect).
    Retry the same typed stage a bounded number of times; the Runtime remains
    the authority and every successful call is still checked against the
    declared sequence.  The aggregate is retained so a completed prefix from
    one turn can be paired with a completed suffix from a retry without
    inventing or replaying any Runtime state in the probe.
    """
    aggregate: list[dict[str, Any]] = []
    last_process_error: str | None = None
    for attempt in range(max_attempts):
        retry_note = ""
        if attempt:
            retry_note = (
                "\nThis is a bounded retry of the same stage. A previous turn may have stopped "
                "after a partial sequence; do not explain, recompute hashes, or call unrelated "
                "tools. Complete the exact remaining sequence now.\n"
            )
        try:
            turn = run_codex_turn(options, environment, prompt_text + retry_note, workspace_root)
        except (OSError, subprocess.SubprocessError) as error:
            # A host can fail before emitting JSONL (for example a transient
            # Codex process exit).  Treat that as a bounded retryable stage
            # failure instead of discarding the whole isolated Runtime run.
            last_process_error = str(error)[:500]
            continue
        turn_outputs.append(turn)
        items = event_items(turn.stdout)
        aggregate.extend(items)
        # MCP call IDs can be reused by a fresh Codex retry.  Validate the
        # bounded stage against raw completed events instead of the compact
        # receipt grouping, otherwise a complete retry prefix can be hidden
        # by an earlier failed/partial call with the same logical ID.
        completed = [
            str(item.get("tool"))
            for item in aggregate
            if item.get("type") == "mcp_tool_call"
            and item.get("server") == "forgecad"
            and item.get("status") == "completed"
        ]
        if has_subsequence(completed, expected) and all(name in completed for name in expected):
            return aggregate
    suffix = f"; last process error: {last_process_error}" if last_process_error else ""
    raise RuntimeError(f"Codex did not complete {label} after {max_attempts} bounded attempts{suffix}")


def render_pass_names(items: list[dict[str, Any]]) -> list[str]:
    names: list[str] = []
    for item in items:
        if item.get("type") != "mcp_tool_call" or item.get("tool") != "render_pass_get" or item.get("status") != "completed":
            continue
        arguments = item.get("arguments")
        if isinstance(arguments, dict) and isinstance(arguments.get("pass"), str):
            # A bounded retry may contain a failed prefix followed by a
            # completed copy of the same AOV sequence.  Report the first
            # successful occurrence of each pass in order, while retaining
            # every raw call in the receipt for transport auditing.
            if arguments["pass"] not in names:
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
        tokens: list[str] = []
        if command_text:
            try:
                tokens = [token.rsplit("/", 1)[-1] for token in shlex.split(command_text)[:3]]
            except ValueError:
                tokens = []
        normalized_command = command_text.lower()
        forbidden_tokens = (
            "rm ", "mv ", "cp ", "chmod ", "chown ", "tee ", "install ",
            "python", "node ", "git ", "curl ", "wget ", "http://", "https://",
        )
        # Codex may use a read-only shell pipeline (for example `cat .../SKILL.md
        # | sed ...`) to inspect a first-party Skill.  Pipes and stderr
        # redirection to /dev/null are not writes by themselves; reject actual
        # command chaining, mutating commands, network access, or redirection
        # to a real file before classifying the event as harmless.
        mutating_command = bool(
            re.search(
                r"(?:^|[\s;&|])(?:rm|mv|cp|chmod|chown|tee|install|python(?:\d+(?:\.\d+)*)?|node|git|curl|wget|osascript|open|defaults|xcrun|cargo|npm|make)(?:[\s;&|]|$)",
                normalized_command,
            )
        )
        command_chain = bool(re.search(r";|&&|\|\|", command_text))
        real_file_redirect = bool(re.search(r"(?<![0-9])>{1,2}\s*(?!/dev/null\b)", normalized_command))
        skill_basename_lookup = any(token.casefold() == "skill.md" for token in tokens)
        read_only_skill_lookup = (
            item_type == "command_execution"
            and (
                "skill.md" in normalized_command
                or (
                    ".codex/" in normalized_command
                    and "/skills/" in normalized_command
                    and normalized_command.endswith(".md")
                )
                or skill_basename_lookup
            )
            and not mutating_command
            and not command_chain
            and not real_file_redirect
            and not any(token in normalized_command for token in forbidden_tokens)
        )
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
    prompt_text = (
        "Before any other ForgeCAD tool in this fresh MCP session, call exactly one read-only "
        "preflight tool: skill_get with {\"skill_id\":\"ponytail-preflight\",\"version\":\"0.1.0\"}. "
        "Wait for its successful typed result, then follow the stage instructions below. "
        "This mandatory preflight is not a substitute for any stage call and must be the first "
        "ForgeCAD tool in this session.\n\n"
        + prompt_text
    )
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
            config_override(
                options.mcp_command,
                "approve" if options.auto_approve_isolated_writes else "writes",
            ),
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


SHA256_RE = re.compile(r"^[0-9a-f]{64}$")


def typed_sha256(value: Any) -> str | None:
    """Keep only contract-shaped hashes in the durable evidence projection."""
    return value if isinstance(value, str) and SHA256_RE.fullmatch(value) else None


def structured_payload(item: dict[str, Any]) -> dict[str, Any] | None:
    result = item.get("result")
    if not isinstance(result, dict):
        return None
    structured = result.get("structured_content")
    if not isinstance(structured, dict):
        structured = result.get("structuredContent")
    return structured if isinstance(structured, dict) else None


def completed_structured_results(items: list[dict[str, Any]], tool_name: str) -> list[dict[str, Any]]:
    """Return successful typed payloads without retaining prompts or images."""
    values: list[dict[str, Any]] = []
    for item in items:
        if (
            item.get("type") != "mcp_tool_call"
            or item.get("tool") != tool_name
            or item.get("status") not in (None, "completed")
        ):
            continue
        payload = structured_payload(item)
        if payload is not None:
            values.append(payload)
    return values


def compact_render_set_pass_artifacts(render_set: Any) -> list[dict[str, Any]]:
    """Project RenderSet@2 pass artifacts into hash-only, bounded evidence."""
    pass_artifacts = field(render_set, "pass_artifacts")
    if not isinstance(pass_artifacts, dict):
        return []
    result: list[dict[str, Any]] = []
    for pass_name in AOV_ORDER:
        artifact = pass_artifacts.get(pass_name)
        if not isinstance(artifact, dict):
            continue
        entry: dict[str, Any] = {"pass": pass_name}
        for key in ("mime", "size_bytes", "width", "height", "channels", "color_space"):
            value = artifact.get(key)
            if isinstance(value, (str, int, float, bool)):
                entry[key] = value
        sha256 = typed_sha256(artifact.get("sha256"))
        if sha256 is not None:
            entry["sha256"] = sha256
        entry["binding_status"] = (
            "PASS_TYPED_RENDER_SET_PASS_ARTIFACT"
            if sha256 is not None and all(key in entry for key in ("mime", "size_bytes", "width", "height", "channels", "color_space"))
            else "INCOMPLETE_TYPED_PASS_ARTIFACT"
        )
        result.append(entry)
    return result


def compact_render_pass_readbacks(items: list[dict[str, Any]], render_set_hash: str) -> tuple[list[dict[str, Any]], str]:
    """Bind each render_pass_get readback to the exact RenderSet hash.

    The MCP image block is intentionally not copied.  Only its typed result
    metadata is retained, so the receipt can prove the readback contract
    without becoming an image cache or leaking image bytes.
    """
    by_pass: dict[str, dict[str, Any]] = {}
    image_block_passes: set[str] = set()
    for item in items:
        if (
            item.get("type") != "mcp_tool_call"
            or item.get("tool") != "render_pass_get"
            or item.get("status") not in (None, "completed")
        ):
            continue
        payload = structured_payload(item) or {}
        arguments = item.get("arguments")
        argument_pass = arguments.get("pass") if isinstance(arguments, dict) else None
        pass_name = payload.get("pass") if isinstance(payload.get("pass"), str) else argument_pass
        if pass_name not in AOV_ORDER or pass_name in by_pass:
            continue
        entry: dict[str, Any] = {"pass": pass_name}
        for key in ("sha256", "mime", "width", "height", "render_set_hash", "candidate_id"):
            value = payload.get(key)
            if key == "sha256":
                value = typed_sha256(value)
            if isinstance(value, (str, int, float, bool)):
                entry[key] = value
        entry["expected_render_set_hash"] = render_set_hash
        entry["render_set_binding"] = (
            "PASS"
            if entry.get("render_set_hash") == render_set_hash
            else "FAIL_OR_NOT_EMITTED"
        )
        result = item.get("result")
        content = result.get("content") if isinstance(result, dict) else None
        if isinstance(content, list):
            image_blocks = [
                block
                for block in content
                if isinstance(block, dict) and block.get("type") == "image"
            ]
            if image_blocks:
                image_block_passes.add(pass_name)
                entry["image_block_count"] = len(image_blocks)
                entry["image_block_mime_types"] = sorted(
                    {
                        str(block.get("mimeType"))
                        for block in image_blocks
                        if isinstance(block.get("mimeType"), str)
                    }
                )
                entry["image_block_nonempty"] = all(
                    isinstance(block.get("data"), str) and bool(block.get("data"))
                    for block in image_blocks
                )
        by_pass[pass_name] = entry
    ordered = [by_pass[name] for name in AOV_ORDER if name in by_pass]
    image_status = (
        "PASS_TYPED_IMAGE_BLOCKS_OBSERVED"
        if len(image_block_passes) == len(AOV_ORDER)
        else "PARTIAL_TYPED_IMAGE_BLOCKS_OBSERVED"
        if image_block_passes
        else "NOT_OBSERVED_IN_SANITIZED_CLI_EVENTS"
    )
    return ordered, image_status


def compact_object_binding(value: Any, object_sha256: Any = None) -> dict[str, Any]:
    """Return a stable identity binding for a nested contract object."""
    result: dict[str, Any] = {}
    if isinstance(value, dict):
        for key in ("schema_version", "camera_hash", "canonical_sha256", "render_set_hash", "comparison_report_hash", "status"):
            candidate = value.get(key)
            if key.endswith("sha256") or key.endswith("_hash"):
                candidate = typed_sha256(candidate)
            if isinstance(candidate, (str, int, float, bool)):
                result[key] = candidate
    object_hash = typed_sha256(object_sha256)
    if object_hash is not None:
        result["object_sha256"] = object_hash
    result["binding_status"] = "PASS" if object_hash is not None else "MISSING_OBJECT_HASH"
    return result


def build_typed_evidence_bindings(
    comparison: dict[str, Any],
    artifact_readback: dict[str, Any],
    render_items: list[dict[str, Any]],
    review: dict[str, Any],
    quality: dict[str, Any],
    render_set_hash: str,
) -> dict[str, Any]:
    """Build the candidate-bound evidence projection used by later stages."""
    render_set = field(comparison, "render_set") or {}
    comparison_report = field(comparison, "comparison_report") or {}
    quality_report = field(comparison, "quality_report") or {}
    review_report = field(review, "review") or {}
    render_pass_artifacts = compact_render_set_pass_artifacts(render_set)
    render_pass_readbacks, image_block_status = compact_render_pass_readbacks(render_items, render_set_hash)
    artifact_binding = {
        key: artifact_readback.get(key)
        for key in (
            "schema_version",
            "artifact_id",
            "candidate_id",
            "object_sha256",
            "program_sha256",
            "triangle_count",
            "validator_status",
        )
        if isinstance(artifact_readback.get(key), (str, int, float, bool))
    }
    artifact_binding["object_sha256"] = typed_sha256(artifact_readback.get("object_sha256"))
    artifact_binding["binding_status"] = "PASS" if artifact_binding["object_sha256"] is not None else "MISSING_OBJECT_HASH"
    return {
        "binding_schema": "ForgeCADMCP010CReceiptBinding@1",
        "artifact_readback": artifact_binding,
        "comparison_bindings": {
            "camera": compact_object_binding(
                field(comparison, "camera") or comparison_report,
                field(comparison, "camera_object_sha256"),
            ),
            "render_set": compact_object_binding(
                render_set,
                field(comparison, "render_set_object_sha256") or field(comparison, "render_set_hash"),
            ),
            "comparison_report": compact_object_binding(
                comparison_report,
                field(comparison, "comparison_report_object_sha256") or field(comparison, "comparison_report_hash"),
            ),
            "quality_report": compact_object_binding(
                quality_report,
                field(comparison, "quality_report_object_sha256") or field(quality, "quality_report_object_sha256"),
            ),
        },
        "mask_binding": {
            key: field(comparison_report, "mask", key)
            for key in ("method", "revision", "width", "height")
            if isinstance(field(comparison_report, "mask", key), (str, int, float, bool))
        }
        | {
            "sha256": typed_sha256(field(comparison_report, "mask", "sha256")),
            "binding_status": "PASS"
            if typed_sha256(field(comparison_report, "mask", "sha256")) is not None
            else "MISSING_MASK_HASH",
        },
        "metric_binding": {
            "schema_version": comparison_report.get("schema_version"),
            "metrics": field(comparison_report, "metrics"),
            "threshold_binding": "NOT_EMITTED_BY_QUALITY_REPORT_V2",
        },
        "render_pass_artifacts": render_pass_artifacts,
        "render_pass_readbacks": render_pass_readbacks,
        "render_pass_image_blocks": image_block_status,
        "visual_review": {
            "object_sha256": typed_sha256(field(review, "review_object_sha256")),
            "canonical_sha256": typed_sha256(field(review_report, "canonical_sha256")),
            "status": review_report.get("status"),
            "binding_status": "PASS"
            if typed_sha256(field(review, "review_object_sha256")) is not None
            else "MISSING_REVIEW_OBJECT_HASH",
        },
        "quality_readback": {
            "object_sha256": typed_sha256(field(comparison, "quality_report_object_sha256")),
            "canonical_sha256": typed_sha256(field(quality_report, "canonical_sha256") or field(quality, "canonical_sha256")),
            "visual_status": field(quality_report, "visual_status") or field(quality, "visual_status"),
            "hard_gate_passed": field(quality_report, "hard_gate_passed") if "hard_gate_passed" in quality_report else field(quality, "hard_gate_passed"),
        },
        "downstream_status": {
            "candidate_confirm": "NOT_RUN",
            "export": "NOT_RUN",
            "restart_hash": "NOT_RUN",
            "packaged_reference_visual_e2e": "NOT_RUN",
            "viewer_accessibility_e2e": "NOT_RUN",
        },
    }


def reference_dimensions(path: Path) -> tuple[int, int]:
    data = path.read_bytes()
    if not data.startswith(b"\x89PNG\r\n\x1a\n") or len(data) < 24:
        raise ValueError("the real Codex C probe currently accepts a PNG reference with a readable IHDR")
    width = int.from_bytes(data[16:20], "big")
    height = int.from_bytes(data[20:24], "big")
    if not (1 <= width <= 8192 and 1 <= height <= 8192):
        raise ValueError("reference dimensions exceed Runtime bounds")
    return width, height


def load_visual_intake(path: Path | None, reference_sha: str) -> tuple[dict[str, list[dict[str, Any]]], str | None]:
    if path is None:
        return {"landmarks": [], "regions": []}, None
    if not path.is_file() or path.is_symlink():
        raise ValueError("visual intake must be a regular JSON file")
    raw = path.read_bytes()
    value = json.loads(raw.decode("utf-8"))
    if not isinstance(value, dict) or value.get("schema_version") not in {"ForgeCADRobotReferenceIntake@1", "ForgeCADCodexReferenceInventory@1"}:
        raise ValueError("visual intake schema_version is unsupported")
    if value.get("reference_sha256") != reference_sha and field(value, "reference", "reference_sha256") != reference_sha:
        raise ValueError("visual intake reference_sha256 does not match reference_import")
    landmarks = value.get("landmarks")
    regions = value.get("regions")
    if not isinstance(landmarks, list) or not isinstance(regions, list):
        raise ValueError("visual intake must contain landmarks and regions arrays")
    if len(landmarks) > 128 or len(regions) > 64:
        raise ValueError("visual intake exceeds ReferenceViewSpec bounds")
    # Copy only the closed ReferenceViewSpec leaves.  Inventory metadata such as
    # feature/operator/material names must never leak into the Runtime request.
    allowed_landmark = {"landmark_id", "x", "y", "visibility", "confidence"}
    allowed_region = {"region_id", "x", "y", "width", "height", "visibility", "confidence"}
    clean_landmarks: list[dict[str, Any]] = []
    clean_regions: list[dict[str, Any]] = []
    for item in landmarks:
        if not isinstance(item, dict) or set(item) != allowed_landmark:
            raise ValueError("visual intake landmark has non-ReferenceViewSpec fields")
        clean_landmarks.append({key: item[key] for key in sorted(allowed_landmark)})
    for item in regions:
        if not isinstance(item, dict) or set(item) != allowed_region:
            raise ValueError("visual intake region has non-ReferenceViewSpec fields")
        clean_regions.append({key: item[key] for key in sorted(allowed_region)})
    return {"landmarks": clean_landmarks, "regions": clean_regions}, hashlib.sha256(raw).hexdigest()


def view_spec(reference_id: str, reference_sha: str, width: int, height: int, intake: dict[str, list[dict[str, Any]]] | None = None) -> dict[str, Any]:
    intake = intake or {"landmarks": [], "regions": []}
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
        "landmarks": intake["landmarks"],
        "regions": intake["regions"],
        "canonical_sha256": "",
    }
    value["canonical_sha256"] = canonical_hash(value)
    return value


def setup_prompt(reference_path: str) -> str:
    return f"""Use only the ForgeCAD MCP server. Do not use shell, filesystem, browser, other MCP servers, or arbitrary code.

This is the first setup turn for a real MCP010C host gate. The mandatory Ponytail preflight has already been requested by the host wrapper. After it succeeds, call exactly these three ForgeCAD tools, in order, then stop:
1) project_create with name=\"MCP010C Codex visual review\" and policy={{\"profile\":\"mvp\"}}; save project_id.
2) reference_import with that project_id, source={{\"kind\":\"codex_local_file\",\"path\":{json.dumps(reference_path, ensure_ascii=False)}}}, authorization={{\"user_authorized\":true,\"declaration\":\"The user supplied and authorized this reference for local ForgeCAD modeling.\"}}; save reference_id and object_sha256.
3) reference_get with that reference_id; verify the returned reference_id and object_sha256 match the import result. Do not request or print image bytes.

Do not call any other ForgeCAD tool in this turn. Return only project_id, reference_id and the metadata readback, then stop. Do not claim similarity, high quality, PBR, human approval or 360-degree coverage.
"""


def reference_get_prompt(reference_id: str) -> str:
    return f"""Use only the ForgeCAD MCP server. Call exactly one tool, then stop:
reference_get with {{\"reference_id\":{json.dumps(reference_id)}}}. Verify the returned reference_id and object_sha256, but do not request or print image bytes. Do not call any other tool or claim visual quality.
"""


def silhouette_target_prompt(project_id: str, reference_id: str, intake: dict[str, list[dict[str, Any]]]) -> str:
    # The intake file carries confidence for the broader ReferenceViewSpec, but
    # SilhouetteTarget@1 intentionally accepts only the four typed landmark
    # leaves.  Strip confidence here so Codex sends the exact closed target
    # shape rather than making Runtime guess or silently dropping annotations.
    landmarks = [
        {
            key: item[key]
            for key in ("landmark_id", "x", "y", "visibility")
        }
        for item in intake.get("landmarks", [])
    ]
    return f"""Use only the ForgeCAD MCP server. Call exactly one tool, then stop:
reference_mask_prepare with {json.dumps({"project_id": project_id, "reference_id": reference_id, "landmarks": landmarks, "parts": []}, ensure_ascii=False, separators=(",", ":"))}.
The landmarks above are the user-authorized, image-derived visible landmarks. Copy them exactly; do not invent contour points, hidden-side annotations, parts or extra landmarks. Let Runtime create its deterministic automatic starting mask. Save the returned target_sha256 and do not call any other ForgeCAD tool in this turn. Do not claim likeness or high quality.
"""


def silhouette_rig_draft(candidate_id: str) -> dict[str, Any]:
    return {
        "schema_version": "SilhouetteRig@1",
        "rig_id": "robot-silhouette-rig",
        "candidate_id": candidate_id,
        "parameters": [
            # Probe broad image-plane envelopes first.  These IDs bind to the
            # actual semantic sinks in the detail program; mirrored sinks are
            # traced back to one shared source by Runtime before compiling a
            # trial, so the search cannot drift left/right.
            {"parameter_id": "head-width", "part_id": "head-shell", "semantic": "width", "value": 1.0, "min": 0.82, "max": 1.18, "step": 0.04, "unit": "ratio"},
            {"parameter_id": "head-height", "part_id": "head-shell", "semantic": "height", "value": 1.0, "min": 0.82, "max": 1.18, "step": 0.04, "unit": "ratio"},
            {"parameter_id": "chest-width", "part_id": "chest-shell", "semantic": "width", "value": 1.0, "min": 0.82, "max": 1.18, "step": 0.04, "unit": "ratio"},
            {"parameter_id": "chest-height", "part_id": "chest-shell", "semantic": "height", "value": 1.0, "min": 0.82, "max": 1.18, "step": 0.04, "unit": "ratio"},
            {"parameter_id": "shoulder-width", "part_id": "shoulder-armor-pair", "semantic": "width", "value": 1.0, "min": 0.84, "max": 1.16, "step": 0.04, "unit": "ratio"},
            {"parameter_id": "shoulder-height", "part_id": "shoulder-armor-pair", "semantic": "height", "value": 1.0, "min": 0.84, "max": 1.16, "step": 0.04, "unit": "ratio"},
            {"parameter_id": "upper-arm-width", "part_id": "upper-arm-pair", "semantic": "width", "value": 1.0, "min": 0.84, "max": 1.16, "step": 0.04, "unit": "ratio"},
            {"parameter_id": "forearm-width", "part_id": "forearm-pair", "semantic": "width", "value": 1.0, "min": 0.84, "max": 1.16, "step": 0.04, "unit": "ratio"},
            {"parameter_id": "pelvis-width", "part_id": "pelvis", "semantic": "width", "value": 1.0, "min": 0.84, "max": 1.16, "step": 0.04, "unit": "ratio"},
            {"parameter_id": "pelvis-height", "part_id": "pelvis", "semantic": "height", "value": 1.0, "min": 0.88, "max": 1.12, "step": 0.04, "unit": "ratio"},
            {"parameter_id": "thigh-width", "part_id": "thigh-pair", "semantic": "width", "value": 1.0, "min": 0.84, "max": 1.16, "step": 0.04, "unit": "ratio"},
            {"parameter_id": "shin-width", "part_id": "shin-pair", "semantic": "width", "value": 1.0, "min": 0.84, "max": 1.16, "step": 0.04, "unit": "ratio"},
        ],
    }


def silhouette_prompt(project_id: str, candidate_id: str, target_sha256: str) -> str:
    rig = json.dumps(silhouette_rig_draft(candidate_id), ensure_ascii=False, separators=(",", ":"))
    return f"""Use only the ForgeCAD MCP server. Do not use shell, filesystem, browser, other MCP servers or arbitrary code.

Call exactly these four ForgeCAD tools in order, then stop:
1) silhouette_target_get with {{"target_sha256":{json.dumps(target_sha256)}}}; verify it is the target for project {json.dumps(project_id)}.
2) scene_observe_get with {{"project_id":{json.dumps(project_id)},"candidate_id":{json.dumps(candidate_id)}}}; treat the complete returned AgenticSceneObserveResult@1 as the only canonical scene/model/reference/quality context for this visual turn. Verify its project/candidate binding, read_only flag and canonical_sha256; do not replace it with a sequence of fragmented project/candidate/artifact/quality reads.
3) camera_fit_prepare with {{"project_id":{json.dumps(project_id)},"candidate_id":{json.dumps(candidate_id)},"target_sha256":{json.dumps(target_sha256)},"camera":null}}; use the one-shot observation as context and save the complete returned selected_camera calibration object (not only camera_hash/canonical_sha256) and its hash.
4) silhouette_rig_hash with {{"schema_version":"SilhouetteRigHashRequest@1","project_id":{json.dumps(project_id)},"candidate_id":{json.dumps(candidate_id)},"rig_draft":{rig}}}; copy only the returned canonical_sha256 into the unchanged rig for the next turn.

Do not call silhouette_fit_prepare yet because the next turn will bind its request hash to the exact selected camera. Do not call geometry, appearance, compare, confirm or export. Return only target/camera/Rig hashes and opaque IDs; do not claim visual quality.
"""


def silhouette_fit_prompt(fit_request: dict[str, Any]) -> str:
    request_json = json.dumps(fit_request, ensure_ascii=False, separators=(",", ":"))
    return f"""Use only the ForgeCAD MCP server. Call exactly one ForgeCAD tool, then stop.
The MCP envelope is the hash-bound SilhouetteFitIntent payload; do not add a schema_version field (the Runtime dispatch contract intentionally does not accept one), do not stringify the optimizer object, and do not remove any field shown below:
silhouette_fit_prepare with this exact JSON object: {request_json}

Do not change any value, recompute any hash, call another tool, or claim that the model matches the reference. Return only the actual iterations, evaluations, selected camera/parameters, loss and status.
"""


def single_part_optimization_rig_draft(candidate_id: str, part_id: str = "chest-shell") -> dict[str, Any]:
    """Build the bounded CADFit search space for one semantic Part.

    The global silhouette rig is intentionally not reused here: OptimizationJob
    is a local Part job and Runtime rejects mixed-Part parameter sets.  Width,
    height, depth and image-plane offsets give the optimizer a useful six
    dimensional envelope while remaining typed, deterministic and bounded.
    Offset parameters use meters because the Runtime's ratio path requires a
    positive scale and therefore cannot represent a negative image-plane delta.
    """
    return {
        "schema_version": "SilhouetteRig@1",
        "rig_id": "robot-chest-shell-cadfit-rig",
        "candidate_id": candidate_id,
        "parameters": [
            {"parameter_id": "chest-shell-width", "part_id": part_id, "semantic": "width", "value": 1.0, "min": 0.72, "max": 1.28, "step": 0.04, "unit": "ratio"},
            {"parameter_id": "chest-shell-height", "part_id": part_id, "semantic": "height", "value": 1.0, "min": 0.72, "max": 1.28, "step": 0.04, "unit": "ratio"},
            {"parameter_id": "chest-shell-depth", "part_id": part_id, "semantic": "depth", "value": 1.0, "min": 0.80, "max": 1.20, "step": 0.04, "unit": "ratio"},
            {"parameter_id": "chest-shell-offset-x", "part_id": part_id, "semantic": "offset_x", "value": 0.0, "min": -0.35, "max": 0.35, "step": 0.05, "unit": "meter"},
            {"parameter_id": "chest-shell-offset-y", "part_id": part_id, "semantic": "offset_y", "value": 0.0, "min": -0.35, "max": 0.35, "step": 0.05, "unit": "meter"},
            {"parameter_id": "chest-shell-scale", "part_id": part_id, "semantic": "scale", "value": 1.0, "min": 0.85, "max": 1.15, "step": 0.05, "unit": "ratio"},
        ],
    }


def cadfit_rig_hash_prompt(project_id: str, candidate_id: str, rig: dict[str, Any]) -> str:
    rig_draft = {key: value for key, value in rig.items() if key != "canonical_sha256"}
    request = {
        "schema_version": "SilhouetteRigHashRequest@1",
        "project_id": project_id,
        "candidate_id": candidate_id,
        "rig_draft": rig_draft,
    }
    return f"""Use only the ForgeCAD MCP server. Call exactly one read-only tool, then stop:
silhouette_rig_hash with this exact JSON object: {json.dumps(request, ensure_ascii=False, separators=(',', ':'))}
Return only the Runtime-owned canonical_sha256. Do not call optimization, geometry, compare, confirm or export, and do not claim visual quality.
"""


def cadfit_optimization_intent(
    project_id: str,
    candidate_id: str,
    reference_id: str,
    reference_sha256: str,
    program_sha256: str,
    target_sha256: str,
    camera: dict[str, Any],
    rig: dict[str, Any],
) -> dict[str, Any]:
    """Create the exact standalone OptimizationIntent sent through Codex MCP."""
    intent: dict[str, Any] = {
        "schema_version": "OptimizationIntent@1",
        "intent_id": "real-codex-cadfit-chest-shell-intent",
        "job_id": "real-codex-cadfit-chest-shell-job",
        "project_id": project_id,
        "candidate_id": candidate_id,
        "reference_id": reference_id,
        "reference_sha256": reference_sha256,
        "program_sha256": program_sha256,
        "target_sha256": target_sha256,
        "camera": camera,
        "camera_hash": camera.get("camera_hash"),
        "part_id": "chest-shell",
        "stage": "primary-form",
        "rig": rig,
        "fidelity": {
            "coarse_resolution": 128,
            "mid_resolution": 256,
            "final_resolution": 512,
            "coarse_evaluations": CADFIT_COARSE_EVALUATIONS,
            "mid_top_k": CADFIT_MID_TOP_K,
            "final_top_k": CADFIT_FINAL_TOP_K,
        },
        "budget": {
            "max_evaluations": 42,
            "max_runtime_ms": 120000,
            "max_output_triangles": 250000,
            "max_worker_memory_bytes": 536870912,
        },
        "objective": {
            "silhouette_iou": 0.40,
            "boundary_f1_4px": 0.30,
            "landmark_coverage": 0.10,
            "landmark_nme": 0.10,
            "part_region": 0.05,
            "program_complexity": 0.05,
        },
        "canonical_sha256": "",
    }
    intent["canonical_sha256"] = canonical_hash(intent)
    return intent


def cadfit_prepare_prompt(request: dict[str, Any]) -> str:
    return f"""Use only the ForgeCAD MCP server. Call exactly one approved write tool, then stop:
optimization_job_prepare with this exact JSON object: {json.dumps(request, ensure_ascii=False, separators=(',', ':'))}
This is a standalone Runtime-owned CADFit proposal for one Part. Preserve every field and hash exactly. Return only the typed OptimizationJobResult@1 status and hashes. Do not call optimization_job_get in this turn, do not confirm a candidate, do not export, and do not claim likeness or high quality.
"""


def cadfit_get_prompt(project_id: str, candidate_id: str, job_id: str) -> str:
    request = {"project_id": project_id, "candidate_id": candidate_id, "job_id": job_id}
    return f"""Use only the ForgeCAD MCP server. Call exactly one read-only tool, then stop:
optimization_job_get with this exact JSON object: {json.dumps(request, ensure_ascii=False, separators=(',', ':'))}
Return only the latest typed job/result/checkpoint status. Do not call another tool, confirm a candidate, export, or claim visual quality.
"""


def compact_cadfit_result(payload: dict[str, Any] | None) -> dict[str, Any]:
    payload = payload or {}
    job = field(payload, "job") or {}
    result = field(payload, "result") or {}
    return {
        "schema_version": payload.get("schema_version"),
        "job_status": field(job, "status"),
        "job_progress": field(job, "progress"),
        "result_status": field(result, "status"),
        "search_strategy": field(result, "search_strategy"),
        "evaluations_count": field(result, "evaluations_count"),
        "fidelity_counts": field(result, "fidelity_counts"),
        "checkpoint_sequence": field(result, "checkpoint_sequence"),
        "next_stage": field(result, "next_stage"),
        "proposal_status": field(result, "proposal_status"),
        "strict_improvement": field(result, "strict_improvement"),
        "baseline_loss": field(result, "baseline_loss"),
        "best_loss": field(result, "best_loss"),
        "best_evaluation_id": field(result, "best_evaluation_id"),
        "best_evaluation_fidelity": field(result, "best_evaluation_fidelity"),
        "best_program_object_sha256": typed_sha256(field(result, "best_program_object_sha256")),
        "best_artifact_sha256": typed_sha256(field(result, "best_artifact_sha256")),
        "proposal_program_object_sha256": typed_sha256(field(result, "proposal_program_object_sha256")),
        "proposal_artifact_sha256": typed_sha256(field(result, "proposal_artifact_sha256")),
        "result_object_sha256": typed_sha256(field(payload, "result_object_sha256")),
    }


def normalize_numeric_representation(value: Any) -> Any:
    """Use the JSON wire spelling Codex emits for integral floats.

    Codex/serde may serialize ``1.0`` as ``1`` after a model copies a
    response.  The Runtime still binds the complete request; this helper only
    makes the probe hash the representation that will actually cross MCP.
    Fractional values remain unchanged.
    """
    if isinstance(value, bool) or value is None or isinstance(value, str):
        return value
    if isinstance(value, float) and math.isfinite(value) and value.is_integer():
        return int(value)
    if isinstance(value, (int, float)):
        return value
    if isinstance(value, list):
        return [normalize_numeric_representation(item) for item in value]
    if isinstance(value, dict):
        return {key: normalize_numeric_representation(item) for key, item in value.items()}
    return value


def boundary_error_prompt(project_id: str, candidate_id: str, target_sha256: str) -> str:
    return f"""Use only the ForgeCAD MCP server. Call exactly one tool, then stop:
boundary_error_get with {{"candidate_id":{json.dumps(candidate_id)},"target_sha256":{json.dumps(target_sha256)},"max_segments":16}}.
This is candidate-bound evidence for project {json.dumps(project_id)}. Return only the largest directional segments and their Part IDs. Do not edit geometry or claim a likeness pass.
"""


def authoring_draft(geometry_route: str, project_id: str, geometry_variant: str, material_variant: str) -> tuple[dict[str, Any], str]:
    if geometry_route == "detail":
        draft_value = robot_detail_program_draft(
            project_id,
            "<copy-exact-live-catalog-hash>",
            geometry_variant,
            material_variant,
        )
        route_instructions = (
            "This is the bounded MCP010D detail route. After discovery, require the active catalog entries needed by the draft "
            "(profile-loft@1, panel@1, vent-array@1, revolve@1, joint-stack@1, mirror@1, transform@2, tube-sweep@1 and array@1) "
            "and fail closed if any required operator is unavailable. Preserve the draft's semantic Part sinks and ordered inputs."
        )
    else:
        draft_value = robot_v2_program_draft(project_id, "<copy-exact-live-catalog-hash>")
        route_instructions = "Use only the primitive@2 blockout route and do not invent or call unavailable detail operators."
    return draft_value, route_instructions


def authoring_prompt(project_id: str, reference_id: str, geometry_route: str, geometry_variant: str, material_variant: str) -> str:
    _draft_value, route_instructions = authoring_draft(
        geometry_route, project_id, geometry_variant, material_variant
    )
    return f"""Use only the ForgeCAD MCP server. Do not use shell, filesystem, browser, images, other MCP servers or arbitrary code.

The project_id is {json.dumps(project_id)} and the authorized reference_id is {json.dumps(reference_id)}. This is the discovery-only turn. Call exactly these five ForgeCAD tools in order, then stop:
1) capabilities_get; require Runtime Ready and save operator_catalog_sha256.
2) runtime_status; require Ready.
3) doctor; require no terminal Runtime failure.
4) operator_catalog_get; require canonical_sha256 exactly equals step 1 and verify the operator entries required by the selected route below.
5) skill_list; record the live status, but do not call an unavailable operator.

{route_instructions}

Do not call geometry_program_hash, geometry_prepare, appearance, compare, confirm or export in this turn. Return only the catalog hash and concise availability evidence. Do not claim visual quality.
"""


def authoring_hash_prepare_prompt(
    project_id: str,
    reference_id: str,
    geometry_route: str,
    geometry_variant: str,
    material_variant: str,
    catalog_hash: str,
) -> str:
    draft_value, route_instructions = authoring_draft(
        geometry_route, project_id, geometry_variant, material_variant
    )
    draft = json.dumps(draft_value, ensure_ascii=False, separators=(",", ":"))
    return f"""Use only the ForgeCAD MCP server. Do not use shell, filesystem, browser, images, other MCP servers, or local hash code.

The project_id is {json.dumps(project_id)} and the authorized reference_id is {json.dumps(reference_id)}. The discovery turn returned this exact operator catalog hash: {json.dumps(catalog_hash)}. Call exactly these two ForgeCAD tools in order, then stop:
1) geometry_program_hash with {{\"schema_version\":\"GeometryProgramHashRequest@1\",\"geometry_program_draft\":<draft below>}}. Replace only the catalog placeholder with the exact hash above. Do not add canonical_sha256 before this call.
2) geometry_prepare with {{\"project_id\":{json.dumps(project_id)},\"request\":{{\"typed\":\"geometry\",\"reference_id\":{json.dumps(reference_id)},\"geometry_program\":<same draft plus only the returned canonical_sha256>}}}}. Save the complete candidate, job and artifact objects.

{route_instructions}

Do not call any other ForgeCAD tool. Do not confirm, export, or submit visual evidence in this turn. Return only the hash/catalog binding and opaque IDs/counts. Do not claim visual quality.

Hash-free GeometryProgram@2 draft:
{draft}
"""


def compare_prompt(
    project_id: str,
    reference_id: str,
    candidate_id: str,
    job_id: str,
    artifact_id: str,
    view: dict[str, Any],
    camera: dict[str, Any] | None = None,
    target_sha256: str | None = None,
) -> str:
    view_json = json.dumps(view, ensure_ascii=False, separators=(",", ":"))
    if camera is None:
        camera_clause = "null"
        target_clause = ""
    else:
        camera_ref = {
            "schema_version": "CameraCalibrationRef@1",
            "camera_hash": camera.get("camera_hash"),
            "canonical_sha256": camera.get("canonical_sha256"),
        }
        camera_clause = json.dumps(camera_ref, ensure_ascii=False, separators=(",", ":"))
        if not isinstance(target_sha256, str) or len(target_sha256) != 64:
            raise ValueError("camera-bound comparison requires silhouette target_sha256")
        target_clause = f",\"target_sha256\":{json.dumps(target_sha256)}"
    camera_instruction = (
        "The camera below is a Runtime-owned CameraCalibrationRef@1 selected by the bounded silhouette fit. "
        "Copy its two hashes byte-for-byte; Runtime will resolve the exact calibration from the target. "
        "Do not expand it into a full camera or reconstruct fields."
        if camera is not None
        else "No camera calibration was selected in this run; pass camera=null and let Runtime use its bounded compatibility framing."
    )
    return f"""Use the ForgeCAD MCP server now. Make only these four calls, in order, then stop. Do not explain or use another tool.

Use these exact opaque values; do not rewrite them:
1) job_get with {{\"job_id\":{json.dumps(job_id)}}}.
2) candidate_get with {{\"candidate_id\":{json.dumps(candidate_id)}}}.
3) artifact_readback_get with {{\"artifact_id\":{json.dumps(artifact_id)},\"candidate_id\":{json.dumps(candidate_id)}}}.
4) reference_compare_prepare with this exact JSON object: {{\"project_id\":{json.dumps(project_id)},\"candidate_id\":{json.dumps(candidate_id)},\"reference_id\":{json.dumps(reference_id)},\"view_spec\":{view_json},\"camera\":{camera_clause}{target_clause}}}. {camera_instruction} Copy the view_spec byte-for-byte, including canonical_sha256.

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


def review_recovery_prompt(candidate_id: str, reference_id: str, render_set_hash: str, comparison_hash: str) -> str:
    """Recover one bounded source-review turn after a Codex tool-choice drift."""
    return f"""The previous review turn did not produce the required source review. This is a bounded recovery turn. Use only the ForgeCAD MCP server and make exactly these two calls, in order, then stop:
1) visual_review_submit with {{\"candidate_id\":{json.dumps(candidate_id)},\"reference_id\":{json.dumps(reference_id)},\"render_set_hash\":{json.dumps(render_set_hash)},\"comparison_report_hash\":{json.dumps(comparison_hash)},\"round\":1,\"stage\":\"silhouette\",\"issues\":[{{\"issue_id\":\"source-review-recovery\",\"pass\":\"silhouette\",\"region_id\":\"whole-body\",\"claim\":\"The current candidate is a structural blockout and the visible silhouette target is not yet met.\",\"confidence\":0.98,\"visibility\":\"observed\",\"action\":\"Keep the evidence and request a bounded contour revision.\"}}],\"status\":\"needs_revision\"}}.
2) quality_get with {{\"candidate_id\":{json.dumps(candidate_id)},\"reference_id\":{json.dumps(reference_id)}}}.
Do not call human_visual_review_submit, candidate_confirm, export, render_pass_get, or any other tool. Return only the typed review and persisted quality status. Do not claim likeness, human approval, or high quality."""


def base_receipt(source_sha: str, source_size: int) -> dict[str, Any]:
    return {
        "schema_version": "ForgeCADMCP010CCodexCliProbe@1",
        "recorded_at": datetime.now(timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z"),
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
    # CADFit is intentionally downstream of the target/camera gate.  Make the
    # dependency explicit for callers instead of allowing a job with no
    # candidate-bound visual target or camera to be submitted.
    if options.cadfit_optimization:
        options.silhouette_first = True
    source = Path(options.reference).expanduser()
    if not source.is_file() or source.is_symlink():
        receipt = base_receipt("", 0) | {"status": "BLOCKED", "reason": "reference is not a regular file"}
        write_receipt(options.evidence, receipt)
        print(json.dumps(receipt, sort_keys=True))
        return 3
    source = source.resolve()
    source_bytes = source.read_bytes()
    source_sha = hashlib.sha256(source_bytes).hexdigest()
    try:
        width, height = reference_dimensions(source)
    except ValueError as error:
        receipt = base_receipt(source_sha, len(source_bytes)) | {"status": "BLOCKED", "reason": str(error)}
        write_receipt(options.evidence, receipt)
        print(json.dumps(receipt, sort_keys=True))
        return 3
    try:
        visual_intake, visual_intake_sha = load_visual_intake(options.intake, source_sha)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        receipt = base_receipt(source_sha, len(source_bytes)) | {"status": "BLOCKED", "reason": f"visual intake unavailable: {str(error)[:240]}"}
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
    # Codex runs each turn from an ephemeral directory.  Keep the resolved
    # paths in the options object used by run_codex_turn; otherwise a relative
    # command works for the host wrapper but fails when the MCP session starts
    # inside that temporary directory.
    options.runtime_command = runtime_command
    options.mcp_command = mcp_command
    worker_command = str(Path(runtime_command).with_name("forgecad-geometry-worker"))
    viewer_command = options.viewer_executable.expanduser().resolve() if options.viewer_executable else None
    if (
        options.timeout <= 0
        or not Path(runtime_command).is_file()
        or not Path(mcp_command).is_file()
        or not Path(worker_command).is_file()
        or viewer_command is not None
        and (not viewer_command.is_file() or not os.access(viewer_command, os.X_OK))
    ):
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
    try:
        policy = runtime_visible_view_policy(Path(__file__).resolve().parents[1])
    except (OSError, RuntimeError) as error:
        receipt = base_receipt(source_sha, len(source_bytes)) | {
            "status": "BLOCKED",
            "reason": f"Runtime visible-view policy unavailable: {str(error)[:240]}",
            "build_cohorts": cohorts,
        }
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
    silhouette_fit_intent_sha: str | None = None
    cadfit_rig_sha: str | None = None
    cadfit_intent_sha: str | None = None
    cadfit_job_id: str | None = None
    cadfit_result: dict[str, Any] | None = None
    cadfit_items: list[dict[str, Any]] = []
    partial_evidence: dict[str, Any] = {}
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

            silhouette_target_sha: str | None = None
            silhouette_camera_hash: str | None = None
            silhouette_fit_camera_hash: str | None = None
            silhouette_fit_camera_canonical: str | None = None
            silhouette_observation_sha: str | None = None
            comparison_camera_hash: str | None = None
            comparison_camera_canonical: str | None = None
            silhouette_rig_sha: str | None = None
            silhouette_fit_result: dict[str, Any] | None = None
            selected_camera_for_compare: dict[str, Any] | None = None
            silhouette_items: list[dict[str, Any]] = []
            if options.silhouette_first:
                target_turn = run_codex_turn(
                    options,
                    environment,
                    silhouette_target_prompt(project_id, reference_id, visual_intake),
                    str(root),
                )
                turn_outputs.append(target_turn)
                target_items = event_items(target_turn.stdout)
                target_result = structured_result(target_items, "reference_mask_prepare") or {}
                silhouette_target_sha = field(target_result, "target_sha256") or field(target_result, "target", "target_sha256")
                if not isinstance(silhouette_target_sha, str) or len(silhouette_target_sha) != 64:
                    raise RuntimeError("Codex silhouette target turn did not return target_sha256")
                if not has_subsequence(call_sequence(target_items), SILHOUETTE_TARGET_SEQUENCE) or not all_completed(target_items, SILHOUETTE_TARGET_SEQUENCE):
                    raise RuntimeError("Codex did not complete reference_mask_prepare")
                silhouette_items.extend(target_items)

            discovery_sequence = ("capabilities_get", "runtime_status", "doctor", "operator_catalog_get", "skill_list")
            discovery_items = run_required_codex_turn(
                options,
                environment,
                authoring_prompt(project_id, reference_id, options.geometry_route, options.geometry_variant, options.material_variant),
                str(root),
                discovery_sequence,
                turn_outputs,
                "operator discovery",
            )
            catalog = structured_result(discovery_items, "operator_catalog_get") or {}
            capabilities = structured_result(discovery_items, "capabilities_get") or {}
            catalog_hash = field(catalog, "canonical_sha256")
            capability_hash = field(capabilities, "operator_catalog_sha256")
            if not all(isinstance(value, str) and value for value in (catalog_hash, capability_hash)):
                raise RuntimeError("Codex discovery did not return matching operator catalog hashes")
            prepare_sequence = ("geometry_program_hash", "geometry_prepare")
            hash_prepare_items = run_required_codex_turn(
                options,
                environment,
                authoring_hash_prepare_prompt(
                    project_id,
                    reference_id,
                    options.geometry_route,
                    options.geometry_variant,
                    options.material_variant,
                    catalog_hash,
                ),
                str(root),
                prepare_sequence,
                turn_outputs,
                "geometry hash and prepare",
            )
            hashed = structured_result(hash_prepare_items, "geometry_program_hash") or {}
            prepared = structured_result(hash_prepare_items, "geometry_prepare") or {}
            program_hash = field(hashed, "canonical_sha256")
            candidate = field(prepared, "candidate") or {}
            job = field(prepared, "job") or {}
            artifact = field(prepared, "artifact") or {}
            candidate_id = field(candidate, "candidate_id")
            job_id = field(job, "job_id")
            artifact_id = field(artifact, "artifact_id")
            if not all(isinstance(value, str) and value for value in (catalog_hash, capability_hash, program_hash, candidate_id, job_id, artifact_id)):
                raise RuntimeError("Codex authoring did not return all V2 hashes and opaque IDs")
            if catalog_hash != capability_hash or not has_subsequence(call_sequence(discovery_items), discovery_sequence) or not all_completed(discovery_items, discovery_sequence) or not has_subsequence(call_sequence(hash_prepare_items), prepare_sequence) or not all_completed(hash_prepare_items, prepare_sequence):
                raise RuntimeError("Codex authoring did not complete matching discovery/hash/prepare sequence")

            if options.silhouette_first:
                silhouette_turn_items = run_required_codex_turn(
                    options,
                    environment,
                    silhouette_prompt(project_id, candidate_id, silhouette_target_sha or ""),
                    str(root),
                    SILHOUETTE_SEQUENCE,
                    turn_outputs,
                    "silhouette target/camera/Rig",
                )
                silhouette_items.extend(silhouette_turn_items)
                camera_result = structured_result(silhouette_turn_items, "camera_fit_prepare") or {}
                rig_result = structured_result(silhouette_turn_items, "silhouette_rig_hash") or {}
                observation_result = structured_result(silhouette_turn_items, "scene_observe_get") or {}
                if (
                    observation_result.get("schema_version") != "AgenticSceneObserveResult@1"
                    or observation_result.get("read_only") is not True
                    or observation_result.get("project_id") != project_id
                    or observation_result.get("candidate_id") != candidate_id
                    or not isinstance(observation_result.get("canonical_sha256"), str)
                    or len(observation_result["canonical_sha256"]) != 64
                ):
                    raise RuntimeError("scene_observe_get did not return the bound canonical Agentic observation")
                silhouette_observation_sha = observation_result["canonical_sha256"]
                selected_camera = field(camera_result, "selected_camera") or field(camera_result, "camera")
                # A real Codex turn can summarize a nested selected_camera as
                # only its two hashes even though CameraFitResult also carries
                # the complete calibration in candidates[]. Recover the exact
                # Runtime-owned object from that same result; never synthesize
                # camera fields from the hashes.
                required_camera_keys = {
                    "schema_version", "camera_hash", "projection", "transform",
                    "fov_y_degrees", "near_m", "far_m", "resolution",
                    "coordinate_system", "renderer_revision", "canonical_sha256",
                }
                if isinstance(selected_camera, dict) and not required_camera_keys.issubset(selected_camera):
                    selected_hash = selected_camera.get("camera_hash")
                    selected_canonical = selected_camera.get("canonical_sha256")
                    for row in camera_result.get("candidates", []):
                        candidate_camera = row.get("camera") if isinstance(row, dict) else None
                        if not isinstance(candidate_camera, dict):
                            continue
                        if (
                            (isinstance(selected_hash, str) and candidate_camera.get("camera_hash") == selected_hash)
                            or (isinstance(selected_canonical, str) and candidate_camera.get("canonical_sha256") == selected_canonical)
                        ):
                            selected_camera = candidate_camera
                            break
                # CameraFitResult@1 binds the selected camera as an object;
                # older Runtime receipts also exposed a top-level hash.  Read
                # the typed nested field first and retain the legacy fallback
                # so a successful camera solve cannot be misclassified as a
                # transport failure.
                silhouette_camera_hash = (
                    field(selected_camera or {}, "camera_hash")
                    or field(camera_result, "selected_camera_hash")
                    or field(camera_result, "camera_hash")
                )
                silhouette_rig_sha = field(rig_result, "canonical_sha256")
                if not isinstance(selected_camera, dict) or not isinstance(silhouette_camera_hash, str) or len(silhouette_camera_hash) != 64:
                    raise RuntimeError("Codex silhouette turn did not return selected camera evidence")
                if not isinstance(silhouette_rig_sha, str) or len(silhouette_rig_sha) != 64:
                    raise RuntimeError("Codex silhouette turn did not return Runtime-owned Rig hash")
                if not has_subsequence(call_sequence(silhouette_turn_items), SILHOUETTE_SEQUENCE) or not all_completed(silhouette_turn_items, SILHOUETTE_SEQUENCE):
                    raise RuntimeError("Codex did not complete target/camera/Rig sequence")
                rig = silhouette_rig_draft(candidate_id)
                rig["canonical_sha256"] = silhouette_rig_sha
                # Keep the full calibration in Runtime's result evidence, but
                # send only its two Runtime-owned hashes through the next
                # Codex turn.  This avoids model-side float rounding while
                # preserving an exact candidate/target camera binding.
                camera_ref = {
                    "schema_version": "CameraCalibrationRef@1",
                    "camera_hash": silhouette_camera_hash,
                    "canonical_sha256": selected_camera.get("canonical_sha256"),
                }
                if not isinstance(camera_ref["canonical_sha256"], str) or len(camera_ref["canonical_sha256"]) != 64:
                    raise RuntimeError("Codex silhouette turn did not return selected camera canonical evidence")
                selected_camera_for_compare = selected_camera
                fit_request: dict[str, Any] = {
                    "project_id": project_id,
                    "candidate_id": candidate_id,
                    "target_sha256": silhouette_target_sha,
                    "rig": rig,
                    "base_camera": camera_ref,
                    "optimizer": {"algorithm": "coordinate_descent", "max_iterations": 2, "max_evaluations": 24, "step_fraction": 0.1},
                    "canonical_sha256": "",
                }
                # Codex may serialize an integral float as an integer while
                # preserving its typed value. Hash the numeric-normalized
                # semantic intent so the Runtime can bind either wire form.
                fit_request["canonical_sha256"] = canonical_hash(normalize_numeric_representation(fit_request))
                silhouette_fit_intent_sha = fit_request["canonical_sha256"]
                fit_items = run_required_codex_turn(
                    options,
                    environment,
                    silhouette_fit_prompt(fit_request),
                    str(root),
                    ("silhouette_fit_prepare",),
                    turn_outputs,
                    "silhouette fit",
                )
                silhouette_items.extend(fit_items)
                silhouette_fit_result = structured_result(fit_items, "silhouette_fit_prepare") or {}
                if not has_subsequence(call_sequence(fit_items), ("silhouette_fit_prepare",)) or not all_completed(fit_items, ("silhouette_fit_prepare",)):
                    raise RuntimeError("Codex did not complete silhouette_fit_prepare")
                fit_selected_camera = field(silhouette_fit_result, "selected_camera")
                if not isinstance(fit_selected_camera, dict):
                    raise RuntimeError("silhouette_fit_prepare did not return selected camera evidence")
                silhouette_fit_camera_hash = field(fit_selected_camera, "camera_hash")
                silhouette_fit_camera_canonical = field(fit_selected_camera, "canonical_sha256")
                if (
                    not isinstance(silhouette_fit_camera_hash, str)
                    or len(silhouette_fit_camera_hash) != 64
                    or not isinstance(silhouette_fit_camera_canonical, str)
                    or len(silhouette_fit_camera_canonical) != 64
                ):
                    raise RuntimeError("silhouette_fit_prepare returned an invalid selected camera")
                # The fit result is the authoritative bounded camera proposal
                # for the subsequent comparison.  Keeping the initial camera
                # fit hash separately makes any accidental handoff drift
                # visible in the receipt instead of silently comparing a
                # different camera than the one the fit optimized.
                selected_camera_for_compare = fit_selected_camera

            if options.cadfit_optimization:
                # The async CADFit job is deliberately a separate local-Part
                # proposal.  It consumes the exact camera selected by the
                # preceding silhouette fit and never mutates the source
                # candidate; promotion remains a separate approved action.
                if not isinstance(selected_camera_for_compare, dict):
                    raise RuntimeError("CADFit requires a complete silhouette-fit camera")
                cadfit_rig = single_part_optimization_rig_draft(candidate_id)
                rig_hash_items = run_required_codex_turn(
                    options,
                    environment,
                    cadfit_rig_hash_prompt(project_id, candidate_id, cadfit_rig),
                    str(root),
                    ("silhouette_rig_hash",),
                    turn_outputs,
                    "CADFit one-Part Rig hash",
                )
                cadfit_items.extend(rig_hash_items)
                cadfit_rig_sha = field(structured_result(rig_hash_items, "silhouette_rig_hash") or {}, "canonical_sha256")
                if not isinstance(cadfit_rig_sha, str) or len(cadfit_rig_sha) != 64:
                    raise RuntimeError("CADFit Rig hash was not Runtime-owned")
                cadfit_rig["canonical_sha256"] = cadfit_rig_sha
                optimization_intent = cadfit_optimization_intent(
                    project_id,
                    candidate_id,
                    reference_id,
                    reference_sha,
                    program_hash,
                    silhouette_target_sha or "",
                    selected_camera_for_compare,
                    cadfit_rig,
                )
                cadfit_intent_sha = optimization_intent["canonical_sha256"]
                cadfit_job_id = optimization_intent["job_id"]
                approval = {
                    "approved": True,
                    "approval_receipt_id": "real-codex-cadfit-chest-shell-approval",
                    "approval_summary": "Run an isolated one-Part multi-fidelity CADFit search and return a proposal only",
                    "approval_expires_at": "9999999999",
                    "approval_session_id": "real-codex-cadfit-chest-shell-session",
                    "idempotency_key": "real-codex-cadfit-chest-shell-idempotency",
                }
                prepare_items = run_required_codex_turn(
                    options,
                    environment,
                    cadfit_prepare_prompt({
                        "project_id": project_id,
                        "candidate_id": candidate_id,
                        "intent": optimization_intent,
                        **approval,
                    }),
                    str(root),
                    ("optimization_job_prepare",),
                    turn_outputs,
                    "CADFit OptimizationJob prepare",
                )
                cadfit_items.extend(prepare_items)
                latest_cadfit = structured_result(prepare_items, "optimization_job_prepare") or {}
                if latest_cadfit.get("schema_version") != "OptimizationJobResult@1":
                    raise RuntimeError("CADFit prepare did not return OptimizationJobResult@1")
                deadline = time.monotonic() + options.timeout
                while True:
                    job_status = field(latest_cadfit, "job", "status")
                    if job_status in CADFIT_TERMINAL_STATUSES:
                        break
                    if time.monotonic() >= deadline:
                        raise RuntimeError("CADFit OptimizationJob polling exceeded the probe timeout")
                    time.sleep(0.25)
                    poll_items = run_required_codex_turn(
                        options,
                        environment,
                        cadfit_get_prompt(project_id, candidate_id, cadfit_job_id),
                        str(root),
                        ("optimization_job_get",),
                        turn_outputs,
                        "CADFit OptimizationJob readback",
                    )
                    cadfit_items.extend(poll_items)
                    latest_cadfit = structured_result(poll_items, "optimization_job_get") or latest_cadfit
                cadfit_result = compact_cadfit_result(latest_cadfit)
                if cadfit_result["job_status"] != "succeeded" or cadfit_result["result_status"] != "succeeded":
                    raise RuntimeError(f"CADFit OptimizationJob did not succeed: {cadfit_result['job_status']}/{cadfit_result['result_status']}")
                if cadfit_result["evaluations_count"] != CADFIT_EXPECTED_EVALUATIONS:
                    raise RuntimeError("CADFit OptimizationJob did not complete the bounded 32+4+2+1 evaluation chain")
                if cadfit_result["fidelity_counts"] != {"coarse": CADFIT_COARSE_EVALUATIONS, "mid": CADFIT_MID_TOP_K, "final": CADFIT_FINAL_TOP_K + 1}:
                    raise RuntimeError("CADFit OptimizationJob fidelity counts drifted")
                if cadfit_result["checkpoint_sequence"] != CADFIT_EXPECTED_EVALUATIONS or cadfit_result["next_stage"] != "done":
                    raise RuntimeError("CADFit OptimizationJob checkpoint chain was incomplete")
                if cadfit_result["best_evaluation_fidelity"] != "final":
                    raise RuntimeError("CADFit best-so-far did not survive to the final fidelity")
                partial_evidence.update({
                    "cadfit_optimization_requested": True,
                    "cadfit_rig_sha256": cadfit_rig_sha,
                    "cadfit_intent_sha256": cadfit_intent_sha,
                    "cadfit_job_id": cadfit_job_id,
                    "cadfit_optimization": cadfit_result,
                    "cadfit_sequence": [call.get("tool") for call in mcp_calls(cadfit_items)],
                })

            spec = view_spec(reference_id, reference_sha, width, height, visual_intake)
            third = run_codex_turn(
                options,
                environment,
                compare_prompt(
                    project_id,
                    reference_id,
                    candidate_id,
                    job_id,
                    artifact_id,
                    spec,
                    selected_camera_for_compare,
                    silhouette_target_sha,
                ),
                str(root),
            )
            turn_outputs.append(third)
            third_items = event_items(third.stdout)
            comparison = structured_result(third_items, "reference_compare_prepare") or {}
            artifact_readback = structured_result(third_items, "artifact_readback_get") or {}
            render_set_hash = field(comparison, "render_set_object_sha256") or field(comparison, "render_set_hash")
            comparison_hash = field(comparison, "comparison_report_object_sha256") or field(comparison, "comparison_report_hash")
            render_set = field(comparison, "render_set") or {}
            metrics = field(comparison, "comparison_report", "metrics")
            actual_third = call_sequence(third_items)
            if not isinstance(render_set_hash, str) or not isinstance(comparison_hash, str):
                raise RuntimeError("Codex comparison did not return candidate-bound CAS hashes")
            partial_evidence.update({
                "candidate_id": candidate_id,
                "job_id": job_id,
                "artifact_id": artifact_id,
                "artifact_sha256": field(artifact, "object_sha256"),
                "program_sha256": program_hash,
                "part_count": len(field(artifact, "part_ids") or []),
                "triangle_count": field(artifact, "triangle_count"),
                "validator_status": field(artifact, "validator_status"),
                "silhouette_target_sha256": silhouette_target_sha,
                "silhouette_camera_hash": silhouette_camera_hash,
                "silhouette_fit_camera_hash": silhouette_fit_camera_hash,
                "silhouette_fit_camera_canonical_sha256": silhouette_fit_camera_canonical,
                "comparison_camera_hash": field(comparison, "camera", "camera_hash") or field(comparison, "comparison_report", "camera_hash"),
                "comparison_camera_canonical_sha256": field(comparison, "camera", "canonical_sha256") or field(comparison, "comparison_report", "camera_canonical_sha256"),
                "render_set_hash": render_set_hash,
                "comparison_report_hash": comparison_hash,
                "comparison_metrics": metrics,
                "artifact_readback_object_sha256": typed_sha256(field(artifact_readback, "object_sha256")),
                "comparison_mask_sha256": typed_sha256(field(comparison, "comparison_report", "mask", "sha256")),
                "quality_report_object_sha256": typed_sha256(field(comparison, "quality_report_object_sha256")),
            })
            if selected_camera_for_compare is not None:
                compared_camera_hash = field(comparison, "camera", "camera_hash") or field(comparison, "comparison_report", "camera_hash")
                expected_camera_hash = selected_camera_for_compare.get("camera_hash")
                if compared_camera_hash != expected_camera_hash:
                    raise RuntimeError(
                        "reference_compare_prepare did not use the Runtime-selected camera hash"
                    )
                comparison_camera_hash = compared_camera_hash
                comparison_camera_canonical = field(comparison, "camera", "canonical_sha256") or field(comparison, "comparison_report", "camera_canonical_sha256")
            if render_set.get("passes") != list(AOV_ORDER):
                raise RuntimeError("Codex comparison did not return the fixed nine AOV order")
            if not has_subsequence(actual_third, COMPARE_SEQUENCE) or not all_completed(third_items, COMPARE_SEQUENCE):
                raise RuntimeError("Codex did not complete the readback/compare sequence")

            boundary_result: dict[str, Any] | None = None
            if options.silhouette_first:
                boundary_turn = run_codex_turn(
                    options,
                    environment,
                    boundary_error_prompt(project_id, candidate_id, silhouette_target_sha or ""),
                    str(root),
                )
                turn_outputs.append(boundary_turn)
                boundary_items = event_items(boundary_turn.stdout)
                boundary_result = structured_result(boundary_items, "boundary_error_get") or {}
                if not has_subsequence(call_sequence(boundary_items), ("boundary_error_get",)) or not all_completed(boundary_items, ("boundary_error_get",)):
                    raise RuntimeError("Codex did not complete boundary_error_get")

            boundary_summary = (
                {
                    "metrics": field(boundary_result or {}, "metrics"),
                    "segments": [
                        {
                            key: segment.get(key)
                            for key in ("reference", "model", "delta_px", "distance_px", "direction", "part_id")
                        }
                        for segment in (field(boundary_result or {}, "segments") or [])
                        if isinstance(segment, dict)
                    ],
                }
                if options.silhouette_first
                else None
            )
            if options.boundary_only:
                all_items = [item for turn in turn_outputs for item in event_items(turn.stdout)]
                side_effects = side_effect_summary(all_items)
                blocking_events = blocking_side_effects(side_effects)
                receipt.update({
                    "status": "PASS_BOUNDARY_ONLY" if not blocking_events else "BLOCKED",
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
                    "visual_intake": {
                        "status": "PROVIDED" if visual_intake_sha else "NOT_PROVIDED",
                        "source_sha256": visual_intake_sha,
                        "landmark_count": len(visual_intake["landmarks"]),
                        "region_count": len(visual_intake["regions"]),
                    },
                    "view_spec_sha256": spec["canonical_sha256"],
                    "catalog_sha256": catalog_hash,
                    "program_sha256": program_hash,
                    "candidate_id": candidate_id,
                    "job_id": job_id,
                    "artifact_id": artifact_id,
                    "artifact_sha256": field(artifact, "object_sha256"),
                    "part_count": len(field(artifact, "part_ids") or []),
                    "triangle_count": field(artifact, "triangle_count"),
                    "validator_status": field(artifact, "validator_status"),
                    "silhouette_target_sha256": silhouette_target_sha,
                    "scene_observation_sha256": silhouette_observation_sha,
                    "scene_observation_schema": "AgenticSceneObserveResult@1",
                    "silhouette_camera_hash": silhouette_camera_hash,
                    "silhouette_fit_camera_hash": silhouette_fit_camera_hash,
                    "silhouette_fit_camera_canonical_sha256": silhouette_fit_camera_canonical,
                    "comparison_camera_hash": comparison_camera_hash,
                    "comparison_camera_canonical_sha256": comparison_camera_canonical,
                    "render_set_hash": field(comparison, "render_set_object_sha256") or field(comparison, "render_set_hash"),
                    "comparison_report_hash": field(comparison, "comparison_report_object_sha256") or field(comparison, "comparison_report_hash"),
                    "comparison_metrics": metrics,
                    "boundary_error": boundary_summary,
                    "boundary_error_count": len(field(boundary_result or {}, "segments") or []),
                    "quality_claim": "NO_LIKENESS_PASS_CLAIM; BOUNDARY_EVIDENCE_ONLY",
                    "geometry_route": options.geometry_route,
                    "geometry_variant": options.geometry_variant if options.geometry_route == "detail" else None,
                    "material_variant": options.material_variant if options.geometry_route == "detail" else None,
                    "silhouette_first": options.silhouette_first,
                    "silhouette_gate": "QUALITY_TARGET_NOT_MET" if field(silhouette_fit_result or {}, "status") != "ready" else "PASS",
                    "detail_material_stages": "LOCKED_UNTIL_SILHOUETTE_GATE",
                    "cadfit_optimization_requested": options.cadfit_optimization,
                    "cadfit_rig_sha256": cadfit_rig_sha,
                    "cadfit_intent_sha256": cadfit_intent_sha,
                    "cadfit_job_id": cadfit_job_id,
                    "cadfit_optimization": cadfit_result,
                    "cadfit_sequence": [call.get("tool") for call in mcp_calls(cadfit_items)] if options.cadfit_optimization else [],
                    "mcp_tool_calls": [call for turn in turn_outputs for call in mcp_calls(event_items(turn.stdout))],
                    "expected_sequences": {
                        "setup": list(SETUP_SEQUENCE),
                        "authoring": list(AUTHORING_SEQUENCE),
                        "silhouette_target": list(SILHOUETTE_TARGET_SEQUENCE),
                        "silhouette": list(SILHOUETTE_SEQUENCE),
                        "compare": list(COMPARE_SEQUENCE),
                        "boundary": ["boundary_error_get"],
                        "render": [],
                        "review": [],
                    },
                    "render_pass_calls": 0,
                    "visual_review_status": "NOT_RUN",
                    "quality_visual_status": "NOT_RUN",
                    "human_review": "NOT_RUN",
                    "pbr_material_pack": "NOT_RUN",
                    "hq_360": "BLOCKED_REFERENCE_COVERAGE",
                })
                # Continue through the common finally/write_receipt path, but
                # skip the AOV/review stages below.
                raise BoundaryOnlyComplete

            # Render passes are encoded in each call's `pass` argument rather
            # than in the MCP tool name, so the generic tool-sequence helper
            # cannot validate this stage. Retry the exact request up to three
            # times and accept only when all nine distinct successful AOV
            # names are present; retain every raw call for transport evidence.
            fourth_items: list[dict[str, Any]] = []
            for attempt in range(3):
                retry_note = ""
                if attempt:
                    retry_note = (
                        "\nThis is a bounded retry of the same render stage. "
                        "Complete all nine render_pass_get calls with the exact hash; do not explain or call another tool.\n"
                    )
                fourth = run_codex_turn(options, environment, render_prompt(render_set_hash) + retry_note, str(root))
                turn_outputs.append(fourth)
                fourth_items.extend(event_items(fourth.stdout))
                if render_pass_names(fourth_items) == list(RENDER_SEQUENCE):
                    break
            actual_fourth = call_sequence(fourth_items)
            actual_render_passes = render_pass_names(fourth_items)
            if actual_render_passes != list(RENDER_SEQUENCE) or len(actual_render_passes) != len(RENDER_SEQUENCE):
                raise RuntimeError("Codex did not complete all nine render-pass reads")

            fifth = run_codex_turn(options, environment, review_prompt(candidate_id, reference_id, render_set_hash, comparison_hash), str(root))
            turn_outputs.append(fifth)
            fifth_items = event_items(fifth.stdout)
            # A real Codex may occasionally choose the separate human-review
            # tool even though this source gate explicitly forbids it.  Give
            # the host one bounded recovery turn so a tool-choice drift does
            # not erase already completed comparison/AOV evidence.  The
            # recovery remains fail-closed: only visual_review_submit and
            # quality_get are accepted; no mutation or approval is retried.
            review_recovered = False
            if not review_stage_completed(fifth_items):
                recovery = run_codex_turn(
                    options,
                    environment,
                    review_recovery_prompt(candidate_id, reference_id, render_set_hash, comparison_hash),
                    str(root),
                )
                turn_outputs.append(recovery)
                recovery_items = event_items(recovery.stdout)
                review_recovered = review_stage_completed(recovery_items)
                fifth_items.extend(recovery_items)
            review = structured_result(fifth_items, "visual_review_submit") or {}
            quality = structured_result(fifth_items, "quality_get") or {}
            quality_report = field(quality, "quality_report") or quality
            review_report = field(review, "review") or review
            if not review_stage_completed(fifth_items) or successful_human_review(fifth_items):
                raise RuntimeError("Codex did not complete typed review and quality readback")
            typed_evidence_bindings = build_typed_evidence_bindings(
                comparison,
                artifact_readback,
                fourth_items,
                review,
                quality,
                render_set_hash,
            )
            packaged_viewer = None
            if viewer_command is not None:
                packaged_viewer = read_bound_viewer_projection(
                    viewer_command,
                    root,
                    cohorts["runtime"],
                    project_id,
                    candidate_id,
                    artifact_id,
                    field(artifact, "object_sha256"),
                    reference_id,
                    reference_sha,
                    render_set_hash,
                    comparison_hash,
                )
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
                "visual_intake": {
                    "status": "PROVIDED" if visual_intake_sha else "NOT_PROVIDED",
                    "source_sha256": visual_intake_sha,
                    "landmark_count": len(visual_intake["landmarks"]),
                    "region_count": len(visual_intake["regions"]),
                },
                "view_spec_sha256": spec["canonical_sha256"],
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
                "render_pass_image_blocks": typed_evidence_bindings["render_pass_image_blocks"],
                "comparison_metrics": metrics,
                "typed_evidence_bindings": typed_evidence_bindings,
                "comparison_bindings": typed_evidence_bindings["comparison_bindings"],
                "mask_binding": typed_evidence_bindings["mask_binding"],
                "render_pass_artifacts": typed_evidence_bindings["render_pass_artifacts"],
                "render_pass_readbacks": typed_evidence_bindings["render_pass_readbacks"],
                "metric_binding": typed_evidence_bindings["metric_binding"],
                "visual_review_receipt_hash": typed_evidence_bindings["visual_review"].get("object_sha256"),
                "quality_report_object_sha256": typed_evidence_bindings["quality_readback"].get("object_sha256"),
                "downstream_status": typed_evidence_bindings["downstream_status"],
                # Preserve only the typed, candidate-bound boundary evidence
                # needed for the next single-Part decision.  The MCP result
                # contains normalized points, pixel deltas, direction and
                # semantic Part IDs; it contains no reference bytes or local
                # paths.  Keeping this summary in the receipt prevents the
                # next round from guessing a Part after the Runtime process
                # is torn down.
                "boundary_error": boundary_summary,
                "visual_review_status": field(review_report, "status"),
                "quality_visual_status": field(quality_report, "visual_status"),
                "quality_hard_gate_passed": field(quality_report, "hard_gate_passed"),
                "review_recovered_after_tool_drift": review_recovered,
                "mcp_tool_calls": [call for turn in turn_outputs for call in mcp_calls(event_items(turn.stdout))],
                "expected_sequences": {
                    "setup": list(SETUP_SEQUENCE),
                    "authoring": list(AUTHORING_SEQUENCE),
                        "silhouette_target": list(SILHOUETTE_TARGET_SEQUENCE) if options.silhouette_first else [],
                        "silhouette": list(SILHOUETTE_SEQUENCE) if options.silhouette_first else [],
                        "cadfit": ["silhouette_rig_hash", "optimization_job_prepare", "optimization_job_get"] if options.cadfit_optimization else [],
                        "compare": list(COMPARE_SEQUENCE),
                    "render": list(RENDER_SEQUENCE),
                    "review": list(REVIEW_SEQUENCE),
                },
                "quality_claim": "QUALITY_TARGET_NOT_MET_OR_NOT_CLAIMED",
                "geometry_route": options.geometry_route,
                "geometry_variant": options.geometry_variant if options.geometry_route == "detail" else None,
                "material_variant": options.material_variant if options.geometry_route == "detail" else None,
                "silhouette_first": options.silhouette_first,
                    "silhouette_target_sha256": silhouette_target_sha,
                    "scene_observation_sha256": silhouette_observation_sha,
                    "scene_observation_schema": "AgenticSceneObserveResult@1" if silhouette_observation_sha else None,
                    "silhouette_camera_hash": silhouette_camera_hash,
                "silhouette_fit_camera_hash": silhouette_fit_camera_hash,
                "silhouette_fit_camera_canonical_sha256": silhouette_fit_camera_canonical,
                "comparison_camera_hash": comparison_camera_hash,
                "comparison_camera_canonical_sha256": comparison_camera_canonical,
                "camera_binding_status": (
                    "PASS_SILHOUETTE_FIT_TO_COMPARE"
                    if silhouette_fit_camera_hash is not None and comparison_camera_hash == silhouette_fit_camera_hash
                    else "PASS_CAMERA_FIT_TO_COMPARE"
                    if silhouette_camera_hash is not None and comparison_camera_hash == silhouette_camera_hash
                    else "NOT_RUN"
                ),
                "silhouette_rig_sha256": silhouette_rig_sha,
                "silhouette_fit_intent_sha256": silhouette_fit_intent_sha,
                "silhouette_fit": {
                    "status": field(silhouette_fit_result or {}, "status"),
                    "iterations": field(silhouette_fit_result or {}, "iterations"),
                    "evaluations": field(silhouette_fit_result or {}, "evaluations"),
                    "loss": field(silhouette_fit_result or {}, "loss"),
                    "metrics": field(silhouette_fit_result or {}, "metrics"),
                } if options.silhouette_first else None,
                "boundary_error_count": len(field(boundary_result or {}, "segments") or []) if options.silhouette_first else None,
                "silhouette_sequence": [call.get("tool") for call in mcp_calls(silhouette_items)] if options.silhouette_first else [],
                    "silhouette_gate": "NOT_RUN" if not options.silhouette_first else ("PASS" if field(silhouette_fit_result or {}, "status") == "ready" else "QUALITY_TARGET_NOT_MET"),
                    "detail_material_stages": "LOCKED_UNTIL_SILHOUETTE_GATE" if options.silhouette_first and field(silhouette_fit_result or {}, "status") != "ready" else "NOT_APPLICABLE",
                    "cadfit_optimization_requested": options.cadfit_optimization,
                    "cadfit_rig_sha256": cadfit_rig_sha,
                    "cadfit_intent_sha256": cadfit_intent_sha,
                    "cadfit_job_id": cadfit_job_id,
                    "cadfit_optimization": cadfit_result,
                    "cadfit_sequence": [call.get("tool") for call in mcp_calls(cadfit_items)] if options.cadfit_optimization else [],
                })
            receipt.update(
                complete_transport_receipt_fields(
                    metrics,
                    typed_evidence_bindings,
                    field(quality_report, "visual_status"),
                    field(quality_report, "hard_gate_passed"),
                    field(review_report, "status"),
                    receipt.get("camera_binding_status", "NOT_RUN"),
                    policy,
                )
            )
            if packaged_viewer is not None:
                receipt["packaged_viewer"] = packaged_viewer
            if options.debug:
                for turn in turn_outputs:
                    print(turn.stdout, file=sys.stderr)
                    print(turn.stderr, file=sys.stderr)
    except BoundaryOnlyComplete:
        pass
    except (OSError, RuntimeError, subprocess.SubprocessError, json.JSONDecodeError) as error:
        receipt["reason"] = str(error)[:2000]
        receipt["silhouette_fit_intent_sha256"] = silhouette_fit_intent_sha
        receipt.update({
            "cadfit_optimization_requested": options.cadfit_optimization,
            "cadfit_rig_sha256": cadfit_rig_sha,
            "cadfit_intent_sha256": cadfit_intent_sha,
            "cadfit_job_id": cadfit_job_id,
            "cadfit_optimization": cadfit_result,
            "cadfit_sequence": [call.get("tool") for call in mcp_calls(cadfit_items)] if options.cadfit_optimization else [],
        })
        receipt.update(partial_evidence)
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
