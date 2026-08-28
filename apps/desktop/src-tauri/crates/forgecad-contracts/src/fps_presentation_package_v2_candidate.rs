//! Closed candidate binding for one editable FPS presentation package.
//!
//! Runtime derives the reviewable weapon candidate from the package-owned
//! foundation AuthoringMesh revision. The request cannot carry topology,
//! paths, URLs, scripts, candidate state or production approvals.

use serde::{Deserialize, Serialize};

pub const FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_BINDING_SCHEMA_VERSION: &str =
    "FpsPresentationPackageV2CandidateBinding@1";
pub const FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_PREPARE_REQUEST_SCHEMA_VERSION: &str =
    "FpsPresentationPackageV2CandidatePrepareRequest@1";
pub const FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_PREPARE_RESULT_SCHEMA_VERSION: &str =
    "FpsPresentationPackageV2CandidatePrepareResult@1";
pub const FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_GET_REQUEST_SCHEMA_VERSION: &str =
    "FpsPresentationPackageV2CandidateGetRequest@1";
pub const FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_GET_RESULT_SCHEMA_VERSION: &str =
    "FpsPresentationPackageV2CandidateGetResult@1";
pub const FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_POLICY: &str =
    "runtime-derived-package-weapon-authoring-mesh-reviewable-candidate@1";
pub const FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_WRITER_POLICY: &str =
    "forgecad-runtime-only-state-writer@1";
pub const FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_CANONICALIZATION_POLICY: &str =
    "canonical-json-sha256-excluding-canonical-sha256@1";
pub const FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_MAX_RESPONSE_BYTES: u64 = 1_048_576;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FpsPresentationPackageV2CandidateBinding {
    pub schema_version: String,
    pub project_id: String,
    pub package_id: String,
    pub package_object_sha256: String,
    pub package_sha256: String,
    pub weapon_materialization_id: String,
    pub weapon_materialization_descriptor_sha256: String,
    pub weapon_part_id: String,
    pub weapon_material_zone_id: String,
    pub weapon_authoring_mesh_revision_id: String,
    pub weapon_authoring_mesh_revision_object_sha256: String,
    pub weapon_authoring_mesh_revision_sha256: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub candidate_state: String,
    pub candidate_artifact_sha256: String,
    pub geometry_program_object_sha256: String,
    pub geometry_program_sha256: String,
    pub geometry_candidate_evidence_sha256: String,
    pub geometry_integrity_status: String,
    pub form_stage: String,
    pub secondary_form_approved: bool,
    pub formal_high_status: String,
    pub quality_status: String,
    pub visual_review_status: String,
    pub engine_validation_status: String,
    pub human_review_status: String,
    pub promotion_eligible: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub policy: String,
    pub canonicalization_policy: String,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FpsPresentationPackageV2CandidatePrepareRequest {
    pub schema_version: String,
    pub project_id: String,
    pub package_id: String,
    pub package_sha256: String,
    pub policy: String,
    pub idempotency_key: String,
    pub max_response_bytes: u64,
    pub runtime_write_performed: bool,
    pub writer_policy: String,
    pub canonicalization_policy: String,
    pub input_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FpsPresentationPackageV2CandidatePrepareResult {
    pub schema_version: String,
    pub binding_object_sha256: String,
    pub binding: FpsPresentationPackageV2CandidateBinding,
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
pub struct FpsPresentationPackageV2CandidateGetRequest {
    pub schema_version: String,
    pub project_id: String,
    pub package_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding_sha256: Option<String>,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub input_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FpsPresentationPackageV2CandidateGetResult {
    pub schema_version: String,
    pub binding_object_sha256: String,
    pub binding: FpsPresentationPackageV2CandidateBinding,
    pub request_input_sha256: String,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub canonical_sha256: String,
}
