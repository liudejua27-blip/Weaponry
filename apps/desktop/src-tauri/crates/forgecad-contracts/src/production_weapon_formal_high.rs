//! Closed public contracts for Runtime-owned Formal High materialization.
//!
//! The public prepare request is intentionally a narrow source-transition
//! capability boundary.  Runtime resolves session/project, the source
//! candidate, and every High artifact/readback/receipt hash from the durable
//! transition and CAS lineage.  Callers can select a distinct High candidate,
//! but cannot supply any Runtime-derived output identity.

use crate::{CandidateRecord, ProductionWeaponHighArtifactRecord};
use serde::{Deserialize, Serialize};

pub const PRODUCTION_WEAPON_FORMAL_HIGH_PREPARE_REQUEST_SCHEMA_VERSION: &str =
    "ProductionWeaponFormalHighPrepareRequest@1";
pub const PRODUCTION_WEAPON_FORMAL_HIGH_PREPARE_RESULT_SCHEMA_VERSION: &str =
    "ProductionWeaponFormalHighPrepareResult@1";
pub const PRODUCTION_WEAPON_FORMAL_HIGH_GET_REQUEST_SCHEMA_VERSION: &str =
    "ProductionWeaponFormalHighGetRequest@1";
pub const PRODUCTION_WEAPON_FORMAL_HIGH_GET_RESULT_SCHEMA_VERSION: &str =
    "ProductionWeaponFormalHighGetResult@1";

pub const PRODUCTION_WEAPON_FORMAL_HIGH_WRITER_POLICY: &str =
    "forgecad-runtime-only-state-writer@1";
pub const PRODUCTION_WEAPON_FORMAL_HIGH_MAX_RESPONSE_BYTES: u64 = 1_048_576;

/// The only caller-owned Formal High prepare inputs.  Runtime derives
/// session/project, source candidate state, artifact/readback and receipt
/// hashes from the exact durable transition and High lineage.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionWeaponFormalHighPrepareRequest {
    pub schema_version: String,
    pub source_stage_head_transition_id: String,
    pub source_stage_head_transition_sha256: String,
    pub source_stage_head_canonical_sha256: String,
    pub high_candidate_id: String,
    pub idempotency_key: String,
    pub max_response_bytes: u64,
    pub writer_policy: String,
    pub input_sha256: String,
}

/// Runtime-owned Formal High materialization result.  The nested candidate and
/// High records are derived by Runtime; replay/restart and non-promotion flags
/// make the persistence boundary explicit without exposing a second writer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionWeaponFormalHighPrepareResult {
    pub schema_version: String,
    pub candidate: CandidateRecord,
    pub high: ProductionWeaponHighArtifactRecord,
    pub replayed: bool,
    pub runtime_write: bool,
    pub restart_hash_verified: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

/// Exact read key for one Runtime-owned Formal High row.  No caller-supplied
/// candidate state, artifact, readback or receipt hash is accepted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionWeaponFormalHighGetRequest {
    pub schema_version: String,
    pub project_id: String,
    pub session_id: String,
    pub high_artifact_id: String,
    pub high_candidate_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionWeaponFormalHighGetResult {
    pub schema_version: String,
    pub candidate: CandidateRecord,
    pub high: ProductionWeaponHighArtifactRecord,
    pub replayed: bool,
    pub runtime_write: bool,
    pub restart_hash_verified: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}
