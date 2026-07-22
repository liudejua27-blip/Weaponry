#!/usr/bin/env python3
"""Validate paired R007B workbench evidence without scoring visual quality.

The producer must run the real packaged workbench once for each R007B evidence
class and write ``manifest.json`` plus paired reference/result PNG captures.
This validator performs only fail-closed engineering checks: immutable lineage,
distinct per-class analysis/plan/effect/ceiling, one renderer, non-placeholder
C106 readback, real PNG bytes, and fresh timestamps.  It never starts a server,
browser, desktop process or Provider, and it can never satisfy M108B.
"""

from __future__ import annotations

import argparse
import copy
from datetime import datetime, timedelta, timezone
import hashlib
import json
from pathlib import Path, PurePosixPath
import struct
import sys
import tempfile
from typing import Any, Mapping, NoReturn
import zlib


SCHEMA = "ForgeCADR007BWorkbenchVisualEvidence@1"
REPORT_SCHEMA = "ForgeCADR007BWorkbenchVisualEvidenceGate@1"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
REFERENCE_CLASSES = {
    "single_image": "single_image_visible_surface_only",
    "multi_view_contact_sheet": "multi_view_image_visible_surface_only",
    "strict_glb_readback": "strict_glb_readback_visible_bounds_only",
}
C106_ROOTS = {
    "recipe_c106_arm_desktop_assistant",
    "recipe_c106_arm_gallery_industrial",
    "recipe_c106_arm_service_display",
}
REQUIRED_ANALYSIS_FACTS = {"retained", "intentionally_changed", "unresolved"}
MIN_TRIANGLES = 1_000
MIN_PARTS = 7
MIN_ZONES = 4
MIN_PNG_WIDTH = 320
MIN_PNG_HEIGHT = 240
MAX_CLOCK_SKEW = timedelta(minutes=5)


class EvidenceFailure(ValueError):
    pass


def fail(code: str) -> NoReturn:
    raise EvidenceFailure(code)


def require_mapping(value: object, code: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        fail(code)
    return value


def require_string(value: object, code: str) -> str:
    if not isinstance(value, str) or not value:
        fail(code)
    return value


def require_int(value: object, minimum: int, code: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < minimum:
        fail(code)
    return value


def require_hash(value: object, code: str) -> str:
    candidate = require_string(value, code)
    if len(candidate) != 64 or any(char not in "0123456789abcdef" for char in candidate):
        fail(code)
    return candidate


def require_unique_strings(value: object, code: str, *, minimum: int = 1) -> list[str]:
    if (
        not isinstance(value, list)
        or len(value) < minimum
        or any(not isinstance(item, str) or not item for item in value)
        or len(value) != len(set(value))
    ):
        fail(code)
    return list(value)


def parse_time(value: object, code: str) -> datetime:
    raw = require_string(value, code)
    try:
        parsed = datetime.fromisoformat(raw.replace("Z", "+00:00"))
    except ValueError:
        fail(code)
    if parsed.tzinfo is None:
        fail(code)
    return parsed.astimezone(timezone.utc)


def read_json(path: Path, code: str) -> Mapping[str, Any]:
    try:
        if path.is_symlink():
            fail(code)
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError):
        fail(code)
    return require_mapping(value, code)


def safe_relative_file(value: object, suffix: str, code: str) -> str:
    raw = require_string(value, code)
    candidate = PurePosixPath(raw)
    if (
        candidate.is_absolute()
        or "\\" in raw
        or ".." in candidate.parts
        or raw.startswith("./")
        or not raw.endswith(suffix)
        or len(candidate.parts) < 2
    ):
        fail(code)
    return raw


def evidence_file(root: Path, relative: str, code: str) -> Path:
    candidate = root.joinpath(*PurePosixPath(relative).parts)
    try:
        candidate.relative_to(root)
    except ValueError:
        fail(code)
    if candidate.is_symlink() or not candidate.is_file():
        fail(code)
    return candidate


def _paeth(left: int, up: int, upper_left: int) -> int:
    estimate = left + up - upper_left
    left_distance = abs(estimate - left)
    up_distance = abs(estimate - up)
    upper_left_distance = abs(estimate - upper_left)
    if left_distance <= up_distance and left_distance <= upper_left_distance:
        return left
    if up_distance <= upper_left_distance:
        return up
    return upper_left


def png_pixels(path: Path, code: str) -> tuple[int, int, bytes, bytes]:
    """Return PNG dimensions, file bytes and canonical RGBA8 pixels."""

    try:
        raw = path.read_bytes()
    except OSError:
        fail(code)
    if len(raw) < 64 or raw[:8] != PNG_SIGNATURE:
        fail(code)
    offset = 8
    width = height = bit_depth = color_type = interlace = None
    idat = bytearray()
    saw_iend = False
    while offset + 12 <= len(raw):
        length = struct.unpack(">I", raw[offset : offset + 4])[0]
        chunk_type = raw[offset + 4 : offset + 8]
        chunk_end = offset + 12 + length
        if chunk_end > len(raw):
            fail(code)
        chunk = raw[offset + 8 : offset + 8 + length]
        declared_crc = struct.unpack(">I", raw[offset + 8 + length : chunk_end])[0]
        if zlib.crc32(chunk_type + chunk) & 0xFFFFFFFF != declared_crc:
            fail(code)
        if chunk_type == b"IHDR":
            if length != 13 or width is not None:
                fail(code)
            width, height, bit_depth, color_type, compression, filtering, interlace = struct.unpack(">IIBBBBB", chunk)
            if compression != 0 or filtering != 0:
                fail(code)
        elif chunk_type == b"IDAT":
            idat.extend(chunk)
        elif chunk_type == b"IEND":
            saw_iend = True
            break
        offset = chunk_end
    channels = {0: 1, 2: 3, 4: 2, 6: 4}.get(color_type)
    if (
        not saw_iend
        or width is None
        or height is None
        or width < 1
        or height < 1
        or bit_depth != 8
        or channels is None
        or interlace != 0
        or not idat
    ):
        fail(code)
    stride = width * channels
    try:
        filtered = zlib.decompress(bytes(idat))
    except zlib.error:
        fail(code)
    if len(filtered) != (stride + 1) * height:
        fail(code)
    rows: list[bytes] = []
    cursor = 0
    prior = bytes(stride)
    for _ in range(height):
        filter_kind = filtered[cursor]
        cursor += 1
        source = filtered[cursor : cursor + stride]
        cursor += stride
        restored = bytearray(stride)
        for index, byte in enumerate(source):
            left = restored[index - channels] if index >= channels else 0
            up = prior[index]
            upper_left = prior[index - channels] if index >= channels else 0
            if filter_kind == 0:
                predictor = 0
            elif filter_kind == 1:
                predictor = left
            elif filter_kind == 2:
                predictor = up
            elif filter_kind == 3:
                predictor = (left + up) // 2
            elif filter_kind == 4:
                predictor = _paeth(left, up, upper_left)
            else:
                fail(code)
            restored[index] = (byte + predictor) & 0xFF
        prior = bytes(restored)
        rows.append(prior)
    rgba = bytearray(width * height * 4)
    output = 0
    for row in rows:
        for index in range(0, len(row), channels):
            if color_type == 0:
                red = green = blue = row[index]
                alpha = 255
            elif color_type == 2:
                red, green, blue = row[index : index + 3]
                alpha = 255
            elif color_type == 4:
                red = green = blue = row[index]
                alpha = row[index + 1]
            else:
                red, green, blue, alpha = row[index : index + 4]
            rgba[output : output + 4] = bytes((red, green, blue, alpha))
            output += 4
    return width, height, raw, bytes(rgba)


def validate_capture(
    root: Path,
    capture: Mapping[str, Any],
    *,
    capture_kind: str,
    renderer_generation: int,
    expected_hash: str,
    expected_result_version: str | None,
) -> dict[str, Any]:
    if capture.get("capture_kind") != capture_kind:
        fail("R007B_VISUAL_CAPTURE_KIND_INVALID")
    relative = safe_relative_file(capture.get("relative_path"), ".png", "R007B_VISUAL_CAPTURE_PATH_INVALID")
    path = evidence_file(root, relative, "R007B_VISUAL_CAPTURE_MISSING")
    width, height, raw, rgba = png_pixels(path, "R007B_VISUAL_CAPTURE_PNG_INVALID")
    if width < MIN_PNG_WIDTH or height < MIN_PNG_HEIGHT or len(raw) < 1_024:
        fail("R007B_VISUAL_CAPTURE_TOO_SMALL")
    if (
        capture.get("width") != width
        or capture.get("height") != height
        or capture.get("byte_size") != len(raw)
        or require_hash(capture.get("sha256"), "R007B_VISUAL_CAPTURE_HASH_INVALID") != hashlib.sha256(raw).hexdigest()
        or capture.get("renderer_generation") != renderer_generation
        or capture.get("lineage_sha256") != expected_hash
    ):
        fail("R007B_VISUAL_CAPTURE_READBACK_DRIFT")
    if expected_result_version is None:
        if capture.get("asset_version_id") is not None:
            fail("R007B_VISUAL_REFERENCE_VERSION_INVALID")
    elif capture.get("asset_version_id") != expected_result_version:
        fail("R007B_VISUAL_RESULT_VERSION_DRIFT")
    pixel_count = width * height
    alpha_visible = sum(1 for index in range(3, len(rgba), 4) if rgba[index] > 0)
    sample_stride = max(4, (len(rgba) // 4_096 // 4) * 4)
    sampled_colors = {rgba[index : index + 4] for index in range(0, len(rgba), sample_stride)}
    if alpha_visible < pixel_count // 10 or len(sampled_colors) < 8:
        fail("R007B_VISUAL_CAPTURE_EMPTY_OR_FLAT")
    return {
        "relative_path": relative,
        "file_sha256": hashlib.sha256(raw).hexdigest(),
        "pixel_sha256": hashlib.sha256(struct.pack(">II", width, height) + rgba).hexdigest(),
        "width": width,
        "height": height,
    }


def validate_renderer(value: object) -> int:
    renderer = require_mapping(value, "R007B_VISUAL_RENDERER_INVALID")
    generation = require_int(renderer.get("renderer_generation"), 1, "R007B_VISUAL_RENDERER_INVALID")
    if (
        renderer.get("renderer_id") != "ForgeCADWorkbenchRenderer@1"
        or renderer.get("canvas_count") != 1
        or renderer.get("active_webgl_contexts") != 1
        or renderer.get("reference_renderer_generation") != generation
        or renderer.get("result_renderer_generation") != generation
        or renderer.get("same_renderer") is not True
        or renderer.get("load_state") != "ready"
    ):
        fail("R007B_VISUAL_RENDERER_INVALID")
    return generation


def validate_geometry(value: object, *, result_glb_sha256: str) -> dict[str, Any]:
    geometry = require_mapping(value, "R007B_VISUAL_GEOMETRY_INVALID")
    operations = require_unique_strings(geometry.get("shape_operation_kinds"), "R007B_VISUAL_GEOMETRY_OPERATIONS_INVALID")
    triangle_count = require_int(geometry.get("triangle_count"), 1, "R007B_VISUAL_TRIANGLE_COUNT_INVALID")
    if triangle_count < MIN_TRIANGLES or triangle_count == 4:
        fail("R007B_VISUAL_PLACEHOLDER_TRIANGLES_REJECTED")
    if (
        geometry.get("artifact_profile_id") != "production_concept"
        or geometry.get("asset_kind") != "c106_robotic_arm"
        or geometry.get("root_recipe_id") not in C106_ROOTS
        or geometry.get("root_operation_kind") == "wedge"
        or set(operations) == {"wedge"}
        or require_int(geometry.get("part_count"), 1, "R007B_VISUAL_PART_COUNT_INVALID") < MIN_PARTS
        or require_int(geometry.get("material_zone_count"), 1, "R007B_VISUAL_ZONE_COUNT_INVALID") < MIN_ZONES
        or require_hash(geometry.get("glb_sha256"), "R007B_VISUAL_GEOMETRY_HASH_INVALID") != result_glb_sha256
    ):
        fail("R007B_VISUAL_PLACEHOLDER_GEOMETRY_REJECTED")
    return {
        "triangle_count": triangle_count,
        "part_count": geometry["part_count"],
        "material_zone_count": geometry["material_zone_count"],
        "root_recipe_id": geometry["root_recipe_id"],
    }


def validate_run(root: Path, value: object, *, generated_at: datetime, now: datetime, max_age: timedelta) -> dict[str, Any]:
    run = require_mapping(value, "R007B_VISUAL_RUN_INVALID")
    reference_class = require_string(run.get("reference_class"), "R007B_VISUAL_REFERENCE_CLASS_INVALID")
    expected_ceiling = REFERENCE_CLASSES.get(reference_class)
    if expected_ceiling is None or run.get("capability_ceiling") != expected_ceiling:
        fail("R007B_VISUAL_CAPABILITY_CEILING_INVALID")
    captured_at = parse_time(run.get("captured_at"), "R007B_VISUAL_CAPTURE_TIME_INVALID")
    if captured_at > generated_at + MAX_CLOCK_SKEW or captured_at > now + MAX_CLOCK_SKEW or now - captured_at > max_age:
        fail("R007B_VISUAL_REPORT_STALE")
    workbench = require_mapping(run.get("workbench"), "R007B_VISUAL_WORKBENCH_INVALID")
    if (
        workbench.get("runtime_kind") != "packaged_tauri_webview"
        or workbench.get("real_workbench") is not True
        or workbench.get("fixture_or_proxy_used") is not False
        or workbench.get("provider_network_calls") != 0
        or workbench.get("credential_reads") != 0
    ):
        fail("R007B_VISUAL_WORKBENCH_INVALID")
    run_id = require_string(run.get("run_id"), "R007B_VISUAL_RUN_ID_INVALID")
    analysis = require_mapping(run.get("analysis"), "R007B_VISUAL_ANALYSIS_INVALID")
    plan = require_mapping(run.get("plan"), "R007B_VISUAL_PLAN_INVALID")
    effect = require_mapping(run.get("sealed_effect"), "R007B_VISUAL_EFFECT_INVALID")
    analysis_id = require_string(analysis.get("analysis_id"), "R007B_VISUAL_ANALYSIS_INVALID")
    analysis_sha = require_hash(analysis.get("sha256"), "R007B_VISUAL_ANALYSIS_INVALID")
    evidence_id = require_string(analysis.get("evidence_id"), "R007B_VISUAL_ANALYSIS_INVALID")
    source_sha = require_hash(analysis.get("source_object_sha256"), "R007B_VISUAL_SOURCE_HASH_INVALID")
    if analysis.get("fidelity_ceiling") != expected_ceiling:
        fail("R007B_VISUAL_ANALYSIS_CEILING_DRIFT")
    for key in REQUIRED_ANALYSIS_FACTS:
        require_unique_strings(analysis.get(key), "R007B_VISUAL_ANALYSIS_FACTS_INVALID")
    plan_id = require_string(plan.get("rebuild_plan_id"), "R007B_VISUAL_PLAN_INVALID")
    plan_sha = require_hash(plan.get("sha256"), "R007B_VISUAL_PLAN_INVALID")
    base_version = require_string(plan.get("base_asset_version_id"), "R007B_VISUAL_PLAN_INVALID")
    result_version = require_string(plan.get("confirmed_asset_version_id"), "R007B_VISUAL_PLAN_INVALID")
    if (
        plan.get("status") != "confirmed"
        or plan.get("analysis_id") != analysis_id
        or plan.get("evidence_id") != evidence_id
        or plan.get("source_object_sha256") != source_sha
        or plan.get("capability_ceiling") != expected_ceiling
        or base_version == result_version
    ):
        fail("R007B_VISUAL_PLAN_LINEAGE_DRIFT")
    change_set_id = require_string(effect.get("change_set_id"), "R007B_VISUAL_EFFECT_INVALID")
    effect_sha = require_hash(effect.get("sha256"), "R007B_VISUAL_EFFECT_INVALID")
    operations = effect.get("operations")
    if not isinstance(operations, list) or not operations:
        fail("R007B_VISUAL_EFFECT_INVALID")
    operation_hashes: list[str] = []
    has_adornment = False
    for operation_value in operations:
        operation = require_mapping(operation_value, "R007B_VISUAL_EFFECT_INVALID")
        operation_hashes.append(require_hash(operation.get("sha256"), "R007B_VISUAL_EFFECT_INVALID"))
        if operation.get("op") == "apply_surface_adornment":
            require_hash(operation.get("program_sha256"), "R007B_VISUAL_EFFECT_INVALID")
            has_adornment = True
    if (
        not has_adornment
        or len(operation_hashes) != len(set(operation_hashes))
        or effect.get("base_asset_version_id") != base_version
        or effect.get("resulting_asset_version_id") != result_version
        or effect.get("status") != "confirmed"
    ):
        fail("R007B_VISUAL_EFFECT_LINEAGE_DRIFT")
    result_glb_sha = require_hash(run.get("result_glb_sha256"), "R007B_VISUAL_RESULT_GLB_INVALID")
    if result_glb_sha == source_sha:
        fail("R007B_VISUAL_SOURCE_RESULT_IDENTITY_INVALID")
    geometry = validate_geometry(run.get("geometry_readback"), result_glb_sha256=result_glb_sha)
    renderer_generation = validate_renderer(run.get("renderer"))
    screenshots = require_mapping(run.get("screenshots"), "R007B_VISUAL_SCREENSHOTS_INVALID")
    reference_capture = validate_capture(
        root,
        require_mapping(screenshots.get("reference"), "R007B_VISUAL_REFERENCE_SCREENSHOT_MISSING"),
        capture_kind="reference",
        renderer_generation=renderer_generation,
        expected_hash=source_sha,
        expected_result_version=None,
    )
    result_capture = validate_capture(
        root,
        require_mapping(screenshots.get("result"), "R007B_VISUAL_RESULT_SCREENSHOT_MISSING"),
        capture_kind="result",
        renderer_generation=renderer_generation,
        expected_hash=result_glb_sha,
        expected_result_version=result_version,
    )
    if (
        reference_capture["relative_path"] == result_capture["relative_path"]
        or reference_capture["file_sha256"] == result_capture["file_sha256"]
        or reference_capture["pixel_sha256"] == result_capture["pixel_sha256"]
    ):
        fail("R007B_VISUAL_IDENTICAL_SCREENSHOTS_REJECTED")
    return {
        "reference_class": reference_class,
        "run_id": run_id,
        "analysis_id": analysis_id,
        "analysis_sha256": analysis_sha,
        "plan_id": plan_id,
        "plan_sha256": plan_sha,
        "change_set_id": change_set_id,
        "effect_sha256": effect_sha,
        "source_sha256": source_sha,
        "result_asset_version_id": result_version,
        "result_glb_sha256": result_glb_sha,
        "capability_ceiling": expected_ceiling,
        "reference_capture": reference_capture,
        "result_capture": result_capture,
        "geometry": geometry,
        "renderer_generation": renderer_generation,
    }


def validate_manifest(root: Path, value: object, *, now: datetime, max_age: timedelta) -> dict[str, Any]:
    manifest = require_mapping(value, "R007B_VISUAL_MANIFEST_INVALID")
    if (
        manifest.get("schema_version") != SCHEMA
        or manifest.get("status") != "pass"
        or manifest.get("visual_fidelity_validated") is not False
        or manifest.get("formal_eligible") is not False
        or manifest.get("m108b_status") != "blocked"
    ):
        fail("R007B_VISUAL_MANIFEST_INVALID")
    generated_at = parse_time(manifest.get("generated_at"), "R007B_VISUAL_GENERATED_AT_INVALID")
    if generated_at > now + MAX_CLOCK_SKEW or now - generated_at > max_age:
        fail("R007B_VISUAL_REPORT_STALE")
    runs_value = manifest.get("runs")
    if not isinstance(runs_value, list) or len(runs_value) != len(REFERENCE_CLASSES):
        fail("R007B_VISUAL_RUN_COUNT_INVALID")
    runs = [validate_run(root, item, generated_at=generated_at, now=now, max_age=max_age) for item in runs_value]
    if {run["reference_class"] for run in runs} != set(REFERENCE_CLASSES):
        fail("R007B_VISUAL_REFERENCE_CLASS_COVERAGE_INVALID")
    distinct_fields = {
        "run_id": "R007B_VISUAL_RUNS_NOT_INDEPENDENT",
        "analysis_id": "R007B_VISUAL_ANALYSES_NOT_DISTINCT",
        "analysis_sha256": "R007B_VISUAL_ANALYSES_NOT_DISTINCT",
        "plan_id": "R007B_VISUAL_PLANS_NOT_DISTINCT",
        "plan_sha256": "R007B_VISUAL_PLANS_NOT_DISTINCT",
        "change_set_id": "R007B_VISUAL_EFFECTS_NOT_DISTINCT",
        "effect_sha256": "R007B_VISUAL_EFFECTS_NOT_DISTINCT",
        "source_sha256": "R007B_VISUAL_SOURCES_NOT_DISTINCT",
        "result_asset_version_id": "R007B_VISUAL_RESULTS_NOT_DISTINCT",
        "result_glb_sha256": "R007B_VISUAL_RESULTS_NOT_DISTINCT",
        "capability_ceiling": "R007B_VISUAL_CEILINGS_NOT_DISTINCT",
    }
    for field, code in distinct_fields.items():
        if len({run[field] for run in runs}) != len(runs):
            fail(code)
    return {
        "schema_version": REPORT_SCHEMA,
        "status": "pass",
        "reference_classes": sorted(REFERENCE_CLASSES),
        "run_count": len(runs),
        "paired_capture_count": len(runs),
        "distinct_analysis_count": len({run["analysis_sha256"] for run in runs}),
        "distinct_plan_count": len({run["plan_sha256"] for run in runs}),
        "distinct_effect_count": len({run["effect_sha256"] for run in runs}),
        "distinct_result_count": len({run["result_glb_sha256"] for run in runs}),
        "single_renderer_per_run": True,
        "placeholder_geometry_rejected": True,
        "identical_screenshots_rejected": True,
        "freshness_validated": True,
        "visual_fidelity_validated": False,
        "formal_eligible": False,
        "m108b_status": "blocked",
        "runs": runs,
    }


def _png_chunk(kind: bytes, payload: bytes) -> bytes:
    return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", zlib.crc32(kind + payload) & 0xFFFFFFFF)


def _test_png(width: int, height: int, seed: int) -> bytes:
    rows = bytearray()
    for y in range(height):
        rows.append(0)
        for x in range(width):
            rows.extend(((x + seed) % 256, (y * 3 + seed) % 256, (x + y + seed * 7) % 256, 255))
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return PNG_SIGNATURE + _png_chunk(b"IHDR", ihdr) + _png_chunk(b"IDAT", zlib.compress(bytes(rows), 6)) + _png_chunk(b"IEND", b"")


def _digest(label: str) -> str:
    return hashlib.sha256(label.encode("utf-8")).hexdigest()


def _self_test_manifest(root: Path, now: datetime) -> dict[str, Any]:
    runs: list[dict[str, Any]] = []
    for index, (reference_class, ceiling) in enumerate(REFERENCE_CLASSES.items(), start=1):
        capture_dir = root / "captures" / reference_class
        capture_dir.mkdir(parents=True, exist_ok=True)
        reference_bytes = _test_png(320, 240, index)
        result_bytes = _test_png(320, 240, index + 30)
        reference_path = capture_dir / "reference.png"
        result_path = capture_dir / "result.png"
        reference_path.write_bytes(reference_bytes)
        result_path.write_bytes(result_bytes)
        source_sha = _digest(f"source-{index}")
        result_sha = _digest(f"result-{index}")
        base_version = f"assetver_base_{index}"
        result_version = f"assetver_result_{index}"
        generation = 10 + index
        runs.append({
            "reference_class": reference_class,
            "run_id": f"r007b-visual-self-test-{index}",
            "captured_at": now.isoformat().replace("+00:00", "Z"),
            "capability_ceiling": ceiling,
            "workbench": {
                "runtime_kind": "packaged_tauri_webview",
                "real_workbench": True,
                "fixture_or_proxy_used": False,
                "provider_network_calls": 0,
                "credential_reads": 0,
            },
            "analysis": {
                "analysis_id": f"refsrfanalysis_self_{index}",
                "sha256": _digest(f"analysis-{index}"),
                "evidence_id": f"refevid_self_{index}",
                "source_object_sha256": source_sha,
                "fidelity_ceiling": ceiling,
                "retained": ["silhouette"],
                "intentionally_changed": ["surface_adornment_normalization"],
                "unresolved": ["hidden_structure"],
            },
            "plan": {
                "rebuild_plan_id": f"rebuildplan_self_{index}",
                "sha256": _digest(f"plan-{index}"),
                "analysis_id": f"refsrfanalysis_self_{index}",
                "evidence_id": f"refevid_self_{index}",
                "source_object_sha256": source_sha,
                "base_asset_version_id": base_version,
                "confirmed_asset_version_id": result_version,
                "capability_ceiling": ceiling,
                "status": "confirmed",
            },
            "sealed_effect": {
                "change_set_id": f"changeset_self_{index}",
                "sha256": _digest(f"effect-{index}"),
                "base_asset_version_id": base_version,
                "resulting_asset_version_id": result_version,
                "status": "confirmed",
                "operations": [{
                    "op": "apply_surface_adornment",
                    "sha256": _digest(f"operation-{index}"),
                    "program_sha256": _digest(f"program-{index}"),
                }],
            },
            "result_glb_sha256": result_sha,
            "geometry_readback": {
                "artifact_profile_id": "production_concept",
                "asset_kind": "c106_robotic_arm",
                "root_recipe_id": list(sorted(C106_ROOTS))[index - 1],
                "root_operation_kind": "revolve",
                "shape_operation_kinds": ["revolve", "sweep", "surface_panel"],
                "triangle_count": 14_000 + index,
                "part_count": 13,
                "material_zone_count": 10,
                "glb_sha256": result_sha,
            },
            "renderer": {
                "renderer_id": "ForgeCADWorkbenchRenderer@1",
                "renderer_generation": generation,
                "reference_renderer_generation": generation,
                "result_renderer_generation": generation,
                "same_renderer": True,
                "canvas_count": 1,
                "active_webgl_contexts": 1,
                "load_state": "ready",
            },
            "screenshots": {
                "reference": {
                    "capture_kind": "reference",
                    "relative_path": f"captures/{reference_class}/reference.png",
                    "sha256": hashlib.sha256(reference_bytes).hexdigest(),
                    "byte_size": len(reference_bytes),
                    "width": 320,
                    "height": 240,
                    "renderer_generation": generation,
                    "lineage_sha256": source_sha,
                    "asset_version_id": None,
                },
                "result": {
                    "capture_kind": "result",
                    "relative_path": f"captures/{reference_class}/result.png",
                    "sha256": hashlib.sha256(result_bytes).hexdigest(),
                    "byte_size": len(result_bytes),
                    "width": 320,
                    "height": 240,
                    "renderer_generation": generation,
                    "lineage_sha256": result_sha,
                    "asset_version_id": result_version,
                },
            },
        })
    return {
        "schema_version": SCHEMA,
        "status": "pass",
        "generated_at": now.isoformat().replace("+00:00", "Z"),
        "visual_fidelity_validated": False,
        "formal_eligible": False,
        "m108b_status": "blocked",
        "runs": runs,
    }


def _make_self_test_screenshots_identical(value: dict[str, Any]) -> None:
    run = value["runs"][0]
    reference = copy.deepcopy(run["screenshots"]["reference"])
    reference.update({
        "capture_kind": "result",
        "asset_version_id": run["plan"]["confirmed_asset_version_id"],
        "lineage_sha256": run["result_glb_sha256"],
    })
    run["screenshots"]["result"] = reference


def self_test() -> int:
    now = datetime(2026, 7, 19, 12, 0, tzinfo=timezone.utc)
    mutations = {
        "missing_reference": (
            lambda value: value["runs"][0]["screenshots"].pop("reference"),
            "R007B_VISUAL_REFERENCE_SCREENSHOT_MISSING",
        ),
        "identical_screenshot": (
            _make_self_test_screenshots_identical,
            "R007B_VISUAL_IDENTICAL_SCREENSHOTS_REJECTED",
        ),
        "wedge_placeholder": (
            lambda value: value["runs"][0]["geometry_readback"].__setitem__("root_operation_kind", "wedge"),
            "R007B_VISUAL_PLACEHOLDER_GEOMETRY_REJECTED",
        ),
        "four_triangles": (
            lambda value: value["runs"][0]["geometry_readback"].__setitem__("triangle_count", 4),
            "R007B_VISUAL_PLACEHOLDER_TRIANGLES_REJECTED",
        ),
        "stale_report": (
            lambda value: value.__setitem__("generated_at", "2026-07-17T12:00:00Z"),
            "R007B_VISUAL_REPORT_STALE",
        ),
        "duplicate_analysis": (
            lambda value: value["runs"][1]["analysis"].__setitem__("sha256", value["runs"][0]["analysis"]["sha256"]),
            "R007B_VISUAL_ANALYSES_NOT_DISTINCT",
        ),
    }
    with tempfile.TemporaryDirectory(prefix="forgecad-r007b-visual-self-test-") as temporary:
        root = Path(temporary)
        valid = _self_test_manifest(root, now)
        report = validate_manifest(root, valid, now=now, max_age=timedelta(hours=24))
        if report.get("status") != "pass" or report.get("paired_capture_count") != 3:
            fail("R007B_VISUAL_SELF_TEST_VALID_REJECTED")
        for name, (mutation, expected) in mutations.items():
            candidate = copy.deepcopy(valid)
            mutation(candidate)
            try:
                validate_manifest(root, candidate, now=now, max_age=timedelta(hours=24))
            except EvidenceFailure as exc:
                if str(exc) != expected:
                    raise EvidenceFailure(f"R007B_VISUAL_SELF_TEST_{name}:{exc}") from exc
            else:
                fail(f"R007B_VISUAL_SELF_TEST_{name}_ACCEPTED")
    print(json.dumps({
        "schema_version": REPORT_SCHEMA,
        "status": "pass",
        "mode": "self_test",
        "negative_cases": sorted(mutations),
        "visual_fidelity_validated": False,
        "formal_eligible": False,
        "m108b_status": "blocked",
    }, sort_keys=True))
    return 0


def main(evidence_root: Path, *, now: datetime, max_age: timedelta) -> int:
    root = evidence_root.resolve()
    manifest = read_json(root / "manifest.json", "R007B_VISUAL_MANIFEST_READ_FAILED")
    print(json.dumps(validate_manifest(root, manifest, now=now, max_age=max_age), ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Validate fresh paired R007B packaged-workbench visual evidence.")
    parser.add_argument("--evidence", type=Path)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--now", help="UTC ISO-8601 override for deterministic audit")
    parser.add_argument("--max-age-hours", type=float, default=24.0)
    arguments = parser.parse_args()
    try:
        if arguments.self_test:
            raise SystemExit(self_test())
        if arguments.evidence is None or arguments.max_age_hours <= 0 or arguments.max_age_hours > 168:
            parser.error("--evidence and --max-age-hours within (0, 168] are required")
        audit_now = parse_time(arguments.now, "R007B_VISUAL_NOW_INVALID") if arguments.now else datetime.now(timezone.utc)
        raise SystemExit(main(arguments.evidence, now=audit_now, max_age=timedelta(hours=arguments.max_age_hours)))
    except EvidenceFailure as exc:
        print(json.dumps({
            "schema_version": REPORT_SCHEMA,
            "status": "fail",
            "error": str(exc),
            "visual_fidelity_validated": False,
            "formal_eligible": False,
            "m108b_status": "blocked",
        }, ensure_ascii=False, sort_keys=True), file=sys.stderr)
        raise SystemExit(1) from None
