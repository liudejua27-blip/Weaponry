#!/usr/bin/env python3
"""Closed result-contract gate for ``reference_compare_prepare``.

This is a structural transport gate only.  It proves that the Runtime result
has one closed envelope and that its nested CameraCalibration, RenderSet@2,
ReferenceComparisonReport@1 and QualityReport@2 objects remain schema-bound.
It does not run a renderer and it never promotes a visual or commercial
quality status.
"""

from __future__ import annotations

import copy
import hashlib
import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
CONTRACT_ROOT = ROOT / "packages" / "forgecad-contracts"
SCHEMA_ROOT = CONTRACT_ROOT / "schemas"
MANIFEST_PATH = CONTRACT_ROOT / "manifest.json"
SCHEMA_NAME = "reference-comparison-prepare-result.schema.json"
NEGATIVE_FIXTURE_PATH = (
    CONTRACT_ROOT
    / "fixtures"
    / "reference-comparison-prepare-result"
    / "negative"
    / "cases.json"
)
SCHEMA_TITLE = "ReferenceComparisonPrepareResult@1"

RESULT_FIELDS = (
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
)
PASS_IDS = (
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
HASH = "a" * 64
CAMERA_HASH = "b" * 64
RENDER_SET_HASH = "c" * 64
COMPARISON_HASH = "d" * 64
REFERENCE_HASH = "e" * 64
ARTIFACT_HASH = "f" * 64
PROGRAM_HASH = "0" * 64

sys.path.insert(0, str(ROOT / "scripts"))
from check_agentic_contracts import is_valid, load_schema_registry  # noqa: E402


def fail(message: str) -> None:
    raise SystemExit(f"Reference comparison result contract violation: {message}")


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        fail(f"cannot load {path.relative_to(ROOT)}: {exc}")


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")


def sha256(value: Any) -> str:
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def canonical_hash(value: dict[str, Any]) -> str:
    payload = copy.deepcopy(value)
    payload["canonical_sha256"] = ""
    return sha256(payload)


def schema_objects(node: Any) -> list[dict[str, Any]]:
    if not isinstance(node, dict):
        return []
    found = [node] if node.get("type") == "object" else []
    for key, child in node.items():
        if key in {"properties", "$defs"} and isinstance(child, dict):
            for value in child.values():
                found.extend(schema_objects(value))
        elif key in {"items", "prefixItems", "allOf", "anyOf", "oneOf", "not", "if", "then", "else"}:
            if isinstance(child, list):
                for value in child:
                    found.extend(schema_objects(value))
            else:
                found.extend(schema_objects(child))
    return found


def property_names(node: Any) -> list[str]:
    if not isinstance(node, dict):
        return []
    names: list[str] = []
    properties = node.get("properties")
    if isinstance(properties, dict):
        names.extend(properties)
        for child in properties.values():
            names.extend(property_names(child))
    for key in ("$defs", "items", "prefixItems", "allOf", "anyOf", "oneOf", "not", "if", "then", "else"):
        child = node.get(key)
        if isinstance(child, list):
            for value in child:
                names.extend(property_names(value))
        elif isinstance(child, dict):
            names.extend(property_names(child))
    return names


def check_schema(schema: dict[str, Any], manifest: dict[str, Any], registry: dict[str, dict[str, Any]]) -> None:
    require(SCHEMA_NAME in manifest.get("schemas", []), "result schema is not registered in manifest")
    require(schema.get("$schema") == "https://json-schema.org/draft/2020-12/schema", "schema draft drifted")
    require(schema.get("$id") == f"https://forgecad.local/contracts/{SCHEMA_NAME}", "schema id drifted")
    require(schema.get("title") == SCHEMA_TITLE, "schema title/version drifted")
    require(schema.get("type") == "object" and schema.get("additionalProperties") is False, "result root is open")
    require(tuple(schema.get("required", [])) == RESULT_FIELDS, "required field order/count drifted")
    require(tuple(schema.get("properties", {})) == RESULT_FIELDS, "property field order/count drifted")
    require(registry.get(schema["$id"]) == schema, "result schema is not registry-bound")

    properties = schema["properties"]
    require(
        properties["camera"].get("oneOf") == [
            {"$ref": "https://forgecad.local/contracts/camera-calibration.schema.json"},
            {"$ref": "https://forgecad.local/contracts/camera-calibration-v2.schema.json"},
        ],
        "camera must accept only CameraCalibration@1 or @2",
    )
    expected_refs = {
        "render_set": "https://forgecad.local/contracts/render-set-v2.schema.json",
        "comparison_report": "https://forgecad.local/contracts/reference-comparison-report.schema.json",
        "quality_report": "https://forgecad.local/contracts/quality-report-v2.schema.json",
    }
    for field, reference in expected_refs.items():
        require(properties[field].get("$ref") == reference, f"{field} reference drifted")
    for field in (
        "camera_object_sha256",
        "render_set_hash",
        "render_set_object_sha256",
        "comparison_report_hash",
        "comparison_report_object_sha256",
        "quality_report_object_sha256",
    ):
        require(properties[field].get("$ref") == "#/$defs/sha256", f"{field} is not SHA-256 bound")
    require(
        schema["$defs"]["identifier"]["pattern"] == "^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$",
        "identifier admits a Runtime-invalid value",
    )
    require(schema["$defs"]["sha256"]["pattern"] == "^[0-9a-f]{64}$", "SHA-256 pattern drifted")
    forbidden = {
        "path",
        "url",
        "uri",
        "raw",
        "bytes",
        "secret",
        "token",
        "password",
        "api_key",
        "prompt",
        "script",
        "shell",
        "environment",
    }
    require(not ({name.lower() for name in property_names(schema)} & forbidden), "result exposes a forbidden transport field")
    for object_schema in schema_objects(schema):
        require(object_schema.get("additionalProperties") is False, "nested result object is open")


def camera() -> dict[str, Any]:
    value: dict[str, Any] = {
        "schema_version": "CameraCalibration@1",
        "camera_hash": CAMERA_HASH,
        "projection": "perspective",
        "transform": {
            "position_m": [0.0, 0.0, 1.0],
            "target_m": [0.0, 0.0, 0.0],
            "up": [0.0, 1.0, 0.0],
        },
        "fov_y_degrees": 45.0,
        "near_m": 0.01,
        "far_m": 100.0,
        "resolution": {"width": 512, "height": 512},
        "coordinate_system": "right-handed-y-up-meter",
        "renderer_revision": "forgecad-renderer-2",
        "canonical_sha256": "",
    }
    value["canonical_sha256"] = canonical_hash(value)
    return value


def render_profile() -> dict[str, Any]:
    encodings = (
        ("beauty", "color", "srgb-u8", "fixed-linear-to-srgb@1", "triangle", "unit", False),
        ("silhouette", "mask", "binary-mask-palette-u8", "none", "nearest", "unit", True),
        ("depth", "depth", "reversed-normalized-depth-u8", "none", "nearest", "unit", False),
        ("normal", "normal-vector", "signed-unit-vector-to-unorm8", "none", "triangle", "unit", False),
        ("ao", "scalar", "normalized-scalar-u8", "none", "triangle", "unit", False),
        ("part-id", "id", "index-palette-u8", "none", "nearest", "categorical-mesh-index-0-255", True),
        ("material-id", "id", "index-palette-u8", "none", "nearest", "categorical-material-index-0-255", True),
        ("wireframe", "diagnostic", "edge-diagnostic-palette-u8", "none", "triangle", "unit", False),
        ("uv-stretch", "diagnostic", "uv-stretch-heatmap-u8", "none", "triangle", "unit", False),
    )
    aovs = [
        {
            "pass_id": pass_id,
            "semantic_kind": semantic_kind,
            "storage": "image/png;rgba8",
            "encoding": encoding,
            "source_value_range": source_range,
            "color_transform": color_transform,
            "filter": filter_kind,
            "alpha_semantics": "opaque-1",
            "background_encoding": "rgba8:8,12,18,255",
            "units": "unit",
            "palette_definition_sha256": None,
            "metric_safe": metric_safe,
            "source_definition": "fixed-aov@1",
        }
        for pass_id, semantic_kind, encoding, color_transform, filter_kind, source_range, metric_safe in encodings
    ]
    value: dict[str, Any] = {
        "schema_version": "RenderProfile@1",
        "profile_id": "forgecad-fixed-software-render-profile",
        "engine_id": "forgecad-fixed-software@2",
        "backend_id": "cpu-raster@1",
        "renderer_revision": "forgecad-renderer-2",
        "resolution": {"width": 512, "height": 512},
        "sampling": {
            "mode": "deterministic-raster",
            "supersample_axis": 2,
            "seed_policy": "not-applicable-no-rng",
            "adaptive": False,
            "temporal": False,
            "motion_blur": False,
        },
        "color_pipeline": {
            "scene_color_space": "linear-rec709-d65",
            "display_device": "srgb",
            "view_transform": "fixed-linear-to-srgb@1",
            "look": "none",
            "exposure_stops": 0,
            "gamma": 1,
            "ocio_config_sha256": None,
        },
        "alpha": {"background": "opaque-fixed", "alpha_mode": "opaque-1", "transparent_film": False},
        "aovs": aovs,
        "aov_definition_sha256": HASH,
        "color_pipeline_sha256": HASH,
        "id_palette_definition_sha256": HASH,
        "canonical_sha256": "",
    }
    value["canonical_sha256"] = canonical_hash(value)
    return value


def render_set(camera_value: dict[str, Any], profile: dict[str, Any]) -> dict[str, Any]:
    passes = list(PASS_IDS)
    pass_artifacts = {
        pass_id: {
            "sha256": sha256({"pass": pass_id}),
            "mime": "image/png",
            "size_bytes": 1024,
            "width": 512,
            "height": 512,
            "channels": "rgba8",
            "color_space": "srgb" if pass_id == "beauty" else "data",
        }
        for pass_id in passes
    }
    value: dict[str, Any] = {
        "schema_version": "RenderSet@2",
        "render_set_id": "render-set-1",
        "view_id": "front",
        "candidate_id": "candidate-1",
        "artifact_sha256": ARTIFACT_HASH,
        "program_sha256": PROGRAM_HASH,
        "reference_id": "reference-1",
        "camera_hash": camera_value["camera_hash"],
        "camera_object_sha256": HASH,
        "renderer_hash": HASH,
        "render_profile": profile,
        "render_profile_sha256": profile["canonical_sha256"],
        "aov_definition_sha256": profile["aov_definition_sha256"],
        "color_pipeline_sha256": profile["color_pipeline_sha256"],
        "id_palette_definition_sha256": profile["id_palette_definition_sha256"],
        "render_worker_build_cohort_sha256": None,
        "render_worker_binding_status": "cohort_unavailable",
        "width": 512,
        "height": 512,
        "passes": passes,
        "pass_artifacts": pass_artifacts,
        "canonical_sha256": "",
    }
    value["canonical_sha256"] = canonical_hash(value)
    return value


def comparison_report() -> dict[str, Any]:
    value: dict[str, Any] = {
        "schema_version": "ReferenceComparisonReport@1",
        "report_id": "comparison-1",
        "candidate_id": "candidate-1",
        "artifact_sha256": ARTIFACT_HASH,
        "reference_id": "reference-1",
        "reference_sha256": REFERENCE_HASH,
        "render_set_hash": RENDER_SET_HASH,
        "view_id": "front",
        "camera_hash": CAMERA_HASH,
        "benchmark_eligibility": "BLOCKED_USER_CONFIRMATION_REQUIRED",
        "mask": {
            "method": "silhouette-target",
            "revision": "mask-v1",
            "sha256": HASH,
            "width": 512,
            "height": 512,
        },
        "metrics": {
            "silhouette_iou": 0.0,
            "boundary_f1_4px": 0.0,
            "bbox_edge_error": 0.0,
            "centroid_error": 0.0,
            "landmark_coverage": 0.0,
            "landmark_nme": 0.0,
            "region_median_iou": 0.0,
            "critical_region_min_iou": 0.0,
        },
        "status": "QUALITY_TARGET_NOT_MET",
        "canonical_sha256": "",
    }
    value["canonical_sha256"] = canonical_hash(value)
    return value


def quality_report() -> dict[str, Any]:
    value: dict[str, Any] = {
        "schema_version": "QualityReport@2",
        "quality_report_id": "quality-1",
        "candidate_id": "candidate-1",
        "artifact_sha256": ARTIFACT_HASH,
        "program_sha256": PROGRAM_HASH,
        "reference_id": "reference-1",
        "reference_sha256": REFERENCE_HASH,
        "view_id": "front",
        "render_set_hash": RENDER_SET_HASH,
        "comparison_report_hash": COMPARISON_HASH,
        "benchmark_eligibility": "BLOCKED_USER_CONFIRMATION_REQUIRED",
        "human_receipt_hash": None,
        "structural_status": "passed",
        "visual_status": "QUALITY_TARGET_NOT_MET",
        "hard_gate_passed": False,
        "threshold_revision": "visible-view-gates@1",
        "threshold_policy_sha256": HASH,
        "threshold_source": "forgecad-runtime-visible-view-gates",
        "metric_gate_results": [],
        "limitations": ["structural-only"],
        "canonical_sha256": "",
    }
    value["canonical_sha256"] = canonical_hash(value)
    return value


def result_fixture() -> dict[str, Any]:
    camera_value = camera()
    profile = render_profile()
    render_value = render_set(camera_value, profile)
    comparison_value = comparison_report()
    quality_value = quality_report()
    return {
        "schema_version": SCHEMA_TITLE,
        "candidate_id": "candidate-1",
        "reference_id": "reference-1",
        "camera": camera_value,
        "camera_object_sha256": HASH,
        "render_set": render_value,
        "render_set_hash": RENDER_SET_HASH,
        "render_set_object_sha256": RENDER_SET_HASH,
        "comparison_report": comparison_value,
        "comparison_report_hash": COMPARISON_HASH,
        "comparison_report_object_sha256": COMPARISON_HASH,
        "quality_report": quality_value,
        "quality_report_object_sha256": HASH,
    }


def check_bindings(result: dict[str, Any]) -> None:
    render_value = result["render_set"]
    comparison_value = result["comparison_report"]
    quality_value = result["quality_report"]
    require(render_value["candidate_id"] == result["candidate_id"], "RenderSet candidate binding drifted")
    require(render_value["reference_id"] == result["reference_id"], "RenderSet reference binding drifted")
    require(render_value["camera_object_sha256"] == result["camera_object_sha256"], "camera object binding drifted")
    require(comparison_value["candidate_id"] == result["candidate_id"], "comparison candidate binding drifted")
    require(comparison_value["reference_id"] == result["reference_id"], "comparison reference binding drifted")
    require(comparison_value["render_set_hash"] == result["render_set_hash"], "comparison RenderSet hash drifted")
    require(quality_value["candidate_id"] == result["candidate_id"], "quality candidate binding drifted")
    require(quality_value["reference_id"] == result["reference_id"], "quality reference binding drifted")
    require(quality_value["render_set_hash"] == result["render_set_hash"], "quality RenderSet hash drifted")
    require(quality_value["comparison_report_hash"] == result["comparison_report_hash"], "quality comparison hash drifted")
    require(comparison_value["status"] != "PARTIAL_VISIBLE_VIEW_PASS", "checker must not create a visual PASS")


def negative_cases() -> dict[str, dict[str, Any]]:
    cases: dict[str, dict[str, Any]] = {}
    value = result_fixture()
    extra = copy.deepcopy(value)
    extra["unexpected"] = True
    cases["extra-root-field"] = extra

    missing = copy.deepcopy(value)
    del missing["render_set"]
    cases["missing-render-set"] = missing

    camera_drift = copy.deepcopy(value)
    camera_drift["camera"]["schema_version"] = "CameraCalibration@2"
    cases["camera-schema-drift"] = camera_drift

    view_drift = copy.deepcopy(value)
    del view_drift["render_set"]["view_id"]
    cases["render-set-missing-view-id"] = view_drift

    status_drift = copy.deepcopy(value)
    status_drift["comparison_report"]["status"] = "VISUAL_PASS"
    cases["comparison-status-drift"] = status_drift

    nested_extra = copy.deepcopy(value)
    nested_extra["quality_report"]["unexpected"] = True
    cases["quality-report-extra-field"] = nested_extra

    hash_drift = copy.deepcopy(value)
    hash_drift["camera_object_sha256"] = "not-a-sha256"
    cases["hash-format-drift"] = hash_drift
    return cases


def check_negative_cases(schema: dict[str, Any], registry: dict[str, dict[str, Any]]) -> None:
    metadata = load_json(NEGATIVE_FIXTURE_PATH)
    require(isinstance(metadata, dict), "negative fixture manifest must be an object")
    require(
        metadata.get("schema_version") == "ReferenceComparisonResultNegativeFixtures@1",
        "negative fixture manifest version drifted",
    )
    entries = metadata.get("cases")
    require(isinstance(entries, list) and entries, "negative fixture manifest is empty")
    expected = tuple(negative_cases())
    actual = tuple(
        entry.get("id") for entry in entries if isinstance(entry, dict)
    )
    require(actual == expected, "negative fixture IDs do not cover the checker mutations exactly")
    mutations = negative_cases()
    for entry in entries:
        case_id = entry["id"]
        require(
            not is_valid(schema, mutations[case_id], registry),
            f"negative fixture unexpectedly passed: {case_id}",
        )


def run_checks() -> None:
    manifest = load_json(MANIFEST_PATH)
    require(isinstance(manifest, dict), "manifest must be an object")
    registry = load_schema_registry(manifest)
    schema = load_json(SCHEMA_ROOT / SCHEMA_NAME)
    require(isinstance(schema, dict), "result schema must be an object")
    check_schema(schema, manifest, registry)

    positive = result_fixture()
    require(is_valid(schema, positive, registry), "positive result fixture is schema-invalid")
    check_bindings(positive)
    check_negative_cases(schema, registry)

    print(
        "Reference comparison prepare result contract OK: "
        "closed 13-field envelope, CameraCalibration/RenderSet@2/ReferenceComparisonReport@1/QualityReport@2 bindings, negatives"
    )


if __name__ == "__main__":
    run_checks()
