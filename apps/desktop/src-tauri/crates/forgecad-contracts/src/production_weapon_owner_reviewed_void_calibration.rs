//! Closed, read-only owner-to-reviewed-void calibration projection.
//!
//! This contract is deliberately a projection rather than a geometry edit.
//! The request contains only references to Runtime-owned candidate, evidence,
//! baseline and camera-lineage records.  Runtime derives the `rear-stock`
//! owner, the reviewed void, the Part-ID/depth observations and the closed
//! transform comparison.  Raw masks, image bytes, vertex IDs and transforms
//! are not part of the public request.

use serde::{Deserialize, Serialize};

pub const PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_SCHEMA_VERSION: &str =
    "ProductionWeaponOwnerReviewedVoidCalibrationProjection@1";
pub const PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_VIEW_SCHEMA_VERSION: &str =
    "ProductionWeaponOwnerReviewedVoidCalibrationProjectionView@1";
pub const PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_GET_REQUEST_SCHEMA_VERSION:
    &str = "ProductionWeaponOwnerReviewedVoidCalibrationProjectionGetRequest@1";
pub const PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_GET_RESULT_SCHEMA_VERSION:
    &str = "ProductionWeaponOwnerReviewedVoidCalibrationProjectionGetResult@1";

pub const PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_GET_OPERATION: &str =
    "forgecad.production.weapon.owner-reviewed-void-calibration-projection-get@1";
pub const PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_POLICY: &str =
    "runtime-derived-registered-camera-owner-to-reviewed-void-calibration@1";
pub const PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_TRANSFORM_POLICY: &str =
    "runtime-derived-closed-part-id-review-region-transform@1";
pub const PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_REVIEWED_VOID_POLICY: &str =
    "reviewed-subtract-contour-intersection-with-candidate-silhouette@1";
pub const PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_DEPTH_POLICY: &str =
    "registered-camera-owner-void-depth-evidence@1";
pub const PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_THRESHOLD_POLICY: &str =
    "owner-reviewed-void-zero-intrusion-adjacency-thresholds@1";
pub const PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_WRITER_POLICY: &str =
    "forgecad-runtime-only-state-writer@1";
pub const PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_CANONICALIZATION_POLICY:
    &str = "canonical-json-sha256-excluding-canonical-sha256@1";
pub const PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_MAX_RESPONSE_BYTES: u64 =
    1_048_576;
pub const PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_OWNER_PART_ID: &str =
    "rear-stock";
pub const PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_VIEW_KINDS: [&str; 3] =
    ["left", "right", "rear-three-quarter"];
pub const PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_AOV_KINDS: [&str; 3] =
    ["silhouette", "part-id", "depth"];
pub const PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_QUALITY_STATUS: &str =
    "NOT_PROVEN";

/// Runtime-derived evidence for one fixed registered-camera view.  The hash
/// fields identify CAS objects or canonical projections; no pixel array or
/// image bytes cross the MCP boundary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionWeaponOwnerReviewedVoidCalibrationProjectionView {
    pub schema_version: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub artifact_readback_sha256: String,
    pub view_kind: String,
    pub view_id: String,
    pub reviewed_structure_id: String,
    pub reference_id: String,
    pub reference_sha256: String,
    pub camera_hash: String,
    pub camera_canonical_sha256: String,
    pub camera_object_sha256: String,
    pub render_set_object_sha256: String,
    pub render_set_canonical_sha256: String,
    pub render_set_view_id: String,
    pub form_art_view_receipt_object_sha256: String,
    pub form_art_view_receipt_canonical_sha256: String,
    pub baseline_view_receipt_object_sha256: String,
    pub target_object_sha256: String,
    pub target_canonical_sha256: String,
    pub visual_structure_canonical_sha256: String,
    pub silhouette_pass_object_sha256: String,
    pub part_id_pass_object_sha256: String,
    pub depth_pass_object_sha256: String,
    pub owner_part_id: String,
    pub derived_owner_region_sha256: String,
    pub derived_reviewed_void_region_sha256: String,
    pub derived_void_boundary_sha256: String,
    pub registered_camera_lineage_verified: bool,
    pub derived_transform_kind: String,
    pub identity_transform_unique: bool,
    pub eligible_transform_count: u64,
    pub transform_rank_tie: bool,
    pub expected_void_pixel_count: u64,
    pub owner_region_pixel_count: u64,
    pub owner_expected_void_overlap_pixel_count: u64,
    pub owner_expected_void_overlap_milli: u64,
    pub boundary_pixel_count: u64,
    pub owner_boundary_adjacency_pixel_count: u64,
    pub owner_boundary_adjacency_milli: u64,
    pub depth_valid_pixel_count: u64,
    pub depth_owner_sample_count: u64,
    pub depth_boundary_sample_count: u64,
    pub depth_invalid_sample_count: u64,
    pub depth_ordering_milli: i64,
    pub depth_status: String,
    pub owner_void_status: String,
    pub strict_owner_void_passed: bool,
    pub strict_depth_passed: bool,
    pub view_status: String,
    pub view_passed: bool,
    pub blocker_codes: Vec<String>,
    pub quality_status: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Immutable parent projection.  `view_kinds` and `views` are closed to the
/// three owner-bearing registered cameras; Runtime, not the caller, chooses
/// the owner Part, reviewed structure IDs and transform candidate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionWeaponOwnerReviewedVoidCalibrationProjection {
    pub schema_version: String,
    pub projection_id: String,
    pub operation: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub artifact_readback_sha256: String,
    pub form_art_evidence_id: String,
    pub form_art_evidence_object_sha256: String,
    pub form_art_evidence_canonical_sha256: String,
    pub fresh_baseline_id: String,
    pub fresh_baseline_canonical_sha256: String,
    pub fresh_baseline_receipt_object_sha256: String,
    pub registration_lineage_id: String,
    pub registration_lineage_canonical_sha256: String,
    pub registration_lineage_receipt_object_sha256: String,
    pub registered_rig_v2_id: String,
    pub registered_rig_v2_object_sha256: String,
    pub registered_rig_v2_canonical_sha256: String,
    pub runtime_build_cohort_sha256: String,
    pub owner_part_id: String,
    pub view_kinds: Vec<String>,
    pub views: Vec<ProductionWeaponOwnerReviewedVoidCalibrationProjectionView>,
    pub calibration_policy: String,
    pub calibration_policy_sha256: String,
    pub transform_policy: String,
    pub reviewed_void_policy: String,
    pub depth_policy: String,
    pub depth_policy_sha256: String,
    pub threshold_policy: String,
    pub threshold_policy_sha256: String,
    pub calibration_status: String,
    pub blocker_codes: Vec<String>,
    pub strict_owner_void_all_views_passed: bool,
    pub strict_depth_all_views_passed: bool,
    pub identity_transform_all_views_unique: bool,
    pub all_views_passed: bool,
    pub eligible: bool,
    pub promotable: bool,
    pub quality_status: String,
    pub depth_status: String,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub worker_started: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub request_sha256: String,
    pub input_sha256: String,
    pub canonicalization_policy: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Zero-write request.  Every source is an opaque Runtime-owned reference;
/// caller-provided masks, image bytes, vertex IDs and transforms are
/// intentionally unrepresentable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionWeaponOwnerReviewedVoidCalibrationProjectionGetRequest {
    pub schema_version: String,
    pub operation: String,
    pub projection_id: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub artifact_readback_sha256: String,
    pub form_art_evidence_id: String,
    pub form_art_evidence_object_sha256: String,
    pub form_art_evidence_canonical_sha256: String,
    pub fresh_baseline_id: String,
    pub fresh_baseline_canonical_sha256: String,
    pub fresh_baseline_receipt_object_sha256: String,
    pub registration_lineage_id: String,
    pub registration_lineage_canonical_sha256: String,
    pub registration_lineage_receipt_object_sha256: String,
    pub registered_rig_v2_id: String,
    pub registered_rig_v2_object_sha256: String,
    pub registered_rig_v2_canonical_sha256: String,
    pub max_response_bytes: u64,
    pub writer_policy: String,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub input_sha256: String,
}

/// Read result for the projection.  A blocked projection remains a valid
/// diagnostic result, but can never be interpreted as a quality or promotion
/// pass.  The result itself performs no Runtime/Store/Worker side effect.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionWeaponOwnerReviewedVoidCalibrationProjectionGetResult {
    pub schema_version: String,
    pub operation: String,
    pub projection_id: String,
    pub projection: ProductionWeaponOwnerReviewedVoidCalibrationProjection,
    pub request_sha256: String,
    pub request_input_sha256: String,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub writer_policy: String,
    pub runtime_write: bool,
    pub persistent_user_data_touched: bool,
    pub worker_started: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub quality_status: String,
    pub depth_status: String,
    pub canonicalization_policy: String,
    pub canonical_sha256: String,
}
