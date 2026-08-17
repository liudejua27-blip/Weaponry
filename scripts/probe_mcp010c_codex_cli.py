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
import signal
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone
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
from probe_mcp010e_raw_stdio import robot_detail_program_draft  # noqa: E402


AOV_ORDER = ("beauty", "silhouette", "depth", "normal", "ao", "part-id", "material-id", "wireframe", "uv-stretch")
SETUP_SEQUENCE = ("skill_get", "project_create", "reference_import", "reference_get")
REFERENCE_VIEW_KINDS = (
    "front",
    "back",
    "left",
    "right",
    "top",
    "perspective",
    "rear-three-quarter",
    "material",
    "detail",
)
REQUIRED_COVERAGE_VIEWS = ("front", "back", "left", "right", "perspective", "rear-three-quarter")
AUTHORING_SEQUENCE = ("capabilities_get", "runtime_status", "doctor", "operator_catalog_get", "skill_list", "geometry_program_hash", "geometry_prepare")
GEOMETRY_HASH_SEQUENCE = ("geometry_program_hash",)
GEOMETRY_PREPARE_SEQUENCE = ("geometry_prepare",)
COMPARE_SEQUENCE = ("job_get", "candidate_get", "artifact_readback_get", "reference_compare_prepare")
# The synchronous Primary Form endpoint is intentionally not used by the
# real-client probe: a bounded 64-evaluation Geometry/Render Worker search can
# outlive one MCP request deadline.  The durable Job path keeps the same
# Runtime-owned action and lets Codex observe completion before reading the
# CAS-backed typed result.
PRIMARY_FORM_REPAIR_SEQUENCE = ("primary_form_repair_job_prepare", "job_get", "job_result_get")
RENDER_SEQUENCE = AOV_ORDER
REVIEW_SEQUENCE = ("visual_review_submit", "quality_get")
SILHOUETTE_TARGET_SEQUENCE = ("reference_mask_prepare",)
# Keep the visual turn as one bounded observe -> decide -> author -> observe
# surface.  The explicit ReferenceCanvas/DesignSpec authoring step occurs
# after target/camera/compare; scene_observe_get then reads the Runtime-owned
# post-authoring snapshot, so the model does not stitch a design decision from
# unrelated fragmented reads or silently re-observe after a state change.
SILHOUETTE_SEQUENCE = (
    "silhouette_target_get",
    "camera_fit_prepare",
    "job_get",
    "candidate_get",
    "artifact_readback_get",
    "reference_compare_prepare",
    "session_create_or_resume",
    "scene_observe_get",
    "silhouette_rig_hash",
)
# Keep the real-client authoring boundary in small, independently bounded
# turns.  The Runtime still owns the complete sequence and the receipt joins
# these raw events in order; only the Codex transport session is split so a
# large ReferenceCanvas/DesignSpec payload cannot starve the observation call.
SILHOUETTE_CORE_SEQUENCE = SILHOUETTE_SEQUENCE[:6]
SILHOUETTE_AUTHORING_SEQUENCE = ("session_create_or_resume",)
SILHOUETTE_OBSERVATION_SEQUENCE = ("scene_observe_get",)
SILHOUETTE_RIG_SEQUENCE = ("silhouette_rig_hash",)


class BoundaryOnlyComplete(RuntimeError):
    """Internal control flow for the evidence-only boundary route."""


def parse_view_path_bindings(values: list[str] | None, option_name: str) -> tuple[tuple[str, Path], ...]:
    """Parse explicit ``view-kind=path`` bindings without accepting inference.

    The primary ``--reference`` remains the supplied perspective view.  Every
    additional view must be named by the caller, so a single image can never
    silently become a fabricated front/back/side reference.
    """
    bindings: list[tuple[str, Path]] = []
    seen: set[str] = set()
    for raw in values or []:
        if not isinstance(raw, str) or "=" not in raw:
            raise ValueError(f"{option_name} requires VIEW_KIND=PATH")
        kind, raw_path = raw.split("=", 1)
        kind = kind.strip()
        raw_path = raw_path.strip()
        if kind not in REFERENCE_VIEW_KINDS:
            raise ValueError(f"{option_name} has unsupported view kind: {kind or '<empty>'}")
        if not raw_path:
            raise ValueError(f"{option_name} requires a non-empty path for {kind}")
        if kind in seen:
            raise ValueError(f"{option_name} repeats view kind: {kind}")
        seen.add(kind)
        bindings.append((kind, Path(raw_path).expanduser()))
    return tuple(bindings)


def stable_view_id(kind: str) -> str:
    """Return the stable view id used by the ReferenceCanvas probe."""
    if kind == "perspective":
        # Preserve the existing single-reference id so old receipts and
        # downstream readbacks remain comparable.
        return "three-quarter-user-reference"
    return f"{kind}-user-reference"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--reference", required=True, help="user-authorized PNG/JPEG path")
    parser.add_argument(
        "--reference-view",
        action="append",
        metavar="VIEW_KIND=PATH",
        help=(
            "Additional explicitly user-authorized reference view. Repeat for front/back/left/right/top/"
            "rear-three-quarter/detail/material; --reference is the primary perspective view."
        ),
    )
    parser.add_argument("--runtime-command", required=True)
    parser.add_argument("--mcp-command", required=True)
    parser.add_argument("--codex-command", default="codex")
    parser.add_argument("--evidence", type=Path, help="JSON receipt below docs/evidence")
    parser.add_argument(
        "--intake",
        type=Path,
        help="Optional sanitized ForgeCADCodexReferenceInventory/robot intake JSON with normalized landmarks and visible regions.",
    )
    parser.add_argument(
        "--intake-view",
        action="append",
        metavar="VIEW_KIND=PATH",
        help=(
            "Optional sanitized intake JSON for an explicitly supplied view. Repeat with the matching view kind; "
            "the primary perspective intake remains --intake."
        ),
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
        "--primary-form-repair",
        action="store_true",
        help="After the one-shot observation/camera/Rig turn, submit one Runtime-owned Primary Form repair action and compare its staged candidate.",
    )
    parser.add_argument(
        "--observation-only",
        action="store_true",
        help="Stop after the candidate-bound durable session, canonical scene observation and Runtime-owned Rig hash; do not run silhouette fit/AOV/review.",
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
    parser.add_argument(
        "--part-contour-part",
        help="after boundary_error_get, request one Runtime-owned PartContourFitResult@1 for this exact semantic Part ID",
    )
    parser.add_argument(
        "--part-contour-trial",
        action="store_true",
        help="scope one Runtime-owned primary_form_repair_prepare candidate trial to --part-contour-part and keep the same-camera acceptance gate",
    )
    parser.add_argument(
        "--part-contour-sequence",
        help="run 2-3 exact Part repairs serially in one project; each accepted staged candidate becomes the next step source",
    )
    options = parser.parse_args()
    try:
        options.reference_views = parse_view_path_bindings(options.reference_view, "--reference-view")
        options.intake_views = parse_view_path_bindings(options.intake_view, "--intake-view")
    except ValueError as error:
        parser.error(str(error))
    if any(kind == "perspective" for kind, _ in options.reference_views):
        parser.error("--reference-view perspective is reserved for the primary --reference")
    if any(kind == "perspective" for kind, _ in options.intake_views):
        parser.error("--intake-view perspective is reserved for the primary --intake")
    supplied_kinds = {"perspective", *(kind for kind, _ in options.reference_views)}
    intake_kinds = {kind for kind, _ in options.intake_views}
    if not intake_kinds.issubset(supplied_kinds):
        missing = sorted(intake_kinds - supplied_kinds)
        parser.error(f"--intake-view has no matching --reference-view: {', '.join(missing)}")
    if len(supplied_kinds) > len(REQUIRED_COVERAGE_VIEWS) + 3:
        parser.error("at most 9 explicit reference views are supported")
    if options.primary_form_repair and not options.silhouette_first:
        parser.error("--primary-form-repair requires --silhouette-first")
    if options.observation_only and not options.silhouette_first:
        parser.error("--observation-only requires --silhouette-first")
    if options.observation_only and (options.primary_form_repair or options.part_contour_part or options.part_contour_trial or options.part_contour_sequence):
        parser.error("--observation-only cannot be combined with Primary Form or Part contour actions")
    if options.observation_only:
        options.boundary_only = True
    if options.part_contour_part and not options.silhouette_first:
        parser.error("--part-contour-part requires --silhouette-first")
    if options.part_contour_trial and not options.part_contour_part:
        parser.error("--part-contour-trial requires --part-contour-part")
    if options.part_contour_trial and not options.silhouette_first:
        parser.error("--part-contour-trial requires --silhouette-first")
    if options.part_contour_sequence:
        if options.part_contour_trial or options.part_contour_part:
            parser.error("--part-contour-sequence cannot be combined with --part-contour-part or --part-contour-trial")
        parts = tuple(part.strip() for part in options.part_contour_sequence.split(",") if part.strip())
        if not 2 <= len(parts) <= 3:
            parser.error("--part-contour-sequence requires 2 or 3 comma-separated Part IDs")
        if len(set(parts)) != len(parts):
            parser.error("--part-contour-sequence must not repeat a Part ID")
        if any(len(part) > 128 or not re.fullmatch(r"[A-Za-z0-9_.:-]+", part) for part in parts):
            parser.error("--part-contour-sequence contains an invalid Part ID")
        options.part_contour_sequence_parts = parts
        options.silhouette_first = True
        options.primary_form_repair = True
    else:
        options.part_contour_sequence_parts = ()
    if options.part_contour_trial:
        options.primary_form_repair = True
    return options


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
    """Return the successful raw ForgeCAD call order for stage validation.

    ``mcp_calls`` is intentionally a compact receipt projection grouped by
    call id.  A fresh Codex retry can reuse a logical call id, so using that
    projection for ordering can hide a completed suffix behind an earlier
    partial call.  Stage gates must consume the raw completed events; the
    compact projection remains reserved for transport receipts.
    """
    return [
        str(item.get("tool"))
        for item in items
        if item.get("type") == "mcp_tool_call"
        and item.get("server") == "forgecad"
        and item.get("status") == "completed"
    ]


def has_subsequence(actual: list[str], expected: tuple[str, ...]) -> bool:
    cursor = 0
    for name in actual:
        if cursor < len(expected) and name == expected[cursor]:
            cursor += 1
    return cursor == len(expected)


def all_completed(items: list[dict[str, Any]], expected: tuple[str, ...]) -> bool:
    completed = set(call_sequence(items))
    return all(name in completed for name in expected)


def structured_results(items: list[dict[str, Any]], tool_name: str) -> list[dict[str, Any]]:
    """Return one typed result per completed call, preserving call order.

    ``structured_result`` intentionally returns the last result for a tool and
    is correct for the historical single-reference path.  Multi-view setup
    imports the same tool several times, so this helper groups lifecycle events
    by MCP call id and keeps each completed typed payload without retaining the
    raw arguments or image data.
    """
    grouped: dict[str, dict[str, Any]] = {}
    order: list[str] = []
    seen_call_ids: set[str] = set()
    fallback_index = 0
    for item in items:
        if item.get("type") != "mcp_tool_call" or item.get("tool") != tool_name:
            continue
        call_id = item.get("id")
        if not isinstance(call_id, str) or not call_id:
            # Codex normally supplies ids.  If a client omits them, completed
            # events are still kept in observation order rather than merged
            # into one result.
            call_id = f"{tool_name}-fallback-{fallback_index}"
            if item.get("status") == "completed":
                fallback_index += 1
        if call_id not in seen_call_ids:
            seen_call_ids.add(call_id)
            order.append(call_id)
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
            grouped[call_id] = structured
    return [grouped[call_id] for call_id in order if call_id in grouped]


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
    # The explicit ReferenceCanvas/DesignSpec session passes max_attempts=1
    # because it is the only potentially mutating authoring boundary in this
    # observation route.  Earlier discovery/hash/prepare and readback stages
    # remain bounded-retry safe inside the probe's isolated temporary Runtime;
    # a single Codex tool-choice drift must not erase an otherwise valid run.
    attempt_limit = max_attempts
    for attempt in range(attempt_limit):
        retry_note = ""
        if attempt:
            retry_note = (
                "\nThis is a bounded retry of the same stage. A previous turn may have stopped "
                "after a partial sequence; do not explain, recompute hashes, or call unrelated "
                "tools. Complete the exact remaining sequence now.\n"
            )
        turn = run_codex_turn(options, environment, prompt_text + retry_note, workspace_root)
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
    diagnostic = ""
    if label == "ReferenceCanvas/DesignSpec durable authoring":
        diagnostic = f"; authoring_argument={json.dumps(authoring_argument_summary(aggregate), ensure_ascii=False, sort_keys=True)}"
    raise RuntimeError(f"Codex did not complete {label} after {attempt_limit} bounded attempts{diagnostic}")


def authoring_argument_summary(items: list[dict[str, Any]]) -> dict[str, Any]:
    """Summarize only canonical/binding facts from a failed authoring call.

    The real Codex event may contain the complete MCP arguments, but receipts
    must never retain the full authoring payload.  These hashes and counts are
    sufficient to distinguish a canvas canonical drift from a DesignSpec
    binding drift without leaking prompts, image bytes or local paths.
    """
    calls = [
        item
        for item in items
        if item.get("type") == "mcp_tool_call"
        and item.get("server") == "forgecad"
        and item.get("tool") == "session_create_or_resume"
    ]
    if not calls:
        return {"session_call": "not_observed"}
    item = calls[-1]
    arguments = item.get("arguments")
    if not isinstance(arguments, dict):
        return {"session_call": "arguments_unavailable", "status": item.get("status")}
    context = arguments.get("authoring_context")
    if not isinstance(context, dict):
        return {"session_call": "authoring_context_missing", "status": item.get("status")}

    def digest_pair(value: Any) -> dict[str, Any]:
        if not isinstance(value, dict):
            return {"present": False}
        declared = value.get("canonical_sha256")
        canonical_input = dict(value)
        canonical_input["canonical_sha256"] = ""
        recomputed = canonical_hash(canonical_input)
        runtime_object = dict(canonical_input)
        runtime_object["canonical_sha256"] = recomputed
        return {
            "present": True,
            "declared": declared if isinstance(declared, str) else None,
            "declared_empty_for_runtime_canonicalization": declared == "",
            "recomputed": recomputed,
            "object_sha256": canonical_hash(value),
            "runtime_canonical_sha256": recomputed,
            "runtime_object_sha256": canonical_hash(runtime_object),
            "declared_matches_recomputed": declared == "" or declared == recomputed,
        }

    canvas = context.get("reference_canvas")
    spec = context.get("design_spec")
    canvas_summary = digest_pair(canvas)
    spec_summary = digest_pair(spec)
    if isinstance(canvas, dict):
        canvas_summary.update({
            "view_count": len(canvas.get("views", [])) if isinstance(canvas.get("views"), list) else None,
            "unknown_count": len(canvas.get("unknowns", [])) if isinstance(canvas.get("unknowns"), list) else None,
            "claim_count": len(canvas.get("claims", [])) if isinstance(canvas.get("claims"), list) else None,
        })
    if isinstance(spec, dict):
        spec_summary.update({
            "reference_canvas_sha256": spec.get("reference_canvas_sha256"),
            "stage_goal_count": len(spec.get("stage_goals", [])) if isinstance(spec.get("stage_goals"), list) else None,
            "primary_form_count": len(spec.get("primary_forms", [])) if isinstance(spec.get("primary_forms"), list) else None,
            "semantic_part_count": len(spec.get("semantic_parts", [])) if isinstance(spec.get("semantic_parts"), list) else None,
        })
    return {
        "session_call": "observed",
        "status": item.get("status"),
        "context_keys": sorted(context),
        "canvas": canvas_summary,
        "design_spec": spec_summary,
        # With a blank producer-owned canonical field the wire object hash is
        # intentionally not the Runtime-stored object hash.  This diagnostic
        # predicts the Runtime CAS object hash after canonicalization without
        # treating it as Runtime truth.
        "expected_binding_matches_argument_runtime_object": (
            None
            if canvas_summary.get("declared_empty_for_runtime_canonicalization")
            else (
                isinstance(spec, dict)
                and spec.get("reference_canvas_sha256") == canvas_summary.get("runtime_object_sha256")
            )
        ),
    }


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
        normalized_command = command_text.lower()
        forbidden_tokens = (
            "rm ", "mv ", "cp ", "chmod ", "chown ", "tee ", "install ",
            "python", "node ", "git ", "curl ", "wget ", "http://", "https://",
        )
        # A read-only Skill lookup may use a pipe (for example, sed/awk into
        # a bounded preview).  Treat only command separators and redirection
        # as mutation boundaries here; the forbidden-token list below still
        # blocks writes, installs, network access and arbitrary runtimes.
        shell_mutation = bool(re.search(r"[;&<>]", command_text))
        read_only_skill_lookup = (
            item_type == "command_execution"
            and (
                "skill.md" in normalized_command
                or (
                    ".codex/" in normalized_command
                    and "/skills/" in normalized_command
                    and normalized_command.endswith(".md")
                )
            )
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


def run_codex_process(
    command: list[str],
    environment: dict[str, str],
    timeout: float,
    input_text: str | None = None,
) -> subprocess.CompletedProcess[str]:
    """Run Codex in a private process group and terminate the whole group on timeout.

    The visual probe may start an MCP server and a code-mode host as children.
    ``subprocess.run(timeout=...)`` only guarantees that the parent call raises;
    it does not make a stale child session disappear.  A private process group
    keeps timeout cleanup isolated to this probe and prevents a retry from
    inheriting a hung MCP session.
    """
    process = subprocess.Popen(
        command,
        env=environment,
        text=True,
        stdin=subprocess.PIPE if input_text is not None else None,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    try:
        stdout, stderr = process.communicate(input=input_text, timeout=timeout)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            stdout, stderr = process.communicate(timeout=5)
        except subprocess.TimeoutExpired:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            stdout, stderr = process.communicate(timeout=5)
        raise
    return subprocess.CompletedProcess(command, process.returncode, stdout, stderr)


def run_codex_turn(
    options: argparse.Namespace,
    environment: dict[str, str],
    prompt_text: str,
    workspace_root: str,
    image_path: str | list[str] | None = None,
) -> subprocess.CompletedProcess[str]:
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
            config_override(options.mcp_command),
            "-c",
            'mcp_servers.cloudflare-api={url="http://127.0.0.1:1",enabled=false,required=false}',
        ]
        if options.sandbox == "read-only":
            command[8:8] = ["--sandbox", "read-only"]
        else:
            command[8:8] = ["--approve-for-me"]
        # Keep the MCP request deadline at least as long as the Codex turn.
        # Geometry preparation can spend time in the bounded Worker while the
        # real client is still waiting for the typed result.
        command[command.index("-c") + 1] = config_override(options.mcp_command).replace(
            "tool_timeout_sec=120",
            f"tool_timeout_sec={max(120, int(options.timeout))}",
        )
        if image_path:
            image_paths = [image_path] if isinstance(image_path, str) else image_path
            for path in image_paths:
                command.extend(["--image", path])
            return run_codex_process(command, environment, options.timeout, prompt_text + "\n")
        command.append(prompt_text)
        return run_codex_process(command, environment, options.timeout)


def field(result: Any, *names: str) -> Any:
    current = result
    for name in names:
        if not isinstance(current, dict):
            return None
        current = current.get(name)
    return current


def parse_bound_silhouette_turn(
    items: list[dict[str, Any]],
    project_id: str,
    candidate_id: str,
) -> dict[str, Any]:
    """Validate one consolidated observation turn and return Runtime-owned refs.

    A composition step must never reuse a camera, observation or baseline
    compare from the previous candidate.  This parser is shared by the first
    step and every later step so the sequence stays candidate-bound while the
    continuous fit remains inside Runtime.
    """
    camera_result = structured_result(items, "camera_fit_prepare") or {}
    rig_result = structured_result(items, "silhouette_rig_hash") or {}
    comparison = structured_result(items, "reference_compare_prepare") or {}
    session_result = structured_result(items, "session_create_or_resume") or {}
    observation = structured_result(items, "scene_observe_get") or {}
    if (
        observation.get("schema_version") != "AgenticSceneObserveResult@1"
        or observation.get("read_only") is not True
        or observation.get("project_id") != project_id
        or observation.get("candidate_id") != candidate_id
        or not isinstance(observation.get("canonical_sha256"), str)
        or len(observation["canonical_sha256"]) != 64
    ):
        raise RuntimeError("scene_observe_get did not return the bound canonical Agentic observation")
    if field(comparison, "candidate_id") not in (None, candidate_id):
        raise RuntimeError("reference_compare_prepare drifted from the observed candidate")
    if not isinstance(
        field(comparison, "render_set_object_sha256") or field(comparison, "render_set_hash"),
        str,
    ):
        raise RuntimeError("Codex silhouette turn did not return baseline compare evidence")
    session = session_result.get("session") if isinstance(session_result, dict) else None
    authoring = session_result.get("authoring_context") if isinstance(session_result, dict) else None
    if (
        session_result.get("schema_version") != "AgenticSessionResult@1"
        or session_result.get("durable") is not True
        or not isinstance(session, dict)
        or session.get("schema_version") != "DesignSession@1"
        or not isinstance(authoring, dict)
        or authoring.get("schema_version") != "AgenticAuthoringContext@1"
        or authoring.get("durable") is not True
        or authoring.get("read_only") is not True
        or session.get("observation_sha256") != observation.get("canonical_sha256")
    ):
        raise RuntimeError("session_create_or_resume did not return the post-authoring bound observation")
    selected_camera = field(camera_result, "selected_camera") or field(camera_result, "camera")
    # Codex can summarize selected_camera as only two hashes even though the
    # typed result carries the complete calibration in candidates[]. Recover
    # the exact Runtime object; never synthesize camera fields from hashes.
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
    camera_hash = (
        field(selected_camera or {}, "camera_hash")
        or field(camera_result, "selected_camera_hash")
        or field(camera_result, "camera_hash")
    )
    rig_sha = field(rig_result, "canonical_sha256")
    if not isinstance(selected_camera, dict) or not isinstance(camera_hash, str) or len(camera_hash) != 64:
        raise RuntimeError("Codex silhouette turn did not return selected camera evidence")
    camera_canonical = selected_camera.get("canonical_sha256")
    if not isinstance(camera_canonical, str) or len(camera_canonical) != 64:
        raise RuntimeError("Codex silhouette turn did not return selected camera canonical evidence")
    if not isinstance(rig_sha, str) or len(rig_sha) != 64:
        raise RuntimeError("Codex silhouette turn did not return Runtime-owned Rig hash")
    if not has_subsequence(call_sequence(items), SILHOUETTE_SEQUENCE) or not all_completed(items, SILHOUETTE_SEQUENCE):
        raise RuntimeError("Codex did not complete target/camera/compare/observation/Rig sequence")
    return {
        "camera": selected_camera,
        "camera_ref": {
            "schema_version": "CameraCalibrationRef@1",
            "camera_hash": camera_hash,
            "canonical_sha256": camera_canonical,
        },
        "camera_hash": camera_hash,
        "camera_canonical": camera_canonical,
        "rig_sha": rig_sha,
        "comparison": comparison,
        "session_result": session_result,
        "observation": observation,
        "observation_sha": observation["canonical_sha256"],
    }


def authoring_binding_summary(result: dict[str, Any] | None) -> dict[str, Any]:
    """Keep the durable authoring proof compact and free of image/path data."""
    if not isinstance(result, dict):
        return {"status": "NOT_RUN"}
    session = result.get("session") if isinstance(result.get("session"), dict) else {}
    context = result.get("authoring_context") if isinstance(result.get("authoring_context"), dict) else {}
    canvas = context.get("reference_canvas") if isinstance(context.get("reference_canvas"), dict) else {}
    coverage = canvas.get("coverage") if isinstance(canvas.get("coverage"), dict) else {}
    documents = result.get("documents") if isinstance(result.get("documents"), dict) else {}
    canvas_document = documents.get("reference_canvas") if isinstance(documents.get("reference_canvas"), dict) else {}
    spec_document = documents.get("design_spec") if isinstance(documents.get("design_spec"), dict) else {}
    return {
        "status": "PASS_DURABLE_REFERENCE_CANVAS_DESIGN_SPEC" if result.get("durable") is True else "BLOCKED",
        "session_id": session.get("session_id"),
        "observation_sha256": session.get("observation_sha256"),
        "reference_canvas_id": canvas.get("canvas_id"),
        "reference_canvas_object_sha256": canvas_document.get("object_sha256"),
        "reference_set_sha256": canvas.get("reference_set_sha256"),
        "reference_view_count": len(canvas.get("views", [])) if isinstance(canvas.get("views"), list) else None,
        "design_spec_id": (context.get("design_spec") or {}).get("spec_id") if isinstance(context.get("design_spec"), dict) else None,
        "design_spec_object_sha256": spec_document.get("object_sha256"),
        "coverage_status": coverage.get("coverage_status"),
        "supplied_views": coverage.get("supplied_views"),
        "missing_views": coverage.get("missing_views"),
        "hq_360_status": coverage.get("hq_360_status"),
    }


def build_primary_form_composition_lineage(
    project_id: str,
    initial_candidate_id: str,
    final_candidate_id: str,
    target_sha256: str,
    requested_part_ids: tuple[str, ...],
    steps: list[dict[str, Any]],
) -> dict[str, Any]:
    """Collapse a serial Primary Form run into one validated hash-bound receipt.

    Raw Codex/MCP events remain available for transport auditing, but the
    next decision must consume this compact lineage instead of reconstructing
    state from scattered turns.  Candidate advancement is deliberately
    fail-closed: only an accepted staged candidate can become the next step's
    source; a retained source keeps the chain on the same candidate.
    """
    if not 2 <= len(requested_part_ids) <= 3 or len(steps) != len(requested_part_ids):
        raise RuntimeError("PRIMARY_FORM_COMPOSITION_INVALID: sequence length drifted")
    current_candidate_id = initial_candidate_id
    normalized_steps: list[dict[str, Any]] = []
    accepted_step_count = 0
    for expected_step, requested_part_id in enumerate(requested_part_ids, start=1):
        step = steps[expected_step - 1]
        if step.get("step") != expected_step or step.get("part_id") != requested_part_id:
            raise RuntimeError("PRIMARY_FORM_COMPOSITION_INVALID: step identity drifted")
        if step.get("source_candidate_id") != current_candidate_id:
            raise RuntimeError("PRIMARY_FORM_COMPOSITION_INVALID: source candidate chain drifted")
        if step.get("observation_candidate_id") != current_candidate_id:
            raise RuntimeError("PRIMARY_FORM_COMPOSITION_INVALID: observation candidate is stale")
        if step.get("target_sha256") != target_sha256:
            raise RuntimeError("PRIMARY_FORM_COMPOSITION_INVALID: target binding drifted")
        for key in (
            "observation_sha256",
            "camera_hash",
            "camera_canonical_sha256",
            "rig_sha256",
            "intent_sha256",
            "fit_camera_hash",
        ):
            value = step.get(key)
            if not isinstance(value, str) or len(value) != 64:
                raise RuntimeError(f"PRIMARY_FORM_COMPOSITION_INVALID: {key} is not hash-bound")
        status = step.get("status")
        acceptance = step.get("acceptance")
        if status not in {"prepared", "no_improvement"} or not isinstance(acceptance, dict):
            raise RuntimeError("PRIMARY_FORM_COMPOSITION_INVALID: unsupported step status")
        prepared_candidate_id = step.get("prepared_candidate_id")
        if status == "prepared":
            if not isinstance(prepared_candidate_id, str) or not prepared_candidate_id:
                raise RuntimeError("PRIMARY_FORM_COMPOSITION_INVALID: accepted step has no staged candidate")
            if prepared_candidate_id == current_candidate_id:
                raise RuntimeError("PRIMARY_FORM_COMPOSITION_INVALID: staged candidate did not advance")
            if acceptance.get("status") != "accepted" or acceptance.get("strict_improvement") is not True:
                raise RuntimeError("PRIMARY_FORM_COMPOSITION_INVALID: accepted step lacks strict acceptance")
            accepted_step_count += 1
            next_candidate_id = prepared_candidate_id
        else:
            if prepared_candidate_id is not None:
                raise RuntimeError("PRIMARY_FORM_COMPOSITION_INVALID: retained step advanced candidate")
            if acceptance.get("status") != "retained_source" or acceptance.get("strict_improvement") is not False:
                raise RuntimeError("PRIMARY_FORM_COMPOSITION_INVALID: retained step acceptance drifted")
            next_candidate_id = current_candidate_id
        normalized_steps.append({
            "step": expected_step,
            "part_id": requested_part_id,
            "source_candidate_id": current_candidate_id,
            "observation_candidate_id": step["observation_candidate_id"],
            "observation_sha256": step["observation_sha256"],
            "target_sha256": target_sha256,
            "camera_hash": step["camera_hash"],
            "camera_canonical_sha256": step["camera_canonical_sha256"],
            "rig_sha256": step["rig_sha256"],
            "intent_sha256": step["intent_sha256"],
            "fit_camera_hash": step["fit_camera_hash"],
            "status": status,
            "acceptance_status": acceptance["status"],
            "acceptance_strict_improvement": acceptance["strict_improvement"],
            "prepared_candidate_id": prepared_candidate_id,
            "next_candidate_id": next_candidate_id,
        })
        current_candidate_id = next_candidate_id
    if current_candidate_id != final_candidate_id:
        raise RuntimeError("PRIMARY_FORM_COMPOSITION_INVALID: final candidate drifted")
    lineage: dict[str, Any] = {
        "schema_version": "ForgeCADPrimaryFormCompositionLineage@1",
        "project_id": project_id,
        "initial_candidate_id": initial_candidate_id,
        "final_candidate_id": final_candidate_id,
        "target_sha256": target_sha256,
        "requested_part_ids": list(requested_part_ids),
        "step_count": len(normalized_steps),
        "accepted_step_count": accepted_step_count,
        "search_owner": "forgecad-runtime",
        "observation_policy": "one_candidate_bound_agentic_observation_per_step",
        "steps": normalized_steps,
        "canonical_sha256": "",
    }
    lineage["canonical_sha256"] = canonical_hash(lineage)
    return lineage


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


def reference_view_receipt_summary(records: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Project local/runtime view bindings without paths or image content."""
    summary: list[dict[str, Any]] = []
    for record in records:
        intake = record.get("intake") if isinstance(record.get("intake"), dict) else {}
        intake_sha = record.get("intake_source_sha256")
        summary.append({
            "kind": record.get("kind"),
            "view_id": record.get("view_id"),
            "reference_id": record.get("reference_id"),
            "reference_sha256": record.get("reference_sha256") or record.get("source_sha256"),
            "width": record.get("width"),
            "height": record.get("height"),
            "visual_intake": {
                "status": "PROVIDED" if isinstance(intake_sha, str) else "NOT_PROVIDED",
                "source_sha256": intake_sha,
                "landmark_count": len(intake.get("landmarks", [])) if isinstance(intake.get("landmarks"), list) else 0,
                "region_count": len(intake.get("regions", [])) if isinstance(intake.get("regions"), list) else 0,
            },
        })
    return summary


def view_spec(
    reference_id: str,
    reference_sha: str,
    width: int,
    height: int,
    intake: dict[str, list[dict[str, Any]]] | None = None,
    *,
    kind: str = "perspective",
    view_id: str | None = None,
) -> dict[str, Any]:
    intake = intake or {"landmarks": [], "regions": []}
    if kind not in REFERENCE_VIEW_KINDS:
        raise ValueError(f"unsupported ReferenceViewSpec kind: {kind}")
    source_view = {
        "perspective": "three-quarter",
        "front": "front",
        "back": "back",
        "left": "left",
        "right": "right",
        "rear-three-quarter": "rear-three-quarter",
    }.get(kind, "unknown")
    value: dict[str, Any] = {
        "schema_version": "ReferenceViewSpec@1",
        "reference_id": reference_id,
        "reference_sha256": reference_sha,
        "view_id": view_id or stable_view_id(kind),
        "source_view": source_view,
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


def reference_canvas_authoring_context(
    project_id: str,
    reference_id: str,
    reference_sha: str,
    width: int,
    height: int,
    intake: dict[str, list[dict[str, Any]]] | None = None,
) -> dict[str, Any]:
    """Build one explicit, hash-bound authoring context for the visual turn.

    The Runtime remains the producer and validator.  This helper only builds
    the closed typed facts that the real Codex turn is allowed to submit.  It
    intentionally records one supplied perspective view plus missing coverage
    instead of letting the observation projection fall back to an unqualified
    "unknown" canvas.  The CAS object hash of the finalized canvas is computed
    locally solely so DesignSpec@1 can bind to the Runtime-created canvas
    object; the Runtime recomputes and verifies both canonical hashes.
    """
    intake = intake or {"landmarks": [], "regions": []}
    evidence = {"kind": "reference", "sha256": reference_sha}

    def state(visibility: str, confidence: float) -> dict[str, Any]:
        return {
            "visibility": visibility,
            "confidence": confidence,
            "evidence_refs": [dict(evidence)],
        }

    visible_regions: list[dict[str, Any]] = []
    for region in intake.get("regions", []):
        region_id = str(region["region_id"])
        visibility = region["visibility"] if region["visibility"] in {"observed", "inferred", "unknown"} else "unknown"
        confidence = float(region["confidence"]) if visibility != "unknown" else 0.0
        visible_regions.append({
            "region_id": region_id,
            "label": region_id.replace("-", " "),
            "state": state(visibility, max(0.0, min(1.0, confidence))),
        })

    canvas_id = "reference-canvas-real-codex"
    spec_id = "design-spec-real-codex"
    created_at = datetime.now(timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z")
    canvas: dict[str, Any] = {
        "schema_version": "ReferenceCanvas@1",
        "canvas_id": canvas_id,
        "project_id": project_id,
        "reference_set_sha256": reference_sha,
        "views": [{
            "view_id": "three-quarter-user-reference",
            "reference_id": reference_id,
            "reference_sha256": reference_sha,
            "kind": "perspective",
            "authorization": {
                "user_authorized": True,
                "declaration": "The user supplied and authorized this reference for local ForgeCAD modeling.",
                "evidence_refs": [dict(evidence)],
            },
            "image_dimensions": {"width": width, "height": height},
            "camera_claim": {
                "visibility": "unknown",
                "camera_hash": None,
                "claim": "Camera parameters are unknown for this supplied perspective reference.",
                "evidence_refs": [dict(evidence)],
            },
            "visible_regions": visible_regions,
            "unknown_regions": [
                {
                    "region_id": "unknown-hidden-side-geometry",
                    "question": "Which forms continue around the hidden side and rear surfaces?",
                    "state": state("unknown", 0.0),
                },
                {
                    "region_id": "unknown-camera-calibration",
                    "question": "What physical camera and focal length produced this perspective?",
                    "state": state("unknown", 0.0),
                },
            ],
        }],
        "coverage": {
            "required_views": ["front", "back", "left", "right", "perspective", "rear-three-quarter"],
            "supplied_views": ["perspective"],
            "missing_views": ["front", "back", "left", "right", "rear-three-quarter"],
            "coverage_status": "blocked",
            "hq_360_status": "BLOCKED_REFERENCE_COVERAGE",
            "evidence_refs": [dict(evidence)],
        },
        "unknowns": [
            {
                "unknown_id": "unknown-reference-coverage",
                "scope_kind": "scene",
                "scope_id": "scene",
                "question": "Are front, back, side and rear-three-quarter references available?",
                "state": state("unknown", 0.0),
            },
            {
                "unknown_id": "unknown-hidden-depth",
                "scope_kind": "scene",
                "scope_id": "scene",
                "question": "What is the true depth and hidden assembly behind the supplied view?",
                "state": state("unknown", 0.0),
            },
        ],
        "claims": [{
            "claim_id": "claim-supplied-perspective",
            "subject_kind": "view",
            "subject_id": "three-quarter-user-reference",
            "statement": "The supplied image is one user-authorized perspective view; hidden coverage remains unknown.",
            "state": state("observed", 0.98),
        }],
        "canonical_sha256": "",
        "created_at": created_at,
    }
    # Codex/serde serializes integral floats such as 0.0 and 1.0 as JSON
    # integers.  ForgeCAD canonical hashes are JSON-number-type sensitive, so
    # normalize the exact wire representation before deriving either object
    # hash or the DesignSpec canvas binding.
    canvas = normalize_numeric_representation(canvas)
    # Runtime keeps two related but distinct digests: the object canonical
    # field is computed with that field blank, while DesignSpec binds to the
    # CAS object hash of the fully canonicalized JSON that Runtime stores.
    canvas_canonical_sha256 = canonical_hash(canvas)
    canvas["canonical_sha256"] = canvas_canonical_sha256
    canvas_object_sha256 = canonical_hash(canvas)

    def gate(
        stage: str,
        required_checks: list[str],
        failed_checks: list[str],
        locks: list[str],
    ) -> dict[str, Any]:
        return {
            "stage": stage,
            "status": "unknown",
            "required_checks": required_checks,
            "failed_checks": failed_checks,
            "evidence_hashes": [reference_sha],
            "unlocks": ["checkpoint", "mark-unknown"],
            "locks": locks,
        }

    spec: dict[str, Any] = {
        "schema_version": "DesignSpec@1",
        "spec_id": spec_id,
        "project_id": project_id,
        "reference_canvas_id": canvas_id,
        "reference_canvas_sha256": canvas_object_sha256,
        "category": "hard-surface humanoid visual asset",
        "style": "white shell with dark mechanical understructure, inferred from one perspective view",
        "primary_forms": [{
            "form_id": "humanoid-primary-form",
            "name": "Humanoid primary form",
            "role": "main-body",
            "description": "Visible body envelope; hidden depth remains unknown.",
            "state": state("inferred", 0.82),
        }],
        "proportions": [],
        "semantic_parts": [{
            "part_id": "scene",
            "role": "root",
            "parent_id": None,
            "symmetry": "unknown",
            "material_zone_ids": ["zone-white-shell", "zone-black-mechanical"],
            "state": state("inferred", 0.72),
        }],
        # Material language is deliberately deferred to the later UV/PBR
        # stage.  The ReferenceCanvas still records the visible shell/core
        # observation, while this durable authoring payload stays small enough
        # for a real Codex tool call to copy byte-for-byte.
        "material_language": [],
        "stage_goals": [
            {
                "stage": "reference-canvas",
                "objective": "Bind reference and coverage before primary form.",
                "allowed_action_kinds": ["coverage-annotation", "checkpoint"],
                "forbidden_action_kinds": ["tertiary-detail", "uv-pbr", "export"],
                "exit_gate": gate("reference-canvas", ["reference-authorized"], ["reference-coverage"], ["tertiary-detail", "uv-pbr", "export"]),
            },
            {
                "stage": "primary-form",
                "objective": "Converge visible primary silhouette and proportions.",
                "allowed_action_kinds": ["primary-blockout", "bounded-repair", "checkpoint"],
                "forbidden_action_kinds": ["tertiary-detail", "uv-pbr", "export"],
                "exit_gate": gate("primary-form", ["primary-silhouette"], ["primary-silhouette", "primary-proportion"], ["tertiary-detail", "uv-pbr", "export"]),
            },
            {
                "stage": "secondary-structure",
                "objective": "Add secondary structure after primary form.",
                "allowed_action_kinds": ["secondary-structure", "bounded-repair", "checkpoint"],
                "forbidden_action_kinds": ["tertiary-detail", "uv-pbr", "export"],
                "exit_gate": gate("secondary-structure", ["secondary-structure"], ["visible-view"], ["tertiary-detail", "uv-pbr", "export"]),
            },
            {
                "stage": "tertiary-detail",
                "objective": "Keep tertiary detail locked until form and coverage pass.",
                "allowed_action_kinds": ["tertiary-detail", "checkpoint"],
                "forbidden_action_kinds": ["uv-pbr", "export"],
                "exit_gate": gate("tertiary-detail", ["tertiary-detail", "visible-view"], ["visible-view"], ["uv-pbr", "export"]),
            },
            {
                "stage": "uv-pbr",
                "objective": "Bind UV, tangent and PBR after geometry gates.",
                "allowed_action_kinds": ["material-zone", "uv-pbr", "checkpoint"],
                "forbidden_action_kinds": ["export"],
                "exit_gate": gate("uv-pbr", ["uv-tangent-pbr", "visible-view"], ["uv-tangent-pbr"], ["export"]),
            },
            {
                "stage": "final-review",
                "objective": "Require comparison, review and restart-safe export evidence.",
                "allowed_action_kinds": ["final-review", "human-review", "export", "checkpoint"],
                "forbidden_action_kinds": [],
                "exit_gate": gate("final-review", ["multi-view-compare"], ["multi-view-compare", "human-review", "export-restart-hash"], ["export"]),
            },
        ],
        "risks": [],
        "unknowns": [],
        "canonical_sha256": "",
        "created_at": created_at,
    }
    spec = normalize_numeric_representation(spec)
    spec["canonical_sha256"] = canonical_hash(spec)
    # The Runtime is the canonical producer.  Leave these two producer-owned
    # fields blank on the Codex wire so a model cannot accidentally replace a
    # field hash with the full object hash while copying the payload.  The
    # DesignSpec still carries the exact expected CAS object hash for the
    # Runtime-created ReferenceCanvas, so binding remains fail-closed.
    canvas["canonical_sha256"] = ""
    spec["canonical_sha256"] = ""
    return {
        "reference_canvas": canvas,
        "design_spec": spec,
    }


def reference_canvas_authoring_context_multi(
    project_id: str,
    reference_records: list[dict[str, Any]],
) -> dict[str, Any]:
    """Build an explicit multi-view canvas from imported Runtime references.

    Every record is already bound to a live ``reference_import`` readback.  No
    image transformation or inferred view is allowed here; the helper only
    projects those exact references into the bounded authoring contracts.
    """
    if not reference_records or len(reference_records) > 32:
        raise ValueError("multi-view authoring requires 1-32 imported references")
    normalized: list[dict[str, Any]] = []
    seen_kinds: set[str] = set()
    seen_view_ids: set[str] = set()
    for record in reference_records:
        if not isinstance(record, dict):
            raise ValueError("reference record must be an object")
        kind = record.get("kind")
        view_id = record.get("view_id")
        reference_id = record.get("reference_id")
        reference_sha = record.get("reference_sha256")
        width = record.get("width")
        height = record.get("height")
        intake = record.get("intake") or {"landmarks": [], "regions": []}
        if (
            not isinstance(kind, str)
            or kind not in REFERENCE_VIEW_KINDS
            or kind in seen_kinds
            or not isinstance(view_id, str)
            or not view_id
            or view_id in seen_view_ids
            or not isinstance(reference_id, str)
            or not reference_id
            or not isinstance(reference_sha, str)
            or not re.fullmatch(r"[0-9a-f]{64}", reference_sha)
            or not isinstance(width, int)
            or not isinstance(height, int)
            or width <= 0
            or height <= 0
            or not isinstance(intake, dict)
        ):
            raise ValueError("invalid multi-view reference record")
        seen_kinds.add(kind)
        seen_view_ids.add(view_id)
        normalized.append({
            "kind": kind,
            "view_id": view_id,
            "reference_id": reference_id,
            "reference_sha256": reference_sha,
            "width": width,
            "height": height,
            "intake": intake,
        })
    if not any(record["kind"] == "perspective" for record in normalized):
        raise ValueError("multi-view authoring requires the primary perspective reference")

    reference_pairs = sorted(
        (record["reference_id"], record["reference_sha256"])
        for record in normalized
    )
    reference_set_sha256 = canonical_hash([
        {"reference_id": reference_id, "reference_sha256": reference_sha}
        for reference_id, reference_sha in reference_pairs
    ])
    evidence_by_sha: dict[str, dict[str, str]] = {}
    for record in normalized:
        reference_sha = record["reference_sha256"]
        evidence_by_sha.setdefault(reference_sha, {"kind": "reference", "sha256": reference_sha})
    all_evidence = list(evidence_by_sha.values())
    required_views = list(REQUIRED_COVERAGE_VIEWS)
    required_views.extend(
        record["kind"]
        for record in normalized
        if record["kind"] not in required_views
    )
    supplied_kinds = {record["kind"] for record in normalized}
    supplied_views = [kind for kind in required_views if kind in supplied_kinds]
    missing_views = [kind for kind in required_views if kind not in supplied_kinds]
    coverage_status = "complete" if not missing_views else (
        "blocked" if supplied_views == ["perspective"] else "partial"
    )
    hq_360_status = "eligible" if not missing_views else "BLOCKED_REFERENCE_COVERAGE"
    created_at = datetime.now(timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z")

    def state(
        visibility: str,
        confidence: float,
        evidence_refs: list[dict[str, str]],
    ) -> dict[str, Any]:
        return {
            "visibility": visibility,
            "confidence": confidence,
            "evidence_refs": [dict(item) for item in evidence_refs],
        }

    def visible_regions(record: dict[str, Any]) -> list[dict[str, Any]]:
        result: list[dict[str, Any]] = []
        intake = record["intake"]
        for index, region in enumerate(intake.get("regions", [])):
            if not isinstance(region, dict):
                continue
            region_id = region.get("region_id")
            if not isinstance(region_id, str) or not region_id:
                continue
            visibility = region.get("visibility")
            if visibility not in {"observed", "inferred", "unknown"}:
                visibility = "unknown"
            confidence = region.get("confidence", 0.0)
            if not isinstance(confidence, (int, float)) or not math.isfinite(float(confidence)):
                confidence = 0.0
            confidence = max(0.0, min(1.0, float(confidence))) if visibility != "unknown" else 0.0
            evidence = [evidence_by_sha[record["reference_sha256"]]]
            result.append({
                "region_id": region_id,
                "label": f"visible region {index + 1}",
                "state": state(visibility, confidence, evidence),
            })
        return result

    def unknown_regions(record: dict[str, Any]) -> list[dict[str, Any]]:
        evidence = [evidence_by_sha[record["reference_sha256"]]]
        kind = record["kind"]
        return [
            {
                "region_id": f"unknown-{kind}-hidden-geometry",
                "question": "Which forms continue around the hidden surfaces of this supplied view?",
                "state": state("unknown", 0.0, evidence),
            },
        ]

    views: list[dict[str, Any]] = []
    claims: list[dict[str, Any]] = []
    for record in normalized:
        evidence = [evidence_by_sha[record["reference_sha256"]]]
        views.append({
            "view_id": record["view_id"],
            "reference_id": record["reference_id"],
            "reference_sha256": record["reference_sha256"],
            "kind": record["kind"],
            "authorization": {
                "user_authorized": True,
                "declaration": "The user supplied and authorized this reference for local ForgeCAD modeling.",
                "evidence_refs": [dict(item) for item in evidence],
            },
            "image_dimensions": {"width": record["width"], "height": record["height"]},
            "camera_claim": {
                "visibility": "unknown",
                "camera_hash": None,
                "claim": "Camera parameters are unknown for this supplied reference view.",
                "evidence_refs": [dict(item) for item in evidence],
            },
            "visible_regions": visible_regions(record),
            "unknown_regions": unknown_regions(record),
        })
    claims.append({
        "claim_id": "claim-supplied-reference-set",
        "subject_kind": "canvas",
        "subject_id": "reference-canvas-real-codex",
        "statement": "The listed reference views are user-authorized; hidden geometry and camera calibration remain explicit.",
        "state": state("observed", 0.98, all_evidence),
    })

    canvas_id = "reference-canvas-real-codex"
    spec_id = "design-spec-real-codex"
    canvas: dict[str, Any] = {
        "schema_version": "ReferenceCanvas@1",
        "canvas_id": canvas_id,
        "project_id": project_id,
        "reference_set_sha256": reference_set_sha256,
        "views": views,
        "coverage": {
            "required_views": required_views,
            "supplied_views": supplied_views,
            "missing_views": missing_views,
            "coverage_status": coverage_status,
            "hq_360_status": hq_360_status,
            "evidence_refs": [dict(item) for item in all_evidence],
        },
        "unknowns": [
            {
                "unknown_id": "unknown-reference-coverage",
                "scope_kind": "scene",
                "scope_id": "scene",
                "question": "Which required reference views are still unavailable?",
                "state": state("unknown", 0.0, all_evidence),
            },
            {
                "unknown_id": "unknown-hidden-depth",
                "scope_kind": "scene",
                "scope_id": "scene",
                "question": "What is the true depth and hidden assembly behind the supplied views?",
                "state": state("unknown", 0.0, all_evidence),
            },
        ],
        "claims": claims,
        "canonical_sha256": "",
        "created_at": created_at,
    }
    canvas = normalize_numeric_representation(canvas)
    canvas["canonical_sha256"] = canonical_hash(canvas)
    canvas_object_sha256 = canonical_hash(canvas)

    def gate(
        stage: str,
        required_checks: list[str],
        failed_checks: list[str],
        locks: list[str],
    ) -> dict[str, Any]:
        return {
            "stage": stage,
            "status": "unknown",
            "required_checks": required_checks,
            "failed_checks": failed_checks,
            "evidence_hashes": [record["reference_sha256"] for record in normalized],
            "unlocks": ["checkpoint", "mark-unknown"],
            "locks": locks,
        }

    spec: dict[str, Any] = {
        "schema_version": "DesignSpec@1",
        "spec_id": spec_id,
        "project_id": project_id,
        "reference_canvas_id": canvas_id,
        "reference_canvas_sha256": canvas_object_sha256,
        "category": "hard-surface humanoid visual asset",
        "style": f"white shell with dark mechanical understructure, inferred from {len(normalized)} authorized view(s)",
        "primary_forms": [{
            "state": state("inferred", 0.82, all_evidence),
        }],
        "proportions": [],
        "semantic_parts": [{
            "state": state("inferred", 0.72, all_evidence),
        }],
        "material_language": [],
        "stage_goals": [
            {
                "stage": "reference-canvas",
                "objective": "Bind authorized references before primary form.",
                "exit_gate": {"stage": "reference-canvas", "status": "unknown"},
            },
            {
                "stage": "primary-form",
                "objective": "Converge primary silhouette across supplied views.",
                "exit_gate": {"stage": "primary-form", "status": "unknown"},
            },
            {
                "stage": "secondary-structure",
                "objective": "Add secondary structure after primary form.",
                "exit_gate": {"stage": "secondary-structure", "status": "unknown"},
            },
            {
                "stage": "tertiary-detail",
                "objective": "Keep tertiary detail locked until form and coverage pass.",
                "exit_gate": {"stage": "tertiary-detail", "status": "unknown"},
            },
            {
                "stage": "uv-pbr",
                "objective": "Bind UV, tangent and PBR after geometry gates.",
                "exit_gate": {"stage": "uv-pbr", "status": "unknown"},
            },
            {
                "stage": "final-review",
                "objective": "Require multi-view comparison, review and restart-safe export evidence.",
                "exit_gate": {"stage": "final-review", "status": "unknown"},
            },
        ],
        "risks": [],
        "unknowns": [],
        "canonical_sha256": "",
        "created_at": created_at,
    }
    spec = normalize_numeric_representation(spec)
    spec["canonical_sha256"] = canonical_hash(spec)
    # See the single-view authoring context above: Runtime owns these
    # canonical fields, while the DesignSpec's canvas binding remains the
    # precomputed hash of the fully canonicalized Runtime canvas object.
    canvas["canonical_sha256"] = ""
    spec["canonical_sha256"] = ""
    return {"reference_canvas": canvas, "design_spec": spec}


def setup_prompt_multi(reference_inputs: list[dict[str, Any]]) -> str:
    """Create a bounded setup turn for explicitly supplied reference views."""
    if not reference_inputs or len(reference_inputs) > 9:
        raise ValueError("setup requires 1-9 reference inputs")
    imports: list[str] = []
    gets: list[str] = []
    for index, item in enumerate(reference_inputs, start=1):
        kind = item.get("kind")
        path = item.get("path")
        source_sha = item.get("source_sha256")
        if not isinstance(kind, str) or not isinstance(path, str) or not isinstance(source_sha, str):
            raise ValueError("setup reference input is incomplete")
        imports.append(
            f'{index}) reference_import for the {kind} view with this exact JSON object: '
            f'{{"project_id":<saved project_id>,"source":{{"kind":"codex_local_file","path":{json.dumps(path, ensure_ascii=False)}}},'
            f'"authorization":{{"user_authorized":true,"declaration":"The user supplied and authorized this reference for local ForgeCAD modeling."}},'
            f'"expected_sha256":{json.dumps(source_sha)}}}; save the returned reference_id and object_sha256.'
        )
        gets.append(
            f'{len(reference_inputs) + index}) reference_get for the {kind} view with '
            'the reference_id returned by the matching import; verify reference_id and object_sha256 exactly.'
        )
    steps = "\n".join(imports + gets)
    return f"""Use only the ForgeCAD MCP server. Do not use shell, filesystem, browser, other MCP servers, or arbitrary code.

This is the first setup turn for a real MCP010C host gate. The mandatory Ponytail preflight has already been requested by the host wrapper. After it succeeds, call exactly these ForgeCAD tools in order, then stop:
1) project_create with name="MCP010C Codex visual review" and policy={{"profile":"mvp"}}; save project_id.
{steps}

Do not request or print image bytes. Return the project_id, one import result and one metadata readback for each named view, preserving the listed order. Do not claim similarity, high quality, PBR, human approval or 360-degree coverage.
"""


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
            {"parameter_id": "upper-arm-height", "part_id": "upper-arm-pair", "semantic": "height", "value": 1.0, "min": 0.80, "max": 1.20, "step": 0.04, "unit": "ratio"},
            {"parameter_id": "forearm-width", "part_id": "forearm-pair", "semantic": "width", "value": 1.0, "min": 0.84, "max": 1.16, "step": 0.04, "unit": "ratio"},
            {"parameter_id": "forearm-height", "part_id": "forearm-pair", "semantic": "height", "value": 1.0, "min": 0.80, "max": 1.20, "step": 0.04, "unit": "ratio"},
            {"parameter_id": "pelvis-width", "part_id": "pelvis", "semantic": "width", "value": 1.0, "min": 0.84, "max": 1.16, "step": 0.04, "unit": "ratio"},
            {"parameter_id": "pelvis-height", "part_id": "pelvis", "semantic": "height", "value": 1.0, "min": 0.88, "max": 1.12, "step": 0.04, "unit": "ratio"},
            {"parameter_id": "hip-width", "part_id": "hip-pair", "semantic": "width", "value": 1.0, "min": 0.84, "max": 1.16, "step": 0.04, "unit": "ratio"},
            {"parameter_id": "hip-height", "part_id": "hip-pair", "semantic": "height", "value": 1.0, "min": 0.80, "max": 1.20, "step": 0.04, "unit": "ratio"},
            {"parameter_id": "thigh-width", "part_id": "thigh-pair", "semantic": "width", "value": 1.0, "min": 0.84, "max": 1.16, "step": 0.04, "unit": "ratio"},
            {"parameter_id": "thigh-height", "part_id": "thigh-pair", "semantic": "height", "value": 1.0, "min": 0.80, "max": 1.20, "step": 0.04, "unit": "ratio"},
            {"parameter_id": "shin-width", "part_id": "shin-pair", "semantic": "width", "value": 1.0, "min": 0.84, "max": 1.16, "step": 0.04, "unit": "ratio"},
            {"parameter_id": "shin-height", "part_id": "shin-pair", "semantic": "height", "value": 1.0, "min": 0.80, "max": 1.20, "step": 0.04, "unit": "ratio"},
            # Landmark ownership is explicit and camera-calibrated in Runtime.
            # These are bounded camera-plane meter offsets, not Codex-side
            # pixel nudges or an open-ended parameter trace.
            {"parameter_id": "head-offset-x", "part_id": "head-shell", "semantic": "offset_x", "value": 0.0, "min": -0.35, "max": 0.35, "step": 0.05, "unit": "meter"},
            {"parameter_id": "head-offset-y", "part_id": "head-shell", "semantic": "offset_y", "value": 0.0, "min": -0.35, "max": 0.35, "step": 0.05, "unit": "meter"},
            # Boundary attribution can identify visible hand/shin drift even
            # when the reference target has no explicit Part contour slices.
            # Keep these as bounded typed camera-plane controls; Codex does
            # not search their values or issue image-space nudges.
            {"parameter_id": "hand-offset-x", "part_id": "hand-pair", "semantic": "offset_x", "value": 0.0, "min": -0.35, "max": 0.35, "step": 0.05, "unit": "meter"},
            {"parameter_id": "hand-offset-y", "part_id": "hand-pair", "semantic": "offset_y", "value": 0.0, "min": -0.45, "max": 0.45, "step": 0.05, "unit": "meter"},
            {"parameter_id": "chest-offset-y", "part_id": "chest-shell", "semantic": "offset_y", "value": 0.0, "min": -0.35, "max": 0.35, "step": 0.05, "unit": "meter"},
            {"parameter_id": "chest-offset-x", "part_id": "chest-shell", "semantic": "offset_x", "value": 0.0, "min": -0.35, "max": 0.35, "step": 0.05, "unit": "meter"},
            {"parameter_id": "shoulder-offset-x", "part_id": "shoulder-armor-pair", "semantic": "offset_x", "value": 0.0, "min": -0.35, "max": 0.35, "step": 0.05, "unit": "meter"},
            {"parameter_id": "shoulder-offset-y", "part_id": "shoulder-armor-pair", "semantic": "offset_y", "value": 0.0, "min": -0.35, "max": 0.35, "step": 0.05, "unit": "meter"},
            {"parameter_id": "elbow-offset-x", "part_id": "elbow-pair", "semantic": "offset_x", "value": 0.0, "min": -0.35, "max": 0.35, "step": 0.05, "unit": "meter"},
            {"parameter_id": "elbow-offset-y", "part_id": "elbow-pair", "semantic": "offset_y", "value": 0.0, "min": -0.45, "max": 0.45, "step": 0.05, "unit": "meter"},
            {"parameter_id": "pelvis-offset-y", "part_id": "pelvis", "semantic": "offset_y", "value": 0.0, "min": -0.35, "max": 0.35, "step": 0.05, "unit": "meter"},
            {"parameter_id": "pelvis-offset-x", "part_id": "pelvis", "semantic": "offset_x", "value": 0.0, "min": -0.35, "max": 0.35, "step": 0.05, "unit": "meter"},
            {"parameter_id": "hip-offset-x", "part_id": "hip-pair", "semantic": "offset_x", "value": 0.0, "min": -0.35, "max": 0.35, "step": 0.05, "unit": "meter"},
            {"parameter_id": "hip-offset-y", "part_id": "hip-pair", "semantic": "offset_y", "value": 0.0, "min": -0.45, "max": 0.45, "step": 0.05, "unit": "meter"},
            {"parameter_id": "knee-offset-x", "part_id": "knee-pair", "semantic": "offset_x", "value": 0.0, "min": -0.35, "max": 0.35, "step": 0.05, "unit": "meter"},
            {"parameter_id": "knee-offset-y", "part_id": "knee-pair", "semantic": "offset_y", "value": 0.0, "min": -0.45, "max": 0.45, "step": 0.05, "unit": "meter"},
            {"parameter_id": "shin-offset-x", "part_id": "shin-pair", "semantic": "offset_x", "value": 0.0, "min": -0.35, "max": 0.35, "step": 0.05, "unit": "meter"},
            {"parameter_id": "shin-offset-y", "part_id": "shin-pair", "semantic": "offset_y", "value": 0.0, "min": -0.45, "max": 0.45, "step": 0.05, "unit": "meter"},
        ],
    }


def part_contour_rig_draft(candidate_id: str, part_id: str) -> dict[str, Any]:
    """Build one exact-Part typed Rig for the bounded contour proposal route.

    Explicit left/right IDs are intentional: a pair Rig remains bilateral,
    while this route can only move the semantic Part that the Runtime
    Part-ID evidence selected.  Runtime still validates the candidate-bound
    hash and returns a read-only proposal; it never edits or confirms a mesh.
    """
    safe_part_id = re.sub(r"[^A-Za-z0-9_.-]", "-", part_id).strip("-") or "part"
    return {
        "schema_version": "SilhouetteRig@1",
        "rig_id": f"part-contour-{safe_part_id}",
        "candidate_id": candidate_id,
        "parameters": [
            {"parameter_id": f"{safe_part_id}-width", "part_id": part_id, "semantic": "width", "value": 1.0, "min": 0.84, "max": 1.16, "step": 0.04, "unit": "ratio"},
            {"parameter_id": f"{safe_part_id}-height", "part_id": part_id, "semantic": "height", "value": 1.0, "min": 0.80, "max": 1.20, "step": 0.04, "unit": "ratio"},
            {"parameter_id": f"{safe_part_id}-offset-x", "part_id": part_id, "semantic": "offset_x", "value": 0.0, "min": -0.35, "max": 0.35, "step": 0.05, "unit": "meter"},
            {"parameter_id": f"{safe_part_id}-offset-y", "part_id": part_id, "semantic": "offset_y", "value": 0.0, "min": -0.45, "max": 0.45, "step": 0.05, "unit": "meter"},
        ],
    }


def silhouette_prompt(
    project_id: str,
    reference_id: str,
    candidate_id: str,
    job_id: str,
    artifact_id: str,
    target_sha256: str,
    view: dict[str, Any],
    authoring_context: dict[str, Any],
) -> str:
    rig = json.dumps(silhouette_rig_draft(candidate_id), ensure_ascii=False, separators=(",", ":"))
    view_json = json.dumps(view, ensure_ascii=False, separators=(",", ":"))
    authoring_json = json.dumps(authoring_context, ensure_ascii=False, separators=(",", ":"))
    return f"""Use only the ForgeCAD MCP server. Do not use shell, filesystem, browser, other MCP servers or arbitrary code.

Call exactly these nine ForgeCAD tools in order, then stop:
1) silhouette_target_get with {{"target_sha256":{json.dumps(target_sha256)}}}; verify it is the target for project {json.dumps(project_id)}.
2) camera_fit_prepare with {{"project_id":{json.dumps(project_id)},"candidate_id":{json.dumps(candidate_id)},"target_sha256":{json.dumps(target_sha256)},"camera":null}}; save the complete returned selected_camera calibration object (not only camera_hash/canonical_sha256) and its hashes.
3) job_get with {{"job_id":{json.dumps(job_id)}}}.
4) candidate_get with {{"candidate_id":{json.dumps(candidate_id)}}}.
5) artifact_readback_get with {{"artifact_id":{json.dumps(artifact_id)},"candidate_id":{json.dumps(candidate_id)}}}.
6) reference_compare_prepare with this exact JSON object: {{"project_id":{json.dumps(project_id)},"candidate_id":{json.dumps(candidate_id)},"reference_id":{json.dumps(reference_id)},"view_spec":{view_json},"camera":{{"schema_version":"CameraCalibrationRef@1","camera_hash":<copy the selected camera_hash from step 2>,"canonical_sha256":<copy the selected camera canonical_sha256 from step 2>}},"target_sha256":{json.dumps(target_sha256)}}}. Copy the view_spec byte-for-byte and do not reconstruct camera fields.
7) session_create_or_resume with {{"session_id":null,"project_id":{json.dumps(project_id)},"candidate_id":{json.dumps(candidate_id)},"idempotency_key":{json.dumps("reference-canvas-" + candidate_id)},"reference_id":{json.dumps(reference_id)},"design_spec_id":"design-spec-real-codex","reference_canvas_id":"reference-canvas-real-codex","camera_hash":<copy the selected camera_hash from step 2>,"evidence_sha256":<copy the comparison_report_object_sha256 from step 6>,"approved":true,"approval_receipt_id":"mcp010c-reference-canvas-approval","approval_summary":"Create isolated reference canvas context","authoring_context":{authoring_json}}}. This is an isolated Runtime metadata write only: it must not confirm, version, export or mutate the candidate. Copy the authoring_context byte-for-byte.
8) scene_observe_get with {{"project_id":{json.dumps(project_id)},"candidate_id":{json.dumps(candidate_id)}}}; call this only after session_create_or_resume. Treat the complete returned AgenticSceneObserveResult@1 as the only canonical scene/model/reference/quality/Part-error context for this visual turn. Verify its project/candidate binding, read_only flag and canonical_sha256, and verify its canonical_sha256 equals the session.session.observation_sha256 returned by step 7; do not replace it with fragmented boundary or quality reads.
9) silhouette_rig_hash with {{"schema_version":"SilhouetteRigHashRequest@1","project_id":{json.dumps(project_id)},"candidate_id":{json.dumps(candidate_id)},"rig_draft":{rig}}}; copy only the returned canonical_sha256 into the unchanged rig for the next turn.

Do not call silhouette_fit_prepare yet because the next turn will bind its request hash to the exact selected camera. Do not call render_pass_get, review, confirm or export. Return only target/camera/compare/session/observation/Rig hashes and opaque IDs; do not claim visual quality.
"""


def silhouette_core_prompt(
    project_id: str,
    reference_id: str,
    candidate_id: str,
    job_id: str,
    artifact_id: str,
    target_sha256: str,
    view: dict[str, Any],
) -> str:
    """Read the candidate and establish the exact camera/compare baseline.

    This is deliberately separate from the durable authoring payload.  It
    gives the later session_create_or_resume call concrete Runtime hashes
    without asking one Codex turn to hold nine calls and a full DesignSpec in
    working memory.
    """
    view_json = json.dumps(view, ensure_ascii=False, separators=(",", ":"))
    return f"""Use only the ForgeCAD MCP server. Do not use shell, filesystem, browser, other MCP servers or arbitrary code.

Call exactly these six ForgeCAD tools in order, then stop:
1) silhouette_target_get with {{"target_sha256":{json.dumps(target_sha256)}}}; verify it belongs to project {json.dumps(project_id)}.
2) camera_fit_prepare with {{"project_id":{json.dumps(project_id)},"candidate_id":{json.dumps(candidate_id)},"target_sha256":{json.dumps(target_sha256)},"camera":null}}; save the complete selected_camera calibration object and both hashes.
3) job_get with {{"job_id":{json.dumps(job_id)}}}.
4) candidate_get with {{"candidate_id":{json.dumps(candidate_id)}}}.
5) artifact_readback_get with {{"artifact_id":{json.dumps(artifact_id)},"candidate_id":{json.dumps(candidate_id)}}}.
6) reference_compare_prepare with this exact JSON object: {{"project_id":{json.dumps(project_id)},"candidate_id":{json.dumps(candidate_id)},"reference_id":{json.dumps(reference_id)},"view_spec":{view_json},"camera":{{"schema_version":"CameraCalibrationRef@1","camera_hash":<copy selected_camera.camera_hash from step 2>,"canonical_sha256":<copy selected_camera.canonical_sha256 from step 2>}},"target_sha256":{json.dumps(target_sha256)}}}. Copy view_spec byte-for-byte and do not reconstruct camera fields.

Do not call session_create_or_resume, scene_observe_get, silhouette_rig_hash, silhouette_fit_prepare, render, review, confirm or export in this turn. Return only the typed baseline objects and hashes; do not claim visual quality.
"""


def silhouette_authoring_prompt(
    project_id: str,
    reference_id: str,
    candidate_id: str,
    camera_hash: str,
    camera_canonical_sha256: str,
    evidence_sha256: str,
    authoring_context: dict[str, Any],
) -> str:
    authoring_json = json.dumps(authoring_context, ensure_ascii=False, separators=(",", ":"))
    return f"""Use only the ForgeCAD MCP server. Do not use shell, filesystem, browser, other MCP servers or arbitrary code.

Call exactly one ForgeCAD tool, then stop:
session_create_or_resume with {{"session_id":null,"project_id":{json.dumps(project_id)},"candidate_id":{json.dumps(candidate_id)},"idempotency_key":{json.dumps("reference-canvas-" + candidate_id)},"reference_id":{json.dumps(reference_id)},"design_spec_id":"design-spec-real-codex","reference_canvas_id":"reference-canvas-real-codex","camera_hash":{json.dumps(camera_hash)},"evidence_sha256":{json.dumps(evidence_sha256)},"approved":true,"approval_receipt_id":"mcp010c-reference-canvas-approval","approval_summary":"Create isolated reference canvas context","authoring_context":{authoring_json}}}

This is an isolated Runtime metadata write only. It must not confirm, version, export or mutate the candidate. Copy authoring_context byte-for-byte and return the complete typed AgenticSessionResult@1. Do not call scene_observe_get or any other ForgeCAD tool in this turn; do not claim visual quality.
The Runtime-selected camera canonical hash is {json.dumps(camera_canonical_sha256)} and must remain bound to the session's CameraCalibrationRef.
"""


def silhouette_runtime_default_authoring_prompt(
    project_id: str,
    reference_id: str,
    candidate_id: str,
    camera_hash: str,
    camera_canonical_sha256: str,
    evidence_sha256: str,
) -> str:
    return f"""Use only the ForgeCAD MCP server. Do not use shell, filesystem, browser, other MCP servers or arbitrary code.

Call exactly one ForgeCAD tool, then stop:
session_create_or_resume with {{"session_id":null,"project_id":{json.dumps(project_id)},"candidate_id":{json.dumps(candidate_id)},"idempotency_key":{json.dumps("reference-canvas-" + candidate_id)},"reference_id":{json.dumps(reference_id)},"design_spec_id":"design-spec-real-codex","reference_canvas_id":"reference-canvas-real-codex","camera_hash":{json.dumps(camera_hash)},"evidence_sha256":{json.dumps(evidence_sha256)},"approved":true,"approval_receipt_id":"mcp010c-reference-canvas-approval","approval_summary":"Create Runtime-owned conservative reference canvas context"}}

Do not include authoring_context. Runtime must create its own conservative, hash-bound ReferenceCanvas@1 and DesignSpec@1 for this exact project/reference/candidate binding. Return the complete typed AgenticSessionResult@1. This is isolated Runtime metadata only: do not confirm, version, export or mutate the candidate. Do not call scene_observe_get or any other ForgeCAD tool in this turn; do not claim visual quality.
The Runtime-selected camera canonical hash is {json.dumps(camera_canonical_sha256)} and must remain bound to the session's CameraCalibrationRef.
"""


def authoring_mode_summary(items: list[dict[str, Any]]) -> dict[str, Any]:
    """Report whether the successful durable session used explicit or default authoring."""
    successful = [
        item
        for item in items
        if item.get("type") == "mcp_tool_call"
        and item.get("server") == "forgecad"
        and item.get("tool") == "session_create_or_resume"
        and item.get("status") == "completed"
    ]
    failed = [
        item
        for item in items
        if item.get("type") == "mcp_tool_call"
        and item.get("server") == "forgecad"
        and item.get("tool") == "session_create_or_resume"
        and item.get("status") == "failed"
    ]
    codes = sorted({str(item.get("structured", {}).get("code")) for item in failed if isinstance(item.get("structured"), dict) and item.get("structured", {}).get("code")})
    if not successful:
        return {"status": "NOT_RUN", "explicit_failure_codes": codes}

    # Keep only digest/binding facts from the raw Codex event.  The compact
    # receipt intentionally omits the authoring payload itself, but without
    # these bounded facts a canonical mismatch cannot distinguish client
    # rewriting from a producer-side hash bug.
    argument_summary: dict[str, Any] | None = None
    authoring_calls = [
        item
        for item in items
        if item.get("type") == "mcp_tool_call"
        and item.get("server") == "forgecad"
        and item.get("tool") == "session_create_or_resume"
    ]
    for item in reversed(authoring_calls):
        arguments = item.get("arguments")
        if not isinstance(arguments, dict):
            continue
        context = arguments.get("authoring_context")
        if not isinstance(context, dict):
            argument_summary = {"authoring_context": "missing", "status": item.get("status")}
            continue

        def digest_pair(value: Any) -> dict[str, Any]:
            if not isinstance(value, dict):
                return {"present": False}
            declared = value.get("canonical_sha256")
            canonical_input = dict(value)
            canonical_input["canonical_sha256"] = ""
            recomputed = canonical_hash(canonical_input)
            return {
                "present": True,
                "declared": declared if isinstance(declared, str) else None,
                "declared_empty_for_runtime_canonicalization": declared == "",
                "recomputed": recomputed,
                "object_sha256": canonical_hash(value),
                "declared_matches_recomputed": declared == "" or declared == recomputed,
            }

        canvas = context.get("reference_canvas")
        spec = context.get("design_spec")
        canvas_summary = digest_pair(canvas)
        spec_summary = digest_pair(spec)
        argument_summary = {
            "status": item.get("status"),
            "context_keys": sorted(context),
            "canvas": canvas_summary,
            "design_spec": spec_summary,
            "spec_reference_canvas_sha256": spec.get("reference_canvas_sha256") if isinstance(spec, dict) else None,
            "expected_spec_binding_matches_canvas_object": (
                None
                if canvas_summary.get("declared_empty_for_runtime_canonicalization")
                else (
                    isinstance(spec, dict)
                    and spec.get("reference_canvas_sha256") == canvas_summary.get("object_sha256")
                )
            ),
        }
        break

    result = {
        "status": "EXPLICIT_CODEX_AUTHORING" if isinstance(successful[-1].get("arguments"), dict) and "authoring_context" in successful[-1].get("arguments", {}) else "RUNTIME_DEFAULT_AFTER_EXPLICIT_FAILURE" if codes else "RUNTIME_DEFAULT_AUTHORING",
        "explicit_failure_codes": codes,
    }
    if argument_summary is not None:
        result["argument_summary"] = argument_summary
    return result


def silhouette_observation_prompt(project_id: str, candidate_id: str) -> str:
    return f"""Use only the ForgeCAD MCP server. Do not use shell, filesystem, browser, other MCP servers or arbitrary code.

Call exactly one ForgeCAD tool, then stop:
scene_observe_get with {{"project_id":{json.dumps(project_id)},"candidate_id":{json.dumps(candidate_id)}}}.

This is the post-authoring canonical observation. Return the complete typed AgenticSceneObserveResult@1, including its canonical_sha256, project/candidate binding, read_only flag and design critic evidence. Do not call any other ForgeCAD tool, do not issue a second fragmented boundary/quality read, and do not claim visual quality.
"""


def silhouette_rig_prompt(project_id: str, candidate_id: str) -> str:
    rig = json.dumps(silhouette_rig_draft(candidate_id), ensure_ascii=False, separators=(",", ":"))
    return f"""Use only the ForgeCAD MCP server. Do not use shell, filesystem, browser, other MCP servers or arbitrary code.

Call exactly one ForgeCAD tool, then stop:
silhouette_rig_hash with {{"schema_version":"SilhouetteRigHashRequest@1","project_id":{json.dumps(project_id)},"candidate_id":{json.dumps(candidate_id)},"rig_draft":{rig}}}.

Return only the Runtime-owned canonical_sha256 for this unchanged bounded Rig. Do not call silhouette_fit_prepare, geometry_prepare, render, compare, confirm or export, and do not claim visual quality.
"""


def run_split_silhouette_observation(
    options: argparse.Namespace,
    environment: dict[str, str],
    workspace_root: str,
    turn_outputs: list[subprocess.CompletedProcess[str]],
    project_id: str,
    reference_id: str,
    candidate_id: str,
    job_id: str,
    artifact_id: str,
    target_sha256: str,
    view: dict[str, Any],
    authoring_context: dict[str, Any],
    allow_runtime_default_authoring: bool = True,
) -> list[dict[str, Any]]:
    """Run the real Codex observation boundary as four bounded MCP turns."""
    core_items = run_required_codex_turn(
        options,
        environment,
        silhouette_core_prompt(
            project_id,
            reference_id,
            candidate_id,
            job_id,
            artifact_id,
            target_sha256,
            view,
        ),
        workspace_root,
        SILHOUETTE_CORE_SEQUENCE,
        turn_outputs,
        "silhouette target/camera/compare baseline",
    )
    camera_result = structured_result(core_items, "camera_fit_prepare") or {}
    comparison = structured_result(core_items, "reference_compare_prepare") or {}
    selected_camera = field(camera_result, "selected_camera") or field(camera_result, "camera")
    if isinstance(selected_camera, dict) and not {
        "schema_version", "camera_hash", "projection", "transform", "fov_y_degrees",
        "near_m", "far_m", "resolution", "coordinate_system", "renderer_revision",
        "canonical_sha256",
    }.issubset(selected_camera):
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
    camera_hash = field(selected_camera or {}, "camera_hash") or field(camera_result, "selected_camera_hash") or field(camera_result, "camera_hash")
    camera_canonical = field(selected_camera or {}, "canonical_sha256")
    evidence_sha256 = field(comparison, "comparison_report_object_sha256") or field(comparison, "comparison_report_hash")
    if (
        not isinstance(selected_camera, dict)
        or not isinstance(camera_hash, str)
        or len(camera_hash) != 64
        or not isinstance(camera_canonical, str)
        or len(camera_canonical) != 64
        or not isinstance(evidence_sha256, str)
        or len(evidence_sha256) != 64
    ):
        raise RuntimeError("split silhouette baseline did not return complete camera/compare hashes")

    try:
        session_items = run_required_codex_turn(
            options,
            environment,
            silhouette_authoring_prompt(
                project_id,
                reference_id,
                candidate_id,
                camera_hash,
                camera_canonical,
                evidence_sha256,
                authoring_context,
            ),
            workspace_root,
            SILHOUETTE_AUTHORING_SEQUENCE,
            turn_outputs,
            "ReferenceCanvas/DesignSpec durable authoring",
            # A multi-view payload is the largest explicit authoring turn and
            # a real Codex may spend one turn explaining or selecting no tool
            # at all.  Retry that failed/no-call turn once; a completed
            # session still returns immediately and is never replayed.
            max_attempts=2 if not allow_runtime_default_authoring else 1,
        )
    except RuntimeError as explicit_error:
        # A real Codex can normalize or recompute a large nested authoring
        # payload while copying it into MCP arguments.  Do not retry that
        # non-idempotent request indefinitely.  The Runtime-owned default is
        # a safe, conservative durable canvas/spec producer and still gives
        # the observation turn one canonical context; retain the explicit
        # typed failure in the raw event stream and receipt mode summary.
        if not allow_runtime_default_authoring:
            raise RuntimeError(
                "MULTI_VIEW_AUTHORING_EXPLICIT_CONTEXT_REQUIRED: "
                f"Codex could not preserve the multi-view authoring payload: {explicit_error}"
            ) from explicit_error
        session_items = run_required_codex_turn(
            options,
            environment,
            silhouette_runtime_default_authoring_prompt(
                project_id,
                reference_id,
                candidate_id,
                camera_hash,
                camera_canonical,
                evidence_sha256,
            ),
            workspace_root,
            SILHOUETTE_AUTHORING_SEQUENCE,
            turn_outputs,
            "Runtime-owned default ReferenceCanvas/DesignSpec durable authoring",
            max_attempts=1,
        )
    observation_items = run_required_codex_turn(
        options,
        environment,
        silhouette_observation_prompt(project_id, candidate_id),
        workspace_root,
        SILHOUETTE_OBSERVATION_SEQUENCE,
        turn_outputs,
        "canonical Agentic scene observation",
    )
    rig_items = run_required_codex_turn(
        options,
        environment,
        silhouette_rig_prompt(project_id, candidate_id),
        workspace_root,
        SILHOUETTE_RIG_SEQUENCE,
        turn_outputs,
        "Runtime-owned silhouette Rig hash",
    )
    return core_items + session_items + observation_items + rig_items


def silhouette_fit_prompt(fit_request: dict[str, Any]) -> str:
    request_json = json.dumps(fit_request, ensure_ascii=False, separators=(",", ":"))
    return f"""Use only the ForgeCAD MCP server. Call exactly one ForgeCAD tool, then stop.
The MCP envelope is the hash-bound SilhouetteFitIntent payload; do not add a schema_version field (the Runtime dispatch contract intentionally does not accept one), do not stringify the optimizer object, and do not remove any field shown below:
silhouette_fit_prepare with this exact JSON object: {request_json}

Do not change any value, recompute any hash, call another tool, or claim that the model matches the reference. Return only the actual iterations, evaluations, selected camera/parameters, loss and status.
"""


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


def part_contour_prompt(project_id: str, candidate_id: str, target_sha256: str, part_id: str) -> str:
    rig_draft = part_contour_rig_draft(candidate_id, part_id)
    rig = json.dumps(rig_draft, ensure_ascii=False, separators=(",", ":"))
    rig_with_placeholder = dict(rig_draft)
    rig_with_placeholder["canonical_sha256"] = "RUNTIME_HASH"
    rig_payload = json.dumps(rig_with_placeholder, ensure_ascii=False, separators=(",", ":"))
    return f"""Use only the ForgeCAD MCP server. Do not use shell, filesystem, browser, other MCP servers or arbitrary code.

This is one bounded single-Part contour decision for project {json.dumps(project_id)} and candidate {json.dumps(candidate_id)}. The exact semantic Part selected by Runtime boundary evidence is {json.dumps(part_id)}.
Call exactly these two tools in order, then stop:
1) silhouette_rig_hash with {{"schema_version":"SilhouetteRigHashRequest@1","project_id":{json.dumps(project_id)},"candidate_id":{json.dumps(candidate_id)},"rig_draft":{rig}}}; save the Runtime-returned canonical_sha256.
2) part_contour_fit_prepare with {{"project_id":{json.dumps(project_id)},"candidate_id":{json.dumps(candidate_id)},"target_sha256":{json.dumps(target_sha256)},"part_id":{json.dumps(part_id)},"rig":{rig_payload}}}.
In step 2 replace the literal RUNTIME_HASH inside rig.canonical_sha256 with the exact hash from step 1. Do not call silhouette_fit_prepare, geometry_prepare, render, compare, confirm or export. Return only the typed PartContourFitResult@1. This is a read-only proposal and is not a likeness or quality pass.
"""


def primary_form_repair_prompt(request: dict[str, Any]) -> str:
    request_json = json.dumps(request, ensure_ascii=False, separators=(",", ":"))
    return f"""Use only the ForgeCAD MCP server. Do not use shell, filesystem, browser, images, other MCP servers or arbitrary code. Do not open or execute forgecad-mcp-workflow.md or any other local workflow file.

Complete this one bounded asynchronous action, then stop:
1) Call primary_form_repair_job_prepare with this exact JSON object: {request_json}
2) Poll job_get with the returned job_id until the Runtime job is terminal. Do not stop while it is queued or running.
3) When the job status is succeeded, call job_result_get with the exact job_id and return its typed result.

This is the single Runtime-owned Primary Form repair action. It must consume the
same target, canonical ReferenceViewSpec, camera reference, Rig and optimizer intent. Runtime owns the
nested bounded silhouette fit and the Geometry Worker/Render Worker compare;
do not call the synchronous primary_form_repair_prepare endpoint, do not call
silhouette_fit_prepare separately, do not edit any parameter, and do not call
geometry_prepare, render, compare, confirm or export separately in this turn.
Read the result field from job_result_get and return the typed
PrimaryFormRepairPrepareResult@1. A no_improvement result must leave the source
candidate unchanged; neither result is user approval or a visual-quality pass.
"""


def run_primary_form_repair_step(
    options: argparse.Namespace,
    environment: dict[str, str],
    workspace_root: str,
    turn_outputs: list[subprocess.CompletedProcess[str]],
    project_id: str,
    candidate_id: str,
    target_sha256: str,
    camera_ref: dict[str, Any],
    rig_sha256: str,
    view_spec: dict[str, Any],
    part_id: str | None,
    label: str,
) -> dict[str, Any]:
    """Run one Runtime-owned repair step and return its typed lineage."""
    rig = silhouette_rig_draft(candidate_id)
    rig["canonical_sha256"] = rig_sha256
    request: dict[str, Any] = {
        "project_id": project_id,
        "candidate_id": candidate_id,
        "target_sha256": target_sha256,
        "rig": rig,
        "base_camera": camera_ref,
        "view_spec": view_spec,
        "optimizer": {
            "algorithm": "coordinate_descent",
            "max_iterations": 2,
            "max_evaluations": 64,
            "step_fraction": 0.1,
        },
        "base_version_id": None,
        "canonical_sha256": "",
    }
    if part_id is not None:
        request["part_id"] = part_id
    request["canonical_sha256"] = canonical_hash(normalize_numeric_representation(request))
    repair_items = run_required_codex_turn(
        options,
        environment,
        primary_form_repair_prompt(request),
        workspace_root,
        PRIMARY_FORM_REPAIR_SEQUENCE,
        turn_outputs,
        label,
    )
    job_prepare = structured_result(repair_items, "primary_form_repair_job_prepare") or {}
    terminal_job = structured_result(repair_items, "job_get") or {}
    job_result = structured_result(repair_items, "job_result_get") or {}
    result = field(job_result, "result") or {}
    if not has_subsequence(call_sequence(repair_items), PRIMARY_FORM_REPAIR_SEQUENCE) or not all_completed(
        repair_items, PRIMARY_FORM_REPAIR_SEQUENCE
    ):
        raise RuntimeError("Codex did not complete the asynchronous Primary Form job/result sequence")
    job_id = field(job_prepare, "job_id")
    if not isinstance(job_id, str) or not job_id:
        raise RuntimeError("primary_form_repair_job_prepare did not return a job_id")
    if field(terminal_job, "job_id") != job_id or field(terminal_job, "status") != "succeeded":
        raise RuntimeError("Primary Form Runtime Job did not reach succeeded")
    if field(job_result, "job", "job_id") != job_id or not isinstance(field(job_result, "result_sha256"), str):
        raise RuntimeError("job_result_get did not return the hash-bound Primary Form result")
    if not isinstance(result, dict) or result.get("schema_version") != "PrimaryFormRepairPrepareResult@1":
        raise RuntimeError("job_result_get did not return PrimaryFormRepairPrepareResult@1")
    if field(result, "source_candidate_id") != candidate_id:
        raise RuntimeError("Primary Form Job source candidate drifted from the observed candidate")
    fit_result = field(result, "fit_result") or {}
    fit_camera = field(fit_result, "selected_camera")
    if not isinstance(fit_camera, dict):
        raise RuntimeError("Primary Form Job did not return Runtime-selected camera evidence")
    fit_camera_hash = field(fit_camera, "camera_hash")
    fit_camera_canonical = field(fit_camera, "canonical_sha256")
    if (
        not isinstance(fit_camera_hash, str)
        or len(fit_camera_hash) != 64
        or not isinstance(fit_camera_canonical, str)
        or len(fit_camera_canonical) != 64
    ):
        raise RuntimeError("Primary Form Job returned an invalid Runtime-selected camera")
    status = field(result, "status")
    if status not in {"prepared", "no_improvement"}:
        raise RuntimeError("Primary Form Job returned an unsupported status")
    prepared = field(result, "prepared_candidate") or {}
    prepared_candidate = field(prepared, "candidate") or {}
    prepared_job = field(prepared, "job") or {}
    prepared_artifact = field(prepared, "artifact") or {}
    staged = {
        "candidate_id": field(prepared_candidate, "candidate_id"),
        "job_id": field(prepared_job, "job_id"),
        "artifact_id": field(prepared_artifact, "artifact_id"),
        "artifact": prepared_artifact,
        "program_sha256": field(prepared_artifact, "program_sha256")
        or field(fit_result, "selected_geometry_program", "canonical_sha256"),
    }
    if status == "prepared" and not all(
        isinstance(value, str) and value
        for value in (staged["candidate_id"], staged["job_id"], staged["artifact_id"], staged["program_sha256"])
    ):
        raise RuntimeError("Primary Form Job did not return a complete staged candidate")
    return {
        "request": request,
        "items": repair_items,
        "result": result,
        "job": {
            "job_id": job_id,
            "status": field(terminal_job, "status"),
            "progress": field(terminal_job, "progress"),
            "result_sha256": field(job_result, "result_sha256"),
        },
        "fit_result": fit_result,
        "fit_camera": fit_camera,
        "fit_camera_hash": fit_camera_hash,
        "fit_camera_canonical": fit_camera_canonical,
        "status": status,
        "staged": staged,
    }


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


def geometry_hash_prompt(
    project_id: str,
    geometry_route: str,
    geometry_variant: str,
    material_variant: str,
    catalog_hash: str,
) -> str:
    draft_value, route_instructions = authoring_draft(
        geometry_route, project_id, geometry_variant, material_variant
    )
    draft_value = dict(draft_value)
    draft_value["operator_catalog_sha256"] = catalog_hash
    draft = json.dumps(draft_value, ensure_ascii=False, separators=(",", ":"))
    return f"""Use only the ForgeCAD MCP server. Do not use shell, filesystem, browser, images, other MCP servers or arbitrary code.

Call exactly one ForgeCAD tool, then stop:
geometry_program_hash with {{"schema_version":"GeometryProgramHashRequest@1","geometry_program_draft":{draft}}}.

{route_instructions}
Do not call geometry_prepare, appearance, compare, confirm, export or any other ForgeCAD tool. Return only the Runtime canonical_sha256 for this exact hash-free GeometryProgram@2 draft. Do not claim visual quality.
"""


def geometry_prepare_prompt(
    project_id: str,
    reference_id: str,
    geometry_route: str,
    geometry_variant: str,
    material_variant: str,
    catalog_hash: str,
    program_hash: str,
) -> str:
    draft_value, route_instructions = authoring_draft(
        geometry_route, project_id, geometry_variant, material_variant
    )
    draft_value = dict(draft_value)
    draft_value["operator_catalog_sha256"] = catalog_hash
    draft_value["canonical_sha256"] = program_hash
    draft = json.dumps(draft_value, ensure_ascii=False, separators=(",", ":"))
    return f"""Use only the ForgeCAD MCP server. Do not use shell, filesystem, browser, images, other MCP servers or arbitrary code.

Call exactly one ForgeCAD tool, then stop:
geometry_prepare with {{"project_id":{json.dumps(project_id)},"request":{{"typed":"geometry","reference_id":{json.dumps(reference_id)},"geometry_program":{draft}}}}}.

The Runtime returned this exact GeometryProgram canonical_sha256 from the previous hash turn: {json.dumps(program_hash)}. Copy it byte-for-byte; do not recompute or alter the draft. {route_instructions}
Do not call geometry_program_hash again, appearance, compare, confirm, export or any other ForgeCAD tool. Return the complete candidate, job and artifact objects; do not claim visual quality.
"""


def run_split_geometry_authoring(
    options: argparse.Namespace,
    environment: dict[str, str],
    workspace_root: str,
    turn_outputs: list[subprocess.CompletedProcess[str]],
    project_id: str,
    reference_id: str,
    geometry_route: str,
    geometry_variant: str,
    material_variant: str,
    catalog_hash: str,
) -> list[dict[str, Any]]:
    """Hash and prepare GeometryProgram in separate real-client turns."""
    hash_items = run_required_codex_turn(
        options,
        environment,
        geometry_hash_prompt(
            project_id,
            geometry_route,
            geometry_variant,
            material_variant,
            catalog_hash,
        ),
        workspace_root,
        GEOMETRY_HASH_SEQUENCE,
        turn_outputs,
        "geometry program hash",
    )
    hashed = structured_result(hash_items, "geometry_program_hash") or {}
    program_hash = field(hashed, "canonical_sha256")
    if not isinstance(program_hash, str) or len(program_hash) != 64:
        raise RuntimeError("geometry_program_hash did not return a canonical_sha256")
    prepare_items = run_required_codex_turn(
        options,
        environment,
        geometry_prepare_prompt(
            project_id,
            reference_id,
            geometry_route,
            geometry_variant,
            material_variant,
            catalog_hash,
            program_hash,
        ),
        workspace_root,
        GEOMETRY_PREPARE_SEQUENCE,
        turn_outputs,
        "geometry prepare",
    )
    return hash_items + prepare_items


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
    source_input = Path(options.reference).expanduser()
    if not source_input.is_file() or source_input.is_symlink():
        receipt = base_receipt("", 0) | {
            "status": "BLOCKED",
            "reason": "perspective reference is not a regular file",
            "reference_view_count": 1 + len(options.reference_views),
            "reference_views": [],
        }
        write_receipt(options.evidence, receipt)
        print(json.dumps(receipt, sort_keys=True))
        return 3
    source = source_input.resolve()
    source_bytes = source.read_bytes()
    source_sha = hashlib.sha256(source_bytes).hexdigest()
    try:
        width, height = reference_dimensions(source)
    except ValueError as error:
        receipt = base_receipt(source_sha, len(source_bytes)) | {"status": "BLOCKED", "reason": str(error)}
        write_receipt(options.evidence, receipt)
        print(json.dumps(receipt, sort_keys=True))
        return 3
    local_reference_records: list[dict[str, Any]] = [{
        "kind": "perspective",
        "view_id": stable_view_id("perspective"),
        "path": source,
        "source_sha256": source_sha,
        "reference_sha256": source_sha,
        "width": width,
        "height": height,
        "intake": {"landmarks": [], "regions": []},
        "intake_source_sha256": None,
    }]
    try:
        for kind, path in options.reference_views:
            if not path.is_file() or path.is_symlink():
                raise ValueError(f"{kind} reference is not a regular file")
            path = path.resolve()
            raw = path.read_bytes()
            view_width, view_height = reference_dimensions(path)
            view_sha = hashlib.sha256(raw).hexdigest()
            local_reference_records.append({
                "kind": kind,
                "view_id": stable_view_id(kind),
                "path": path,
                "source_sha256": view_sha,
                "reference_sha256": view_sha,
                "width": view_width,
                "height": view_height,
                "intake": {"landmarks": [], "regions": []},
                "intake_source_sha256": None,
            })
    except (OSError, ValueError) as error:
        receipt = base_receipt(source_sha, len(source_bytes)) | {
            "status": "BLOCKED",
            "reason": str(error)[:240],
            "reference_view_count": 1 + len(options.reference_views),
            "reference_views": reference_view_receipt_summary(local_reference_records),
        }
        write_receipt(options.evidence, receipt)
        print(json.dumps(receipt, sort_keys=True))
        return 3
    try:
        visual_intake, visual_intake_sha = load_visual_intake(options.intake, source_sha)
        local_reference_records[0]["intake"] = visual_intake
        local_reference_records[0]["intake_source_sha256"] = visual_intake_sha
        intake_by_kind = dict(options.intake_views)
        for record in local_reference_records[1:]:
            view_intake, view_intake_sha = load_visual_intake(
                intake_by_kind.get(record["kind"]),
                record["source_sha256"],
            )
            record["intake"] = view_intake
            record["intake_source_sha256"] = view_intake_sha
    except (OSError, ValueError, json.JSONDecodeError) as error:
        receipt = base_receipt(source_sha, len(source_bytes)) | {
            "status": "BLOCKED",
            "reason": f"visual intake unavailable: {str(error)[:240]}",
            "reference_view_count": len(local_reference_records),
            "reference_views": reference_view_receipt_summary(local_reference_records),
        }
        write_receipt(options.evidence, receipt)
        print(json.dumps(receipt, sort_keys=True))
        return 3
    reference_views_receipt = reference_view_receipt_summary(local_reference_records)
    if not options.execute:
        receipt = base_receipt(source_sha, len(source_bytes)) | {
            "status": "NOT_RUN",
            "reason": "Pass --execute to run the isolated local Runtime and Codex CLI.",
            "reference_view_count": len(local_reference_records),
            "reference_views": reference_views_receipt,
        }
        write_receipt(options.evidence, receipt)
        print(json.dumps(receipt, sort_keys=True))
        return 2

    runtime_command = str(Path(options.runtime_command).expanduser().resolve())
    mcp_command = str(Path(options.mcp_command).expanduser().resolve())
    # Codex starts each turn from a private temporary ``-C`` directory.  The
    # Runtime supervisor may use the resolved local paths above, but the MCP
    # server command is also copied into Codex's fresh configuration; leave no
    # relative path for that second consumer to resolve against the temp cwd.
    options.runtime_command = runtime_command
    options.mcp_command = mcp_command
    worker_command = str(Path(runtime_command).with_name("forgecad-geometry-worker"))
    render_worker_command = str(Path(runtime_command).with_name("forgecad-render-worker"))
    viewer_command = options.viewer_executable.expanduser().resolve() if options.viewer_executable else None
    if (
        options.timeout <= 0
        or not Path(runtime_command).is_file()
        or not Path(mcp_command).is_file()
        or not Path(worker_command).is_file()
        or not Path(render_worker_command).is_file()
        or viewer_command is not None
        and (not viewer_command.is_file() or not os.access(viewer_command, os.X_OK))
    ):
        receipt = base_receipt(source_sha, len(source_bytes)) | {
            "status": "BLOCKED",
            "reason": "same-cohort source MCP, Runtime, Geometry Worker and Render Worker binaries were unavailable",
            "reference_view_count": len(local_reference_records),
            "reference_views": reference_views_receipt,
        }
        write_receipt(options.evidence, receipt)
        print(json.dumps(receipt, sort_keys=True))
        return 3
    try:
        cohorts = {
            "mcp": build_cohort(mcp_command, "forgecad-mcp"),
            "runtime": build_cohort(runtime_command, "forgecad-runtime"),
            "worker": build_cohort(worker_command, "forgecad-geometry-worker"),
            "render_worker": build_cohort(render_worker_command, "forgecad-render-worker"),
        }
    except (OSError, subprocess.SubprocessError, ValueError, json.JSONDecodeError) as error:
        receipt = base_receipt(source_sha, len(source_bytes)) | {
            "status": "BLOCKED",
            "reason": f"build identity unavailable: {str(error)[:240]}",
            "reference_view_count": len(local_reference_records),
            "reference_views": reference_views_receipt,
        }
        write_receipt(options.evidence, receipt)
        print(json.dumps(receipt, sort_keys=True))
        return 3
    if len(set(cohorts.values())) != 1:
        receipt = base_receipt(source_sha, len(source_bytes)) | {
            "status": "BLOCKED",
            "reason": "MCP, Runtime, Geometry Worker and Render Worker build cohorts did not match",
            "build_cohorts": cohorts,
            "reference_view_count": len(local_reference_records),
            "reference_views": reference_views_receipt,
        }
        write_receipt(options.evidence, receipt)
        print(json.dumps(receipt, sort_keys=True))
        return 3

    environment = os.environ.copy()
    for key in ("CODEX_MCP_PROTOCOL_VERSION", "FORGECAD_RUNTIME_SOCKET", "FORGECAD_RUNTIME_TOKEN", "FORGECAD_RUNTIME_DATA_DIR", "FORGECAD_RUNTIME_COMMAND", "FORGECAD_RUNTIME_READY_FILE", "FORGECAD_RUNTIME_STATUS_FILE"):
        environment.pop(key, None)
    environment["FORGECAD_MCP_ENABLE_MCP004_WRITES"] = "1"
    attachment_roots: list[str] = []
    for record in local_reference_records:
        root_path = str(record["path"].parent.resolve())
        if root_path not in attachment_roots:
            attachment_roots.append(root_path)
    environment["FORGECAD_ATTACHMENT_ROOTS"] = os.pathsep.join(attachment_roots)

    receipt = base_receipt(source_sha, len(source_bytes)) | {
        "status": "BLOCKED",
        "build_cohorts": cohorts,
        "reference_view_count": len(local_reference_records),
        "reference_views": reference_views_receipt,
    }
    runtime: subprocess.Popen[str] | None = None
    turn_outputs: list[subprocess.CompletedProcess[str]] = []
    silhouette_fit_intent_sha: str | None = None
    primary_form_repair_intent_sha: str | None = None
    partial_evidence: dict[str, Any] = {}
    primary_form_composition_lineage: dict[str, Any] | None = None
    primary_form_composition_initial_candidate_id: str | None = None
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

            setup_images = [str(record["path"]) for record in local_reference_records]
            if len(local_reference_records) == 1:
                setup_text = setup_prompt(str(source))
            else:
                setup_text = setup_prompt_multi([
                    {
                        "kind": record["kind"],
                        "path": str(record["path"]),
                        "source_sha256": record["source_sha256"],
                    }
                    for record in local_reference_records
                ])
            first = run_codex_turn(options, environment, setup_text, str(root), setup_images)
            turn_outputs.append(first)
            first_items = event_items(first.stdout)
            project_result = structured_result(first_items, "project_create") or {}
            project_id = field(project_result, "project_id")
            import_results = structured_results(first_items, "reference_import")
            get_results = structured_results(first_items, "reference_get")
            setup_calls = mcp_calls(first_items)
            if not isinstance(project_id, str) or len(import_results) != len(local_reference_records):
                raise RuntimeError("Codex setup did not return one import result per reference view")
            imported_references: list[dict[str, Any]] = []
            for record, import_result in zip(local_reference_records, import_results):
                imported = field(import_result, "reference") or import_result
                imported_id = field(imported, "reference_id")
                imported_sha = field(imported, "object_sha256")
                if (
                    not isinstance(imported_id, str)
                    or not isinstance(imported_sha, str)
                    or imported_sha != record["source_sha256"]
                ):
                    raise RuntimeError(f"reference_import did not bind the {record['kind']} source bytes")
                record["reference_id"] = imported_id
                record["reference_sha256"] = imported_sha
                imported_references.append({
                    "reference_id": imported_id,
                    "reference_sha256": imported_sha,
                })
            if len(get_results) != len(local_reference_records):
                # A real Codex setup turn can stop after the import batch and
                # return only a prefix of the readback batch.  Imports are
                # already Runtime-bound at this point, so recover only the
                # missing reference IDs one at a time instead of replaying
                # reference_import and creating duplicate evidence objects.
                readbacks_by_id: dict[str, dict[str, Any]] = {}
                for get_result in get_results:
                    readback = field(get_result, "reference") or get_result
                    readback_id = field(readback, "reference_id")
                    if isinstance(readback_id, str) and readback_id not in readbacks_by_id:
                        readbacks_by_id[readback_id] = get_result
                for imported in imported_references:
                    reference_id_for_readback = imported["reference_id"]
                    if reference_id_for_readback in readbacks_by_id:
                        continue
                    setup_readback = run_codex_turn(
                        options,
                        environment,
                        reference_get_prompt(reference_id_for_readback),
                        str(root),
                    )
                    turn_outputs.append(setup_readback)
                    setup_readback_items = event_items(setup_readback.stdout)
                    setup_calls.extend(mcp_calls(setup_readback_items))
                    recovered = structured_results(setup_readback_items, "reference_get")
                    if len(recovered) != 1:
                        raise RuntimeError(
                            f"Codex setup recovery did not return one readback for reference {reference_id_for_readback}"
                        )
                    recovered_readback = field(recovered[0], "reference") or recovered[0]
                    recovered_id = field(recovered_readback, "reference_id")
                    if recovered_id != reference_id_for_readback:
                        raise RuntimeError(
                            f"Codex setup recovery returned the wrong reference_id for {reference_id_for_readback}"
                        )
                    readbacks_by_id[reference_id_for_readback] = recovered[0]
                get_results = [readbacks_by_id[imported["reference_id"]] for imported in imported_references]
            if len(get_results) != len(local_reference_records):
                raise RuntimeError("Codex setup did not return reference metadata readback")
            runtime_reference_records: list[dict[str, Any]] = []
            for record, imported, get_result in zip(local_reference_records, imported_references, get_results):
                readback = field(get_result, "reference") or get_result
                if (
                    field(readback, "reference_id") != imported["reference_id"]
                    or field(readback, "object_sha256") != imported["reference_sha256"]
                ):
                    raise RuntimeError(f"reference_get did not match the {record['kind']} reference_import")
                runtime_reference_records.append({
                    "kind": record["kind"],
                    "view_id": record["view_id"],
                    "reference_id": imported["reference_id"],
                    "reference_sha256": imported["reference_sha256"],
                    "width": record["width"],
                    "height": record["height"],
                    "intake": record["intake"],
                })
            primary_reference = runtime_reference_records[0]
            reference_id = primary_reference["reference_id"]
            reference_sha = primary_reference["reference_sha256"]
            width = primary_reference["width"]
            height = primary_reference["height"]
            reference_views_receipt = reference_view_receipt_summary(local_reference_records)
            if not isinstance(reference_id, str) or not isinstance(reference_sha, str):
                raise RuntimeError("Codex setup did not return the primary reference evidence")
            setup_expected = (
                "skill_get",
                "project_create",
                *(["reference_import"] * len(local_reference_records)),
                *(["reference_get"] * len(local_reference_records)),
            )
            completed_setup_tool_names = [
                str(call.get("tool"))
                for call in setup_calls
                if call.get("server") == "forgecad" and call.get("status") == "completed"
            ]
            if not (
                has_subsequence(completed_setup_tool_names, tuple(setup_expected))
                and all(name in completed_setup_tool_names for name in setup_expected)
            ):
                raise RuntimeError("Codex setup did not complete the required MCP sequence")

            spec = view_spec(
                reference_id,
                reference_sha,
                width,
                height,
                visual_intake,
                kind="perspective",
                view_id=primary_reference["view_id"],
            )
            authoring_context = (
                reference_canvas_authoring_context_multi(project_id, runtime_reference_records)
                if len(runtime_reference_records) > 1
                else reference_canvas_authoring_context(
                    project_id,
                    reference_id,
                    reference_sha,
                    width,
                    height,
                    visual_intake,
                )
            )
            receipt.update({
                "reference_view_count": len(local_reference_records),
                "reference_views": reference_views_receipt,
                "reference_set_sha256": field(authoring_context, "reference_canvas", "reference_set_sha256"),
                "reference_coverage": {
                    "required_views": field(authoring_context, "reference_canvas", "coverage", "required_views"),
                    "supplied_views": field(authoring_context, "reference_canvas", "coverage", "supplied_views"),
                    "missing_views": field(authoring_context, "reference_canvas", "coverage", "missing_views"),
                    "coverage_status": field(authoring_context, "reference_canvas", "coverage", "coverage_status"),
                    "hq_360_status": field(authoring_context, "reference_canvas", "coverage", "hq_360_status"),
                },
            })
            silhouette_target_sha: str | None = None
            silhouette_camera_hash: str | None = None
            silhouette_fit_camera_hash: str | None = None
            silhouette_fit_camera_canonical: str | None = None
            silhouette_observation_sha: str | None = None
            comparison_camera_hash: str | None = None
            comparison_camera_canonical: str | None = None
            silhouette_rig_sha: str | None = None
            silhouette_fit_result: dict[str, Any] | None = None
            primary_form_repair_result: dict[str, Any] | None = None
            primary_form_repair_source_candidate_id: str | None = None
            primary_form_repair_items: list[dict[str, Any]] = []
            primary_form_repair_steps: list[dict[str, Any]] = []
            canonical_observation_result: dict[str, Any] | None = None
            canonical_observation_candidate_id: str | None = None
            silhouette_comparison_result: dict[str, Any] | None = None
            authoring_session_result: dict[str, Any] | None = None
            selected_camera_for_compare: dict[str, Any] | None = None
            primary_form_runtime_compare = False
            canonical_compare_in_silhouette = False
            silhouette_items: list[dict[str, Any]] = []
            fit_items: list[dict[str, Any]] = []
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
            hash_prepare_items = run_split_geometry_authoring(
                options,
                environment,
                str(root),
                turn_outputs,
                project_id,
                reference_id,
                options.geometry_route,
                options.geometry_variant,
                options.material_variant,
                catalog_hash,
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
            if catalog_hash != capability_hash or not has_subsequence(call_sequence(discovery_items), discovery_sequence) or not all_completed(discovery_items, discovery_sequence) or not has_subsequence(call_sequence(hash_prepare_items), ("geometry_program_hash", "geometry_prepare")) or not all_completed(hash_prepare_items, ("geometry_program_hash", "geometry_prepare")):
                raise RuntimeError("Codex authoring did not complete matching discovery/hash/prepare sequence")

            if options.silhouette_first:
                silhouette_turn_items = run_split_silhouette_observation(
                    options,
                    environment,
                    str(root),
                    turn_outputs,
                    project_id,
                    reference_id,
                    candidate_id,
                    job_id,
                    artifact_id,
                    silhouette_target_sha or "",
                    spec,
                    authoring_context,
                    allow_runtime_default_authoring=len(runtime_reference_records) == 1,
                )
                silhouette_items.extend(silhouette_turn_items)
                silhouette_turn = parse_bound_silhouette_turn(
                    silhouette_turn_items,
                    project_id,
                    candidate_id,
                )
                silhouette_comparison_result = silhouette_turn["comparison"]
                authoring_session_result = silhouette_turn["session_result"]
                canonical_observation_result = silhouette_turn["observation"]
                silhouette_observation_sha = silhouette_turn["observation_sha"]
                canonical_observation_candidate_id = candidate_id
                selected_camera = silhouette_turn["camera"]
                silhouette_camera_hash = silhouette_turn["camera_hash"]
                silhouette_rig_sha = silhouette_turn["rig_sha"]
                rig = silhouette_rig_draft(candidate_id)
                rig["canonical_sha256"] = silhouette_rig_sha
                camera_ref = silhouette_turn["camera_ref"]
                selected_camera_for_compare = selected_camera
                if options.observation_only:
                    # The baseline compare is already part of the same
                    # candidate-bound silhouette turn.  Observation-only is a
                    # transport gate: it must not issue a fragmented compare
                    # or a continuous parameter search after the canonical
                    # scene observation has been verified.
                    comparison = silhouette_comparison_result or {}
                    canonical_compare_in_silhouette = True
                elif options.primary_form_repair:
                    sequence_parts = options.part_contour_sequence_parts or (
                        (options.part_contour_part,) if options.part_contour_trial else (None,)
                    )
                    if options.part_contour_sequence_parts:
                        primary_form_composition_initial_candidate_id = candidate_id
                    for step_index, part_id in enumerate(sequence_parts):
                        if step_index:
                            # Never carry the previous candidate's observation
                            # into the next repair.  Re-read the consolidated
                            # silhouette observation before composition step:
                            # target/camera,
                            # baseline compare, canonical observation and Rig
                            # after every accepted staged candidate.
                            next_silhouette_items = run_split_silhouette_observation(
                                options,
                                environment,
                                str(root),
                                turn_outputs,
                                project_id,
                                reference_id,
                                candidate_id,
                                job_id,
                                artifact_id,
                                silhouette_target_sha or "",
                                spec,
                                authoring_context,
                                allow_runtime_default_authoring=len(runtime_reference_records) == 1,
                            )
                            silhouette_items.extend(next_silhouette_items)
                            silhouette_turn = parse_bound_silhouette_turn(
                                next_silhouette_items,
                                project_id,
                                candidate_id,
                            )
                            silhouette_comparison_result = silhouette_turn["comparison"]
                            authoring_session_result = silhouette_turn["session_result"]
                            canonical_observation_result = silhouette_turn["observation"]
                            silhouette_observation_sha = silhouette_turn["observation_sha"]
                            canonical_observation_candidate_id = candidate_id
                            selected_camera = silhouette_turn["camera"]
                            silhouette_camera_hash = silhouette_turn["camera_hash"]
                            silhouette_rig_sha = silhouette_turn["rig_sha"]
                            camera_ref = silhouette_turn["camera_ref"]
                            selected_camera_for_compare = selected_camera
                        step = run_primary_form_repair_step(
                            options,
                            environment,
                            str(root),
                            turn_outputs,
                            project_id,
                            candidate_id,
                            silhouette_target_sha or "",
                            camera_ref,
                            silhouette_rig_sha,
                            spec,
                            part_id,
                            f"Primary Form composition step {step_index + 1}",
                        )
                        primary_form_repair_items.extend(step["items"])
                        primary_form_repair_steps.append({
                            "step": step_index + 1,
                            "part_id": part_id,
                            "source_candidate_id": candidate_id,
                            "observation_candidate_id": canonical_observation_candidate_id,
                            "target_sha256": silhouette_target_sha,
                            "observation_sha256": silhouette_observation_sha,
                            "camera_hash": camera_ref["camera_hash"],
                            "camera_canonical_sha256": camera_ref["canonical_sha256"],
                            "rig_sha256": silhouette_rig_sha,
                            "intent_sha256": step["request"]["canonical_sha256"],
                            "status": step["status"],
                            "fit_evaluations": field(step["fit_result"], "evaluations"),
                            "fit_loss": field(step["fit_result"], "selected_loss"),
                            "fit_camera_hash": step["fit_camera_hash"],
                            "acceptance": {
                                "status": field(step["result"], "acceptance", "status"),
                                "strict_improvement": field(step["result"], "acceptance", "strict_improvement"),
                                "source_loss": field(step["result"], "acceptance", "source_loss"),
                                "proposal_loss": field(step["result"], "acceptance", "proposal_loss"),
                                "camera_hash": field(step["result"], "acceptance", "camera_hash"),
                            },
                            # Preserve the Runtime-owned cross-view evidence
                            # projection without image bytes or local paths.
                            # Its absence is meaningful for incomplete canvases;
                            # never synthesize it in the probe.
                            "multi_view_evaluation": field(step["result"], "multi_view_evaluation"),
                            "prepared_candidate_id": step["staged"]["candidate_id"] if step["status"] == "prepared" else None,
                        })
                        partial_evidence["primary_form_repair_steps"] = primary_form_repair_steps
                        primary_form_repair_result = step["result"]
                        primary_form_repair_source_candidate_id = candidate_id
                        primary_form_repair_intent_sha = step["request"]["canonical_sha256"]
                        silhouette_fit_result = step["fit_result"]
                        silhouette_fit_camera_hash = step["fit_camera_hash"]
                        silhouette_fit_camera_canonical = step["fit_camera_canonical"]
                        next_candidate_id = candidate_id
                        if options.part_contour_sequence_parts:
                            if step["status"] == "prepared":
                                staged_payload = step.get("staged")
                                staged_candidate_id = (
                                    staged_payload.get("candidate_id")
                                    if isinstance(staged_payload, dict)
                                    else None
                                )
                                if not isinstance(staged_candidate_id, str) or not staged_candidate_id:
                                    raise RuntimeError("PRIMARY_FORM_COMPOSITION_INVALID: staged candidate is missing")
                                next_candidate_id = staged_candidate_id
                            elif step["status"] != "no_improvement":
                                raise RuntimeError(
                                    "PRIMARY_FORM_COMPOSITION_INVALID: unsupported step status before candidate advance"
                                )
                            # Once a two-step prefix exists, the compact
                            # lineage is the authoritative chain projection
                            # for the next step.  Do not advance from the raw
                            # event list if its candidate/observation/hash
                            # bindings fail closed.
                            if step_index >= 1:
                                if primary_form_composition_initial_candidate_id is None:
                                    raise RuntimeError("PRIMARY_FORM_COMPOSITION_INVALID: initial candidate is missing")
                                primary_form_composition_lineage = build_primary_form_composition_lineage(
                                    project_id,
                                    primary_form_composition_initial_candidate_id,
                                    next_candidate_id,
                                    silhouette_target_sha or "",
                                    options.part_contour_sequence_parts[: step_index + 1],
                                    primary_form_repair_steps,
                                )
                                if primary_form_composition_lineage["final_candidate_id"] != next_candidate_id:
                                    raise RuntimeError(
                                        "PRIMARY_FORM_COMPOSITION_INVALID: lineage did not authorize candidate advance"
                                    )
                                partial_evidence["primary_form_composition_lineage"] = primary_form_composition_lineage
                        if step["status"] == "prepared":
                            selected_camera_for_compare = step["fit_camera"]
                            staged = step["staged"]
                            candidate_id = staged["candidate_id"]
                            job_id = staged["job_id"]
                            artifact_id = staged["artifact_id"]
                            artifact = staged["artifact"]
                            program_hash = staged["program_sha256"]
                        elif options.part_contour_sequence_parts:
                            candidate_id = next_candidate_id
                    if options.part_contour_sequence_parts:
                        if primary_form_composition_lineage is None:
                            raise RuntimeError("PRIMARY_FORM_COMPOSITION_INVALID: lineage was not consumed")
                        if primary_form_composition_lineage["final_candidate_id"] != candidate_id:
                            raise RuntimeError("PRIMARY_FORM_COMPOSITION_INVALID: final candidate is not lineage-bound")
                else:
                    fit_request: dict[str, Any] = {
                        "project_id": project_id,
                        "candidate_id": candidate_id,
                        "target_sha256": silhouette_target_sha,
                        "rig": rig,
                        "base_camera": camera_ref,
                        "optimizer": {"algorithm": "coordinate_descent", "max_iterations": 2, "max_evaluations": 64, "step_fraction": 0.1},
                        "canonical_sha256": "",
                    }
                    if options.part_contour_trial:
                        fit_request["part_id"] = options.part_contour_part
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
                    # The fit result is the authoritative bounded camera
                    # proposal for the subsequent comparison.  Keeping the
                    # initial camera fit hash separately makes any accidental
                    # handoff drift visible in the receipt instead of silently
                    # comparing a different camera than the one the fit optimized.
                    selected_camera_for_compare = fit_selected_camera

            third_items: list[dict[str, Any]] = []
            actual_third: list[str] = []
            runtime_visual_evidence = field(primary_form_repair_result or {}, "visual_evidence")
            if (
                options.primary_form_repair
                and isinstance(runtime_visual_evidence, dict)
                and field(runtime_visual_evidence, "candidate_id") == candidate_id
            ):
                # primary_form_repair_prepare already performed the complete
                # Runtime-owned fit -> Geometry Worker -> Render Worker ->
                # candidate-bound comparison. Re-entering reference_compare_prepare
                # from Codex would duplicate that compare and would force the
                # compact camera reference through a new staged-candidate cache
                # key. Consume the typed visual evidence returned by the one
                # Runtime action instead.
                comparison = {
                    "render_set_object_sha256": field(runtime_visual_evidence, "render_set_hash"),
                    "comparison_report_object_sha256": field(runtime_visual_evidence, "comparison_report_hash"),
                    "render_set": field(runtime_visual_evidence, "render_set") or {},
                    "comparison_report": field(runtime_visual_evidence, "comparison_report") or {},
                    "camera": {
                        "camera_hash": field(runtime_visual_evidence, "camera_hash"),
                        "canonical_sha256": field(
                            selected_camera_for_compare or {}, "canonical_sha256"
                        ),
                    },
                }
                primary_form_runtime_compare = True
            elif (
                options.primary_form_repair
                and isinstance(silhouette_comparison_result, dict)
                and field(silhouette_comparison_result, "candidate_id") == candidate_id
            ):
                # Primary Form retained the source candidate. Reuse the
                # baseline compare that was part of the canonical silhouette
                # turn; do not issue another fragmented compare or boundary
                # diagnostic call.
                comparison = silhouette_comparison_result
                canonical_compare_in_silhouette = True
            else:
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
                actual_third = call_sequence(third_items)
            render_set_hash = field(comparison, "render_set_object_sha256") or field(comparison, "render_set_hash")
            comparison_hash = field(comparison, "comparison_report_object_sha256") or field(comparison, "comparison_report_hash")
            render_set = field(comparison, "render_set") or {}
            metrics = field(comparison, "comparison_report", "metrics")
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
                "authoring_context_binding": authoring_binding_summary(authoring_session_result),
                "authoring_mode": authoring_mode_summary(silhouette_items),
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
            if (
                not primary_form_runtime_compare
                and not canonical_compare_in_silhouette
                and (not has_subsequence(actual_third, COMPARE_SEQUENCE) or not all_completed(third_items, COMPARE_SEQUENCE))
            ):
                raise RuntimeError("Codex did not complete the readback/compare sequence")

            boundary_result: dict[str, Any] | None = None
            canonical_part_error: dict[str, Any] | None = None
            boundary_summary = None
            if options.silhouette_first:
                observed_part_error = field(
                    canonical_observation_result or {},
                    "design_critic_report",
                    "primary_form_directive",
                    "part_error",
                )
                if (
                    isinstance(observed_part_error, dict)
                    and observed_part_error.get("schema_version") == "SilhouettePartErrorResult@1"
                    and isinstance(observed_part_error.get("parts"), list)
                ):
                    # The complete Part error table is already part of the
                    # canonical scene observation.  Do not make Codex issue a
                    # second boundary read that can drift from the scene,
                    # camera or candidate it just observed.
                    canonical_part_error = observed_part_error
                    boundary_summary = {
                        "source": "canonical_observation",
                        "metrics": field(observed_part_error, "metrics"),
                        "parts": observed_part_error.get("parts", []),
                        "recommended_part_ids": observed_part_error.get("recommended_part_ids", []),
                    }
                else:
                    # Compatibility fallback for older Runtime cohorts whose
                    # automatic silhouette target cannot yet project a Part
                    # table.  New cohorts must take the canonical branch.
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
                    boundary_summary = {
                        "source": "boundary_error_get_compatibility_fallback",
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
            part_contour_result: dict[str, Any] | None = None
            part_contour_rig_sha256: str | None = None
            part_contour_items: list[dict[str, Any]] = []
            if options.part_contour_part:
                part_contour_turn = run_codex_turn(
                    options,
                    environment,
                    part_contour_prompt(
                        project_id,
                        candidate_id,
                        silhouette_target_sha or "",
                        options.part_contour_part,
                    ),
                    str(root),
                )
                turn_outputs.append(part_contour_turn)
                part_contour_items = event_items(part_contour_turn.stdout)
                part_contour_rig_result = structured_result(part_contour_items, "silhouette_rig_hash") or {}
                part_contour_result = structured_result(part_contour_items, "part_contour_fit_prepare") or {}
                part_contour_rig_sha256 = field(part_contour_rig_result, "canonical_sha256")
                if not has_subsequence(
                    call_sequence(part_contour_items),
                    ("silhouette_rig_hash", "part_contour_fit_prepare"),
                ) or not all_completed(
                    part_contour_items,
                    ("silhouette_rig_hash", "part_contour_fit_prepare"),
                ):
                    raise RuntimeError("Codex did not complete the single-Part contour sequence")
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
                    "reference_view_count": len(local_reference_records),
                    "reference_views": reference_views_receipt,
                    "reference_set_sha256": field(authoring_context, "reference_canvas", "reference_set_sha256"),
                    "reference_coverage": {
                        "required_views": field(authoring_context, "reference_canvas", "coverage", "required_views"),
                        "supplied_views": field(authoring_context, "reference_canvas", "coverage", "supplied_views"),
                        "missing_views": field(authoring_context, "reference_canvas", "coverage", "missing_views"),
                        "coverage_status": field(authoring_context, "reference_canvas", "coverage", "coverage_status"),
                        "hq_360_status": field(authoring_context, "reference_canvas", "coverage", "hq_360_status"),
                    },
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
                    "silhouette_rig_sha256": silhouette_rig_sha,
                    "comparison_camera_hash": comparison_camera_hash,
                    "comparison_camera_canonical_sha256": comparison_camera_canonical,
                    "render_set_hash": field(comparison, "render_set_object_sha256") or field(comparison, "render_set_hash"),
                    "comparison_report_hash": field(comparison, "comparison_report_object_sha256") or field(comparison, "comparison_report_hash"),
                    "comparison_metrics": metrics,
                    "authoring_context_binding": authoring_binding_summary(authoring_session_result),
                    "authoring_mode": authoring_mode_summary(silhouette_items),
                    "boundary_error": boundary_summary,
                    "canonical_part_error": canonical_part_error,
                    "boundary_error_count": (
                        len(field(boundary_result or {}, "segments") or [])
                        if boundary_result is not None
                        else len(field(canonical_part_error or {}, "parts") or [])
                    ),
                    "part_contour_part_id": options.part_contour_part,
                    "part_contour_trial_requested": options.part_contour_trial,
                    "part_contour_sequence_requested": bool(options.part_contour_sequence_parts),
                    "part_contour_sequence_parts": list(options.part_contour_sequence_parts),
                    "primary_form_repair_steps": primary_form_repair_steps,
                    "primary_form_composition_lineage": primary_form_composition_lineage,
                    "part_contour_trial": {
                        "part_id": field(primary_form_repair_result or {}, "part_id"),
                        "status": field(primary_form_repair_result or {}, "status"),
                        "quality_status": field(primary_form_repair_result or {}, "quality_status"),
                        "candidate_state": field(primary_form_repair_result or {}, "candidate_state"),
                        "source_candidate_id": field(primary_form_repair_result or {}, "source_candidate_id"),
                        "prepared_candidate_id": field(primary_form_repair_result or {}, "visual_evidence", "candidate_id"),
                        "acceptance_status": field(primary_form_repair_result or {}, "acceptance", "status"),
                        "acceptance_strict_improvement": field(primary_form_repair_result or {}, "acceptance", "strict_improvement"),
                        "acceptance_source_loss": field(primary_form_repair_result or {}, "acceptance", "source_loss"),
                        "acceptance_proposal_loss": field(primary_form_repair_result or {}, "acceptance", "proposal_loss"),
                        "acceptance_camera_hash": field(primary_form_repair_result or {}, "acceptance", "camera_hash"),
                    } if options.part_contour_trial else None,
                    "part_contour_rig_sha256": part_contour_rig_sha256,
                    "part_contour_fit": part_contour_result,
                    "quality_claim": "NO_LIKENESS_PASS_CLAIM; BOUNDARY_EVIDENCE_ONLY",
                    "geometry_route": options.geometry_route,
                    "geometry_variant": options.geometry_variant if options.geometry_route == "detail" else None,
                    "material_variant": options.material_variant if options.geometry_route == "detail" else None,
                    "silhouette_first": options.silhouette_first,
                    "silhouette_gate": "QUALITY_TARGET_NOT_MET" if field(silhouette_fit_result or {}, "status") != "ready" else "PASS",
                    "detail_material_stages": "LOCKED_UNTIL_SILHOUETTE_GATE",
                    "mcp_tool_calls": [call for turn in turn_outputs for call in mcp_calls(event_items(turn.stdout))],
                    "expected_sequences": {
                        "setup": list(setup_expected),
                        "authoring": list(AUTHORING_SEQUENCE),
                        "silhouette_target": list(SILHOUETTE_TARGET_SEQUENCE),
                        "silhouette": list(SILHOUETTE_SEQUENCE),
                        "compare": [] if silhouette_comparison_result else list(COMPARE_SEQUENCE),
                        "boundary": ["boundary_error_get"] if boundary_result is not None else [],
                        "part_contour": ["silhouette_rig_hash", "part_contour_fit_prepare"] if options.part_contour_part else [],
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
                "reference_view_count": len(local_reference_records),
                "reference_views": reference_views_receipt,
                "reference_set_sha256": field(authoring_context, "reference_canvas", "reference_set_sha256"),
                "reference_coverage": {
                    "required_views": field(authoring_context, "reference_canvas", "coverage", "required_views"),
                    "supplied_views": field(authoring_context, "reference_canvas", "coverage", "supplied_views"),
                    "missing_views": field(authoring_context, "reference_canvas", "coverage", "missing_views"),
                    "coverage_status": field(authoring_context, "reference_canvas", "coverage", "coverage_status"),
                    "hq_360_status": field(authoring_context, "reference_canvas", "coverage", "hq_360_status"),
                },
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
                "authoring_context_binding": authoring_binding_summary(authoring_session_result),
                "authoring_mode": authoring_mode_summary(silhouette_items),
                "primary_form_repair_requested": options.primary_form_repair,
                "part_contour_trial_requested": options.part_contour_trial,
                "part_contour_sequence_requested": bool(options.part_contour_sequence_parts),
                "part_contour_sequence_parts": list(options.part_contour_sequence_parts),
                "primary_form_repair_intent_sha256": primary_form_repair_intent_sha,
                "primary_form_repair_steps": primary_form_repair_steps,
                "primary_form_composition_lineage": primary_form_composition_lineage,
                "primary_form_repair": {
                    "part_id": field(primary_form_repair_result or {}, "part_id"),
                    "status": field(primary_form_repair_result or {}, "status"),
                    "quality_status": field(primary_form_repair_result or {}, "quality_status"),
                    "candidate_state": field(primary_form_repair_result or {}, "candidate_state"),
                    "source_candidate_id": primary_form_repair_source_candidate_id,
                    "prepared_candidate_id": field(
                        primary_form_repair_result or {},
                        "visual_evidence",
                        "candidate_id",
                    ),
                    "staged_render_set_hash": field(
                        primary_form_repair_result or {},
                        "visual_evidence",
                        "render_set_hash",
                    ),
                    "staged_comparison_report_hash": field(
                        primary_form_repair_result or {},
                        "visual_evidence",
                        "comparison_report_hash",
                    ),
                    "staged_quality_report_hash": field(
                        primary_form_repair_result or {},
                        "visual_evidence",
                        "quality_report_hash",
                    ),
                    "fit_status": field(primary_form_repair_result or {}, "fit_result", "status"),
                    "fit_evaluations": field(primary_form_repair_result or {}, "fit_result", "evaluations"),
                    "fit_metrics": field(primary_form_repair_result or {}, "fit_result", "metrics"),
                    "acceptance_status": field(primary_form_repair_result or {}, "acceptance", "status"),
                    "acceptance_strict_improvement": field(primary_form_repair_result or {}, "acceptance", "strict_improvement"),
                    "acceptance_source_loss": field(primary_form_repair_result or {}, "acceptance", "source_loss"),
                    "acceptance_proposal_loss": field(primary_form_repair_result or {}, "acceptance", "proposal_loss"),
                    "acceptance_camera_hash": field(primary_form_repair_result or {}, "acceptance", "camera_hash"),
                    "multi_view_evaluation": field(primary_form_repair_result or {}, "multi_view_evaluation"),
                } if options.primary_form_repair else None,
                "primary_form_multi_view_evaluation": field(
                    primary_form_repair_result or {}, "multi_view_evaluation"
                ),
                "primary_form_runtime_compare": primary_form_runtime_compare,
                "canonical_compare_in_silhouette": canonical_compare_in_silhouette,
                "aov_order": list(AOV_ORDER),
                "render_pass_calls": len(actual_render_passes),
                "render_pass_order": actual_render_passes,
                "render_pass_image_blocks": "NOT_OBSERVED_IN_SANITIZED_CLI_EVENTS",
                "comparison_metrics": metrics,
                # Preserve only the typed, candidate-bound boundary evidence
                # needed for the next single-Part decision.  The MCP result
                # contains normalized points, pixel deltas, direction and
                # semantic Part IDs; it contains no reference bytes or local
                # paths.  Keeping this summary in the receipt prevents the
                # next round from guessing a Part after the Runtime process
                # is torn down.
                "boundary_error": boundary_summary,
                "canonical_part_error": canonical_part_error,
                "part_contour_part_id": options.part_contour_part,
                "part_contour_rig_sha256": part_contour_rig_sha256,
                "part_contour_fit": part_contour_result,
                "visual_review_status": field(review_report, "status"),
                "quality_visual_status": field(quality_report, "visual_status"),
                "quality_hard_gate_passed": field(quality_report, "hard_gate_passed"),
                "review_recovered_after_tool_drift": review_recovered,
                "mcp_tool_calls": [call for turn in turn_outputs for call in mcp_calls(event_items(turn.stdout))],
                "expected_sequences": {
                    "setup": list(setup_expected),
                    "authoring": list(AUTHORING_SEQUENCE),
                    "silhouette_target": list(SILHOUETTE_TARGET_SEQUENCE) if options.silhouette_first else [],
                    "silhouette": list(SILHOUETTE_SEQUENCE) if options.silhouette_first else [],
                    "compare": []
                    if primary_form_runtime_compare or canonical_compare_in_silhouette
                    else list(COMPARE_SEQUENCE),
                    "boundary": ["boundary_error_get"] if boundary_result is not None else [],
                    "part_contour": ["silhouette_rig_hash", "part_contour_fit_prepare"] if options.part_contour_part else [],
                    "primary_form_repair": list(PRIMARY_FORM_REPAIR_SEQUENCE) if options.primary_form_repair else [],
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
                "boundary_error_count": (
                    len(field(boundary_result or {}, "segments") or [])
                    if boundary_result is not None
                    else len(field(canonical_part_error or {}, "parts") or [])
                    if options.silhouette_first
                    else None
                ),
                # Keep the legacy flat sequence for existing consumers, but
                # expose the canonical observation as its own typed stage.
                # This prevents later readers from reconstructing scene state
                # by interleaving target, camera and fit tool calls.
                "silhouette_stage_sequences": {
                    "target": [call.get("tool") for call in mcp_calls(target_items)] if options.silhouette_first else [],
                    "observation": [call.get("tool") for call in mcp_calls(silhouette_turn_items)] if options.silhouette_first else [],
                    "fit": [call.get("tool") for call in mcp_calls(fit_items)] if options.silhouette_first else [],
                    "part_contour": [call.get("tool") for call in mcp_calls(part_contour_items)] if options.part_contour_part else [],
                    "repair": [call.get("tool") for call in mcp_calls(primary_form_repair_items)] if options.primary_form_repair else [],
                },
                "canonical_observation": {
                    "schema_version": "AgenticSceneObserveResult@1",
                    "project_id": project_id,
                    "candidate_id": canonical_observation_candidate_id,
                    "read_only": True,
                    "canonical_sha256": silhouette_observation_sha,
                } if options.silhouette_first else None,
                "silhouette_sequence": [call.get("tool") for call in mcp_calls(silhouette_items)] if options.silhouette_first else [],
                "silhouette_gate": "NOT_RUN" if not options.silhouette_first else ("PASS" if field(silhouette_fit_result or {}, "status") == "ready" else "QUALITY_TARGET_NOT_MET"),
                "detail_material_stages": "LOCKED_UNTIL_SILHOUETTE_GATE" if options.silhouette_first and field(silhouette_fit_result or {}, "status") != "ready" else "NOT_APPLICABLE",
            })
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
