//! Closed contracts for the additive, Part-bounded AuthoringMesh V2 genesis
//! derived from one persisted WeaponFoundation import.
//!
//! The source import remains the only input surface: callers provide project
//! scope and hashes for the already persisted foundation request/result,
//! topology, socket map, rig map and presentation package.  Runtime derives
//! every mesh/lineage/revision identity.  No path, URL, bytes, script, mesh
//! array, candidate, version or export state is representable here.

use serde::{Deserialize, Serialize};

pub const AUTHORING_MESH_V2_FOUNDATION_SOURCE_BINDING_SCHEMA_VERSION: &str =
    "AuthoringMeshV2FoundationSourceBinding@1";
pub const WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_PREPARE_REQUEST_SCHEMA_VERSION: &str =
    "WeaponFoundationAuthoringMaterializationPrepareRequest@1";
pub const WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_PREPARE_RESULT_SCHEMA_VERSION: &str =
    "WeaponFoundationAuthoringMaterializationPrepareResult@1";
pub const WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_GET_REQUEST_SCHEMA_VERSION: &str =
    "WeaponFoundationAuthoringMaterializationGetRequest@1";
pub const WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_GET_RESULT_SCHEMA_VERSION: &str =
    "WeaponFoundationAuthoringMaterializationGetResult@1";
pub const WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_RECORD_SCHEMA_VERSION: &str =
    "WeaponFoundationAuthoringMaterializationRecord@1";
pub const WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_DESCRIPTOR_SCHEMA_VERSION: &str =
    "WeaponFoundationAuthoringMaterializationDescriptor@1";

pub const WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_PROFILE: &str =
    "part-bounded-authoring-mesh-v2-genesis@1";
pub const WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_STATUS: &str =
    "runtime-owned-durable-authoring-mesh-v2-foundation@1";
pub const WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_QUALITY_STATUS: &str = "structural_only";
pub const WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_REVIEW_STATUS: &str = "DRAFT_UNREVIEWED";
pub const WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_RESPONSE_SHAPE: &str =
    "hash-only-per-part-summary-no-inline-topology@1";
pub const WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_WRITER_POLICY: &str =
    "forgecad-runtime-only-state-writer@1";
pub const WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_STORAGE_POLICY: &str =
    "runtime-owned-sqlite-cas-per-part-authoring-mesh-v2@1";
pub const WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_CANONICALIZATION_POLICY: &str =
    "canonical-json-sha256-excluding-canonical-sha256@1";
pub const WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_MAX_RESPONSE_BYTES: u64 = 1_048_576;
pub const WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_MAX_PARTS: u32 = 128;
pub const WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_SOURCE_ONLY: bool = true;
pub const WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_LIMITATIONS: &[&str] = &[
    "RUNTIME_SOLE_WRITER",
    "NO_CANDIDATE_BINDING",
    "NO_VERSION",
    "NO_EXPORT",
    "NO_PROMOTION",
    "DRAFT_UNREVIEWED",
    "STRUCTURAL_ONLY",
    "NO_INLINE_TOPOLOGY",
    "HASH_ONLY_SOURCE_DESCRIPTOR",
    "PART_BOUNDED_ONE_IMPORT_PER_CALL",
];

/// Runtime-derived source lineage for one materialized Part.  The binding
/// carries only hashes/opaque ids and the Part's semantic material zone; it
/// never embeds the AuthoringMesh topology.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshV2FoundationSourceBinding {
    pub schema_version: String,
    pub project_id: String,
    pub materialization_id: String,
    pub record_id: String,
    pub foundation_request_id: String,
    pub foundation_request_sha256: String,
    pub foundation_result_object_sha256: String,
    pub topology_object_sha256: String,
    pub socket_map_object_sha256: String,
    pub rig_map_object_sha256: String,
    pub fps_presentation_package_object_sha256: String,
    pub source_asset_id: String,
    pub source_asset_sha256: String,
    pub source_asset_role: String,
    pub part_id: String,
    pub material_zone_id: String,
    pub source_part_topology_sha256: String,
    pub authoring_mesh_id: String,
    pub authoring_mesh_lineage_id: String,
    pub authoring_mesh_revision_id: String,
    pub binding_policy: String,
    pub materialization_profile: String,
    pub source_only: bool,
    pub quality_status: String,
    pub review_status: String,
    pub canonicalization_policy: String,
    pub canonical_sha256: String,
}

/// Compact per-Part read model.  The actual topology is a separate CAS
/// object and is addressed only by hashes in the durable binding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WeaponFoundationAuthoringMaterializationPartSummary {
    pub part_id: String,
    pub material_zone_id: String,
    pub source_part_topology_sha256: String,
    pub authoring_mesh_id: String,
    pub authoring_mesh_object_sha256: String,
    pub authoring_mesh_sha256: String,
    pub authoring_mesh_lineage_id: String,
    pub authoring_mesh_lineage_sha256: String,
    pub authoring_mesh_revision_id: String,
    pub authoring_mesh_revision_sha256: String,
    pub source_binding_sha256: String,
    pub vertex_count: u32,
    pub edge_count: u32,
    pub half_edge_count: u32,
    pub corner_count: u32,
    pub face_count: u32,
    pub loop_count: u32,
    pub ring_count: u32,
    pub source_triangle_count: u32,
    pub sanitized_triangle_count: u32,
}

/// Compact CAS root for the complete Part revision set.  It carries only
/// identities, hashes and counts; authored topology remains in the listed
/// AuthoringMesh revision objects.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WeaponFoundationAuthoringMaterializationDescriptorPartRevision {
    pub part_id: String,
    pub mesh_id: String,
    pub lineage_id: String,
    pub revision_id: String,
    pub idempotency_key: String,
    pub revision_object_sha256: String,
    pub revision_sha256: String,
    pub vertex_count: u64,
    pub face_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WeaponFoundationAuthoringMaterializationDescriptor {
    pub schema_version: String,
    pub project_id: String,
    pub foundation_request_id: String,
    pub foundation_request_sha256: String,
    pub foundation_result_object_sha256: String,
    pub foundation_topology_object_sha256: String,
    pub foundation_socket_map_object_sha256: String,
    pub foundation_rig_map_object_sha256: String,
    pub foundation_fps_presentation_package_object_sha256: String,
    pub part_revisions: Vec<WeaponFoundationAuthoringMaterializationDescriptorPartRevision>,
    pub part_revision_summary_sha256: String,
    pub part_count: u64,
    pub vertex_count: u64,
    pub face_count: u64,
    pub status: String,
    pub canonical_sha256: String,
}

/// Runtime-owned durable index for one import and all of its Part-bounded
/// AuthoringMesh V2 genesis objects.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WeaponFoundationAuthoringMaterializationRecord {
    pub schema_version: String,
    pub record_id: String,
    pub materialization_id: String,
    pub project_id: String,
    pub descriptor_object_sha256: String,
    pub descriptor_sha256: String,
    pub foundation_request_id: String,
    pub foundation_request_sha256: String,
    pub foundation_result_object_sha256: String,
    pub topology_object_sha256: String,
    pub socket_map_object_sha256: String,
    pub rig_map_object_sha256: String,
    pub fps_presentation_package_object_sha256: String,
    pub source_asset_id: String,
    pub source_asset_sha256: String,
    pub source_asset_role: String,
    pub materialization_profile: String,
    pub source_only: bool,
    pub part_count: u32,
    pub parts: Vec<WeaponFoundationAuthoringMaterializationPartSummary>,
    pub materialization_status: String,
    pub quality_status: String,
    pub review_status: String,
    pub storage_policy: String,
    pub writer_policy: String,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub request_input_sha256: String,
    pub idempotency_key: String,
    pub limitations: Vec<String>,
    pub canonicalization_policy: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// The prepare request is a closed source-hash envelope.  Runtime resolves
/// the asset role and Part list from the persisted foundation objects.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WeaponFoundationAuthoringMaterializationPrepareRequest {
    pub schema_version: String,
    pub project_id: String,
    pub foundation_request_id: String,
    pub foundation_request_sha256: String,
    pub foundation_result_object_sha256: String,
    pub topology_object_sha256: String,
    pub socket_map_object_sha256: String,
    pub rig_map_object_sha256: String,
    pub fps_presentation_package_object_sha256: String,
    pub materialization_profile: String,
    pub idempotency_key: String,
    pub max_response_bytes: u64,
    pub runtime_write_performed: bool,
    pub writer_policy: String,
    pub canonicalization_policy: String,
    pub input_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WeaponFoundationAuthoringMaterializationPrepareResult {
    pub schema_version: String,
    pub project_id: String,
    pub materialization_id: String,
    pub descriptor_object_sha256: String,
    pub descriptor_sha256: String,
    pub descriptor: WeaponFoundationAuthoringMaterializationDescriptor,
    pub record_sha256: String,
    pub record: WeaponFoundationAuthoringMaterializationRecord,
    pub request_input_sha256: String,
    pub idempotency_key: String,
    pub max_response_bytes: u64,
    pub materialization_profile: String,
    pub source_only: bool,
    pub materialization_status: String,
    pub quality_status: String,
    pub review_status: String,
    pub response_shape: String,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub writer_policy: String,
    pub limitations: Vec<String>,
    pub canonicalization_policy: String,
    pub canonical_sha256: String,
}

/// Read key for a Runtime-owned materialization.  The descriptor hash is an
/// optional caller-provided consistency check; Runtime remains authoritative.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WeaponFoundationAuthoringMaterializationGetRequest {
    pub schema_version: String,
    pub project_id: String,
    pub materialization_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descriptor_sha256: Option<String>,
    pub writer_policy: String,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub input_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WeaponFoundationAuthoringMaterializationGetResult {
    pub schema_version: String,
    pub project_id: String,
    pub materialization_id: String,
    pub descriptor_object_sha256: String,
    pub descriptor_sha256: String,
    pub descriptor: WeaponFoundationAuthoringMaterializationDescriptor,
    pub record_sha256: String,
    pub record: WeaponFoundationAuthoringMaterializationRecord,
    pub request_input_sha256: String,
    pub max_response_bytes: u64,
    pub materialization_profile: String,
    pub source_only: bool,
    pub materialization_status: String,
    pub quality_status: String,
    pub review_status: String,
    pub response_shape: String,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub writer_policy: String,
    pub limitations: Vec<String>,
    pub canonicalization_policy: String,
    pub canonical_sha256: String,
}
