"""FGC-R007 evidence normalization and robotic-arm constrained rebuild tests."""

from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path

import pytest

from forgecad_agent.application.geometry_worker import compile_shape_program
from forgecad_agent.application.reference_guided_rebuild import (
    ReferenceEvidenceError,
    build_robotic_arm_rebuild_plan,
    extract_reference_evidence,
)


ROOT = Path(__file__).resolve().parents[3]
FIXTURE = Path(__file__).with_name("fixtures") / "r007_robotic_arm_reference.json"


def _fixture() -> dict:
    return json.loads(FIXTURE.read_text(encoding="utf-8"))


def _robotic_arm_glb() -> bytes:
    golden = json.loads((ROOT / "packages/concept-spec/fixtures/component-recipe-expanded-golden.json").read_text(encoding="utf-8"))
    candidate = next(item for item in golden["candidates"] if item["recipe"]["recipe_id"] == "recipe_robotic_arm_link")
    return compile_shape_program(candidate["expanded_shape_program"], artifact_profile_id="interactive_preview").glb_bytes


def test_forgecad_glb_becomes_read_only_evidence_and_constrained_plan() -> None:
    fixture = _fixture()
    glb = _robotic_arm_glb()
    before = hashlib.sha256(glb).hexdigest()
    evidence = extract_reference_evidence(fixture["source"], glb)
    plan = build_robotic_arm_rebuild_plan(evidence)
    assert evidence.reference_sha256 == before == evidence.source_unchanged_sha256
    assert evidence.source_read_only is True
    assert evidence.readback_kind == "forgecad_glb_readback"
    assert evidence.facts["triangle_count"] > 0
    assert plan.reference_sha256 == before
    assert plan.read_only_reference is True
    assert plan.c105_recipe_ids == fixture["expected"]["recipes"]
    assert plan.g819_operation_allowlist == fixture["expected"]["operations"]
    assert "参考 GLB/图片保持只读" in plan.reference_constraints[2]
    assert hashlib.sha256(glb).hexdigest() == before


def test_repeated_evidence_and_plan_are_deterministic() -> None:
    fixture = _fixture()
    glb = _robotic_arm_glb()
    first = extract_reference_evidence(fixture["source"], glb)
    second = extract_reference_evidence(fixture["source"], glb)
    assert first.model_dump(mode="json") == second.model_dump(mode="json")
    assert build_robotic_arm_rebuild_plan(first).model_dump(mode="json") == build_robotic_arm_rebuild_plan(second).model_dump(mode="json")


def test_single_image_is_declaration_only_and_marks_missing_views() -> None:
    source = {
        "source_id": "reference_arm_single_image",
        "cas_object_id": "cas_arm_single_image",
        "source_kind": "image",
        "source_statement": "用户授权的机械臂外观图片。",
        "license_statement": "用户声明拥有测试使用权。",
        "declared_view": "side",
        "visible_features": ["joint", "upper_link", "cable"],
        "color_blocks": ["dark_metal", "blue_accent"],
    }
    evidence = extract_reference_evidence(source, b"\x89PNG\r\n\x1a\nfixture")
    assert evidence.readback_kind == "image_declaration"
    assert evidence.view_coverage == ["side"]
    assert "top" in evidence.missing_views
    assert any("隐藏结构" in item for item in evidence.uncertainties)
    plan = build_robotic_arm_rebuild_plan(evidence)
    assert plan.read_only_reference is True
    assert plan.c105_recipe_ids == ["recipe_robotic_arm_link", "recipe_robotic_arm_detail"]


def test_invalid_image_and_unapproved_source_are_rejected() -> None:
    source = {
        "source_id": "reference_invalid_image",
        "cas_object_id": "cas_invalid_image",
        "source_kind": "image",
        "source_statement": "授权。",
        "license_statement": "测试。",
    }
    with pytest.raises(ReferenceEvidenceError, match="REFERENCE_IMAGE_FORMAT_INVALID"):
        extract_reference_evidence(source, b"not-image")
    unauthorized = copy.deepcopy(source)
    unauthorized["authorization"] = "internet_search"
    with pytest.raises(ValueError):
        extract_reference_evidence(unauthorized, b"\x89PNG\r\n\x1a\nfixture")


def test_external_generic_glb_is_metadata_only_not_a_mesh_copy() -> None:
    # A small valid GLB that lacks ForgeCAD provenance.  It remains usable only
    # as conservative metadata evidence, never as executable geometry.
    from forgecad_agent.application.combined_glb import write_glb

    glb = write_glb(
        {"asset": {"version": "2.0"}, "scene": 0, "scenes": [{"nodes": [0]}], "nodes": [{"name": "arm_joint_shell"}], "meshes": [], "buffers": [{"byteLength": 0}]},
        b"",
    )
    fixture = _fixture()
    source = {**fixture["source"], "cas_object_id": "cas_external_glb_metadata"}
    evidence = extract_reference_evidence(source, glb)
    assert evidence.readback_kind == "generic_glb_metadata"
    assert evidence.facts["analysis"] == "metadata_only_no_mesh_copy"
    assert build_robotic_arm_rebuild_plan(evidence).g819_operation_allowlist == ["sweep", "box"]
