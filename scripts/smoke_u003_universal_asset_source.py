#!/usr/bin/env python3
"""Offline U003 portable-contract gate; performs no Provider or geometry call."""

from __future__ import annotations

import copy
import json
from pathlib import Path

from jsonschema import Draft202012Validator, ValidationError
from referencing import Registry, Resource


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_DIR = ROOT / "packages" / "concept-spec" / "schemas"


def load(name: str) -> dict:
    return json.loads((SCHEMA_DIR / name).read_text(encoding="utf-8"))


def validator(name: str) -> Draft202012Validator:
    schema = load(name)
    resources = []
    for path in SCHEMA_DIR.glob("*.json"):
        value = json.loads(path.read_text(encoding="utf-8"))
        if "$id" in value:
            resources.append((value["$id"], Resource.from_contents(value)))
    return Draft202012Validator(schema, registry=Registry().with_resources(resources))


def expect_invalid(name: str, value: dict) -> None:
    try:
        validator(name).validate(value)
    except ValidationError:
        return
    raise AssertionError(f"{name} unexpectedly accepted an invalid value")


def property_names(value: object) -> set[str]:
    if isinstance(value, dict):
        names = set(value.get("properties", {}).keys())
        for nested in value.values():
            names.update(property_names(nested))
        return names
    if isinstance(value, list):
        names: set[str] = set()
        for nested in value:
            names.update(property_names(nested))
        return names
    return set()


def main() -> None:
    camera = {
        "schema_version": "ReferenceCameraHypothesis@1",
        "hypothesis_id": "camera_evidence_front",
        "evidence_id": "evidence_front",
        "view_id": "front",
        "projection_type": "unknown",
        "parameter_source": "unresolved",
        "vertical_fov_millidegrees": None,
        "reprojection_error_bps": None,
        "landmark_feature_ids": [],
        "confidence_bps": 0,
        "unresolved_fields": ["projection_type", "intrinsics", "extrinsics"],
    }
    validator("reference-camera-hypothesis.schema.json").validate(camera)
    invalid_camera = copy.deepcopy(camera)
    invalid_camera["projection_type"] = "solved"
    expect_invalid("reference-camera-hypothesis.schema.json", invalid_camera)

    bundle = {
        "schema_version": "AppearanceEvidenceBundle@1",
        "bundle_id": "appearance_evidence_fixture",
        "request_sha256": "a" * 64,
        "references": [{"evidence_id": "evidence_front", "evidence_sha256": "b" * 64}],
        "camera_hypotheses": [camera],
        "derived_artifacts": [],
    }
    validator("appearance-evidence-bundle.schema.json").validate(bundle)

    detail = {
        "schema_version": "VisualDetailClaim@2",
        "claim_id": "detail_claim_macro",
        "feature_id": "feature_macro",
        "level": "macro",
        "evidence_status": "observed",
        "salience_bps": 10000,
        "affected_part_ids": ["part_subject"],
        "channels": ["geometry"],
        "silhouette_impact": True,
        "bindings": [{"kind": "procedural_program", "source_id": "visualprog_fixture"}],
        "minimum_acceptance_views": ["front", "iso"],
    }
    validator("visual-detail-claim-v2.schema.json").validate(detail)
    invalid_detail = copy.deepcopy(detail)
    invalid_detail["bindings"][0]["kind"] = "provider_code"
    expect_invalid("visual-detail-claim-v2.schema.json", invalid_detail)

    material = {
        "schema_version": "MaterialZoneAppearance@1",
        "appearance_id": "part_shell:zone_shell",
        "material_zone_id": "zone_shell",
        "source_part_id": "part_shell",
        "base_material_id": "mat_aluminum",
        "finish": "reviewed_catalog_pbr",
        "coating": None,
        "transmission_bps": 0,
        "uncertainty_bps": 5000,
        "texture_width": 1024,
        "texture_height": 1024,
        "channels": ["base_color", "metallic", "roughness", "normal", "occlusion", "emissive"],
        "projection_layers": [],
    }
    validator("material-zone-appearance.schema.json").validate(material)
    invalid_material = copy.deepcopy(material)
    invalid_material["projection_layers"] = [{
        "layer_id": "projection_unsafe",
        "evidence_artifact_id": "artifact_color",
        "camera_hypothesis_id": "camera_evidence_front",
        "channels": ["base_color"],
        "unobserved_texel_mask_artifact_id": "artifact_unobserved",
        "shader_code": "arbitrary()",
    }]
    expect_invalid("material-zone-appearance.schema.json", invalid_material)

    universal = load("universal-asset-source.schema.json")
    required = set(universal["required"])
    assert {
        "request", "subject_profile", "visual_feature_contract", "representation_plan",
        "procedural_source", "component_sources", "detail_claims", "material_zones",
        "appearance_evidence",
    }.issubset(required)
    assert universal["properties"]["state"]["enum"] == ["planned", "compiled"]
    assert universal["properties"]["procedural_source"]["$ref"] == "forge-visual-program-revision.schema.json"

    forbidden = {"url", "path", "file_path", "shader", "shader_code", "script", "code", "provider_payload"}
    for schema_name in [
        "reference-camera-hypothesis.schema.json",
        "appearance-evidence-bundle.schema.json",
        "visual-detail-claim-v2.schema.json",
        "material-zone-appearance.schema.json",
        "universal-asset-source.schema.json",
    ]:
        assert not (property_names(load(schema_name)) & forbidden), schema_name

    core = (ROOT / "apps" / "desktop" / "src-tauri" / "crates" / "forgecad-core" / "src" / "universal_asset_source.rs").read_text(encoding="utf-8")
    executor = (ROOT / "apps" / "desktop" / "src-tauri" / "crates" / "forgecad-app-server" / "src" / "product_tools" / "native_executor.rs").read_text(encoding="utf-8")
    bridge = (ROOT / "apps" / "desktop" / "src-tauri" / "src" / "app_server_bridge.rs").read_text(encoding="utf-8")
    assert "REFERENCE_CAMERA_FALSE_SOLVE" in core
    assert "APPEARANCE_PROJECTION_LAYER_INVALID" in core
    assert "derive_universal_asset_source_for_revision" in executor
    assert '"universal_asset_source".into()' in bridge
    print("u003 universal asset source gate ok: Rust-derived source, hash-only appearance evidence, no arbitrary projection code")


if __name__ == "__main__":
    main()
