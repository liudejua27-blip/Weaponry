#!/usr/bin/env python3
"""Offline U002 contract gate; performs no Provider or geometry call."""

from __future__ import annotations

import copy
import json
from pathlib import Path

from jsonschema import Draft202012Validator, RefResolver, ValidationError


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_DIR = ROOT / "packages" / "concept-spec" / "schemas"
FIXTURE = ROOT / "packages" / "concept-spec" / "fixtures" / "u002-open-subject-profile-set.json"


def load(name: str) -> dict:
    return json.loads((SCHEMA_DIR / name).read_text(encoding="utf-8"))


def validator(name: str) -> Draft202012Validator:
    schema = load(name)
    store = {}
    for path in SCHEMA_DIR.glob("*.json"):
        value = json.loads(path.read_text(encoding="utf-8"))
        if "$id" in value:
            store[value["$id"]] = value
        store[path.name] = value
    return Draft202012Validator(schema, resolver=RefResolver.from_schema(schema, store=store))


def profile(subject: dict) -> dict:
    return {
        "schema_version": "SubjectProfile@1",
        "profile_id": f"subject_{subject['fixture_id']}",
        "request_sha256": "a" * 64,
        "identity_label": subject["identity_label"],
        "category": subject["category"],
        "category_tags": subject["traits"],
        "silhouette": "subject-specific primary silhouette",
        "negative_space": "subject-specific negative-space structure",
        "pose": "stable presentation pose",
        "visible_views": ["front"],
        "occlusions": ["rear surface hidden"],
        "uncertainties": ["unobserved rear appearance"],
        "parts": [{
            "part_id": f"part_{subject['fixture_id']}",
            "label": subject["identity_label"],
            "semantic_role": "primary_subject",
            "traits": subject["traits"],
            "uncertainty_bps": 2500,
        }],
        "features": [
            {"feature_id": f"feature_{subject['fixture_id']}_macro", "part_id": f"part_{subject['fixture_id']}", "level": "macro", "description": "primary silhouette and mass"},
            {"feature_id": f"feature_{subject['fixture_id']}_meso", "part_id": f"part_{subject['fixture_id']}", "level": "meso", "description": "part proportions and regions"},
            {"feature_id": f"feature_{subject['fixture_id']}_micro", "part_id": f"part_{subject['fixture_id']}", "level": "micro", "description": "surface finish and local detail"},
        ],
        "materials": [{"material_id": f"material_{subject['fixture_id']}", "label": "primary appearance", "part_ids": [f"part_{subject['fixture_id']}"], "appearance_traits": ["subject_specific"]}],
    }


def main() -> None:
    fixture = json.loads(FIXTURE.read_text(encoding="utf-8"))
    assert fixture["schema_version"] == "U002OpenSubjectProfileSet@1"
    assert len(fixture["fixtures"]) == 8
    profile_validator = validator("subject-profile.schema.json")
    categories = set()
    for subject in fixture["fixtures"]:
        value = profile(subject)
        profile_validator.validate(value)
        assert {feature["level"] for feature in value["features"]} == {"macro", "meso", "micro"}
        categories.add(value["category"])
    assert len(categories) == 8

    # Category remains open text rather than a product allowlist.
    category_schema = load("subject-profile.schema.json")["properties"]["category"]
    assert category_schema.get("type") == "string" and "enum" not in category_schema

    # An unsupported observed claim is structurally rejected by the Rust
    # validator; the portable schema still makes all evidence status explicit.
    feature_schema = load("visual-feature-contract.schema.json")
    statuses = feature_schema["$defs"]["requirement"]["properties"]["evidence_status"]["enum"]
    assert statuses == ["observed", "inferred", "hidden", "conflicting"]

    limitation = {
        "schema_version": "RepresentationLimitation@1",
        "code": "representation_unavailable",
        "message": "Current representation capability is unavailable.",
        "affected_part_ids": ["part_cat"],
        "missing_capability_ids": ["deformable.generic_v1"],
        "suggested_views": ["front", "side", "back"],
        "retryable": True,
    }
    validator("representation-limitation.schema.json").validate(limitation)
    invalid = copy.deepcopy(limitation)
    invalid["code"] = "fallback_to_c111"
    try:
        validator("representation-limitation.schema.json").validate(invalid)
    except ValidationError:
        pass
    else:
        raise AssertionError("unknown limitation code must fail closed")

    reference_enum = load("reference-evidence.schema.json")["properties"]["domain_pack_id"]["enum"]
    assert "pack_unclassified" in reference_enum

    executor = (ROOT / "apps" / "desktop" / "src-tauri" / "crates" / "forgecad-app-server" / "src" / "product_tools" / "native_executor.rs").read_text(encoding="utf-8")
    assert "Err(provider_error) if arm_fallback_allowed" not in executor
    assert "multimodal_binding.is_err() && arm_fallback_allowed" not in executor
    print("u002 universal author contract gate ok: 8 open categories, typed limitations, no product fallback")


if __name__ == "__main__":
    main()
