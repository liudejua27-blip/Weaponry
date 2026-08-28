//! Typed contracts for the Runtime-owned explicit Low quad-draft source slice.
//!
//! This module is intentionally additive and is not wired into the public
//! contract root by this change.  The Runtime/MCP integration lane should add
//! `mod low_quad_durable;` and re-export the types after reviewing the frozen
//! names.  The existing triangle edge-collapse contracts remain independent.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const LOW_QUAD_DRAFT_DURABLE_PREPARE_SCHEMA_VERSION: &str =
    "LowQuadDraftDurablePrepareRequest@1";
pub const LOW_QUAD_DRAFT_DURABLE_GET_SCHEMA_VERSION: &str = "LowQuadDraftDurableGetRequest@1";
pub const LOW_QUAD_DRAFT_DURABLE_PREPARE_RESULT_SCHEMA_VERSION: &str =
    "LowQuadDraftDurablePrepareResult@1";
pub const LOW_QUAD_DRAFT_DURABLE_GET_RESULT_SCHEMA_VERSION: &str = "LowQuadDraftDurableGetResult@1";
pub const LOW_QUAD_DRAFT_DURABLE_LINK_SCHEMA_VERSION: &str = "LowQuadDraftDurableLink@1";
pub const LOW_QUAD_DRAFT_DURABLE_RECORD_SCHEMA_VERSION: &str = "LowQuadDraftDurableRecord@1";
pub const LOW_QUAD_DRAFT_DURABLE_OPERATION_PREPARE: &str =
    "forgecad.production.low-quad-draft-durable-prepare@1";
pub const LOW_QUAD_DRAFT_DURABLE_OPERATION_GET: &str =
    "forgecad.production.low-quad-draft-durable-get@1";
pub const LOW_QUAD_DRAFT_DURABLE_POLICY: &str = "runtime-owned-explicit-quad-draft-source-only@1";
pub const LOW_QUAD_DRAFT_DURABLE_OBJECT_KIND: &str = "low-quad-draft-durable-link";
pub const LOW_QUAD_DRAFT_DURABLE_WORKER_RESULT_KIND: &str = "low-quad-draft-worker-result";
pub const LOW_QUAD_DRAFT_DURABLE_ARTIFACT_KIND: &str = "production-weapon-low-quad-draft-glb";
pub const LOW_QUAD_DRAFT_DURABLE_READBACK_KIND: &str = "low-quad-draft-artifact-readback";
pub const LOW_QUAD_DRAFT_DURABLE_ARTIFACT_READBACK_SCHEMA_VERSION: &str =
    "LowQuadDraftArtifactReadback@1";
pub const LOW_QUAD_DRAFT_DURABLE_WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
pub const LOW_QUAD_DRAFT_DURABLE_CANONICALIZATION_POLICY: &str =
    "canonical-json-sha256-excluding-canonical-sha256@1";
pub const LOW_QUAD_DRAFT_DURABLE_MAX_RESPONSE_BYTES: u64 = 1_048_576;
pub const LOW_QUAD_DRAFT_DURABLE_MAX_JSON_BYTES: u64 = 8 * 1024 * 1024;
pub const LOW_QUAD_DRAFT_DURABLE_MAX_GLB_BYTES: u64 = 64 * 1024 * 1024;

pub const LOW_QUAD_DRAFT_DURABLE_LIMITATIONS: &[&str] = &[
    "RUNTIME_SOLE_WRITER",
    "NO_STAGE_ADVANCEMENT",
    "NO_CANDIDATE_CONFIRM",
    "NO_VERSION_CREATED",
    "NO_EXPORT",
    "DRAFT_UNREVIEWED",
    "STRUCTURAL_ONLY_NOT_COMMERCIAL_QUALITY",
    "PROMOTION_INELIGIBLE",
    "DOES_NOT_REPLACE_TRIANGLE_EDGE_COLLAPSE",
];

/// The nested value is the already closed `LowQuadDraftWorkerRequest@1`.
/// Keeping it as a value here avoids duplicating the Worker topology contract
/// and lets the Runtime perform the exact Worker-owned validation before any
/// CAS write.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LowQuadDraftDurablePrepareRequest {
    pub schema_version: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub base_version_id: Option<String>,
    pub source_high_artifact_id: String,
    pub source_high_artifact_object_sha256: String,
    pub source_high_artifact_sha256: String,
    pub source_high_artifact_readback_object_sha256: String,
    pub source_high_artifact_readback_sha256: String,
    pub low_quad_draft_worker_request: Value,
    pub low_quad_draft_worker_request_sha256: String,
    pub idempotency_key: String,
    pub max_response_bytes: u64,
    pub source_only: bool,
    pub runtime_write_performed: bool,
    pub writer_policy: String,
    pub canonicalization_policy: String,
    pub input_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LowQuadDraftDurableGetRequest {
    pub schema_version: String,
    pub operation: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub base_version_id: Option<String>,
    pub link_id: String,
    pub link_object_sha256: String,
    pub source_high_artifact_id: String,
    pub source_high_artifact_sha256: String,
    pub worker_result_object_sha256: String,
    pub worker_result_sha256: String,
    pub artifact_object_sha256: String,
    pub artifact_sha256: String,
    pub readback_object_sha256: String,
    pub readback_sha256: String,
    pub idempotency_key: String,
    pub source_only: bool,
    pub writer_policy: String,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub input_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LowQuadDraftDurableLink {
    pub schema_version: String,
    pub operation: String,
    pub link_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub base_version_id: Option<String>,
    pub source_high_artifact_id: String,
    pub source_high_artifact_object_sha256: String,
    pub source_high_artifact_sha256: String,
    pub source_high_artifact_readback_object_sha256: String,
    pub source_high_artifact_readback_sha256: String,
    pub worker_result_object_sha256: String,
    pub worker_result_sha256: String,
    pub artifact_object_sha256: String,
    pub artifact_sha256: String,
    pub artifact_size_bytes: u64,
    pub readback_object_sha256: String,
    pub readback_sha256: String,
    pub low_retopology_policy: String,
    pub edge_flow_status: String,
    pub quality_status: String,
    pub visual_status: String,
    pub human_status: String,
    pub engine_status: String,
    pub distribution_status: String,
    pub validator_status: String,
    pub hard_gate_passed: bool,
    pub explicit_quad_faces: bool,
    pub auto_retopology_performed: bool,
    pub retopology_derived: bool,
    pub artist_authored_quad_topology: bool,
    pub promotion_eligible: bool,
    pub runtime_write_performed: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub writer_policy: String,
    pub materialization_status: String,
    pub limitations: Vec<String>,
    pub request_sha256: String,
    pub input_sha256: String,
    pub idempotency_key: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Store-local row.  It intentionally keeps CAS object hashes separate from
/// payload/canonical hashes so a payload can never create a self-hash fixed
/// point and restart verification can check both layers independently.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LowQuadDraftDurableRecord {
    pub schema_version: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub base_version_id: Option<String>,
    pub source_high_artifact_id: String,
    pub source_high_artifact_object_sha256: String,
    pub source_high_artifact_sha256: String,
    pub source_high_artifact_readback_object_sha256: String,
    pub source_high_artifact_readback_sha256: String,
    pub worker_result_object_sha256: String,
    pub worker_result_sha256: String,
    pub artifact_object_sha256: String,
    pub artifact_sha256: String,
    pub artifact_size_bytes: u64,
    pub readback_object_sha256: String,
    pub readback_sha256: String,
    pub link_id: String,
    pub link_object_sha256: String,
    pub request_sha256: String,
    pub input_sha256: String,
    pub idempotency_key: String,
    pub worker_build_cohort_sha256: String,
    pub materialization_status: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LowQuadDraftDurablePrepareResult {
    pub schema_version: String,
    pub operation: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub base_version_id: Option<String>,
    pub link_id: String,
    pub link_object_sha256: String,
    pub durable_link: LowQuadDraftDurableLink,
    pub worker_result_object_sha256: String,
    pub worker_result_sha256: String,
    pub artifact_object_sha256: String,
    pub artifact_sha256: String,
    pub readback_object_sha256: String,
    pub readback_sha256: String,
    pub request_input_sha256: String,
    pub idempotency_key: String,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub production_stage_advanced: bool,
    pub promotion_eligible: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub quality_status: String,
    pub edge_flow_status: String,
    pub limitations: Vec<String>,
    pub canonicalization_policy: String,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LowQuadDraftDurableGetResult {
    pub schema_version: String,
    pub operation: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub base_version_id: Option<String>,
    pub link_id: String,
    pub link_object_sha256: String,
    pub durable_link: LowQuadDraftDurableLink,
    pub worker_result_object_sha256: String,
    pub worker_result_sha256: String,
    pub artifact_object_sha256: String,
    pub artifact_sha256: String,
    pub readback_object_sha256: String,
    pub readback_sha256: String,
    pub request_input_sha256: String,
    pub idempotency_key: String,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub production_stage_advanced: bool,
    pub promotion_eligible: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub quality_status: String,
    pub edge_flow_status: String,
    pub limitations: Vec<String>,
    pub canonicalization_policy: String,
    pub canonical_sha256: String,
}
