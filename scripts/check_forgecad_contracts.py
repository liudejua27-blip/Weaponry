#!/usr/bin/env python3
"""MCP002 contract smoke: every checked-in JSON contract must be valid and versioned."""

from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CONTRACT_ROOT = ROOT / "packages" / "forgecad-contracts"
SCHEMA_ROOT = CONTRACT_ROOT / "schemas"

ANIMATED_SOCKET_PARTICLES_V2_POLICY = (
    "projection-v2-driven-animated-socket-particles-dual-candidate@2"
)
ANIMATED_SOCKET_PARTICLES_V2_TRANSFORM_PROJECTION_POLICY = (
    "glb-animation-linear-nlerp-flat-part-hierarchy-composed-world-trs-matrix@2"
)
ANIMATED_SOCKET_PARTICLES_V1_POLICY = "projection-driven-animated-socket-particles-dual-candidate@1"
ANIMATED_SOCKET_PARTICLES_V1_TRANSFORM_PROJECTION_POLICY = (
    "glb-animation-linear-nlerp-flat-part-hierarchy-composed-world-trs@1"
)
ANIMATED_SOCKET_TRAILS_V2_POLICY = (
    "projection-v2-driven-animated-socket-trails-dual-candidate@2"
)
ANIMATED_SOCKET_TRAILS_V2_HISTORY_POLICY = (
    "particles-v2-history-oldest-to-newest-ordinal-zero-last-immediate-previous-earlier-particle-frames-plus-current@2"
)
ANIMATED_SOCKET_TRAILS_V2_HISTORY_PREROLL_POLICY = (
    "same-parent-particles-v2-frame-zero-is-preroll-output-frames-one-to-fifteen@2"
)
ANIMATED_SOCKET_TRAILS_V2_FRAME_SCOPE = (
    "lod0-animation-trails-v2-source-frames-1-15-with-particles-v2-frame-zero-preroll@2"
)
ANIMATED_SOCKET_TRAILS_BLOOM_V2_POLICY = (
    "projection-v2-driven-animated-socket-trails-bloom-dual-candidate@2"
)
ANIMATED_SOCKET_TRAILS_BLOOM_V2_FRAME_SCOPE = (
    "lod0-animation-trails-bloom-v2-source-frames-1-15-with-trails-v2-frame-zero-preroll@2"
)
ANIMATED_SOCKET_TRAILS_BLOOM_V2_TRAIL_KEY_SCOPE = (
    "animated-socket-trails-sequence-v2-frame-binding@2"
)
CANDIDATE_ANIMATION_VFX_QUALITY_V2_SCOPE = (
    "lod0-rigid-animation-full-vfx-stack-attachment-v3-all-15-frames@2"
)
CANDIDATE_ANIMATION_VFX_QUALITY_V2_POLICY = (
    "candidate-animation-vfx-attachment-v3-structural-hard-gate@2"
)
CANDIDATE_ANIMATION_VFX_QUALITY_V2_BINDING_STATUS = (
    "same-material-surface-head-candidate-exact-attachment-v3-all-15-frames-no-geometry-mutation"
)
CANDIDATE_ANIMATION_VFX_QUALITY_V2_FRAME_SET_SCHEMA = (
    "CandidateAnimationVfxQualityAttachmentFrameSet@1"
)

HIGH_LOW_BAKE_POLICIES = {
    "high": "production-weapon-independent-high-detail-graph@1",
    "low": "production-weapon-independent-low-retopology@1",
    "cage": "production-weapon-low-bound-cage-offset-field@1",
    "correspondence": "production-weapon-high-low-cage-part-face-corner-correspondence@1",
    "plan": "production-weapon-high-low-cage-ray-diagnostic-plan@1",
    "diagnostic": "production-weapon-high-low-cage-ray-diagnostic@1",
    "bake": "production-weapon-high-low-cage-bake-gate@1",
    "mode": "independent-high-low-cage-ray-bake@1",
}
HIGH_LOW_SCHEMA_VERSIONS = {
    "production-weapon-high-artifact.schema.json": "ProductionWeaponHighArtifact@1",
    "production-weapon-low-artifact.schema.json": "ProductionWeaponLowArtifact@1",
    "production-weapon-cage-artifact.schema.json": "ProductionWeaponCageArtifact@1",
    "production-weapon-high-low-correspondence.schema.json": "ProductionWeaponHighLowCorrespondence@1",
    "production-weapon-high-low-bake-plan.schema.json": "ProductionWeaponHighLowBakePlan@1",
    "production-weapon-high-low-diagnostic.schema.json": "ProductionWeaponHighLowDiagnostic@1",
    "production-weapon-high-low-bake-receipt.schema.json": "ProductionWeaponHighLowBakeReceipt@1",
    "production-weapon-high-low-bake-prepare-request.schema.json": "ProductionWeaponHighLowBakePrepareRequest@1",
    "production-weapon-high-low-bake-prepare-result.schema.json": "ProductionWeaponHighLowBakePrepareResult@1",
    "production-weapon-high-low-bake-get-request.schema.json": "ProductionWeaponHighLowBakeGetRequest@1",
    "production-weapon-high-low-bake-get-result.schema.json": "ProductionWeaponHighLowBakeGetResult@1",
}

BLENDER_WORKER_CAPABILITY_SCHEMA_VERSIONS = {
    "blender-worker-capability.schema.json": "BlenderWorkerCapability@1",
    "blender-worker-capability-get-request.schema.json": "BlenderWorkerCapabilityGetRequest@1",
    "blender-worker-capability-get-result.schema.json": "BlenderWorkerCapabilityGetResult@1",
}
BLENDER_WORKER_CAPABILITY_ID = "blender-headless-worker-evaluation"
BLENDER_WORKER_CAPABILITY_SOURCE_REVISION = "72ccdd6e96ca119a1ffa3372559cc5654343b477"
BLENDER_WORKER_CAPABILITY_GATE_STATUSES = ["not-run", "pending", "passed", "failed", "blocked"]
BLENDER_WORKER_CAPABILITY_GATE_FIELDS = [
    "binary_status",
    "recipe_status",
    "python_bundle_status",
    "license_status",
    "sandbox_status",
    "determinism_status",
    "package_gate_status",
]
BLENDER_WORKER_CAPABILITY_HASH_FIELDS = [
    "binary_sha256",
    "recipe_sha256",
    "python_bundle_sha256",
    "license_file_sha256",
    "license_full_text_sha256",
    "sandbox_sha256",
    "determinism_sha256",
    "package_sha256",
]


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(f"ForgeCAD contract violation: {message}")


def load_schema(name: str) -> dict:
    return json.loads((SCHEMA_ROOT / name).read_text(encoding="utf-8"))


def require_required(schema: dict, expected: set[str], label: str) -> None:
    actual = set(schema.get("required", []))
    missing = sorted(expected - actual)
    require(not missing, f"{label} missing required fields: {missing}")


def check_recessed_channel_kit_enums() -> None:
    """Keep the channel kit reachable from every action/review/profile contract."""
    action_kits = {
        "forgecad.kit.housing@1",
        "forgecad.kit.panel@1",
        "forgecad.kit.vent@1",
        "forgecad.kit.channel@1",
        "forgecad.kit.energy-core@1",
        "forgecad.kit.joint@1",
        "forgecad.kit.sensor@1",
        "forgecad.kit.frame@1",
        "forgecad.kit.handle@1",
        "forgecad.kit.foot@1",
        "forgecad.kit.wheel@1",
        "forgecad.kit.fastener@1",
        "forgecad.kit.cable@1",
        "forgecad.kit.light@1",
    }
    repair = load_schema("repair-intent.schema.json")
    critic = load_schema("design-critic-report.schema.json")
    profile = load_schema("fictional-energy-rifle-profile.schema.json")
    require(
        set(repair["$defs"]["kit_id"].get("enum", [])) == action_kits,
        "RepairIntent kit_id enum must include the bounded channel kit",
    )
    require(
        set(critic["$defs"]["kit_id"].get("enum", [])) == action_kits,
        "DesignCriticReport kit_id enum must include the bounded channel kit",
    )
    proposed_action_kits = critic["$defs"]["proposed_action"]["properties"]["kit_id"].get("enum", [])
    require(
        proposed_action_kits[:1] == [None]
        and set(proposed_action_kits[1:]) == action_kits,
        "DesignCriticReport proposed_action kit_id enum must mirror the closed action kit set",
    )
    profile_kits = set(
        profile["$defs"]["macro_intent"]["properties"]["kit_id"].get("enum", [])
    )
    require(
        profile_kits
        == {
            "forgecad.kit.housing@1",
            "forgecad.kit.panel@1",
            "forgecad.kit.vent@1",
            "forgecad.kit.channel@1",
            "forgecad.kit.energy-core@1",
            "forgecad.kit.joint@1",
            "forgecad.kit.sensor@1",
            "forgecad.kit.frame@1",
        }
        and "forgecad.geometry.recessed-channel@1"
        in profile["$defs"]["macro_intent"]["properties"]["operator_id"].get("enum", []),
        "FictionalEnergyRifleProfile must bind the channel kit to recessed-channel@1",
    )
    require(
        "forgecad.geometry.energy-core@1"
        in profile["$defs"]["macro_intent"]["properties"]["operator_id"].get("enum", [])
        and "forgecad.kit.energy-core@1" in profile_kits,
        "FictionalEnergyRifleProfile must bind the energy-core kit to energy-core@1",
    )
    pdk_kits = {
        "forgecad.kit.housing@1",
        "forgecad.kit.panel@1",
        "forgecad.kit.vent@1",
        "forgecad.kit.channel@1",
        "forgecad.kit.energy-core@1",
        "forgecad.kit.joint@1",
        "forgecad.kit.sensor@1",
        "forgecad.kit.frame@1",
    }
    for schema_name, location in [
        ("fictional-energy-rifle-plan.schema.json", ("$defs", "pdk_request", "properties")),
        ("parametric-design-kit-request.schema.json", ("properties",)),
        ("parametric-design-kit-program.schema.json", ("properties",)),
    ]:
        schema = load_schema(schema_name)
        node = schema
        for key in location:
            node = node[key]
        require(
            set(node["kit_id"].get("enum", [])) == pdk_kits,
            f"{schema_name} kit_id enum must mirror the bounded PDK kit set",
        )


def check_production_weapon_high_low_cage_contracts() -> None:
    """Keep FPS-HIGH-LOW-CAGE-05 independent from SurfaceBake and transient LOD."""
    expected_files = set(HIGH_LOW_SCHEMA_VERSIONS)
    actual_files = {path.name for path in SCHEMA_ROOT.glob("*.json")}
    require(
        expected_files <= actual_files,
        "FPS-HIGH-LOW-CAGE-05 schema files are missing",
    )

    schemas = {name: load_schema(name) for name in expected_files}
    for name, schema_version in HIGH_LOW_SCHEMA_VERSIONS.items():
        schema = schemas[name]
        require(
            schema.get("title") == schema_version
            and schema.get("properties", {}).get("schema_version", {}).get("const")
            == schema_version
            and schema.get("additionalProperties") is False
            and set(schema.get("required", [])) == set(schema.get("properties", {})),
            f"{schema_version} must be a closed exact-field schema",
        )

    high = schemas["production-weapon-high-artifact.schema.json"]
    low = schemas["production-weapon-low-artifact.schema.json"]
    cage = schemas["production-weapon-cage-artifact.schema.json"]
    correspondence = schemas["production-weapon-high-low-correspondence.schema.json"]
    plan = schemas["production-weapon-high-low-bake-plan.schema.json"]
    diagnostic = schemas["production-weapon-high-low-diagnostic.schema.json"]
    receipt = schemas["production-weapon-high-low-bake-receipt.schema.json"]
    prepare_request = schemas["production-weapon-high-low-bake-prepare-request.schema.json"]
    prepare_result = schemas["production-weapon-high-low-bake-prepare-result.schema.json"]
    get_request = schemas["production-weapon-high-low-bake-get-request.schema.json"]
    get_result = schemas["production-weapon-high-low-bake-get-result.schema.json"]

    require_required(
        high,
        {
            "high_artifact_id",
            "source_candidate_id",
            "source_artifact_sha256",
            "high_candidate_id",
            "high_artifact_sha256",
            "high_artifact_readback_sha256",
            "high_geometry_program_sha256",
            "high_detail_graph_object_sha256",
            "high_part_inventory_sha256",
            "high_part_ids",
            "high_material_zone_ids",
            "high_policy",
            "high_artifact_kind",
            "high_mime",
            "high_worker_build_cohort_sha256",
            "high_worker_replay_count",
            "high_replay_byte_exact",
            "high_authoring_topology_status",
            "quality_status",
            "visual_status",
            "hard_gate_passed",
            "candidate_confirmed",
            "version_created",
            "export_performed",
        },
        "ProductionWeaponHighArtifact@1",
    )
    require(
        high["properties"]["high_policy"].get("const") == HIGH_LOW_BAKE_POLICIES["high"]
        and high["properties"]["high_artifact_kind"].get("const")
        == "production-weapon-high-artifact-glb"
        and high["properties"]["high_mime"].get("const") == "model/gltf-binary"
        and high["properties"]["high_worker_replay_count"].get("const") == 2
        and high["properties"]["high_replay_byte_exact"].get("const") is True,
        "HighArtifact must be an independently replay-verified GLB",
    )

    require_required(
        low,
        {
            "source_high_candidate_id",
            "source_high_artifact_sha256",
            "low_candidate_id",
            "low_artifact_sha256",
            "low_artifact_readback_sha256",
            "low_retopology_policy",
            "low_triangle_budget_sha256",
            "low_authoring_topology_status",
            "low_authoring_topology_object_sha256",
            "low_uv_binding_sha256",
            "low_tangent_input_sha256",
            "low_artifact_kind",
            "low_mime",
            "low_worker_build_cohort_sha256",
            "low_worker_replay_count",
            "low_replay_byte_exact",
            "quality_status",
            "visual_status",
            "hard_gate_passed",
        },
        "ProductionWeaponLowArtifact@1",
    )
    require(
        low["properties"]["low_retopology_policy"].get("const") == HIGH_LOW_BAKE_POLICIES["low"]
        and low["properties"]["low_authoring_topology_status"].get("const") == "complete"
        and low["properties"]["low_uv_status"].get("const") == "passed"
        and low["properties"]["low_tangent_status"].get("const") == "passed"
        and low["properties"]["low_artifact_kind"].get("const")
        == "production-weapon-low-artifact-glb"
        and low["properties"]["low_mime"].get("const") == "model/gltf-binary"
        and low["properties"]["low_worker_replay_count"].get("const") == 2
        and low["properties"]["low_replay_byte_exact"].get("const") is True,
        "LowArtifact must be independent authored retopology with UV/tangent evidence",
    )

    require_required(
        cage,
        {
            "source_high_artifact_sha256",
            "source_low_artifact_sha256",
            "cage_artifact_id",
            "cage_artifact_sha256",
            "cage_artifact_readback_sha256",
            "cage_topology_correspondence_sha256",
            "cage_offset_field_object_sha256",
            "cage_offset_field_canonical_sha256",
            "cage_self_intersection_count",
            "cage_cross_part_count",
            "cage_out_of_range_count",
            "cage_skew_count",
            "cage_artifact_kind",
            "cage_mime",
            "cage_worker_build_cohort_sha256",
            "cage_worker_replay_count",
            "cage_replay_byte_exact",
            "quality_status",
            "visual_status",
            "hard_gate_passed",
        },
        "ProductionWeaponCageArtifact@1",
    )
    require(
        cage["properties"]["cage_policy"].get("const") == HIGH_LOW_BAKE_POLICIES["cage"]
        and cage["properties"]["cage_artifact_kind"].get("const")
        == "production-weapon-cage-artifact-glb"
        and cage["properties"]["cage_mime"].get("const") == "model/gltf-binary"
        and cage["properties"]["cage_worker_replay_count"].get("const") == 2
        and cage["properties"]["cage_replay_byte_exact"].get("const") is True,
        "CageArtifact must bind an independent mesh, offset field and diagnostics",
    )

    require_required(
        correspondence,
        {
            "high_artifact_id",
            "high_artifact_sha256",
            "low_artifact_id",
            "low_artifact_sha256",
            "cage_artifact_id",
            "cage_artifact_sha256",
            "part_inventory_sha256",
            "part_ids",
            "material_zone_ids",
            "correspondence_policy",
            "part_pairs",
            "mapping_object_sha256",
            "unmapped_count",
            "ambiguous_count",
            "cross_part_count",
            "cross_material_zone_count",
            "mapping_status",
            "quality_status",
            "visual_status",
            "production_stage_advanced",
        },
        "ProductionWeaponHighLowCorrespondence@1",
    )
    require(
        correspondence["properties"]["correspondence_policy"].get("const")
        == HIGH_LOW_BAKE_POLICIES["correspondence"]
        and correspondence["$defs"]["part_pair"].get("additionalProperties") is False
        and "part_pair" not in correspondence["properties"]
        and "part_pair" not in correspondence["required"]
        and correspondence["properties"]["part_pairs"].get("maxItems") == 256,
        "HighLowCorrespondence must use only the closed bounded part_pairs array",
    )

    for schema, label in [(plan, "BakePlan"), (diagnostic, "Diagnostic")]:
        require(
            schema["properties"]["normal_convention"].get("const") == "OpenGL+Y"
            and schema["properties"]["surface_bake_reuse_allowed"].get("const") is False
            and schema["properties"]["bake_mode"].get("const") == HIGH_LOW_BAKE_POLICIES["mode"],
            f"{label} must explicitly exclude CandidateSurfaceBake@1",
        )
    require(
        plan["properties"]["bake_policy"].get("const") == HIGH_LOW_BAKE_POLICIES["plan"]
        and plan["properties"]["diagnostic_required"].get("const") is True
        and plan["properties"]["output_semantics"].get("const")
        == ["tangent-normal", "ao", "curvature", "thickness", "position", "object-id", "material-id", "part-id"],
        "HighLowBakePlan must bind ray policies, diagnostics and map semantics",
    )
    require(
        diagnostic["properties"]["diagnostic_policy"].get("const") == HIGH_LOW_BAKE_POLICIES["diagnostic"]
        and diagnostic["properties"]["diagnostic_status"].get("enum")
        == ["NOT_RUN", "BLOCKED", "FAILED", "PASS_SOURCE_STRUCTURAL"]
        and diagnostic["properties"]["high_low_bake_status"].get("$ref")
        == "#/$defs/bake_status",
        "HighLowDiagnostic must expose bounded ray diagnostics and bake status",
    )

    require_required(
        receipt,
        {
            "bake_receipt_id",
            "gate_scope",
            "source_stage",
            "target_stage",
            "high_artifact_id",
            "low_artifact_id",
            "cage_artifact_id",
            "correspondence_id",
            "bake_plan_id",
            "diagnostic_id",
            "high_status",
            "low_status",
            "cage_status",
            "correspondence_status",
            "diagnostic_status",
            "high_low_bake_status",
            "hard_gate",
            "hard_gate_passed",
            "stage_advance_allowed",
            "production_stage_advanced",
            "quality_status",
            "visual_status",
            "human_status",
            "engine_status",
            "distribution_status",
            "limitations",
        },
        "ProductionWeaponHighLowBakeReceipt@1",
    )
    require(
        receipt["properties"]["bake_policy"].get("const") == HIGH_LOW_BAKE_POLICIES["bake"]
        and receipt["properties"]["stage_advance_allowed"].get("const") is False
        and receipt["properties"]["production_stage_advanced"].get("const") is False
        and receipt["properties"]["quality_status"].get("const") == "structural_only"
        and receipt["$defs"]["hard_gate"].get("additionalProperties") is False,
        "HighLowBakeReceipt must remain structural-only and non-promoting",
    )
    require(
        len(receipt.get("allOf", [])) == 4,
        "HighLowBakeReceipt must map each scope to one adjacent ProductionStage@3 edge",
    )

    require(
        prepare_request["properties"]["bake_policy"].get("const") == HIGH_LOW_BAKE_POLICIES["bake"]
        and prepare_request["properties"]["idempotency_key"].get("$ref") == "#/$defs/id"
        and set(prepare_request["properties"]) == set(prepare_request["required"]),
        "HighLowBakePrepareRequest must be closed, hash-bound and idempotent",
    )
    for result, schema_version, runtime_write in [
        (prepare_result, "ProductionWeaponHighLowBakePrepareResult@1", None),
        (get_result, "ProductionWeaponHighLowBakeGetResult@1", False),
    ]:
        require(
            result["properties"]["schema_version"].get("const") == schema_version
            and result["properties"]["bake_receipt"].get("$ref")
            == "production-weapon-high-low-bake-receipt.schema.json"
            and result["properties"]["restart_hash_verified"].get("const") is True
            and result["properties"]["production_stage_advanced"].get("const") is False
            and result["properties"]["candidate_confirmed"].get("const") is False
            and result["properties"]["version_created"].get("const") is False
            and result["properties"]["export_performed"].get("const") is False,
            f"{schema_version} must be restart-verified and non-promoting",
        )
        if runtime_write is not None:
            require(
                result["properties"]["runtime_write"].get("const") is runtime_write,
                "HighLowBakeGetResult must be read-only",
            )
    require(
        set(get_request["properties"]) == set(get_request["required"])
        and get_request["properties"]["schema_version"].get("const")
        == "ProductionWeaponHighLowBakeGetRequest@1",
        "HighLowBakeGetRequest must be a closed key-bound read request",
    )

    # Explicitly prevent accidental semantic reuse of old surface/LOD receipts.
    for schema in schemas.values():
        require(
            "surface_bake_receipt_object_sha256" not in schema.get("properties", {})
            and "game_asset_lod_derive_result" not in schema.get("properties", {}),
            "High/Low/Cage contracts must not accept CandidateSurfaceBake or transient LOD as source truth",
        )


def check_production_weapon_high_low_bake_preflight_contracts() -> None:
    request = load_schema("production-weapon-high-low-bake-preflight-get-request.schema.json")
    result = load_schema("production-weapon-high-low-bake-preflight-get-result.schema.json")
    for schema, title in (
        (request, "ProductionWeaponHighLowBakePreflightGetRequest@1"),
        (result, "ProductionWeaponHighLowBakePreflightGetResult@1"),
    ):
        require(
            schema.get("title") == title
            and schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and set(schema.get("required", [])) == set(schema.get("properties", {})),
            f"{title} must be closed and exact-field",
        )
    require(
        request["properties"]["schema_version"].get("const")
        == "ProductionWeaponHighLowBakePreflightGetRequest@1"
        and "except input_sha256" in request["properties"]["input_sha256"].get("description", ""),
        "HighLowBake preflight request identity/hash semantics drifted",
    )
    expected_checks = {
        "secondary_form_head", "formal_high_artifact", "authoring_low_topology",
        "hero_uv_layout", "formal_cage_artifact", "high_low_correspondence",
        "ray_diagnostic", "formal_bake",
    }
    checks = result["properties"]["checks"]
    require(
        checks.get("additionalProperties") is False
        and set(checks.get("required", [])) == expected_checks
        and set(checks.get("properties", {})) == expected_checks,
        "HighLowBake preflight checks drifted",
    )
    require(
        checks["properties"]["secondary_form_head"].get("$ref") == "#/$defs/head_check"
        and all(
            checks["properties"][name].get("$ref") == "#/$defs/unverified_check"
            for name in expected_checks - {"secondary_form_head"}
        )
        and set(result["$defs"]["head_check"]["properties"]["status"].get("enum", []))
        == {"passed", "blocked", "missing", "failed"}
        and set(result["$defs"]["unverified_check"]["properties"]["status"].get("enum", []))
        == {"blocked", "missing", "failed"},
        "HighLowBake preflight must not schema-authorize unverified formal checks as passed",
    )
    properties = result["properties"]
    require(
        properties["quality_status"].get("const") == "structural_only"
        and properties["visual_quality_status"].get("const") == "NOT_PROVEN"
        and properties["human_review_status"].get("const") == "NOT_RUN"
        and properties["commercial_engine_status"].get("const") == "NOT_RUN"
        and properties["distribution_status"].get("const") == "NOT_RUN"
        and properties["runtime_write"].get("const") is False
        and properties["worker_started"].get("const") is False
        and properties["production_stage_advanced"].get("const") is False
        and properties["candidate_confirmed"].get("const") is False
        and properties["version_created"].get("const") is False
        and properties["export_performed"].get("const") is False
        and properties["restart_hash_verified"].get("const") is True,
        "HighLowBake preflight must remain structural-only, read-only and non-promoting",
    )


def check_blender_worker_capability_contracts() -> None:
    """Keep ADR-0028 capability discovery closed, unavailable until gated, and non-promoting."""
    expected_files = set(BLENDER_WORKER_CAPABILITY_SCHEMA_VERSIONS)
    actual_files = {path.name for path in SCHEMA_ROOT.glob("*.json")}
    require(
        expected_files <= actual_files,
        "BlenderWorkerCapability@1 schema files are missing",
    )

    schemas = {name: load_schema(name) for name in expected_files}
    for name, schema_version in BLENDER_WORKER_CAPABILITY_SCHEMA_VERSIONS.items():
        schema = schemas[name]
        properties = schema.get("properties", {})
        require(
            schema.get("title") == schema_version
            and properties.get("schema_version", {}).get("const") == schema_version
            and schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and set(schema.get("required", [])) == set(properties),
            f"{schema_version} must be a closed exact-field schema",
        )

    capability = schemas["blender-worker-capability.schema.json"]
    request = schemas["blender-worker-capability-get-request.schema.json"]
    result = schemas["blender-worker-capability-get-result.schema.json"]
    capability_fields = {
        "schema_version", "capability_id", "worker_id", "worker_kind", "source_identity",
        "source_revision", "adoption_status", "capability_status", "binary_status",
        "binary_sha256", "recipe_id", "recipe_version", "recipe_status", "recipe_sha256",
        "python_bundle_status", "python_bundle_sha256", "license_name", "license_spdx",
        "license_status", "license_file_sha256", "license_full_text_sha256", "sandbox_status",
        "sandbox_sha256", "determinism_status", "determinism_sha256", "package_gate_status",
        "package_sha256", "read_only", "runtime_write_performed", "worker_invoked",
        "candidate_generated", "production_stage_advanced", "candidate_confirmed",
        "version_created", "export_performed", "limitations", "canonical_sha256",
    }
    request_fields = {"schema_version", "capability_id"}
    result_fields = {
        "schema_version", "capability", "read_only", "runtime_write_performed", "worker_invoked",
        "candidate_generated", "production_stage_advanced", "candidate_confirmed", "version_created",
        "export_performed",
    }
    require(
        set(capability.get("properties", {})) == capability_fields
        and set(request.get("properties", {})) == request_fields
        and set(result.get("properties", {})) == result_fields,
        "BlenderWorkerCapability request/result field sets drifted",
    )
    require(
        capability["properties"]["capability_id"].get("const") == BLENDER_WORKER_CAPABILITY_ID
        and capability["properties"]["worker_id"].get("const") == "blender"
        and capability["properties"]["worker_kind"].get("const") == "tool/worker"
        and capability["properties"]["source_identity"].get("const")
        == "official-reference-only-research"
        and capability["properties"]["source_revision"].get("const")
        == BLENDER_WORKER_CAPABILITY_SOURCE_REVISION
        and capability["properties"]["adoption_status"].get("const")
        == "approved-for-evaluation"
        and capability["properties"]["license_name"].get("const")
        == "GNU General Public License version 2 or later"
        and capability["properties"]["license_spdx"].get("const") == "GPL-2.0-or-later"
        and request["properties"]["capability_id"].get("const") == BLENDER_WORKER_CAPABILITY_ID
        and result["properties"]["capability"].get("$ref")
        == "https://forgecad.local/contracts/blender-worker-capability.schema.json",
        "BlenderWorkerCapability identity, adoption and license bindings drifted",
    )

    capability_properties = capability["properties"]
    require(
        capability["$defs"]["gate_status"].get("enum") == BLENDER_WORKER_CAPABILITY_GATE_STATUSES
        and capability["$defs"]["sha256"].get("pattern") == "^[0-9a-f]{64}$"
        and capability["$defs"]["nullable_sha256"].get("anyOf")
        and all(
            capability_properties[field].get("$ref") == "#/$defs/nullable_sha256"
            for field in BLENDER_WORKER_CAPABILITY_HASH_FIELDS
        )
        and capability_properties["canonical_sha256"].get("$ref") == "#/$defs/sha256"
        and capability_properties["capability_status"].get("enum") == ["unavailable", "available"],
        "BlenderWorkerCapability status/hash definitions drifted",
    )
    require(
        capability_properties["read_only"].get("const") is True
        and all(
            capability_properties[field].get("const") is False
            for field in (
                "runtime_write_performed", "worker_invoked", "candidate_generated",
                "production_stage_advanced", "candidate_confirmed", "version_created",
                "export_performed",
            )
        )
        and result["properties"]["read_only"].get("const") is True
        and all(
            result["properties"][field].get("const") is False
            for field in (
                "runtime_write_performed", "worker_invoked", "candidate_generated",
                "production_stage_advanced", "candidate_confirmed", "version_created",
                "export_performed",
            )
        ),
        "BlenderWorkerCapability must remain read-only and non-promoting",
    )

    all_of = capability.get("allOf", [])
    available_branch = next(
        (
            branch for branch in all_of
            if branch.get("if", {}).get("properties", {})
            .get("capability_status", {}).get("const") == "available"
        ),
        None,
    )
    unavailable_branch = next(
        (
            branch for branch in all_of
            if branch.get("then", {}).get("properties", {})
            .get("capability_status", {}).get("const") == "unavailable"
        ),
        None,
    )
    all_pass_branch = next(
        (
            branch for branch in all_of
            if isinstance(branch.get("if", {}).get("allOf"), list)
            and branch.get("then", {}).get("properties", {})
            .get("capability_status", {}).get("const") == "available"
        ),
        None,
    )
    require(
        available_branch is not None
        and unavailable_branch is not None
        and all_pass_branch is not None,
        "BlenderWorkerCapability must encode both availability directions",
    )
    available_properties = available_branch["then"].get("properties", {})
    require(
        set(available_properties) >= set(BLENDER_WORKER_CAPABILITY_GATE_FIELDS + BLENDER_WORKER_CAPABILITY_HASH_FIELDS)
        and all(
            available_properties[field].get("const") == "passed"
            for field in BLENDER_WORKER_CAPABILITY_GATE_FIELDS
        )
        and all(
            available_properties[field].get("$ref") == "#/$defs/sha256"
            for field in BLENDER_WORKER_CAPABILITY_HASH_FIELDS
        ),
        "available must require every passed gate and every non-null SHA-256",
    )

    all_pass_conditions = all_pass_branch["if"].get("allOf", [])
    for field in BLENDER_WORKER_CAPABILITY_GATE_FIELDS:
        require(
            any(
                condition.get("properties", {}).get(field, {}).get("const") == "passed"
                for condition in all_pass_conditions
            ),
            f"all-passed availability guard must include {field}",
        )
    for field in BLENDER_WORKER_CAPABILITY_HASH_FIELDS:
        require(
            any(
                condition.get("properties", {}).get(field, {}).get("$ref") == "#/$defs/sha256"
                for condition in all_pass_conditions
            ),
            f"all-passed availability guard must include {field}",
        )

    unavailable_conditions = unavailable_branch["if"].get("anyOf", [])
    not_passed = {"not-run", "pending", "failed", "blocked"}
    for field in BLENDER_WORKER_CAPABILITY_GATE_FIELDS:
        require(
            any(
                set(condition.get("properties", {}).get(field, {}).get("enum", [])) == not_passed
                for condition in unavailable_conditions
            ),
            f"unavailable guard must cover a non-passed {field}",
        )
    for field in BLENDER_WORKER_CAPABILITY_HASH_FIELDS:
        require(
            any(
                condition.get("properties", {}).get(field, {}).get("type") == "null"
                for condition in unavailable_conditions
            ),
            f"unavailable guard must cover a missing {field}",
        )

    forbidden_property_names = {
        "path", "file_path", "absolute_path", "url", "uri", "script", "script_path",
        "python", "javascript", "raw", "raw_bytes", "bytes", "environment", "env",
        "secret", "network",
    }

    def property_names(node: object) -> set[str]:
        names: set[str] = set()
        if isinstance(node, dict):
            properties = node.get("properties")
            if isinstance(properties, dict):
                names.update(properties)
            for value in node.values():
                names.update(property_names(value))
        elif isinstance(node, list):
            for value in node:
                names.update(property_names(value))
        return names

    for schema, filename in [
        (capability, "blender-worker-capability.schema.json"),
        (request, "blender-worker-capability-get-request.schema.json"),
        (result, "blender-worker-capability-get-result.schema.json"),
    ]:
        require(
            forbidden_property_names.isdisjoint(property_names(schema)),
            f"{filename} must reject paths, URLs, scripts, raw bytes and environment inputs",
        )


def check_blender_task_contracts() -> None:
    """Keep the not-yet-enabled Blender Worker transport closed and bounded."""
    request = load_schema("blender-task-request.schema.json")
    result = load_schema("blender-task-result.schema.json")
    error = load_schema("blender-task-error.schema.json")
    for schema, title in (
        (request, "BlenderTaskRequest@1"),
        (result, "BlenderTaskResult@1"),
        (error, "BlenderTaskError@1"),
    ):
        require(
            schema.get("title") == title
            and schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and schema.get("x-operation") == "blender.render_fixed@1",
            f"{title} must remain a closed fixed-operation contract",
        )
    request_properties = request["properties"]
    require(
        set(request["required"]) == set(request_properties)
        and request_properties["schema_version"].get("const") == "BlenderTaskRequest@1"
        and request_properties["recipe_id"].get("const") == "forgecad-blender-render-fixed@1"
        and request_properties["recipe_version"].get("const") == "1.0.0"
        and request_properties["network_policy"].get("const") == "disabled"
        and request_properties["filesystem_policy"].get("const") == "runtime_scratch_only"
        and request_properties["script_policy"].get("const") == "frozen_bundle_only"
        and request_properties["output_policy"].get("const") == "runtime_cas_after_readback"
        and "operation" not in request_properties,
        "BlenderTaskRequest identity/policies drifted",
    )
    budgets = request["$defs"]["budgets"]
    require(
        budgets.get("additionalProperties") is False
        and budgets["properties"]["max_runtime_ms"].get("maximum") == 120000
        and budgets["properties"]["max_cpu_seconds"].get("maximum") == 120
        and budgets["properties"]["max_memory_bytes"].get("maximum") == 536870912
        and budgets["properties"]["max_gpu_bytes"].get("const") == 0
        and budgets["properties"]["max_stderr_bytes"].get("maximum") == 65536,
        "BlenderTaskRequest budget ceilings drifted",
    )
    input_object = request["$defs"]["input_object"]
    output = result["$defs"]["output"]
    checks = result["$defs"]["checks"]
    require(
        input_object.get("additionalProperties") is False
        and output.get("additionalProperties") is False
        and checks.get("additionalProperties") is False
        and output["properties"]["cas_owner"].get("const") == "runtime"
        and output["properties"]["durability"].get("const") == "pending_runtime_adoption",
        "BlenderTask nested transport objects drifted",
    )
    result_properties = result["properties"]
    require(
        set(result["required"]) == set(result_properties)
        and result_properties["runtime_write"].get("const") is False
        and result_properties["stage_advanced"].get("const") is False
        and result_properties["candidate_confirmed"].get("const") is False
        and result_properties["version_created"].get("const") is False
        and result_properties["export_performed"].get("const") is False,
        "BlenderTaskResult must remain non-writing and non-promoting",
    )
    require(
        set(error["required"]) == {"code", "message"}
        and error["properties"]["message"].get("maxLength") == 512
        and "CAPABILITY_UNAVAILABLE" in error["$defs"]["code"].get("enum", [])
        and "WORKER_SANDBOX_VIOLATION" in error["$defs"]["code"].get("enum", []),
        "BlenderTaskError bounds/error family drifted",
    )
    forbidden = {
        "path", "url", "env", "secret", "token", "script", "addon", "blend",
        "argv", "command", "executable", "module", "import", "subprocess", "host",
        "socket", "cas", "sqlite",
    }
    def property_names(node: object) -> set[str]:
        names: set[str] = set()
        if isinstance(node, dict):
            properties = node.get("properties")
            if isinstance(properties, dict):
                names.update(properties)
            for value in node.values():
                names.update(property_names(value))
        elif isinstance(node, list):
            for value in node:
                names.update(property_names(value))
        return names
    for schema in (request, result, error):
        require(forbidden.isdisjoint(property_names(schema)), "BlenderTask exposes an escape hatch")


def check_mcp010b_contracts() -> None:
    """Keep V2 geometry truth contracts closed while V1 remains addressable."""
    expected_schemas = {
        "geometry-program-v2.schema.json",
        "geometry-program-hash-request.schema.json",
        "geometry-program-hash-result.schema.json",
        "operator-catalog.schema.json",
        "artifact-readback-v2.schema.json",
        "geometry-prepare-result-v2.schema.json",
        "geometry-quality-report-v2.schema.json",
        "geometry-candidate-evidence.schema.json",
    }
    actual_schemas = {path.name for path in SCHEMA_ROOT.glob("*.json")}
    require(expected_schemas <= actual_schemas, "MCP010B V2 schema files are missing")

    geometry = load_schema("geometry-program-v2.schema.json")
    require(
        geometry.get("properties", {}).get("schema_version", {}).get("const") == "GeometryProgram@2",
        "GeometryProgram@2 schema_version is not closed",
    )
    require_required(
        geometry,
        {
            "schema_version",
            "project_id",
            "representation_plan_sha256",
            "operator_catalog_sha256",
            "units",
            "budgets",
            "nodes",
            "part_outputs",
            "canonical_sha256",
        },
        "GeometryProgram@2",
    )
    units = geometry["properties"]["units"]
    require(
        units.get("properties", {}).get("length", {}).get("const") == "meter"
        and units.get("properties", {}).get("angle", {}).get("const") == "radian"
        and units.get("properties", {}).get("coordinate_system", {}).get("const") == "right-handed-y-up",
        "GeometryProgram@2 units must be meter/radian/right-handed-y-up",
    )
    budget_properties = geometry["properties"]["budgets"].get("properties", {})
    require(
        budget_properties.get("max_nodes", {}).get("maximum") == 512
        and budget_properties.get("max_triangles", {}).get("maximum") == 250000
        and budget_properties.get("max_glb_bytes", {}).get("maximum") == 67108864
        and budget_properties.get("max_worker_memory_bytes", {}).get("maximum") == 536870912
        and budget_properties.get("max_runtime_ms", {}).get("maximum") == 10000,
        "GeometryProgram@2 limits do not match MCP010B budgets",
    )
    node_properties = geometry["$defs"]["geometry_node"].get("properties", {})
    require(
        set(node_properties.get("operator_id", {}).get("enum", []))
        == {
            "forgecad.geometry.primitive@2",
            "forgecad.geometry.profile-extrude@1",
            "forgecad.geometry.profile-loft@1",
            "forgecad.geometry.profile-loft@2",
            "forgecad.geometry.multi-loop-profile-loft@1",
            "forgecad.geometry.longitudinal-section-loft@1",
            "forgecad.geometry.subd-cage@1",
            "forgecad.geometry.subd-cage@2",
            "forgecad.geometry.authoring-mesh@1",
            "forgecad.geometry.surface-patch@1",
            "forgecad.geometry.surface-shell@1",
            "forgecad.geometry.revolve@1",
            "forgecad.geometry.tube-sweep@1",
            "forgecad.geometry.transform@2",
            "forgecad.geometry.mirror@1",
            "forgecad.geometry.array@1",
            "forgecad.geometry.bevel@1",
            "forgecad.geometry.bevel@2",
            "forgecad.geometry.normal-policy@1",
            "forgecad.geometry.panel@1",
            "forgecad.geometry.panel@2",
            "forgecad.geometry.vent-array@1",
            "forgecad.geometry.vent-array@2",
            "forgecad.geometry.recessed-channel@1",
            "forgecad.geometry.energy-core@1",
            "forgecad.geometry.joint-stack@1",
            "forgecad.geometry.boolean@1",
            "forgecad.geometry.part-output@1",
        }
        and node_properties.get("inputs", {}).get("maxItems") == 64
        and node_properties.get("inputs", {}).get("uniqueItems") is True,
        "GeometryProgram@2 must expose the closed MCP010D operator set and explicit DAG inputs",
    )
    parameter_refs = {
        item.get("$ref")
        for item in node_properties.get("parameters", {}).get("oneOf", [])
    }
    require(
        parameter_refs
        == {
            "#/$defs/box_parameters",
            "#/$defs/cylinder_parameters",
            "#/$defs/ellipsoid_parameters",
            "#/$defs/sphere_parameters",
            "#/$defs/profile_extrude_parameters",
            "#/$defs/profile_loft_parameters",
            "#/$defs/profile_loft_v2_parameters",
            "#/$defs/multi_loop_profile_loft_parameters",
            "#/$defs/longitudinal_section_loft_parameters",
            "#/$defs/subd_cage_parameters",
            "#/$defs/subd_cage_crease_parameters",
            "#/$defs/authoring_mesh_parameters",
            "#/$defs/surface_patch_parameters",
            "#/$defs/surface_shell_parameters",
            "#/$defs/revolve_parameters",
            "#/$defs/tube_sweep_parameters",
            "#/$defs/transform_parameters",
            "#/$defs/mirror_parameters",
            "#/$defs/array_parameters",
            "#/$defs/bevel_parameters",
            "#/$defs/bevel_v2_parameters",
            "#/$defs/normal_policy_parameters",
            "#/$defs/panel_parameters",
            "#/$defs/panel_v2_parameters",
            "#/$defs/vent_array_parameters",
            "#/$defs/vent_array_v2_parameters",
            "#/$defs/recessed_channel_parameters",
            "#/$defs/energy_core_parameters",
            "#/$defs/joint_stack_parameters",
            "#/$defs/boolean_parameters",
            "#/$defs/part_output_parameters",
        },
        "GeometryProgram@2 operator parameter variants drifted",
    )
    operator_conditions = {}
    for condition in geometry["$defs"]["geometry_node"].get("allOf", []):
        operator_id = (
            condition.get("if", {})
            .get("properties", {})
            .get("operator_id", {})
            .get("const")
        )
        if operator_id:
            operator_conditions[operator_id] = condition.get("then", {}).get("properties", {})
    require(
        operator_conditions.get("forgecad.geometry.panel@1", {}).get("parameters", {}).get("$ref")
        == "#/$defs/panel_parameters"
        and operator_conditions.get("forgecad.geometry.panel@1", {}).get("inputs", {}).get("maxItems") == 0
        and operator_conditions.get("forgecad.geometry.panel@2", {}).get("parameters", {}).get("$ref")
        == "#/$defs/panel_v2_parameters"
        and operator_conditions.get("forgecad.geometry.panel@2", {}).get("inputs", {}).get("maxItems") == 0
        and operator_conditions.get("forgecad.geometry.vent-array@1", {}).get("parameters", {}).get("$ref")
        == "#/$defs/vent_array_parameters"
        and operator_conditions.get("forgecad.geometry.vent-array@1", {}).get("inputs", {}).get("maxItems") == 0
        and operator_conditions.get("forgecad.geometry.vent-array@2", {}).get("parameters", {}).get("$ref")
        == "#/$defs/vent_array_v2_parameters"
        and operator_conditions.get("forgecad.geometry.vent-array@2", {}).get("inputs", {}).get("maxItems") == 0
        and operator_conditions.get("forgecad.geometry.recessed-channel@1", {}).get("parameters", {}).get("$ref")
        == "#/$defs/recessed_channel_parameters"
        and operator_conditions.get("forgecad.geometry.recessed-channel@1", {}).get("inputs", {}).get("maxItems") == 0
        and operator_conditions.get("forgecad.geometry.energy-core@1", {}).get("parameters", {}).get("$ref")
        == "#/$defs/energy_core_parameters"
        and operator_conditions.get("forgecad.geometry.energy-core@1", {}).get("inputs", {}).get("maxItems") == 0
        and operator_conditions.get("forgecad.geometry.bevel@2", {}).get("parameters", {}).get("$ref")
        == "#/$defs/bevel_v2_parameters"
        and operator_conditions.get("forgecad.geometry.bevel@2", {}).get("inputs", {}).get("minItems") == 1
        and operator_conditions.get("forgecad.geometry.bevel@2", {}).get("inputs", {}).get("maxItems") == 1,
        "GeometryProgram@2 must bind panel and vent-array operator revisions to their exact parameter schemas",
    )
    definitions = geometry["$defs"]
    require(
        definitions.get("identifier", {}).get("pattern") == "^[A-Za-z0-9_.-]{1,128}$"
        and definitions.get("dimension_scalar", {}).get("maximum") == 10
        and definitions.get("radius_scalar", {}).get("maximum") == 5
        and definitions.get("coordinate_scalar", {}).get("minimum") == -10
        and definitions.get("coordinate_scalar", {}).get("maximum") == 10,
        "GeometryProgram@2 identifier and physical primitive bounds drifted from the bounded worker",
    )
    panel_v1_bevel = definitions["panel_parameters"]["properties"]["bevel_m"]
    require(
        panel_v1_bevel.get("minimum") == 0.0
        and panel_v1_bevel.get("maximum") == 5.0,
        "GeometryProgram@2 panel@1 bevel bounds drifted from the compatible Worker zero-bevel branch",
    )
    crease_parameters = definitions["subd_cage_crease_parameters"]
    require(
        crease_parameters.get("additionalProperties") is False
        and crease_parameters["properties"]["control_points"].get("$ref")
        == "#/$defs/subd_cage_crease_control_points"
        and definitions["subd_cage_crease_control_points"].get("minItems") == 9
        and definitions["subd_cage_crease_control_points"].get("maxItems") == 256
        and crease_parameters["properties"]["u_points"].get("minimum") == 3
        and crease_parameters["properties"]["v_points"].get("minimum") == 3
        and crease_parameters["properties"]["subdivision_levels"].get("minimum") == 1
        and crease_parameters["properties"]["subdivision_levels"].get("maximum") == 2
        and crease_parameters["properties"]["crease_edges"].get("maxItems") == 128,
        "GeometryProgram@2 crease cage must require a 3x3-or-larger bounded grid",
    )
    authoring_parameters = definitions["authoring_mesh_parameters"]
    require(
        authoring_parameters.get("additionalProperties") is False
        and authoring_parameters["properties"]["topology_policy"].get("const")
        == "triangle-quad-manifold-with-boundary@1"
        and authoring_parameters["properties"]["vertices"].get("maxItems") == 1536
        and authoring_parameters["properties"]["edges"].get("maxItems") == 1536
        and authoring_parameters["properties"]["loops"].get("maxItems") == 1536
        and authoring_parameters["properties"]["faces"].get("maxItems") == 512
        and definitions["authoring_mesh_face"]["properties"]["loop_ids"].get("minItems") == 3
        and definitions["authoring_mesh_face"]["properties"]["loop_ids"].get("maxItems") == 4,
        "GeometryProgram@2 authoring mesh must remain a bounded triangle/quad topology",
    )
    part_output = geometry["$defs"]["part_output"]
    part_output_fields = {
        "part_id",
        "input_node_ids",
        "material_zone_id",
        "solid",
    }
    require(
        part_output.get("type") == "object"
        and part_output.get("additionalProperties") is False
        and set(part_output.get("required", [])) == part_output_fields
        and set(part_output.get("properties", {})) == part_output_fields
        and "source_node_id" not in part_output.get("properties", {}),
        "GeometryProgram@2 part_output must be a closed semantic Part sink",
    )
    part_output_inputs = part_output["properties"]["input_node_ids"]
    require(
        part_output_inputs.get("type") == "array"
        and part_output_inputs.get("minItems") == 1
        and part_output_inputs.get("maxItems") == 512
        and part_output_inputs.get("uniqueItems") is True
        and part_output_inputs.get("items", {}).get("$ref") == "#/$defs/identifier",
        "GeometryProgram@2 Part sink inputs must be ordered, non-empty, unique node IDs",
    )

    hash_request = load_schema("geometry-program-hash-request.schema.json")
    require(
        hash_request.get("type") == "object"
        and hash_request.get("additionalProperties") is False
        and hash_request.get("properties", {}).get("schema_version", {}).get("const")
        == "GeometryProgramHashRequest@1",
        "GeometryProgramHashRequest@1 must be a closed request envelope",
    )
    require_required(
        hash_request,
        {"schema_version", "geometry_program_draft"},
        "GeometryProgramHashRequest@1",
    )
    draft = hash_request.get("properties", {}).get("geometry_program_draft", {})
    expected_draft_fields = {
        "schema_version",
        "project_id",
        "representation_plan_sha256",
        "operator_catalog_sha256",
        "units",
        "budgets",
        "nodes",
        "part_outputs",
    }
    require(
        draft.get("type") == "object"
        and draft.get("additionalProperties") is False
        and set(draft.get("required", [])) == expected_draft_fields
        and set(draft.get("properties", {})) == expected_draft_fields
        and "canonical_sha256" not in draft.get("properties", {}),
        "GeometryProgramHashRequest@1 must accept exactly a hash-free GeometryProgram@2 draft",
    )
    require(
        draft.get("properties", {}).get("schema_version", {}).get("const") == "GeometryProgram@2"
        and draft.get("properties", {}).get("nodes", {}).get("items", {}).get("$ref")
        == "https://forgecad.local/contracts/geometry-program-v2.schema.json#/$defs/geometry_node"
        and draft.get("properties", {}).get("part_outputs", {}).get("items", {}).get("$ref")
        == "https://forgecad.local/contracts/geometry-program-v2.schema.json#/$defs/part_output",
        "GeometryProgramHashRequest@1 draft must reuse the GeometryProgram@2 node and Part output definitions",
    )

    hash_result = load_schema("geometry-program-hash-result.schema.json")
    expected_hash_result_fields = {
        "schema_version",
        "geometry_program_schema_version",
        "canonical_sha256",
        "operator_catalog_sha256",
        "validation_status",
    }
    require(
        hash_result.get("type") == "object"
        and hash_result.get("additionalProperties") is False
        and set(hash_result.get("required", [])) == expected_hash_result_fields
        and set(hash_result.get("properties", {})) == expected_hash_result_fields
        and hash_result.get("properties", {}).get("schema_version", {}).get("const")
        == "GeometryProgramHashResult@1"
        and hash_result.get("properties", {}).get("geometry_program_schema_version", {}).get("const")
        == "GeometryProgram@2"
        and hash_result.get("properties", {}).get("validation_status", {}).get("const") == "passed",
        "GeometryProgramHashResult@1 must be a closed, passing V2 hash receipt",
    )
    require(
        hash_result.get("properties", {}).get("canonical_sha256", {}).get("$ref") == "#/$defs/sha256"
        and hash_result.get("properties", {}).get("operator_catalog_sha256", {}).get("$ref")
        == "#/$defs/sha256"
        and hash_result.get("$defs", {}).get("sha256", {}).get("pattern") == "^[0-9a-f]{64}$",
        "GeometryProgramHashResult@1 hashes must be lowercase SHA-256 values",
    )

    catalog = load_schema("operator-catalog.schema.json")
    require(
        catalog.get("properties", {}).get("schema_version", {}).get("const") == "OperatorCatalog@1",
        "OperatorCatalog@1 schema_version is not closed",
    )
    operator = catalog["$defs"]["operator"]
    catalog_entries = catalog.get("properties", {}).get("operators", {})
    require(
        catalog_entries.get("maxItems") == 32
        and operator.get("properties", {}).get("status", {}).get("enum") == ["active", "unavailable"],
        "OperatorCatalog@1 must expose a bounded active/unavailable operator catalog",
    )
    require(
        operator.get("properties", {}).get("operator_id", {}).get("$ref")
        == "#/$defs/identifier"
        and operator.get("properties", {}).get("supported_shapes", {}).get("maxItems") == 8,
        "OperatorCatalog@1 generic operator definition drifted",
    )

    readback = load_schema("artifact-readback-v2.schema.json")
    require(
        readback.get("properties", {}).get("schema_version", {}).get("const") == "ArtifactReadback@2",
        "ArtifactReadback@2 schema_version is not closed",
    )
    require_required(
        readback,
        {
            "program_sha256",
            "operator_catalog_sha256",
            "readback_config_sha256",
            "triangle_count",
            "part_ids",
            "source_node_ids",
            "material_zone_ids",
            "part_bindings",
            "validator_status",
            "hard_gate_passed",
            "integrity",
            "canonical_sha256",
        },
        "ArtifactReadback@2",
    )
    require_required(
        readback["$defs"]["part_binding"],
        {"part_id", "source_node_id", "material_zone_id", "solid", "triangle_count"},
        "ArtifactReadback@2 part_binding",
    )
    require(
        "input_node_ids" not in readback["$defs"]["part_binding"].get("properties", {}),
        "ArtifactReadback@2 must preserve one source binding per Part input",
    )
    integrity_required = {
        "glb_parse_status",
        "invalid_index_count",
        "non_finite_count",
        "degenerate_triangle_count",
        "boundary_edge_count",
        "non_manifold_edge_count",
        "winding_error_count",
        "uv_non_finite_count",
        "zero_area_uv_triangle_count",
        "tangent_non_finite_count",
        "tangent_orthogonality_error_count",
        "tangent_handedness_error_count",
        "metadata_mismatch_count",
        "external_uri_count",
        "part_coverage",
        "source_coverage",
        "material_zone_coverage",
    }
    require_required(readback["$defs"]["integrity"], integrity_required, "ArtifactReadback@2 integrity")
    passing_integrity = readback["$defs"]["passing_integrity"]["allOf"][1].get("properties", {})
    require(
        all(passing_integrity.get(name, {}).get("const") == 0 for name in integrity_required if name.endswith("_count"))
        and all(
            passing_integrity.get(name, {}).get("const") == 1
            for name in {"part_coverage", "source_coverage", "material_zone_coverage"}
        )
        and passing_integrity.get("glb_parse_status", {}).get("const") == "passed",
        "ArtifactReadback@2 passing integrity must represent actual zero-error readback",
    )

    result = load_schema("geometry-prepare-result-v2.schema.json")
    require(
        result.get("properties", {}).get("schema_version", {}).get("const") == "GeometryPrepareResult@2"
        and result.get("properties", {}).get("operator_catalog", {}).get("$ref")
        == "https://forgecad.local/contracts/operator-catalog.schema.json"
        and result.get("properties", {}).get("artifact", {}).get("$ref")
        == "https://forgecad.local/contracts/artifact-readback-v2.schema.json",
        "GeometryPrepareResult@2 must bind the V2 catalog and readback contracts",
    )

    quality = load_schema("geometry-quality-report-v2.schema.json")
    expected_quality_fields = {
        "schema_version",
        "scope",
        "quality_report_id",
        "candidate_id",
        "artifact_sha256",
        "program_sha256",
        "operator_catalog_sha256",
        "readback_config_sha256",
        "artifact_readback_object_sha256",
        "integrity",
        "hard_gate_passed",
        "canonical_sha256",
    }
    quality_properties = quality.get("properties", {})
    require(
        quality.get("type") == "object"
        and quality.get("additionalProperties") is False
        and set(quality.get("required", [])) == expected_quality_fields
        and set(quality_properties) == expected_quality_fields
        and quality_properties.get("schema_version", {}).get("const") == "GeometryQualityReport@2"
        and quality_properties.get("scope", {}).get("const")
        == "mcp010b-strict-glb-bin-accessor-hard-gates"
        and quality_properties.get("hard_gate_passed", {}).get("const") is True,
        "GeometryQualityReport@2 must be a closed, strict V2 hard-gate receipt",
    )
    require(
        all(
            quality_properties.get(name, {}).get("$ref") == "#/$defs/identifier"
            for name in {"quality_report_id", "candidate_id"}
        )
        and all(
            quality_properties.get(name, {}).get("$ref") == "#/$defs/sha256"
            for name in {
                "artifact_sha256",
                "program_sha256",
                "operator_catalog_sha256",
                "readback_config_sha256",
                "artifact_readback_object_sha256",
                "canonical_sha256",
            }
        )
        and quality.get("$defs", {}).get("identifier", {}).get("pattern")
        == "^[A-Za-z0-9_.-]{1,128}$"
        and quality.get("$defs", {}).get("sha256", {}).get("pattern") == "^[0-9a-f]{64}$",
        "GeometryQualityReport@2 identity and provenance hashes must be bounded",
    )
    require(
        quality_properties.get("integrity", {}).get("$ref")
        == "geometry-quality-report-v2.schema.json#/$defs/integrity"
        and quality.get("$defs", {}).get("integrity", {}).get("$ref")
        == "artifact-readback-v2.schema.json#/$defs/passing_integrity",
        "GeometryQualityReport@2 must require ArtifactReadback@2 passing integrity",
    )

    evidence = load_schema("geometry-candidate-evidence.schema.json")
    expected_evidence_fields = {
        "schema_version",
        "candidate_id",
        "project_id",
        "reference_id",
        "reference_sha256",
        "geometry_program_sha256",
        "geometry_program_object_sha256",
        "operator_catalog_sha256",
        "readback_config_sha256",
        "artifact_object_sha256",
        "artifact_readback_object_sha256",
        "quality_report_object_sha256",
        "quality_report_id",
        "canonical_sha256",
        "created_at",
    }
    evidence_properties = evidence.get("properties", {})
    require(
        evidence.get("type") == "object"
        and evidence.get("additionalProperties") is False
        and set(evidence.get("required", [])) == expected_evidence_fields
        and set(evidence_properties) == expected_evidence_fields
        and evidence_properties.get("schema_version", {}).get("const") == "GeometryCandidateEvidence@1",
        "GeometryCandidateEvidence@1 must be a closed candidate provenance record",
    )
    require(
        all(
            evidence_properties.get(name, {}).get("$ref") == "#/$defs/identifier"
            for name in {"candidate_id", "project_id", "quality_report_id"}
        )
        and all(
            evidence_properties.get(name, {}).get("$ref") == "#/$defs/sha256"
            for name in {
                "geometry_program_sha256",
                "geometry_program_object_sha256",
                "operator_catalog_sha256",
                "readback_config_sha256",
                "artifact_object_sha256",
                "artifact_readback_object_sha256",
                "quality_report_object_sha256",
                "canonical_sha256",
            }
        )
        and evidence.get("$defs", {}).get("identifier", {}).get("pattern")
        == "^[A-Za-z0-9_.-]{1,128}$"
        and evidence.get("$defs", {}).get("sha256", {}).get("pattern") == "^[0-9a-f]{64}$"
        and evidence_properties.get("created_at", {}).get("type") == "string"
        and evidence_properties.get("created_at", {}).get("minLength") == 1
        and evidence_properties.get("created_at", {}).get("maxLength") == 64,
        "GeometryCandidateEvidence@1 must bind bounded identities, hashes, and creation evidence",
    )
    require(
        set(evidence_properties.get("reference_id", {}).get("type", [])) == {"string", "null"}
        and evidence_properties.get("reference_id", {}).get("pattern")
        == "^[A-Za-z0-9_.-]{1,128}$"
        and set(evidence_properties.get("reference_sha256", {}).get("type", [])) == {"string", "null"}
        and evidence_properties.get("reference_sha256", {}).get("pattern") == "^[0-9a-f]{64}$",
        "GeometryCandidateEvidence@1 reference fields must be bounded nullable values",
    )
    reference_pairing = evidence.get("allOf", [])
    require(
        len(reference_pairing) == 1
        and reference_pairing[0].get("if", {}).get("properties", {}).get("reference_id", {}).get("const")
        is None
        and reference_pairing[0].get("if", {}).get("required") == ["reference_id"]
        and reference_pairing[0].get("then", {}).get("properties", {}).get("reference_sha256", {}).get("const")
        is None
        and reference_pairing[0].get("else", {}).get("properties", {}).get("reference_sha256", {}).get("$ref")
        == "#/$defs/sha256",
        "GeometryCandidateEvidence@1 must pair a missing reference ID with null hash and a bound ID with SHA-256",
    )

    appearance = load_schema("appearance-prepare-result.schema.json")
    require_required(
        appearance,
        {"render_set_object_sha256", "quality_report_object_sha256"},
        "AppearancePrepareResult@1 CAS receipts",
    )
    render_set = load_schema("render-set.schema.json")
    require_required(render_set, {"pass_artifacts"}, "RenderSet@1 pass artifacts")
    pass_artifacts = render_set["properties"]["pass_artifacts"]
    require(
        pass_artifacts.get("additionalProperties") is False
        and set(pass_artifacts.get("properties", {}))
        == {"beauty", "silhouette", "normal", "part-id", "material-id", "depth", "ao", "wireframe"},
        "RenderSet@1 pass artifacts must use a closed pass mapping",
    )

    skill_manifest = load_schema("skill-bundle-manifest.schema.json")
    execution_availability = skill_manifest.get("properties", {}).get("execution_availability", {})
    missing_operator_ids = skill_manifest.get("properties", {}).get("missing_operator_ids", {})
    require(
        execution_availability.get("enum") == ["active", "partial", "unavailable"],
        "SkillBundleManifest execution availability is not closed",
    )
    require(
        missing_operator_ids.get("uniqueItems") is True
        and missing_operator_ids.get("items", {}).get("pattern")
        == "^forgecad\\.[a-z0-9_.-]+@[0-9]+$",
        "SkillBundleManifest missing operator IDs are not bounded",
    )
    conditions = skill_manifest.get("allOf", [])
    require(
        len(conditions) == 3
        and conditions[0].get("then", {}).get("required") == ["missing_operator_ids"]
        and conditions[1].get("then", {}).get("properties", {}).get("missing_operator_ids", {}).get("maxItems") == 0
        and conditions[2].get("then", {}).get("properties", {}).get("missing_operator_ids", {}).get("minItems") == 1,
        "SkillBundleManifest execution availability must fail closed when an operator lock is incomplete",
    )


def check_mcp010c_contracts() -> None:
    """Keep the fixed-render/reference-review contracts closed and hash-bound."""
    expected = {
        "reference-view-spec.schema.json": "ReferenceViewSpec@1",
        "camera-calibration.schema.json": "CameraCalibration@1",
        "render-set-v2.schema.json": "RenderSet@2",
        "reference-comparison-report.schema.json": "ReferenceComparisonReport@1",
        "visual-review-report.schema.json": "VisualReviewReport@1",
        "human-visual-review-receipt.schema.json": "HumanVisualReviewReceipt@1",
        "quality-report-v2.schema.json": "QualityReport@2",
    }
    for filename, version in expected.items():
        schema = load_schema(filename)
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and schema.get("properties", {}).get("schema_version", {}).get("const") == version,
            f"{version} must be a closed object contract",
        )
        require_required(schema, {"schema_version", "canonical_sha256"}, version)

    camera = load_schema("camera-calibration.schema.json")
    require(
        camera["properties"]["projection"].get("const") == "perspective"
        and camera["properties"]["resolution"]["properties"]["width"].get("const") == 512
        and camera["properties"]["resolution"]["properties"]["height"].get("const") == 512
        and camera["properties"]["coordinate_system"].get("const") == "right-handed-y-up-meter",
        "CameraCalibration@1 must be a deterministic 512x512 perspective camera",
    )

    render = load_schema("render-set-v2.schema.json")
    render_passes = [
        "beauty",
        "silhouette",
        "depth",
        "normal",
        "ao",
        "part-id",
        "material-id",
        "wireframe",
        "uv-stretch",
    ]
    passes = render["properties"]["passes"]
    pass_prefix = passes.get("prefixItems", [])
    require(
        passes.get("minItems") == 9
        and passes.get("maxItems") == 9
        and passes.get("items") is False
        and [entry.get("const") for entry in pass_prefix] == render_passes
        and set(render["properties"]["pass_artifacts"].get("required", [])) == set(render_passes),
        "RenderSet@2 must require the nine fixed AOV passes in exact order",
    )
    render_fields = set(render.get("properties", {}))
    require(
        render_fields == set(render.get("required", [])) and "view_id" in render_fields,
        "RenderSet@2 schema fields must match the Runtime exact-key validator",
    )
    pass_artifact = render["$defs"]["pass_artifact"]["properties"]
    require(
        pass_artifact["mime"].get("const") == "image/png"
        and pass_artifact["width"].get("const") == 512
        and pass_artifact["height"].get("const") == 512,
        "RenderSet@2 pass artifacts must be 512x512 PNGs",
    )
    artifact_properties = render["properties"]["pass_artifacts"]["properties"]
    for pass_id in render_passes:
        overlays = artifact_properties[pass_id].get("allOf", [])
        expected_color_space = "srgb" if pass_id == "beauty" else "data"
        require(
            len(overlays) == 2
            and overlays[0].get("$ref") == "#/$defs/pass_artifact"
            and overlays[1]
            .get("properties", {})
            .get("color_space", {})
            .get("const")
            == expected_color_space,
            f"RenderSet@2 {pass_id} color_space must match Runtime semantics",
        )

    comparison = load_schema("reference-comparison-report.schema.json")
    require(
        set(comparison["properties"]["status"].get("enum", []))
        == {"PARTIAL_VISIBLE_VIEW_PASS", "QUALITY_TARGET_NOT_MET", "BLOCKED_REFERENCE_COVERAGE"}
        and set(comparison["properties"]["metrics"]["required"])
        == {
            "silhouette_iou",
            "boundary_f1_4px",
            "bbox_edge_error",
            "centroid_error",
            "landmark_coverage",
            "landmark_nme",
            "region_median_iou",
            "critical_region_min_iou",
        },
        "ReferenceComparisonReport@1 must expose the fixed metric set and explicit partial/blocked status",
    )
    require(
        comparison["properties"]["mask"]["properties"]["method"].get("enum")
        == ["local-border-flood-fill-morphology", "silhouette-target"],
        "ReferenceComparisonReport@1 must allow the deterministic local mask or an explicit silhouette target",
    )

    review = load_schema("visual-review-report.schema.json")
    require(
        review["properties"]["round"].get("maximum") == 5
        and set(review["properties"]["stage"].get("enum", []))
        == {"silhouette", "structure", "form", "material-surface", "final"},
        "VisualReviewReport@1 must bound review rounds to the five fixed stages",
    )
    human = load_schema("human-visual-review-receipt.schema.json")
    require(
        set(human["properties"]["scores"]["required"])
        == {"likeness", "geometry_detail", "material_fidelity", "editability"}
        and human["properties"]["scores"]["properties"]["likeness"].get("minimum") == 1
        and human["properties"]["scores"]["properties"]["likeness"].get("maximum") == 5,
        "HumanVisualReviewReceipt@1 must bind all four 1-5 user scores",
    )
    quality = load_schema("quality-report-v2.schema.json")
    require(
        quality["properties"]["visual_status"].get("enum")
        == [
            "PARTIAL_VISIBLE_VIEW_PASS",
            "QUALITY_TARGET_NOT_MET",
            "BLOCKED_REFERENCE_COVERAGE",
            "not-run",
        ]
        and quality["properties"]["hard_gate_passed"].get("type") == "boolean",
        "QualityReport@2 must distinguish visual status from structural hard gates",
    )
    require(
        set(
            [
                "threshold_revision",
                "threshold_policy_sha256",
                "threshold_source",
                "metric_gate_results",
            ]
        ).issubset(set(quality.get("required", [])))
        and quality["properties"]["threshold_revision"].get("const") == "visible-view-gates@1"
        and quality["properties"]["threshold_source"].get("const")
        == "forgecad-runtime-visible-view-gates"
        and quality["properties"]["threshold_policy_sha256"].get("$ref") == "#/$defs/sha256"
        and quality["properties"]["metric_gate_results"].get("maxItems") == 8,
        "QualityReport@2 must emit Runtime-owned threshold policy and metric gate results",
    )


def check_mcp010e_contracts() -> None:
    """Keep the offline material/texture authoring path closed and bounded."""
    expected = {
        "material-pack-manifest.schema.json": "MaterialPackManifest@1",
        "material-pack-manifest-v2.schema.json": "MaterialPackManifest@2",
        "material-definition.schema.json": "MaterialDefinition@1",
        "texture-set.schema.json": "TextureSet@1",
        "texture-set-v2.schema.json": "TextureSet@2",
        "texture-build-receipt.schema.json": "TextureBuildReceipt@1",
        "texture-build-receipt-v2.schema.json": "TextureBuildReceipt@2",
        "appearance-program-v2.schema.json": "AppearanceProgram@2",
        "appearance-prepare-result-v2.schema.json": "AppearancePrepareResult@2",
        "appearance-prepare-result-v3.schema.json": "AppearancePrepareResult@3",
    }
    versioned = {
        filename: version
        for filename, version in expected.items()
        if filename
        not in {"material-definition.schema.json", "texture-set.schema.json"}
    }
    for filename, version in expected.items():
        schema = load_schema(filename)
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and (
                filename not in {"material-definition.schema.json", "texture-set.schema.json"}
                or schema.get("title") == version.removesuffix("@1")
            ),
            f"{version} must be a closed object contract",
        )
        if filename in versioned:
            require_required(schema, {"schema_version"}, version)

    manifest = load_schema("material-pack-manifest.schema.json")
    require_required(
        manifest,
        {
            "schema_version",
            "pack_id",
            "version",
            "source_assets",
            "textures",
            "material_definitions",
            "texture_sets",
            "limits",
            "canonical_sha256",
        },
        "MaterialPackManifest@1",
    )
    limits = manifest["properties"]["limits"]["properties"]
    require(
        limits["max_texture_bytes"].get("const") == 67108864
        and limits["max_export_bytes"].get("const") == 134217728
        and limits["embedded_only"].get("const") is True
        and limits["external_uri"].get("const") is False,
        "MaterialPackManifest@1 must keep embedded texture and size limits closed",
    )
    manifest_v2 = load_schema("material-pack-manifest-v2.schema.json")
    require_required(
        manifest_v2,
        {
            "schema_version",
            "source_textures",
            "textures",
            "derived_outputs",
            "texture_recipe",
            "limits",
            "canonical_sha256",
        },
        "MaterialPackManifest@2",
    )
    require(
        manifest_v2["properties"]["texture_recipe"]["properties"]["resolution"].get("const")
        == 2048
        and manifest_v2["$defs"]["source_texture"]["properties"]["width"].get("const")
        == 512
        and manifest_v2["$defs"]["source_texture"]["properties"]["height"].get("const")
        == 512
        and manifest_v2["$defs"]["output_texture"]["properties"]["width"].get("const")
        == 2048
        and manifest_v2["$defs"]["output_texture"]["properties"]["height"].get("const")
        == 2048
        and manifest_v2["properties"]["limits"]["properties"]["embedded_only"].get("const")
        is True
        and manifest_v2["properties"]["limits"]["properties"]["external_uri"].get("const")
        is False,
        "MaterialPackManifest@2 must bind actual 2K runtime-derived embedded outputs",
    )
    receipt_v2 = load_schema("texture-build-receipt-v2.schema.json")
    require(
        receipt_v2["properties"]["source_inputs"]["items"].get("$ref")
        == "#/$defs/source_texture"
        and receipt_v2["properties"]["outputs"]["items"].get("$ref")
        == "#/$defs/output_texture"
        and receipt_v2["$defs"]["source_texture"]["properties"]["width"].get("const")
        == 512
        and receipt_v2["$defs"]["source_texture"]["properties"]["height"].get("const")
        == 512
        and receipt_v2["$defs"]["output_texture"]["properties"]["width"].get("const")
        == 2048
        and receipt_v2["$defs"]["output_texture"]["properties"]["height"].get("const")
        == 2048,
        "TextureBuildReceipt@2 must distinguish exact 512 inputs from exact 2048 outputs",
    )
    texture = load_schema("texture-set.schema.json")
    texture_fields = texture["$defs"]["texture"]["properties"]
    require(
        texture_fields["file"].get("pattern") == r"^textures/[A-Za-z0-9_.-]+\.png$"
        and texture_fields["normal_convention"].get("enum") == ["OpenGL+Y", None]
        and texture_fields["width"].get("maximum") == 2048,
        "TextureSet@1 must use embedded PNGs and explicit OpenGL normal convention",
    )
    texture_v2 = load_schema("texture-set-v2.schema.json")
    require_required(
        texture_v2,
        {
            "schema_version",
            "emissive_texture_id",
            "clearcoat_texture_id",
            "clearcoat_roughness_texture_id",
            "clearcoat_normal_texture_id",
            "derived_texture_receipt_sha256",
        },
        "TextureSet@2",
    )
    require(
        texture_v2["$defs"]["texture"]["properties"]["semantic"].get("enum")
        == [
            "baseColor",
            "normal",
            "roughness",
            "metallic",
            "ao",
            "emissive",
            "clearcoat",
            "clearcoatRoughness",
            "clearcoatNormal",
        ]
        and texture_v2["$defs"]["texture"]["properties"]["width"].get("maximum")
        == 2048,
        "TextureSet@2 must add bounded emissive and complete clearcoat channels",
    )
    appearance = load_schema("appearance-program-v2.schema.json")
    require_required(
        appearance,
        {
            "schema_version",
            "project_id",
            "geometry_program_sha256",
            "material_pack_id",
            "material_pack_manifest_sha256",
            "material_zones",
            "canonical_sha256",
        },
        "AppearanceProgram@2",
    )
    require(
        appearance["properties"]["material_pack_id"].get("enum")
        == [
            "forgecad-hard-surface-robot",
            "forgecad-fictional-energy-weapon",
            "forgecad-fictional-energy-weapon-2k",
        ]
        and appearance["properties"]["material_zones"]["items"].get("additionalProperties")
        is False,
        "AppearanceProgram@2 must bind the first-party pack and closed material zones",
    )
    uv_pbr_appearance = (
        ROOT
        / "packages"
        / "forgecad-skills"
        / "bundles"
        / "uv-pbr"
        / "0.2.0"
        / "schemas"
        / "appearance-program-v2.schema.json"
    )
    require(
        uv_pbr_appearance.is_file()
        and uv_pbr_appearance.read_bytes()
        == (SCHEMA_ROOT / "appearance-program-v2.schema.json").read_bytes(),
        "uv-pbr bundled AppearanceProgram@2 must remain byte-identical to the root contract",
    )
    pack_root = (
        ROOT
        / "packages"
        / "forgecad-assets"
        / "forgecad-fictional-energy-weapon-2k"
        / "1.0.0"
    )
    pack_manifest_path = pack_root / "manifest.json"
    required_pack_files = [
        pack_manifest_path,
        pack_root / "NOTICE",
        pack_root / "LICENSES" / "CC0-1.0.txt",
        pack_root / "provenance.json",
        pack_root / "sbom.spdx.json",
    ]
    require(
        all(path.is_file() for path in required_pack_files),
        "the first-party 2K MaterialPack must include manifest, NOTICE, license, provenance, and SPDX SBOM",
    )
    pack_manifest = json.loads(pack_manifest_path.read_text(encoding="utf-8"))
    require(
        pack_manifest.get("schema_version") == "MaterialPackManifest@2"
        and pack_manifest.get("pack_id") == "forgecad-fictional-energy-weapon-2k"
        and pack_manifest.get("canonical_sha256")
        == "88504cca9aa20393a1577fc9ae2bbb65d3ccb0a3ca21d61a4c72efa501214fb6"
        and pack_manifest.get("texture_recipe", {}).get("resolution") == 2048
        and len(pack_manifest.get("source_textures", [])) == 7
        and len(pack_manifest.get("textures", [])) == 7
        and len(pack_manifest.get("derived_outputs", [])) == 2
        and all(
            item.get("width") == 2048 and item.get("height") == 2048
            for item in [
                *pack_manifest.get("textures", []),
                *pack_manifest.get("derived_outputs", []),
            ]
        ),
        "the admitted 2K MaterialPack content must stay bound to its reviewed recipe, inventory, and canonical hash",
    )
    result = load_schema("appearance-prepare-result-v2.schema.json")
    require(
        result["properties"]["artifact"].get("$ref")
        == "https://forgecad.local/contracts/artifact-readback-v2.schema.json"
        and result["properties"]["render_set"].get("$ref")
        == "https://forgecad.local/contracts/render-set-v2.schema.json",
        "AppearancePrepareResult@2 must expose strict artifact and nine-pass render receipts",
    )
    result_v3 = load_schema("appearance-prepare-result-v3.schema.json")
    require(
        result_v3["properties"]["artifact"].get("$ref")
        == "https://forgecad.local/contracts/artifact-readback-v2.schema.json"
        and result_v3["properties"]["render_set"].get("$ref")
        == "https://forgecad.local/contracts/render-set-v2.schema.json"
        and "candidate_surface_bake_receipt_sha256" in result_v3.get("required", []),
        "AppearancePrepareResult@3 must expose strict artifact, render, and candidate surface bake receipts",
    )
    layer_stack = load_schema("material-layer-stack.schema.json")
    require_required(
        layer_stack,
        {
            "schema_version",
            "stack_id",
            "material_pack_id",
            "material_pack_manifest_sha256",
            "uv_source",
            "layers",
            "budget",
            "canonical_sha256",
        },
        "MaterialLayerStack@1",
    )
    require(
        layer_stack["properties"]["material_pack_id"].get("const")
        == "forgecad-fictional-energy-weapon-2k"
        and layer_stack["properties"]["uv_source"].get("const") == "TEXCOORD_0"
        and layer_stack["properties"]["layers"].get("minItems") == 3
        and layer_stack["properties"]["layers"].get("maxItems") == 3
        and [
            layer_stack["$defs"][name]["properties"]["kind"].get("const")
            for name in ("decal", "wear", "clearcoat")
        ]
        == ["decal", "wear", "clearcoat"],
        "MaterialLayerStack@1 must keep the decal/wear/clearcoat route closed and ordered",
    )
    appearance_v3 = load_schema("appearance-program-v3.schema.json")
    require_required(
        appearance_v3,
        {
            "schema_version",
            "project_id",
            "geometry_program_sha256",
            "material_pack_id",
            "material_pack_manifest_sha256",
            "material_zones",
            "material_layer_stack",
            "material_layer_stack_sha256",
            "canonical_sha256",
        },
        "AppearanceProgram@3",
    )
    require(
        appearance_v3["properties"]["material_pack_id"].get("const")
        == "forgecad-fictional-energy-weapon-2k"
        and appearance_v3["properties"]["material_layer_stack"].get("$ref")
        == "material-layer-stack.schema.json",
        "AppearanceProgram@3 must bind the exact 2K pack to one typed layer stack",
    )
    bake_receipt = load_schema("candidate-surface-bake-receipt.schema.json")
    require_required(
        bake_receipt,
        {
            "candidate_id",
            "candidate_canonical_sha256",
            "artifact_sha256",
            "artifact_readback_sha256",
            "geometry_program_sha256",
            "appearance_program_sha256",
            "material_pack_manifest_sha256",
            "input_texture_receipt_sha256",
            "uv_binding_sha256",
            "material_layer_stack_sha256",
            "bake_policy_sha256",
            "worker_algorithm_sha256",
            "worker_build_cohort_sha256",
            "outputs",
            "lineage_sha256",
            "hard_gate_passed",
            "canonical_sha256",
        },
        "CandidateSurfaceBakeReceipt@1",
    )
    require(
        bake_receipt["properties"]["external_uri"].get("const") is False
        and bake_receipt["properties"]["network_at_runtime"].get("const") is False
        and bake_receipt["$defs"]["output"]["properties"]["width"].get("const")
        == 2048
        and bake_receipt["$defs"]["output"]["properties"]["height"].get("const")
        == 2048,
        "CandidateSurfaceBakeReceipt@1 must bind physical embedded 2K outputs without network or URI",
    )


def check_mcp010f_silhouette_contracts() -> None:
    """Keep the contour-first target and diagnostic receipts closed."""
    expected = {
        "silhouette-target.schema.json": "SilhouetteTarget@1",
        "reference-mask-prepare-result.schema.json": "ReferenceMaskPrepareResult@1",
        "camera-fit-result.schema.json": "CameraFitResult@1",
        "camera-calibration-ref.schema.json": "CameraCalibrationRef@1",
        "boundary-error-result.schema.json": "BoundaryErrorResult@1",
        "silhouette-rig.schema.json": "SilhouetteRig@1",
        "silhouette-rig-hash-request.schema.json": "SilhouetteRigHashRequest@1",
        "silhouette-rig-hash-result.schema.json": "SilhouetteRigHashResult@1",
        "silhouette-fit-intent.schema.json": "SilhouetteFitIntent@1",
        "silhouette-fit-result.schema.json": "SilhouetteFitResult@1",
        "part-contour-fit-result.schema.json": "PartContourFitResult@1",
        "silhouette-candidate-compare-result.schema.json": "SilhouetteCandidateCompareResult@1",
    }
    for filename, version in expected.items():
        schema = load_schema(filename)
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and schema.get("properties", {}).get("schema_version", {}).get("const") == version,
            f"{version} must be a closed object contract",
        )
        required = {"schema_version"}
        if filename not in {"silhouette-rig-hash-request.schema.json"}:
            required.add("canonical_sha256")
        require_required(schema, required, version)

    target = load_schema("silhouette-target.schema.json")
    require(
        target["properties"]["coordinate_space"].get("const")
        == "normalized_reference_image"
        and target["properties"]["width"].get("const") == 512
        and target["properties"]["height"].get("const") == 512
        and target["properties"]["contour_points"].get("maxItems") == 512,
        "SilhouetteTarget@1 must use normalized 512x512 contour truth",
    )
    target_guards = target.get("allOf", [])
    target_unreviewed_guard = next(
        (
            guard
            for guard in target_guards
            if guard.get("if", {})
            .get("properties", {})
            .get("annotation_status", {})
            .get("const")
            == "unreviewed"
        ),
        None,
    )
    target_confirmed_guard = next(
        (
            guard
            for guard in target_guards
            if guard.get("if", {})
            .get("properties", {})
            .get("annotation_status", {})
            .get("const")
            == "user_confirmed"
        ),
        None,
    )
    target_unreviewed_then = (target_unreviewed_guard or {}).get("then", {})
    target_unreviewed_part = (
        target_unreviewed_then.get("properties", {})
        .get("parts", {})
        .get("items", {})
    )
    target_unreviewed_landmark = (
        target_unreviewed_then.get("properties", {})
        .get("landmarks", {})
        .get("items", {})
    )
    require(
        target_unreviewed_guard is not None
        and not target_unreviewed_guard.get("if", {}).get("required")
        and target_unreviewed_part.get("properties", {})
        .get("visibility", {})
        .get("const")
        == "unknown"
        and target_unreviewed_landmark.get("properties", {})
        .get("visibility", {})
        .get("const")
        == "unknown"
        and target_confirmed_guard is not None
        and set(target_confirmed_guard.get("if", {}).get("required", []))
        == {"annotation_status"}
        and target_confirmed_guard.get("then", {})
        .get("properties", {})
        .get("source", {})
        .get("const")
        == "user_refined",
        "SilhouetteTarget@1 annotation policy must bind source and visibility to review status",
    )

    def accepts_silhouette_target_annotation_fixture(
        annotation_status: str | None,
        source: str,
        parts: list[dict],
        landmarks: list[dict],
    ) -> bool:
        """Exercise the Runtime-equivalent target review policy."""
        if annotation_status not in {None, "unreviewed", "user_confirmed"}:
            return False
        if source not in {"automatic", "user_refined"}:
            return False
        if any(
            entry.get("visibility") not in {"observed", "inferred", "unknown"}
            for entry in [*parts, *landmarks]
        ):
            return False
        effective_status = annotation_status or "unreviewed"
        if effective_status == "unreviewed" and any(
            entry.get("visibility") != "unknown" for entry in [*parts, *landmarks]
        ):
            return False
        return not (
            effective_status == "user_confirmed" and source != "user_refined"
        )

    require(
        accepts_silhouette_target_annotation_fixture(
            "unreviewed",
            "automatic",
            [{"visibility": "unknown"}],
            [{"visibility": "unknown"}],
        ),
        "SilhouetteTarget@1 unreviewed unknown positive fixture must remain accepted",
    )
    require(
        accepts_silhouette_target_annotation_fixture(
            None,
            "automatic",
            [{"visibility": "unknown"}],
            [{"visibility": "unknown"}],
        ),
        "SilhouetteTarget@1 omitted annotation status must default to unreviewed",
    )
    require(
        not accepts_silhouette_target_annotation_fixture(
            "unreviewed",
            "automatic",
            [{"visibility": "observed"}],
            [{"visibility": "unknown"}],
        ),
        "SilhouetteTarget@1 unreviewed observed-part negative fixture must be rejected",
    )
    require(
        not accepts_silhouette_target_annotation_fixture(
            "unreviewed",
            "automatic",
            [{"visibility": "unknown"}],
            [{"visibility": "inferred"}],
        ),
        "SilhouetteTarget@1 unreviewed inferred-landmark negative fixture must be rejected",
    )
    for visibility in ("observed", "inferred", "unknown"):
        require(
            accepts_silhouette_target_annotation_fixture(
                "user_confirmed",
                "user_refined",
                [{"visibility": visibility}],
                [{"visibility": visibility}],
            ),
            f"SilhouetteTarget@1 user-confirmed {visibility} positive fixture must remain accepted",
        )
    require(
        not accepts_silhouette_target_annotation_fixture(
            "user_confirmed",
            "automatic",
            [{"visibility": "unknown"}],
            [{"visibility": "unknown"}],
        ),
        "SilhouetteTarget@1 user-confirmed automatic-source negative fixture must be rejected",
    )
    visual_structure = load_schema("reference-visual-structure.schema.json")
    region = visual_structure["$defs"]["region"]
    require(
        region.get("additionalProperties") is False
        and "mask_operation" not in region.get("required", [])
        and region["properties"]["mask_operation"].get("enum")
        == ["none", "subtract"],
        "ReferenceVisualStructure@1 negative-space operation must be optional, closed and bounded",
    )
    visual_structure_guards = visual_structure.get("allOf", [])
    unreviewed_guard = next(
        (
            guard
            for guard in visual_structure_guards
            if guard.get("if", {})
            .get("properties", {})
            .get("review_status", {})
            .get("const")
            == "unreviewed"
        ),
        None,
    )
    unreviewed_then = (unreviewed_guard or {}).get("then", {})
    unreviewed_region = (
        unreviewed_then.get("properties", {})
        .get("regions", {})
        .get("items", {})
    )
    unreviewed_line_flow = (
        unreviewed_then.get("properties", {})
        .get("line_flows", {})
        .get("items", {})
    )
    require(
        unreviewed_guard is not None
        and set(unreviewed_guard.get("if", {}).get("required", [])) == {"review_status"}
        and unreviewed_region.get("properties", {}).get("visibility", {}).get("const")
        == "unknown"
        and set(unreviewed_region.get("not", {}).get("required", [])) == {"mask_operation"}
        and unreviewed_region.get("not", {})
        .get("properties", {})
        .get("mask_operation", {})
        .get("const")
        == "subtract"
        and unreviewed_line_flow.get("properties", {})
        .get("visibility", {})
        .get("const")
        == "unknown",
        "ReferenceVisualStructure@1 unreviewed positive/negative fixtures must force unknown visibility and reject subtract",
    )
    subtract_guard = next(
        (
            guard
            for guard in visual_structure_guards
            if "contains"
            in guard.get("if", {})
            .get("properties", {})
            .get("regions", {})
        ),
        None,
    )
    subtract_if = (subtract_guard or {}).get("if", {})
    subtract_contains = (
        subtract_if.get("properties", {})
        .get("regions", {})
        .get("contains", {})
    )
    subtract_then = (subtract_guard or {}).get("then", {})
    subtract_region_if = (
        subtract_then.get("properties", {})
        .get("regions", {})
        .get("items", {})
        .get("if", {})
    )
    subtract_region_then = (
        subtract_then.get("properties", {})
        .get("regions", {})
        .get("items", {})
        .get("then", {})
    )
    subtract_region_properties = subtract_region_then.get("properties", {})
    require(
        subtract_guard is not None
        and set(subtract_contains.get("required", [])) == {"mask_operation"}
        and subtract_contains.get("properties", {})
        .get("mask_operation", {})
        .get("const")
        == "subtract"
        and subtract_then.get("properties", {})
        .get("review_status", {})
        .get("const")
        == "user_confirmed"
        and set(subtract_region_if.get("required", [])) == {"mask_operation"}
        and subtract_region_properties.get("visual_role", {}).get("const") == "open-frame"
        and subtract_region_properties.get("visibility", {}).get("const") == "observed"
        and subtract_region_properties.get("boundary_relationship", {}).get("const")
        == "enclosed",
        "ReferenceVisualStructure@1 subtract positive/negative fixtures must require user-confirmed observed enclosed open-frame regions",
    )

    def accepts_visual_structure_policy_fixture(
        review_status: str,
        regions: list[dict],
        line_flows: list[dict],
    ) -> bool:
        """Exercise the review/negative-space policy without a third-party validator."""
        if review_status == "unreviewed":
            if any(
                region.get("visibility") != "unknown"
                or region.get("mask_operation") == "subtract"
                for region in regions
            ):
                return False
            if any(line_flow.get("visibility") != "unknown" for line_flow in line_flows):
                return False
        for region in regions:
            if region.get("mask_operation") == "subtract" and (
                review_status != "user_confirmed"
                or region.get("visual_role") != "open-frame"
                or region.get("visibility") != "observed"
                or region.get("boundary_relationship") != "enclosed"
            ):
                return False
        return True

    require(
        accepts_visual_structure_policy_fixture(
            "unreviewed",
            [{"visibility": "unknown", "mask_operation": "none"}],
            [{"visibility": "unknown"}],
        ),
        "ReferenceVisualStructure@1 unreviewed positive fixture must remain accepted",
    )
    require(
        not accepts_visual_structure_policy_fixture(
            "unreviewed",
            [{"visibility": "observed", "mask_operation": "none"}],
            [],
        ),
        "ReferenceVisualStructure@1 unreviewed observed-region negative fixture must be rejected",
    )
    require(
        not accepts_visual_structure_policy_fixture(
            "unreviewed",
            [{
                "visibility": "unknown",
                "mask_operation": "subtract",
                "visual_role": "open-frame",
                "boundary_relationship": "enclosed",
            }],
            [],
        ),
        "ReferenceVisualStructure@1 unreviewed subtract negative fixture must be rejected",
    )
    require(
        accepts_visual_structure_policy_fixture(
            "user_confirmed",
            [{
                "visibility": "observed",
                "mask_operation": "subtract",
                "visual_role": "open-frame",
                "boundary_relationship": "enclosed",
            }],
            [],
        ),
        "ReferenceVisualStructure@1 user-confirmed subtract positive fixture must remain accepted",
    )
    require(
        not accepts_visual_structure_policy_fixture(
            "user_confirmed",
            [{
                "visibility": "inferred",
                "mask_operation": "subtract",
                "visual_role": "open-frame",
                "boundary_relationship": "enclosed",
            }],
            [],
        ),
        "ReferenceVisualStructure@1 inferred subtract negative fixture must be rejected",
    )
    reference_views = load_schema("reference-view-set-v2.schema.json")
    require(
        reference_views["properties"]["coordinate_policy"].get("const")
        == "normalized-source-image"
        and reference_views["properties"]["views"].get("maxItems") == 12
        and reference_views["$defs"]["view"]["properties"]["silhouette_status"].get("enum")
        == ["unavailable", "automatic-unreviewed", "user-confirmed"],
        "ReferenceViewSet@2 must preserve bounded crops and explicit silhouette review state",
    )
    neutral_graph = load_schema("neutral-structure-graph.schema.json")
    require(
        neutral_graph["properties"]["decomposition_policy"].get("const")
        == "visual-structure-not-functional-parts"
        and neutral_graph["properties"]["global_contour_authority"].get("const") is True
        and neutral_graph["$defs"]["region"]["properties"]["operation"].get("enum")
        == ["add", "subtract", "material-only", "guide"],
        "NeutralStructureGraph@1 must keep visual groups non-functional and preserve negative space",
    )
    camera_fit = load_schema("camera-fit-result.schema.json")
    require(
        camera_fit["properties"]["candidates"].get("maxItems") == 128
        and camera_fit["properties"]["status"].get("enum")
        == ["ready", "no_improvement", "unavailable"],
        "CameraFitResult@1 must remain bounded and typed",
    )
    boundary = load_schema("boundary-error-result.schema.json")
    require(
        boundary["properties"]["segments"].get("maxItems") == 64
        and len(boundary["$defs"]["point_px"].get("prefixItems", [])) == 2
        and boundary["properties"]["segments"]["items"]["properties"]["direction"].get("enum")
        == ["inward", "outward", "aligned"],
        "BoundaryErrorResult@1 must expose bounded signed pixel segments",
    )
    require(
        "sdf_chamfer_px" in boundary["properties"]["metrics"]["required"],
        "BoundaryErrorResult@1 must expose SDF/Chamfer loss",
    )
    rig = load_schema("silhouette-rig.schema.json")
    require(
        rig["properties"]["parameters"].get("maxItems") == 64
        and rig["additionalProperties"] is False,
        "SilhouetteRig@1 must be bounded and closed",
    )
    fit = load_schema("silhouette-fit-result.schema.json")
    require(
        fit["properties"]["evaluations"].get("maximum") == 64
        and fit["$defs"]["thresholds"]["properties"]["silhouette_iou"].get("const") == 0.9,
        "SilhouetteFitResult@1 must expose bounded optimizer evidence and strict thresholds",
    )
    require(
        set(
            [
                "baseline_camera",
                "baseline_metrics",
                "camera_evaluations",
                "baseline_loss",
                "selected_loss",
                "strict_improvement",
            ]
        ).issubset(set(fit.get("required", []))),
        "SilhouetteFitResult@1 must retain Runtime-owned baseline and strict-improvement evidence",
    )
    selected_geometry_program = fit["properties"].get("selected_geometry_program", {})
    require(
        "selected_geometry_program" in fit.get("required", [])
        and selected_geometry_program.get("oneOf", [{}])[0].get("type") == "null"
        and selected_geometry_program.get("oneOf", [{}, {}])[1].get("$ref")
        == "https://forgecad.local/contracts/geometry-program-v2.schema.json",
        "SilhouetteFitResult@1 must expose an optional Runtime-validated GeometryProgram proposal",
    )
    camera_ref = load_schema("camera-calibration-ref.schema.json")
    require(
        camera_ref.get("additionalProperties") is False
        and set(camera_ref.get("required", []))
        == {"schema_version", "camera_hash", "canonical_sha256"}
        and camera_ref["properties"]["schema_version"].get("const")
        == "CameraCalibrationRef@1",
        "CameraCalibrationRef@1 must contain only Runtime-owned camera hashes",
    )
    fit_intent = load_schema("silhouette-fit-intent.schema.json")
    fit_camera = fit_intent["properties"]["base_camera"]
    require(
        {item.get("$ref") for item in fit_camera.get("oneOf", [])}
        == {
            "https://forgecad.local/contracts/camera-calibration.schema.json",
            "https://forgecad.local/contracts/camera-calibration-ref.schema.json",
        },
        "SilhouetteFitIntent@1 must accept a full camera or a compact Runtime camera reference",
    )
    compare = load_schema("silhouette-candidate-compare-result.schema.json")
    require(
        compare["properties"]["candidates"].get("minItems") == 2
        and compare["properties"]["candidates"].get("maxItems") == 8,
        "SilhouetteCandidateCompareResult@1 must compare a bounded candidate set",
    )


def check_modifier_stack_contracts() -> None:
    request = load_schema("geometry-modifier-stack-request.schema.json")
    modifier = request["properties"]["modifiers"]["items"]
    modifier_refs = [item.get("$ref") for item in modifier.get("oneOf", [])]
    base_node = request["properties"]["base_node"]
    base_refs = [item.get("$ref") for item in base_node.get("oneOf", [])]
    require(
        request.get("additionalProperties") is False
        and request["properties"]["modifiers"].get("minItems") == 1
        and request["properties"]["modifiers"].get("maxItems") == 8
        and modifier_refs
        == [
            "#/$defs/transform_modifier",
            "#/$defs/mirror_modifier",
            "#/$defs/array_modifier",
            "#/$defs/bevel_modifier",
            "#/$defs/bevel_v2_modifier",
            "#/$defs/normal_policy_modifier",
        ],
        "GeometryModifierStackRequest@1 must remain closed, bounded and unary-only",
    )
    require(
        base_refs
        == [
            "#/$defs/primitive_base_node",
            "#/$defs/profile_extrude_base_node",
            "#/$defs/profile_loft_base_node",
            "#/$defs/longitudinal_section_loft_base_node",
            "#/$defs/subd_cage_base_node",
            "#/$defs/authoring_mesh_base_node",
            "#/$defs/surface_patch_base_node",
            "#/$defs/revolve_base_node",
            "#/$defs/tube_sweep_base_node",
            "#/$defs/panel_base_node",
            "#/$defs/vent_array_base_node",
            "#/$defs/vent_array_v2_base_node",
            "#/$defs/recessed_channel_base_node",
            "#/$defs/energy_core_base_node",
            "#/$defs/joint_stack_base_node",
        ]
        and all(
            request["$defs"][definition].get("$ref") == "#/$defs/base_node_common"
            for definition in [
                "primitive_base_node",
                "profile_extrude_base_node",
                "profile_loft_base_node",
                "longitudinal_section_loft_base_node",
                "subd_cage_base_node",
                "authoring_mesh_base_node",
                "surface_patch_base_node",
                "revolve_base_node",
                "tube_sweep_base_node",
                "panel_base_node",
                "vent_array_base_node",
                "vent_array_v2_base_node",
                "recessed_channel_base_node",
                "energy_core_base_node",
                "joint_stack_base_node",
            ]
        )
        and request["$defs"]["primitive_base_node"]["properties"]["operator_id"].get("const")
        == "forgecad.geometry.primitive@2"
        and request["$defs"]["primitive_base_node"]["properties"]["parameters"].get("$ref")
        == "#/$defs/primitive_base_parameters"
        and request["$defs"]["authoring_mesh_base_node"]["properties"]["operator_id"].get("const")
        == "forgecad.geometry.authoring-mesh@1"
        and request["$defs"]["authoring_mesh_base_node"]["properties"]["parameters"].get("$ref")
        == "https://forgecad.local/contracts/geometry-program-v2.schema.json#/$defs/authoring_mesh_parameters"
        and [
            item.get("$ref")
            for item in request["$defs"]["primitive_base_parameters"].get("oneOf", [])
        ]
        == [
            "https://forgecad.local/contracts/geometry-program-v2.schema.json#/$defs/box_parameters",
            "https://forgecad.local/contracts/geometry-program-v2.schema.json#/$defs/cylinder_parameters",
            "https://forgecad.local/contracts/geometry-program-v2.schema.json#/$defs/ellipsoid_parameters",
            "https://forgecad.local/contracts/geometry-program-v2.schema.json#/$defs/sphere_parameters",
        ]
        and request["$defs"]["panel_base_node"]["properties"]["operator_id"].get("const")
        == "forgecad.geometry.panel@1"
        and request["$defs"]["panel_base_node"]["properties"]["parameters"].get("$ref")
        == "https://forgecad.local/contracts/geometry-program-v2.schema.json#/$defs/panel_parameters"
        and request["$defs"]["vent_array_base_node"]["properties"]["operator_id"].get("const")
        == "forgecad.geometry.vent-array@1"
        and request["$defs"]["vent_array_base_node"]["properties"]["parameters"].get("$ref")
        == "https://forgecad.local/contracts/geometry-program-v2.schema.json#/$defs/vent_array_parameters"
        and request["$defs"]["vent_array_v2_base_node"]["properties"]["operator_id"].get("const")
        == "forgecad.geometry.vent-array@2"
        and request["$defs"]["vent_array_v2_base_node"]["properties"]["parameters"].get("$ref")
        == "https://forgecad.local/contracts/geometry-program-v2.schema.json#/$defs/vent_array_v2_parameters"
        and request["$defs"]["recessed_channel_base_node"]["properties"]["operator_id"].get("const")
        == "forgecad.geometry.recessed-channel@1"
        and request["$defs"]["recessed_channel_base_node"]["properties"]["parameters"].get("$ref")
        == "https://forgecad.local/contracts/geometry-program-v2.schema.json#/$defs/recessed_channel_parameters"
        and request["$defs"]["energy_core_base_node"]["properties"]["operator_id"].get("const")
        == "forgecad.geometry.energy-core@1"
        and request["$defs"]["energy_core_base_node"]["properties"]["parameters"].get("$ref")
        == "https://forgecad.local/contracts/geometry-program-v2.schema.json#/$defs/energy_core_parameters"
        and request["$defs"]["transform_modifier"]["properties"]["parameters"].get("$ref")
        == "https://forgecad.local/contracts/geometry-program-v2.schema.json#/$defs/transform_parameters"
        and request["$defs"]["mirror_modifier"]["properties"]["parameters"].get("$ref")
        == "https://forgecad.local/contracts/geometry-program-v2.schema.json#/$defs/mirror_parameters"
        and request["$defs"]["array_modifier"]["properties"]["parameters"].get("$ref")
        == "https://forgecad.local/contracts/geometry-program-v2.schema.json#/$defs/array_parameters"
        and request["$defs"]["bevel_modifier"]["properties"]["parameters"].get("$ref")
        == "https://forgecad.local/contracts/geometry-program-v2.schema.json#/$defs/bevel_parameters"
        and request["$defs"]["bevel_v2_modifier"]["properties"]["parameters"].get("$ref")
        == "https://forgecad.local/contracts/geometry-program-v2.schema.json#/$defs/bevel_v2_parameters"
        and request["$defs"]["normal_policy_modifier"]["properties"]["parameters"].get("$ref")
        == "https://forgecad.local/contracts/geometry-program-v2.schema.json#/$defs/normal_policy_parameters",
        "GeometryModifierStackRequest@1 must bind base and modifier parameters to GeometryProgram@2",
    )
    program = load_schema("geometry-modifier-stack-program.schema.json")
    stage = program["properties"]["evaluation_stages"]["items"]
    require(
        program.get("additionalProperties") is False
        and program["properties"]["geometry_program"].get("$ref")
        == "https://forgecad.local/contracts/geometry-program-v2.schema.json"
        and program["properties"]["quality_status"].get("const") == "structural_only"
        and stage.get("additionalProperties") is False
        and set(stage["properties"]["operator_id"].get("enum", []))
        == {
            "forgecad.geometry.transform@2",
            "forgecad.geometry.mirror@1",
            "forgecad.geometry.array@1",
            "forgecad.geometry.bevel@1",
            "forgecad.geometry.bevel@2",
            "forgecad.geometry.normal-policy@1",
        }
        and {
            "modifier_id",
            "enabled",
            "effective_node_id",
            "input_evaluation_sha256",
            "output_evaluation_sha256",
        }.issubset(set(stage.get("required", []))),
        "GeometryModifierStackProgram@1 must preserve ordered structural evaluation evidence",
    )

    evaluation_request = load_schema("geometry-modifier-evaluation-request.schema.json")
    expected_evaluation_request_fields = {
        "schema_version",
        "project_id",
        "representation_plan_sha256",
        "part_id",
        "material_zone_id",
        "solid",
        "base_node",
        "modifiers",
        "previous_evaluation",
        "input_sha256",
    }
    previous_variants = evaluation_request["properties"]["previous_evaluation"].get(
        "oneOf", []
    )


    require(
        evaluation_request.get("additionalProperties") is False
        and set(evaluation_request.get("required", []))
        == expected_evaluation_request_fields
        and set(evaluation_request.get("properties", {}))
        == expected_evaluation_request_fields
        and evaluation_request["properties"]["schema_version"].get("const")
        == "GeometryModifierEvaluationRequest@2"
        and evaluation_request["properties"]["base_node"].get("$ref")
        == "https://forgecad.local/contracts/geometry-modifier-stack-request.schema.json#/properties/base_node"
        and evaluation_request["properties"]["modifiers"].get("$ref")
        == "https://forgecad.local/contracts/geometry-modifier-stack-request.schema.json#/properties/modifiers"
        and {variant.get("type") for variant in previous_variants} == {None, "null"}
        and {
            variant.get("$ref")
            for variant in previous_variants
            if variant.get("$ref")
        }
        == {
            "https://forgecad.local/contracts/geometry-modifier-evaluation-signature.schema.json"
        },
        "GeometryModifierEvaluationRequest@2 must be closed and accept only null or one canonical previous signature",
    )

    signature = load_schema("geometry-modifier-evaluation-signature.schema.json")
    signature_stage = signature["properties"]["stages"]["items"]
    expected_signature_fields = {
        "schema_version",
        "project_id",
        "representation_plan_sha256",
        "part_id",
        "material_zone_id",
        "solid",
        "source_input_sha256",
        "stack_definition_sha256",
        "evaluation_sha256",
        "output_sha256",
        "evaluation_policy_sha256",
        "operator_catalog_sha256",
        "catalog_cohort_sha256",
        "cache_key_sha256",
        "stages",
        "canonical_sha256",
    }
    signature_hashes = {
        "source_input_sha256",
        "stack_definition_sha256",
        "evaluation_sha256",
        "output_sha256",
        "evaluation_policy_sha256",
        "operator_catalog_sha256",
        "catalog_cohort_sha256",
        "cache_key_sha256",
        "canonical_sha256",
    }
    require(
        signature.get("additionalProperties") is False
        and set(signature.get("required", [])) == expected_signature_fields
        and set(signature.get("properties", {})) == expected_signature_fields
        and signature["properties"]["schema_version"].get("const")
        == "GeometryModifierEvaluationSignature@1"
        and signature_hashes.issubset(set(signature.get("required", [])))
        and signature["properties"]["stages"].get("minItems") == 1
        and signature["properties"]["stages"].get("maxItems") == 8
        and signature_stage.get("$ref") == "#/$defs/stage_signature"
        and signature["$defs"]["stage_signature"].get("additionalProperties") is False
        and {
            "parameters_sha256",
            "definition_sha256",
            "input_evaluation_sha256",
            "output_evaluation_sha256",
            "stage_cache_key_sha256",
        }.issubset(set(signature["$defs"]["stage_signature"].get("required", []))),
        "GeometryModifierEvaluationSignature@1 must bind the closed ordered hash chain and cache identity",
    )

    evaluation_result = load_schema("geometry-modifier-evaluation-result.schema.json")
    result_stage = evaluation_result["properties"]["evaluation_stages"]["items"]
    expected_evaluation_result_fields = {
        "schema_version",
        "project_id",
        "representation_plan_sha256",
        "part_id",
        "material_zone_id",
        "solid",
        "input_sha256",
        "previous_evaluation_sha256",
        "source_input_sha256",
        "stack_definition_sha256",
        "evaluation_sha256",
        "output_sha256",
        "evaluation_policy_sha256",
        "operator_catalog_sha256",
        "catalog_cohort_sha256",
        "cache_key_sha256",
        "reuse_kind",
        "output_kind",
        "cache_decision",
        "evaluation_dirty",
        "dirty_reasons",
        "evaluation_signature",
        "geometry_program",
        "evaluation_stages",
        "validator_status",
        "quality_status",
        "limitations",
        "canonical_sha256",
    }
    require(
        evaluation_result.get("additionalProperties") is False
        and set(evaluation_result.get("required", []))
        == expected_evaluation_result_fields
        and set(evaluation_result.get("properties", {}))
        == expected_evaluation_result_fields
        and evaluation_result["properties"]["schema_version"].get("const")
        == "GeometryModifierEvaluationResult@2"
        and evaluation_result["properties"]["evaluation_signature"].get("$ref")
        == "https://forgecad.local/contracts/geometry-modifier-evaluation-signature.schema.json"
        and evaluation_result["properties"]["geometry_program"].get("$ref")
        == "https://forgecad.local/contracts/geometry-program-v2.schema.json"
        and evaluation_result["properties"]["quality_status"].get("const")
        == "structural_only"
        and evaluation_result["properties"]["reuse_kind"].get("const")
        == "semantic-signature-only"
        and evaluation_result["properties"]["output_kind"].get("const")
        == "geometry-program-canonical-sha256"
        and {"reuse_kind", "output_kind"}.issubset(
            set(evaluation_result.get("required", []))
        )
        and set(evaluation_result["properties"]["cache_decision"].get("enum", []))
        == {"initial-miss", "reusable", "invalidated"}
        and "clean"
        in evaluation_result["$defs"]["dirty_reason"].get("enum", [])
        and "disabled-modifier-definition-changed"
        in evaluation_result["$defs"]["dirty_reason"].get("enum", [])
        and result_stage.get("$ref") == "#/$defs/evaluation_stage"
        and evaluation_result["$defs"]["evaluation_stage"].get(
            "additionalProperties"
        )
        is False
        and "effective_node_id"
        in evaluation_result["$defs"]["evaluation_stage"].get("required", []),
        "GeometryModifierEvaluationResult@2 must expose only structural evaluation, dirty and deterministic reuse evidence",
    )


def check_modifier_apply_contracts() -> None:
    request = load_schema("geometry-modifier-apply-request.schema.json")
    request_fields = {
        "schema_version",
        "project_id",
        "source_candidate_id",
        "source_candidate_canonical_sha256",
        "source_artifact_sha256",
        "source_artifact_readback_sha256",
        "source_geometry_program_sha256",
        "source_operator_catalog_sha256",
        "source_readback_config_sha256",
        "source_part_id",
        "base_version_id",
        "modifiers",
        "idempotency_key",
        "max_response_bytes",
        "input_sha256",
    }
    modifier_items = request["properties"]["modifiers"].get("items", {})
    require(
        request.get("type") == "object"
        and request.get("additionalProperties") is False
        and set(request.get("required", [])) == request_fields
        and set(request.get("properties", {})) == request_fields
        and request["properties"]["schema_version"].get("const")
        == "GeometryModifierApplyRequest@1"
        and modifier_items.get("oneOf")
        and len(modifier_items["oneOf"]) == 5
        and request["properties"]["modifiers"].get("minItems") == 1
        and request["properties"]["modifiers"].get("maxItems") == 8
        and request["properties"]["max_response_bytes"].get("const") == 1048576,
        "GeometryModifierApplyRequest@1 must be closed, public-flow candidate-bound and 1 MiB bounded",
    )

    result = load_schema("geometry-modifier-apply-result.schema.json")
    result_fields = {
        "schema_version",
        "project_id",
        "source_candidate_id",
        "source_candidate_canonical_sha256",
        "new_candidate_id",
        "base_version_id",
        "source_artifact_sha256",
        "source_artifact_readback_sha256",
        "source_geometry_candidate_evidence_sha256",
        "source_geometry_program_sha256",
        "source_operator_catalog_sha256",
        "source_readback_config_sha256",
        "source_part_id",
        "source_terminal_node_id",
        "derived_artifact_sha256",
        "derived_artifact_readback_sha256",
        "derived_geometry_candidate_evidence_sha256",
        "derived_geometry_program_sha256",
        "derived_program_object_sha256",
        "derived_terminal_node_id",
        "preserved_part_ids",
        "modifier_apply_request_sha256",
        "modifier_evaluation_canonical_sha256",
        "modifier_stack_definition_sha256",
        "modifier_evaluation_sha256",
        "modifier_output_sha256",
        "source_worker_build_cohort_sha256",
        "derived_worker_build_cohort_sha256",
        "materialization_status",
        "runtime_write_performed",
        "quality_status",
        "limitations",
        "canonical_sha256",
    }
    require(
        result.get("type") == "object"
        and result.get("additionalProperties") is False
        and set(result.get("required", [])) == result_fields
        and set(result.get("properties", {})) == result_fields
        and result["properties"]["schema_version"].get("const")
        == "GeometryModifierApplyResult@1"
        and result["properties"]["materialization_status"].get("const")
        == "runtime-owned-immutable-cas-sidecar"
        and result["properties"]["runtime_write_performed"].get("const") is True
        and result["properties"]["quality_status"].get("const") == "structural_only"
        and result["properties"]["preserved_part_ids"].get("uniqueItems") is True
        and result["properties"]["preserved_part_ids"].get("maxItems") == 512,
        "GeometryModifierApplyResult@1 must be a closed immutable single-Part lineage sidecar",
    )

    request_v2 = load_schema("geometry-modifier-apply-request-v2.schema.json")
    request_v2_fields = {
        "schema_version",
        "project_id",
        "source_candidate_id",
        "source_candidate_canonical_sha256",
        "source_artifact_sha256",
        "source_artifact_readback_sha256",
        "source_geometry_program_sha256",
        "source_operator_catalog_sha256",
        "source_readback_config_sha256",
        "source_part_id",
        "source_terminal_node_id",
        "source_authoring_topology_sha256",
        "source_edge_id",
        "bevel_m",
        "segments",
        "profile",
        "clamp_overlap",
        "base_version_id",
        "idempotency_key",
        "max_response_bytes",
        "input_sha256",
    }
    request_v2_properties = request_v2.get("properties", {})
    require(
        request_v2.get("type") == "object"
        and request_v2.get("additionalProperties") is False
        and set(request_v2.get("required", [])) == request_v2_fields
        and set(request_v2_properties) == request_v2_fields
        and request_v2_properties["schema_version"].get("const")
        == "GeometryModifierApplyRequest@2"
        and request_v2_properties["bevel_m"].get("type") == "number"
        and request_v2_properties["bevel_m"].get("exclusiveMinimum") == 0.0
        and request_v2_properties["bevel_m"].get("maximum") == 0.25
        and request_v2_properties["segments"].get("type") == "integer"
        and request_v2_properties["segments"].get("minimum") == 1
        and request_v2_properties["segments"].get("maximum") == 4
        and request_v2_properties["profile"].get("type") == "number"
        and request_v2_properties["profile"].get("minimum") == 0.25
        and request_v2_properties["profile"].get("maximum") == 0.75
        and request_v2_properties["clamp_overlap"].get("type") == "boolean"
        and request_v2_properties["max_response_bytes"].get("const") == 1048576
        and request_v2_properties["source_authoring_topology_sha256"].get("$ref")
        == "#/$defs/sha256"
        and request_v2_properties["source_edge_id"].get("$ref")
        == "#/$defs/identifier",
        "GeometryModifierApplyRequest@2 must be a closed, exact-field, edge-bound 1 MiB request",
    )

    result_v2 = load_schema("geometry-modifier-apply-result-v2.schema.json")
    result_v2_fields = result_fields | {
        "source_authoring_topology_sha256",
        "source_edge_id",
        "bevel_parameters_sha256",
        "non_target_part_bindings_sha256",
    }
    result_v2_properties = result_v2.get("properties", {})
    require(
        result_v2.get("type") == "object"
        and result_v2.get("additionalProperties") is False
        and set(result_v2.get("required", [])) == result_v2_fields
        and set(result_v2_properties) == result_v2_fields
        and result_v2_properties["schema_version"].get("const")
        == "GeometryModifierApplyResult@2"
        and result_v2_properties["source_authoring_topology_sha256"].get("$ref")
        == "#/$defs/sha256"
        and result_v2_properties["source_edge_id"].get("$ref")
        == "#/$defs/identifier"
        and result_v2_properties["bevel_parameters_sha256"].get("$ref")
        == "#/$defs/sha256"
        and result_v2_properties["non_target_part_bindings_sha256"].get("$ref")
        == "#/$defs/sha256"
        and result_v2_properties["materialization_status"].get("const")
        == "runtime-owned-immutable-cas-sidecar"
        and result_v2_properties["runtime_write_performed"].get("const") is True
        and result_v2_properties["quality_status"].get("const") == "structural_only"
        and result_v2_properties["preserved_part_ids"].get("uniqueItems") is True
        and result_v2_properties["preserved_part_ids"].get("maxItems") == 512,
        "GeometryModifierApplyResult@2 must be a closed sidecar with explicit edge, bevel and non-target bindings",
    )


def check_parametric_group_contracts() -> None:
    request = load_schema("parametric-design-kit-request-v2.schema.json")
    expected_fields = {
        "schema_version", "project_id", "representation_plan_sha256", "template_id",
        "instance_id", "part_id", "material_zone_id", "parameters", "input_sha256",
    }
    template_ids = {
        "forgecad.group.rounded-box@1",
        "forgecad.group.mirrored-box@1",
        "forgecad.group.arrayed-cylinder@1",
    }
    parameter_refs = [
        item.get("$ref") for item in request["properties"]["parameters"].get("oneOf", [])
    ]
    require(
        request.get("additionalProperties") is False
        and set(request.get("required", [])) == expected_fields
        and set(request.get("properties", {})) == expected_fields
        and request["properties"]["schema_version"].get("const") == "ParametricDesignKitRequest@2"
        and set(request["properties"]["template_id"].get("enum", [])) == template_ids
        and parameter_refs == [
            "#/$defs/rounded_box_parameters",
            "#/$defs/mirrored_box_parameters",
            "#/$defs/arrayed_cylinder_parameters",
        ]
        and len(request.get("allOf", [])) == 3
        and all(
            branch.get("then", {}).get("properties", {}).get("parameters", {}).get("$ref")
            in parameter_refs
            for branch in request.get("allOf", [])
        )
        and all(
            request["$defs"][name].get("additionalProperties") is False
            for name in ["rounded_box_parameters", "mirrored_box_parameters", "arrayed_cylinder_parameters"]
        ),
        "ParametricDesignKitRequest@2 must pair a fixed template enum with closed typed sockets",
    )
    forbidden = {"script", "python", "expression", "url", "path", "env", "secret", "network"}
    for definition in [
        request["$defs"]["rounded_box_parameters"],
        request["$defs"]["mirrored_box_parameters"],
        request["$defs"]["arrayed_cylinder_parameters"],
    ]:
        require(
            forbidden.isdisjoint(definition.get("properties", {})),
            "ParametricDesignKitRequest@2 exposes a dynamic extension field",
        )

    program = load_schema("parametric-design-kit-program-v2.schema.json")
    definition = program["properties"]["template_definition"]
    require(
        program.get("additionalProperties") is False
        and program["properties"]["schema_version"].get("const") == "ParametricDesignKitProgram@2"
        and set(program["properties"]["template_id"].get("enum", [])) == template_ids
        and definition.get("additionalProperties") is False
        and definition["properties"]["nested_group_depth"].get("const") == 0
        and definition["properties"]["max_nodes"].get("const") == 3
        and definition["properties"]["lowering_sha256"].get("$ref") == "#/$defs/sha256"
        and "lowering_sha256" in definition.get("required", [])
        and definition["properties"]["interface"]["properties"]["field_mode"].get("const") == "single-value-only"
        and program["properties"]["geometry_program"].get("$ref")
        == "https://forgecad.local/contracts/geometry-program-v2.schema.json"
        and program["properties"]["evaluation_order"].get("minItems") == 3
        and program["properties"]["evaluation_order"].get("maxItems") == 3
        and program["properties"]["source_map"].get("minItems") == 3
        and program["properties"]["source_map"].get("maxItems") == 3
        and program["properties"]["quality_status"].get("const") == "structural_only"
        and program["properties"]["runtime_write_performed"].get("const") is False
        and program["properties"]["candidate_created"].get("const") is False,
        "ParametricDesignKitProgram@2 must remain fixed-template, non-persistent and structural-only",
    )


def check_topology_snapshot_contracts() -> None:
    request = load_schema("topology-snapshot-request.schema.json")
    expected_request = {
        "schema_version",
        "project_id",
        "artifact_id",
        "candidate_id",
        "part_id",
        "artifact_readback_sha256",
        "program_sha256",
        "operator_catalog_sha256",
        "readback_config_sha256",
        "snapshot_policy_sha256",
        "max_face_count",
    }
    require(
        request.get("additionalProperties") is False
        and set(request.get("required", [])) == expected_request
        and set(request.get("properties", {})) == expected_request
        and request["properties"]["schema_version"].get("const")
        == "TopologySnapshotRequest@1"
        and request["properties"]["max_face_count"].get("maximum") == 512,
        "TopologySnapshotRequest@1 must be closed, fully hash-bound and capped at 512 faces",
    )
    snapshot = load_schema("topology-snapshot.schema.json")
    properties = snapshot.get("properties", {})
    require(
        snapshot.get("additionalProperties") is False
        and properties.get("schema_version", {}).get("const") == "TopologySnapshot@1"
        and properties.get("scope", {}).get("const") == "part"
        and properties.get("complete", {}).get("const") is True
        and properties.get("topology_space", {}).get("const")
        == "evaluated-glb-triangle-mesh@1"
        and properties.get("id_scope", {}).get("const") == "artifact-bound"
        and properties.get("cross_version_stable", {}).get("const") is False
        and properties.get("quality_status", {}).get("const") == "structural_only",
        "TopologySnapshot@1 must describe complete evaluated artifact-local structural truth only",
    )
    require(
        properties.get("faces", {}).get("maxItems") == 512
        and properties.get("vertices", {}).get("maxItems") == 1536
        and properties.get("edges", {}).get("maxItems") == 1536
        and properties.get("corners", {}).get("maxItems") == 1536
        and properties.get("max_response_bytes", {}).get("const") == 1048576,
        "TopologySnapshot@1 element and response budgets drifted",
    )
    corner = snapshot["$defs"]["corner"]
    require_required(
        corner,
        {
            "corner_id",
            "face_id",
            "ordinal",
            "vertex_id",
            "edge_id",
            "edge_forward",
            "position_m",
            "normal",
            "texcoord_0",
            "tangent",
        },
        "TopologySnapshot@1 corner",
    )


def check_authoring_topology_contracts() -> None:
    request = load_schema("authoring-topology-request.schema.json")
    request_fields = {
        "schema_version",
        "project_id",
        "candidate_id",
        "artifact_id",
        "artifact_readback_sha256",
        "program_sha256",
        "operator_catalog_sha256",
        "readback_config_sha256",
        "authoring_node_id",
        "part_id",
        "authoring_topology_policy_sha256",
        "max_response_bytes",
    }
    require(
        request.get("additionalProperties") is False
        and set(request.get("required", [])) == request_fields
        and set(request.get("properties", {})) == request_fields
        and request["properties"]["schema_version"].get("const")
        == "AuthoringTopologyRequest@1"
        and request["properties"]["authoring_topology_policy_sha256"].get("const")
        == "a6fb36a530e49537673b66d65ecb6e4fb4f51ffb3e7d01a0980be71f28cb367d"
        and request["properties"]["max_response_bytes"].get("const") == 1048576,
        "AuthoringTopologyRequest@1 must remain closed and fully candidate/program bound",
    )

    topology = load_schema("authoring-topology.schema.json")
    properties = topology.get("properties", {})
    require(
        topology.get("additionalProperties") is False
        and properties.get("schema_version", {}).get("const") == "AuthoringTopology@1"
        and properties.get("scope", {}).get("const")
        == "single-direct-authoring-mesh-part"
        and properties.get("complete", {}).get("const") is True
        and properties.get("topology_space", {}).get("const")
        == "source-authoring-node-local@1"
        and properties.get("id_scope", {}).get("const")
        == "geometry-program-node-bound"
        and properties.get("cross_version_stable", {}).get("const") is False
        and properties.get("runtime_write_performed", {}).get("const") is False
        and properties.get("persistent_user_data_touched", {}).get("const") is False
        and properties.get("quality_status", {}).get("const") == "structural_only"
        and properties.get("max_response_bytes", {}).get("const") == 1048576,
        "AuthoringTopology@1 must remain source-program-bound, read-only and structural",
    )
    require(
        properties.get("vertices", {}).get("maxItems") == 1536
        and properties.get("edges", {}).get("maxItems") == 1536
        and properties.get("loops", {}).get("maxItems") == 1536
        and properties.get("faces", {}).get("maxItems") == 512
        and properties["faces"]["items"].get("$ref") == "#/$defs/face"
        and topology["$defs"]["face"]["properties"]["loop_ids"].get("maxItems") == 4,
        "AuthoringTopology@1 V/E/Loop/Face budgets or triangle/quad boundary drifted",
    )

    preview_request = load_schema("authoring-mesh-edit-preview-request.schema.json")
    edit = preview_request["properties"]["edit"]
    edit_branches = [
        preview_request["$defs"][branch["$ref"].rsplit("/", 1)[-1]]
        for branch in edit.get("oneOf", [])
    ]
    operations = {
        branch["properties"]["operation"].get("const") for branch in edit_branches
    }
    require(
        preview_request.get("additionalProperties") is False
        and set(preview_request.get("required", []))
        == {"schema_version", "topology_request", "base_topology_sha256", "edit", "edit_policy_sha256", "input_sha256"}
        and preview_request["properties"]["topology_request"].get("$ref")
        == "https://forgecad.local/contracts/authoring-topology-request.schema.json"
        and operations
        == {
            "translate_vertices",
            "single_face_extrude",
            "split_edge",
            "collapse_edge",
            "dissolve_edge",
        }
        and preview_request["properties"]["edit_policy_sha256"].get("const")
        == "fc76c6dffef2a41c05ff0a65ff160c8fce5eb37d312a3ef7f78043ef92539144",
        "AuthoringMeshEditPreviewRequest@1 must expose exactly five closed edit branches",
    )
    forbidden = {"script", "python", "path", "url", "env", "secret", "network", "plugin"}
    for branch in edit_branches:
        require(
            branch.get("additionalProperties") is False
            and forbidden.isdisjoint(branch.get("properties", {})),
            "AuthoringMeshEditPreviewRequest@1 exposes a dynamic extension field",
        )

    preview = load_schema("authoring-mesh-edit-preview.schema.json")
    preview_properties = preview.get("properties", {})
    require(
        preview.get("additionalProperties") is False
        and preview_properties.get("schema_version", {}).get("const")
        == "AuthoringMeshEditPreview@1"
        and set(preview_properties.get("operation", {}).get("enum", []))
        == {
            "translate_vertices",
            "single_face_extrude",
            "split_edge",
            "collapse_edge",
            "dissolve_edge",
        }
        and preview_properties.get("geometry_materialization", {}).get("const")
        == "transient-worker-glb-not-persisted"
        and preview_properties.get("runtime_write_performed", {}).get("const") is False
        and preview_properties.get("persistent_user_data_touched", {}).get("const") is False
        and preview_properties.get("validator_status", {}).get("const") == "passed"
        and preview_properties.get("quality_status", {}).get("const") == "structural_only"
        and preview_properties.get("edit_lineage_sha256", {}).get("$ref") == "#/$defs/sha256"
        and preview_properties.get("max_response_bytes", {}).get("const") == 1048576
        and preview["$defs"]["replay"]["properties"]["artifact_size_bytes"].get("maximum")
        == 67108864,
        "AuthoringMeshEditPreview@1 must remain transient, bounded and structural-only",
    )
    proof = preview["$defs"].get("typed_operation_proof", {})
    correspondence = preview["$defs"].get("correspondence", {})
    require(
        proof.get("additionalProperties") is False
        and proof.get("properties", {}).get("schema_version", {}).get("const")
        == "TopologyOperationProof@1"
        and proof.get("properties", {}).get("identity_namespace_status", {}).get("const")
        == "source-element-only-not-materialized-to-identity-lineage@1"
        and correspondence.get("additionalProperties") is False
        and correspondence.get("properties", {}).get("identity_namespace_status", {}).get("const")
        == "source-element-only-not-materialized-to-identity-lineage@1"
        and "topology_edit" not in preview_properties,
        "typed topology proof must remain source-element-only and must not claim materialized IdentityLineage",
    )


def check_authoring_mesh_contracts() -> None:
    """Keep the first canonical Runtime-derived half-edge projection closed and bounded."""
    policy_sha256 = "aa72cadabba90ddb43dd0014cfa434ab9b13f4e072b09258072f37334c72e709"

    request = load_schema("authoring-mesh-request.schema.json")
    request_fields = {
        "schema_version",
        "project_id",
        "candidate_id",
        "artifact_id",
        "artifact_readback_sha256",
        "program_sha256",
        "operator_catalog_sha256",
        "readback_config_sha256",
        "authoring_node_id",
        "part_id",
        "authoring_mesh_policy_sha256",
        "max_response_bytes",
    }
    require(
        request.get("additionalProperties") is False
        and set(request.get("required", [])) == request_fields
        and set(request.get("properties", {})) == request_fields
        and request["properties"]["schema_version"].get("const")
        == "AuthoringMeshRequest@1"
        and request["properties"]["artifact_id"].get("$ref") == "#/$defs/sha256"
        and request["properties"]["artifact_readback_sha256"].get("$ref")
        == "#/$defs/sha256"
        and request["properties"]["program_sha256"].get("$ref")
        == "#/$defs/sha256"
        and request["properties"]["operator_catalog_sha256"].get("$ref")
        == "#/$defs/sha256"
        and request["properties"]["readback_config_sha256"].get("$ref")
        == "#/$defs/sha256"
        and request["properties"]["authoring_mesh_policy_sha256"].get("const")
        == policy_sha256
        and request["properties"]["max_response_bytes"].get("const") == 1048576,
        "AuthoringMeshRequest@1 must be closed, candidate/program/artifact/readback-bound and response-bounded",
    )

    mesh = load_schema("authoring-mesh.schema.json")
    mesh_properties = mesh.get("properties", {})
    mesh_fields = {
        "schema_version",
        "mesh_id",
        "mesh_sha256",
        "scope",
        "representation",
        "projection_kind",
        "lineage",
        "mesh_identity_derivation",
        "mesh_identity_sha256",
        "identity_policy",
        "original_identity",
        "evaluated_identity",
        "cross_version_stable",
        "counts",
        "vertices",
        "edges",
        "half_edges",
        "corners",
        "faces",
        "loops",
        "rings",
        "topology_policy",
        "topology",
        "authoring_mesh_policy_sha256",
        "max_response_bytes",
        "runtime_write_performed",
        "persistent_user_data_touched",
        "quality_status",
        "canonical_sha256",
    }
    require(
        mesh.get("additionalProperties") is False
        and set(mesh.get("required", [])) == mesh_fields
        and set(mesh_properties) == mesh_fields
        and mesh_properties["schema_version"].get("const") == "AuthoringMesh@1"
        and mesh_properties["representation"].get("const") == "half-edge-authoring@1"
        and mesh_properties["projection_kind"].get("const")
        == "runtime-derived-read-only-projection@1"
        and mesh_properties["mesh_identity_derivation"].get("const")
        == "runtime-derived-from-candidate-program-artifact-readback@1"
        and mesh_properties["mesh_identity_sha256"].get("$ref") == "#/$defs/sha256"
        and mesh_properties["identity_policy"].get("const")
        == "runtime-derived-original-evaluated-non-bijective@1"
        and mesh_properties["cross_version_stable"].get("const") is False
        and mesh_properties["topology_policy"].get("const")
        == "bounded-half-edge-manifold-with-boundary@1"
        and mesh_properties["authoring_mesh_policy_sha256"].get("const")
        == policy_sha256
        and mesh_properties["max_response_bytes"].get("const") == 1048576
        and mesh_properties["runtime_write_performed"].get("const") is False
        and mesh_properties["persistent_user_data_touched"].get("const") is False
        and mesh_properties["quality_status"].get("const") == "structural_only"
        and mesh_properties["lineage"].get("$ref") == "#/$defs/lineage"
        and mesh_properties["original_identity"].get("$ref")
        == "#/$defs/original_identity"
        and mesh_properties["evaluated_identity"].get("$ref")
        == "#/$defs/evaluated_identity",
        "AuthoringMesh@1 must separate original/evaluated identity and remain structural-only",
    )

    array_refs_and_bounds = {
        "vertices": ("#/$defs/vertex", 1, 8192),
        "edges": ("#/$defs/edge", 1, 16384),
        "half_edges": ("#/$defs/half_edge", 1, 32768),
        "corners": ("#/$defs/corner", 1, 32768),
        "faces": ("#/$defs/face", 1, 8192),
        "loops": ("#/$defs/loop", 1, 32768),
        "rings": ("#/$defs/ring", 0, 8192),
    }
    for field, (item_ref, min_items, max_items) in array_refs_and_bounds.items():
        array = mesh_properties[field]
        require(
            array.get("type") == "array"
            and array.get("minItems") == min_items
            and array.get("maxItems") == max_items
            and array.get("items", {}).get("$ref") == item_ref,
            f"AuthoringMesh@1 {field} array must remain typed and bounded",
        )

    required_element_defs = {
        "lineage",
        "original_identity",
        "evaluated_identity",
        "counts",
        "topology",
        "element_lineage",
        "vertex",
        "edge",
        "half_edge",
        "corner",
        "face",
        "loop",
        "ring",
    }
    for definition_name in required_element_defs:
        definition = mesh["$defs"].get(definition_name)
        require(
            isinstance(definition, dict)
            and definition.get("type") == "object"
            and definition.get("additionalProperties") is False
            and set(definition.get("required", []))
            == set(definition.get("properties", {})),
            f"AuthoringMesh@1 {definition_name} definition must be closed and exact-field",
        )

    lineage = mesh["$defs"]["lineage"]
    lineage_fields = {
        "project_id",
        "candidate_id",
        "artifact_id",
        "artifact_readback_sha256",
        "program_sha256",
        "operator_catalog_sha256",
        "readback_config_sha256",
        "authoring_node_id",
        "part_id",
        "lineage_status",
        "lineage_sha256",
    }
    require(
        set(lineage.get("required", [])) == lineage_fields
        and set(lineage.get("properties", {})) == lineage_fields
        and lineage["properties"]["lineage_status"].get("const")
        == "candidate-program-artifact-readback-bound@1"
        and lineage["properties"]["lineage_sha256"].get("$ref") == "#/$defs/sha256",
        "AuthoringMesh@1 lineage must bind candidate/program/artifact/readback exactly",
    )

    original_identity = mesh["$defs"]["original_identity"]
    evaluated_identity = mesh["$defs"]["evaluated_identity"]
    require(
        original_identity["properties"]["identity_kind"].get("const")
        == "runtime-derived-original-authoring@1"
        and original_identity["properties"]["namespace"].get("const") == "original"
        and original_identity["properties"]["element_id_policy"].get("const")
        == "stable-within-authoring-mesh-lineage@1"
        and evaluated_identity["properties"]["identity_kind"].get("const")
        == "runtime-derived-evaluated-artifact-readback@1"
        and evaluated_identity["properties"]["namespace"].get("const") == "evaluated"
        and evaluated_identity["properties"]["element_id_policy"].get("const")
        == "artifact-local-no-authoring-bijection@1"
        and evaluated_identity["properties"]["correspondence_policy"].get("const")
        == "non-bijective-derived-only@1",
        "AuthoringMesh@1 original/evaluated identity must not claim a one-to-one mapping",
    )

    element_lineage = mesh["$defs"]["element_lineage"]
    require(
        set(element_lineage.get("required", []))
        == {
            "original_element_ids",
            "evaluated_element_ids",
            "correspondence_kind",
            "correspondence_sha256",
        }
        and set(element_lineage["properties"]["correspondence_kind"].get("enum", []))
        == {"not_materialized", "one_to_many", "many_to_one", "many_to_many", "unknown"}
        and "one_to_one" not in element_lineage["properties"]["correspondence_kind"].get("enum", [])
        and element_lineage["properties"]["evaluated_element_ids"].get("maxItems") == 64,
        "AuthoringMesh@1 evaluated correspondence must be bounded and explicitly non-bijective",
    )

    topology = mesh["$defs"]["topology"]
    require(
        topology["properties"]["non_manifold_edge_count"].get("const") == 0
        and topology["properties"]["orientation_conflict_count"].get("const") == 0
        and set(topology["properties"]["status"].get("enum", []))
        == {"closed_manifold", "manifold_with_boundary"}
        and topology["properties"]["validation_status"].get("const") == "passed"
        and topology["properties"]["rejection_policy"].get("const")
        == "fail-closed-on-non-manifold@1"
        and topology["properties"]["face_cycle_policy"].get("const")
        == "next-prev-complete-mutual@1"
        and topology["properties"]["twin_policy"].get("const")
        == "boundary-only-null-symmetric@1",
        "AuthoringMesh@1 topology must fail closed for non-manifold and require complete cycles",
    )

    half_edge = mesh["$defs"]["half_edge"]
    half_edge_boundary_case = next(
        (
            branch
            for branch in half_edge.get("allOf", [])
            if branch.get("if", {}).get("properties", {}).get("boundary", {}).get("const")
            is True
        ),
        {},
    )
    half_edge_interior_case = next(
        (
            branch
            for branch in half_edge.get("allOf", [])
            if branch.get("if", {}).get("properties", {}).get("boundary", {}).get("const")
            is False
        ),
        {},
    )
    require(
        {
            "twin_id",
            "next_id",
            "prev_id",
            "boundary",
        }.issubset(half_edge["properties"])
        and half_edge["properties"]["twin_id"].get("$ref")
        == "#/$defs/nullable_identifier"
        and half_edge["properties"]["next_id"].get("$ref")
        == "#/$defs/identifier"
        and half_edge["properties"]["prev_id"].get("$ref")
        == "#/$defs/identifier"
        and half_edge["properties"]["boundary"].get("type") == "boolean"
        and len(half_edge.get("allOf", [])) == 2
        and half_edge_boundary_case.get("if", {}).get("required") == ["boundary"]
        and half_edge_boundary_case.get("then", {}).get("properties", {}).get("twin_id", {}).get("type")
        == "null"
        and half_edge_interior_case.get("if", {}).get("required") == ["boundary"]
        and half_edge_interior_case.get("then", {}).get("properties", {}).get("twin_id", {}).get("$ref")
        == "#/$defs/identifier"
        and half_edge["properties"]["face_id"].get("$ref") == "#/$defs/identifier"
        and half_edge["properties"]["corner_id"].get("$ref") == "#/$defs/identifier",
        "AuthoringMesh@1 half-edge must require next/prev and allow nullable twin only on boundary",
    )

    edge = mesh["$defs"]["edge"]
    require(
        edge["properties"]["hard_edge"].get("type") == "boolean"
        and edge["properties"]["crease"].get("minimum") == 0
        and edge["properties"]["crease"].get("maximum") == 2
        and edge["properties"]["uv_seam"].get("type") == "boolean",
        "AuthoringMesh@1 edge must carry bounded hard_edge, crease and uv_seam attributes",
    )

    for definition_name in ["vertex", "edge", "half_edge", "corner", "face", "loop", "ring"]:
        properties = mesh["$defs"][definition_name]["properties"]
        require(
            "original_identity" not in properties
            and "evaluated_identity" not in properties
            and properties["lineage"].get("$ref") == "#/$defs/element_lineage",
            f"AuthoringMesh@1 {definition_name} must expose non-bijective element lineage only",
        )


def check_durable_authoring_mesh_contracts() -> None:
    """Keep the durable canonical AuthoringMesh family closed and honest.

    ``AuthoringMesh@1`` is a Runtime-derived read-only projection.  This
    separate family is the next-stage durable shape: Runtime owns its CAS /
    SQLite materialization, the source is bound to one candidate/program /
    evaluated-artifact/readback lineage, and evaluated IDs never become an
    asserted one-to-one or cross-revision identity.
    """
    expected = {
        "authoring-mesh-canonical.schema.json": "AuthoringMeshCanonical@1",
        "authoring-mesh-artifact.schema.json": "AuthoringMeshArtifact@1",
        "authoring-mesh-link.schema.json": "AuthoringMeshLink@1",
        "authoring-mesh-prepare-request.schema.json": "AuthoringMeshPrepareRequest@1",
        "authoring-mesh-prepare-result.schema.json": "AuthoringMeshPrepareResult@1",
        "authoring-mesh-get-request.schema.json": "AuthoringMeshGetRequest@1",
        "authoring-mesh-get-result.schema.json": "AuthoringMeshGetResult@1",
    }
    actual = {path.name for path in SCHEMA_ROOT.glob("authoring-mesh-*.schema.json")}
    require(set(expected) <= actual, "durable AuthoringMesh schema family is incomplete")

    schemas = {name: load_schema(name) for name in expected}
    for filename, schema_version in expected.items():
        schema = schemas[filename]
        properties = schema.get("properties", {})
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and schema.get("title") == schema_version
            and properties.get("schema_version", {}).get("const") == schema_version
            and set(schema.get("required", [])) == set(properties),
            f"{schema_version} must be a closed exact-field object",
        )
        for definition_name, definition in schema.get("$defs", {}).items():
            if definition.get("type") == "object":
                definition_properties = definition.get("properties", {})
                require(
                    definition.get("additionalProperties") is False
                    and set(definition.get("required", [])) == set(definition_properties),
                    f"{schema_version} {definition_name} must be closed and exact-field",
                )

    canonical = schemas["authoring-mesh-canonical.schema.json"]
    canonical_properties = canonical["properties"]
    lineage_fields = {
        "project_id",
        "candidate_id",
        "candidate_state_sha256",
        "source_program_object_sha256",
        "source_program_sha256",
        "source_artifact_object_sha256",
        "source_artifact_sha256",
        "source_artifact_readback_object_sha256",
        "source_artifact_readback_sha256",
        "source_lineage_sha256",
    }
    require(
        lineage_fields <= set(canonical_properties)
        and canonical_properties["representation"].get("const")
        == "runtime-owned-original-half-edge@1"
        and canonical_properties["storage_policy"].get("const")
        == "runtime-owned-sqlite-cas-canonical-authoring-mesh@1"
        and canonical_properties["writer_policy"].get("const")
        == "forgecad-runtime-only-state-writer@1"
        and canonical_properties["runtime_write_performed"].get("const") is True
        and canonical_properties["persistent_user_data_touched"].get("const") is True,
        "AuthoringMeshCanonical@1 must be Runtime-owned and candidate/program/artifact/readback bound",
    )
    require(
        canonical_properties["cross_version_stable"].get("const") is False
        and canonical["$defs"]["cross_version_stability"]["properties"]["status"].get("const")
        == "not-proven@1"
        and canonical["$defs"]["original_identity"]["properties"]["element_id_policy"].get("const")
        == "lineage-scoped-opaque-not-cross-version-stable@1"
        and canonical["$defs"]["evaluated_identity"]["properties"]["correspondence_policy"].get("const")
        == "non-bijective-derived-only@1"
        and canonical["$defs"]["evaluated_identity"]["properties"]["cross_version_stable"].get("const")
        is False,
        "AuthoringMeshCanonical@1 must not invent cross-version or evaluated identity stability",
    )
    correspondence_kinds = set(
        canonical["$defs"]["element_lineage"]["properties"]["correspondence_kind"].get("enum", [])
    )
    require(
        "one_to_one" not in correspondence_kinds
        and correspondence_kinds
        == {"not_materialized", "one_to_many", "many_to_one", "many_to_many", "unknown"},
        "AuthoringMeshCanonical@1 evaluated lineage must be explicitly non-bijective",
    )
    require(
        all(
            canonical_properties[field].get("const") is False
            for field in ("stage_advanced", "candidate_confirmed", "version_created", "export_performed")
        )
        and canonical_properties["quality_status"].get("const") == "structural_only",
        "AuthoringMeshCanonical@1 must not advance Stage, confirm, version or export",
    )
    for definition_name in (
        "vertex",
        "edge",
        "half_edge",
        "corner",
        "face",
        "loop",
        "ring",
    ):
        definition = canonical["$defs"][definition_name]
        require(
            definition["properties"].get("lineage", {}).get("$ref")
            == "#/$defs/element_lineage",
            f"AuthoringMeshCanonical@1 {definition_name} must retain non-bijective element lineage",
        )
    half_edge = canonical["$defs"]["half_edge"]
    require(
        half_edge["properties"]["twin_id"].get("$ref")
        == "#/$defs/nullable_identifier"
        and len(half_edge.get("allOf", [])) == 2,
        "AuthoringMeshCanonical@1 half-edge must keep boundary-only nullable twins",
    )
    topology = canonical["$defs"]["topology"]
    require(
        topology["properties"]["non_manifold_edge_count"].get("const") == 0
        and topology["properties"]["orientation_conflict_count"].get("const") == 0
        and topology["properties"]["validation_status"].get("const") == "passed"
        and topology["properties"]["rejection_policy"].get("const")
        == "fail-closed-on-non-manifold@1",
        "AuthoringMeshCanonical@1 must fail closed on non-manifold or orientation conflicts",
    )

    artifact = schemas["authoring-mesh-artifact.schema.json"]["properties"]
    require(
        {
            "candidate_id",
            "candidate_state_sha256",
            "source_program_object_sha256",
            "source_program_sha256",
            "evaluated_artifact_object_sha256",
            "evaluated_artifact_sha256",
            "evaluated_artifact_readback_object_sha256",
            "evaluated_artifact_readback_sha256",
        } <= set(artifact)
        and artifact["correspondence_policy"].get("const")
        == "non-bijective-derived-only@1"
        and artifact["writer_policy"].get("const")
        == "forgecad-runtime-only-state-writer@1"
        and artifact["stage_advanced"].get("const") is False
        and artifact["candidate_confirmed"].get("const") is False
        and artifact["version_created"].get("const") is False
        and artifact["export_performed"].get("const") is False,
        "AuthoringMeshArtifact@1 must remain evaluated-sidecar-only and non-promoting",
    )

    link = schemas["authoring-mesh-link.schema.json"]["properties"]
    require(
        link["link_policy"].get("const")
        == "canonical-original-plus-evaluated-sidecar-exact-lineage@1"
        and link["materialization_status"].get("const")
        == "runtime-owned-durable-authoring-mesh-link@1"
        and link["runtime_write_performed"].get("const") is True
        and link["stage_advanced"].get("const") is False
        and link["candidate_confirmed"].get("const") is False
        and link["version_created"].get("const") is False
        and link["export_performed"].get("const") is False,
        "AuthoringMeshLink@1 must be Runtime-owned and non-promoting",
    )

    prepare_request = schemas["authoring-mesh-prepare-request.schema.json"]["properties"]
    get_request = schemas["authoring-mesh-get-request.schema.json"]["properties"]
    require(
        prepare_request["runtime_write_performed"].get("const") is False
        and prepare_request["writer_policy"].get("const")
        == "forgecad-runtime-only-state-writer@1"
        and prepare_request["max_response_bytes"].get("const") == 1048576
        and prepare_request["source_lineage_sha256"].get("$ref") == "#/$defs/sha256"
        and get_request["input_sha256"].get("$ref") == "#/$defs/sha256"
        and get_request["writer_policy"].get("const")
        == "forgecad-runtime-only-state-writer@1"
        and get_request["runtime_write_performed"].get("const") is False
        and get_request["persistent_user_data_touched"].get("const") is False
        and "canonical_mesh" not in prepare_request
        and "evaluated_identity" not in prepare_request,
        "AuthoringMesh prepare/get requests must be closed, bounded and free of caller-supplied mesh truth",
    )

    result_limits = [
        "RUNTIME_SOLE_WRITER",
        "NO_STAGE_ADVANCEMENT",
        "NO_CANDIDATE_CONFIRM",
        "NO_VERSION_CREATED",
        "NO_EXPORT",
        "EVALUATED_IDENTITY_NON_BIJECTIVE",
        "CROSS_VERSION_STABILITY_NOT_PROVEN",
        "STRUCTURAL_ONLY_NOT_COMMERCIAL_QUALITY",
    ]
    for filename, schema_version, write_performed, touched in (
        (
            "authoring-mesh-prepare-result.schema.json",
            "AuthoringMeshPrepareResult@1",
            True,
            True,
        ),
        (
            "authoring-mesh-get-result.schema.json",
            "AuthoringMeshGetResult@1",
            False,
            False,
        ),
    ):
        properties = schemas[filename]["properties"]
        require(
            properties["canonical_mesh"].get("$ref")
            == "https://forgecad.local/contracts/authoring-mesh-canonical.schema.json"
            and properties["artifact"].get("$ref")
            == "https://forgecad.local/contracts/authoring-mesh-artifact.schema.json"
            and properties["durable_link"].get("$ref")
            == "https://forgecad.local/contracts/authoring-mesh-link.schema.json"
            and properties["runtime_write_performed"].get("const") is write_performed
            and properties["persistent_user_data_touched"].get("const") is touched
            and properties["stage_advanced"].get("const") is False
            and properties["candidate_confirmed"].get("const") is False
            and properties["version_created"].get("const") is False
            and properties["export_performed"].get("const") is False
            and properties["quality_status"].get("const") == "structural_only"
            and properties["limitations"].get("const") == result_limits,
            f"{schema_version} must expose durable readback while locking all promotion actions",
        )


def check_subdivision_evaluation_contracts() -> None:
    request = load_schema("subdivision-evaluation-request.schema.json")
    request_fields = {
        "schema_version",
        "project_id",
        "representation_plan_sha256",
        "part_id",
        "material_zone_id",
        "solid",
        "control_cage",
        "policy",
        "transform",
        "budgets",
        "input_sha256",
    }
    policy = request["$defs"]["policy"]
    require(
        request.get("additionalProperties") is False
        and set(request.get("required", [])) == request_fields
        and set(request.get("properties", {})) == request_fields
        and request["properties"]["schema_version"].get("const")
        == "SubdivisionEvaluationRequest@2"
        and request["properties"]["solid"].get("const") is False
        and request["$defs"]["control_cage"]["properties"]["u_points"].get("maximum")
        == 16
        and request["$defs"]["control_cage"]["properties"]["v_points"].get("maximum")
        == 16
        and request["$defs"]["control_cage"]["properties"]["control_points"].get("maxItems")
        == 256
        and request["$defs"]["coordinate_scalar"].get("minimum") == -10
        and request["$defs"]["coordinate_scalar"].get("maximum") == 10
        and request["$defs"]["radian_scalar"].get("minimum")
        == -6.283185307179586
        and request["$defs"]["radian_scalar"].get("maximum")
        == 6.283185307179586
        and policy.get("additionalProperties") is False
        and policy["properties"]["scheme"].get("const")
        == "catmull-clark-uniform-regular-quad-grid"
        and policy["properties"]["subdivision_levels"].get("maximum") == 2
        and policy["properties"]["boundary_interpolation"].get("const")
        == "edge-and-corner"
        and policy["properties"]["crease_mode"].get("const") == "unsupported"
        and policy["properties"]["limit_surface"].get("const") is False
        and policy["properties"]["adaptive"].get("const") is False,
        "SubdivisionEvaluationRequest@2 must remain a closed bounded regular-grid policy",
    )
    _check_subdivision_evaluation_result_contracts()


def check_subdivision_crease_contracts() -> None:
    request = load_schema("subdivision-crease-evaluation-request.schema.json")
    request_fields = {
        "schema_version", "project_id", "representation_plan_sha256", "part_id",
        "material_zone_id", "solid", "control_cage", "crease_edges", "policy",
        "transform", "budgets", "input_sha256",
    }
    cage = request["$defs"]["control_cage"]
    crease = request["$defs"]["crease_edge"]
    policy = request["$defs"]["policy"]
    require(
        request.get("additionalProperties") is False
        and set(request.get("required", [])) == request_fields
        and set(request.get("properties", {})) == request_fields
        and request["properties"]["schema_version"].get("const")
        == "SubdivisionCreaseEvaluationRequest@1"
        and request["properties"]["solid"].get("const") is False
        and cage.get("additionalProperties") is False
        and cage["properties"]["u_points"].get("minimum") == 3
        and cage["properties"]["u_points"].get("maximum") == 16
        and cage["properties"]["v_points"].get("minimum") == 3
        and cage["properties"]["v_points"].get("maximum") == 16
        and cage["properties"]["control_points"].get("minItems") == 9
        and cage["properties"]["control_points"].get("maxItems") == 256
        and request["properties"]["crease_edges"].get("minItems") == 1
        and request["properties"]["crease_edges"].get("maxItems") == 128
        and crease.get("additionalProperties") is False
        and set(crease.get("required", []))
        == {"vertex_a", "vertex_b", "sharpness_levels"}
        and crease["properties"]["sharpness_levels"].get("minimum") == 1
        and crease["properties"]["sharpness_levels"].get("maximum") == 2
        and policy.get("additionalProperties") is False
        and policy["properties"]["subdivision_levels"].get("minimum") == 1
        and policy["properties"]["subdivision_levels"].get("maximum") == 2
        and policy["properties"]["boundary_interpolation"].get("const") == "edge-only"
        and policy["properties"]["crease_method"].get("const")
        == "uniform-integer-level-decay@1"
        and policy["properties"]["sharpness_domain"].get("const")
        == "integer-levels-1-to-2"
        and policy["properties"]["limit_surface"].get("const") is False
        and policy["properties"]["adaptive"].get("const") is False,
        "SubdivisionCreaseEvaluationRequest@1 must remain a closed bounded edge-only integer-crease policy",
    )

    result = load_schema("subdivision-crease-evaluation-result.schema.json")
    result_fields = {
        "schema_version", "project_id", "representation_plan_sha256", "part_id",
        "material_zone_id", "solid", "input_sha256", "control_cage_sha256",
        "crease_edges_sha256", "evaluation_policy_sha256", "predicted_topology_sha256",
        "program_sha256", "operator_catalog_sha256", "predicted_topology", "crease_policy",
        "attribute_policy", "geometry_program", "validator_status", "validator_scope",
        "quality_status", "limitations", "canonical_sha256",
    }
    limitations = result["properties"]["limitations"].get("const", [])
    require(
        result.get("additionalProperties") is False
        and set(result.get("required", [])) == result_fields
        and set(result.get("properties", {})) == result_fields
        and result["properties"]["schema_version"].get("const")
        == "SubdivisionCreaseEvaluationResult@1"
        and result["properties"]["geometry_program"].get("$ref")
        == "https://forgecad.local/contracts/geometry-program-v2.schema.json"
        and result["properties"]["validator_scope"].get("const")
        == "typed-policy-program-hash-and-worker-operator-validation"
        and result["properties"]["quality_status"].get("const") == "structural_only"
        and result["$defs"]["crease_policy"].get("additionalProperties") is False
        and result["$defs"]["crease_policy"]["properties"]["boundary_vertices"].get("const")
        == "edge-only-crease-rule-not-corner-pinned"
        and "compiled_geometry_not_created_by_read_only_projection" in limitations
        and "visual_quality_not_evaluated" in limitations
        and "fractional_and_vertex_creases_unsupported" in limitations
        and "root_lineage_requires_separate_subdivision_topology_lineage_preview"
        in limitations,
        "SubdivisionCreaseEvaluationResult@1 must expose request-bound structural truth and explicit non-claims",
    )


def check_render_profile_contracts() -> None:
    profile = load_schema("render-profile.schema.json")
    fields = {
        "schema_version",
        "profile_id",
        "engine_id",
        "backend_id",
        "renderer_revision",
        "resolution",
        "sampling",
        "color_pipeline",
        "alpha",
        "aovs",
        "aov_definition_sha256",
        "color_pipeline_sha256",
        "id_palette_definition_sha256",
        "canonical_sha256",
    }
    aovs = profile["properties"]["aovs"]
    expected_passes = [
        "beauty",
        "silhouette",
        "depth",
        "normal",
        "ao",
        "part-id",
        "material-id",
        "wireframe",
        "uv-stretch",
    ]
    prefix = aovs.get("prefixItems", [])
    require(
        profile.get("additionalProperties") is False
        and set(profile.get("required", [])) == fields
        and set(profile.get("properties", {})) == fields
        and profile["properties"]["schema_version"].get("const") == "RenderProfile@1"
        and profile["properties"]["engine_id"].get("const") == "forgecad-fixed-software@2"
        and profile["properties"]["backend_id"].get("const") == "cpu-raster@1"
        and profile["properties"]["renderer_revision"].get("const") == "forgecad-renderer-2"
        and profile["properties"]["sampling"]["properties"]["seed_policy"].get("const")
        == "not-applicable-no-rng"
        and profile["properties"]["sampling"]["properties"]["adaptive"].get("const") is False
        and profile["properties"]["sampling"]["properties"]["motion_blur"].get("const") is False
        and profile["properties"]["alpha"]["properties"]["transparent_film"].get("const") is False
        and len(prefix) == 9
        and aovs.get("minItems") == 9
        and aovs.get("maxItems") == 9
        and aovs.get("items") is False,
        "RenderProfile@1 must remain a closed fixed software-render contract",
    )
    for index, pass_id in enumerate(expected_passes):
        overlay = prefix[index]["allOf"][1]["properties"]
        require(
            overlay["pass_id"].get("const") == pass_id,
            f"RenderProfile@1 AOV order drifted at {pass_id}",
        )
        if pass_id == "beauty":
            require(
                overlay["semantic_kind"].get("const") == "color"
                and overlay["color_transform"].get("const") == "fixed-linear-to-srgb@1",
                "beauty must be the only display-color AOV",
            )
        else:
            require(
                overlay["semantic_kind"].get("const") != "color"
                and overlay["color_transform"].get("const") == "none",
                f"{pass_id} must remain non-color data",
            )
        if pass_id == "part-id":
            require(
                overlay["source_value_range"].get("const")
                == "categorical-mesh-index-0-255",
                "part-id must bind the bounded u8 palette domain",
            )
        if pass_id == "material-id":
            require(
                overlay["source_value_range"].get("const")
                == "categorical-material-index-0-255",
                "material-id must bind the bounded u8 palette domain",
            )

    render_set = load_schema("render-set-v2.schema.json")
    for field in (
        "render_profile",
        "render_profile_sha256",
        "aov_definition_sha256",
        "color_pipeline_sha256",
        "id_palette_definition_sha256",
    ):
        require(field in render_set.get("required", []), f"RenderSet@2 lacks {field}")
    require(
        render_set["properties"]["render_profile"].get("$ref")
        == "https://forgecad.local/contracts/render-profile.schema.json",
        "RenderSet@2 must bind the full RenderProfile@1",
    )


def check_render_evidence_integrity_contracts() -> None:
    request = load_schema("render-evidence-integrity-request.schema.json")
    request_fields = {
        "schema_version",
        "project_id",
        "candidate_id",
        "artifact_sha256",
        "artifact_readback_object_sha256",
        "program_sha256",
        "reference_id",
        "reference_sha256",
        "camera_hash",
        "camera_object_sha256",
        "render_set_object_sha256",
        "comparison_report_object_sha256",
        "quality_report_object_sha256",
        "canonical_sha256",
    }
    require(
        request.get("additionalProperties") is False
        and set(request.get("required", [])) == request_fields
        and set(request.get("properties", {})) == request_fields
        and request["properties"]["schema_version"].get("const")
        == "RenderEvidenceIntegrityRequest@1",
        "RenderEvidenceIntegrityRequest@1 must remain closed and exact-hash-bound",
    )

    result = load_schema("render-evidence-integrity.schema.json")
    result_fields = {
        "schema_version",
        "projection_status",
        "read_only",
        "project_id",
        "candidate_id",
        "artifact_sha256",
        "artifact_readback_object_sha256",
        "program_sha256",
        "reference_id",
        "reference_sha256",
        "request_sha256",
        "camera_binding",
        "object_hashes",
        "source_bytes_binding",
        "render_profile_binding",
        "aov_artifacts",
        "comparison_mask_binding",
        "comparison_status",
        "quality_gate_binding",
        "binding_status",
        "runtime_write_performed",
        "max_response_bytes",
        "limitations",
        "canonical_sha256",
    }
    expected_passes = [
        "beauty",
        "silhouette",
        "depth",
        "normal",
        "ao",
        "part-id",
        "material-id",
        "wireframe",
        "uv-stretch",
    ]
    aovs = result["properties"]["aov_artifacts"]
    prefixes = aovs.get("prefixItems", [])
    require(
        result.get("additionalProperties") is False
        and set(result.get("required", [])) == result_fields
        and set(result.get("properties", {})) == result_fields
        and result["properties"]["schema_version"].get("const")
        == "RenderEvidenceIntegrity@1"
        and result["properties"]["projection_status"].get("const")
        == "projection/read-only"
        and result["properties"]["read_only"].get("const") is True
        and result["properties"]["runtime_write_performed"].get("const") is False
        and result["properties"]["max_response_bytes"].get("const") == 1048576
        and aovs.get("minItems") == 9
        and aovs.get("maxItems") == 9
        and aovs.get("items") is False
        and len(prefixes) == 9
        and result["$defs"]["camera_binding"]["properties"]["status"].get("const")
        == "same_camera_verified"
        and set(result["$defs"]["object_hashes"].get("required", []))
        == {"artifact_readback", "render_set", "comparison_report", "quality_report"}
        and set(result["$defs"]["source_bytes_binding"].get("required", []))
        == {"artifact", "reference"}
        and "HISTORICAL_RECEIPTS_NOT_REPAIRED"
        in result["properties"]["limitations"].get("const", [])
        and "STRUCTURAL_INTEGRITY_DOES_NOT_PROVE_VISUAL_QUALITY"
        in result["properties"]["limitations"].get("const", []),
        "RenderEvidenceIntegrity@1 must remain bounded, read-only and structurally honest",
    )
    for index, pass_id in enumerate(expected_passes):
        ref = prefixes[index].get("$ref", "")
        require(
            pass_id.replace("-", "_") in ref or pass_id == "beauty",
            f"RenderEvidenceIntegrity@1 AOV order drifted at {pass_id}",
        )
    aov_base = result["$defs"]["aov_base"]
    require(
        aov_base.get("additionalProperties") is False
        and "cas_object_sha256" in aov_base.get("required", [])
        and "bytes_sha256" in aov_base.get("required", [])
        and aov_base["properties"]["width"].get("const") == 512
        and aov_base["properties"]["height"].get("const") == 512
        and aov_base["properties"]["mime"].get("const") == "image/png"
        and aov_base["properties"]["channels"].get("const") == "rgba8",
        "RenderEvidenceIntegrity@1 AOV rows must separate CAS and byte hashes and decode exact RGBA8 PNG",
    )
    bytes_row = result["$defs"]["source_bytes_row"]
    metric_row = result["$defs"]["metric_gate_result"]
    require(
        bytes_row.get("additionalProperties") is False
        and set(bytes_row.get("required", []))
        == {"cas_object_sha256", "bytes_sha256", "size_bytes", "mime", "cas_hash_verified"}
        and metric_row.get("additionalProperties") is False
        and set(metric_row.get("required", []))
        == {"metric_name", "direction", "observed", "threshold", "status"}
        and result["$defs"]["quality_gate_binding"]["properties"]["metric_gate_results"]
        .get("items", {})
        .get("$ref")
        == "#/$defs/metric_gate_result",
        "RenderEvidenceIntegrity@1 must close source-byte and threshold-row bindings",
    )


def check_render_evidence_replay_contracts() -> None:
    request = load_schema("render-evidence-replay-request.schema.json")
    request_fields = {
        "schema_version",
        "candidate_state_sha256",
        "integrity_request",
        "replay_policy",
        "canonical_sha256",
    }
    require(
        request.get("additionalProperties") is False
        and set(request.get("required", [])) == request_fields
        and set(request.get("properties", {})) == request_fields
        and request["properties"]["schema_version"].get("const")
        == "RenderEvidenceReplayRequest@1"
        and request["properties"]["integrity_request"].get("$ref")
        == "https://forgecad.local/contracts/render-evidence-integrity-request.schema.json"
        and request["properties"]["replay_policy"].get("const")
        == "fixed-worker-nine-aov-byte-replay-read-only@1",
        "RenderEvidenceReplayRequest@1 must remain closed and reuse the exact integrity envelope",
    )

    result = load_schema("render-evidence-replay.schema.json")
    result_fields = {
        "schema_version", "projection_status", "read_only", "project_id", "candidate_id",
        "candidate_state_sha256", "artifact_sha256", "camera_hash",
        "source_render_set_object_sha256", "request_sha256", "integrity_request_sha256",
        "integrity_result_sha256", "replay_policy", "appearance_binding_status",
        "temporary_materialization", "render_profile_binding", "worker_cohort_binding",
        "aov_replay_rows", "mismatched_passes", "replay_status", "determinism_claim",
        "binding_status", "runtime_write_performed", "persistent_user_data_touched",
        "max_response_bytes", "limitations", "canonical_sha256",
    }
    properties = result.get("properties", {})
    expected_passes = [
        "beauty", "silhouette", "depth", "normal", "ao", "part-id",
        "material-id", "wireframe", "uv-stretch",
    ]
    rows = properties["aov_replay_rows"]
    prefixes = rows.get("prefixItems", [])
    limitations = properties["limitations"].get("const", [])
    require(
        result.get("additionalProperties") is False
        and set(result.get("required", [])) == result_fields
        and set(properties) == result_fields
        and properties["schema_version"].get("const") == "RenderEvidenceReplay@1"
        and properties["projection_status"].get("const") == "transient-replay/read-only"
        and properties["read_only"].get("const") is True
        and properties["replay_policy"].get("const")
        == "fixed-worker-nine-aov-byte-replay-read-only@1"
        and properties["appearance_binding_status"].get("const")
        == "artifact-embedded-materials-only"
        and properties["temporary_materialization"].get("const") == "in-memory-only"
        and properties["replay_status"].get("const") == "repeat_byte_exact_match"
        and properties["determinism_claim"].get("const")
        == "repeat_byte_exact_same_cohort"
        and properties["binding_status"].get("const") == "passed"
        and properties["runtime_write_performed"].get("const") is False
        and properties["persistent_user_data_touched"].get("const") is False
        and properties["max_response_bytes"].get("const") == 1048576
        and rows.get("minItems") == 9
        and rows.get("maxItems") == 9
        and rows.get("items") is False
        and len(prefixes) == 9
        and "AUTHORED_APPEARANCE_PROGRAM_NOT_DURABLY_BOUND" in limitations
        and "STRUCTURAL_REPLAY_DOES_NOT_PROVE_VISUAL_QUALITY" in limitations
        and "NO_CYCLES_EEVEE_OR_OCIO_PARITY" in limitations
        and "NO_CAS_SQLITE_CANDIDATE_OR_VERSION_WRITE" in limitations,
        "RenderEvidenceReplay@1 must remain same-cohort, read-only, bounded and structurally honest",
    )
    for index, pass_id in enumerate(expected_passes):
        ref = prefixes[index].get("$ref", "")
        require(
            pass_id.replace("-", "_") in ref or pass_id == "beauty",
            f"RenderEvidenceReplay@1 AOV order drifted at {pass_id}",
        )
    profile = result["$defs"]["render_profile_binding"]
    cohort = result["$defs"]["worker_cohort_binding"]
    row = result["$defs"]["aov_replay_row"]
    row_required = {
        "pass", "source_cas_object_sha256", "source_bytes_sha256",
        "source_pixel_sha256", "first_replay_bytes_sha256",
        "first_replay_pixel_sha256", "repeat_replay_bytes_sha256",
        "repeat_replay_pixel_sha256", "source_size_bytes", "first_replay_size_bytes",
        "repeat_replay_size_bytes", "width", "height", "mime", "channels",
        "color_space", "source_cas_verified", "first_replay_png_decode_verified",
        "repeat_replay_png_decode_verified", "byte_exact", "pixel_exact",
        "repeat_byte_exact",
    }
    require(
        profile.get("additionalProperties") is False
        and profile["properties"]["profile_match"].get("const") is True
        and cohort.get("additionalProperties") is False
        and cohort["properties"]["status"].get("const") == "same_cohort_verified"
        and all(
            cohort["properties"][field].get("$ref") == "#/$defs/sha256"
            for field in [
                "source_render_worker_build_cohort_sha256",
                "first_replay_render_worker_build_cohort_sha256",
                "repeat_replay_render_worker_build_cohort_sha256",
            ]
        )
        and row.get("additionalProperties") is False
        and set(row.get("required", [])) == row_required
        and row["properties"]["width"].get("const") == 512
        and row["properties"]["height"].get("const") == 512
        and row["properties"]["mime"].get("const") == "image/png"
        and row["properties"]["channels"].get("const") == "rgba8"
        and all(
            row["properties"][field].get("const") is True
            for field in [
                "source_cas_verified", "first_replay_png_decode_verified",
                "repeat_replay_png_decode_verified", "byte_exact", "pixel_exact",
                "repeat_byte_exact",
            ]
        ),
        "RenderEvidenceReplay@1 must bind exact profile/cohort and raw plus decoded AOV equality",
    )


def _check_subdivision_evaluation_result_contracts() -> None:
    result = load_schema("subdivision-evaluation-result.schema.json")
    result_fields = {
        "schema_version",
        "project_id",
        "representation_plan_sha256",
        "part_id",
        "material_zone_id",
        "solid",
        "input_sha256",
        "control_cage_sha256",
        "evaluation_policy_sha256",
        "predicted_topology_sha256",
        "program_sha256",
        "operator_catalog_sha256",
        "predicted_topology",
        "attribute_policy",
        "geometry_program",
        "validator_status",
        "validator_scope",
        "quality_status",
        "limitations",
        "canonical_sha256",
    }
    attributes = result["$defs"]["attribute_policy"]["properties"]
    require(
        result.get("additionalProperties") is False
        and set(result.get("required", [])) == result_fields
        and set(result.get("properties", {})) == result_fields
        and result["properties"]["schema_version"].get("const")
        == "SubdivisionEvaluationResult@2"
        and result["properties"]["solid"].get("const") is False
        and result["properties"]["geometry_program"].get("$ref")
        == "https://forgecad.local/contracts/geometry-program-v2.schema.json"
        and result["properties"]["quality_status"].get("const") == "structural_only"
        and result["properties"]["validator_scope"].get("const")
        == "typed-policy-and-program-hash-only"
        and attributes["normals"].get("const") == "worker-regenerated-smooth"
        and attributes["uv"].get("const")
        == "worker-triangle-chart-postprocess"
        and attributes["tangents"].get("const")
        == "worker-mikktspace-0.3.0-postprocess"
        and "limit_surface_not_evaluated"
        in result["properties"]["limitations"].get("const", [])
        and "face_varying_uv_not_interpolated"
        in result["properties"]["limitations"].get("const", [])
        and "per_element_lineage_not_available"
        in result["properties"]["limitations"].get("const", []),
        "SubdivisionEvaluationResult@2 must expose structural topology and honest attribute limitations",
    )


def check_mechanical_pose_contracts() -> None:
    request = load_schema("mechanical-pose-evaluation-request.schema.json")
    request_fields = {
        "schema_version", "project_id", "artifact_id", "candidate_id",
        "artifact_readback_sha256", "program_sha256", "operator_catalog_sha256",
        "readback_config_sha256", "rest_frame_draft", "pose_action_draft",
        "sample_time_ticks", "input_sha256",
    }
    rest_draft = request["$defs"]["rest_frame_draft"]
    action_draft = request["$defs"]["action_draft"]
    require(
        request.get("additionalProperties") is False
        and set(request.get("required", [])) == request_fields
        and set(request.get("properties", {})) == request_fields
        and request["properties"]["schema_version"].get("const")
        == "MechanicalPoseEvaluationRequest@1"
        and rest_draft.get("additionalProperties") is False
        and rest_draft["properties"]["links"].get("maxItems") == 64
        and rest_draft["properties"]["parent_map"].get("maxItems") == 63
        and action_draft.get("additionalProperties") is False
        and action_draft["properties"]["timebase_hz"].get("const") == 1000
        and action_draft["properties"]["interpolation"].get("const") == "linear@1"
        and action_draft["properties"]["extrapolation"].get("const") == "clamp@1"
        and action_draft["properties"]["channels"].get("maxItems") == 64
        and request["$defs"]["channel"]["properties"]["keys"].get("maxItems") == 32,
        "MechanicalPoseEvaluationRequest@1 must remain closed, candidate-bound and bounded",
    )
    rest = load_schema("mechanical-rest-frame.schema.json")
    action = load_schema("mechanical-pose-action.schema.json")
    result = load_schema("mechanical-pose-evaluation-result.schema.json")
    require(
        rest.get("additionalProperties") is False
        and rest["properties"]["schema_version"].get("const") == "MechanicalRestFrame@1"
        and rest["properties"]["coordinate_system"].get("const") == "forgecad-rh-y-up-m@1"
        and rest["properties"]["transform_convention"].get("const")
        == "column-vector-trs-quaternion@1"
        and rest["properties"]["links"].get("maxItems") == 64
        and action.get("additionalProperties") is False
        and action["properties"]["schema_version"].get("const") == "MechanicalPoseAction@1"
        and action["properties"]["interpolation"].get("const") == "linear@1"
        and result.get("additionalProperties") is False
        and result["properties"]["schema_version"].get("const")
        == "MechanicalPoseEvaluationResult@1"
        and result["properties"]["geometry_materialization"].get("const")
        == "not-materialized"
        and result["properties"]["worker_evaluation"].get("const")
        == "not-run-runtime-read-only-projection"
        and result["properties"]["quality_status"].get("const") == "structural_only"
        and "no-skinning-or-mesh-deformation"
        in result["properties"]["limitations"].get("const", [])
        and "no-ik-constraints-nla-fcurves-or-drivers"
        in result["properties"]["limitations"].get("const", []),
        "Mechanical pose outputs must remain canonical rigid-link structural evidence",
    )
    sequence_request = load_schema("mechanical-pose-sequence-preview-request.schema.json")
    sequence_result = load_schema("mechanical-pose-sequence-preview.schema.json")
    require(
        sequence_request.get("additionalProperties") is False
        and set(sequence_request.get("required", [])) == request_fields
        and set(sequence_request.get("properties", {})) == request_fields
        and sequence_request["properties"]["schema_version"].get("const")
        == "MechanicalPoseSequencePreviewRequest@1"
        and sequence_request["properties"]["sample_time_ticks"].get("minItems") == 1
        and sequence_request["properties"]["sample_time_ticks"].get("maxItems") == 16
        and sequence_request["properties"]["sample_time_ticks"].get("uniqueItems") is True
        and sequence_request["$defs"]["rest_frame_draft"].get("additionalProperties") is False
        and sequence_request["$defs"]["action_draft"].get("additionalProperties") is False,
        "MechanicalPoseSequencePreviewRequest@1 must remain closed, candidate-bound and bounded to 16 samples",
    )
    require(
        sequence_result.get("additionalProperties") is False
        and sequence_result["properties"]["schema_version"].get("const")
        == "MechanicalPoseSequencePreview@1"
        and sequence_result["properties"]["samples"].get("minItems") == 1
        and sequence_result["properties"]["samples"].get("maxItems") == 16
        and sequence_result["properties"]["geometry_materialization"].get("const")
        == "not-materialized"
        and sequence_result["properties"]["worker_evaluation"].get("const")
        == "not-run-runtime-read-only-projection"
        and sequence_result["properties"]["quality_status"].get("const")
        == "structural_only"
        and "sequence-preview-only"
        in sequence_result["properties"]["limitations"].get("const", [])
        and "no-animation-asset-or-timeline"
        in sequence_result["properties"]["limitations"].get("const", []),
        "MechanicalPoseSequencePreview@1 must remain a bounded structural-only projection",
    )
    preview_request = load_schema("mechanical-pose-geometry-preview-request.schema.json")
    preview_request_fields = {
        "schema_version", "pose_evaluation_request", "preview_policy", "input_sha256",
    }
    preview_result = load_schema("mechanical-pose-geometry-preview.schema.json")
    preview_result_fields = {
        "schema_version", "project_id", "candidate_id", "source_artifact_id",
        "source_artifact_readback_sha256", "source_program_sha256",
        "operator_catalog_sha256", "readback_config_sha256", "input_sha256",
        "rest_frame_sha256", "pose_action_sha256", "sample_time_ticks",
        "evaluated_pose_sha256", "application_policy", "application_policy_sha256",
        "part_deltas", "part_deltas_sha256", "posed_geometry_program",
        "posed_program_sha256", "transient_artifact", "geometry_materialization",
        "worker_replay", "runtime_write_performed", "persistent_user_data_touched", "validator_status",
        "quality_status", "limitations", "canonical_sha256",
    }
    application_policy = preview_result["properties"]["application_policy"]["properties"]
    transient_artifact = preview_result["$defs"]["transient_artifact"]["properties"]
    strict_readback = transient_artifact["strict_readback"]["properties"]
    worker_replay = preview_result["$defs"]["worker_replay"]["properties"]
    require(
        preview_request.get("additionalProperties") is False
        and set(preview_request.get("required", [])) == preview_request_fields
        and set(preview_request.get("properties", {})) == preview_request_fields
        and preview_request["properties"]["schema_version"].get("const")
        == "MechanicalPoseGeometryPreviewRequest@1"
        and preview_request["properties"]["pose_evaluation_request"].get("$ref")
        == "https://forgecad.local/contracts/mechanical-pose-evaluation-request.schema.json"
        and preview_request["properties"]["preview_policy"].get("const")
        == "transient-derived-program-worker-readback@1"
        and preview_result.get("additionalProperties") is False
        and set(preview_result.get("required", [])) == preview_result_fields
        and set(preview_result.get("properties", {})) == preview_result_fields
        and preview_result["properties"]["schema_version"].get("const")
        == "MechanicalPoseGeometryPreview@1"
        and preview_result["properties"]["posed_geometry_program"].get("$ref")
        == "https://forgecad.local/contracts/geometry-program-v2.schema.json"
        and application_policy["delta_formula"].get("const")
        == "posed-world-times-inverse-rest-world@1"
        and application_policy["rest_frame_provenance"].get("const")
        == "caller-authored-hash-bound-not-artifact-rig-provenance@1"
        and preview_result["$defs"]["world_vec3"]["items"].get("minimum") == -640
        and preview_result["$defs"]["world_vec3"]["items"].get("maximum") == 640
        and preview_result["$defs"]["worker_vec3"]["items"].get("minimum") == -10
        and preview_result["$defs"]["worker_vec3"]["items"].get("maximum") == 10
        and preview_result["$defs"]["pose"]["properties"]["translation_m"].get("$ref")
        == "#/$defs/world_vec3"
        and preview_result["$defs"]["delta_pose"]["properties"]["translation_m"].get("$ref")
        == "#/$defs/worker_vec3"
        and preview_result["$defs"]["part_delta"]["properties"]["delta_pose"].get("$ref")
        == "#/$defs/delta_pose"
        and preview_result["properties"]["part_deltas"].get("maxItems") == 64
        and transient_artifact["size_bytes"].get("maximum") == 67108864
        and transient_artifact["delivery"].get("const")
        == "hash-and-readback-only-no-cas-object"
        and strict_readback["hard_gate_passed"].get("const") is True
        and worker_replay["byte_exact"].get("const") is True
        and worker_replay["metadata_exact"].get("const") is True
        and preview_result["properties"]["geometry_materialization"].get("const")
        == "transient-worker-glb-not-persisted"
        and preview_result["properties"]["runtime_write_performed"].get("const") is False
        and preview_result["properties"]["persistent_user_data_touched"].get("const") is False
        and preview_result["properties"]["quality_status"].get("const") == "structural_only",
        "MechanicalPoseGeometryPreview@1 must remain closed, bounded, transient and structural-only",
    )


def check_mechanical_animation_clip_contracts() -> None:
    prepare = load_schema("mechanical-animation-clip-prepare-request.schema.json")
    prepare_fields = {
        "schema_version", "clip_id", "pose_sequence_request", "clip_policy", "input_sha256",
    }
    require(
        prepare.get("additionalProperties") is False
        and set(prepare.get("required", [])) == prepare_fields
        and set(prepare.get("properties", {})) == prepare_fields
        and prepare["properties"]["schema_version"].get("const")
        == "MechanicalAnimationClipPrepareRequest@1"
        and prepare["properties"]["pose_sequence_request"].get("$ref")
        == "https://forgecad.local/contracts/mechanical-pose-sequence-preview-request.schema.json"
        and prepare["properties"]["clip_policy"].get("const")
        == "runtime-owned-immutable-cas-rigid-mechanical-action@1",
        "MechanicalAnimationClipPrepareRequest@1 must remain closed and bind one bounded pose sequence",
    )

    clip = load_schema("mechanical-animation-clip.schema.json")
    clip_fields = set(clip.get("required", []))
    sampling = clip["$defs"]["sampling_policy"]["properties"]
    source_replay = clip["$defs"]["source_replay"]["properties"]
    require(
        clip.get("additionalProperties") is False
        and set(clip.get("properties", {})) == clip_fields
        and clip["properties"]["schema_version"].get("const")
        == "MechanicalAnimationClip@1"
        and sampling["timebase_hz"].get("const") == 1000
        and sampling["max_samples"].get("const") == 16
        and sampling["frame_preview_batch_size"].get("const") == 1
        and sampling["sample_time_ticks"].get("maxItems") == 16
        and source_replay["byte_exact_with_candidate_artifact"].get("const") is True
        and source_replay["strict_readback_passed"].get("const") is True
        and clip["properties"]["materialization_status"].get("const")
        == "runtime-owned-immutable-cas-clip"
        and clip["properties"]["quality_status"].get("const") == "structural_only"
        and "no-ik-constraints-nla-fcurves-drivers-or-timeline"
        in clip["properties"]["limitations"].get("const", [])
        and "not-blender-armature-animation-or-python-parity"
        in clip["properties"]["limitations"].get("const", []),
        "MechanicalAnimationClip@1 must remain immutable, source-replayed, bounded and structural-only",
    )

    link = load_schema("mechanical-animation-clip-link.schema.json")
    link_fields = set(link.get("required", []))
    require(
        link.get("additionalProperties") is False
        and set(link.get("properties", {})) == link_fields
        and link["properties"]["schema_version"].get("const")
        == "MechanicalAnimationClipLink@1"
        and link["properties"]["clip"].get("$ref")
        == "https://forgecad.local/contracts/mechanical-animation-clip.schema.json"
        and link["properties"]["materialization_status"].get("const")
        == "runtime-owned-immutable-cas-clip",
        "MechanicalAnimationClipLink@1 must remain an exact closed CAS and SQLite binding",
    )

    get_request = load_schema("mechanical-animation-clip-get-request.schema.json")
    get_fields = {"schema_version", "project_id", "candidate_id", "clip_id", "canonical_sha256"}
    preview_request = load_schema("mechanical-animation-clip-preview-request.schema.json")
    preview_request_fields = get_fields | {"sample_time_ticks", "preview_policy"}
    require(
        get_request.get("additionalProperties") is False
        and set(get_request.get("required", [])) == get_fields
        and set(get_request.get("properties", {})) == get_fields
        and preview_request.get("additionalProperties") is False
        and set(preview_request.get("required", [])) == preview_request_fields
        and set(preview_request.get("properties", {})) == preview_request_fields
        and preview_request["properties"]["preview_policy"].get("const")
        == "single-tick-transient-double-worker-replay@1",
        "Mechanical animation clip reads must remain closed and single-tick bounded",
    )

    inventory_request = load_schema("mechanical-animation-clip-inventory-request.schema.json")
    inventory_request_fields = {
        "schema_version", "project_id", "candidate_id", "artifact_id", "max_clips",
        "canonical_sha256",
    }
    inventory = load_schema("viewer-mechanical-animation-inventory.schema.json")
    inventory_fields = set(inventory.get("required", []))
    require(
        inventory_request.get("additionalProperties") is False
        and set(inventory_request.get("required", [])) == inventory_request_fields
        and set(inventory_request.get("properties", {})) == inventory_request_fields
        and inventory_request["properties"]["max_clips"].get("const") == 16
        and inventory.get("additionalProperties") is False
        and set(inventory.get("properties", {})) == inventory_fields
        and inventory["properties"]["schema_version"].get("const")
        == "ViewerMechanicalAnimationInventory@1"
        and inventory["properties"]["read_only"].get("const") is True
        and inventory["properties"]["runtime_write_performed"].get("const") is False
        and inventory["properties"]["persistent_user_data_touched"].get("const") is False
        and inventory["properties"]["clips"].get("maxItems") == 16
        and inventory["properties"]["quality_status"].get("const") == "structural_only",
        "Viewer mechanical animation inventory must remain closed, bounded and read-only",
    )

    preview = load_schema("mechanical-animation-clip-preview.schema.json")
    preview_fields = set(preview.get("required", []))
    require(
        preview.get("additionalProperties") is False
        and set(preview.get("properties", {})) == preview_fields
        and preview["properties"]["schema_version"].get("const")
        == "MechanicalAnimationClipPreview@1"
        and preview["properties"]["pose_geometry_preview"].get("$ref")
        == "https://forgecad.local/contracts/mechanical-pose-geometry-preview.schema.json"
        and preview["properties"]["geometry_materialization"].get("const")
        == "transient-double-worker-glb-not-persisted"
        and preview["properties"]["runtime_write_performed"].get("const") is False
        and preview["properties"]["persistent_user_data_touched"].get("const") is False
        and preview["properties"]["quality_status"].get("const") == "structural_only",
        "MechanicalAnimationClipPreview@1 must remain transient, read-only and structural-only",
    )

    glb_prepare = load_schema("mechanical-animation-glb-prepare-request.schema.json")
    glb_prepare_fields = {
        "schema_version", "project_id", "candidate_id", "candidate_state_sha256", "clip_id",
        "materialization_policy", "canonical_sha256",
    }
    glb_receipt = load_schema("mechanical-animation-glb-receipt.schema.json")
    glb_result = load_schema("mechanical-animation-glb-prepare-result.schema.json")
    require(
        glb_prepare.get("additionalProperties") is False
        and set(glb_prepare.get("required", [])) == glb_prepare_fields
        and set(glb_prepare.get("properties", {})) == glb_prepare_fields
        and glb_prepare["properties"]["materialization_policy"].get("const")
        == "rigid-node-trs-gltf-linear-scheduled-samples@1"
        and glb_receipt.get("additionalProperties") is False
        and set(glb_receipt.get("properties", {})) == set(glb_receipt.get("required", []))
        and glb_receipt["properties"]["sample_time_ticks"].get("minItems") == 2
        and glb_receipt["properties"]["sample_time_ticks"].get("maxItems") == 16
        and glb_receipt["properties"]["interpolation"].get("const") == "LINEAR"
        and glb_receipt["properties"]["no_skinning"].get("const") is True
        and glb_receipt["properties"]["no_morph_targets"].get("const") is True
        and {
            "candidate_state_sha256", "artifact_readback_sha256",
            "geometry_candidate_evidence_sha256", "program_sha256",
            "operator_catalog_sha256", "readback_config_sha256",
        }.issubset(set(glb_receipt.get("required", [])))
        and glb_receipt["properties"]["quality_status"].get("const") == "structural_only"
        and glb_result["properties"]["receipt"].get("$ref")
        == "https://forgecad.local/contracts/mechanical-animation-glb-receipt.schema.json"
        and glb_result["properties"]["candidate_confirmed"].get("const") is False
        and glb_result["properties"]["export_performed"].get("const") is False,
        "Mechanical rigid animation GLB contracts must remain closed, bounded, unskinned and prepare-only",
    )


def check_viewer_provenance_graph_contracts() -> None:
    request = load_schema("viewer-provenance-graph-request.schema.json")
    request_fields = {
        "schema_version", "project_id", "candidate_id", "candidate_state_sha256",
        "artifact_id", "max_nodes", "max_edges", "canonical_sha256",
    }
    graph = load_schema("viewer-provenance-graph.schema.json")
    graph_fields = set(graph.get("required", []))
    node = graph["$defs"]["node"]
    edge = graph["$defs"]["edge"]
    require(
        request.get("additionalProperties") is False
        and set(request.get("required", [])) == request_fields
        and set(request.get("properties", {})) == request_fields
        and request["properties"]["schema_version"].get("const")
        == "ViewerProvenanceGraphRequest@1"
        and request["properties"]["max_nodes"].get("const") == 64
        and request["properties"]["max_edges"].get("const") == 128,
        "ViewerProvenanceGraphRequest@1 must remain exact-state-bound and bounded",
    )
    require(
        graph.get("additionalProperties") is False
        and set(graph.get("properties", {})) == graph_fields
        and graph["properties"]["schema_version"].get("const")
        == "ViewerProvenanceGraph@1"
        and graph["properties"]["read_only"].get("const") is True
        and graph["properties"]["runtime_write_performed"].get("const") is False
        and graph["properties"]["persistent_user_data_touched"].get("const") is False
        and graph["properties"]["complete"].get("const") is True
        and graph["properties"]["truncated"].get("const") is False
        and graph["properties"]["nodes"].get("maxItems") == 64
        and graph["properties"]["edges"].get("maxItems") == 128
        and graph["properties"]["quality_status"].get("const") == "structural_only"
        and node.get("additionalProperties") is False
        and edge.get("additionalProperties") is False
        and "geometry-evidence" in node["properties"]["kind"].get("enum", [])
        and "feeds" in edge["properties"]["relation"].get("enum", [])
        and "structural-evidence-does-not-prove-visual-quality"
        in graph["properties"]["limitations"].get("const", []),
        "ViewerProvenanceGraph@1 must remain closed, complete-or-fail, read-only and structural-only",
    )


def check_mechanical_animation_clip_v2_contracts() -> None:
    """Keep the appearance-aware clip additive, closed and non-promoting."""
    clip = load_schema("mechanical-animation-clip-v2.schema.json")
    link = load_schema("mechanical-animation-clip-v2-link.schema.json")
    prepare = load_schema("mechanical-animation-clip-v2-prepare-request.schema.json")
    prepare_result = load_schema("mechanical-animation-clip-v2-prepare-result.schema.json")
    get_request = load_schema("mechanical-animation-clip-v2-get-request.schema.json")
    get_result = load_schema("mechanical-animation-clip-v2-get-result.schema.json")
    preview_request = load_schema("mechanical-animation-clip-v2-preview-request.schema.json")
    preview = load_schema("mechanical-animation-clip-v2-preview.schema.json")

    common_bindings = {
        "appearance_candidate_id", "appearance_candidate_state_sha256", "appearance_artifact_id",
        "appearance_artifact_sha256", "appearance_artifact_readback_sha256",
        "appearance_artifact_readback_object_sha256", "source_geometry_candidate_id",
        "source_geometry_candidate_state_sha256", "source_geometry_artifact_id",
        "source_geometry_artifact_sha256", "source_geometry_candidate_evidence_sha256",
        "material_surface_quality_id", "material_surface_quality_report_object_sha256",
        "material_surface_quality_canonical_sha256", "appearance_source_lineage_sidecar_object_sha256",
        "appearance_source_lineage_canonical_sha256", "appearance_program_object_sha256",
        "appearance_program_sha256", "geometry_program_object_sha256", "geometry_program_sha256",
        "geometry_preservation_projection_sha256", "operator_catalog_sha256", "readback_config_sha256",
    }
    structural_flags = {
        "quality_status", "visual_quality_status", "commercial_fps_quality_status",
        "human_review_status", "commercial_engine_status", "runtime_write_performed",
        "production_stage_advanced", "candidate_confirmed", "version_created", "export_performed",
    }

    for schema, label in [
        (clip, "MechanicalAnimationClip@2"),
        (link, "MechanicalAnimationClipLink@2"),
        (prepare, "MechanicalAnimationClipPrepareRequest@2"),
        (prepare_result, "MechanicalAnimationClipPrepareResult@2"),
        (get_request, "MechanicalAnimationClipGetRequest@2"),
        (get_result, "MechanicalAnimationClipGetResult@2"),
        (preview_request, "MechanicalAnimationClipPreviewRequest@2"),
        (preview, "MechanicalAnimationClipPreview@2"),
    ]:
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and schema.get("properties", {}).get("schema_version", {}).get("const") == label,
            f"{label} must remain a closed object contract",
        )

    clip_fields = set(clip["required"])
    require(
        set(clip["properties"]) == clip_fields
        and common_bindings.issubset(clip_fields)
        and {"rest_frame", "pose_action", "sampling_policy", "source_replay", "request_sha256", "clip_id", "canonical_sha256"}.issubset(clip_fields)
        and clip["properties"]["replay_policy"].get("const")
        == "geometry-plus-appearance-double-worker-replay@1"
        and clip["properties"]["materialization_status"].get("const")
        == "runtime-owned-immutable-cas-appearance-aware-clip"
        and clip["properties"]["quality_status"].get("const") == "structural_only"
        and clip["properties"]["visual_quality_status"].get("const") == "NOT_PROVEN"
        and clip["properties"]["commercial_fps_quality_status"].get("const") == "NOT_PROVEN"
        and clip["properties"]["human_review_status"].get("const") == "NOT_RUN"
        and clip["properties"]["commercial_engine_status"].get("const") == "NOT_RUN"
        and clip["properties"]["runtime_write_performed"].get("const") is True
        and all(clip["properties"][field].get("const") is False for field in structural_flags - {"quality_status", "visual_quality_status", "commercial_fps_quality_status", "human_review_status", "commercial_engine_status", "runtime_write_performed"}),
        "MechanicalAnimationClip@2 must bind appearance and geometry evidence while remaining structural-only and non-promoting",
    )

    link_fields = set(link["required"])
    require(
        set(link["properties"]) == link_fields
        and common_bindings.issubset(link_fields)
        and {"clip", "clip_object_sha256", "clip_sha256", "rest_frame_sha256", "pose_action_sha256", "request_sha256"}.issubset(link_fields)
        and link["properties"]["clip"].get("$ref")
        == "https://forgecad.local/contracts/mechanical-animation-clip-v2.schema.json"
        and link["properties"]["replay_policy"].get("const")
        == "geometry-plus-appearance-double-worker-replay@1"
        and link["properties"]["materialization_status"].get("const")
        == "runtime-owned-immutable-cas-appearance-aware-clip"
        and link["properties"]["quality_status"].get("const") == "structural_only",
        "MechanicalAnimationClipLink@2 must be an exact durable appearance/geometry binding",
    )

    prepare_fields = set(prepare["required"])
    require(
        set(prepare["properties"]) == prepare_fields
        and common_bindings.issubset(prepare_fields)
        and {"rest_frame", "pose_action", "sampling_policy", "replay_policy", "input_sha256", "idempotency_key"}.issubset(prepare_fields)
        and prepare["properties"]["replay_policy"].get("const")
        == "geometry-plus-appearance-double-worker-replay@1",
        "MechanicalAnimationClipPrepareRequest@2 must be closed and bind the full appearance-aware source set",
    )

    for result, label, runtime_write in [
        (prepare_result, "MechanicalAnimationClipPrepareResult@2", True),
        (get_result, "MechanicalAnimationClipGetResult@2", False),
    ]:
        fields = set(result["required"])
        require(
            set(result["properties"]) == fields
            and {"clip", "durable_link", "replayed", "restart_hash_verified", "quality_status"}.issubset(fields)
            and result["properties"]["restart_hash_verified"].get("const") is True
            and result["properties"]["runtime_write_performed"].get("const") is runtime_write
            and result["properties"]["production_stage_advanced"].get("const") is False
            and result["properties"]["candidate_confirmed"].get("const") is False
            and result["properties"]["version_created"].get("const") is False
            and result["properties"]["export_performed"].get("const") is False
            and result["properties"]["quality_status"].get("const") == "structural_only",
            f"{label} must be restart-verifiable and non-promoting",
        )

    require(
        set(get_request["properties"]) == set(get_request["required"])
        and set(get_request["required"]) == {"schema_version", "project_id", "appearance_candidate_id", "clip_id"}
        and set(preview_request["properties"]) == set(preview_request["required"])
        and preview_request["properties"]["preview_policy"].get("const")
        == "single-tick-transient-geometry-plus-appearance-double-worker-replay@1"
        and preview_request["properties"]["sample_time_ticks"].get("maximum") == 1000000,
        "MechanicalAnimationClip@2 read requests must be closed and preview must remain one bounded transient tick",
    )

    preview_fields = set(preview["required"])
    appearance_preview_evidence = {
        "appearance_transient_artifact_sha256",
        "appearance_transient_artifact_readback_sha256",
        "appearance_replay_worker_cohort_sha256",
        "appearance_program_sha256",
        "appearance_transient_program_sha256",
        "material_pack_manifest_sha256",
        "geometry_preservation_projection_sha256",
    }
    require(
        set(preview["properties"]) == preview_fields
        and appearance_preview_evidence.issubset(preview_fields)
        and common_bindings.intersection(preview_fields)
        == {
            "appearance_candidate_id", "appearance_candidate_state_sha256", "appearance_artifact_sha256",
            "appearance_artifact_readback_sha256", "appearance_artifact_readback_object_sha256",
            "source_geometry_candidate_id", "source_geometry_candidate_state_sha256",
            "source_geometry_artifact_sha256", "source_geometry_candidate_evidence_sha256",
            "appearance_program_sha256", "geometry_preservation_projection_sha256",
        }
        and preview["properties"]["geometry_materialization"].get("const")
        == "transient-double-worker-glb-not-persisted"
        and preview["properties"]["appearance_materialization"].get("const")
        == "transient-double-worker-appearance-not-persisted"
        and preview["properties"]["runtime_write_performed"].get("const") is False
        and preview["properties"]["persistent_user_data_touched"].get("const") is False
        and preview["properties"]["quality_status"].get("const") == "structural_only"
        and preview["properties"]["visual_quality_status"].get("const") == "NOT_PROVEN"
        and preview["properties"]["commercial_fps_quality_status"].get("const") == "NOT_PROVEN"
        and preview["properties"]["human_review_status"].get("const") == "NOT_RUN"
        and preview["properties"]["commercial_engine_status"].get("const") == "NOT_RUN",
        "MechanicalAnimationClipPreview@2 must be read-only, transient and structural-only",
    )


def check_game_asset_delivery_contracts() -> None:
    expected = {
        "game-asset-delivery-prepare-request.schema.json": "GameAssetDeliveryPrepareRequest@1",
        "game-asset-delivery-manifest.schema.json": "GameAssetDeliveryManifest@1",
        "game-lod-set-receipt.schema.json": "GameLodSetReceipt@1",
        "collision-proxy-set.schema.json": "CollisionProxySet@1",
        "game-engine-import-readiness.schema.json": "GameEngineImportReadiness@1",
        "game-asset-delivery-prepare-result.schema.json": "GameAssetDeliveryPrepareResult@1",
        "game-asset-lod-derive-request.schema.json": "GameAssetLodDeriveRequest@1",
        "game-asset-lod-derive-result.schema.json": "GameAssetLodDeriveResult@1",
    }
    for filename, version in expected.items():
        schema = load_schema(filename)
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and schema.get("properties", {}).get("schema_version", {}).get("const") == version,
            f"{version} must remain a closed object contract",
        )
        require_required(schema, {"schema_version"}, version)

    request = load_schema("game-asset-delivery-prepare-request.schema.json")
    require(
        request["properties"]["lods"].get("minItems") == 3
        and request["properties"]["lods"].get("maxItems") == 3
        and request["properties"]["lod_policy"].get("const")
        == "authored-three-level-part-stable-progressive-triangles@1"
        and request["properties"]["collision_policy"].get("const")
        == "per-part-aabb-box-from-lod2-visual-geometry@1",
        "GameAssetDeliveryPrepareRequest@1 must require exact authored LOD0/1/2 and derived collision policy",
    )
    lod = load_schema("game-lod-set-receipt.schema.json")
    collision = load_schema("collision-proxy-set.schema.json")
    readiness = load_schema("game-engine-import-readiness.schema.json")
    manifest = load_schema("game-asset-delivery-manifest.schema.json")
    result = load_schema("game-asset-delivery-prepare-result.schema.json")
    require(
        lod["properties"]["triangle_policy"].get("const")
        == "lod1-at-most-75pct-lod0-and-lod2-at-most-50pct-lod0@1"
        and collision["properties"]["gameplay_only"].get("const") is True
        and collision["properties"]["physical_properties_included"].get("const") is False
        and "lod_receipt_object_sha256" in collision.get("required", [])
        and "source_artifact_readback_sha256" in collision.get("required", [])
        and "collision_proxy_object_sha256" in readiness.get("required", [])
        and readiness["properties"]["animation_status"].get("enum")
        == ["absent", "lod0-only-strict-rigid-gltf-animation-readback-pass"]
        and readiness["properties"]["actual_engine_roundtrip"].get("const") is False
        and readiness["properties"]["external_uri_count"].get("const") == 0
        and manifest["properties"]["candidate_confirmed"].get("const") is False
        and manifest["properties"]["export_performed"].get("const") is False
        and result["properties"]["candidate_confirmed"].get("const") is False
        and result["properties"]["export_performed"].get("const") is False,
        "game delivery contracts must preserve progressive LOD, gameplay-only collision and honest no-export/no-engine-roundtrip truth",
    )
    derive_request = load_schema("game-asset-lod-derive-request.schema.json")
    derive_result = load_schema("game-asset-lod-derive-result.schema.json")
    derive_level = derive_result["$defs"]["level"]
    derive_readback = derive_result["$defs"]["readback"]
    require(
        set(derive_request.get("required", []))
        == set(derive_request.get("properties", {}))
        and derive_request["properties"]["derive_policy"].get("const")
        == "runtime-owned-typed-segment-lowering-lod1-half-lod2-quarter@1"
        and derive_result["properties"]["levels"].get("minItems") == 3
        and derive_result["properties"]["levels"].get("maxItems") == 3
        and derive_result["properties"]["worker_replay_verified"].get("const") is True
        and derive_result["properties"]["runtime_write_performed"].get("const") is False
        and derive_result["properties"]["persistent_user_data_touched"].get("const") is False
        and derive_result["properties"]["materialization_required"].get("const") is True
        and derive_result["properties"]["quality_status"].get("const") == "structural_only"
        and derive_level.get("additionalProperties") is False
        and derive_level["properties"]["worker_replay_count"].get("const") == 2
        and derive_level["properties"]["replay_byte_exact"].get("const") is True
        and derive_readback.get("additionalProperties") is False
        and derive_readback["properties"]["storage"].get("const") == "memory-only-no-CAS"
        and derive_readback["properties"]["uv_status"].get("const") == "passed"
        and derive_readback["properties"]["tangent_status"].get("const") == "passed"
        and derive_readback["properties"]["external_uri_count"].get("const") == 0
        and derive_readback["properties"]["hard_gate_passed"].get("const") is True,
        "automatic LOD contracts must remain closed, transient, replay-bound, structural-only and write-free",
    )


def check_game_weapon_anchor_contracts() -> None:
    expected = {
        "game-weapon-anchor-prepare-request.schema.json": "GameWeaponAnchorPrepareRequest@1",
        "game-weapon-anchor-set.schema.json": "GameWeaponAnchorSet@1",
        "game-weapon-anchor-link.schema.json": "GameWeaponAnchorLink@1",
        "game-weapon-anchor-prepare-result.schema.json": "GameWeaponAnchorPrepareResult@1",
        "game-weapon-anchor-get-request.schema.json": "GameWeaponAnchorGetRequest@1",
        "game-weapon-anchor-get-result.schema.json": "GameWeaponAnchorGetResult@1",
    }
    for filename, version in expected.items():
        schema = load_schema(filename)
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and schema.get("properties", {}).get("schema_version", {}).get("const") == version
            and set(schema.get("required", [])) == set(schema.get("properties", {})),
            f"{version} must remain a closed exact-field object contract",
        )

    request = load_schema("game-weapon-anchor-prepare-request.schema.json")
    anchor = request["$defs"]["anchor"]
    anchor_set = load_schema("game-weapon-anchor-set.schema.json")
    link = load_schema("game-weapon-anchor-link.schema.json")
    result = load_schema("game-weapon-anchor-prepare-result.schema.json")
    get_result = load_schema("game-weapon-anchor-get-result.schema.json")
    require(
        request["properties"]["anchor_policy"].get("const")
        == "weapon-rh-x-forward-y-up-model-space-six-role@1"
        and request["properties"]["anchors"].get("minItems") == 6
        and request["properties"]["anchors"].get("maxItems") == 6
        and anchor.get("additionalProperties") is False
        and set(anchor.get("required", [])) == set(anchor.get("properties", {}))
        and anchor["properties"]["role"].get("enum")
        == ["weapon-root", "grip-primary", "muzzle-vfx", "magazine-well", "sight-primary", "energy-core-vfx"]
        and anchor["properties"]["local_scale_xyz"].get("const") == [1.0, 1.0, 1.0]
        and anchor_set["properties"]["lod_bindings"].get("minItems") == 3
        and anchor_set["properties"]["lod_bindings"].get("maxItems") == 3
        and anchor_set["properties"]["semantic_scope"].get("const")
        == "fictional-nonfunctional-game-visual-authoring-only@1"
        and anchor_set["properties"]["functional_semantics"].get("const") is False
        and anchor_set["properties"]["limitations"].get("const")
        == ["no-ballistics", "no-damage-or-hitbox", "no-physics-binding", "no-manufacturing-or-operation", "no-commercial-engine-roundtrip"]
        and anchor_set["properties"]["pivot_status"].get("const") == "not-proven-runtime-pivot"
        and anchor_set["properties"]["node_materialization"].get("const") == "sidecar-only-not-glb-nodes"
        and anchor_set["properties"]["candidate_confirmed"].get("const") is False
        and anchor_set["properties"]["export_performed"].get("const") is False
        and anchor_set["properties"]["actual_engine_roundtrip"].get("const") is False
        and link["properties"]["materialization_status"].get("const")
        == "runtime-owned-durable-weapon-anchor-sidecar"
        and result["properties"]["quality_status"].get("const") == "structural_only"
        and get_result["properties"]["restart_hash_verified"].get("const") is True
        and get_result["properties"]["runtime_write_performed"].get("const") is False,
        "weapon anchor contracts must remain six-role, Part-bound, durable, sidecar-only and honest about pivot/engine/quality status",
    )


def check_game_weapon_glb_socket_materialization_contracts() -> None:
    """Keep GLB socket materialization closed, LOD-bound and structural-only."""
    expected = {
        "game-weapon-glb-socket-materialization-prepare-request.schema.json": "GameWeaponGlbSocketMaterializationPrepareRequest@1",
        "game-weapon-glb-socket-materialization-prepare-result.schema.json": "GameWeaponGlbSocketMaterializationPrepareResult@1",
        "game-weapon-glb-socket-materialization-get-request.schema.json": "GameWeaponGlbSocketMaterializationGetRequest@1",
        "game-weapon-glb-socket-materialization-get-result.schema.json": "GameWeaponGlbSocketMaterializationGetResult@1",
        "game-weapon-glb-socket-materialization-receipt.schema.json": "GameWeaponGlbSocketMaterializationReceipt@1",
        "game-weapon-glb-socket-materialization-link.schema.json": "GameWeaponGlbSocketMaterializationLink@1",
    }
    actual = {path.name for path in SCHEMA_ROOT.glob("game-weapon-glb-socket-materialization-*.schema.json")}
    require(actual == set(expected), "GLB socket materialization schema set must contain exactly six V1 contracts")
    for filename, version in expected.items():
        schema = load_schema(filename)
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and schema.get("title") == version
            and schema.get("properties", {}).get("schema_version", {}).get("const") == version
            and set(schema.get("required", [])) == set(schema.get("properties", {})),
            f"{version} must remain a closed exact-field object contract",
        )

    request = load_schema("game-weapon-glb-socket-materialization-prepare-request.schema.json")
    get_request = load_schema("game-weapon-glb-socket-materialization-get-request.schema.json")
    prepare_result = load_schema("game-weapon-glb-socket-materialization-prepare-result.schema.json")
    get_result = load_schema("game-weapon-glb-socket-materialization-get-result.schema.json")
    receipt = load_schema("game-weapon-glb-socket-materialization-receipt.schema.json")
    link = load_schema("game-weapon-glb-socket-materialization-link.schema.json")
    request_fields = {
        "schema_version",
        "project_id",
        "delivery_manifest_object_sha256",
        "anchor_set_object_sha256",
        "materialization_policy",
        "lod_scope",
        "canonical_sha256",
    }
    require(
        set(request["required"]) == request_fields
        and set(request["properties"]) == request_fields
        and request["properties"]["materialization_policy"].get("const")
        == "gltf-anchor-node-materialization-preserve-renderable-content@1"
        and request["properties"]["lod_scope"].get("const") == "lod0-lod1-lod2@1",
        "GLB socket materialization prepare request fields and policies must remain exact",
    )
    require(
        set(get_request["required"]) == {"schema_version", "project_id", "socket_materialization_key_sha256"}
        and set(get_request["properties"]) == set(get_request["required"]),
        "GLB socket materialization get must use only project_id and socket_materialization_key_sha256",
    )
    link_fields = {
        "schema_version",
        "socket_materialization_key_sha256",
        "project_id",
        "delivery_manifest_object_sha256",
        "anchor_set_object_sha256",
        "anchor_set_canonical_sha256",
        "request_sha256",
        "socket_materialization_policy",
        "lod_scope",
        "socket_node_id_encoding_sha256",
        "receipt_object_sha256",
        "materialization_status",
        "canonical_sha256",
        "created_at",
    }
    require(
        set(link["required"]) == link_fields
        and set(link["properties"]) == link_fields
        and link["properties"]["socket_materialization_policy"].get("const")
        == "gltf-anchor-node-materialization-preserve-renderable-content@1"
        and link["properties"]["lod_scope"].get("const") == "lod0-lod1-lod2@1"
        and link["properties"]["materialization_status"].get("const")
        == "runtime-owned-durable-game-weapon-glb-socket-materialization",
        "GLB socket materialization durable link fields and policy must remain exact",
    )
    prepare_result_fields = {
        "schema_version",
        "socket_materialization_key_sha256",
        "receipt_object_sha256",
        "receipt",
        "durable_link",
        "candidate_confirmed",
        "export_performed",
        "actual_engine_roundtrip",
        "quality_status",
    }
    get_result_fields = {
        "schema_version",
        "socket_materialization_key_sha256",
        "receipt_object_sha256",
        "receipt",
        "link",
        "restart_hash_verified",
        "runtime_write_performed",
        "candidate_confirmed",
        "export_performed",
        "actual_engine_roundtrip",
        "quality_status",
    }
    require(
        set(prepare_result["required"]) == prepare_result_fields
        and set(prepare_result["properties"]) == prepare_result_fields
        and prepare_result["properties"]["candidate_confirmed"].get("const") is False
        and prepare_result["properties"]["export_performed"].get("const") is False
        and prepare_result["properties"]["actual_engine_roundtrip"].get("const") is False
        and prepare_result["properties"]["quality_status"].get("const") == "structural_only",
        "GLB socket materialization prepare result must remain structural-only and unconfirmed",
    )
    require(
        set(get_result["required"]) == get_result_fields
        and set(get_result["properties"]) == get_result_fields
        and get_result["properties"]["restart_hash_verified"].get("const") is True
        and get_result["properties"]["runtime_write_performed"].get("const") is False
        and get_result["properties"]["candidate_confirmed"].get("const") is False
        and get_result["properties"]["export_performed"].get("const") is False
        and get_result["properties"]["actual_engine_roundtrip"].get("const") is False
        and get_result["properties"]["quality_status"].get("const") == "structural_only",
        "GLB socket materialization get result must remain restart-verified, read-only and structural-only",
    )
    receipt_fields = {
        "schema_version",
        "socket_materialization_key_sha256",
        "project_id",
        "delivery_manifest_object_sha256",
        "anchor_set_object_sha256",
        "anchor_set_canonical_sha256",
        "request_sha256",
        "socket_materialization_policy",
        "lod_scope",
        "socket_node_id_encoding_sha256",
        "levels",
        "semantic_scope",
        "functional_semantics",
        "materialization_status",
        "runtime_write_performed",
        "candidate_confirmed",
        "export_performed",
        "actual_engine_roundtrip",
        "quality_status",
        "limitations",
        "canonical_sha256",
        "created_at",
    }
    require(
        set(receipt["required"]) == receipt_fields
        and set(receipt["properties"]) == receipt_fields
        and receipt["properties"]["levels"].get("minItems") == 3
        and receipt["properties"]["levels"].get("maxItems") == 3
        and len(receipt["properties"]["levels"].get("prefixItems", [])) == 3
        and receipt["properties"]["levels"].get("items") is False
        and receipt["properties"]["semantic_scope"].get("const")
        == "fictional-nonfunctional-game-visual-authoring-only@1"
        and receipt["properties"]["functional_semantics"].get("const") is False
        and receipt["properties"]["materialization_status"].get("const")
        == "runtime-owned-durable-game-weapon-glb-socket-materialization"
        and receipt["properties"]["runtime_write_performed"].get("const") is True
        and receipt["properties"]["candidate_confirmed"].get("const") is False
        and receipt["properties"]["export_performed"].get("const") is False
        and receipt["properties"]["actual_engine_roundtrip"].get("const") is False
        and receipt["properties"]["quality_status"].get("const") == "structural_only",
        "GLB socket materialization receipt must inline exactly three structural-only LOD readbacks",
    )
    lod = receipt["$defs"]["lod"]
    lod_fields = {
        "schema_version",
        "socket_materialization_key_sha256",
        "lod_level",
        "source_candidate_id",
        "source_candidate_state_sha256",
        "source_artifact_sha256",
        "source_artifact_readback_sha256",
        "derived_artifact_sha256",
        "derived_artifact_readback_sha256",
        "source_renderable_inventory_sha256",
        "derived_renderable_inventory_sha256",
        "socket_node_inventory_sha256",
        "source_bin_sha256",
        "derived_bin_sha256",
        "source_renderable_projection_exact",
        "source_bin_byte_exact",
        "socket_nodes_materialized",
        "source_node_count",
        "derived_node_count",
        "socket_node_count",
        "socket_nodes",
        "canonical_sha256",
    }
    socket_node = receipt["$defs"]["socket_node"]
    socket_node_fields = {
        "socket_node_id",
        "anchor_id",
        "role",
        "node_name",
        "node_kind",
        "parent_kind",
        "parent_node_name",
        "owner_part_id",
        "local_translation_m",
        "local_rotation_quat_xyzw",
        "local_scale_xyz",
    }
    require(
        lod.get("additionalProperties") is False
        and set(lod.get("required", [])) == lod_fields
        and set(lod.get("properties", {})) == lod_fields
        and lod["properties"]["source_renderable_projection_exact"].get("const") is True
        and lod["properties"]["source_bin_byte_exact"].get("const") is True
        and lod["properties"]["socket_nodes_materialized"].get("const") is True
        and lod["properties"]["socket_node_count"].get("const") == 6
        and lod["properties"]["socket_nodes"].get("minItems") == 6
        and lod["properties"]["socket_nodes"].get("maxItems") == 6
        and socket_node.get("additionalProperties") is False
        and set(socket_node.get("required", [])) == socket_node_fields
        and set(socket_node.get("properties", {})) == socket_node_fields
        and socket_node["properties"]["node_kind"].get("const") == "empty"
        and socket_node["properties"]["local_scale_xyz"].get("const") == [1.0, 1.0, 1.0],
        "GLB socket materialization LOD readback must bind source/derived artifacts, BINs and six closed empty socket nodes",
    )


def check_game_weapon_animated_glb_socket_materialization_contracts() -> None:
    """Keep the fixed-LOD0 animated GLB socket materialization closed and source-bound."""
    expected = {
        "game-weapon-animated-glb-socket-materialization-prepare-request.schema.json": "GameWeaponAnimatedGlbSocketMaterializationPrepareRequest@1",
        "game-weapon-animated-glb-socket-materialization-prepare-result.schema.json": "GameWeaponAnimatedGlbSocketMaterializationPrepareResult@1",
        "game-weapon-animated-glb-socket-materialization-get-request.schema.json": "GameWeaponAnimatedGlbSocketMaterializationGetRequest@1",
        "game-weapon-animated-glb-socket-materialization-get-result.schema.json": "GameWeaponAnimatedGlbSocketMaterializationGetResult@1",
        "game-weapon-animated-glb-socket-materialization-receipt.schema.json": "GameWeaponAnimatedGlbSocketMaterializationReceipt@1",
        "game-weapon-animated-glb-socket-materialization-link.schema.json": "GameWeaponAnimatedGlbSocketMaterializationLink@1",
    }
    actual = {
        path.name
        for path in SCHEMA_ROOT.glob("game-weapon-animated-glb-socket-materialization-*.schema.json")
        if "-v2-" not in path.name
    }
    require(actual == set(expected), "animated GLB socket materialization schema set must contain exactly six V1 contracts")
    for filename, version in expected.items():
        schema = load_schema(filename)
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and schema.get("title") == version
            and schema.get("properties", {}).get("schema_version", {}).get("const") == version
            and set(schema.get("required", [])) == set(schema.get("properties", {})),
            f"{version} must remain a closed exact-field object contract",
        )

    request = load_schema(
        "game-weapon-animated-glb-socket-materialization-prepare-request.schema.json"
    )
    prepare_result = load_schema(
        "game-weapon-animated-glb-socket-materialization-prepare-result.schema.json"
    )
    get_request = load_schema(
        "game-weapon-animated-glb-socket-materialization-get-request.schema.json"
    )
    get_result = load_schema(
        "game-weapon-animated-glb-socket-materialization-get-result.schema.json"
    )
    receipt = load_schema(
        "game-weapon-animated-glb-socket-materialization-receipt.schema.json"
    )
    link = load_schema(
        "game-weapon-animated-glb-socket-materialization-link.schema.json"
    )
    request_fields = {
        "schema_version",
        "project_id",
        "delivery_manifest_object_sha256",
        "anchor_set_object_sha256",
        "source_candidate_id",
        "source_candidate_state_sha256",
        "source_animated_artifact_sha256",
        "source_animation_receipt_object_sha256",
        "materialization_policy",
        "canonical_sha256",
    }
    require(
        set(request["required"]) == request_fields
        and set(request["properties"]) == request_fields
        and request["properties"]["materialization_policy"].get("const")
        == "gltf-animated-anchor-node-materialization-preserve-animations-renderable-content@1",
        "animated GLB socket materialization prepare request must bind the candidate, source animated GLB, receipt and AnchorSet",
    )
    require(
        set(get_request["required"])
        == {"schema_version", "project_id", "animated_socket_materialization_key_sha256"}
        and set(get_request["properties"]) == set(get_request["required"]),
        "animated GLB socket materialization get must use only project_id and animated_socket_materialization_key_sha256",
    )

    prepare_result_fields = {
        "schema_version",
        "animated_socket_materialization_key_sha256",
        "derived_animated_socket_artifact_sha256",
        "receipt_object_sha256",
        "receipt",
        "durable_link",
        "runtime_write_performed",
        "candidate_confirmed",
        "export_performed",
        "actual_engine_roundtrip",
        "quality_status",
    }
    get_result_fields = {
        "schema_version",
        "animated_socket_materialization_key_sha256",
        "derived_animated_socket_artifact_sha256",
        "receipt_object_sha256",
        "receipt",
        "link",
        "restart_hash_verified",
        "runtime_write_performed",
        "candidate_confirmed",
        "export_performed",
        "actual_engine_roundtrip",
        "quality_status",
    }
    require(
        set(prepare_result["required"]) == prepare_result_fields
        and set(prepare_result["properties"]) == prepare_result_fields
        and prepare_result["properties"]["receipt"].get("$ref")
        == "game-weapon-animated-glb-socket-materialization-receipt.schema.json"
        and prepare_result["properties"]["durable_link"].get("$ref")
        == "game-weapon-animated-glb-socket-materialization-link.schema.json"
        and prepare_result["properties"]["runtime_write_performed"].get("const") is True
        and prepare_result["properties"]["candidate_confirmed"].get("const") is False
        and prepare_result["properties"]["export_performed"].get("const") is False
        and prepare_result["properties"]["actual_engine_roundtrip"].get("const") is False
        and prepare_result["properties"]["quality_status"].get("const") == "structural_only",
        "animated GLB socket materialization prepare result must expose only its derived GLB, receipt/link and structural truth flags",
    )
    require(
        set(get_result["required"]) == get_result_fields
        and set(get_result["properties"]) == get_result_fields
        and get_result["properties"]["receipt"].get("$ref")
        == "game-weapon-animated-glb-socket-materialization-receipt.schema.json"
        and get_result["properties"]["link"].get("$ref")
        == "game-weapon-animated-glb-socket-materialization-link.schema.json"
        and get_result["properties"]["restart_hash_verified"].get("const") is True
        and get_result["properties"]["runtime_write_performed"].get("const") is False
        and get_result["properties"]["candidate_confirmed"].get("const") is False
        and get_result["properties"]["export_performed"].get("const") is False
        and get_result["properties"]["actual_engine_roundtrip"].get("const") is False
        and get_result["properties"]["quality_status"].get("const") == "structural_only",
        "animated GLB socket materialization get result must remain restart-verified, read-only and structural-only",
    )

    link_fields = {
        "schema_version",
        "animated_socket_materialization_key_sha256",
        "project_id",
        "candidate_id",
        "candidate_state_sha256",
        "delivery_manifest_object_sha256",
        "lod0_artifact_sha256",
        "source_artifact_sha256",
        "source_artifact_readback_sha256",
        "animated_artifact_sha256",
        "animated_artifact_readback_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "anchor_set_object_sha256",
        "anchor_set_canonical_sha256",
        "request_sha256",
        "socket_materialization_policy",
        "lod_scope",
        "socket_node_id_encoding_sha256",
        "derived_animated_socket_artifact_sha256",
        "derived_animated_socket_artifact_readback_sha256",
        "receipt_object_sha256",
        "materialization_status",
        "canonical_sha256",
        "created_at",
    }
    require(
        set(link["required"]) == link_fields
        and set(link["properties"]) == link_fields
        and link["properties"]["socket_materialization_policy"].get("const")
        == "gltf-animated-anchor-node-materialization-preserve-animations-renderable-content@1"
        and link["properties"]["lod_scope"].get("const") == "lod0-animated-source-only@1"
        and link["properties"]["materialization_status"].get("const")
        == "runtime-owned-durable-game-weapon-animated-glb-socket-materialization",
        "animated GLB socket materialization durable link must retain the source pair, LOD0, AnchorSet and exactly two owned outputs",
    )

    receipt_fields = {
        "schema_version",
        "animated_socket_materialization_key_sha256",
        "project_id",
        "candidate_id",
        "candidate_state_sha256",
        "delivery_manifest_object_sha256",
        "lod0_artifact_sha256",
        "source_artifact_sha256",
        "source_artifact_readback_sha256",
        "animated_artifact_sha256",
        "animated_artifact_readback_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "anchor_set_object_sha256",
        "anchor_set_canonical_sha256",
        "request_sha256",
        "socket_materialization_policy",
        "lod_scope",
        "socket_node_id_encoding_sha256",
        "derived_animated_socket_artifact_sha256",
        "derived_animated_socket_artifact_readback_sha256",
        "source_animation_projection_sha256",
        "derived_animation_projection_sha256",
        "source_animation_validation_sha256",
        "derived_animation_validation_sha256",
        "source_renderable_inventory_sha256",
        "derived_renderable_inventory_sha256",
        "source_bin_sha256",
        "derived_bin_sha256",
        "sample_time_ticks",
        "part_ids",
        "sampler_count",
        "channel_count",
        "node_count",
        "source_node_count",
        "derived_node_count",
        "accessor_count_added",
        "buffer_view_count_added",
        "socket_node_inventory_sha256",
        "socket_node_count",
        "socket_nodes",
        "owned_cas_kinds",
        "animations_preserved",
        "channels_preserved",
        "samplers_preserved",
        "renderable_projection_exact",
        "bin_byte_exact",
        "source_static_projection_exact",
        "no_skinning",
        "no_morph_targets",
        "socket_nodes_materialized",
        "runtime_write_performed",
        "restart_hash_verified",
        "candidate_confirmed",
        "export_performed",
        "actual_engine_roundtrip",
        "semantic_scope",
        "functional_semantics",
        "materialization_status",
        "quality_status",
        "limitations",
        "canonical_sha256",
        "created_at",
    }
    require(
        set(receipt["required"]) == receipt_fields
        and set(receipt["properties"]) == receipt_fields
        and receipt["properties"]["socket_materialization_policy"].get("const")
        == "gltf-animated-anchor-node-materialization-preserve-animations-renderable-content@1"
        and receipt["properties"]["lod_scope"].get("const") == "lod0-animated-source-only@1"
        and receipt["properties"]["owned_cas_kinds"].get("const")
        == [
            "game-weapon-animated-glb-socket-materialized-glb",
            "game-weapon-animated-glb-socket-materialization-receipt",
        ]
        and receipt["properties"]["animations_preserved"].get("const") is True
        and receipt["properties"]["channels_preserved"].get("const") is True
        and receipt["properties"]["samplers_preserved"].get("const") is True
        and receipt["properties"]["renderable_projection_exact"].get("const") is True
        and receipt["properties"]["bin_byte_exact"].get("const") is True
        and receipt["properties"]["source_static_projection_exact"].get("const") is True
        and receipt["properties"]["no_skinning"].get("const") is True
        and receipt["properties"]["no_morph_targets"].get("const") is True
        and receipt["properties"]["socket_nodes_materialized"].get("const") is True
        and receipt["properties"]["socket_node_count"].get("const") == 6
        and receipt["properties"]["socket_nodes"].get("minItems") == 6
        and receipt["properties"]["socket_nodes"].get("maxItems") == 6
        and receipt["properties"]["semantic_scope"].get("const")
        == "fictional-nonfunctional-game-visual-authoring-only@1"
        and receipt["properties"]["functional_semantics"].get("const") is False
        and receipt["properties"]["materialization_status"].get("const")
        == "runtime-owned-durable-game-weapon-animated-glb-socket-materialization"
        and receipt["properties"]["runtime_write_performed"].get("const") is True
        and receipt["properties"]["restart_hash_verified"].get("const") is True
        and receipt["properties"]["candidate_confirmed"].get("const") is False
        and receipt["properties"]["export_performed"].get("const") is False
        and receipt["properties"]["actual_engine_roundtrip"].get("const") is False
        and receipt["properties"]["quality_status"].get("const") == "structural_only",
        "animated GLB socket materialization receipt must preserve animation/renderable/BIN truth and remain structural-only",
    )
    socket_node = receipt["$defs"]["socket_node"]
    socket_node_fields = {
        "socket_node_id",
        "anchor_id",
        "role",
        "node_name",
        "node_kind",
        "parent_kind",
        "parent_node_name",
        "owner_part_id",
        "local_translation_m",
        "local_rotation_quat_xyzw",
        "local_scale_xyz",
    }
    require(
        socket_node.get("type") == "object"
        and socket_node.get("additionalProperties") is False
        and set(socket_node.get("required", [])) == socket_node_fields
        and set(socket_node.get("properties", {})) == socket_node_fields
        and socket_node["properties"]["role"].get("enum")
        == ["weapon-root", "grip-primary", "muzzle-vfx", "magazine-well", "sight-primary", "energy-core-vfx"]
        and socket_node["properties"]["node_kind"].get("const") == "empty"
        and socket_node["properties"]["local_scale_xyz"].get("const") == [1.0, 1.0, 1.0],
        "animated GLB socket materialization must use six closed named empty nodes with fixed visual-only roles",
    )


def check_game_weapon_animated_glb_socket_materialization_v2_contracts() -> None:
    """Keep the appearance-aware V2 socket materialization additive and closed."""
    expected = {
        "game-weapon-animated-glb-socket-materialization-v2-prepare-request.schema.json": "GameWeaponAnimatedGlbSocketMaterializationPrepareRequest@2",
        "game-weapon-animated-glb-socket-materialization-v2-prepare-result.schema.json": "GameWeaponAnimatedGlbSocketMaterializationPrepareResult@2",
        "game-weapon-animated-glb-socket-materialization-v2-get-request.schema.json": "GameWeaponAnimatedGlbSocketMaterializationGetRequest@2",
        "game-weapon-animated-glb-socket-materialization-v2-get-result.schema.json": "GameWeaponAnimatedGlbSocketMaterializationGetResult@2",
        "game-weapon-animated-glb-socket-materialization-v2-link.schema.json": "GameWeaponAnimatedGlbSocketMaterializationLink@2",
        "game-weapon-animated-glb-socket-materialization-v2-receipt.schema.json": "GameWeaponAnimatedGlbSocketMaterializationReceipt@2",
    }
    actual = {
        path.name
        for path in SCHEMA_ROOT.glob("game-weapon-animated-glb-socket-materialization-v2-*.schema.json")
    }
    require(actual == set(expected), "animated GLB socket materialization V2 schema set must contain exactly six contracts")
    for filename, version in expected.items():
        schema = load_schema(filename)
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and schema.get("title") == version
            and schema.get("properties", {}).get("schema_version", {}).get("const") == version
            and set(schema.get("required", [])) == set(schema.get("properties", {})),
            f"{version} must remain a closed exact-field object contract",
        )

    request = load_schema(
        "game-weapon-animated-glb-socket-materialization-v2-prepare-request.schema.json"
    )
    request_fields = {
        "schema_version",
        "project_id",
        "appearance_candidate_id",
        "appearance_candidate_state_sha256",
        "clip_id",
        "clip_object_sha256",
        "clip_sha256",
        "appearance_delivery_manifest_object_sha256",
        "anchor_set_object_sha256",
        "anchor_set_canonical_sha256",
        "materialization_policy",
        "input_sha256",
        "idempotency_key",
    }
    require(
        set(request["required"]) == request_fields
        and set(request["properties"]) == request_fields
        and request["properties"]["materialization_policy"].get("const")
        == "appearance-aware-animation-v2-socket-node-materialization-preserve-renderable-content@2",
        "animated GLB socket materialization V2 prepare must bind appearance candidate, Clip@2, delivery and AnchorSet",
    )

    get_request = load_schema(
        "game-weapon-animated-glb-socket-materialization-v2-get-request.schema.json"
    )
    get_fields = {
        "schema_version",
        "project_id",
        "appearance_candidate_id",
        "clip_id",
        "animated_socket_materialization_key_sha256",
    }
    require(
        set(get_request["required"]) == get_fields
        and set(get_request["properties"]) == get_fields,
        "animated GLB socket materialization V2 get must bind project, appearance candidate, Clip@2 and key",
    )

    link = load_schema(
        "game-weapon-animated-glb-socket-materialization-v2-link.schema.json"
    )
    link_fields = {
        "schema_version",
        "animated_socket_materialization_key_sha256",
        "project_id",
        "appearance_candidate_id",
        "appearance_candidate_state_sha256",
        "appearance_delivery_manifest_object_sha256",
        "appearance_artifact_sha256",
        "appearance_artifact_readback_sha256",
        "animation_glb_key_sha256",
        "animated_artifact_sha256",
        "animated_artifact_readback_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "clip_id",
        "clip_object_sha256",
        "clip_sha256",
        "anchor_set_object_sha256",
        "anchor_set_canonical_sha256",
        "request_sha256",
        "socket_materialization_policy",
        "lod_scope",
        "socket_node_id_encoding_sha256",
        "derived_animated_socket_artifact_sha256",
        "derived_animated_socket_artifact_readback_sha256",
        "receipt_object_sha256",
        "validator_status",
        "hard_gate_passed",
        "materialization_status",
        "quality_status",
        "canonical_sha256",
        "created_at",
    }
    require(
        len(link_fields) == 31
        and set(link["required"]) == link_fields
        and set(link["properties"]) == link_fields
        and link["properties"]["socket_materialization_policy"].get("const")
        == "appearance-aware-animation-v2-socket-node-materialization-preserve-renderable-content@2"
        and link["properties"]["lod_scope"].get("const")
        == "lod0-appearance-animated-source-only@2"
        and link["properties"]["validator_status"].get("const")
        == "strict-appearance-aware-animated-glb-socket-materialization-readback-pass"
        and link["properties"]["hard_gate_passed"].get("const") is True
        and link["properties"]["materialization_status"].get("const")
        == "runtime-owned-durable-game-weapon-animated-glb-v2-socket-materialization"
        and link["properties"]["quality_status"].get("const") == "structural_only",
        "animated GLB socket materialization V2 durable link must match the 31-field Store projection",
    )

    receipt = load_schema(
        "game-weapon-animated-glb-socket-materialization-v2-receipt.schema.json"
    )
    receipt_fields = {
        "schema_version",
        "animated_socket_materialization_key_sha256",
        "project_id",
        "appearance_candidate_id",
        "appearance_candidate_state_sha256",
        "appearance_delivery_manifest_object_sha256",
        "appearance_artifact_sha256",
        "appearance_artifact_readback_sha256",
        "animation_glb_key_sha256",
        "animated_artifact_sha256",
        "animated_artifact_readback_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "clip_id",
        "clip_object_sha256",
        "clip_sha256",
        "anchor_set_object_sha256",
        "anchor_set_canonical_sha256",
        "request_sha256",
        "socket_materialization_policy",
        "lod_scope",
        "socket_node_id_encoding_sha256",
        "derived_animated_socket_artifact_sha256",
        "derived_animated_socket_artifact_readback_sha256",
        "source_animation_projection_sha256",
        "derived_animation_projection_sha256",
        "source_animation_validation_sha256",
        "derived_animation_validation_sha256",
        "source_renderable_inventory_sha256",
        "derived_renderable_inventory_sha256",
        "source_bin_sha256",
        "derived_bin_sha256",
        "source_appearance_material_projection_sha256",
        "derived_appearance_material_projection_sha256",
        "sampling_policy_sha256",
        "sample_time_ticks",
        "part_ids",
        "sampler_count",
        "channel_count",
        "node_count",
        "source_node_count",
        "derived_node_count",
        "accessor_count_added",
        "buffer_view_count_added",
        "socket_node_inventory_sha256",
        "socket_node_count",
        "socket_nodes",
        "owned_cas_kinds",
        "animations_preserved",
        "channels_preserved",
        "samplers_preserved",
        "renderable_projection_exact",
        "bin_byte_exact",
        "source_static_projection_exact",
        "appearance_material_projection_exact",
        "material_pack_identity_exact",
        "no_skinning",
        "no_morph_targets",
        "socket_nodes_materialized",
        "runtime_write_performed",
        "restart_hash_verified",
        "candidate_confirmed",
        "version_created",
        "export_performed",
        "production_stage_advanced",
        "actual_engine_roundtrip",
        "semantic_scope",
        "functional_semantics",
        "materialization_status",
        "validator_status",
        "hard_gate_passed",
        "quality_status",
        "visual_quality_status",
        "commercial_fps_quality_status",
        "human_review_status",
        "commercial_engine_status",
        "limitations",
        "canonical_sha256",
        "created_at",
    }
    require(
        set(receipt["required"]) == receipt_fields
        and set(receipt["properties"]) == receipt_fields
        and receipt["properties"]["socket_materialization_policy"].get("const")
        == "appearance-aware-animation-v2-socket-node-materialization-preserve-renderable-content@2"
        and receipt["properties"]["lod_scope"].get("const")
        == "lod0-appearance-animated-source-only@2"
        and receipt["properties"]["owned_cas_kinds"].get("const")
        == [
            "game-weapon-animated-glb-v2-socket-materialized-glb",
            "game-weapon-animated-glb-v2-socket-materialization-receipt",
        ]
        and all(receipt["properties"][field].get("const") is True for field in [
            "animations_preserved", "channels_preserved", "samplers_preserved",
            "renderable_projection_exact", "bin_byte_exact", "source_static_projection_exact",
            "appearance_material_projection_exact", "material_pack_identity_exact",
            "no_skinning", "no_morph_targets", "socket_nodes_materialized",
            "runtime_write_performed", "restart_hash_verified", "hard_gate_passed",
        ])
        and receipt["properties"]["candidate_confirmed"].get("const") is False
        and receipt["properties"]["version_created"].get("const") is False
        and receipt["properties"]["export_performed"].get("const") is False
        and receipt["properties"]["production_stage_advanced"].get("const") is False
        and receipt["properties"]["actual_engine_roundtrip"].get("const") is False
        and receipt["properties"]["quality_status"].get("const") == "structural_only"
        and receipt["properties"]["visual_quality_status"].get("const") == "NOT_PROVEN"
        and receipt["properties"]["commercial_fps_quality_status"].get("const") == "NOT_PROVEN"
        and receipt["properties"]["human_review_status"].get("const") == "NOT_RUN"
        and receipt["properties"]["commercial_engine_status"].get("const") == "NOT_RUN",
        "animated GLB socket materialization V2 receipt must preserve structural truth and fail closed on quality boundaries",
    )
    socket_node = receipt["$defs"]["socket_node"]
    socket_node_fields = {
        "socket_node_id", "anchor_id", "role", "node_name", "node_kind",
        "parent_kind", "parent_node_name", "owner_part_id", "local_translation_m",
        "local_rotation_quat_xyzw", "local_scale_xyz",
    }
    require(
        socket_node.get("type") == "object"
        and socket_node.get("additionalProperties") is False
        and set(socket_node.get("required", [])) == socket_node_fields
        and set(socket_node.get("properties", {})) == socket_node_fields
        and socket_node["properties"]["role"].get("enum")
        == ["weapon-root", "grip-primary", "muzzle-vfx", "magazine-well", "sight-primary", "energy-core-vfx"]
        and socket_node["properties"]["node_kind"].get("const") == "empty"
        and socket_node["properties"]["local_scale_xyz"].get("const") == [1.0, 1.0, 1.0],
        "animated GLB socket materialization V2 must use six closed named empty nodes with fixed visual-only roles",
    )

    prepare_result = load_schema(
        "game-weapon-animated-glb-socket-materialization-v2-prepare-result.schema.json"
    )
    get_result = load_schema(
        "game-weapon-animated-glb-socket-materialization-v2-get-result.schema.json"
    )
    result_fields = {
        "schema_version", "animated_socket_materialization_key_sha256",
        "derived_animated_socket_artifact_sha256", "receipt_object_sha256", "receipt",
        "durable_link", "replayed", "restart_hash_verified", "runtime_write_performed",
        "candidate_confirmed", "version_created", "export_performed",
        "production_stage_advanced", "actual_engine_roundtrip", "quality_status",
    }
    require(
        set(prepare_result["required"]) == result_fields
        and set(prepare_result["properties"]) == result_fields
        and prepare_result["properties"]["receipt"].get("$ref")
        == "game-weapon-animated-glb-socket-materialization-v2-receipt.schema.json"
        and prepare_result["properties"]["durable_link"].get("$ref")
        == "game-weapon-animated-glb-socket-materialization-v2-link.schema.json"
        and prepare_result["properties"]["runtime_write_performed"].get("const") is True
        and set(get_result["required"]) == result_fields
        and set(get_result["properties"]) == result_fields
        and get_result["properties"]["receipt"].get("$ref")
        == "game-weapon-animated-glb-socket-materialization-v2-receipt.schema.json"
        and get_result["properties"]["durable_link"].get("$ref")
        == "game-weapon-animated-glb-socket-materialization-v2-link.schema.json"
        and get_result["properties"]["runtime_write_performed"].get("const") is False
        and get_result["properties"]["restart_hash_verified"].get("const") is True,
        "animated GLB socket materialization V2 results must expose durable_link, replay/restart and structural no-op flags",
    )


def check_godot_game_weapon_import_contracts() -> None:
    """Keep the real Godot headless import receipt aggregate closed and non-fabricated."""
    filename = "godot-game-weapon-import-receipt.schema.json"
    version = "GodotGameWeaponImportReceipt@1"
    schema = load_schema(filename)
    root_fields = {
        "schema_version",
        "godot_game_weapon_import_key_sha256",
        "static_project_id",
        "static_delivery_manifest_object_sha256",
        "static_socket_materialization_key_sha256",
        "static_lod0_derived_artifact_sha256",
        "static_lod1_derived_artifact_sha256",
        "static_lod2_derived_artifact_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_project_id",
        "animated_delivery_manifest_object_sha256",
        "animated_derived_artifact_sha256",
        "collision_proxy_set_canonical_sha256",
        "godot_binary_sha256",
        "godot_version",
        "godot_build_sha256",
        "probe_harness_sha256",
        "harness_policy",
        "canonicalization_policy",
        "godot_process_exit_code",
        "godot_report_sha256",
        "scene_count",
        "scene_projections",
        "lod_triangle_counts",
        "lod_triangles_strictly_decreasing",
        "lod_material_signatures_exact",
        "source_gltf_channel_count",
        "godot_optimized_track_count",
        "cross_loader_semantic_sampling_exact",
        "half_duration_follow_names",
        "collision_proxy_count",
        "godot_collision_shape_count",
        "collision_shape_kind",
        "collision_aabb_sidecar_readback_exact",
        "collision_physics_simulation",
        "hitbox_semantics",
        "actual_godot_headless_import",
        "actual_engine_roundtrip",
        "commercial_engine_roundtrip",
        "unity_status",
        "unreal_status",
        "candidate_confirmed",
        "export_performed",
        "human_review",
        "visual_quality_status",
        "quality_status",
        "semantic_scope",
        "functional_semantics",
        "limitations",
        "canonical_sha256",
        "created_at",
    }
    require(
        schema.get("type") == "object"
        and schema.get("additionalProperties") is False
        and schema.get("title") == version
        and schema.get("properties", {}).get("schema_version", {}).get("const") == version
        and set(schema.get("required", [])) == root_fields
        and set(schema.get("properties", {})) == root_fields,
        f"{version} must remain a closed exact-field aggregate",
    )

    def check_closed_objects(node: object, label: str) -> None:
        if isinstance(node, list):
            for index, value in enumerate(node):
                check_closed_objects(value, f"{label}[{index}]")
            return
        if not isinstance(node, dict):
            return
        if node.get("type") == "object":
            require(
                node.get("additionalProperties") is False,
                f"{version} {label} object must reject unknown fields",
            )
            require(
                set(node.get("required", [])) == set(node.get("properties", {})),
                f"{version} {label} object required/properties must be exact",
            )
        for key, value in node.items():
            check_closed_objects(value, f"{label}.{key}")

    check_closed_objects(schema, "root")
    definitions = schema["$defs"]
    scene = definitions["scene_projection"]
    scene_fields = {
        "scene_role",
        "scene_artifact_sha256",
        "scene_projection_sha256",
        "mesh_count",
        "triangle_count",
        "material_count",
        "material_names",
        "material_signature_sha256",
        "mesh_projection_sha256",
        "material_projection_sha256",
        "socket_node_count",
        "socket_node_inventory_sha256",
        "socket_nodes",
        "scale_trs_projection_sha256",
        "animation",
    }
    require(
        set(scene["required"]) == scene_fields
        and set(scene["properties"]) == scene_fields
        and scene["properties"]["scene_role"].get("enum")
        == ["lod0", "lod1", "lod2", "animated"]
        and scene["properties"]["mesh_count"].get("const") == 5
        and scene["properties"]["socket_node_count"].get("const") == 6
        and scene["properties"]["socket_nodes"].get("minItems") == 6
        and scene["properties"]["socket_nodes"].get("maxItems") == 6,
        "Godot import scene projection must bind five meshes and six sockets",
    )
    animation = definitions["animation_projection"]
    animation_fields = {
        "animation_status",
        "source_gltf_channel_count",
        "godot_optimized_track_count",
        "cross_loader_semantic_sampling_exact",
        "half_duration_follow_names",
        "animation_projection_sha256",
    }
    require(
        set(animation["required"]) == animation_fields
        and set(animation["properties"]) == animation_fields
        and animation["properties"]["source_gltf_channel_count"].get("enum") == [0, 10]
        and animation["properties"]["godot_optimized_track_count"].get("enum") == [0, 2]
        and animation["properties"]["cross_loader_semantic_sampling_exact"].get("const") is True,
        "Godot animation projection must permit only absent or 10-channel/2-track optimized animation",
    )
    socket_node = definitions["socket_node"]
    socket_names = [
        "forgecad-anchor-grip-primary",
        "forgecad-anchor-socket-energy-core-vfx",
        "forgecad-anchor-socket-magazine-well",
        "forgecad-anchor-socket-muzzle-vfx",
        "forgecad-anchor-socket-sight-primary",
        "forgecad-anchor-weapon-root",
    ]
    socket_node_fields = {
        "node_name",
        "parent_node_name",
        "node_kind",
        "local_translation_m",
        "local_rotation_quat_xyzw",
        "local_scale_xyz",
        "non_rendering",
        "parent_local_trs_exact",
    }
    require(
        set(socket_node["required"]) == socket_node_fields
        and set(socket_node["properties"]) == socket_node_fields
        and socket_node["properties"]["node_name"].get("enum") == socket_names
        and socket_node["properties"]["node_kind"].get("const") == "Node3D"
        and socket_node["properties"]["non_rendering"].get("const") is True
        and socket_node["properties"]["parent_local_trs_exact"].get("const") is True
        and socket_node["properties"]["local_scale_xyz"].get("const") == [1.0, 1.0, 1.0],
        "Godot import socket projections must be exact six non-rendering Node3D parent/local TRS records",
    )

    projection_array = schema["properties"]["scene_projections"]
    require(
        projection_array.get("minItems") == 4
        and projection_array.get("maxItems") == 4
        and len(projection_array.get("prefixItems", [])) == 4
        and projection_array.get("items") is False,
        "Godot import receipt must aggregate exactly four imported scene projections",
    )
    require(
        schema["properties"]["scene_count"].get("const") == 4
        and schema["properties"]["lod_triangle_counts"].get("minItems") == 3
        and schema["properties"]["lod_triangle_counts"].get("maxItems") == 3
        and schema["properties"]["lod_triangle_counts"].get("items") is False
        and schema["properties"]["lod_triangles_strictly_decreasing"].get("const") is True
        and schema["properties"]["lod_material_signatures_exact"].get("const") is True,
        "Godot import receipt must enforce three strictly decreasing LOD triangle counts and exact materials",
    )
    require(
        schema["properties"]["source_gltf_channel_count"].get("const") == 10
        and schema["properties"]["godot_optimized_track_count"].get("const") == 2
        and schema["properties"]["cross_loader_semantic_sampling_exact"].get("const") is True
        and schema["properties"]["half_duration_follow_names"].get("const")
        == [
            "forgecad-anchor-socket-energy-core-vfx",
            "forgecad-anchor-socket-magazine-well",
        ],
        "Godot import receipt must bind 10 source channels to two optimized tracks with two sampled followers",
    )
    require(
        schema["properties"]["collision_proxy_count"].get("const") == 5
        and schema["properties"]["godot_collision_shape_count"].get("const") == 5
        and schema["properties"]["collision_shape_kind"].get("const") == "BoxShape3D"
        and schema["properties"]["collision_aabb_sidecar_readback_exact"].get("const") is True
        and schema["properties"]["collision_physics_simulation"].get("const") == "NOT_RUN"
        and schema["properties"]["hitbox_semantics"].get("const") is False,
        "Godot import receipt must verify five BoxShape3D sidecar rows without claiming physics or hitboxes",
    )
    require(
        schema["properties"]["godot_binary_sha256"].get("const")
        == "c7cccbf8fb143e34e02fd6521e09be2c2b974f0d5db080b19071c9c570718ccf"
        and schema["properties"]["godot_build_sha256"].get("const")
        == "c7cccbf8fb143e34e02fd6521e09be2c2b974f0d5db080b19071c9c570718ccf"
        and schema["properties"]["godot_version"].get("const")
        == "4.7.2.stable.official.ed1daf0bf"
        and schema["properties"]["harness_policy"].get("const")
        == "first-party-fixed-godot-headless-import-probe@1"
        and schema["properties"]["canonicalization_policy"].get("const")
        == "canonical-json-sha256-excluding-canonical-sha256-and-created-at@1"
        and schema["properties"]["godot_process_exit_code"].get("const") == 0
        and schema["properties"]["actual_godot_headless_import"].get("const") is True
        and schema["properties"]["actual_engine_roundtrip"].get("const") is True
        and schema["properties"]["commercial_engine_roundtrip"].get("const") is False
        and schema["properties"]["unity_status"].get("const") == "NOT_RUN"
        and schema["properties"]["unreal_status"].get("const") == "NOT_RUN",
        "Godot import receipt must bind the real 4.7.2 process and retain non-commercial engine boundaries",
    )
    require(
        schema["properties"]["candidate_confirmed"].get("const") is False
        and schema["properties"]["export_performed"].get("const") is False
        and schema["properties"]["quality_status"].get("const") == "structural_only"
        and schema["properties"]["functional_semantics"].get("const") is False,
        "Godot import receipt must remain structural-only and unconfirmed",
    )

    forbidden_property_names = {
        "path",
        "file_path",
        "absolute_path",
        "url",
        "script",
        "script_path",
        "command",
        "shell",
        "environment",
        "env",
        "secret",
        "api_key",
        "source_glb_self_check",
        "threejs_roundtrip",
    }
    all_property_names: set[str] = set()

    def collect_property_names(node: object) -> None:
        if isinstance(node, dict):
            properties = node.get("properties")
            if isinstance(properties, dict):
                all_property_names.update(properties)
            for value in node.values():
                collect_property_names(value)
        elif isinstance(node, list):
            for value in node:
                collect_property_names(value)

    collect_property_names(schema)
    require(
        not (all_property_names & forbidden_property_names),
        "Godot import receipt must not expose paths, URLs, scripts, environment values or substitute-loader evidence",
    )


def check_fictional_energy_vfx_contracts() -> None:
    expected = {
        "fictional-energy-vfx-prepare-request.schema.json": "FictionalEnergyVfxPrepareRequest@1",
        "fictional-energy-vfx-profile.schema.json": "FictionalEnergyVfxProfile@1",
        "fictional-energy-vfx-link.schema.json": "FictionalEnergyVfxLink@1",
        "fictional-energy-vfx-prepare-result.schema.json": "FictionalEnergyVfxPrepareResult@1",
        "fictional-energy-vfx-get-request.schema.json": "FictionalEnergyVfxGetRequest@1",
        "fictional-energy-vfx-get-result.schema.json": "FictionalEnergyVfxGetResult@1",
        "fictional-energy-vfx-frame-sample-request.schema.json": "FictionalEnergyVfxFrameSampleRequest@1",
        "fictional-energy-vfx-frame-sample.schema.json": "FictionalEnergyVfxFrameSample@1",
    }
    for filename, version in expected.items():
        schema = load_schema(filename)
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and schema.get("properties", {}).get("schema_version", {}).get("const") == version
            and set(schema.get("required", [])) == set(schema.get("properties", {})),
            f"{version} must remain a closed exact-field object contract",
        )
    request = load_schema("fictional-energy-vfx-prepare-request.schema.json")
    effect = request["$defs"]["effect"]
    profile = load_schema("fictional-energy-vfx-profile.schema.json")
    link = load_schema("fictional-energy-vfx-link.schema.json")
    get_result = load_schema("fictional-energy-vfx-get-result.schema.json")
    frame_request = load_schema("fictional-energy-vfx-frame-sample-request.schema.json")
    frame = load_schema("fictional-energy-vfx-frame-sample.schema.json")
    require(
        request["properties"]["material_pack_id"].get("const")
        == "forgecad-fictional-energy-weapon-2k"
        and request["properties"]["effects"].get("minItems") == 2
        and request["properties"]["effects"].get("maxItems") == 2
        and effect.get("additionalProperties") is False
        and set(effect.get("required", [])) == set(effect.get("properties", {}))
        and effect["properties"]["color_linear_rgb"]["items"].get("maximum") == 1
        and effect["properties"]["emissive_strength_samples"]["items"].get("maximum") == 16
        and profile["properties"]["static_emissive_material_definition_verified"].get("const") is True
        and profile["properties"]["emissive_animation_rendered"].get("const") is False
        and profile["properties"]["bloom_rendered"].get("const") is False
        and profile["properties"]["particles_rendered"].get("const") is False
        and profile["properties"]["trails_rendered"].get("const") is False
        and profile["properties"]["actual_engine_roundtrip"].get("const") is False
        and profile["properties"]["quality_status"].get("const") == "structural_only"
        and link["properties"]["materialization_status"].get("const")
        == "runtime-owned-durable-fictional-energy-vfx-profile"
        and get_result["properties"]["restart_hash_verified"].get("const") is True
        and get_result["properties"]["runtime_write_performed"].get("const") is False
        and frame_request["properties"]["sample_time_ticks"].get("maximum") == 1000000
        and frame_request["properties"]["sampling_policy"].get("const")
        == "integer-tick-linear-once-clamp-loop-modulo-duration@1"
        and frame["properties"]["interpolation"].get("const") == "LINEAR"
        and frame["properties"]["glb_material_zone_binding_verified"].get("const") is False
        and frame["properties"]["emissive_animation_rendered"].get("const") is False
        and frame["properties"]["runtime_write_performed"].get("const") is False
        and frame["properties"]["persistent_user_data_touched"].get("const") is False,
        "fictional energy VFX contracts must remain bounded sampled intent with honest no-render/no-engine truth",
    )


def check_fictional_energy_vfx_animated_socket_particles_sequence_contracts() -> None:
    """Keep projection-driven animated particle sequence contracts closed."""
    expected = {
        "fictional-energy-vfx-animated-socket-particles-sequence.schema.json": "FictionalEnergyVfxAnimatedSocketParticlesSequence@1",
        "fictional-energy-vfx-animated-socket-particles-sequence-prepare-request.schema.json": "FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareRequest@1",
        "fictional-energy-vfx-animated-socket-particles-sequence-prepare-result.schema.json": "FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareResult@1",
        "fictional-energy-vfx-animated-socket-particles-sequence-get-request.schema.json": "FictionalEnergyVfxAnimatedSocketParticlesSequenceGetRequest@1",
        "fictional-energy-vfx-animated-socket-particles-sequence-get-result.schema.json": "FictionalEnergyVfxAnimatedSocketParticlesSequenceGetResult@1",
    }
    actual = {
        path.name
        for path in SCHEMA_ROOT.glob(
            "fictional-energy-vfx-animated-socket-particles-sequence*.schema.json"
        )
        if "-v2" not in path.name
    }
    require(
        actual == set(expected),
        "animated socket particles sequence schema set must contain exactly five V1 contracts",
    )
    for filename, version in expected.items():
        schema = load_schema(filename)
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and schema.get("title") == version
            and schema.get("properties", {}).get("schema_version", {}).get("const")
            == version
            and set(schema.get("required", [])) == set(schema.get("properties", {})),
            f"{version} must remain a closed exact-field object contract",
        )

    sequence = load_schema(
        "fictional-energy-vfx-animated-socket-particles-sequence.schema.json"
    )
    parent_fields = {
        "schema_version",
        "sequence_key_sha256",
        "project_id",
        "candidate_id",
        "candidate_state_sha256",
        "delivery_manifest_object_sha256",
        "source_artifact_sha256",
        "projection_key_sha256",
        "projection_object_sha256",
        "projection_canonical_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_artifact_sha256",
        "animated_socket_anchor_set_object_sha256",
        "animated_socket_anchor_set_canonical_sha256",
        "animation_clip_id",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "socket_node_id_encoding_sha256",
        "socket_roles_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
        "sample_schedule_sha256",
        "sample_count",
        "sample_time_ticks",
        "frame_scope",
        "particles_sequence_policy",
        "emitter_binding_policy",
        "transform_projection_policy",
        "frames",
        "sequence_status",
        "quality_status",
        "visual_quality_status",
        "commercial_fps_quality_status",
        "human_review_status",
        "commercial_engine_status",
        "runtime_write_performed",
        "restart_hash_verified",
        "candidate_confirmed",
        "version_created",
        "export_performed",
        "actual_engine_roundtrip",
        "production_stage_advanced",
        "input_sha256",
        "canonical_sha256",
        "created_at",
    }
    frame_fields = {
        "schema_version",
        "frame_index",
        "sample_time_ticks",
        "projection_frame_canonical_sha256",
        "projection_socket_transform_inventory_sha256",
        "projection_socket_transform_readback_sha256",
        "base_frame_key_sha256",
        "bloom_key_sha256",
        "emitter_socket_bindings_sha256",
        "input_sha256",
        "particle_key_sha256",
        "particle_seed_sha256",
        "render_set_object_sha256",
        "receipt_object_sha256",
        "particle_color_object_sha256",
        "particle_id_object_sha256",
        "particle_depth_object_sha256",
        "canonical_sha256",
        "created_at",
    }
    frame = sequence.get("$defs", {}).get("frame", {})
    properties = sequence.get("properties", {})
    require(
        set(properties) == parent_fields
        and set(sequence.get("required", [])) == parent_fields
        and set(frame.get("properties", {})) == frame_fields
        and set(frame.get("required", [])) == frame_fields
        and frame.get("type") == "object"
        and frame.get("additionalProperties") is False
        and frame["properties"]["schema_version"].get("const")
        == "FictionalEnergyVfxAnimatedSocketParticlesSequenceFrame@1"
        and properties["schema_version"].get("const")
        == "FictionalEnergyVfxAnimatedSocketParticlesSequence@1"
        and properties["sequence_key_sha256"].get("$ref") == "#/$defs/sha256"
        and properties["projection_object_sha256"].get("$ref") == "#/$defs/sha256"
        and properties["projection_canonical_sha256"].get("$ref") == "#/$defs/sha256"
        and properties["frames"].get("minItems") == 1
        and properties["frames"].get("maxItems") == 16
        and properties["sample_count"].get("minimum") == 1
        and properties["sample_count"].get("maximum") == 16
        and properties["sample_time_ticks"].get("items", {}).get("minimum") == 0
        and properties["sample_time_ticks"].get("items", {}).get("maximum") == 1000000
        and properties["sample_time_ticks"].get("uniqueItems") is True,
        "animated socket particles sequence parent/frame fields must remain exact and bounded",
    )
    require(
        "report_object_sha256" not in properties
        and "report_object_sha256" not in frame_fields,
        "animated socket particles sequence must keep the owned report hash out of durable records",
    )
    require(
        properties["frame_scope"].get("const")
        == "lod0-animation-particles-frame-range-1-16@1"
        and properties["particles_sequence_policy"].get("const")
        == "projection-driven-animated-socket-particles@1"
        and properties["emitter_binding_policy"].get("const")
        == "projection-role-muzzle-vfx-energy-core-vfx-to-particle-emitter@1"
        and properties["transform_projection_policy"].get("const")
        == "glb-animation-linear-nlerp-flat-part-hierarchy-composed-world-trs@1"
        and properties["sequence_status"].get("const")
        == "runtime-owned-durable-fictional-energy-vfx-animated-socket-particles-sequence"
        and properties["quality_status"].get("const") == "structural_only"
        and properties["visual_quality_status"].get("const") == "NOT_PROVEN"
        and properties["commercial_fps_quality_status"].get("const") == "NOT_PROVEN"
        and properties["human_review_status"].get("const") == "NOT_RUN"
        and properties["commercial_engine_status"].get("const") == "NOT_RUN"
        and properties["runtime_write_performed"].get("const") is True
        and properties["restart_hash_verified"].get("const") is True
        and properties["candidate_confirmed"].get("const") is False
        and properties["version_created"].get("const") is False
        and properties["export_performed"].get("const") is False
        and properties["actual_engine_roundtrip"].get("const") is False
        and properties["production_stage_advanced"].get("const") is False,
        "animated socket particles sequence must remain structural-only and non-promoting",
    )

    prepare = load_schema(
        "fictional-energy-vfx-animated-socket-particles-sequence-prepare-request.schema.json"
    )
    prepare_fields = parent_fields - {
        "sequence_status",
        "quality_status",
        "visual_quality_status",
        "commercial_fps_quality_status",
        "human_review_status",
        "commercial_engine_status",
        "runtime_write_performed",
        "restart_hash_verified",
        "candidate_confirmed",
        "version_created",
        "export_performed",
        "actual_engine_roundtrip",
        "production_stage_advanced",
        "canonical_sha256",
        "created_at",
    }
    prepare_fields |= {"idempotency_key"}
    prepare_properties = prepare.get("properties", {})
    frame_input = prepare.get("$defs", {}).get("frame_input", {})
    frame_input_fields = {
        "frame_index",
        "sample_time_ticks",
        "projection_frame_canonical_sha256",
        "projection_socket_transform_inventory_sha256",
        "projection_socket_transform_readback_sha256",
        "base_frame_key_sha256",
        "bloom_key_sha256",
    }
    require(
        set(prepare_properties) == prepare_fields
        and set(prepare.get("required", [])) == prepare_fields
        and prepare_properties["schema_version"].get("const")
        == "FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareRequest@1"
        and prepare_properties["frames"].get("items", {}).get("$ref")
        == "#/$defs/frame_input"
        and frame_input.get("type") == "object"
        and frame_input.get("additionalProperties") is False
        and set(frame_input.get("properties", {})) == frame_input_fields
        and set(frame_input.get("required", [])) == frame_input_fields
        and frame_input["properties"]["frame_index"].get("minimum") == 0
        and frame_input["properties"]["frame_index"].get("maximum") == 15
        and frame_input["properties"]["sample_time_ticks"].get("minimum") == 0
        and frame_input["properties"]["sample_time_ticks"].get("maximum") == 1000000
        and prepare_properties["idempotency_key"].get("$ref") == "#/$defs/id",
        "animated socket particles prepare must accept only bounded projection/base/Bloom frame inputs",
    )
    require(
        not ({
            "emitter_socket_bindings_sha256",
            "input_sha256",
            "particle_key_sha256",
            "particle_seed_sha256",
            "render_set_object_sha256",
            "receipt_object_sha256",
            "particle_color_object_sha256",
            "particle_id_object_sha256",
            "particle_depth_object_sha256",
        } & frame_input_fields),
        "animated socket particles prepare negative frame fixture must reject Runtime-derived particle outputs",
    )

    get_request = load_schema(
        "fictional-energy-vfx-animated-socket-particles-sequence-get-request.schema.json"
    )
    get_fields = {"schema_version", "sequence_key_sha256", "project_id", "candidate_id"}
    require(
        set(get_request.get("properties", {})) == get_fields
        and set(get_request.get("required", [])) == get_fields
        and get_request["properties"]["schema_version"].get("const")
        == "FictionalEnergyVfxAnimatedSocketParticlesSequenceGetRequest@1",
        "animated socket particles get request must bind exact key/project/candidate scope",
    )

    result_fields = {
        "schema_version",
        "sequence_key_sha256",
        "sequence",
        "replayed",
        "restart_hash_verified",
        "runtime_write",
        "quality_status",
        "visual_quality_status",
        "commercial_fps_quality_status",
        "human_review_status",
        "commercial_engine_status",
        "actual_engine_roundtrip",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
    }
    for filename, schema_version, runtime_write in [
        (
            "fictional-energy-vfx-animated-socket-particles-sequence-prepare-result.schema.json",
            "FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareResult@1",
            True,
        ),
        (
            "fictional-energy-vfx-animated-socket-particles-sequence-get-result.schema.json",
            "FictionalEnergyVfxAnimatedSocketParticlesSequenceGetResult@1",
            False,
        ),
    ]:
        result = load_schema(filename)
        result_properties = result.get("properties", {})
        require(
            set(result_properties) == result_fields
            and set(result.get("required", [])) == result_fields
            and result_properties["schema_version"].get("const") == schema_version
            and result_properties["sequence"].get("$ref")
            == "fictional-energy-vfx-animated-socket-particles-sequence.schema.json"
            and result_properties["restart_hash_verified"].get("const") is True
            and result_properties["runtime_write"].get("const") is runtime_write
            and result_properties["quality_status"].get("const") == "structural_only"
            and result_properties["visual_quality_status"].get("const") == "NOT_PROVEN"
            and result_properties["commercial_fps_quality_status"].get("const") == "NOT_PROVEN"
            and result_properties["human_review_status"].get("const") == "NOT_RUN"
            and result_properties["commercial_engine_status"].get("const") == "NOT_RUN"
            and result_properties["actual_engine_roundtrip"].get("const") is False
            and result_properties["production_stage_advanced"].get("const") is False
            and result_properties["candidate_confirmed"].get("const") is False
            and result_properties["version_created"].get("const") is False
            and result_properties["export_performed"].get("const") is False,
            f"{schema_version} must remain restart-verified, structural-only and non-promoting",
        )


def check_fictional_energy_vfx_animated_socket_particles_sequence_v2_contracts() -> None:
    """Keep the additive dual-candidate particle sequence contract closed."""
    prefix = "fictional-energy-vfx-animated-socket-particles-sequence-v2"
    expected = {
        f"{prefix}-frame.schema.json": "FictionalEnergyVfxAnimatedSocketParticlesSequenceFrame@2",
        f"{prefix}.schema.json": "FictionalEnergyVfxAnimatedSocketParticlesSequence@2",
        f"{prefix}-prepare-request.schema.json": "FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareRequest@2",
        f"{prefix}-prepare-result.schema.json": "FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareResult@2",
        f"{prefix}-get-request.schema.json": "FictionalEnergyVfxAnimatedSocketParticlesSequenceGetRequest@2",
        f"{prefix}-get-result.schema.json": "FictionalEnergyVfxAnimatedSocketParticlesSequenceGetResult@2",
    }
    actual = {path.name for path in SCHEMA_ROOT.glob(f"{prefix}*.schema.json")}
    require(actual == set(expected), "animated socket particles V2 schema set must contain exactly six contracts")

    frame_fields = [
        "schema_version",
        "frame_index",
        "sample_time_ticks",
        "projection_frame_canonical_sha256",
        "projection_socket_transform_inventory_sha256",
        "projection_socket_transform_readback_sha256",
        "base_frame_key_sha256",
        "bloom_key_sha256",
        "emitter_socket_bindings_sha256",
        "input_sha256",
        "particle_key_sha256",
        "particle_seed_sha256",
        "render_set_object_sha256",
        "receipt_object_sha256",
        "particle_color_object_sha256",
        "particle_id_object_sha256",
        "particle_depth_object_sha256",
        "canonical_sha256",
        "created_at",
    ]
    parent_fields = [
        "schema_version",
        "sequence_key_sha256",
        "project_id",
        "geometry_candidate_id",
        "geometry_candidate_state_sha256",
        "geometry_delivery_manifest_object_sha256",
        "geometry_artifact_sha256",
        "appearance_candidate_id",
        "appearance_candidate_state_sha256",
        "appearance_delivery_manifest_object_sha256",
        "appearance_artifact_sha256",
        "material_surface_quality_id",
        "material_surface_quality_report_object_sha256",
        "material_surface_quality_canonical_sha256",
        "geometry_preservation_projection_sha256",
        "geometry_preservation_status",
        "projection_key_sha256",
        "projection_object_sha256",
        "projection_canonical_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_artifact_sha256",
        "animated_socket_anchor_set_object_sha256",
        "animated_socket_anchor_set_canonical_sha256",
        "appearance_anchor_set_object_sha256",
        "appearance_anchor_set_canonical_sha256",
        "anchor_binding_policy",
        "anchor_binding_sha256",
        "animation_clip_id",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "socket_node_id_encoding_sha256",
        "socket_roles_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
        "sample_schedule_sha256",
        "sample_count",
        "sample_time_ticks",
        "frame_scope",
        "particles_sequence_policy",
        "emitter_binding_policy",
        "transform_projection_policy",
        "frames",
        "sequence_status",
        "quality_status",
        "visual_quality_status",
        "commercial_fps_quality_status",
        "human_review_status",
        "commercial_engine_status",
        "runtime_write_performed",
        "restart_hash_verified",
        "candidate_confirmed",
        "version_created",
        "export_performed",
        "actual_engine_roundtrip",
        "production_stage_advanced",
        "input_sha256",
        "canonical_sha256",
        "created_at",
    ]
    prepare_fields = [
        "schema_version",
        "sequence_key_sha256",
        "project_id",
        "geometry_candidate_id",
        "geometry_candidate_state_sha256",
        "geometry_delivery_manifest_object_sha256",
        "geometry_artifact_sha256",
        "appearance_candidate_id",
        "appearance_candidate_state_sha256",
        "appearance_delivery_manifest_object_sha256",
        "appearance_artifact_sha256",
        "material_surface_quality_id",
        "material_surface_quality_report_object_sha256",
        "material_surface_quality_canonical_sha256",
        "projection_key_sha256",
        "projection_object_sha256",
        "projection_canonical_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_artifact_sha256",
        "animated_socket_anchor_set_object_sha256",
        "animated_socket_anchor_set_canonical_sha256",
        "appearance_anchor_set_object_sha256",
        "appearance_anchor_set_canonical_sha256",
        "anchor_binding_policy",
        "animation_clip_id",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "socket_node_id_encoding_sha256",
        "socket_roles_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
        "sample_schedule_sha256",
        "sample_count",
        "sample_time_ticks",
        "frame_scope",
        "particles_sequence_policy",
        "emitter_binding_policy",
        "transform_projection_policy",
        "frames",
        "input_sha256",
        "idempotency_key",
    ]
    get_fields = [
        "schema_version",
        "sequence_key_sha256",
        "project_id",
        "geometry_candidate_id",
        "appearance_candidate_id",
        "geometry_delivery_manifest_object_sha256",
        "appearance_delivery_manifest_object_sha256",
    ]
    result_fields = [
        "schema_version",
        "sequence_key_sha256",
        "sequence",
        "replayed",
        "restart_hash_verified",
        "runtime_write",
        "quality_status",
        "visual_quality_status",
        "commercial_fps_quality_status",
        "human_review_status",
        "commercial_engine_status",
        "actual_engine_roundtrip",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
    ]

    def closed(filename: str, version: str, fields: list[str]) -> tuple[dict, dict]:
        schema = load_schema(filename)
        properties = schema.get("properties", {})
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and schema.get("title") == version
            and properties.get("schema_version", {}).get("const") == version
            and list(properties) == fields
            and list(schema.get("required", [])) == fields,
            f"{version} must be a closed exact-field object in frozen order",
        )
        return schema, properties

    frame, frame_properties = closed(
        f"{prefix}-frame.schema.json",
        "FictionalEnergyVfxAnimatedSocketParticlesSequenceFrame@2",
        frame_fields,
    )
    require(
        frame_properties["frame_index"].get("minimum") == 0
        and frame_properties["frame_index"].get("maximum") == 15
        and frame_properties["sample_time_ticks"].get("minimum") == 0
        and frame_properties["sample_time_ticks"].get("maximum") == 1000000,
        "animated socket particles V2 frame bounds must remain 16 samples and 1e6 ticks",
    )

    sequence, sequence_properties = closed(
        f"{prefix}.schema.json",
        "FictionalEnergyVfxAnimatedSocketParticlesSequence@2",
        parent_fields,
    )
    frame_def = sequence.get("$defs", {}).get("frame", {})
    require(
        list(frame_def.get("properties", {})) == frame_fields
        and list(frame_def.get("required", [])) == frame_fields
        and frame_def.get("type") == "object"
        and frame_def.get("additionalProperties") is False
        and frame_def["properties"]["schema_version"].get("const")
        == "FictionalEnergyVfxAnimatedSocketParticlesSequenceFrame@2"
        and sequence_properties["frames"].get("items", {}).get("$ref") == "#/$defs/frame"
        and sequence_properties["frames"].get("minItems") == 1
        and sequence_properties["frames"].get("maxItems") == 16
        and sequence_properties["sample_count"].get("minimum") == 1
        and sequence_properties["sample_count"].get("maximum") == 16
        and sequence_properties["sample_time_ticks"].get("uniqueItems") is True,
        "animated socket particles V2 sequence frame definition is not exact/bounded",
    )
    require(
        sequence_properties["geometry_candidate_id"].get("x-forgecad-distinct-from")
        == "appearance_candidate_id"
        and sequence_properties["appearance_candidate_id"].get("x-forgecad-distinct-from")
        == "geometry_candidate_id"
        and sequence_properties["geometry_candidate_id"].get("$ref") == "#/$defs/id"
        and sequence_properties["appearance_candidate_id"].get("$ref") == "#/$defs/id",
        "animated socket particles V2 must declare geometry/appearance candidate inequality",
    )
    require(
        sequence_properties["material_surface_quality_report_object_sha256"].get("$ref")
        == "#/$defs/sha256"
        and "report_object_sha256" not in sequence_properties
        and sequence_properties["geometry_preservation_projection_sha256"].get("$ref")
        == "#/$defs/sha256",
        "animated socket particles V2 must bind the material quality ancestor and omit an owned sequence report",
    )
    require(
        sequence_properties["geometry_preservation_status"].get("const")
        == "source-output-renderable-geometry-byte-exact"
        and sequence_properties["frame_scope"].get("const")
        == "lod0-animation-particles-frame-range-1-16@2"
        and sequence_properties["particles_sequence_policy"].get("const")
        == ANIMATED_SOCKET_PARTICLES_V2_POLICY
        and sequence_properties["particles_sequence_policy"].get("const")
        != ANIMATED_SOCKET_PARTICLES_V1_POLICY
        and sequence_properties["anchor_binding_policy"].get("const")
        == "geometry-appearance-anchor-role-owner-trs-equivalent@1"
        and sequence_properties["emitter_binding_policy"].get("const")
        == "projection-role-muzzle-vfx-energy-core-vfx-to-particle-emitter@1"
        and sequence_properties["transform_projection_policy"].get("const")
        == ANIMATED_SOCKET_PARTICLES_V2_TRANSFORM_PROJECTION_POLICY
        and sequence_properties["transform_projection_policy"].get("const")
        != ANIMATED_SOCKET_PARTICLES_V1_TRANSFORM_PROJECTION_POLICY
        and sequence_properties["sequence_status"].get("const")
        == "runtime-owned-durable-fictional-energy-vfx-animated-socket-particles-sequence-v2",
        "animated socket particles V2 fixed policies drifted",
    )
    require(
        sequence_properties["quality_status"].get("const") == "structural_only"
        and sequence_properties["visual_quality_status"].get("const") == "NOT_PROVEN"
        and sequence_properties["commercial_fps_quality_status"].get("const") == "NOT_PROVEN"
        and sequence_properties["human_review_status"].get("const") == "NOT_RUN"
        and sequence_properties["commercial_engine_status"].get("const") == "NOT_RUN"
        and sequence_properties["runtime_write_performed"].get("const") is True
        and sequence_properties["restart_hash_verified"].get("const") is True
        and sequence_properties["candidate_confirmed"].get("const") is False
        and sequence_properties["version_created"].get("const") is False
        and sequence_properties["export_performed"].get("const") is False
        and sequence_properties["actual_engine_roundtrip"].get("const") is False
        and sequence_properties["production_stage_advanced"].get("const") is False,
        "animated socket particles V2 must remain structural-only and non-promoting",
    )

    prepare, prepare_properties = closed(
        f"{prefix}-prepare-request.schema.json",
        "FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareRequest@2",
        prepare_fields,
    )
    frame_input = prepare.get("$defs", {}).get("frame_input", {})
    frame_input_fields = [
        "frame_index",
        "sample_time_ticks",
        "projection_frame_canonical_sha256",
        "projection_socket_transform_inventory_sha256",
        "projection_socket_transform_readback_sha256",
        "base_frame_key_sha256",
        "bloom_key_sha256",
    ]
    require(
        prepare_properties["frames"].get("items", {}).get("$ref") == "#/$defs/frame_input"
        and list(frame_input.get("properties", {})) == frame_input_fields
        and list(frame_input.get("required", [])) == frame_input_fields
        and frame_input.get("type") == "object"
        and frame_input.get("additionalProperties") is False
        and frame_input["properties"]["frame_index"].get("maximum") == 15
        and frame_input["properties"]["sample_time_ticks"].get("maximum") == 1000000
        and prepare_properties["geometry_candidate_id"].get("x-forgecad-distinct-from")
        == "appearance_candidate_id"
        and prepare_properties["appearance_candidate_id"].get("x-forgecad-distinct-from")
        == "geometry_candidate_id",
        "animated socket particles V2 prepare must reject equal candidates and Runtime-derived frame outputs",
    )
    require(
        prepare_properties["particles_sequence_policy"].get("const")
        == ANIMATED_SOCKET_PARTICLES_V2_POLICY
        and prepare_properties["particles_sequence_policy"].get("const")
        != ANIMATED_SOCKET_PARTICLES_V1_POLICY
        and prepare_properties["transform_projection_policy"].get("const")
        == ANIMATED_SOCKET_PARTICLES_V2_TRANSFORM_PROJECTION_POLICY
        and prepare_properties["transform_projection_policy"].get("const")
        != ANIMATED_SOCKET_PARTICLES_V1_TRANSFORM_PROJECTION_POLICY,
        "animated socket particles V2 prepare must reject V1 policy identities",
    )
    require(
        "geometry_preservation_projection_sha256" not in prepare_properties
        and "geometry_preservation_status" not in prepare_properties
        and "anchor_binding_sha256" not in prepare_properties
        and "canonical_sha256" not in prepare_properties
        and "created_at" not in prepare_properties
        and "report_object_sha256" not in prepare_properties
        and prepare_properties["material_surface_quality_report_object_sha256"].get("$ref")
        == "#/$defs/sha256",
        "animated socket particles V2 prepare must keep derived/report fields closed",
    )

    get_request, get_properties = closed(
        f"{prefix}-get-request.schema.json",
        "FictionalEnergyVfxAnimatedSocketParticlesSequenceGetRequest@2",
        get_fields,
    )
    require(
        get_properties["geometry_candidate_id"].get("x-forgecad-distinct-from")
        == "appearance_candidate_id"
        and get_properties["appearance_candidate_id"].get("x-forgecad-distinct-from")
        == "geometry_candidate_id"
        and get_properties["geometry_delivery_manifest_object_sha256"].get("$ref")
        == "#/$defs/sha256"
        and get_properties["appearance_delivery_manifest_object_sha256"].get("$ref")
        == "#/$defs/sha256",
        "animated socket particles V2 get request must bind both candidate/delivery identities",
    )

    for filename, schema_version, runtime_write in [
        (
            f"{prefix}-prepare-result.schema.json",
            "FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareResult@2",
            True,
        ),
        (
            f"{prefix}-get-result.schema.json",
            "FictionalEnergyVfxAnimatedSocketParticlesSequenceGetResult@2",
            False,
        ),
    ]:
        result, result_properties = closed(filename, schema_version, result_fields)
        require(
            result_properties["sequence"].get("$ref") == f"{prefix}.schema.json"
            and result_properties["restart_hash_verified"].get("const") is True
            and result_properties["runtime_write"].get("const") is runtime_write
            and result_properties["quality_status"].get("const") == "structural_only"
            and result_properties["visual_quality_status"].get("const") == "NOT_PROVEN"
            and result_properties["commercial_fps_quality_status"].get("const") == "NOT_PROVEN"
            and result_properties["human_review_status"].get("const") == "NOT_RUN"
            and result_properties["commercial_engine_status"].get("const") == "NOT_RUN"
            and result_properties["actual_engine_roundtrip"].get("const") is False
            and result_properties["production_stage_advanced"].get("const") is False
            and result_properties["candidate_confirmed"].get("const") is False
            and result_properties["version_created"].get("const") is False
            and result_properties["export_performed"].get("const") is False,
            f"{schema_version} must remain restart-verified, structural-only and non-promoting",
        )

def check_fictional_energy_vfx_animated_socket_trails_sequence_contracts() -> None:
    """Keep the animated socket Trails/TrailsBloom contracts closed and source-bound."""
    trail_expected = {
        "fictional-energy-vfx-animated-socket-trails-sequence.schema.json": "FictionalEnergyVfxAnimatedSocketTrailsSequence@1",
        "fictional-energy-vfx-animated-socket-trails-sequence-prepare-request.schema.json": "FictionalEnergyVfxAnimatedSocketTrailsSequencePrepareRequest@1",
        "fictional-energy-vfx-animated-socket-trails-sequence-prepare-result.schema.json": "FictionalEnergyVfxAnimatedSocketTrailsSequencePrepareResult@1",
        "fictional-energy-vfx-animated-socket-trails-sequence-get-request.schema.json": "FictionalEnergyVfxAnimatedSocketTrailsSequenceGetRequest@1",
        "fictional-energy-vfx-animated-socket-trails-sequence-get-result.schema.json": "FictionalEnergyVfxAnimatedSocketTrailsSequenceGetResult@1",
    }
    bloom_expected = {
        "fictional-energy-vfx-animated-socket-trails-bloom-sequence.schema.json": "FictionalEnergyVfxAnimatedSocketTrailsBloomSequence@1",
        "fictional-energy-vfx-animated-socket-trails-bloom-sequence-prepare-request.schema.json": "FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareRequest@1",
        "fictional-energy-vfx-animated-socket-trails-bloom-sequence-prepare-result.schema.json": "FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareResult@1",
        "fictional-energy-vfx-animated-socket-trails-bloom-sequence-get-request.schema.json": "FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetRequest@1",
        "fictional-energy-vfx-animated-socket-trails-bloom-sequence-get-result.schema.json": "FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetResult@1",
    }
    require(
        {
            path.name
            for path in SCHEMA_ROOT.glob("fictional-energy-vfx-animated-socket-trails-sequence*.schema.json")
            if "sequence-v2" not in path.name
        }
        == set(trail_expected),
        "animated socket Trails schema set must contain exactly five V1 contracts",
    )
    require(
        {
            path.name
            for path in SCHEMA_ROOT.glob("fictional-energy-vfx-animated-socket-trails-bloom-sequence*.schema.json")
            if "sequence-v2" not in path.name
        }
        == set(bloom_expected),
        "animated socket TrailsBloom schema set must contain exactly five V1 contracts",
    )

    def closed(schema: dict, version: str, label: str) -> None:
        properties = schema.get("properties", {})
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and schema.get("title") == version
            and properties.get("schema_version", {}).get("const") == version
            and set(schema.get("required", [])) == set(properties),
            f"{label} must be a closed exact-field object",
        )

    for filename, version in {**trail_expected, **bloom_expected}.items():
        closed(load_schema(filename), version, version)

    lineage_fields = {
        "schema_version",
        "sequence_key_sha256",
        "project_id",
        "candidate_id",
        "candidate_state_sha256",
        "delivery_manifest_object_sha256",
        "source_artifact_sha256",
        "projection_key_sha256",
        "projection_object_sha256",
        "projection_canonical_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_artifact_sha256",
        "animated_socket_anchor_set_object_sha256",
        "animated_socket_anchor_set_canonical_sha256",
        "animation_clip_id",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "socket_node_id_encoding_sha256",
        "socket_roles_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
        "sample_schedule_sha256",
        "sample_count",
        "sample_time_ticks",
        "frame_scope",
    }
    flags = {
        "sequence_status",
        "quality_status",
        "visual_quality_status",
        "commercial_fps_quality_status",
        "human_review_status",
        "commercial_engine_status",
        "runtime_write_performed",
        "restart_hash_verified",
        "candidate_confirmed",
        "version_created",
        "export_performed",
        "actual_engine_roundtrip",
        "production_stage_advanced",
        "input_sha256",
        "canonical_sha256",
        "created_at",
    }
    trail_constants = {
        "trails_sequence_policy",
        "history_policy",
        "history_pre_roll_policy",
        "trail_count",
        "trail_emitter_roles",
    }
    trail_sequence_fields = lineage_fields | trail_constants | {"frames"} | flags
    trail_request_fields = lineage_fields | trail_constants | {"frames", "input_sha256", "idempotency_key"}
    bloom_constants = {
        "trails_bloom_sequence_policy",
        "trail_sequence_key_sha256",
        "trail_sequence_canonical_sha256",
        "trail_key_scope",
        "trail_count",
        "trail_emitter_roles",
        "trail_bloom_profile_sha256",
        "trail_bloom_profile",
    }
    bloom_sequence_fields = lineage_fields | bloom_constants | {"frames"} | flags
    bloom_request_fields = lineage_fields | bloom_constants | {"frames", "input_sha256", "idempotency_key"}

    trail = load_schema("fictional-energy-vfx-animated-socket-trails-sequence.schema.json")
    trail_request = load_schema(
        "fictional-energy-vfx-animated-socket-trails-sequence-prepare-request.schema.json"
    )
    require(
        set(trail["properties"]) == trail_sequence_fields
        and set(trail_request["properties"]) == trail_request_fields
        and trail["properties"]["frame_scope"].get("const")
        == "lod0-animation-trails-source-frames-1-15@1"
        and trail["properties"]["trails_sequence_policy"].get("const")
        == "projection-driven-animated-socket-trails@1"
        and trail["properties"]["history_policy"].get("const")
        == "one-to-eight-oldest-to-newest-ordinal-zero-last-immediate-previous-earlier-particle-frames-plus-current@1"
        and trail["properties"]["history_pre_roll_policy"].get("const")
        == "same-parent-source-frame-zero-is-preroll-output-frames-one-to-fifteen@1"
        and trail["properties"]["trail_count"].get("const") == 2
        and trail["properties"]["trail_emitter_roles"].get("const")
        == ["muzzle-vfx", "energy-core-vfx"]
        and trail["properties"]["frames"].get("minItems") == 1
        and trail["properties"]["frames"].get("maxItems") == 15,
        "animated socket Trails parent must freeze one-parent pre-roll and two-emitter bounds",
    )

    frame = trail["$defs"]["frame"]
    frame_fields = set(frame.get("properties", {}))
    require(
        frame.get("additionalProperties") is False
        and set(frame.get("required", [])) == frame_fields
        and frame_fields
        == {
            "schema_version",
            "frame_index",
            "sample_time_ticks",
            "history_origin",
            "current_projection_frame_index",
            "current_particle_frame_index",
            "current_particle_key_sha256",
            "current_particle_frame_canonical_sha256",
            "current_projection_frame_canonical_sha256",
            "current_projection_socket_transform_inventory_sha256",
            "current_projection_socket_transform_readback_sha256",
            "previous_projection_frame_index",
            "previous_particle_frame_index",
            "previous_particle_sequence_frame_canonical_sha256",
            "previous_projection_frame_canonical_sha256",
            "previous_projection_socket_transform_inventory_sha256",
            "previous_projection_socket_transform_readback_sha256",
            "projection_sample_set_sha256",
            "particle_sequence_key_sha256",
            "base_frame_key_sha256",
            "bloom_key_sha256",
            "camera_object_sha256",
            "camera_identity_sha256",
            "render_profile_sha256",
            "render_worker_build_cohort_sha256",
            "history_samples",
            "trail_count",
            "trail_emitter_roles",
            "trails",
            "trail_key_sha256",
            "trail_seed_sha256",
            "trail_inventory_sha256",
            "trail_id_encoding_sha256",
            "emitter_binding_sha256",
            "trail_color_object_sha256",
            "trail_id_object_sha256",
            "trail_depth_object_sha256",
            "render_set_object_sha256",
            "receipt_object_sha256",
            "canonical_sha256",
            "created_at",
        }
        and frame["properties"]["history_origin"].get("const")
        == "same-parent-sequence-source-frame-zero-preroll@1"
        and frame["properties"]["frame_index"].get("maximum") == 14
        and frame["properties"]["current_projection_frame_index"].get("maximum") == 15
        and frame["properties"]["current_particle_frame_index"].get("maximum") == 15
        and frame["properties"]["history_samples"].get("minItems") == 1
        and frame["properties"]["history_samples"].get("maxItems") == 8
        and frame["properties"]["trails"].get("minItems") == 2
        and frame["properties"]["trails"].get("maxItems") == 2,
        "animated socket Trails frame must bind current/previous sources and closed history",
    )
    history = trail["$defs"]["history_sample"]
    require(
        history.get("additionalProperties") is False
        and set(history.get("required", []))
        == {
            "history_ordinal",
            "projection_key_sha256",
            "projection_frame_index",
            "projection_frame_canonical_sha256",
            "projection_socket_transform_inventory_sha256",
            "projection_socket_transform_readback_sha256",
            "particle_sequence_key_sha256",
            "particle_frame_index",
            "particle_key_sha256",
            "particle_frame_canonical_sha256",
            "sample_time_ticks",
        }
        and history["properties"]["history_ordinal"].get("minimum") == 0
        and history["properties"]["history_ordinal"].get("maximum") == 7,
        "animated socket Trails history rows must be closed projection/particle composite bindings",
    )
    point = trail["$defs"]["trail_point"]
    require(
        point.get("additionalProperties") is False
        and set(point.get("required", []))
        == {
            "source_frame_index",
            "sample_time_ticks",
            "source_particle_key_sha256",
            "source_particle_frame_index",
            "source_particle_id",
            "local_offset_micrometers",
            "world_position_micrometers",
            "depth_micrometers",
        }
        and point["properties"]["source_particle_id"].get("enum") == [10000, 20000]
        and point["properties"]["local_offset_micrometers"].get("minItems") == 3
        and point["properties"]["local_offset_micrometers"].get("maxItems") == 3
        and trail["$defs"]["trail"]["properties"]["points"].get("minItems") == 2
        and trail["$defs"]["trail"]["properties"]["points"].get("maxItems") == 9,
        "animated socket Trails points must carry semantic source/frame/quantized position data",
    )
    frame_input = trail_request["$defs"]["frame_input"]
    frame_input_fields = set(frame_input.get("properties", {}))
    require(
        frame_input.get("additionalProperties") is False
        and set(frame_input.get("required", [])) == frame_input_fields
        and frame_input_fields
        == {
            "frame_index",
            "sample_time_ticks",
            "history_origin",
            "current_projection_frame_index",
            "current_particle_frame_index",
            "current_particle_key_sha256",
            "current_particle_frame_canonical_sha256",
            "current_projection_frame_canonical_sha256",
            "current_projection_socket_transform_inventory_sha256",
            "current_projection_socket_transform_readback_sha256",
            "previous_projection_frame_index",
            "previous_particle_frame_index",
            "previous_particle_sequence_frame_canonical_sha256",
            "previous_projection_frame_canonical_sha256",
            "previous_projection_socket_transform_inventory_sha256",
            "previous_projection_socket_transform_readback_sha256",
            "particle_sequence_key_sha256",
            "base_frame_key_sha256",
            "bloom_key_sha256",
            "camera_object_sha256",
            "camera_identity_sha256",
            "render_profile_sha256",
            "render_worker_build_cohort_sha256",
        }
        and not (
            frame_input_fields
            & {
                "history_samples",
                "projection_sample_set_sha256",
                "trail_key_sha256",
                "trail_seed_sha256",
                "trail_inventory_sha256",
                "emitter_binding_sha256",
                "trail_color_object_sha256",
                "trail_id_object_sha256",
                "trail_depth_object_sha256",
                "render_set_object_sha256",
                "receipt_object_sha256",
            }
        ),
        "animated socket Trails prepare frame ordinals must be 0..14 while source frames are 1..15",
    )
    require(
        trail_request["$defs"]["frame_input"]["properties"]["frame_index"].get("maximum") == 14
        and trail_request["properties"]["sample_count"].get("maximum") == 15,
        "animated socket Trails prepare frames must exclude derived history/CAS outputs",
    )

    for filename, version, runtime_write in [
        (
            "fictional-energy-vfx-animated-socket-trails-sequence-prepare-result.schema.json",
            "FictionalEnergyVfxAnimatedSocketTrailsSequencePrepareResult@1",
            True,
        ),
        (
            "fictional-energy-vfx-animated-socket-trails-sequence-get-result.schema.json",
            "FictionalEnergyVfxAnimatedSocketTrailsSequenceGetResult@1",
            False,
        ),
    ]:
        result = load_schema(filename)
        props = result.get("properties", {})
        require(
            set(props)
            == {
                "schema_version",
                "sequence_key_sha256",
                "sequence",
                "replayed",
                "restart_hash_verified",
                "runtime_write",
                "quality_status",
                "visual_quality_status",
                "commercial_fps_quality_status",
                "human_review_status",
                "commercial_engine_status",
                "actual_engine_roundtrip",
                "production_stage_advanced",
                "candidate_confirmed",
                "version_created",
                "export_performed",
            }
            and props["schema_version"].get("const") == version
            and props["sequence"].get("$ref")
            == "fictional-energy-vfx-animated-socket-trails-sequence.schema.json"
            and props["runtime_write"].get("const") is runtime_write
            and props["quality_status"].get("const") == "structural_only"
            and props["visual_quality_status"].get("const") == "NOT_PROVEN"
            and props["commercial_fps_quality_status"].get("const") == "NOT_PROVEN"
            and props["human_review_status"].get("const") == "NOT_RUN"
            and props["commercial_engine_status"].get("const") == "NOT_RUN"
            and props["restart_hash_verified"].get("const") is True
            and props["actual_engine_roundtrip"].get("const") is False
            and props["production_stage_advanced"].get("const") is False
            and props["candidate_confirmed"].get("const") is False
            and props["version_created"].get("const") is False
            and props["export_performed"].get("const") is False,
            f"{version} must remain restart-verified, structural-only and non-promoting",
        )

    bloom = load_schema("fictional-energy-vfx-animated-socket-trails-bloom-sequence.schema.json")
    bloom_request = load_schema(
        "fictional-energy-vfx-animated-socket-trails-bloom-sequence-prepare-request.schema.json"
    )
    require(
        set(bloom["properties"]) == bloom_sequence_fields
        and set(bloom_request["properties"]) == bloom_request_fields
        and bloom["properties"]["trails_bloom_sequence_policy"].get("const")
        == "projection-driven-animated-socket-trails-bloom@1"
        and bloom["properties"]["frame_scope"].get("const")
        == "lod0-animation-trails-bloom-source-frames-1-15@1"
        and bloom["properties"]["trail_key_scope"].get("const")
        == "animated-socket-trails-sequence-frame-binding@1"
        and bloom["properties"]["trail_count"].get("const") == 2
        and bloom["properties"]["trail_emitter_roles"].get("const")
        == ["muzzle-vfx", "energy-core-vfx"]
        and bloom["properties"]["frames"].get("maxItems") == 15,
        "animated socket TrailsBloom parent must bind the exact Trails sequence",
    )
    bloom_frame = bloom["$defs"]["frame"]
    bloom_frame_fields = set(bloom_frame.get("properties", {}))
    require(
        bloom_frame.get("additionalProperties") is False
        and set(bloom_frame.get("required", [])) == bloom_frame_fields
        and {
            "trail_sequence_key_sha256",
            "trail_sequence_canonical_sha256",
            "trail_frame_canonical_sha256",
            "trail_color_object_sha256",
            "trail_id_object_sha256",
            "trail_depth_object_sha256",
            "base_aov_byte_exact_verified",
            "bloom_pass_byte_exact_reused",
            "particle_passes_byte_exact_reused",
            "trail_passes_byte_exact_reused",
            "trail_emissive_source_object_sha256",
            "trail_bloom_contribution_object_sha256",
        }
        <= bloom_frame_fields,
        "animated socket TrailsBloom frame must bind Trail composite and own only two new passes",
    )
    bloom_input = bloom_request["$defs"]["frame_input"]
    require(
        bloom_input.get("additionalProperties") is False
        and set(bloom_input.get("required", [])) == set(bloom_input.get("properties", {}))
        and set(bloom_input.get("properties", {}))
        == {
            "frame_index",
            "sample_time_ticks",
            "trail_sequence_key_sha256",
            "trail_sequence_canonical_sha256",
            "trail_frame_canonical_sha256",
            "particle_sequence_frame_canonical_sha256",
            "base_frame_key_sha256",
            "bloom_key_sha256",
            "camera_object_sha256",
            "camera_identity_sha256",
            "render_profile_sha256",
            "render_worker_build_cohort_sha256",
        }
        and not (
            set(bloom_input.get("properties", {}))
            & {
                "trail_color_object_sha256",
                "trail_id_object_sha256",
                "trail_depth_object_sha256",
                "trail_bloom_key_sha256",
                "trail_bloom_seed_sha256",
                "trail_emissive_source_object_sha256",
                "trail_bloom_contribution_object_sha256",
            }
        ),
        "animated socket TrailsBloom prepare frames must exclude derived pass keys and CAS outputs",
    )
    profile_fields = {
        "threshold": 1,
        "source_gain": 8,
        "radius_px": 8,
        "intensity": 4,
        "hdr_clamp": 16,
        "blur_passes": 2,
        "kernel": "separable-box-two-pass-fixed-radius@1",
    }
    profile = bloom["$defs"]["trail_bloom_profile"]
    require(
        profile.get("additionalProperties") is False
        and {
            key: value.get("const")
            for key, value in profile.get("properties", {}).items()
        }
        == profile_fields,
        "animated socket TrailsBloom must use the fixed bounded Bloom profile",
    )
    truth = {
        "base_aov_byte_exact_verified": True,
        "base_opaque_depth_byte_exact_reused": True,
        "bloom_pass_byte_exact_reused": True,
        "particle_passes_byte_exact_reused": True,
        "trail_passes_byte_exact_reused": True,
        "base_bloom_mutated": False,
        "particle_passes_mutated": False,
        "trail_passes_mutated": False,
        "trail_bloom_input": True,
        "trail_emissive_source_rendered": True,
        "trail_bloom_contribution_rendered": True,
        "trail_bloom_rendered": True,
    }
    for schema, label in [(bloom["$defs"]["frame"], "frame")]:
        require(
            all(schema["properties"][key].get("const") == value for key, value in truth.items()),
            f"animated socket TrailsBloom {label} truth flags drifted",
        )
    for filename, version, runtime_write in [
        (
            "fictional-energy-vfx-animated-socket-trails-bloom-sequence-prepare-result.schema.json",
            "FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareResult@1",
            True,
        ),
        (
            "fictional-energy-vfx-animated-socket-trails-bloom-sequence-get-result.schema.json",
            "FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetResult@1",
            False,
        ),
    ]:
        result = load_schema(filename)
        props = result.get("properties", {})
        require(
            props["schema_version"].get("const") == version
            and props["sequence"].get("$ref")
            == "fictional-energy-vfx-animated-socket-trails-bloom-sequence.schema.json"
            and props["runtime_write"].get("const") is runtime_write
            and props["quality_status"].get("const") == "structural_only"
            and props["visual_quality_status"].get("const") == "NOT_PROVEN"
            and props["commercial_fps_quality_status"].get("const") == "NOT_PROVEN"
            and props["human_review_status"].get("const") == "NOT_RUN"
            and props["commercial_engine_status"].get("const") == "NOT_RUN"
            and props["restart_hash_verified"].get("const") is True
            and props["actual_engine_roundtrip"].get("const") is False
            and props["production_stage_advanced"].get("const") is False
            and props["candidate_confirmed"].get("const") is False
            and props["version_created"].get("const") is False
            and props["export_performed"].get("const") is False,
            f"{version} must remain restart-verified, structural-only and non-promoting",
        )


def check_fictional_energy_vfx_animated_socket_trails_sequence_v2_contracts() -> None:
    """Keep Trails@2 additive, dual-candidate and Projection/Particles V2-bound."""
    prefix = "fictional-energy-vfx-animated-socket-trails-sequence-v2"
    expected = {
        f"{prefix}-frame.schema.json": "FictionalEnergyVfxAnimatedSocketTrailsSequenceFrame@2",
        f"{prefix}.schema.json": "FictionalEnergyVfxAnimatedSocketTrailsSequence@2",
        f"{prefix}-prepare-request.schema.json": "FictionalEnergyVfxAnimatedSocketTrailsSequencePrepareRequest@2",
        f"{prefix}-prepare-result.schema.json": "FictionalEnergyVfxAnimatedSocketTrailsSequencePrepareResult@2",
        f"{prefix}-get-request.schema.json": "FictionalEnergyVfxAnimatedSocketTrailsSequenceGetRequest@2",
        f"{prefix}-get-result.schema.json": "FictionalEnergyVfxAnimatedSocketTrailsSequenceGetResult@2",
    }
    require(
        {path.name for path in SCHEMA_ROOT.glob(f"{prefix}*.schema.json")} == set(expected),
        "animated socket Trails V2 schema set must contain exactly six additive contracts",
    )

    frame_fields = [
        "schema_version",
        "frame_index",
        "sample_time_ticks",
        "history_origin",
        "current_projection_frame_index",
        "current_particle_frame_index",
        "current_particle_key_sha256",
        "current_particle_frame_canonical_sha256",
        "current_projection_frame_canonical_sha256",
        "current_projection_socket_transform_inventory_sha256",
        "current_projection_socket_transform_readback_sha256",
        "previous_projection_frame_index",
        "previous_particle_frame_index",
        "previous_particle_sequence_frame_canonical_sha256",
        "previous_projection_frame_canonical_sha256",
        "previous_projection_socket_transform_inventory_sha256",
        "previous_projection_socket_transform_readback_sha256",
        "projection_sample_set_sha256",
        "particle_sequence_key_sha256",
        "base_frame_key_sha256",
        "bloom_key_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
        "history_samples",
        "trail_count",
        "trail_emitter_roles",
        "trails",
        "trail_key_sha256",
        "trail_seed_sha256",
        "trail_inventory_sha256",
        "trail_id_encoding_sha256",
        "emitter_binding_sha256",
        "trail_color_object_sha256",
        "trail_id_object_sha256",
        "trail_depth_object_sha256",
        "render_set_object_sha256",
        "receipt_object_sha256",
        "canonical_sha256",
        "created_at",
    ]
    parent_fields = [
        "schema_version",
        "sequence_key_sha256",
        "project_id",
        "geometry_candidate_id",
        "geometry_candidate_state_sha256",
        "geometry_delivery_manifest_object_sha256",
        "geometry_artifact_sha256",
        "appearance_candidate_id",
        "appearance_candidate_state_sha256",
        "appearance_delivery_manifest_object_sha256",
        "appearance_artifact_sha256",
        "material_surface_quality_id",
        "material_surface_quality_report_object_sha256",
        "material_surface_quality_canonical_sha256",
        "geometry_preservation_projection_sha256",
        "geometry_preservation_status",
        "projection_key_sha256",
        "projection_object_sha256",
        "projection_canonical_sha256",
        "particle_sequence_key_sha256",
        "particle_sequence_canonical_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_artifact_sha256",
        "animated_socket_anchor_set_object_sha256",
        "animated_socket_anchor_set_canonical_sha256",
        "appearance_anchor_set_object_sha256",
        "appearance_anchor_set_canonical_sha256",
        "anchor_binding_policy",
        "anchor_binding_sha256",
        "animation_clip_id",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "socket_node_id_encoding_sha256",
        "socket_roles_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
        "sample_schedule_sha256",
        "sample_count",
        "sample_time_ticks",
        "frame_scope",
        "trails_sequence_policy",
        "history_policy",
        "history_pre_roll_policy",
        "trail_count",
        "trail_emitter_roles",
        "frames",
        "sequence_status",
        "quality_status",
        "visual_quality_status",
        "commercial_fps_quality_status",
        "human_review_status",
        "commercial_engine_status",
        "runtime_write_performed",
        "restart_hash_verified",
        "candidate_confirmed",
        "version_created",
        "export_performed",
        "actual_engine_roundtrip",
        "production_stage_advanced",
        "input_sha256",
        "canonical_sha256",
        "created_at",
    ]
    frame_input_fields = [
        "frame_index",
        "sample_time_ticks",
        "history_origin",
        "current_projection_frame_index",
        "current_particle_frame_index",
        "current_particle_key_sha256",
        "current_particle_frame_canonical_sha256",
        "current_projection_frame_canonical_sha256",
        "current_projection_socket_transform_inventory_sha256",
        "current_projection_socket_transform_readback_sha256",
        "previous_projection_frame_index",
        "previous_particle_frame_index",
        "previous_particle_sequence_frame_canonical_sha256",
        "previous_projection_frame_canonical_sha256",
        "previous_projection_socket_transform_inventory_sha256",
        "previous_projection_socket_transform_readback_sha256",
        "particle_sequence_key_sha256",
        "base_frame_key_sha256",
        "bloom_key_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
    ]
    prepare_fields = [
        "schema_version",
        "sequence_key_sha256",
        "project_id",
        "geometry_candidate_id",
        "geometry_candidate_state_sha256",
        "geometry_delivery_manifest_object_sha256",
        "geometry_artifact_sha256",
        "appearance_candidate_id",
        "appearance_candidate_state_sha256",
        "appearance_delivery_manifest_object_sha256",
        "appearance_artifact_sha256",
        "material_surface_quality_id",
        "material_surface_quality_report_object_sha256",
        "material_surface_quality_canonical_sha256",
        "projection_key_sha256",
        "projection_object_sha256",
        "projection_canonical_sha256",
        "particle_sequence_key_sha256",
        "particle_sequence_canonical_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_artifact_sha256",
        "animated_socket_anchor_set_object_sha256",
        "animated_socket_anchor_set_canonical_sha256",
        "appearance_anchor_set_object_sha256",
        "appearance_anchor_set_canonical_sha256",
        "anchor_binding_policy",
        "animation_clip_id",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "socket_node_id_encoding_sha256",
        "socket_roles_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
        "sample_schedule_sha256",
        "sample_count",
        "sample_time_ticks",
        "frame_scope",
        "trails_sequence_policy",
        "history_policy",
        "history_pre_roll_policy",
        "trail_count",
        "trail_emitter_roles",
        "frames",
        "input_sha256",
        "idempotency_key",
    ]
    get_fields = [
        "schema_version",
        "sequence_key_sha256",
        "project_id",
        "geometry_candidate_id",
        "appearance_candidate_id",
        "geometry_delivery_manifest_object_sha256",
        "appearance_delivery_manifest_object_sha256",
    ]
    result_fields = [
        "schema_version",
        "sequence_key_sha256",
        "sequence",
        "replayed",
        "restart_hash_verified",
        "runtime_write",
        "quality_status",
        "visual_quality_status",
        "commercial_fps_quality_status",
        "human_review_status",
        "commercial_engine_status",
        "actual_engine_roundtrip",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
    ]

    def closed(filename: str, version: str, fields: list[str]) -> tuple[dict, dict]:
        schema = load_schema(filename)
        properties = schema.get("properties", {})
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and schema.get("title") == version
            and properties.get("schema_version", {}).get("const") == version
            and list(properties) == fields
            and list(schema.get("required", [])) == fields,
            f"{version} must be a closed exact-field object in frozen order",
        )
        return schema, properties

    frame, frame_properties = closed(
        f"{prefix}-frame.schema.json",
        "FictionalEnergyVfxAnimatedSocketTrailsSequenceFrame@2",
        frame_fields,
    )
    require(
        frame_properties["frame_index"].get("minimum") == 0
        and frame_properties["frame_index"].get("maximum") == 14
        and frame_properties["current_projection_frame_index"].get("maximum") == 15
        and frame_properties["current_particle_frame_index"].get("maximum") == 15
        and frame_properties["previous_projection_frame_index"].get("maximum") == 15
        and frame_properties["previous_particle_frame_index"].get("maximum") == 15
        and frame_properties["history_samples"].get("minItems") == 1
        and frame_properties["history_samples"].get("maxItems") == 8
        and frame_properties["trails"].get("minItems") == 2
        and frame_properties["trails"].get("maxItems") == 2
        and frame_properties["history_origin"].get("const")
        == ANIMATED_SOCKET_TRAILS_V2_HISTORY_PREROLL_POLICY,
        "animated socket Trails V2 frame must retain explicit frame-zero pre-roll and bounded history",
    )
    history = frame["$defs"]["history_sample"]
    history_fields = [
        "history_ordinal",
        "projection_key_sha256",
        "projection_frame_index",
        "projection_frame_canonical_sha256",
        "projection_socket_transform_inventory_sha256",
        "projection_socket_transform_readback_sha256",
        "particle_sequence_key_sha256",
        "particle_frame_index",
        "particle_key_sha256",
        "particle_frame_canonical_sha256",
        "sample_time_ticks",
    ]
    require(
        history.get("additionalProperties") is False
        and list(history.get("properties", {})) == history_fields
        and list(history.get("required", [])) == history_fields
        and history["properties"]["history_ordinal"].get("maximum") == 7
        and history["properties"]["projection_frame_index"].get("maximum") == 15
        and history["properties"]["particle_frame_index"].get("maximum") == 15,
        "animated socket Trails V2 history rows must be closed Projection@2/Particles@2 bindings",
    )
    trail_point = frame["$defs"]["trail_point"]
    require(
        trail_point.get("additionalProperties") is False
        and list(trail_point.get("properties", {}))
        == [
            "source_frame_index",
            "sample_time_ticks",
            "source_particle_key_sha256",
            "source_particle_frame_index",
            "source_particle_id",
            "local_offset_micrometers",
            "world_position_micrometers",
            "depth_micrometers",
        ]
        and trail_point["properties"]["source_particle_id"].get("enum") == [10000, 20000]
        and trail_point["properties"]["depth_micrometers"].get("type") == "integer"
        and trail_point["properties"]["depth_micrometers"].get("minimum") == 0
        and frame["$defs"]["trail"]["properties"]["points"].get("minItems") == 2
        and frame["$defs"]["trail"]["properties"]["points"].get("maxItems") == 9,
        "animated socket Trails V2 points must remain bounded and semantic",
    )

    sequence, sequence_properties = closed(
        f"{prefix}.schema.json",
        "FictionalEnergyVfxAnimatedSocketTrailsSequence@2",
        parent_fields,
    )
    require(
        sequence_properties["geometry_candidate_id"].get("x-forgecad-distinct-from")
        == "appearance_candidate_id"
        and sequence_properties["appearance_candidate_id"].get("x-forgecad-distinct-from")
        == "geometry_candidate_id"
        and sequence_properties["frames"].get("items", {}).get("$ref") == "#/$defs/frame"
        and sequence_properties["frames"].get("minItems") == 1
        and sequence_properties["frames"].get("maxItems") == 15
        and sequence_properties["sample_count"].get("minimum") == 1
        and sequence_properties["sample_count"].get("maximum") == 15
        and sequence_properties["sample_time_ticks"].get("uniqueItems") is True,
        "animated socket Trails V2 parent frame bounds are not closed",
    )
    require(
        sequence_properties["frame_scope"].get("const") == ANIMATED_SOCKET_TRAILS_V2_FRAME_SCOPE
        and sequence_properties["trails_sequence_policy"].get("const")
        == ANIMATED_SOCKET_TRAILS_V2_POLICY
        and sequence_properties["trails_sequence_policy"].get("const")
        != "projection-driven-animated-socket-trails@1"
        and sequence_properties["history_policy"].get("const")
        == ANIMATED_SOCKET_TRAILS_V2_HISTORY_POLICY
        and sequence_properties["history_pre_roll_policy"].get("const")
        == ANIMATED_SOCKET_TRAILS_V2_HISTORY_PREROLL_POLICY
        and sequence_properties["geometry_preservation_status"].get("const")
        == "source-output-renderable-geometry-byte-exact"
        and sequence_properties["anchor_binding_policy"].get("const")
        == "geometry-appearance-anchor-role-owner-trs-equivalent@1"
        and sequence_properties["sequence_status"].get("const")
        == "runtime-owned-durable-fictional-energy-vfx-animated-socket-trails-sequence-v2",
        "animated socket Trails V2 policies must be Projection@2/Particles@2-only",
    )
    require(
        sequence_properties["particle_sequence_key_sha256"].get("$ref") == "#/$defs/sha256"
        and sequence_properties["particle_sequence_canonical_sha256"].get("$ref")
        == "#/$defs/sha256"
        and "particle_sequence_object_sha256" not in sequence_properties
        and "trail_sequence_object_sha256" not in sequence_properties,
        "animated socket Trails V2 must not invent a particle/trail CAS object",
    )
    require(
        sequence_properties["quality_status"].get("const") == "structural_only"
        and sequence_properties["visual_quality_status"].get("const") == "NOT_PROVEN"
        and sequence_properties["commercial_fps_quality_status"].get("const") == "NOT_PROVEN"
        and sequence_properties["human_review_status"].get("const") == "NOT_RUN"
        and sequence_properties["commercial_engine_status"].get("const") == "NOT_RUN"
        and sequence_properties["runtime_write_performed"].get("const") is True
        and sequence_properties["restart_hash_verified"].get("const") is True
        and sequence_properties["candidate_confirmed"].get("const") is False
        and sequence_properties["version_created"].get("const") is False
        and sequence_properties["export_performed"].get("const") is False
        and sequence_properties["actual_engine_roundtrip"].get("const") is False
        and sequence_properties["production_stage_advanced"].get("const") is False,
        "animated socket Trails V2 must remain structural-only and non-promoting",
    )

    prepare, prepare_properties = closed(
        f"{prefix}-prepare-request.schema.json",
        "FictionalEnergyVfxAnimatedSocketTrailsSequencePrepareRequest@2",
        prepare_fields,
    )
    frame_input = prepare["$defs"]["frame_input"]
    require(
        prepare_properties["frames"].get("items", {}).get("$ref") == "#/$defs/frame_input"
        and frame_input.get("additionalProperties") is False
        and list(frame_input.get("properties", {})) == frame_input_fields
        and list(frame_input.get("required", [])) == frame_input_fields
        and frame_input["properties"]["frame_index"].get("maximum") == 14
        and frame_input["properties"]["current_projection_frame_index"].get("maximum") == 15
        and frame_input["properties"]["current_particle_frame_index"].get("maximum") == 15
        and prepare_properties["geometry_candidate_id"].get("x-forgecad-distinct-from")
        == "appearance_candidate_id"
        and prepare_properties["appearance_candidate_id"].get("x-forgecad-distinct-from")
        == "geometry_candidate_id",
        "animated socket Trails V2 prepare must accept only closed source bindings",
    )
    require(
        prepare_properties["trails_sequence_policy"].get("const")
        == ANIMATED_SOCKET_TRAILS_V2_POLICY
        and prepare_properties["history_policy"].get("const")
        == ANIMATED_SOCKET_TRAILS_V2_HISTORY_POLICY
        and prepare_properties["history_pre_roll_policy"].get("const")
        == ANIMATED_SOCKET_TRAILS_V2_HISTORY_PREROLL_POLICY
        and "geometry_preservation_projection_sha256" not in prepare_properties
        and "geometry_preservation_status" not in prepare_properties
        and "anchor_binding_sha256" not in prepare_properties
        and "canonical_sha256" not in prepare_properties
        and "created_at" not in prepare_properties
        and "trail_key_sha256" not in frame_input["properties"]
        and "trail_color_object_sha256" not in frame_input["properties"],
        "animated socket Trails V2 prepare must exclude derived trail/history/CAS outputs",
    )

    get_request, get_properties = closed(
        f"{prefix}-get-request.schema.json",
        "FictionalEnergyVfxAnimatedSocketTrailsSequenceGetRequest@2",
        get_fields,
    )
    require(
        get_properties["geometry_candidate_id"].get("x-forgecad-distinct-from")
        == "appearance_candidate_id"
        and get_properties["appearance_candidate_id"].get("x-forgecad-distinct-from")
        == "geometry_candidate_id",
        "animated socket Trails V2 get must bind both candidates and deliveries",
    )
    for filename, schema_version, runtime_write in [
        (
            f"{prefix}-prepare-result.schema.json",
            "FictionalEnergyVfxAnimatedSocketTrailsSequencePrepareResult@2",
            True,
        ),
        (
            f"{prefix}-get-result.schema.json",
            "FictionalEnergyVfxAnimatedSocketTrailsSequenceGetResult@2",
            False,
        ),
    ]:
        result, result_properties = closed(filename, schema_version, result_fields)
        require(
            result_properties["sequence"].get("$ref")
            == f"{prefix}.schema.json"
            and result_properties["restart_hash_verified"].get("const") is True
            and result_properties["runtime_write"].get("const") is runtime_write
            and result_properties["quality_status"].get("const") == "structural_only"
            and result_properties["visual_quality_status"].get("const") == "NOT_PROVEN"
            and result_properties["commercial_fps_quality_status"].get("const") == "NOT_PROVEN"
            and result_properties["human_review_status"].get("const") == "NOT_RUN"
            and result_properties["commercial_engine_status"].get("const") == "NOT_RUN"
            and result_properties["actual_engine_roundtrip"].get("const") is False
            and result_properties["production_stage_advanced"].get("const") is False
            and result_properties["candidate_confirmed"].get("const") is False
            and result_properties["version_created"].get("const") is False
            and result_properties["export_performed"].get("const") is False,
            f"{schema_version} must remain restart-verified, structural-only and non-promoting",
        )




def check_fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_contracts() -> None:
    """Keep TrailsBloom@2 additive, terminal-Trails-bound and dual-candidate."""
    prefix = "fictional-energy-vfx-animated-socket-trails-bloom-sequence-v2"
    expected = {
        prefix + "-frame.schema.json": "FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceFrame@2",
        prefix + ".schema.json": "FictionalEnergyVfxAnimatedSocketTrailsBloomSequence@2",
        prefix + "-prepare-request.schema.json": "FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareRequest@2",
        prefix + "-prepare-result.schema.json": "FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareResult@2",
        prefix + "-get-request.schema.json": "FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetRequest@2",
        prefix + "-get-result.schema.json": "FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetResult@2",
    }
    require(
        {path.name for path in SCHEMA_ROOT.glob(prefix + "*.schema.json")} == set(expected),
        "animated socket TrailsBloom V2 schema set must contain exactly six additive contracts",
    )

    frame_fields = """
        schema_version frame_index sample_time_ticks trail_frame_index
        trail_sequence_key_sha256 trail_sequence_canonical_sha256 trail_frame_canonical_sha256
        trail_key_sha256 trail_inventory_sha256 trail_id_encoding_sha256 emitter_binding_sha256
        trail_color_object_sha256 trail_id_object_sha256 trail_depth_object_sha256
        particle_sequence_key_sha256 particle_sequence_frame_canonical_sha256
        current_projection_frame_index current_particle_frame_index
        current_projection_frame_canonical_sha256 current_projection_socket_transform_inventory_sha256
        current_projection_socket_transform_readback_sha256 base_frame_key_sha256 bloom_key_sha256
        camera_object_sha256 camera_identity_sha256 render_profile_sha256
        render_worker_build_cohort_sha256 trail_bloom_profile_sha256
        base_opaque_depth_object_sha256 base_aov_byte_exact_verified
        base_opaque_depth_byte_exact_reused bloom_pass_byte_exact_reused
        particle_passes_byte_exact_reused trail_passes_byte_exact_reused
        base_bloom_mutated particle_passes_mutated trail_passes_mutated
        trail_bloom_input trail_emissive_source_rendered trail_bloom_contribution_rendered
        trail_bloom_rendered trail_bloom_key_sha256 trail_bloom_seed_sha256
        trail_bloom_contributions trail_emissive_source_object_sha256
        trail_bloom_contribution_object_sha256 render_set_object_sha256 receipt_object_sha256
        canonical_sha256 created_at
    """.split()
    parent_fields = """
        schema_version sequence_key_sha256 project_id geometry_candidate_id
        geometry_candidate_state_sha256 geometry_delivery_manifest_object_sha256 geometry_artifact_sha256
        appearance_candidate_id appearance_candidate_state_sha256 appearance_delivery_manifest_object_sha256
        appearance_artifact_sha256 material_surface_quality_id
        material_surface_quality_report_object_sha256 material_surface_quality_canonical_sha256
        geometry_preservation_projection_sha256 geometry_preservation_status
        projection_key_sha256 projection_object_sha256 projection_canonical_sha256
        particle_sequence_key_sha256 particle_sequence_canonical_sha256
        animated_socket_materialization_key_sha256 animated_artifact_sha256
        animated_socket_anchor_set_object_sha256 animated_socket_anchor_set_canonical_sha256
        appearance_anchor_set_object_sha256 appearance_anchor_set_canonical_sha256
        anchor_binding_policy anchor_binding_sha256 animation_clip_id animation_clip_object_sha256
        animation_clip_canonical_sha256 animation_receipt_object_sha256 animation_receipt_canonical_sha256
        vfx_profile_object_sha256 vfx_profile_canonical_sha256 socket_node_id_encoding_sha256
        socket_roles_sha256 camera_object_sha256 camera_identity_sha256 render_profile_sha256
        render_worker_build_cohort_sha256 sample_schedule_sha256 sample_count sample_time_ticks
        frame_scope trails_bloom_sequence_policy history_policy history_pre_roll_policy
        trail_sequence_key_sha256 trail_sequence_canonical_sha256 trail_key_scope trail_count
        trail_emitter_roles trail_bloom_profile_sha256 trail_bloom_profile frames sequence_status
        quality_status visual_quality_status commercial_fps_quality_status human_review_status
        commercial_engine_status runtime_write_performed restart_hash_verified candidate_confirmed
        version_created export_performed actual_engine_roundtrip production_stage_advanced
        input_sha256 canonical_sha256 created_at
    """.split()
    frame_input_fields = """
        frame_index sample_time_ticks trail_frame_index trail_sequence_key_sha256
        trail_sequence_canonical_sha256 trail_frame_canonical_sha256 trail_key_sha256
        trail_inventory_sha256 trail_id_encoding_sha256 emitter_binding_sha256
        particle_sequence_key_sha256 particle_sequence_frame_canonical_sha256
        current_projection_frame_index current_particle_frame_index
        current_projection_frame_canonical_sha256 current_projection_socket_transform_inventory_sha256
        current_projection_socket_transform_readback_sha256 base_frame_key_sha256 bloom_key_sha256
        camera_object_sha256 camera_identity_sha256 render_profile_sha256 render_worker_build_cohort_sha256
    """.split()
    prepare_fields = """
        schema_version sequence_key_sha256 project_id geometry_candidate_id
        geometry_candidate_state_sha256 geometry_delivery_manifest_object_sha256 geometry_artifact_sha256
        appearance_candidate_id appearance_candidate_state_sha256 appearance_delivery_manifest_object_sha256
        appearance_artifact_sha256 material_surface_quality_id
        material_surface_quality_report_object_sha256 material_surface_quality_canonical_sha256
        projection_key_sha256 projection_object_sha256 projection_canonical_sha256
        particle_sequence_key_sha256 particle_sequence_canonical_sha256
        animated_socket_materialization_key_sha256 animated_artifact_sha256
        animated_socket_anchor_set_object_sha256 animated_socket_anchor_set_canonical_sha256
        appearance_anchor_set_object_sha256 appearance_anchor_set_canonical_sha256 anchor_binding_policy
        animation_clip_id animation_clip_object_sha256 animation_clip_canonical_sha256
        animation_receipt_object_sha256 animation_receipt_canonical_sha256
        vfx_profile_object_sha256 vfx_profile_canonical_sha256 socket_node_id_encoding_sha256
        socket_roles_sha256 camera_object_sha256 camera_identity_sha256 render_profile_sha256
        render_worker_build_cohort_sha256 sample_schedule_sha256 sample_count sample_time_ticks
        frame_scope trails_bloom_sequence_policy history_policy history_pre_roll_policy
        trail_sequence_key_sha256 trail_sequence_canonical_sha256 trail_key_scope trail_count
        trail_emitter_roles trail_bloom_profile_sha256 trail_bloom_profile frames
        input_sha256 idempotency_key
    """.split()
    get_fields = """
        schema_version sequence_key_sha256 project_id geometry_candidate_id appearance_candidate_id
        geometry_delivery_manifest_object_sha256 appearance_delivery_manifest_object_sha256
    """.split()
    result_fields = """
        schema_version sequence_key_sha256 sequence replayed restart_hash_verified runtime_write
        quality_status visual_quality_status commercial_fps_quality_status human_review_status
        commercial_engine_status actual_engine_roundtrip production_stage_advanced candidate_confirmed
        version_created export_performed
    """.split()

    def closed(filename: str, version: str, fields: list[str]) -> tuple[dict, dict]:
        schema = load_schema(filename)
        properties = schema.get("properties", {})
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and schema.get("title") == version
            and properties.get("schema_version", {}).get("const") == version
            and list(properties) == fields
            and list(schema.get("required", [])) == fields,
            version + " must be a closed exact-field object in frozen order",
        )
        return schema, properties

    frame, frame_properties = closed(
        prefix + "-frame.schema.json",
        "FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceFrame@2",
        frame_fields,
    )
    require(
        frame_properties["frame_index"].get("maximum") == 14
        and frame_properties["trail_frame_index"].get("maximum") == 14
        and frame_properties["current_projection_frame_index"].get("maximum") == 15
        and frame_properties["current_particle_frame_index"].get("maximum") == 15
        and frame_properties["trail_bloom_contributions"].get("minItems") == 2
        and frame_properties["trail_bloom_contributions"].get("maxItems") == 2
        and frame_properties["trail_bloom_contributions"].get("uniqueItems") is True,
        "animated socket TrailsBloom V2 frame must map 0..14 Trail@2 frames and exactly two contributions",
    )
    contribution = frame["$defs"]["trail_bloom_contribution"]
    contribution_fields = """
        emitter_role trail_id trail_key_sha256 trail_frame_canonical_sha256
        trail_bloom_contribution_sha256
    """.split()
    require(
        contribution.get("additionalProperties") is False
        and list(contribution.get("properties", {})) == contribution_fields
        and list(contribution.get("required", [])) == contribution_fields
        and contribution["properties"]["emitter_role"].get("enum")
        == ["muzzle-vfx", "energy-core-vfx"],
        "animated socket TrailsBloom V2 contributions must be two closed semantic trail bindings",
    )

    sequence, sequence_properties = closed(
        prefix + ".schema.json",
        "FictionalEnergyVfxAnimatedSocketTrailsBloomSequence@2",
        parent_fields,
    )
    require(
        sequence_properties["geometry_candidate_id"].get("x-forgecad-distinct-from")
        == "appearance_candidate_id"
        and sequence_properties["appearance_candidate_id"].get("x-forgecad-distinct-from")
        == "geometry_candidate_id"
        and sequence_properties["frames"].get("minItems") == 1
        and sequence_properties["frames"].get("maxItems") == 15
        and sequence_properties["sample_count"].get("maximum") == 15
        and sequence_properties["sample_time_ticks"].get("uniqueItems") is True
        and sequence_properties["frame_scope"].get("const") == ANIMATED_SOCKET_TRAILS_BLOOM_V2_FRAME_SCOPE
        and sequence_properties["trails_bloom_sequence_policy"].get("const") == ANIMATED_SOCKET_TRAILS_BLOOM_V2_POLICY
        and sequence_properties["trails_bloom_sequence_policy"].get("const") != "projection-driven-animated-socket-trails-bloom@1"
        and sequence_properties["history_policy"].get("const") == ANIMATED_SOCKET_TRAILS_V2_HISTORY_POLICY
        and sequence_properties["history_pre_roll_policy"].get("const") == ANIMATED_SOCKET_TRAILS_V2_HISTORY_PREROLL_POLICY
        and sequence_properties["trail_key_scope"].get("const") == ANIMATED_SOCKET_TRAILS_BLOOM_V2_TRAIL_KEY_SCOPE
        and sequence_properties["trail_count"].get("const") == 2
        and sequence_properties["trail_emitter_roles"].get("const") == ["muzzle-vfx", "energy-core-vfx"],
        "animated socket TrailsBloom V2 parent must freeze Trail@2 frame scope, history and two contributions",
    )
    require(
        sequence_properties["particle_sequence_key_sha256"].get("$ref") == "#/$defs/sha256"
        and sequence_properties["particle_sequence_canonical_sha256"].get("$ref") == "#/$defs/sha256"
        and sequence_properties["projection_key_sha256"].get("$ref") == "#/$defs/sha256"
        and sequence_properties["projection_canonical_sha256"].get("$ref") == "#/$defs/sha256"
        and "particle_sequence_object_sha256" not in sequence_properties
        and "trail_sequence_object_sha256" not in sequence_properties,
        "animated socket TrailsBloom V2 must bind Projection@2/Particles@2 by hash without fake parent CAS objects",
    )
    profile_fields = {
        "threshold": 1,
        "source_gain": 8,
        "radius_px": 8,
        "intensity": 4,
        "hdr_clamp": 16,
        "blur_passes": 2,
        "kernel": "separable-box-two-pass-fixed-radius@1",
    }
    profile = sequence["$defs"]["trail_bloom_profile"]
    require(
        profile.get("additionalProperties") is False
        and {key: value.get("const") for key, value in profile.get("properties", {}).items()} == profile_fields,
        "animated socket TrailsBloom V2 must retain the fixed bounded Bloom profile",
    )
    truth = {
        "base_aov_byte_exact_verified": True,
        "base_opaque_depth_byte_exact_reused": True,
        "bloom_pass_byte_exact_reused": True,
        "particle_passes_byte_exact_reused": True,
        "trail_passes_byte_exact_reused": True,
        "base_bloom_mutated": False,
        "particle_passes_mutated": False,
        "trail_passes_mutated": False,
        "trail_bloom_input": True,
        "trail_emissive_source_rendered": True,
        "trail_bloom_contribution_rendered": True,
        "trail_bloom_rendered": True,
    }
    require(
        sequence_properties["quality_status"].get("const") == "structural_only"
        and sequence_properties["visual_quality_status"].get("const") == "NOT_PROVEN"
        and sequence_properties["commercial_fps_quality_status"].get("const") == "NOT_PROVEN"
        and sequence_properties["human_review_status"].get("const") == "NOT_RUN"
        and sequence_properties["commercial_engine_status"].get("const") == "NOT_RUN"
        and sequence_properties["runtime_write_performed"].get("const") is True
        and sequence_properties["restart_hash_verified"].get("const") is True
        and sequence_properties["candidate_confirmed"].get("const") is False
        and sequence_properties["version_created"].get("const") is False
        and sequence_properties["export_performed"].get("const") is False
        and sequence_properties["actual_engine_roundtrip"].get("const") is False
        and sequence_properties["production_stage_advanced"].get("const") is False
        and all(frame_properties[key].get("const") == value for key, value in truth.items()),
        "animated socket TrailsBloom V2 must remain structural-only and non-promoting",
    )

    prepare, prepare_properties = closed(
        prefix + "-prepare-request.schema.json",
        "FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareRequest@2",
        prepare_fields,
    )
    frame_input = prepare["$defs"]["frame_input"]
    require(
        prepare_properties["frames"].get("items", {}).get("$ref") == "#/$defs/frame_input"
        and frame_input.get("additionalProperties") is False
        and list(frame_input.get("properties", {})) == frame_input_fields
        and list(frame_input.get("required", [])) == frame_input_fields
        and frame_input["properties"]["frame_index"].get("maximum") == 14
        and frame_input["properties"]["trail_frame_index"].get("maximum") == 14
        and frame_input["properties"]["current_projection_frame_index"].get("maximum") == 15
        and frame_input["properties"]["current_particle_frame_index"].get("maximum") == 15
        and prepare_properties["geometry_candidate_id"].get("x-forgecad-distinct-from") == "appearance_candidate_id"
        and prepare_properties["appearance_candidate_id"].get("x-forgecad-distinct-from") == "geometry_candidate_id"
        and prepare_properties["trails_bloom_sequence_policy"].get("const") == ANIMATED_SOCKET_TRAILS_BLOOM_V2_POLICY
        and prepare_properties["history_policy"].get("const") == ANIMATED_SOCKET_TRAILS_V2_HISTORY_POLICY
        and prepare_properties["history_pre_roll_policy"].get("const") == ANIMATED_SOCKET_TRAILS_V2_HISTORY_PREROLL_POLICY
        and "geometry_preservation_projection_sha256" not in prepare_properties
        and "geometry_preservation_status" not in prepare_properties
        and "anchor_binding_sha256" not in prepare_properties
        and "canonical_sha256" not in prepare_properties
        and "created_at" not in prepare_properties
        and "trail_bloom_contributions" not in frame_input["properties"]
        and "trail_emissive_source_object_sha256" not in frame_input["properties"]
        and "trail_bloom_contribution_object_sha256" not in frame_input["properties"],
        "animated socket TrailsBloom V2 prepare must exclude derived contributions and CAS outputs",
    )

    _, get_properties = closed(
        prefix + "-get-request.schema.json",
        "FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetRequest@2",
        get_fields,
    )
    require(
        get_properties["geometry_candidate_id"].get("x-forgecad-distinct-from") == "appearance_candidate_id"
        and get_properties["appearance_candidate_id"].get("x-forgecad-distinct-from") == "geometry_candidate_id",
        "animated socket TrailsBloom V2 get must bind both candidates and deliveries",
    )
    for filename, schema_version, runtime_write in [
        (
            prefix + "-prepare-result.schema.json",
            "FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareResult@2",
            True,
        ),
        (
            prefix + "-get-result.schema.json",
            "FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetResult@2",
            False,
        ),
    ]:
        result, result_properties = closed(filename, schema_version, result_fields)
        require(
            result_properties["sequence"].get("$ref") == prefix + ".schema.json"
            and result_properties["restart_hash_verified"].get("const") is True
            and result_properties["runtime_write"].get("const") is runtime_write
            and result_properties["quality_status"].get("const") == "structural_only"
            and result_properties["visual_quality_status"].get("const") == "NOT_PROVEN"
            and result_properties["commercial_fps_quality_status"].get("const") == "NOT_PROVEN"
            and result_properties["human_review_status"].get("const") == "NOT_RUN"
            and result_properties["commercial_engine_status"].get("const") == "NOT_RUN"
            and result_properties["actual_engine_roundtrip"].get("const") is False
            and result_properties["production_stage_advanced"].get("const") is False
            and result_properties["candidate_confirmed"].get("const") is False
            and result_properties["version_created"].get("const") is False
            and result_properties["export_performed"].get("const") is False,
            schema_version + " must remain restart-verified, structural-only and non-promoting",
        )

def check_fictional_energy_vfx_trails_bloom_contracts() -> None:
    """Keep the additive typed-trail Bloom frame contracts closed and hash-bound."""
    expected = {
        "fictional-energy-vfx-trails-bloom-frame-render-prepare-request.schema.json": "FictionalEnergyVfxTrailsBloomFrameRenderPrepareRequest@1",
        "fictional-energy-vfx-trails-bloom-frame-receipt.schema.json": "FictionalEnergyVfxTrailsBloomFrameReceipt@1",
        "fictional-energy-vfx-trails-bloom-frame-link.schema.json": "FictionalEnergyVfxTrailsBloomFrameLink@1",
        "fictional-energy-vfx-trails-bloom-render-set.schema.json": "FictionalEnergyVfxTrailsBloomRenderSet@1",
        "fictional-energy-vfx-trails-bloom-frame-prepare-result.schema.json": "FictionalEnergyVfxTrailsBloomFramePrepareResult@1",
        "fictional-energy-vfx-trails-bloom-frame-get-request.schema.json": "FictionalEnergyVfxTrailsBloomFrameGetRequest@1",
        "fictional-energy-vfx-trails-bloom-frame-get-result.schema.json": "FictionalEnergyVfxTrailsBloomFrameGetResult@1",
    }
    actual = {path.name for path in SCHEMA_ROOT.glob("fictional-energy-vfx-trails-bloom-*.schema.json")}
    require(actual == set(expected), "typed-trail Bloom schema set must contain exactly seven V1 contracts")
    for filename, version in expected.items():
        schema = load_schema(filename)
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and schema.get("title") == version
            and schema.get("properties", {}).get("schema_version", {}).get("const") == version
            and set(schema.get("required", [])) == set(schema.get("properties", {})),
            f"{version} must remain a closed exact-field object contract",
        )

    request = load_schema(
        "fictional-energy-vfx-trails-bloom-frame-render-prepare-request.schema.json"
    )
    receipt = load_schema("fictional-energy-vfx-trails-bloom-frame-receipt.schema.json")
    link = load_schema("fictional-energy-vfx-trails-bloom-frame-link.schema.json")
    render_set = load_schema("fictional-energy-vfx-trails-bloom-render-set.schema.json")
    profile_fields = {
        "threshold": 1.0,
        "source_gain": 8.0,
        "radius_px": 8,
        "intensity": 4.0,
        "hdr_clamp": 16.0,
        "blur_passes": 2,
        "kernel": "separable-box-two-pass-fixed-radius@1",
    }
    for schema, label in [
        (request, "prepare request"),
        (receipt, "receipt"),
        (render_set, "render set"),
    ]:
        profile = schema["$defs"]["trail_bloom_profile"]
        require(
            profile.get("type") == "object"
            and profile.get("additionalProperties") is False
            and set(profile.get("required", [])) == set(profile_fields)
            and {
                key: value.get("const")
                for key, value in profile.get("properties", {}).items()
            }
            == profile_fields,
            f"typed-trail Bloom {label} must use the fixed threshold/gain/blur profile",
        )
    policies = {
        "trail_bloom_policy": "lod0-typed-trails-hdr-source-two-pass-fixed-kernel@1",
        "input_policy": "existing-trail-color-depth-plus-current-base-opaque-depth-byte-exact@1",
        "occlusion_policy": "current-base-opaque-depth-before-trail-depth-reversed-normalized-u8-epsilon-1e-4@1",
        "render_policy": "lod0-trail-bloom-two-new-passes-base-bloom-particles-trails-byte-exact-reused@1",
    }
    for schema, label in [(request, "prepare request"), (receipt, "receipt"), (render_set, "render set")]:
        require(
            all(schema["properties"][name].get("const") == value for name, value in policies.items()),
            f"typed-trail Bloom {label} policy/canonical bindings drifted",
        )

    link_fields = {
        "schema_version",
        "trail_bloom_key_sha256",
        "project_id",
        "delivery_manifest_object_sha256",
        "vfx_profile_object_sha256",
        "anchor_set_object_sha256",
        "source_candidate_id",
        "source_artifact_sha256",
        "sample_request_sha256",
        "base_frame_key_sha256",
        "bloom_key_sha256",
        "source_trail_key_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
        "trail_bloom_profile_sha256",
        "base_opaque_depth_object_sha256",
        "trail_seed_sha256",
        "node_inventory_sha256",
        "owner_world_transform_sha256",
        "trail_inventory_sha256",
        "trail_id_encoding_sha256",
        "source_trail_color_object_sha256",
        "source_trail_id_object_sha256",
        "source_trail_depth_object_sha256",
        "render_set_object_sha256",
        "receipt_object_sha256",
        "source_object_sha256",
        "contribution_object_sha256",
        "materialization_status",
        "canonical_sha256",
        "created_at",
    }
    require(
        set(link["required"]) == link_fields
        and set(link["properties"]) == link_fields
        and link["properties"]["materialization_status"].get("const")
        == "runtime-owned-durable-fictional-energy-vfx-typed-trails-bloom-frame",
        "typed-trail Bloom durable link must use the frozen exact field set",
    )

    expected_passes = ["trail-emissive-source", "trail-bloom-contribution"]
    require(
        render_set["properties"]["passes"].get("const") == expected_passes
        and set(render_set["properties"]["pass_artifacts"]["required"]) == set(expected_passes),
        "typed-trail Bloom RenderSet must own exactly the two additive pass artifacts",
    )
    pass_definition = render_set["$defs"]["pass"]
    require(
        pass_definition.get("additionalProperties") is False
        and set(pass_definition.get("required", []))
        == {"pass", "sha256", "mime", "size_bytes", "width", "height", "channels", "color_space"}
        and pass_definition["properties"]["pass"].get("enum") == expected_passes
        and pass_definition["properties"]["mime"].get("const") == "image/png"
        and pass_definition["properties"]["width"].get("const") == 512
        and pass_definition["properties"]["height"].get("const") == 512
        and pass_definition["properties"]["channels"].get("const") == "rgba8"
        and pass_definition["properties"]["color_space"].get("const") == "data",
        "typed-trail Bloom pass artifacts must remain bounded 512px RGBA8 PNGs",
    )
    truth_fields = {
        "base_aov_byte_exact_verified": True,
        "base_opaque_depth_byte_exact_reused": True,
        "bloom_pass_byte_exact_reused": True,
        "particle_passes_byte_exact_reused": True,
        "source_trail_passes_byte_exact_reused": True,
        "base_bloom_mutated": False,
        "particle_passes_mutated": False,
        "trail_passes_mutated": False,
        "trail_bloom_source_rendered": True,
        "trail_bloom_contribution_rendered": True,
        "trail_bloom_rendered": True,
        "trail_bloom_input": True,
    }
    for schema, label in [(receipt, "receipt"), (render_set, "render set")]:
        require(
            all(schema["properties"][name].get("const") == value for name, value in truth_fields.items()),
            f"typed-trail Bloom {label} truth-boundary flags drifted",
        )


def check_boolean_operand_lineage_contracts() -> None:
    request = load_schema("boolean-operand-lineage-request.schema.json")
    request_fields = {
        "schema_version",
        "geometry_program",
        "boolean_node_id",
        "max_lineage_runs",
        "canonical_sha256",
    }
    require(
        request.get("type") == "object"
        and request.get("additionalProperties") is False
        and set(request.get("required", [])) == request_fields
        and set(request.get("properties", {})) == request_fields
        and request["properties"]["schema_version"].get("const")
        == "BooleanOperandLineageRequest@1"
        and request["properties"]["geometry_program"].get("$ref")
        == "https://forgecad.local/contracts/geometry-program-v2.schema.json"
        and request["properties"]["max_lineage_runs"].get("minimum") == 1
        and request["properties"]["max_lineage_runs"].get("maximum") == 4096,
        "BooleanOperandLineageRequest@1 must remain closed, canonical and bounded",
    )

    result = load_schema("boolean-operand-lineage.schema.json")
    result_fields = {
        "schema_version",
        "program_sha256",
        "operator_catalog_sha256",
        "boolean_node_id",
        "operation",
        "operands",
        "output_triangle_count",
        "lineage_run_count",
        "lineage_runs",
        "lineage_sha256",
        "lineage_kind",
        "materialization_status",
        "runtime_write_performed",
        "limitations",
        "canonical_sha256",
    }
    operands = result["properties"]["operands"]
    run = result["$defs"]["lineage_run"]
    require(
        result.get("type") == "object"
        and result.get("additionalProperties") is False
        and set(result.get("required", [])) == result_fields
        and set(result.get("properties", {})) == result_fields
        and result["properties"]["schema_version"].get("const")
        == "BooleanOperandLineage@1"
        and result["properties"]["operation"].get("enum")
        == ["union", "difference", "intersection"]
        and operands.get("minItems") == 2
        and operands.get("maxItems") == 2
        and operands.get("items") is False
        and len(operands.get("prefixItems", [])) == 2
        and result["properties"]["lineage_runs"].get("minItems") == 1
        and result["properties"]["lineage_runs"].get("maxItems") == 4096
        and run.get("additionalProperties") is False
        and result["properties"]["lineage_kind"].get("const")
        == "evaluated-face-with-operand-run"
        and result["properties"]["materialization_status"].get("const")
        == "preview-only-not-persisted-in-glb"
        and result["properties"]["runtime_write_performed"].get("const") is False,
        "BooleanOperandLineage@1 must remain a closed bounded read-only projection",
    )
    limitations = result["properties"]["limitations"].get("const", [])
    require(
        limitations
        == [
            "EVALUATED_FACE_ID_NOT_ORIGINAL_AUTHORING_FACE_ID",
            "FACE_IDS_NOT_STABLE_ACROSS_PROGRAM_CHANGE",
            "LINEAGE_NOT_PERSISTED_IN_CURRENT_GLB",
            "STRUCTURAL_LINEAGE_DOES_NOT_PROVE_VISUAL_QUALITY",
        ],
        "BooleanOperandLineage@1 must not claim original authoring-face or visual truth",
    )


def check_subdivision_topology_lineage_contracts() -> None:
    request = load_schema("subdivision-topology-lineage-request.schema.json")
    request_fields = {
        "schema_version", "geometry_program", "subdivision_node_id",
        "max_lineage_elements", "canonical_sha256",
    }
    require(
        request.get("type") == "object"
        and request.get("additionalProperties") is False
        and set(request.get("required", [])) == request_fields
        and set(request.get("properties", {})) == request_fields
        and request["properties"]["schema_version"].get("const")
        == "SubdivisionTopologyLineageRequest@1"
        and request["properties"]["geometry_program"].get("$ref")
        == "https://forgecad.local/contracts/geometry-program-v2.schema.json"
        and request["properties"]["max_lineage_elements"].get("minimum") == 1
        and request["properties"]["max_lineage_elements"].get("maximum") == 25000,
        "SubdivisionTopologyLineageRequest@1 must remain closed, canonical and bounded",
    )

    result = load_schema("subdivision-topology-lineage.schema.json")
    result_fields = {
        "schema_version", "program_sha256", "operator_catalog_sha256",
        "subdivision_node_id", "lineage_kind", "lineage_space", "id_scope",
        "complete", "completeness_scope", "cross_version_stable",
        "artifact_binding_status", "max_lineage_elements", "lineage_element_count",
        "lineage", "lineage_sha256", "materialization_status",
        "runtime_write_performed", "quality_status", "limitations", "canonical_sha256",
    }
    lineage = result["$defs"]["lineage"]
    limitations = result["properties"]["limitations"].get("const", [])
    require(
        result.get("type") == "object"
        and result.get("additionalProperties") is False
        and set(result.get("required", [])) == result_fields
        and set(result.get("properties", {})) == result_fields
        and result["properties"]["schema_version"].get("const")
        == "SubdivisionTopologyLineage@1"
        and result["properties"]["lineage_kind"].get("const")
        == "control-root-to-evaluated-quad-topology@1"
        and result["properties"]["lineage_space"].get("const")
        == "evaluated-quad-topology@1"
        and result["properties"]["id_scope"].get("const")
        == "program-and-evaluation-bound"
        and result["properties"]["complete"].get("const") is True
        and result["properties"]["completeness_scope"].get("const")
        == "all-root-mappings-within-declared-preview-lineage"
        and result["properties"]["cross_version_stable"].get("const") is False
        and result["properties"]["artifact_binding_status"].get("const")
        == "unavailable-preview-only"
        and result["properties"]["max_lineage_elements"].get("maximum") == 25000
        and result["properties"]["lineage_element_count"].get("maximum") == 25000
        and result["properties"]["runtime_write_performed"].get("const") is False
        and result["properties"]["quality_status"].get("const") == "structural_only"
        and lineage.get("additionalProperties") is False
        and lineage["properties"]["evaluated_vertex_root_origins"].get("maxItems") == 3721
        and lineage["properties"]["evaluated_edge_root_origins"].get("maxItems") == 7320
        and lineage["properties"]["evaluated_quad_control_quad_ids"].get("maxItems") == 3600
        and lineage["properties"]["quad_triangulation"].get("const") == "0-1-2_0-2-3",
        "SubdivisionTopologyLineage@1 must remain a closed bounded root-lineage projection",
    )
    require(
        limitations
        == [
            "REGULAR_RECTANGULAR_OPEN_QUAD_GRID_ONLY",
            "INTEGER_EDGE_SHARPNESS_LEVELS_1_TO_2_ONLY",
            "ELEMENT_IDS_CHANGE_WHEN_PROGRAM_OR_EVALUATION_CHANGES",
            "EVALUATED_QUAD_IDS_ARE_NOT_GLTF_TRIANGLE_OR_DEDUPLICATED_VERTEX_IDS",
            "ROOT_ANCESTRY_ONLY_NO_INFLUENCE_WEIGHTS_OR_CORNER_DOMAIN",
            "PREVIEW_NOT_ARTIFACT_OR_READBACK_BOUND",
            "LINEAGE_NOT_PERSISTED_IN_CURRENT_GLB",
            "STRUCTURAL_LINEAGE_DOES_NOT_PROVE_VISUAL_QUALITY",
        ],
        "SubdivisionTopologyLineage@1 must not claim corner, artifact, GLB or visual truth",
    )

    artifact_request = load_schema("subdivision-artifact-lineage-request.schema.json")
    artifact_request_fields = {
        "schema_version", "project_id", "candidate_id", "artifact_id",
        "artifact_readback_sha256", "subdivision_node_id",
        "max_lineage_elements", "canonical_sha256",
    }
    require(
        artifact_request.get("type") == "object"
        and artifact_request.get("additionalProperties") is False
        and set(artifact_request.get("required", [])) == artifact_request_fields
        and set(artifact_request.get("properties", {})) == artifact_request_fields
        and artifact_request["properties"]["schema_version"].get("const")
        == "SubdivisionArtifactLineageRequest@1"
        and artifact_request["properties"]["max_lineage_elements"].get("minimum") == 1
        and artifact_request["properties"]["max_lineage_elements"].get("maximum") == 25000,
        "SubdivisionArtifactLineageRequest@1 must remain closed, candidate-bound and bounded",
    )

    artifact_result = load_schema("subdivision-artifact-lineage-projection.schema.json")
    artifact_result_fields = {
        "schema_version", "project_id", "candidate_id", "artifact_id",
        "artifact_readback_sha256", "artifact_readback_object_sha256",
        "geometry_candidate_evidence_sha256", "program_sha256",
        "geometry_program_object_sha256", "operator_catalog_sha256",
        "readback_config_sha256", "subdivision_node_id", "part_id",
        "material_zone_id", "solid", "lineage_kind", "lineage_space",
        "max_lineage_elements", "lineage_element_count",
        "root_lineage", "lineage_sha256", "artifact_binding",
        "artifact_binding_sha256", "complete", "completeness_scope",
        "cross_version_stable", "materialization_status", "runtime_write_performed",
        "quality_status", "limitations", "canonical_sha256",
    }
    artifact_binding = artifact_result["$defs"]["artifact_binding"]
    artifact_limitations = artifact_result["properties"]["limitations"].get("const", [])
    require(
        artifact_result.get("type") == "object"
        and artifact_result.get("additionalProperties") is False
        and set(artifact_result.get("required", [])) == artifact_result_fields
        and set(artifact_result.get("properties", {})) == artifact_result_fields
        and artifact_result["properties"]["schema_version"].get("const")
        == "SubdivisionArtifactLineageProjection@1"
        and artifact_result["properties"]["root_lineage"].get("$ref")
        == "https://forgecad.local/contracts/subdivision-topology-lineage.schema.json#/$defs/lineage"
        and artifact_result["properties"]["lineage_space"].get("const")
        == "evaluated-quad-topology-to-source-primitive-triangles@1"
        and artifact_result["properties"]["max_lineage_elements"].get("maximum") == 25000
        and artifact_result["properties"]["lineage_element_count"].get("maximum") == 25000
        and artifact_result["properties"]["complete"].get("const") is True
        and artifact_result["properties"]["cross_version_stable"].get("const") is False
        and artifact_result["properties"]["materialization_status"].get("const")
        == "read-only-reconstructed-projection-not-persisted-sidecar"
        and artifact_result["properties"]["runtime_write_performed"].get("const") is False
        and artifact_result["properties"]["quality_status"].get("const") == "structural_only"
        and artifact_binding.get("additionalProperties") is False
        and artifact_binding["properties"]["binding_method"].get("const")
        == "deterministic-full-glb-byte-replay-and-source-primitive-triangle-order@1"
        and artifact_binding["properties"]["artifact_triangle_domain"].get("const")
        == "source-primitive-local-triangle-index@1"
        and "ArtifactReadback@2.part_bindings"
        in artifact_binding["properties"]["source_primitive_ordinal"].get("description", "")
        and artifact_binding["properties"]["mapping_complete"].get("const") is True,
        "SubdivisionArtifactLineageProjection@1 must remain closed, replay-bound and read-only",
    )
    require(
        artifact_limitations
        == [
            "REGULAR_RECTANGULAR_OPEN_QUAD_GRID_ONLY",
            "INTEGER_EDGE_SHARPNESS_LEVELS_1_TO_2_ONLY",
            "SOURCE_PRIMITIVE_LOCAL_TRIANGLE_IDS_ONLY",
            "NO_GLTF_VERTEX_EDGE_OR_CORNER_IDENTITY",
            "NO_CROSS_VERSION_ELEMENT_ID_STABILITY",
            "PROJECTION_NOT_PERSISTED_AS_A_CAS_SIDECAR",
            "DETERMINISTIC_FULL_GLB_BYTE_REPLAY_REQUIRED",
            "STRUCTURAL_LINEAGE_DOES_NOT_PROVE_VISUAL_QUALITY",
        ],
        "SubdivisionArtifactLineageProjection@1 must not claim persisted or visual truth",
    )


def check_subdivision_artifact_lineage_sidecar_contracts() -> None:
    request = load_schema("subdivision-artifact-lineage-sidecar-request.schema.json")
    request_fields = {
        "schema_version", "project_id", "candidate_id", "artifact_id",
        "artifact_readback_sha256", "subdivision_node_id",
        "max_lineage_elements", "canonical_sha256",
    }
    require(
        request.get("type") == "object"
        and request.get("additionalProperties") is False
        and set(request.get("required", [])) == request_fields
        and set(request.get("properties", {})) == request_fields
        and request["properties"]["schema_version"].get("const")
        == "SubdivisionArtifactLineageSidecarRequest@1"
        and request["properties"]["max_lineage_elements"].get("minimum") == 1
        and request["properties"]["max_lineage_elements"].get("maximum") == 25000,
        "SubdivisionArtifactLineageSidecarRequest@1 must remain closed and bounded",
    )

    sidecar = load_schema("subdivision-artifact-lineage-sidecar.schema.json")
    sidecar_fields = {
        "schema_version", "project_id", "candidate_id", "artifact_id",
        "artifact_readback_sha256", "artifact_readback_object_sha256",
        "geometry_candidate_evidence_sha256", "program_sha256",
        "geometry_program_object_sha256", "operator_catalog_sha256",
        "readback_config_sha256", "subdivision_node_id", "part_id",
        "material_zone_id", "solid", "lineage_kind", "lineage_space",
        "max_lineage_elements", "lineage_element_count", "root_lineage",
        "lineage_sha256", "artifact_binding", "artifact_binding_sha256",
        "complete", "completeness_scope", "cross_version_stable",
        "materialization_status", "quality_status", "limitations",
        "canonical_sha256",
    }
    sidecar_binding = sidecar["$defs"]["artifact_binding"]
    sidecar_limitations = sidecar["properties"]["limitations"].get("const", [])
    require(
        sidecar.get("type") == "object"
        and sidecar.get("additionalProperties") is False
        and set(sidecar.get("required", [])) == sidecar_fields
        and set(sidecar.get("properties", {})) == sidecar_fields
        and sidecar["properties"]["schema_version"].get("const")
        == "SubdivisionArtifactLineageSidecar@1"
        and sidecar["properties"]["root_lineage"].get("$ref")
        == "https://forgecad.local/contracts/subdivision-topology-lineage.schema.json#/$defs/lineage"
        and sidecar["properties"]["lineage_space"].get("const")
        == "evaluated-quad-topology-to-source-primitive-triangles@1"
        and sidecar["properties"]["max_lineage_elements"].get("maximum") == 25000
        and sidecar["properties"]["lineage_element_count"].get("maximum") == 25000
        and sidecar["properties"]["solid"].get("const") is False
        and sidecar["properties"]["complete"].get("const") is True
        and sidecar["properties"]["cross_version_stable"].get("const") is False
        and sidecar["properties"]["materialization_status"].get("const")
        == "runtime-owned-immutable-cas-sidecar"
        and sidecar["properties"]["quality_status"].get("const") == "structural_only"
        and sidecar["properties"].get("runtime_write_performed") is None
        and sidecar_binding.get("additionalProperties") is False
        and sidecar_binding["properties"]["binding_method"].get("const")
        == "deterministic-full-glb-byte-replay-and-source-primitive-triangle-order@1"
        and sidecar_binding["properties"]["artifact_triangle_domain"].get("const")
        == "source-primitive-local-triangle-index@1"
        and sidecar_binding["properties"]["mapping_complete"].get("const") is True,
        "SubdivisionArtifactLineageSidecar@1 must remain closed and artifact-bound",
    )
    require(
        sidecar_limitations
        == [
            "REGULAR_RECTANGULAR_OPEN_QUAD_GRID_ONLY",
            "INTEGER_EDGE_SHARPNESS_LEVELS_1_TO_2_ONLY",
            "SOURCE_PRIMITIVE_LOCAL_TRIANGLE_IDS_ONLY",
            "NO_GLTF_VERTEX_EDGE_OR_CORNER_IDENTITY",
            "NO_CROSS_VERSION_ELEMENT_ID_STABILITY",
            "IMMUTABLE_CAS_SIDECAR_NO_CROSS_VERSION_STABILITY",
            "DETERMINISTIC_FULL_GLB_BYTE_REPLAY_REQUIRED",
            "STRUCTURAL_LINEAGE_DOES_NOT_PROVE_VISUAL_QUALITY",
        ],
        "SubdivisionArtifactLineageSidecar@1 must retain bounded structural limitations",
    )

    link = load_schema("subdivision-artifact-lineage-link.schema.json")
    link_fields = {
        "schema_version", "project_id", "candidate_id", "artifact_id",
        "artifact_readback_sha256", "geometry_candidate_evidence_sha256",
        "subdivision_node_id", "request_sha256", "sidecar_object_sha256",
        "lineage_sha256", "artifact_binding_sha256", "materialization_status",
        "sidecar", "canonical_sha256",
    }
    require(
        link.get("type") == "object"
        and link.get("additionalProperties") is False
        and set(link.get("required", [])) == link_fields
        and set(link.get("properties", {})) == link_fields
        and link["properties"]["schema_version"].get("const")
        == "SubdivisionArtifactLineageLink@1"
        and link["properties"]["sidecar"].get("$ref")
        == "https://forgecad.local/contracts/subdivision-artifact-lineage-sidecar.schema.json"
        and link["properties"]["materialization_status"].get("const")
        == "runtime-owned-immutable-cas-sidecar",
        "SubdivisionArtifactLineageLink@1 must remain closed and embed the full sidecar",
    )


def check_production_stage_transition_contracts() -> None:
    """Keep the production-stage transition axis closed and Runtime-owned."""
    stages = [
        "draft",
        "gray-model",
        "topology",
        "material-surface",
        "animation-vfx",
        "game-delivery",
    ]
    output_kinds = [
        "gray-model-artifact",
        "topology-quality",
        "appearance-lineage",
        "animation-vfx-bundle",
        "game-asset-delivery",
    ]
    transition_fields = {
        "schema_version", "transition_id", "session_id", "project_id", "candidate_id",
        "from_stage", "to_stage", "candidate_state_sha256", "artifact_sha256",
        "output_kind", "output_object_sha256", "quality_report_object_sha256",
        "comparison_report_object_sha256", "reference_id", "reference_sha256", "camera_hash",
        "evidence_sha256", "parent_checkpoint_id", "parent_checkpoint_sha256", "gate_status",
        "status", "input_sha256", "canonical_sha256", "created_at",
    }
    transition = load_schema("production-stage-transition.schema.json")
    transition_properties = transition.get("properties", {})
    require(
        transition.get("type") == "object"
        and transition.get("additionalProperties") is False
        and set(transition.get("required", [])) == transition_fields
        and set(transition_properties) == transition_fields
        and transition_properties["schema_version"].get("const")
        == "ProductionStageTransition@1"
        and transition_properties["from_stage"].get("$ref") == "#/$defs/stage"
        and transition_properties["to_stage"].get("$ref") == "#/$defs/stage"
        and transition_properties["output_kind"].get("$ref") == "#/$defs/output_kind"
        and transition_properties["gate_status"].get("enum") == ["pass", "fail", "unknown"]
        and transition_properties["status"].get("enum") == ["blocked", "passed"],
        "ProductionStageTransition@1 must remain closed with frozen fields and statuses",
    )
    require(
        transition["$defs"]["stage"].get("enum") == stages
        and transition["$defs"]["output_kind"].get("enum") == output_kinds,
        "ProductionStageTransition@1 stage/output enums drifted",
    )
    for field in [
        "quality_report_object_sha256",
        "comparison_report_object_sha256",
        "parent_checkpoint_id",
        "parent_checkpoint_sha256",
    ]:
        require(
            transition_properties[field].get("$ref", "").endswith("nullable_sha256")
            or transition_properties[field].get("$ref", "").endswith("nullable_id"),
            f"ProductionStageTransition@1 {field} must be nullable",
        )

    prepare_fields = transition_fields - {"gate_status", "status", "canonical_sha256", "created_at"}
    prepare_fields |= {
        "approved", "approval_receipt_id", "approval_summary", "approval_expires_at",
        "approval_session_id", "idempotency_key",
    }
    prepare = load_schema("production-stage-transition-prepare-request.schema.json")
    prepare_properties = prepare.get("properties", {})
    require(
        prepare.get("type") == "object"
        and prepare.get("additionalProperties") is False
        and set(prepare.get("required", [])) == prepare_fields
        and set(prepare_properties) == prepare_fields
        and prepare_properties["schema_version"].get("const")
        == "ProductionStageTransitionPrepareRequest@1"
        and prepare_properties["approved"].get("const") is True
        and not ({"gate_status", "status", "canonical_sha256"} & set(prepare_properties)),
        "ProductionStageTransitionPrepareRequest@1 must require approval and reject Runtime-owned fields",
    )

    get_request = load_schema("production-stage-transition-get-request.schema.json")
    get_request_fields = {"schema_version", "transition_id", "session_id", "project_id", "candidate_id"}
    require(
        get_request.get("type") == "object"
        and get_request.get("additionalProperties") is False
        and set(get_request.get("required", [])) == get_request_fields
        and set(get_request.get("properties", {})) == get_request_fields
        and get_request["properties"]["schema_version"].get("const")
        == "ProductionStageTransitionGetRequest@1",
        "ProductionStageTransitionGetRequest@1 must be closed and fully bound",
    )

    result_fields = {
        "schema_version", "transition", "production_stage", "replayed", "runtime_write",
        "candidate_confirmed", "version_created", "export_performed",
    }
    for filename, schema_version, runtime_write in [
        (
            "production-stage-transition-prepare-result.schema.json",
            "ProductionStageTransitionPrepareResult@1",
            True,
        ),
        (
            "production-stage-transition-get-result.schema.json",
            "ProductionStageTransitionGetResult@1",
            False,
        ),
    ]:
        result = load_schema(filename)
        result_properties = result.get("properties", {})
        require(
            result.get("type") == "object"
            and result.get("additionalProperties") is False
            and set(result.get("required", [])) == result_fields
            and set(result_properties) == result_fields
            and result_properties["schema_version"].get("const") == schema_version
            and result_properties["transition"].get("$ref")
            == "https://forgecad.local/contracts/production-stage-transition.schema.json"
            and result_properties["production_stage"].get("$ref") == "#/$defs/stage"
            and result_properties["runtime_write"].get("const") is runtime_write
            and result_properties["candidate_confirmed"].get("const") is False
            and result_properties["version_created"].get("const") is False
            and result_properties["export_performed"].get("const") is False,
            f"{schema_version} must expose Runtime-owned transition result flags",
        )
        require(
            result["$defs"]["stage"].get("enum") == stages,
            f"{schema_version} production_stage enum drifted",
        )


def check_production_stage_v2_contracts() -> None:
    """Keep V2 dual-candidate promotion closed, approval-bound and V1-independent."""
    head_fields = {
        "schema_version", "session_id", "project_id", "root_candidate_id",
        "root_candidate_role", "root_candidate_state_sha256", "source_artifact_id",
        "root_artifact_sha256", "root_stage", "previous_head_candidate_id",
        "previous_head_candidate_role", "previous_head_candidate_state_sha256",
        "previous_head_artifact_id", "previous_head_artifact_sha256", "previous_head_stage",
        "head_candidate_id", "head_candidate_role", "head_candidate_state_sha256",
        "output_artifact_id", "head_artifact_sha256", "head_stage", "topology_quality_id",
        "topology_quality_status", "topology_quality_report_object_sha256",
        "topology_quality_canonical_sha256", "material_surface_quality_id",
        "material_surface_quality_status", "material_surface_quality_report_object_sha256",
        "material_surface_quality_canonical_sha256", "reference_id", "reference_sha256",
        "camera_hash", "evidence_sha256", "approval_receipt_id", "approval_session_id",
        "approval_expires_at", "approval_summary_sha256", "candidate_binding_status",
        "quality_status", "visual_quality_status", "commercial_fps_quality_status",
        "candidate_confirmed", "version_created", "export_performed", "head_transition_id",
        "head_transition_sha256", "parent_topology_transition_id",
        "parent_topology_transition_sha256", "parent_topology_transition_schema_version",
        "materialization_status", "canonical_sha256", "updated_at",
    }
    transition_fields = {
        "schema_version", "transition_id", "session_id", "project_id", "root_candidate_id",
        "root_candidate_role", "root_candidate_state_sha256", "source_artifact_id",
        "root_artifact_sha256", "previous_head_candidate_id", "previous_head_candidate_role",
        "previous_head_candidate_state_sha256", "previous_head_artifact_id",
        "previous_head_artifact_sha256", "previous_head_stage", "head_candidate_id",
        "head_candidate_role", "head_candidate_state_sha256", "output_artifact_id",
        "head_artifact_sha256", "from_stage", "to_stage", "topology_quality_id",
        "topology_quality_status", "topology_quality_report_object_sha256",
        "topology_quality_canonical_sha256", "material_surface_quality_id",
        "material_surface_quality_status", "material_surface_quality_report_object_sha256",
        "material_surface_quality_canonical_sha256", "candidate_binding_status", "reference_id",
        "reference_sha256", "camera_hash", "evidence_sha256", "approval_receipt_id",
        "approval_session_id", "approval_expires_at", "approval_summary_sha256",
        "parent_topology_transition_id", "parent_topology_transition_sha256",
        "parent_topology_transition_schema_version", "gate_status", "status", "input_sha256",
        "canonical_sha256", "created_at",
    }
    head = load_schema("production-stage-head-v2.schema.json")
    transition = load_schema("production-stage-transition-v2.schema.json")
    head_properties = head.get("properties", {})
    transition_properties = transition.get("properties", {})
    v2_id_pattern = r"^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"
    v2_epoch_pattern = r"^[0-9]{1,10}$"
    v2_id_re = re.compile(v2_id_pattern)
    v2_epoch_re = re.compile(v2_epoch_pattern)

    # V2 opaque identifiers are deliberately stricter than the historical
    # MCP helper's length-only shape. Keep the rule local to this V2 family so
    # older tools remain wire-compatible while spaces and path-like IDs fail
    # before Runtime dispatch.
    v2_schemas = [
        ("ProductionStageHead@2", head),
        ("ProductionStageTransition@2", transition),
    ]
    prepare = load_schema("production-stage-transition-v2-prepare-request.schema.json")
    get_request = load_schema("production-stage-transition-v2-get-request.schema.json")
    v2_schemas.extend([
        ("ProductionStageTransitionPrepareRequest@2", prepare),
        ("ProductionStageTransitionGetRequest@2", get_request),
    ])
    for label, schema in v2_schemas:
        properties = schema.get("properties", {})
        opaque_fields = [
            field for field in properties
            if field.endswith("_id") or field == "idempotency_key"
        ]
        for field in opaque_fields:
            require(
                properties[field].get("$ref") == "#/$defs/id",
                f"{label} {field} must use the bounded V2 opaque-id pattern",
            )
        require(
            schema.get("$defs", {}).get("id", {}).get("pattern") == v2_id_pattern,
            f"{label} opaque-id definition must reject spaces and slashes",
        )
    for label, schema in v2_schemas[:3]:
        require(
            schema.get("properties", {}).get("approval_expires_at", {}).get("pattern")
            == v2_epoch_pattern,
            f"{label} approval_expires_at must be 1..10 decimal epoch seconds",
        )
    # Structural negative fixtures: these values must not match the frozen
    # patterns even if a caller supplies otherwise well-shaped JSON.
    require(v2_id_re.fullmatch("candidate-1") is not None,
            "V2 opaque-id positive fixture must remain accepted")
    require(v2_id_re.fullmatch("candidate with space") is None,
            "V2 opaque-id negative space fixture must be rejected")
    require(v2_id_re.fullmatch("candidate/child") is None,
            "V2 opaque-id negative slash fixture must be rejected")
    require(v2_epoch_re.fullmatch("1700000000") is not None,
            "V2 approval epoch positive fixture must remain accepted")
    require(v2_epoch_re.fullmatch("2026-08-21T23:59:59Z") is None,
            "V2 approval ISO expiry negative fixture must be rejected")
    require(
        head.get("type") == "object"
        and head.get("additionalProperties") is False
        and set(head.get("required", [])) == head_fields
        and set(head_properties) == head_fields
        and head_properties["schema_version"].get("const") == "ProductionStageHead@2"
        and head_properties["root_candidate_role"].get("const") == "topology-source"
        and head_properties["root_stage"].get("const") == "topology"
        and head_properties["head_candidate_role"].get("const") == "material-surface-output"
        and head_properties["head_stage"].get("const") == "material-surface"
        and head_properties["candidate_binding_status"].get("const")
        == "distinct-root-topology-to-material-surface-head"
        and head_properties["topology_quality_status"].get("const") == "passed"
        and head_properties["material_surface_quality_status"].get("const") == "passed"
        and head_properties["candidate_confirmed"].get("const") is False
        and head_properties["version_created"].get("const") is False
        and head_properties["export_performed"].get("const") is False,
        "ProductionStageHead@2 must be closed, root-keyed and dual-candidate bound",
    )
    require(
        transition.get("type") == "object"
        and transition.get("additionalProperties") is False
        and set(transition.get("required", [])) == transition_fields
        and set(transition_properties) == transition_fields
        and transition_properties["schema_version"].get("const")
        == "ProductionStageTransition@2"
        and transition_properties["root_candidate_role"].get("const") == "topology-source"
        and transition_properties["head_candidate_role"].get("const")
        == "material-surface-output"
        and transition_properties["from_stage"].get("const") == "topology"
        and transition_properties["to_stage"].get("const") == "material-surface"
        and transition_properties["candidate_binding_status"].get("const")
        == "distinct-root-topology-to-material-surface-head"
        and transition_properties["topology_quality_status"].get("const") == "passed"
        and transition_properties["material_surface_quality_status"].get("const") == "passed"
        and transition_properties["gate_status"].get("const") == "pass"
        and transition_properties["status"].get("const") == "passed"
        and transition_properties["parent_topology_transition_schema_version"].get("const")
        == "ProductionStageTransition@1",
        "ProductionStageTransition@2 must be closed, dual-candidate bound and passed-only",
    )
    for field in ["source_artifact_id", "output_artifact_id"]:
        require(
            transition_properties[field].get("$ref") == "#/$defs/id",
            f"ProductionStageTransition@2 {field} must be an exact artifact FK",
        )
    for field in ["topology_quality_report_object_sha256", "material_surface_quality_report_object_sha256"]:
        require(
            transition_properties[field].get("$ref") == "#/$defs/sha256",
            f"ProductionStageTransition@2 {field} must bind link.report_object_sha256",
        )

    prepare_fields = transition_fields - {
        "gate_status", "status", "canonical_sha256", "created_at", "approval_summary_sha256",
    }
    prepare_fields |= {"approved", "approval_summary", "idempotency_key"}
    prepare_properties = prepare.get("properties", {})
    require(
        prepare.get("type") == "object"
        and prepare.get("additionalProperties") is False
        and set(prepare.get("required", [])) == prepare_fields
        and set(prepare_properties) == prepare_fields
        and prepare_properties["schema_version"].get("const")
        == "ProductionStageTransitionPrepareRequest@2"
        and prepare_properties["approved"].get("const") is True
        and "approval_summary" in prepare_properties
        and "approval_summary_sha256" not in prepare_properties
        and not ({"gate_status", "status", "canonical_sha256", "created_at"} & set(prepare_properties)),
        "ProductionStageTransitionPrepareRequest@2 must keep raw approval transient and reject Runtime fields",
    )
    for filename, schema_version, runtime_write in [
        ("production-stage-transition-v2-prepare-result.schema.json", "ProductionStageTransitionPrepareResult@2", True),
        ("production-stage-transition-v2-get-result.schema.json", "ProductionStageTransitionGetResult@2", False),
    ]:
        result = load_schema(filename)
        result_properties = result.get("properties", {})
        require(
            result.get("type") == "object"
            and result.get("additionalProperties") is False
            and result_properties["schema_version"].get("const") == schema_version
            and result_properties["transition"].get("$ref")
            == "https://forgecad.local/contracts/production-stage-transition-v2.schema.json"
            and result_properties["production_stage_head"].get("$ref")
            == "https://forgecad.local/contracts/production-stage-head-v2.schema.json"
            and result_properties["runtime_write"].get("const") is runtime_write
            and result_properties["production_stage_advanced"].get("const") is True
            and result_properties["candidate_confirmed"].get("const") is False
            and result_properties["version_created"].get("const") is False
            and result_properties["export_performed"].get("const") is False,
            f"{schema_version} must return a non-null advanced head and forbid blocked results",
        )

    # Negative fixtures are intentionally structural: these payload mutations
    # must be rejected by the closed/const boundaries before Runtime writes.
    require("unexpected" not in head_properties and head.get("additionalProperties") is False,
            "ProductionStageHead@2 negative extra-field fixture must be rejected")
    require(transition_properties["status"].get("const") != "blocked"
            and transition_properties["gate_status"].get("const") != "fail",
            "ProductionStageTransition@2 negative blocked/failed fixture must be rejected zero-write")
    require("approval_summary" not in head_properties and "approval_summary" not in transition_properties,
            "V2 persisted records must not retain raw approval summary")

    v1 = load_schema("production-stage-transition.schema.json")
    require(
        v1["properties"]["schema_version"].get("const") == "ProductionStageTransition@1"
        and "candidate_id" in v1.get("required", [])
        and "root_candidate_id" not in v1.get("properties", {})
        and "head_candidate_id" not in v1.get("properties", {}),
        "ProductionStageTransition@1 must remain independently frozen",
    )


def check_production_stage_v3_contracts() -> None:
    """Keep ProductionStage@3 fine-grained, closed and V1/V2-independent."""
    stages = [
        "reference-intake",
        "reference-coverage-reviewed",
        "camera-calibrated",
        "blockout-reviewed",
        "primary-form-approved",
        "secondary-form-approved",
        "high-poly-approved",
        "low-poly-approved",
        "uv-approved",
        "cage-approved",
        "bake-approved",
        "material-approved",
        "rig-socket-approved",
        "animation-approved",
        "vfx-approved",
        "lod-collision-approved",
        "hero-art-review-approved",
        "engine-validated",
        "export-confirmed",
    ]
    coarse_stages = [
        "draft",
        "gray-model",
        "topology",
        "material-surface",
        "animation-vfx",
        "game-delivery",
    ]
    status_enums = {
        "structural_status": ["NOT_RUN", "BLOCKED", "PASS_SOURCE_STRUCTURAL"],
        "visual_status": ["NOT_RUN", "BLOCKED", "QUALITY_TARGET_NOT_MET", "PASS_STAGE_VISUAL", "PASS_STAGE_VISUAL_STRUCTURE_ONLY"],
        "human_status": ["NOT_RUN", "BLOCKED", "REJECTED", "PASS_HUMAN_ART_REVIEW"],
        "engine_status": ["NOT_RUN", "BLOCKED", "FAILED", "PASS_ENGINE_VALIDATION"],
        "distribution_status": ["NOT_RUN", "BLOCKED", "FAILED", "PASS_DISTRIBUTION"],
    }
    camera_binding_fields = {
        "camera_lock_id", "camera_lock_canonical_sha256", "camera_rig_object_sha256",
        "camera_rig_canonical_sha256", "camera_lock_receipt_object_sha256",
        "camera_lock_source_transition_id", "camera_lock_source_transition_sha256",
        "camera_lock_source_head_canonical_sha256",
    }
    transition_fields = {
        "schema_version", "transition_id", "session_id", "project_id",
        "root_candidate_id", "root_candidate_role", "root_candidate_state_sha256",
        "source_artifact_id", "root_artifact_sha256", "previous_head_candidate_id",
        "previous_head_candidate_role", "previous_head_candidate_state_sha256",
        "previous_head_artifact_id", "previous_head_artifact_sha256", "previous_head_stage",
        "head_candidate_id", "head_candidate_role", "head_candidate_state_sha256",
        "output_artifact_id", "head_artifact_sha256", "from_stage", "to_stage",
        "candidate_binding_status", "reference_id", "reference_sha256", "camera_hash",
        *camera_binding_fields,
        "evidence_sha256", "reference_canvas_object_sha256", "quality_report_object_sha256",
        "comparison_report_object_sha256", "design_spec_object_sha256",
        "visual_receipt_object_sha256", "human_review_receipt_object_sha256",
        "engine_validation_receipt_object_sha256", "distribution_receipt_object_sha256",
        "structural_status", "visual_status", "human_status", "engine_status",
        "distribution_status", "approval_receipt_id", "approval_session_id",
        "approval_expires_at", "approval_summary_sha256", "request_key_sha256",
        "parent_transition_id", "parent_transition_sha256",
        "parent_transition_schema_version", "gate_status", "status", "input_sha256",
        "receipt_object_sha256", "canonical_sha256", "created_at",
    }
    head_fields = {
        "schema_version", "session_id", "project_id", "root_candidate_id",
        "root_candidate_role", "root_candidate_state_sha256", "source_artifact_id",
        "root_artifact_sha256", "root_stage", "previous_head_candidate_id",
        "previous_head_candidate_role", "previous_head_candidate_state_sha256",
        "previous_head_artifact_id", "previous_head_artifact_sha256", "previous_head_stage",
        "head_candidate_id", "head_candidate_role", "head_candidate_state_sha256",
        "output_artifact_id", "head_artifact_sha256", "head_stage",
        "candidate_binding_status", "reference_id", "reference_sha256", "camera_hash",
        *camera_binding_fields,
        "evidence_sha256", "reference_canvas_object_sha256", "quality_report_object_sha256",
        "comparison_report_object_sha256", "design_spec_object_sha256",
        "visual_receipt_object_sha256", "human_review_receipt_object_sha256",
        "engine_validation_receipt_object_sha256", "distribution_receipt_object_sha256",
        "structural_status", "visual_status", "human_status", "engine_status",
        "distribution_status", "approval_receipt_id", "approval_session_id",
        "approval_expires_at", "approval_summary_sha256", "head_transition_id",
        "head_transition_sha256", "compatibility_projection", "candidate_confirmed",
        "version_created", "export_performed", "materialization_status", "canonical_sha256",
        "payload_json", "updated_at",
    }
    projection_fields = {
        "schema_version", "source_schema_version", "v3_stage", "v3_stage_complete",
        "v1_projection_stage", "v1_projection_complete", "v2_projection_stage",
        "v2_projection_complete", "projection_status", "legacy_head_transition_id",
        "legacy_head_transition_sha256", "projection_policy_sha256",
    }

    def require_closed(schema: dict, fields: set[str], label: str) -> dict:
        properties = schema.get("properties", {})
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and set(schema.get("required", [])) == fields
            and set(properties) == fields,
            f"{label} must be closed with its exact field set",
        )
        return properties

    transition = load_schema("production-stage-transition-v3.schema.json")
    head = load_schema("production-stage-head-v3.schema.json")
    camera_lock = load_schema("production-camera-lock.schema.json")
    prepare = load_schema("production-stage-transition-v3-prepare-request.schema.json")
    prepare_result = load_schema("production-stage-transition-v3-prepare-result.schema.json")
    get_request = load_schema("production-stage-transition-v3-get-request.schema.json")
    get_result = load_schema("production-stage-transition-v3-get-result.schema.json")

    transition_properties = require_closed(
        transition, transition_fields, "ProductionStageTransition@3"
    )
    head_properties = require_closed(head, head_fields, "ProductionStageHead@3")
    prepare_fields = transition_fields - {
        "approval_summary_sha256", "request_key_sha256", "gate_status", "status",
        "receipt_object_sha256", "canonical_sha256", "created_at",
    }
    prepare_fields |= {"approved", "approval_summary", "idempotency_key"}
    prepare_properties = require_closed(
        prepare, prepare_fields, "ProductionStageTransitionPrepareRequest@3"
    )
    get_request_properties = require_closed(
        get_request,
        {"schema_version", "transition_id", "session_id", "project_id", "root_candidate_id", "head_candidate_id"},
        "ProductionStageTransitionGetRequest@3",
    )
    result_fields = {
        "schema_version", "transition", "production_stage_head", "compatibility_projection",
        "replayed", "runtime_write", "production_stage_advanced", "candidate_confirmed",
        "version_created", "export_performed",
    }
    prepare_result_properties = require_closed(
        prepare_result, result_fields, "ProductionStageTransitionPrepareResult@3"
    )
    get_result_properties = require_closed(
        get_result, result_fields, "ProductionStageTransitionGetResult@3"
    )

    for label, schema, properties in [
        ("ProductionStageTransition@3", transition, transition_properties),
        ("ProductionStageHead@3", head, head_properties),
        ("ProductionStageTransitionPrepareRequest@3", prepare, prepare_properties),
    ]:
        require(
            schema.get("$defs", {}).get("stage", {}).get("enum") == stages,
            f"{label} must expose the exact 19-stage closed enum",
        )
        require(
            schema.get("$defs", {}).get("id", {}).get("pattern")
            == r"^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$",
            f"{label} opaque ids must reject spaces and slashes",
        )
        require(
            properties.get("approval_expires_at", {}).get("$ref") == "#/$defs/epoch",
            f"{label} approval expiry must use bounded decimal epoch seconds",
        )
        for field, values in status_enums.items():
            require(
                properties.get(field, {}).get("$ref") == f"#/$defs/{field}",
                f"{label} {field} must use the closed V3 status enum",
            )

    require(
        stages[0] == "reference-intake"
        and stages[1] == "reference-coverage-reviewed"
        and transition_properties["from_stage"].get("$ref") == "#/$defs/stage"
        and transition_properties["to_stage"].get("$ref") == "#/$defs/stage",
        "ProductionStage@3 first executable edge must be reference-intake to reference-coverage-reviewed",
    )
    for label, schema in [
        ("ProductionStageTransition@3", transition),
        ("ProductionStageTransitionPrepareRequest@3", prepare),
    ]:
        first_edge_guard = any(
            rule.get("if", {}).get("properties", {}).get("from_stage", {}).get("const")
            == "reference-intake"
            and rule.get("then", {}).get("properties", {}).get("to_stage", {}).get("const")
            == "reference-coverage-reviewed"
            for rule in schema.get("allOf", [])
        )
        require(first_edge_guard, f"{label} must guard the first public V3 edge")

    camera_sha_fields = {
        "camera_lock_canonical_sha256", "camera_rig_object_sha256",
        "camera_rig_canonical_sha256", "camera_lock_receipt_object_sha256",
        "camera_lock_source_transition_sha256",
        "camera_lock_source_head_canonical_sha256",
    }
    camera_id_fields = {"camera_lock_id", "camera_lock_source_transition_id"}
    for label, properties in [
        ("ProductionStageTransition@3", transition_properties),
        ("ProductionStageHead@3", head_properties),
        ("ProductionStageTransitionPrepareRequest@3", prepare_properties),
    ]:
        for field in camera_id_fields:
            require(
                properties[field].get("$ref") == "#/$defs/nullable_id",
                f"{label} {field} must be nullable on the additive V3 contract",
            )
        for field in camera_sha_fields:
            require(
                properties[field].get("$ref") == "#/$defs/nullable_sha256",
                f"{label} {field} must be nullable on the additive V3 contract",
            )

    def camera_edge_guard(schema: dict, label: str, *, head: bool = False) -> None:
        guards = schema.get("allOf", [])
        expected_refs = {
            **{field: "#/$defs/nullable_id" for field in camera_id_fields},
            **{field: "#/$defs/nullable_sha256" for field in camera_sha_fields},
        }
        if head:
            edge_guard = any(
                rule.get("if", {}).get("properties", {}).get("previous_head_stage", {}).get("const")
                == "reference-coverage-reviewed"
                and rule.get("if", {}).get("properties", {}).get("head_stage", {}).get("const")
                == "camera-calibrated"
                and all(
                    rule.get("then", {}).get("properties", {}).get(field, {}).get("$ref")
                    == expected_refs[field].replace("nullable_", "")
                    for field in camera_binding_fields
                )
                for rule in guards
            )
        else:
            edge_guard = any(
                rule.get("if", {}).get("properties", {}).get("from_stage", {}).get("const")
                == "reference-coverage-reviewed"
                and rule.get("then", {}).get("properties", {}).get("to_stage", {}).get("const")
                == "camera-calibrated"
                and all(
                    rule.get("then", {}).get("properties", {}).get(field, {}).get("$ref")
                    == expected_refs[field].replace("nullable_", "")
                    for field in camera_binding_fields
                )
                for rule in guards
            )
        require(edge_guard, f"{label} must guard the camera-calibrated edge with a non-null camera lock")

    camera_edge_guard(transition, "ProductionStageTransition@3")
    camera_edge_guard(prepare, "ProductionStageTransitionPrepareRequest@3")
    camera_edge_guard(head, "ProductionStageHead@3", head=True)

    form_edges = [
        ("camera-calibrated", "blockout-reviewed"),
        ("blockout-reviewed", "primary-form-approved"),
        ("primary-form-approved", "secondary-form-approved"),
    ]

    def form_edge_guard(schema: dict, label: str, *, head: bool = False, prepare_request: bool = False) -> None:
        for source_stage, target_stage in form_edges:
            if head:
                matching = [
                    rule for rule in schema.get("allOf", [])
                    if rule.get("if", {}).get("properties", {}).get("previous_head_stage", {}).get("const") == source_stage
                    and rule.get("if", {}).get("properties", {}).get("head_stage", {}).get("const") == target_stage
                ]
            else:
                matching = [
                    rule for rule in schema.get("allOf", [])
                    if rule.get("if", {}).get("properties", {}).get("from_stage", {}).get("const") == source_stage
                ]
            require(len(matching) == 1, f"{label} must define exactly one {source_stage}->{target_stage} FormQuality@2 guard")
            rule = matching[0]
            then = rule.get("then", {})
            properties = then.get("properties", {})
            require(
                (target_stage if head else properties.get("to_stage", {}).get("const")) == target_stage
                and (source_stage if head else properties.get("previous_head_stage", {}).get("const")) == source_stage
                and properties.get("candidate_binding_status", {}).get("const") == "same-candidate-evidence"
                and all(
                    properties.get(field, {}).get("$ref")
                    == ("#/$defs/id" if field in {"camera_lock_id", "camera_lock_source_transition_id"} else "#/$defs/sha256")
                    for field in camera_binding_fields
                )
                and properties.get("quality_report_object_sha256", {}).get("$ref") == "#/$defs/sha256"
                and properties.get("visual_receipt_object_sha256", {}).get("$ref") == "#/$defs/sha256"
                and properties.get("structural_status", {}).get("const") == "PASS_SOURCE_STRUCTURAL"
                and properties.get("visual_status", {}).get("const") == "PASS_STAGE_VISUAL_STRUCTURE_ONLY"
                and properties.get("human_status", {}).get("const") == "NOT_RUN"
                and properties.get("engine_status", {}).get("const") == "NOT_RUN"
                and properties.get("distribution_status", {}).get("const") == "NOT_RUN",
                f"{label} {source_stage}->{target_stage} must require same-candidate structure-only form evidence",
            )
            receipt_kinds = then.get("x-forgecad-receipt-kinds", {})
            require(
                receipt_kinds == {
                    "quality_report_object_sha256": "ProductionWeaponFormQuality@2",
                    "visual_receipt_object_sha256": "ProductionWeaponFormArtEvidence@1",
                },
                f"{label} {source_stage}->{target_stage} receipt kinds drifted",
            )
            if not head and not prepare_request:
                require(
                    properties.get("gate_status", {}).get("const") == "pass"
                    and properties.get("status", {}).get("const") == "passed",
                    f"{label} {source_stage}->{target_stage} durable transition must be passed",
                )

    form_edge_guard(transition, "ProductionStageTransition@3")
    form_edge_guard(prepare, "ProductionStageTransitionPrepareRequest@3", prepare_request=True)
    form_edge_guard(head, "ProductionStageHead@3", head=True)

    def first_camera_null_guard(schema: dict, label: str, *, head: bool = False) -> None:
        def all_null(rule: dict) -> bool:
            properties = rule.get("then", {}).get("properties", {})
            return all(
                field in properties and properties[field].get("const", object()) is None
                for field in camera_binding_fields
            )

        if head:
            found = any(
                rule.get("if", {}).get("properties", {}).get("previous_head_stage", {}).get("const")
                == "reference-intake"
                and rule.get("if", {}).get("properties", {}).get("head_stage", {}).get("const")
                == "reference-coverage-reviewed"
                and all_null(rule)
                for rule in schema.get("allOf", [])
            )
        else:
            found = any(
                rule.get("if", {}).get("properties", {}).get("from_stage", {}).get("const")
                == "reference-intake"
                and all_null(rule)
                for rule in schema.get("allOf", [])
            )
        require(found, f"{label} first edge must force every camera-lock binding field to null")

    first_camera_null_guard(transition, "ProductionStageTransition@3")
    first_camera_null_guard(prepare, "ProductionStageTransitionPrepareRequest@3")
    first_camera_null_guard(head, "ProductionStageHead@3", head=True)
    require(
        camera_lock.get("properties", {}).get("schema_version", {}).get("const")
        == "ProductionCameraLock@1"
        and camera_lock.get("properties", {}).get("calibration_policy", {}).get("const")
        == "fps-weapon-reviewed-six-reference-seven-camera-lock@1"
        and camera_lock.get("properties", {}).get("camera_rig_object_sha256", {}).get("$ref")
        == "#/$defs/sha256"
        and camera_lock.get("properties", {}).get("receipt_object_sha256", {}).get("$ref")
        == "#/$defs/sha256",
        "ProductionStage@3 camera edge must consume the frozen ProductionCameraLock@1 policy",
    )
    require(
        prepare_properties["approved"].get("const") is True
        and "approval_summary" in prepare_properties
        and "approval_summary_sha256" not in prepare_properties
        and not ({"gate_status", "status", "canonical_sha256", "receipt_object_sha256", "created_at"} & set(prepare_properties)),
        "ProductionStageTransitionPrepareRequest@3 must keep approval/raw request fields transient",
    )
    for field in [
        "quality_report_object_sha256",
        "comparison_report_object_sha256",
        "visual_receipt_object_sha256",
        "human_review_receipt_object_sha256",
        "engine_validation_receipt_object_sha256",
        "distribution_receipt_object_sha256",
    ]:
        for label, properties in [
            ("ProductionStageTransition@3", transition_properties),
            ("ProductionStageHead@3", head_properties),
            ("ProductionStageTransitionPrepareRequest@3", prepare_properties),
        ]:
            require(
                properties[field].get("$ref") == "#/$defs/nullable_sha256",
                f"{label} future evidence {field} must be nullable",
            )
    for field in ["parent_transition_id", "parent_transition_sha256", "parent_transition_schema_version"]:
        require(
            transition_properties[field].get("$ref", "").startswith("#/$defs/nullable_"),
            f"ProductionStageTransition@3 {field} must remain nullable for the first edge",
        )
    require(
        transition_properties["candidate_binding_status"].get("$ref")
        == "#/$defs/candidate_binding_status",
        "ProductionStageTransition@3 must expose explicit same/distinct candidate binding",
    )

    projection = head.get("$defs", {}).get("compatibility_projection", {})
    projection_properties = require_closed(projection, projection_fields, "ProductionStageCompatibilityProjection@3")
    require(
        projection_properties["schema_version"].get("const")
        == "ProductionStageCompatibilityProjection@3"
        and projection_properties["v1_projection_stage"].get("$ref")
        == "#/$defs/nullable_coarse_stage"
        and projection_properties["v2_projection_stage"].get("$ref")
        == "#/$defs/nullable_coarse_stage"
        and projection_properties["projection_status"].get("enum")
        == ["exact", "lossy", "not-proven"],
        "ProductionStageCompatibilityProjection@3 must remain a read-only lossy V1/V2 projection",
    )
    require(
        head_properties["compatibility_projection"].get("$ref")
        == "#/$defs/compatibility_projection",
        "ProductionStageHead@3 must carry the nested compatibility projection",
    )
    for schema, properties, runtime_write in [
        (prepare_result, prepare_result_properties, True),
        (get_result, get_result_properties, False),
    ]:
        label = schema.get("title", "ProductionStage@3 result")
        require(
            properties["transition"].get("$ref")
            == "https://forgecad.local/contracts/production-stage-transition-v3.schema.json"
            and properties["production_stage_head"].get("$ref")
            == "https://forgecad.local/contracts/production-stage-head-v3.schema.json"
            and properties["compatibility_projection"].get("$ref")
            == "https://forgecad.local/contracts/production-stage-head-v3.schema.json#/$defs/compatibility_projection"
            and properties["runtime_write"].get("const") is runtime_write
            and properties["candidate_confirmed"].get("const") is False
            and properties["version_created"].get("const") is False
            and properties["export_performed"].get("const") is False,
            f"{label} must preserve V3 nested head/projection and side-effect false flags",
        )
    require(
        get_request_properties["schema_version"].get("const")
        == "ProductionStageTransitionGetRequest@3",
        "ProductionStageTransitionGetRequest@3 schema version drifted",
    )

    # Compatibility regression guards: the additive V3 contract must not
    # widen or rewrite the frozen V1/V2 stage meanings.
    v1 = load_schema("production-stage-transition.schema.json")
    v2 = load_schema("production-stage-transition-v2.schema.json")
    require(
        v1.get("$defs", {}).get("stage", {}).get("enum")
        == coarse_stages
        and v2.get("properties", {}).get("from_stage", {}).get("const") == "topology"
        and v2.get("properties", {}).get("to_stage", {}).get("const") == "material-surface",
        "ProductionStage@3 must remain additive and leave V1/V2 stage meanings frozen",
    )


def check_production_camera_lock_contracts() -> None:
    """Keep the camera lock independent, closed and non-promoting."""
    reference_view_kinds = [
        "front",
        "back",
        "left",
        "right",
        "top",
        "rear-three-quarter",
    ]
    camera_view_kinds = [
        "front",
        "back",
        "left",
        "right",
        "top",
        "bottom",
        "rear-three-quarter",
    ]
    record_fields = {
        "schema_version", "camera_lock_id", "session_id", "project_id",
        "source_transition_id", "source_transition_sha256",
        "source_head_canonical_sha256", "candidate_id", "candidate_state_sha256",
        "artifact_id", "artifact_sha256", "reference_id", "reference_sha256",
        "reference_canvas_object_sha256", "reference_canvas_canonical_sha256",
        "design_spec_object_sha256", "design_spec_canonical_sha256",
        "camera_rig_object_sha256", "camera_rig_canonical_sha256",
        "required_reference_view_kinds", "required_camera_view_kinds",
        "primary_view_kind", "calibration_policy", "review_status",
        "calibration_status", "structural_status", "visual_status", "human_status",
        "engine_status", "distribution_status", "approval_receipt_id",
        "approval_session_id", "approval_expires_at", "approval_summary_sha256",
        "input_sha256", "request_key_sha256", "receipt_object_sha256",
        "canonical_sha256", "created_at",
    }
    prepare_fields = {
        "schema_version", "camera_lock_id", "session_id", "project_id",
        "source_transition_id", "source_transition_sha256",
        "source_head_canonical_sha256", "candidate_id", "candidate_state_sha256",
        "artifact_id", "artifact_sha256", "reference_id", "reference_sha256",
        "required_reference_view_kinds", "required_camera_view_kinds",
        "primary_view_kind", "calibration_policy", "input_sha256", "approved", "camera_rig",
        "approval_receipt_id", "approval_session_id", "approval_expires_at",
        "approval_summary", "idempotency_key",
    }
    get_request_fields = {
        "schema_version", "camera_lock_id", "session_id", "project_id", "candidate_id",
    }
    result_fields = {
        "schema_version", "camera_lock", "replayed", "runtime_write",
        "production_stage_advanced", "candidate_confirmed", "version_created",
        "export_performed", "restart_hash_verified",
    }

    def require_closed(schema: dict, fields: set[str], label: str) -> dict:
        properties = schema.get("properties", {})
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and set(schema.get("required", [])) == fields
            and set(properties) == fields,
            f"{label} must be closed with its exact field set",
        )
        return properties

    record = load_schema("production-camera-lock.schema.json")
    prepare = load_schema("production-camera-lock-prepare-request.schema.json")
    prepare_result = load_schema("production-camera-lock-prepare-result.schema.json")
    get_request = load_schema("production-camera-lock-get-request.schema.json")
    get_result = load_schema("production-camera-lock-get-result.schema.json")

    record_properties = require_closed(record, record_fields, "ProductionCameraLock@1")
    prepare_properties = require_closed(
        prepare, prepare_fields, "ProductionCameraLockPrepareRequest@1"
    )
    get_request_properties = require_closed(
        get_request, get_request_fields, "ProductionCameraLockGetRequest@1"
    )
    prepare_result_properties = require_closed(
        prepare_result, result_fields, "ProductionCameraLockPrepareResult@1"
    )
    get_result_properties = require_closed(
        get_result, result_fields, "ProductionCameraLockGetResult@1"
    )

    require(
        record_properties["schema_version"].get("const") == "ProductionCameraLock@1",
        "ProductionCameraLock@1 schema version drifted",
    )
    require(
        prepare_properties["schema_version"].get("const")
        == "ProductionCameraLockPrepareRequest@1",
        "ProductionCameraLockPrepareRequest@1 schema version drifted",
    )
    require(
        get_request_properties["schema_version"].get("const")
        == "ProductionCameraLockGetRequest@1",
        "ProductionCameraLockGetRequest@1 schema version drifted",
    )
    require(
        record_properties["required_reference_view_kinds"].get("const")
        == reference_view_kinds
        and record_properties["required_camera_view_kinds"].get("const")
        == camera_view_kinds
        and record_properties["primary_view_kind"].get("const") == "left"
        and prepare_properties["required_reference_view_kinds"].get("const")
        == reference_view_kinds
        and prepare_properties["required_camera_view_kinds"].get("const")
        == camera_view_kinds
        and prepare_properties["primary_view_kind"].get("const") == "left",
        "ProductionCameraLock@1 must freeze the six reviewed references and seven camera views",
    )
    require(
        record_properties["calibration_policy"].get("const")
        == "fps-weapon-reviewed-six-reference-seven-camera-lock@1"
        and record_properties["review_status"].get("const")
        == "user-approved-reference-coverage"
        and record_properties["calibration_status"].get("const") == "passed"
        and record_properties["structural_status"].get("const") == "PASS_SOURCE_STRUCTURAL"
        and record_properties["visual_status"].get("const") == "QUALITY_TARGET_NOT_MET"
        and record_properties["human_status"].get("const") == "NOT_RUN"
        and record_properties["engine_status"].get("const") == "NOT_RUN"
        and record_properties["distribution_status"].get("const") == "NOT_RUN",
        "ProductionCameraLock@1 status and calibration policy drifted",
    )
    require(
        prepare_properties["calibration_policy"].get("const")
        == "fps-weapon-reviewed-six-reference-seven-camera-lock@1"
        and prepare_properties["camera_rig"].get("$ref")
        == "https://forgecad.local/contracts/camera-rig-calibration.schema.json"
        and prepare_properties["approved"].get("const") is True
        and prepare_properties["approval_receipt_id"].get("$ref") == "#/$defs/id"
        and prepare_properties["approval_session_id"].get("$ref") == "#/$defs/id"
        and prepare_properties["approval_expires_at"].get("$ref") == "#/$defs/epoch"
        and prepare_properties["approval_summary"].get("type") == "string"
        and prepare_properties["idempotency_key"].get("$ref") == "#/$defs/id",
        "ProductionCameraLockPrepareRequest@1 must carry a complete CameraRigCalibration@1 and current approval binding",
    )

    derived_prepare_fields = record_fields - prepare_fields
    require(
        derived_prepare_fields
        == {
            "reference_canvas_object_sha256", "reference_canvas_canonical_sha256",
            "design_spec_object_sha256", "design_spec_canonical_sha256",
            "camera_rig_object_sha256", "camera_rig_canonical_sha256",
            "review_status", "calibration_status", "structural_status", "visual_status",
            "human_status", "engine_status", "distribution_status",
            "approval_summary_sha256", "request_key_sha256", "receipt_object_sha256",
            "canonical_sha256", "created_at",
        },
        "ProductionCameraLockPrepareRequest@1 must reject Runtime-derived object/canonical/receipt-object/status fields while carrying current approval binding",
    )

    for schema, label in [
        (record, "ProductionCameraLock@1"),
        (prepare, "ProductionCameraLockPrepareRequest@1"),
    ]:
        description = schema.get("description", "").lower()
        require(
            "hq360" in description
            and "six orthographic" in description
            and "bottom" in description,
            f"{label} must distinguish reviewed references, camera helper views and HQ360",
        )

    for filename, schema_version, runtime_write in [
        (
            "production-camera-lock-prepare-result.schema.json",
            "ProductionCameraLockPrepareResult@1",
            True,
        ),
        (
            "production-camera-lock-get-result.schema.json",
            "ProductionCameraLockGetResult@1",
            False,
        ),
    ]:
        result = load_schema(filename)
        properties = require_closed(result, result_fields, schema_version)
        require(
            properties["schema_version"].get("const") == schema_version
            and properties["camera_lock"].get("$ref")
            == "https://forgecad.local/contracts/production-camera-lock.schema.json"
            and properties["runtime_write"].get("const") is runtime_write
            and properties["production_stage_advanced"].get("const") is False
            and properties["candidate_confirmed"].get("const") is False
            and properties["version_created"].get("const") is False
            and properties["export_performed"].get("const") is False
            and properties["restart_hash_verified"].get("const") is True,
            f"{schema_version} must remain restart-verified and non-promoting",
        )

    forbidden_property_names = {
        "path", "file_path", "absolute_path", "url", "uri", "script", "script_path",
        "python", "javascript", "raw", "raw_bytes", "bytes", "environment", "env",
        "secret", "network",
    }

    def property_names(node: object) -> set[str]:
        names: set[str] = set()
        if isinstance(node, dict):
            properties = node.get("properties")
            if isinstance(properties, dict):
                names.update(properties)
            for value in node.values():
                names.update(property_names(value))
        elif isinstance(node, list):
            for value in node:
                names.update(property_names(value))
        return names

    for filename in [
        "production-camera-lock.schema.json",
        "production-camera-lock-prepare-request.schema.json",
        "production-camera-lock-prepare-result.schema.json",
        "production-camera-lock-get-request.schema.json",
        "production-camera-lock-get-result.schema.json",
    ]:
        require(
            forbidden_property_names.isdisjoint(property_names(load_schema(filename))),
            f"{filename} must reject paths, URLs, scripts, raw bytes and environment inputs",
        )


def check_production_weapon_form_quality_contracts() -> None:
    """Keep the additive FPS form-quality receipt bundle-bound and non-promoting."""
    reviewed_reference_view_kinds = [
        "front", "back", "left", "right", "top", "rear-three-quarter",
    ]
    fixed_camera_view_kinds = [
        "front", "back", "left", "right", "top", "bottom", "rear-three-quarter",
    ]
    form_stage_edges = {
        "blockout": ("camera-calibrated", "blockout-reviewed"),
        "primary": ("blockout-reviewed", "primary-form-approved"),
        "secondary": ("primary-form-approved", "secondary-form-approved"),
    }
    record_fields = {
        "schema_version", "form_quality_id", "session_id", "project_id", "form_stage",
        "source_stage", "target_stage", "camera_calibrated_head_transition_id",
        "camera_calibrated_head_transition_sha256", "camera_calibrated_head_canonical_sha256",
        "camera_calibrated_head_candidate_id", "camera_calibrated_head_candidate_state_sha256",
        "camera_calibrated_head_artifact_id", "camera_calibrated_head_artifact_sha256",
        "camera_calibrated_head_stage", "candidate_id", "candidate_state_sha256", "artifact_id",
        "artifact_sha256", "reference_id", "reference_sha256", "reference_canvas_object_sha256",
        "reference_canvas_canonical_sha256", "design_spec_object_sha256", "design_spec_canonical_sha256",
        "camera_lock_id", "camera_lock_canonical_sha256", "camera_rig_object_sha256",
        "camera_rig_canonical_sha256", "camera_lock_receipt_object_sha256",
        "camera_lock_source_transition_id", "camera_lock_source_transition_sha256",
        "camera_lock_source_head_canonical_sha256", "reviewed_reference_view_kinds",
        "fixed_camera_view_kinds", "cross_view_evidence_object_sha256",
        "cross_view_evidence_canonical_sha256", "cross_view_evidence_view_kinds",
        "form_evidence_object_sha256", "form_evidence_canonical_sha256",
        "form_view_evaluations", "previous_form_quality_id",
        "previous_form_quality_report_object_sha256", "previous_form_quality_canonical_sha256",
        "form_quality_policy", "form_quality_policy_sha256", "threshold_policy",
        "threshold_policy_sha256", "layer_status", "hard_gate", "hard_gate_passed",
        "form_gate", "form_gate_passed", "validator_status", "structural_status",
        "visual_status", "human_status", "engine_status", "distribution_status",
        "quality_status", "runtime_write_performed", "production_stage_advanced",
        "candidate_confirmed", "version_created", "export_performed", "request_sha256",
        "input_sha256", "receipt_object_sha256", "canonical_sha256", "created_at",
    }
    prepare_fields = {
        "schema_version", "form_quality_id", "session_id", "project_id", "form_stage",
        "source_stage", "target_stage", "camera_calibrated_head_transition_id",
        "camera_calibrated_head_transition_sha256", "camera_calibrated_head_canonical_sha256",
        "camera_calibrated_head_candidate_id", "camera_calibrated_head_candidate_state_sha256",
        "camera_calibrated_head_artifact_id", "camera_calibrated_head_artifact_sha256",
        "camera_calibrated_head_stage", "candidate_id", "candidate_state_sha256", "artifact_id",
        "artifact_sha256", "reference_id", "reference_sha256", "reference_canvas_object_sha256",
        "reference_canvas_canonical_sha256", "design_spec_object_sha256", "design_spec_canonical_sha256",
        "camera_lock_id", "camera_lock_canonical_sha256", "camera_rig_object_sha256",
        "camera_rig_canonical_sha256", "camera_lock_receipt_object_sha256",
        "camera_lock_source_transition_id", "camera_lock_source_transition_sha256",
        "camera_lock_source_head_canonical_sha256", "reviewed_reference_view_kinds",
        "fixed_camera_view_kinds", "cross_view_evidence_object_sha256",
        "cross_view_evidence_canonical_sha256", "cross_view_evidence_view_kinds",
        "form_evidence_object_sha256", "form_evidence_canonical_sha256",
        "form_view_evaluations", "previous_form_quality_id",
        "previous_form_quality_report_object_sha256", "previous_form_quality_canonical_sha256",
        "form_quality_policy", "form_quality_policy_sha256", "threshold_policy",
        "threshold_policy_sha256", "input_sha256", "idempotency_key",
    }
    get_request_fields = {
        "schema_version", "form_quality_id", "session_id", "project_id", "candidate_id",
        "form_stage",
    }
    result_fields = {
        "schema_version", "form_quality", "replayed", "runtime_write",
        "production_stage_advanced", "candidate_confirmed", "version_created", "export_performed",
    }

    def require_closed(schema: dict, fields: set[str], label: str) -> dict:
        properties = schema.get("properties", {})
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and set(schema.get("required", [])) == fields
            and set(properties) == fields,
            f"{label} must be closed with its exact field set",
        )
        return properties

    record = load_schema("production-weapon-form-quality.schema.json")
    prepare = load_schema("production-weapon-form-quality-prepare-request.schema.json")
    prepare_result = load_schema("production-weapon-form-quality-prepare-result.schema.json")
    get_request = load_schema("production-weapon-form-quality-get-request.schema.json")
    get_result = load_schema("production-weapon-form-quality-get-result.schema.json")

    record_properties = require_closed(record, record_fields, "ProductionWeaponFormQuality@1")
    prepare_properties = require_closed(
        prepare, prepare_fields, "ProductionWeaponFormQualityPrepareRequest@1"
    )
    get_request_properties = require_closed(
        get_request, get_request_fields, "ProductionWeaponFormQualityGetRequest@1"
    )
    prepare_result_properties = require_closed(
        prepare_result, result_fields, "ProductionWeaponFormQualityPrepareResult@1"
    )
    get_result_properties = require_closed(
        get_result, result_fields, "ProductionWeaponFormQualityGetResult@1"
    )

    require(
        record.get("$defs", {}).get("stage", {}).get("enum") == [
            "reference-intake", "reference-coverage-reviewed", "camera-calibrated",
            "blockout-reviewed", "primary-form-approved", "secondary-form-approved",
            "high-poly-approved", "low-poly-approved", "uv-approved", "cage-approved",
            "bake-approved", "material-approved", "rig-socket-approved", "animation-approved",
            "vfx-approved", "lod-collision-approved", "hero-art-review-approved",
            "engine-validated", "export-confirmed",
        ],
        "ProductionWeaponFormQuality@1 must retain the exact ProductionStage@3 stage vocabulary",
    )
    require(
        record_properties["schema_version"].get("const") == "ProductionWeaponFormQuality@1"
        and prepare_properties["schema_version"].get("const")
        == "ProductionWeaponFormQualityPrepareRequest@1"
        and get_request_properties["schema_version"].get("const")
        == "ProductionWeaponFormQualityGetRequest@1",
        "ProductionWeaponFormQuality schema versions drifted",
    )
    require(
        record_properties["reviewed_reference_view_kinds"].get("const")
        == reviewed_reference_view_kinds
        and record_properties["fixed_camera_view_kinds"].get("const")
        == fixed_camera_view_kinds
        and record_properties["cross_view_evidence_view_kinds"].get("const")
        == reviewed_reference_view_kinds
        and prepare_properties["reviewed_reference_view_kinds"].get("const")
        == reviewed_reference_view_kinds
        and prepare_properties["fixed_camera_view_kinds"].get("const")
        == fixed_camera_view_kinds
        and prepare_properties["cross_view_evidence_view_kinds"].get("const")
        == reviewed_reference_view_kinds,
        "ProductionWeaponFormQuality@1 must freeze six reviewed references and seven fixed cameras",
    )
    require(
        record_properties["camera_calibrated_head_stage"].get("const") == "camera-calibrated"
        and prepare_properties["camera_calibrated_head_stage"].get("const") == "camera-calibrated"
        and all(
            field in record_properties
            for field in (
                "camera_calibrated_head_transition_id", "camera_calibrated_head_transition_sha256",
                "camera_calibrated_head_canonical_sha256", "camera_calibrated_head_candidate_id",
                "camera_calibrated_head_candidate_state_sha256", "camera_calibrated_head_artifact_id",
                "camera_calibrated_head_artifact_sha256",
            )
        ),
        "ProductionWeaponFormQuality@1 must bind the current camera-calibrated Stage@3 head",
    )
    for properties, label in [
        (record_properties, "ProductionWeaponFormQuality@1"),
        (prepare_properties, "ProductionWeaponFormQualityPrepareRequest@1"),
    ]:
        require(
            properties["reference_canvas_object_sha256"].get("$ref") == "#/$defs/sha256"
            and properties["reference_canvas_canonical_sha256"].get("$ref") == "#/$defs/sha256"
            and properties["design_spec_object_sha256"].get("$ref") == "#/$defs/sha256"
            and properties["design_spec_canonical_sha256"].get("$ref") == "#/$defs/sha256"
            and properties["cross_view_evidence_object_sha256"].get("$ref") == "#/$defs/sha256"
            and properties["cross_view_evidence_canonical_sha256"].get("$ref") == "#/$defs/sha256",
            f"{label} must bind ReferenceCanvas, DesignSpec and CrossViewEvidenceBundle exact hashes",
        )
        require(
            properties["form_evidence_object_sha256"].get("$ref") == "#/$defs/sha256"
            and properties["form_evidence_canonical_sha256"].get("$ref") == "#/$defs/sha256",
            f"{label} must bind the independent ProductionWeaponFormEvidence parent by object and canonical hash",
        )
        require(
            "reference_view_set_object_sha256" not in properties
            and "reference_view_set_canonical_sha256" not in properties
            and "visual_evidence_views_sha256" not in properties,
            f"{label} must not invent a durable ReferenceViewSet or duplicate visual-evidence parent hash",
        )
        require(
            properties["form_quality_policy"].get("const")
            == "production-weapon-form-quality-six-view-no-regression@1"
            and properties["threshold_policy"].get("const")
            == "production-weapon-form-view-thresholds@1",
            f"{label} form/threshold policy drifted",
        )

    require(
        record_properties["form_view_evaluations"].get("prefixItems")
        and len(record_properties["form_view_evaluations"]["prefixItems"]) == 6
        and record_properties["form_view_evaluations"].get("items") is False
        and prepare_properties["form_view_evaluations"].get("prefixItems")
        and len(prepare_properties["form_view_evaluations"]["prefixItems"]) == 6
        and prepare_properties["form_view_evaluations"].get("items") is False,
        "ProductionWeaponFormQuality@1 must persist exactly six ordered form-view child projections",
    )
    form_view = record.get("$defs", {}).get("form_view", {})
    require(
        set(form_view.get("required", []))
        == {"view_kind", "view_id", "part_id_evidence", "negative_space_evidence", "line_flow_evidence", "no_regression"}
        and set(form_view.get("properties", {})) == set(form_view.get("required", [])),
        "ProductionWeaponFormQuality@1 form-view children must retain only bundle FK plus stage-specific art evidence",
    )
    require(
        not {
            "reference_id", "reference_sha256", "camera_hash", "render_set_object_sha256",
            "comparison_report_object_sha256", "quality_report_object_sha256", "metrics", "thresholds",
        }.intersection(form_view.get("properties", {})),
        "ProductionWeaponFormQuality@1 must not duplicate CrossViewEvidenceBundle image/metric truth",
    )
    evidence_binding = record.get("$defs", {}).get("evidence_binding", {})
    require(
        evidence_binding.get("properties", {}).get("source_kind", {}).get("enum")
        == ["cross-view-evidence-bundle", "design-spec", "not-proven"]
        and any(
            branch.get("if", {}).get("properties", {}).get("source_kind", {}).get("const")
            == "not-proven"
            and branch.get("then", {}).get("properties", {}).get("status", {}).get("const")
            == "NOT_PROVEN"
            for branch in evidence_binding.get("allOf", [])
        ),
        "Part-ID/negative-space/line-flow prepare evidence must bind CrossView/DesignSpec or remain NOT_PROVEN",
    )
    for evidence_name, required_fields in [
        (
            "part_id_evidence",
            {"source", "expected_part_ids", "observed_part_ids", "missing_part_ids", "unexpected_part_ids", "coverage_milli"},
        ),
        (
            "negative_space_evidence",
            {"source", "expected_count", "observed_count", "missing_count", "sealed_count", "coverage_milli"},
        ),
        (
            "line_flow_evidence",
            {"source", "expected_count", "observed_count", "coverage_milli", "continuity_milli", "deviation_milli"},
        ),
    ]:
        evidence = record.get("$defs", {}).get(evidence_name, {})
        require(
            set(evidence.get("required", [])) == required_fields
            and evidence.get("properties", {}).get("source", {}).get("$ref")
            == "#/$defs/evidence_binding",
            f"{evidence_name} must be a closed stage-specific receipt binding",
        )

    for properties, label in [
        (record_properties, "ProductionWeaponFormQuality@1"),
        (prepare_properties, "ProductionWeaponFormQualityPrepareRequest@1"),
    ]:
        guards = record.get("allOf", []) if properties is record_properties else prepare.get("allOf", [])
        for form_stage, (source_stage, target_stage) in form_stage_edges.items():
            matching = [
                guard for guard in guards
                if guard.get("if", {}).get("properties", {}).get("form_stage", {}).get("const")
                == form_stage
            ]
            require(len(matching) == 1, f"{label} must define one frozen {form_stage} edge guard")
            then_properties = matching[0].get("then", {}).get("properties", {})
            require(
                then_properties.get("source_stage", {}).get("const") == source_stage
                and then_properties.get("target_stage", {}).get("const") == target_stage,
                f"{label} {form_stage} edge must map {source_stage} to {target_stage}",
            )
    require(
        "blockout_gate_status" not in record_properties
        and "primary_gate_status" not in record_properties
        and "secondary_gate_status" not in record_properties,
        "ProductionWeaponFormQuality@1 must not compress three independent form records into one multi-layer status row",
    )

    hard_gate = record.get("$defs", {}).get("hard_gate", {})
    form_gate = record.get("$defs", {}).get("form_gate", {})
    require(
        set(hard_gate.get("required", []))
        == {
            "stage_head_binding", "camera_lock_binding", "same_candidate_artifact",
            "reviewed_reference_views", "fixed_camera_views", "cross_view_evidence_binding",
            "form_view_evaluations", "part_id_evidence", "negative_space_evidence",
            "line_flow_evidence", "threshold_policy_binding",
        }
        and set(form_gate.get("required", []))
        == {"layer_status", "all_view_thresholds", "all_view_no_regression", "previous_form_quality_binding"},
        "ProductionWeaponFormQuality@1 hard/form gates must cover binding, thresholds and no-regression",
    )
    require(
        record_properties["quality_status"].get("const") == "structural_only"
        and record_properties["runtime_write_performed"].get("const") is True
        and record_properties["production_stage_advanced"].get("const") is False
        and record_properties["candidate_confirmed"].get("const") is False
        and record_properties["version_created"].get("const") is False
        and record_properties["export_performed"].get("const") is False,
        "ProductionWeaponFormQuality@1 must preserve the structural-only, non-promoting boundary",
    )
    require(
        set(record_fields - prepare_fields)
        == {
            "layer_status", "hard_gate", "hard_gate_passed", "form_gate", "form_gate_passed", "validator_status",
            "structural_status", "visual_status", "human_status", "engine_status",
            "distribution_status", "quality_status", "runtime_write_performed",
            "production_stage_advanced", "candidate_confirmed", "version_created",
            "export_performed", "request_sha256", "receipt_object_sha256", "canonical_sha256",
            "created_at",
        },
        "ProductionWeaponFormQualityPrepareRequest@1 must reject Runtime-derived status/gate/receipt fields",
    )

    cross_view = load_schema("cross-view-evidence-bundle.schema.json")
    require(
        cross_view.get("properties", {}).get("schema_version", {}).get("const")
        == "CrossViewEvidenceBundle@1"
        and {"candidate_id", "artifact_sha256", "view_evaluations", "non_regressing"}.issubset(
            set(cross_view.get("required", []))
        ),
        "ProductionWeaponFormQuality@1 must bind the existing CrossViewEvidenceBundle@1 parent rather than copy its metrics",
    )
    for schema, schema_version, runtime_write in [
        (prepare_result, "ProductionWeaponFormQualityPrepareResult@1", True),
        (get_result, "ProductionWeaponFormQualityGetResult@1", False),
    ]:
        properties = prepare_result_properties if schema is prepare_result else get_result_properties
        require(
            properties["schema_version"].get("const") == schema_version
            and properties["form_quality"].get("$ref")
            == "https://forgecad.local/contracts/production-weapon-form-quality.schema.json"
            and properties["runtime_write"].get("const") is runtime_write
            and properties["production_stage_advanced"].get("const") is False
            and properties["candidate_confirmed"].get("const") is False
            and properties["version_created"].get("const") is False
            and properties["export_performed"].get("const") is False,
            f"{schema_version} must preserve the non-promoting result boundary",
        )
    require(
        get_request_properties["schema_version"].get("const")
        == "ProductionWeaponFormQualityGetRequest@1",
        "ProductionWeaponFormQualityGetRequest@1 schema version drifted",
    )

    forbidden_property_names = {
        "path", "file_path", "absolute_path", "url", "uri", "script", "script_path",
        "python", "javascript", "raw", "raw_bytes", "bytes", "environment", "env",
        "secret", "network",
    }

    def property_names(node: object) -> set[str]:
        names: set[str] = set()
        if isinstance(node, dict):
            properties = node.get("properties")
            if isinstance(properties, dict):
                names.update(properties)
            for value in node.values():
                names.update(property_names(value))
        elif isinstance(node, list):
            for value in node:
                names.update(property_names(value))
        return names

    for schema, filename in [
        (record, "production-weapon-form-quality.schema.json"),
        (prepare, "production-weapon-form-quality-prepare-request.schema.json"),
        (prepare_result, "production-weapon-form-quality-prepare-result.schema.json"),
        (get_request, "production-weapon-form-quality-get-request.schema.json"),
        (get_result, "production-weapon-form-quality-get-result.schema.json"),
    ]:
        require(
            forbidden_property_names.isdisjoint(property_names(schema)),
            f"{filename} must reject paths, URLs, scripts, raw bytes and environment inputs",
        )


def check_production_weapon_form_quality_v2_contracts() -> None:
    """Keep passing FormQuality@2 additive, six-view and source-bound."""
    view_kinds = ["front", "back", "left", "right", "top", "rear-three-quarter"]
    camera_kinds = ["front", "back", "left", "right", "top", "bottom", "rear-three-quarter"]
    edges = {
        "blockout": ("camera-calibrated", "blockout-reviewed"),
        "primary": ("blockout-reviewed", "primary-form-approved"),
        "secondary": ("primary-form-approved", "secondary-form-approved"),
    }
    decision_fields = {
        "view_kind", "legacy_form_quality_view_id", "legacy_form_quality_view_canonical_sha256",
        "form_art_view_id", "form_art_view_canonical_sha256", "form_art_view_receipt_object_sha256",
        "target_object_sha256", "target_canonical_sha256", "silhouette_pass_object_sha256",
        "part_id_pass_object_sha256", "depth_pass_object_sha256", "normal_pass_object_sha256",
        "cross_view_thresholds_passed", "no_regression_passed", "part_id_passed",
        "negative_space_passed", "line_flow_passed", "view_passed",
    }
    aggregate_fields = {
        "view_count", "all_cross_view_thresholds_passed", "all_no_regression_passed",
        "all_part_id_passed", "all_negative_space_passed", "all_line_flow_passed", "all_view_passed",
    }
    record_fields = {
        "schema_version", "form_quality_id", "session_id", "project_id", "form_stage", "source_stage", "target_stage",
        "current_source_head_transition_id", "current_source_head_transition_sha256", "current_source_head_canonical_sha256",
        "current_source_head_stage", "current_source_head_candidate_id", "current_source_head_candidate_state_sha256",
        "current_source_head_artifact_id", "current_source_head_artifact_sha256", "candidate_id", "candidate_state_sha256",
        "artifact_id", "artifact_sha256", "reference_id", "reference_sha256", "reference_canvas_object_sha256",
        "reference_canvas_canonical_sha256", "design_spec_object_sha256", "design_spec_canonical_sha256", "camera_hash",
        "camera_lock_id", "camera_lock_canonical_sha256", "camera_rig_object_sha256", "camera_rig_canonical_sha256",
        "camera_lock_receipt_object_sha256", "camera_lock_source_transition_id", "camera_lock_source_transition_sha256",
        "camera_lock_source_head_canonical_sha256", "reviewed_reference_view_kinds", "fixed_camera_view_kinds",
        "legacy_form_quality_object_sha256", "legacy_form_quality_canonical_sha256", "form_art_evidence_object_sha256",
        "form_art_evidence_canonical_sha256", "view_decisions", "aggregate", "previous_form_quality_id",
        "previous_form_quality_report_object_sha256", "previous_form_quality_canonical_sha256", "form_quality_policy",
        "form_quality_policy_sha256", "threshold_policy", "threshold_policy_sha256", "hard_gate_passed",
        "form_gate_passed", "validator_status", "structural_status", "visual_status", "human_status", "engine_status",
        "distribution_status", "quality_status", "runtime_write_performed", "production_stage_advanced",
        "candidate_confirmed", "version_created", "export_performed", "request_sha256", "input_sha256",
        "receipt_object_sha256", "canonical_sha256", "created_at",
    }
    prepare_fields = {
        "schema_version", "form_quality_id", "session_id", "project_id", "form_stage", "source_stage", "target_stage",
        "legacy_form_quality_object_sha256", "legacy_form_quality_canonical_sha256", "form_art_evidence_object_sha256",
        "form_art_evidence_canonical_sha256", "current_source_head_transition_id", "current_source_head_transition_sha256",
        "current_source_head_canonical_sha256", "previous_form_quality_id", "previous_form_quality_report_object_sha256",
        "previous_form_quality_canonical_sha256", "form_quality_policy", "form_quality_policy_sha256", "threshold_policy",
        "threshold_policy_sha256", "input_sha256", "idempotency_key",
    }
    result_fields = {
        "schema_version", "form_quality", "replayed", "runtime_write", "production_stage_advanced",
        "candidate_confirmed", "version_created", "export_performed",
    }
    get_request_fields = {"schema_version", "form_quality_id", "session_id", "project_id", "candidate_id", "form_stage"}
    get_result_fields = result_fields | {"restart_hash_verified"}
    scope_fields = {
        "source_candidate_id", "source_candidate_state_sha256", "source_artifact_id", "source_artifact_sha256",
        "source_fresh_baseline_id", "source_fresh_baseline_canonical_sha256", "source_fresh_baseline_receipt_object_sha256",
        "source_registration_lineage_id", "source_registration_lineage_canonical_sha256", "source_registration_lineage_receipt_object_sha256",
        "source_registered_rig_v2_id", "source_registered_rig_v2_object_sha256", "source_registered_rig_v2_canonical_sha256",
        "source_runtime_build_cohort_sha256", "proposal_candidate_id", "proposal_candidate_state_sha256", "proposal_artifact_id",
        "proposal_artifact_sha256", "proposal_artifact_readback_sha256", "proposal_worker_build_cohort_sha256",
        "cross_view_evidence_bundle_sha256", "proposal_form_art_evidence_id", "proposal_form_art_evidence_object_sha256",
        "proposal_form_art_evidence_canonical_sha256", "proposal_part_id_evidence_sha256",
        "proposal_negative_space_evidence_sha256", "proposal_line_flow_evidence_sha256", "evidence_source_kind",
    }
    fresh_scope_fields = scope_fields - {"evidence_source_kind"}

    def require_closed(schema: dict, fields: set[str], label: str, optional_fields: set[str] | None = None) -> dict:
        properties = schema.get("properties", {})
        optional_fields = optional_fields or set()
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and set(schema.get("required", [])) == fields - optional_fields
            and set(properties) == fields,
            f"{label} must be closed with its exact field set",
        )
        return properties

    record = load_schema("production-weapon-form-quality-v2.schema.json")
    view = load_schema("production-weapon-form-quality-v2-view.schema.json")
    prepare = load_schema("production-weapon-form-quality-v2-prepare-request.schema.json")
    prepare_result = load_schema("production-weapon-form-quality-v2-prepare-result.schema.json")
    get_request = load_schema("production-weapon-form-quality-v2-get-request.schema.json")
    get_result = load_schema("production-weapon-form-quality-v2-get-result.schema.json")
    record_properties = require_closed(record, record_fields | scope_fields, "ProductionWeaponFormQuality@2", scope_fields - {"evidence_source_kind"})
    view_properties = require_closed(view, decision_fields, "ProductionWeaponFormQualityView@2")
    prepare_properties = require_closed(prepare, prepare_fields | scope_fields, "ProductionWeaponFormQualityPrepareRequest@2", scope_fields - {"evidence_source_kind"})
    prepare_result_properties = require_closed(prepare_result, result_fields, "ProductionWeaponFormQualityPrepareResult@2")
    get_request_properties = require_closed(get_request, get_request_fields | scope_fields, "ProductionWeaponFormQualityGetRequest@2", scope_fields - {"evidence_source_kind"})
    get_result_properties = require_closed(get_result, get_result_fields, "ProductionWeaponFormQualityGetResult@2")

    require(
        record_properties["schema_version"].get("const") == "ProductionWeaponFormQuality@2"
        and view_properties["view_kind"].get("enum") == view_kinds
        and prepare_properties["schema_version"].get("const") == "ProductionWeaponFormQualityPrepareRequest@2"
        and prepare_result_properties["schema_version"].get("const") == "ProductionWeaponFormQualityPrepareResult@2"
        and get_request_properties["schema_version"].get("const") == "ProductionWeaponFormQualityGetRequest@2"
        and get_result_properties["schema_version"].get("const") == "ProductionWeaponFormQualityGetResult@2",
        "ProductionWeaponFormQuality@2 schema versions drifted",
    )
    require(
        record_properties["reviewed_reference_view_kinds"].get("const") == view_kinds
        and record_properties["fixed_camera_view_kinds"].get("const") == camera_kinds
        and record_properties["view_decisions"].get("minItems") == 6
        and record_properties["view_decisions"].get("maxItems") == 6
        and record_properties["view_decisions"].get("items") is False
        and len(record_properties["view_decisions"].get("prefixItems", [])) == 6
        and all(item.get("$ref") == "https://forgecad.local/contracts/production-weapon-form-quality-v2-view.schema.json" for item in record_properties["view_decisions"]["prefixItems"]),
        "ProductionWeaponFormQuality@2 must persist exactly six ordered view decisions",
    )
    decision_sha_fields = {
        "legacy_form_quality_view_canonical_sha256", "form_art_view_canonical_sha256", "form_art_view_receipt_object_sha256",
        "target_object_sha256", "target_canonical_sha256", "silhouette_pass_object_sha256", "part_id_pass_object_sha256",
        "depth_pass_object_sha256", "normal_pass_object_sha256",
    }
    for field in decision_sha_fields:
        require(view_properties[field].get("$ref") == "#/$defs/sha256", f"FormQuality@2 view {field} must be hash-bound")
    for field in ["cross_view_thresholds_passed", "no_regression_passed", "part_id_passed", "negative_space_passed", "line_flow_passed", "view_passed"]:
        require(view_properties[field].get("const") is True, f"FormQuality@2 view {field} must be true in a passing receipt")
    aggregate = record.get("$defs", {}).get("aggregate", {})
    require(
        set(aggregate.get("required", [])) == aggregate_fields
        and set(aggregate.get("properties", {})) == aggregate_fields
        and aggregate["properties"]["view_count"].get("const") == 6
        and all(aggregate["properties"][field].get("const") is True for field in aggregate_fields - {"view_count"}),
        "FormQuality@2 aggregate must be six-view all-pass",
    )
    record_sha_fields = {
        "current_source_head_transition_sha256", "current_source_head_canonical_sha256", "current_source_head_candidate_state_sha256",
        "current_source_head_artifact_sha256", "candidate_state_sha256", "artifact_sha256", "reference_sha256",
        "reference_canvas_object_sha256", "reference_canvas_canonical_sha256", "design_spec_object_sha256",
        "design_spec_canonical_sha256", "camera_hash", "camera_lock_canonical_sha256", "camera_rig_object_sha256",
        "camera_rig_canonical_sha256", "camera_lock_receipt_object_sha256", "camera_lock_source_transition_sha256",
        "camera_lock_source_head_canonical_sha256", "legacy_form_quality_object_sha256", "legacy_form_quality_canonical_sha256",
        "form_art_evidence_object_sha256", "form_art_evidence_canonical_sha256", "form_quality_policy_sha256",
        "threshold_policy_sha256", "request_sha256", "input_sha256", "receipt_object_sha256", "canonical_sha256",
    }
    for field in record_sha_fields:
        require(record_properties[field].get("$ref") == "#/$defs/sha256", f"FormQuality@2 {field} must be a SHA-256 binding")
    require(
        record_properties["evidence_source_kind"].get("enum") == ["legacy-source", "fresh-baseline-proposal"],
        "FormQuality@2 evidence_source_kind must expose the legacy/fresh closed union",
    )
    for properties, label in [
        (record_properties, "record"), (prepare_properties, "prepare"), (get_request_properties, "get"),
    ]:
        for field in scope_fields - {"evidence_source_kind"}:
            expected = "#/$defs/nullable_id" if field.endswith("_id") else "#/$defs/nullable_sha256"
            require(properties[field].get("$ref") == expected, f"FormQuality@2 {label} {field} must be nullable hash/id bound")
        require(properties["evidence_source_kind"].get("enum") == ["legacy-source", "fresh-baseline-proposal"], f"FormQuality@2 {label} source union drifted")
    for schema, label in [(record, "record"), (prepare, "prepare"), (get_request, "get")]:
        fresh_rules = [
            rule for rule in schema.get("allOf", [])
            if rule.get("if", {}).get("properties", {}).get("evidence_source_kind", {}).get("const") == "fresh-baseline-proposal"
        ]
        legacy_rules = [
            rule for rule in schema.get("allOf", [])
            if rule.get("if", {}).get("properties", {}).get("evidence_source_kind", {}).get("const") == "legacy-source"
        ]
        require(len(fresh_rules) == 1 and len(legacy_rules) == 1, f"FormQuality@2 {label} must define both source union branches")
        require(set(fresh_rules[0].get("then", {}).get("required", [])) == fresh_scope_fields, f"FormQuality@2 {label} fresh branch must bind both candidate scopes")
        legacy_properties = legacy_rules[0].get("then", {}).get("properties", {})
        for field in ["proposal_form_art_evidence_id", "proposal_form_art_evidence_object_sha256", "proposal_form_art_evidence_canonical_sha256"]:
            require(legacy_properties.get(field, {}).get("const") is None, f"FormQuality@2 {label} legacy branch must null {field}")
    require(
        record_properties["form_quality_policy"].get("const") == "production-weapon-form-quality-six-view-art-evidence-gate@2"
        and record_properties["threshold_policy"].get("const") == "production-weapon-form-view-thresholds@1"
        and record_properties["hard_gate_passed"].get("const") is True
        and record_properties["form_gate_passed"].get("const") is True
        and record_properties["validator_status"].get("const") == "passed"
        and record_properties["structural_status"].get("const") == "PASS_SOURCE_STRUCTURAL"
        and record_properties["visual_status"].get("const") == "PASS_STAGE_VISUAL_STRUCTURE_ONLY"
        and record_properties["human_status"].get("const") == "NOT_RUN"
        and record_properties["engine_status"].get("const") == "NOT_RUN"
        and record_properties["distribution_status"].get("const") == "NOT_RUN"
        and record_properties["quality_status"].get("const") == "PASS_FORM_GATE"
        and record_properties["runtime_write_performed"].get("const") is True
        and all(record_properties[field].get("const") is False for field in ("production_stage_advanced", "candidate_confirmed", "version_created", "export_performed")),
        "FormQuality@2 must be a passing but non-promoting structure-only receipt",
    )
    for stage, (source, target) in edges.items():
        matching = [rule for rule in record.get("allOf", []) if rule.get("if", {}).get("properties", {}).get("form_stage", {}).get("const") == stage]
        require(len(matching) == 1, f"FormQuality@2 must define one {stage} edge guard")
        props = matching[0].get("then", {}).get("properties", {})
        require(props.get("source_stage", {}).get("const") == source and props.get("target_stage", {}).get("const") == target, f"FormQuality@2 {stage} edge drifted")
    prepare_previous = {
        "previous_form_quality_id": "#/$defs/nullable_id",
        "previous_form_quality_report_object_sha256": "#/$defs/nullable_sha256",
        "previous_form_quality_canonical_sha256": "#/$defs/nullable_sha256",
    }
    for field, ref in prepare_previous.items():
        require(prepare_properties[field].get("$ref") == ref, f"FormQuality@2 prepare {field} must be nullable")
    require(
        prepare_properties["legacy_form_quality_object_sha256"].get("$ref") == "#/$defs/sha256"
        and prepare_properties["form_art_evidence_object_sha256"].get("$ref") == "#/$defs/sha256"
        and prepare_properties["current_source_head_transition_id"].get("$ref") == "#/$defs/id"
        and prepare_properties["current_source_head_transition_sha256"].get("$ref") == "#/$defs/sha256"
        and prepare_properties["current_source_head_canonical_sha256"].get("$ref") == "#/$defs/sha256"
        and prepare_properties["form_quality_policy"].get("const") == "production-weapon-form-quality-six-view-art-evidence-gate@2"
        and prepare_properties["threshold_policy"].get("const") == "production-weapon-form-view-thresholds@1",
        "FormQuality@2 prepare must bind legacy quality, FormArt and current source head only",
    )
    for stage, (source, target) in edges.items():
        matching = [rule for rule in prepare.get("allOf", []) if rule.get("if", {}).get("properties", {}).get("form_stage", {}).get("const") == stage]
        require(len(matching) == 1, f"FormQuality@2 prepare must define one {stage} edge guard")
        props = matching[0].get("then", {}).get("properties", {})
        require(props.get("source_stage", {}).get("const") == source and props.get("target_stage", {}).get("const") == target, f"FormQuality@2 prepare {stage} edge drifted")
        for field in prepare_previous:
            expected = "#/$defs/nullable_" + ("id" if field.endswith("_id") else "sha256")
            if stage == "blockout":
                require(props.get(field, {}).get("const") is None, f"FormQuality@2 blockout previous {field} must be null")
            else:
                require(props.get(field, {}).get("$ref") == expected.replace("nullable_", ""), f"FormQuality@2 {stage} previous {field} must be non-null")
    for schema, properties, schema_version, runtime_write in [
        (prepare_result, prepare_result_properties, "ProductionWeaponFormQualityPrepareResult@2", True),
        (get_result, get_result_properties, "ProductionWeaponFormQualityGetResult@2", False),
    ]:
        require(
            properties["schema_version"].get("const") == schema_version
            and properties["form_quality"].get("$ref") == "https://forgecad.local/contracts/production-weapon-form-quality-v2.schema.json"
            and properties["runtime_write"].get("const") is runtime_write
            and properties["production_stage_advanced"].get("const") is False
            and properties["candidate_confirmed"].get("const") is False
            and properties["version_created"].get("const") is False
            and properties["export_performed"].get("const") is False,
            f"{schema_version} result boundary drifted",
        )
    require(
        get_result_properties["restart_hash_verified"].get("const") is True,
        "FormQuality@2 get must be restart-hash verified",
    )
    forbidden_property_names = {"path", "file_path", "absolute_path", "url", "uri", "script", "script_path", "python", "javascript", "raw", "raw_bytes", "bytes", "environment", "env", "secret", "network", "geometry_program", "operator_id"}

    def property_names(node: object) -> set[str]:
        names: set[str] = set()
        if isinstance(node, dict):
            if isinstance(node.get("properties"), dict):
                names.update(node["properties"])
            for value in node.values():
                names.update(property_names(value))
        elif isinstance(node, list):
            for value in node:
                names.update(property_names(value))
        return names

    for schema, filename in [
        (record, "production-weapon-form-quality-v2.schema.json"), (view, "production-weapon-form-quality-v2-view.schema.json"),
        (prepare, "production-weapon-form-quality-v2-prepare-request.schema.json"), (prepare_result, "production-weapon-form-quality-v2-prepare-result.schema.json"),
        (get_request, "production-weapon-form-quality-v2-get-request.schema.json"), (get_result, "production-weapon-form-quality-v2-get-result.schema.json"),
    ]:
        require(forbidden_property_names.isdisjoint(property_names(schema)), f"{filename} must reject paths, scripts, raw bytes and environment inputs")
    require(load_schema("production-weapon-form-quality.schema.json")["properties"]["schema_version"].get("const") == "ProductionWeaponFormQuality@1", "FormQuality@1 must remain unchanged and additive")


def check_production_weapon_form_quality_v2_preflight_contracts() -> None:
    """Keep the FormQuality@2 preflight projection closed, read-only and hash-bound."""
    request_base_fields = {
        "schema_version", "preflight_id", "session_id", "project_id", "candidate_id", "form_stage",
        "legacy_form_quality_object_sha256", "legacy_form_quality_canonical_sha256",
        "form_art_evidence_object_sha256", "form_art_evidence_canonical_sha256",
        "current_source_head_transition_id", "current_source_head_transition_sha256",
        "current_source_head_canonical_sha256", "input_sha256",
    }
    result_base_fields = {
        "schema_version", "preflight_id", "session_id", "project_id", "candidate_id", "form_stage",
        "checks", "ready_for_v2_prepare", "blocking_reasons", "quality_status", "visual_quality_status",
        "human_review_status", "commercial_engine_status", "runtime_write", "worker_started",
        "production_stage_advanced", "candidate_confirmed", "version_created", "export_performed",
        "restart_hash_verified", "readiness_sha256",
    }
    check_fields = {"status", "reason_code", "object_sha256", "canonical_sha256"}
    check_names = [
        "legacy_form_quality", "form_art_evidence", "form_art_target_observation",
        "cross_view_evidence", "camera_lock_stage", "reference_authoring", "candidate_artifact",
    ]
    fresh_check_names = check_names + ["fresh_baseline_registration", "proposal_candidate_evidence"]
    scope_fields = {
        "source_candidate_id", "source_candidate_state_sha256", "source_artifact_id", "source_artifact_sha256",
        "source_fresh_baseline_id", "source_fresh_baseline_canonical_sha256", "source_fresh_baseline_receipt_object_sha256",
        "source_registration_lineage_id", "source_registration_lineage_canonical_sha256", "source_registration_lineage_receipt_object_sha256",
        "source_registered_rig_v2_id", "source_registered_rig_v2_object_sha256", "source_registered_rig_v2_canonical_sha256",
        "source_runtime_build_cohort_sha256", "proposal_candidate_id", "proposal_candidate_state_sha256", "proposal_artifact_id",
        "proposal_artifact_sha256", "proposal_artifact_readback_sha256", "proposal_worker_build_cohort_sha256",
        "cross_view_evidence_bundle_sha256", "proposal_form_art_evidence_id", "proposal_form_art_evidence_object_sha256",
        "proposal_form_art_evidence_canonical_sha256", "proposal_part_id_evidence_sha256",
        "proposal_negative_space_evidence_sha256", "proposal_line_flow_evidence_sha256", "evidence_source_kind",
    }
    request_fields = request_base_fields | scope_fields
    result_fields = result_base_fields | scope_fields
    form_stages = ["blockout", "primary", "secondary"]
    request_id_fields = {"preflight_id", "session_id", "project_id", "candidate_id", "current_source_head_transition_id"}
    request_hash_fields = request_base_fields - {"schema_version", "form_stage"} - request_id_fields
    reason_codes = [
        "READY",
        "CAMERA_LOCK_OR_STAGE_BLOCKED", "CANDIDATE_MISSING", "CANDIDATE_READ_FAILED",
        "CROSS_VIEW_EVIDENCE_BLOCKED",
        "FORM_ART_EVIDENCE_BINDING_MISMATCH", "FORM_ART_EVIDENCE_MALFORMED",
        "FORM_ART_EVIDENCE_MISSING", "FORM_ART_EVIDENCE_REQUIRED",
        "FORM_ART_EVIDENCE_SCHEMA_MISMATCH", "FORM_ART_EVIDENCE_SCOPE_RETARGET",
        "FORM_ART_EVIDENCE_STORE_MISMATCH", "FORM_ART_EVIDENCE_STORE_ROW_MISSING",
        "FORM_ART_EVIDENCE_STORE_UNAVAILABLE", "FORM_ART_EVIDENCE_UNREADABLE",
        "FORM_ART_EVIDENCE_VIEW_ORDER_MISMATCH", "FORM_ART_TARGET_OBSERVATION_BLOCKED",
        "FORM_ART_VIEW_RECEIPT_BINDING_MISMATCH", "FORM_ART_VIEW_RECEIPT_MALFORMED",
        "FORM_ART_VIEW_RECEIPT_MISSING", "FORM_ART_VIEW_RECEIPT_SCHEMA_MISMATCH",
        "FORM_ART_VIEW_RECEIPT_SELF_REFERENCE", "LEGACY_AND_FORM_ART_REQUIRED",
        "LEGACY_FORM_QUALITY_CANONICAL_MISMATCH", "LEGACY_FORM_QUALITY_MALFORMED",
        "LEGACY_FORM_QUALITY_MISSING", "LEGACY_FORM_QUALITY_REQUIRED",
        "LEGACY_FORM_QUALITY_SCHEMA_MISMATCH", "LEGACY_FORM_QUALITY_SCOPE_RETARGET",
        "LEGACY_FORM_QUALITY_STORE_MISMATCH", "LEGACY_FORM_QUALITY_STORE_ROW_MISSING",
        "LEGACY_FORM_QUALITY_STORE_UNAVAILABLE", "LEGACY_FORM_QUALITY_UNREADABLE",
        "REFERENCE_CANVAS_OR_DESIGN_SPEC_BLOCKED",
    ]

    def require_closed(schema: dict, fields: set[str], label: str, optional_fields: set[str] | None = None) -> dict:
        properties = schema.get("properties", {})
        optional_fields = optional_fields or set()
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and set(schema.get("required", [])) == fields - optional_fields
            and set(properties) == fields,
            f"{label} must be closed with its exact field set",
        )
        return properties

    request = load_schema("production-weapon-form-quality-v2-preflight-get-request.schema.json")
    result = load_schema("production-weapon-form-quality-v2-preflight-get-result.schema.json")
    request_properties = require_closed(
        request, request_fields, "ProductionWeaponFormQualityV2PreflightGetRequest@1", scope_fields - {"evidence_source_kind"}
    )
    result_properties = require_closed(
        result, result_fields, "ProductionWeaponFormQualityV2PreflightGetResult@1", scope_fields - {"evidence_source_kind"}
    )
    require(
        request_properties["schema_version"].get("const")
        == "ProductionWeaponFormQualityV2PreflightGetRequest@1"
        and result_properties["schema_version"].get("const")
        == "ProductionWeaponFormQualityV2PreflightGetResult@1"
        and request_properties["form_stage"].get("enum") == form_stages
        and result_properties["form_stage"].get("enum") == form_stages,
        "FormQuality@2 preflight schema versions or stages drifted",
    )
    require(
        all(request_properties[field].get("$ref") == "#/$defs/id" for field in request_id_fields)
        and all(request_properties[field].get("$ref") == "#/$defs/sha256" for field in request_hash_fields)
        and all(result_properties[field].get("$ref") == "#/$defs/id" for field in (
            "preflight_id", "session_id", "project_id", "candidate_id"
        ))
        and result_properties["readiness_sha256"].get("$ref") == "#/$defs/sha256",
        "FormQuality@2 preflight IDs and hashes must use the closed local definitions",
    )
    for properties, label in [(request_properties, "request"), (result_properties, "result")]:
        require(
            properties["evidence_source_kind"].get("enum") == ["legacy-source", "fresh-baseline-proposal"],
            f"FormQuality@2 preflight {label} source union drifted",
        )
        for field in scope_fields - {"evidence_source_kind"}:
            expected = "#/$defs/nullable_id" if field.endswith("_id") else "#/$defs/nullable_sha256"
            require(
                properties[field].get("$ref") == expected,
                f"FormQuality@2 preflight {label} {field} must be nullable hash/id bound",
            )

    checks = result_properties["checks"]
    check_alternatives = checks.get("oneOf", [])
    require(len(check_alternatives) == 2, "FormQuality@2 preflight checks must expose legacy/fresh map union")
    for alternative, names in zip(check_alternatives, [check_names, fresh_check_names]):
        require(
            alternative.get("type") == "object"
            and alternative.get("additionalProperties") is False
            and alternative.get("minProperties") == len(names)
            and alternative.get("maxProperties") == len(names)
            and set(alternative.get("required", [])) == set(names)
            and list(alternative.get("properties", {})) == names
            and all(alternative["properties"][name].get("$ref") == "#/$defs/check" for name in names),
            "FormQuality@2 preflight check union branch is not closed",
        )
    check = result.get("$defs", {}).get("check", {})
    check_properties = require_closed(check, check_fields, "ProductionWeaponFormQualityV2PreflightCheck@1")
    require(
        check_properties["status"].get("enum") == ["ready", "blocked", "invalid"]
        and check_properties["reason_code"].get("enum") == reason_codes
        and check_properties["object_sha256"].get("$ref") == "#/$defs/nullable_sha256"
        and check_properties["canonical_sha256"].get("$ref") == "#/$defs/nullable_sha256",
        "FormQuality@2 preflight check status, reason and hash fields drifted",
    )
    require(
        result_properties["blocking_reasons"].get("$ref") is None
        and result_properties["blocking_reasons"].get("minItems") == 0
        and result_properties["blocking_reasons"].get("maxItems") == len(fresh_check_names)
        and result_properties["blocking_reasons"].get("uniqueItems") is True
        and result_properties["blocking_reasons"].get("items", {}).get("$ref")
        == "#/$defs/blocking_reason",
        "FormQuality@2 preflight blockers must be a bounded unique reason projection",
    )
    require(
        result.get("$defs", {}).get("blocking_reason", {}).get("pattern")
        == "^(legacy_form_quality|form_art_evidence|form_art_target_observation|cross_view_evidence|camera_lock_stage|reference_authoring|candidate_artifact|fresh_baseline_registration|proposal_candidate_evidence):[A-Z][A-Z0-9_]*$",
        "FormQuality@2 preflight blockers must remain check-name qualified",
    )
    require(
        result_properties["ready_for_v2_prepare"].get("type") == "boolean"
        and result_properties["quality_status"].get("const") == "NOT_PROVEN"
        and result_properties["visual_quality_status"].get("const") == "NOT_PROVEN"
        and result_properties["human_review_status"].get("const") == "NOT_RUN"
        and result_properties["commercial_engine_status"].get("const") == "NOT_RUN"
        and result_properties["restart_hash_verified"].get("const") is True
        and all(result_properties[field].get("const") is False for field in (
            "runtime_write", "worker_started", "production_stage_advanced",
            "candidate_confirmed", "version_created", "export_performed",
        )),
        "FormQuality@2 preflight must be NOT_PROVEN, restart-verified and non-promoting",
    )
    require(
        request.get("$defs", {}).get("id", {}).get("pattern")
        == "^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"
        and request.get("$defs", {}).get("sha256", {}).get("pattern") == "^[0-9a-f]{64}$"
        and result.get("$defs", {}).get("id", {}).get("pattern")
        == "^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"
        and result.get("$defs", {}).get("sha256", {}).get("pattern") == "^[0-9a-f]{64}$",
        "FormQuality@2 preflight ID and SHA-256 patterns must remain strict",
    )
    forbidden_property_names = {
        "path", "file_path", "absolute_path", "url", "uri", "script", "script_path",
        "python", "javascript", "raw", "raw_bytes", "bytes", "environment", "env",
        "secret", "network",
    }

    def property_names(node: object) -> set[str]:
        names: set[str] = set()
        if isinstance(node, dict):
            properties = node.get("properties")
            if isinstance(properties, dict):
                names.update(properties)
            for value in node.values():
                names.update(property_names(value))
        elif isinstance(node, list):
            for value in node:
                names.update(property_names(value))
        return names

    for schema, filename in [
        (request, "production-weapon-form-quality-v2-preflight-get-request.schema.json"),
        (result, "production-weapon-form-quality-v2-preflight-get-result.schema.json"),
    ]:
        require(
            forbidden_property_names.isdisjoint(property_names(schema)),
            f"{filename} must reject paths, URLs, scripts, raw bytes and environment inputs",
        )


def check_production_weapon_form_evidence_contracts() -> None:
    """Keep six-view typed form evidence independently bound and non-promoting."""
    view_kinds = ["front", "back", "left", "right", "top", "rear-three-quarter"]
    parent_fields = {
        "schema_version", "form_evidence_id", "session_id", "project_id", "candidate_id",
        "candidate_state_sha256", "artifact_id", "artifact_sha256",
        "reference_canvas_object_sha256", "reference_canvas_canonical_sha256",
        "design_spec_object_sha256", "design_spec_canonical_sha256", "camera_lock_id",
        "camera_lock_canonical_sha256", "camera_rig_object_sha256", "camera_rig_canonical_sha256",
        "camera_lock_receipt_object_sha256", "camera_lock_source_transition_id",
        "camera_lock_source_transition_sha256", "camera_lock_source_head_canonical_sha256",
        "view_kinds", "views", "evidence_policy", "evidence_policy_sha256", "quality_status",
        "runtime_write_performed", "production_stage_advanced", "candidate_confirmed",
        "version_created", "export_performed", "request_sha256", "input_sha256",
        "receipt_object_sha256", "canonical_sha256", "created_at",
    }
    prepare_fields = {
        "schema_version", "form_evidence_id", "session_id", "project_id", "candidate_id",
        "candidate_state_sha256", "artifact_id", "artifact_sha256",
        "reference_canvas_object_sha256", "reference_canvas_canonical_sha256",
        "design_spec_object_sha256", "design_spec_canonical_sha256", "camera_lock_id",
        "camera_lock_canonical_sha256", "camera_rig_object_sha256", "camera_rig_canonical_sha256",
        "camera_lock_receipt_object_sha256", "camera_lock_source_transition_id",
        "camera_lock_source_transition_sha256", "camera_lock_source_head_canonical_sha256",
        "view_kinds", "views", "evidence_policy", "evidence_policy_sha256", "input_sha256",
        "idempotency_key",
    }
    view_fields = {
        "schema_version", "project_id", "candidate_id", "candidate_state_sha256", "artifact_id",
        "artifact_sha256", "view_kind", "view_id", "reference_id", "reference_sha256",
        "camera_hash", "camera_canonical_sha256", "render_set_object_sha256",
        "render_set_canonical_sha256", "render_set_view_id", "part_id_evidence",
        "negative_space_evidence", "line_flow_evidence", "view_observation_status",
        "quality_status", "receipt_object_sha256", "canonical_sha256", "created_at",
    }
    view_input_fields = {
        "view_kind", "view_id", "reference_id", "reference_sha256", "camera_hash",
        "camera_canonical_sha256", "render_set_object_sha256", "render_set_canonical_sha256",
        "render_set_view_id",
    }
    result_fields = {
        "schema_version", "form_evidence", "replayed", "runtime_write",
        "production_stage_advanced", "candidate_confirmed", "version_created", "export_performed",
    }
    get_result_fields = result_fields | {"restart_hash_verified"}

    def require_closed(schema: dict, fields: set[str], label: str) -> dict:
        properties = schema.get("properties", {})
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and set(schema.get("required", [])) == fields
            and set(properties) == fields,
            f"{label} must be closed with its exact field set",
        )
        return properties

    parent = load_schema("production-weapon-form-evidence.schema.json")
    view = load_schema("production-weapon-form-evidence-view.schema.json")
    prepare = load_schema("production-weapon-form-evidence-prepare-request.schema.json")
    prepare_result = load_schema("production-weapon-form-evidence-prepare-result.schema.json")
    get_request = load_schema("production-weapon-form-evidence-get-request.schema.json")
    get_result = load_schema("production-weapon-form-evidence-get-result.schema.json")

    parent_properties = require_closed(parent, parent_fields, "ProductionWeaponFormEvidence@1")
    view_properties = require_closed(view, view_fields, "ProductionWeaponFormEvidenceView@1")
    prepare_properties = require_closed(
        prepare, prepare_fields, "ProductionWeaponFormEvidencePrepareRequest@1"
    )
    prepare_result_properties = require_closed(
        prepare_result, result_fields, "ProductionWeaponFormEvidencePrepareResult@1"
    )
    get_request_properties = require_closed(
        get_request,
        {"schema_version", "form_evidence_id", "session_id", "project_id", "candidate_id"},
        "ProductionWeaponFormEvidenceGetRequest@1",
    )
    get_result_properties = require_closed(
        get_result, get_result_fields, "ProductionWeaponFormEvidenceGetResult@1"
    )

    require(
        parent_properties["schema_version"].get("const") == "ProductionWeaponFormEvidence@1"
        and view_properties["schema_version"].get("const") == "ProductionWeaponFormEvidenceView@1"
        and prepare_properties["schema_version"].get("const")
        == "ProductionWeaponFormEvidencePrepareRequest@1"
        and prepare_result_properties["schema_version"].get("const")
        == "ProductionWeaponFormEvidencePrepareResult@1"
        and get_request_properties["schema_version"].get("const")
        == "ProductionWeaponFormEvidenceGetRequest@1"
        and get_result_properties["schema_version"].get("const")
        == "ProductionWeaponFormEvidenceGetResult@1",
        "ProductionWeaponFormEvidence schema versions drifted",
    )
    for properties, label in [
        (parent_properties, "ProductionWeaponFormEvidence@1"),
        (prepare_properties, "ProductionWeaponFormEvidencePrepareRequest@1"),
    ]:
        for field in (
            "candidate_state_sha256", "artifact_sha256", "reference_canvas_object_sha256",
            "reference_canvas_canonical_sha256", "design_spec_object_sha256",
            "design_spec_canonical_sha256", "camera_lock_canonical_sha256",
            "camera_rig_object_sha256", "camera_rig_canonical_sha256",
            "camera_lock_receipt_object_sha256", "camera_lock_source_transition_sha256",
            "camera_lock_source_head_canonical_sha256", "evidence_policy_sha256",
        ):
            require(
                properties[field].get("$ref") == "#/$defs/sha256",
                f"{label} {field} must be a SHA-256 binding",
            )
        require(
            properties["evidence_policy"].get("const")
            == "production-weapon-form-evidence-six-view-typed-observation@1",
            f"{label} evidence policy drifted",
        )
        require(
            properties["view_kinds"].get("const") == view_kinds,
            f"{label} must freeze exactly six reviewed view kinds",
        )
    require(
        parent_properties["views"].get("prefixItems")
        and len(parent_properties["views"]["prefixItems"]) == 6
        and parent_properties["views"].get("items") is False
        and parent_properties["views"].get("minItems") == 6
        and parent_properties["views"].get("maxItems") == 6
        and parent_properties["views"].get("uniqueItems") is True
        and all(
            item.get("$ref")
            == "https://forgecad.local/contracts/production-weapon-form-evidence-view.schema.json"
            for item in parent_properties["views"]["prefixItems"]
        ),
        "ProductionWeaponFormEvidence@1 must persist exactly six ordered view children",
    )
    require(
        prepare_properties["views"].get("prefixItems")
        and len(prepare_properties["views"]["prefixItems"]) == 6
        and prepare_properties["views"].get("items") is False
        and prepare_properties["views"].get("minItems") == 6
        and prepare_properties["views"].get("maxItems") == 6
        and prepare_properties["views"].get("uniqueItems") is True
        and [item.get("$ref") for item in prepare_properties["views"]["prefixItems"]]
        == [
            "#/$defs/view_input_front", "#/$defs/view_input_back", "#/$defs/view_input_left",
            "#/$defs/view_input_right", "#/$defs/view_input_top",
            "#/$defs/view_input_rear_three_quarter",
        ],
        "ProductionWeaponFormEvidencePrepareRequest@1 must bind exactly six ordered view inputs",
    )
    for properties, label in [(view_properties, "ProductionWeaponFormEvidenceView@1")]:
        for field in (
            "candidate_state_sha256", "artifact_sha256", "reference_sha256", "camera_hash",
            "camera_canonical_sha256", "render_set_object_sha256",
            "render_set_canonical_sha256", "receipt_object_sha256", "canonical_sha256",
        ):
            require(
                properties[field].get("$ref") == "#/$defs/sha256",
                f"{label} {field} must be a SHA-256 binding",
            )
        require(
            properties["view_kind"].get("enum") == view_kinds
            and properties["view_observation_status"].get("enum")
            == ["observed", "inferred", "unknown"]
            and properties["quality_status"].get("const") == "NOT_PROVEN",
            f"{label} must preserve observed/inferred/unknown and NOT_PROVEN boundaries",
        )
    view_input = prepare.get("$defs", {}).get("view_input", {})
    require(
        set(view_input.get("required", [])) == view_input_fields
        and set(view_input.get("properties", {})) == view_input_fields
        and view_input.get("properties", {}).get("view_kind", {}).get("enum") == view_kinds,
        "ProductionWeaponFormEvidencePrepareRequest@1 view inputs must be closed and hash-bound",
    )
    for evidence_name, fields, kind in [
        (
            "part_id_evidence",
            {"observation", "expected_part_ids", "observed_part_ids", "missing_part_ids", "unexpected_part_ids", "coverage_milli"},
            "part-id",
        ),
        (
            "negative_space_evidence",
            {"observation", "expected_count", "observed_count", "missing_count", "sealed_count", "coverage_milli"},
            "negative-space",
        ),
        (
            "line_flow_evidence",
            {"observation", "expected_count", "observed_count", "coverage_milli", "continuity_milli", "deviation_milli"},
            "line-flow",
        ),
    ]:
        evidence = view.get("$defs", {}).get(evidence_name, {})
        require(
            set(evidence.get("required", [])) == fields
            and evidence.get("properties", {}).get("observation", {}).get("allOf")
            and evidence["properties"]["observation"]["allOf"][1]
            .get("properties", {}).get("evidence_kind", {}).get("const") == kind,
            f"{evidence_name} must be a typed {kind} observation",
        )
        observation = view.get("$defs", {}).get("observation", {})
        require(
            observation.get("properties", {}).get("observation_status", {}).get("enum")
            == ["observed", "inferred", "unknown"]
            and observation.get("properties", {}).get("quality_status", {}).get("const")
            == "NOT_PROVEN",
            "typed form evidence must never claim quality PASS",
        )
    require(
        parent_properties["quality_status"].get("const") == "NOT_PROVEN"
        and parent_properties["runtime_write_performed"].get("const") is True
        and parent_properties["production_stage_advanced"].get("const") is False
        and parent_properties["candidate_confirmed"].get("const") is False
        and parent_properties["version_created"].get("const") is False
        and parent_properties["export_performed"].get("const") is False,
        "ProductionWeaponFormEvidence@1 must remain evidence-only and non-promoting",
    )
    require(
        prepare_result_properties["form_evidence"].get("$ref")
        == "https://forgecad.local/contracts/production-weapon-form-evidence.schema.json"
        and prepare_result_properties["runtime_write"].get("const") is True
        and all(
            prepare_result_properties[field].get("const") is False
            for field in ("production_stage_advanced", "candidate_confirmed", "version_created", "export_performed")
        ),
        "ProductionWeaponFormEvidencePrepareResult@1 must be a non-promoting write result",
    )
    require(
        get_result_properties["form_evidence"].get("$ref")
        == "https://forgecad.local/contracts/production-weapon-form-evidence.schema.json"
        and get_result_properties["runtime_write"].get("const") is False
        and get_result_properties["restart_hash_verified"].get("const") is True
        and all(
            get_result_properties[field].get("const") is False
            for field in ("production_stage_advanced", "candidate_confirmed", "version_created", "export_performed")
        ),
        "ProductionWeaponFormEvidenceGetResult@1 must be read-only and restart verified",
    )
    forbidden_property_names = {
        "path", "file_path", "absolute_path", "url", "uri", "script", "script_path",
        "python", "javascript", "raw", "raw_bytes", "bytes", "environment", "env",
        "secret", "network", "geometry_program", "operator_id",
    }

    def property_names(node: object) -> set[str]:
        names: set[str] = set()
        if isinstance(node, dict):
            properties = node.get("properties")
            if isinstance(properties, dict):
                names.update(properties)
            for value in node.values():
                names.update(property_names(value))
        elif isinstance(node, list):
            for value in node:
                names.update(property_names(value))
        return names

    for schema, filename in [
        (parent, "production-weapon-form-evidence.schema.json"),
        (view, "production-weapon-form-evidence-view.schema.json"),
        (prepare, "production-weapon-form-evidence-prepare-request.schema.json"),
        (prepare_result, "production-weapon-form-evidence-prepare-result.schema.json"),
        (get_request, "production-weapon-form-evidence-get-request.schema.json"),
        (get_result, "production-weapon-form-evidence-get-result.schema.json"),
    ]:
        require(
            forbidden_property_names.isdisjoint(property_names(schema)),
            f"{filename} must reject geometry, paths, URLs, scripts, raw bytes and environment inputs",
        )


def check_production_weapon_form_art_evidence_contracts() -> None:
    """Keep additive six-view art evidence closed, bounded and non-promoting."""
    view_kinds = ["front", "back", "left", "right", "top", "rear-three-quarter"]
    parent_fields = {
        "schema_version", "art_evidence_id", "session_id", "project_id", "candidate_id",
        "candidate_state_sha256", "artifact_id", "artifact_sha256",
        "reference_canvas_object_sha256", "reference_canvas_canonical_sha256",
        "design_spec_object_sha256", "design_spec_canonical_sha256", "camera_lock_id",
        "camera_lock_canonical_sha256", "camera_rig_object_sha256", "camera_rig_canonical_sha256",
        "camera_lock_receipt_object_sha256", "camera_lock_source_transition_id",
        "camera_lock_source_transition_sha256", "camera_lock_source_head_canonical_sha256",
        "form_evidence_object_sha256", "form_evidence_canonical_sha256", "view_kinds", "views",
        "part_id_aggregate", "art_evidence_policy", "art_evidence_policy_sha256", "quality_status",
        "runtime_write_performed", "production_stage_advanced", "candidate_confirmed",
        "version_created", "export_performed", "request_sha256", "input_sha256",
        "receipt_object_sha256", "canonical_sha256", "created_at",
    }
    view_fields = {
        "schema_version", "project_id", "candidate_id", "candidate_state_sha256", "artifact_id",
        "artifact_sha256", "view_kind", "view_id", "reference_id", "reference_sha256",
        "camera_hash", "camera_canonical_sha256", "form_evidence_view_receipt_object_sha256",
        "form_evidence_view_receipt_canonical_sha256", "target_object_sha256", "target_canonical_sha256",
        "visual_structure_canonical_sha256", "visual_structure_review_status",
        "silhouette_pass_object_sha256", "part_id_pass_object_sha256", "depth_pass_object_sha256",
        "normal_pass_object_sha256", "part_id_status", "part_id_expected_count",
        "part_id_observed_count", "part_id_missing_count", "part_id_unexpected_count",
        "part_id_coverage_milli", "negative_space_status", "negative_space_rows",
        "line_flow_status", "line_flow_rows", "view_observation_status", "quality_status",
        "receipt_object_sha256", "canonical_sha256", "created_at",
    }
    prepare_fields = {
        "schema_version", "art_evidence_id", "session_id", "project_id", "candidate_id",
        "form_evidence_object_sha256", "form_evidence_canonical_sha256", "art_evidence_policy",
        "art_evidence_policy_sha256", "input_sha256", "idempotency_key",
    }
    result_fields = {
        "schema_version", "art_evidence", "replayed", "runtime_write",
        "production_stage_advanced", "candidate_confirmed", "version_created", "export_performed",
    }
    get_request_required_fields = {"schema_version", "art_evidence_id", "session_id", "project_id", "candidate_id"}
    get_request_fields = get_request_required_fields | {"raster_source_attribution_diagnostic"}
    get_result_required_fields = result_fields | {"restart_hash_verified"}
    get_result_fields = get_result_required_fields | {"raster_source_attribution_diagnostic"}

    def require_closed(schema: dict, fields: set[str], label: str) -> dict:
        properties = schema.get("properties", {})
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and set(schema.get("required", [])) == fields
            and set(properties) == fields,
            f"{label} must be closed with its exact field set",
        )
        return properties

    parent = load_schema("production-weapon-form-art-evidence.schema.json")
    view = load_schema("production-weapon-form-art-evidence-view.schema.json")
    prepare = load_schema("production-weapon-form-art-evidence-prepare-request.schema.json")
    prepare_result = load_schema("production-weapon-form-art-evidence-prepare-result.schema.json")
    get_request = load_schema("production-weapon-form-art-evidence-get-request.schema.json")
    get_result = load_schema("production-weapon-form-art-evidence-get-result.schema.json")
    parent_properties = require_closed(parent, parent_fields, "ProductionWeaponFormArtEvidence@1")
    view_properties = require_closed(view, view_fields, "ProductionWeaponFormArtEvidenceView@1")
    prepare_properties = require_closed(
        prepare, prepare_fields, "ProductionWeaponFormArtEvidencePrepareRequest@1"
    )
    prepare_result_properties = require_closed(
        prepare_result, result_fields, "ProductionWeaponFormArtEvidencePrepareResult@1"
    )
    get_request_properties = get_request.get("properties", {})
    require(
        get_request.get("type") == "object"
        and get_request.get("additionalProperties") is False
        and set(get_request.get("required", [])) == get_request_required_fields
        and set(get_request_properties) == get_request_fields,
        "ProductionWeaponFormArtEvidenceGetRequest@1 must be closed with its exact optional diagnostic field",
    )
    get_result_properties = get_result.get("properties", {})
    require(
        get_result.get("type") == "object"
        and get_result.get("additionalProperties") is False
        and set(get_result.get("required", [])) == get_result_required_fields
        and set(get_result_properties) == get_result_fields,
        "ProductionWeaponFormArtEvidenceGetResult@1 must be closed with its exact optional diagnostic field",
    )
    raster_request_ref = get_request_properties["raster_source_attribution_diagnostic"].get("$ref")
    raster_result_ref = get_result_properties["raster_source_attribution_diagnostic"].get("$ref")
    require(
        raster_request_ref == "#/$defs/rasterAttributionRequest"
        and raster_result_ref == "#/$defs/rasterAttributionResult"
        and get_request.get("$defs", {}).get("rasterAttributionRequest", {}).get("additionalProperties") is False
        and get_result.get("$defs", {}).get("rasterAttributionResult", {}).get("additionalProperties") is False
        and get_result.get("$defs", {}).get("diagnostic", {}).get("additionalProperties") is False
        and get_result.get("$defs", {}).get("sourceRow", {}).get("additionalProperties") is False,
        "ProductionWeaponFormArtEvidence raster attribution request/result must remain closed",
    )

    versions = {
        "schema_version": "ProductionWeaponFormArtEvidence@1",
        "view": "ProductionWeaponFormArtEvidenceView@1",
        "prepare": "ProductionWeaponFormArtEvidencePrepareRequest@1",
        "prepare_result": "ProductionWeaponFormArtEvidencePrepareResult@1",
        "get": "ProductionWeaponFormArtEvidenceGetRequest@1",
        "get_result": "ProductionWeaponFormArtEvidenceGetResult@1",
    }
    require(
        parent_properties["schema_version"].get("const") == versions["schema_version"]
        and view_properties["schema_version"].get("const") == versions["view"]
        and prepare_properties["schema_version"].get("const") == versions["prepare"]
        and prepare_result_properties["schema_version"].get("const") == versions["prepare_result"]
        and get_request_properties["schema_version"].get("const") == versions["get"]
        and get_result_properties["schema_version"].get("const") == versions["get_result"],
        "ProductionWeaponFormArtEvidence schema versions drifted",
    )
    require(
        parent_properties["view_kinds"].get("const") == view_kinds
        and view_properties["view_kind"].get("enum") == view_kinds,
        "ProductionWeaponFormArtEvidence must freeze exactly six view kinds",
    )
    view_refs = parent_properties["views"]
    require(
        view_refs.get("minItems") == 6
        and view_refs.get("maxItems") == 6
        and view_refs.get("uniqueItems") is True
        and view_refs.get("items") is False
        and len(view_refs.get("prefixItems", [])) == 6
        and all(
            item.get("$ref")
            == "https://forgecad.local/contracts/production-weapon-form-art-evidence-view.schema.json"
            for item in view_refs["prefixItems"]
        ),
        "ProductionWeaponFormArtEvidence must persist exactly six ordered view children",
    )

    parent_sha_fields = {
        "candidate_state_sha256", "artifact_sha256", "reference_canvas_object_sha256",
        "reference_canvas_canonical_sha256", "design_spec_object_sha256", "design_spec_canonical_sha256",
        "camera_lock_canonical_sha256", "camera_rig_object_sha256", "camera_rig_canonical_sha256",
        "camera_lock_receipt_object_sha256", "camera_lock_source_transition_sha256",
        "camera_lock_source_head_canonical_sha256", "form_evidence_object_sha256",
        "form_evidence_canonical_sha256", "art_evidence_policy_sha256", "request_sha256",
        "input_sha256", "receipt_object_sha256", "canonical_sha256",
    }
    for field in parent_sha_fields:
        require(
            parent_properties[field].get("$ref") == "#/$defs/sha256",
            f"ProductionWeaponFormArtEvidence@1 {field} must be a SHA-256 binding",
        )
    for field in ("art_evidence_policy", "quality_status"):
        expected = (
            "production-weapon-form-art-evidence-six-view-typed-observation@1"
            if field == "art_evidence_policy" else "NOT_PROVEN"
        )
        require(
            parent_properties[field].get("const") == expected,
            f"ProductionWeaponFormArtEvidence@1 {field} is not frozen",
        )
    require(
        parent_properties["runtime_write_performed"].get("const") is True
        and all(parent_properties[field].get("const") is False for field in (
            "production_stage_advanced", "candidate_confirmed", "version_created", "export_performed"
        )),
        "ProductionWeaponFormArtEvidence@1 must remain non-promoting",
    )

    view_sha_fields = {
        "candidate_state_sha256", "artifact_sha256", "reference_sha256", "camera_hash",
        "camera_canonical_sha256", "form_evidence_view_receipt_object_sha256",
        "form_evidence_view_receipt_canonical_sha256", "target_object_sha256", "target_canonical_sha256",
        "visual_structure_canonical_sha256", "silhouette_pass_object_sha256", "part_id_pass_object_sha256",
        "depth_pass_object_sha256", "normal_pass_object_sha256", "receipt_object_sha256", "canonical_sha256",
    }
    for field in view_sha_fields:
        require(
            view_properties[field].get("$ref") == "#/$defs/sha256",
            f"ProductionWeaponFormArtEvidenceView@1 {field} must be a SHA-256 binding",
        )
    require(
        view_properties["quality_status"].get("const") == "NOT_PROVEN"
        and view_properties["visual_structure_review_status"].get("enum")
        == ["user_confirmed", "inferred", "unknown"]
        and view_properties["negative_space_status"].get("enum")
        == ["observed", "inferred", "unknown", "not-applicable"]
        and view_properties["line_flow_status"].get("enum")
        == ["observed", "inferred", "unknown", "not-applicable"],
        "ProductionWeaponFormArtEvidenceView@1 must preserve observation truth states",
    )

    part_aggregate = parent.get("$defs", {}).get("part_id_aggregate", {})
    require(
        set(part_aggregate.get("required", []))
        == {"status", "expected_count", "observed_count", "missing_count", "unexpected_count", "coverage_milli"}
        and set(part_aggregate.get("properties", {}))
        == {"status", "expected_count", "observed_count", "missing_count", "unexpected_count", "coverage_milli"}
        and part_aggregate["properties"]["status"].get("enum") == ["observed", "inferred", "unknown"]
        and part_aggregate["properties"]["coverage_milli"].get("maximum") == 1000,
        "ProductionWeaponFormArtEvidence part-id aggregate must be bounded and typed",
    )
    row_specs = [
        (
            "negative_space_row",
            {"structure_id", "expected_region_canonical_sha256", "iou_milli", "boundary_f1_milli", "area_ratio_milli", "centroid_error_milli", "sealed", "missing", "status"},
            {"status": ["observed", "inferred", "unknown"]},
        ),
        (
            "line_flow_row",
            {"line_flow_id", "expected_line_canonical_sha256", "coverage_milli", "continuity_milli", "symmetric_chamfer_milli", "max_deviation_milli", "direction_order_milli", "duplicate_crossing_count", "status"},
            {"status": ["observed", "inferred", "unknown"]},
        ),
    ]
    for definition, fields, enum_fields in row_specs:
        row = view.get("$defs", {}).get(definition, {})
        row_properties = row.get("properties", {})
        require(
            row.get("type") == "object"
            and row.get("additionalProperties") is False
            and set(row.get("required", [])) == fields
            and set(row_properties) == fields,
            f"{definition} must be closed with its exact field set",
        )
        for field, enum in enum_fields.items():
            require(row_properties[field].get("enum") == enum, f"{definition} {field} status drifted")
    require(
        view.get("allOf")
        and any(
            guard.get("if", {}).get("properties", {}).get("negative_space_status", {}).get("const")
            == "not-applicable"
            and guard.get("then", {}).get("properties", {}).get("negative_space_rows", {}).get("maxItems") == 0
            and guard.get("then", {}).get("properties", {}).get("visual_structure_review_status", {}).get("const")
            == "user_confirmed"
            for guard in view["allOf"]
        )
        and any(
            guard.get("if", {}).get("properties", {}).get("line_flow_status", {}).get("const")
            == "not-applicable"
            and guard.get("then", {}).get("properties", {}).get("line_flow_rows", {}).get("maxItems") == 0
            and guard.get("then", {}).get("properties", {}).get("visual_structure_review_status", {}).get("const")
            == "user_confirmed"
            for guard in view["allOf"]
        ),
        "not-applicable negative-space/line-flow requires user-confirmed structure and zero rows",
    )
    thresholds = {
        "negative_space": {"iou_milli_min": 850, "boundary_f1_milli_min": 800, "area_ratio_milli_min": 850, "area_ratio_milli_max": 1150, "centroid_error_milli_max": 3000},
        "line_flow": {"coverage_milli_min": 900, "continuity_milli_min": 900, "symmetric_chamfer_milli_max": 3000, "max_deviation_milli_max": 5000, "direction_order_milli_min": 950, "duplicate_crossing_count_max": 0},
    }
    require(
        parent.get("x-forgecad-thresholds") == thresholds,
        "ProductionWeaponFormArtEvidence thresholds must be frozen in the contract",
    )

    require(
        prepare_properties["art_evidence_policy"].get("const")
        == "production-weapon-form-art-evidence-six-view-typed-observation@1"
        and prepare_properties["form_evidence_object_sha256"].get("$ref") == "#/$defs/sha256"
        and prepare_properties["form_evidence_canonical_sha256"].get("$ref") == "#/$defs/sha256"
        and prepare_properties["art_evidence_policy_sha256"].get("$ref") == "#/$defs/sha256"
        and prepare_properties["input_sha256"].get("$ref") == "#/$defs/sha256",
        "ProductionWeaponFormArtEvidencePrepareRequest@1 must bind only existing FormEvidence and policy hashes",
    )
    require(
        prepare_result_properties["art_evidence"].get("$ref")
        == "https://forgecad.local/contracts/production-weapon-form-art-evidence.schema.json"
        and prepare_result_properties["runtime_write"].get("const") is True
        and all(prepare_result_properties[field].get("const") is False for field in (
            "production_stage_advanced", "candidate_confirmed", "version_created", "export_performed"
        )),
        "ProductionWeaponFormArtEvidencePrepareResult@1 must be a non-promoting write result",
    )
    require(
        get_result_properties["art_evidence"].get("$ref")
        == "https://forgecad.local/contracts/production-weapon-form-art-evidence.schema.json"
        and get_result_properties["runtime_write"].get("const") is False
        and get_result_properties["restart_hash_verified"].get("const") is True
        and all(get_result_properties[field].get("const") is False for field in (
            "production_stage_advanced", "candidate_confirmed", "version_created", "export_performed"
        )),
        "ProductionWeaponFormArtEvidenceGetResult@1 must be read-only and restart verified",
    )

    forbidden_property_names = {
        "path", "file_path", "absolute_path", "url", "uri", "script", "script_path",
        "python", "javascript", "raw", "raw_bytes", "bytes", "environment", "env",
        "secret", "network", "geometry_program", "operator_id",
    }

    def property_names(node: object) -> set[str]:
        names: set[str] = set()
        if isinstance(node, dict):
            properties = node.get("properties")
            if isinstance(properties, dict):
                names.update(properties)
            for value in node.values():
                names.update(property_names(value))
        elif isinstance(node, list):
            for value in node:
                names.update(property_names(value))
        return names

    for schema, filename in [
        (parent, "production-weapon-form-art-evidence.schema.json"),
        (view, "production-weapon-form-art-evidence-view.schema.json"),
        (prepare, "production-weapon-form-art-evidence-prepare-request.schema.json"),
        (prepare_result, "production-weapon-form-art-evidence-prepare-result.schema.json"),
        (get_request, "production-weapon-form-art-evidence-get-request.schema.json"),
        (get_result, "production-weapon-form-art-evidence-get-result.schema.json"),
    ]:
        require(
            forbidden_property_names.isdisjoint(property_names(schema)),
            f"{filename} must reject geometry, paths, URLs, scripts, raw bytes and environment inputs",
        )


def check_production_weapon_owner_reviewed_void_calibration_contracts() -> None:
    """Keep owner-to-reviewed-void calibration Runtime-derived and read-only."""
    projection = load_schema(
        "production-weapon-owner-reviewed-void-calibration-projection.schema.json"
    )
    request = load_schema(
        "production-weapon-owner-reviewed-void-calibration-projection-get-request.schema.json"
    )
    result = load_schema(
        "production-weapon-owner-reviewed-void-calibration-projection-get-result.schema.json"
    )

    projection_fields = {
        "schema_version", "projection_id", "operation", "session_id", "project_id", "candidate_id",
        "candidate_state_sha256", "artifact_id", "artifact_sha256", "artifact_readback_sha256",
        "form_art_evidence_id", "form_art_evidence_object_sha256", "form_art_evidence_canonical_sha256",
        "fresh_baseline_id", "fresh_baseline_canonical_sha256", "fresh_baseline_receipt_object_sha256",
        "registration_lineage_id", "registration_lineage_canonical_sha256",
        "registration_lineage_receipt_object_sha256", "registered_rig_v2_id",
        "registered_rig_v2_object_sha256", "registered_rig_v2_canonical_sha256",
        "runtime_build_cohort_sha256", "owner_part_id", "view_kinds", "views", "calibration_policy",
        "calibration_policy_sha256", "transform_policy", "reviewed_void_policy", "depth_policy",
        "depth_policy_sha256", "threshold_policy", "threshold_policy_sha256", "calibration_status",
        "blocker_codes", "strict_owner_void_all_views_passed", "strict_depth_all_views_passed",
        "identity_transform_all_views_unique", "all_views_passed", "eligible", "promotable",
        "quality_status", "depth_status", "runtime_write_performed", "persistent_user_data_touched",
        "worker_started", "production_stage_advanced", "candidate_confirmed", "version_created",
        "export_performed", "request_sha256", "input_sha256", "canonicalization_policy",
        "canonical_sha256", "created_at",
    }
    view_fields = {
        "schema_version", "project_id", "candidate_id", "candidate_state_sha256", "artifact_id",
        "artifact_sha256", "artifact_readback_sha256", "view_kind", "view_id", "reviewed_structure_id",
        "reference_id", "reference_sha256", "camera_hash", "camera_canonical_sha256",
        "camera_object_sha256", "render_set_object_sha256", "render_set_canonical_sha256",
        "render_set_view_id", "form_art_view_receipt_object_sha256",
        "form_art_view_receipt_canonical_sha256", "baseline_view_receipt_object_sha256",
        "target_object_sha256", "target_canonical_sha256", "visual_structure_canonical_sha256",
        "silhouette_pass_object_sha256", "part_id_pass_object_sha256", "depth_pass_object_sha256",
        "owner_part_id", "derived_owner_region_sha256", "derived_reviewed_void_region_sha256",
        "derived_void_boundary_sha256", "registered_camera_lineage_verified", "derived_transform_kind",
        "identity_transform_unique", "eligible_transform_count", "transform_rank_tie",
        "expected_void_pixel_count", "owner_region_pixel_count", "owner_expected_void_overlap_pixel_count",
        "owner_expected_void_overlap_milli", "boundary_pixel_count", "owner_boundary_adjacency_pixel_count",
        "owner_boundary_adjacency_milli", "depth_valid_pixel_count", "depth_owner_sample_count",
        "depth_boundary_sample_count", "depth_invalid_sample_count", "depth_ordering_milli",
        "depth_status", "owner_void_status", "strict_owner_void_passed", "strict_depth_passed",
        "view_status", "view_passed", "blocker_codes", "quality_status", "canonical_sha256", "created_at",
    }
    request_fields = {
        "schema_version", "operation", "projection_id", "session_id", "project_id", "candidate_id",
        "candidate_state_sha256", "artifact_id", "artifact_sha256", "artifact_readback_sha256",
        "form_art_evidence_id", "form_art_evidence_object_sha256", "form_art_evidence_canonical_sha256",
        "fresh_baseline_id", "fresh_baseline_canonical_sha256", "fresh_baseline_receipt_object_sha256",
        "registration_lineage_id", "registration_lineage_canonical_sha256",
        "registration_lineage_receipt_object_sha256", "registered_rig_v2_id",
        "registered_rig_v2_object_sha256", "registered_rig_v2_canonical_sha256", "max_response_bytes",
        "writer_policy", "runtime_write_performed", "persistent_user_data_touched", "input_sha256",
    }
    result_fields = {
        "schema_version", "operation", "projection_id", "projection", "request_sha256",
        "request_input_sha256", "replayed", "restart_hash_verified", "writer_policy", "runtime_write",
        "persistent_user_data_touched", "worker_started", "production_stage_advanced", "candidate_confirmed",
        "version_created", "export_performed", "quality_status", "depth_status",
        "canonicalization_policy", "canonical_sha256",
    }

    def closed(schema: dict, fields: set[str], label: str) -> dict:
        properties = schema.get("properties", {})
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and set(schema.get("required", [])) == fields
            and set(properties) == fields,
            f"{label} must be closed with its exact field set",
        )
        return properties

    projection_properties = closed(
        projection, projection_fields, "ProductionWeaponOwnerReviewedVoidCalibrationProjection@1"
    )
    request_properties = closed(
        request, request_fields, "ProductionWeaponOwnerReviewedVoidCalibrationProjectionGetRequest@1"
    )
    result_properties = closed(
        result, result_fields, "ProductionWeaponOwnerReviewedVoidCalibrationProjectionGetResult@1"
    )
    require(
        projection.get("$id")
        == "https://forgecad.local/contracts/production-weapon-owner-reviewed-void-calibration-projection.schema.json"
        and projection.get("title")
        == "ProductionWeaponOwnerReviewedVoidCalibrationProjection@1"
        and projection_properties["schema_version"].get("const")
        == "ProductionWeaponOwnerReviewedVoidCalibrationProjection@1"
        and request_properties["schema_version"].get("const")
        == "ProductionWeaponOwnerReviewedVoidCalibrationProjectionGetRequest@1"
        and result_properties["schema_version"].get("const")
        == "ProductionWeaponOwnerReviewedVoidCalibrationProjectionGetResult@1"
        and projection_properties["operation"].get("const")
        == "forgecad.production.weapon.owner-reviewed-void-calibration-projection-get@1"
        and request_properties["operation"].get("const") == projection_properties["operation"].get("const")
        and result_properties["operation"].get("const") == projection_properties["operation"].get("const"),
        "owner-reviewed-void calibration contract identity drifted",
    )
    view_array = projection_properties["views"]
    require(
        projection_properties["view_kinds"].get("const") == ["left", "right", "rear-three-quarter"]
        and view_array.get("minItems") == 3
        and view_array.get("maxItems") == 3
        and view_array.get("uniqueItems") is True
        and view_array.get("items") is False
        and view_array.get("prefixItems")
        == [{"$ref": "#/$defs/view_left"}, {"$ref": "#/$defs/view_right"}, {"$ref": "#/$defs/view_rear_three_quarter"}],
        "owner-reviewed-void calibration must freeze the ordered three-view set",
    )
    view = projection["$defs"]["view"]
    require(
        view.get("type") == "object"
        and view.get("additionalProperties") is False
        and set(view.get("required", [])) == view_fields
        and set(view.get("properties", {})) == view_fields
        and projection_properties["owner_part_id"].get("const") == "rear-stock"
        and view["properties"]["owner_part_id"].get("const") == "rear-stock"
        and projection_properties["calibration_policy"].get("const")
        == "runtime-derived-registered-camera-owner-to-reviewed-void-calibration@1"
        and projection_properties["transform_policy"].get("const")
        == "runtime-derived-closed-part-id-review-region-transform@1"
        and projection_properties["reviewed_void_policy"].get("const")
        == "reviewed-subtract-contour-intersection-with-candidate-silhouette@1"
        and projection_properties["depth_policy"].get("const")
        == "registered-camera-owner-void-depth-evidence@1"
        and projection_properties["threshold_policy"].get("const")
        == "owner-reviewed-void-zero-intrusion-adjacency-thresholds@1",
        "owner-reviewed-void calibration view or policy identity drifted",
    )
    for properties, label in [(projection_properties, "owner-reviewed-void projection"), (request_properties, "owner-reviewed-void request")]:
        for field in (
            "candidate_state_sha256", "artifact_sha256", "artifact_readback_sha256",
            "form_art_evidence_object_sha256", "form_art_evidence_canonical_sha256",
            "fresh_baseline_canonical_sha256", "fresh_baseline_receipt_object_sha256",
            "registration_lineage_canonical_sha256", "registration_lineage_receipt_object_sha256",
            "registered_rig_v2_object_sha256", "registered_rig_v2_canonical_sha256",
        ):
            require(properties[field].get("$ref") == "#/$defs/sha256", f"{label} {field} must be SHA-256 bound")
    for field in {
        "candidate_state_sha256", "artifact_sha256", "artifact_readback_sha256", "reference_sha256",
        "camera_hash", "camera_canonical_sha256", "camera_object_sha256", "render_set_object_sha256",
        "render_set_canonical_sha256", "form_art_view_receipt_object_sha256",
        "form_art_view_receipt_canonical_sha256", "baseline_view_receipt_object_sha256",
        "target_object_sha256", "target_canonical_sha256", "visual_structure_canonical_sha256",
        "silhouette_pass_object_sha256", "part_id_pass_object_sha256", "depth_pass_object_sha256",
        "derived_owner_region_sha256", "derived_reviewed_void_region_sha256", "derived_void_boundary_sha256",
        "canonical_sha256",
    }:
        require(view["properties"][field].get("$ref") == "#/$defs/sha256", f"owner-reviewed-void view {field} must be SHA-256 bound")
    require(
        projection_properties["promotable"].get("const") is False
        and projection_properties["quality_status"].get("const") == "NOT_PROVEN"
        and all(projection_properties[field].get("const") is False for field in (
            "runtime_write_performed", "persistent_user_data_touched", "worker_started",
            "production_stage_advanced", "candidate_confirmed", "version_created", "export_performed",
        ))
        and request_properties["max_response_bytes"].get("const") == 1_048_576
        and request_properties["writer_policy"].get("const") == "forgecad-runtime-only-state-writer@1"
        and request_properties["runtime_write_performed"].get("const") is False
        and request_properties["persistent_user_data_touched"].get("const") is False
        and result_properties["projection"].get("$ref")
        == "https://forgecad.local/contracts/production-weapon-owner-reviewed-void-calibration-projection.schema.json"
        and result_properties["restart_hash_verified"].get("const") is False
        and result_properties["runtime_write"].get("const") is False
        and result_properties["persistent_user_data_touched"].get("const") is False
        and result_properties["worker_started"].get("const") is False
        and result_properties["production_stage_advanced"].get("const") is False
        and result_properties["candidate_confirmed"].get("const") is False
        and result_properties["version_created"].get("const") is False
        and result_properties["export_performed"].get("const") is False
        and result_properties["quality_status"].get("const") == "NOT_PROVEN",
        "owner-reviewed-void request/result must preserve the read-only boundary",
    )
    forbidden = {
        "path", "file_path", "absolute_path", "url", "uri", "script", "script_path", "python",
        "javascript", "raw", "raw_bytes", "bytes", "environment", "env", "secret", "network",
        "mask", "masks", "vertex", "vertices", "vertex_id", "triangle", "triangle_id", "transform",
        "camera_matrix", "pixel", "image",
    }
    require(
        not forbidden.intersection(request_properties),
        "owner-reviewed-void request must not accept raw masks, vertices, pixels or transforms",
    )


def check_production_weapon_form_art_mesh_proposal_edit_contracts() -> None:
    """Keep the FormArt mesh proposal edit union closed and Runtime-owned."""
    filename = "production-weapon-form-art-mesh-proposal-edit.schema.json"
    schema = load_schema(filename)
    require(
        schema.get("$id") == f"https://forgecad.local/contracts/{filename}"
        and schema.get("title") == "ProductionWeaponFormArtMeshProposalEdit@5"
        and schema.get("oneOf") == [
            {"$ref": "#/$defs/move_vertices"},
            {"$ref": "#/$defs/open_frame_notch"},
            {"$ref": "#/$defs/rear_stock_void_rail_bow"},
            {"$ref": "#/$defs/rear_stock_void_boundary_bridge"},
        ],
        "ProductionWeaponFormArtMeshProposalEdit@5 must expose exactly the four typed edit variants",
    )
    expected_fields = {
        "schema_version",
        "operation",
        "source_node_id",
        "part_id",
        "coordinate_space",
        "selection_policy",
        "canonical_sha256",
    }
    move = schema["$defs"]["move_vertices"]
    notch = schema["$defs"]["open_frame_notch"]
    rail_bow = schema["$defs"]["rear_stock_void_rail_bow"]
    boundary_bridge = schema["$defs"]["rear_stock_void_boundary_bridge"]
    require(
        move.get("additionalProperties") is False
        and set(move.get("required", []))
        == expected_fields | {"vertex_moves"}
        and set(move.get("properties", {}))
        == expected_fields | {"vertex_moves"}
        and move["properties"]["schema_version"].get("const")
        == "AuthoringMeshMoveVertices@1"
        and move["properties"]["operation"].get("const") == "move_vertices"
        and move["properties"]["selection_policy"].get("const")
        == "explicit-stable-vertex-ids@1"
        and move["properties"]["vertex_moves"].get("minItems") == 1
        and move["properties"]["vertex_moves"].get("maxItems") == 32,
        "MoveVertices proposal edit contract drifted",
    )
    require(
        notch.get("additionalProperties") is False
        and set(notch.get("required", []))
        == expected_fields | {"opening_width_milli", "opening_height_milli"}
        and set(notch.get("properties", {}))
        == expected_fields | {"opening_width_milli", "opening_height_milli"}
        and notch["properties"]["schema_version"].get("const")
        == "AuthoringMeshOpenFrameNotch@1"
        and notch["properties"]["operation"].get("const") == "open_frame_notch"
        and notch["properties"]["coordinate_space"].get("const") == "source-local"
        and notch["properties"]["selection_policy"].get("const")
        == "runtime-derived-box-open-frame@1"
        and notch["properties"]["opening_width_milli"].get("minimum") == 1
        and notch["properties"]["opening_width_milli"].get("maximum") == 999
        and notch["properties"]["opening_height_milli"].get("minimum") == 1
        and notch["properties"]["opening_height_milli"].get("maximum") == 999,
        "OpenFrameNotch proposal edit contract drifted",
    )
    require(
        rail_bow.get("additionalProperties") is False
        and set(rail_bow.get("required", [])) == expected_fields
        and set(rail_bow.get("properties", {})) == expected_fields
        and rail_bow["properties"]["schema_version"].get("const")
        == "AuthoringMeshRearStockVoidRailBow@1"
        and rail_bow["properties"]["operation"].get("const") == "rear_stock_void_rail_bow"
        and rail_bow["properties"]["coordinate_space"].get("const") == "source-local"
        and rail_bow["properties"]["selection_policy"].get("const")
        == "runtime-derived-rear-stock-void-rail-bow@1"
        and not any(
            forbidden in field.lower()
            for field in rail_bow["properties"]
            for forbidden in ("centroid", "normal", "vertex", "point", "mesh", "path", "script")
        ),
        "RearStockVoidRailBow proposal edit contract must remain closed and semantic-only",
    )
    require(
        boundary_bridge.get("additionalProperties") is False
        and set(boundary_bridge.get("required", [])) == expected_fields | {"profile_id"}
        and set(boundary_bridge.get("properties", {})) == expected_fields | {"profile_id"}
        and boundary_bridge["properties"]["schema_version"].get("const")
        == "AuthoringMeshRearStockVoidBoundaryBridge@1"
        and boundary_bridge["properties"]["operation"].get("const")
        == "rear_stock_void_boundary_bridge"
        and boundary_bridge["properties"]["source_node_id"].get("const") == "rear-stock"
        and boundary_bridge["properties"]["part_id"].get("const") == "rear-stock"
        and boundary_bridge["properties"]["coordinate_space"].get("const") == "source-local"
        and boundary_bridge["properties"]["selection_policy"].get("const")
        == "runtime-derived-rear-stock-void-boundary-bridge@1"
        and boundary_bridge["properties"]["profile_id"].get("const")
        == "registered-void-boundary-depth-wedge-5@1"
        and not any(
            forbidden in field.lower()
            for field in boundary_bridge["properties"]
            for forbidden in ("centroid", "normal", "vertex", "point", "mesh", "path", "script", "camera", "mask", "transform", "scalar")
        ),
        "RearStockVoidBoundaryBridge proposal edit contract must remain closed and semantic-only",
    )
    for variant in (move, notch, rail_bow):
        for field in ("source_node_id", "part_id"):
            require(
                variant["properties"][field].get("$ref") == "#/$defs/identifier",
                f"{filename} {field} must remain an opaque Runtime-bound identifier",
            )
        require(
            variant["properties"]["canonical_sha256"].get("$ref") == "#/$defs/sha256",
            f"{filename} canonical_sha256 must remain hash-bound",
        )
    require(
        boundary_bridge["properties"]["canonical_sha256"].get("$ref") == "#/$defs/sha256",
        f"{filename} canonical_sha256 must remain hash-bound",
    )
    move_vertex = schema["$defs"]["move_vertex"]
    require(
        move_vertex.get("additionalProperties") is False
        and set(move_vertex.get("required", []))
        == {"vertex_id", "before_position_m", "after_position_m"}
        and set(move_vertex.get("properties", {}))
        == {"vertex_id", "before_position_m", "after_position_m"}
        and move["properties"]["vertex_moves"].get("items")
        == {"$ref": "#/$defs/move_vertex"},
        "MoveVertices proposal vertex payload must remain closed and bounded",
    )


def check_production_weapon_form_art_proposal_evidence_contracts() -> None:
    """Keep proposal-side FormArt evidence closed, hash-bound and non-promoting."""
    filename = "production-weapon-form-art-proposal-evidence.schema.json"
    schema = load_schema(filename)
    view_kinds = ["front", "back", "left", "right", "top", "rear-three-quarter"]
    owner_view_kinds = ["left", "right", "rear-three-quarter"]
    root_fields = {
        "schema_version", "policy", "policy_sha256", "policy_definition", "project_id", "session_id",
        "source_candidate_id", "source_candidate_state_sha256", "source_artifact_sha256",
        "source_artifact_readback_sha256", "source_form_art_evidence_id",
        "source_form_art_evidence_object_sha256", "source_form_art_evidence_canonical_sha256",
        "source_form_art_role", "source_camera_evidence_kind", "source_fresh_baseline_id",
        "source_fresh_baseline_canonical_sha256", "source_fresh_baseline_receipt_object_sha256",
        "source_registration_lineage_id", "source_registration_lineage_canonical_sha256",
        "source_registered_rig_v2_id",
        "proposal_candidate_id", "proposal_candidate_state_sha256", "proposal_artifact_sha256",
        "proposal_artifact_readback_sha256", "worker_build_cohort_sha256",
        "cross_view_evidence_bundle_sha256", "proposal_part_id_vocabulary_sha256", "owner_part_id",
        "reference_canvas_object_sha256", "reference_canvas_canonical_sha256", "design_spec_object_sha256",
        "design_spec_canonical_sha256", "camera_lock_id", "camera_lock_canonical_sha256",
        "camera_rig_object_sha256", "camera_rig_canonical_sha256", "camera_lock_receipt_object_sha256",
        "camera_lock_source_transition_id", "camera_lock_source_transition_sha256",
        "camera_lock_source_head_canonical_sha256", "views", "part_id_all_views_observed",
        "negative_space_all_views_resolved", "line_flow_all_views_resolved",
        "strict_owner_void_all_views_passed", "proposal_form_art_evidence_ready", "status",
        "candidate_confirm_allowed", "secondary_form_approved", "production_stage_advanced",
        "quality_status", "canonical_sha256",
    }
    view_fields = {
        "view_kind", "view_id", "reference_id", "reference_sha256", "view_spec_canonical_sha256", "crop",
        "rotation_degrees", "camera_hash", "camera_canonical_sha256", "camera_source",
        "camera_object_sha256",
        "render_set_object_sha256", "render_set_canonical_sha256", "silhouette_pass_object_sha256",
        "part_id_pass_object_sha256", "depth_pass_object_sha256", "normal_pass_object_sha256",
        "target_object_sha256", "target_canonical_sha256", "visual_structure_canonical_sha256",
        "visual_structure_review_status", "source_form_evidence_view_receipt_object_sha256",
        "source_form_evidence_view_receipt_canonical_sha256",
        "source_fresh_baseline_view_receipt_object_sha256", "proposal_candidate_id",
        "proposal_candidate_state_sha256", "proposal_artifact_sha256", "owner_evidence", "part_id_status",
        "part_id_expected_count", "part_id_observed_count", "part_id_missing_count",
        "part_id_unexpected_count", "part_id_coverage_milli", "expected_visible_part_ids",
        "observed_part_ids", "missing_part_ids", "unexpected_part_ids", "negative_space_status",
        "negative_space_rows", "line_flow_status", "line_flow_rows", "view_observation_status",
        "quality_status",
    }
    policy_fields = {
        "policy", "owner_part_id", "required_view_kinds", "owner_view_kinds",
        "registered_camera_transform", "fixed_raster_size_px", "min_expected_void_pixels",
        "min_expected_boundary_pixels", "min_owner_region_pixels", "min_boundary_adjacency_pixels",
        "min_boundary_adjacency_milli", "max_owner_expected_void_overlap_pixels",
        "max_owner_expected_void_overlap_milli", "negative_space_thresholds",
        "line_flow_thresholds",
    }
    owner_pass_fields = {
        "structure_id", "owner_part_id", "policy", "expected_region_canonical_sha256",
        "expected_void_pixel_count", "expected_boundary_pixel_count", "owner_region_pixel_count",
        "owner_boundary_adjacency_pixel_count", "owner_boundary_adjacency_milli",
        "owner_expected_void_overlap_pixel_count", "owner_expected_void_overlap_milli",
        "registered_camera_lineage_verified", "strict_owner_void_passed", "status", "quality_status",
        "depth_status",
    }
    owner_blocked_fields = {
        "structure_id", "owner_part_id", "policy", "expected_region_canonical_sha256",
        "expected_void_pixel_count", "expected_boundary_pixel_count", "owner_region_pixel_count",
        "expected_void_bbox_px", "owner_bbox_px", "owner_minus_expected_void_bbox_edge_delta_px",
        "owner_minus_expected_void_centroid_delta_milli_px",
        "owner_boundary_adjacency_pixel_count", "owner_boundary_adjacency_milli",
        "owner_expected_void_overlap_pixel_count", "owner_expected_void_overlap_milli",
        "identity_passes_thresholds", "ranked_transform", "ranked_transform_unique",
        "eligible_transform_count", "registered_camera_lineage_verified", "strict_owner_void_passed",
        "blocker_code", "diagnostic_error", "status", "quality_status", "depth_status",
    }
    negative_fields = {
        "structure_id", "expected_region_canonical_sha256", "iou_milli", "boundary_f1_milli",
        "area_ratio_milli", "centroid_error_milli", "sealed", "missing", "status",
    }
    line_fields = {
        "line_flow_id", "expected_line_canonical_sha256", "coverage_milli", "continuity_milli",
        "symmetric_chamfer_milli", "max_deviation_milli", "direction_order_milli",
        "duplicate_crossing_count", "status",
    }

    def closed(node: dict, fields: set[str], label: str) -> dict:
        properties = node.get("properties", {})
        require(
            node.get("type") == "object"
            and node.get("additionalProperties") is False
            and set(node.get("required", [])) == fields
            and set(properties) == fields,
            f"{label} must be closed with its exact field set",
        )
        return properties

    properties = closed(schema, root_fields, "ProductionWeaponFormArtProposalEvidence@1")
    require(
        schema.get("$id") == f"https://forgecad.local/contracts/{filename}"
        and schema.get("title") == "ProductionWeaponFormArtProposalEvidence@1"
        and properties["schema_version"].get("const") == "ProductionWeaponFormArtProposalEvidence@1"
        and properties["policy"].get("const")
        == "proposal-candidate-six-view-form-art-part-owner-negative-line@1"
        and properties["owner_part_id"].get("const") == "rear-stock"
        and properties["quality_status"].get("const") == "QUALITY_TARGET_NOT_MET"
        and properties["candidate_confirm_allowed"].get("const") is False
        and properties["secondary_form_approved"].get("const") == "NOT_CREATED"
        and properties["production_stage_advanced"].get("const") is False,
        "ProductionWeaponFormArtProposalEvidence@1 identity and non-promoting fields drifted",
    )
    for field in {
        "policy_sha256", "source_candidate_state_sha256", "source_artifact_sha256",
        "source_artifact_readback_sha256", "source_form_art_evidence_object_sha256",
        "source_form_art_evidence_canonical_sha256", "proposal_candidate_state_sha256",
        "proposal_artifact_sha256", "proposal_artifact_readback_sha256", "worker_build_cohort_sha256",
        "cross_view_evidence_bundle_sha256", "proposal_part_id_vocabulary_sha256",
        "reference_canvas_object_sha256", "reference_canvas_canonical_sha256", "design_spec_object_sha256",
        "design_spec_canonical_sha256", "camera_lock_canonical_sha256", "camera_rig_object_sha256",
        "camera_rig_canonical_sha256", "camera_lock_receipt_object_sha256",
        "camera_lock_source_transition_sha256", "camera_lock_source_head_canonical_sha256", "canonical_sha256",
    }:
        require(properties[field].get("$ref") == "#/$defs/sha256", f"{filename} {field} must be SHA-256 bound")

    view_array = properties["views"]
    require(
        view_array.get("minItems") == 6
        and view_array.get("maxItems") == 6
        and view_array.get("uniqueItems") is True
        and view_array.get("items") is False
        and len(view_array.get("prefixItems", [])) == 6
        and all(item.get("$ref") == "#/$defs/view" for item in view_array["prefixItems"]),
        "ProductionWeaponFormArtProposalEvidence@1 must persist exactly six closed view rows",
    )
    require(
        {
            guard.get("contains", {}).get("properties", {}).get("view_kind", {}).get("const")
            for guard in view_array.get("allOf", [])
        }
        == set(view_kinds),
        "ProductionWeaponFormArtProposalEvidence@1 must cover every fixed view kind",
    )

    policy = closed(schema["$defs"]["policy_definition"], policy_fields, "proposal policy definition")
    require(
        policy["policy"].get("const") == properties["policy"].get("const")
        and policy["owner_part_id"].get("const") == "rear-stock"
        and policy["required_view_kinds"].get("const") == view_kinds
        and policy["owner_view_kinds"].get("const") == owner_view_kinds
        and policy["registered_camera_transform"].get("const") == "identity"
        and policy["fixed_raster_size_px"].get("const") == 512
        and policy["min_expected_void_pixels"].get("const") == 256
        and policy["min_expected_boundary_pixels"].get("const") == 64
        and policy["min_owner_region_pixels"].get("const") == 128
        and policy["min_boundary_adjacency_pixels"].get("const") == 32
        and policy["min_boundary_adjacency_milli"].get("const") == 250
        and policy["max_owner_expected_void_overlap_pixels"].get("const") == 0
        and policy["max_owner_expected_void_overlap_milli"].get("const") == 0
        and policy["negative_space_thresholds"].get("const") == {
            "iou_milli_min": 850,
            "boundary_f1_milli_min": 800,
            "area_ratio_milli_min": 850,
            "area_ratio_milli_max": 1150,
            "centroid_error_milli_max": 3000,
        }
        and policy["line_flow_thresholds"].get("const") == {
            "coverage_milli_min": 900,
            "continuity_milli_min": 900,
            "symmetric_chamfer_milli_max": 3000,
            "max_deviation_milli_max": 5000,
            "direction_order_milli_min": 950,
            "duplicate_crossing_count_max": 0,
        },
        "proposal owner policy thresholds or view coverage drifted",
    )

    view = closed(schema["$defs"]["view"], view_fields, "proposal FormArt view")
    require(
        view["view_kind"].get("$ref") == "#/$defs/view_kind"
        and view["visual_structure_review_status"].get("const") == "user_confirmed"
        and view["quality_status"].get("const") == "NOT_PROVEN",
        "proposal view observation status drifted",
    )
    for field in {
        "reference_sha256", "view_spec_canonical_sha256", "camera_hash", "camera_canonical_sha256",
        "camera_object_sha256", "render_set_object_sha256", "render_set_canonical_sha256",
        "silhouette_pass_object_sha256", "part_id_pass_object_sha256", "depth_pass_object_sha256",
        "normal_pass_object_sha256", "target_object_sha256", "target_canonical_sha256",
        "visual_structure_canonical_sha256", "source_form_evidence_view_receipt_object_sha256",
        "source_form_evidence_view_receipt_canonical_sha256", "proposal_candidate_state_sha256",
        "proposal_artifact_sha256",
    }:
        require(view[field].get("$ref") == "#/$defs/sha256", f"proposal view {field} must be SHA-256 bound")
    require(
        {
            branch.get("$ref")
            for branch in view["owner_evidence"]["oneOf"]
            if isinstance(branch, dict) and branch.get("$ref")
        }
        == {"#/$defs/owner_pass", "#/$defs/owner_blocked"}
        and any(branch.get("type") == "null" for branch in view["owner_evidence"]["oneOf"]),
        "proposal view owner evidence must allow null/pass/blocked branches only",
    )
    for definition, fields in [
        ("negative_space_row", negative_fields),
        ("line_flow_row", line_fields),
        ("owner_pass", owner_pass_fields),
        ("owner_blocked", owner_blocked_fields),
    ]:
        closed(schema["$defs"][definition], fields, f"proposal {definition}")
    owner_pass = schema["$defs"]["owner_pass"]["properties"]
    owner_blocked = schema["$defs"]["owner_blocked"]["properties"]
    require(
        owner_pass["policy"].get("const") == "registered-camera-direct-part-id-owner-void-zero-intrusion@1"
        and owner_pass["expected_void_pixel_count"].get("minimum") == 256
        and owner_pass["expected_boundary_pixel_count"].get("minimum") == 64
        and owner_pass["owner_region_pixel_count"].get("minimum") == 128
        and owner_pass["owner_boundary_adjacency_pixel_count"].get("minimum") == 32
        and owner_pass["owner_boundary_adjacency_milli"].get("minimum") == 250
        and owner_pass["owner_expected_void_overlap_pixel_count"].get("const") == 0
        and owner_pass["owner_expected_void_overlap_milli"].get("const") == 0
        and owner_pass["status"].get("const") == "READY_PROPOSAL_OWNER_VOID_BINDING"
        and owner_blocked["policy"].get("const") == "registered-camera-direct-part-id-owner-void-zero-intrusion@1"
        and owner_blocked["expected_void_pixel_count"].get("minimum") == 1
        and owner_blocked["expected_boundary_pixel_count"].get("minimum") == 1
        and owner_blocked["expected_void_bbox_px"].get("minItems") == 4
        and owner_blocked["owner_bbox_px"].get("maxItems") == 4
        and owner_blocked["owner_minus_expected_void_centroid_delta_milli_px"].get("minItems") == 2
        and owner_blocked["owner_region_pixel_count"].get("minimum") == 0
        and owner_blocked["owner_expected_void_overlap_pixel_count"].get("minimum") == 0
        and owner_blocked["eligible_transform_count"].get("maximum") == 4
        and owner_blocked["strict_owner_void_passed"].get("const") is False
        and owner_blocked["blocker_code"].get("const") == "STRICT_REGISTERED_CAMERA_OWNER_VOID_NOT_ELIGIBLE"
        and owner_blocked["status"].get("const") == "BLOCKED_PROPOSAL_OWNER_VOID_BINDING",
        "proposal owner pass/blocker thresholds or statuses drifted",
    )
    negative = schema["$defs"]["negative_space_row"]["properties"]
    line = schema["$defs"]["line_flow_row"]["properties"]
    require(
        negative["iou_milli"].get("maximum") == 1000
        and negative["boundary_f1_milli"].get("maximum") == 1000
        and negative["area_ratio_milli"].get("maximum") == 10000
        and negative["centroid_error_milli"].get("maximum") == 100000
        and line["coverage_milli"].get("maximum") == 1000
        and line["continuity_milli"].get("maximum") == 1000
        and line["symmetric_chamfer_milli"].get("maximum") == 100000
        and line["max_deviation_milli"].get("maximum") == 100000
        and line["direction_order_milli"].get("maximum") == 1000
        and line["duplicate_crossing_count"].get("maximum") == 512,
        "proposal negative-space/line-flow metrics must remain bounded",
    )
    require(
        schema.get("x-forgecad-thresholds", {}).get("owner_void", {}).get("fixed_raster_size_px") == 512
        and schema["x-forgecad-thresholds"]["owner_void"]["min_boundary_adjacency_milli"] == 250
        and schema["x-forgecad-thresholds"]["negative_space"]["iou_milli_min"] == 850
        and schema["x-forgecad-thresholds"]["line_flow"]["duplicate_crossing_count_max"] == 0,
        "proposal FormArt thresholds are not frozen",
    )
    forbidden_property_names = {
        "path", "file_path", "absolute_path", "url", "uri", "script", "script_path", "python",
        "javascript", "raw", "raw_bytes", "bytes", "environment", "env", "secret", "network",
        "geometry_program", "operator_id",
    }

    def property_names(node: object) -> set[str]:
        names: set[str] = set()
        if isinstance(node, dict):
            properties = node.get("properties")
            if isinstance(properties, dict):
                names.update(properties)
            for value in node.values():
                names.update(property_names(value))
        elif isinstance(node, list):
            for value in node:
                names.update(property_names(value))
        return names

    require(
        forbidden_property_names.isdisjoint(property_names(schema)),
        f"{filename} must reject paths, URLs, scripts, raw bytes and environment inputs",
    )


def check_production_weapon_art_decision_contracts() -> None:
    """Freeze the first read-only assembly/art decision projection."""
    registry = load_schema("production-weapon-assembly-decision-registry.schema.json")
    request = load_schema("production-weapon-art-decision-proposal-get-request.schema.json")
    result = load_schema("production-weapon-art-decision-proposal-get-result.schema.json")

    def require_closed(schema: dict, fields: set[str], label: str) -> dict:
        properties = schema.get("properties", {})
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and set(schema.get("required", [])) == fields
            and set(properties) == fields,
            f"{label} must be closed with its exact field set",
        )
        return properties

    group_ids = [
        "receiver-envelope", "muzzle-axis", "stock-open-frame", "trigger-void", "rail-spine"
    ]
    view_kinds = ["front", "back", "left", "right", "top", "rear-three-quarter"]
    operator_ids = {
        "forgecad.geometry.primitive@2", "forgecad.geometry.profile-extrude@1",
        "forgecad.geometry.profile-loft@1", "forgecad.geometry.profile-loft@2",
        "forgecad.geometry.multi-loop-profile-loft@1",
        "forgecad.geometry.longitudinal-section-loft@1", "forgecad.geometry.subd-cage@1",
        "forgecad.geometry.subd-cage@2", "forgecad.geometry.authoring-mesh@1",
        "forgecad.geometry.surface-patch@1", "forgecad.geometry.surface-shell@1",
        "forgecad.geometry.revolve@1", "forgecad.geometry.tube-sweep@1",
        "forgecad.geometry.transform@2", "forgecad.geometry.mirror@1",
        "forgecad.geometry.array@1", "forgecad.geometry.bevel@1",
        "forgecad.geometry.bevel@2", "forgecad.geometry.normal-policy@1",
        "forgecad.geometry.panel@1", "forgecad.geometry.panel@2",
        "forgecad.geometry.vent-array@1", "forgecad.geometry.vent-array@2",
        "forgecad.geometry.recessed-channel@1", "forgecad.geometry.energy-core@1",
        "forgecad.geometry.joint-stack@1", "forgecad.geometry.part-output@1",
        "forgecad.geometry.boolean@1",
    }

    registry_fields = {
        "schema_version", "registry_id", "profile_id", "operator_catalog_sha256",
        "registry_policy", "groups", "canonical_sha256",
    }
    registry_properties = require_closed(
        registry, registry_fields, "ProductionWeaponAssemblyDecisionRegistry@1"
    )
    require(
        registry_properties["schema_version"].get("const")
        == "ProductionWeaponAssemblyDecisionRegistry@1"
        and registry_properties["profile_id"].get("const") == "fps-weapon-form-assembly@1"
        and registry_properties["registry_policy"].get("const")
        == "fps-weapon-closed-assembly-form-decision-registry@1"
        and registry_properties["operator_catalog_sha256"].get("$ref") == "#/$defs/sha256"
        and registry_properties["canonical_sha256"].get("$ref") == "#/$defs/sha256",
        "ProductionWeaponAssemblyDecisionRegistry@1 bindings drifted",
    )
    group_fields = {
        "group_id", "intent_kind", "part_ids", "source_node_ids", "parameter_ids",
        "allowed_operator_ids", "coupling_mode", "invariants", "affected_view_kinds",
        "priority",
    }
    group = registry.get("$defs", {}).get("group", {})
    group_properties = require_closed(
        group, group_fields, "ProductionWeaponAssemblyDecisionRegistryGroup@1"
    )
    require(
        group_properties["group_id"].get("enum") == group_ids
        and group_properties["intent_kind"].get("enum") == group_ids
        and group_properties["coupling_mode"].get("enum") == ["independent", "linked", "mirror"]
        and group_properties["affected_view_kinds"].get("const") == view_kinds
        and set(group_properties["allowed_operator_ids"].get("items", {}).get("enum", []))
        == operator_ids,
        "ProductionWeaponAssemblyDecisionRegistryGroup@1 vocabulary drifted",
    )
    groups = registry_properties["groups"]
    require(
        groups.get("minItems") == 5
        and groups.get("maxItems") == 5
        and groups.get("uniqueItems") is True
        and groups.get("items") is False
        and len(groups.get("prefixItems", [])) == 5
        and all(item.get("$ref") == "#/$defs/group" for item in groups["prefixItems"])
        and {
            item.get("contains", {}).get("properties", {}).get("group_id", {}).get("const")
            for item in groups.get("allOf", [])
        } == set(group_ids),
        "ProductionWeaponAssemblyDecisionRegistry@1 must freeze exactly five groups",
    )
    # These are the only coupled semantic groups admitted by the slice.  The
    # part sets intentionally do not claim coverage of every structural Part.
    expected_parts = {
        "receiver-envelope": {"receiver-main", "receiver-upper", "receiver-lower"},
        "muzzle-axis": {"muzzle-shroud", "muzzle-emitter", "muzzle-core", "energy-ring", "energy-core", "core-housing"},
        "stock-open-frame": {"rear-stock", "rear-cap", "underbrace"},
        "trigger-void": {"trigger-guard", "grip", "magazine"},
        "rail-spine": {"top-fin", "top-rail", "bottom-rail"},
    }
    require(
        set(expected_parts) == set(group_ids)
        and not any(expected_parts[first].intersection(expected_parts[second]) for first in group_ids for second in group_ids if first < second),
        "assembly registry five groups must have closed, non-overlapping Part sets",
    )

    request_fields = {
        "schema_version", "session_id", "project_id", "candidate_id", "candidate_state_sha256",
        "artifact_id", "artifact_sha256", "geometry_program_sha256",
        "geometry_program_canonical_sha256", "operator_catalog_sha256",
        "reference_canvas_canonical_sha256",
        "design_spec_canonical_sha256", "camera_lock_id", "camera_lock_canonical_sha256",
        "form_evidence_id", "form_evidence_object_sha256", "form_evidence_canonical_sha256",
        "form_art_evidence_id", "form_art_evidence_object_sha256",
        "form_art_evidence_canonical_sha256", "first_person_profile_id",
        "first_person_profile_sha256",
    }
    request_properties = require_closed(
        request, request_fields, "ProductionWeaponArtDecisionProposalGetRequest@1"
    )
    require(
        request_properties["schema_version"].get("const")
        == "ProductionWeaponArtDecisionProposalGetRequest@1",
        "ProductionWeaponArtDecisionProposalGetRequest@1 schema version drifted",
    )
    request_id_fields = {
        "session_id", "project_id", "candidate_id", "artifact_id",
        "camera_lock_id", "form_evidence_id", "form_art_evidence_id",
    }
    request_sha_fields = request_fields - {"schema_version", "first_person_profile_id", "first_person_profile_sha256"} - request_id_fields
    require(
        all(request_properties[field].get("$ref") == "#/$defs/id" for field in request_id_fields)
        and all(request_properties[field].get("$ref") == "#/$defs/sha256" for field in request_sha_fields)
        and request_properties["first_person_profile_id"].get("$ref") == "#/$defs/nullable_id"
        and request_properties["first_person_profile_sha256"].get("$ref") == "#/$defs/nullable_sha256",
        "ProductionWeaponArtDecisionProposalGetRequest@1 must accept only id/hash/null first-person bindings",
    )
    forbidden_request_names = {
        "path", "file_path", "absolute_path", "url", "uri", "script", "prompt", "raw",
        "raw_bytes", "bytes", "secret", "token", "status", "gate", "metric", "operator_id",
    }
    require(
        not forbidden_request_names.intersection(request_properties),
        "ProductionWeaponArtDecisionProposalGetRequest@1 exposes forbidden decision inputs",
    )

    result_fields = {
        "schema_version", "proposal_projection_id", "session_id", "project_id", "candidate_id",
        "candidate_state_sha256", "artifact_id", "artifact_sha256", "geometry_program_sha256",
        "geometry_program_canonical_sha256", "operator_catalog_sha256", "assembly_registry_id",
        "assembly_registry_canonical_sha256", "reference_canvas_canonical_sha256",
        "design_spec_canonical_sha256", "camera_lock_id", "camera_lock_canonical_sha256",
        "form_evidence_id", "form_evidence_object_sha256", "form_evidence_canonical_sha256",
        "form_art_evidence_id", "form_art_evidence_object_sha256",
        "form_art_evidence_canonical_sha256", "first_person_profile_id",
        "first_person_profile_sha256", "view_bindings", "assembly_group_decisions",
        "objective_policy", "gate_results", "blockers", "proposal_status", "read_only",
        "runtime_write_performed", "worker_invoked", "candidate_generated",
        "production_stage_advanced", "candidate_confirmed", "version_created",
        "export_performed", "replayed", "restart_hash_verified", "canonical_sha256",
    }
    result_properties = require_closed(
        result, result_fields, "ProductionWeaponArtDecisionProposalGetResult@1"
    )
    require(
        result_properties["schema_version"].get("const")
        == "ProductionWeaponArtDecisionProposalGetResult@1"
        and result_properties["objective_policy"].get("const")
        == "assembly-form-search-negative-space-line-flow-first-person@1"
        and result_properties["read_only"].get("const") is True
        and result_properties["restart_hash_verified"].get("const") is True
        and all(result_properties[field].get("const") is False for field in (
            "runtime_write_performed", "worker_invoked", "candidate_generated",
            "production_stage_advanced", "candidate_confirmed", "version_created",
            "export_performed",
        )),
        "ProductionWeaponArtDecisionProposalGetResult@1 must be strictly read-only",
    )
    result_id_fields = {
        "proposal_projection_id", "session_id", "project_id", "candidate_id", "artifact_id",
        "assembly_registry_id", "camera_lock_id", "form_evidence_id", "form_art_evidence_id",
    }
    result_sha_fields = {
        "candidate_state_sha256", "artifact_sha256", "geometry_program_sha256",
        "geometry_program_canonical_sha256", "operator_catalog_sha256",
        "assembly_registry_canonical_sha256", "reference_canvas_canonical_sha256",
        "design_spec_canonical_sha256", "camera_lock_canonical_sha256",
        "form_evidence_object_sha256", "form_evidence_canonical_sha256",
        "form_art_evidence_object_sha256", "form_art_evidence_canonical_sha256",
        "canonical_sha256",
    }
    require(
        all(result_properties[field].get("$ref") == "#/$defs/id" for field in result_id_fields)
        and all(result_properties[field].get("$ref") == "#/$defs/sha256" for field in result_sha_fields)
        and result_properties["first_person_profile_id"].get("$ref") == "#/$defs/nullable_id"
        and result_properties["first_person_profile_sha256"].get("$ref") == "#/$defs/nullable_sha256",
        "ProductionWeaponArtDecisionProposalGetResult@1 bindings must remain id/hash/null",
    )

    view_fields = {
        "view_kind", "view_id", "reference_id", "reference_sha256", "camera_hash",
        "camera_canonical_sha256", "render_set_object_sha256", "render_set_canonical_sha256",
        "form_evidence_view_receipt_object_sha256", "form_evidence_view_receipt_canonical_sha256",
        "form_art_evidence_view_receipt_object_sha256", "form_art_evidence_view_receipt_canonical_sha256",
        "target_sha256", "visual_structure_canonical_sha256", "part_id_status",
        "negative_space_status", "line_flow_status", "view_observation_status",
    }
    view = result.get("$defs", {}).get("view_binding", {})
    view_properties = require_closed(view, view_fields, "ProductionWeaponArtDecisionProposalViewBinding@1")
    require(
        result_properties["view_bindings"].get("minItems") == 6
        and result_properties["view_bindings"].get("maxItems") == 6
        and result_properties["view_bindings"].get("items") is False
        and len(result_properties["view_bindings"].get("prefixItems", [])) == 6
        and view_properties["view_kind"].get("$ref") == "#/$defs/view_kind"
        and view_properties["part_id_status"].get("$ref") == "#/$defs/observation_status"
        and {
            item.get("contains", {}).get("properties", {}).get("view_kind", {}).get("const")
            for item in result_properties["view_bindings"].get("allOf", [])
        } == set(view_kinds),
        "result must freeze six ordered view bindings",
    )
    group_decision_fields = {
        "group_id", "status", "part_ids", "source_node_ids", "parameter_ids",
        "allowed_operator_ids", "coupling_mode", "invariants", "affected_view_kinds",
        "blocker_codes",
    }
    group_decision = result.get("$defs", {}).get("assembly_group_decision", {})
    group_decision_properties = require_closed(
        group_decision, group_decision_fields, "ProductionWeaponArtDecisionProposalAssemblyGroupDecision@1"
    )
    require(
        result_properties["assembly_group_decisions"].get("minItems") == 5
        and result_properties["assembly_group_decisions"].get("maxItems") == 5
        and result_properties["assembly_group_decisions"].get("items") is False
        and len(result_properties["assembly_group_decisions"].get("prefixItems", [])) == 5
        and group_decision_properties["group_id"].get("enum") == group_ids
        and {
            item.get("contains", {}).get("properties", {}).get("group_id", {}).get("const")
            for item in result_properties["assembly_group_decisions"].get("allOf", [])
        } == set(group_ids),
        "result must freeze five assembly group decisions",
    )
    gate_fields = {"gate_id", "status", "evidence_sha256", "blocker_codes"}
    gate = result.get("$defs", {}).get("gate_result", {})
    gate_properties = require_closed(gate, gate_fields, "ProductionWeaponArtDecisionProposalGateResult@1")
    gate_ids = [
        "lineage", "reference-annotation", "camera", "assembly-registry", "parameter-sink",
        "negative-space", "line-flow", "first-person-readability", "candidate-search-critic",
        "surface-scope",
    ]
    require(
        result_properties["gate_results"].get("minItems") == 10
        and result_properties["gate_results"].get("maxItems") == 10
        and result_properties["gate_results"].get("items") is False
        and len(result_properties["gate_results"].get("prefixItems", [])) == 10
        and gate_properties["gate_id"].get("enum") == gate_ids
        and gate_properties["status"].get("enum") == ["PASS", "BLOCKED", "NOT_RUN", "LOCKED"]
        and gate_properties["evidence_sha256"].get("$ref") == "#/$defs/nullable_sha256"
        and {
            item.get("contains", {}).get("properties", {}).get("gate_id", {}).get("const")
            for item in result_properties["gate_results"].get("allOf", [])
        } == set(gate_ids),
        "result must freeze ten gate decisions",
    )
    blocker_fields = {"blocker_code", "scope", "group_id", "view_kind", "evidence_sha256"}
    blocker = result.get("$defs", {}).get("blocker", {})
    blocker_properties = require_closed(blocker, blocker_fields, "ProductionWeaponArtDecisionProposalBlocker@1")
    blocker_codes = {
        "BLOCKED_LINEAGE", "BLOCKED_REFERENCE_ANNOTATION", "BLOCKED_CAMERA",
        "BLOCKED_NEGATIVE_SPACE", "BLOCKED_LINE_FLOW", "BLOCKED_FIRST_PERSON_PROFILE",
        "BLOCKED_ASSEMBLY_REGISTRY", "BLOCKED_PARAMETER_SINK", "NO_STRICT_MULTI_VIEW_IMPROVEMENT",
    }
    require(
        blocker_properties["blocker_code"].get("$ref") == "#/$defs/blocker_code"
        and blocker_properties["scope"].get("enum") == ["global", "assembly", "view"]
        and blocker_properties["group_id"].get("$ref") == "#/$defs/nullable_id"
        and blocker_properties["evidence_sha256"].get("$ref") == "#/$defs/nullable_sha256"
        and result_properties["blockers"].get("maxItems") == 64,
        "result blockers must be typed, bounded and hash/id-only",
    )
    require(
        result_properties["proposal_status"].get("enum") == [
            "READY_ASSEMBLY_FORM_SEARCH", "BLOCKED_LINEAGE", "BLOCKED_REFERENCE_ANNOTATION",
            "BLOCKED_CAMERA", "BLOCKED_NEGATIVE_SPACE", "BLOCKED_LINE_FLOW",
            "BLOCKED_FIRST_PERSON_PROFILE", "BLOCKED_ASSEMBLY_REGISTRY",
            "BLOCKED_PARAMETER_SINK", "NO_STRICT_MULTI_VIEW_IMPROVEMENT",
        ],
        "proposal status vocabulary drifted",
    )

    # The current real six-view fixture has durable six-view lineage, while
    # negative-space/line-flow remain unknown or inferred, first-person is
    # absent, and no assembly parameter sink is proven.  Keep that result
    # representable without allowing a misleading READY/PASS claim.
    fixture_blockers = [
        {"blocker_code": "BLOCKED_NEGATIVE_SPACE", "scope": "view", "group_id": None, "view_kind": "left", "evidence_sha256": None},
        {"blocker_code": "BLOCKED_LINE_FLOW", "scope": "view", "group_id": None, "view_kind": "left", "evidence_sha256": None},
        {"blocker_code": "BLOCKED_FIRST_PERSON_PROFILE", "scope": "global", "group_id": None, "view_kind": None, "evidence_sha256": None},
        {"blocker_code": "BLOCKED_PARAMETER_SINK", "scope": "assembly", "group_id": "receiver-envelope", "view_kind": None, "evidence_sha256": None},
    ]
    require(
        all(item["blocker_code"] in blocker_codes for item in fixture_blockers)
        and fixture_blockers[0]["view_kind"] in view_kinds
        and fixture_blockers[3]["group_id"] in group_ids
        and all(item["evidence_sha256"] is None for item in fixture_blockers),
        "real six-view fixture blocker projection must remain representable",
    )
    fixture_flags = {
        "read_only": True,
        "runtime_write_performed": False,
        "worker_invoked": False,
        "candidate_generated": False,
        "production_stage_advanced": False,
        "candidate_confirmed": False,
        "version_created": False,
        "export_performed": False,
        "restart_hash_verified": True,
    }
    require(
        fixture_flags["read_only"] is True
        and all(not fixture_flags[field] for field in (
            "runtime_write_performed", "worker_invoked", "candidate_generated",
            "production_stage_advanced", "candidate_confirmed", "version_created",
            "export_performed",
        ))
        and fixture_flags["restart_hash_verified"] is True,
        "real six-view fixture blocker result must remain strictly read-only",
    )


def check_production_weapon_assembly_parameter_sink_contracts_legacy() -> None:
    """Superseded six-row draft retained only for diff context."""
    schema = load_schema("production-weapon-assembly-parameter-sink-registry.schema.json")

    def require_closed(node: dict, fields: set[str], label: str) -> dict:
        properties = node.get("properties", {})
        require(
            node.get("type") == "object"
            and node.get("additionalProperties") is False
            and set(node.get("required", [])) == fields
            and set(properties) == fields,
            f"{label} must be closed with its exact field set",
        )
        return properties

    supported_groups = ["receiver-envelope", "muzzle-axis"]
    supported_parameters = [
        "receiver-envelope-width",
        "receiver-envelope-height",
        "receiver-envelope-shoulder",
        "muzzle-axis-shroud-envelope",
        "muzzle-axis-emitter-envelope",
        "muzzle-axis-core-aperture",
    ]
    unavailable_parameters = [
        "stock-open-frame-clearance",
        "stock-open-frame-angle",
        "trigger-void-clearance",
        "trigger-void-centroid",
        "rail-spine-continuity",
        "rail-spine-offset",
    ]
    all_parameters = supported_parameters + unavailable_parameters
    mutators = [
        "forgecad.assembly.mutator.receiver-envelope@1",
        "forgecad.assembly.mutator.muzzle-axis@1",
    ]
    effects = ["silhouette", "negative-space", "line-flow", "first-person-readability"]
    views = ["front", "back", "left", "right", "top", "rear-three-quarter"]
    operator_ids = {
        "forgecad.geometry.primitive@2", "forgecad.geometry.profile-extrude@1",
        "forgecad.geometry.profile-loft@1", "forgecad.geometry.profile-loft@2",
        "forgecad.geometry.multi-loop-profile-loft@1",
        "forgecad.geometry.longitudinal-section-loft@1", "forgecad.geometry.subd-cage@1",
        "forgecad.geometry.subd-cage@2", "forgecad.geometry.authoring-mesh@1",
        "forgecad.geometry.surface-patch@1", "forgecad.geometry.surface-shell@1",
        "forgecad.geometry.revolve@1", "forgecad.geometry.tube-sweep@1",
        "forgecad.geometry.transform@2", "forgecad.geometry.mirror@1",
        "forgecad.geometry.array@1", "forgecad.geometry.bevel@1",
        "forgecad.geometry.bevel@2", "forgecad.geometry.normal-policy@1",
        "forgecad.geometry.panel@1", "forgecad.geometry.panel@2",
        "forgecad.geometry.vent-array@1", "forgecad.geometry.vent-array@2",
        "forgecad.geometry.recessed-channel@1", "forgecad.geometry.energy-core@1",
        "forgecad.geometry.joint-stack@1", "forgecad.geometry.part-output@1",
        "forgecad.geometry.boolean@1",
    }

    registry_fields = {
        "schema_version", "sink_registry_id", "profile_id", "sink_policy",
        "session_id", "project_id", "candidate_id", "candidate_state_sha256",
        "artifact_id", "artifact_sha256", "geometry_program_sha256",
        "geometry_program_canonical_sha256", "operator_catalog_sha256",
        "assembly_registry_id", "assembly_registry_canonical_sha256",
        "supported_group_ids", "sinks", "unavailable_parameter_ids", "status",
        "read_only", "runtime_write_performed", "worker_invoked", "candidate_generated",
        "production_stage_advanced", "candidate_confirmed", "version_created",
        "export_performed", "canonical_sha256",
    }
    properties = require_closed(
        schema, registry_fields, "ProductionWeaponAssemblyParameterSinkRegistry@1"
    )
    require(
        properties["schema_version"].get("const")
        == "ProductionWeaponAssemblyParameterSinkRegistry@1"
        and properties["profile_id"].get("const") == "fps-weapon-form-assembly@1"
        and properties["sink_policy"].get("const")
        == "fps-weapon-product-owned-aggregate-parameter-sink-registry@1"
        and properties["supported_group_ids"].get("const") == supported_groups
        and properties["status"].get("enum") == ["PARTIAL_TYPED_SINKS", "READY"]
        and properties["read_only"].get("const") is True
        and all(properties[field].get("const") is False for field in (
            "runtime_write_performed", "worker_invoked", "candidate_generated",
            "production_stage_advanced", "candidate_confirmed", "version_created",
            "export_performed",
        )),
        "ProductionWeaponAssemblyParameterSinkRegistry@1 must be a non-promoting read-only projection",
    )
    require(
        properties["sinks"].get("minItems") == 0
        and properties["sinks"].get("maxItems") == 6
        and properties["sinks"].get("uniqueItems") is True
        and properties["sinks"].get("items", {}).get("$ref") == "#/$defs/sink"
        and properties["unavailable_parameter_ids"].get("minItems") == 6
        and properties["unavailable_parameter_ids"].get("maxItems") == 12
        and properties["unavailable_parameter_ids"].get("uniqueItems") is True
        and properties["unavailable_parameter_ids"].get("items", {}).get("$ref")
        == "#/$defs/parameter_id",
        "sink coverage must be bounded and keep unavailable parameter IDs explicit",
    )
    unavailable_contains = {
        item.get("contains", {}).get("const")
        for item in properties["unavailable_parameter_ids"].get("allOf", [])
    }
    require(
        unavailable_contains == set(unavailable_parameters),
        "unavailable_parameter_ids must always name the unimplemented stock/trigger/rail slice",
    )
    status_guards = schema.get("allOf", [])
    require(
        any(
            guard.get("if", {}).get("properties", {}).get("status", {}).get("const")
            == "PARTIAL_TYPED_SINKS"
            and guard.get("then", {}).get("properties", {}).get("sinks", {}).get("maxItems") == 5
            for guard in status_guards
        )
        and any(
            guard.get("if", {}).get("properties", {}).get("status", {}).get("const") == "READY"
            and guard.get("then", {}).get("properties", {}).get("sinks", {}).get("minItems") == 6
            and guard.get("then", {}).get("properties", {}).get("sinks", {}).get("maxItems") == 6
            for guard in status_guards
        ),
        "status must distinguish partial coverage from the six-sink READY slice",
    )

    sink_fields = {
        "parameter_id", "group_id", "mutator_id", "semantic", "target_part_ids",
        "source_node_ids", "operator_ids", "bounds", "current", "unit", "step",
        "evidence_requirements", "view_effects", "application_status",
    }
    sink_properties = require_closed(
        schema.get("$defs", {}).get("sink", {}),
        sink_fields,
        "ProductionWeaponAssemblyParameterSink@1",
    )
    require(
        sink_properties["parameter_id"].get("$ref") == "#/$defs/supported_parameter_id"
        and sink_properties["group_id"].get("$ref") == "#/$defs/group_id"
        and sink_properties["mutator_id"].get("$ref") == "#/$defs/mutator_id"
        and sink_properties["semantic"].get("$ref") == "#/$defs/semantic"
        and sink_properties["target_part_ids"].get("$ref") == "#/$defs/id_list"
        and sink_properties["source_node_ids"].get("$ref") == "#/$defs/id_list"
        and sink_properties["operator_ids"].get("items", {}).get("$ref") == "#/$defs/operator_id"
        and sink_properties["bounds"].get("$ref") == "#/$defs/bounds"
        and sink_properties["unit"].get("enum") == ["meter", "ratio"]
        and sink_properties["step"].get("exclusiveMinimum") == 0
        and sink_properties["application_status"].get("const") == "AVAILABLE",
        "sink targets must be typed mutator bindings, not paths or descriptors",
    )
    require(
        schema["$defs"]["supported_parameter_id"].get("enum") == supported_parameters
        and schema["$defs"]["semantic"].get("enum") == supported_parameters
        and schema["$defs"]["group_id"].get("enum") == supported_groups
        and schema["$defs"]["mutator_id"].get("enum") == mutators
        and schema["$defs"]["parameter_id"].get("enum") == all_parameters,
        "sink semantic and mutator vocabularies must be the closed first slice",
    )
    bounds = schema["$defs"]["bounds"]
    require(
        set(bounds.get("required", [])) == {"min", "max"}
        and set(bounds.get("properties", {})) == {"min", "max"}
        and bounds.get("additionalProperties") is False,
        "sink bounds must be a closed min/max pair",
    )
    view_effect = schema["$defs"]["view_effect"]
    require(
        set(view_effect.get("required", [])) == {"view_kind", "effect"}
        and set(view_effect.get("properties", {})) == {"view_kind", "effect"}
        and view_effect.get("additionalProperties") is False
        and view_effect["properties"]["view_kind"].get("$ref") == "#/$defs/view_kind"
        and view_effect["properties"]["effect"].get("enum") == effects,
        "sink view effects must be closed and product-owned",
    )
    evidence = sink_properties["evidence_requirements"]
    require(
        evidence.get("minItems") == 1
        and evidence.get("maxItems") == 6
        and evidence.get("uniqueItems") is True
        and evidence.get("items", {}).get("enum") == [
            "assembly-registry", "geometry-program", "operator-catalog",
            "artifact-readback", "candidate-state", "view-evidence",
        ],
        "sink evidence requirements must use the closed hash-bound evidence vocabulary",
    )
    forbidden_names = {
        "json_pointer", "json_path", "path", "file_path", "absolute_path", "script",
        "script_path", "expression", "parameter_key", "component", "python",
        "javascript", "raw", "raw_bytes", "bytes", "url", "uri", "environment", "env",
    }

    def property_names(node: object) -> set[str]:
        names: set[str] = set()
        if isinstance(node, dict):
            properties_node = node.get("properties")
            if isinstance(properties_node, dict):
                names.update(properties_node)
            for value in node.values():
                names.update(property_names(value))
        elif isinstance(node, list):
            for value in node:
                names.update(property_names(value))
        return names

    require(
        not forbidden_names.intersection(property_names(schema)),
        "sink registry must reject arbitrary JSON paths, scripts, expressions and raw inputs",
    )


def check_production_weapon_assembly_parameter_sink_contracts() -> None:
    """Freeze the exact twelve-row typed sink projection and its readback pair."""
    registry = load_schema("production-weapon-assembly-parameter-sink-registry.schema.json")
    get_request = load_schema("production-weapon-assembly-parameter-sink-get-request.schema.json")
    get_result = load_schema("production-weapon-assembly-parameter-sink-get-result.schema.json")

    def require_closed(node: dict, fields: set[str], label: str) -> dict:
        properties = node.get("properties", {})
        require(
            node.get("type") == "object"
            and node.get("additionalProperties") is False
            and set(node.get("required", [])) == fields
            and set(properties) == fields,
            f"{label} must be closed with its exact field set",
        )
        return properties

    supported_groups = ["receiver-envelope", "muzzle-axis", "stock-open-frame"]
    available_parameters = [
        "receiver-envelope-width", "receiver-envelope-height", "receiver-envelope-shoulder",
        "muzzle-axis-shroud-envelope", "muzzle-axis-emitter-envelope", "muzzle-axis-core-aperture",
        "stock-open-frame-clearance", "stock-open-frame-angle",
    ]
    unavailable_parameters = [
        "trigger-void-clearance", "trigger-void-centroid",
        "rail-spine-continuity", "rail-spine-offset",
    ]
    parameter_ids = available_parameters + unavailable_parameters
    mutator_ids = [
        "forgecad.assembly.mutator.receiver-envelope@1",
        "forgecad.assembly.mutator.muzzle-axis@1",
        "forgecad.assembly.mutator.stock-open-frame@1",
    ]
    operator_ids = {
        "forgecad.geometry.primitive@2", "forgecad.geometry.profile-extrude@1",
        "forgecad.geometry.profile-loft@1", "forgecad.geometry.profile-loft@2",
        "forgecad.geometry.multi-loop-profile-loft@1",
        "forgecad.geometry.longitudinal-section-loft@1", "forgecad.geometry.subd-cage@1",
        "forgecad.geometry.subd-cage@2", "forgecad.geometry.authoring-mesh@1",
        "forgecad.geometry.surface-patch@1", "forgecad.geometry.surface-shell@1",
        "forgecad.geometry.revolve@1", "forgecad.geometry.tube-sweep@1",
        "forgecad.geometry.transform@2", "forgecad.geometry.mirror@1",
        "forgecad.geometry.array@1", "forgecad.geometry.bevel@1",
        "forgecad.geometry.bevel@2", "forgecad.geometry.normal-policy@1",
        "forgecad.geometry.panel@1", "forgecad.geometry.panel@2",
        "forgecad.geometry.vent-array@1", "forgecad.geometry.vent-array@2",
        "forgecad.geometry.recessed-channel@1", "forgecad.geometry.energy-core@1",
        "forgecad.geometry.joint-stack@1", "forgecad.geometry.part-output@1",
        "forgecad.geometry.boolean@1",
    }

    registry_fields = {
        "schema_version", "sink_registry_id", "profile_id", "sink_policy",
        "session_id", "project_id", "candidate_id", "candidate_state_sha256",
        "artifact_id", "artifact_sha256", "geometry_program_sha256",
        "geometry_program_canonical_sha256", "operator_catalog_sha256",
        "assembly_registry_id", "assembly_registry_canonical_sha256",
        "supported_group_ids", "sinks", "unavailable_parameter_ids", "status",
        "read_only", "runtime_write_performed", "worker_invoked", "candidate_generated",
        "production_stage_advanced", "candidate_confirmed", "version_created",
        "export_performed", "canonical_sha256",
    }
    registry_properties = require_closed(
        registry, registry_fields, "ProductionWeaponAssemblyParameterSinkRegistry@1"
    )
    require(
        registry_properties["schema_version"].get("const")
        == "ProductionWeaponAssemblyParameterSinkRegistry@1"
        and registry_properties["profile_id"].get("const") == "fps-weapon-form-assembly@1"
        and registry_properties["sink_policy"].get("const")
        == "fps-weapon-product-owned-aggregate-parameter-sink-registry@1"
        and registry_properties["supported_group_ids"].get("const") == supported_groups
        and registry_properties["status"].get("enum") == ["PARTIAL_TYPED_SINKS", "READY"]
        and registry_properties["sinks"].get("minItems") == 0
        and registry_properties["sinks"].get("maxItems") == 8
        and registry_properties["sinks"].get("uniqueItems") is True
        and registry_properties["unavailable_parameter_ids"].get("minItems") == 4
        and registry_properties["unavailable_parameter_ids"].get("maxItems") == 12
        and registry_properties["unavailable_parameter_ids"].get("uniqueItems") is True
        and registry_properties["read_only"].get("const") is True
        and all(registry_properties[field].get("const") is False for field in (
            "runtime_write_performed", "worker_invoked", "candidate_generated",
            "production_stage_advanced", "candidate_confirmed", "version_created",
            "export_performed",
        )),
        "ProductionWeaponAssemblyParameterSinkRegistry@1 must be bounded, partial and non-promoting",
    )
    unavailable_contains = {
        item.get("contains", {}).get("const")
        for item in registry_properties["unavailable_parameter_ids"].get("allOf", [])
    }
    require(
        unavailable_contains == set(unavailable_parameters)
        and registry["$defs"]["parameter_id"].get("enum") == parameter_ids
        and registry["$defs"]["supported_parameter_id"].get("enum") == available_parameters
        and registry["$defs"]["group_id"].get("enum") == supported_groups
        and registry["$defs"]["mutator_id"].get("enum") == mutator_ids,
        "Registry parameter, group, mutator and unavailable vocabularies must be closed",
    )
    status_guards = registry.get("allOf", [])
    ready_guards = [
        guard for guard in status_guards
        if guard.get("if", {}).get("properties", {}).get("status", {}).get("const") == "READY"
    ]
    require(
        any(
            guard.get("if", {}).get("properties", {}).get("status", {}).get("const")
            == "PARTIAL_TYPED_SINKS"
            and guard.get("then", {}).get("properties", {}).get("sinks", {}).get("maxItems") == 7
            and guard.get("then", {}).get("properties", {}).get("unavailable_parameter_ids", {}).get("minItems") == 5
            and guard.get("then", {}).get("properties", {}).get("unavailable_parameter_ids", {}).get("maxItems") == 12
            for guard in status_guards
        )
        and len(ready_guards) == 1
        and ready_guards[0].get("then", {}).get("properties", {}).get("sinks", {}).get("minItems") == 8
        and ready_guards[0].get("then", {}).get("properties", {}).get("sinks", {}).get("maxItems") == 8
        and ready_guards[0].get("then", {}).get("properties", {}).get("unavailable_parameter_ids", {}).get("const") == unavailable_parameters,
        "PARTIAL and READY must describe available-only sinks and explicit unavailable coverage",
    )
    ready_sink_contains = [
        item.get("contains", {}).get("properties", {}).get("parameter_id", {}).get("const")
        for item in ready_guards[0].get("then", {}).get("properties", {}).get("sinks", {}).get("allOf", [])
    ]
    require(
        len(ready_sink_contains) == len(available_parameters)
        and len(set(ready_sink_contains)) == len(available_parameters)
        and set(ready_sink_contains) == set(available_parameters),
        "READY must contain each supported parameter ID exactly once",
    )

    sink_fields = {
        "parameter_id", "group_id", "mutator_id", "current", "min", "max", "step",
        "unit", "application_status", "blocker_codes", "target_part_ids",
        "source_node_ids", "operator_ids", "evidence_requirements",
    }
    sink_properties = require_closed(
        registry.get("$defs", {}).get("sink", {}),
        sink_fields,
        "ProductionWeaponAssemblyParameterSink@1",
    )
    require(
        sink_properties["parameter_id"].get("$ref") == "#/$defs/supported_parameter_id"
        and sink_properties["group_id"].get("$ref") == "#/$defs/group_id"
        and sink_properties["mutator_id"].get("$ref") == "#/$defs/mutator_id"
        and sink_properties["current"].get("type") == "number"
        and sink_properties["min"].get("type") == "number"
        and sink_properties["max"].get("type") == "number"
        and sink_properties["step"].get("type") == "number"
        and sink_properties["step"].get("exclusiveMinimum") == 0
        and sink_properties["unit"].get("enum") == ["meter", "radian", "ratio"]
        and sink_properties["application_status"].get("const") == "AVAILABLE"
        and sink_properties["blocker_codes"].get("items", {}).get("$ref")
        == "#/$defs/blocker_code"
        and sink_properties["target_part_ids"].get("$ref") == "#/$defs/id_list"
        and sink_properties["source_node_ids"].get("$ref") == "#/$defs/id_list"
        and sink_properties["operator_ids"].get("items", {}).get("$ref")
        == "#/$defs/operator_id"
        and sink_properties["evidence_requirements"].get("items", {}).get("$ref")
        == "#/$defs/evidence_requirement",
        "Sink rows must expose only typed mutator bindings and bounded evidence fields",
    )
    require(
        registry["$defs"]["operator_id"].get("enum")
        and set(registry["$defs"]["operator_id"].get("enum", [])) == operator_ids
        and registry["$defs"]["blocker_code"].get("const") == "BLOCKED_PARAMETER_SINK"
        and registry["$defs"]["evidence_requirement"].get("enum") == [
            "assembly-registry", "geometry-program", "operator-catalog",
            "artifact-readback", "candidate-state",
        ],
        "Sink operator, blocker and evidence vocabularies must remain product-owned",
    )
    require(
        sink_properties["application_status"].get("const") == "AVAILABLE"
        and sink_properties["blocker_codes"].get("maxItems") == 0,
        "Sink rows must be real AVAILABLE mutator bindings; unavailable IDs stay outside sinks",
    )

    request_fields = {
        "schema_version", "sink_registry_id", "session_id", "project_id", "candidate_id",
        "candidate_state_sha256", "artifact_id", "artifact_sha256", "geometry_program_sha256",
        "geometry_program_canonical_sha256", "operator_catalog_sha256", "assembly_registry_id",
        "assembly_registry_canonical_sha256",
    }
    request_properties = require_closed(
        get_request, request_fields, "ProductionWeaponAssemblyParameterSinkGetRequest@1"
    )
    request_id_fields = {"sink_registry_id", "session_id", "project_id", "candidate_id", "artifact_id", "assembly_registry_id"}
    request_sha_fields = request_fields - {"schema_version"} - request_id_fields
    require(
        request_properties["schema_version"].get("const")
        == "ProductionWeaponAssemblyParameterSinkGetRequest@1"
        and all(request_properties[field].get("$ref") == "#/$defs/id" for field in request_id_fields)
        and all(request_properties[field].get("$ref") == "#/$defs/sha256" for field in request_sha_fields),
        "Sink GetRequest must be exact id/hash-only lineage",
    )

    result_fields = {
        "schema_version", "registry", "registry_canonical_sha256", "recomputed",
        "restart_hash_verified", "read_only", "structural_status", "quality_status",
        "visual_quality_status", "human_review_status", "commercial_engine_status",
        "runtime_write_performed", "worker_invoked", "candidate_generated",
        "production_stage_advanced", "candidate_confirmed", "version_created",
        "export_performed",
    }
    result_properties = require_closed(
        get_result, result_fields, "ProductionWeaponAssemblyParameterSinkGetResult@1"
    )
    require(
        result_properties["schema_version"].get("const")
        == "ProductionWeaponAssemblyParameterSinkGetResult@1"
        and result_properties["registry"].get("$ref")
        == "https://forgecad.local/contracts/production-weapon-assembly-parameter-sink-registry.schema.json"
        and result_properties["registry_canonical_sha256"].get("$ref") == "#/$defs/sha256"
        and result_properties["recomputed"].get("const") is True
        and result_properties["restart_hash_verified"].get("const") is True
        and result_properties["read_only"].get("const") is True
        and result_properties["structural_status"].get("const") == "structural_only"
        and result_properties["quality_status"].get("const") == "structural_only"
        and result_properties["visual_quality_status"].get("const") == "NOT_PROVEN"
        and result_properties["human_review_status"].get("const") == "NOT_RUN"
        and result_properties["commercial_engine_status"].get("const") == "NOT_RUN"
        and all(result_properties[field].get("const") is False for field in (
            "runtime_write_performed", "worker_invoked", "candidate_generated",
            "production_stage_advanced", "candidate_confirmed", "version_created",
            "export_performed",
        )),
        "Sink GetResult must remain structural-only, recomputed and non-promoting",
    )

    forbidden_names = {
        "json_pointer", "json_path", "path", "file_path", "absolute_path", "script",
        "script_path", "expression", "parameter_key", "component", "python",
        "javascript", "raw", "raw_bytes", "bytes", "url", "uri", "environment", "env",
    }

    def property_names(node: object) -> set[str]:
        names: set[str] = set()
        if isinstance(node, dict):
            properties = node.get("properties")
            if isinstance(properties, dict):
                names.update(properties)
            for value in node.values():
                names.update(property_names(value))
        elif isinstance(node, list):
            for value in node:
                names.update(property_names(value))
        return names

    for document, filename in [
        (registry, "production-weapon-assembly-parameter-sink-registry.schema.json"),
        (get_request, "production-weapon-assembly-parameter-sink-get-request.schema.json"),
        (get_result, "production-weapon-assembly-parameter-sink-get-result.schema.json"),
    ]:
        require(
            not forbidden_names.intersection(property_names(document)),
            f"{filename} must reject arbitrary paths, scripts, expressions and raw inputs",
        )


def check_candidate_topology_quality_contracts() -> None:
    """Keep the candidate-wide objective topology gate closed and non-artistic."""
    record_fields = {
        "schema_version", "topology_quality_id", "project_id", "candidate_id",
        "candidate_state_sha256", "artifact_id", "artifact_sha256",
        "artifact_readback_sha256", "artifact_readback_object_sha256",
        "geometry_candidate_evidence_sha256", "geometry_program_sha256",
        "geometry_program_object_sha256", "operator_catalog_sha256",
        "readback_config_sha256", "part_inventory_sha256", "part_ids",
        "part_topology_snapshot_sha256s", "authoring_topology_status",
        "part_authoring_topology_sha256s", "topology_quality_policy",
        "topology_quality_policy_sha256", "from_stage", "to_stage",
        "topology_status", "thresholds", "metrics", "hard_gate",
        "validator_status", "hard_gate_passed", "edge_flow_status",
        "artistic_quality_status", "visual_quality_status", "materialization_status",
        "quality_status", "runtime_write_performed", "candidate_confirmed",
        "version_created", "export_performed", "request_sha256", "input_sha256",
        "canonical_sha256", "created_at",
    }
    record = load_schema("candidate-topology-quality.schema.json")
    properties = record.get("properties", {})
    require(
        record.get("type") == "object"
        and record.get("additionalProperties") is False
        and set(record.get("required", [])) == record_fields
        and set(properties) == record_fields
        and properties["schema_version"].get("const") == "CandidateTopologyQuality@1"
        and properties["from_stage"].get("const") == "gray-model"
        and properties["to_stage"].get("const") == "topology"
        and properties["topology_quality_policy"].get("const")
        == "candidate-topology-hard-gate@1"
        and properties["edge_flow_status"].get("const") == "NOT_PROVEN"
        and properties["artistic_quality_status"].get("const") == "NOT_PROVEN"
        and properties["visual_quality_status"].get("const") == "NOT_PROVEN"
        and properties["quality_status"].get("const") == "structural_only"
        and properties["runtime_write_performed"].get("const") is True
        and properties["candidate_confirmed"].get("const") is False
        and properties["version_created"].get("const") is False
        and properties["export_performed"].get("const") is False,
        "CandidateTopologyQuality@1 must be an immutable, structural-only gray-model to topology receipt",
    )
    require(
        properties["part_ids"].get("$ref") == "#/$defs/id_list"
        and properties["part_topology_snapshot_sha256s"].get("$ref")
        == "#/$defs/sha256_list"
        and properties["part_authoring_topology_sha256s"].get("$ref")
        == "#/$defs/nullable_sha256_list"
        and properties["authoring_topology_status"].get("enum")
        == ["complete", "partial", "not-available"],
        "CandidateTopologyQuality@1 must bind every Part and allow per-Part authoring topology absence",
    )
    thresholds = record["$defs"]["thresholds"]
    require(
        set(thresholds.get("required", []))
        == {
            "max_triangle_aspect_ratio", "max_vertex_valence", "min_triangle_area_m2",
            "min_semantic_part_coverage", "min_semantic_material_zone_coverage",
            "min_semantic_source_node_coverage",
        },
        "CandidateTopologyQuality@1 thresholds must cover aspect, valence and semantic coverage",
    )
    metrics = record["$defs"]["metrics"]
    require(
        set(metrics.get("required", []))
        == {
            "invalid_index_count", "non_finite_count", "degenerate_triangle_count",
            "boundary_edge_count", "non_manifold_edge_count", "orientation_conflict_count",
            "winding_error_count", "part_count", "solid_part_count", "non_solid_part_count",
            "solid_boundary_violation_count", "triangle_count", "vertex_count", "edge_count",
            "min_triangle_area_m2", "max_triangle_aspect_ratio", "max_vertex_valence",
            "normal_non_finite_count", "normal_non_unit_count", "normal_alignment_error_count",
            "uv_non_finite_count", "uv_degenerate_triangle_count", "tangent_non_finite_count",
            "tangent_orthogonality_error_count", "tangent_handedness_error_count",
            "semantic_part_coverage", "semantic_material_zone_coverage",
            "semantic_source_node_coverage",
        },
        "CandidateTopologyQuality@1 metrics must cover topology, geometry, normal, UV, tangent and semantic evidence",
    )
    hard_gate = record["$defs"]["hard_gate"]
    require(
        set(hard_gate.get("required", []))
        == {
            "finite_geometry", "valid_indices", "non_degenerate_triangles",
            "boundary_policy", "manifold", "orientation", "counts_within_budget",
            "triangle_aspect_ratio", "vertex_valence", "normal_integrity",
            "uv_integrity", "tangent_integrity", "semantic_coverage",
        },
        "CandidateTopologyQuality@1 hard gate must expose every objective condition",
    )
    require(
        record["$defs"]["id_list"].get("minItems") == 1
        and record["$defs"]["id_list"].get("maxItems") == 512
        and record["$defs"]["sha256_list"].get("minItems") == 1
        and record["$defs"]["sha256_list"].get("maxItems") == 512
        and record["$defs"]["nullable_sha256_list"].get("minItems") == 1
        and record["$defs"]["nullable_sha256_list"].get("maxItems") == 512,
        "CandidateTopologyQuality@1 Part bindings must be bounded and non-empty",
    )

    prepare = load_schema("candidate-topology-quality-prepare-request.schema.json")
    prepare_fields = {
        "schema_version", "topology_quality_id", "project_id", "candidate_id",
        "candidate_state_sha256", "artifact_id", "artifact_sha256",
        "artifact_readback_sha256", "artifact_readback_object_sha256",
        "geometry_candidate_evidence_sha256", "geometry_program_sha256",
        "geometry_program_object_sha256", "operator_catalog_sha256",
        "readback_config_sha256", "part_inventory_sha256", "part_ids",
        "part_topology_snapshot_sha256s", "authoring_topology_status",
        "part_authoring_topology_sha256s", "topology_quality_policy",
        "topology_quality_policy_sha256", "from_stage", "to_stage", "input_sha256",
        "idempotency_key",
    }
    prepare_properties = prepare.get("properties", {})
    require(
        prepare.get("type") == "object"
        and prepare.get("additionalProperties") is False
        and set(prepare.get("required", [])) == prepare_fields
        and set(prepare_properties) == prepare_fields
        and prepare_properties["schema_version"].get("const")
        == "CandidateTopologyQualityPrepareRequest@1"
        and prepare_properties["from_stage"].get("const") == "gray-model"
        and prepare_properties["to_stage"].get("const") == "topology"
        and prepare_properties["part_ids"].get("$ref") == "#/$defs/id_list"
        and prepare_properties["part_topology_snapshot_sha256s"].get("$ref")
        == "#/$defs/sha256_list"
        and prepare_properties["part_authoring_topology_sha256s"].get("$ref")
        == "#/$defs/nullable_sha256_list",
        "CandidateTopologyQualityPrepareRequest@1 must be closed and candidate-wide",
    )

    get_request = load_schema("candidate-topology-quality-get-request.schema.json")
    get_fields = {"schema_version", "topology_quality_id", "project_id", "candidate_id"}
    require(
        get_request.get("type") == "object"
        and get_request.get("additionalProperties") is False
        and set(get_request.get("required", [])) == get_fields
        and set(get_request.get("properties", {})) == get_fields
        and get_request["properties"]["schema_version"].get("const")
        == "CandidateTopologyQualityGetRequest@1",
        "CandidateTopologyQualityGetRequest@1 must be closed and project/candidate bound",
    )

    result_fields = {
        "schema_version", "topology_quality", "replayed", "runtime_write",
        "production_stage_advanced", "candidate_confirmed", "version_created",
        "export_performed",
    }
    for filename, schema_version, runtime_write in [
        (
            "candidate-topology-quality-prepare-result.schema.json",
            "CandidateTopologyQualityPrepareResult@1",
            True,
        ),
        (
            "candidate-topology-quality-get-result.schema.json",
            "CandidateTopologyQualityGetResult@1",
            False,
        ),
    ]:
        result = load_schema(filename)
        result_properties = result.get("properties", {})
        require(
            result.get("type") == "object"
            and result.get("additionalProperties") is False
            and set(result.get("required", [])) == result_fields
            and set(result_properties) == result_fields
            and result_properties["schema_version"].get("const") == schema_version
            and result_properties["topology_quality"].get("$ref")
            == "https://forgecad.local/contracts/candidate-topology-quality.schema.json"
            and result_properties["runtime_write"].get("const") is runtime_write
            and result_properties["production_stage_advanced"].get("const") is False
            and result_properties["candidate_confirmed"].get("const") is False
            and result_properties["version_created"].get("const") is False
            and result_properties["export_performed"].get("const") is False,
            f"{schema_version} must not advance the production head or confirm a candidate",
        )


def check_candidate_material_surface_quality_contracts() -> None:
    """Keep the first material-surface gate dual-candidate and structural-only."""
    binding_fields = {
        "material_surface_quality_id", "project_id",
        "source_candidate_id", "source_candidate_state_sha256", "source_artifact_id",
        "source_artifact_sha256", "source_artifact_readback_sha256",
        "source_artifact_readback_object_sha256", "source_geometry_candidate_evidence_sha256",
        "source_geometry_program_sha256", "source_topology_quality_id",
        "source_topology_quality_report_object_sha256", "source_topology_quality_canonical_sha256",
        "output_candidate_id", "output_candidate_state_sha256", "output_artifact_id",
        "output_artifact_sha256", "output_artifact_readback_sha256",
        "output_artifact_readback_object_sha256", "output_geometry_program_sha256",
        "appearance_source_lineage_sidecar_object_sha256", "appearance_source_lineage_canonical_sha256",
        "appearance_program_object_sha256", "appearance_program_sha256", "material_layer_stack_sha256",
        "material_pack_manifest_object_sha256", "material_pack_manifest_sha256",
        "material_pack_provenance_sha256", "texture_build_receipt_object_sha256",
        "texture_build_receipt_canonical_sha256", "candidate_surface_bake_receipt_object_sha256",
        "candidate_surface_bake_receipt_canonical_sha256", "uv_binding_sha256",
        "tangent_binding_sha256", "material_zone_inventory_sha256", "material_provenance_sha256",
        "lod_scope", "geometry_preservation_projection_sha256", "material_surface_quality_policy",
        "material_surface_quality_policy_sha256", "from_stage", "to_stage", "input_sha256",
    }
    record_fields = {
        "schema_version", *binding_fields,
        "material_pack_id", "material_pack_version", "material_pack_license_spdx",
        "source_output_candidate_binding_status", "geometry_preservation_status", "hard_gate",
        "validator_status", "hard_gate_passed", "visual_quality_status", "artistic_quality_status",
        "human_review_status", "commercial_fps_quality_status", "commercial_engine_status",
        "materialization_status", "quality_status", "runtime_write_performed",
        "production_stage_advanced", "candidate_confirmed", "version_created", "export_performed",
        "request_sha256", "canonical_sha256", "created_at",
    }
    record = load_schema("candidate-material-surface-quality.schema.json")
    properties = record.get("properties", {})
    require(
        record.get("type") == "object"
        and record.get("additionalProperties") is False
        and set(record.get("required", [])) == record_fields
        and set(properties) == record_fields
        and properties["schema_version"].get("const") == "CandidateMaterialSurfaceQuality@1"
        and properties["lod_scope"].get("const") == "lod0-only@1"
        and properties["source_output_candidate_binding_status"].get("const")
        == "distinct-candidates-verified"
        and properties["geometry_preservation_status"].get("const")
        == "source-output-renderable-geometry-byte-exact"
        and properties["material_surface_quality_policy"].get("const")
        == "candidate-material-surface-structural-hard-gate@1"
        and properties["from_stage"].get("const") == "topology"
        and properties["to_stage"].get("const") == "material-surface",
        "CandidateMaterialSurfaceQuality@1 must be a closed LOD0 dual-candidate topology-to-material receipt",
    )
    require(
        "distinct" in record.get("description", "").lower()
        and "report_object_sha256" not in properties,
        "CandidateMaterialSurfaceQuality@1 must require distinct candidates and keep its owned report hash outside the record",
    )
    require(
        properties["material_pack_id"].get("const") == "forgecad-fictional-energy-weapon-2k"
        and properties["material_pack_version"].get("const") == "1.0.0"
        and properties["material_pack_license_spdx"].get("const") == "CC0-1.0"
        and properties["visual_quality_status"].get("const") == "NOT_PROVEN"
        and properties["artistic_quality_status"].get("const") == "NOT_PROVEN"
        and properties["human_review_status"].get("const") == "NOT_RUN"
        and properties["commercial_fps_quality_status"].get("const") == "NOT_PROVEN"
        and properties["commercial_engine_status"].get("const") == "NOT_RUN"
        and properties["quality_status"].get("const") == "structural_only"
        and properties["runtime_write_performed"].get("const") is True
        and properties["production_stage_advanced"].get("const") is False
        and properties["candidate_confirmed"].get("const") is False
        and properties["version_created"].get("const") is False
        and properties["export_performed"].get("const") is False,
        "CandidateMaterialSurfaceQuality@1 must freeze 2K provenance and retain every visual/commercial boundary",
    )
    hard_gate_fields = {
        "distinct_candidates", "source_topology_quality", "source_artifact_readback",
        "output_artifact_readback", "geometry_preserved", "appearance_source_lineage",
        "material_pack_2k", "texture_build_v2", "surface_bake_v1", "uv_integrity",
        "tangent_integrity", "material_provenance",
    }
    hard_gate = record["$defs"]["hard_gate"]
    require(
        hard_gate.get("type") == "object"
        and hard_gate.get("additionalProperties") is False
        and set(hard_gate.get("required", [])) == hard_gate_fields
        and set(hard_gate.get("properties", {})) == hard_gate_fields,
        "CandidateMaterialSurfaceQuality@1 hard gate must expose every structural predicate",
    )
    passing_properties = record["$defs"]["passing_hard_gate"]["allOf"][1]["properties"]
    require(
        set(passing_properties) == hard_gate_fields
        and all(value.get("const") is True for value in passing_properties.values()),
        "CandidateMaterialSurfaceQuality@1 passing hard gate must require every predicate",
    )

    prepare = load_schema("candidate-material-surface-quality-prepare-request.schema.json")
    prepare_fields = {"schema_version", *binding_fields, "idempotency_key"}
    prepare_properties = prepare.get("properties", {})
    require(
        prepare.get("type") == "object"
        and prepare.get("additionalProperties") is False
        and set(prepare.get("required", [])) == prepare_fields
        and set(prepare_properties) == prepare_fields
        and prepare_properties["schema_version"].get("const")
        == "CandidateMaterialSurfaceQualityPrepareRequest@1"
        and prepare_properties["lod_scope"].get("const") == "lod0-only@1"
        and prepare_properties["material_surface_quality_policy"].get("const")
        == "candidate-material-surface-structural-hard-gate@1"
        and prepare_properties["from_stage"].get("const") == "topology"
        and prepare_properties["to_stage"].get("const") == "material-surface"
        and "distinct" in prepare.get("description", "").lower(),
        "CandidateMaterialSurfaceQualityPrepareRequest@1 must be closed and explicitly dual-candidate",
    )

    get_request = load_schema("candidate-material-surface-quality-get-request.schema.json")
    get_fields = {
        "schema_version", "material_surface_quality_id", "project_id",
        "source_candidate_id", "output_candidate_id",
    }
    require(
        get_request.get("type") == "object"
        and get_request.get("additionalProperties") is False
        and set(get_request.get("required", [])) == get_fields
        and set(get_request.get("properties", {})) == get_fields
        and get_request["properties"]["schema_version"].get("const")
        == "CandidateMaterialSurfaceQualityGetRequest@1",
        "CandidateMaterialSurfaceQualityGetRequest@1 must be closed and bind both candidates",
    )

    result_fields = {
        "schema_version", "material_surface_quality", "replayed", "runtime_write",
        "production_stage_advanced", "candidate_confirmed", "version_created", "export_performed",
    }
    for filename, schema_version, runtime_write in [
        (
            "candidate-material-surface-quality-prepare-result.schema.json",
            "CandidateMaterialSurfaceQualityPrepareResult@1",
            True,
        ),
        (
            "candidate-material-surface-quality-get-result.schema.json",
            "CandidateMaterialSurfaceQualityGetResult@1",
            False,
        ),
    ]:
        result = load_schema(filename)
        result_properties = result.get("properties", {})
        require(
            result.get("type") == "object"
            and result.get("additionalProperties") is False
            and set(result.get("required", [])) == result_fields
            and set(result_properties) == result_fields
            and result_properties["schema_version"].get("const") == schema_version
            and result_properties["material_surface_quality"].get("$ref")
            == "https://forgecad.local/contracts/candidate-material-surface-quality.schema.json"
            and result_properties["runtime_write"].get("const") is runtime_write
            and result_properties["production_stage_advanced"].get("const") is False
            and result_properties["candidate_confirmed"].get("const") is False
            and result_properties["version_created"].get("const") is False
            and result_properties["export_performed"].get("const") is False,
            f"{schema_version} must not advance production, confirm, version or export",
        )


def check_candidate_animation_vfx_quality_contracts() -> None:
    """Keep the animation/VFX quality receipt bound to one material head."""
    binding_fields = {
        "schema_version", "animation_vfx_quality_id", "project_id",
        "source_material_surface_transition_id", "source_material_surface_transition_sha256",
        "source_material_surface_head_canonical_sha256", "source_material_surface_quality_id",
        "source_material_surface_quality_report_object_sha256",
        "source_material_surface_quality_canonical_sha256", "candidate_id",
        "candidate_state_sha256", "artifact_id", "artifact_sha256",
        "delivery_manifest_object_sha256", "anchor_set_object_sha256",
        "anchor_set_canonical_sha256", "animation_clip_id", "animation_clip_object_sha256",
        "animation_clip_sha256", "animated_socket_materialization_key_sha256",
        "animated_artifact_sha256", "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256", "derived_animated_socket_artifact_sha256",
        "animated_socket_receipt_object_sha256", "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256", "vfx_sequence_key_sha256",
        "vfx_sequence_canonical_sha256", "vfx_frame_key_sha256",
        "vfx_frame_canonical_sha256", "vfx_bloom_key_sha256",
        "vfx_bloom_canonical_sha256", "vfx_particle_key_sha256",
        "vfx_particle_canonical_sha256", "vfx_trail_key_sha256",
        "vfx_trail_canonical_sha256", "vfx_trail_bloom_key_sha256",
        "vfx_trail_bloom_canonical_sha256", "particle_history_key_sha256s",
        "sample_request_sha256", "camera_object_sha256", "camera_identity_sha256",
        "render_profile_sha256", "render_worker_build_cohort_sha256",
        "animation_vfx_scope", "animation_vfx_policy", "animation_vfx_policy_sha256",
        "from_stage", "to_stage", "input_sha256",
    }
    record_fields = {
        *binding_fields, "candidate_binding_status", "hard_gate", "validator_status",
        "hard_gate_passed", "animation_status", "vfx_status", "visual_quality_status",
        "artistic_quality_status", "human_review_status", "commercial_fps_quality_status",
        "commercial_engine_status", "actual_engine_roundtrip", "functional_semantics",
        "materialization_status", "quality_status", "runtime_write_performed",
        "production_stage_advanced", "candidate_confirmed", "version_created",
        "export_performed", "request_sha256", "canonical_sha256", "created_at",
    }
    record = load_schema("candidate-animation-vfx-quality.schema.json")
    properties = record.get("properties", {})
    require(
        record.get("type") == "object"
        and record.get("additionalProperties") is False
        and set(record.get("required", [])) == record_fields
        and set(properties) == record_fields
        and properties["schema_version"].get("const") == "CandidateAnimationVfxQuality@1"
        and properties["animation_vfx_scope"].get("const")
        == "lod0-rigid-animation-full-vfx-stack-single-frame@1"
        and properties["animation_vfx_policy"].get("const")
        == "candidate-animation-vfx-structural-hard-gate@1"
        and properties["from_stage"].get("const") == "material-surface"
        and properties["to_stage"].get("const") == "animation-vfx"
        and properties["candidate_binding_status"].get("const")
        == "same-material-surface-head-candidate-no-geometry-mutation"
        and "report_object_sha256" not in properties
        and properties["runtime_write_performed"].get("const") is True
        and properties["production_stage_advanced"].get("const") is False
        and properties["candidate_confirmed"].get("const") is False
        and properties["version_created"].get("const") is False
        and properties["export_performed"].get("const") is False,
        "CandidateAnimationVfxQuality@1 must be a closed same-candidate structural receipt",
    )
    require(
        properties["animation_status"].get("const") == "structural_only"
        and properties["vfx_status"].get("const") == "structural_only"
        and properties["visual_quality_status"].get("const") == "NOT_PROVEN"
        and properties["artistic_quality_status"].get("const") == "NOT_PROVEN"
        and properties["human_review_status"].get("const") == "NOT_RUN"
        and properties["commercial_fps_quality_status"].get("const") == "NOT_PROVEN"
        and properties["commercial_engine_status"].get("const") == "NOT_RUN"
        and properties["actual_engine_roundtrip"].get("const") is False
        and properties["functional_semantics"].get("const") is False
        and properties["materialization_status"].get("const")
        == "runtime-owned-durable-candidate-animation-vfx-quality"
        and properties["quality_status"].get("const") == "structural_only",
        "CandidateAnimationVfxQuality@1 must preserve structural-only and engine boundaries",
    )
    require(
        properties["particle_history_key_sha256s"].get("type") == "array"
        and properties["particle_history_key_sha256s"].get("minItems") == 1
        and properties["particle_history_key_sha256s"].get("maxItems") == 4
        and properties["particle_history_key_sha256s"].get("uniqueItems") is True,
        "CandidateAnimationVfxQuality@1 particle history must be bounded and unique",
    )
    hard_gate_fields = {
        "material_surface_head_binding", "material_surface_quality", "delivery_lod0_binding",
        "anchor_set_binding", "animation_clip_binding", "animation_glb_readback",
        "animated_socket_readback", "vfx_profile_binding", "base_frame_stack", "bloom_stack",
        "particle_stack", "trail_stack", "trail_bloom_stack", "cross_layer_parent_binding",
        "sample_camera_binding", "worker_cohort_binding", "render_pass_byte_exact",
        "bounded_resource_policy", "vfx_glb_socket_attachment", "nonfunctional_scope",
    }
    hard_gate = record["$defs"]["hard_gate"]
    require(
        hard_gate.get("type") == "object"
        and hard_gate.get("additionalProperties") is False
        and set(hard_gate.get("required", [])) == hard_gate_fields
        and set(hard_gate.get("properties", {})) == hard_gate_fields,
        "CandidateAnimationVfxQuality@1 hard gate must expose all twenty predicates",
    )
    passing_properties = record["$defs"]["passing_hard_gate"]["allOf"][1]["properties"]
    require(
        set(passing_properties) == hard_gate_fields
        and all(value.get("const") is True for value in passing_properties.values()),
        "CandidateAnimationVfxQuality@1 passing hard gate must require every predicate",
    )

    prepare = load_schema("candidate-animation-vfx-quality-prepare-request.schema.json")
    prepare_fields = {*binding_fields, "idempotency_key"}
    prepare_properties = prepare.get("properties", {})
    require(
        prepare.get("type") == "object"
        and prepare.get("additionalProperties") is False
        and set(prepare.get("required", [])) == prepare_fields
        and set(prepare_properties) == prepare_fields
        and prepare_properties["schema_version"].get("const")
        == "CandidateAnimationVfxQualityPrepareRequest@1"
        and prepare_properties["animation_vfx_scope"].get("const")
        == "lod0-rigid-animation-full-vfx-stack-single-frame@1"
        and prepare_properties["animation_vfx_policy"].get("const")
        == "candidate-animation-vfx-structural-hard-gate@1"
        and prepare_properties["from_stage"].get("const") == "material-surface"
        and prepare_properties["to_stage"].get("const") == "animation-vfx",
        "CandidateAnimationVfxQualityPrepareRequest@1 must be closed and fully bound",
    )

    get_request = load_schema("candidate-animation-vfx-quality-get-request.schema.json")
    get_fields = {"schema_version", "animation_vfx_quality_id", "project_id", "candidate_id"}
    require(
        get_request.get("type") == "object"
        and get_request.get("additionalProperties") is False
        and set(get_request.get("required", [])) == get_fields
        and set(get_request.get("properties", {})) == get_fields
        and get_request["properties"]["schema_version"].get("const")
        == "CandidateAnimationVfxQualityGetRequest@1",
        "CandidateAnimationVfxQualityGetRequest@1 must bind schema/id/project/candidate",
    )

    result_fields = {
        "schema_version", "animation_vfx_quality", "replayed", "runtime_write",
        "production_stage_advanced", "candidate_confirmed", "version_created", "export_performed",
    }
    for filename, schema_version, runtime_write in [
        ("candidate-animation-vfx-quality-prepare-result.schema.json", "CandidateAnimationVfxQualityPrepareResult@1", True),
        ("candidate-animation-vfx-quality-get-result.schema.json", "CandidateAnimationVfxQualityGetResult@1", False),
    ]:
        result = load_schema(filename)
        result_properties = result.get("properties", {})
        require(
            result.get("type") == "object"
            and result.get("additionalProperties") is False
            and set(result.get("required", [])) == result_fields
            and set(result_properties) == result_fields
            and result_properties["schema_version"].get("const") == schema_version
            and result_properties["animation_vfx_quality"].get("$ref")
            == "https://forgecad.local/contracts/candidate-animation-vfx-quality.schema.json"
            and result_properties["runtime_write"].get("const") is runtime_write
            and result_properties["production_stage_advanced"].get("const") is False
            and result_properties["candidate_confirmed"].get("const") is False
            and result_properties["version_created"].get("const") is False
            and result_properties["export_performed"].get("const") is False,
            f"{schema_version} must not advance production, confirm, version or export",
        )


def check_candidate_animation_vfx_quality_v2_contracts() -> None:
    """Keep Quality@2 bound to the durable Attachment@3 full frame set."""
    expected = {
        "candidate-animation-vfx-quality-v2.schema.json": "CandidateAnimationVfxQuality@2",
        "candidate-animation-vfx-quality-v2-prepare-request.schema.json": "CandidateAnimationVfxQualityPrepareRequest@2",
        "candidate-animation-vfx-quality-v2-prepare-result.schema.json": "CandidateAnimationVfxQualityPrepareResult@2",
        "candidate-animation-vfx-quality-v2-get-request.schema.json": "CandidateAnimationVfxQualityGetRequest@2",
        "candidate-animation-vfx-quality-v2-get-result.schema.json": "CandidateAnimationVfxQualityGetResult@2",
    }
    actual = {
        path.name
        for path in SCHEMA_ROOT.glob("candidate-animation-vfx-quality-v2*.schema.json")
    }
    require(actual == set(expected), "CandidateAnimationVfxQuality@2 schema set must contain exactly five closed contracts")
    for filename, version in expected.items():
        schema = load_schema(filename)
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and schema.get("title") == version
            and schema.get("properties", {}).get("schema_version", {}).get("const") == version
            and set(schema.get("required", [])) == set(schema.get("properties", {})),
            f"{version} must remain a closed exact-field object contract",
        )

    binding_fields = {
        "schema_version", "animation_vfx_quality_id", "project_id",
        "source_material_surface_transition_id", "source_material_surface_transition_sha256",
        "source_material_surface_head_canonical_sha256", "source_material_surface_quality_id",
        "source_material_surface_quality_report_object_sha256",
        "source_material_surface_quality_canonical_sha256", "candidate_id",
        "geometry_candidate_id", "geometry_candidate_state_sha256",
        "geometry_delivery_manifest_object_sha256", "geometry_artifact_sha256",
        "appearance_candidate_id", "appearance_candidate_state_sha256",
        "appearance_delivery_manifest_object_sha256", "appearance_artifact_sha256",
        "geometry_preservation_projection_sha256", "geometry_preservation_status",
        "animated_socket_materialization_key_sha256", "animated_artifact_sha256",
        "animated_socket_anchor_set_object_sha256", "animated_socket_anchor_set_canonical_sha256",
        "appearance_anchor_set_object_sha256", "appearance_anchor_set_canonical_sha256",
        "anchor_binding_policy", "anchor_binding_sha256", "animation_clip_id",
        "animation_clip_object_sha256", "animation_clip_canonical_sha256",
        "animation_receipt_object_sha256", "animation_receipt_canonical_sha256",
        "projection_key_sha256", "projection_object_sha256", "projection_canonical_sha256",
        "particle_sequence_key_sha256", "particle_sequence_canonical_sha256",
        "trail_sequence_key_sha256", "trail_sequence_canonical_sha256",
        "trail_bloom_sequence_key_sha256", "trail_bloom_sequence_canonical_sha256",
        "vfx_profile_object_sha256", "vfx_profile_canonical_sha256", "trail_bloom_profile_sha256",
        "socket_node_id_encoding_sha256", "socket_roles_sha256", "camera_object_sha256",
        "camera_identity_sha256", "render_profile_sha256", "render_worker_build_cohort_sha256",
        "sample_schedule_sha256", "sample_count", "sample_time_ticks", "attachment_policy",
        "frame_scope", "attachment_key_sha256",
        "attachment_canonical_sha256", "attachment_receipt_object_sha256",
        "attachment_receipt_canonical_sha256", "attachment_frame_count",
        "attachment_frame_set_sha256",
        "animation_vfx_scope", "animation_vfx_policy", "animation_vfx_policy_sha256",
        "from_stage", "to_stage", "input_sha256",
    }
    record_fields = {
        *binding_fields, "candidate_binding_status", "hard_gate", "validator_status",
        "hard_gate_passed", "animation_status", "vfx_status", "visual_quality_status",
        "artistic_quality_status", "human_review_status", "commercial_fps_quality_status",
        "commercial_engine_status", "actual_engine_roundtrip", "functional_semantics",
        "materialization_status", "quality_status", "runtime_write_performed",
        "production_stage_advanced", "candidate_confirmed", "version_created",
        "export_performed", "request_sha256", "canonical_sha256", "created_at",
    }
    record = load_schema("candidate-animation-vfx-quality-v2.schema.json")
    properties = record.get("properties", {})
    require(
        set(record.get("required", [])) == record_fields
        and set(properties) == record_fields
        and properties["schema_version"].get("const") == "CandidateAnimationVfxQuality@2"
        and properties["animation_vfx_scope"].get("const") == CANDIDATE_ANIMATION_VFX_QUALITY_V2_SCOPE
        and properties["animation_vfx_policy"].get("const") == CANDIDATE_ANIMATION_VFX_QUALITY_V2_POLICY
        and properties["from_stage"].get("const") == "material-surface"
        and properties["to_stage"].get("const") == "animation-vfx"
        and properties["candidate_binding_status"].get("const") == CANDIDATE_ANIMATION_VFX_QUALITY_V2_BINDING_STATUS
        and properties["attachment_frame_count"].get("const") == 15
        and properties["attachment_frame_set_sha256"].get("$ref") == "#/$defs/sha256"
        and properties["attachment_frame_set_sha256"].get("x-forgecad-frame-set-digest") == {
            "schema_version": CANDIDATE_ANIMATION_VFX_QUALITY_V2_FRAME_SET_SCHEMA,
            "frame_count": 15,
            "ordered_by": "frame_index-ascending-0..14",
            "fields": ["frame_index", "canonical_sha256"],
        }
        and "animation_vfx_quality_report_object_sha256" not in properties
        and properties["runtime_write_performed"].get("const") is True
        and properties["production_stage_advanced"].get("const") is False
        and properties["candidate_confirmed"].get("const") is False
        and properties["version_created"].get("const") is False
        and properties["export_performed"].get("const") is False,
        "CandidateAnimationVfxQuality@2 must be a closed non-promoting full Attachment@3 receipt without an owned report hash",
    )
    require(
        properties["animation_status"].get("const") == "structural_only"
        and properties["vfx_status"].get("const") == "structural_only"
        and properties["visual_quality_status"].get("const") == "NOT_PROVEN"
        and properties["artistic_quality_status"].get("const") == "NOT_PROVEN"
        and properties["human_review_status"].get("const") == "NOT_RUN"
        and properties["commercial_fps_quality_status"].get("const") == "NOT_PROVEN"
        and properties["commercial_engine_status"].get("const") == "NOT_RUN"
        and properties["actual_engine_roundtrip"].get("const") is False
        and properties["functional_semantics"].get("const") is False
        and properties["materialization_status"].get("const")
        == "runtime-owned-durable-candidate-animation-vfx-quality-v2"
        and properties["quality_status"].get("const") == "structural_only",
        "CandidateAnimationVfxQuality@2 must preserve structural-only and engine boundaries",
    )
    require(
        properties["sample_count"].get("const") == 15
        and properties["sample_time_ticks"].get("minItems") == 15
        and properties["sample_time_ticks"].get("maxItems") == 15
        and properties["sample_time_ticks"].get("uniqueItems") is True
        and properties["attachment_policy"].get("const")
        == "projection-v2-particles-v2-trails-v2-trails-bloom-v2-animated-socket-attachment-bridge@3"
        and properties["frame_scope"].get("const")
        == "lod0-animation-attachment-v3-source-frames-1-15-with-trails-bloom-v2-frames-0-14@3"
        and properties["geometry_preservation_status"].get("const")
        == "source-output-renderable-geometry-byte-exact"
        and properties["anchor_binding_policy"].get("const")
        == "geometry-appearance-anchor-role-owner-trs-equivalent@1",
        "CandidateAnimationVfxQuality@2 must retain the exact Attachment@3 parent schedule and binding policies",
    )

    hard_gate_fields = {
        "material_surface_head_binding", "material_surface_quality", "delivery_lod0_binding",
        "anchor_set_binding", "animation_clip_binding", "animation_glb_readback",
        "animated_socket_readback", "vfx_profile_binding", "base_frame_stack", "bloom_stack",
        "particle_stack", "trail_stack", "trail_bloom_stack", "cross_layer_parent_binding",
        "sample_camera_binding", "worker_cohort_binding", "render_pass_byte_exact",
        "bounded_resource_policy", "vfx_glb_socket_attachment", "nonfunctional_scope",
    }
    hard_gate = record["$defs"]["hard_gate"]
    hard_gate_properties = hard_gate.get("properties", {})
    require(
        hard_gate.get("type") == "object"
        and hard_gate.get("additionalProperties") is False
        and set(hard_gate.get("required", [])) == hard_gate_fields
        and set(hard_gate_properties) == hard_gate_fields
        and hard_gate_properties["vfx_glb_socket_attachment"].get("x-forgecad-derived-from") == {
            "schema_version": "FictionalEnergyVfxAnimatedSocketAttachment@3",
            "source": "durable-exact-get",
            "frame_count": 15,
            "frame_set_field": "frames[].canonical_sha256",
            "frame_set_digest_schema": CANDIDATE_ANIMATION_VFX_QUALITY_V2_FRAME_SET_SCHEMA,
            "frame_set_order": "frame_index-ascending-0..14",
        }
        and "legacy sidecar boolean is not a valid source"
        in hard_gate_properties["vfx_glb_socket_attachment"].get("description", ""),
        "CandidateAnimationVfxQuality@2 hard gate must derive socket attachment only from exact Attachment@3 full-frame binding",
    )
    passing_properties = record["$defs"]["passing_hard_gate"]["allOf"][1]["properties"]
    require(
        set(passing_properties) == hard_gate_fields
        and all(value.get("const") is True for value in passing_properties.values()),
        "CandidateAnimationVfxQuality@2 passing hard gate must require every predicate",
    )

    prepare = load_schema("candidate-animation-vfx-quality-v2-prepare-request.schema.json")
    prepare_fields = {*binding_fields, "idempotency_key"}
    prepare_properties = prepare.get("properties", {})
    require(
        set(prepare.get("required", [])) == prepare_fields
        and set(prepare_properties) == prepare_fields
        and prepare_properties["schema_version"].get("const")
        == "CandidateAnimationVfxQualityPrepareRequest@2"
        and prepare_properties["animation_vfx_scope"].get("const")
        == CANDIDATE_ANIMATION_VFX_QUALITY_V2_SCOPE
        and prepare_properties["animation_vfx_policy"].get("const")
        == CANDIDATE_ANIMATION_VFX_QUALITY_V2_POLICY
        and prepare_properties["from_stage"].get("const") == "material-surface"
        and prepare_properties["to_stage"].get("const") == "animation-vfx"
        and prepare_properties["attachment_frame_count"].get("const") == 15
        and prepare_properties["attachment_frame_set_sha256"].get("x-forgecad-frame-set-digest") == {
            "schema_version": CANDIDATE_ANIMATION_VFX_QUALITY_V2_FRAME_SET_SCHEMA,
            "frame_count": 15,
            "ordered_by": "frame_index-ascending-0..14",
            "fields": ["frame_index", "canonical_sha256"],
        },
        "CandidateAnimationVfxQualityPrepareRequest@2 must be closed and fully bound to the full Attachment@3 frame set",
    )

    get_request = load_schema("candidate-animation-vfx-quality-v2-get-request.schema.json")
    get_fields = {"schema_version", "animation_vfx_quality_id", "project_id", "candidate_id"}
    require(
        set(get_request.get("required", [])) == get_fields
        and set(get_request.get("properties", {})) == get_fields
        and get_request["properties"]["schema_version"].get("const")
        == "CandidateAnimationVfxQualityGetRequest@2",
        "CandidateAnimationVfxQualityGetRequest@2 must bind schema/id/project/candidate",
    )

    result_fields = {
        "schema_version", "animation_vfx_quality", "replayed", "runtime_write",
        "production_stage_advanced", "candidate_confirmed", "version_created", "export_performed",
    }
    for filename, schema_version, runtime_write in [
        (
            "candidate-animation-vfx-quality-v2-prepare-result.schema.json",
            "CandidateAnimationVfxQualityPrepareResult@2",
            True,
        ),
        (
            "candidate-animation-vfx-quality-v2-get-result.schema.json",
            "CandidateAnimationVfxQualityGetResult@2",
            False,
        ),
    ]:
        result = load_schema(filename)
        result_properties = result.get("properties", {})
        require(
            result.get("type") == "object"
            and result.get("additionalProperties") is False
            and set(result.get("required", [])) == result_fields
            and set(result_properties) == result_fields
            and result_properties["schema_version"].get("const") == schema_version
            and result_properties["animation_vfx_quality"].get("$ref")
            == "https://forgecad.local/contracts/candidate-animation-vfx-quality-v2.schema.json"
            and result_properties["runtime_write"].get("const") is runtime_write
            and result_properties["production_stage_advanced"].get("const") is False
            and result_properties["candidate_confirmed"].get("const") is False
            and result_properties["version_created"].get("const") is False
            and result_properties["export_performed"].get("const") is False,
            f"{schema_version} must not advance production, confirm, version or export",
        )


def check_fictional_energy_vfx_animated_socket_attachment_contracts() -> None:
    """Keep animated socket attachment frame evidence bounded and closed."""
    expected = {
        "fictional-energy-vfx-animated-socket-attachment.schema.json": "FictionalEnergyVfxAnimatedSocketAttachment@1",
        "fictional-energy-vfx-animated-socket-attachment-prepare-request.schema.json": "FictionalEnergyVfxAnimatedSocketAttachmentPrepareRequest@1",
        "fictional-energy-vfx-animated-socket-attachment-prepare-result.schema.json": "FictionalEnergyVfxAnimatedSocketAttachmentPrepareResult@1",
        "fictional-energy-vfx-animated-socket-attachment-get-request.schema.json": "FictionalEnergyVfxAnimatedSocketAttachmentGetRequest@1",
        "fictional-energy-vfx-animated-socket-attachment-get-result.schema.json": "FictionalEnergyVfxAnimatedSocketAttachmentGetResult@1",
        "fictional-energy-vfx-animated-socket-attachment-v2.schema.json": "FictionalEnergyVfxAnimatedSocketAttachment@2",
        "fictional-energy-vfx-animated-socket-attachment-v2-prepare-request.schema.json": "FictionalEnergyVfxAnimatedSocketAttachmentPrepareRequest@2",
        "fictional-energy-vfx-animated-socket-attachment-v2-prepare-result.schema.json": "FictionalEnergyVfxAnimatedSocketAttachmentPrepareResult@2",
        "fictional-energy-vfx-animated-socket-attachment-v2-get-request.schema.json": "FictionalEnergyVfxAnimatedSocketAttachmentGetRequest@2",
        "fictional-energy-vfx-animated-socket-attachment-v2-get-result.schema.json": "FictionalEnergyVfxAnimatedSocketAttachmentGetResult@2",
        "fictional-energy-vfx-animated-socket-attachment-v3.schema.json": "FictionalEnergyVfxAnimatedSocketAttachment@3",
        "fictional-energy-vfx-animated-socket-attachment-v3-prepare-request.schema.json": "FictionalEnergyVfxAnimatedSocketAttachmentPrepareRequest@3",
        "fictional-energy-vfx-animated-socket-attachment-v3-prepare-result.schema.json": "FictionalEnergyVfxAnimatedSocketAttachmentPrepareResult@3",
        "fictional-energy-vfx-animated-socket-attachment-v3-get-request.schema.json": "FictionalEnergyVfxAnimatedSocketAttachmentGetRequest@3",
        "fictional-energy-vfx-animated-socket-attachment-v3-get-result.schema.json": "FictionalEnergyVfxAnimatedSocketAttachmentGetResult@3",
    }
    actual = {
        path.name
        for path in SCHEMA_ROOT.glob("fictional-energy-vfx-animated-socket-attachment*.schema.json")
    }
    require(actual == set(expected), "animated socket attachment schema set must contain exactly five immutable V1, five projection-bound V2 and five terminal bridge V3 contracts")
    for filename, version in expected.items():
        schema = load_schema(filename)
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and schema.get("title") == version
            and schema.get("properties", {}).get("schema_version", {}).get("const") == version
            and set(schema.get("required", [])) == set(schema.get("properties", {})),
            f"{version} must remain a closed exact-field object contract",
        )

    parent_fields = {
        "schema_version", "attachment_key_sha256", "project_id",
        "delivery_manifest_object_sha256", "candidate_id", "candidate_state_sha256",
        "source_artifact_sha256", "animated_socket_materialization_key_sha256",
        "animated_socket_anchor_set_object_sha256", "animated_socket_anchor_set_canonical_sha256",
        "animation_clip_id", "animation_clip_object_sha256", "animation_clip_canonical_sha256",
        "animated_artifact_sha256", "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256", "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256", "vfx_sequence_key_sha256",
        "vfx_sequence_canonical_sha256", "attachment_policy",
        "socket_node_id_encoding_sha256", "socket_roles_sha256", "frame_scope",
        "frames", "attachment_status", "canonical_sha256", "created_at",
    }
    frame_fields = {
        "schema_version", "attachment_key_sha256", "frame_index", "sample_time_ticks",
        "animation_pose_readback_sha256", "socket_transform_inventory_sha256",
        "socket_transform_readback_sha256", "emitter_socket_bindings_sha256",
        "trail_socket_bindings_sha256", "base_frame_key_sha256", "bloom_key_sha256",
        "particle_key_sha256", "trail_key_sha256", "trail_bloom_key_sha256",
        "canonical_sha256", "created_at",
    }
    attachment = load_schema("fictional-energy-vfx-animated-socket-attachment.schema.json")
    attachment_properties = attachment.get("properties", {})
    frame = attachment.get("$defs", {}).get("frame", {})
    require(
        set(attachment.get("required", [])) == parent_fields
        and set(attachment_properties) == parent_fields
        and attachment_properties["schema_version"].get("const")
        == "FictionalEnergyVfxAnimatedSocketAttachment@1"
        and attachment_properties["attachment_policy"].get("const")
        == "fictional-energy-vfx-animated-socket-attachment-structural-only@1"
        and attachment_properties["frame_scope"].get("const")
        == "lod0-animation-vfx-frame-range-1-16@1"
        and attachment_properties["attachment_status"].get("const")
        == "runtime-owned-durable-fictional-energy-vfx-animated-socket-attachment"
        and attachment_properties["frames"].get("minItems") == 1
        and attachment_properties["frames"].get("maxItems") == 16
        and frame.get("type") == "object"
        and frame.get("additionalProperties") is False
        and set(frame.get("required", [])) == frame_fields
        and set(frame.get("properties", {})) == frame_fields
        and frame["properties"]["schema_version"].get("const")
        == "FictionalEnergyVfxAnimatedSocketAttachmentFrame@1"
        and frame["properties"]["frame_index"].get("minimum") == 0
        and frame["properties"]["frame_index"].get("maximum") == 15,
        "animated socket attachment parent and frame records must use the frozen bounded field sets",
    )

    prepare = load_schema(
        "fictional-energy-vfx-animated-socket-attachment-prepare-request.schema.json"
    )
    prepare_fields = parent_fields - {"schema_version", "frames", "attachment_status", "canonical_sha256", "created_at"}
    prepare_fields |= {"schema_version", "input_sha256", "idempotency_key"}
    prepare_properties = prepare.get("properties", {})
    require(
        set(prepare.get("required", [])) == prepare_fields
        and set(prepare_properties) == prepare_fields
        and prepare_properties["schema_version"].get("const")
        == "FictionalEnergyVfxAnimatedSocketAttachmentPrepareRequest@1"
        and prepare_properties["attachment_policy"].get("const")
        == "fictional-energy-vfx-animated-socket-attachment-structural-only@1"
        and prepare_properties["frame_scope"].get("const")
        == "lod0-animation-vfx-frame-range-1-16@1",
        "FictionalEnergyVfxAnimatedSocketAttachmentPrepareRequest@1 must bind all sources and remain closed",
    )

    get_request = load_schema(
        "fictional-energy-vfx-animated-socket-attachment-get-request.schema.json"
    )
    get_fields = {"schema_version", "attachment_key_sha256", "project_id", "candidate_id"}
    require(
        set(get_request.get("required", [])) == get_fields
        and set(get_request.get("properties", {})) == get_fields
        and get_request["properties"]["schema_version"].get("const")
        == "FictionalEnergyVfxAnimatedSocketAttachmentGetRequest@1",
        "FictionalEnergyVfxAnimatedSocketAttachmentGetRequest@1 must bind exact key/project/candidate",
    )

    result_fields = {
        "schema_version", "attachment_key_sha256", "attachment", "replayed",
        "restart_hash_verified", "runtime_write", "quality_status", "visual_quality_status",
        "commercial_fps_quality_status", "human_review_status", "commercial_engine_status",
        "actual_engine_roundtrip", "production_stage_advanced", "candidate_confirmed",
        "version_created", "export_performed",
    }
    for filename, schema_version, runtime_write in [
        (
            "fictional-energy-vfx-animated-socket-attachment-prepare-result.schema.json",
            "FictionalEnergyVfxAnimatedSocketAttachmentPrepareResult@1",
            True,
        ),
        (
            "fictional-energy-vfx-animated-socket-attachment-get-result.schema.json",
            "FictionalEnergyVfxAnimatedSocketAttachmentGetResult@1",
            False,
        ),
    ]:
        result = load_schema(filename)
        properties = result.get("properties", {})
        require(
            set(result.get("required", [])) == result_fields
            and set(properties) == result_fields
            and properties["schema_version"].get("const") == schema_version
            and properties["attachment"].get("$ref")
            == "fictional-energy-vfx-animated-socket-attachment.schema.json"
            and properties["restart_hash_verified"].get("const") is True
            and properties["runtime_write"].get("const") is runtime_write
            and properties["quality_status"].get("const") == "structural_only"
            and properties["visual_quality_status"].get("const") == "NOT_PROVEN"
            and properties["commercial_fps_quality_status"].get("const") == "NOT_PROVEN"
            and properties["human_review_status"].get("const") == "NOT_RUN"
            and properties["commercial_engine_status"].get("const") == "NOT_RUN"
            and properties["actual_engine_roundtrip"].get("const") is False
            and properties["production_stage_advanced"].get("const") is False
            and properties["candidate_confirmed"].get("const") is False
            and properties["version_created"].get("const") is False
            and properties["export_performed"].get("const") is False,
            f"{schema_version} must remain restart-verified, structural-only and non-promoting",
        )

    parent_v3_fields = {
        "schema_version", "attachment_key_sha256", "project_id",
        "geometry_candidate_id", "geometry_candidate_state_sha256",
        "geometry_delivery_manifest_object_sha256", "geometry_artifact_sha256",
        "appearance_candidate_id", "appearance_candidate_state_sha256",
        "appearance_delivery_manifest_object_sha256", "appearance_artifact_sha256",
        "material_surface_quality_id", "material_surface_quality_report_object_sha256",
        "material_surface_quality_canonical_sha256", "geometry_preservation_projection_sha256",
        "geometry_preservation_status", "animated_socket_materialization_key_sha256",
        "animated_artifact_sha256", "animated_socket_anchor_set_object_sha256",
        "animated_socket_anchor_set_canonical_sha256", "appearance_anchor_set_object_sha256",
        "appearance_anchor_set_canonical_sha256", "anchor_binding_policy",
        "anchor_binding_sha256", "animation_clip_id", "animation_clip_object_sha256",
        "animation_clip_canonical_sha256", "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256", "projection_key_sha256", "projection_object_sha256",
        "projection_canonical_sha256", "particle_sequence_key_sha256",
        "particle_sequence_canonical_sha256", "trail_sequence_key_sha256",
        "trail_sequence_canonical_sha256", "trail_bloom_sequence_key_sha256",
        "trail_bloom_sequence_canonical_sha256", "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256", "trail_bloom_profile_sha256",
        "socket_node_id_encoding_sha256", "socket_roles_sha256", "camera_object_sha256",
        "camera_identity_sha256", "render_profile_sha256", "render_worker_build_cohort_sha256",
        "sample_schedule_sha256", "sample_count", "sample_time_ticks", "attachment_policy",
        "frame_scope", "attachment_receipt_object_sha256", "attachment_receipt_canonical_sha256",
        "frames", "attachment_status", "quality_status", "visual_quality_status",
        "commercial_fps_quality_status", "human_review_status", "commercial_engine_status",
        "runtime_write_performed", "restart_hash_verified", "candidate_confirmed",
        "version_created", "export_performed", "actual_engine_roundtrip",
        "production_stage_advanced", "input_sha256", "canonical_sha256", "created_at",
    }
    frame_v3_fields = {
        "schema_version", "attachment_key_sha256", "frame_index", "sample_time_ticks",
        "projection_frame_index", "particle_sequence_frame_index", "trail_frame_index",
        "trail_bloom_frame_index", "projection_frame_canonical_sha256",
        "projection_socket_transform_inventory_sha256", "projection_socket_transform_readback_sha256",
        "particle_sequence_key_sha256", "particle_sequence_frame_canonical_sha256",
        "trail_sequence_key_sha256", "trail_sequence_frame_canonical_sha256", "trail_key_sha256",
        "trail_inventory_sha256", "trail_id_encoding_sha256", "emitter_binding_sha256",
        "trail_bloom_sequence_key_sha256", "trail_bloom_sequence_frame_canonical_sha256",
        "trail_bloom_key_sha256", "trail_bloom_seed_sha256", "base_frame_key_sha256",
        "bloom_key_sha256", "camera_object_sha256", "camera_identity_sha256",
        "render_profile_sha256", "render_worker_build_cohort_sha256", "canonical_sha256", "created_at",
    }
    attachment_v3 = load_schema(
        "fictional-energy-vfx-animated-socket-attachment-v3.schema.json"
    )
    attachment_v3_properties = attachment_v3.get("properties", {})
    frame_v3 = attachment_v3.get("$defs", {}).get("frame", {})
    require(
        set(attachment_v3.get("required", [])) == parent_v3_fields
        and set(attachment_v3_properties) == parent_v3_fields
        and attachment_v3_properties["schema_version"].get("const")
        == "FictionalEnergyVfxAnimatedSocketAttachment@3"
        and attachment_v3_properties["attachment_policy"].get("const")
        == "projection-v2-particles-v2-trails-v2-trails-bloom-v2-animated-socket-attachment-bridge@3"
        and attachment_v3_properties["frame_scope"].get("const")
        == "lod0-animation-attachment-v3-source-frames-1-15-with-trails-bloom-v2-frames-0-14@3"
        and attachment_v3_properties["attachment_status"].get("const")
        == "runtime-owned-durable-fictional-energy-vfx-animated-socket-attachment-v3"
        and attachment_v3_properties["geometry_preservation_status"].get("const")
        == "source-output-renderable-geometry-byte-exact"
        and attachment_v3_properties["anchor_binding_policy"].get("const")
        == "geometry-appearance-anchor-role-owner-trs-equivalent@1"
        and attachment_v3_properties["sample_count"].get("const") == 15
        and attachment_v3_properties["sample_time_ticks"].get("minItems") == 15
        and attachment_v3_properties["sample_time_ticks"].get("maxItems") == 15
        and attachment_v3_properties["sample_time_ticks"].get("uniqueItems") is True
        and attachment_v3_properties["frames"].get("minItems") == 15
        and attachment_v3_properties["frames"].get("maxItems") == 15
        and frame_v3.get("type") == "object"
        and frame_v3.get("additionalProperties") is False
        and set(frame_v3.get("required", [])) == frame_v3_fields
        and set(frame_v3.get("properties", {})) == frame_v3_fields
        and frame_v3["properties"]["schema_version"].get("const")
        == "FictionalEnergyVfxAnimatedSocketAttachmentFrame@3"
        and frame_v3["properties"]["frame_index"].get("minimum") == 0
        and frame_v3["properties"]["frame_index"].get("maximum") == 14
        and frame_v3["properties"]["projection_frame_index"].get("minimum") == 1
        and frame_v3["properties"]["projection_frame_index"].get("maximum") == 15
        and frame_v3["properties"]["particle_sequence_frame_index"].get("minimum") == 1
        and frame_v3["properties"]["particle_sequence_frame_index"].get("maximum") == 15
        and frame_v3["properties"]["trail_frame_index"].get("minimum") == 0
        and frame_v3["properties"]["trail_frame_index"].get("maximum") == 14
        and frame_v3["properties"]["trail_bloom_frame_index"].get("minimum") == 0
        and frame_v3["properties"]["trail_bloom_frame_index"].get("maximum") == 14,
        "Attachment@3 must bind the complete V2 projection/particles/trails/trails-bloom stack with exactly fifteen mapped frames",
    )

    prepare_v3 = load_schema(
        "fictional-energy-vfx-animated-socket-attachment-v3-prepare-request.schema.json"
    )
    prepare_v3_fields = parent_v3_fields - {
        "schema_version", "attachment_receipt_object_sha256",
        "attachment_receipt_canonical_sha256", "frames", "attachment_status",
        "anchor_binding_sha256",
        "quality_status", "visual_quality_status", "commercial_fps_quality_status",
        "human_review_status", "commercial_engine_status", "runtime_write_performed",
        "restart_hash_verified", "candidate_confirmed", "version_created", "export_performed",
        "actual_engine_roundtrip", "production_stage_advanced", "canonical_sha256", "created_at",
    }
    prepare_v3_fields |= {"schema_version", "input_sha256", "idempotency_key"}
    prepare_v3_properties = prepare_v3.get("properties", {})
    require(
        set(prepare_v3.get("required", [])) == prepare_v3_fields
        and set(prepare_v3_properties) == prepare_v3_fields
        and prepare_v3_properties["schema_version"].get("const")
        == "FictionalEnergyVfxAnimatedSocketAttachmentPrepareRequest@3"
        and prepare_v3_properties["attachment_policy"].get("const")
        == "projection-v2-particles-v2-trails-v2-trails-bloom-v2-animated-socket-attachment-bridge@3"
        and prepare_v3_properties["frame_scope"].get("const")
        == "lod0-animation-attachment-v3-source-frames-1-15-with-trails-bloom-v2-frames-0-14@3"
        and prepare_v3_properties["sample_count"].get("const") == 15,
        "AttachmentPrepareRequest@3 must be closed, dual-candidate and receipt-derived",
    )

    get_v3 = load_schema(
        "fictional-energy-vfx-animated-socket-attachment-v3-get-request.schema.json"
    )
    get_v3_fields = {
        "schema_version", "attachment_key_sha256", "project_id", "geometry_candidate_id",
        "appearance_candidate_id", "geometry_delivery_manifest_object_sha256",
        "appearance_delivery_manifest_object_sha256",
    }
    require(
        set(get_v3.get("required", [])) == get_v3_fields
        and set(get_v3.get("properties", {})) == get_v3_fields
        and get_v3["properties"]["schema_version"].get("const")
        == "FictionalEnergyVfxAnimatedSocketAttachmentGetRequest@3",
        "AttachmentGetRequest@3 must bind both candidate and delivery lineages",
    )

    for filename, schema_version, runtime_write in [
        (
            "fictional-energy-vfx-animated-socket-attachment-v3-prepare-result.schema.json",
            "FictionalEnergyVfxAnimatedSocketAttachmentPrepareResult@3",
            True,
        ),
        (
            "fictional-energy-vfx-animated-socket-attachment-v3-get-result.schema.json",
            "FictionalEnergyVfxAnimatedSocketAttachmentGetResult@3",
            False,
        ),
    ]:
        result = load_schema(filename)
        properties = result.get("properties", {})
        require(
            set(result.get("required", [])) == result_fields
            and set(properties) == result_fields
            and properties["schema_version"].get("const") == schema_version
            and properties["attachment"].get("$ref")
            == "fictional-energy-vfx-animated-socket-attachment-v3.schema.json"
            and properties["restart_hash_verified"].get("const") is True
            and properties["runtime_write"].get("const") is runtime_write
            and properties["quality_status"].get("const") == "structural_only"
            and properties["visual_quality_status"].get("const") == "NOT_PROVEN"
            and properties["commercial_fps_quality_status"].get("const") == "NOT_PROVEN"
            and properties["human_review_status"].get("const") == "NOT_RUN"
            and properties["commercial_engine_status"].get("const") == "NOT_RUN"
            and properties["actual_engine_roundtrip"].get("const") is False
            and properties["production_stage_advanced"].get("const") is False
            and properties["candidate_confirmed"].get("const") is False
            and properties["version_created"].get("const") is False
            and properties["export_performed"].get("const") is False,
            f"{schema_version} must remain restart-verified, structural-only and non-promoting",
        )

    parent_v2_fields = {
        "schema_version", "attachment_key_sha256", "project_id",
        "delivery_manifest_object_sha256", "candidate_id", "candidate_state_sha256",
        "source_artifact_sha256", "animated_socket_materialization_key_sha256",
        "animated_socket_anchor_set_object_sha256", "animated_socket_anchor_set_canonical_sha256",
        "animation_clip_id", "animation_clip_object_sha256", "animation_clip_canonical_sha256",
        "animated_artifact_sha256", "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256", "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256", "projection_key_sha256", "projection_object_sha256",
        "projection_canonical_sha256", "particle_sequence_key_sha256",
        "particle_sequence_canonical_sha256", "trail_sequence_key_sha256",
        "trail_sequence_canonical_sha256", "trail_bloom_sequence_key_sha256",
        "trail_bloom_sequence_canonical_sha256", "attachment_policy",
        "socket_node_id_encoding_sha256", "socket_roles_sha256", "frame_scope",
        "frames", "attachment_status", "canonical_sha256", "created_at",
    }
    frame_v2_fields = frame_fields | {
        "projection_frame_index",
        "particle_sequence_frame_index",
        "projection_frame_canonical_sha256",
        "particle_sequence_frame_canonical_sha256",
        "trail_sequence_frame_canonical_sha256",
        "trail_bloom_sequence_frame_canonical_sha256",
    }
    attachment_v2 = load_schema(
        "fictional-energy-vfx-animated-socket-attachment-v2.schema.json"
    )
    attachment_v2_properties = attachment_v2.get("properties", {})
    frame_v2 = attachment_v2.get("$defs", {}).get("frame", {})
    require(
        set(attachment_v2.get("required", [])) == parent_v2_fields
        and set(attachment_v2_properties) == parent_v2_fields
        and attachment_v2_properties["schema_version"].get("const")
        == "FictionalEnergyVfxAnimatedSocketAttachment@2"
        and attachment_v2_properties["attachment_policy"].get("const")
        == "fictional-energy-vfx-animated-socket-attachment-projection-bound@2"
        and attachment_v2_properties["frame_scope"].get("const")
        == "lod0-animation-vfx-trail-frame-range-1-15@2"
        and attachment_v2_properties["attachment_status"].get("const")
        == "runtime-owned-durable-fictional-energy-vfx-animated-socket-attachment-v2"
        and attachment_v2_properties["frames"].get("minItems") == 1
        and attachment_v2_properties["frames"].get("maxItems") == 15
        and frame_v2.get("type") == "object"
        and frame_v2.get("additionalProperties") is False
        and set(frame_v2.get("required", [])) == frame_v2_fields
        and set(frame_v2.get("properties", {})) == frame_v2_fields
        and frame_v2["properties"]["schema_version"].get("const")
        == "FictionalEnergyVfxAnimatedSocketAttachmentFrame@2"
        and frame_v2["properties"]["frame_index"].get("minimum") == 0
        and frame_v2["properties"]["frame_index"].get("maximum") == 14
        and frame_v2["properties"]["projection_frame_index"].get("minimum") == 1
        and frame_v2["properties"]["projection_frame_index"].get("maximum") == 15
        and frame_v2["properties"]["particle_sequence_frame_index"].get("minimum") == 1
        and frame_v2["properties"]["particle_sequence_frame_index"].get("maximum") == 15,
        "Attachment@2 must explicitly bind the projection and all three animated VFX sequence layers",
    )

    prepare_v2 = load_schema(
        "fictional-energy-vfx-animated-socket-attachment-v2-prepare-request.schema.json"
    )
    prepare_v2_fields = parent_v2_fields - {
        "schema_version", "frames", "attachment_status", "canonical_sha256", "created_at"
    }
    prepare_v2_fields |= {"schema_version", "input_sha256", "idempotency_key"}
    prepare_v2_properties = prepare_v2.get("properties", {})
    require(
        set(prepare_v2.get("required", [])) == prepare_v2_fields
        and set(prepare_v2_properties) == prepare_v2_fields
        and prepare_v2_properties["schema_version"].get("const")
        == "FictionalEnergyVfxAnimatedSocketAttachmentPrepareRequest@2"
        and prepare_v2_properties["attachment_policy"].get("const")
        == "fictional-energy-vfx-animated-socket-attachment-projection-bound@2"
        and prepare_v2_properties["frame_scope"].get("const")
        == "lod0-animation-vfx-trail-frame-range-1-15@2",
        "AttachmentPrepareRequest@2 must explicitly bind projection and animated sequence keys",
    )

    get_v2 = load_schema(
        "fictional-energy-vfx-animated-socket-attachment-v2-get-request.schema.json"
    )
    require(
        set(get_v2.get("required", [])) == get_fields
        and set(get_v2.get("properties", {})) == get_fields
        and get_v2["properties"]["schema_version"].get("const")
        == "FictionalEnergyVfxAnimatedSocketAttachmentGetRequest@2",
        "AttachmentGetRequest@2 must bind exact key/project/candidate",
    )

    for filename, schema_version, runtime_write in [
        (
            "fictional-energy-vfx-animated-socket-attachment-v2-prepare-result.schema.json",
            "FictionalEnergyVfxAnimatedSocketAttachmentPrepareResult@2",
            True,
        ),
        (
            "fictional-energy-vfx-animated-socket-attachment-v2-get-result.schema.json",
            "FictionalEnergyVfxAnimatedSocketAttachmentGetResult@2",
            False,
        ),
    ]:
        result = load_schema(filename)
        properties = result.get("properties", {})
        require(
            set(result.get("required", [])) == result_fields
            and set(properties) == result_fields
            and properties["schema_version"].get("const") == schema_version
            and properties["attachment"].get("$ref")
            == "fictional-energy-vfx-animated-socket-attachment-v2.schema.json"
            and properties["restart_hash_verified"].get("const") is True
            and properties["runtime_write"].get("const") is runtime_write
            and properties["quality_status"].get("const") == "structural_only"
            and properties["visual_quality_status"].get("const") == "NOT_PROVEN"
            and properties["commercial_fps_quality_status"].get("const") == "NOT_PROVEN"
            and properties["human_review_status"].get("const") == "NOT_RUN"
            and properties["commercial_engine_status"].get("const") == "NOT_RUN"
            and properties["actual_engine_roundtrip"].get("const") is False
            and properties["production_stage_advanced"].get("const") is False
            and properties["candidate_confirmed"].get("const") is False
            and properties["version_created"].get("const") is False
            and properties["export_performed"].get("const") is False,
            f"{schema_version} must remain restart-verified, structural-only and non-promoting",
        )


def check_game_weapon_animated_glb_socket_transform_projection_contracts() -> None:
    """Keep the independent six-socket animated GLB projection closed and bounded."""
    expected = {
        "game-weapon-animated-glb-socket-transform-projection.schema.json": "GameWeaponAnimatedGlbSocketTransformProjection@1",
        "game-weapon-animated-glb-socket-transform-projection-prepare-request.schema.json": "GameWeaponAnimatedGlbSocketTransformProjectionPrepareRequest@1",
        "game-weapon-animated-glb-socket-transform-projection-prepare-result.schema.json": "GameWeaponAnimatedGlbSocketTransformProjectionPrepareResult@1",
        "game-weapon-animated-glb-socket-transform-projection-get-request.schema.json": "GameWeaponAnimatedGlbSocketTransformProjectionGetRequest@1",
        "game-weapon-animated-glb-socket-transform-projection-get-result.schema.json": "GameWeaponAnimatedGlbSocketTransformProjectionGetResult@1",
    }
    actual = {
        path.name
        for path in SCHEMA_ROOT.glob("game-weapon-animated-glb-socket-transform-projection*.schema.json")
        if "-v2" not in path.name
    }
    require(actual == set(expected), "animated GLB socket transform projection must contain exactly five V1 contracts")
    for filename, version in expected.items():
        schema = load_schema(filename)
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and schema.get("title") == version
            and schema.get("properties", {}).get("schema_version", {}).get("const") == version
            and set(schema.get("required", [])) == set(schema.get("properties", {})),
            f"{version} must be a closed exact-field object contract",
        )

    projection_fields = {
        "schema_version", "projection_key_sha256", "project_id", "candidate_id",
        "candidate_state_sha256", "delivery_manifest_object_sha256", "source_artifact_sha256",
        "source_artifact_readback_sha256", "animated_artifact_sha256", "animated_artifact_readback_sha256",
        "animation_receipt_object_sha256", "animation_receipt_canonical_sha256",
        "animated_socket_materialization_key_sha256", "derived_animated_socket_artifact_sha256",
        "derived_animated_socket_artifact_readback_sha256", "derived_animated_socket_receipt_object_sha256",
        "derived_animated_socket_receipt_canonical_sha256", "anchor_set_object_sha256",
        "anchor_set_canonical_sha256", "animation_clip_id", "animation_clip_object_sha256",
        "animation_clip_canonical_sha256", "socket_node_id_encoding_sha256", "socket_node_inventory_sha256",
        "socket_roles_sha256", "socket_roles", "part_hierarchy_sha256", "part_hierarchy_policy",
        "transform_representation_policy", "sample_schedule_sha256", "sample_count", "sample_time_ticks",
        "frame_scope", "timebase_hz", "transform_projection_policy", "coordinate_system",
        "transform_convention", "float_quantization_policy", "input_sha256", "frames",
        "projection_status", "quality_status", "visual_quality_status", "commercial_fps_quality_status",
        "human_review_status", "commercial_engine_status", "runtime_write_performed",
        "restart_hash_verified", "candidate_confirmed", "version_created", "export_performed",
        "actual_engine_roundtrip", "production_stage_advanced", "limitations", "canonical_sha256",
        "created_at",
    }
    projection = load_schema("game-weapon-animated-glb-socket-transform-projection.schema.json")
    properties = projection.get("properties", {})
    require(
        set(projection.get("required", [])) == projection_fields
        and set(properties) == projection_fields
        and properties["schema_version"].get("const")
        == "GameWeaponAnimatedGlbSocketTransformProjection@1"
        and properties["projection_status"].get("const")
        == "runtime-owned-durable-game-weapon-animated-glb-socket-transform-projection"
        and properties["part_hierarchy_policy"].get("const")
        == "flat-identity-rest-part-hierarchy-only@1"
        and properties["transform_representation_policy"].get("const")
        == "trs-quaternion-no-matrix-no-shear@1"
        and properties["transform_projection_policy"].get("const")
        == "glb-animation-linear-nlerp-flat-part-hierarchy-composed-world-trs@1"
        and properties["coordinate_system"].get("const") == "forgecad-rh-y-up-m@1"
        and properties["transform_convention"].get("const")
        == "column-vector-parent-world-times-trs-quaternion-xyzw@1"
        and properties["float_quantization_policy"].get("const")
        == "f32-round-nearest-canonical-json@1"
        and properties["frame_scope"].get("const") == "lod0-animation-frame-range-1-16@1"
        and properties["timebase_hz"].get("const") == 1000
        and properties["sample_count"].get("maximum") == 16
        and properties["sample_time_ticks"].get("minItems") == 1
        and properties["sample_time_ticks"].get("maxItems") == 16
        and properties["frames"].get("minItems") == 1
        and properties["frames"].get("maxItems") == 16,
        "GameWeaponAnimatedGlbSocketTransformProjection@1 must bind bounded replay policy and source lineage",
    )
    roles = ["weapon-root", "grip-primary", "muzzle-vfx", "magazine-well", "sight-primary", "energy-core-vfx"]
    require(
        properties["socket_roles"].get("const") == roles
        and properties["quality_status"].get("const") == "structural_only"
        and properties["visual_quality_status"].get("const") == "NOT_PROVEN"
        and properties["commercial_fps_quality_status"].get("const") == "NOT_PROVEN"
        and properties["human_review_status"].get("const") == "NOT_RUN"
        and properties["commercial_engine_status"].get("const") == "NOT_RUN"
        and properties["runtime_write_performed"].get("const") is True
        and properties["restart_hash_verified"].get("const") is True
        and properties["actual_engine_roundtrip"].get("const") is False
        and properties["production_stage_advanced"].get("const") is False
        and properties["candidate_confirmed"].get("const") is False
        and properties["version_created"].get("const") is False
        and properties["export_performed"].get("const") is False
        and properties["limitations"].get("const")
        == [
            "flat-identity-rest-part-hierarchy-only",
            "nested-part-hierarchy-rejected",
            "nonidentity-rest-part-transform-rejected",
            "matrix-and-shear-rejected",
            "structural-transform-readback-only",
            "no-visual-quality-or-likeness-pass",
            "no-commercial-engine-roundtrip",
            "no-functional-weapon-semantics",
        ],
        "animated GLB socket projection must preserve structural-only and nonfunctional boundaries",
    )

    pose = projection.get("$defs", {}).get("pose", {})
    require(
        pose.get("type") == "object"
        and pose.get("additionalProperties") is False
        and set(pose.get("required", []))
        == {"translation_m", "rotation_quat_xyzw", "scale_xyz"}
        and set(pose.get("properties", {}))
        == {"translation_m", "rotation_quat_xyzw", "scale_xyz"}
        and pose["properties"]["scale_xyz"].get("const") == [1.0, 1.0, 1.0],
        "projection poses must be finite TRS with identity scale",
    )
    socket = projection.get("$defs", {}).get("socket_transform", {})
    socket_fields = {
        "socket_node_id", "anchor_id", "role", "node_index", "parent_node_index", "node_name",
        "parent_node_name", "node_kind", "parent_kind", "owner_part_id", "local_transform",
        "parent_world_transform", "composed_world_transform",
    }
    require(
        socket.get("type") == "object"
        and socket.get("additionalProperties") is False
        and set(socket.get("required", [])) == socket_fields
        and set(socket.get("properties", {})) == socket_fields
        and socket["properties"]["role"].get("enum") == roles
        and socket["properties"]["node_kind"].get("const") == "empty"
        and socket["properties"]["parent_kind"].get("enum") == ["synthetic-scene-root", "part-node"],
        "projection socket rows must expose exactly six fixed non-rendering roles and TRS layers",
    )
    frame = projection.get("$defs", {}).get("frame", {})
    frame_fields = {
        "schema_version", "projection_key_sha256", "frame_index", "sample_time_ticks",
        "source_animation_sample_sha256", "derived_socket_sample_sha256",
        "socket_transform_inventory_sha256", "socket_transform_readback_sha256",
        "socket_transforms", "canonical_sha256", "created_at",
    }
    require(
        frame.get("type") == "object"
        and frame.get("additionalProperties") is False
        and set(frame.get("required", [])) == frame_fields
        and set(frame.get("properties", {})) == frame_fields
        and frame["properties"]["schema_version"].get("const")
        == "GameWeaponAnimatedGlbSocketTransformProjectionFrame@1"
        and frame["properties"]["frame_index"].get("maximum") == 15
        and frame["properties"]["socket_transforms"].get("minItems") == 6
        and frame["properties"]["socket_transforms"].get("maxItems") == 6,
        "projection frames must be bounded to sixteen samples and six socket transforms",
    )

    request_fields = projection_fields - {
        "frames", "projection_status", "quality_status", "visual_quality_status",
        "commercial_fps_quality_status", "human_review_status", "commercial_engine_status",
        "runtime_write_performed", "restart_hash_verified", "candidate_confirmed", "version_created",
        "export_performed", "actual_engine_roundtrip", "production_stage_advanced", "limitations",
        "canonical_sha256", "created_at",
    }
    request_fields.add("idempotency_key")
    request = load_schema(
        "game-weapon-animated-glb-socket-transform-projection-prepare-request.schema.json"
    )
    request_properties = request.get("properties", {})
    require(
        set(request.get("required", [])) == request_fields
        and set(request_properties) == request_fields
        and request_properties["schema_version"].get("const")
        == "GameWeaponAnimatedGlbSocketTransformProjectionPrepareRequest@1"
        and request_properties["transform_projection_policy"].get("const")
        == "glb-animation-linear-nlerp-flat-part-hierarchy-composed-world-trs@1"
        and request_properties["input_sha256"].get("$ref") == "#/$defs/sha256"
        and request_properties["idempotency_key"].get("$ref") == "#/$defs/id",
        "projection prepare request must canonically bind every source and explicit sample schedule",
    )

    get_request = load_schema(
        "game-weapon-animated-glb-socket-transform-projection-get-request.schema.json"
    )
    get_fields = {"schema_version", "projection_key_sha256", "project_id", "candidate_id"}
    require(
        set(get_request.get("required", [])) == get_fields
        and set(get_request.get("properties", {})) == get_fields
        and get_request["properties"]["schema_version"].get("const")
        == "GameWeaponAnimatedGlbSocketTransformProjectionGetRequest@1",
        "projection get request must bind exact key, project and candidate scope",
    )

    result_fields = {
        "schema_version", "projection_key_sha256", "projection_object_sha256", "projection", "replayed",
        "restart_hash_verified", "runtime_write", "quality_status", "visual_quality_status",
        "commercial_fps_quality_status", "human_review_status", "commercial_engine_status",
        "actual_engine_roundtrip", "production_stage_advanced", "candidate_confirmed", "version_created",
        "export_performed",
    }
    for filename, schema_version, runtime_write in [
        (
            "game-weapon-animated-glb-socket-transform-projection-prepare-result.schema.json",
            "GameWeaponAnimatedGlbSocketTransformProjectionPrepareResult@1",
            True,
        ),
        (
            "game-weapon-animated-glb-socket-transform-projection-get-result.schema.json",
            "GameWeaponAnimatedGlbSocketTransformProjectionGetResult@1",
            False,
        ),
    ]:
        result = load_schema(filename)
        result_properties = result.get("properties", {})
        require(
            set(result.get("required", [])) == result_fields
            and set(result_properties) == result_fields
            and result_properties["schema_version"].get("const") == schema_version
            and result_properties["projection"].get("$ref")
            == "game-weapon-animated-glb-socket-transform-projection.schema.json"
            and result_properties["projection_object_sha256"].get("$ref") == "#/$defs/sha256"
            and result_properties["restart_hash_verified"].get("const") is True
            and result_properties["runtime_write"].get("const") is runtime_write
            and result_properties["quality_status"].get("const") == "structural_only"
            and result_properties["visual_quality_status"].get("const") == "NOT_PROVEN"
            and result_properties["commercial_fps_quality_status"].get("const") == "NOT_PROVEN"
            and result_properties["human_review_status"].get("const") == "NOT_RUN"
            and result_properties["commercial_engine_status"].get("const") == "NOT_RUN"
            and result_properties["actual_engine_roundtrip"].get("const") is False
            and result_properties["production_stage_advanced"].get("const") is False
            and result_properties["candidate_confirmed"].get("const") is False
            and result_properties["version_created"].get("const") is False
            and result_properties["export_performed"].get("const") is False,
            f"{schema_version} must remain restart-verified, structural-only and non-promoting",
        )

    # V2 is additive.  Keep the V1 field set above untouched while binding the
    # appearance candidate and the V2 MechanicalAnimationGlb/AnimatedSocket
    # parents explicitly.  The projection's own CAS report hash is exposed by
    # the result contracts only; storing it on the projection would be a hash
    # cycle.
    expected_v2 = {
        "game-weapon-animated-glb-socket-transform-projection-v2.schema.json": "GameWeaponAnimatedGlbSocketTransformProjection@2",
        "game-weapon-animated-glb-socket-transform-projection-v2-prepare-request.schema.json": "GameWeaponAnimatedGlbSocketTransformProjectionPrepareRequest@2",
        "game-weapon-animated-glb-socket-transform-projection-v2-prepare-result.schema.json": "GameWeaponAnimatedGlbSocketTransformProjectionPrepareResult@2",
        "game-weapon-animated-glb-socket-transform-projection-v2-get-request.schema.json": "GameWeaponAnimatedGlbSocketTransformProjectionGetRequest@2",
        "game-weapon-animated-glb-socket-transform-projection-v2-get-result.schema.json": "GameWeaponAnimatedGlbSocketTransformProjectionGetResult@2",
    }
    actual_v2 = {
        path.name
        for path in SCHEMA_ROOT.glob("game-weapon-animated-glb-socket-transform-projection-v2*.schema.json")
    }
    require(actual_v2 == set(expected_v2), "animated GLB socket transform projection V2 must contain exactly five additive contracts")
    for filename, version in expected_v2.items():
        schema = load_schema(filename)
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and schema.get("title") == version
            and schema.get("properties", {}).get("schema_version", {}).get("const") == version
            and set(schema.get("required", [])) == set(schema.get("properties", {})),
            f"{version} must be a closed exact-field object contract",
        )

    projection_v2_fields = {
        "schema_version", "projection_key_sha256", "project_id", "appearance_candidate_id",
        "appearance_candidate_state_sha256", "appearance_delivery_manifest_object_sha256",
        "appearance_artifact_sha256", "appearance_artifact_readback_sha256", "animation_clip_id",
        "animation_clip_object_sha256", "animation_clip_canonical_sha256", "animation_glb_key_sha256",
        "animated_artifact_sha256", "animated_artifact_readback_sha256", "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256", "animated_socket_materialization_key_sha256",
        "derived_animated_socket_artifact_sha256", "derived_animated_socket_artifact_readback_sha256",
        "derived_animated_socket_receipt_object_sha256", "derived_animated_socket_receipt_canonical_sha256",
        "anchor_set_object_sha256", "anchor_set_canonical_sha256", "socket_node_id_encoding_sha256",
        "socket_node_inventory_sha256", "socket_roles_sha256", "socket_roles", "part_hierarchy_sha256",
        "part_hierarchy_policy", "transform_representation_policy", "sampling_policy_sha256",
        "sample_schedule_sha256", "sample_count", "sample_time_ticks", "frame_scope", "timebase_hz",
        "transform_projection_policy", "coordinate_system", "transform_convention",
        "float_quantization_policy", "input_sha256", "frames", "projection_status", "quality_status",
        "visual_quality_status", "commercial_fps_quality_status", "human_review_status",
        "commercial_engine_status", "runtime_write_performed", "restart_hash_verified",
        "candidate_confirmed", "version_created", "export_performed", "actual_engine_roundtrip",
        "production_stage_advanced", "limitations", "canonical_sha256", "created_at",
    }
    projection_v2 = load_schema("game-weapon-animated-glb-socket-transform-projection-v2.schema.json")
    projection_v2_properties = projection_v2.get("properties", {})
    require(
        set(projection_v2.get("required", [])) == projection_v2_fields
        and set(projection_v2_properties) == projection_v2_fields
        and "projection_object_sha256" not in projection_v2_fields
        and projection_v2_properties["schema_version"].get("const")
        == "GameWeaponAnimatedGlbSocketTransformProjection@2"
        and projection_v2_properties["projection_status"].get("const")
        == "runtime-owned-durable-game-weapon-animated-glb-socket-transform-projection-v2"
        and projection_v2_properties["part_hierarchy_policy"].get("const")
        == "flat-identity-rest-part-hierarchy-only@2"
        and projection_v2_properties["transform_representation_policy"].get("const")
        == "trs-quaternion-no-shear-plus-column-major-matrix@2"
        and projection_v2_properties["transform_projection_policy"].get("const")
        == "glb-animation-linear-nlerp-flat-part-hierarchy-composed-world-trs-matrix@2"
        and projection_v2_properties["sampling_policy_sha256"].get("$ref") == "#/$defs/sha256"
        and projection_v2_properties["frame_scope"].get("const") == "lod0-animation-frame-range-1-16@2"
        and projection_v2_properties["timebase_hz"].get("const") == 1000
        and projection_v2_properties["sample_count"].get("maximum") == 16
        and projection_v2_properties["sample_time_ticks"].get("minItems") == 1
        and projection_v2_properties["sample_time_ticks"].get("maxItems") == 16
        and projection_v2_properties["frames"].get("minItems") == 1
        and projection_v2_properties["frames"].get("maxItems") == 16,
        "GameWeaponAnimatedGlbSocketTransformProjection@2 must remain bounded and explicitly V2-bound",
    )
    roles_v2 = ["weapon-root", "grip-primary", "muzzle-vfx", "magazine-well", "sight-primary", "energy-core-vfx"]
    require(
        projection_v2_properties["socket_roles"].get("const") == roles_v2
        and projection_v2_properties["quality_status"].get("const") == "structural_only"
        and projection_v2_properties["visual_quality_status"].get("const") == "NOT_PROVEN"
        and projection_v2_properties["commercial_fps_quality_status"].get("const") == "NOT_PROVEN"
        and projection_v2_properties["human_review_status"].get("const") == "NOT_RUN"
        and projection_v2_properties["commercial_engine_status"].get("const") == "NOT_RUN"
        and projection_v2_properties["runtime_write_performed"].get("const") is True
        and projection_v2_properties["restart_hash_verified"].get("const") is True
        and projection_v2_properties["actual_engine_roundtrip"].get("const") is False
        and projection_v2_properties["production_stage_advanced"].get("const") is False
        and projection_v2_properties["candidate_confirmed"].get("const") is False
        and projection_v2_properties["version_created"].get("const") is False
        and projection_v2_properties["export_performed"].get("const") is False,
        "animated GLB socket transform projection V2 must remain structural-only and non-promoting",
    )

    pose_v2 = projection_v2.get("$defs", {}).get("pose", {})
    require(
        pose_v2.get("type") == "object"
        and pose_v2.get("additionalProperties") is False
        and set(pose_v2.get("required", [])) == {"translation_m", "rotation_quat_xyzw", "scale_xyz"}
        and set(pose_v2.get("properties", {})) == {"translation_m", "rotation_quat_xyzw", "scale_xyz"}
        and pose_v2["properties"]["scale_xyz"].get("const") == [1.0, 1.0, 1.0],
        "projection V2 poses must preserve V1 finite TRS with identity scale",
    )
    socket_v2 = projection_v2.get("$defs", {}).get("socket_transform", {})
    socket_v2_fields = {
        "socket_node_id", "anchor_id", "role", "node_index", "parent_node_index", "node_name",
        "parent_node_name", "node_kind", "parent_kind", "owner_part_id", "local_transform",
        "parent_world_transform", "composed_world_transform", "local_matrix_4x4",
        "parent_world_matrix_4x4", "composed_world_matrix_4x4",
    }
    require(
        socket_v2.get("type") == "object"
        and socket_v2.get("additionalProperties") is False
        and set(socket_v2.get("required", [])) == socket_v2_fields
        and set(socket_v2.get("properties", {})) == socket_v2_fields
        and socket_v2["properties"]["role"].get("enum") == roles_v2
        and socket_v2["properties"]["node_kind"].get("const") == "empty"
        and socket_v2["properties"]["parent_kind"].get("enum") == ["synthetic-scene-root", "part-node"],
        "projection V2 socket rows must expose six fixed empty nodes with TRS and 4x4 matrices",
    )
    frame_v2 = projection_v2.get("$defs", {}).get("frame", {})
    frame_v2_fields = {
        "schema_version", "projection_key_sha256", "frame_index", "sample_time_ticks",
        "source_animation_sample_sha256", "derived_socket_sample_sha256",
        "socket_transform_inventory_sha256", "socket_transform_readback_sha256",
        "projection_frame_canonical_sha256", "socket_transforms", "canonical_sha256", "created_at",
    }
    require(
        frame_v2.get("type") == "object"
        and frame_v2.get("additionalProperties") is False
        and set(frame_v2.get("required", [])) == frame_v2_fields
        and set(frame_v2.get("properties", {})) == frame_v2_fields
        and frame_v2["properties"]["schema_version"].get("const")
        == "GameWeaponAnimatedGlbSocketTransformProjectionFrame@2"
        and frame_v2["properties"]["frame_index"].get("maximum") == 15
        and frame_v2["properties"]["socket_transforms"].get("minItems") == 6
        and frame_v2["properties"]["socket_transforms"].get("maxItems") == 6,
        "projection V2 frames must be bounded to sixteen samples and six socket transforms",
    )

    request_v2_fields = projection_v2_fields - {
        "frames", "projection_status", "quality_status", "visual_quality_status",
        "commercial_fps_quality_status", "human_review_status", "commercial_engine_status",
        "runtime_write_performed", "restart_hash_verified", "candidate_confirmed", "version_created",
        "export_performed", "actual_engine_roundtrip", "production_stage_advanced", "limitations",
        "canonical_sha256", "created_at",
    }
    request_v2_fields.add("idempotency_key")
    request_v2 = load_schema("game-weapon-animated-glb-socket-transform-projection-v2-prepare-request.schema.json")
    require(
        set(request_v2.get("required", [])) == request_v2_fields
        and set(request_v2.get("properties", {})) == request_v2_fields
        and request_v2["properties"]["schema_version"].get("const")
        == "GameWeaponAnimatedGlbSocketTransformProjectionPrepareRequest@2"
        and request_v2["properties"]["transform_projection_policy"].get("const")
        == "glb-animation-linear-nlerp-flat-part-hierarchy-composed-world-trs-matrix@2"
        and request_v2["properties"]["input_sha256"].get("$ref") == "#/$defs/sha256"
        and request_v2["properties"]["idempotency_key"].get("$ref") == "#/$defs/id",
        "projection V2 prepare request must bind all V2 sources and the bounded schedule",
    )

    get_v2 = load_schema("game-weapon-animated-glb-socket-transform-projection-v2-get-request.schema.json")
    require(
        set(get_v2.get("required", []))
        == {"schema_version", "projection_key_sha256", "project_id", "appearance_candidate_id", "animation_clip_id"}
        and set(get_v2.get("properties", {}))
        == {"schema_version", "projection_key_sha256", "project_id", "appearance_candidate_id", "animation_clip_id"}
        and get_v2["properties"]["schema_version"].get("const")
        == "GameWeaponAnimatedGlbSocketTransformProjectionGetRequest@2",
        "projection V2 get request must bind exact key, project, appearance candidate and Clip@2",
    )

    result_v2_fields = {
        "schema_version", "projection_key_sha256", "projection_object_sha256", "projection", "replayed",
        "restart_hash_verified", "runtime_write_performed", "quality_status", "visual_quality_status",
        "commercial_fps_quality_status", "human_review_status", "commercial_engine_status",
        "actual_engine_roundtrip", "production_stage_advanced", "candidate_confirmed", "version_created",
        "export_performed",
    }
    for filename, schema_version, runtime_write in [
        (
            "game-weapon-animated-glb-socket-transform-projection-v2-prepare-result.schema.json",
            "GameWeaponAnimatedGlbSocketTransformProjectionPrepareResult@2",
            True,
        ),
        (
            "game-weapon-animated-glb-socket-transform-projection-v2-get-result.schema.json",
            "GameWeaponAnimatedGlbSocketTransformProjectionGetResult@2",
            False,
        ),
    ]:
        result = load_schema(filename)
        result_properties = result.get("properties", {})
        require(
            set(result.get("required", [])) == result_v2_fields
            and set(result_properties) == result_v2_fields
            and result_properties["schema_version"].get("const") == schema_version
            and result_properties["projection"].get("$ref")
            == "game-weapon-animated-glb-socket-transform-projection-v2.schema.json"
            and result_properties["projection_object_sha256"].get("$ref") == "#/$defs/sha256"
            and result_properties["restart_hash_verified"].get("const") is True
            and result_properties["runtime_write_performed"].get("const") is runtime_write
            and result_properties["quality_status"].get("const") == "structural_only"
            and result_properties["visual_quality_status"].get("const") == "NOT_PROVEN"
            and result_properties["commercial_fps_quality_status"].get("const") == "NOT_PROVEN"
            and result_properties["human_review_status"].get("const") == "NOT_RUN"
            and result_properties["commercial_engine_status"].get("const") == "NOT_RUN"
            and result_properties["actual_engine_roundtrip"].get("const") is False
            and result_properties["production_stage_advanced"].get("const") is False
            and result_properties["candidate_confirmed"].get("const") is False
            and result_properties["version_created"].get("const") is False
            and result_properties["export_performed"].get("const") is False,
            f"{schema_version} must remain restart-verified, structural-only and non-promoting",
        )


def check_registered_camera_rig_contracts() -> None:
    registration = load_schema("production-weapon-subject-frame-registration.schema.json")
    registration_fields = {
        "schema_version", "registration_id", "geometry_program_sha256",
        "subject_coordinate_frame_sha256", "derivation_policy", "geometry_semantic_axes",
        "subject_semantic_axes", "anchor_evidence", "transform", "read_only",
        "geometry_program_modified", "depth_modified", "canonical_sha256",
    }
    registration_properties = registration.get("properties", {})
    require(
        registration.get("additionalProperties") is False
        and set(registration.get("required", [])) == registration_fields
        and set(registration_properties) == registration_fields
        and registration_properties["schema_version"].get("const")
        == "ProductionWeaponSubjectFrameRegistration@1"
        and registration_properties["derivation_policy"].get("const")
        == "exact-semantic-anchor-axis-registration@1"
        and registration_properties["read_only"].get("const") is True
        and registration_properties["geometry_program_modified"].get("const") is False
        and registration_properties["depth_modified"].get("const") is False,
        "subject-frame registration must remain closed, exact, read-only and non-mutating",
    )
    anchor = registration_properties["anchor_evidence"]
    anchor_fields = {
        "stock_node_ids", "stock_position_x_m", "muzzle_node_ids",
        "muzzle_position_x_m", "side_left_node_id", "side_left_position_z_m",
        "side_right_node_id", "side_right_position_z_m",
    }
    require(
        anchor.get("additionalProperties") is False
        and set(anchor.get("required", [])) == anchor_fields
        and set(anchor.get("properties", {})) == anchor_fields
        and anchor["properties"]["stock_node_ids"].get("const")
        == ["rear-stock", "rear-stock-lower-beam"]
        and anchor["properties"]["muzzle_node_ids"].get("const")
        == ["muzzle-shroud", "muzzle-emitter", "muzzle-core"]
        and anchor["properties"]["side_left_node_id"].get("const") == "side-light-left"
        and anchor["properties"]["side_right_node_id"].get("const") == "side-light-right",
        "subject-frame registration anchors must be exact semantic Part sources",
    )
    transform = registration_properties["transform"]
    require(
        transform.get("additionalProperties") is False
        and transform["properties"]["direction"].get("const") == "geometry-to-subject"
        and transform["properties"]["kind"].get("enum") == ["identity", "yaw-180-y"]
        and transform["properties"]["translation_m"].get("const") == [0.0, 0.0, 0.0]
        and transform["properties"]["scale"].get("const") == [1.0, 1.0, 1.0],
        "subject-frame transform must remain a closed rigid identity/yaw-180 projection",
    )
    transform_guards = transform.get("allOf", [])
    require(
        any(
            guard.get("if", {}).get("properties", {}).get("kind", {}).get("const") == "identity"
            and guard.get("then", {}).get("properties", {}).get("rotation_rad", {}).get("const")
            == [0.0, 0.0, 0.0]
            for guard in transform_guards
        )
        and any(
            guard.get("if", {}).get("properties", {}).get("kind", {}).get("const") == "yaw-180-y"
            and guard.get("then", {}).get("properties", {}).get("rotation_rad", {}).get("const")
            == [0.0, 3.141592653589793, 0.0]
            for guard in transform_guards
        ),
        "registration kind must select the exact closed rotation",
    )

    rig = load_schema("registered-camera-rig-calibration.schema.json")
    rig_fields = {
        "schema_version", "registered_rig_id", "project_id", "candidate_id",
        "candidate_state_sha256", "artifact_id", "artifact_sha256",
        "geometry_program_object_sha256", "geometry_program_sha256",
        "operator_catalog_sha256", "subject_camera_rig", "subject_camera_rig_object_sha256",
        "subject_camera_rig_canonical_sha256", "subject_frame_registration",
        "subject_frame_registration_canonical_sha256", "renderer_views", "read_only",
        "runtime_write", "depth_status", "quality_status", "production_stage_advanced",
        "candidate_confirmed", "version_created", "export_performed", "canonical_sha256",
    }
    rig_properties = rig.get("properties", {})
    require(
        rig.get("additionalProperties") is False
        and set(rig.get("required", [])) == rig_fields
        and set(rig_properties) == rig_fields
        and rig_properties["schema_version"].get("const")
        == "RegisteredCameraRigCalibration@1"
        and rig_properties["subject_camera_rig"].get("$ref")
        == "https://forgecad.local/contracts/camera-rig-calibration.schema.json"
        and rig_properties["subject_frame_registration"].get("$ref")
        == "https://forgecad.local/contracts/production-weapon-subject-frame-registration.schema.json"
        and rig_properties["read_only"].get("const") is True
        and rig_properties["runtime_write"].get("const") is False
        and rig_properties["depth_status"].get("const") == "UNKNOWN"
        and rig_properties["quality_status"].get("const") == "NOT_EVALUATED",
        "registered camera rig must preserve subject truth, exact registration and non-promoting status",
    )
    require(
        all(
            rig_properties[field].get("const") is False
            for field in (
                "production_stage_advanced", "candidate_confirmed", "version_created",
                "export_performed",
            )
        ),
        "registered camera rig must not promote, confirm, version or export",
    )
    renderer_views = rig_properties["renderer_views"]
    registered_view = rig.get("$defs", {}).get("registered_view", {})
    registered_view_fields = {
        "view_id", "kind", "subject_camera_hash", "registered_camera",
        "registered_camera_hash", "registration_canonical_sha256", "weight", "primary",
    }
    require(
        renderer_views.get("minItems") == 6
        and renderer_views.get("maxItems") == 8
        and registered_view.get("additionalProperties") is False
        and set(registered_view.get("required", [])) == registered_view_fields
        and set(registered_view.get("properties", {})) == registered_view_fields
        and registered_view["properties"]["registered_camera"].get("$ref")
        == "https://forgecad.local/contracts/camera-calibration-v2.schema.json",
        "registered renderer views must bind every materialized camera to subject and registration hashes",
    )


def check_native_high_source_contracts() -> None:
    request = load_schema("high-mesh-worker-request.schema.json")
    artifact = load_schema("high-mesh-artifact.schema.json")

    request_fields = {
        "schema_version", "operation", "source_authoring_mesh",
        "source_authoring_mesh_sha256", "detail_graph",
        "detail_graph_canonical_sha256", "budgets", "canonical_sha256",
    }
    request_properties = request.get("properties", {})
    require(
        request.get("additionalProperties") is False
        and set(request.get("required", [])) == request_fields
        and set(request_properties) == request_fields
        and request_properties["schema_version"].get("const") == "HighMeshWorkerRequest@1"
        and request_properties["operation"].get("const")
        == "forgecad.production.high-mesh-prepare@1",
        "Native High request must remain closed and operation-bound",
    )
    definitions = request.get("$defs", {})
    authoring_mesh = definitions.get("authoring_mesh", {})
    detail_graph = definitions.get("detail_graph", {})
    detail_node = definitions.get("detail_node", {})
    require(
        authoring_mesh.get("additionalProperties") is False
        and authoring_mesh.get("properties", {}).get("schema_version", {}).get("const")
        == "AuthoringMeshSource@1"
        and authoring_mesh.get("properties", {}).get("parts", {}).get("maxItems") == 128
        and detail_graph.get("additionalProperties") is False
        and detail_graph.get("properties", {}).get("schema_version", {}).get("const")
        == "DetailGraph@1"
        and detail_graph.get("properties", {}).get("nodes", {}).get("minItems") == 1
        and detail_graph.get("properties", {}).get("nodes", {}).get("maxItems") == 256
        and detail_node.get("additionalProperties") is False
        and detail_node.get("properties", {}).get("kind", {}).get("enum")
        == ["support_loop", "crease", "floating_detail"],
        "Native High source projection must keep bounded closed mesh and DetailGraph shapes",
    )

    artifact_fields = {
        "schema_version", "operation", "policy", "artifact_id", "artifact_sha256",
        "source_authoring_mesh_sha256", "detail_graph_canonical_sha256",
        "request_sha256", "input_sha256", "high_worker_algorithm_sha256",
        "high_worker_build_cohort_sha256", "replay_count", "replay_byte_exact",
        "base_parts", "detail_primitives", "detail_lineage", "part_ids",
        "material_zone_ids", "triangle_count", "base_triangle_count",
        "detail_triangle_count", "non_destructive", "high_topology_status",
        "high_authoring_topology_status", "uv_status", "tangent_status",
        "structural_status", "visual_status", "human_status", "engine_status",
        "distribution_status", "quality_status", "hard_gate_passed",
        "runtime_write_performed", "production_stage_advanced", "candidate_confirmed",
        "version_created", "export_performed", "canonical_sha256",
    }
    artifact_properties = artifact.get("properties", {})
    require(
        artifact.get("additionalProperties") is False
        and set(artifact.get("required", [])) == artifact_fields
        and set(artifact_properties) == artifact_fields
        and artifact_properties["schema_version"].get("const") == "HighMeshArtifact@1"
        and artifact_properties["replay_count"].get("const") == 2
        and artifact_properties["replay_byte_exact"].get("const") is True
        and artifact_properties["non_destructive"].get("const") is True
        and artifact_properties["structural_status"].get("const")
        == "PASS_SOURCE_STRUCTURAL"
        and artifact_properties["quality_status"].get("const") == "structural_only",
        "Native High artifact must remain deterministic, base-preserving and structural-only",
    )
    for field in ("visual_status", "human_status", "engine_status", "distribution_status"):
        require(
            artifact_properties[field].get("const") == "NOT_RUN",
            f"Native High artifact must not promote {field}",
        )
    for field in (
        "hard_gate_passed", "runtime_write_performed", "production_stage_advanced",
        "candidate_confirmed", "version_created", "export_performed",
    ):
        require(
            artifact_properties[field].get("const") is False,
            f"Native High artifact must keep {field}=false",
        )


def main() -> int:
    required = [
        CONTRACT_ROOT / "manifest.json",
        SCHEMA_ROOT / "audit-event.schema.json",
        SCHEMA_ROOT / "candidate.schema.json",
        SCHEMA_ROOT / "cas-object.schema.json",
        SCHEMA_ROOT / "design-asset-version.schema.json",
        SCHEMA_ROOT / "job-event.schema.json",
        SCHEMA_ROOT / "project.schema.json",
        SCHEMA_ROOT / "runtime-capabilities.schema.json",
        SCHEMA_ROOT / "runtime-tool.schema.json",
        SCHEMA_ROOT / "runtime-project.schema.json",
        SCHEMA_ROOT / "runtime-snapshot.schema.json",
        SCHEMA_ROOT / "runtime-job.schema.json",
        SCHEMA_ROOT / "runtime-error.schema.json",
        SCHEMA_ROOT / "runtime-resource.schema.json",
        SCHEMA_ROOT / "runtime-selection.schema.json",
        SCHEMA_ROOT / "snapshot.schema.json",
        ROOT / "migrations-runtime-v1" / "0001_runtime.sql",
    ]
    missing = [str(path.relative_to(ROOT)) for path in required if not path.exists()]
    if missing:
        raise SystemExit(f"missing MCP002 contract files: {missing}")

    for path in sorted(SCHEMA_ROOT.glob("*.json")):
        document = json.loads(path.read_text(encoding="utf-8"))
        if document.get("$schema") != "https://json-schema.org/draft/2020-12/schema":
            raise SystemExit(f"schema draft missing: {path}")
        if not str(document.get("$id", "")).startswith("https://forgecad.local/contracts/"):
            raise SystemExit(f"schema id missing: {path}")

    manifest = json.loads((CONTRACT_ROOT / "manifest.json").read_text(encoding="utf-8"))
    if manifest.get("contract_set") != "forgecad-runtime-contracts@1":
        raise SystemExit("unexpected contract set")
    if manifest.get("model_calls") is not False:
        raise SystemExit("MCP002 contracts must declare model_calls=false")
    actual_schemas = sorted(path.name for path in SCHEMA_ROOT.glob("*.json"))
    declared_schemas = sorted(manifest.get("schemas", []))
    if actual_schemas != declared_schemas:
        raise SystemExit("contract manifest schema list does not match checked-in schemas")
    check_mcp010b_contracts()
    check_recessed_channel_kit_enums()
    check_mcp010c_contracts()
    check_mcp010f_silhouette_contracts()
    check_mcp010e_contracts()
    check_modifier_stack_contracts()
    check_modifier_apply_contracts()
    check_parametric_group_contracts()
    check_topology_snapshot_contracts()
    check_authoring_topology_contracts()
    check_authoring_mesh_contracts()
    check_durable_authoring_mesh_contracts()
    check_subdivision_evaluation_contracts()
    check_subdivision_crease_contracts()
    check_render_profile_contracts()
    check_render_evidence_integrity_contracts()
    check_render_evidence_replay_contracts()
    check_mechanical_pose_contracts()
    check_mechanical_animation_clip_contracts()
    check_mechanical_animation_clip_v2_contracts()
    check_viewer_provenance_graph_contracts()
    check_game_asset_delivery_contracts()
    check_game_weapon_anchor_contracts()
    check_game_weapon_glb_socket_materialization_contracts()
    check_game_weapon_animated_glb_socket_materialization_contracts()
    check_game_weapon_animated_glb_socket_materialization_v2_contracts()
    check_fictional_energy_vfx_contracts()
    check_fictional_energy_vfx_trails_bloom_contracts()
    check_boolean_operand_lineage_contracts()
    check_subdivision_topology_lineage_contracts()
    check_subdivision_artifact_lineage_sidecar_contracts()
    check_production_stage_transition_contracts()
    check_production_stage_v2_contracts()
    check_production_stage_v3_contracts()
    check_production_camera_lock_contracts()
    check_registered_camera_rig_contracts()
    check_production_weapon_form_quality_contracts()
    check_production_weapon_form_quality_v2_contracts()
    check_production_weapon_form_quality_v2_preflight_contracts()
    check_production_weapon_form_evidence_contracts()
    check_production_weapon_form_art_evidence_contracts()
    check_production_weapon_owner_reviewed_void_calibration_contracts()
    check_production_weapon_form_art_mesh_proposal_edit_contracts()
    check_production_weapon_form_art_proposal_evidence_contracts()
    check_production_weapon_art_decision_contracts()
    check_production_weapon_assembly_parameter_sink_contracts()
    check_production_weapon_high_low_cage_contracts()
    check_production_weapon_high_low_bake_preflight_contracts()
    check_blender_worker_capability_contracts()
    check_blender_task_contracts()
    check_native_high_source_contracts()
    check_candidate_topology_quality_contracts()
    check_candidate_material_surface_quality_contracts()
    check_candidate_animation_vfx_quality_contracts()
    check_candidate_animation_vfx_quality_v2_contracts()
    check_fictional_energy_vfx_animated_socket_attachment_contracts()
    check_game_weapon_animated_glb_socket_transform_projection_contracts()
    check_fictional_energy_vfx_animated_socket_particles_sequence_contracts()
    check_fictional_energy_vfx_animated_socket_particles_sequence_v2_contracts()
    check_fictional_energy_vfx_animated_socket_trails_sequence_contracts()
    check_fictional_energy_vfx_animated_socket_trails_sequence_v2_contracts()
    check_fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_contracts()
    print(f"ForgeCAD contracts OK: {len(actual_schemas)} schemas")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
