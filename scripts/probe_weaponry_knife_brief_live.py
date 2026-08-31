#!/usr/bin/env python3
"""Run an authorized Knife Production Brief through the default MCP façade.

The probe uses an isolated Runtime database/CAS and the real source binaries.
It imports the supplied image bytes through Runtime, persists the conflicted
parent, persists one immutable resolved successor, replays it, restarts the
Runtime, and reads the exact successor back.  Its optional receipt is hash-only:
source paths, image bytes, contact details and signature content are excluded.
By default it does not create geometry.  With ``--source-binding`` it also
creates a bounded multi-part Dragonfang structural blockout, derives one
profile-extrude AuthoringMeshV2 genesis from the real candidate, and persists
an immutable KnifeSourceBinding.  With ``--materialize`` it also replaces only
the SourceBinding-selected Part with that AuthoringMesh revision while keeping
the other source Part outputs.  Those optional paths still do not create a High
mesh, version, export, visual pass, human approval, or engine acceptance.
``--visual-pass`` executes one hash-bound Dragonfang front-view visual operation
for each selected candidate: it prepares an unconfirmed 63-point silhouette
target and runs one candidate-bound reference comparison.  With
``--correction-pass`` this is exactly two independent comparisons (initial and
corrected); the second is never treated as a replay.  It verifies the complete
nine-AOV RenderSet and comparison/quality lineage, then performs read-only
scene, target, RenderSet and AOV readback after Runtime restart.  The returned
quality status is recorded verbatim; this probe never upgrades it to commercial
or High quality.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import copy
import hashlib
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
from probe_mcp010b_raw_stdio import (  # noqa: E402
    GateFailure,
    MCP_PROTOCOL_VERSION,
    McpClient,
    build_identity,
    shutdown_runtime,
    wait_for_ready,
)


MAX_RESPONSE_BYTES = 1_048_576
SHA256 = re.compile(r"[0-9a-f]{64}")
DRAGONFANG_VISUAL_AOV_PASSES = (
    "beauty",
    "silhouette",
    "depth",
    "normal",
    "ao",
    "part-id",
    "material-id",
    "wireframe",
    "uv-stretch",
)

# img2threejs-inspired review contract.  These are deliberately probe-local
# constants: the product schemas already carry the source of truth and this
# script only checks that a live Brief is connected to the expected review
# loop.  Orbit views are diagnostics, never additional reference evidence.
DRAGONFANG_DETAIL_INVENTORY_COUNT = 18
DRAGONFANG_FIXED_VIEW_IDS = ("view-front", "view-orbit-a", "view-orbit-b", "view-fps-inspect")
DRAGONFANG_ORBIT_VIEW_IDS = ("view-orbit-a", "view-orbit-b")
DRAGONFANG_CORRECTION_SCOPE_POLICY = {
    "one_changed_scope_per_iteration": True,
    "baseline_preserved": True,
    "max_iterations_per_pass": 3,
    "max_iterations_total": 6,
}

DRAGONFANG_FRONT_REFERENCE_OBJECT_SHA256 = (
    "932c5ec407249678f69d1a9d61daa8f59177bf54766695e30ec3d2bbef00bf7e"
)
DRAGONFANG_FRONT_REFERENCE_WIDTH = 1536
DRAGONFANG_FRONT_REFERENCE_HEIGHT = 1024
# The original single-sheet reference and the later generated multiview sheet
# share 1536x1024 dimensions but do not share panel layout.  Bind the ROI to
# the immutable reference object hash; dimension-only routing silently cropped
# away the new sheet's spine, guard and handle.
DRAGONFANG_FRONT_CROP_PIXELS = {"x": 20, "y": 127, "width": 670, "height": 111}
DRAGONFANG_GENERATED_MULTIVIEW_FRONT_CROP_PIXELS = {
    "x": 15,
    "y": 20,
    "width": 880,
    "height": 180,
}

# The second user-authorized sheet is a generated multi-view design reference,
# not a literal orthographic capture.  It is kept as a separate intake profile
# so the original 2026-08-30 reference and its receipts remain reproducible.
# The Brief contract intentionally has no free-form panel metadata; the
# supplemental panel names are therefore orchestration/receipt metadata only.
DRAGONFANG_GENERATED_MULTIVIEW_REFERENCE_OBJECT_SHA256 = (
    "a8f1a169a3957cbeaaff2a8ceebcb9dd03802fcd7e165f043d329e8a5172dbd2"
)
DRAGONFANG_GENERATED_MULTIVIEW_SUPPLIED_VIEWS = (
    "front",
    "back",
    "left",
    "right",
    "top",
    "bottom",
    "fps-hold",
)
DRAGONFANG_GENERATED_MULTIVIEW_SUPPLEMENTAL_PANELS = (
    "guard-bottom",
    "pommel",
)
DRAGONFANG_GENERATED_MULTIVIEW_MISSING_VIEWS = (
    "front-three-quarter",
    "rear-three-quarter",
    "fps-inspect",
)

# This is a bounded, algorithm-derived contour proposal for the visible front
# silhouette.  Pixel coordinates stay as a reviewable source constant and are
# converted to full-image normalized coordinates only by the pure helper below.
# It is not user-confirmed shape truth.  The live probe sends it only as a
# bounded contour proposal to reference_mask_prepare with user_confirmed=false;
# it never becomes geometry or a quality approval.
DRAGONFANG_FRONT_PANEL_CONTOUR_PIXELS = (
    (31, 196),
    (63, 190),
    (187, 144),
    (229, 135),
    (256, 132),
    (257, 127),
    (357, 127),
    (358, 132),
    (432, 139),
    (455, 145),
    (480, 145),
    (498, 149),
    (509, 149),
    (515, 145),
    (535, 145),
    (537, 155),
    (541, 155),
    (544, 159),
    (565, 163),
    (570, 166),
    (578, 166),
    (582, 168),
    (583, 172),
    (606, 177),
    (627, 186),
    (640, 186),
    (657, 193),
    (659, 196),
    (684, 196),
    (666, 228),
    (663, 237),
    (622, 237),
    (620, 229),
    (604, 211),
    (585, 208),
    (582, 204),
    (569, 204),
    (563, 200),
    (542, 198),
    (539, 212),
    (534, 212),
    (530, 206),
    (520, 206),
    (507, 217),
    (491, 218),
    (477, 214),
    (475, 198),
    (468, 198),
    (459, 188),
    (440, 180),
    (404, 177),
    (399, 173),
    (381, 173),
    (368, 176),
    (364, 173),
    (354, 173),
    (331, 177),
    (290, 189),
    (260, 202),
    (226, 212),
    (162, 222),
    (105, 220),
    (70, 213),
)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise GateFailure(message)


def load_object(path: Path, label: str) -> dict[str, Any]:
    require(path.is_file() and not path.is_symlink(), f"{label} must be a regular file")
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise GateFailure(f"{label} is not valid JSON") from error
    require(isinstance(value, dict), f"{label} must be an object")
    return value


def canonical_hash(value: dict[str, Any], excluded_field: str) -> str:
    preimage = copy.deepcopy(value)
    preimage[excluded_field] = ""
    encoded = json.dumps(
        preimage,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def canonical_hash_without_field(value: dict[str, Any], excluded_field: str) -> str:
    preimage = copy.deepcopy(value)
    preimage.pop(excluded_field, None)
    return object_sha256(preimage)


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")


def object_sha256(value: Any) -> str:
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def geometry_program_semantic_sha256(value: dict[str, Any]) -> str:
    """Match GeometryProgram@2's remove-field canonical preimage exactly."""
    require(isinstance(value, dict), "GeometryProgram hash input must be an object")
    preimage = copy.deepcopy(value)
    preimage.pop("canonical_sha256", None)
    return object_sha256(preimage)


def require_sha256(value: Any, label: str) -> str:
    require(isinstance(value, str) and SHA256.fullmatch(value) is not None, f"{label} is not a SHA-256")
    return value


def verify_canonical_object(value: Any, field: str, label: str) -> str:
    require(isinstance(value, dict), f"{label} is not an object")
    actual = require_sha256(value.get(field), f"{label}.{field}")
    require(actual == canonical_hash(value, field), f"{label} canonical hash drifted")
    return actual


def _replace_reference_evidence_hash(value: Any, old_hash: str, new_hash: str) -> Any:
    """Rebind only fixture values that represented the previous image bytes.

    Brief successors must preserve the source-claim *shape* and all text
    evidence, while a new intake cohort must bind every image-derived claim to
    the newly imported ReferenceEvidence.  The operation is deliberately
    value-based: it cannot rewrite an unrelated text/hash claim and it does not
    change any checked-in historical fixture.
    """
    if isinstance(value, dict):
        return {
            key: _replace_reference_evidence_hash(item, old_hash, new_hash)
            for key, item in value.items()
        }
    if isinstance(value, list):
        return [_replace_reference_evidence_hash(item, old_hash, new_hash) for item in value]
    if value == old_hash:
        return new_hash
    return value


def dragonfang_front_mask_request(
    project_id: str,
    reference_id: str,
    reference_object_sha256: str,
    width: int,
    height: int,
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Build the one-shot, unconfirmed 63-point target request and view spec."""
    view_spec = dragonfang_front_reference_view_spec(
        reference_id, reference_object_sha256, width, height
    )
    points = _normalize_full_image_contour(
        DRAGONFANG_FRONT_PANEL_CONTOUR_PIXELS, width, height
    )
    require(len(points) == 63, "Dragonfang front mask contour must contain exactly 63 points")
    return (
        {
            "project_id": project_id,
            "reference_id": reference_id,
            "contour_points": points,
            "user_confirmed": False,
        },
        view_spec,
    )


def verify_reference_mask_result(
    result: dict[str, Any],
    request: dict[str, Any],
    project_id: str,
    reference: dict[str, Any],
) -> dict[str, Any]:
    """Verify the actual Runtime mask/target envelope without promoting it."""
    expected = {
        "schema_version",
        "project_id",
        "reference_id",
        "target_sha256",
        "mask_sha256",
        "target",
        "canonical_sha256",
    }
    require(set(result) == expected, "ReferenceMaskPrepareResult field set drifted")
    require(result["schema_version"] == "ReferenceMaskPrepareResult@1", "mask result schema drifted")
    require(result["project_id"] == project_id, "mask result project binding drifted")
    require(result["reference_id"] == reference["reference_id"], "mask result reference binding drifted")
    target_sha = require_sha256(result["target_sha256"], "mask target_sha256")
    mask_sha = require_sha256(result["mask_sha256"], "mask mask_sha256")
    verify_canonical_object(result, "canonical_sha256", "ReferenceMaskPrepareResult")
    target = result.get("target")
    require(isinstance(target, dict), "mask result omitted SilhouetteTarget")
    target_canonical = verify_canonical_object(target, "canonical_sha256", "SilhouetteTarget")
    require(object_sha256(target) == target_sha, "SilhouetteTarget CAS object hash drifted")
    require(target.get("schema_version") == "SilhouetteTarget@1", "mask target schema drifted")
    require(target.get("reference_id") == reference["reference_id"], "mask target reference drifted")
    require(
        target.get("reference_sha256") == reference["object_sha256"],
        "mask target reference object binding drifted",
    )
    require(target.get("mask_sha256") == mask_sha, "mask target/mask hash drifted")
    require(target.get("width") == 512 and target.get("height") == 512, "mask target dimensions drifted")
    require(target.get("coordinate_space") == "normalized_reference_image", "mask target coordinate space drifted")
    require(target.get("source") == "user_refined", "unconfirmed contour was not recorded as user_refined")
    require(target.get("annotation_status") == "unreviewed", "unconfirmed contour crossed review boundary")
    actual_points = target.get("contour_points")
    requested_points = request.get("contour_points")
    require(
        isinstance(actual_points, list)
        and len(actual_points) == 63
        and isinstance(requested_points, list)
        and len(requested_points) == 63
        and all(
            isinstance(actual, list)
            and isinstance(requested, list)
            and len(actual) == 2
            and len(requested) == 2
            and all(
                isinstance(actual[index], (int, float))
                and isinstance(requested[index], (int, float))
                and abs(float(actual[index]) - float(requested[index])) <= 1.0e-15
                for index in range(2)
            )
            for actual, requested in zip(actual_points, requested_points)
        ),
        "Runtime mask target contour does not bind the ordered 63-point request",
    )
    return {
        "target_sha256": target_sha,
        "mask_sha256": mask_sha,
        "target_canonical_sha256": target_canonical,
        "target": target,
    }


def _candidate_artifact_sha(candidate: dict[str, Any], artifact: dict[str, Any]) -> str:
    artifact_sha = next(
        (
            value
            for value in (
                artifact.get("object_sha256"),
                artifact.get("artifact_id"),
            )
            if isinstance(value, str) and SHA256.fullmatch(value)
        ),
        None,
    )
    require(artifact_sha is not None, "visual artifact readback did not expose an artifact SHA-256")
    candidate_hashes = {
        value
        for value in (
            candidate.get("prepared_object_sha256"),
            candidate.get("manifest_hash"),
        )
        if isinstance(value, str) and SHA256.fullmatch(value)
    }
    require(
        artifact_sha in candidate_hashes,
        "visual artifact is not bound to the selected candidate state",
    )
    return artifact_sha


def verify_artifact_readback(
    artifact: dict[str, Any], candidate_id: str, label: str
) -> tuple[str, str]:
    """Verify the actual candidate GLB readback used by the renderer.

    The comparison Runtime performs its own GLB inspection, but the live
    probe must bind the comparison to the readback returned by the current
    candidate rather than to an earlier prepare response or a caller-supplied
    hash.  The returned pair is (artifact/object SHA, geometry-program SHA).
    """
    verify_canonical_object(artifact, "canonical_sha256", label)
    require(artifact.get("schema_version") == "ArtifactReadback@2", f"{label} schema drifted")
    require(artifact.get("candidate_id") == candidate_id, f"{label} candidate binding drifted")
    artifact_sha = require_sha256(artifact.get("object_sha256"), f"{label}.object_sha256")
    require(artifact.get("artifact_id") == artifact_sha, f"{label} artifact/object identity drifted")
    require(artifact.get("mime") == "model/gltf-binary", f"{label} MIME drifted")
    require(artifact.get("hard_gate_passed") is True, f"{label} strict GLB gate did not pass")
    program_sha = require_sha256(artifact.get("program_sha256"), f"{label}.program_sha256")
    return artifact_sha, program_sha


def verify_reference_compare_result(
    result: dict[str, Any],
    project_id: str,
    candidate: dict[str, Any],
    artifact: dict[str, Any],
    reference: dict[str, Any],
    view_spec: dict[str, Any],
    mask: dict[str, Any],
    expected_build_cohort: str,
) -> dict[str, Any]:
    """Strictly verify the one live comparison and return hash-only identity."""
    expected_result_fields = {
        "schema_version",
        "candidate_id",
        "reference_id",
        "camera",
        "camera_object_sha256",
        "render_set",
        "render_set_hash",
        "render_set_object_sha256",
        "comparison_report",
        "comparison_report_hash",
        "comparison_report_object_sha256",
        "quality_report",
        "quality_report_object_sha256",
    }
    require(set(result) == expected_result_fields, "ReferenceComparisonPrepareResult field set drifted")
    require(result.get("schema_version") == "ReferenceComparisonPrepareResult@1", "comparison result schema drifted")
    candidate_id = candidate.get("candidate_id")
    reference_id = reference.get("reference_id")
    require(result.get("candidate_id") == candidate_id, "comparison candidate binding drifted")
    require(result.get("reference_id") == reference_id, "comparison reference binding drifted")
    require(isinstance(candidate_id, str) and isinstance(reference_id, str), "comparison identity is incomplete")

    view_spec_canonical = verify_canonical_object(view_spec, "canonical_sha256", "ReferenceViewSpec")
    require(view_spec.get("schema_version") == "ReferenceViewSpec@1", "ReferenceViewSpec schema drifted")
    require(view_spec.get("reference_id") == reference_id, "ReferenceViewSpec reference binding drifted")
    require(view_spec.get("reference_sha256") == reference.get("object_sha256"), "ReferenceViewSpec reference hash drifted")
    require(view_spec.get("view_id") == "view-front", "ReferenceViewSpec view_id drifted")
    require(view_spec.get("source_view") == "front", "ReferenceViewSpec source view drifted")
    image = view_spec.get("image")
    require(isinstance(image, dict), "ReferenceViewSpec image is missing")
    require(
        image.get("width") == DRAGONFANG_FRONT_REFERENCE_WIDTH
        and image.get("height") == DRAGONFANG_FRONT_REFERENCE_HEIGHT,
        "ReferenceViewSpec image dimensions drifted",
    )
    crop = image.get("crop")
    require(isinstance(crop, dict), "ReferenceViewSpec crop is missing")
    expected_crop = _dragonfang_front_normalized_crop(reference["object_sha256"])
    require(crop == expected_crop, "ReferenceViewSpec normalized crop drifted")
    regions = view_spec.get("regions")
    require(
        isinstance(regions, list)
        and len(regions) == 1
        and isinstance(regions[0], dict)
        and regions[0].get("visibility") == "observed",
        "ReferenceViewSpec observed region binding drifted",
    )

    camera = result.get("camera")
    require(isinstance(camera, dict), "comparison omitted camera calibration")
    verify_canonical_object(camera, "canonical_sha256", "CameraCalibration")
    camera_hash = require_sha256(camera.get("camera_hash"), "comparison camera_hash")
    camera_object_sha = require_sha256(result.get("camera_object_sha256"), "comparison camera_object_sha256")
    require(object_sha256(camera) == camera_object_sha, "camera CAS object hash drifted")
    require(result.get("render_set", {}).get("camera_object_sha256") == camera_object_sha, "RenderSet camera object binding drifted")

    render_set = result.get("render_set")
    require(isinstance(render_set, dict), "comparison omitted RenderSet")
    render_set_canonical = verify_canonical_object(render_set, "canonical_sha256", "RenderSet@2")
    render_set_object_sha = require_sha256(result.get("render_set_object_sha256"), "render_set_object_sha256")
    require(object_sha256(render_set) == render_set_object_sha, "RenderSet CAS object hash drifted")
    require(result.get("render_set_hash") == render_set_object_sha, "render_set_hash/object hash drifted")
    require(render_set.get("schema_version") == "RenderSet@2", "RenderSet schema drifted")
    require(render_set.get("project_id") is None, "RenderSet unexpectedly introduced an uncontracted project field")
    require(render_set.get("candidate_id") == candidate_id, "RenderSet candidate binding drifted")
    require(render_set.get("reference_id") == reference_id, "RenderSet reference binding drifted")
    require(render_set.get("view_id") == view_spec.get("view_id"), "RenderSet view_id is not view_spec.view_id")
    require(render_set.get("camera_hash") == camera_hash, "RenderSet camera hash drifted")
    require(render_set.get("render_worker_binding_status") == "same_cohort_verified", "RenderSet worker cohort was not verified")
    require(render_set.get("render_worker_build_cohort_sha256") == expected_build_cohort, "RenderSet worker cohort drifted")
    require(render_set.get("width") == 512 and render_set.get("height") == 512, "RenderSet dimensions drifted")
    require(render_set.get("passes") == list(DRAGONFANG_VISUAL_AOV_PASSES), "RenderSet AOV order/set drifted")
    pass_artifacts = render_set.get("pass_artifacts")
    require(isinstance(pass_artifacts, dict), "RenderSet omitted pass_artifacts")
    require(set(pass_artifacts) == set(DRAGONFANG_VISUAL_AOV_PASSES), "RenderSet did not expose exactly nine AOV artifacts")
    expected_artifact_sha = _candidate_artifact_sha(candidate, artifact)
    require(render_set.get("artifact_sha256") == expected_artifact_sha, "RenderSet artifact is not candidate-bound")
    program_sha = artifact.get("program_sha256")
    if isinstance(program_sha, str) and SHA256.fullmatch(program_sha):
        require(render_set.get("program_sha256") == program_sha, "RenderSet geometry program binding drifted")
    aov_hashes: dict[str, str] = {}
    for pass_name in DRAGONFANG_VISUAL_AOV_PASSES:
        entry = pass_artifacts.get(pass_name)
        require(isinstance(entry, dict), f"RenderSet {pass_name} artifact is not an object")
        aov_hashes[pass_name] = require_sha256(entry.get("sha256"), f"RenderSet {pass_name}.sha256")
        require(entry.get("mime") == "image/png", f"RenderSet {pass_name} MIME drifted")
        require(isinstance(entry.get("size_bytes"), int) and entry["size_bytes"] > 0, f"RenderSet {pass_name} size is invalid")
        require(entry.get("width") == 512 and entry.get("height") == 512, f"RenderSet {pass_name} dimensions drifted")
        require(entry.get("channels") == "rgba8", f"RenderSet {pass_name} channels drifted")
        require(entry.get("color_space") == ("srgb" if pass_name == "beauty" else "data"), f"RenderSet {pass_name} color space drifted")

    comparison = result.get("comparison_report")
    require(isinstance(comparison, dict), "comparison result omitted ReferenceComparisonReport")
    comparison_canonical = verify_canonical_object(comparison, "canonical_sha256", "ReferenceComparisonReport")
    comparison_object_sha = require_sha256(result.get("comparison_report_object_sha256"), "comparison_report_object_sha256")
    require(object_sha256(comparison) == comparison_object_sha, "comparison report CAS object hash drifted")
    require(result.get("comparison_report_hash") == comparison_object_sha, "comparison_report_hash/object hash drifted")
    require(comparison.get("schema_version") == "ReferenceComparisonReport@1", "comparison report schema drifted")
    require(comparison.get("candidate_id") == candidate_id and comparison.get("reference_id") == reference_id, "comparison report identity drifted")
    require(comparison.get("view_id") == view_spec.get("view_id"), "comparison report view_id drifted")
    require(comparison.get("artifact_sha256") == expected_artifact_sha, "comparison report artifact binding drifted")
    require(comparison.get("reference_sha256") == reference.get("object_sha256"), "comparison report reference hash drifted")
    require(comparison.get("render_set_hash") == render_set_object_sha, "comparison report RenderSet binding drifted")
    require(comparison.get("camera_hash") == camera_hash, "comparison report camera binding drifted")
    comparison_mask = comparison.get("mask")
    require(isinstance(comparison_mask, dict), "comparison report omitted mask")
    require(comparison_mask.get("method") == "silhouette-target", "comparison did not use the prepared silhouette target")
    # Runtime projects the full-reference silhouette mask into the fixed view
    # before comparison.  The comparison therefore binds a new PNG CAS object,
    # while VisualEvidence.target_sha256 keeps the immutable SilhouetteTarget
    # lineage.  Treating both PNG hashes as identical rejects a valid crop and
    # also supplies the wrong CAS identity to KnifePassState.
    comparison_mask_object_sha256 = require_sha256(
        comparison_mask.get("sha256"), "comparison mask object sha256"
    )
    require(comparison_mask.get("width") == 512 and comparison_mask.get("height") == 512, "comparison mask dimensions drifted")
    comparison_status = comparison.get("status")
    require(comparison_status in {"PARTIAL_VISIBLE_VIEW_PASS", "QUALITY_TARGET_NOT_MET", "BLOCKED_REFERENCE_COVERAGE"}, "comparison status is outside the closed contract")
    require(comparison_status != "PARTIAL_VISIBLE_VIEW_PASS", "unconfirmed contour was incorrectly promoted to a visual pass")

    quality = result.get("quality_report")
    require(isinstance(quality, dict), "comparison result omitted QualityReport")
    quality_canonical = verify_canonical_object(quality, "canonical_sha256", "QualityReport@2")
    quality_object_sha = require_sha256(result.get("quality_report_object_sha256"), "quality_report_object_sha256")
    require(object_sha256(quality) == quality_object_sha, "quality report CAS object hash drifted")
    require(quality.get("schema_version") == "QualityReport@2", "quality report schema drifted")
    require(quality.get("candidate_id") == candidate_id and quality.get("reference_id") == reference_id, "quality report identity drifted")
    require(quality.get("reference_sha256") == reference.get("object_sha256"), "quality report reference binding drifted")
    require(quality.get("view_id") == view_spec.get("view_id"), "quality report view_id drifted")
    require(quality.get("artifact_sha256") == expected_artifact_sha, "quality report artifact binding drifted")
    require(quality.get("program_sha256") == render_set.get("program_sha256"), "quality report program binding drifted")
    require(quality.get("render_set_hash") == render_set_object_sha, "quality report RenderSet binding drifted")
    require(quality.get("comparison_report_hash") == comparison_object_sha, "quality report comparison binding drifted")
    require(quality.get("visual_status") == comparison_status, "quality/comparison visual status drifted")
    require(quality.get("benchmark_eligibility") == comparison.get("benchmark_eligibility"), "quality/comparison benchmark binding drifted")
    require(quality.get("hard_gate_passed") is False, "unconfirmed contour incorrectly passed the quality hard gate")
    require(quality.get("human_receipt_hash") is None, "visual probe unexpectedly recorded human review")
    return {
        "view_id": view_spec["view_id"],
        "reference_id": reference_id,
        "candidate_id": candidate_id,
        "reference_view_spec_sha256": view_spec_canonical,
        "target_sha256": mask["target_sha256"],
        "mask_sha256": mask["mask_sha256"],
        "comparison_mask_object_sha256": comparison_mask_object_sha256,
        "camera_hash": camera_hash,
        "camera_object_sha256": camera_object_sha,
        "camera_id": camera.get("camera_id"),
        "render_set_id": render_set.get("render_set_id"),
        "render_set_sha256": render_set_canonical,
        "render_set_object_sha256": render_set_object_sha,
        "render_worker_build_cohort_sha256": expected_build_cohort,
        "aov_pass_sha256": aov_hashes,
        "reference_comparison_id": comparison.get("report_id"),
        "reference_comparison_sha256": comparison_canonical,
        "reference_comparison_object_sha256": comparison_object_sha,
        "quality_report_id": quality.get("quality_report_id"),
        "quality_report_sha256": quality_canonical,
        "quality_report_object_sha256": quality_object_sha,
        "comparison_status": comparison_status,
        "quality_status": quality.get("visual_status"),
        "benchmark_eligibility": comparison.get("benchmark_eligibility"),
        "candidate_artifact_sha256": expected_artifact_sha,
        "candidate_state_sha256": require_sha256(
            candidate.get("canonical_sha256"), "visual candidate canonical_sha256"
        ),
        "artifact_readback_sha256": require_sha256(
            artifact.get("canonical_sha256"), "visual ArtifactReadback canonical_sha256"
        ),
        "geometry_program_sha256": require_sha256(
            artifact.get("program_sha256"), "visual ArtifactReadback program_sha256"
        ),
    }


def verify_visual_evidence_projection(
    value: dict[str, Any],
    project_id: str,
    candidate_id: str,
    identity: dict[str, Any],
    label: str,
) -> None:
    """Verify a read-only Agentic visual bundle against compare identities."""
    verify_canonical_object(value, "canonical_sha256", label)
    require(value.get("schema_version") == "VisualEvidenceBundle@1", f"{label} schema drifted")
    require(value.get("available") is True and value.get("read_only") is True, f"{label} is not an available read-only bundle")
    require(value.get("project_id") == project_id and value.get("candidate_id") == candidate_id, f"{label} scope drifted")
    require(
        value.get("reference_id") == identity["reference_id"],
        f"{label} reference binding drifted",
    )
    hashes = value.get("hashes")
    require(isinstance(hashes, dict), f"{label} hashes are missing")
    require(hashes.get("target_sha256") == identity["target_sha256"], f"{label} target binding drifted")
    require(hashes.get("render_set_hash") == identity["render_set_object_sha256"], f"{label} RenderSet binding drifted")
    require(hashes.get("comparison_report_hash") == identity["reference_comparison_object_sha256"], f"{label} comparison binding drifted")
    require(hashes.get("quality_report_hash") == identity["quality_report_object_sha256"], f"{label} quality binding drifted")
    render_set = value.get("render_set")
    require(isinstance(render_set, dict), f"{label} RenderSet is missing")
    verify_canonical_object(render_set, "canonical_sha256", f"{label} RenderSet")
    require(render_set.get("view_id") == identity["view_id"], f"{label} view_id drifted")
    require(render_set.get("candidate_id") == candidate_id, f"{label} candidate binding drifted")
    require(render_set.get("reference_id") == identity["reference_id"], f"{label} reference binding drifted")
    require(set(render_set.get("pass_artifacts", {})) == set(DRAGONFANG_VISUAL_AOV_PASSES), f"{label} AOV set drifted")
    comparison = value.get("comparison_report")
    quality = value.get("quality_report")
    require(isinstance(comparison, dict) and isinstance(quality, dict), f"{label} reports are missing")
    verify_canonical_object(comparison, "canonical_sha256", f"{label} comparison report")
    verify_canonical_object(quality, "canonical_sha256", f"{label} quality report")
    require(comparison.get("view_id") == identity["view_id"], f"{label} comparison view_id drifted")
    require(quality.get("view_id") == identity["view_id"], f"{label} quality view_id drifted")
    require(comparison.get("candidate_id") == candidate_id, f"{label} comparison candidate drifted")
    require(quality.get("candidate_id") == candidate_id, f"{label} quality candidate drifted")
    require(comparison.get("reference_id") == identity["reference_id"], f"{label} comparison reference drifted")
    require(quality.get("reference_id") == identity["reference_id"], f"{label} quality reference drifted")
    require(comparison.get("render_set_hash") == identity["render_set_object_sha256"], f"{label} comparison RenderSet drifted")
    require(quality.get("comparison_report_hash") == identity["reference_comparison_object_sha256"], f"{label} quality comparison drifted")
    require(quality.get("visual_status") == identity["quality_status"], f"{label} quality status drifted")


def verify_geometry_evidence_lineage(
    observation: dict[str, Any],
    project_id: str,
    candidate_id: str,
    artifact_sha256: str,
    program_sha256: str,
    label: str,
) -> dict[str, str]:
    """Extract the durable GeometryCandidateEvidence identities from observe.

    Candidate/ArtifactReadback responses deliberately do not expose the
    evidence object's CAS identity.  The Runtime-owned scene observation is
    the read-only projection that does, so PassState construction must use its
    lineage rather than guessing an object hash from a semantic hash.
    """
    lineage = observation.get("lineage")
    require(isinstance(lineage, dict), f"{label} observation lineage is missing")
    require(lineage.get("project_id") == project_id, f"{label} lineage project drifted")
    require(lineage.get("candidate_id") == candidate_id, f"{label} lineage candidate drifted")
    require(
        lineage.get("artifact_sha256") == artifact_sha256,
        f"{label} lineage artifact drifted",
    )
    require(
        lineage.get("geometry_program_sha256") == program_sha256,
        f"{label} lineage GeometryProgram drifted",
    )
    program_object_sha256 = require_sha256(
        lineage.get("geometry_program_object_sha256"),
        f"{label} lineage geometry_program_object_sha256",
    )
    readback_object_sha256 = require_sha256(
        lineage.get("artifact_readback_sha256"),
        f"{label} lineage artifact_readback_sha256",
    )
    return {
        "artifact_sha256": artifact_sha256,
        "program_sha256": program_sha256,
        "program_object_sha256": program_object_sha256,
        "readback_object_sha256": readback_object_sha256,
    }


def observe_candidate_geometry(
    client: McpClient,
    project_id: str,
    candidate_id: str,
    artifact_sha256: str,
    program_sha256: str,
    label: str,
) -> tuple[dict[str, Any], str, dict[str, str]]:
    """Read one candidate projection and return its geometry evidence lineage."""
    observation = facade_tool(
        client,
        "observe",
        "scene_observe_get",
        {"project_id": project_id, "candidate_id": candidate_id},
    )
    observation_sha256 = verify_canonical_object(
        observation, "canonical_sha256", f"{label} AgenticSceneObserveResult"
    )
    require(
        observation.get("project_id") == project_id
        and observation.get("candidate_id") == candidate_id
        and observation.get("read_only") is True,
        f"{label} scene observation is not candidate-bound",
    )
    geometry = verify_geometry_evidence_lineage(
        observation,
        project_id,
        candidate_id,
        artifact_sha256,
        program_sha256,
        label,
    )
    return observation, observation_sha256, geometry


def run_dragonfang_visual_candidate(
    client: McpClient,
    project_id: str,
    candidate: dict[str, Any],
    artifact: dict[str, Any],
    reference: dict[str, Any],
    expected_build_cohort: str,
    label: str,
    artifact_readback: dict[str, Any] | None = None,
    mask_context: tuple[
        dict[str, Any], dict[str, Any], dict[str, Any], dict[str, Any]
    ]
    | None = None,
) -> dict[str, Any]:
    """Execute exactly one mask/compare pair for one immutable candidate.

    The helper is intentionally candidate-scoped.  Calling it twice is two
    independent visual attempts (baseline and correction), never a comparison
    replay.  All post-compare reads are projections and cannot create another
    RenderSet or report.
    """
    require(isinstance(candidate, dict) and isinstance(artifact, dict), f"{label} candidate/artifact is unavailable")
    candidate_id = candidate.get("candidate_id")
    require(isinstance(candidate_id, str) and candidate_id, f"{label} candidate_id is missing")
    if artifact_readback is None:
        artifact_readback = facade_tool(
            client,
            "observe",
            "artifact_readback_get",
            {"artifact_id": artifact.get("artifact_id"), "candidate_id": candidate_id},
        )
    selected_artifact_sha, selected_program_sha = verify_artifact_readback(
        artifact_readback, candidate_id, f"{label} visual ArtifactReadback"
    )
    require(
        selected_artifact_sha == _candidate_artifact_sha(candidate, artifact),
        f"{label} visual ArtifactReadback is not bound to its candidate artifact",
    )
    require(
        selected_program_sha == artifact.get("program_sha256"),
        f"{label} visual ArtifactReadback GeometryProgram drifted",
    )
    if mask_context is None:
        mask_request, view_spec = dragonfang_front_mask_request(
            project_id=project_id,
            reference_id=reference["reference_id"],
            reference_object_sha256=reference["object_sha256"],
            width=reference["width"],
            height=reference["height"],
        )
        mask_result = facade_tool(
            client, "reference_intake", "reference_mask_prepare", mask_request
        )
        mask = verify_reference_mask_result(
            mask_result, mask_request, project_id, reference
        )
    else:
        mask_result, mask_request, mask, view_spec = mask_context
        require(
            mask_request.get("project_id") == project_id
            and mask_request.get("reference_id") == reference["reference_id"],
            f"{label} shared mask context scope drifted",
        )
    compare_request = {
        "project_id": project_id,
        "candidate_id": candidate_id,
        "reference_id": reference["reference_id"],
        "view_spec": view_spec,
        "target_sha256": mask["target_sha256"],
    }
    compare_result = facade_tool(
        client,
        "quality_review",
        "reference_compare_prepare",
        compare_request,
    )
    identity = verify_reference_compare_result(
        compare_result,
        project_id,
        candidate,
        artifact_readback,
        reference,
        view_spec,
        mask,
        expected_build_cohort,
    )
    observation, observation_sha256, geometry = observe_candidate_geometry(
        client,
        project_id,
        candidate_id,
        selected_artifact_sha,
        selected_program_sha,
        label,
    )
    bundle = facade_tool(
        client,
        "quality_review",
        "visual_evidence_bundle_get",
        {
            "project_id": project_id,
            "candidate_id": candidate_id,
            "observation_sha256": observation_sha256,
        },
    )
    verify_visual_evidence_projection(
        bundle, project_id, candidate_id, identity, f"{label} visual evidence bundle"
    )
    return {
        "candidate": candidate,
        "artifact": artifact,
        "artifact_readback": artifact_readback,
        "mask_request": mask_request,
        "mask_result": mask_result,
        "mask": mask,
        "view_spec": view_spec,
        "compare_request": compare_request,
        "compare_result": compare_result,
        "identity": identity,
        "observation": observation,
        "observation_sha256": observation_sha256,
        "geometry": geometry,
        "bundle": bundle,
        "mask_context": (mask_result, mask_request, mask, view_spec),
    }


PASS_STATE_RESULT_FIELDS = {
    "schema_version", "operation", "request_kind", "status", "project_id",
    "pass_id", "pass_state_sha256", "pass_state_object_sha256", "pass_state",
    "source_binding_id", "source_binding_sha256", "source_binding_object_sha256",
    "intent_bundle_id", "intent_bundle_sha256", "intent_bundle_object_sha256",
    "brief_id", "brief_sha256", "brief_object_sha256", "reference_id",
    "reference_object_sha256", "reference_evidence_sha256", "source_candidate_id",
    "source_candidate_state_sha256", "baseline_candidate_id", "baseline_candidate_state_sha256",
    "baseline_artifact_sha256", "baseline_geometry_program_sha256",
    "baseline_geometry_program_object_sha256", "baseline_artifact_readback_object_sha256",
    "baseline_representation_plan_sha256", "attempt_candidate_id",
    "attempt_candidate_state_sha256", "attempt_artifact_sha256",
    "attempt_geometry_program_sha256", "attempt_geometry_program_object_sha256",
    "attempt_artifact_readback_object_sha256", "attempt_representation_plan_sha256",
    "authoring_mesh_id", "authoring_mesh_lineage_id", "authoring_mesh_revision_id",
    "authoring_mesh_revision_index", "authoring_mesh_revision_sha256",
    "authoring_mesh_revision_object_sha256", "authoring_mesh_identity_sha256",
    "authoring_mesh_sha256", "fixed_view_id", "camera_set_sha256", "render_set_id",
    "render_set_sha256", "render_set_object_sha256", "reference_comparison_id",
    "reference_comparison_sha256", "reference_comparison_object_sha256",
    "quality_report_id", "quality_report_sha256", "quality_report_object_sha256",
    "evidence_bundle_sha256", "hard_gate_status", "visual_gate_status", "quality_status",
    "high_status", "human_status", "engine_status", "high_mesh_created",
    "high_stage_unlocked", "production_stage_advanced", "candidate_confirmed",
    "version_created", "export_performed", "idempotency_key", "replayed",
    "store_effect", "cas_effect", "atomicity_status", "store_commit_status",
    "cas_commit_status", "runtime_write_performed", "persistent_user_data_touched",
    "partial_result_exposed", "writer_policy", "canonicalization_policy", "canonical_sha256",
}

PASS_STATE_GET_MAIN_FIELDS = (
    "source_binding_id", "source_binding_sha256", "source_binding_object_sha256",
    "intent_bundle_id", "intent_bundle_sha256", "intent_bundle_object_sha256",
    "brief_id", "brief_sha256", "brief_object_sha256", "reference_id",
    "reference_object_sha256", "reference_evidence_sha256", "source_candidate_id",
    "source_candidate_state_sha256", "baseline_candidate_id", "baseline_candidate_state_sha256",
    "baseline_artifact_sha256", "baseline_geometry_program_sha256",
    "baseline_geometry_program_object_sha256", "baseline_artifact_readback_object_sha256",
    "baseline_representation_plan_sha256", "attempt_candidate_id",
    "attempt_candidate_state_sha256", "attempt_artifact_sha256",
    "attempt_geometry_program_sha256", "attempt_geometry_program_object_sha256",
    "attempt_artifact_readback_object_sha256", "attempt_representation_plan_sha256",
    "authoring_mesh_id", "authoring_mesh_lineage_id", "authoring_mesh_revision_id",
    "authoring_mesh_revision_index", "authoring_mesh_revision_sha256",
    "authoring_mesh_revision_object_sha256", "authoring_mesh_identity_sha256",
    "authoring_mesh_sha256", "camera_set_sha256", "render_set_id", "render_set_sha256",
    "render_set_object_sha256", "reference_comparison_id", "reference_comparison_sha256",
    "reference_comparison_object_sha256", "quality_report_id", "quality_report_sha256",
    "quality_report_object_sha256", "evidence_bundle_sha256",
)


def _pass_state_unknowns(brief: dict[str, Any], intent: dict[str, Any]) -> list[dict[str, Any]]:
    """Mirror Runtime's deterministic unknown projection for the checked Brief."""
    missing = brief.get("reference_coverage", {}).get("missing_views")
    require(isinstance(missing, list), "PassState Brief missing_views is unavailable")
    descriptions = {
        "front-three-quarter": "A front three-quarter authorized reference is required before multi-view silhouette acceptance.",
        "rear-three-quarter": "A rear three-quarter authorized reference is required before multi-view silhouette acceptance.",
        "top": "A top authorized reference is required before proportion acceptance.",
        "bottom": "A bottom authorized reference is required before proportion acceptance.",
        "fps-inspect": "An FPS inspect authorized reference is required before first-person presentation acceptance.",
        "front": "An authorized primary reference view is required before silhouette acceptance.",
        "back": "An authorized primary reference view is required before silhouette acceptance.",
        "left": "An authorized primary reference view is required before silhouette acceptance.",
        "right": "An authorized primary reference view is required before silhouette acceptance.",
    }
    unknowns = []
    for view in missing:
        require(isinstance(view, str) and view in descriptions, "PassState Brief missing view is invalid")
        unknowns.append({
            "unknown_id": f"missing-{view}-reference",
            "category": "reference-coverage",
            "view_kind": view,
            "description": descriptions[view],
            "impact": "blocking",
            "status": "open",
        })
    if not unknowns:
        fixed_views = intent.get("quality_contract", {}).get("fixed_views", [])
        primary = next(
            view for view in fixed_views
            if view.get("comparison_role") == "primary-reference"
            and view.get("reference_required") is True
        )
        unknowns.append({
            "unknown_id": "pass-state-promotion-locked",
            "category": "lineage",
            "view_kind": primary["view"],
            "description": "Promotion remains locked until Runtime-owned structural and review evidence is complete.",
            "impact": "blocking",
            "status": "open",
        })
    return unknowns


def _pass_state_geometry_truth(
    candidate: dict[str, Any],
    artifact: dict[str, Any],
    geometry: dict[str, str],
    representation_plan_sha256: str,
    label: str,
) -> dict[str, str]:
    candidate_id = candidate.get("candidate_id")
    require(isinstance(candidate_id, str) and candidate_id, f"{label} candidate_id is missing")
    candidate_state_sha256 = require_sha256(candidate.get("canonical_sha256"), f"{label} candidate state")
    artifact_sha256 = _candidate_artifact_sha(candidate, artifact)
    require(geometry.get("artifact_sha256") == artifact_sha256, f"{label} artifact lineage drifted")
    program_sha256 = require_sha256(artifact.get("program_sha256"), f"{label} GeometryProgram semantic hash")
    require(geometry.get("program_sha256") == program_sha256, f"{label} program lineage drifted")
    require_sha256(representation_plan_sha256, f"{label} representation plan")
    require(candidate.get("quality_hard_gate_passed") is True, f"{label} candidate structural gate is not true")
    return {
        "candidate_id": candidate_id,
        "candidate_state_sha256": candidate_state_sha256,
        "artifact_sha256": artifact_sha256,
        "program_sha256": program_sha256,
        "program_object_sha256": require_sha256(geometry.get("program_object_sha256"), f"{label} program object"),
        "readback_object_sha256": require_sha256(geometry.get("readback_object_sha256"), f"{label} ArtifactReadback object"),
        "representation_plan_sha256": representation_plan_sha256,
    }


def _pass_state_fixed_view(visual_identity: dict[str, Any]) -> tuple[dict[str, Any], str]:
    fixed_view = {
        "view_id": visual_identity["view_id"],
        "view_kind": "front",
        "comparison_role": "primary-reference",
        "reference_required": True,
        "camera_id": visual_identity.get("camera_id") or f"knife-camera-front-{visual_identity['camera_hash'][:16]}",
        "camera_sha256": visual_identity["camera_hash"],
        "reference_view_id": visual_identity["view_id"],
        "reference_view_sha256": visual_identity["comparison_mask_object_sha256"],
        "fixed_view_policy": "single-runtime-bound-primary-reference-view@1",
    }
    camera_set = {
        "schema_version": "KnifeCameraSet@1",
        "fixed_views": [fixed_view],
        "fixed_view_count": 1,
    }
    return fixed_view, object_sha256(camera_set)


def build_knife_pass_state_main(
    *,
    pass_id: str,
    parent_pass_id: str | None,
    parent_pass_sha256: str | None,
    source_binding_result: dict[str, Any],
    intent_result: dict[str, Any],
    successor_brief: dict[str, Any],
    reference: dict[str, Any],
    source_candidate: dict[str, Any],
    baseline_candidate: dict[str, Any],
    baseline_artifact: dict[str, Any],
    baseline_geometry: dict[str, str],
    baseline_representation_plan_sha256: str,
    attempt_candidate: dict[str, Any],
    attempt_artifact: dict[str, Any],
    attempt_geometry: dict[str, str],
    attempt_representation_plan_sha256: str,
    selected_revision: dict[str, Any],
    visual_identity: dict[str, Any],
    created_at: str,
    parent_visual_identity: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Build the exact closed Main proposal consumed by Runtime PassState."""
    require(bool(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]{0,127}", pass_id)), "PassState pass_id is invalid")
    if parent_pass_id is None:
        require(parent_pass_sha256 is None, "root PassState parent hash must be null")
        require(created_at.endswith("Z"), "PassState created_at is invalid")
    else:
        require(parent_pass_sha256 is not None and SHA256.fullmatch(parent_pass_sha256), "child PassState parent hash is invalid")
        require(parent_visual_identity is not None, "child PassState parent visual identity is missing")
    source = source_binding_result
    intent = intent_result.get("intent_bundle")
    require(isinstance(intent, dict), "PassState intent bundle is missing")
    source_truth = _pass_state_geometry_truth(
        baseline_candidate,
        baseline_artifact,
        baseline_geometry,
        baseline_representation_plan_sha256,
        "PassState baseline",
    )
    attempt_truth = _pass_state_geometry_truth(
        attempt_candidate,
        attempt_artifact,
        attempt_geometry,
        attempt_representation_plan_sha256,
        "PassState attempt",
    )
    # The selected mesh revision is Runtime-derived.  This helper only
    # constructs a proposal; Runtime re-derives and compares every field.
    selected_revision_object = selected_revision.get("authoring_mesh_v2")
    if not isinstance(selected_revision_object, dict):
        selected_revision_object = selected_revision
    selected_revision = {
        key: selected_revision.get(key, selected_revision_object.get(key))
        for key in (
            "mesh_id",
            "lineage_id",
            "revision_id",
            "revision_index",
            "revision_sha256",
            "revision_object_sha256",
        )
    }
    for key in ("mesh_id", "lineage_id", "revision_id", "revision_sha256", "revision_object_sha256"):
        require(isinstance(selected_revision.get(key), (str, int)), f"PassState selected revision {key} is missing")
    require_sha256(selected_revision["revision_sha256"], "PassState selected revision SHA")
    require_sha256(selected_revision["revision_object_sha256"], "PassState selected revision object SHA")
    fixed_view, camera_set_sha256 = _pass_state_fixed_view(visual_identity)
    evidence_bundle = {
        "schema_version": "KnifeEvidenceBundle@1",
        "render_set_sha256": visual_identity["render_set_sha256"],
        "reference_comparison_sha256": visual_identity["reference_comparison_sha256"],
        "quality_report_sha256": visual_identity["quality_report_sha256"],
        "camera_set_sha256": camera_set_sha256,
    }
    main: dict[str, Any] = {
        "schema_version": "KnifePassState@1",
        "pass_id": pass_id,
        "parent_pass_id": parent_pass_id,
        "parent_pass_sha256": parent_pass_sha256,
        "project_id": source["project_id"],
        "stage": "camera-lock",
        "source_binding_id": source["source_binding_id"],
        "source_binding_sha256": source["source_binding_sha256"],
        "source_binding_object_sha256": source["source_binding_object_sha256"],
        "intent_bundle_id": source["intent_bundle_id"],
        "intent_bundle_sha256": source["intent_bundle_sha256"],
        "intent_bundle_object_sha256": source["intent_bundle_object_sha256"],
        "brief_id": source["brief_id"],
        "brief_sha256": source["brief_sha256"],
        "brief_object_sha256": source["brief_object_sha256"],
        "reference_id": reference["reference_id"],
        "reference_object_sha256": reference["object_sha256"],
        "reference_evidence_sha256": reference["canonical_sha256"],
        "source_candidate_id": source["source_candidate_id"],
        "source_candidate_state_sha256": source["source_candidate_state_sha256"],
        "baseline_candidate_id": source_truth["candidate_id"],
        "baseline_candidate_state_sha256": source_truth["candidate_state_sha256"],
        "baseline_artifact_sha256": source_truth["artifact_sha256"],
        "baseline_geometry_program_sha256": source_truth["program_sha256"],
        "baseline_geometry_program_object_sha256": source_truth["program_object_sha256"],
        "baseline_artifact_readback_object_sha256": source_truth["readback_object_sha256"],
        "baseline_representation_plan_sha256": source_truth["representation_plan_sha256"],
        "attempt_candidate_id": attempt_truth["candidate_id"],
        "attempt_candidate_state_sha256": attempt_truth["candidate_state_sha256"],
        "attempt_artifact_sha256": attempt_truth["artifact_sha256"],
        "attempt_geometry_program_sha256": attempt_truth["program_sha256"],
        "attempt_geometry_program_object_sha256": attempt_truth["program_object_sha256"],
        "attempt_artifact_readback_object_sha256": attempt_truth["readback_object_sha256"],
        "attempt_representation_plan_sha256": attempt_truth["representation_plan_sha256"],
        "authoring_mesh_id": selected_revision["mesh_id"],
        "authoring_mesh_lineage_id": selected_revision["lineage_id"],
        "authoring_mesh_revision_id": selected_revision["revision_id"],
        "authoring_mesh_revision_index": selected_revision["revision_index"],
        "authoring_mesh_revision_sha256": selected_revision["revision_sha256"],
        "authoring_mesh_revision_object_sha256": selected_revision["revision_object_sha256"],
        "authoring_mesh_identity_sha256": source["source_binding"]["authoring_mesh_identity_sha256"]
        if isinstance(source.get("source_binding"), dict)
        else source.get("authoring_mesh_identity_sha256"),
        "authoring_mesh_sha256": selected_revision["revision_sha256"],
        "modifier_graph_id": None,
        "modifier_graph_sha256": None,
        "evaluated_mesh_id": None,
        "evaluated_mesh_sha256": None,
        "high_artifact_id": None,
        "high_artifact_sha256": None,
        "fixed_view": fixed_view,
        "camera_set_sha256": camera_set_sha256,
        "render_set_id": visual_identity["render_set_id"],
        "render_set_sha256": visual_identity["render_set_sha256"],
        "render_set_object_sha256": visual_identity["render_set_object_sha256"],
        "reference_comparison_id": visual_identity["reference_comparison_id"],
        "reference_comparison_sha256": visual_identity["reference_comparison_sha256"],
        "reference_comparison_object_sha256": visual_identity["reference_comparison_object_sha256"],
        "quality_report_id": visual_identity["quality_report_id"],
        "quality_report_sha256": visual_identity["quality_report_sha256"],
        "quality_report_object_sha256": visual_identity["quality_report_object_sha256"],
        "evidence_bundle_sha256": object_sha256(evidence_bundle),
        "hard_gate_status": "PASS_SOURCE_STRUCTURAL",
        "visual_gate_status": visual_identity["quality_status"],
        "quality_status": visual_identity["quality_status"],
        "high_status": "NOT_RUN",
        "human_status": "NOT_RUN",
        "engine_status": "NOT_RUN",
        "unknowns": _pass_state_unknowns(successor_brief, intent),
        "unlocked_successor": "none",
        "high_mesh_created": False,
        "high_stage_unlocked": False,
        "production_stage_advanced": False,
        "candidate_confirmed": False,
        "version_created": False,
        "export_performed": False,
        "canonicalization_policy": "canonical-json-sha256-excluding-canonical-sha256@1",
        "canonical_sha256": "",
        "created_at": created_at,
    }
    main["canonical_sha256"] = canonical_hash(main, "canonical_sha256")
    # A child must retain the parent fixed camera exactly; Runtime performs
    # the authoritative comparison again, this check just prevents sending a
    # clearly inconsistent proposal over MCP.
    if parent_visual_identity is not None:
        parent_fixed_view, parent_camera_set_sha256 = _pass_state_fixed_view(
            parent_visual_identity
        )
        require(
            fixed_view == parent_fixed_view
            and camera_set_sha256 == parent_camera_set_sha256,
            "child PassState fixed view/camera set drifted from root",
        )
    return main


def pass_state_prepare_request(
    project_id: str, main: dict[str, Any], idempotency_key: str
) -> dict[str, Any]:
    value: dict[str, Any] = {
        "schema_version": "KnifePassStatePrepareRequest@1",
        "operation": "knife_pass_state_prepare",
        "project_id": project_id,
        "pass_state": main,
        "idempotency_key": idempotency_key,
        "max_response_bytes": MAX_RESPONSE_BYTES,
        "runtime_write_performed": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-input-sha256@1",
        "input_sha256": "",
    }
    value["input_sha256"] = canonical_hash(value, "input_sha256")
    return value


def pass_state_get_request(result: dict[str, Any]) -> dict[str, Any]:
    main = result.get("pass_state")
    require(isinstance(main, dict), "PassState result omitted Main for get request")
    value: dict[str, Any] = {
        "schema_version": "KnifePassStateGetRequest@1",
        "operation": "knife_pass_state_get",
        "project_id": main["project_id"],
        "pass_id": main["pass_id"],
        "pass_state_sha256": main["canonical_sha256"],
        "pass_state_object_sha256": result["pass_state_object_sha256"],
    }
    for field in PASS_STATE_GET_MAIN_FIELDS:
        value[field] = main[field]
    value["fixed_view_id"] = main["fixed_view"]["view_id"]
    value.update({
        "max_response_bytes": MAX_RESPONSE_BYTES,
        "runtime_write_performed": False,
        "persistent_user_data_touched": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-input-sha256@1",
        "input_sha256": "",
    })
    value["input_sha256"] = canonical_hash(value, "input_sha256")
    return value


def verify_pass_state_result(
    result: dict[str, Any],
    expected_main: dict[str, Any],
    label: str,
    *,
    expected_status: str,
    expected_request_kind: str,
    expected_idempotency_key: str | None,
) -> None:
    require(set(result) == PASS_STATE_RESULT_FIELDS, f"{label} result fields drifted")
    require(result.get("schema_version") == "KnifePassStateResult@1", f"{label} schema drifted")
    verify_canonical_object(result, "canonical_sha256", label)
    pass_state = result.get("pass_state")
    require(pass_state == expected_main, f"{label} Main readback drifted")
    require(
        result.get("pass_state_sha256") == expected_main.get("canonical_sha256"),
        f"{label} semantic hash drifted",
    )
    object_sha = require_sha256(result.get("pass_state_object_sha256"), f"{label} object hash")
    require(object_sha256(expected_main) == object_sha, f"{label} Main object hash drifted")
    require(result.get("project_id") == expected_main.get("project_id"), f"{label} project drifted")
    require(result.get("pass_id") == expected_main.get("pass_id"), f"{label} pass ID drifted")
    for key in (
        "source_binding_id", "source_binding_sha256", "source_binding_object_sha256",
        "intent_bundle_id", "intent_bundle_sha256", "intent_bundle_object_sha256",
        "brief_id", "brief_sha256", "brief_object_sha256", "reference_id",
        "reference_object_sha256", "reference_evidence_sha256", "source_candidate_id",
        "source_candidate_state_sha256", "baseline_candidate_id", "baseline_candidate_state_sha256",
        "baseline_artifact_sha256", "baseline_geometry_program_sha256",
        "baseline_geometry_program_object_sha256", "baseline_artifact_readback_object_sha256",
        "baseline_representation_plan_sha256", "attempt_candidate_id",
        "attempt_candidate_state_sha256", "attempt_artifact_sha256",
        "attempt_geometry_program_sha256", "attempt_geometry_program_object_sha256",
        "attempt_artifact_readback_object_sha256", "attempt_representation_plan_sha256",
        "authoring_mesh_id", "authoring_mesh_lineage_id", "authoring_mesh_revision_id",
        "authoring_mesh_revision_index", "authoring_mesh_revision_sha256",
        "authoring_mesh_revision_object_sha256", "authoring_mesh_identity_sha256",
        "authoring_mesh_sha256", "camera_set_sha256", "render_set_id", "render_set_sha256",
        "render_set_object_sha256", "reference_comparison_id", "reference_comparison_sha256",
        "reference_comparison_object_sha256", "quality_report_id", "quality_report_sha256",
        "quality_report_object_sha256", "evidence_bundle_sha256", "hard_gate_status",
        "visual_gate_status", "quality_status", "high_status", "human_status", "engine_status",
        "high_mesh_created", "high_stage_unlocked", "production_stage_advanced",
        "candidate_confirmed", "version_created", "export_performed",
    ):
        require(result.get(key) == expected_main.get(key), f"{label} {key} drifted")
    require(result.get("fixed_view_id") == expected_main["fixed_view"]["view_id"], f"{label} fixed view drifted")
    require(result.get("operation") in {"knife_pass_state_prepare", "knife_pass_state_get"}, f"{label} operation drifted")
    require(result.get("request_kind") == expected_request_kind, f"{label} request kind drifted")
    require(result.get("status") == expected_status, f"{label} status drifted")
    require(result.get("writer_policy") == "forgecad-runtime-only-state-writer@1", f"{label} writer policy drifted")
    require(result.get("canonicalization_policy") == "canonical-json-sha256-excluding-canonical-sha256@1", f"{label} canonical policy drifted")
    require(result.get("partial_result_exposed") is False, f"{label} exposed a partial result")
    if expected_status == "prepared":
        require(result.get("operation") == "knife_pass_state_prepare", f"{label} prepare operation drifted")
        require(result.get("idempotency_key") == expected_idempotency_key and isinstance(expected_idempotency_key, str), f"{label} committed idempotency key drifted")
        require(result.get("replayed") is False and result.get("store_effect") == "inserted" and result.get("cas_effect") == "inserted", f"{label} committed effects drifted")
        require(result.get("atomicity_status") == "committed" and result.get("store_commit_status") == "committed" and result.get("cas_commit_status") == "committed", f"{label} committed atomicity drifted")
        require(result.get("runtime_write_performed") is True and result.get("persistent_user_data_touched") is True, f"{label} committed write flags drifted")
    elif expected_status == "replayed":
        require(result.get("operation") == "knife_pass_state_prepare", f"{label} replay operation drifted")
        require(result.get("idempotency_key") is None and result.get("replayed") is True, f"{label} replay identity drifted")
        require(result.get("store_effect") == "not-touched" and result.get("cas_effect") == "not-touched", f"{label} replay effects drifted")
        require(result.get("atomicity_status") == "not-touched" and result.get("store_commit_status") == "not-touched" and result.get("cas_commit_status") == "not-touched", f"{label} replay atomicity drifted")
        require(result.get("runtime_write_performed") is False and result.get("persistent_user_data_touched") is False, f"{label} replay write flags drifted")
    else:
        require(expected_status == "found" and result.get("operation") == "knife_pass_state_get", f"{label} get operation drifted")
        require(result.get("idempotency_key") is None and result.get("replayed") is False, f"{label} get identity drifted")
        require(result.get("store_effect") == "not-touched" and result.get("cas_effect") == "not-touched", f"{label} get effects drifted")
        require(result.get("atomicity_status") == "not-touched" and result.get("store_commit_status") == "not-touched" and result.get("cas_commit_status") == "not-touched", f"{label} get atomicity drifted")
        require(result.get("runtime_write_performed") is False and result.get("persistent_user_data_touched") is False, f"{label} get write flags drifted")


def readback_visual_aovs(
    client: McpClient,
    render_set_hash: str,
    candidate_id: str,
    expected_aov_hashes: dict[str, str],
) -> dict[str, str]:
    """Read every persisted AOV exactly once; never rerun comparison."""
    readback: dict[str, str] = {}
    for pass_name in DRAGONFANG_VISUAL_AOV_PASSES:
        value = facade_tool(
            client,
            "quality_review",
            "render_pass_get",
            {"render_set_hash": render_set_hash, "pass": pass_name},
        )
        require(
            set(value)
            == {
                "schema_version",
                "render_set_hash",
                "candidate_id",
                "pass",
                "mime",
                "width",
                "height",
                "sha256",
                "png_base64",
            },
            f"AOV {pass_name} result field set drifted",
        )
        require(value.get("schema_version") == "RenderPassGet@1", f"AOV {pass_name} schema drifted")
        require(value.get("render_set_hash") == render_set_hash, f"AOV {pass_name} RenderSet binding drifted")
        require(value.get("candidate_id") == candidate_id, f"AOV {pass_name} candidate binding drifted")
        require(value.get("pass") == pass_name, f"AOV {pass_name} identity drifted")
        require(value.get("mime") == "image/png" and value.get("width") == 512 and value.get("height") == 512, f"AOV {pass_name} metadata drifted")
        actual_hash = require_sha256(value.get("sha256"), f"AOV {pass_name} sha256")
        require(actual_hash == expected_aov_hashes[pass_name], f"AOV {pass_name} hash drifted after restart")
        encoded = value.get("png_base64")
        require(isinstance(encoded, str), f"AOV {pass_name} omitted PNG bytes")
        try:
            decoded = base64.b64decode(encoded, validate=True)
        except (ValueError, binascii.Error) as error:
            raise GateFailure(f"AOV {pass_name} PNG payload is invalid") from error
        require(hashlib.sha256(decoded).hexdigest() == actual_hash, f"AOV {pass_name} PNG bytes/hash drifted")
        require(decoded.startswith(b"\x89PNG\r\n\x1a\n"), f"AOV {pass_name} is not PNG bytes")
        readback[pass_name] = actual_hash
    return readback


def _dragonfang_front_crop_pixels(reference_object_sha256: str) -> dict[str, int]:
    """Resolve the exact front ROI from immutable reference identity."""
    require(
        reference_object_sha256
        in {
            DRAGONFANG_FRONT_REFERENCE_OBJECT_SHA256,
            DRAGONFANG_GENERATED_MULTIVIEW_REFERENCE_OBJECT_SHA256,
        },
        "Dragonfang front crop requires an authorized reference object hash",
    )
    return dict(
        DRAGONFANG_GENERATED_MULTIVIEW_FRONT_CROP_PIXELS
        if reference_object_sha256
        == DRAGONFANG_GENERATED_MULTIVIEW_REFERENCE_OBJECT_SHA256
        else DRAGONFANG_FRONT_CROP_PIXELS
    )


def _dragonfang_front_normalized_crop(
    reference_object_sha256: str = DRAGONFANG_FRONT_REFERENCE_OBJECT_SHA256,
) -> dict[str, float]:
    """Return the hash-selected front ROI in ReferenceViewSpec coordinates."""
    crop = _dragonfang_front_crop_pixels(reference_object_sha256)
    return {
        "x": crop["x"] / DRAGONFANG_FRONT_REFERENCE_WIDTH,
        "y": crop["y"] / DRAGONFANG_FRONT_REFERENCE_HEIGHT,
        "width": crop["width"] / DRAGONFANG_FRONT_REFERENCE_WIDTH,
        "height": crop["height"] / DRAGONFANG_FRONT_REFERENCE_HEIGHT,
    }


def _normalize_full_image_contour(
    pixels: tuple[tuple[int, int], ...], width: int, height: int
) -> list[list[float]]:
    require(3 <= len(pixels) <= 256, "front contour proposal must contain 3..256 points")
    points: list[list[float]] = []
    for x, y in pixels:
        require(0 <= x <= width and 0 <= y <= height, "front contour pixel is outside the image")
        point = [x / width, y / height]
        require(
            0.0 <= point[0] <= 1.0 and 0.0 <= point[1] <= 1.0,
            "front contour is not full-image normalized",
        )
        points.append(point)
    return points


def dragonfang_front_reference_view_spec(
    reference_id: str,
    reference_object_sha256: str,
    width: int,
    height: int,
) -> dict[str, Any]:
    """Build the closed, hash-bound Dragonfang front ReferenceViewSpec.

    This helper intentionally accepts metadata only.  It does not open an image,
    import Pillow, or retain source bytes; the caller must provide the Runtime's
    already-verified ReferenceEvidence identity and dimensions.
    """
    require(
        isinstance(reference_id, str)
        and bool(re.fullmatch(r"[A-Za-z0-9_.-]{1,128}", reference_id)),
        "Dragonfang front reference_id is invalid",
    )
    require(
        reference_object_sha256
        in {
            DRAGONFANG_FRONT_REFERENCE_OBJECT_SHA256,
            DRAGONFANG_GENERATED_MULTIVIEW_REFERENCE_OBJECT_SHA256,
        },
        "Dragonfang visual helper requires an authorized Dragonfang reference object hash",
    )
    require(
        width == DRAGONFANG_FRONT_REFERENCE_WIDTH
        and height == DRAGONFANG_FRONT_REFERENCE_HEIGHT,
        "Dragonfang visual helper requires the authorized 1536x1024 reference dimensions",
    )
    crop_pixels = _dragonfang_front_crop_pixels(reference_object_sha256)
    crop = _dragonfang_front_normalized_crop(reference_object_sha256)
    for key, expected in crop_pixels.items():
        dimension = width if key in {"x", "width"} else height
        require(
            round(crop[key] * dimension) == expected,
            f"Dragonfang front crop drifted for {key}",
        )
    value: dict[str, Any] = {
        "schema_version": "ReferenceViewSpec@1",
        "reference_id": reference_id,
        "reference_sha256": reference_object_sha256,
        # Use the immutable primary-reference identity declared by the
        # ReferenceIntent quality contract.  A visually descriptive local ID
        # would make RenderSet evidence impossible to admit into PassState.
        "view_id": "view-front",
        "source_view": "front",
        "image": {
            "width": width,
            "height": height,
            "rotation_degrees": 0.0,
            "crop": crop,
        },
        "landmarks": [],
        "regions": [
            {
                "region_id": "dragonfang-front-panel",
                "x": crop["x"],
                "y": crop["y"],
                "width": crop["width"],
                "height": crop["height"],
                "visibility": "observed",
                "confidence": 1.0,
            }
        ],
        "canonical_sha256": "",
    }
    value["canonical_sha256"] = canonical_hash_without_field(
        value, "canonical_sha256"
    )
    return value


def dragonfang_fixed_reference_views(
    reference_id: str,
    reference_object_sha256: str,
    width: int,
    height: int,
) -> dict[str, dict[str, Any]]:
    """Return the fixed Dragonfang view set used by the visual director loop.

    Only ``view-front`` is backed by the supplied image and mask.  The two
    orbit specs intentionally use ``source_view=unknown`` and no regions: they
    are fixed-camera diagnostics, not invented reference images or likeness
    evidence.  ``view-fps-inspect`` remains declared by the intent contract but
    is not rendered by this probe.
    """
    front = dragonfang_front_reference_view_spec(
        reference_id, reference_object_sha256, width, height
    )
    full_image = {"x": 0.0, "y": 0.0, "width": 1.0, "height": 1.0}

    def orbit(view_id: str, source_view: str) -> dict[str, Any]:
        value: dict[str, Any] = {
            "schema_version": "ReferenceViewSpec@1",
            "reference_id": reference_id,
            "reference_sha256": reference_object_sha256,
            "view_id": view_id,
            # There is no user-supplied orbit image in this intake.  Keeping
            # this unknown prevents the renderer result from being interpreted
            # as a comparison against a non-existent reference view.
            "source_view": "unknown",
            "image": {
                "width": width,
                "height": height,
                "rotation_degrees": 0.0,
                "crop": full_image,
            },
            "landmarks": [],
            "regions": [],
            "canonical_sha256": "",
        }
        value["canonical_sha256"] = canonical_hash(value, "canonical_sha256")
        return value

    return {
        "view-front": front,
        "view-orbit-a": orbit("view-orbit-a", "rear-three-quarter"),
        "view-orbit-b": orbit("view-orbit-b", "front-three-quarter"),
        # Keep the declared fixed set visible to receipts while making its
        # non-execution explicit.  The runtime does not need a spec for this
        # unrendered view.
    }


def verify_dragonfang_intent_visual_contract(
    bundle: dict[str, Any], label: str = "Dragonfang intent bundle"
) -> dict[str, Any]:
    """Check the existing intent fixture is wired to the bounded visual loop.

    This is a structural preflight only.  It does not assert visual quality or
    convert inferred/unknown detail entries into observed evidence.
    """
    require(isinstance(bundle, dict), f"{label} is not an object")
    require(
        bundle.get("schema_version") == "KnifeReferenceIntentBundle@1",
        f"{label} schema drifted",
    )
    details = bundle.get("detail_inventory", {}).get("details")
    require(
        isinstance(details, list)
        and len(details) == DRAGONFANG_DETAIL_INVENTORY_COUNT,
        f"{label} must expose exactly 18 detail inventory entries",
    )
    detail_ids: set[str] = set()
    for detail in details:
        require(isinstance(detail, dict), f"{label} detail entry is not an object")
        detail_id = detail.get("detail_id")
        require(
            isinstance(detail_id, str) and detail_id and detail_id not in detail_ids,
            f"{label} detail inventory ids are not unique",
        )
        detail_ids.add(detail_id)
        require(
            isinstance(detail.get("target"), dict)
            and isinstance(detail["target"].get("target_kind"), str)
            and isinstance(detail["target"].get("target_id"), str),
            f"{label} detail target mapping is incomplete",
        )
        require(
            detail["target"].get("mapping_status")
            in {"mapped", "inferred", "unknown"},
            f"{label} detail target mapping status is outside the closed contract",
        )

    quality = bundle.get("quality_contract")
    require(isinstance(quality, dict), f"{label} quality contract is missing")
    fixed_views = quality.get("fixed_views")
    require(
        isinstance(fixed_views, list)
        and {view.get("view_id") for view in fixed_views if isinstance(view, dict)}
        == set(DRAGONFANG_FIXED_VIEW_IDS),
        f"{label} fixed view set drifted",
    )
    primary = [
        view
        for view in fixed_views
        if isinstance(view, dict) and view.get("view_id") == "view-front"
    ]
    require(
        len(primary) == 1
        and primary[0].get("comparison_role") == "primary-reference"
        and primary[0].get("reference_required") is True,
        f"{label} primary front view binding drifted",
    )
    orbit_views = [
        view
        for view in fixed_views
        if isinstance(view, dict)
        and view.get("comparison_role") == "orbit-nonreference"
    ]
    require(
        {view.get("view_id") for view in orbit_views} == set(DRAGONFANG_ORBIT_VIEW_IDS),
        f"{label} must declare two non-reference orbit views",
    )
    correction = quality.get("correction_policy")
    require(isinstance(correction, dict), f"{label} correction policy is missing")
    for key, expected in DRAGONFANG_CORRECTION_SCOPE_POLICY.items():
        require(
            correction.get(key) == expected,
            f"{label} correction policy {key} drifted",
        )
    return {
        "detail_inventory_count": len(details),
        "detail_ids": sorted(detail_ids),
        "fixed_view_ids": list(DRAGONFANG_FIXED_VIEW_IDS),
        "orbit_view_ids": list(DRAGONFANG_ORBIT_VIEW_IDS),
        "correction_policy": copy.deepcopy(DRAGONFANG_CORRECTION_SCOPE_POLICY),
        "hq_360_status": quality.get("hq_360_status"),
    }


def dragonfang_front_contour_proposal(
    reference_id: str,
    reference_object_sha256: str,
    width: int,
    height: int,
) -> dict[str, Any]:
    """Return an unconfirmed full-image normalized contour proposal."""
    # Reuse the exact identity/dimension gate from the ReferenceViewSpec helper.
    view_spec = dragonfang_front_reference_view_spec(
        reference_id, reference_object_sha256, width, height
    )
    value: dict[str, Any] = {
        "proposal_kind": "reference-contour",
        "reference_id": reference_id,
        "reference_object_sha256": reference_object_sha256,
        "view_id": view_spec["view_id"],
        "source_view": "front",
        "coordinate_space": "full-image-normalized",
        "crop_pixels": _dragonfang_front_crop_pixels(reference_object_sha256),
        "contour_points": _normalize_full_image_contour(
            DRAGONFANG_FRONT_PANEL_CONTOUR_PIXELS, width, height
        ),
        "contour_source": "Codex/algorithm-proposed",
        "proposal_status": "proposed",
        "user_confirmed": False,
        "canonicalization_policy": "canonical-json-sha256-excluding-canonical-sha256@1",
        "canonical_sha256": "",
    }
    value["canonical_sha256"] = canonical_hash_without_field(
        value, "canonical_sha256"
    )
    return value


def dragonfang_front_visual_pass_preview(
    reference_id: str,
    reference_object_sha256: str,
    width: int,
    height: int,
) -> dict[str, Any]:
    """Build only the future visual-pass inputs; never invoke Runtime or write a receipt."""
    view_spec = dragonfang_front_reference_view_spec(
        reference_id, reference_object_sha256, width, height
    )
    contour = dragonfang_front_contour_proposal(
        reference_id, reference_object_sha256, width, height
    )
    return {
        "status": "PREPARED_NOT_RUN",
        "reference_view_spec": view_spec,
        "reference_view_spec_sha256": view_spec["canonical_sha256"],
        "contour_proposal": contour,
        "contour_proposal_sha256": contour["canonical_sha256"],
    }


def self_check_dragonfang_front_visual_helpers() -> None:
    """Exercise pure helper invariants and fail-closed identity negatives."""
    preview = dragonfang_front_visual_pass_preview(
        "dragonfang-reference-self-check-001",
        DRAGONFANG_FRONT_REFERENCE_OBJECT_SHA256,
        DRAGONFANG_FRONT_REFERENCE_WIDTH,
        DRAGONFANG_FRONT_REFERENCE_HEIGHT,
    )
    spec = preview["reference_view_spec"]
    contour = preview["contour_proposal"]
    require(
        spec["schema_version"] == "ReferenceViewSpec@1"
        and spec["canonical_sha256"] == canonical_hash(spec, "canonical_sha256"),
        "Dragonfang front ReferenceViewSpec canonical hash drifted",
    )
    points = contour["contour_points"]
    require(
        isinstance(points, list)
        and 3 <= len(points) <= 256
        and all(0.0 <= point[0] <= 1.0 and 0.0 <= point[1] <= 1.0 for point in points),
        "Dragonfang front contour proposal is not bounded full-image normalized data",
    )
    require(
        contour["contour_source"] == "Codex/algorithm-proposed"
        and contour["proposal_status"] == "proposed"
        and contour["user_confirmed"] is False,
        "Dragonfang front contour proposal crossed its confirmation boundary",
    )
    for bad_hash, message in (
        ("0" * 64, "wrong reference hash was accepted"),
        (DRAGONFANG_FRONT_REFERENCE_OBJECT_SHA256[:-1] + "0", "hash drift was accepted"),
    ):
        try:
            dragonfang_front_reference_view_spec(
                "dragonfang-reference-self-check-001",
                bad_hash,
                DRAGONFANG_FRONT_REFERENCE_WIDTH,
                DRAGONFANG_FRONT_REFERENCE_HEIGHT,
            )
        except GateFailure:
            pass
        else:
            raise GateFailure(message)
    try:
        dragonfang_front_reference_view_spec(
            "dragonfang-reference-self-check-001",
            DRAGONFANG_FRONT_REFERENCE_OBJECT_SHA256,
            DRAGONFANG_FRONT_REFERENCE_WIDTH - 1,
            DRAGONFANG_FRONT_REFERENCE_HEIGHT,
        )
    except GateFailure:
        pass
    else:
        raise GateFailure("wrong reference dimensions were accepted")


def facade_tool(
    client: McpClient, facade: str, operation: str, request: dict[str, Any]
) -> dict[str, Any]:
    try:
        value = client.tool(facade, {"operation": operation, "request": request})
    except GateFailure as error:
        debug_dir = os.environ.get("WEAPONRY_PROBE_DEBUG_REQUEST_DIR")
        if debug_dir and operation in {"knife_pass_state_prepare", "knife_pass_state_get"}:
            destination = Path(debug_dir)
            destination.mkdir(parents=True, exist_ok=True)
            (destination / f"{operation}.json").write_bytes(canonical_bytes(request))
        raise GateFailure(f"{facade}.{operation}: {error}") from error
    require(isinstance(value, dict), f"{facade}.{operation} returned no typed object")
    return value


def initialize_client(binary: Path, environment: dict[str, str], timeout: float) -> McpClient:
    client = McpClient(binary, environment, max(timeout, 30.0))
    initialized = client.request(
        "initialize",
        {
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {"name": "weaponry-knife-brief-live-probe", "version": "1"},
        },
    )
    require(
        initialized.get("result", {}).get("protocolVersion") == MCP_PROTOCOL_VERSION,
        "MCP initialize failed",
    )
    client.notify("notifications/initialized")
    tools = client.request("tools/list").get("result", {}).get("tools")
    require(isinstance(tools, list) and len(tools) == 11, "default Knife façade count drifted")
    names = {tool.get("name") for tool in tools if isinstance(tool, dict)}
    require(
        {"weapon_preflight", "reference_intake"} <= names,
        "required Knife façades are unavailable",
    )
    preflight = facade_tool(
        client,
        "weapon_preflight",
        "skill_get",
        {"skill_id": "ponytail-preflight", "version": "0.1.0"},
    )
    require(
        preflight.get("skill", {}).get("skill_id") == "ponytail-preflight",
        "Ponytail preflight did not bind",
    )
    return client


def start_runtime(
    binary: Path,
    data_root: Path,
    environment: dict[str, str],
    timeout: float,
) -> tuple[subprocess.Popen[str], Path, dict[str, Any]]:
    ready_path = data_root / "ipc" / "ready.json"
    runtime = subprocess.Popen(
        [
            str(binary),
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
        env=environment,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
    )
    return runtime, ready_path, wait_for_ready(ready_path, runtime, timeout)


def mcp_environment(base: dict[str, str], ready: dict[str, Any]) -> dict[str, str]:
    socket_path = ready.get("socket_path")
    token = ready.get("token")
    require(
        isinstance(socket_path, str) and isinstance(token, str),
        "Runtime ready handoff lacked authenticated endpoint",
    )
    environment = base.copy()
    environment["FORGECAD_RUNTIME_SOCKET"] = socket_path
    environment["FORGECAD_RUNTIME_TOKEN"] = token
    environment["FORGECAD_MCP_ENABLE_MCP004_WRITES"] = "1"
    return environment


def bind_reference(
    template: dict[str, Any],
    project_id: str,
    reference: dict[str, Any],
    reference_profile: str = "legacy",
) -> dict[str, Any]:
    require(
        reference_profile in {"legacy", "generated-multiview"},
        "reference profile is outside the closed probe profiles",
    )
    source_sha256 = reference.get("object_sha256")
    width = reference.get("width")
    height = reference.get("height")
    require(
        isinstance(source_sha256, str)
        and SHA256.fullmatch(source_sha256) is not None
        and isinstance(width, int)
        and isinstance(height, int),
        "ReferenceEvidence was incomplete",
    )
    if reference_profile == "generated-multiview":
        require(
            source_sha256 == DRAGONFANG_GENERATED_MULTIVIEW_REFERENCE_OBJECT_SHA256
            and width == DRAGONFANG_FRONT_REFERENCE_WIDTH
            and height == DRAGONFANG_FRONT_REFERENCE_HEIGHT,
            "generated-multiview profile requires the authorized Dragonfang sheet",
        )
    value = _replace_reference_evidence_hash(
        copy.deepcopy(template), DRAGONFANG_FRONT_REFERENCE_OBJECT_SHA256, source_sha256
    )
    value["project_id"] = project_id
    value["authorization"]["source_reference_sha256"] = source_sha256
    value["reference_coverage"]["source_reference_sha256"] = source_sha256
    value["reference_coverage"]["source_dimensions"] = {
        "width": width,
        "height": height,
    }
    if reference_profile == "generated-multiview":
        coverage = value["reference_coverage"]
        coverage["required_views"] = [
            "front",
            "back",
            "left",
            "right",
            "front-three-quarter",
            "rear-three-quarter",
            "top",
            "bottom",
            "fps-hold",
            "fps-inspect",
        ]
        coverage["supplied_views"] = list(DRAGONFANG_GENERATED_MULTIVIEW_SUPPLIED_VIEWS)
        coverage["missing_views"] = list(DRAGONFANG_GENERATED_MULTIVIEW_MISSING_VIEWS)
        # guard-bottom and pommel are supplemental panels in the source sheet,
        # not members of the closed Brief view enum.  Keep them in the
        # orchestration receipt below rather than widening the product schema.
        coverage["detail_views"] = [
            "blade-detail",
            "guard-detail",
            "handle-detail",
        ]
        coverage["coverage_status"] = "partial"
        coverage["hq_360_status"] = "BLOCKED_REFERENCE_COVERAGE"
        coverage["camera_status"] = "inferred"
    value["canonical_sha256"] = canonical_hash(value, "canonical_sha256")
    return value


def bind_confirmed_successor_to_runtime(value: dict[str, Any]) -> None:
    authorization = value["authorization"]
    authorization["status"] = "user-confirmed"
    authorization["evidence_status"] = "runtime-bound"
    authorization["user_confirmation_required"] = False
    acceptance = value["acceptance_constraints"]
    for gate in acceptance["gate_statuses"]:
        if gate.get("gate_id") == "K0_AUTH_REFERENCE":
            gate["status"] = "pass"
    acceptance["blocking_reasons"] = [
        reason
        for reason in acceptance["blocking_reasons"]
        if reason != "authorization-not-runtime-bound"
    ]


def prepare_request(
    brief: dict[str, Any], reference: dict[str, Any], idempotency_key: str
) -> dict[str, Any]:
    value: dict[str, Any] = {
        "schema_version": "WeaponryKnifeProductionBriefPrepareRequest@1",
        "operation": "weaponry_knife_production_brief_prepare",
        "project_id": brief["project_id"],
        "brief": brief,
        "reference_id": reference["reference_id"],
        "reference_object_sha256": reference["object_sha256"],
        "reference_evidence_sha256": reference["canonical_sha256"],
        "idempotency_key": idempotency_key,
        "max_response_bytes": MAX_RESPONSE_BYTES,
        "runtime_write_performed": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-input-sha256@1",
        "input_sha256": "",
    }
    value["input_sha256"] = canonical_hash(value, "input_sha256")
    return value


def get_request(result: dict[str, Any]) -> dict[str, Any]:
    value: dict[str, Any] = {
        "schema_version": "WeaponryKnifeProductionBriefGetRequest@1",
        "operation": "weaponry_knife_production_brief_get",
        "project_id": result["project_id"],
        "reference_id": result["reference_id"],
        "reference_object_sha256": result["reference_object_sha256"],
        "reference_evidence_sha256": result["reference_evidence_sha256"],
        "brief_id": result["brief_id"],
        "brief_sha256": result["brief_sha256"],
        "brief_object_sha256": result["brief_object_sha256"],
        "max_response_bytes": MAX_RESPONSE_BYTES,
        "runtime_write_performed": False,
        "persistent_user_data_touched": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-input-sha256@1",
        "input_sha256": "",
    }
    value["input_sha256"] = canonical_hash(value, "input_sha256")
    return value


def bind_reference_intent(
    template: dict[str, Any],
    project_id: str,
    brief_result: dict[str, Any],
    reference: dict[str, Any],
    reference_profile: str = "legacy",
) -> dict[str, Any]:
    """Bind the checked-in intent semantics to this isolated Runtime lineage."""
    require(
        reference_profile in {"legacy", "generated-multiview"},
        "reference profile is outside the closed probe profiles",
    )
    value = copy.deepcopy(template)
    reference_id = reference["reference_id"]
    reference_object_sha256 = reference["object_sha256"]
    reference_evidence_sha256 = reference["canonical_sha256"]
    value["project_id"] = project_id
    value["brief_binding"].update(
        {
            "brief_id": brief_result["brief_id"],
            "brief_sha256": brief_result["brief_sha256"],
            "brief_object_sha256": brief_result["brief_object_sha256"],
            "authoring_eligibility": "ELIGIBLE",
            "authorization_binding_status": "runtime-bound",
        }
    )
    value["reference_binding"].update(
        {
            "reference_id": reference_id,
            "reference_object_sha256": reference_object_sha256,
            "reference_evidence_sha256": reference_evidence_sha256,
            "binding_status": "runtime-bound",
        }
    )
    for record in value["intake_manifest"]["records"]:
        record.update(
            {
                "reference_id": reference_id,
                "reference_object_sha256": reference_object_sha256,
                "reference_evidence_sha256": reference_evidence_sha256,
                "resolution": {
                    "width": reference["width"],
                    "height": reference["height"],
                },
            }
        )
        if reference_profile == "generated-multiview":
            # Keep only the supplied primary views as observed evidence.  The
            # guard-bottom and pommel panels are supplemental sheet panels;
            # they are not members of the closed view_kind enum and therefore
            # cannot be smuggled into the typed intake manifest.
            record["visible_coverage"] = [
                {"view": view, "status": "observed"}
                for view in DRAGONFANG_GENERATED_MULTIVIEW_SUPPLIED_VIEWS
            ]
    for detail in value["detail_inventory"]["details"]:
        for region in detail["evidence_regions"]:
            region["reference_id"] = reference_id
            if reference_profile == "generated-multiview":
                if region["view"] == "rear-three-quarter":
                    # Preserve the old detail's inferred status, but anchor
                    # its bounded observation to a supplied top/bottom panel.
                    # It must never remain an observed rear-three-quarter
                    # claim after switching to this sheet.
                    region["view"] = (
                        "top"
                        if detail["detail_id"] == "blade-thickness-taper"
                        else "bottom"
                    )
    for feature in value["quality_contract"]["critical_features"]:
        feature["evidence_region_ids"] = [
            f"{reference_id}:{region_id.rsplit(':', 1)[-1]}"
            for region_id in feature["evidence_region_ids"]
        ]
        if reference_profile == "generated-multiview":
            feature["evidence_region_ids"] = [
                (
                    f"{reference_id}:top"
                    if region_id.endswith(":rear-three-quarter")
                    and feature["feature_id"] == "feature-blade-section"
                    else (
                        f"{reference_id}:bottom"
                        if region_id.endswith(":rear-three-quarter")
                        else region_id
                    )
                )
                for region_id in feature["evidence_region_ids"]
            ]
    if reference_profile == "generated-multiview":
        unknowns = value["unknowns"]
        value["unknowns"] = [
            unknown
            for unknown in unknowns
            if unknown.get("view") not in {"top", "bottom"}
        ]
        value["unknowns"].append(
            {
                "unknown_id": "unknown-rear-three-quarter-view",
                "topic": "reference-view",
                "view": "rear-three-quarter",
                "description": "rear three-quarter view is not supplied by the generated multi-view sheet",
                "impact": "blocking",
                "resolution_status": "open",
            }
        )
    for key in ("intake_manifest", "detail_inventory", "quality_contract"):
        value[key]["canonical_sha256"] = canonical_hash(value[key], "canonical_sha256")
    value["canonical_sha256"] = canonical_hash(value, "canonical_sha256")
    return value


def intent_prepare_request(
    bundle: dict[str, Any],
    brief_result: dict[str, Any],
    reference: dict[str, Any],
    idempotency_key: str,
) -> dict[str, Any]:
    value: dict[str, Any] = {
        "schema_version": "KnifeReferenceIntentBundlePrepareRequest@1",
        "operation": "knife_reference_intent_bundle_prepare",
        "project_id": bundle["project_id"],
        "brief_id": brief_result["brief_id"],
        "brief_sha256": brief_result["brief_sha256"],
        "brief_object_sha256": brief_result["brief_object_sha256"],
        "reference_id": reference["reference_id"],
        "reference_object_sha256": reference["object_sha256"],
        "reference_evidence_sha256": reference["canonical_sha256"],
        "brief_authoring_eligibility": "ELIGIBLE",
        "intent_bundle": bundle,
        "idempotency_key": idempotency_key,
        "max_response_bytes": MAX_RESPONSE_BYTES,
        "runtime_write_performed": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-input-sha256@1",
        "input_sha256": "",
    }
    value["input_sha256"] = canonical_hash(value, "input_sha256")
    return value


def intent_get_request(result: dict[str, Any]) -> dict[str, Any]:
    value: dict[str, Any] = {
        "schema_version": "KnifeReferenceIntentBundleGetRequest@1",
        "operation": "knife_reference_intent_bundle_get",
        "project_id": result["project_id"],
        "brief_id": result["brief_id"],
        "brief_sha256": result["brief_sha256"],
        "brief_object_sha256": result["brief_object_sha256"],
        "reference_id": result["reference_id"],
        "reference_object_sha256": result["reference_object_sha256"],
        "reference_evidence_sha256": result["reference_evidence_sha256"],
        "brief_authoring_eligibility": "ELIGIBLE",
        "intent_bundle_id": result["intent_bundle_id"],
        "intent_bundle_sha256": result["intent_bundle_sha256"],
        "intent_bundle_object_sha256": result["intent_bundle_object_sha256"],
        "max_response_bytes": MAX_RESPONSE_BYTES,
        "runtime_write_performed": False,
        "persistent_user_data_touched": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-input-sha256@1",
        "input_sha256": "",
    }
    value["input_sha256"] = canonical_hash(value, "input_sha256")
    return value


def dragonfang_blockout_program(project_id: str, catalog_sha256: str) -> dict[str, Any]:
    """Return a bounded structural silhouette study, not a High asset."""

    def profile_node(
        node_id: str,
        profile: list[list[float]],
        depth_m: float,
        position_m: list[float] | None = None,
    ) -> dict[str, Any]:
        return {
            "node_id": node_id,
            "operator_id": "forgecad.geometry.profile-extrude@1",
            "inputs": [],
            "parameters": {
                "shape": "profile-extrude",
                "profile": profile,
                "depth_m": depth_m,
                "position_m": position_m or [0.0, 0.0, 0.0],
                "rotation_rad": [0.0, 0.0, 0.0],
            },
        }

    def sphere_node(node_id: str, position_m: list[float], radius_m: float) -> dict[str, Any]:
        return {
            "node_id": node_id,
            "operator_id": "forgecad.geometry.primitive@2",
            "inputs": [],
            "parameters": {
                "shape": "sphere",
                "radius_m": radius_m,
                "longitude_segments": 16,
                "latitude_segments": 8,
                "position_m": position_m,
                "rotation_rad": [0.0, 0.0, 0.0],
            },
        }

    # The profiles intentionally capture only the dominant game-view silhouette
    # and semantic part split. Relief, bevel language and material response are
    # deferred to the later High/Surface stages.
    nodes = [
        profile_node(
            "dragonfang-blade-body",
            [
                [-1.18, -0.10],
                [-0.92, -0.17],
                [-0.52, -0.24],
                [-0.08, -0.30],
                [0.42, -0.31],
                [0.92, -0.24],
                [1.38, -0.12],
                [1.80, 0.05],
                [2.02, 0.18],
                [1.72, 0.25],
                [1.28, 0.34],
                [0.78, 0.42],
                [0.24, 0.40],
                [-0.30, 0.31],
                [-0.78, 0.17],
                [-1.10, 0.05],
            ],
            0.060,
        ),
        profile_node(
            "dragonfang-cutting-edge",
            [
                [-1.08, -0.105],
                [-0.72, -0.175],
                [-0.25, -0.245],
                [0.28, -0.285],
                [0.82, -0.225],
                [1.34, -0.105],
                [1.80, 0.055],
                [1.94, 0.155],
                [1.72, 0.105],
                [1.28, -0.010],
                [0.78, -0.105],
                [0.24, -0.155],
                [-0.30, -0.135],
                [-0.78, -0.075],
            ],
            0.066,
        ),
        profile_node(
            "dragonfang-gold-spine",
            [
                [-0.98, 0.055],
                [-0.68, 0.16],
                [-0.26, 0.285],
                [0.24, 0.36],
                [0.76, 0.38],
                [1.25, 0.31],
                [1.66, 0.23],
                [1.52, 0.17],
                [1.13, 0.23],
                [0.70, 0.29],
                [0.22, 0.27],
                [-0.22, 0.21],
                [-0.62, 0.11],
            ],
            0.074,
        ),
        profile_node(
            "dragonfang-guard-head",
            [
                [-1.48, -0.23],
                [-1.22, -0.27],
                [-1.02, -0.14],
                [-0.96, 0.05],
                [-1.06, 0.25],
                [-1.28, 0.34],
                [-1.50, 0.22],
                [-1.38, 0.06],
                [-1.58, -0.04],
            ],
            0.115,
        ),
        profile_node(
            "dragonfang-grip",
            [
                [-2.46, -0.22],
                [-2.12, -0.27],
                [-1.72, -0.24],
                [-1.38, -0.15],
                [-1.34, 0.13],
                [-1.66, 0.20],
                [-2.05, 0.24],
                [-2.38, 0.16],
            ],
            0.105,
        ),
        profile_node(
            "dragonfang-pommel",
            [
                [-2.67, -0.14],
                [-2.46, -0.24],
                [-2.35, -0.08],
                [-2.41, 0.15],
                [-2.60, 0.27],
                [-2.56, 0.05],
            ],
            0.125,
        ),
        sphere_node("dragonfang-eye-left", [-1.22, 0.10, 0.067], 0.055),
        sphere_node("dragonfang-eye-right", [-1.22, 0.10, -0.067], 0.055),
        sphere_node("dragonfang-grip-gem", [-2.12, 0.02, 0.058], 0.052),
        sphere_node("dragonfang-fastener-a", [-1.72, 0.02, 0.058], 0.035),
        sphere_node("dragonfang-fastener-b", [-2.32, 0.00, 0.058], 0.035),
    ]
    part_outputs = [
        {
            "part_id": "blade-body",
            "input_node_ids": ["dragonfang-blade-body"],
            "material_zone_id": "dark-red-blade",
            "solid": True,
        },
        {
            "part_id": "cutting-edge",
            "input_node_ids": ["dragonfang-cutting-edge"],
            "material_zone_id": "silver-cutting-edge",
            "solid": True,
        },
        {
            "part_id": "dragon-relief",
            "input_node_ids": ["dragonfang-gold-spine"],
            "material_zone_id": "antique-gold-ornament",
            "solid": True,
        },
        {
            "part_id": "guard-dragon-head",
            "input_node_ids": ["dragonfang-guard-head"],
            "material_zone_id": "antique-gold-ornament",
            "solid": True,
        },
        {
            "part_id": "grip",
            "input_node_ids": ["dragonfang-grip"],
            "material_zone_id": "black-grip",
            "solid": True,
        },
        {
            "part_id": "pommel",
            "input_node_ids": ["dragonfang-pommel"],
            "material_zone_id": "antique-gold-ornament",
            "solid": True,
        },
        {
            "part_id": "dragon-eye-left",
            "input_node_ids": ["dragonfang-eye-left"],
            "material_zone_id": "ruby-gem",
            "solid": True,
        },
        {
            "part_id": "dragon-eye-right",
            "input_node_ids": ["dragonfang-eye-right"],
            "material_zone_id": "ruby-gem",
            "solid": True,
        },
        {
            "part_id": "gem",
            "input_node_ids": ["dragonfang-grip-gem"],
            "material_zone_id": "ruby-gem",
            "solid": True,
        },
        {
            "part_id": "grip-fastener",
            "input_node_ids": ["dragonfang-fastener-a", "dragonfang-fastener-b"],
            "material_zone_id": "antique-gold-ornament",
            "solid": True,
        },
    ]
    return {
        "schema_version": "GeometryProgram@2",
        "project_id": project_id,
        "representation_plan_sha256": object_sha256(
            {
                "schema_version": "DragonfangStructuralRepresentationPlan@1",
                "intent": "dominant-kukri-silhouette-and-semantic-part-split",
                "quality_status": "structural_only",
            }
        ),
        "operator_catalog_sha256": catalog_sha256,
        "units": {
            "length": "meter",
            "angle": "radian",
            "coordinate_system": "right-handed-y-up",
        },
        "budgets": {
            "max_nodes": 32,
            "max_triangles": 25000,
            "max_glb_bytes": 67108864,
            "max_worker_memory_bytes": 536870912,
            "max_runtime_ms": 10000,
        },
        "nodes": nodes,
        "part_outputs": part_outputs,
    }


# This payload is an orchestration boundary for the next High attempt.  It is
# deliberately kept in the probe (rather than Runtime/Store/MCP) until the
# High Artifact render selector is live.  The selector must bind one immutable
# baseline; this builder then derives a new GeometryProgram proposal without
# mutating that baseline.  It is not a GeometryProgram contract and it does
# not itself create a candidate.  The current Runtime SourceBinding route is
# single-Part (blade-body only), so a two-Part child remains explicitly
# blocked until a Runtime-owned multi-Part materializer exists.
DRAGONFANG_SILHOUETTE_CORRECTION_SCHEMA_VERSION = (
    "DragonfangSilhouetteCorrectionPayload@1"
)
DRAGONFANG_SILHOUETTE_SELECTOR_SCHEMA_VERSION = "DragonfangHighArtifactSelector@1"
DRAGONFANG_SILHOUETTE_CORRECTION_PART_IDS = ("blade-body", "cutting-edge")
DRAGONFANG_SILHOUETTE_CORRECTION_MAX_PARTS = 2
DRAGONFANG_SILHOUETTE_CORRECTION_FORBIDDEN_PART_IDS = (
    "dragon-relief",
    "guard-dragon-head",
    "grip",
    "pommel",
    "dragon-eye-left",
    "dragon-eye-right",
    "gem",
    "grip-fastener",
)
DRAGONFANG_SILHOUETTE_CORRECTION_VIEW_IDS = (
    "front",
    "back",
    "left",
    "right",
    "top",
    "bottom",
)

# Round two follows the fixed RH/Y-up front capture.  The first millimetre-scale
# attempt was visually indistinguishable on a roughly 3m blade.  These bounded
# values form a deliberate macro silhouette edit: arch the centreline, deepen
# the forward belly, extend/narrow the tip, and lower the apex.  They are
# applied identically to the two contour-bearing Parts so the silver edge does
# not detach from the red blade.  Every other Part remains hash-frozen.
DRAGONFANG_SILHOUETTE_CORRECTION_DEFAULTS = {
    "bend_centerline_delta_m": 0.110,
    "belly_depth_delta_m": -0.260,
    "tip_extension_delta_m": 0.240,
    "tip_lift_delta_m": -0.120,
}
DRAGONFANG_SILHOUETTE_CORRECTION_LIMITS = {
    "bend_centerline_delta_m": (-0.180, 0.180),
    "belly_depth_delta_m": (-0.320, 0.320),
    "tip_extension_delta_m": (-0.300, 0.300),
    "tip_lift_delta_m": (-0.180, 0.180),
}
DRAGONFANG_SILHOUETTE_CORRECTION_ALLOWED_KEYS = frozenset(
    DRAGONFANG_SILHOUETTE_CORRECTION_DEFAULTS
)
DRAGONFANG_SILHOUETTE_CORRECTION_STATUS = (
    "PROPOSAL_READY_MATERIALIZATION_BLOCKED"
)
DRAGONFANG_SILHOUETTE_MATERIALIZATION_STATUS = (
    "BLOCKED_CURRENT_SINGLE_PART_SOURCE_BINDING"
)
DRAGONFANG_SILHOUETTE_CURRENT_MATERIALIZER = (
    "AuthoringMeshV2CandidateMaterializeRequest@1:single-source-binding-part-replacement"
)
DRAGONFANG_SILHOUETTE_REQUIRED_MATERIALIZER = (
    "runtime-owned-multi-part-child-candidate@1"
)
DRAGONFANG_SILHOUETTE_PROFILE_LAYOUT = {
    "dragonfang-blade-body": {
        "point_count": 16,
        "belly_indices": tuple(range(0, 8)),
        "tip_indices": (7, 8, 9),
        "spine_indices": tuple(range(9, 16)),
    },
    "dragonfang-cutting-edge": {
        "point_count": 14,
        "belly_indices": tuple(range(0, 8)),
        "tip_indices": (7, 8, 9),
        "spine_indices": tuple(range(9, 14)),
    },
}


def _finite_correction_number(value: Any, name: str) -> float:
    require(
        isinstance(value, (int, float))
        and not isinstance(value, bool)
        and value == value,
        f"Dragonfang correction {name} is not finite",
    )
    result = float(value)
    require(abs(result) < float("inf"), f"Dragonfang correction {name} is not finite")
    return result


def _profile_node_by_part(
    program: dict[str, Any], part_id: str
) -> tuple[dict[str, Any], dict[str, Any]]:
    outputs = program.get("part_outputs")
    nodes = program.get("nodes")
    require(isinstance(outputs, list) and isinstance(nodes, list), "Dragonfang baseline program is missing nodes/part_outputs")
    matches = [
        output
        for output in outputs
        if isinstance(output, dict) and output.get("part_id") == part_id
    ]
    require(len(matches) == 1, f"Dragonfang baseline must contain exactly one {part_id} Part output")
    output = matches[0]
    input_node_ids = output.get("input_node_ids")
    require(
        isinstance(input_node_ids, list) and len(input_node_ids) == 1
        and isinstance(input_node_ids[0], str),
        f"Dragonfang {part_id} output must have one profile node",
    )
    node_matches = [
        node
        for node in nodes
        if isinstance(node, dict) and node.get("node_id") == input_node_ids[0]
    ]
    require(len(node_matches) == 1, f"Dragonfang {part_id} profile node is missing or duplicated")
    node = node_matches[0]
    require(
        node.get("operator_id") == "forgecad.geometry.profile-extrude@1",
        f"Dragonfang {part_id} is not a bounded profile-extrude node",
    )
    parameters = node.get("parameters")
    profile = parameters.get("profile") if isinstance(parameters, dict) else None
    layout = DRAGONFANG_SILHOUETTE_PROFILE_LAYOUT.get(node.get("node_id"))
    require(isinstance(layout, dict), f"Dragonfang {part_id} profile layout is unknown")
    require(
        isinstance(profile, list) and len(profile) == layout["point_count"],
        f"Dragonfang {part_id} profile point count drifted",
    )
    for index, point in enumerate(profile):
        require(
            isinstance(point, list)
            and len(point) == 2
            and all(
                isinstance(value, (int, float))
                and not isinstance(value, bool)
                and value == value
                and abs(float(value)) <= 10.0
                for value in point
            ),
            f"Dragonfang {part_id} profile point {index} is invalid",
        )
    return output, node


def _ready_dragonfang_selector(
    selector: dict[str, Any], baseline_program: dict[str, Any]
) -> dict[str, Any]:
    require(isinstance(selector, dict), "Dragonfang High Artifact selector must be an object")
    require(
        selector.get("schema_version") == DRAGONFANG_SILHOUETTE_SELECTOR_SCHEMA_VERSION,
        "Dragonfang High Artifact selector schema drifted",
    )
    require(
        selector.get("status") in {"READY", "ready", "READY_FOR_CORRECTION"},
        "Dragonfang High Artifact selector is not ready",
    )
    for field in (
        "selector_id",
        "baseline_candidate_id",
        "baseline_candidate_state_sha256",
        "baseline_geometry_program_sha256",
        "baseline_artifact_sha256",
        "baseline_artifact_readback_sha256",
        "reference_id",
        "reference_object_sha256",
        "fixed_view_ids",
        "selected_part_ids",
    ):
        require(field in selector, f"Dragonfang selector omitted {field}")
    for field in (
        "baseline_candidate_state_sha256",
        "baseline_geometry_program_sha256",
        "baseline_artifact_sha256",
        "baseline_artifact_readback_sha256",
        "reference_object_sha256",
    ):
        require_sha256(selector[field], f"Dragonfang selector {field}")
    require(
        isinstance(selector["selector_id"], str)
        and 1 <= len(selector["selector_id"]) <= 128
        and bool(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*", selector["selector_id"])),
        "Dragonfang selector_id is invalid",
    )
    require(
        isinstance(selector["baseline_candidate_id"], str)
        and selector["baseline_candidate_id"],
        "Dragonfang baseline candidate ID is invalid",
    )
    fixed_views = selector["fixed_view_ids"]
    require(
        isinstance(fixed_views, list)
        and fixed_views == list(DRAGONFANG_SILHOUETTE_CORRECTION_VIEW_IDS),
        "Dragonfang selector must lock front/back/left/right/top/bottom views in order",
    )
    selected_parts = selector["selected_part_ids"]
    require(
        isinstance(selected_parts, list)
        and selected_parts == list(DRAGONFANG_SILHOUETTE_CORRECTION_PART_IDS),
        "Dragonfang selector scope must be exactly blade-body + cutting-edge",
    )
    declared_program_sha = baseline_program.get("canonical_sha256")
    baseline_program_sha = (
        declared_program_sha
        if isinstance(declared_program_sha, str) and SHA256.fullmatch(declared_program_sha)
        else geometry_program_semantic_sha256(baseline_program)
    )
    require(
        selector["baseline_geometry_program_sha256"] == baseline_program_sha,
        "Dragonfang selector baseline GeometryProgram hash drifted",
    )
    return selector


def _correction_profile(
    node_id: str,
    profile: list[list[float]],
    parameters: dict[str, float],
) -> tuple[list[list[float]], list[dict[str, Any]]]:
    layout = DRAGONFANG_SILHOUETTE_PROFILE_LAYOUT[node_id]
    corrected = [[float(point[0]), float(point[1])] for point in profile]
    root_x = float(profile[0][0])
    tip_index = max(range(len(profile)), key=lambda index: float(profile[index][0]))
    tip_x = float(profile[tip_index][0])
    span = tip_x - root_x
    require(span > 0.25, f"Dragonfang {node_id} profile span is too small for correction")
    changes: list[dict[str, Any]] = []

    def apply(index: int, dx: float, dy: float, reason: str) -> None:
        before = corrected[index][:]
        corrected[index][0] += dx
        corrected[index][1] += dy
        require(
            all(abs(value) <= 10.0 for value in corrected[index]),
            f"Dragonfang {node_id} correction point {index} exceeds bounds",
        )
        if before != corrected[index]:
            changes.append(
                {
                    "index": index,
                    "before": before,
                    "after": corrected[index][:],
                    "delta_m": [dx, dy],
                    "reason": reason,
                }
            )

    belly_delta = parameters["belly_depth_delta_m"]
    bend_delta = parameters["bend_centerline_delta_m"]
    for index in sorted(set(layout["belly_indices"])):
        t = max(0.0, min(1.0, (float(profile[index][0]) - root_x) / span))
        # The forward belly receives the strongest correction around t=.72;
        # the root and the tip remain attached to their existing junctions.
        belly_weight = max(0.0, 1.0 - abs(t - 0.72) / 0.50)
        bend_weight = max(0.0, min(1.0, 4.0 * t * (1.0 - t)))
        apply(
            index,
            0.0,
            bend_delta * bend_weight + belly_delta * belly_weight,
            "centerline bend + forward belly depth",
        )
    for index in sorted(set(layout["spine_indices"])):
        t = max(0.0, min(1.0, (float(profile[index][0]) - root_x) / span))
        bend_weight = max(0.0, min(1.0, 4.0 * t * (1.0 - t)))
        apply(index, 0.0, bend_delta * bend_weight, "centerline bend on spine")

    tip_extension = parameters["tip_extension_delta_m"]
    tip_lift = parameters["tip_lift_delta_m"]
    tip_neighbours = (
        ((tip_index - 1) % len(profile), 0.45),
        (tip_index, 1.0),
        ((tip_index + 1) % len(profile), 0.45),
    )
    for index, weight in tip_neighbours:
        apply(
            index,
            tip_extension * weight,
            tip_lift * weight,
            "tip extension and taper transition",
        )
    return corrected, changes


def build_dragonfang_silhouette_correction_payload(
    baseline_program: dict[str, Any],
    selector: dict[str, Any],
    *,
    correction_parameters: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Build one bounded, two-Part correction from a ready High selector.

    The function is intentionally pure: it never mutates ``baseline_program``
    or ``selector``, never writes Runtime/CAS, and never calls MCP.  Its output
    is suitable for a later immutable child-candidate prepare once the live
    selector/materializer route accepts this exact payload.
    """
    require(isinstance(baseline_program, dict), "Dragonfang baseline GeometryProgram must be an object")
    require(
        baseline_program.get("schema_version") == "GeometryProgram@2",
        "Dragonfang baseline GeometryProgram schema drifted",
    )
    selector = _ready_dragonfang_selector(selector, baseline_program)
    declared_program_sha = baseline_program.get("canonical_sha256")
    baseline_program_sha256 = (
        declared_program_sha
        if isinstance(declared_program_sha, str) and SHA256.fullmatch(declared_program_sha)
        else geometry_program_semantic_sha256(baseline_program)
    )
    baseline_program_object_sha256 = object_sha256(baseline_program)
    provided = correction_parameters or {}
    require(isinstance(provided, dict), "Dragonfang correction parameters must be an object")
    unknown = set(provided) - DRAGONFANG_SILHOUETTE_CORRECTION_ALLOWED_KEYS
    require(not unknown, f"Dragonfang correction contains unknown parameters: {sorted(unknown)}")
    parameters: dict[str, float] = {}
    for name, default in DRAGONFANG_SILHOUETTE_CORRECTION_DEFAULTS.items():
        value = _finite_correction_number(provided.get(name, default), name)
        lower, upper = DRAGONFANG_SILHOUETTE_CORRECTION_LIMITS[name]
        require(lower <= value <= upper, f"Dragonfang correction {name} is outside its bounded range")
        parameters[name] = value
    require(
        any(abs(value) > 0.0 for value in parameters.values()),
        "Dragonfang correction must change at least one bounded parameter",
    )

    attempt_program = copy.deepcopy(baseline_program)
    node_changes: list[dict[str, Any]] = []
    target_node_ids: list[str] = []
    for part_id in DRAGONFANG_SILHOUETTE_CORRECTION_PART_IDS:
        _output, baseline_node = _profile_node_by_part(baseline_program, part_id)
        _attempt_output, attempt_node = _profile_node_by_part(attempt_program, part_id)
        target_node_ids.append(attempt_node["node_id"])
        baseline_parameters = baseline_node["parameters"]
        attempt_parameters = attempt_node["parameters"]
        require(
            isinstance(baseline_parameters, dict)
            and isinstance(attempt_parameters, dict),
            f"Dragonfang {part_id} profile parameters are missing",
        )
        corrected_profile, changes = _correction_profile(
            attempt_node["node_id"], baseline_parameters["profile"], parameters
        )
        attempt_parameters["profile"] = corrected_profile
        node_changes.append(
            {
                "part_id": part_id,
                "node_id": attempt_node["node_id"],
                "baseline_node_sha256": object_sha256(baseline_node),
                "attempt_node_sha256": object_sha256(attempt_node),
                "changed_profile_indices": sorted({change["index"] for change in changes}),
                "profile_changes": changes,
            }
        )
        require(
            object_sha256(baseline_node) != object_sha256(attempt_node),
            f"Dragonfang {part_id} correction did not change its profile node",
        )

    # The representation plan is re-derived from the corrected program.  It
    # intentionally preserves the baseline's semantic Part/node graph and all
    # non-target nodes; Runtime will later establish the canonical candidate
    # and artifact identities from its own hashes.
    attempt_program["representation_plan_sha256"] = object_sha256(
        {
            "schema_version": "DragonfangSilhouetteRepresentationPlan@1",
            "baseline_program_sha256": baseline_program_sha256,
            "scope": list(DRAGONFANG_SILHOUETTE_CORRECTION_PART_IDS),
            "view_ids": list(DRAGONFANG_SILHOUETTE_CORRECTION_VIEW_IDS),
            "parameters": parameters,
        }
    )
    attempt_program.pop("canonical_sha256", None)
    attempt_program_sha256 = geometry_program_semantic_sha256(attempt_program)
    attempt_program_object_sha256 = object_sha256(attempt_program)

    baseline_nodes = {
        node.get("node_id"): object_sha256(node)
        for node in baseline_program.get("nodes", [])
        if isinstance(node, dict) and isinstance(node.get("node_id"), str)
    }
    attempt_nodes = {
        node.get("node_id"): object_sha256(node)
        for node in attempt_program.get("nodes", [])
        if isinstance(node, dict) and isinstance(node.get("node_id"), str)
    }
    baseline_parts = {
        output.get("part_id"): object_sha256(output)
        for output in baseline_program.get("part_outputs", [])
        if isinstance(output, dict) and isinstance(output.get("part_id"), str)
    }
    attempt_parts = {
        output.get("part_id"): object_sha256(output)
        for output in attempt_program.get("part_outputs", [])
        if isinstance(output, dict) and isinstance(output.get("part_id"), str)
    }
    preserved_part_ids = sorted(
        part_id
        for part_id in baseline_parts
        if part_id not in DRAGONFANG_SILHOUETTE_CORRECTION_PART_IDS
    )
    require(
        len(preserved_part_ids) == 8,
        f"Dragonfang expected eight preserved Parts, found {len(preserved_part_ids)}",
    )
    preserved_node_ids = sorted(
        node_id
        for node_id in baseline_nodes
        if node_id not in target_node_ids
    )
    require(
        baseline_nodes.keys() == attempt_nodes.keys(),
        "Dragonfang correction changed the GeometryProgram node identity set",
    )
    require(
        baseline_parts.keys() == attempt_parts.keys(),
        "Dragonfang correction changed the GeometryProgram Part identity set",
    )
    require(
        all(baseline_nodes[node_id] == attempt_nodes[node_id] for node_id in preserved_node_ids),
        "Dragonfang correction changed a preserved node",
    )
    require(
        all(baseline_parts[part_id] == attempt_parts[part_id] for part_id in preserved_part_ids),
        "Dragonfang correction changed a preserved Part output",
    )

    correction_identity = {
        "schema_version": DRAGONFANG_SILHOUETTE_CORRECTION_SCHEMA_VERSION,
        "selector_id": selector["selector_id"],
        "baseline_candidate_id": selector["baseline_candidate_id"],
        "baseline_geometry_program_sha256": baseline_program_sha256,
        "attempt_geometry_program_sha256": attempt_program_sha256,
        "scope": list(DRAGONFANG_SILHOUETTE_CORRECTION_PART_IDS),
        "parameters": parameters,
    }
    correction_id = "dragonfang-silhouette-" + object_sha256(correction_identity)[:24]
    return {
        "schema_version": DRAGONFANG_SILHOUETTE_CORRECTION_SCHEMA_VERSION,
        "correction_id": correction_id,
        "status": DRAGONFANG_SILHOUETTE_CORRECTION_STATUS,
        "selector": {
            "selector_id": selector["selector_id"],
            "baseline_candidate_id": selector["baseline_candidate_id"],
            "baseline_candidate_state_sha256": selector["baseline_candidate_state_sha256"],
            "baseline_geometry_program_sha256": selector["baseline_geometry_program_sha256"],
            "baseline_artifact_sha256": selector["baseline_artifact_sha256"],
            "baseline_artifact_readback_sha256": selector["baseline_artifact_readback_sha256"],
            "reference_id": selector["reference_id"],
            "reference_object_sha256": selector["reference_object_sha256"],
            "fixed_view_ids": list(selector["fixed_view_ids"]),
            "selected_part_ids": list(selector["selected_part_ids"]),
        },
        "baseline": {
            "candidate_id": selector["baseline_candidate_id"],
            "candidate_state_sha256": selector["baseline_candidate_state_sha256"],
            "geometry_program_sha256": baseline_program_sha256,
            "geometry_program_object_sha256": baseline_program_object_sha256,
            "artifact_sha256": selector["baseline_artifact_sha256"],
            "artifact_readback_sha256": selector["baseline_artifact_readback_sha256"],
        },
        "attempt": {
            "parent_candidate_id": selector["baseline_candidate_id"],
            "geometry_program_sha256": attempt_program_sha256,
            "geometry_program_object_sha256": attempt_program_object_sha256,
            "representation_plan_sha256": attempt_program["representation_plan_sha256"],
            "candidate_status": "NOT_MATERIALIZED",
            "candidate_id": None,
            "candidate_state_sha256": None,
            "artifact_sha256": None,
            "artifact_readback_sha256": None,
        },
        "scope": {
            "changed_part_ids": list(DRAGONFANG_SILHOUETTE_CORRECTION_PART_IDS),
            "changed_node_ids": target_node_ids,
            "preserved_part_ids": preserved_part_ids,
            "preserved_node_ids": preserved_node_ids,
            "preserved_node_hashes": {
                node_id: baseline_nodes[node_id] for node_id in preserved_node_ids
            },
            "preserved_part_output_hashes": {
                part_id: baseline_parts[part_id] for part_id in preserved_part_ids
            },
            "preserved_part_count": len(preserved_part_ids),
            "max_changed_parts": DRAGONFANG_SILHOUETTE_CORRECTION_MAX_PARTS,
            "forbidden_part_ids": list(
                DRAGONFANG_SILHOUETTE_CORRECTION_FORBIDDEN_PART_IDS
            ),
        },
        "materialization_gate": {
            "status": DRAGONFANG_SILHOUETTE_MATERIALIZATION_STATUS,
            "current_route": DRAGONFANG_SILHOUETTE_CURRENT_MATERIALIZER,
            "current_source_binding_part_ids": ["blade-body"],
            "required_route": DRAGONFANG_SILHOUETTE_REQUIRED_MATERIALIZER,
            "required_part_ids": list(DRAGONFANG_SILHOUETTE_CORRECTION_PART_IDS),
            "candidate_created": False,
            "runtime_write_performed": False,
        },
        "parameters_m": parameters,
        "node_changes": node_changes,
        "geometry_program": attempt_program,
        "review_contract": {
            "comparison_style": "img2threejs-same-renderer-reference-vs-attempt",
            "fixed_view_ids": list(DRAGONFANG_SILHOUETTE_CORRECTION_VIEW_IDS),
            "metric_priority": [
                "boundary_f1_4px",
                "silhouette_iou",
                "bbox_edge_error",
                "centroid_error",
            ],
            "status": "NOT_RUN_UNTIL_RUNTIME_MULTI_PART_MATERIALIZES_CHILD",
            "visual_quality_promotion": "NOT_PROMOTED",
        },
        "lineage": {
            "parent_candidate_id": selector["baseline_candidate_id"],
            "parent_geometry_program_sha256": baseline_program_sha256,
            "child_candidate_created_by": "forgecad-runtime-only-state-writer@1",
            "immutable_parent_preserved": True,
        },
    }


def bind_dragonfang_correction_runtime_hash(
    payload: dict[str, Any], runtime_program_sha256: str
) -> dict[str, Any]:
    """Bind a pure correction draft to the Runtime/Worker canonical identity.

    GeometryProgram numeric canonicalization is owned by the Rust Worker.  The
    Python loop may propose numbers, but it must not invent the persisted
    semantic hash.  This function is called only after geometry_program_hash
    has accepted the exact draft.
    """
    runtime_program_sha256 = require_sha256(
        runtime_program_sha256, "Dragonfang Runtime GeometryProgram hash"
    )
    bound = copy.deepcopy(payload)
    program = bound.get("geometry_program")
    attempt = bound.get("attempt")
    selector = bound.get("selector")
    require(
        isinstance(program, dict)
        and isinstance(attempt, dict)
        and isinstance(selector, dict),
        "Dragonfang correction payload cannot bind Runtime hash",
    )
    require(
        "canonical_sha256" not in program,
        "Dragonfang correction draft already contains canonical_sha256",
    )
    program["canonical_sha256"] = runtime_program_sha256
    attempt["geometry_program_sha256"] = runtime_program_sha256
    attempt["geometry_program_object_sha256"] = object_sha256(program)
    correction_identity = {
        "schema_version": DRAGONFANG_SILHOUETTE_CORRECTION_SCHEMA_VERSION,
        "selector_id": selector["selector_id"],
        "baseline_candidate_id": selector["baseline_candidate_id"],
        "baseline_geometry_program_sha256": selector["baseline_geometry_program_sha256"],
        "attempt_geometry_program_sha256": runtime_program_sha256,
        "scope": list(DRAGONFANG_SILHOUETTE_CORRECTION_PART_IDS),
        "parameters": bound["parameters_m"],
    }
    bound["correction_id"] = (
        "dragonfang-silhouette-" + object_sha256(correction_identity)[:24]
    )
    return bound


def validate_dragonfang_silhouette_correction_payload(
    payload: dict[str, Any],
) -> None:
    """Run the probe-local closed-field and preservation checks.

    This validator is deliberately independent of Runtime and therefore only
    proves that a selector-bound proposal is safe to hand to a future child
    materializer.  It does not score a render or claim visual likeness.
    """
    require(isinstance(payload, dict), "Dragonfang correction payload must be an object")
    require(
        set(payload)
        == {
            "schema_version",
            "correction_id",
            "status",
            "selector",
            "baseline",
            "attempt",
            "scope",
            "materialization_gate",
            "parameters_m",
            "node_changes",
            "geometry_program",
            "review_contract",
            "lineage",
        },
        "Dragonfang correction payload fields are not closed",
    )
    require(
        payload["schema_version"] == DRAGONFANG_SILHOUETTE_CORRECTION_SCHEMA_VERSION
        and payload["status"] == DRAGONFANG_SILHOUETTE_CORRECTION_STATUS,
        "Dragonfang correction payload status/schema drifted",
    )
    selector = payload["selector"]
    require(
        isinstance(selector, dict)
        and set(selector)
        == {
            "selector_id",
            "baseline_candidate_id",
            "baseline_candidate_state_sha256",
            "baseline_geometry_program_sha256",
            "baseline_artifact_sha256",
            "baseline_artifact_readback_sha256",
            "reference_id",
            "reference_object_sha256",
            "fixed_view_ids",
            "selected_part_ids",
        },
        "Dragonfang correction selector fields are not closed",
    )
    require(
        selector["selected_part_ids"] == list(DRAGONFANG_SILHOUETTE_CORRECTION_PART_IDS),
        "Dragonfang correction selector scope drifted",
    )
    baseline = payload["baseline"]
    attempt = payload["attempt"]
    require(
        isinstance(baseline, dict)
        and set(baseline)
        == {
            "candidate_id",
            "candidate_state_sha256",
            "geometry_program_sha256",
            "geometry_program_object_sha256",
            "artifact_sha256",
            "artifact_readback_sha256",
        },
        "Dragonfang correction baseline fields are not closed",
    )
    require(
        isinstance(attempt, dict)
        and set(attempt)
        == {
            "parent_candidate_id",
            "geometry_program_sha256",
            "geometry_program_object_sha256",
            "representation_plan_sha256",
            "candidate_status",
            "candidate_id",
            "candidate_state_sha256",
            "artifact_sha256",
            "artifact_readback_sha256",
        },
        "Dragonfang correction attempt fields are not closed",
    )
    require(
        baseline["candidate_id"] == selector["baseline_candidate_id"]
        and attempt["parent_candidate_id"] == baseline["candidate_id"]
        and attempt["candidate_status"] == "NOT_MATERIALIZED"
        and all(attempt[field] is None for field in ("candidate_id", "candidate_state_sha256", "artifact_sha256", "artifact_readback_sha256")),
        "Dragonfang correction attempt is not an unmaterialized child proposal",
    )
    for field in (
        "candidate_state_sha256",
        "geometry_program_sha256",
        "geometry_program_object_sha256",
        "artifact_sha256",
        "artifact_readback_sha256",
    ):
        require_sha256(baseline[field], f"Dragonfang correction baseline {field}")
    for field in (
        "geometry_program_sha256",
        "geometry_program_object_sha256",
        "representation_plan_sha256",
    ):
        require_sha256(attempt[field], f"Dragonfang correction attempt {field}")
    program = payload["geometry_program"]
    require(isinstance(program, dict), "Dragonfang correction GeometryProgram is missing")
    require(
        program.get("canonical_sha256") == attempt["geometry_program_sha256"]
        and object_sha256(program) == attempt["geometry_program_object_sha256"],
        "Dragonfang correction GeometryProgram hashes drifted",
    )
    require(
        program.get("representation_plan_sha256") == attempt["representation_plan_sha256"],
        "Dragonfang correction representation plan hash drifted",
    )
    scope = payload["scope"]
    require(
        isinstance(scope, dict)
        and set(scope)
        == {
            "changed_part_ids",
            "changed_node_ids",
            "preserved_part_ids",
            "preserved_node_ids",
            "preserved_node_hashes",
            "preserved_part_output_hashes",
            "preserved_part_count",
            "max_changed_parts",
            "forbidden_part_ids",
        },
        "Dragonfang correction scope fields are not closed",
    )
    require(
        scope["changed_part_ids"] == list(DRAGONFANG_SILHOUETTE_CORRECTION_PART_IDS)
        and scope["forbidden_part_ids"] == list(DRAGONFANG_SILHOUETTE_CORRECTION_FORBIDDEN_PART_IDS)
        and scope["preserved_part_count"] == 8
        and scope["max_changed_parts"] == DRAGONFANG_SILHOUETTE_CORRECTION_MAX_PARTS,
        "Dragonfang correction scope policy drifted",
    )
    materialization_gate = payload["materialization_gate"]
    require(
        isinstance(materialization_gate, dict)
        and set(materialization_gate)
        == {
            "status",
            "current_route",
            "current_source_binding_part_ids",
            "required_route",
            "required_part_ids",
            "candidate_created",
            "runtime_write_performed",
        }
        and materialization_gate["status"] == DRAGONFANG_SILHOUETTE_MATERIALIZATION_STATUS
        and materialization_gate["current_route"] == DRAGONFANG_SILHOUETTE_CURRENT_MATERIALIZER
        and materialization_gate["current_source_binding_part_ids"] == ["blade-body"]
        and materialization_gate["required_route"] == DRAGONFANG_SILHOUETTE_REQUIRED_MATERIALIZER
        and materialization_gate["required_part_ids"] == list(DRAGONFANG_SILHOUETTE_CORRECTION_PART_IDS)
        and materialization_gate["candidate_created"] is False
        and materialization_gate["runtime_write_performed"] is False,
        "Dragonfang correction materialization gate drifted",
    )
    require(
        isinstance(scope["preserved_node_hashes"], dict)
        and isinstance(scope["preserved_part_output_hashes"], dict)
        and sorted(scope["preserved_node_hashes"]) == sorted(scope["preserved_node_ids"])
        and sorted(scope["preserved_part_output_hashes"]) == sorted(scope["preserved_part_ids"]),
        "Dragonfang correction preservation hash inventory drifted",
    )
    current_nodes = {
        node.get("node_id"): object_sha256(node)
        for node in program.get("nodes", [])
        if isinstance(node, dict) and isinstance(node.get("node_id"), str)
    }
    current_parts = {
        output.get("part_id"): object_sha256(output)
        for output in program.get("part_outputs", [])
        if isinstance(output, dict) and isinstance(output.get("part_id"), str)
    }
    require(
        all(current_nodes.get(node_id) == node_hash for node_id, node_hash in scope["preserved_node_hashes"].items()),
        "Dragonfang correction preserved node hash drifted",
    )
    require(
        all(current_parts.get(part_id) == part_hash for part_id, part_hash in scope["preserved_part_output_hashes"].items()),
        "Dragonfang correction preserved Part output hash drifted",
    )
    require(
        isinstance(payload["node_changes"], list)
        and len(payload["node_changes"]) == len(DRAGONFANG_SILHOUETTE_CORRECTION_PART_IDS),
        "Dragonfang correction node change inventory drifted",
    )
    require(
        {change.get("part_id") for change in payload["node_changes"]}
        == set(DRAGONFANG_SILHOUETTE_CORRECTION_PART_IDS),
        "Dragonfang correction changed-node Part scope drifted",
    )
    for change in payload["node_changes"]:
        require(
            isinstance(change, dict)
            and set(change)
            == {
                "part_id",
                "node_id",
                "baseline_node_sha256",
                "attempt_node_sha256",
                "changed_profile_indices",
                "profile_changes",
            },
            "Dragonfang correction node change fields are not closed",
        )
        require_sha256(change["baseline_node_sha256"], "Dragonfang baseline node hash")
        require_sha256(change["attempt_node_sha256"], "Dragonfang attempt node hash")
        require(change["baseline_node_sha256"] != change["attempt_node_sha256"], "Dragonfang target node did not change")
    review = payload["review_contract"]
    require(
        isinstance(review, dict)
        and set(review)
        == {"comparison_style", "fixed_view_ids", "metric_priority", "status", "visual_quality_promotion"}
        and review["fixed_view_ids"] == list(DRAGONFANG_SILHOUETTE_CORRECTION_VIEW_IDS)
        and review["status"] == "NOT_RUN_UNTIL_RUNTIME_MULTI_PART_MATERIALIZES_CHILD"
        and review["visual_quality_promotion"] == "NOT_PROMOTED",
        "Dragonfang correction review contract drifted",
    )
    lineage = payload["lineage"]
    require(
        isinstance(lineage, dict)
        and set(lineage)
        == {"parent_candidate_id", "parent_geometry_program_sha256", "child_candidate_created_by", "immutable_parent_preserved"}
        and lineage["parent_candidate_id"] == baseline["candidate_id"]
        and lineage["parent_geometry_program_sha256"] == baseline["geometry_program_sha256"]
        and lineage["child_candidate_created_by"] == "forgecad-runtime-only-state-writer@1"
        and lineage["immutable_parent_preserved"] is True,
        "Dragonfang correction lineage boundary drifted",
    )


def _operation_suffix(value: str, label: str, *, max_length: int = 64) -> str:
    require(
        isinstance(value, str)
        and 1 <= len(value) <= max_length
        and bool(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*", value)),
        f"{label} is not a bounded operation suffix",
    )
    return value


def geometry_program_hash_request(program: dict[str, Any]) -> dict[str, Any]:
    require(isinstance(program, dict), "GeometryProgram hash input must be an object")
    return {
        "schema_version": "GeometryProgramHashRequest@1",
        "geometry_program_draft": program,
    }


def geometry_prepare_request(
    project_id: str,
    reference_id: str,
    program: dict[str, Any],
    idempotency_key: str,
) -> dict[str, Any]:
    require(isinstance(project_id, str) and project_id, "geometry project ID is missing")
    require(isinstance(reference_id, str) and reference_id, "geometry reference ID is missing")
    require(isinstance(program, dict), "geometry program is missing")
    _operation_suffix(idempotency_key, "geometry idempotency key", max_length=128)
    return {
        "project_id": project_id,
        "base_version_id": None,
        "idempotency_key": idempotency_key,
        "request": {
            "typed": "geometry",
            "reference_id": reference_id,
            "geometry_program": program,
        },
    }


def verify_geometry_candidate_program(
    result: dict[str, Any],
    project_id: str,
    program_sha256: str,
    program: dict[str, Any],
    label: str,
) -> tuple[dict[str, Any], dict[str, Any]]:
    require(result.get("schema_version") == "GeometryPrepareResult@2", f"{label} schema drifted")
    candidate = result.get("candidate")
    artifact = result.get("artifact")
    require(isinstance(candidate, dict) and isinstance(artifact, dict), f"{label} omitted candidate/artifact")
    require(candidate.get("project_id") == project_id, f"{label} candidate project drifted")
    require(candidate.get("state") == "reviewable", f"{label} candidate is not reviewable")
    require(candidate.get("quality_hard_gate_passed") is True, f"{label} candidate hard gate failed")
    require(candidate.get("prepared_object_sha256") == artifact.get("object_sha256"), f"{label} candidate/artifact hash drifted")
    require(artifact.get("program_sha256") == program_sha256, f"{label} GeometryProgram hash drifted")
    verify_artifact_readback(artifact, candidate["candidate_id"], f"{label} ArtifactReadback")
    expected_bindings = sorted(
        (
            output["part_id"],
            node_id,
            output["material_zone_id"],
            output["solid"],
        )
        for output in program.get("part_outputs", [])
        for node_id in output.get("input_node_ids", [])
    )
    actual_bindings = sorted(
        (
            binding.get("part_id"),
            binding.get("source_node_id"),
            binding.get("material_zone_id"),
            binding.get("solid"),
        )
        for binding in artifact.get("part_bindings", [])
    )
    require(
        expected_bindings and actual_bindings == expected_bindings,
        f"{label} ArtifactReadback Part/node binding set drifted",
    )
    return candidate, artifact


def source_prepare_request(
    project_id: str,
    candidate: dict[str, Any],
    artifact: dict[str, Any],
    *,
    suffix: str = "source",
    part_id: str = "blade-body",
    source_node_id: str = "dragonfang-blade-body",
) -> dict[str, Any]:
    suffix = _operation_suffix(suffix, "source prepare suffix")
    idempotency_key = (
        f"{project_id}-dragonfang-amv2-source"
        if suffix == "source"
        else f"{project_id}-dragonfang-amv2-source-{suffix}"
    )
    _operation_suffix(idempotency_key, "source prepare idempotency key", max_length=128)
    value: dict[str, Any] = {
        "schema_version": "ProductionWeaponAuthoringMeshV2SourcePrepareRequest@1",
        "project_id": project_id,
        "candidate_id": candidate["candidate_id"],
        "candidate_state_sha256": candidate["canonical_sha256"],
        "geometry_program_sha256": artifact["program_sha256"],
        "artifact_sha256": candidate["prepared_object_sha256"],
        "artifact_readback_sha256": artifact["canonical_sha256"],
        "part_id": part_id,
        "source_node_id": source_node_id,
        "idempotency_key": idempotency_key,
        "max_response_bytes": MAX_RESPONSE_BYTES,
        "runtime_write_performed": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-canonical-sha256@1",
        "input_sha256": "",
    }
    value["input_sha256"] = canonical_hash(value, "input_sha256")
    return value


def authoring_mesh_identity_sha256(source: dict[str, Any]) -> str:
    durable = source["authoring_mesh_v2"]
    return object_sha256(
        {
            "schema_version": "AuthoringMeshSourceIdentity@1",
            "mesh_id": source["mesh_id"],
            "lineage_id": source["lineage_id"],
            "revision_id": source["revision_id"],
            "revision_index": durable["revision_index"],
            "revision_sha256": source["revision_sha256"],
        }
    )


def dragonfang_v2_curve(
    curve_id: str, role: str, control_points_m: list[list[float]]
) -> dict[str, Any]:
    """Create one closed cubic rail; callers cannot inject scripts or buffers."""
    require(role in {"blade_spine", "blade_edge"}, "Dragonfang V2 curve role is invalid")
    require(len(control_points_m) == 4, "Dragonfang V2 rail must be one cubic Bezier segment")
    value: dict[str, Any] = {
        "curve_id": curve_id,
        "role": role,
        "basis": "bezier",
        "degree": 3,
        "control_points_m": control_points_m,
        "weights": [],
        "knots": [],
        "closed": False,
        "canonical_sha256": "",
    }
    value["canonical_sha256"] = canonical_hash_without_field(
        value, "canonical_sha256"
    )
    return value


def dragonfang_v2_blade_requests(
    project_id: str,
    candidate: dict[str, Any],
    source: dict[str, Any],
    *,
    suffix: str,
    correction_round: bool = False,
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Bind the two Dragonfang rails and four calibrated sections to V2.

    The optional correction changes only the two rails and the four blade
    sections.  The public output scope remains exactly blade-body and
    cutting-edge; dragon relief, guard, grip and material programs are absent.
    """
    suffix = _operation_suffix(suffix, "Dragonfang V2 suffix")
    if correction_round:
        spine_points = [
            [-1.10, 0.055, 0.0],
            [-0.36, 0.535, 0.0],
            [1.18, 0.500, 0.0],
            [2.17, 0.175, 0.0],
        ]
        edge_points = [
            [-1.17, -0.105, 0.0],
            [-0.22, -0.485, 0.0],
            [1.30, -0.305, 0.0],
            [2.10, 0.105, 0.0],
        ]
        belly_thickness = 0.046
        tip_thickness = 0.010
    else:
        spine_points = [
            [-1.10, 0.050, 0.0],
            [-0.35, 0.480, 0.0],
            [1.20, 0.440, 0.0],
            [2.09, 0.205, 0.0],
        ]
        edge_points = [
            [-1.18, -0.100, 0.0],
            [-0.20, -0.420, 0.0],
            [1.25, -0.250, 0.0],
            [2.02, 0.135, 0.0],
        ]
        belly_thickness = 0.050
        tip_thickness = 0.012
    spine = dragonfang_v2_curve(
        f"dragonfang-spine-{suffix}", "blade_spine", spine_points
    )
    edge = dragonfang_v2_curve(
        f"dragonfang-edge-{suffix}", "blade_edge", edge_points
    )
    durable = source.get("authoring_mesh_v2")
    require(isinstance(durable, dict), "Dragonfang V2 source revision is missing")
    graph: dict[str, Any] = {
        "graph_id": f"dragonfang-v2-graph-{suffix}",
        "source_revision_id": source["revision_id"],
        "source_revision_sha256": source["revision_sha256"],
        "nodes": [
            {
                "node_id": "dragonfang-v2-profile",
                "operator": {
                    "operator": "curve_profile",
                    "curve_id": spine["curve_id"],
                    "curve_sha256": spine["canonical_sha256"],
                },
                "input_node_ids": [],
                "selection_query_sha256": None,
                "enabled": True,
            }
        ],
        "output_node_ids": ["dragonfang-v2-profile"],
        "canonical_sha256": "",
    }
    graph["canonical_sha256"] = canonical_hash_without_field(
        graph, "canonical_sha256"
    )
    identity_sha256 = authoring_mesh_identity_sha256(source)
    structural: dict[str, Any] = {
        "schema_version": "KnifeCurveModifierGraphPrepareRequest@1",
        "operation": "knife_curve_modifier_graph_prepare",
        "project_id": project_id,
        "source_candidate_id": candidate["candidate_id"],
        "source_candidate_state_sha256": candidate["canonical_sha256"],
        "source_authoring_mesh_id": source["mesh_id"],
        "source_authoring_mesh_lineage_id": source["lineage_id"],
        "source_authoring_mesh_revision_id": source["revision_id"],
        "source_authoring_mesh_revision_index": durable["revision_index"],
        "source_authoring_mesh_revision_sha256": source["revision_sha256"],
        "source_authoring_mesh_identity_sha256": identity_sha256,
        "curves": [spine, edge],
        "modifier_graph": graph,
        "dirty_seeds": ["dragonfang-v2-profile"],
        "recompute_policy": "dirty-seed-dependency-closure-recompute@1",
        "evaluation_policy": "original-authoring-mesh-modifier-graph-deterministic@1",
        "idempotency_key": f"{project_id}-dragonfang-v2-graph-{suffix}",
        "max_response_bytes": MAX_RESPONSE_BYTES,
        "runtime_write_performed": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-input-sha256@1",
        "input_sha256": "",
    }
    structural["input_sha256"] = canonical_hash(structural, "input_sha256")
    plan: dict[str, Any] = {
        "schema_version": "KnifeBladeProfileSweepLoftPlan@2",
        "evaluation_id": f"dragonfang-v2-evaluation-{suffix}",
        "spine_curve_id": spine["curve_id"],
        "spine_curve_sha256": spine["canonical_sha256"],
        "edge_curve_id": edge["curve_id"],
        "edge_curve_sha256": edge["canonical_sha256"],
        "station_count": 10,
        "sections": [
            {"section_id": "dragonfang-root", "role": "root", "station_t": 0.0, "body_thickness_m": 0.060, "edge_thickness_m": 0.018, "spine_bevel_fraction": 0.10, "edge_bevel_fraction": 0.12, "center_offset_m": 0.0},
            {"section_id": "dragonfang-mid", "role": "mid", "station_t": 0.32, "body_thickness_m": 0.055, "edge_thickness_m": 0.014, "spine_bevel_fraction": 0.12, "edge_bevel_fraction": 0.16, "center_offset_m": 0.002},
            {"section_id": "dragonfang-belly", "role": "belly", "station_t": 0.72, "body_thickness_m": belly_thickness, "edge_thickness_m": 0.010, "spine_bevel_fraction": 0.15, "edge_bevel_fraction": 0.20, "center_offset_m": -0.001},
            {"section_id": "dragonfang-tip", "role": "tip", "station_t": 1.0, "body_thickness_m": tip_thickness, "edge_thickness_m": 0.003, "spine_bevel_fraction": 0.20, "edge_bevel_fraction": 0.24, "center_offset_m": 0.0},
        ],
        "thickness_axis": "local_normal",
        "root_cap": True,
        "tip_cap": True,
        "view_constraints": [
            {"view": "front", "min_x_m": -1.25, "max_x_m": 2.18, "min_y_m": -0.55, "max_y_m": 0.60},
            {"view": "top", "min_x_m": -1.25, "max_x_m": 2.18, "min_y_m": -0.05, "max_y_m": 0.05},
            {"view": "bottom", "min_x_m": -1.25, "max_x_m": 2.18, "min_y_m": -0.05, "max_y_m": 0.05},
            {"view": "left", "min_x_m": -0.55, "max_x_m": 0.60, "min_y_m": -0.05, "max_y_m": 0.05},
            {"view": "right", "min_x_m": -0.55, "max_x_m": 0.60, "min_y_m": -0.05, "max_y_m": 0.05},
        ],
        "stable_triangulation": "station-ring-fixed-diagonal@2",
        "stable_lineage_policy": "source-curve-modifier-graph-sectioned-evaluated-mesh@1",
        "canonical_sha256": "",
    }
    plan["canonical_sha256"] = canonical_hash_without_field(
        plan, "canonical_sha256"
    )
    evaluation: dict[str, Any] = {
        "schema_version": "KnifeCurveEvaluatedMeshPrepareRequest@1",
        "operation": "knife_curve_evaluated_mesh_prepare",
        "project_id": project_id,
        "source_candidate_id": candidate["candidate_id"],
        "source_candidate_state_sha256": candidate["canonical_sha256"],
        "source_authoring_mesh_id": source["mesh_id"],
        "source_authoring_mesh_lineage_id": source["lineage_id"],
        "source_authoring_mesh_revision_id": source["revision_id"],
        "source_authoring_mesh_revision_index": durable["revision_index"],
        "source_authoring_mesh_revision_sha256": source["revision_sha256"],
        "source_authoring_mesh_identity_sha256": identity_sha256,
        "source_modifier_graph_id": graph["graph_id"],
        "source_modifier_graph_sha256": "",
        "curve_set_semantic_sha256": "",
        "sample_set_semantic_sha256": "",
        "modifier_graph_semantic_sha256": "",
        "dependency_graph_semantic_sha256": "",
        "recompute_plan_semantic_sha256": "",
        "evaluation_plan": plan,
        "idempotency_key": f"{project_id}-dragonfang-v2-evaluation-{suffix}",
        "max_response_bytes": MAX_RESPONSE_BYTES,
        "runtime_write_performed": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-input-sha256@1",
        "input_sha256": "",
    }
    return structural, evaluation


def bind_dragonfang_v2_evaluation_request(
    request: dict[str, Any], structural: dict[str, Any]
) -> dict[str, Any]:
    value = copy.deepcopy(request)
    value["source_modifier_graph_sha256"] = require_sha256(
        structural["modifier_graph_semantic_sha256"],
        "Dragonfang V2 source_modifier_graph_sha256",
    )
    for field in (
        "curve_set_semantic_sha256",
        "sample_set_semantic_sha256",
        "modifier_graph_semantic_sha256",
        "dependency_graph_semantic_sha256",
        "recompute_plan_semantic_sha256",
    ):
        value[field] = require_sha256(structural[field], f"Dragonfang V2 {field}")
    value["input_sha256"] = canonical_hash_without_field(value, "input_sha256")
    return value


def source_binding_main(
    project_id: str,
    successor: dict[str, Any],
    intent_result: dict[str, Any],
    reference: dict[str, Any],
    candidate: dict[str, Any],
    source: dict[str, Any],
    *,
    suffix: str = "source",
) -> dict[str, Any]:
    suffix = _operation_suffix(suffix, "source binding suffix")
    source_binding_id = (
        f"dragonfang-source-binding-{project_id[-12:]}"
        if suffix == "source"
        else f"dragonfang-source-binding-{project_id[-12:]}-{suffix}"
    )
    _operation_suffix(source_binding_id, "source binding ID", max_length=128)
    intent_bundle = intent_result["intent_bundle"]
    quality = intent_bundle["quality_contract"]
    value: dict[str, Any] = {
        "schema_version": "KnifeSourceBinding@1",
        "source_binding_id": source_binding_id,
        "project_id": project_id,
        "binding_status": "runtime-bound",
        "authoring_eligibility": "ELIGIBLE",
        "intent_bundle_id": intent_result["intent_bundle_id"],
        "intent_bundle_sha256": intent_result["intent_bundle_sha256"],
        "intent_bundle_object_sha256": intent_result["intent_bundle_object_sha256"],
        "brief_id": successor["brief_id"],
        "brief_sha256": successor["brief_sha256"],
        "brief_object_sha256": successor["brief_object_sha256"],
        "reference_id": reference["reference_id"],
        "reference_object_sha256": reference["object_sha256"],
        "reference_evidence_sha256": reference["canonical_sha256"],
        "quality_contract_id": quality["contract_id"],
        "quality_contract_sha256": quality["canonical_sha256"],
        "quality_contract_object_sha256": object_sha256(quality),
        "source_candidate_id": candidate["candidate_id"],
        "source_candidate_state_sha256": candidate["canonical_sha256"],
        "authoring_mesh_id": source["mesh_id"],
        "authoring_mesh_lineage_id": source["lineage_id"],
        "authoring_mesh_revision_id": source["revision_id"],
        "authoring_mesh_revision_index": source["authoring_mesh_v2"]["revision_index"],
        "authoring_mesh_revision_sha256": source["revision_sha256"],
        "authoring_mesh_revision_object_sha256": source["revision_object_sha256"],
        "authoring_mesh_identity_sha256": authoring_mesh_identity_sha256(source),
        "downstream_binding_requirements": {
            "curve_modifier_graph": "must-inherit-source-binding-sha256@1",
            "curve_evaluated_mesh": "must-inherit-source-binding-sha256@1",
            "high": "must-inherit-source-binding-sha256@1",
            "render": "must-inherit-source-binding-sha256@1",
        },
        "high_mesh_created": False,
        "high_stage_unlocked": False,
        "production_stage_advanced": False,
        "candidate_confirmed": False,
        "version_created": False,
        "export_performed": False,
        "quality_status": "source_binding_only",
        "visual_status": "NOT_RUN",
        "human_status": "NOT_RUN",
        "engine_status": "NOT_RUN",
        "binding_policy": "intent-brief-reference-quality-to-authoring-mesh-exact@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-canonical-sha256@1",
        "canonical_sha256": "",
        "created_at": "2026-08-30T00:00:00Z",
    }
    value["canonical_sha256"] = canonical_hash(value, "canonical_sha256")
    return value


def source_binding_prepare_request(
    project_id: str,
    source_binding: dict[str, Any],
    *,
    suffix: str = "source",
) -> dict[str, Any]:
    suffix = _operation_suffix(suffix, "source binding prepare suffix")
    idempotency_key = (
        f"{project_id}-dragonfang-source-binding"
        if suffix == "source"
        else f"{project_id}-dragonfang-source-binding-{suffix}"
    )
    _operation_suffix(idempotency_key, "source binding idempotency key", max_length=128)
    value: dict[str, Any] = {
        "schema_version": "KnifeSourceBindingPrepareRequest@1",
        "operation": "knife_source_binding_prepare",
        "project_id": project_id,
        "source_binding": source_binding,
        "idempotency_key": idempotency_key,
        "max_response_bytes": MAX_RESPONSE_BYTES,
        "runtime_write_performed": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-input-sha256@1",
        "input_sha256": "",
    }
    value["input_sha256"] = canonical_hash(value, "input_sha256")
    return value


def source_binding_get_request(result: dict[str, Any]) -> dict[str, Any]:
    fields = [
        "project_id",
        "source_binding_id",
        "source_binding_sha256",
        "source_binding_object_sha256",
        "intent_bundle_id",
        "intent_bundle_sha256",
        "intent_bundle_object_sha256",
        "brief_id",
        "brief_sha256",
        "brief_object_sha256",
        "reference_id",
        "reference_object_sha256",
        "reference_evidence_sha256",
        "quality_contract_id",
        "quality_contract_sha256",
        "quality_contract_object_sha256",
        "source_candidate_id",
        "source_candidate_state_sha256",
        "authoring_mesh_id",
        "authoring_mesh_lineage_id",
        "authoring_mesh_revision_id",
        "authoring_mesh_revision_index",
        "authoring_mesh_revision_sha256",
        "authoring_mesh_revision_object_sha256",
        "authoring_mesh_identity_sha256",
    ]
    value = {field: result[field] for field in fields}
    value.update(
        {
            "schema_version": "KnifeSourceBindingGetRequest@1",
            "operation": "knife_source_binding_get",
            "max_response_bytes": MAX_RESPONSE_BYTES,
            "runtime_write_performed": False,
            "persistent_user_data_touched": False,
            "writer_policy": "forgecad-runtime-only-state-writer@1",
            "canonicalization_policy": "canonical-json-sha256-excluding-input-sha256@1",
            "input_sha256": "",
        }
    )
    value["input_sha256"] = canonical_hash(value, "input_sha256")
    return value


def materializer_prepare_request(
    project_id: str,
    source: dict[str, Any],
    source_binding: dict[str, Any],
    revision_result: dict[str, Any] | None = None,
    idempotency_key: str | None = None,
) -> dict[str, Any]:
    if revision_result is None:
        revision_index = source["authoring_mesh_v2"]["revision_index"]
        revision_id = source["revision_id"]
        revision_sha256 = source["revision_sha256"]
        revision_object_sha256 = source["revision_object_sha256"]
    else:
        revision_index = revision_result["revision_index"]
        revision_id = revision_result["revision_id"]
        revision_sha256 = revision_result["revision_sha256"]
        revision_object_sha256 = revision_result["revision_object_sha256"]
    value: dict[str, Any] = {
        "schema_version": "AuthoringMeshV2CandidateMaterializeRequest@1",
        "operation": "authoring_mesh_v2_candidate_materialize",
        "project_id": project_id,
        "mesh_id": source["mesh_id"],
        "lineage_id": source["lineage_id"],
        "revision_id": revision_id,
        "revision_index": revision_index,
        "revision_sha256": revision_sha256,
        "revision_object_sha256": revision_object_sha256,
        "source_binding_id": source_binding["source_binding_id"],
        "source_binding_sha256": source_binding["source_binding_sha256"],
        "source_binding_object_sha256": source_binding[
            "source_binding_object_sha256"
        ],
        "base_version_id": None,
        "idempotency_key": idempotency_key
        or f"{project_id}-dragonfang-amv2-materialize",
        "max_response_bytes": MAX_RESPONSE_BYTES,
        "runtime_write_performed": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-input-sha256@1",
        "input_sha256": "",
    }
    value["input_sha256"] = canonical_hash(value, "input_sha256")
    return value


HIGH_BRIDGE_SCOPE_LIMITATIONS = [
    "RUNTIME_DERIVES_COMPLETE_ORDERED_PART_INPUTS",
    "RUNTIME_CONSTRUCTS_CPU_STITCHED_STEPS",
    "NO_CALLER_SUPPLIED_REVISION_TOPOLOGY",
    "NO_OPEN_SUBDIVISION_BACKEND",
    "VERIFIED_PRESERVED_PARTS_FROM_MATERIALIZED_GLB",
]


def high_bridge_prepare_request(
    project_id: str,
    source: dict[str, Any],
    source_binding: dict[str, Any],
    materialized: dict[str, Any],
    evidence: dict[str, str],
    artifact_readback: dict[str, Any],
    bridge_id: str,
    idempotency_key: str,
) -> dict[str, Any]:
    """Build the closed identity-only High Bridge request from live lineage."""
    candidate = materialized.get("candidate")
    artifact = materialized.get("artifact")
    require(isinstance(candidate, dict) and isinstance(artifact, dict), "High Bridge materialization is incomplete")
    revision = source.get("authoring_mesh_v2")
    require(isinstance(revision, dict), "High Bridge source revision is unavailable")
    artifact_object_sha = require_sha256(
        artifact.get("object_sha256"), "High Bridge materialized artifact object sha256"
    )
    artifact_id = artifact.get("artifact_id")
    require(
        isinstance(artifact_id, str)
        and artifact_id
        and artifact_readback.get("artifact_id") == artifact_id,
        "High Bridge materialized artifact ID is not readback-bound",
    )
    require(
        artifact_object_sha == candidate.get("prepared_object_sha256"),
        "High Bridge materialized artifact is not candidate-bound",
    )
    readback_canonical = require_sha256(
        artifact_readback.get("canonical_sha256"),
        "High Bridge materialized ArtifactReadback canonical_sha256",
    )
    value: dict[str, Any] = {
        "schema_version": "AuthoringMeshV2HighBridgePrepareRequest@1",
        "operation": "authoring_mesh_v2_high_bridge_prepare",
        "project_id": project_id,
        "bridge_id": bridge_id,
        "source_scope": "materialized-v2-revision-part-set@1",
        "source_revision_schema_version": "AuthoringMeshRevision@2",
        "mesh_id": source["mesh_id"],
        "lineage_id": source["lineage_id"],
        "revision_id": source["revision_id"],
        "revision_index": revision["revision_index"],
        "revision_sha256": source["revision_sha256"],
        "revision_object_sha256": source["revision_object_sha256"],
        "source_binding_id": source_binding["source_binding_id"],
        "source_binding_sha256": source_binding["source_binding_sha256"],
        "source_binding_object_sha256": source_binding["source_binding_object_sha256"],
        "materialized_candidate_id": candidate["candidate_id"],
        "materialized_candidate_state_sha256": candidate["canonical_sha256"],
        "materialized_program_sha256": evidence["program_sha256"],
        "materialized_program_object_sha256": evidence["program_object_sha256"],
        "materialized_artifact_id": artifact_id,
        "materialized_artifact_sha256": candidate["prepared_object_sha256"],
        "materialized_artifact_object_sha256": artifact_object_sha,
        "materialized_artifact_readback_sha256": readback_canonical,
        "materialized_artifact_readback_object_sha256": evidence["readback_object_sha256"],
        "representation_plan_sha256": materialized["representation_plan_sha256"],
        "source_node_id": materialized["source_node_id"],
        "part_id": materialized["source_part_id"],
        "material_zone_id": materialized["source_material_zone_id"],
        "solid": materialized["source_solid"],
        "source_part_output_sha256": materialized["source_part_output_sha256"],
        "preserved_part_ids": materialized["preserved_part_ids"],
        "materialized_artifact_hash_policy": "artifact-sha256-equals-object-sha256-until-semantic-artifact-contract@1",
        "high_execution_request_schema_version": "AuthoringMeshV2HighExecutionRequest@2",
        "high_execution_operation": "forgecad.production.authoring-mesh-v2-high-execute@1",
        "high_operation": "forgecad.production.authoring-mesh-v2-high-evaluate@1",
        "high_result_schema_version": "AuthoringMeshV2HighResult@2",
        "high_readback_schema_version": "AuthoringMeshV2HighReadback@2",
        "high_evaluator_contract": "forgecad-owned-cpu-catmull-clark-stitched-polygon@2",
        "high_subdivision_backend": "cpu_regular_quad",
        "high_subdivision_levels": 1,
        "high_max_triangles_per_face": 32,
        "high_max_output_vertices": 32768,
        "high_max_output_triangles": 600000,
        "scope_limitations": list(HIGH_BRIDGE_SCOPE_LIMITATIONS),
        "idempotency_key": idempotency_key,
        "max_response_bytes": MAX_RESPONSE_BYTES,
        "runtime_write_performed": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-input-sha256@1",
        "input_sha256": "",
    }
    value["input_sha256"] = canonical_hash(value, "input_sha256")
    return value


def high_bridge_get_request(result: dict[str, Any]) -> dict[str, Any]:
    """Build exact readback identity from the durable High Bridge result."""
    bridge = result.get("bridge")
    require(isinstance(bridge, dict), "High Bridge result omitted its durable bridge")
    fields = (
        "project_id", "bridge_id", "bridge_sha256", "bridge_object_sha256",
        "source_scope", "source_revision_schema_version", "mesh_id", "lineage_id",
        "revision_id", "revision_index", "revision_sha256", "revision_object_sha256",
        "source_binding_id", "source_binding_sha256", "source_binding_object_sha256",
        "materialized_candidate_id", "materialized_candidate_state_sha256",
        "materialized_program_sha256", "materialized_program_object_sha256",
        "materialized_artifact_id", "materialized_artifact_sha256",
        "materialized_artifact_object_sha256", "materialized_artifact_readback_sha256",
        "materialized_artifact_readback_object_sha256", "representation_plan_sha256",
        "source_node_id", "part_id", "material_zone_id", "solid",
        "source_part_output_sha256", "preserved_part_ids", "materialized_artifact_hash_policy",
        "high_execution_request_schema_version", "high_execution_operation", "high_operation",
        "high_result_schema_version", "high_readback_schema_version", "high_evaluator_contract",
        "high_subdivision_backend", "high_subdivision_levels", "high_max_triangles_per_face",
        "high_max_output_vertices", "high_max_output_triangles", "high_execution_request_sha256",
        "high_evaluation_sha256", "high_result_sha256", "high_result_object_sha256",
        "high_readback_sha256", "high_readback_object_sha256", "high_worker_algorithm_sha256",
        "high_worker_build_cohort_sha256", "high_replay_count", "high_replay_byte_exact",
        "high_non_destructive", "high_projected_source_mesh_sha256", "high_source_vertex_count",
        "high_source_triangle_count", "high_evaluated_part_count", "high_evaluated_triangle_count",
        "scope_limitations",
    )
    # The immutable Main record intentionally owns only canonical_sha256.  Its
    # semantic/object identities live on the durable result envelope so the
    # CAS hash never becomes self-referential.
    envelope_fields = {"project_id", "bridge_id", "bridge_sha256", "bridge_object_sha256"}
    value: dict[str, Any] = {
        field: result[field] if field in envelope_fields else bridge[field]
        for field in fields
    }
    value.update({
        "schema_version": "AuthoringMeshV2HighBridgeGetRequest@1",
        "operation": "authoring_mesh_v2_high_bridge_get",
        "max_response_bytes": MAX_RESPONSE_BYTES,
        "runtime_write_performed": False,
        "persistent_user_data_touched": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-input-sha256@1",
        "input_sha256": "",
    })
    value["input_sha256"] = canonical_hash(value, "input_sha256")
    return value


def high_artifact_prepare_request(
    bridge_result: dict[str, Any], artifact_id: str, idempotency_key: str
) -> dict[str, Any]:
    value: dict[str, Any] = {
        "schema_version": "AuthoringMeshV2HighArtifactPrepareRequest@1",
        "operation": "authoring_mesh_v2_high_artifact_prepare",
        "project_id": bridge_result["project_id"],
        "high_artifact_id": artifact_id,
        "high_bridge_id": bridge_result["bridge_id"],
        "high_bridge_sha256": bridge_result["bridge_sha256"],
        "high_bridge_object_sha256": bridge_result["bridge_object_sha256"],
        "idempotency_key": idempotency_key,
        "max_response_bytes": MAX_RESPONSE_BYTES,
        "runtime_write_performed": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-input-sha256@1",
        "input_sha256": "",
    }
    value["input_sha256"] = canonical_hash(value, "input_sha256")
    return value


def high_artifact_get_request(result: dict[str, Any]) -> dict[str, Any]:
    fields = (
        "project_id", "high_artifact_id", "high_artifact_sha256",
        "high_artifact_object_sha256", "high_artifact_readback_sha256",
        "high_artifact_readback_object_sha256", "high_artifact_receipt_sha256",
        "high_artifact_receipt_object_sha256", "high_bridge_id", "high_bridge_sha256",
        "high_bridge_object_sha256",
    )
    value: dict[str, Any] = {field: result[field] for field in fields}
    value.update({
        "schema_version": "AuthoringMeshV2HighArtifactGetRequest@1",
        "operation": "authoring_mesh_v2_high_artifact_get",
        "max_response_bytes": MAX_RESPONSE_BYTES,
        "runtime_write_performed": False,
        "persistent_user_data_touched": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-input-sha256@1",
        "input_sha256": "",
    })
    value["input_sha256"] = canonical_hash(value, "input_sha256")
    return value


def verify_high_bridge_result(
    result: dict[str, Any], project_id: str, expected_cohort: str, label: str
) -> dict[str, Any]:
    require(result.get("schema_version") == "AuthoringMeshV2HighBridgeResult@1", f"{label} schema drifted")
    require(result.get("project_id") == project_id, f"{label} project binding drifted")
    bridge = result.get("bridge")
    require(isinstance(bridge, dict), f"{label} bridge payload is missing")
    bridge_semantic_sha = verify_canonical_object(bridge, "canonical_sha256", f"{label} bridge")
    require(result.get("bridge_sha256") == bridge_semantic_sha, f"{label} bridge semantic hash drifted")
    require(result.get("high_worker_build_cohort_sha256") == expected_cohort, f"{label} Worker cohort drifted")
    require(result.get("high_structural_status") == "PASS_SOURCE_STRUCTURAL", f"{label} structural status drifted")
    require(result.get("high_status") == "NOT_RUN" and result.get("quality_status") == "structural_only", f"{label} quality boundary drifted")
    require(result.get("visual_status") == "NOT_RUN" and result.get("human_status") == "NOT_RUN", f"{label} review boundary drifted")
    require(result.get("high_mesh_created") is False, f"{label} unexpectedly promoted High")
    return {
        "bridge_id": result["bridge_id"],
        "bridge_sha256": require_sha256(result["bridge_sha256"], f"{label} bridge_sha256"),
        "bridge_object_sha256": require_sha256(result["bridge_object_sha256"], f"{label} bridge_object_sha256"),
        "high_result_sha256": require_sha256(result["high_result_sha256"], f"{label} high_result_sha256"),
        "high_result_object_sha256": require_sha256(result["high_result_object_sha256"], f"{label} high_result_object_sha256"),
        "high_readback_sha256": require_sha256(result["high_readback_sha256"], f"{label} high_readback_sha256"),
        "high_readback_object_sha256": require_sha256(result["high_readback_object_sha256"], f"{label} high_readback_object_sha256"),
    }


def verify_high_artifact_result(
    result: dict[str, Any], project_id: str, expected_cohort: str, label: str
) -> dict[str, Any]:
    require(result.get("schema_version") == "AuthoringMeshV2HighArtifactResult@1", f"{label} schema drifted")
    require(result.get("project_id") == project_id, f"{label} project binding drifted")
    require(result.get("high_artifact_status") == "PASS_SOURCE_STRUCTURAL", f"{label} structural status drifted")
    require(result.get("high_artifact_hard_gate_passed") is True, f"{label} strict GLB gate did not pass")
    require(result.get("high_mesh_created") is True, f"{label} did not create a High artifact")
    require(result.get("high_worker_build_cohort_sha256") == expected_cohort, f"{label} Worker cohort drifted")
    require(result.get("quality_status") == "structural_only", f"{label} quality boundary drifted")
    require(result.get("visual_status") == "NOT_RUN", f"{label} visual boundary drifted")
    require(result.get("high_artifact_mime") == "model/gltf-binary", f"{label} MIME drifted")
    size = result.get("high_artifact_size_bytes")
    require(isinstance(size, int) and size > 0, f"{label} GLB size is invalid")
    glb_sha = require_sha256(result.get("high_artifact_sha256"), f"{label} high_artifact_sha256")
    require(result.get("glb_sha256") == glb_sha, f"{label} GLB hash drifted")
    require(result.get("glb_object_sha256") == result.get("high_artifact_object_sha256"), f"{label} GLB object hash drifted")
    strict = result.get("strict_readback")
    require(isinstance(strict, dict), f"{label} strict readback is missing")
    require(strict.get("schema_version") == "AuthoringMeshV2HighArtifactReadback@1", f"{label} strict readback schema drifted")
    part_ids = strict.get("part_ids")
    zone_ids = strict.get("material_zone_ids")
    require(isinstance(part_ids, list) and isinstance(zone_ids, list), f"{label} semantic inventory is missing")
    return {
        "high_artifact_id": result["high_artifact_id"],
        "high_artifact_sha256": glb_sha,
        "high_artifact_object_sha256": require_sha256(result["high_artifact_object_sha256"], f"{label} high_artifact_object_sha256"),
        "high_artifact_size_bytes": size,
        "high_artifact_readback_sha256": require_sha256(result["high_artifact_readback_sha256"], f"{label} high_artifact_readback_sha256"),
        "high_artifact_readback_object_sha256": require_sha256(result["high_artifact_readback_object_sha256"], f"{label} high_artifact_readback_object_sha256"),
        "high_artifact_receipt_sha256": require_sha256(result["high_artifact_receipt_sha256"], f"{label} high_artifact_receipt_sha256"),
        "high_artifact_receipt_object_sha256": require_sha256(result["high_artifact_receipt_object_sha256"], f"{label} high_artifact_receipt_object_sha256"),
        "high_artifact_readback_schema_version": result.get("high_artifact_readback_schema_version"),
        "part_ids": list(part_ids),
        "material_zone_ids": list(zone_ids),
        "part_inventory_sha256": result.get("high_artifact", {}).get("high_part_inventory_sha256"),
        "visual_status": result.get("visual_status"),
        "quality_status": result.get("quality_status"),
    }


def low_quad_draft_from_v2_program(
    program: dict[str, Any],
    part_id: str,
    source_high_artifact_sha256: str,
    source_high_artifact_readback_sha256: str,
) -> tuple[dict[str, Any], dict[str, int]]:
    """Compile one explicit quad draft from one Runtime-derived V2 Part.

    The V2 evaluator deliberately exposes a triangle authoring projection.
    Low cannot pass that projection through as an all-quad draft, so this
    probe performs a deterministic, source-bound triangle-pairing step.  It
    never calls a decimator or claims artist authorship: unpaired triangles
    are omitted from this structural draft and a Part with no valid pair is
    rejected explicitly.
    """
    require(isinstance(program, dict), "Low V2 materialization program is missing")
    outputs = program.get("part_outputs")
    nodes = program.get("nodes")
    require(isinstance(outputs, list) and isinstance(nodes, list), "Low V2 program topology is missing")
    output = next((value for value in outputs if isinstance(value, dict) and value.get("part_id") == part_id), None)
    require(isinstance(output, dict), f"Low V2 Part {part_id} is not declared")
    node_ids = output.get("input_node_ids")
    require(isinstance(node_ids, list) and len(node_ids) == 1 and isinstance(node_ids[0], str), f"Low V2 Part {part_id} node binding is invalid")
    node = next((value for value in nodes if isinstance(value, dict) and value.get("node_id") == node_ids[0]), None)
    require(isinstance(node, dict), f"Low V2 Part {part_id} node is missing")
    parameters = node.get("parameters")
    require(isinstance(parameters, dict), f"Low V2 Part {part_id} authoring mesh is missing")
    vertices = parameters.get("vertices")
    edges = parameters.get("edges")
    loops = parameters.get("loops")
    faces = parameters.get("faces")
    require(all(isinstance(value, list) for value in (vertices, edges, loops, faces)), f"Low V2 Part {part_id} topology arrays are invalid")
    vertex_positions = {
        value.get("element_id"): value.get("position_m")
        for value in vertices
        if isinstance(value, dict)
    }
    require(vertex_positions and all(isinstance(key, str) and isinstance(value, list) and len(value) == 3 for key, value in vertex_positions.items()), f"Low V2 Part {part_id} vertices are invalid")
    loop_by_id = {
        value.get("element_id"): value
        for value in loops
        if isinstance(value, dict)
    }
    face_vertices: dict[str, list[str]] = {}
    for face in faces:
        require(isinstance(face, dict) and isinstance(face.get("element_id"), str), f"Low V2 Part {part_id} face is invalid")
        loop_ids = face.get("loop_ids")
        require(isinstance(loop_ids, list) and len(loop_ids) == 3, f"Low V2 Part {part_id} input is not an explicit triangle source")
        ordered = []
        for loop_id in loop_ids:
            loop = loop_by_id.get(loop_id)
            require(isinstance(loop, dict) and isinstance(loop.get("vertex_id"), str), f"Low V2 Part {part_id} face loop is invalid")
            ordered.append(loop["vertex_id"])
        require(len(set(ordered)) == 3 and all(value in vertex_positions for value in ordered), f"Low V2 Part {part_id} has a degenerate triangle")
        face_vertices[face["element_id"]] = ordered
    require(face_vertices, f"Low V2 Part {part_id} has no source faces")

    edge_faces: dict[tuple[str, str], list[str]] = {}
    for face_id, triangle in face_vertices.items():
        for index in range(3):
            first, second = triangle[index], triangle[(index + 1) % 3]
            key = tuple(sorted((first, second)))
            edge_faces.setdefault(key, []).append(face_id)

    def paired_polygon(first: list[str], second: list[str]) -> list[str] | None:
        shared = set(first) & set(second)
        if len(shared) != 2 or len(set(first + second)) != 4:
            return None
        shared_edge = tuple(sorted(shared))
        directed: list[tuple[str, str]] = []
        for triangle in (first, second):
            for index in range(3):
                start, end = triangle[index], triangle[(index + 1) % 3]
                if tuple(sorted((start, end))) != shared_edge:
                    directed.append((start, end))
        outgoing: dict[str, str] = {}
        for start, end in directed:
            if start in outgoing:
                return None
            outgoing[start] = end
        if len(outgoing) != 4 or len(set(outgoing.values())) != 4:
            return None
        start = min(outgoing)
        polygon = [start]
        for _ in range(3):
            next_value = outgoing.get(polygon[-1])
            if next_value is None or next_value in polygon:
                return None
            polygon.append(next_value)
        if outgoing.get(polygon[-1]) != start:
            return None
        return polygon

    require(
        len(face_vertices) % 2 == 0,
        f"BLOCKED_LOW_INCOMPLETE_COVERAGE: Part {part_id} has an odd source triangle count",
    )
    # Build the legal face-dual graph first. The earlier face-order greedy
    # could strand a later face despite a complete matching being available.
    # A deterministic exact matching keeps the Low surface fully covered:
    # recurse on the remaining face with the fewest available neighbors,
    # break ties by face ID, try neighbors by ID, and prune zero-degree
    # vertices before branching. Dragonfang's two source graphs resolve in
    # linear-sized search with this ordering (51 and 29 quads).
    pair_candidates: dict[str, dict[str, list[str]]] = {
        face_id: {} for face_id in face_vertices
    }
    for face_id in sorted(face_vertices):
        triangle = face_vertices[face_id]
        for index in range(3):
            edge = tuple(sorted((triangle[index], triangle[(index + 1) % 3])))
            for neighbor in sorted(edge_faces.get(edge, [])):
                if neighbor == face_id or neighbor not in face_vertices:
                    continue
                first, second = sorted((face_id, neighbor))
                polygon = paired_polygon(face_vertices[first], face_vertices[second])
                if polygon is None:
                    continue
                pair_candidates[first][second] = polygon
                pair_candidates[second][first] = polygon

    failed_remaining: set[tuple[str, ...]] = set()

    def exact_matching(remaining: frozenset[str]) -> list[tuple[str, str, list[str]]] | None:
        if not remaining:
            return []
        key = tuple(sorted(remaining))
        if key in failed_remaining:
            return None
        available: dict[str, list[str]] = {
            face_id: sorted(
                neighbor for neighbor in pair_candidates[face_id] if neighbor in remaining
            )
            for face_id in remaining
        }
        # Zero-degree prune is both a correctness guard and a cheap rejection
        # of branches that cannot possibly cover every source triangle.
        if any(not neighbors for neighbors in available.values()):
            failed_remaining.add(key)
            return None
        face_id = min(remaining, key=lambda value: (len(available[value]), value))
        for neighbor in available[face_id]:
            next_remaining = remaining.difference((face_id, neighbor))
            result = exact_matching(frozenset(next_remaining))
            if result is not None:
                return [(face_id, neighbor, pair_candidates[face_id][neighbor]), *result]
        failed_remaining.add(key)
        return None

    matching = exact_matching(frozenset(face_vertices))
    require(
        matching is not None,
        f"BLOCKED_MULTI_PART_LOW: Part {part_id} has no deterministic perfect quad matching",
    )
    matching.sort(key=lambda value: (value[0], value[1]))
    used = {face_id for first, second, _ in matching for face_id in (first, second)}
    paired = [
        (f"low-quad-{part_id}-{ordinal:04d}", polygon)
        for ordinal, (_, _, polygon) in enumerate(matching)
    ]
    # A Low component must preserve the complete source surface. Omitting an
    # unpaired triangle would leave holes while still passing the structural
    # Worker validator.
    require(
        len(used) == len(face_vertices) and len(matching) * 2 == len(face_vertices),
        f"BLOCKED_LOW_INCOMPLETE_COVERAGE: Part {part_id} paired {len(used)} of {len(face_vertices)} source triangles",
    )

    used_vertices = sorted({vertex for _, polygon in paired for vertex in polygon})
    edge_keys = sorted({tuple(sorted((polygon[index], polygon[(index + 1) % 4]))) for _, polygon in paired for index in range(4)})
    edge_ids = {
        key: f"e-{hashlib.sha256(('|'.join(key)).encode('utf-8')).hexdigest()[:24]}"
        for key in edge_keys
    }
    low_loops: list[dict[str, Any]] = []
    low_faces: list[dict[str, Any]] = []
    for face_id, polygon in paired:
        face_loops: list[dict[str, Any]] = []
        for ordinal, vertex_id in enumerate(polygon):
            next_vertex = polygon[(ordinal + 1) % 4]
            edge_key = tuple(sorted((vertex_id, next_vertex)))
            loop_id = f"l-{hashlib.sha256((face_id + f'|{ordinal}').encode('utf-8')).hexdigest()[:24]}"
            face_loops.append({
                "element_id": loop_id,
                "face_id": face_id,
                "ordinal": ordinal,
                "vertex_id": vertex_id,
                "edge_id": edge_ids[edge_key],
                "edge_forward": vertex_id == edge_key[0],
            })
        # Preserve winding while rotating the ring to the lexicographically
        # smallest loop ID required by the canonical AuthoringMesh contract.
        rotation = min(range(4), key=lambda index: face_loops[index]["element_id"])
        face_loops = face_loops[rotation:] + face_loops[:rotation]
        for ordinal, loop in enumerate(face_loops):
            loop["ordinal"] = ordinal
        low_loops.extend(face_loops)
        loop_ids = [loop["element_id"] for loop in face_loops]
        low_faces.append({"element_id": face_id, "loop_ids": loop_ids})
    # AuthoringMesh@1 arrays are canonical, not merely referential: loop IDs
    # must be globally lexicographic even though each Face preserves its own
    # ordered loop_ids ring. Sorting the records does not change winding.
    low_loops.sort(key=lambda value: value["element_id"])
    authoring_mesh = {
        "shape": "authoring-mesh",
        "topology_policy": "triangle-quad-manifold-with-boundary@1",
        "vertices": [{"element_id": value, "position_m": vertex_positions[value]} for value in used_vertices],
        # The Worker contract requires the edge array to be ordered by its
        # emitted element IDs, not by the source vertex pair used to derive it.
        "edges": [
            {"element_id": edge_ids[key], "vertex_ids": list(key)}
            for key in sorted(edge_keys, key=lambda value: edge_ids[value])
        ],
        "loops": low_loops,
        "faces": low_faces,
        "position_m": [0.0, 0.0, 0.0],
        "rotation_rad": [0.0, 0.0, 0.0],
    }
    source_lineage = {
        "source_high_artifact_sha256": require_sha256(source_high_artifact_sha256, "Low source High artifact SHA"),
        "source_high_artifact_readback_sha256": require_sha256(source_high_artifact_readback_sha256, "Low source High readback SHA"),
        "source_high_part_id": part_id,
        "source_high_node_id": node_ids[0],
        "source_high_material_zone_id": output.get("material_zone_id"),
    }
    require(isinstance(source_lineage["source_high_material_zone_id"], str), f"Low V2 Part {part_id} material zone is missing")
    return {
        "schema_version": "LowQuadRetopologyDraft@1",
        "source_lineage": source_lineage,
        "authoring_mesh": authoring_mesh,
    }, {
        "source_triangle_count": len(face_vertices),
        "paired_triangle_count": len(paired) * 2,
        "quad_face_count": len(paired),
        "coverage_ratio": len(used) / len(face_vertices),
        "vertex_count": len(used_vertices),
        "edge_count": len(edge_keys),
    }


def low_quad_prepare_request(
    project_id: str,
    candidate_id: str,
    candidate_state_sha256: str,
    high: dict[str, Any],
    draft: dict[str, Any],
    idempotency_key: str,
) -> dict[str, Any]:
    lineage = draft["source_lineage"]
    worker: dict[str, Any] = {
        "schema_version": "LowQuadDraftWorkerRequest@1",
        "preview_only": True,
        "project_id": project_id,
        "source_high_artifact_sha256": high["high_artifact_sha256"],
        "source_high_artifact_readback_sha256": high["high_artifact_readback_sha256"],
        "source_high_part_id": lineage["source_high_part_id"],
        "source_high_node_id": lineage["source_high_node_id"],
        "source_high_material_zone_id": lineage["source_high_material_zone_id"],
        "draft": draft,
        "max_vertices": 8192,
        "max_edges": 8192,
        "max_faces": 8192,
        "low_retopology_policy": "explicit-artist-editable-quad-draft-source-only@1",
        "algorithm": "deterministic-explicit-quad-compile-edge-flow@1",
        "canonical_sha256": "",
    }
    worker["canonical_sha256"] = canonical_hash_without_field(worker, "canonical_sha256")
    value: dict[str, Any] = {
        "schema_version": "LowQuadDraftDurablePrepareRequest@1",
        "project_id": project_id,
        "candidate_id": candidate_id,
        "candidate_state_sha256": candidate_state_sha256,
        "base_version_id": None,
        "source_high_artifact_id": high["high_artifact_id"],
        "source_high_artifact_object_sha256": high["high_artifact_object_sha256"],
        "source_high_artifact_sha256": high["high_artifact_sha256"],
        "source_high_artifact_readback_object_sha256": high["high_artifact_readback_object_sha256"],
        "source_high_artifact_readback_sha256": high["high_artifact_readback_sha256"],
        "low_quad_draft_worker_request": worker,
        "low_quad_draft_worker_request_sha256": worker["canonical_sha256"],
        "idempotency_key": idempotency_key,
        "max_response_bytes": MAX_RESPONSE_BYTES,
        "source_only": True,
        "runtime_write_performed": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-canonical-sha256@1",
        "input_sha256": "",
    }
    preimage = copy.deepcopy(value)
    preimage.pop("input_sha256", None)
    preimage.pop("idempotency_key", None)
    value["input_sha256"] = object_sha256(preimage)
    return value


def low_quad_get_request(result: dict[str, Any], request: dict[str, Any]) -> dict[str, Any]:
    value: dict[str, Any] = {
        "schema_version": "LowQuadDraftDurableGetRequest@1",
        "operation": "forgecad.production.low-quad-draft-durable-get@1",
        "project_id": request["project_id"],
        "candidate_id": request["candidate_id"],
        "candidate_state_sha256": request["candidate_state_sha256"],
        "base_version_id": request["base_version_id"],
        "link_id": result["link_id"],
        "link_object_sha256": result["link_object_sha256"],
        "source_high_artifact_id": request["source_high_artifact_id"],
        "source_high_artifact_sha256": request["source_high_artifact_sha256"],
        "worker_result_object_sha256": result["worker_result_object_sha256"],
        "worker_result_sha256": result["worker_result_sha256"],
        "artifact_object_sha256": result["artifact_object_sha256"],
        "artifact_sha256": result["artifact_sha256"],
        "readback_object_sha256": result["readback_object_sha256"],
        "readback_sha256": result["readback_sha256"],
        "idempotency_key": request["idempotency_key"],
        "source_only": True,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "runtime_write_performed": False,
        "persistent_user_data_touched": False,
        "input_sha256": "",
    }
    value["input_sha256"] = canonical_hash(value, "input_sha256")
    return value


def verify_low_quad_result(
    result: dict[str, Any],
    request: dict[str, Any],
    draft: dict[str, Any],
    label: str,
    *,
    expected_replayed: bool,
    expected_write: bool,
) -> dict[str, Any]:
    require(result.get("schema_version") in {"LowQuadDraftDurablePrepareResult@1", "LowQuadDraftDurableGetResult@1"}, f"{label} schema drifted")
    require(result.get("project_id") == request["project_id"] and result.get("candidate_id") == request["candidate_id"], f"{label} candidate binding drifted")
    durable_link = result.get("durable_link")
    require(isinstance(durable_link, dict), f"{label} durable link is missing")
    require(
        durable_link.get("source_high_artifact_id")
        == request["source_high_artifact_id"]
        and durable_link.get("source_high_artifact_sha256")
        == request["source_high_artifact_sha256"],
        f"{label} High source binding drifted",
    )
    require(result.get("replayed") is expected_replayed, f"{label} replay marker drifted")
    require(result.get("runtime_write_performed") is expected_write and result.get("persistent_user_data_touched") is expected_write, f"{label} write marker drifted")
    # The operation response cannot know whether this Runtime process was
    # freshly reopened. The probe proves that fact separately by closing and
    # reopening Runtime/MCP before the final exact GET pass.
    require(result.get("restart_hash_verified") is False, f"{label} restart/hash flag must remain false")
    require(result.get("quality_status") == "structural_only" and result.get("edge_flow_status") == "DRAFT_UNREVIEWED", f"{label} quality boundary drifted")
    for field in ("production_stage_advanced", "promotion_eligible", "candidate_confirmed", "version_created", "export_performed"):
        require(result.get(field) is False, f"{label} advanced forbidden state {field}")
    require(
        isinstance(result.get("link_id"), str)
        and bool(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]{0,127}", result["link_id"])),
        f"{label} link_id is not an opaque id",
    )
    for field in ("link_object_sha256", "worker_result_object_sha256", "worker_result_sha256", "artifact_object_sha256", "artifact_sha256", "readback_object_sha256", "readback_sha256"):
        require_sha256(result.get(field), f"{label} {field}")
    return {field: result[field] for field in ("link_id", "link_object_sha256", "worker_result_object_sha256", "worker_result_sha256", "artifact_object_sha256", "artifact_sha256", "readback_object_sha256", "readback_sha256")}


def run_low_quad_durable(
    client: McpClient,
    project_id: str,
    correction_program: dict[str, Any],
    correction_high: dict[str, Any],
    candidate_id: str,
    candidate_state_sha256: str,
) -> dict[str, Any]:
    high = correction_high["high_artifact"]
    expected_parts = ["blade-body", "cutting-edge"]
    require(sorted(high.get("part_ids", [])) == expected_parts, "BLOCKED_MULTI_PART_LOW: corrected V2 High did not expose both required Parts")
    runs: dict[str, Any] = {}
    for part_id in expected_parts:
        draft, counts = low_quad_draft_from_v2_program(
            correction_program,
            part_id,
            high["high_artifact_sha256"],
            high["high_artifact_readback_sha256"],
        )
        require(counts.get("coverage_ratio") == 1.0, f"BLOCKED_LOW_INCOMPLETE_COVERAGE: Part {part_id} coverage is not 1.0")
        request = low_quad_prepare_request(
            project_id,
            candidate_id,
            candidate_state_sha256,
            high,
            draft,
            f"{project_id}-dragonfang-low-{part_id}",
        )
        prepared = facade_tool(client, "surface_pipeline", "low_quad_draft_durable_prepare", request)
        prepared_ids = verify_low_quad_result(prepared, request, draft, f"Dragonfang Low {part_id} prepare", expected_replayed=False, expected_write=True)
        replay = facade_tool(client, "surface_pipeline", "low_quad_draft_durable_prepare", request)
        verify_low_quad_result(replay, request, draft, f"Dragonfang Low {part_id} replay", expected_replayed=True, expected_write=True)
        require(replay.get("durable_link") == prepared.get("durable_link"), f"Dragonfang Low {part_id} replay changed durable link")
        get_request = low_quad_get_request(prepared, request)
        found = facade_tool(client, "surface_pipeline", "low_quad_draft_durable_get", get_request)
        verify_low_quad_result(found, request, draft, f"Dragonfang Low {part_id} get", expected_replayed=False, expected_write=False)
        require(found.get("durable_link") == prepared.get("durable_link"), f"Dragonfang Low {part_id} get changed durable link")
        runs[part_id] = {
            "source_lineage": draft["source_lineage"],
            "draft_sha256": object_sha256(draft),
            "draft_counts": counts,
            "prepare_request_input_sha256": request["input_sha256"],
            "prepare": prepared_ids,
            "prepare_status": "PASS_PREPARED",
            "replay_status": "PASS_EXACT_REPLAY",
            "get_status": "PASS_FOUND",
            "get_request_input_sha256": get_request["input_sha256"],
            "get_request": get_request,
        }
    return {
        "source_high_artifact_id": high["high_artifact_id"],
        "source_high_artifact_sha256": high["high_artifact_sha256"],
        "source_high_artifact_readback_sha256": high["high_artifact_readback_sha256"],
        "source_high_artifact_readback_object_sha256": high["high_artifact_readback_object_sha256"],
        "component_set": expected_parts,
        "components": runs,
        "quality_status": "structural_only",
        "visual_status": "NOT_RUN",
        "human_status": "NOT_RUN",
        "engine_status": "NOT_RUN",
        "commercial_status": "NOT_PROVEN",
    }


KNIFE_UV_BAKE_V2_VISIBILITY_WEIGHT_POLICY = "dragonfang-fps-visible-blade-components@1"
KNIFE_UV_BAKE_V2_MAP_NAMES = (
    "normal",
    "ao",
    "curvature",
    "thickness",
    "position",
    "object-id",
    "material-id",
    "part-id",
)


def knife_uv_bake_v2_prepare_request(
    project_id: str,
    candidate_id: str,
    candidate_state_sha256: str,
    high_run: dict[str, Any],
    low_quad_evidence: dict[str, Any],
) -> dict[str, Any]:
    """Build the closed aggregate request from one corrected High and two Low rows.

    The four direct Worker High identities come from the High Bridge summary;
    the durable GLB and its readback come from the separate High Artifact
    summary.  This is deliberately explicit because those identity domains
    must not be inferred from similarly named fields.
    """
    high = high_run.get("high_artifact")
    bridge = high_run.get("bridge")
    require(isinstance(high, dict) and isinstance(bridge, dict), "UV/Bake V2 corrected High lineage is incomplete")
    required_high = (
        "high_artifact_id", "high_artifact_sha256", "high_artifact_object_sha256",
        "high_artifact_readback_sha256", "high_artifact_readback_object_sha256",
    )
    required_direct = (
        "high_result_sha256", "high_result_object_sha256",
        "high_readback_sha256", "high_readback_object_sha256",
    )
    for field in required_high:
        require_sha256(high[field], f"UV/Bake V2 High {field}") if field != "high_artifact_id" else require(
            isinstance(high[field], str) and bool(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]{0,127}", high[field])),
            "UV/Bake V2 High artifact id is invalid",
        )
    for field in required_direct:
        require_sha256(bridge.get(field), f"UV/Bake V2 direct High {field}")
    source_components: list[dict[str, Any]] = []
    component_set = ["blade-body", "cutting-edge"]
    require(low_quad_evidence.get("component_set") == component_set, "UV/Bake V2 requires the corrected two-Part Low set")
    for part_id in component_set:
        low_run = low_quad_evidence.get("components", {}).get(part_id)
        require(isinstance(low_run, dict), f"UV/Bake V2 Low row is missing for {part_id}")
        lineage = low_run.get("source_lineage")
        prepared = low_run.get("prepare")
        require(isinstance(lineage, dict) and isinstance(prepared, dict), f"UV/Bake V2 Low lineage is incomplete for {part_id}")
        for field in ("source_high_part_id", "source_high_node_id", "source_high_material_zone_id"):
            require(isinstance(lineage.get(field), str) and lineage[field], f"UV/Bake V2 Low {part_id} {field} is missing")
        for field in (
            "link_id", "artifact_object_sha256", "artifact_sha256",
            "readback_object_sha256", "readback_sha256",
        ):
            require_sha256(prepared[field], f"UV/Bake V2 Low {part_id} {field}") if field != "link_id" else require(
                isinstance(prepared[field], str) and bool(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]{0,127}", prepared[field])),
                f"UV/Bake V2 Low {part_id} link_id is invalid",
            )
        # These are a closed policy declaration from the frozen FPS brief and
        # Part scope, not pixel measurements.  The receipt records that fact.
        source_components.append({
            "part_id": part_id,
            "material_zone_id": lineage["source_high_material_zone_id"],
            "source_high_part_id": lineage["source_high_part_id"],
            "source_high_node_id": lineage["source_high_node_id"],
            "source_high_material_zone_id": lineage["source_high_material_zone_id"],
            "low_link_id": prepared["link_id"],
            "low_artifact_object_sha256": prepared["artifact_object_sha256"],
            "low_artifact_sha256": prepared["artifact_sha256"],
            "low_readback_object_sha256": prepared["readback_object_sha256"],
            "low_readback_sha256": prepared["readback_sha256"],
            "visibility_weights": [{"part_id": part_id, "first_person": 1.0, "world": 0.8, "hidden": 0.0}],
            "hero_uv_idempotency_key": f"{project_id}-dragonfang-hero-uv-{part_id}",
        })
    request: dict[str, Any] = {
        "schema_version": "WeaponryKnifeUvBakeV2PrepareRequest@1",
        "operation": "production_knife_uv_bake_v2_prepare",
        "project_id": project_id,
        "candidate_id": candidate_id,
        "candidate_state_sha256": candidate_state_sha256,
        "base_version_id": None,
        "source_high_artifact_id": high["high_artifact_id"],
        "source_high_result_sha256": bridge["high_result_sha256"],
        "source_high_result_object_sha256": bridge["high_result_object_sha256"],
        "source_high_readback_sha256": bridge["high_readback_sha256"],
        "source_high_readback_object_sha256": bridge["high_readback_object_sha256"],
        "source_high_artifact_sha256": high["high_artifact_sha256"],
        "source_high_artifact_object_sha256": high["high_artifact_object_sha256"],
        "source_high_artifact_readback_sha256": high["high_artifact_readback_sha256"],
        "source_high_artifact_readback_object_sha256": high["high_artifact_readback_object_sha256"],
        "components": source_components,
        "idempotency_key": f"{project_id}-dragonfang-uv-bake-v2",
        "source_only": True,
        "runtime_write_performed": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-canonical-sha256@1",
        "input_sha256": "",
    }
    preimage = copy.deepcopy(request)
    preimage.pop("input_sha256", None)
    preimage.pop("idempotency_key", None)
    request["input_sha256"] = object_sha256(preimage)
    return request


def knife_uv_bake_v2_get_request(
    request: dict[str, Any], result: dict[str, Any]
) -> dict[str, Any]:
    record = result.get("record")
    require(isinstance(record, dict), "UV/Bake V2 prepare omitted its aggregate record")
    value: dict[str, Any] = {
        "schema_version": "WeaponryKnifeUvBakeV2GetRequest@1",
        "operation": "production_knife_uv_bake_v2_get",
        "project_id": request["project_id"],
        "candidate_id": request["candidate_id"],
        "candidate_state_sha256": request["candidate_state_sha256"],
        "aggregate_id": record["aggregate_id"],
        "idempotency_key": request["idempotency_key"],
        "source_only": True,
        "runtime_write_performed": False,
        "persistent_user_data_touched": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "input_sha256": "",
    }
    value["input_sha256"] = canonical_hash(value, "input_sha256")
    return value


def verify_knife_uv_bake_v2_result(
    result: dict[str, Any],
    request: dict[str, Any],
    label: str,
    *,
    expected_replayed: bool,
    expected_write: bool,
    expected_restart: bool,
) -> dict[str, Any]:
    require(result.get("schema_version") in {"WeaponryKnifeUvBakeV2PrepareResult@1", "WeaponryKnifeUvBakeV2GetResult@1"}, f"{label} schema drifted")
    require(result.get("operation") in {"production_knife_uv_bake_v2_prepare", "production_knife_uv_bake_v2_get"}, f"{label} operation drifted")
    require(result.get("replayed") is expected_replayed, f"{label} replay marker drifted")
    require(result.get("restart_hash_verified") is expected_restart, f"{label} restart marker drifted")
    require(result.get("runtime_write_performed") is expected_write and result.get("persistent_user_data_touched") is expected_write, f"{label} write marker drifted")
    for field in ("production_stage_advanced", "candidate_confirmed", "version_created", "export_performed"):
        require(result.get(field) is False, f"{label} advanced forbidden state {field}")
    require(result.get("source_only") is True, f"{label} source_only boundary drifted")
    verify_canonical_object(result, "canonical_sha256", label)
    record = result.get("record")
    require(isinstance(record, dict), f"{label} aggregate record is missing")
    require(record.get("project_id") == request["project_id"] and record.get("candidate_id") == request["candidate_id"], f"{label} candidate binding drifted")
    require(record.get("candidate_state_sha256") == request["candidate_state_sha256"], f"{label} candidate state drifted")
    require(record.get("quality_status") == "structural_only" and record.get("visual_status") == "NOT_PROVEN", f"{label} quality boundary drifted")
    require(record.get("uv_status") == "PASS_SOURCE_STRUCTURAL" and record.get("cage_status") == "PASS_SOURCE_STRUCTURAL" and record.get("bake_status") == "PASS_SOURCE_STRUCTURAL", f"{label} structural statuses drifted")
    components = record.get("components")
    require(isinstance(components, list) and len(components) == 2, f"{label} did not preserve the two-Part component set")
    by_part: dict[str, Any] = {}
    for component in components:
        require(isinstance(component, dict), f"{label} component is not an object")
        part_id = component.get("part_id")
        require(part_id in {"blade-body", "cutting-edge"} and part_id not in by_part, f"{label} Part inventory drifted")
        for field in ("low_artifact_object_sha256", "low_artifact_sha256", "low_readback_object_sha256", "low_readback_sha256"):
            require_sha256(component.get(field), f"{label} {part_id} {field}")
        for field in ("hero_uv_link_object_sha256", "hero_uv_layout_object_sha256", "cage_artifact_object_sha256", "cage_readback_object_sha256", "bake_worker_result_object_sha256"):
            require_sha256(component.get(field), f"{label} {part_id} {field}")
        maps = component.get("bake_output_object_sha256s")
        require(isinstance(maps, list) and len(maps) == len(KNIFE_UV_BAKE_V2_MAP_NAMES), f"{label} {part_id} bake map inventory drifted")
        for map_name, map_hash in zip(KNIFE_UV_BAKE_V2_MAP_NAMES, maps):
            require_sha256(map_hash, f"{label} {part_id} {map_name} CAS hash")
        by_part[part_id] = component
    require(set(by_part) == {"blade-body", "cutting-edge"}, f"{label} component set is incomplete")
    return {
        "aggregate_id": record["aggregate_id"],
        "record_canonical_sha256": require_sha256(record.get("canonical_sha256"), f"{label} record canonical_sha256"),
        "components": {
            part_id: {
                "hero_uv_link_object_sha256": component["hero_uv_link_object_sha256"],
                "hero_uv_layout_object_sha256": component["hero_uv_layout_object_sha256"],
                "cage_artifact_object_sha256": component["cage_artifact_object_sha256"],
                "cage_readback_object_sha256": component["cage_readback_object_sha256"],
                "normal_cas_sha256": component["bake_output_object_sha256s"][0],
                "ao_cas_sha256": component["bake_output_object_sha256s"][1],
                "curvature_cas_sha256": component["bake_output_object_sha256s"][2],
                "bake_output_object_sha256s": list(component["bake_output_object_sha256s"]),
                "uv_status": component["uv_status"],
                "cage_status": component["cage_status"],
                "bake_status": component["bake_status"],
            }
            for part_id, component in by_part.items()
        },
    }


def run_knife_uv_bake_v2(
    client: McpClient,
    project_id: str,
    corrected_candidate_id: str,
    corrected_candidate_state_sha256: str,
    correction_high: dict[str, Any],
    low_quad_evidence: dict[str, Any],
) -> dict[str, Any]:
    request = knife_uv_bake_v2_prepare_request(
        project_id,
        corrected_candidate_id,
        corrected_candidate_state_sha256,
        correction_high,
        low_quad_evidence,
    )
    prepared = facade_tool(client, "surface_pipeline", "production_knife_uv_bake_v2_prepare", request)
    prepared_summary = verify_knife_uv_bake_v2_result(
        prepared, request, "Dragonfang UV/Bake V2 prepare",
        expected_replayed=False, expected_write=True, expected_restart=False,
    )
    replay = facade_tool(client, "surface_pipeline", "production_knife_uv_bake_v2_prepare", request)
    replay_summary = verify_knife_uv_bake_v2_result(
        replay, request, "Dragonfang UV/Bake V2 replay",
        expected_replayed=True, expected_write=False, expected_restart=False,
    )
    require(replay_summary["aggregate_id"] == prepared_summary["aggregate_id"], "Dragonfang UV/Bake V2 replay changed aggregate identity")
    get_request = knife_uv_bake_v2_get_request(request, prepared)
    found = facade_tool(client, "surface_pipeline", "production_knife_uv_bake_v2_get", get_request)
    get_summary = verify_knife_uv_bake_v2_result(
        found, request, "Dragonfang UV/Bake V2 get",
        expected_replayed=False, expected_write=False, expected_restart=False,
    )
    require(get_summary["aggregate_id"] == prepared_summary["aggregate_id"], "Dragonfang UV/Bake V2 get changed aggregate identity")
    return {
        "operation": "production_knife_uv_bake_v2",
        "request": request,
        "prepare": prepared_summary,
        "replay": replay_summary,
        "get": get_summary,
        "get_request": get_request,
        "visibility_weight_policy": KNIFE_UV_BAKE_V2_VISIBILITY_WEIGHT_POLICY,
        "visibility_weight_provenance": "derived_from_frozen_brief_part_scope_not_visual_measurement",
        "quality_status": "structural_only",
        "visual_status": "NOT_PROVEN",
        "human_status": "NOT_RUN",
        "engine_status": "NOT_RUN",
        "commercial_status": "NOT_RUN",
        "restart_status": "NOT_RUN",
    }


def high_artifact_reference_compare_request(
    artifact_result: dict[str, Any],
    bridge_result: dict[str, Any],
    reference: dict[str, Any],
    view_spec: dict[str, Any],
    *,
    camera: dict[str, Any] | None = None,
    target_sha256: str | None = None,
) -> dict[str, Any]:
    """Build the closed High selector from Runtime-owned artifact/bridge identity."""
    bridge = bridge_result
    require(
        isinstance(bridge_result.get("bridge"), dict),
        "High comparison bridge record is missing",
    )
    identity_fields = (
        "high_artifact_id", "high_artifact_sha256", "high_artifact_object_sha256",
        "high_artifact_readback_sha256", "high_artifact_readback_object_sha256",
        "high_artifact_receipt_sha256", "high_artifact_receipt_object_sha256",
    )
    for field in identity_fields:
        require(field in artifact_result, f"High comparison artifact identity omitted {field}")
    require(
        isinstance(artifact_result["high_artifact_id"], str)
        and bool(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]{0,127}", artifact_result["high_artifact_id"])),
        "High comparison high_artifact_id is invalid",
    )
    for field in identity_fields[1:]:
        require_sha256(artifact_result[field], f"High comparison {field}")
    bridge_fields = (
        "bridge_id", "bridge_sha256", "bridge_object_sha256", "revision_id",
        "revision_sha256", "revision_object_sha256", "high_result_sha256",
        "high_result_object_sha256", "high_readback_sha256", "high_readback_object_sha256",
        "high_worker_algorithm_sha256", "high_worker_build_cohort_sha256",
    )
    for field in bridge_fields:
        require(field in bridge, f"High comparison bridge identity omitted {field}")
    for field in ("bridge_id", "revision_id"):
        require(
            isinstance(bridge[field], str)
            and bool(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]{0,127}", bridge[field])),
            f"High comparison bridge {field} is invalid",
        )
    for field in bridge_fields:
        if field in {"bridge_id", "revision_id"}:
            continue
        require_sha256(bridge[field], f"High comparison bridge {field}")
    require(isinstance(view_spec, dict), "High comparison view_spec is missing")
    verify_canonical_object(view_spec, "canonical_sha256", "High comparison ReferenceViewSpec")
    require(view_spec.get("reference_id") == reference.get("reference_id"), "High comparison view reference drifted")
    require(isinstance(view_spec.get("view_id"), str), "High comparison view_id is missing")
    value: dict[str, Any] = {
        "project_id": artifact_result["project_id"],
        **{field: artifact_result[field] for field in identity_fields},
        "high_bridge_id": bridge["bridge_id"],
        "high_bridge_sha256": bridge["bridge_sha256"],
        "high_bridge_object_sha256": bridge["bridge_object_sha256"],
        "revision_id": bridge["revision_id"],
        "revision_sha256": bridge["revision_sha256"],
        "revision_object_sha256": bridge["revision_object_sha256"],
        "high_result_sha256": bridge["high_result_sha256"],
        "high_result_object_sha256": bridge["high_result_object_sha256"],
        "high_readback_sha256": bridge["high_readback_sha256"],
        "high_readback_object_sha256": bridge["high_readback_object_sha256"],
        "high_worker_algorithm_sha256": bridge["high_worker_algorithm_sha256"],
        "high_worker_build_cohort_sha256": bridge["high_worker_build_cohort_sha256"],
        "reference_id": reference["reference_id"],
        "view_spec": view_spec,
    }
    if camera is not None:
        value["camera"] = camera
    if target_sha256 is not None:
        value["target_sha256"] = require_sha256(target_sha256, "High comparison target_sha256")
    return value


def default_high_artifact_reference_view_spec(reference: dict[str, Any]) -> dict[str, Any]:
    """Build the fixed front selector without assuming the legacy reference hash."""
    reference_id = reference.get("reference_id")
    reference_sha256 = reference.get("object_sha256")
    width = reference.get("width")
    height = reference.get("height")
    require(isinstance(reference_id, str) and isinstance(reference_sha256, str), "High comparison reference identity is incomplete")
    require(SHA256.fullmatch(reference_sha256) is not None, "High comparison reference hash is invalid")
    require(isinstance(width, int) and isinstance(height, int) and width > 0 and height > 0, "High comparison reference dimensions are invalid")
    if width == DRAGONFANG_FRONT_REFERENCE_WIDTH and height == DRAGONFANG_FRONT_REFERENCE_HEIGHT:
        crop = _dragonfang_front_normalized_crop(reference_sha256)
    else:
        crop = {"x": 0.0, "y": 0.0, "width": 1.0, "height": 1.0}
    value: dict[str, Any] = {
        "schema_version": "ReferenceViewSpec@1",
        "reference_id": reference_id,
        "reference_sha256": reference_sha256,
        "view_id": "view-front",
        "source_view": "front",
        "image": {
            "width": width,
            "height": height,
            "rotation_degrees": 0.0,
            "crop": crop,
        },
        "landmarks": [],
        "regions": [
            {
                "region_id": "dragonfang-front-panel",
                "x": crop["x"],
                "y": crop["y"],
                "width": crop["width"],
                "height": crop["height"],
                "visibility": "observed",
                "confidence": 1.0,
            }
        ],
        "canonical_sha256": "",
    }
    value["canonical_sha256"] = canonical_hash(value, "canonical_sha256")
    return value


def dragonfang_high_five_view_specs(reference: dict[str, Any]) -> list[dict[str, Any]]:
    """Return the five immutable sheet crops used by the V2 blade review.

    The crops are bound to the authorized generated sheet hash.  They are not
    inferred on each run, so a correction cannot improve its score by moving
    the target frame or changing handedness.
    """
    require(
        reference.get("object_sha256")
        == DRAGONFANG_GENERATED_MULTIVIEW_REFERENCE_OBJECT_SHA256,
        "Dragonfang five-view review requires the authorized generated multiview sheet",
    )
    require(
        reference.get("width") == 1536 and reference.get("height") == 1024,
        "Dragonfang five-view sheet dimensions drifted",
    )
    crops = {
        "front": {"x": 15, "y": 20, "width": 880, "height": 180},
        "top": {"x": 925, "y": 35, "width": 595, "height": 150},
        "bottom": {"x": 925, "y": 220, "width": 595, "height": 150},
        "left": {"x": 15, "y": 395, "width": 745, "height": 100},
        "right": {"x": 15, "y": 505, "width": 745, "height": 105},
    }
    values: list[dict[str, Any]] = []
    for source_view in ("front", "top", "bottom", "left", "right"):
        pixels = crops[source_view]
        crop = {
            "x": pixels["x"] / 1536.0,
            "y": pixels["y"] / 1024.0,
            "width": pixels["width"] / 1536.0,
            "height": pixels["height"] / 1024.0,
        }
        value: dict[str, Any] = {
            "schema_version": "ReferenceViewSpec@1",
            "reference_id": reference["reference_id"],
            "reference_sha256": reference["object_sha256"],
            "view_id": f"view-{source_view}",
            "source_view": source_view,
            "image": {
                "width": 1536,
                "height": 1024,
                "rotation_degrees": 0.0,
                "crop": crop,
            },
            "landmarks": [],
            "regions": [
                {
                    "region_id": f"dragonfang-{source_view}-panel",
                    **crop,
                    "visibility": "observed",
                    "confidence": 1.0,
                }
            ],
            "canonical_sha256": "",
        }
        value["canonical_sha256"] = canonical_hash(value, "canonical_sha256")
        values.append(value)
    return values


def verify_high_artifact_reference_compare_result(
    result: dict[str, Any],
    project_id: str,
    artifact_result: dict[str, Any],
    bridge_result: dict[str, Any],
    reference: dict[str, Any],
    view_spec: dict[str, Any],
    expected_build_cohort: str,
    label: str,
) -> dict[str, Any]:
    """Verify one direct High comparison and expose only hash-bound identity."""
    expected_fields = {
        "schema_version", "project_id", "high_artifact_id", "high_artifact_sha256",
        "high_artifact_object_sha256", "high_artifact_readback_sha256",
        "high_artifact_readback_object_sha256", "high_artifact_receipt_sha256",
        "high_artifact_receipt_object_sha256", "reference_id", "view_id", "camera",
        "camera_object_sha256", "render_set", "render_set_hash", "render_set_object_sha256",
        "comparison_report", "comparison_report_hash", "comparison_report_object_sha256",
        "high_artifact_status", "visual_status", "human_status", "engine_status",
        "candidate_visual_evidence_projection",
    }
    require(set(result) == expected_fields, f"{label} comparison result field set drifted")
    require(result.get("schema_version") == "HighArtifactReferenceComparisonPrepareResult@1", f"{label} comparison schema drifted")
    require(result.get("project_id") == project_id, f"{label} project binding drifted")
    for field in (
        "high_artifact_id", "high_artifact_sha256", "high_artifact_object_sha256",
        "high_artifact_readback_sha256", "high_artifact_readback_object_sha256",
        "high_artifact_receipt_sha256", "high_artifact_receipt_object_sha256",
    ):
        require(result.get(field) == artifact_result.get(field), f"{label} {field} drifted")
    require(result.get("reference_id") == reference.get("reference_id"), f"{label} reference binding drifted")
    require(result.get("view_id") == view_spec.get("view_id"), f"{label} view binding drifted")
    require(result.get("high_artifact_status") == "PASS_SOURCE_STRUCTURAL", f"{label} structural status drifted")
    require(result.get("human_status") == "NOT_RUN" and result.get("engine_status") == "NOT_RUN", f"{label} review boundary drifted")
    require(result.get("candidate_visual_evidence_projection") == "NOT_UPDATED", f"{label} projection boundary drifted")

    camera = result.get("camera")
    require(isinstance(camera, dict), f"{label} camera is missing")
    verify_canonical_object(camera, "canonical_sha256", f"{label} camera")
    camera_hash = require_sha256(camera.get("camera_hash"), f"{label} camera_hash")
    camera_object_sha256 = require_sha256(result.get("camera_object_sha256"), f"{label} camera_object_sha256")
    require(object_sha256(camera) == camera_object_sha256, f"{label} camera object hash drifted")

    bridge = bridge_result
    require(
        isinstance(bridge_result.get("bridge"), dict),
        f"{label} bridge record is missing",
    )
    render_set = result.get("render_set")
    require(isinstance(render_set, dict), f"{label} RenderSet is missing")
    render_set_canonical = verify_canonical_object(render_set, "canonical_sha256", f"{label} HighArtifactRenderSet")
    render_set_object_sha256 = require_sha256(result.get("render_set_object_sha256"), f"{label} render_set_object_sha256")
    require(object_sha256(render_set) == render_set_object_sha256, f"{label} RenderSet object hash drifted")
    require(result.get("render_set_hash") == render_set_object_sha256, f"{label} RenderSet hash drifted")
    require(render_set.get("schema_version") == "HighArtifactRenderSet@1", f"{label} RenderSet schema drifted")
    require(render_set.get("project_id") == project_id, f"{label} RenderSet project drifted")
    require(render_set.get("reference_id") == reference.get("reference_id"), f"{label} RenderSet reference drifted")
    require(render_set.get("view_id") == view_spec.get("view_id"), f"{label} RenderSet view drifted")
    require(render_set.get("camera_hash") == camera_hash, f"{label} RenderSet camera drifted")
    require(render_set.get("render_worker_binding_status") == "same_cohort_verified", f"{label} RenderSet worker binding drifted")
    require(render_set.get("render_worker_build_cohort_sha256") == expected_build_cohort, f"{label} RenderSet worker cohort drifted")
    require(render_set.get("width") == 512 and render_set.get("height") == 512, f"{label} RenderSet dimensions drifted")
    require(render_set.get("passes") == list(DRAGONFANG_VISUAL_AOV_PASSES), f"{label} RenderSet AOV order drifted")
    expected_artifact_identity = {
        "high_artifact_id": artifact_result["high_artifact_id"],
        "high_artifact_sha256": artifact_result["high_artifact_sha256"],
        "high_artifact_object_sha256": artifact_result["high_artifact_object_sha256"],
        "high_artifact_readback_sha256": artifact_result["high_artifact_readback_sha256"],
        "high_artifact_readback_object_sha256": artifact_result["high_artifact_readback_object_sha256"],
        "high_artifact_receipt_sha256": artifact_result["high_artifact_receipt_sha256"],
        "high_artifact_receipt_object_sha256": artifact_result["high_artifact_receipt_object_sha256"],
        "high_bridge_id": bridge["bridge_id"],
        "high_bridge_sha256": bridge["bridge_sha256"],
        "high_bridge_object_sha256": bridge["bridge_object_sha256"],
        "revision_id": bridge["revision_id"],
        "revision_sha256": bridge["revision_sha256"],
        "revision_object_sha256": bridge["revision_object_sha256"],
        "high_result_sha256": bridge["high_result_sha256"],
        "high_result_object_sha256": bridge["high_result_object_sha256"],
        "high_readback_sha256": bridge["high_readback_sha256"],
        "high_readback_object_sha256": bridge["high_readback_object_sha256"],
        "high_worker_algorithm_sha256": bridge["high_worker_algorithm_sha256"],
        "high_worker_build_cohort_sha256": bridge["high_worker_build_cohort_sha256"],
    }
    for field, expected in expected_artifact_identity.items():
        require(render_set.get(field) == expected, f"{label} RenderSet {field} drifted")
    pass_artifacts = render_set.get("pass_artifacts")
    require(isinstance(pass_artifacts, dict) and set(pass_artifacts) == set(DRAGONFANG_VISUAL_AOV_PASSES), f"{label} RenderSet pass artifacts drifted")
    aov_hashes: dict[str, str] = {}
    for pass_name in DRAGONFANG_VISUAL_AOV_PASSES:
        entry = pass_artifacts[pass_name]
        require(isinstance(entry, dict), f"{label} {pass_name} artifact is missing")
        aov_hashes[pass_name] = require_sha256(entry.get("sha256"), f"{label} {pass_name} sha256")
        require(entry.get("mime") == "image/png", f"{label} {pass_name} MIME drifted")
        require(isinstance(entry.get("size_bytes"), int) and entry["size_bytes"] > 0, f"{label} {pass_name} size is invalid")
        require(entry.get("width") == 512 and entry.get("height") == 512, f"{label} {pass_name} dimensions drifted")
        require(entry.get("channels") == "rgba8", f"{label} {pass_name} channels drifted")
        require(entry.get("color_space") == ("srgb" if pass_name == "beauty" else "data"), f"{label} {pass_name} color space drifted")

    comparison = result.get("comparison_report")
    require(isinstance(comparison, dict), f"{label} comparison report is missing")
    comparison_canonical = verify_canonical_object(comparison, "canonical_sha256", f"{label} HighArtifactComparisonReport")
    comparison_object_sha256 = require_sha256(result.get("comparison_report_object_sha256"), f"{label} comparison_report_object_sha256")
    require(object_sha256(comparison) == comparison_object_sha256, f"{label} comparison report object hash drifted")
    require(result.get("comparison_report_hash") == comparison_object_sha256, f"{label} comparison report hash drifted")
    require(comparison.get("schema_version") == "HighArtifactReferenceComparisonReport@1", f"{label} comparison report schema drifted")
    for field, expected in expected_artifact_identity.items():
        require(comparison.get(field) == expected, f"{label} comparison report {field} drifted")
    require(comparison.get("reference_id") == reference.get("reference_id"), f"{label} comparison report reference drifted")
    require(comparison.get("reference_sha256") == reference.get("object_sha256"), f"{label} comparison report reference hash drifted")
    require(comparison.get("render_set_hash") == render_set_object_sha256, f"{label} comparison report RenderSet drifted")
    require(comparison.get("camera_hash") == camera_hash, f"{label} comparison report camera drifted")
    require(comparison.get("benchmark_eligibility") == "DIRECT_HIGH_ARTIFACT_COMPARE", f"{label} benchmark eligibility drifted")
    mask = comparison.get("mask")
    require(isinstance(mask, dict) and mask.get("method") == "direct-reference-mask", f"{label} comparison mask drifted")
    require(mask.get("width") == 512 and mask.get("height") == 512, f"{label} comparison mask dimensions drifted")
    require_sha256(mask.get("sha256"), f"{label} comparison mask sha256")
    metrics = comparison.get("metrics")
    require(isinstance(metrics, dict), f"{label} comparison metrics are missing")
    metric_names = ("silhouette_iou", "boundary_f1_4px", "bbox_edge_error", "centroid_error", "landmark_coverage", "landmark_nme", "region_median_iou", "critical_region_min_iou")
    require(set(metrics) == set(metric_names), f"{label} comparison metric set drifted")
    for metric_name in metric_names:
        metric = metrics[metric_name]
        require(isinstance(metric, (int, float)) and not isinstance(metric, bool) and 0.0 <= float(metric) <= 1.0, f"{label} {metric_name} is outside [0,1]")
    comparison_status = comparison.get("status")
    require(comparison_status in {"PARTIAL_VISIBLE_VIEW_PASS", "QUALITY_TARGET_NOT_MET"}, f"{label} comparison status drifted")
    require(result.get("visual_status") == comparison_status, f"{label} visual status drifted")
    require(comparison.get("limitations") == [
        "candidate_visual_evidence_projection_not_updated",
        "human_visual_review_not_run",
        "commercial_quality_not_proven",
    ], f"{label} comparison limitations drifted")
    return {
        "view_id": view_spec["view_id"],
        "reference_id": reference["reference_id"],
        "camera_hash": camera_hash,
        "camera_object_sha256": camera_object_sha256,
        "render_set_id": None,
        "render_set_sha256": render_set_canonical,
        "render_set_object_sha256": render_set_object_sha256,
        "render_worker_build_cohort_sha256": expected_build_cohort,
        "aov_pass_sha256": aov_hashes,
        "reference_comparison_id": comparison.get("report_id"),
        "reference_comparison_sha256": comparison_canonical,
        "reference_comparison_object_sha256": comparison_object_sha256,
        "comparison_status": comparison_status,
        "quality_status": comparison_status,
        "benchmark_eligibility": comparison["benchmark_eligibility"],
        "metrics": {name: float(metrics[name]) for name in metric_names},
    }


def readback_high_artifact_aovs(
    client: McpClient,
    render_set_hash: str,
    artifact_result: dict[str, Any],
    expected_aov_hashes: dict[str, str],
    label: str,
) -> dict[str, str]:
    """Read each fixed High AOV exactly once without rerunning comparison."""
    readback: dict[str, str] = {}
    for pass_name in DRAGONFANG_VISUAL_AOV_PASSES:
        value = facade_tool(
            client,
            "quality_review",
            "render_pass_get",
            {"render_set_hash": render_set_hash, "pass": pass_name},
        )
        require(
            set(value)
            == {
                "schema_version", "render_set_hash", "high_artifact_id",
                "high_artifact_sha256", "high_artifact_object_sha256",
                "high_artifact_readback_sha256", "high_artifact_readback_object_sha256",
                "high_artifact_receipt_sha256", "high_artifact_receipt_object_sha256",
                "pass", "mime", "width", "height", "sha256", "png_base64",
            },
            f"{label} {pass_name} result field set drifted",
        )
        require(value.get("schema_version") == "RenderPassGet@1", f"{label} {pass_name} schema drifted")
        require(value.get("render_set_hash") == render_set_hash, f"{label} {pass_name} RenderSet binding drifted")
        for field in (
            "high_artifact_id", "high_artifact_sha256", "high_artifact_object_sha256",
            "high_artifact_readback_sha256", "high_artifact_readback_object_sha256",
            "high_artifact_receipt_sha256", "high_artifact_receipt_object_sha256",
        ):
            require(value.get(field) == artifact_result.get(field), f"{label} {pass_name} {field} drifted")
        require(value.get("pass") == pass_name, f"{label} {pass_name} identity drifted")
        require(value.get("mime") == "image/png" and value.get("width") == 512 and value.get("height") == 512, f"{label} {pass_name} metadata drifted")
        actual_hash = require_sha256(value.get("sha256"), f"{label} {pass_name} sha256")
        require(actual_hash == expected_aov_hashes[pass_name], f"{label} {pass_name} hash drifted")
        encoded = value.get("png_base64")
        require(isinstance(encoded, str), f"{label} {pass_name} PNG payload is missing")
        try:
            decoded = base64.b64decode(encoded, validate=True)
        except (ValueError, binascii.Error) as error:
            raise GateFailure(f"{label} {pass_name} PNG payload is invalid") from error
        require(hashlib.sha256(decoded).hexdigest() == actual_hash, f"{label} {pass_name} PNG bytes/hash drifted")
        require(decoded.startswith(b"\x89PNG\r\n\x1a\n"), f"{label} {pass_name} is not PNG bytes")
        readback[pass_name] = actual_hash
    return readback


def run_dragonfang_high_artifact(
    client: McpClient,
    project_id: str,
    source: dict[str, Any],
    source_binding: dict[str, Any],
    materialized: dict[str, Any],
    expected_cohort: str,
    label: str,
    suffix: str,
    *,
    reference: dict[str, Any],
    view_spec: dict[str, Any] | None = None,
    view_specs: list[dict[str, Any]] | None = None,
    camera: dict[str, Any] | None = None,
    target_sha256: str | None = None,
) -> dict[str, Any]:
    """Materialize one live High GLB and compare a fixed, bounded view set."""
    candidate = materialized.get("candidate")
    artifact = materialized.get("artifact")
    require(isinstance(candidate, dict) and isinstance(artifact, dict), f"{label} materialization is incomplete")
    candidate_id = candidate.get("candidate_id")
    require(isinstance(candidate_id, str), f"{label} candidate id is missing")
    artifact_readback = facade_tool(
        client,
        "observe",
        "artifact_readback_get",
        {"artifact_id": artifact.get("artifact_id"), "candidate_id": candidate_id},
    )
    verify_artifact_readback(artifact_readback, candidate_id, f"{label} materialized ArtifactReadback")
    observation, _observation_sha, evidence = observe_candidate_geometry(
        client,
        project_id,
        candidate_id,
        candidate["prepared_object_sha256"],
        artifact["program_sha256"],
        f"{label} materialized geometry",
    )
    bridge_request = high_bridge_prepare_request(
        project_id,
        source,
        source_binding,
        materialized,
        evidence,
        artifact_readback,
        f"dragonfang-high-bridge-{suffix}",
        f"{project_id}-dragonfang-high-bridge-{suffix}",
    )
    bridge_result = facade_tool(
        client,
        "authoring_transaction",
        "authoring_mesh_v2_high_bridge_prepare",
        bridge_request,
    )
    bridge_summary = verify_high_bridge_result(
        bridge_result, project_id, expected_cohort, f"{label} High Bridge"
    )
    bridge_replay = facade_tool(
        client,
        "authoring_transaction",
        "authoring_mesh_v2_high_bridge_prepare",
        bridge_request,
    )
    require(
        bridge_replay.get("status") == "replayed"
        and bridge_replay.get("runtime_write_performed") is False
        and bridge_replay.get("bridge_sha256") == bridge_result.get("bridge_sha256"),
        f"{label} High Bridge replay was not side-effect free",
    )
    bridge_get_request = high_bridge_get_request(bridge_result)
    bridge_get = facade_tool(
        client,
        "authoring_transaction",
        "authoring_mesh_v2_high_bridge_get",
        bridge_get_request,
    )
    verify_high_bridge_result(bridge_get, project_id, expected_cohort, f"{label} High Bridge get")

    artifact_request = high_artifact_prepare_request(
        bridge_result,
        f"dragonfang-high-glb-artifact-{suffix}",
        f"{project_id}-dragonfang-high-artifact-{suffix}",
    )
    artifact_result = facade_tool(
        client,
        "surface_pipeline",
        "authoring_mesh_v2_high_artifact_prepare",
        artifact_request,
    )
    artifact_summary = verify_high_artifact_result(
        artifact_result, project_id, expected_cohort, f"{label} High Artifact"
    )
    artifact_replay = facade_tool(
        client,
        "surface_pipeline",
        "authoring_mesh_v2_high_artifact_prepare",
        artifact_request,
    )
    require(
        artifact_replay.get("status") == "replayed"
        and artifact_replay.get("runtime_write_performed") is False
        and artifact_replay.get("high_artifact_sha256") == artifact_result.get("high_artifact_sha256"),
        f"{label} High Artifact replay was not side-effect free",
    )
    artifact_get_request = high_artifact_get_request(artifact_result)
    artifact_get = facade_tool(
        client,
        "surface_pipeline",
        "authoring_mesh_v2_high_artifact_get",
        artifact_get_request,
    )
    artifact_get_summary = verify_high_artifact_result(
        artifact_get, project_id, expected_cohort, f"{label} High Artifact get"
    )
    require(
        artifact_get_summary["high_artifact_sha256"] == artifact_summary["high_artifact_sha256"]
        and artifact_get_summary["high_artifact_readback_sha256"] == artifact_summary["high_artifact_readback_sha256"],
        f"{label} High Artifact exact readback drifted",
    )
    if view_specs is None:
        view_specs = [view_spec or default_high_artifact_reference_view_spec(reference)]
    require(
        1 <= len(view_specs) <= 5
        and len({value.get("source_view") for value in view_specs}) == len(view_specs),
        f"{label} High fixed view set is invalid",
    )
    fixed_view_comparisons: dict[str, dict[str, Any]] = {}
    for fixed_view in view_specs:
        source_view = fixed_view.get("source_view")
        require(isinstance(source_view, str), f"{label} High source_view is missing")
        compare_request = high_artifact_reference_compare_request(
            artifact_result,
            bridge_result,
            reference,
            fixed_view,
            camera=camera if len(view_specs) == 1 else None,
            target_sha256=target_sha256 if len(view_specs) == 1 else None,
        )
        compare_result = facade_tool(
            client,
            "quality_review",
            "high_artifact_reference_compare_prepare",
            compare_request,
        )
        comparison = verify_high_artifact_reference_compare_result(
            compare_result,
            project_id,
            artifact_result,
            bridge_result,
            reference,
            fixed_view,
            expected_cohort,
            f"{label} High {source_view} comparison",
        )
        aov_readback = readback_high_artifact_aovs(
            client,
            comparison["render_set_object_sha256"],
            artifact_result,
            comparison["aov_pass_sha256"],
            f"{label} High {source_view} AOV readback",
        )
        fixed_view_comparisons[source_view] = {
            "compare_request": compare_request,
            "compare": comparison,
            "aov_readback": aov_readback,
        }
    primary_view = "front" if "front" in fixed_view_comparisons else next(iter(fixed_view_comparisons))
    primary = fixed_view_comparisons[primary_view]
    comparison = primary["compare"]
    return {
        "candidate_id": candidate_id,
        "candidate_state_sha256": candidate["canonical_sha256"],
        "materialized_artifact_sha256": candidate["prepared_object_sha256"],
        "materialized_program_sha256": artifact["program_sha256"],
        "geometry_observation_sha256": verify_canonical_object(observation, "canonical_sha256", f"{label} geometry observation"),
        "bridge_request_input_sha256": bridge_request["input_sha256"],
        "bridge": bridge_summary,
        "bridge_replay": "PASS_NOT_TOUCHED",
        "bridge_get": "PASS_FOUND",
        "high_artifact": artifact_summary,
        "high_artifact_replay": "PASS_NOT_TOUCHED",
        "high_artifact_get": "PASS_FOUND",
        "compare_request": primary["compare_request"],
        "compare": comparison,
        "aov_readback": primary["aov_readback"],
        "fixed_view_count": len(fixed_view_comparisons),
        "fixed_view_comparisons": fixed_view_comparisons,
        "visual_status": comparison["comparison_status"],
        "visual_quality_promotion": "NOT_PROMOTED",
    }


DURABLE_RESULT_FIELDS = {
    "schema_version",
    "project_id",
    "mesh_id",
    "lineage_id",
    "revision_id",
    "revision_index",
    "parent_revision_ids",
    "revision_sha256",
    "revision_object_sha256",
    "operation",
    "revision",
    "durable_record",
    "request_input_sha256",
    "idempotency_key",
    "replayed",
    "restart_hash_verified",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "stage_advanced",
    "candidate_confirmed",
    "version_created",
    "export_performed",
    "quality_status",
    "limitations",
    "canonicalization_policy",
    "canonical_sha256",
}


def durable_get_request(result: dict[str, Any]) -> dict[str, Any]:
    """Build the exact read envelope for one Runtime-owned revision."""
    value: dict[str, Any] = {
        "schema_version": "AuthoringMeshV2DurableGetRequest@1",
        "project_id": result["project_id"],
        "mesh_id": result["mesh_id"],
        "revision_id": result["revision_id"],
        "revision_sha256": result["revision_sha256"],
        "revision_object_sha256": result["revision_object_sha256"],
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "runtime_write_performed": False,
        "persistent_user_data_touched": False,
        "input_sha256": "",
    }
    value["input_sha256"] = canonical_hash(value, "input_sha256")
    return value


def verify_durable_result(
    result: dict[str, Any],
    request: dict[str, Any],
    label: str,
    *,
    expected_replayed: bool | None = None,
    expected_runtime_write: bool | None = None,
    expected_operation: str | None = None,
) -> dict[str, Any]:
    """Verify the full durable result and return its canonical revision."""
    require(set(result) == DURABLE_RESULT_FIELDS, f"{label} fields drifted")
    require(
        result.get("schema_version") == "AuthoringMeshV2DurableResult@1",
        f"{label} schema drifted",
    )
    verify_canonical_object(result, "canonical_sha256", label)
    require(
        result.get("request_input_sha256") == request.get("input_sha256"),
        f"{label} request input hash drifted",
    )
    for field in ("project_id", "mesh_id"):
        require(result.get(field) == request.get(field), f"{label} {field} drifted")
    # Durable prepare requests intentionally do not accept caller-selected
    # child revision IDs or hashes; Runtime derives them. Exact get requests
    # do carry those identities and must bind them here.
    for field in ("revision_id", "revision_sha256", "revision_object_sha256"):
        if field in request:
            require(result.get(field) == request.get(field), f"{label} {field} drifted")
    require(result.get("lineage_id") == request.get("lineage_id", result.get("lineage_id")), f"{label} lineage drifted")
    require(result.get("quality_status") == "structural_only", f"{label} quality status drifted")
    require(result.get("stage_advanced") is False, f"{label} advanced a production stage")
    require(result.get("candidate_confirmed") is False, f"{label} confirmed a candidate")
    require(result.get("version_created") is False, f"{label} created a version")
    require(result.get("export_performed") is False, f"{label} performed an export")
    require(result.get("runtime_write_performed") is expected_runtime_write if expected_runtime_write is not None else True, f"{label} runtime-write flag drifted")
    if expected_replayed is not None:
        require(result.get("replayed") is expected_replayed, f"{label} replay flag drifted")
    if expected_runtime_write is not None:
        require(
            result.get("persistent_user_data_touched") is expected_runtime_write,
            f"{label} persistent-write flag drifted",
        )
    if expected_operation is not None:
        require(result.get("operation") == expected_operation, f"{label} operation drifted")
    revision = result.get("revision")
    require(isinstance(revision, dict), f"{label} omitted AuthoringMeshRevision")
    verify_canonical_object(revision, "canonical_sha256", f"{label}.revision")
    require(revision.get("schema_version") == "AuthoringMeshRevision@2", f"{label} revision schema drifted")
    require(revision.get("mesh_id") == result.get("mesh_id"), f"{label} revision mesh drifted")
    require(revision.get("lineage_id") == result.get("lineage_id"), f"{label} revision lineage drifted")
    require(revision.get("revision_id") == result.get("revision_id"), f"{label} revision id drifted")
    require(revision.get("revision_index") == result.get("revision_index"), f"{label} revision index drifted")
    require(revision.get("parent_revision_ids") == result.get("parent_revision_ids"), f"{label} parent chain drifted")
    require(revision.get("canonical_sha256") == result.get("revision_sha256"), f"{label} revision semantic hash drifted")
    require(object_sha256(revision) == result.get("revision_object_sha256"), f"{label} revision object hash drifted")
    record = result.get("durable_record")
    require(isinstance(record, dict), f"{label} omitted durable Store record")
    verify_canonical_object(record, "canonical_sha256", f"{label}.durable_record")
    for field in (
        "project_id",
        "mesh_id",
        "lineage_id",
        "revision_id",
        "revision_index",
        "parent_revision_ids",
        "revision_sha256",
        "revision_object_sha256",
    ):
        require(record.get(field) == result.get(field), f"{label} durable record {field} drifted")
    require(
        result.get("canonicalization_policy")
        == "canonical-json-sha256-excluding-canonical-sha256@1",
        f"{label} canonicalization policy drifted",
    )
    return revision


def select_dragonfang_correction_vertices(
    revision: dict[str, Any],
) -> tuple[list[str], list[list[float]], str]:
    """Select a deterministic, bounded blade-spine contour correction."""
    require(revision.get("schema_version") == "AuthoringMeshRevision@2", "correction revision schema drifted")
    source_binding = revision.get("source_binding")
    require(isinstance(source_binding, dict), "correction revision lost its source binding")
    require(source_binding.get("part_id") == "blade-body", "correction is not scoped to blade-body")
    original = revision.get("original")
    vertices = original.get("vertices") if isinstance(original, dict) else None
    require(isinstance(vertices, list) and vertices, "correction revision has no authored vertices")
    parsed: list[tuple[str, list[float]]] = []
    for vertex in vertices:
        require(isinstance(vertex, dict), "correction vertex is not an object")
        vertex_id = vertex.get("vertex_id")
        position = vertex.get("position_m")
        require(isinstance(vertex_id, str) and vertex_id, "correction vertex ID is not stable")
        require(
            isinstance(position, list)
            and len(position) == 3
            and all(isinstance(value, (int, float)) and value == value for value in position),
            f"correction vertex {vertex_id} position is invalid",
        )
        require(all(abs(float(value)) <= 10.0 for value in position), f"correction vertex {vertex_id} exceeds bounds")
        parsed.append((vertex_id, [float(value) for value in position]))
    top_y = max(position[1] for _, position in parsed)
    selected = sorted(
        vertex_id
        for vertex_id, position in parsed
        if abs(position[1] - top_y) <= 1.0e-9
    )[:32]
    require(1 <= len(selected) <= 32, "correction vertex selection is outside the 1..32 bound")
    delta = [[0.0, -0.004, 0.0] for _ in selected]
    selected_positions = {vertex_id: position for vertex_id, position in parsed}
    for vertex_id in selected:
        require(
            abs(selected_positions[vertex_id][1] + delta[0][1]) <= 10.0,
            f"correction vertex {vertex_id} would exceed bounds",
        )
    return selected, delta, "lower blade-body upper spine by 4mm for front contour calibration"


def correction_operation_lineage_sha256(
    project_id: str,
    revision: dict[str, Any],
    vertex_ids: list[str],
    delta_m: list[list[float]],
) -> str:
    return object_sha256(
        {
            "schema_version": "DragonfangKnifeCorrectionIntent@1",
            "project_id": project_id,
            "operation": "move_vertices",
            "mesh_id": revision["mesh_id"],
            "lineage_id": revision["lineage_id"],
            "parent_revision_id": revision["revision_id"],
            "vertex_ids": vertex_ids,
            "delta_m": delta_m,
            "rationale": "lower blade-body upper spine by 4mm for front contour calibration",
        }
    )


def durable_move_vertices_request(
    project_id: str,
    revision: dict[str, Any],
    vertex_ids: list[str],
    delta_m: list[list[float]],
    operation_lineage_sha256: str,
) -> dict[str, Any]:
    suffix = project_id[-12:]
    value: dict[str, Any] = {
        "schema_version": "AuthoringMeshV2DurablePrepareRequest@1",
        "project_id": project_id,
        "operation": "move_vertices",
        "mesh_id": revision["mesh_id"],
        "lineage_id": revision["lineage_id"],
        "parent_revision_id": revision["revision_id"],
        "operation_id": f"dragonfang-correction-move-spine-{suffix}",
        "edge_id": None,
        "split_ratio_milli": None,
        "vertex_ids": vertex_ids,
        "delta_m": delta_m,
        "operation_lineage_sha256": operation_lineage_sha256,
        "positions_m": None,
        "faces": None,
        "evaluated": None,
        "idempotency_key": f"{project_id}-dragonfang-correction-move-spine",
        "max_response_bytes": MAX_RESPONSE_BYTES,
        "runtime_write_performed": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-canonical-sha256@1",
        "input_sha256": "",
    }
    value["input_sha256"] = canonical_hash(value, "input_sha256")
    return value


def verify_move_vertices_result(
    result: dict[str, Any],
    request: dict[str, Any],
    parent_revision: dict[str, Any],
    vertex_ids: list[str],
    delta_m: list[list[float]],
    label: str,
) -> dict[str, Any]:
    child = verify_durable_result(
        result,
        request,
        label,
        expected_replayed=False,
        expected_runtime_write=True,
        expected_operation="move_vertices",
    )
    require(child.get("parent_revision_ids") == [parent_revision["revision_id"]], f"{label} parent is not the selected revision")
    require(child.get("revision_index") == parent_revision["revision_index"] + 1, f"{label} revision index is not a direct child")
    require(child.get("evaluated") is None, f"{label} retained a stale evaluated sidecar")
    require(child.get("source_binding") == parent_revision.get("source_binding"), f"{label} source binding drifted")
    operation = child.get("operation")
    require(isinstance(operation, dict), f"{label} operation journal is missing")
    require(operation.get("kind") == "move_vertices", f"{label} operation kind drifted")
    require(operation.get("operation_id") == request["operation_id"], f"{label} operation ID drifted")
    require(operation.get("parent_revision_id") == parent_revision["revision_id"], f"{label} operation parent drifted")
    require(operation.get("operation_lineage_sha256") == request["operation_lineage_sha256"], f"{label} operation lineage hash drifted")
    parent_vertices = {
        vertex.get("vertex_id"): vertex.get("position_m")
        for vertex in parent_revision.get("original", {}).get("vertices", [])
    }
    child_vertices = {
        vertex.get("vertex_id"): vertex.get("position_m")
        for vertex in child.get("original", {}).get("vertices", [])
    }
    require(set(parent_vertices) == set(child_vertices), f"{label} changed the stable vertex set")
    moves = dict(zip(vertex_ids, delta_m))
    for vertex_id, parent_position in parent_vertices.items():
        child_position = child_vertices.get(vertex_id)
        require(isinstance(parent_position, list) and isinstance(child_position, list), f"{label} vertex positions are incomplete")
        expected = [float(parent_position[index]) + moves[vertex_id][index] if vertex_id in moves else float(parent_position[index]) for index in range(3)]
        require(
            all(abs(float(child_position[index]) - expected[index]) <= 1.0e-12 for index in range(3)),
            f"{label} vertex {vertex_id} position drifted",
        )
    return child


def verify_durable_replay(
    result: dict[str, Any],
    request: dict[str, Any],
    expected: dict[str, Any],
    label: str,
) -> None:
    replay_revision = verify_durable_result(
        result,
        request,
        label,
        expected_replayed=True,
        expected_runtime_write=False,
        expected_operation=expected.get("operation"),
    )
    require(result.get("revision_id") == expected.get("revision_id"), f"{label} revision identity changed on replay")
    require(result.get("revision_sha256") == expected.get("revision_sha256"), f"{label} revision hash changed on replay")
    require(result.get("revision_object_sha256") == expected.get("revision_object_sha256"), f"{label} revision object changed on replay")
    require(replay_revision == expected.get("revision"), f"{label} revision payload changed on replay")


def write_receipt(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = copy.deepcopy(value)
    payload["canonical_sha256"] = canonical_hash(payload, "canonical_sha256")
    path.write_text(
        json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--data-root", type=Path, required=True)
    parser.add_argument("--reference", type=Path, required=True)
    parser.add_argument(
        "--reference-profile",
        choices=("legacy", "generated-multiview"),
        default="legacy",
        help=(
            "interpret the authorized reference as the historical front-panel "
            "sheet or as the new generated multi-view Dragonfang sheet"
        ),
    )
    parser.add_argument("--initial-brief", type=Path, required=True)
    parser.add_argument("--successor-brief", type=Path, required=True)
    parser.add_argument(
        "--intent-bundle",
        type=Path,
        help="optional KnifeReferenceIntentBundle fixture to persist and replay after the Brief",
    )
    parser.add_argument(
        "--source-binding",
        action="store_true",
        help="also create the bounded Dragonfang blockout and immutable KnifeSourceBinding",
    )
    parser.add_argument(
        "--materialize",
        action="store_true",
        help=(
            "materialize the SourceBinding-selected AuthoringMesh revision as a "
            "multi-part-preserving structural candidate; requires --source-binding"
        ),
    )
    parser.add_argument(
        "--visual-pass",
        action="store_true",
        help=(
            "run one hash-bound Dragonfang front mask/compare, verify nine AOVs and "
            "restart readback; requires --source-binding"
        ),
    )
    parser.add_argument(
        "--correction-pass",
        action="store_true",
        help=(
            "create one bounded move_vertices AuthoringMesh child, rematerialize it "
            "with the original SourceBinding, compare both initial and corrected "
            "candidates once, and persist root/child PassState; requires "
            "--materialize and --visual-pass"
        ),
    )
    parser.add_argument(
        "--high-artifact",
        action="store_true",
        help=(
            "run the Runtime-owned AuthoringMeshV2 High Bridge and strict GLB "
            "adapter (prepare/replay/get), one fixed-view High comparison and "
            "nine AOV readbacks; requires --materialize"
        ),
    )
    parser.add_argument(
        "--v2-blade",
        action="store_true",
        help=(
            "replace the legacy profile blockout with the Runtime-derived dual-curve, "
            "four-section V2 blade before SourceBinding/materialization/High; requires "
            "--source-binding"
        ),
    )
    parser.add_argument(
        "--low-quad",
        action="store_true",
        help=(
            "materialize two per-Part explicit Low quad drafts from the corrected "
            "V2 High Artifact and verify prepare/replay/get/restart; requires "
            "--v2-blade --materialize --correction-pass --high-artifact"
        ),
    )
    parser.add_argument(
        "--uv-bake-v2",
        action="store_true",
        help=(
            "run the Runtime-owned corrected two-Part UV/Cage/geometric-Bake "
            "aggregate through surface_pipeline; requires --low-quad"
        ),
    )
    parser.add_argument("--receipt", type=Path, required=True)
    parser.add_argument("--expected-build-cohort", required=True)
    parser.add_argument("--timeout", type=float, default=60.0)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    require(args.timeout > 0, "timeout must be positive")
    require(args.mcp.is_file() and args.runtime.is_file(), "MCP/Runtime binaries are unavailable")
    require(SHA256.fullmatch(args.expected_build_cohort) is not None, "invalid expected cohort")
    data_root = args.data_root.absolute()
    require(not data_root.exists(), "isolated data root must not pre-exist")
    data_root.mkdir(mode=0o700, parents=True)
    reference_path = args.reference.absolute()
    require(
        reference_path.is_file() and not reference_path.is_symlink(),
        "reference must be a regular non-symlink file",
    )
    reference_bytes = reference_path.read_bytes()
    require(0 < len(reference_bytes) <= 8 * 1024 * 1024, "reference exceeds Runtime intake budget")
    reference_sha256 = hashlib.sha256(reference_bytes).hexdigest()
    initial_template = load_object(args.initial_brief, "initial brief")
    successor_template = load_object(args.successor_brief, "successor brief")
    intent_template = (
        load_object(args.intent_bundle, "reference intent bundle")
        if args.intent_bundle is not None
        else None
    )
    require(
        not args.source_binding or intent_template is not None,
        "--source-binding requires --intent-bundle",
    )
    require(
        not args.materialize or args.source_binding,
        "--materialize requires --source-binding",
    )
    require(
        not args.visual_pass or args.source_binding,
        "--visual-pass requires --source-binding so a candidate-bound RenderSet exists",
    )
    require(
        not args.correction_pass or args.materialize,
        "--correction-pass requires --materialize",
    )
    require(
        not args.high_artifact or (args.materialize and args.source_binding),
        "--high-artifact requires --source-binding and --materialize",
    )
    require(not args.v2_blade or args.source_binding, "--v2-blade requires --source-binding")
    require(
        not args.v2_blade or args.reference_profile == "generated-multiview",
        "--v2-blade requires --reference-profile generated-multiview for the fixed five-view gate",
    )
    require(
        not args.low_quad
        or (
            args.v2_blade
            and args.materialize
            and args.correction_pass
            and args.high_artifact
        ),
        "--low-quad requires --v2-blade, --materialize, --correction-pass and --high-artifact",
    )
    require(not args.uv_bake_v2 or args.low_quad, "--uv-bake-v2 requires --low-quad")

    identities = {
        "mcp": build_identity(args.mcp),
        "runtime": build_identity(args.runtime),
    }
    require(
        all(
            value.get("build_cohort_sha256") == args.expected_build_cohort
            for value in identities.values()
        ),
        "MCP and Runtime do not match the expected cohort",
    )

    base_environment = os.environ.copy()
    for key in (
        "FORGECAD_RUNTIME_SOCKET",
        "FORGECAD_RUNTIME_TOKEN",
        "FORGECAD_RUNTIME_DATA_DIR",
        "FORGECAD_RUNTIME_COMMAND",
    ):
        base_environment.pop(key, None)

    runtime: subprocess.Popen[str] | None = None
    client: McpClient | None = None
    ready: dict[str, Any] | None = None
    ready_path: Path | None = None
    try:
        runtime, ready_path, ready = start_runtime(
            args.runtime, data_root, base_environment, args.timeout
        )
        client = initialize_client(
            args.mcp, mcp_environment(base_environment, ready), args.timeout
        )
        capabilities = facade_tool(client, "weapon_preflight", "capabilities_get", {})
        require(
            capabilities.get("build_cohort_sha256") == args.expected_build_cohort,
            "live Runtime capability cohort mismatch",
        )
        runtime_status = facade_tool(client, "weapon_preflight", "runtime_status", {})
        require(runtime_status.get("state") == "Ready", "live Runtime is not Ready")
        doctor = facade_tool(client, "weapon_preflight", "doctor", {})
        require(doctor.get("state") == "Ready", "live Runtime doctor is not Ready")

        project = facade_tool(
            client,
            "reference_intake",
            "project_create",
            {"name": "Dragonfang Knife Brief isolated live intake", "policy": {"profile": "knife"}},
        )
        project_id = project.get("project_id")
        require(isinstance(project_id, str) and project_id, "project_create omitted project_id")
        imported = facade_tool(
            client,
            "reference_intake",
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
                    "declaration": "User supplied and authorized this Dragonfang reference for local Weaponry modeling.",
                },
                "expected_sha256": reference_sha256,
            },
        )
        reference = imported.get("reference")
        require(isinstance(reference, dict), "reference_import omitted ReferenceEvidence")
        require(reference.get("object_sha256") == reference_sha256, "reference bytes/hash drifted")
        reread = facade_tool(
            client,
            "reference_intake",
            "reference_get",
            {"reference_id": reference.get("reference_id")},
        )
        reread_reference = reread.get("reference")
        require(isinstance(reread_reference, dict), "reference_get omitted ReferenceEvidence")
        require(
            reread_reference.get("canonical_sha256") == reference.get("canonical_sha256"),
            "reference evidence readback drifted",
        )
        parent_brief = bind_reference(
            initial_template, project_id, reference, args.reference_profile
        )
        parent = facade_tool(
            client,
            "reference_intake",
            "weaponry_knife_production_brief_prepare",
            prepare_request(parent_brief, reference, f"{project_id}-dragonfang-parent"),
        )
        require(
            parent.get("status") == "stored"
            and parent.get("conflict_status") == "conflicted"
            and parent.get("authoring_eligibility") == "BLOCKED",
            "initial Dragonfang Brief did not remain blocked",
        )
        parent_get = facade_tool(
            client,
            "reference_intake",
            "weaponry_knife_production_brief_get",
            get_request(parent),
        )
        require(parent_get.get("status") == "found", "parent Brief readback failed")

        successor_brief = bind_reference(
            successor_template, project_id, reference, args.reference_profile
        )
        bind_confirmed_successor_to_runtime(successor_brief)
        successor_brief["parent_brief_id"] = parent["brief_id"]
        successor_brief["parent_brief_sha256"] = parent["brief_sha256"]
        successor_brief["canonical_sha256"] = canonical_hash(
            successor_brief, "canonical_sha256"
        )
        successor_request = prepare_request(
            successor_brief, reference, f"{project_id}-dragonfang-successor"
        )
        successor = facade_tool(
            client,
            "reference_intake",
            "weaponry_knife_production_brief_prepare",
            successor_request,
        )
        require(
            successor.get("status") == "stored"
            and successor.get("conflict_status") == "resolved"
            and successor.get("authorization_binding_status") == "runtime-bound"
            and successor.get("authoring_eligibility") == "ELIGIBLE",
            "resolved successor did not become authoring eligible",
        )
        for field in (
            "production_stage_advanced",
            "candidate_confirmed",
            "version_created",
            "export_performed",
        ):
            require(successor.get(field) is False, f"Brief unexpectedly changed {field}")
        replay = facade_tool(
            client,
            "reference_intake",
            "weaponry_knife_production_brief_prepare",
            successor_request,
        )
        require(
            replay.get("status") == "replayed"
            and replay.get("replayed") is True
            and replay.get("store_effect") == "not-touched"
            and replay.get("cas_effect") == "not-touched",
            "successor exact replay was not side-effect free",
        )
        successor_get_request = get_request(successor)
        successor_get = facade_tool(
            client,
            "reference_intake",
            "weaponry_knife_production_brief_get",
            successor_get_request,
        )
        require(successor_get.get("status") == "found", "successor readback failed")

        intent_result: dict[str, Any] | None = None
        intent_get: dict[str, Any] | None = None
        blockout_candidate: dict[str, Any] | None = None
        blockout_artifact: dict[str, Any] | None = None
        blockout_artifact_readback: dict[str, Any] | None = None
        source_result: dict[str, Any] | None = None
        source_binding_result: dict[str, Any] | None = None
        source_binding_get: dict[str, Any] | None = None
        materializer_result: dict[str, Any] | None = None
        materializer_request: dict[str, Any] | None = None
        baseline_materializer_result: dict[str, Any] | None = None
        baseline_materializer_request: dict[str, Any] | None = None
        correction_initial_get_request: dict[str, Any] | None = None
        correction_initial_get_result: dict[str, Any] | None = None
        correction_parent_revision: dict[str, Any] | None = None
        correction_payload: dict[str, Any] | None = None
        correction_geometry_hash_result: dict[str, Any] | None = None
        correction_source_candidate: dict[str, Any] | None = None
        correction_source_artifact: dict[str, Any] | None = None
        correction_source_artifact_readback: dict[str, Any] | None = None
        correction_source_result: dict[str, Any] | None = None
        correction_source_binding_result: dict[str, Any] | None = None
        correction_source_binding_get: dict[str, Any] | None = None
        correction_vertex_ids: list[str] = []
        correction_delta_m: list[list[float]] = []
        correction_rationale: str | None = None
        correction_request: dict[str, Any] | None = None
        correction_result: dict[str, Any] | None = None
        correction_replay_result: dict[str, Any] | None = None
        correction_materializer_request: dict[str, Any] | None = None
        correction_materializer_result: dict[str, Any] | None = None
        correction_pass_state_status = "NOT_RUN"
        correction_child_get_request: dict[str, Any] | None = None
        correction_child_get_result: dict[str, Any] | None = None
        blockout_geometry: dict[str, str] | None = None
        blockout_observation_sha256: str | None = None
        visual_mask_result: dict[str, Any] | None = None
        visual_compare_result: dict[str, Any] | None = None
        visual_identity: dict[str, Any] | None = None
        visual_candidate: dict[str, Any] | None = None
        visual_artifact: dict[str, Any] | None = None
        visual_artifact_readback: dict[str, Any] | None = None
        visual_observation: dict[str, Any] | None = None
        visual_bundle: dict[str, Any] | None = None
        visual_aov_restart_readback: dict[str, str] | None = None
        reopened_observation: dict[str, Any] | None = None
        visual_runs: dict[str, dict[str, Any]] = {}
        visual_restart_evidence: dict[str, dict[str, Any]] = {}
        intent_visual_contract: dict[str, Any] | None = None
        high_artifact_runs: dict[str, dict[str, Any]] = {}
        v2_blade_evidence: dict[str, Any] | None = None
        correction_v2_program: dict[str, Any] | None = None
        low_quad_evidence: dict[str, Any] | None = None
        uv_bake_v2_evidence: dict[str, Any] | None = None
        baseline_visual_identity: dict[str, Any] | None = None
        correction_visual_identity: dict[str, Any] | None = None
        pass_state_root_main: dict[str, Any] | None = None
        pass_state_root_request: dict[str, Any] | None = None
        pass_state_root_result: dict[str, Any] | None = None
        pass_state_root_replay: dict[str, Any] | None = None
        pass_state_root_get_request: dict[str, Any] | None = None
        pass_state_root_get_result: dict[str, Any] | None = None
        pass_state_child_main: dict[str, Any] | None = None
        pass_state_child_request: dict[str, Any] | None = None
        pass_state_child_result: dict[str, Any] | None = None
        pass_state_child_replay: dict[str, Any] | None = None
        pass_state_child_get_request: dict[str, Any] | None = None
        pass_state_child_get_result: dict[str, Any] | None = None
        pass_state_restart_get_results: dict[str, dict[str, Any]] = {}
        if intent_template is not None:
            intent_bundle = bind_reference_intent(
                intent_template,
                project_id,
                successor,
                reference,
                args.reference_profile,
            )
            intent_request = intent_prepare_request(
                intent_bundle,
                successor,
                reference,
                f"{project_id}-dragonfang-reference-intent",
            )
            intent_result = facade_tool(
                client,
                "reference_intake",
                "knife_reference_intent_bundle_prepare",
                intent_request,
            )
            intent_visual_contract = verify_dragonfang_intent_visual_contract(
                intent_result.get("intent_bundle"), "Dragonfang live intent bundle"
            )
            require(
                intent_result.get("status") == "stored"
                and intent_result.get("brief_authoring_eligibility") == "ELIGIBLE"
                and intent_result.get("high_stage_unlocked") is False
                and intent_result.get("high_mesh_created") is False,
                "reference intent did not persist as an eligible but High-locked control record",
            )
            intent_replay = facade_tool(
                client,
                "reference_intake",
                "knife_reference_intent_bundle_prepare",
                intent_request,
            )
            require(
                intent_replay.get("status") == "replayed"
                and intent_replay.get("replayed") is True
                and intent_replay.get("store_effect") == "not-touched"
                and intent_replay.get("cas_effect") == "not-touched",
                "reference intent exact replay was not side-effect free",
            )
            intent_get = intent_get_request(intent_result)
            intent_readback = facade_tool(
                client,
                "reference_intake",
                "knife_reference_intent_bundle_get",
                intent_get,
            )
            require(
                intent_readback.get("status") == "found"
                and intent_readback.get("intent_bundle_sha256")
                == intent_result.get("intent_bundle_sha256"),
                "reference intent exact readback failed",
            )

        if args.source_binding:
            require(intent_result is not None, "source binding requires persisted reference intent")
            catalog_sha256 = capabilities.get("operator_catalog_sha256")
            require(
                isinstance(catalog_sha256, str)
                and SHA256.fullmatch(catalog_sha256) is not None,
                "operator catalog hash is unavailable",
            )
            # The bounded Knife profile intentionally does not expose the raw
            # compatibility operator catalog.  geometry_program_hash is the
            # authoritative active-operator and catalog-binding preflight.
            draft = dragonfang_blockout_program(project_id, catalog_sha256)
            hashed = facade_tool(
                client,
                "authoring_transaction",
                "geometry_program_hash",
                {
                    "schema_version": "GeometryProgramHashRequest@1",
                    "geometry_program_draft": draft,
                },
            )
            require(
                hashed.get("validation_status") == "passed"
                and SHA256.fullmatch(str(hashed.get("canonical_sha256"))) is not None,
                "Dragonfang blockout GeometryProgram was rejected",
            )
            program = copy.deepcopy(draft)
            program["canonical_sha256"] = hashed["canonical_sha256"]
            prepared = facade_tool(
                client,
                "authoring_transaction",
                "geometry_prepare",
                {
                    "project_id": project_id,
                    "base_version_id": None,
                    "idempotency_key": f"{project_id}-dragonfang-blockout",
                    "request": {
                        "typed": "geometry",
                        "reference_id": reference["reference_id"],
                        "geometry_program": program,
                    },
                },
            )
            blockout_candidate = prepared.get("candidate")
            blockout_artifact = prepared.get("artifact")
            require(
                isinstance(blockout_candidate, dict)
                and isinstance(blockout_artifact, dict)
                and blockout_artifact.get("hard_gate_passed") is True
                and blockout_artifact.get("program_sha256") == hashed["canonical_sha256"],
                "Dragonfang blockout did not produce strict structural readback",
            )
            blockout_candidate = facade_tool(
                client,
                "observe",
                "candidate_get",
                {"candidate_id": blockout_candidate["candidate_id"]},
            )
            blockout_artifact_readback = facade_tool(
                client,
                "observe",
                "artifact_readback_get",
                {
                    "artifact_id": blockout_artifact["artifact_id"],
                    "candidate_id": blockout_candidate["candidate_id"],
                },
            )
            require(
                blockout_artifact_readback.get("canonical_sha256")
                == blockout_artifact.get("canonical_sha256"),
                "Dragonfang blockout ArtifactReadback drifted",
            )
            verify_artifact_readback(
                blockout_artifact_readback,
                blockout_candidate["candidate_id"],
                "Dragonfang blockout ArtifactReadback",
            )
            source_request = source_prepare_request(
                project_id, blockout_candidate, blockout_artifact
            )
            source_result = facade_tool(
                client,
                "authoring_transaction",
                "production_weapon_authoring_mesh_v2_source_prepare",
                source_request,
            )
            require(
                source_result.get("quality_status")
                == "structural_source_bound_not_visually_evaluated"
                and source_result.get("source_operator_id")
                == "forgecad.geometry.profile-extrude@1"
                and source_result.get("runtime_write_performed") is True
                and source_result.get("stage_advanced") is False,
                "Dragonfang AuthoringMesh source genesis semantics drifted",
            )
            if args.v2_blade:
                structural_request, evaluation_template = dragonfang_v2_blade_requests(
                    project_id,
                    blockout_candidate,
                    source_result,
                    suffix="baseline",
                )
                structural_result = facade_tool(
                    client,
                    "authoring_transaction",
                    "knife_curve_modifier_graph_prepare",
                    structural_request,
                )
                evaluation_request = bind_dragonfang_v2_evaluation_request(
                    evaluation_template, structural_result
                )
                evaluation_result = facade_tool(
                    client,
                    "authoring_transaction",
                    "knife_curve_evaluated_mesh_prepare",
                    evaluation_request,
                )
                v2_program = evaluation_result.get("materialization_program")
                v2_program_sha256 = evaluation_result.get("materialization_program_sha256")
                require(
                    evaluation_result.get("materialization_program_status")
                    == "runtime-derived-v2-blade-body-cutting-edge-program-ready"
                    and isinstance(v2_program, dict)
                    and v2_program.get("canonical_sha256") == v2_program_sha256
                    and [output.get("part_id") for output in v2_program.get("part_outputs", [])]
                    == ["blade-body", "cutting-edge"],
                    "Dragonfang V2 evaluated mesh did not return the closed two-Part materialization program",
                )
                v2_prepared = facade_tool(
                    client,
                    "authoring_transaction",
                    "geometry_prepare",
                    geometry_prepare_request(
                        project_id,
                        reference["reference_id"],
                        v2_program,
                        f"{project_id}-dragonfang-v2-geometry-baseline",
                    ),
                )
                blockout_candidate, blockout_artifact = verify_geometry_candidate_program(
                    v2_prepared,
                    project_id,
                    v2_program_sha256,
                    v2_program,
                    "Dragonfang V2 blade candidate",
                )
                blockout_candidate = facade_tool(
                    client,
                    "observe",
                    "candidate_get",
                    {"candidate_id": blockout_candidate["candidate_id"]},
                )
                blockout_artifact_readback = facade_tool(
                    client,
                    "observe",
                    "artifact_readback_get",
                    {
                        "artifact_id": blockout_artifact["artifact_id"],
                        "candidate_id": blockout_candidate["candidate_id"],
                    },
                )
                verify_artifact_readback(
                    blockout_artifact_readback,
                    blockout_candidate["candidate_id"],
                    "Dragonfang V2 blade ArtifactReadback",
                )
                v2_source_request = source_prepare_request(
                    project_id,
                    blockout_candidate,
                    blockout_artifact,
                    suffix="v2-baseline",
                    part_id="blade-body",
                    source_node_id="knife-v2-part-0",
                )
                source_result = facade_tool(
                    client,
                    "authoring_transaction",
                    "production_weapon_authoring_mesh_v2_source_prepare",
                    v2_source_request,
                )
                require(
                    source_result.get("quality_status")
                    == "structural_source_bound_not_visually_evaluated"
                    and source_result.get("source_operator_id")
                    == "forgecad.geometry.authoring-mesh@1"
                    and source_result.get("source_node_id") == "knife-v2-part-0"
                    and source_result.get("runtime_write_performed") is True,
                    "Dragonfang V2 AuthoringMesh source genesis semantics drifted",
                )
                draft = v2_program
                v2_blade_evidence = {
                    "structural_request_input_sha256": structural_request["input_sha256"],
                    "curve_set_semantic_sha256": structural_result["curve_set_semantic_sha256"],
                    "modifier_graph_semantic_sha256": structural_result["modifier_graph_semantic_sha256"],
                    "evaluation_request_input_sha256": evaluation_request["input_sha256"],
                    "evaluation_plan_sha256": evaluation_result["evaluation_plan_semantic_sha256"],
                    "evaluated_mesh_sha256": evaluation_result["evaluated_mesh_semantic_sha256"],
                    "materialization_program_sha256": v2_program_sha256,
                    "public_part_ids": ["blade-body", "cutting-edge"],
                    "fixed_constraint_views": ["front", "top", "bottom", "left", "right"],
                    "frozen_scope": ["dragon-relief", "guard", "grip", "materials"],
                }
            binding_main = source_binding_main(
                project_id,
                successor,
                intent_result,
                reference,
                blockout_candidate,
                source_result,
            )
            binding_request = source_binding_prepare_request(project_id, binding_main)
            source_binding_result = facade_tool(
                client,
                "authoring_transaction",
                "knife_source_binding_prepare",
                binding_request,
            )
            require(
                source_binding_result.get("status") == "prepared"
                and source_binding_result.get("binding_status") == "runtime-bound"
                and source_binding_result.get("authoring_eligibility") == "ELIGIBLE"
                and source_binding_result.get("source_binding") == binding_main
                and source_binding_result.get("high_mesh_created") is False
                and source_binding_result.get("high_stage_unlocked") is False,
                "KnifeSourceBinding did not persist the exact Runtime-derived control record",
            )
            binding_replay = facade_tool(
                client,
                "authoring_transaction",
                "knife_source_binding_prepare",
                binding_request,
            )
            require(
                binding_replay.get("status") == "replayed"
                and binding_replay.get("replayed") is True
                and binding_replay.get("store_effect") == "not-touched"
                and binding_replay.get("cas_effect") == "not-touched",
                "KnifeSourceBinding replay was not side-effect free",
            )
            source_binding_get = source_binding_get_request(source_binding_result)
            binding_readback = facade_tool(
                client,
                "authoring_transaction",
                "knife_source_binding_get",
                source_binding_get,
            )
            require(
                binding_readback.get("status") == "found"
                and binding_readback.get("source_binding_sha256")
                == source_binding_result.get("source_binding_sha256"),
                "KnifeSourceBinding exact readback failed",
            )
            if args.materialize:
                materializer_request = materializer_prepare_request(
                    project_id, source_result, source_binding_result
                )
                baseline_materializer_request = materializer_request
                materializer_result = facade_tool(
                    client,
                    "authoring_transaction",
                    "authoring_mesh_v2_candidate_materialize",
                    materializer_request,
                )
                expected_preserved_parts = sorted(
                    output["part_id"]
                    for output in draft["part_outputs"]
                    if output["part_id"] != source_result["part_id"]
                )
                materialized_candidate = materializer_result.get("candidate")
                materialized_artifact = materializer_result.get("artifact")
                require(
                    materializer_result.get("status") == "prepared"
                    and materializer_result.get("materialization_mode")
                    == "source_binding_part_replacement"
                    and materializer_result.get("source_candidate_id")
                    == blockout_candidate["candidate_id"]
                    and materializer_result.get("source_program_sha256")
                    == blockout_artifact["program_sha256"]
                    and materializer_result.get("source_part_id")
                    == source_result["part_id"]
                    and sorted(materializer_result.get("preserved_part_ids", []))
                    == expected_preserved_parts
                    and isinstance(materialized_candidate, dict)
                    and isinstance(materialized_artifact, dict)
                    and materialized_artifact.get("hard_gate_passed") is True,
                    "source-bound materializer did not preserve the Dragonfang multi-part candidate",
                )
                materialized_parts = materialized_artifact.get("part_bindings")
                replacement_node_id = materializer_result.get("replacement_node_id")
                source_node_id = source_result.get("source_node_id")
                expected_materialized_bindings = []
                for output in draft["part_outputs"]:
                    for node_id in output["input_node_ids"]:
                        expected_materialized_bindings.append(
                            (
                                output["part_id"],
                                replacement_node_id
                                if output["part_id"] == source_result["part_id"]
                                and node_id == source_node_id
                                else node_id,
                                output["material_zone_id"],
                                output["solid"],
                            )
                        )
                actual_materialized_bindings = (
                    [
                        (
                            binding.get("part_id"),
                            binding.get("source_node_id"),
                            binding.get("material_zone_id"),
                            binding.get("solid"),
                        )
                        for binding in materialized_parts
                    ]
                    if isinstance(materialized_parts, list)
                    else []
                )
                require(
                    isinstance(materialized_parts, list)
                    and isinstance(replacement_node_id, str)
                    and isinstance(source_node_id, str)
                    and len(actual_materialized_bindings)
                    == len(expected_materialized_bindings)
                    and sorted(actual_materialized_bindings)
                    == sorted(expected_materialized_bindings),
                    "materialized ArtifactReadback changed the Dragonfang Part/node semantics",
                )
                materializer_replay = facade_tool(
                    client,
                    "authoring_transaction",
                    "authoring_mesh_v2_candidate_materialize",
                    materializer_request,
                )
                require(
                    materializer_replay.get("status") == "replayed"
                    and materializer_replay.get("replayed") is True
                    and materializer_replay.get("runtime_write_performed") is False
                    and materializer_replay.get("persistent_user_data_touched") is False
                    and materializer_replay.get("idempotency_key") is None
                    and materializer_replay.get("candidate", {}).get("candidate_id")
                    == materialized_candidate["candidate_id"],
                    "source-bound materializer exact replay was not side-effect free",
                )
                # Keep the first materialization immutable as the baseline
                # attempt.  A correction is a separate descendant candidate;
                # never overwrite this identity with the corrected result.
                baseline_materializer_result = materializer_result
                baseline_materializer_request = materializer_request

                if args.high_artifact:
                    require(
                        source_result is not None and source_binding_result is not None,
                        "High artifact path lacks source lineage",
                    )
                    high_artifact_runs["baseline"] = run_dragonfang_high_artifact(
                        client,
                        project_id,
                        source_result,
                        source_binding_result,
                        baseline_materializer_result,
                        args.expected_build_cohort,
                        "Dragonfang baseline",
                        "baseline",
                        reference=reference,
                        view_specs=(
                            dragonfang_high_five_view_specs(reference)
                            if args.v2_blade
                            else None
                        ),
                    )

                if args.correction_pass and args.v2_blade:
                    correction_structural_request, correction_evaluation_template = (
                        dragonfang_v2_blade_requests(
                            project_id,
                            blockout_candidate,
                            source_result,
                            suffix="correction",
                            correction_round=True,
                        )
                    )
                    correction_structural_result = facade_tool(
                        client,
                        "authoring_transaction",
                        "knife_curve_modifier_graph_prepare",
                        correction_structural_request,
                    )
                    correction_evaluation_request = bind_dragonfang_v2_evaluation_request(
                        correction_evaluation_template, correction_structural_result
                    )
                    correction_evaluation_result = facade_tool(
                        client,
                        "authoring_transaction",
                        "knife_curve_evaluated_mesh_prepare",
                        correction_evaluation_request,
                    )
                    correction_program = correction_evaluation_result.get(
                        "materialization_program"
                    )
                    correction_program_sha256 = correction_evaluation_result.get(
                        "materialization_program_sha256"
                    )
                    require(
                        isinstance(correction_program, dict)
                        and correction_program.get("canonical_sha256")
                        == correction_program_sha256,
                        "Dragonfang corrected V2 materialization program is invalid",
                    )
                    correction_v2_program = copy.deepcopy(correction_program)
                    correction_prepared = facade_tool(
                        client,
                        "authoring_transaction",
                        "geometry_prepare",
                        geometry_prepare_request(
                            project_id,
                            reference["reference_id"],
                            correction_program,
                            f"{project_id}-dragonfang-v2-geometry-correction",
                        ),
                    )
                    correction_v2_candidate, correction_v2_artifact = (
                        verify_geometry_candidate_program(
                            correction_prepared,
                            project_id,
                            correction_program_sha256,
                            correction_program,
                            "Dragonfang corrected V2 blade candidate",
                        )
                    )
                    correction_v2_candidate = facade_tool(
                        client,
                        "observe",
                        "candidate_get",
                        {"candidate_id": correction_v2_candidate["candidate_id"]},
                    )
                    correction_v2_source = facade_tool(
                        client,
                        "authoring_transaction",
                        "production_weapon_authoring_mesh_v2_source_prepare",
                        source_prepare_request(
                            project_id,
                            correction_v2_candidate,
                            correction_v2_artifact,
                            suffix="v2-correction",
                            part_id="blade-body",
                            source_node_id="knife-v2-part-0",
                        ),
                    )
                    require(
                        correction_v2_source.get("source_operator_id")
                        == "forgecad.geometry.authoring-mesh@1"
                        and correction_v2_source.get("runtime_write_performed") is True,
                        "Dragonfang corrected V2 AuthoringMesh source was rejected",
                    )
                    correction_binding_main = source_binding_main(
                        project_id,
                        successor,
                        intent_result,
                        reference,
                        correction_v2_candidate,
                        correction_v2_source,
                        suffix="v2-correction",
                    )
                    correction_binding_request = source_binding_prepare_request(
                        project_id,
                        correction_binding_main,
                        suffix="v2-correction",
                    )
                    correction_v2_binding = facade_tool(
                        client,
                        "authoring_transaction",
                        "knife_source_binding_prepare",
                        correction_binding_request,
                    )
                    require(
                        correction_v2_binding.get("status") == "prepared"
                        and correction_v2_binding.get("source_binding")
                        == correction_binding_main,
                        "Dragonfang corrected V2 SourceBinding was rejected",
                    )
                    correction_v2_materializer_request = materializer_prepare_request(
                        project_id,
                        correction_v2_source,
                        correction_v2_binding,
                        idempotency_key=(
                            f"{project_id}-dragonfang-amv2-materialize-v2-correction"
                        ),
                    )
                    correction_v2_materialized = facade_tool(
                        client,
                        "authoring_transaction",
                        "authoring_mesh_v2_candidate_materialize",
                        correction_v2_materializer_request,
                    )
                    require(
                        correction_v2_materialized.get("status") == "prepared"
                        and correction_v2_materialized.get("materialization_mode")
                        == "source_binding_part_replacement"
                        and correction_v2_materialized.get("preserved_part_ids")
                        == ["cutting-edge"],
                        "Dragonfang corrected V2 materializer changed the frozen public scope",
                    )
                    if args.high_artifact:
                        high_artifact_runs["correction"] = run_dragonfang_high_artifact(
                            client,
                            project_id,
                            correction_v2_source,
                            correction_v2_binding,
                            correction_v2_materialized,
                            args.expected_build_cohort,
                            "Dragonfang V2 correction",
                            "v2-correction",
                            reference=reference,
                            view_specs=dragonfang_high_five_view_specs(reference),
                        )
                        if args.low_quad:
                            require(
                                correction_v2_program is not None,
                                "Low quad path lacks the corrected V2 materialization program",
                            )
                            # The source-bound materializer replaces the
                            # blade-body node identity while preserving its
                            # authored geometry. Project that Runtime-derived
                            # replacement ID into the local Low source view so
                            # its lineage matches the durable High GLB instead
                            # of the pre-materialization V2 node.
                            low_source_program = copy.deepcopy(correction_v2_program)
                            replacement_node_id = correction_v2_materialized.get(
                                "replacement_node_id"
                            )
                            require(
                                isinstance(replacement_node_id, str)
                                and bool(
                                    re.fullmatch(
                                        r"[A-Za-z0-9][A-Za-z0-9_.-]{0,127}",
                                        replacement_node_id,
                                    )
                                ),
                                "Low quad path lacks the materialized replacement node",
                            )
                            blade_output = next(
                                (
                                    output
                                    for output in low_source_program.get(
                                        "part_outputs", []
                                    )
                                    if output.get("part_id") == "blade-body"
                                ),
                                None,
                            )
                            require(
                                isinstance(blade_output, dict)
                                and isinstance(blade_output.get("input_node_ids"), list)
                                and len(blade_output["input_node_ids"]) == 1,
                                "Low quad blade-body source node is ambiguous",
                            )
                            original_node_id = blade_output["input_node_ids"][0]
                            blade_node = next(
                                (
                                    node
                                    for node in low_source_program.get("nodes", [])
                                    if node.get("node_id") == original_node_id
                                ),
                                None,
                            )
                            require(
                                isinstance(blade_node, dict),
                                "Low quad blade-body source node is unavailable",
                            )
                            blade_node["node_id"] = replacement_node_id
                            blade_output["input_node_ids"] = [replacement_node_id]
                            low_quad_evidence = run_low_quad_durable(
                                client,
                                project_id,
                                low_source_program,
                                high_artifact_runs["correction"],
                                correction_v2_materialized["candidate"]["candidate_id"],
                                correction_v2_materialized["candidate"]["canonical_sha256"],
                            )
                            if args.uv_bake_v2:
                                uv_bake_v2_evidence = run_knife_uv_bake_v2(
                                    client,
                                    project_id,
                                    correction_v2_materialized["candidate"]["candidate_id"],
                                    correction_v2_materialized["candidate"]["canonical_sha256"],
                                    high_artifact_runs["correction"],
                                    low_quad_evidence,
                                )
                    require(v2_blade_evidence is not None, "Dragonfang V2 evidence is missing")
                    v2_blade_evidence["correction"] = {
                        "scope": ["blade-body", "cutting-edge"],
                        "structural_request_input_sha256": correction_structural_request[
                            "input_sha256"
                        ],
                        "evaluation_request_input_sha256": correction_evaluation_request[
                            "input_sha256"
                        ],
                        "evaluated_mesh_sha256": correction_evaluation_result[
                            "evaluated_mesh_semantic_sha256"
                        ],
                        "materialization_program_sha256": correction_program_sha256,
                        "preserved_part_ids": ["cutting-edge"],
                        "fixed_review_views": ["front", "top", "bottom", "left", "right"],
                    }

                if args.correction_pass and not args.v2_blade:
                    source_durable = source_result.get("authoring_mesh_v2")
                    require(isinstance(source_durable, dict), "source genesis omitted its durable AuthoringMesh result")
                    correction_initial_get_request = durable_get_request(source_durable)
                    correction_initial_get_result = facade_tool(
                        client,
                        "observe",
                        "authoring_mesh_v2_durable_get",
                        correction_initial_get_request,
                    )
                    correction_parent_revision = verify_durable_result(
                        correction_initial_get_result,
                        correction_initial_get_request,
                        "Dragonfang correction initial source durable get",
                        expected_replayed=True,
                        expected_runtime_write=False,
                        expected_operation="genesis",
                    )

                    correction_selector = {
                        "schema_version": DRAGONFANG_SILHOUETTE_SELECTOR_SCHEMA_VERSION,
                        "status": "READY_FOR_CORRECTION",
                        "selector_id": f"dragonfang-high-selector-{project_id[-12:]}",
                        "baseline_candidate_id": blockout_candidate["candidate_id"],
                        "baseline_candidate_state_sha256": blockout_candidate["canonical_sha256"],
                        "baseline_geometry_program_sha256": blockout_artifact["program_sha256"],
                        "baseline_artifact_sha256": blockout_candidate["prepared_object_sha256"],
                        "baseline_artifact_readback_sha256": blockout_artifact_readback["canonical_sha256"],
                        "reference_id": reference["reference_id"],
                        "reference_object_sha256": reference["object_sha256"],
                        "fixed_view_ids": list(DRAGONFANG_SILHOUETTE_CORRECTION_VIEW_IDS),
                        "selected_part_ids": list(DRAGONFANG_SILHOUETTE_CORRECTION_PART_IDS),
                    }
                    correction_payload = build_dragonfang_silhouette_correction_payload(
                        program,
                        correction_selector,
                    )
                    correction_program_draft = copy.deepcopy(correction_payload["geometry_program"])
                    correction_geometry_hash_request = geometry_program_hash_request(
                        correction_program_draft
                    )
                    correction_geometry_hash_result = facade_tool(
                        client,
                        "authoring_transaction",
                        "geometry_program_hash",
                        correction_geometry_hash_request,
                    )
                    require(
                        correction_geometry_hash_result.get("validation_status") == "passed"
                        and SHA256.fullmatch(
                            str(correction_geometry_hash_result.get("canonical_sha256"))
                        )
                        is not None,
                        "Dragonfang correction GeometryProgram was rejected",
                    )
                    correction_payload = bind_dragonfang_correction_runtime_hash(
                        correction_payload,
                        correction_geometry_hash_result["canonical_sha256"],
                    )
                    validate_dragonfang_silhouette_correction_payload(correction_payload)
                    correction_suffix = _operation_suffix(
                        f"correction-{correction_payload['correction_id'][-24:]}",
                        "Dragonfang correction suffix",
                    )
                    correction_program = copy.deepcopy(correction_payload["geometry_program"])
                    correction_geometry_idempotency_key = (
                        f"{project_id}-dragonfang-correction-geometry-{correction_suffix[-32:]}"
                    )
                    correction_request = geometry_prepare_request(
                        project_id,
                        reference["reference_id"],
                        correction_program,
                        correction_geometry_idempotency_key,
                    )
                    correction_geometry_prepare_result = facade_tool(
                        client,
                        "authoring_transaction",
                        "geometry_prepare",
                        correction_request,
                    )
                    correction_source_candidate, correction_source_artifact = (
                        verify_geometry_candidate_program(
                            correction_geometry_prepare_result,
                            project_id,
                            correction_geometry_hash_result["canonical_sha256"],
                            correction_program,
                            "Dragonfang correction source candidate",
                        )
                    )
                    correction_geometry_replay_result = facade_tool(
                        client,
                        "authoring_transaction",
                        "geometry_prepare",
                        correction_request,
                    )
                    replay_candidate, replay_artifact = verify_geometry_candidate_program(
                        correction_geometry_replay_result,
                        project_id,
                        correction_geometry_hash_result["canonical_sha256"],
                        correction_program,
                        "Dragonfang correction source candidate replay",
                    )
                    require(
                        replay_candidate["candidate_id"] == correction_source_candidate["candidate_id"]
                        and replay_candidate["canonical_sha256"] == correction_source_candidate["canonical_sha256"]
                        and replay_artifact["object_sha256"] == correction_source_artifact["object_sha256"],
                        "Dragonfang correction geometry replay changed immutable candidate",
                    )
                    correction_source_artifact_readback = facade_tool(
                        client,
                        "observe",
                        "artifact_readback_get",
                        {
                            "artifact_id": correction_source_artifact["artifact_id"],
                            "candidate_id": correction_source_candidate["candidate_id"],
                        },
                    )
                    verify_artifact_readback(
                        correction_source_artifact_readback,
                        correction_source_candidate["candidate_id"],
                        "Dragonfang correction source ArtifactReadback",
                    )
                    correction_source_prepare_request = source_prepare_request(
                        project_id,
                        correction_source_candidate,
                        correction_source_artifact,
                        suffix=correction_suffix,
                    )
                    correction_source_result = facade_tool(
                        client,
                        "authoring_transaction",
                        "production_weapon_authoring_mesh_v2_source_prepare",
                        correction_source_prepare_request,
                    )
                    require(
                        correction_source_result.get("quality_status")
                        == "structural_source_bound_not_visually_evaluated"
                        and correction_source_result.get("source_operator_id")
                        == "forgecad.geometry.profile-extrude@1"
                        and correction_source_result.get("candidate_id")
                        == correction_source_candidate["candidate_id"]
                        and correction_source_result.get("geometry_program_sha256")
                        == correction_source_artifact["program_sha256"]
                        and correction_source_result.get("part_id") == "blade-body"
                        and correction_source_result.get("source_node_id")
                        == "dragonfang-blade-body"
                        and correction_source_result.get("runtime_write_performed") is True,
                        "Dragonfang correction source genesis did not bind the corrected candidate",
                    )
                    correction_result = correction_source_result
                    correction_child_get_request = durable_get_request(
                        correction_source_result["authoring_mesh_v2"]
                    )
                    correction_child_get_result = facade_tool(
                        client,
                        "observe",
                        "authoring_mesh_v2_durable_get",
                        correction_child_get_request,
                    )
                    verify_durable_result(
                        correction_child_get_result,
                        correction_child_get_request,
                        "Dragonfang correction source durable get",
                        expected_replayed=True,
                        expected_runtime_write=False,
                        expected_operation="genesis",
                    )

                    correction_source_binding_main = source_binding_main(
                        project_id,
                        successor,
                        intent_result,
                        reference,
                        correction_source_candidate,
                        correction_source_result,
                        suffix=correction_suffix,
                    )
                    correction_source_binding_request = source_binding_prepare_request(
                        project_id,
                        correction_source_binding_main,
                        suffix=correction_suffix,
                    )
                    correction_source_binding_result = facade_tool(
                        client,
                        "authoring_transaction",
                        "knife_source_binding_prepare",
                        correction_source_binding_request,
                    )
                    require(
                        correction_source_binding_result.get("status") == "prepared"
                        and correction_source_binding_result.get("source_binding")
                        == correction_source_binding_main
                        and correction_source_binding_result.get("source_candidate_id")
                        == correction_source_candidate["candidate_id"]
                        and correction_source_binding_result.get("source_binding_id")
                        != source_binding_result.get("source_binding_id")
                        and correction_source_binding_result.get("high_mesh_created") is False,
                        "Dragonfang correction SourceBinding did not bind the corrected source candidate",
                    )
                    correction_source_binding_replay = facade_tool(
                        client,
                        "authoring_transaction",
                        "knife_source_binding_prepare",
                        correction_source_binding_request,
                    )
                    require(
                        correction_source_binding_replay.get("status") == "replayed"
                        and correction_source_binding_replay.get("replayed") is True
                        and correction_source_binding_replay.get("store_effect") == "not-touched"
                        and correction_source_binding_replay.get("cas_effect") == "not-touched",
                        "Dragonfang correction SourceBinding replay was not side-effect free",
                    )
                    correction_source_binding_get = source_binding_get_request(
                        correction_source_binding_result
                    )
                    correction_source_binding_readback = facade_tool(
                        client,
                        "authoring_transaction",
                        "knife_source_binding_get",
                        correction_source_binding_get,
                    )
                    require(
                        correction_source_binding_readback.get("status") == "found"
                        and correction_source_binding_readback.get("source_binding_sha256")
                        == correction_source_binding_result.get("source_binding_sha256"),
                        "Dragonfang correction SourceBinding exact readback failed",
                    )

                    correction_materializer_request = materializer_prepare_request(
                        project_id,
                        correction_source_result,
                        correction_source_binding_result,
                        idempotency_key=(
                            f"{project_id}-dragonfang-{correction_suffix}-materialize"
                        ),
                    )
                    correction_materializer_result = facade_tool(
                        client,
                        "authoring_transaction",
                        "authoring_mesh_v2_candidate_materialize",
                        correction_materializer_request,
                    )
                    correction_candidate = correction_materializer_result.get("candidate")
                    correction_artifact = correction_materializer_result.get("artifact")
                    correction_expected_preserved_parts = sorted(
                        output["part_id"]
                        for output in correction_program["part_outputs"]
                        if output["part_id"] != correction_source_result["part_id"]
                    )
                    require(
                        correction_materializer_result.get("status") == "prepared"
                        and correction_materializer_result.get("materialization_mode")
                        == "source_binding_part_replacement"
                        and correction_materializer_result.get("source_candidate_id")
                        == correction_source_candidate["candidate_id"]
                        and correction_materializer_result.get("source_binding_id")
                        == correction_source_binding_result["source_binding_id"]
                        and correction_materializer_result.get("source_binding_sha256")
                        == correction_source_binding_result["source_binding_sha256"]
                        and correction_materializer_result.get("revision_id")
                        == correction_source_result["revision_id"]
                        and correction_materializer_result.get("revision_sha256")
                        == correction_source_result["revision_sha256"]
                        and correction_materializer_result.get("source_program_sha256")
                        == correction_source_artifact["program_sha256"]
                        and correction_materializer_result.get("source_part_id")
                        == correction_source_result["part_id"]
                        and sorted(correction_materializer_result.get("preserved_part_ids", []))
                        == correction_expected_preserved_parts
                        and isinstance(correction_candidate, dict)
                        and isinstance(correction_artifact, dict)
                        and correction_artifact.get("hard_gate_passed") is True,
                        "Dragonfang correction materialization did not bind the durable child",
                    )
                    require(
                        correction_candidate.get("candidate_id")
                        != materialized_candidate["candidate_id"]
                        and correction_candidate.get("canonical_sha256")
                        != materialized_candidate.get("canonical_sha256")
                        and correction_artifact.get("program_sha256")
                        != materialized_artifact.get("program_sha256")
                        and correction_materializer_result.get(
                            "representation_plan_sha256"
                        )
                        != materializer_result.get("representation_plan_sha256"),
                        "Dragonfang correction did not produce a distinct candidate/program/plan",
                    )
                    correction_parts = correction_artifact.get("part_bindings")
                    correction_replacement_node_id = correction_materializer_result.get(
                        "replacement_node_id"
                    )
                    expected_correction_bindings = []
                    for output in correction_program["part_outputs"]:
                        for node_id in output["input_node_ids"]:
                            expected_correction_bindings.append(
                                (
                                    output["part_id"],
                                    correction_replacement_node_id
                                    if output["part_id"] == correction_source_result["part_id"]
                                    and node_id == correction_source_result["source_node_id"]
                                    else node_id,
                                    output["material_zone_id"],
                                    output["solid"],
                                )
                            )
                    actual_correction_bindings = (
                        [
                            (
                                binding.get("part_id"),
                                binding.get("source_node_id"),
                                binding.get("material_zone_id"),
                                binding.get("solid"),
                            )
                            for binding in correction_parts
                        ]
                        if isinstance(correction_parts, list)
                        else []
                    )
                    require(
                        isinstance(correction_parts, list)
                        and isinstance(correction_replacement_node_id, str)
                        and len(actual_correction_bindings)
                        == len(expected_correction_bindings)
                        and sorted(actual_correction_bindings)
                        == sorted(expected_correction_bindings),
                        "Dragonfang correction materialization changed the Part/node semantics",
                    )
                    if args.high_artifact:
                        require(
                            correction_source_result is not None
                            and correction_source_binding_result is not None,
                            "High artifact correction path lacks source lineage",
                        )
                        high_artifact_runs["correction"] = run_dragonfang_high_artifact(
                            client,
                            project_id,
                            correction_source_result,
                            correction_source_binding_result,
                            correction_materializer_result,
                            args.expected_build_cohort,
                            "Dragonfang correction",
                            "correction",
                            reference=reference,
                        )

        # The V2 High path already performs the requested immutable five-view
        # baseline/correction comparisons directly against each High Artifact.
        # Do not also enter the legacy candidate-bound front-only visual loop;
        # that would add a third evidence model and obscure the bounded V2
        # correction result.
        if args.visual_pass and not (args.v2_blade and args.high_artifact):
            require(
                blockout_candidate is not None and blockout_artifact is not None,
                "visual pass requires the source-bound blockout candidate",
            )
            # Each entry in visual_runs is one independent candidate-bound
            # mask/compare.  In correction mode this is deliberately exactly
            # two entries: initial materialization and corrected descendant.
            baseline_result = baseline_materializer_result
            baseline_candidate = (
                baseline_result.get("candidate")
                if baseline_result is not None
                else blockout_candidate
            )
            baseline_artifact = (
                baseline_result.get("artifact")
                if baseline_result is not None
                else blockout_artifact
            )
            baseline_readback = (
                None
                if baseline_result is not None
                else blockout_artifact_readback
            )
            require(
                isinstance(baseline_candidate, dict)
                and isinstance(baseline_artifact, dict),
                "baseline visual candidate/artifact readback is unavailable",
            )
            visual_runs["baseline"] = run_dragonfang_visual_candidate(
                client,
                project_id,
                baseline_candidate,
                baseline_artifact,
                reference,
                args.expected_build_cohort,
                "Dragonfang baseline",
                artifact_readback=baseline_readback,
            )
            baseline_visual_identity = visual_runs["baseline"]["identity"]
            if args.correction_pass:
                require(
                    correction_materializer_result is not None,
                    "correction visual pass lacks corrected materialization",
                )
                corrected_candidate = correction_materializer_result.get("candidate")
                corrected_artifact = correction_materializer_result.get("artifact")
                require(
                    isinstance(corrected_candidate, dict)
                    and isinstance(corrected_artifact, dict),
                    "corrected visual candidate/artifact readback is unavailable",
                )
                visual_runs["correction"] = run_dragonfang_visual_candidate(
                    client,
                    project_id,
                    corrected_candidate,
                    corrected_artifact,
                    reference,
                    args.expected_build_cohort,
                    "Dragonfang correction",
                    mask_context=visual_runs["baseline"]["mask_context"],
                )
                correction_visual_identity = visual_runs["correction"]["identity"]

            selected_visual_label = "correction" if args.correction_pass else "baseline"
            selected_visual = visual_runs[selected_visual_label]
            visual_candidate = selected_visual["candidate"]
            visual_artifact = selected_visual["artifact"]
            visual_artifact_readback = selected_visual["artifact_readback"]
            visual_mask_result = selected_visual["mask_result"]
            visual_compare_result = selected_visual["compare_result"]
            visual_identity = selected_visual["identity"]
            visual_observation = selected_visual["observation"]
            visual_bundle = selected_visual["bundle"]

            if args.correction_pass:
                # The root pass anchors its baseline to the original
                # SourceBinding candidate and its attempt to the initial
                # materialization.  No third visual compare is allowed for the
                # source candidate; geometry identity comes from observe only.
                blockout_artifact_sha = _candidate_artifact_sha(
                    blockout_candidate, blockout_artifact
                )
                blockout_program_sha = require_sha256(
                    blockout_artifact.get("program_sha256"),
                    "Dragonfang source candidate GeometryProgram",
                )
                _, blockout_observation_sha256, blockout_geometry = (
                    observe_candidate_geometry(
                        client,
                        project_id,
                        blockout_candidate["candidate_id"],
                        blockout_artifact_sha,
                        blockout_program_sha,
                        "Dragonfang PassState root baseline",
                    )
                )
                pass_created_at = (
                    source_binding_result.get("source_binding", {}).get("created_at")
                    if isinstance(source_binding_result.get("source_binding"), dict)
                    else None
                ) or "2026-08-30T00:00:00Z"
                require(
                    source_result is not None
                    and source_binding_result is not None
                    and intent_result is not None
                    and baseline_materializer_result is not None
                    and correction_result is not None
                    and blockout_geometry is not None,
                    "PassState lineage inputs are incomplete",
                )
                root_pass_id = f"dragonfang-pass-root-{project_id[-12:]}"
                child_pass_id = f"dragonfang-pass-correction-{project_id[-12:]}"
                pass_state_root_main = build_knife_pass_state_main(
                    pass_id=root_pass_id,
                    parent_pass_id=None,
                    parent_pass_sha256=None,
                    source_binding_result=source_binding_result,
                    intent_result=intent_result,
                    successor_brief=successor_brief,
                    reference=reference,
                    source_candidate=blockout_candidate,
                    baseline_candidate=blockout_candidate,
                    baseline_artifact=blockout_artifact,
                    baseline_geometry=blockout_geometry,
                    baseline_representation_plan_sha256=draft[
                        "representation_plan_sha256"
                    ],
                    attempt_candidate=baseline_materializer_result["candidate"],
                    attempt_artifact=baseline_materializer_result["artifact"],
                    attempt_geometry=visual_runs["baseline"]["geometry"],
                    attempt_representation_plan_sha256=baseline_materializer_result[
                        "representation_plan_sha256"
                    ],
                    selected_revision=source_result,
                    visual_identity=baseline_visual_identity,
                    created_at=pass_created_at,
                )
                pass_state_root_request = pass_state_prepare_request(
                    project_id,
                    pass_state_root_main,
                    f"{project_id}-dragonfang-pass-root",
                )
                pass_state_root_result = facade_tool(
                    client,
                    "quality_review",
                    "knife_pass_state_prepare",
                    pass_state_root_request,
                )
                verify_pass_state_result(
                    pass_state_root_result,
                    pass_state_root_main,
                    "Dragonfang root PassState prepare",
                    expected_status="prepared",
                    expected_request_kind="prepare",
                    expected_idempotency_key=pass_state_root_request["idempotency_key"],
                )
                pass_state_root_replay = facade_tool(
                    client,
                    "quality_review",
                    "knife_pass_state_prepare",
                    pass_state_root_request,
                )
                verify_pass_state_result(
                    pass_state_root_replay,
                    pass_state_root_main,
                    "Dragonfang root PassState replay",
                    expected_status="replayed",
                    expected_request_kind="prepare",
                    expected_idempotency_key=None,
                )
                pass_state_root_get_request = pass_state_get_request(
                    pass_state_root_result
                )
                pass_state_root_get_result = facade_tool(
                    client,
                    "quality_review",
                    "knife_pass_state_get",
                    pass_state_root_get_request,
                )
                verify_pass_state_result(
                    pass_state_root_get_result,
                    pass_state_root_main,
                    "Dragonfang root PassState get",
                    expected_status="found",
                    expected_request_kind="get",
                    expected_idempotency_key=None,
                )
                pass_state_child_main = build_knife_pass_state_main(
                    pass_id=child_pass_id,
                    parent_pass_id=root_pass_id,
                    parent_pass_sha256=pass_state_root_main["canonical_sha256"],
                    source_binding_result=source_binding_result,
                    intent_result=intent_result,
                    successor_brief=successor_brief,
                    reference=reference,
                    source_candidate=blockout_candidate,
                    baseline_candidate=baseline_materializer_result["candidate"],
                    baseline_artifact=baseline_materializer_result["artifact"],
                    baseline_geometry=visual_runs["baseline"]["geometry"],
                    baseline_representation_plan_sha256=baseline_materializer_result[
                        "representation_plan_sha256"
                    ],
                    attempt_candidate=correction_materializer_result["candidate"],
                    attempt_artifact=correction_materializer_result["artifact"],
                    attempt_geometry=visual_runs["correction"]["geometry"],
                    attempt_representation_plan_sha256=correction_materializer_result[
                        "representation_plan_sha256"
                    ],
                    selected_revision=correction_result,
                    visual_identity=correction_visual_identity,
                    created_at=pass_created_at,
                    parent_visual_identity=baseline_visual_identity,
                )
                pass_state_child_request = pass_state_prepare_request(
                    project_id,
                    pass_state_child_main,
                    f"{project_id}-dragonfang-pass-correction",
                )
                pass_state_child_result = facade_tool(
                    client,
                    "quality_review",
                    "knife_pass_state_prepare",
                    pass_state_child_request,
                )
                verify_pass_state_result(
                    pass_state_child_result,
                    pass_state_child_main,
                    "Dragonfang correction PassState prepare",
                    expected_status="prepared",
                    expected_request_kind="prepare",
                    expected_idempotency_key=pass_state_child_request["idempotency_key"],
                )
                pass_state_child_replay = facade_tool(
                    client,
                    "quality_review",
                    "knife_pass_state_prepare",
                    pass_state_child_request,
                )
                verify_pass_state_result(
                    pass_state_child_replay,
                    pass_state_child_main,
                    "Dragonfang correction PassState replay",
                    expected_status="replayed",
                    expected_request_kind="prepare",
                    expected_idempotency_key=None,
                )
                pass_state_child_get_request = pass_state_get_request(
                    pass_state_child_result
                )
                pass_state_child_get_result = facade_tool(
                    client,
                    "quality_review",
                    "knife_pass_state_get",
                    pass_state_child_get_request,
                )
                verify_pass_state_result(
                    pass_state_child_get_result,
                    pass_state_child_main,
                    "Dragonfang correction PassState get",
                    expected_status="found",
                    expected_request_kind="get",
                    expected_idempotency_key=None,
                )
                correction_pass_state_status = "PASS_ROOT_CHILD_PREPARE_REPLAY_GET"

        client.close()
        client = None
        shutdown_runtime(ready, ready_path, runtime)
        runtime = None
        ready = None

        runtime, ready_path, ready = start_runtime(
            args.runtime, data_root, base_environment, args.timeout
        )
        client = initialize_client(
            args.mcp, mcp_environment(base_environment, ready), args.timeout
        )
        reopened = facade_tool(
            client,
            "reference_intake",
            "weaponry_knife_production_brief_get",
            successor_get_request,
        )
        require(
            reopened.get("status") == "found"
            and reopened.get("brief_sha256") == successor.get("brief_sha256")
            and reopened.get("brief_object_sha256") == successor.get("brief_object_sha256")
            and reopened.get("authoring_eligibility") == "ELIGIBLE",
            "successor restart readback drifted",
        )
        if intent_result is not None and intent_get is not None:
            reopened_intent = facade_tool(
                client,
                "reference_intake",
                "knife_reference_intent_bundle_get",
                intent_get,
            )
            require(
                reopened_intent.get("status") == "found"
                and reopened_intent.get("intent_bundle_sha256")
                == intent_result.get("intent_bundle_sha256")
                and reopened_intent.get("intent_bundle_object_sha256")
                == intent_result.get("intent_bundle_object_sha256")
                and reopened_intent.get("high_stage_unlocked") is False,
                "reference intent restart readback drifted",
            )
        if source_binding_result is not None and source_binding_get is not None:
            reopened_binding = facade_tool(
                client,
                "authoring_transaction",
                "knife_source_binding_get",
                source_binding_get,
            )
            require(
                reopened_binding.get("status") == "found"
                and reopened_binding.get("source_binding_sha256")
                == source_binding_result.get("source_binding_sha256")
                and reopened_binding.get("source_binding_object_sha256")
                == source_binding_result.get("source_binding_object_sha256")
                and reopened_binding.get("high_mesh_created") is False,
                "KnifeSourceBinding restart readback drifted",
            )
        materializer_restart_results: dict[str, dict[str, Any]] = {}
        restart_materializers = []
        if baseline_materializer_result is not None:
            restart_materializers.append(("baseline", baseline_materializer_result))
        if correction_materializer_result is not None:
            restart_materializers.append(("correction", correction_materializer_result))
        for materializer_label, materializer in restart_materializers:
            materialized_candidate = materializer["candidate"]
            materialized_artifact = materializer["artifact"]
            reopened_materialized_candidate = facade_tool(
                client,
                "observe",
                "candidate_get",
                {"candidate_id": materialized_candidate["candidate_id"]},
            )
            reopened_materialized_artifact = facade_tool(
                client,
                "observe",
                "artifact_readback_get",
                {
                    "artifact_id": materialized_artifact["artifact_id"],
                    "candidate_id": materialized_candidate["candidate_id"],
                },
            )
            require(
                reopened_materialized_candidate.get("canonical_sha256")
                == materialized_candidate.get("canonical_sha256")
                and reopened_materialized_artifact.get("canonical_sha256")
                == materialized_artifact.get("canonical_sha256"),
                f"{materializer_label} source-bound materialized candidate restart readback drifted",
            )
            materializer_restart_results[materializer_label] = {
                "candidate": reopened_materialized_candidate,
                "artifact": reopened_materialized_artifact,
            }
        if low_quad_evidence is not None:
            for part_id, component in low_quad_evidence["components"].items():
                restart_get = facade_tool(
                    client,
                    "surface_pipeline",
                    "low_quad_draft_durable_get",
                    component["get_request"],
                )
                verify_low_quad_result(
                    restart_get,
                    component["get_request"],
                    {"source_lineage": component["source_lineage"]},
                    f"Dragonfang Low {part_id} restart get",
                    expected_replayed=False,
                    expected_write=False,
                )
                for field, expected in component["prepare"].items():
                    require(
                        restart_get.get(field) == expected,
                        f"Dragonfang Low {part_id} restart hash drifted for {field}",
                    )
                component["restart_get_status"] = "PASS_EXACT_GET"
                component["restart_get_request_input_sha256"] = component["get_request"]["input_sha256"]
            low_quad_evidence["restart_status"] = "PASS_ALL_COMPONENTS_EXACT_GET"
        if uv_bake_v2_evidence is not None:
            uv_restart_get = facade_tool(
                client,
                "surface_pipeline",
                "production_knife_uv_bake_v2_get",
                uv_bake_v2_evidence["get_request"],
            )
            uv_restart_summary = verify_knife_uv_bake_v2_result(
                uv_restart_get,
                uv_bake_v2_evidence["request"],
                "Dragonfang UV/Bake V2 restart get",
                expected_replayed=False,
                expected_write=False,
                expected_restart=False,
            )
            require(
                uv_restart_summary == uv_bake_v2_evidence["get"],
                "Dragonfang UV/Bake V2 restart child CAS hashes drifted",
            )
            uv_bake_v2_evidence["restart_get"] = uv_restart_summary
            uv_bake_v2_evidence["restart_status"] = "PASS_EXACT_GET_AFTER_RUNTIME_RESTART"
        for visual_label, visual_run in visual_runs.items():
            restart_identity = visual_run["identity"]
            restart_candidate = visual_run["candidate"]
            reopened_target = facade_tool(
                client,
                "quality_review",
                "silhouette_target_get",
                {"target_sha256": restart_identity["target_sha256"]},
            )
            verify_canonical_object(
                reopened_target,
                "canonical_sha256",
                f"reopened {visual_label} SilhouetteTarget",
            )
            require(
                object_sha256(reopened_target) == restart_identity["target_sha256"]
                and reopened_target.get("reference_id") == restart_identity["reference_id"]
                and reopened_target.get("mask_sha256") == restart_identity["mask_sha256"]
                and reopened_target.get("annotation_status") == "unreviewed",
                f"{visual_label} SilhouetteTarget restart readback drifted",
            )
            reopened_visual_observation, reopened_visual_observation_sha256, _ = (
                observe_candidate_geometry(
                    client,
                    project_id,
                    restart_candidate["candidate_id"],
                    restart_identity["candidate_artifact_sha256"],
                    restart_identity["geometry_program_sha256"],
                    f"reopened {visual_label}",
                )
            )
            reopened_visual_bundle = facade_tool(
                client,
                "quality_review",
                "visual_evidence_bundle_get",
                {
                    "project_id": project_id,
                    "candidate_id": restart_candidate["candidate_id"],
                    "observation_sha256": reopened_visual_observation_sha256,
                },
            )
            verify_visual_evidence_projection(
                reopened_visual_bundle,
                project_id,
                restart_candidate["candidate_id"],
                restart_identity,
                f"reopened {visual_label} visual evidence bundle",
            )
            restart_aovs = readback_visual_aovs(
                client,
                restart_identity["render_set_object_sha256"],
                restart_candidate["candidate_id"],
                restart_identity["aov_pass_sha256"],
            )
            visual_restart_evidence[visual_label] = {
                "observation": reopened_visual_observation,
                "observation_sha256": reopened_visual_observation_sha256,
                "bundle": reopened_visual_bundle,
                "aov_pass_sha256": restart_aovs,
            }
        selected_visual_label = "correction" if args.correction_pass else "baseline"
        if selected_visual_label in visual_restart_evidence:
            selected_restart = visual_restart_evidence[selected_visual_label]
            reopened_observation = selected_restart["observation"]
            visual_aov_restart_readback = selected_restart["aov_pass_sha256"]

        if pass_state_root_main is not None and pass_state_root_get_request is not None:
            pass_state_root_restart = facade_tool(
                client,
                "quality_review",
                "knife_pass_state_get",
                pass_state_root_get_request,
            )
            verify_pass_state_result(
                pass_state_root_restart,
                pass_state_root_main,
                "Dragonfang root PassState restart get",
                expected_status="found",
                expected_request_kind="get",
                expected_idempotency_key=None,
            )
            pass_state_restart_get_results["root"] = pass_state_root_restart
        if pass_state_child_main is not None and pass_state_child_get_request is not None:
            pass_state_child_restart = facade_tool(
                client,
                "quality_review",
                "knife_pass_state_get",
                pass_state_child_get_request,
            )
            verify_pass_state_result(
                pass_state_child_restart,
                pass_state_child_main,
                "Dragonfang correction PassState restart get",
                expected_status="found",
                expected_request_kind="get",
                expected_idempotency_key=None,
            )
            pass_state_restart_get_results["correction"] = pass_state_child_restart
        if correction_initial_get_request is not None and source_result is not None:
            reopened_initial_durable = facade_tool(
                client,
                "observe",
                "authoring_mesh_v2_durable_get",
                correction_initial_get_request,
            )
            verify_durable_result(
                reopened_initial_durable,
                correction_initial_get_request,
                "Dragonfang initial durable restart get",
                expected_replayed=True,
                expected_runtime_write=False,
                expected_operation="genesis",
            )
        if correction_child_get_request is not None and correction_result is not None:
            reopened_child_durable = facade_tool(
                client,
                "observe",
                "authoring_mesh_v2_durable_get",
                correction_child_get_request,
            )
            verify_durable_result(
                reopened_child_durable,
                correction_child_get_request,
                "Dragonfang correction durable restart get",
                expected_replayed=True,
                expected_runtime_write=False,
                expected_operation="genesis",
            )

        selected_high_compare = (
            high_artifact_runs.get("correction", high_artifact_runs.get("baseline", {})).get(
                "compare"
            )
            if high_artifact_runs
            else None
        )
        if uv_bake_v2_evidence is not None:
            receipt_status = "PASS_LIVE_UV_CAGE_BAKE_MULTIPART_STRUCTURAL_ONLY"
        elif low_quad_evidence is not None:
            receipt_status = "PASS_LIVE_LOW_QUAD_MULTIPART_STRUCTURAL_ONLY"
        elif visual_identity is not None:
            receipt_status = "LIVE_VISUAL_EVIDENCE_CAPTURED_" + visual_identity["quality_status"]
        elif isinstance(selected_high_compare, dict):
            receipt_status = (
                "LIVE_HIGH_ARTIFACT_FIXED_VIEW_CAPTURED_"
                + str(selected_high_compare["quality_status"])
            )
        elif materializer_result is not None:
            receipt_status = "PASS_ISOLATED_LIVE_MULTI_PART_MATERIALIZATION_VISUAL_NOT_RUN"
        elif source_binding_result is not None:
            receipt_status = "PASS_ISOLATED_LIVE_STRUCTURAL_SOURCE_BOUND_HIGH_NOT_RUN"
        elif intent_result is not None:
            receipt_status = "PASS_ISOLATED_LIVE_REFERENCE_INTENT_HIGH_LOCKED"
        else:
            receipt_status = "PASS_ISOLATED_LIVE_AUTHORING_ELIGIBLE_SUCCESSOR"

        receipt = {
            "schema_version": "WeaponryKnifeUvBakeV2LiveReceipt@1"
            if uv_bake_v2_evidence is not None
            else (
                "WeaponryKnifeLowQuadLiveReceipt@1"
                if low_quad_evidence is not None
                else (
                    "WeaponryKnifeVisualEvidenceLiveReceipt@1"
                    if visual_identity is not None
                    else (
                        "WeaponryKnifeHighArtifactVisualLiveReceipt@1"
                        if isinstance(selected_high_compare, dict)
                        else (
                            "WeaponryKnifeMaterializationLiveReceipt@1"
                            if materializer_result is not None
                            else (
                                "WeaponryKnifeSourceBindingLiveReceipt@1"
                                if source_binding_result is not None
                                else (
                                    "WeaponryKnifeReferenceIntentLiveReceipt@1"
                                    if intent_result is not None
                                    else "WeaponryKnifeProductionBriefLiveReceipt@1"
                                )
                            )
                        )
                    )
                )
            ),
            "task_id": "WPN-KNIFE-UVBAKE-LIVE-001"
            if uv_bake_v2_evidence is not None
            else (
                "WPN-KNIFE-LOW-LIVE-001"
                if low_quad_evidence is not None
                else (
                    "WPN-KNIFE-CORRECTION-PASS-001"
                    if args.correction_pass
                    else (
                        "WPN-AUTH-MATERIALIZE-001"
                        if materializer_result is not None
                        else (
                            "WPN-KNIFE-SOURCE-BINDING-001"
                            if source_binding_result is not None
                            else (
                                "WPN-KNIFE-HIGH-001-SLICE-A"
                                if intent_result is not None
                                else "WPN-KNIFE-BRIEF-RUNTIME-001"
                            )
                        )
                    )
                )
            ),
            "status": receipt_status,
            "build_cohort_sha256": args.expected_build_cohort,
            "project_id": project_id,
            "reference_id": reference["reference_id"],
            "reference_object_sha256": reference["object_sha256"],
            "reference_evidence_sha256": reference["canonical_sha256"],
            "reference_dimensions": {
                "width": reference["width"],
                "height": reference["height"],
            },
            "reference_profile": args.reference_profile,
            "reference_source_assessment": (
                {
                    "kind": "generated-multiview-design-sheet",
                    "supplied_views": list(DRAGONFANG_GENERATED_MULTIVIEW_SUPPLIED_VIEWS),
                    "supplemental_panels": list(
                        DRAGONFANG_GENERATED_MULTIVIEW_SUPPLEMENTAL_PANELS
                    ),
                    "missing_views": list(DRAGONFANG_GENERATED_MULTIVIEW_MISSING_VIEWS),
                    "cross_view_consistency": "micro-drift-unknown",
                    "surface_micro_detail_status": "design-intent-not-exact-geometry",
                }
                if args.reference_profile == "generated-multiview"
                else None
            ),
            "parent_brief_id": parent["brief_id"],
            "parent_brief_sha256": parent["brief_sha256"],
            "parent_brief_object_sha256": parent["brief_object_sha256"],
            "parent_authoring_eligibility": parent["authoring_eligibility"],
            "successor_brief_id": successor["brief_id"],
            "successor_brief_sha256": successor["brief_sha256"],
            "successor_brief_object_sha256": successor["brief_object_sha256"],
            "successor_authoring_eligibility": successor["authoring_eligibility"],
            "successor_authorization_binding_status": successor[
                "authorization_binding_status"
            ],
            "exact_replay": "PASS_NOT_TOUCHED",
            "restart_readback": "PASS_EXACT_HASH",
            "missing_reference_views": successor_brief["reference_coverage"][
                "missing_views"
            ],
            "hq_360_status": successor_brief["reference_coverage"]["hq_360_status"],
            "high_entry_status": "HIGH_ARTIFACT_CREATED_FIXED_VIEW_EVALUATED_STRUCTURAL_ONLY"
            if isinstance(selected_high_compare, dict)
            else ("HIGH_ARTIFACT_CREATED_STRUCTURAL_ONLY" if high_artifact_runs else "AUTHORIZED_TO_START_NOT_EXECUTED"),
            "high_artifact_runs": high_artifact_runs,
            "low_quad_durable": low_quad_evidence,
            "uv_bake_v2": uv_bake_v2_evidence,
            "v2_blade_language": v2_blade_evidence,
            "intent_bundle_id": intent_result.get("intent_bundle_id")
            if intent_result is not None
            else None,
            "intent_bundle_sha256": intent_result.get("intent_bundle_sha256")
            if intent_result is not None
            else None,
            "intent_bundle_object_sha256": intent_result.get(
                "intent_bundle_object_sha256"
            )
            if intent_result is not None
            else None,
            "intent_exact_replay": "PASS_NOT_TOUCHED"
            if intent_result is not None
            else "NOT_RUN",
            "intent_restart_readback": "PASS_EXACT_HASH"
            if intent_result is not None
            else "NOT_RUN",
            "intent_visual_contract": intent_visual_contract,
            "fixed_reference_views": (
                (
                    {
                        view["view_id"]: view
                        for view in dragonfang_high_five_view_specs(reference)
                    }
                    if args.v2_blade
                    else dragonfang_fixed_reference_views(
                        reference["reference_id"],
                        reference["object_sha256"],
                        reference["width"],
                        reference["height"],
                    )
                )
                if args.visual_pass and intent_visual_contract is not None
                else None
            ),
            "blockout_candidate_id": blockout_candidate.get("candidate_id")
            if blockout_candidate is not None
            else None,
            "blockout_candidate_state_sha256": blockout_candidate.get(
                "canonical_sha256"
            )
            if blockout_candidate is not None
            else None,
            "blockout_geometry_program_sha256": blockout_artifact.get(
                "program_sha256"
            )
            if blockout_artifact is not None
            else None,
            "blockout_artifact_readback_sha256": blockout_artifact.get(
                "canonical_sha256"
            )
            if blockout_artifact is not None
            else None,
            "authoring_mesh_id": source_result.get("mesh_id")
            if source_result is not None
            else None,
            "authoring_mesh_lineage_id": source_result.get("lineage_id")
            if source_result is not None
            else None,
            "authoring_mesh_revision_sha256": source_result.get("revision_sha256")
            if source_result is not None
            else None,
            "source_binding_id": source_binding_result.get("source_binding_id")
            if source_binding_result is not None
            else None,
            "source_binding_sha256": source_binding_result.get(
                "source_binding_sha256"
            )
            if source_binding_result is not None
            else None,
            "source_binding_object_sha256": source_binding_result.get(
                "source_binding_object_sha256"
            )
            if source_binding_result is not None
            else None,
            "source_binding_exact_replay": "PASS_NOT_TOUCHED"
            if source_binding_result is not None
            else "NOT_RUN",
            "source_binding_restart_readback": "PASS_EXACT_HASH"
            if source_binding_result is not None
            else "NOT_RUN",
            "materialization_mode": materializer_result.get("materialization_mode")
            if materializer_result is not None
            else None,
            "materialized_candidate_id": materializer_result.get("candidate", {}).get(
                "candidate_id"
            )
            if materializer_result is not None
            else None,
            "materialized_candidate_state_sha256": materializer_result.get(
                "candidate", {}
            ).get("canonical_sha256")
            if materializer_result is not None
            else None,
            "materialized_artifact_sha256": materializer_result.get("candidate", {}).get(
                "prepared_object_sha256"
            )
            if materializer_result is not None
            else None,
            "materialized_artifact_readback_sha256": materializer_result.get(
                "artifact", {}
            ).get("canonical_sha256")
            if materializer_result is not None
            else None,
            "materialized_geometry_program_sha256": materializer_result.get(
                "artifact", {}
            ).get("program_sha256")
            if materializer_result is not None
            else None,
            "materialization_representation_plan_sha256": materializer_result.get(
                "representation_plan_sha256"
            )
            if materializer_result is not None
            else None,
            "materialization_replacement_node_id": materializer_result.get(
                "replacement_node_id"
            )
            if materializer_result is not None
            else None,
            "materialization_preserved_part_ids": materializer_result.get(
                "preserved_part_ids"
            )
            if materializer_result is not None
            else [],
            "materialization_exact_replay": "PASS_NOT_TOUCHED"
            if materializer_result is not None
            else "NOT_RUN",
            "materialization_restart_readback": "PASS_EXACT_HASH"
            if materializer_result is not None
            else "NOT_RUN",
            "blockout_quality_status": "structural_only"
            if source_binding_result is not None
            else "NOT_RUN",
            "visual_review_status": visual_identity.get("comparison_status")
            if visual_identity is not None
            else (
                selected_high_compare.get("comparison_status")
                if isinstance(selected_high_compare, dict)
                else "NOT_RUN"
            ),
            "quality_status": visual_identity.get("quality_status")
            if visual_identity is not None
            else (
                selected_high_compare.get("quality_status")
                if isinstance(selected_high_compare, dict)
                else (
                    "structural_only"
                    if source_binding_result is not None
                    else "NOT_RUN"
                )
            ),
            "human_review_status": "NOT_RUN",
            "unreal_5_6_status": "NOT_RUN",
            "high_stage_unlocked": False,
            "high_mesh_created": bool(high_artifact_runs),
            "production_stage_advanced": False,
            "candidate_confirmed": False,
            "version_created": False,
            "export_performed": False,
            "image_bytes_recorded": False,
            "source_path_recorded": False,
            "contact_or_signature_recorded": False,
            "persistent_user_data_scope": "isolated-local-runtime-only",
            "canonical_sha256": "",
        }
        if args.correction_pass and args.v2_blade:
            require(
                v2_blade_evidence is not None
                and baseline_materializer_result is not None
                and "baseline" in high_artifact_runs
                and "correction" in high_artifact_runs,
                "V2 correction receipt lacks baseline/correction High evidence",
            )
            baseline_high = high_artifact_runs["baseline"]
            correction_high = high_artifact_runs["correction"]
            receipt["correction_pass"] = {
                "correction_mode": "dual-curve-four-section-re-evaluation@1",
                "changed_part_ids": ["blade-body", "cutting-edge"],
                "frozen_scope": ["dragon-relief", "guard", "grip", "materials"],
                "baseline_evaluated_mesh_sha256": v2_blade_evidence[
                    "evaluated_mesh_sha256"
                ],
                "corrected_evaluated_mesh_sha256": v2_blade_evidence["correction"][
                    "evaluated_mesh_sha256"
                ],
                "baseline_materialized_candidate_id": baseline_materializer_result[
                    "candidate"
                ]["candidate_id"],
                "corrected_candidate_id": correction_v2_materialized["candidate"][
                    "candidate_id"
                ],
                "baseline_high_artifact_id": baseline_high["high_artifact"][
                    "high_artifact_id"
                ],
                "baseline_high_artifact_sha256": baseline_high["high_artifact"][
                    "high_artifact_sha256"
                ],
                "corrected_high_artifact_id": correction_high["high_artifact"][
                    "high_artifact_id"
                ],
                "corrected_high_artifact_sha256": correction_high["high_artifact"][
                    "high_artifact_sha256"
                ],
                "fixed_view_ids": [
                    "view-front",
                    "view-top",
                    "view-bottom",
                    "view-left",
                    "view-right",
                ],
                "compare_count": sum(
                    run["fixed_view_count"] for run in high_artifact_runs.values()
                ),
                "baseline_view_status": {
                    view: evidence["compare"]["comparison_status"]
                    for view, evidence in baseline_high[
                        "fixed_view_comparisons"
                    ].items()
                },
                "correction_view_status": {
                    view: evidence["compare"]["comparison_status"]
                    for view, evidence in correction_high[
                        "fixed_view_comparisons"
                    ].items()
                },
                "pass_state_status": "NOT_RUN_V2_DIRECT_HIGH_COMPARE",
                "human_review_status": "NOT_RUN",
                "engine_status": "NOT_RUN",
                "visual_quality_promotion": "NOT_PROMOTED",
            }
        elif args.correction_pass:
            require(
                correction_parent_revision is not None
                and correction_result is not None
                and correction_materializer_result is not None,
                "correction receipt lacks the durable child lineage",
            )
            receipt["correction_pass"] = {
                "correction_id": correction_payload["correction_id"]
                if correction_payload is not None
                else None,
                "profile_correction": {
                    "changed_part_ids": correction_payload["scope"]["changed_part_ids"],
                    "changed_node_ids": correction_payload["scope"]["changed_node_ids"],
                    "preserved_part_ids": correction_payload["scope"]["preserved_part_ids"],
                    "preserved_part_count": correction_payload["scope"]["preserved_part_count"],
                    "forbidden_part_ids": correction_payload["scope"]["forbidden_part_ids"],
                    "parameters_m": correction_payload["parameters_m"],
                    "immutable_parent_preserved": correction_payload["lineage"][
                        "immutable_parent_preserved"
                    ],
                }
                if correction_payload is not None
                else None,
                "baseline_materialized_candidate_id": baseline_materializer_result[
                    "candidate"
                ]["candidate_id"],
                "baseline_materialized_candidate_state_sha256": baseline_materializer_result[
                    "candidate"
                ]["canonical_sha256"],
                "baseline_materialized_geometry_program_sha256": baseline_materializer_result[
                    "artifact"
                ]["program_sha256"],
                "baseline_materialization_plan_sha256": baseline_materializer_result[
                    "representation_plan_sha256"
                ],
                "parent_revision_id": correction_parent_revision["revision_id"],
                "parent_revision_sha256": correction_parent_revision["canonical_sha256"],
                "correction_vertex_ids": correction_vertex_ids,
                "correction_delta_m": correction_delta_m,
                "correction_rationale": correction_rationale,
                "child_revision_id": correction_result["revision_id"],
                "child_revision_sha256": correction_result["revision_sha256"],
                "child_revision_object_sha256": correction_result[
                    "revision_object_sha256"
                ],
                "child_exact_replay": "PASS_NOT_TOUCHED",
                "corrected_candidate_id": correction_materializer_result[
                    "candidate"
                ]["candidate_id"],
                "corrected_candidate_state_sha256": correction_materializer_result[
                    "candidate"
                ]["canonical_sha256"],
                "corrected_geometry_program_sha256": correction_materializer_result[
                    "artifact"
                ]["program_sha256"],
                "corrected_materialization_plan_sha256": correction_materializer_result[
                    "representation_plan_sha256"
                ],
                "pass_state_status": correction_pass_state_status,
                "compare_count": len(high_artifact_runs)
                if high_artifact_runs
                else len(visual_runs),
                "baseline_visual": {
                    "candidate_id": baseline_visual_identity["candidate_id"]
                    if baseline_visual_identity is not None
                    else None,
                    "comparison_status": baseline_visual_identity["comparison_status"]
                    if baseline_visual_identity is not None
                    else "NOT_RUN",
                    "quality_status": baseline_visual_identity["quality_status"]
                    if baseline_visual_identity is not None
                    else "NOT_RUN",
                    "benchmark_eligibility": baseline_visual_identity[
                        "benchmark_eligibility"
                    ]
                    if baseline_visual_identity is not None
                    else "NOT_RUN",
                    "reference_comparison_id": baseline_visual_identity[
                        "reference_comparison_id"
                    ]
                    if baseline_visual_identity is not None
                    else None,
                    "reference_comparison_object_sha256": baseline_visual_identity[
                        "reference_comparison_object_sha256"
                    ]
                    if baseline_visual_identity is not None
                    else None,
                    "geometry_evidence": visual_runs["baseline"]["geometry"]
                    if "baseline" in visual_runs
                    else None,
                    "restart_observation_sha256": visual_restart_evidence.get(
                        "baseline", {}
                    ).get("observation_sha256"),
                    "restart_aov_sha256": visual_restart_evidence.get(
                        "baseline", {}
                    ).get("aov_pass_sha256"),
                },
                "correction_visual": {
                    "candidate_id": correction_visual_identity["candidate_id"]
                    if correction_visual_identity is not None
                    else None,
                    "comparison_status": correction_visual_identity[
                        "comparison_status"
                    ]
                    if correction_visual_identity is not None
                    else "NOT_RUN",
                    "quality_status": correction_visual_identity["quality_status"]
                    if correction_visual_identity is not None
                    else "NOT_RUN",
                    "benchmark_eligibility": correction_visual_identity[
                        "benchmark_eligibility"
                    ]
                    if correction_visual_identity is not None
                    else "NOT_RUN",
                    "reference_comparison_id": correction_visual_identity[
                        "reference_comparison_id"
                    ]
                    if correction_visual_identity is not None
                    else None,
                    "reference_comparison_object_sha256": correction_visual_identity[
                        "reference_comparison_object_sha256"
                    ]
                    if correction_visual_identity is not None
                    else None,
                    "geometry_evidence": visual_runs["correction"]["geometry"]
                    if "correction" in visual_runs
                    else None,
                    "restart_observation_sha256": visual_restart_evidence.get(
                        "correction", {}
                    ).get("observation_sha256"),
                    "restart_aov_sha256": visual_restart_evidence.get(
                        "correction", {}
                    ).get("aov_pass_sha256"),
                },
                "root_pass_state": {
                    "pass_id": pass_state_root_main["pass_id"]
                    if pass_state_root_main is not None
                    else None,
                    "pass_state_sha256": pass_state_root_main["canonical_sha256"]
                    if pass_state_root_main is not None
                    else None,
                    "pass_state_object_sha256": pass_state_root_result.get(
                        "pass_state_object_sha256"
                    )
                    if pass_state_root_result is not None
                    else None,
                    "prepare": "PASS_COMMITTED"
                    if pass_state_root_result is not None
                    else "NOT_RUN",
                    "exact_replay": "PASS_NOT_TOUCHED"
                    if pass_state_root_replay is not None
                    else "NOT_RUN",
                    "get": "PASS_FOUND"
                    if pass_state_root_get_result is not None
                    else "NOT_RUN",
                    "restart_get": "PASS_FOUND"
                    if "root" in pass_state_restart_get_results
                    else "NOT_RUN",
                },
                "child_pass_state": {
                    "pass_id": pass_state_child_main["pass_id"]
                    if pass_state_child_main is not None
                    else None,
                    "parent_pass_id": pass_state_child_main.get("parent_pass_id")
                    if pass_state_child_main is not None
                    else None,
                    "pass_state_sha256": pass_state_child_main["canonical_sha256"]
                    if pass_state_child_main is not None
                    else None,
                    "pass_state_object_sha256": pass_state_child_result.get(
                        "pass_state_object_sha256"
                    )
                    if pass_state_child_result is not None
                    else None,
                    "prepare": "PASS_COMMITTED"
                    if pass_state_child_result is not None
                    else "NOT_RUN",
                    "exact_replay": "PASS_NOT_TOUCHED"
                    if pass_state_child_replay is not None
                    else "NOT_RUN",
                    "get": "PASS_FOUND"
                    if pass_state_child_get_result is not None
                    else "NOT_RUN",
                    "restart_get": "PASS_FOUND"
                    if "correction" in pass_state_restart_get_results
                    else "NOT_RUN",
                },
                "human_review_status": "NOT_RUN",
                "engine_status": "NOT_RUN",
                "visual_quality_promotion": "NOT_PROMOTED",
            }
        if visual_identity is not None:
            require(
                visual_candidate is not None
                and visual_observation is not None
                and visual_aov_restart_readback is not None,
                "visual receipt lacks the required restart evidence",
            )
            receipt["visual_pass"] = {
                "operation_sequence": [
                    "reference_mask_prepare",
                    "reference_compare_prepare",
                ]
                if len(visual_runs) == 1
                else [
                    "reference_mask_prepare",
                    "reference_compare_prepare",
                    "reference_compare_prepare",
                ],
                "compare_count": len(visual_runs),
                "candidate_id": visual_candidate["candidate_id"],
                "candidate_artifact_sha256": visual_identity["candidate_artifact_sha256"],
                "source_geometry_evidence": {
                    "geometry_program_sha256": receipt["blockout_geometry_program_sha256"],
                    "artifact_readback_sha256": receipt["blockout_artifact_readback_sha256"],
                    "authoring_mesh_id": receipt["authoring_mesh_id"],
                    "authoring_mesh_lineage_id": receipt["authoring_mesh_lineage_id"],
                    "authoring_mesh_revision_sha256": receipt["authoring_mesh_revision_sha256"],
                },
                "selected_geometry_evidence": {
                    "candidate_state_sha256": visual_identity["candidate_state_sha256"],
                    "artifact_sha256": visual_identity["candidate_artifact_sha256"],
                    "artifact_readback_sha256": visual_identity["artifact_readback_sha256"],
                    "geometry_program_sha256": visual_identity["geometry_program_sha256"],
                },
                "view_id": visual_identity["view_id"],
                "reference_view_spec_sha256": visual_identity["reference_view_spec_sha256"],
                "contour_point_count": 63,
                "contour_coordinate_space": "full-image-normalized",
                "user_confirmed": False,
                "target_sha256": visual_identity["target_sha256"],
                "mask_sha256": visual_identity["mask_sha256"],
                "camera_hash": visual_identity["camera_hash"],
                "camera_object_sha256": visual_identity["camera_object_sha256"],
                "render_set_id": visual_identity["render_set_id"],
                "render_set_sha256": visual_identity["render_set_sha256"],
                "render_set_object_sha256": visual_identity["render_set_object_sha256"],
                "render_worker_build_cohort_sha256": visual_identity[
                    "render_worker_build_cohort_sha256"
                ],
                "aov_pass_sha256": visual_identity["aov_pass_sha256"],
                "reference_comparison_id": visual_identity["reference_comparison_id"],
                "reference_comparison_sha256": visual_identity[
                    "reference_comparison_sha256"
                ],
                "reference_comparison_object_sha256": visual_identity[
                    "reference_comparison_object_sha256"
                ],
                "quality_report_id": visual_identity["quality_report_id"],
                "quality_report_sha256": visual_identity["quality_report_sha256"],
                "quality_report_object_sha256": visual_identity[
                    "quality_report_object_sha256"
                ],
                "comparison_status": visual_identity["comparison_status"],
                "quality_status": visual_identity["quality_status"],
                "benchmark_eligibility": visual_identity["benchmark_eligibility"],
                "exact_replay": "NOT_RUN_COMPARE_IS_SINGLE_SHOT",
                "restart_readback": "PASS_EXACT_HASH",
                "restart_observation_sha256": verify_canonical_object(
                    reopened_observation,
                    "canonical_sha256",
                    "reopened AgenticSceneObserveResult",
                ),
                "restart_aov_sha256": visual_aov_restart_readback,
                "visual_quality_promotion": "NOT_PROMOTED",
                "human_review_status": "NOT_RUN",
                "engine_status": "NOT_RUN",
            }
        write_receipt(args.receipt, receipt)
        output = {
            "status": receipt["status"],
            "build_cohort_sha256": args.expected_build_cohort,
            "project_id": project_id,
            "reference_id": reference["reference_id"],
            "successor_brief_id": successor["brief_id"],
            "successor_brief_sha256": successor["brief_sha256"],
            "intent_bundle_id": receipt["intent_bundle_id"],
            "intent_bundle_sha256": receipt["intent_bundle_sha256"],
            "blockout_candidate_id": receipt["blockout_candidate_id"],
            "authoring_mesh_id": receipt["authoring_mesh_id"],
            "source_binding_id": receipt["source_binding_id"],
            "source_binding_sha256": receipt["source_binding_sha256"],
            "materialized_candidate_id": receipt["materialized_candidate_id"],
            "materialization_mode": receipt["materialization_mode"],
            "authoring_eligibility": successor["authoring_eligibility"],
            "hq_360_status": receipt["hq_360_status"],
            "receipt": str(args.receipt),
        }
        if visual_identity is not None:
            output["visual_pass"] = {
                "status": visual_identity["comparison_status"],
                "quality_status": visual_identity["quality_status"],
                "benchmark_eligibility": visual_identity["benchmark_eligibility"],
                "view_id": visual_identity["view_id"],
                "aov_count": len(visual_identity["aov_pass_sha256"]),
                "compare_count": len(visual_runs),
                "exact_replay": "NOT_RUN_COMPARE_IS_SINGLE_SHOT",
                "restart_readback": "PASS_EXACT_HASH",
                "visual_quality_promotion": "NOT_PROMOTED",
                "user_confirmed": False,
            }
        if low_quad_evidence is not None:
            output["low_quad"] = {
                "status": receipt["status"],
                "component_set": low_quad_evidence["component_set"],
                "artifact_sha256": {
                    part_id: value["prepare"]["artifact_sha256"]
                    for part_id, value in low_quad_evidence["components"].items()
                },
                "artifact_object_sha256": {
                    part_id: value["prepare"]["artifact_object_sha256"]
                    for part_id, value in low_quad_evidence["components"].items()
                },
                "restart_readback": low_quad_evidence.get("restart_status"),
            }
        if uv_bake_v2_evidence is not None:
            output["uv_bake_v2"] = {
                "status": receipt["status"],
                "aggregate_id": uv_bake_v2_evidence["prepare"]["aggregate_id"],
                "component_set": ["blade-body", "cutting-edge"],
                "components": uv_bake_v2_evidence["get"]["components"],
                "prepare": "PASS_COMMITTED",
                "replay": "PASS_EXACT_REPLAY",
                "get": "PASS_FOUND",
                "restart_status": uv_bake_v2_evidence.get("restart_status"),
                "quality_status": "structural_only",
                "visual_status": "NOT_PROVEN",
            }
        print(json.dumps(output, ensure_ascii=False, sort_keys=True))
        return 0
    finally:
        if client is not None:
            client.close()
        if runtime is not None and ready is not None and ready_path is not None:
            shutdown_runtime(ready, ready_path, runtime)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except GateFailure as error:
        raise SystemExit(f"Weaponry Knife Brief live probe failed: {error}") from error
