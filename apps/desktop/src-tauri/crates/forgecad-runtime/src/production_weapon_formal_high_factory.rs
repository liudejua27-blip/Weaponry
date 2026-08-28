//! Pure construction of the derived High candidate and its formal High record.
//!
//! This module deliberately has no Runtime/Store/CAS dependency.  Its input is
//! the result of the read-only High lineage resolver and therefore contains
//! only closed, hash-bound scalar data.  The caller remains responsible for
//! verifying the GLB, readback, program, detail graph and candidate rows before
//! calling [`build_formal_high_artifact`].

use super::{canonical_json_bytes, canonical_json_hash, sha256_hex};
use forgecad_contracts::{
    CandidateRecord, ProductionWeaponHighArtifactRecord, PRODUCTION_STAGE_V3_STAGES,
    PRODUCTION_WEAPON_HIGH_ARTIFACT_KIND, PRODUCTION_WEAPON_HIGH_ARTIFACT_POLICY,
    PRODUCTION_WEAPON_HIGH_ARTIFACT_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use serde_json::{to_value, Value};
use std::collections::BTreeSet;

const GLB_MIME: &str = "model/gltf-binary";
const CANDIDATE_SCHEMA_VERSION: &str = "Candidate@1";
const CANDIDATE_STATE: &str = "prepared";
const STRUCTURAL_STATUS: &str = "PASS_SOURCE_STRUCTURAL";
const VISUAL_STATUS: &str = "NOT_RUN";
const HUMAN_STATUS: &str = "NOT_RUN";
const ENGINE_STATUS: &str = "NOT_RUN";
const DISTRIBUTION_STATUS: &str = "NOT_RUN";
const QUALITY_STATUS: &str = "structural_only";
// HighMeshArtifact@1 reports the Worker-local source preservation mode as
// `source-preserved`. ProductionWeaponHighArtifact@1 uses the normalized
// commercial-pipeline vocabulary: the derived GLB retains source authoring
// lineage but is not itself a complete editable authoring cage.
const HIGH_AUTHORING_TOPOLOGY_STATUS: &str = "partial";
const HIGH_UV_STATUS: &str = "NOT_RUN";
const HIGH_TANGENT_STATUS: &str = "NOT_RUN";
const HIGH_REPLAY_COUNT: u64 = 2;
const MAX_PARTS: usize = 256;
const MAX_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;

/// All data needed after the read-only lineage resolver has verified the
/// source Stage head and the distinct derived High candidate.  No JSON blob,
/// GLB bytes, path, script or Store object is representable here.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct VerifiedFormalHighFactoryInput {
    pub session_id: String,
    pub project_id: String,

    pub source_stage_head_transition_id: String,
    pub source_stage_head_transition_sha256: String,
    pub source_stage_head_canonical_sha256: String,
    pub source_stage_head_stage: String,
    pub source_candidate_id: String,
    pub source_candidate_state_sha256: String,
    pub source_artifact_id: String,
    pub source_artifact_sha256: String,
    pub source_artifact_readback_sha256: String,

    pub high_candidate_id: String,
    pub high_artifact_id: String,
    pub high_artifact_sha256: String,
    pub high_artifact_readback_sha256: String,
    pub high_artifact_readback_object_sha256: String,
    pub high_geometry_program_sha256: String,
    pub high_geometry_program_object_sha256: String,
    pub high_geometry_candidate_evidence_sha256: String,
    pub high_detail_graph_object_sha256: String,
    pub high_detail_graph_canonical_sha256: String,
    pub high_part_ids: Vec<String>,
    pub high_material_zone_ids: Vec<String>,
    pub high_size_bytes: u64,
    pub high_worker_algorithm_sha256: String,
    pub high_worker_build_cohort_sha256: String,
    pub high_topology_status: String,

    pub base_version_id: Option<String>,
    pub source_version_id: Option<String>,
    pub candidate_manifest_hash: Option<String>,
    pub request_sha256: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub(crate) struct FormalHighFactoryOutput {
    pub candidate: CandidateRecord,
    pub high: ProductionWeaponHighArtifactRecord,
    /// Canonical formal High payload bytes.  The `receipt_object_sha256`
    /// field is intentionally empty in these bytes, matching the Store's
    /// detached receipt-object convention.
    pub receipt_json_bytes: Vec<u8>,
    pub receipt_object_sha256: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum FormalHighFactoryError {
    #[error("formal High factory field {field} is invalid")]
    InvalidField { field: &'static str },
    #[error("formal High factory binding is invalid: {0}")]
    Binding(&'static str),
    #[error("formal High factory canonical JSON failed: {0}")]
    Serialization(String),
}

fn invalid_field(field: &'static str) -> FormalHighFactoryError {
    FormalHighFactoryError::InvalidField { field }
}

fn require_id(field: &'static str, value: &str) -> Result<(), FormalHighFactoryError> {
    if forgecad_contracts::is_opaque_id(value) {
        Ok(())
    } else {
        Err(invalid_field(field))
    }
}

fn require_hash(field: &'static str, value: &str) -> Result<(), FormalHighFactoryError> {
    if forgecad_contracts::is_sha256(value) {
        Ok(())
    } else {
        Err(invalid_field(field))
    }
}

fn require_text(
    field: &'static str,
    value: &str,
    max_len: usize,
) -> Result<(), FormalHighFactoryError> {
    if !value.is_empty() && value.len() <= max_len {
        Ok(())
    } else {
        Err(invalid_field(field))
    }
}

fn require_optional_id(
    field: &'static str,
    value: Option<&str>,
) -> Result<(), FormalHighFactoryError> {
    value.map_or(Ok(()), |value| require_id(field, value))
}

fn require_optional_hash(
    field: &'static str,
    value: Option<&str>,
) -> Result<(), FormalHighFactoryError> {
    value.map_or(Ok(()), |value| require_hash(field, value))
}

fn require_unique_ids(
    field: &'static str,
    values: &[String],
) -> Result<(), FormalHighFactoryError> {
    if values.is_empty() || values.len() > MAX_PARTS {
        return Err(invalid_field(field));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        require_id(field, value)?;
        if !unique.insert(value.as_str()) {
            return Err(invalid_field(field));
        }
    }
    Ok(())
}

fn input_sha256(input: &VerifiedFormalHighFactoryInput) -> Result<String, FormalHighFactoryError> {
    let value = to_value(input)
        .map_err(|error| FormalHighFactoryError::Serialization(format!("input encode: {error}")))?;
    Ok(canonical_json_hash(&value))
}

fn validate_input(input: &VerifiedFormalHighFactoryInput) -> Result<(), FormalHighFactoryError> {
    for (field, value) in [
        ("session_id", input.session_id.as_str()),
        ("project_id", input.project_id.as_str()),
        (
            "source_stage_head_transition_id",
            input.source_stage_head_transition_id.as_str(),
        ),
        ("source_candidate_id", input.source_candidate_id.as_str()),
        ("source_artifact_id", input.source_artifact_id.as_str()),
        ("high_candidate_id", input.high_candidate_id.as_str()),
        ("high_artifact_id", input.high_artifact_id.as_str()),
    ] {
        require_id(field, value)?;
    }
    for (field, value) in [
        (
            "source_stage_head_transition_sha256",
            input.source_stage_head_transition_sha256.as_str(),
        ),
        (
            "source_stage_head_canonical_sha256",
            input.source_stage_head_canonical_sha256.as_str(),
        ),
        (
            "source_candidate_state_sha256",
            input.source_candidate_state_sha256.as_str(),
        ),
        (
            "source_artifact_sha256",
            input.source_artifact_sha256.as_str(),
        ),
        (
            "source_artifact_readback_sha256",
            input.source_artifact_readback_sha256.as_str(),
        ),
        ("high_artifact_sha256", input.high_artifact_sha256.as_str()),
        (
            "high_artifact_readback_sha256",
            input.high_artifact_readback_sha256.as_str(),
        ),
        (
            "high_artifact_readback_object_sha256",
            input.high_artifact_readback_object_sha256.as_str(),
        ),
        (
            "high_geometry_program_sha256",
            input.high_geometry_program_sha256.as_str(),
        ),
        (
            "high_geometry_program_object_sha256",
            input.high_geometry_program_object_sha256.as_str(),
        ),
        (
            "high_geometry_candidate_evidence_sha256",
            input.high_geometry_candidate_evidence_sha256.as_str(),
        ),
        (
            "high_detail_graph_object_sha256",
            input.high_detail_graph_object_sha256.as_str(),
        ),
        (
            "high_detail_graph_canonical_sha256",
            input.high_detail_graph_canonical_sha256.as_str(),
        ),
        (
            "high_worker_algorithm_sha256",
            input.high_worker_algorithm_sha256.as_str(),
        ),
        (
            "high_worker_build_cohort_sha256",
            input.high_worker_build_cohort_sha256.as_str(),
        ),
        ("request_sha256", input.request_sha256.as_str()),
    ] {
        require_hash(field, value)?;
    }
    if !PRODUCTION_STAGE_V3_STAGES.contains(&input.source_stage_head_stage.as_str()) {
        return Err(invalid_field("source_stage_head_stage"));
    }
    require_text("high_topology_status", &input.high_topology_status, 64)?;
    require_text("created_at", &input.created_at, 128)?;
    require_text("updated_at", &input.updated_at, 128)?;
    require_optional_id("base_version_id", input.base_version_id.as_deref())?;
    require_optional_id("source_version_id", input.source_version_id.as_deref())?;
    require_optional_hash(
        "candidate_manifest_hash",
        input.candidate_manifest_hash.as_deref(),
    )?;
    require_unique_ids("high_part_ids", &input.high_part_ids)?;
    require_unique_ids("high_material_zone_ids", &input.high_material_zone_ids)?;
    if input.high_size_bytes == 0 || input.high_size_bytes > MAX_ARTIFACT_BYTES {
        return Err(invalid_field("high_size_bytes"));
    }
    if input.source_candidate_id == input.high_candidate_id {
        return Err(FormalHighFactoryError::Binding(
            "source and derived High candidates must be distinct",
        ));
    }
    if input.source_artifact_sha256 == input.high_artifact_sha256 {
        return Err(FormalHighFactoryError::Binding(
            "source and derived High GLB roots must be distinct",
        ));
    }
    Ok(())
}

fn formal_high_canonical_sha256(
    record: &ProductionWeaponHighArtifactRecord,
) -> Result<String, FormalHighFactoryError> {
    let mut value = to_value(record).map_err(|error| {
        FormalHighFactoryError::Serialization(format!("High record encode: {error}"))
    })?;
    value["receipt_object_sha256"] = Value::String(String::new());
    value["canonical_sha256"] = Value::String(String::new());
    Ok(canonical_json_hash(&value))
}

fn candidate_state_sha256(candidate: &CandidateRecord) -> Result<String, FormalHighFactoryError> {
    let mut value = to_value(candidate).map_err(|error| {
        FormalHighFactoryError::Serialization(format!("candidate encode: {error}"))
    })?;
    value["canonical_sha256"] = Value::String(String::new());
    Ok(canonical_json_hash(&value))
}

fn formal_high_receipt_bytes(
    record: &ProductionWeaponHighArtifactRecord,
) -> Result<Vec<u8>, FormalHighFactoryError> {
    let mut value = to_value(record).map_err(|error| {
        FormalHighFactoryError::Serialization(format!("High receipt encode: {error}"))
    })?;
    value["receipt_object_sha256"] = Value::String(String::new());
    canonical_json_bytes(&value)
        .map_err(|error| FormalHighFactoryError::Serialization(error.to_string()))
}

/// Construct the derived High candidate row, formal High record and detached
/// receipt payload.  This function performs no persistence and cannot advance
/// a Stage, confirm a candidate, create a version or export an asset.
pub(crate) fn build_formal_high_artifact(
    input: VerifiedFormalHighFactoryInput,
) -> Result<FormalHighFactoryOutput, FormalHighFactoryError> {
    validate_input(&input)?;

    let mut candidate = CandidateRecord {
        schema_version: CANDIDATE_SCHEMA_VERSION.to_owned(),
        candidate_id: input.high_candidate_id.clone(),
        project_id: input.project_id.clone(),
        base_version_id: input.base_version_id.clone(),
        source_version_id: input.source_version_id.clone(),
        prepared_object_id: Some(input.high_artifact_id.clone()),
        prepared_object_sha256: Some(input.high_artifact_sha256.clone()),
        state: CANDIDATE_STATE.to_owned(),
        request_sha256: input.request_sha256.clone(),
        manifest_hash: input.candidate_manifest_hash.clone(),
        quality_report_id: None,
        quality_hard_gate_passed: false,
        canonical_sha256: String::new(),
        error_code: None,
        created_at: input.created_at.clone(),
        updated_at: input.updated_at.clone(),
    };
    candidate.canonical_sha256 = candidate_state_sha256(&candidate)?;
    let derived_candidate_state_sha256 = candidate.canonical_sha256.clone();
    let derived_input_sha256 = input_sha256(&input)?;
    let high_part_inventory_sha256 = canonical_json_hash(&serde_json::json!({
        "part_ids":input.high_part_ids,
        "material_zone_ids":input.high_material_zone_ids
    }));

    let mut high = ProductionWeaponHighArtifactRecord {
        schema_version: PRODUCTION_WEAPON_HIGH_ARTIFACT_SCHEMA_VERSION.to_owned(),
        high_artifact_id: input.high_artifact_id.clone(),
        session_id: input.session_id.clone(),
        project_id: input.project_id.clone(),
        source_stage_head_transition_id: input.source_stage_head_transition_id.clone(),
        source_stage_head_transition_sha256: input.source_stage_head_transition_sha256.clone(),
        source_stage_head_canonical_sha256: input.source_stage_head_canonical_sha256.clone(),
        source_stage_head_stage: input.source_stage_head_stage.clone(),
        source_candidate_id: input.source_candidate_id.clone(),
        source_candidate_state_sha256: input.source_candidate_state_sha256.clone(),
        source_artifact_id: input.source_artifact_id.clone(),
        source_artifact_sha256: input.source_artifact_sha256.clone(),
        source_artifact_readback_sha256: input.source_artifact_readback_sha256.clone(),
        high_candidate_id: input.high_candidate_id.clone(),
        high_candidate_state_sha256: derived_candidate_state_sha256,
        high_artifact_sha256: input.high_artifact_sha256.clone(),
        high_artifact_readback_sha256: input.high_artifact_readback_sha256.clone(),
        high_artifact_readback_object_sha256: input.high_artifact_readback_object_sha256.clone(),
        high_geometry_program_sha256: input.high_geometry_program_sha256.clone(),
        high_geometry_program_object_sha256: input.high_geometry_program_object_sha256.clone(),
        high_geometry_candidate_evidence_sha256: input
            .high_geometry_candidate_evidence_sha256
            .clone(),
        high_detail_graph_object_sha256: input.high_detail_graph_object_sha256.clone(),
        high_detail_graph_canonical_sha256: input.high_detail_graph_canonical_sha256.clone(),
        high_part_inventory_sha256,
        high_part_ids: input.high_part_ids.clone(),
        high_material_zone_ids: input.high_material_zone_ids.clone(),
        high_policy: PRODUCTION_WEAPON_HIGH_ARTIFACT_POLICY.to_owned(),
        high_policy_sha256: sha256_hex(PRODUCTION_WEAPON_HIGH_ARTIFACT_POLICY.as_bytes()),
        high_artifact_kind: PRODUCTION_WEAPON_HIGH_ARTIFACT_KIND.to_owned(),
        high_mime: GLB_MIME.to_owned(),
        high_size_bytes: input.high_size_bytes,
        high_worker_algorithm_sha256: input.high_worker_algorithm_sha256.clone(),
        high_worker_build_cohort_sha256: input.high_worker_build_cohort_sha256.clone(),
        high_worker_replay_count: HIGH_REPLAY_COUNT,
        high_replay_byte_exact: true,
        high_topology_status: input.high_topology_status.clone(),
        high_authoring_topology_status: HIGH_AUTHORING_TOPOLOGY_STATUS.to_owned(),
        high_uv_status: HIGH_UV_STATUS.to_owned(),
        high_tangent_status: HIGH_TANGENT_STATUS.to_owned(),
        validator_status: "passed".to_owned(),
        structural_status: STRUCTURAL_STATUS.to_owned(),
        visual_status: VISUAL_STATUS.to_owned(),
        human_status: HUMAN_STATUS.to_owned(),
        engine_status: ENGINE_STATUS.to_owned(),
        distribution_status: DISTRIBUTION_STATUS.to_owned(),
        quality_status: QUALITY_STATUS.to_owned(),
        hard_gate_passed: true,
        runtime_write_performed: true,
        production_stage_advanced: false,
        candidate_confirmed: false,
        version_created: false,
        export_performed: false,
        request_sha256: input.request_sha256,
        input_sha256: derived_input_sha256,
        receipt_object_sha256: String::new(),
        canonical_sha256: String::new(),
        created_at: input.created_at,
    };

    high.canonical_sha256 = formal_high_canonical_sha256(&high)?;
    let receipt_json_bytes = formal_high_receipt_bytes(&high)?;
    if receipt_json_bytes.is_empty() {
        return Err(FormalHighFactoryError::Binding(
            "formal High receipt payload is empty",
        ));
    }
    let receipt_object_sha256 = sha256_hex(&receipt_json_bytes);
    high.receipt_object_sha256 = receipt_object_sha256.clone();

    Ok(FormalHighFactoryOutput {
        candidate,
        high,
        receipt_json_bytes,
        receipt_object_sha256,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(seed: char) -> String {
        seed.to_string().repeat(64)
    }

    fn input_with_hash() -> VerifiedFormalHighFactoryInput {
        VerifiedFormalHighFactoryInput {
            session_id: "session-1".to_owned(),
            project_id: "project-1".to_owned(),
            source_stage_head_transition_id: "transition-1".to_owned(),
            source_stage_head_transition_sha256: hash('a'),
            source_stage_head_canonical_sha256: hash('b'),
            source_stage_head_stage: "secondary-form-approved".to_owned(),
            source_candidate_id: "source-candidate-1".to_owned(),
            source_candidate_state_sha256: hash('c'),
            source_artifact_id: "source-artifact-1".to_owned(),
            source_artifact_sha256: hash('d'),
            source_artifact_readback_sha256: hash('e'),
            high_candidate_id: "high-candidate-1".to_owned(),
            high_artifact_id: "high-artifact-1".to_owned(),
            high_artifact_sha256: hash('0'),
            high_artifact_readback_sha256: hash('1'),
            high_artifact_readback_object_sha256: hash('2'),
            high_geometry_program_sha256: hash('3'),
            high_geometry_program_object_sha256: hash('4'),
            high_geometry_candidate_evidence_sha256: hash('5'),
            high_detail_graph_object_sha256: hash('6'),
            high_detail_graph_canonical_sha256: hash('7'),
            high_part_ids: vec!["receiver".to_owned(), "muzzle".to_owned()],
            high_material_zone_ids: vec!["outer-shell".to_owned()],
            high_size_bytes: 1024,
            high_worker_algorithm_sha256: hash('a'),
            high_worker_build_cohort_sha256: hash('b'),
            high_topology_status: "structural-readback".to_owned(),
            base_version_id: None,
            source_version_id: None,
            candidate_manifest_hash: None,
            request_sha256: hash('c'),
            created_at: "2026-08-26T00:00:00Z".to_owned(),
            updated_at: "2026-08-26T00:00:00Z".to_owned(),
        }
    }

    #[test]
    fn factory_binds_candidate_and_detached_receipt() {
        let output = build_formal_high_artifact(input_with_hash()).expect("factory output");
        assert_eq!(output.candidate.candidate_id, output.high.high_candidate_id);
        assert_eq!(
            output.candidate.prepared_object_sha256.as_deref(),
            Some(output.high.high_artifact_sha256.as_str())
        );
        assert_eq!(output.high.quality_status, QUALITY_STATUS);
        assert_eq!(output.high.visual_status, VISUAL_STATUS);
        assert_eq!(output.high.human_status, HUMAN_STATUS);
        assert_eq!(output.high.engine_status, ENGINE_STATUS);
        assert_eq!(output.high.distribution_status, DISTRIBUTION_STATUS);
        assert!(!output.high.production_stage_advanced);
        assert!(!output.high.candidate_confirmed);
        assert!(!output.high.version_created);
        assert!(!output.high.export_performed);
        assert_eq!(
            output.receipt_object_sha256,
            sha256_hex(&output.receipt_json_bytes)
        );
        let receipt: Value = serde_json::from_slice(&output.receipt_json_bytes).expect("receipt");
        assert_eq!(
            receipt["receipt_object_sha256"],
            Value::String(String::new())
        );
        assert_eq!(receipt["canonical_sha256"], output.high.canonical_sha256);
    }

    #[test]
    fn factory_rejects_same_source_and_high_candidate() {
        let mut input = input_with_hash();
        input.high_candidate_id = input.source_candidate_id.clone();
        assert!(matches!(
            build_formal_high_artifact(input),
            Err(FormalHighFactoryError::Binding(
                "source and derived High candidates must be distinct"
            ))
        ));
    }

    #[test]
    fn factory_derives_candidate_policy_inventory_and_input_hashes() {
        let input = input_with_hash();
        let expected_input = input_sha256(&input).expect("input hash");
        let output = build_formal_high_artifact(input).expect("factory output");
        assert_eq!(output.high.input_sha256, expected_input);
        assert_eq!(
            output.high.high_policy_sha256,
            sha256_hex(PRODUCTION_WEAPON_HIGH_ARTIFACT_POLICY.as_bytes())
        );
        assert_eq!(
            output.high.high_part_inventory_sha256,
            canonical_json_hash(&serde_json::json!({
                "part_ids":output.high.high_part_ids,
                "material_zone_ids":output.high.high_material_zone_ids
            }))
        );
        assert_eq!(
            output.high.high_candidate_state_sha256,
            output.candidate.canonical_sha256
        );
    }
}
