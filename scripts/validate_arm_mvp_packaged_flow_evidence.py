#!/usr/bin/env python3
"""Fail-closed validator for one real packaged robotic-arm MVP lineage.

This file never starts a model, desktop window, or compatibility proxy.  The
packaged UI E2E is the sole producer of its report; this validator only makes
the required identity joins explicit before the aggregate Gate can pass.
"""

from __future__ import annotations

import json
from pathlib import Path
import sys
from typing import Any, NoReturn


SCHEMA_VERSION = "ForgeCADArmMvpPackagedFlowEvidence@1"
PACKAGED_PROTOCOL_SCHEMA_VERSION = "ForgeCADArmMvpPackagedProtocolProof@3"
GOLDEN_BRIEF = "流线三关节维护机械臂，固定基座、双连杆、旋转腕部和夹爪"
SERVICE_ROOT = "recipe_c106_arm_service_display"


class EvidenceFailure(ValueError):
    pass


def fail(code: str) -> NoReturn:
    raise EvidenceFailure(code)


def object_at(value: object, *, code: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        fail(code)
    return value


def string_at(value: object, *, code: str) -> str:
    if not isinstance(value, str) or not value:
        fail(code)
    return value


def sha256(value: object, *, code: str) -> str:
    candidate = string_at(value, code=code)
    if len(candidate) != 64 or any(character not in "0123456789abcdef" for character in candidate):
        fail(code)
    return candidate


def same(value: object, expected: str, *, code: str) -> str:
    candidate = string_at(value, code=code)
    if candidate != expected:
        fail(code)
    return candidate


def validate_evidence(value: object) -> dict[str, Any]:
    evidence = object_at(value, code="ARM_PACKAGED_EVIDENCE_OBJECT_INVALID")
    if evidence.get("schema_version") == PACKAGED_PROTOCOL_SCHEMA_VERSION:
        return validate_packaged_protocol_v3(evidence)
    if evidence.get("schema_version") != SCHEMA_VERSION or evidence.get("status") != "pass":
        fail("ARM_PACKAGED_EVIDENCE_SCHEMA_INVALID")
    same(evidence.get("brief"), GOLDEN_BRIEF, code="ARM_PACKAGED_BRIEF_MISMATCH")
    same(evidence.get("root_recipe_id"), SERVICE_ROOT, code="ARM_PACKAGED_ROOT_MISMATCH")
    project_id = string_at(evidence.get("project_id"), code="ARM_PACKAGED_PROJECT_ID_MISSING")
    turn_id = string_at(evidence.get("turn_id"), code="ARM_PACKAGED_TURN_ID_MISSING")
    preview = object_at(evidence.get("preview"), code="ARM_PACKAGED_PREVIEW_MISSING")
    preview_id = string_at(preview.get("preview_id"), code="ARM_PACKAGED_PREVIEW_ID_MISSING")
    preview_sha = sha256(preview.get("glb_sha256"), code="ARM_PACKAGED_PREVIEW_SHA_INVALID")
    same(preview.get("artifact_profile_id"), "production_concept", code="ARM_PACKAGED_PREVIEW_PROFILE_INVALID")
    versions = object_at(evidence.get("versions"), code="ARM_PACKAGED_VERSIONS_MISSING")
    v1 = object_at(versions.get("v1"), code="ARM_PACKAGED_V1_MISSING")
    v1_id = string_at(v1.get("asset_version_id"), code="ARM_PACKAGED_V1_ID_MISSING")
    same(v1.get("project_id"), project_id, code="ARM_PACKAGED_V1_PROJECT_DRIFT")
    same(v1.get("turn_id"), turn_id, code="ARM_PACKAGED_V1_TURN_DRIFT")
    same(v1.get("preview_id"), preview_id, code="ARM_PACKAGED_V1_PREVIEW_DRIFT")
    same(v1.get("preview_glb_sha256"), preview_sha, code="ARM_PACKAGED_V1_GLB_DRIFT")
    a005 = object_at(versions.get("a005"), code="ARM_PACKAGED_A005_MISSING")
    change_set_id = string_at(a005.get("change_set_id"), code="ARM_PACKAGED_A005_CHANGESET_MISSING")
    v2_id = string_at(a005.get("asset_version_id"), code="ARM_PACKAGED_V2_ID_MISSING")
    same(a005.get("project_id"), project_id, code="ARM_PACKAGED_V2_PROJECT_DRIFT")
    same(a005.get("parent_asset_version_id"), v1_id, code="ARM_PACKAGED_A005_PARENT_DRIFT")
    if not isinstance(a005.get("surface_adornment_count"), int) or a005["surface_adornment_count"] < 1:
        fail("ARM_PACKAGED_A005_PROVENANCE_MISSING")
    restart = object_at(evidence.get("restart"), code="ARM_PACKAGED_RESTART_MISSING")
    same(restart.get("project_id"), project_id, code="ARM_PACKAGED_RESTART_PROJECT_DRIFT")
    same(restart.get("active_asset_version_id"), v2_id, code="ARM_PACKAGED_RESTART_HEAD_DRIFT")
    if not isinstance(restart.get("snapshot_revision"), int) or restart["snapshot_revision"] < 1:
        fail("ARM_PACKAGED_RESTART_SNAPSHOT_INVALID")
    export = object_at(evidence.get("export"), code="ARM_PACKAGED_EXPORT_MISSING")
    same(export.get("asset_version_id"), v2_id, code="ARM_PACKAGED_EXPORT_VERSION_DRIFT")
    export_sha = sha256(export.get("glb_sha256"), code="ARM_PACKAGED_EXPORT_SHA_INVALID")
    headers = object_at(export.get("headers"), code="ARM_PACKAGED_EXPORT_HEADERS_MISSING")
    same(headers.get("x-forgecad-glb-sha256"), export_sha, code="ARM_PACKAGED_EXPORT_HEADER_DRIFT")
    renderer = object_at(evidence.get("renderer"), code="ARM_PACKAGED_RENDERER_MISSING")
    if renderer.get("canvas_count") != 1 or renderer.get("load_state") != "ready":
        fail("ARM_PACKAGED_RENDERER_INVALID")
    provider = object_at(evidence.get("provider"), code="ARM_PACKAGED_PROVIDER_MISSING")
    # The opt-in MVP provider deliberately performs eight *local* ActionLoop
    # subrequests.  Zero is required only at the external/credential boundary;
    # treating all Provider activity as zero would reject the real offline
    # deterministic path and hide its tool-loop provenance.
    if (
        provider.get("source_kind") != "offline_deterministic"
        or provider.get("internal_subrequests") != 8
        or provider.get("action_loop_steps") != 8
        or provider.get("product_tool_calls") != 7
        or provider.get("external_network_calls") != 0
        or provider.get("credential_reads") != 0
    ):
        fail("ARM_PACKAGED_PROVIDER_POLICY_INVALID")
    if evidence.get("visual_formal") is not False or evidence.get("m108b_status") != "blocked":
        fail("ARM_PACKAGED_FORMAL_STATUS_INVALID")
    return {
        "schema_version": SCHEMA_VERSION,
        "project_id": project_id,
        "turn_id": turn_id,
        "preview_id": preview_id,
        "preview_glb_sha256": preview_sha,
        "v1_asset_version_id": v1_id,
        "a005_change_set_id": change_set_id,
        "v2_asset_version_id": v2_id,
        "restart_asset_version_id": v2_id,
        "export_glb_sha256": export_sha,
        "provider": {
            "source_kind": "offline_deterministic",
            "internal_subrequests": 8,
            "action_loop_steps": 8,
            "product_tool_calls": 7,
            "external_network_calls": 0,
            "credential_reads": 0,
        },
        "visual_formal": False,
        "m108b_status": "blocked",
    }


def validate_packaged_protocol_v3(evidence: dict[str, Any]) -> dict[str, Any]:
    """Validate the current Rust-owned four-version packaged golden path."""
    if evidence.get("status") != "pass":
        fail("ARM_PACKAGED_PROTOCOL_STATUS_INVALID")
    same(evidence.get("brief"), GOLDEN_BRIEF, code="ARM_PACKAGED_BRIEF_MISMATCH")
    same(evidence.get("root_recipe_id"), SERVICE_ROOT, code="ARM_PACKAGED_ROOT_MISMATCH")
    project_id = string_at(evidence.get("project_id"), code="ARM_PACKAGED_PROJECT_ID_MISSING")
    turn_id = string_at(evidence.get("turn_id"), code="ARM_PACKAGED_TURN_ID_MISSING")
    preview = object_at(evidence.get("preview"), code="ARM_PACKAGED_PREVIEW_MISSING")
    preview_id = string_at(preview.get("preview_id"), code="ARM_PACKAGED_PREVIEW_ID_MISSING")
    preview_sha = sha256(preview.get("glb_sha256"), code="ARM_PACKAGED_PREVIEW_SHA_INVALID")
    same(preview.get("artifact_profile_id"), "production_concept", code="ARM_PACKAGED_PREVIEW_PROFILE_INVALID")
    if not isinstance(preview.get("triangle_count"), int) or not 80_000 <= preview["triangle_count"] <= 150_000:
        fail("ARM_PACKAGED_PREVIEW_TRIANGLE_BUDGET_INVALID")
    a005 = object_at(evidence.get("a005"), code="ARM_PACKAGED_A005_MISSING")
    v1_id = string_at(evidence.get("v1_asset_version_id"), code="ARM_PACKAGED_V1_ID_MISSING")
    v2_id = string_at(a005.get("v2_asset_version_id"), code="ARM_PACKAGED_V2_ID_MISSING")
    same(a005.get("parent_asset_version_id"), v1_id, code="ARM_PACKAGED_A005_PARENT_DRIFT")
    if not isinstance(a005.get("surface_adornment_count"), int) or a005["surface_adornment_count"] < 1:
        fail("ARM_PACKAGED_A005_PROVENANCE_MISSING")
    c110c = object_at(evidence.get("c110c"), code="ARM_PACKAGED_C110C_MISSING")
    v3_id = string_at(c110c.get("v3_asset_version_id"), code="ARM_PACKAGED_V3_ID_MISSING")
    same(c110c.get("parent_asset_version_id"), v2_id, code="ARM_PACKAGED_C110C_PARENT_DRIFT")
    if c110c.get("operation_count") != 3:
        fail("ARM_PACKAGED_C110C_OPERATION_COUNT_INVALID")
    c110d = object_at(evidence.get("c110d"), code="ARM_PACKAGED_C110D_MISSING")
    v4_id = string_at(c110d.get("v4_asset_version_id"), code="ARM_PACKAGED_V4_ID_MISSING")
    same(c110d.get("parent_asset_version_id"), v3_id, code="ARM_PACKAGED_C110D_PARENT_DRIFT")
    if c110d.get("operation_count") != 2 or c110d.get("added_part_ids") != ["part_c110d_actuator_cover", "part_c110d_cable_guide"]:
        fail("ARM_PACKAGED_C110D_OPERATIONS_INVALID")
    active = object_at(evidence.get("active_design"), code="ARM_PACKAGED_ACTIVE_DESIGN_MISSING")
    same(active.get("asset_version_id"), v4_id, code="ARM_PACKAGED_ACTIVE_HEAD_DRIFT")
    export = object_at(evidence.get("export"), code="ARM_PACKAGED_EXPORT_MISSING")
    same(export.get("asset_version_id"), v4_id, code="ARM_PACKAGED_EXPORT_VERSION_DRIFT")
    export_sha = sha256(export.get("glb_sha256"), code="ARM_PACKAGED_EXPORT_SHA_INVALID")
    same(export.get("x_forgecad_glb_sha256"), export_sha, code="ARM_PACKAGED_EXPORT_HEADER_DRIFT")
    if not isinstance(export.get("triangle_count"), int) or not 80_000 <= export["triangle_count"] <= 150_000:
        fail("ARM_PACKAGED_EXPORT_TRIANGLE_BUDGET_INVALID")
    provider = object_at(evidence.get("provider"), code="ARM_PACKAGED_PROVIDER_MISSING")
    expected_provider = {
        "source_kind": "offline_deterministic",
        "internal_subrequests": 8,
        "action_loop_steps": 8,
        "product_tool_calls": 7,
        "external_network_calls": 0,
        "credential_reads": 0,
    }
    if provider != expected_provider:
        fail("ARM_PACKAGED_PROVIDER_POLICY_INVALID")
    return {
        "schema_version": PACKAGED_PROTOCOL_SCHEMA_VERSION,
        "project_id": project_id,
        "turn_id": turn_id,
        "preview_id": preview_id,
        "preview_glb_sha256": preview_sha,
        "v1_asset_version_id": v1_id,
        "v2_asset_version_id": v2_id,
        "v3_asset_version_id": v3_id,
        "v4_asset_version_id": v4_id,
        "export_glb_sha256": export_sha,
        "provider": expected_provider,
        "visual_formal": False,
        "m108b_status": "blocked",
    }


def main(path: Path) -> int:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise EvidenceFailure("ARM_PACKAGED_EVIDENCE_READ_FAILED") from exc
    print(json.dumps(validate_evidence(value), ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("usage: validate_arm_mvp_packaged_flow_evidence.py <evidence.json>", file=sys.stderr)
        raise SystemExit(2)
    try:
        raise SystemExit(main(Path(sys.argv[1])))
    except EvidenceFailure as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1) from None
