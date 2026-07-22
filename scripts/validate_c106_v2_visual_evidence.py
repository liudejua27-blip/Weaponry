#!/usr/bin/env python3
"""Fail-closed automatic visual-evidence gate for one packaged C106 v2 asset.

This validator intentionally does *not* score appearance.  Its sole purpose is
to bind a future same-workbench capture to the persisted packaged C106 v2
lineage and ensure that the capture contains the minimum mechanical/PBR facts
needed for a human visual review.  It therefore always emits
``formal_eligible=false`` and can never satisfy M108B's independent-human
review requirement.

Input contract (all paths below are relative to ``--evidence``):

``manifest.json`` must be ``ForgeCADC106V2VisualCapture@1`` and contain four
captures named ``front``, ``side``, ``three_quarter`` and ``detail_pbr``.  The
first three are same-asset overview captures; the last one is a close PBR
capture.  Every capture repeats the exact asset version, GLB hash, production
profile and one-canvas runtime facts.  Screenshot files must be real PNG files
with their declared SHA-256/byte size.  The manifest also contains the
renderer-observed visible roles/primitives/zones, cable visibility and PBR map
binding facts.  These facts must be recorded by the renderer capture producer,
not manually inferred from an image.

``--packaged-protocol-proof`` and ``--packaged-resume-proof`` bind that input
to the existing persisted V2/export/restart evidence.  The sanitized output
does not contain asset/project/thread IDs or absolute filesystem paths.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
from pathlib import Path, PurePosixPath
import struct
import sys
import tempfile
from typing import Any, Mapping
import zlib


SCHEMA = "ForgeCADC106V2VisualCapture@1"
REPORT_SCHEMA = "ForgeCADC106V2VisualEvidenceGate@1"
PACKAGED_SCHEMA = "ForgeCADArmMvpPackagedProtocolProof@1"
RESUME_SCHEMA = "ForgeCADArmMvpPackagedResumeProof@1"
ROOT_RECIPE = "recipe_c106_arm_service_display"
REQUIRED_VIEWS = ("front", "side", "three_quarter", "detail_pbr")
OVERVIEW_VIEWS = frozenset(REQUIRED_VIEWS[:-1])
REQUIRED_ROLES = frozenset(
    {
        "base_form",
        "turntable",
        "joint_housing",
        "link_armor",
        "cable_harness",
        "end_effector_form",
        "surface_trim",
    }
)
PBR_TEXTURE_ROLES = frozenset(
    {"base_color", "metallic_roughness", "normal", "occlusion", "emissive"}
)
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


class GateFailure(RuntimeError):
    pass


def fail(code: str) -> None:
    raise GateFailure(code)


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def is_hash(value: object) -> bool:
    return isinstance(value, str) and len(value) == 64 and all(char in "0123456789abcdef" for char in value)


def require_mapping(value: object, code: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        fail(code)
    return value


def require_string(value: object, code: str) -> str:
    if not isinstance(value, str) or not value:
        fail(code)
    return value


def require_hash(value: object, code: str) -> str:
    if not is_hash(value):
        fail(code)
    return str(value)


def require_int(value: object, minimum: int, code: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < minimum:
        fail(code)
    return value


def read_json(path: Path, code: str) -> tuple[Mapping[str, Any], bytes]:
    try:
        if path.is_symlink():
            fail(code)
        raw = path.read_bytes()
        value = json.loads(raw)
    except (OSError, json.JSONDecodeError):
        fail(code)
    return require_mapping(value, code), raw


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


def within(root: Path, relative: str, code: str) -> Path:
    candidate = root.joinpath(*PurePosixPath(relative).parts)
    try:
        candidate.relative_to(root)
    except ValueError:
        fail(code)
    if candidate.is_symlink() or not candidate.is_file():
        fail(code)
    return candidate


def png_dimensions(path: Path, code: str) -> tuple[int, int, bytes]:
    try:
        value = path.read_bytes()
    except OSError:
        fail(code)
    if len(value) < 24 or value[:8] != PNG_SIGNATURE or value[12:16] != b"IHDR":
        fail(code)
    width, height = struct.unpack(">II", value[16:24])
    if width < 1 or height < 1:
        fail(code)
    return width, height, value


def packaged_lineage(protocol_path: Path, resume_path: Path) -> dict[str, str]:
    protocol, protocol_raw = read_json(protocol_path, "C106_VISUAL_PROTOCOL_PROOF_INVALID")
    resume, resume_raw = read_json(resume_path, "C106_VISUAL_RESUME_PROOF_INVALID")
    if protocol.get("schema_version") != PACKAGED_SCHEMA or protocol.get("status") != "pass":
        fail("C106_VISUAL_PROTOCOL_PROOF_INVALID")
    if protocol.get("root_recipe_id") != ROOT_RECIPE:
        fail("C106_VISUAL_PROTOCOL_RECIPE_INVALID")
    a005 = require_mapping(protocol.get("a005"), "C106_VISUAL_PROTOCOL_A005_INVALID")
    export = require_mapping(protocol.get("export"), "C106_VISUAL_PROTOCOL_EXPORT_INVALID")
    asset_version_id = require_string(a005.get("v2_asset_version_id"), "C106_VISUAL_PROTOCOL_V2_INVALID")
    glb_sha256 = require_hash(export.get("glb_sha256"), "C106_VISUAL_PROTOCOL_GLB_INVALID")
    if export.get("asset_version_id") != asset_version_id or export.get("x_forgecad_glb_sha256") != glb_sha256:
        fail("C106_VISUAL_PROTOCOL_EXPORT_DRIFT")
    if resume.get("schema_version") != RESUME_SCHEMA or resume.get("status") != "pass":
        fail("C106_VISUAL_RESUME_PROOF_INVALID")
    resumed = require_mapping(resume.get("export"), "C106_VISUAL_RESUME_EXPORT_INVALID")
    if (
        resume.get("expected_asset_version_id") != asset_version_id
        or resumed.get("asset_version_id") != asset_version_id
        or resumed.get("glb_sha256") != glb_sha256
        or resumed.get("x_forgecad_glb_sha256") != glb_sha256
    ):
        fail("C106_VISUAL_RESUME_DRIFT")
    return {
        "asset_version_id": asset_version_id,
        "glb_sha256": glb_sha256,
        "protocol_proof_sha256": sha256_bytes(protocol_raw),
        "resume_proof_sha256": sha256_bytes(resume_raw),
    }


def list_of_strings(value: object, code: str) -> set[str]:
    if not isinstance(value, list) or any(not isinstance(item, str) or not item for item in value):
        fail(code)
    return set(value)


def validate_runtime(runtime: Mapping[str, Any], *, view_id: str) -> dict[str, Any]:
    if (
        runtime.get("renderer_id") != "ForgeCADWorkbenchRenderer@1"
        or runtime.get("canvas_count") != 1
        or runtime.get("active_webgl_contexts") != 1
        or runtime.get("load_state") != "ready"
        or runtime.get("render_source") != "glb_pbr"
        or runtime.get("pbr_color_spaces") != "valid"
        or runtime.get("pbr_sampling_valid") != "true"
    ):
        fail("C106_VISUAL_RENDERER_CONTRACT_INVALID")
    roles = list_of_strings(runtime.get("visible_part_roles"), "C106_VISUAL_VISIBLE_ROLES_INVALID")
    zones = list_of_strings(runtime.get("visible_material_zone_ids"), "C106_VISUAL_VISIBLE_ZONES_INVALID")
    visible_primitives = require_int(runtime.get("visible_primitive_count"), 1, "C106_VISUAL_VISIBLE_PRIMITIVES_INVALID")
    if view_id in OVERVIEW_VIEWS and (visible_primitives < 10 or len(zones) < 4):
        fail("C106_VISUAL_OVERVIEW_VISIBILITY_INVALID")
    if view_id == "detail_pbr":
        detail = require_mapping(runtime.get("close_pbr"), "C106_VISUAL_CLOSE_PBR_INVALID")
        texture_roles = list_of_strings(detail.get("texture_roles"), "C106_VISUAL_CLOSE_PBR_INVALID")
        if (
            texture_roles != PBR_TEXTURE_ROLES
            or detail.get("normal_map_bound") is not True
            or detail.get("metallic_roughness_map_bound") is not True
            or detail.get("occlusion_map_bound") is not True
            or detail.get("emissive_map_bound") is not True
            or require_int(detail.get("embedded_pbr_material_count"), 1, "C106_VISUAL_CLOSE_PBR_INVALID") < 1
        ):
            fail("C106_VISUAL_CLOSE_PBR_INVALID")
    return {"roles": roles, "zones": zones, "visible_primitives": visible_primitives}


def validate_capture(
    capture: Mapping[str, Any],
    *,
    root: Path,
    lineage: Mapping[str, str],
    renderer_generation: str,
) -> tuple[str, dict[str, Any]]:
    view_id = require_string(capture.get("view_id"), "C106_VISUAL_VIEW_ID_INVALID")
    if view_id not in REQUIRED_VIEWS:
        fail("C106_VISUAL_VIEW_ID_INVALID")
    if (
        capture.get("asset_version_id") != lineage["asset_version_id"]
        or capture.get("source_glb_sha256") != lineage["glb_sha256"]
        or capture.get("artifact_profile_id") != "production_concept"
        or capture.get("renderer_generation") != renderer_generation
    ):
        fail("C106_VISUAL_CAPTURE_LINEAGE_DRIFT")
    file = safe_relative_file(capture.get("screenshot"), ".png", "C106_VISUAL_SCREENSHOT_PATH_INVALID")
    png_path = within(root, file, "C106_VISUAL_SCREENSHOT_PATH_INVALID")
    width, height, png = png_dimensions(png_path, "C106_VISUAL_SCREENSHOT_INVALID")
    if width < 320 or height < 240:
        fail("C106_VISUAL_SCREENSHOT_DIMENSIONS_INVALID")
    if capture.get("screenshot_sha256") != sha256_bytes(png) or capture.get("screenshot_byte_size") != len(png):
        fail("C106_VISUAL_SCREENSHOT_HASH_DRIFT")
    runtime = validate_runtime(require_mapping(capture.get("runtime"), "C106_VISUAL_RENDERER_CONTRACT_INVALID"), view_id=view_id)
    return view_id, {
        "view_id": view_id,
        "screenshot": file,
        "screenshot_sha256": sha256_bytes(png),
        "screenshot_byte_size": len(png),
        "dimensions": [width, height],
        "visible_part_roles": sorted(runtime["roles"]),
        "visible_material_zone_ids": sorted(runtime["zones"]),
        "visible_primitive_count": runtime["visible_primitives"],
    }


def validate(
    evidence_root: Path,
    protocol_path: Path,
    resume_path: Path,
) -> dict[str, Any]:
    if not evidence_root.is_dir() or evidence_root.is_symlink():
        fail("C106_VISUAL_EVIDENCE_ROOT_INVALID")
    manifest, manifest_raw = read_json(evidence_root / "manifest.json", "C106_VISUAL_MANIFEST_INVALID")
    if (
        manifest.get("schema_version") != SCHEMA
        or manifest.get("evidence_kind") != "automated_visual_evidence"
        or manifest.get("evidence_origin") != "same_packaged_asset_workbench_capture"
        or manifest.get("status") != "captured"
        or manifest.get("formal_eligible") is not False
        or manifest.get("human_benchmark_evidence") is not False
        or manifest.get("m108b_status") != "blocked"
        or manifest.get("score_status") != "not_scored"
    ):
        fail("C106_VISUAL_MANIFEST_SCOPE_INVALID")
    lineage = packaged_lineage(protocol_path, resume_path)
    asset = require_mapping(manifest.get("asset"), "C106_VISUAL_ASSET_INVALID")
    if (
        asset.get("asset_version_id") != lineage["asset_version_id"]
        or asset.get("glb_sha256") != lineage["glb_sha256"]
        or asset.get("artifact_profile_id") != "production_concept"
        or asset.get("root_recipe_id") != ROOT_RECIPE
    ):
        fail("C106_VISUAL_ASSET_LINEAGE_DRIFT")
    binding = require_mapping(manifest.get("packaged_proof"), "C106_VISUAL_PROOF_BINDING_INVALID")
    if (
        binding.get("protocol_proof_sha256") != lineage["protocol_proof_sha256"]
        or binding.get("resume_proof_sha256") != lineage["resume_proof_sha256"]
    ):
        fail("C106_VISUAL_PROOF_BINDING_DRIFT")
    renderer = require_mapping(manifest.get("renderer"), "C106_VISUAL_RENDERER_CONTRACT_INVALID")
    renderer_generation = require_string(renderer.get("renderer_generation"), "C106_VISUAL_RENDERER_GENERATION_INVALID")
    # The top-level renderer fact prevents a manifest that only makes each
    # individual screenshot look one-canvas while silently creating a second
    # renderer between captures.
    if renderer.get("canvas_count") != 1 or renderer.get("active_webgl_contexts") != 1:
        fail("C106_VISUAL_RENDERER_CONTRACT_INVALID")
    captures = manifest.get("captures")
    if not isinstance(captures, list) or len(captures) != len(REQUIRED_VIEWS):
        fail("C106_VISUAL_CAPTURE_SET_INVALID")
    captured: dict[str, dict[str, Any]] = {}
    visible_roles: set[str] = set()
    visible_zones: set[str] = set()
    for raw_capture in captures:
        view_id, sanitized = validate_capture(
            require_mapping(raw_capture, "C106_VISUAL_CAPTURE_INVALID"),
            root=evidence_root,
            lineage=lineage,
            renderer_generation=renderer_generation,
        )
        if view_id in captured:
            fail("C106_VISUAL_CAPTURE_SET_INVALID")
        captured[view_id] = sanitized
        visible_roles.update(sanitized["visible_part_roles"])
        visible_zones.update(sanitized["visible_material_zone_ids"])
    if set(captured) != set(REQUIRED_VIEWS):
        fail("C106_VISUAL_CAPTURE_SET_INVALID")
    if not REQUIRED_ROLES <= visible_roles or "cable_harness" not in set(captured["side"]["visible_part_roles"]) | set(captured["three_quarter"]["visible_part_roles"]):
        fail("C106_VISUAL_REQUIRED_FORM_OR_CABLE_NOT_VISIBLE")
    if len(visible_zones) < 8:
        fail("C106_VISUAL_ZONE_COVERAGE_INVALID")
    readback = require_mapping(manifest.get("readback"), "C106_VISUAL_READBACK_INVALID")
    if (
        readback.get("source_glb_sha256") != lineage["glb_sha256"]
        or require_int(readback.get("primitive_count"), 21, "C106_VISUAL_READBACK_INVALID") < 21
        or require_int(readback.get("material_zone_count"), 10, "C106_VISUAL_READBACK_INVALID") < 10
        or require_int(readback.get("cable_harness_primitive_count"), 1, "C106_VISUAL_READBACK_INVALID") < 1
    ):
        fail("C106_VISUAL_READBACK_INVALID")
    return {
        "schema_version": REPORT_SCHEMA,
        "status": "pass",
        "automated_visual_evidence_only": True,
        "formal_eligible": False,
        "human_benchmark_evidence": False,
        "m108b_status": "blocked",
        "score_status": "not_scored",
        "asset": {
            "asset_version_fingerprint": sha256_bytes(lineage["asset_version_id"].encode("utf-8")),
            "source_glb_sha256": lineage["glb_sha256"],
            "artifact_profile_id": "production_concept",
            "root_recipe_id": ROOT_RECIPE,
        },
        "packaged_proof": {
            "protocol_proof_sha256": lineage["protocol_proof_sha256"],
            "resume_proof_sha256": lineage["resume_proof_sha256"],
            "manifest_sha256": sha256_bytes(manifest_raw),
        },
        "renderer": {
            "canvas_count": 1,
            "active_webgl_contexts": 1,
            "renderer_generation_fingerprint": sha256_bytes(renderer_generation.encode("utf-8")),
        },
        "captures": [captured[view_id] for view_id in REQUIRED_VIEWS],
        "coverage": {
            "visible_part_roles": sorted(visible_roles),
            "visible_material_zone_count": len(visible_zones),
            "readback_primitive_count": readback["primitive_count"],
            "readback_material_zone_count": readback["material_zone_count"],
            "readback_cable_harness_primitive_count": readback["cable_harness_primitive_count"],
        },
        "limits": [
            "This gate validates renderer-recorded capture/readback facts and screenshot file integrity; it does not perform semantic image scoring.",
            "This automated evidence is not an M108B formal kit, visual score, independent-human review, or production-release claim.",
            "The capture producer must record a same-packaged-asset workbench lineage; browser fixtures or manually assembled screenshots fail this contract.",
        ],
    }


def atomic_json(path: Path, value: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists() and path.is_symlink():
        fail("C106_VISUAL_OUTPUT_PATH_INVALID")
    temporary = path.with_name(f".{path.name}.tmp")
    try:
        temporary.write_text(json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        temporary.replace(path)
    except OSError:
        fail("C106_VISUAL_OUTPUT_PATH_INVALID")


def write_png(path: Path, width: int, height: int, rgba: tuple[int, int, int, int]) -> None:
    raw = b"".join(b"\x00" + bytes(rgba) * width for _ in range(height))
    chunks = []
    for kind, body in ((b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)), (b"IDAT", zlib.compress(raw)), (b"IEND", b"")):
        chunks.append(struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body) & 0xFFFFFFFF))
    path.write_bytes(PNG_SIGNATURE + b"".join(chunks))


def self_test_manifest(root: Path, protocol: Mapping[str, Any], resume: Mapping[str, Any]) -> None:
    protocol_raw = json.dumps(protocol, ensure_ascii=False).encode("utf-8")
    resume_raw = json.dumps(resume, ensure_ascii=False).encode("utf-8")
    (root / "protocol.json").write_bytes(protocol_raw)
    (root / "resume.json").write_bytes(resume_raw)
    evidence = root / "evidence"
    captures_dir = evidence / "captures"
    captures_dir.mkdir(parents=True)
    views = []
    role_sets = {
        "front": ["base_form", "turntable", "joint_housing", "link_armor", "end_effector_form", "surface_trim"],
        "side": ["base_form", "turntable", "joint_housing", "link_armor", "cable_harness", "end_effector_form"],
        "three_quarter": ["base_form", "turntable", "joint_housing", "link_armor", "cable_harness", "end_effector_form", "surface_trim"],
        "detail_pbr": ["link_armor", "surface_trim", "cable_harness"],
    }
    all_zones = [f"zone_arm_{name}" for name in ("base", "turntable", "joint_shell", "joint_core", "link_shell", "link_armor", "cable", "gripper", "surface_trim", "signal")]
    v2_id = protocol["a005"]["v2_asset_version_id"]
    glb = protocol["export"]["glb_sha256"]
    for index, view_id in enumerate(REQUIRED_VIEWS):
        relative = f"captures/{view_id}.png"
        image = captures_dir / f"{view_id}.png"
        write_png(image, 640, 480, (12 + index, 34 + index, 56 + index, 255))
        png = image.read_bytes()
        runtime: dict[str, Any] = {
            "renderer_id": "ForgeCADWorkbenchRenderer@1",
            "canvas_count": 1,
            "active_webgl_contexts": 1,
            "load_state": "ready",
            "render_source": "glb_pbr",
            "pbr_color_spaces": "valid",
            "pbr_sampling_valid": "true",
            "visible_part_roles": role_sets[view_id],
            "visible_material_zone_ids": all_zones if view_id == "three_quarter" else all_zones[:6],
            "visible_primitive_count": 12 if view_id != "detail_pbr" else 4,
        }
        if view_id == "detail_pbr":
            runtime["close_pbr"] = {
                "texture_roles": sorted(PBR_TEXTURE_ROLES),
                "normal_map_bound": True,
                "metallic_roughness_map_bound": True,
                "occlusion_map_bound": True,
                "emissive_map_bound": True,
                "embedded_pbr_material_count": 2,
            }
        views.append({
            "view_id": view_id,
            "asset_version_id": v2_id,
            "source_glb_sha256": glb,
            "artifact_profile_id": "production_concept",
            "renderer_generation": "same-packaged-renderer-generation-1",
            "screenshot": relative,
            "screenshot_sha256": sha256_bytes(png),
            "screenshot_byte_size": len(png),
            "runtime": runtime,
        })
    manifest = {
        "schema_version": SCHEMA,
        "status": "captured",
        "evidence_kind": "automated_visual_evidence",
        "evidence_origin": "same_packaged_asset_workbench_capture",
        "formal_eligible": False,
        "human_benchmark_evidence": False,
        "m108b_status": "blocked",
        "score_status": "not_scored",
        "asset": {"asset_version_id": v2_id, "glb_sha256": glb, "artifact_profile_id": "production_concept", "root_recipe_id": ROOT_RECIPE},
        "packaged_proof": {"protocol_proof_sha256": sha256_bytes(protocol_raw), "resume_proof_sha256": sha256_bytes(resume_raw)},
        "renderer": {"canvas_count": 1, "active_webgl_contexts": 1, "renderer_generation": "same-packaged-renderer-generation-1"},
        "readback": {"source_glb_sha256": glb, "primitive_count": 21, "material_zone_count": 10, "cable_harness_primitive_count": 3},
        "captures": views,
    }
    (evidence / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8")


def run_self_test() -> dict[str, Any]:
    v2 = "assetver_c106_v2_self_test_0123456789"
    glb = "a" * 64
    protocol = {
        "schema_version": PACKAGED_SCHEMA,
        "status": "pass",
        "root_recipe_id": ROOT_RECIPE,
        "a005": {"v2_asset_version_id": v2},
        "export": {"asset_version_id": v2, "glb_sha256": glb, "x_forgecad_glb_sha256": glb},
    }
    resume = {
        "schema_version": RESUME_SCHEMA,
        "status": "pass",
        "expected_asset_version_id": v2,
        "export": {"asset_version_id": v2, "glb_sha256": glb, "x_forgecad_glb_sha256": glb},
    }
    with tempfile.TemporaryDirectory(prefix="forgecad-c106-v2-visual-evidence-") as directory:
        root = Path(directory)
        self_test_manifest(root, protocol, resume)
        report = validate(root / "evidence", root / "protocol.json", root / "resume.json")
        if report.get("status") != "pass" or report.get("formal_eligible") is not False:
            fail("C106_VISUAL_SELF_TEST_BASELINE_INVALID")
        manifest_path = root / "evidence" / "manifest.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        broken = copy.deepcopy(manifest)
        broken["renderer"]["canvas_count"] = 2
        manifest_path.write_text(json.dumps(broken), encoding="utf-8")
        try:
            validate(root / "evidence", root / "protocol.json", root / "resume.json")
        except GateFailure as error:
            if str(error) != "C106_VISUAL_RENDERER_CONTRACT_INVALID":
                raise
        else:
            fail("C106_VISUAL_SELF_TEST_SECOND_CANVAS_ACCEPTED")
        broken = copy.deepcopy(manifest)
        broken["captures"][1]["source_glb_sha256"] = "b" * 64
        manifest_path.write_text(json.dumps(broken), encoding="utf-8")
        try:
            validate(root / "evidence", root / "protocol.json", root / "resume.json")
        except GateFailure as error:
            if str(error) != "C106_VISUAL_CAPTURE_LINEAGE_DRIFT":
                raise
        else:
            fail("C106_VISUAL_SELF_TEST_LINEAGE_DRIFT_ACCEPTED")
        broken = copy.deepcopy(manifest)
        broken["captures"][2]["runtime"]["visible_part_roles"].remove("cable_harness")
        broken["captures"][1]["runtime"]["visible_part_roles"].remove("cable_harness")
        manifest_path.write_text(json.dumps(broken), encoding="utf-8")
        try:
            validate(root / "evidence", root / "protocol.json", root / "resume.json")
        except GateFailure as error:
            if str(error) != "C106_VISUAL_REQUIRED_FORM_OR_CABLE_NOT_VISIBLE":
                raise
        else:
            fail("C106_VISUAL_SELF_TEST_CABLE_ACCEPTED")
    return {
        "schema_version": "ForgeCADC106V2VisualEvidenceGateSelfTest@1",
        "status": "pass",
        "formal_eligible": False,
        "negative_probes": ["second_canvas", "asset_hash_drift", "missing_cable_visibility"],
    }


def main(args: argparse.Namespace) -> int:
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, sort_keys=True))
        return 0
    if not all((args.evidence, args.packaged_protocol_proof, args.packaged_resume_proof, args.output)):
        fail("C106_VISUAL_ARGUMENTS_REQUIRED")
    report = validate(Path(args.evidence), Path(args.packaged_protocol_proof), Path(args.packaged_resume_proof))
    output = Path(args.output)
    atomic_json(output, report)
    print(json.dumps(report, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Validate same-lineage C106 v2 workbench visual evidence.")
    parser.add_argument("--evidence", help="directory containing the capture manifest and relative PNG screenshots")
    parser.add_argument("--packaged-protocol-proof", help="ForgeCADArmMvpPackagedProtocolProof@1 JSON")
    parser.add_argument("--packaged-resume-proof", help="ForgeCADArmMvpPackagedResumeProof@1 JSON")
    parser.add_argument("--output", help="sanitized output JSON path")
    parser.add_argument("--self-test", action="store_true", help="exercise valid and fail-closed synthetic contracts")
    try:
        raise SystemExit(main(parser.parse_args()))
    except GateFailure as error:
        print(json.dumps({"schema_version": REPORT_SCHEMA, "status": "fail", "error": str(error), "formal_eligible": False}, ensure_ascii=False, sort_keys=True), file=sys.stderr)
        raise SystemExit(1) from None
