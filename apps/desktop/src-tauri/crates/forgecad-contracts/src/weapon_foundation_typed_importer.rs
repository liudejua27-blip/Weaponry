//! Contracts for the allowlisted FPS foundation typed importer.
//!
//! These records deliberately carry identities, hashes, semantic mappings and
//! bounded readback summaries only.  Mesh positions/faces remain opaque CAS
//! data and AuthoringMesh materialization is explicitly pending until a later
//! Runtime-owned stage.

use serde::{Deserialize, Serialize};

pub const WEAPON_FOUNDATION_ASSET_REQUEST_SCHEMA_VERSION: &str = "WeaponFoundationAssetRequest@1";
pub const WEAPON_FOUNDATION_ASSET_RESULT_SCHEMA_VERSION: &str = "WeaponFoundationAssetResult@1";
pub const WEAPON_FOUNDATION_IMPORT_RECORD_SCHEMA_VERSION: &str = "WeaponFoundationImportRecord@1";
pub const FORGECAD_FOUNDATION_TOPOLOGY_SCHEMA_VERSION: &str = "ForgeCadFoundationTopology@1";
pub const WEAPON_FOUNDATION_COORDINATE_SPEC_SCHEMA_VERSION: &str =
    "WeaponFoundationCoordinateSpec@1";
pub const WEAPON_FOUNDATION_SOCKET_MAP_SCHEMA_VERSION: &str = "WeaponFoundationSocketMap@1";
pub const WEAPON_FOUNDATION_RIG_MAP_SCHEMA_VERSION: &str = "WeaponFoundationRigMap@1";
pub const FPS_PRESENTATION_PACKAGE_SCHEMA_VERSION: &str = "FpsPresentationPackage@1";

pub const WEAPON_FOUNDATION_PACK_ID: &str = "forgecad-fps-production-foundation";
pub const WEAPON_FOUNDATION_PACK_VERSION: &str = "0.1.0-proposal";
pub const WEAPON_FOUNDATION_PACK_MANIFEST_SHA256: &str =
    "cc7dccca305a1d9bbaf1df80e78e9cab6b2ee39f12de7ffc88d5cf52194330cb";
pub const WEAPON_FOUNDATION_COORDINATE_FRAME: &str = "weapon-right-handed-x-muzzle-y-up-z-right";
pub const WEAPON_FOUNDATION_UNITS: &str = "meter";
pub const WEAPON_FOUNDATION_SOURCE_TO_TARGET_AXIS_MAPPING: &[&str] = &["-Z", "+Y", "+X"];
pub const WEAPON_FOUNDATION_SOURCE_TO_TARGET_AXIS_MAPPING_ALTERNATE: &[&str] = &["+Z", "+Y", "-X"];
pub const WEAPON_FOUNDATION_DEGENERATE_FACE_EPSILON_M2: f64 = 1e-12;
pub const WEAPON_FOUNDATION_AUTHORING_MESH_STATUS: &str = "PENDING";

/// Asset ids admitted by the frozen evaluation-only foundation pack.  The
/// importer accepts ids/hashes from this list, never a path, URL or payload.
pub const WEAPON_FOUNDATION_ASSET_IDS: &[&str] =
    &["pichuliru-weapon-west", "wrad-arms", "lightning-low-pbr"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponFoundationCoordinateSpec {
    pub schema_version: String,
    pub source_asset_id: String,
    pub coordinate_frame: String,
    pub handedness: String,
    pub units: String,
    pub forward_axis: String,
    pub up_axis: String,
    pub right_axis: String,
    pub source_to_target: WeaponFoundationSourceToTarget,
    pub transform_convention: String,
    pub finite_value_policy: String,
    pub canonicalization_policy: String,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponFoundationSourceToTarget {
    pub mapping_evidence: String,
    pub axis_mapping: Vec<String>,
    pub matrix_row_major: [[i8; 3]; 3],
    pub translation_m: [f64; 3],
    pub scale_xyz: [f64; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponFoundationDegenerateFacePolicy {
    pub policy: String,
    pub test: String,
    pub area_epsilon_m2: f64,
    pub area_comparison: String,
    pub ordering: String,
    pub reindexing: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponFoundationImportBudgets {
    pub max_source_nodes: u32,
    pub max_source_meshes: u32,
    pub max_source_triangles: u32,
    pub max_cas_objects: u32,
    pub max_wire_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponFoundationAssetRequest {
    pub schema_version: String,
    pub request_id: String,
    pub foundation_pack_id: String,
    pub foundation_pack_version: String,
    pub foundation_manifest_sha256: String,
    pub asset_id: String,
    pub asset_sha256: String,
    pub asset_role: String,
    pub source_format: String,
    pub coordinate_spec_sha256: String,
    pub coordinate_frame: String,
    pub units: String,
    pub source_to_target: WeaponFoundationSourceToTarget,
    pub import_profile: String,
    pub strict_readback_policy: String,
    pub degenerate_face_policy: WeaponFoundationDegenerateFacePolicy,
    pub budgets: WeaponFoundationImportBudgets,
    pub canonicalization_policy: String,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeCadFoundationTopology {
    pub schema_version: String,
    pub topology_id: String,
    pub source_asset_id: String,
    pub source_asset_sha256: String,
    pub source_format: String,
    pub coordinate_spec_sha256: String,
    pub normalization_policy: String,
    pub degenerate_face_sanitation: WeaponFoundationDegenerateFaceSanitation,
    pub source_node_count: u32,
    pub source_mesh_count: u32,
    pub source_primitive_count: u32,
    pub source_triangle_count: u32,
    pub sanitized_triangle_count: u32,
    pub parts: Vec<ForgeCadFoundationTopologyPart>,
    pub topology_cas_sha256: String,
    pub storage_policy: String,
    pub authoring_mesh_materialization_status: String,
    pub topology_status: String,
    pub quality_status: String,
    pub promotion_eligible: bool,
    pub canonicalization_policy: String,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeCadFoundationTopologyPart {
    pub part_id: String,
    pub source_node_name: String,
    pub source_node_index: u32,
    pub semantic_role: String,
    pub material_zone_id: String,
    pub source_primitive_count: u32,
    pub source_vertex_count: u32,
    pub source_triangle_count: u32,
    pub sanitized_triangle_count: u32,
    pub source_degenerate_face_count: u32,
    pub sanitized_degenerate_face_count: u32,
    pub part_topology_cas_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponFoundationDegenerateFaceSanitation {
    pub policy: String,
    pub test: String,
    pub area_epsilon_m2: f64,
    pub area_comparison: String,
    pub ordering: String,
    pub source_degenerate_face_count: u32,
    pub dropped_face_count: u32,
    pub remaining_degenerate_face_count: u32,
    pub stable_reindexing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponFoundationStrictReadback {
    pub status: String,
    pub policy: String,
    pub readback_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_node_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_mesh_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_primitive_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_triangle_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sanitized_triangle_count: Option<u32>,
    pub invalid_index_count: u32,
    pub non_finite_count: u32,
    pub external_reference_count: u32,
    pub remaining_degenerate_face_count: u32,
    pub semantic_metadata_exact: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponFoundationDeterministicReplay {
    pub policy: String,
    pub first_topology_sha256: String,
    pub repeat_topology_sha256: String,
    pub first_record_sha256: String,
    pub repeat_record_sha256: String,
    pub byte_exact: bool,
    pub metadata_exact: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponFoundationAssetResult {
    pub schema_version: String,
    pub result_id: String,
    pub request_id: String,
    pub request_sha256: String,
    pub foundation_pack_id: String,
    pub foundation_pack_version: String,
    pub foundation_manifest_sha256: String,
    pub asset_id: String,
    pub asset_sha256: String,
    pub asset_role: String,
    pub source_format: String,
    pub coordinate_spec_sha256: String,
    pub coordinate_frame: String,
    pub units: String,
    pub source_to_target: WeaponFoundationSourceToTarget,
    pub strict_readback: WeaponFoundationStrictReadback,
    pub degenerate_face_sanitation: WeaponFoundationDegenerateFaceSanitation,
    pub deterministic_replay: WeaponFoundationDeterministicReplay,
    pub topology_object_sha256: String,
    pub socket_map_object_sha256: String,
    pub rig_map_object_sha256: String,
    pub fps_presentation_package_object_sha256: String,
    pub authoring_mesh_materialization_status: String,
    pub socket_materialization_status: String,
    pub rig_materialization_status: String,
    pub presentation_materialization_status: String,
    pub import_status: String,
    pub quality_status: String,
    pub promotion_eligible: bool,
    pub runtime_write_performed: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub actual_engine_roundtrip: bool,
    pub human_review_status: String,
    pub canonicalization_policy: String,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponFoundationSocketMap {
    pub schema_version: String,
    pub socket_map_id: String,
    pub source_asset_id: String,
    pub source_asset_sha256: String,
    pub coordinate_spec_sha256: String,
    pub mapping_policy: String,
    pub target_socket_namespace: String,
    pub required_socket_ids: Vec<String>,
    pub source_missing_socket_ids: Vec<String>,
    pub mappings: Vec<WeaponFoundationSocketMapping>,
    pub transform_status: String,
    pub materialization_status: String,
    pub quality_status: String,
    pub promotion_eligible: bool,
    pub canonicalization_policy: String,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponFoundationSocketMapping {
    pub socket_id: String,
    pub target_node_name: String,
    pub source_node_name: Option<String>,
    pub source_presence: String,
    pub source_semantic: String,
    pub parent_target_id: String,
    pub local_transform_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponFoundationRigMap {
    pub schema_version: String,
    pub rig_map_id: String,
    pub weapon_asset_id: String,
    pub weapon_asset_sha256: String,
    pub arms_asset_id: String,
    pub arms_asset_sha256: String,
    pub coordinate_spec_sha256: String,
    pub socket_map_object_sha256: String,
    pub rig_policy: String,
    pub rig_type: String,
    pub weapon_root_candidates: Vec<String>,
    pub part_mappings: Vec<WeaponFoundationRigPartMapping>,
    pub arms_mapping: WeaponFoundationArmsMapping,
    pub rest_pose: WeaponFoundationRestPose,
    pub source_animation_clips: Vec<String>,
    pub required_target_clips: Vec<String>,
    pub skinning_status: String,
    pub materialization_status: String,
    pub quality_status: String,
    pub promotion_eligible: bool,
    pub canonicalization_policy: String,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponFoundationRigPartMapping {
    pub part_id: String,
    pub source_node_name: String,
    pub target_node_name: String,
    pub movement_class: String,
    pub source_presence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponFoundationArmsMapping {
    pub root: String,
    pub left_grip_candidate: String,
    pub right_grip_candidate: String,
    pub left_wrist_ik: String,
    pub right_wrist_ik: String,
    pub left_arm_target: String,
    pub right_arm_target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponFoundationRestPose {
    pub status: String,
    pub rest_pose_sha256: Option<String>,
    pub derivation_policy: String,
    pub source_transform_payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponFoundationImportRecord {
    pub schema_version: String,
    pub import_record_id: String,
    pub request_id: String,
    pub request_sha256: String,
    pub result_id: String,
    pub result_sha256: String,
    pub foundation_pack_id: String,
    pub foundation_pack_version: String,
    pub foundation_manifest_sha256: String,
    pub asset_id: String,
    pub asset_sha256: String,
    pub asset_role: String,
    pub source_format: String,
    pub coordinate_spec_sha256: String,
    pub coordinate_frame: String,
    pub units: String,
    pub source_to_target: WeaponFoundationSourceToTarget,
    pub strict_readback: WeaponFoundationStrictReadback,
    pub source_observation: WeaponFoundationSourceObservation,
    pub degenerate_face_sanitation: WeaponFoundationDegenerateFaceSanitation,
    pub deterministic_replay: WeaponFoundationDeterministicReplay,
    pub topology_object_sha256: String,
    pub socket_map_object_sha256: String,
    pub rig_map_object_sha256: String,
    pub fps_presentation_package_object_sha256: String,
    pub authoring_mesh_materialization_status: String,
    pub import_status: String,
    pub quality_status: String,
    pub promotion_eligible: bool,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub actual_engine_roundtrip: bool,
    pub human_review_status: String,
    pub limitations: Vec<String>,
    pub canonicalization_policy: String,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponFoundationSourceObservation {
    pub node_count: u32,
    pub mesh_count: u32,
    pub primitive_count: u32,
    pub triangle_count: u32,
    pub skin_count: u32,
    pub animation_clip_ids: Vec<String>,
    pub material_image_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FpsPresentationPackage {
    pub schema_version: String,
    pub package_id: String,
    pub foundation_import_record_sha256: String,
    pub source_asset_ids: Vec<String>,
    pub coordinate_spec_sha256: String,
    pub topology_object_sha256: String,
    pub socket_map_object_sha256: String,
    pub rig_map_object_sha256: String,
    pub presentation_policy: String,
    pub status: String,
    pub promotion_eligible: bool,
    pub authoring_mesh_materialization_status: String,
    pub required_clip_ids: Vec<String>,
    pub clips: Vec<FpsPresentationClip>,
    pub missing_clip_ids: Vec<String>,
    pub complete_required_clips: bool,
    pub required_event_marker_ids: Vec<String>,
    pub event_markers: Vec<FpsPresentationEventMarker>,
    pub missing_event_marker_ids: Vec<String>,
    pub complete_event_markers: bool,
    pub camera_profiles: Vec<FpsPresentationCameraProfile>,
    pub screen_occupancy: FpsPresentationMeasurementStatus,
    pub reticle_safe_region: FpsPresentationMeasurementStatus,
    pub muzzle_safe_region: FpsPresentationMeasurementStatus,
    pub hands_weapon_clipping_status: String,
    pub vfx_cues: Vec<FpsPresentationCue>,
    pub audio_cues: Vec<FpsPresentationCue>,
    pub gameplay_beats: Vec<FpsPresentationGameplayBeat>,
    pub complete_vfx_audio_timeline: bool,
    pub engine_validation_status: String,
    pub human_review_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub materialization_status: String,
    pub quality_status: String,
    pub runtime_write_performed: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub actual_engine_roundtrip: bool,
    pub canonicalization_policy: String,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FpsPresentationClip {
    pub clip_id: String,
    pub source_clip_id: Option<String>,
    pub status: String,
    pub clip_object_sha256: Option<String>,
    pub event_markers_complete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FpsPresentationEventMarker {
    pub marker_id: String,
    pub clip_id: String,
    pub status: String,
    pub time_ticks: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FpsPresentationCameraProfile {
    pub profile_id: String,
    pub status: String,
    pub camera_object_sha256: Option<String>,
    pub source_of_truth: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FpsPresentationMeasurementStatus {
    pub status: String,
    pub measurement_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FpsPresentationCue {
    pub cue_id: String,
    pub status: String,
    pub cue_object_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FpsPresentationGameplayBeat {
    pub beat_id: String,
    pub status: String,
    pub beat_object_sha256: Option<String>,
}
