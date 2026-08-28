//! Closed contracts for the Runtime-owned fresh FormArt baseline.
//!
//! A baseline is an immutable, candidate-bound evidence record.  It carries
//! only opaque identifiers and hashes; reference/camera/RenderSet content is
//! resolved by Runtime from the approved registration lineage and the fixed
//! RigV2.  The record is intentionally evidence-only: it never promotes a
//! candidate or advances a production stage.

use serde::{Deserialize, Serialize};

pub const PRODUCTION_WEAPON_FORM_ART_BASELINE_SCHEMA_VERSION: &str =
    "ProductionWeaponFormArtBaseline@1";
pub const PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_SCHEMA_VERSION: &str =
    "ProductionWeaponFormArtBaselineView@1";
pub const PRODUCTION_WEAPON_FORM_ART_BASELINE_PREPARE_REQUEST_SCHEMA_VERSION: &str =
    "ProductionWeaponFormArtBaselinePrepareRequest@1";
pub const PRODUCTION_WEAPON_FORM_ART_BASELINE_PREPARE_RESULT_SCHEMA_VERSION: &str =
    "ProductionWeaponFormArtBaselinePrepareResult@1";
pub const PRODUCTION_WEAPON_FORM_ART_BASELINE_GET_REQUEST_SCHEMA_VERSION: &str =
    "ProductionWeaponFormArtBaselineGetRequest@1";
pub const PRODUCTION_WEAPON_FORM_ART_BASELINE_GET_RESULT_SCHEMA_VERSION: &str =
    "ProductionWeaponFormArtBaselineGetResult@1";

pub const PRODUCTION_WEAPON_FORM_ART_BASELINE_PREPARE_OPERATION: &str =
    "forgecad.production.weapon.form-art-baseline-prepare@1";
pub const PRODUCTION_WEAPON_FORM_ART_BASELINE_GET_OPERATION: &str =
    "forgecad.production.weapon.form-art-baseline-get@1";
pub const PRODUCTION_WEAPON_FORM_ART_BASELINE_POLICY: &str =
    "fresh-same-cohort-form-art-baseline-registration-lineage-rig-v2@1";
pub const PRODUCTION_WEAPON_FORM_ART_BASELINE_MATERIALIZATION_STATUS: &str =
    "runtime-owned-durable-form-art-baseline@1";
pub const PRODUCTION_WEAPON_FORM_ART_BASELINE_IDEMPOTENCY_POLICY: &str =
    "same-input-hash-replays-without-new-record@1";
pub const PRODUCTION_WEAPON_FORM_ART_BASELINE_WRITER_POLICY: &str =
    "forgecad-runtime-only-state-writer@1";
pub const PRODUCTION_WEAPON_FORM_ART_BASELINE_CANONICALIZATION_POLICY: &str =
    "canonical-json-sha256-excluding-canonical-sha256@1";
pub const PRODUCTION_WEAPON_FORM_ART_BASELINE_MAX_RESPONSE_BYTES: u64 = 1_048_576;
pub const PRODUCTION_WEAPON_FORM_ART_BASELINE_QUALITY_STATUS: &str = "NOT_PROVEN";
pub const PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_KINDS: [&str; 6] = [
    "front",
    "back",
    "left",
    "right",
    "top",
    "rear-three-quarter",
];
pub const PRODUCTION_WEAPON_FORM_ART_BASELINE_AOV_KINDS: [&str; 9] = [
    "beauty",
    "silhouette",
    "depth",
    "normal",
    "ao",
    "part-id",
    "material-id",
    "wireframe",
    "uv-stretch",
];

/// One fixed review view.  The renderer cohort is part of the view binding so
/// a baseline cannot silently combine RenderSets from different workers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionWeaponFormArtBaselineView {
    pub schema_version: String,
    pub view_kind: String,
    pub view_id: String,
    pub reference_id: String,
    pub reference_sha256: String,
    pub camera_hash: String,
    pub camera_canonical_sha256: String,
    pub camera_object_sha256: String,
    pub render_set_id: String,
    pub render_set_object_sha256: String,
    pub render_set_canonical_sha256: String,
    pub render_set_view_id: String,
    /// Exact CAS hashes for the fixed nine AOVs, in
    /// `PRODUCTION_WEAPON_FORM_ART_BASELINE_AOV_KINDS` order.
    pub pass_artifact_object_sha256: Vec<String>,
    pub reference_mask_object_sha256: String,
    pub comparison_report_object_sha256: String,
    pub quality_report_object_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub quality_status: String,
    pub receipt_object_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Runtime-owned immutable baseline parent.  `views` must be the ordered six
/// view set declared by `PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_KINDS`.
/// All quality/promotion flags are deliberately closed at the evidence
/// boundary and cannot be used to create a Stage transition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionWeaponFormArtBaselineRecord {
    pub schema_version: String,
    pub baseline_id: String,
    pub registration_lineage_id: String,
    pub registration_lineage_canonical_sha256: String,
    pub registration_lineage_receipt_object_sha256: String,
    pub registered_rig_v2_id: String,
    pub registered_rig_v2_object_sha256: String,
    pub registered_rig_v2_canonical_sha256: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub base_version_id: Option<String>,
    pub view_kinds: Vec<String>,
    pub views: Vec<ProductionWeaponFormArtBaselineView>,
    pub runtime_build_cohort_sha256: String,
    pub baseline_policy: String,
    pub materialization_status: String,
    pub historical_form_art_reused: bool,
    pub worker_started: bool,
    pub worker_cohort_verified: bool,
    pub quality_status: String,
    pub visual_status: String,
    pub human_status: String,
    pub engine_status: String,
    pub distribution_status: String,
    pub promotion_eligible: bool,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub request_sha256: String,
    pub input_sha256: String,
    pub idempotency_key: String,
    pub idempotency_policy: String,
    pub writer_policy: String,
    pub receipt_object_sha256: String,
    pub canonicalization_policy: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Alias matching the schema title for callers that do not use the explicit
/// `Record` suffix.
pub type ProductionWeaponFormArtBaseline = ProductionWeaponFormArtBaselineRecord;

/// Prepare is a narrow source binding.  Runtime resolves RigV2, camera,
/// RenderSet and all six views from the registration lineage; callers cannot
/// inject those objects, image bytes, paths or scripts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionWeaponFormArtBaselinePrepareRequest {
    pub schema_version: String,
    pub operation: String,
    pub baseline_id: String,
    pub registration_lineage_id: String,
    pub registration_lineage_canonical_sha256: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub base_version_id: Option<String>,
    pub idempotency_key: String,
    pub max_response_bytes: u64,
    pub writer_policy: String,
    pub canonicalization_policy: String,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub input_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionWeaponFormArtBaselinePrepareResult {
    pub schema_version: String,
    pub operation: String,
    pub baseline: ProductionWeaponFormArtBaselineRecord,
    pub baseline_id: String,
    pub registration_lineage_id: String,
    pub registration_lineage_canonical_sha256: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub runtime_build_cohort_sha256: String,
    pub request_sha256: String,
    pub request_input_sha256: String,
    pub idempotency_key: String,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub writer_policy: String,
    pub canonicalization_policy: String,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub promotion_eligible: bool,
    pub quality_status: String,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionWeaponFormArtBaselineGetRequest {
    pub schema_version: String,
    pub operation: String,
    pub baseline_id: String,
    pub registration_lineage_id: String,
    pub registration_lineage_canonical_sha256: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub base_version_id: Option<String>,
    pub idempotency_key: String,
    pub max_response_bytes: u64,
    pub writer_policy: String,
    pub canonicalization_policy: String,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub input_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionWeaponFormArtBaselineGetResult {
    pub schema_version: String,
    pub operation: String,
    pub baseline: ProductionWeaponFormArtBaselineRecord,
    pub baseline_id: String,
    pub registration_lineage_id: String,
    pub registration_lineage_canonical_sha256: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub runtime_build_cohort_sha256: String,
    pub request_sha256: String,
    pub request_input_sha256: String,
    pub idempotency_key: String,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub writer_policy: String,
    pub canonicalization_policy: String,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub promotion_eligible: bool,
    pub quality_status: String,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub canonical_sha256: String,
}
