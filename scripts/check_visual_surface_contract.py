#!/usr/bin/env python3
"""Check the bounded Visual Surface request/result contract and negative gates.

This checker validates the typed evidence boundary and the bounded mesh-derived
surface signal projection.  It does not claim that SubD/NURBS principal
curvature or visual-quality promotion exists.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_ROOT = ROOT / "packages/forgecad-contracts/schemas"
RUNTIME_SOURCE = ROOT / "apps/desktop/src-tauri/crates/forgecad-runtime/src/agentic_design.rs"
OPTIMIZATION_SOURCE = ROOT / "apps/desktop/src-tauri/crates/forgecad-runtime/src/optimization.rs"
MCP_SOURCE = ROOT / "apps/desktop/src-tauri/crates/forgecad-mcp/src/agentic_tools.rs"

SIGNALS = {
    "silhouette",
    "boundary",
    "depth",
    "normal",
    "part-id",
    "material-id",
    "curvature",
    "feature-line",
}
AOV_PASSES = (
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
BINDING_KEYS = {
    "reference_id",
    "reference_sha256",
    "artifact_sha256",
    "render_set_hash",
    "camera_hash",
    "comparison_report_hash",
    "quality_report_hash",
}


def fail(message: str) -> None:
    raise SystemExit(f"Visual Surface contract violation: {message}")


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def canonical_hash(value: dict[str, Any]) -> str:
    payload = dict(value)
    payload["canonical_sha256"] = ""
    encoded = json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(encoded).hexdigest()


def is_sha(value: Any) -> bool:
    return isinstance(value, str) and len(value) == 64 and all(char in "0123456789abcdef" for char in value)


def is_id(value: Any) -> bool:
    return isinstance(value, str) and 1 <= len(value) <= 128 and all(
        char.isalnum() or char in "_.-" for char in value
    )


def validate_nullable_sha(value: Any, label: str) -> None:
    require(value is None or is_sha(value), f"{label} is invalid")


def validate_bbox(value: Any, label: str) -> None:
    if value is None:
        return
    require(
        isinstance(value, list)
        and len(value) == 4
        and all(isinstance(item, int) and 0 <= item <= 511 for item in value),
        f"{label} is invalid",
    )


def validate_readback(value: dict[str, Any]) -> None:
    readback_keys = {
        "schema_version",
        "status",
        "resolution",
        "reference_mask",
        "candidate_mask",
        "edge",
        "roi",
        "aov",
        "surface",
        "canonical_sha256",
    }
    closed_object(value, readback_keys, "VisualSurfaceReadback@1")
    require(value["schema_version"] == "VisualSurfaceReadback@1", "readback version drifted")
    require(value["status"] in {"ready", "blocked", "not-run"}, "readback status invalid")
    require(value["resolution"] == [512, 512], "readback resolution drifted")

    for label in ("reference_mask", "candidate_mask"):
        mask = value[label]
        closed_object(
            mask,
            {"sha256", "decoded", "foreground_pixels", "edge_pixels", "bbox"},
            f"readback {label}",
        )
        validate_nullable_sha(mask["sha256"], f"readback {label}.sha256")
        require(isinstance(mask["decoded"], bool), f"readback {label}.decoded is invalid")
        for key in ("foreground_pixels", "edge_pixels"):
            count = mask[key]
            require(
                count is None or isinstance(count, int) and 0 <= count <= 262144,
                f"readback {label}.{key} is invalid",
            )
        validate_bbox(mask["bbox"], f"readback {label}.bbox")

    edge = value["edge"]
    closed_object(
        edge,
        {
            "status",
            "radius_px",
            "reference_edge_pixels",
            "candidate_edge_pixels",
            "matched_reference_edge_pixels",
            "matched_candidate_edge_pixels",
            "f1",
            "sdf_chamfer_px",
        },
        "readback edge",
    )
    require(edge["status"] in {"ready", "not-run"}, "readback edge status invalid")
    require(edge["radius_px"] == 4, "readback edge radius drifted")
    for key in (
        "reference_edge_pixels",
        "candidate_edge_pixels",
        "matched_reference_edge_pixels",
        "matched_candidate_edge_pixels",
    ):
        require(
            edge[key] is None or isinstance(edge[key], int) and 0 <= edge[key] <= 262144,
            f"readback edge {key} is invalid",
        )
    require(edge["f1"] is None or isinstance(edge["f1"], (int, float)) and 0 <= edge["f1"] <= 1, "readback edge f1 is invalid")
    require(edge["sdf_chamfer_px"] is None or isinstance(edge["sdf_chamfer_px"], (int, float)) and 0 <= edge["sdf_chamfer_px"] <= 512, "readback edge chamfer is invalid")

    roi = value["roi"]
    closed_object(
        roi,
        {"status", "source", "part_id_sha256", "material_id_sha256", "parts", "regions", "unknowns"},
        "readback roi",
    )
    require(roi["status"] in {"ready", "partial", "not-run"}, "readback roi status invalid")
    require(roi["source"] in {"part-id+reference-regions", "part-id", "not-run"}, "readback roi source invalid")
    validate_nullable_sha(roi["part_id_sha256"], "readback roi.part_id_sha256")
    validate_nullable_sha(roi["material_id_sha256"], "readback roi.material_id_sha256")
    require(isinstance(roi["parts"], list) and len(roi["parts"]) <= 512, "readback roi parts invalid")
    seen_parts: set[str] = set()
    for part in roi["parts"]:
        closed_object(part, {"part_id", "pixel_count", "normalized_area", "bbox"}, "readback roi part")
        require(is_id(part["part_id"]) and part["part_id"] not in seen_parts, "readback roi part ID invalid or duplicated")
        seen_parts.add(part["part_id"])
        require(isinstance(part["pixel_count"], int) and 0 <= part["pixel_count"] <= 262144, "readback roi pixel count invalid")
        require(isinstance(part["normalized_area"], (int, float)) and 0 <= part["normalized_area"] <= 1, "readback roi area invalid")
        validate_bbox(part["bbox"], "readback roi part bbox")
    require(isinstance(roi["regions"], list) and len(roi["regions"]) <= 64, "readback roi regions invalid")
    for region in roi["regions"]:
        closed_object(region, {"region_id", "visibility", "bbox", "reference_pixels", "candidate_pixels", "iou"}, "readback roi region")
        require(is_id(region["region_id"]), "readback roi region ID invalid")
        require(region["visibility"] in {"observed", "inferred", "unknown"}, "readback roi visibility invalid")
        require(isinstance(region["bbox"], list) and len(region["bbox"]) == 4 and all(isinstance(item, (int, float)) and 0 <= item <= 1 for item in region["bbox"]), "readback roi region bbox invalid")
        for key in ("reference_pixels", "candidate_pixels"):
            require(isinstance(region[key], int) and 0 <= region[key] <= 262144, f"readback roi {key} invalid")
        require(region["iou"] is None or isinstance(region["iou"], (int, float)) and 0 <= region["iou"] <= 1, "readback roi iou invalid")
    require(isinstance(roi["unknowns"], list) and len(roi["unknowns"]) <= 16 and all(isinstance(item, str) and 1 <= len(item) <= 128 and item.replace("_", "").replace("-", "").replace(".", "").isalnum() for item in roi["unknowns"]), "readback roi unknowns invalid")

    aov = value["aov"]
    closed_object(aov, {"status", "source", "passes", "missing_passes"}, "readback aov")
    require(aov["status"] in {"ready", "partial", "not-run"}, "readback aov status invalid")
    require(aov["source"] == "RenderSet@2/pass_artifacts", "readback aov source drifted")
    require(isinstance(aov["passes"], list) and len(aov["passes"]) <= 9, "readback aov passes invalid")
    seen_passes: set[str] = set()
    for row in aov["passes"]:
        closed_object(row, {"pass", "sha256", "status", "pixel_count", "nonzero_pixel_count", "mean_rgba"}, "readback aov pass")
        require(row["pass"] in AOV_PASSES and row["pass"] not in seen_passes, "readback aov pass invalid or duplicated")
        seen_passes.add(row["pass"])
        require(is_sha(row["sha256"]) and row["status"] == "decoded", "readback aov pass identity invalid")
        require(row["pixel_count"] == 262144 and isinstance(row["nonzero_pixel_count"], int) and 0 <= row["nonzero_pixel_count"] <= 262144, "readback aov pixel count invalid")
        require(isinstance(row["mean_rgba"], list) and len(row["mean_rgba"]) == 4 and all(isinstance(item, int) and 0 <= item <= 255 for item in row["mean_rgba"]), "readback aov mean invalid")
    require(isinstance(aov["missing_passes"], list) and len(aov["missing_passes"]) <= 9 and all(item in AOV_PASSES for item in aov["missing_passes"]), "readback aov missing list invalid")
    surface = value["surface"]
    closed_object(
        surface,
        {"schema_version", "status", "artifact_sha256", "triangle_count", "vertex_count", "edge_count", "non_manifold_edge_count", "curvature", "feature_line", "canonical_sha256"},
        "readback surface",
    )
    require(surface["schema_version"] == "SurfaceSignalReadback@1", "surface readback version drifted")
    require(surface["status"] in {"ready", "blocked", "not-run"}, "surface readback status invalid")
    validate_nullable_sha(surface["artifact_sha256"], "readback surface.artifact_sha256")
    for key in ("triangle_count", "vertex_count", "edge_count", "non_manifold_edge_count"):
        require(surface[key] is None or isinstance(surface[key], int) and surface[key] >= 0, f"readback surface {key} invalid")
    curvature = surface["curvature"]
    closed_object(curvature, {"status", "method", "mean_abs_dihedral_rad", "max_abs_dihedral_rad", "curved_triangle_count"}, "readback surface curvature")
    require(curvature["status"] in {"ready", "not-run"} and curvature["method"] in {"triangle-dihedral@1", "not-run"}, "surface curvature status invalid")
    for key in ("mean_abs_dihedral_rad", "max_abs_dihedral_rad"):
        require(curvature[key] is None or isinstance(curvature[key], (int, float)) and 0 <= curvature[key] <= 3.141592653589793, f"surface curvature {key} invalid")
    require(curvature["curved_triangle_count"] is None or isinstance(curvature["curved_triangle_count"], int) and curvature["curved_triangle_count"] >= 0, "surface curved triangle count invalid")
    feature_line = surface["feature_line"]
    closed_object(feature_line, {"status", "method", "threshold_rad", "edge_count", "boundary_edge_count", "crease_edge_count"}, "readback feature line")
    require(feature_line["status"] in {"ready", "not-run"} and feature_line["method"] in {"boundary-and-crease-edge@1", "not-run"}, "surface feature line status invalid")
    require(feature_line["threshold_rad"] is None or isinstance(feature_line["threshold_rad"], (int, float)) and 0 <= feature_line["threshold_rad"] <= 3.141592653589793, "surface feature line threshold invalid")
    for key in ("edge_count", "boundary_edge_count", "crease_edge_count"):
        require(feature_line[key] is None or isinstance(feature_line[key], int) and feature_line[key] >= 0, f"surface feature line {key} invalid")
    require(is_sha(value["canonical_sha256"]) and canonical_hash(value) == value["canonical_sha256"], "readback canonical hash drifted")


def closed_object(value: Any, keys: set[str], label: str) -> None:
    require(isinstance(value, dict), f"{label} must be an object")
    require(set(value) == keys, f"{label} field set is not closed")


def validate_request(value: dict[str, Any]) -> None:
    request_keys = {
        "schema_version",
        "project_id",
        "candidate_id",
        "requested_signals",
        "expected_binding",
        "target_sha256",
        "max_part_errors",
        "canonical_sha256",
    }
    closed_object(value, request_keys, "VisualSurfaceRequest@1")
    require(value["schema_version"] == "VisualSurfaceRequest@1", "request version drifted")
    require(is_id(value["project_id"]) and is_id(value["candidate_id"]), "request IDs invalid")
    require(
        isinstance(value["requested_signals"], list)
        and 1 <= len(value["requested_signals"]) <= 8
        and len(set(value["requested_signals"])) == len(value["requested_signals"])
        and set(value["requested_signals"]) <= SIGNALS,
        "request signal set is invalid",
    )
    closed_object(value["expected_binding"], BINDING_KEYS, "request expected_binding")
    for key, child in value["expected_binding"].items():
        if child is None:
            continue
        require(is_id(child) if key == "reference_id" else is_sha(child), f"request binding {key} invalid")
    require(value["target_sha256"] is None or is_sha(value["target_sha256"]), "request target hash invalid")
    require(isinstance(value["max_part_errors"], int) and 1 <= value["max_part_errors"] <= 64, "request part bound invalid")
    require(is_sha(value["canonical_sha256"]), "request canonical hash invalid")
    require(canonical_hash(value) == value["canonical_sha256"], "request canonical hash drifted")


def validate_result(value: dict[str, Any]) -> None:
    result_keys = {
        "schema_version",
        "projection_status",
        "read_only",
        "project_id",
        "candidate_id",
        "target_sha256",
        "status",
        "backend",
        "surface_program_status",
        "requested_signals",
        "available_signals",
        "unsupported_signals",
        "binding",
        "metrics",
        "part_errors",
        "readback",
        "unknowns",
        "lineage",
        "canonical_sha256",
    }
    closed_object(value, result_keys, "VisualSurfaceResult@1")
    require(value["schema_version"] == "VisualSurfaceResult@1", "result version drifted")
    require(value["projection_status"] == "projection/read-only" and value["read_only"] is True, "result is not read-only")
    require(value["backend"] in {"candidate-bound-aov-diagnostics@1", "candidate-bound-surface-analysis@1"}, "result backend claim drifted")
    require(value["surface_program_status"] in {"ready", "not-run", "unavailable"}, "surface program status invalid")
    require(value["status"] in {"ready", "blocked", "not-run"}, "result status invalid")
    require(value["target_sha256"] is None or is_sha(value["target_sha256"]), "result target hash invalid")
    requested = set(value["requested_signals"])
    available = set(value["available_signals"])
    unsupported = set(value["unsupported_signals"])
    require(requested <= SIGNALS and available <= requested and unsupported <= requested, "result signal scope invalid")
    require(not available & unsupported, "result signal partition overlaps")
    closed_object(value["binding"], BINDING_KEYS, "result binding")
    for key, child in value["binding"].items():
        if child is None:
            continue
        require(is_id(child) if key == "reference_id" else is_sha(child), f"result binding {key} invalid")
    require(isinstance(value["part_errors"], list) and len(value["part_errors"]) <= 64, "result part errors invalid")
    validate_readback(value["readback"])
    require(isinstance(value["unknowns"], list) and len(value["unknowns"]) <= 16, "result unknowns invalid")
    lineage_keys = {"project_id", "candidate_id", "target_sha256"} | BINDING_KEYS
    closed_object(value["lineage"], lineage_keys, "result lineage")
    require(value["lineage"]["project_id"] == value["project_id"], "lineage project crossed")
    require(value["lineage"]["candidate_id"] == value["candidate_id"], "lineage candidate crossed")
    require(value["lineage"]["target_sha256"] == value["target_sha256"], "lineage target crossed")
    for key in BINDING_KEYS:
        require(value["lineage"][key] == value["binding"][key], f"lineage {key} drifted")
    require(is_sha(value["canonical_sha256"]) and canonical_hash(value) == value["canonical_sha256"], "result canonical hash drifted")


def main() -> int:
    request_schema = json.loads((SCHEMA_ROOT / "visual-surface-request.schema.json").read_text(encoding="utf-8"))
    result_schema = json.loads((SCHEMA_ROOT / "visual-surface-result.schema.json").read_text(encoding="utf-8"))
    critic_schema = json.loads((SCHEMA_ROOT / "design-critic-report-projection.schema.json").read_text(encoding="utf-8"))
    require(request_schema["properties"]["schema_version"]["const"] == "VisualSurfaceRequest@1", "request schema version missing")
    require(result_schema["properties"]["schema_version"]["const"] == "VisualSurfaceResult@1", "result schema version missing")
    require(request_schema["additionalProperties"] is False and result_schema["additionalProperties"] is False, "schemas must be closed")
    runtime_source = RUNTIME_SOURCE.read_text(encoding="utf-8")
    optimization_source = OPTIMIZATION_SOURCE.read_text(encoding="utf-8")
    mcp_source = MCP_SOURCE.read_text(encoding="utf-8")
    for token in ("visual_surface_get", "validate_visual_surface_result", "surface-program-not-run", "AGENTIC_BINDING_FAIL_CLOSED"):
        require(token in runtime_source, f"Runtime producer marker missing: {token}")
    require("VisualSurfaceResult@1" in mcp_source and "VisualSurfaceRequest@1" in mcp_source, "MCP typed surface markers missing")
    require("visual_surface" in critic_schema["properties"] and "visual_surface" in critic_schema["required"], "Critic schema is not surface-bound")
    require(
        "OPTIMIZATION_RESIDUAL_VISUAL_SURFACE_UNAVAILABLE" in optimization_source
        and "OPTIMIZATION_RESIDUAL_VISUAL_SURFACE_BINDING_MISMATCH" in optimization_source
        and "source_visual_surface_sha256" in optimization_source
        and "OPTIMIZATION_RESIDUAL_VISUAL_SURFACE_SIGNAL_BINDING_MISMATCH" in optimization_source,
        "CADFit surface readback revalidation markers missing",
    )
    require(
        "surface_signal_status" in critic_schema["$defs"]["visual_surface"]["properties"]
        and "surface_signal_canonical_sha256" in critic_schema["$defs"]["visual_surface"]["properties"],
        "Critic surface-signal projection fields are missing",
    )

    binding = {key: None for key in BINDING_KEYS}
    request = {
        "schema_version": "VisualSurfaceRequest@1",
        "project_id": "project-surface-fixture",
        "candidate_id": "candidate-surface-fixture",
        "requested_signals": ["silhouette", "boundary", "curvature"],
        "expected_binding": binding,
        "target_sha256": None,
        "max_part_errors": 8,
        "canonical_sha256": "",
    }
    request["canonical_sha256"] = canonical_hash(request)
    validate_request(request)

    invalid_extra = dict(request)
    invalid_extra["raw_image_path"] = "/tmp/secret.png"
    try:
        closed_object(invalid_extra, set(request), "negative extra-field request")
    except SystemExit:
        pass
    else:
        fail("extra field negative fixture unexpectedly passed")
    invalid_duplicate = dict(request)
    invalid_duplicate["requested_signals"] = ["silhouette", "silhouette"]
    try:
        validate_request(invalid_duplicate)
    except SystemExit:
        pass
    else:
        fail("duplicate signal negative fixture unexpectedly passed")

    result_binding = {key: None for key in BINDING_KEYS}
    readback = {
        "schema_version": "VisualSurfaceReadback@1",
        "status": "not-run",
        "resolution": [512, 512],
        "reference_mask": {
            "sha256": None,
            "decoded": False,
            "foreground_pixels": None,
            "edge_pixels": None,
            "bbox": None,
        },
        "candidate_mask": {
            "sha256": None,
            "decoded": False,
            "foreground_pixels": None,
            "edge_pixels": None,
            "bbox": None,
        },
        "edge": {
            "status": "not-run",
            "radius_px": 4,
            "reference_edge_pixels": None,
            "candidate_edge_pixels": None,
            "matched_reference_edge_pixels": None,
            "matched_candidate_edge_pixels": None,
            "f1": None,
            "sdf_chamfer_px": None,
        },
        "roi": {
            "status": "not-run",
            "source": "not-run",
            "part_id_sha256": None,
            "material_id_sha256": None,
            "parts": [],
            "regions": [],
            "unknowns": ["render-set-not-run"],
        },
        "aov": {
            "status": "not-run",
            "source": "RenderSet@2/pass_artifacts",
            "passes": [],
            "missing_passes": list(AOV_PASSES),
        },
        "surface": {
            "schema_version": "SurfaceSignalReadback@1",
            "status": "not-run",
            "artifact_sha256": None,
            "triangle_count": None,
            "vertex_count": None,
            "edge_count": None,
            "non_manifold_edge_count": None,
            "curvature": {
                "status": "not-run",
                "method": "not-run",
                "mean_abs_dihedral_rad": None,
                "max_abs_dihedral_rad": None,
                "curved_triangle_count": None,
            },
            "feature_line": {
                "status": "not-run",
                "method": "not-run",
                "threshold_rad": None,
                "edge_count": None,
                "boundary_edge_count": None,
                "crease_edge_count": None,
            },
            "canonical_sha256": "",
        },
        "canonical_sha256": "",
    }
    readback["canonical_sha256"] = canonical_hash(readback)
    result = {
        "schema_version": "VisualSurfaceResult@1",
        "projection_status": "projection/read-only",
        "read_only": True,
        "project_id": request["project_id"],
        "candidate_id": request["candidate_id"],
        "target_sha256": request["target_sha256"],
        "status": "blocked",
        "backend": "candidate-bound-aov-diagnostics@1",
        "surface_program_status": "not-run",
        "requested_signals": ["silhouette", "curvature"],
        "available_signals": [],
        "unsupported_signals": ["curvature"],
        "binding": result_binding,
        "metrics": {key: None for key in ("silhouette_iou", "boundary_f1_4px", "bbox_edge_error", "centroid_error", "landmark_coverage", "landmark_nme", "region_median_iou", "critical_region_min_iou")},
        "part_errors": [],
        "readback": readback,
        "unknowns": ["surface-program-not-run", "unsupported-curvature"],
        "lineage": {"project_id": request["project_id"], "candidate_id": request["candidate_id"], "target_sha256": request["target_sha256"], **result_binding},
        "canonical_sha256": "",
    }
    result["canonical_sha256"] = canonical_hash(result)
    validate_result(result)
    cross_candidate = dict(result)
    cross_candidate["lineage"] = dict(result["lineage"])
    cross_candidate["lineage"]["candidate_id"] = "candidate-other"
    try:
        validate_result(cross_candidate)
    except SystemExit:
        pass
    else:
        fail("cross-candidate lineage negative fixture unexpectedly passed")

    print(json.dumps({
        "schema_version": "ForgeCADVisualSurfaceContractGate@1",
        "status": "PASS_TYPED_BOUNDARY_WITH_SURFACE_ANALYSIS_NEGATIVE_GATES",
        "backend": "candidate-bound-surface-analysis@1",
        "surface_program": "BOUNDED_MESH_DERIVED",
        "unsupported": ["SubD", "NURBS-principal-curvature", "quality-promotion"],
    }, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
