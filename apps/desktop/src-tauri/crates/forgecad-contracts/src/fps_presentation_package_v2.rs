//! Runtime-owned editable composite FPS presentation package.
//!
//! V2 is additive to the immutable single-import `FpsPresentationPackage@1`.
//! It binds one weapon materialization, one first-person arms materialization,
//! one animation-source materialization, their AuthoringMesh@2 revision sets,
//! sockets and rig maps.  It carries no topology arrays, paths, URLs, scripts,
//! candidate confirmation, version or export state.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const FPS_PRESENTATION_PACKAGE_V2_SCHEMA_VERSION: &str = "FpsPresentationPackage@2";
pub const FPS_PRESENTATION_PACKAGE_V2_COMPONENT_SCHEMA_VERSION: &str =
    "FpsPresentationPackageV2ComponentBinding@1";
pub const FPS_PRESENTATION_PACKAGE_V2_PREPARE_REQUEST_SCHEMA_VERSION: &str =
    "FpsPresentationPackageV2PrepareRequest@1";
pub const FPS_PRESENTATION_PACKAGE_V2_PREPARE_RESULT_SCHEMA_VERSION: &str =
    "FpsPresentationPackageV2PrepareResult@1";
pub const FPS_PRESENTATION_PACKAGE_V2_GET_REQUEST_SCHEMA_VERSION: &str =
    "FpsPresentationPackageV2GetRequest@1";
pub const FPS_PRESENTATION_PACKAGE_V2_GET_RESULT_SCHEMA_VERSION: &str =
    "FpsPresentationPackageV2GetResult@1";
pub const FPS_PRESENTATION_PACKAGE_V2_PRODUCTION_PREFLIGHT_RESULT_SCHEMA_VERSION: &str =
    "FpsPresentationPackageV2ProductionPreflightResult@1";

pub const FPS_PRESENTATION_PACKAGE_V2_POLICY: &str =
    "runtime-owned-editable-weapon-arms-sockets-rig-clips-composite@2";
pub const FPS_PRESENTATION_PACKAGE_V2_WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
pub const FPS_PRESENTATION_PACKAGE_V2_CANONICALIZATION_POLICY: &str =
    "canonical-json-sha256-excluding-canonical-sha256@1";
pub const FPS_PRESENTATION_PACKAGE_V2_STATUS: &str = "EDITABLE_COMPOSITE_BOUND";
pub const FPS_PRESENTATION_PACKAGE_V2_QUALITY_STATUS: &str = "structural_only";
pub const FPS_PRESENTATION_PACKAGE_V2_REVIEW_STATUS: &str = "DRAFT_UNREVIEWED";
pub const FPS_PRESENTATION_PACKAGE_V2_MAX_RESPONSE_BYTES: u64 = 1_048_576;
pub const FPS_PRESENTATION_PACKAGE_V2_REQUIRED_CLIPS: &[&str] = &[
    "idle",
    "equip",
    "fire_recoil",
    "reload",
    "inspect",
    "ads_in",
    "ads_out",
    "sprint",
    "holster",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FpsPresentationPackageV2ComponentBinding {
    pub schema_version: String,
    pub component_role: String,
    pub source_asset_id: String,
    pub source_asset_sha256: String,
    pub source_asset_role: String,
    pub materialization_id: String,
    pub materialization_descriptor_object_sha256: String,
    pub materialization_descriptor_sha256: String,
    pub foundation_package_object_sha256: String,
    pub socket_map_object_sha256: String,
    pub rig_map_object_sha256: String,
    pub part_ids: Vec<String>,
    pub part_revision_object_sha256s: Vec<String>,
    pub part_revision_summary_sha256: String,
    pub part_count: u64,
    pub vertex_count: u64,
    pub face_count: u64,
    pub editable_authoring_mesh_v2: bool,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FpsPresentationProductionPipelineBinding {
    pub formal_high_entrypoint: String,
    pub low_cage_bake_entrypoint: String,
    pub hero_uv_entrypoint: String,
    pub fps_validation_entrypoint: String,
    pub engine_validation_entrypoint: String,
    pub human_hero_review_entrypoint: String,
    pub formal_high_status: String,
    pub low_status: String,
    pub hero_uv_status: String,
    pub cage_bake_status: String,
    pub fps_validation_status: String,
    pub engine_validation_status: String,
    pub human_hero_review_status: String,
    pub blocker: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FpsPresentationPackageV2 {
    pub schema_version: String,
    pub package_id: String,
    pub project_id: String,
    pub weapon: FpsPresentationPackageV2ComponentBinding,
    pub first_person_arms: FpsPresentationPackageV2ComponentBinding,
    pub animation_source: FpsPresentationPackageV2ComponentBinding,
    pub coordinate_spec_sha256: String,
    pub weapon_socket_map_object_sha256: String,
    pub weapon_rig_map_object_sha256: String,
    pub arms_rig_map_object_sha256: String,
    pub animation_rig_map_object_sha256: String,
    pub source_package_object_sha256s: Vec<String>,
    pub aggregate_part_revision_summary_sha256: String,
    pub aggregate_part_count: u64,
    pub aggregate_vertex_count: u64,
    pub aggregate_face_count: u64,
    pub source_animation_clip_ids: Vec<String>,
    pub required_clip_ids: Vec<String>,
    pub missing_required_clip_ids: Vec<String>,
    pub animation_binding_status: String,
    pub socket_binding_status: String,
    pub rig_binding_status: String,
    pub authoring_status: String,
    pub production_pipeline: FpsPresentationProductionPipelineBinding,
    pub package_policy: String,
    pub status: String,
    pub quality_status: String,
    pub review_status: String,
    pub promotion_eligible: bool,
    pub candidate_created: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub actual_engine_roundtrip: bool,
    pub human_review_performed: bool,
    pub canonicalization_policy: String,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FpsPresentationPackageV2PrepareRequest {
    pub schema_version: String,
    pub project_id: String,
    pub weapon_materialization_id: String,
    pub weapon_descriptor_sha256: String,
    pub arms_materialization_id: String,
    pub arms_descriptor_sha256: String,
    pub animation_materialization_id: String,
    pub animation_descriptor_sha256: String,
    pub package_policy: String,
    pub idempotency_key: String,
    pub max_response_bytes: u64,
    pub runtime_write_performed: bool,
    pub writer_policy: String,
    pub canonicalization_policy: String,
    pub input_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FpsPresentationPackageV2PrepareResult {
    pub schema_version: String,
    pub project_id: String,
    pub package_id: String,
    pub package_object_sha256: String,
    pub package_sha256: String,
    pub package: FpsPresentationPackageV2,
    pub request_input_sha256: String,
    pub idempotency_key: String,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FpsPresentationPackageV2GetRequest {
    pub schema_version: String,
    pub project_id: String,
    pub package_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_sha256: Option<String>,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub input_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FpsPresentationPackageV2GetResult {
    pub schema_version: String,
    pub project_id: String,
    pub package_id: String,
    pub package_object_sha256: String,
    pub package_sha256: String,
    pub package: FpsPresentationPackageV2,
    pub request_input_sha256: String,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FpsPresentationPackageV2ProductionPreflightResult {
    pub schema_version: String,
    pub project_id: String,
    pub package_id: String,
    pub package_object_sha256: String,
    pub package_sha256: String,
    pub editable_composite_ready: bool,
    pub gates: BTreeMap<String, String>,
    pub next_action: String,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub canonicalization_policy: String,
    pub canonical_sha256: String,
}
