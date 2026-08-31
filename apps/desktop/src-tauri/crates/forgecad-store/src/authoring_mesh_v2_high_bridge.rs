//! Durable Store/CAS boundary for the narrow AuthoringMesh V2 High bridge.
//!
//! This module is deliberately independent from the existing NativeHigh and
//! AuthoringMesh@1 persistence paths.  Runtime stages the bridge, direct High
//! result and readback objects in CAS; this repository verifies those objects
//! and the complete upstream lineage in one SQLite transaction.  It never
//! accepts topology, evaluator steps, a GLB-derived mesh, a path or a worker
//! selection.

use super::*;
use forgecad_contracts::{AuthoringMeshRevision, AuthoringMeshV2SourceBinding};
use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const AUTHORING_MESH_V2_HIGH_BRIDGE_SCHEMA_VERSION: &str = "AuthoringMeshV2HighBridge@1";
pub const AUTHORING_MESH_V2_HIGH_BRIDGE_RECORD_SCHEMA_VERSION: &str =
    "AuthoringMeshV2HighBridgeStoreRecord@1";
pub const AUTHORING_MESH_V2_HIGH_BRIDGE_OBJECT_KIND: &str = "authoring-mesh-v2-high-bridge@1";
pub const AUTHORING_MESH_V2_HIGH_RESULT_OBJECT_KIND: &str = "authoring-mesh-v2-high-result@2";
pub const AUTHORING_MESH_V2_HIGH_READBACK_OBJECT_KIND: &str = "authoring-mesh-v2-high-readback@2";
pub const AUTHORING_MESH_V2_HIGH_JSON_MIME: &str = "application/json";
pub const AUTHORING_MESH_V2_HIGH_MAX_JSON_BYTES: u64 = 8 * 1024 * 1024;
pub const AUTHORING_MESH_V2_HIGH_MAX_BRIDGE_BYTES: u64 = 1024 * 1024;
pub const AUTHORING_MESH_V2_HIGH_STATUS: &str =
    "runtime-owned-store-authoring-mesh-v2-high-bridge@1";
pub const AUTHORING_MESH_V2_HIGH_SOURCE_SCOPE: &str = "materialized-v2-revision-part-set@1";
pub const AUTHORING_MESH_V2_HIGH_REVISION_SCHEMA_VERSION: &str = "AuthoringMeshRevision@2";
pub const AUTHORING_MESH_V2_HIGH_EXECUTION_REQUEST_SCHEMA_VERSION: &str =
    "AuthoringMeshV2HighExecutionRequest@2";
pub const AUTHORING_MESH_V2_HIGH_EXECUTION_OPERATION: &str =
    "forgecad.production.authoring-mesh-v2-high-execute@1";
pub const AUTHORING_MESH_V2_HIGH_OPERATION: &str =
    "forgecad.production.authoring-mesh-v2-high-evaluate@1";
pub const AUTHORING_MESH_V2_HIGH_EVALUATOR_OPERATION: &str = "forgecad.production.high-evaluator@1";
pub const AUTHORING_MESH_V2_HIGH_RESULT_SCHEMA_VERSION: &str = "AuthoringMeshV2HighResult@2";
pub const AUTHORING_MESH_V2_HIGH_READBACK_SCHEMA_VERSION: &str = "AuthoringMeshV2HighReadback@2";
pub const AUTHORING_MESH_V2_HIGH_EVALUATOR_CONTRACT: &str =
    "forgecad-owned-cpu-catmull-clark-stitched-polygon@2";
pub const AUTHORING_MESH_V2_HIGH_SUBDIVISION_BACKEND: &str = "cpu_regular_quad";
pub const AUTHORING_MESH_V2_HIGH_CANONICALIZATION_POLICY: &str =
    "canonical-json-sha256-excluding-canonical-sha256@1";
pub const AUTHORING_MESH_V2_HIGH_WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
pub const AUTHORING_MESH_V2_HIGH_ARTIFACT_HASH_POLICY: &str =
    "artifact-sha256-equals-object-sha256-until-semantic-artifact-contract@1";
pub const AUTHORING_MESH_V2_HIGH_COHORT_POLICY: &str =
    "same-worker-build-cohort-required-for-durable-link@1";
pub const AUTHORING_MESH_V2_HIGH_MAX_RESPONSE_BYTES: u64 = 1024 * 1024;
const AUTHORING_MESH_V2_MATERIALIZATION_PLAN_SCHEMA_VERSION: &str =
    "AuthoringMeshV2CandidateMaterializationRepresentationPlan@1";
const AUTHORING_MESH_V2_MATERIALIZATION_OPERATOR_ID: &str = "forgecad.geometry.authoring-mesh@1";

pub const AUTHORING_MESH_V2_HIGH_SCOPE_LIMITATIONS: [&str; 5] = [
    "RUNTIME_DERIVES_COMPLETE_ORDERED_PART_INPUTS",
    "RUNTIME_CONSTRUCTS_CPU_STITCHED_STEPS",
    "NO_CALLER_SUPPLIED_REVISION_TOPOLOGY",
    "NO_OPEN_SUBDIVISION_BACKEND",
    "VERIFIED_PRESERVED_PARTS_FROM_MATERIALIZED_GLB",
];

/// Store-local representation of the closed `AuthoringMeshV2HighBridge@1`
/// Main object.  The three Store-only fields at the end are intentionally not
/// part of the Main CAS payload: they identify the Store request and the CAS
/// object itself.  `bridge_sha256` is a checked duplicate of `canonical_sha256`
/// because Get/Result expose that semantic identity under the bridge name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthoringMeshV2HighBridgeStoreRecord {
    pub schema_version: String,
    pub bridge_id: String,
    pub project_id: String,
    pub source_scope: String,
    pub source_revision_schema_version: String,
    pub mesh_id: String,
    pub lineage_id: String,
    pub revision_id: String,
    pub revision_index: u64,
    pub revision_sha256: String,
    pub revision_object_sha256: String,
    pub source_binding_id: String,
    pub source_binding_sha256: String,
    pub source_binding_object_sha256: String,
    pub materialized_candidate_id: String,
    pub materialized_candidate_state_sha256: String,
    pub materialized_program_sha256: String,
    pub materialized_program_object_sha256: String,
    pub materialized_artifact_id: String,
    pub materialized_artifact_sha256: String,
    pub materialized_artifact_object_sha256: String,
    pub materialized_artifact_readback_sha256: String,
    pub materialized_artifact_readback_object_sha256: String,
    pub representation_plan_sha256: String,
    pub source_node_id: String,
    pub part_id: String,
    pub material_zone_id: String,
    pub solid: bool,
    pub source_part_output_sha256: String,
    pub preserved_part_ids: Vec<String>,
    pub materialized_artifact_hash_policy: String,
    pub high_execution_request_schema_version: String,
    pub high_execution_operation: String,
    pub high_operation: String,
    pub high_result_schema_version: String,
    pub high_readback_schema_version: String,
    pub high_evaluator_contract: String,
    pub high_subdivision_backend: String,
    pub high_subdivision_levels: u64,
    pub high_max_triangles_per_face: u64,
    pub high_max_output_vertices: u64,
    pub high_max_output_triangles: u64,
    pub high_execution_request_sha256: String,
    pub high_evaluation_sha256: String,
    pub high_result_sha256: String,
    pub high_result_object_sha256: String,
    pub high_readback_sha256: String,
    pub high_readback_object_sha256: String,
    pub high_worker_algorithm_sha256: String,
    pub high_worker_build_cohort_sha256: String,
    pub high_replay_count: u64,
    pub high_replay_byte_exact: bool,
    pub high_non_destructive: bool,
    pub high_projected_source_mesh_sha256: String,
    pub high_source_vertex_count: u64,
    pub high_source_triangle_count: u64,
    pub high_evaluated_part_count: u64,
    pub high_evaluated_triangle_count: u64,
    pub cohort_policy: String,
    pub scope_limitations: Vec<String>,
    pub high_structural_status: String,
    pub high_status: String,
    pub quality_status: String,
    pub visual_status: String,
    pub human_status: String,
    pub engine_status: String,
    pub high_mesh_created: bool,
    pub high_stage_unlocked: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub writer_policy: String,
    pub canonicalization_policy: String,
    pub canonical_sha256: String,
    pub created_at: String,
    /// Store-only semantic alias for `canonical_sha256`.
    pub bridge_sha256: String,
    /// Store-only CAS object identity for the Main payload.
    pub bridge_object_sha256: String,
    pub request_input_sha256: String,
    pub idempotency_key: String,
}

/// Shared in-crate fixture for Store-only downstream repositories.  It stays
/// test-only so production callers cannot manufacture a High bridge by using
/// test data or bypass its Runtime-owned validation path.
#[cfg(test)]
pub(crate) fn test_setup_fixture() -> tests::Fixture {
    tests::setup_fixture()
}

/// CAS objects staged by Runtime before the Store transaction.  The bridge,
/// direct High result and direct High readback are separate immutable roots;
/// none is synthesized from another object or from a GLB.
#[derive(Debug, Clone)]
pub struct AuthoringMeshV2HighBridgeCasBundle {
    pub bridge: CasObjectRecord,
    pub high_result: CasObjectRecord,
    pub high_readback: CasObjectRecord,
}

#[derive(Debug, Clone)]
pub struct AuthoringMeshV2HighBridgeCommit {
    pub record: AuthoringMeshV2HighBridgeStoreRecord,
    pub cas: AuthoringMeshV2HighBridgeCasBundle,
}

// Compatibility aliases make the domain boundary usable by Runtime code that
// calls this capability "High" rather than "HighBridge" without introducing
// another persistence implementation.
pub type AuthoringMeshV2HighDurableRecord = AuthoringMeshV2HighBridgeStoreRecord;
pub type AuthoringMeshV2HighCommit = AuthoringMeshV2HighBridgeCommit;
pub type AuthoringMeshV2HighCasBundle = AuthoringMeshV2HighBridgeCasBundle;

fn contract(code: &str, message: impl Into<String>) -> StoreError {
    StoreError::Contract {
        code: code.to_owned(),
        message: message.into(),
    }
}

fn record_value(record: &AuthoringMeshV2HighBridgeStoreRecord) -> Result<Value, StoreError> {
    serde_json::to_value(record).map_err(|error| StoreError::InvalidData(error.to_string()))
}

/// Convert the Store-local row into the exact Main object.  This is the only
/// accepted bridge CAS payload shape; Store request identity never enters the
/// Main semantic hash.
pub fn main_value(record: &AuthoringMeshV2HighBridgeStoreRecord) -> Result<Value, StoreError> {
    let mut object = record_value(record)?
        .as_object()
        .cloned()
        .ok_or_else(|| StoreError::InvalidData("High bridge record is not an object".to_owned()))?;
    object.insert(
        "schema_version".to_owned(),
        Value::String(AUTHORING_MESH_V2_HIGH_BRIDGE_SCHEMA_VERSION.to_owned()),
    );
    object.remove("bridge_sha256");
    object.remove("bridge_object_sha256");
    object.remove("request_input_sha256");
    object.remove("idempotency_key");
    Ok(Value::Object(object))
}

fn canonical_sha256(value: &Value) -> Result<String, StoreError> {
    let mut preimage = value.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    Ok(canonical_json_hash(&preimage))
}

fn record_canonical_sha256(
    record: &AuthoringMeshV2HighBridgeStoreRecord,
) -> Result<String, StoreError> {
    canonical_sha256(&main_value(record)?)
}

fn ids<'a>(record: &'a AuthoringMeshV2HighBridgeStoreRecord) -> [&'a str; 12] {
    [
        &record.bridge_id,
        &record.project_id,
        &record.mesh_id,
        &record.lineage_id,
        &record.revision_id,
        &record.source_binding_id,
        &record.materialized_candidate_id,
        &record.materialized_artifact_id,
        &record.source_node_id,
        &record.part_id,
        &record.material_zone_id,
        &record.idempotency_key,
    ]
}

fn hashes<'a>(record: &'a AuthoringMeshV2HighBridgeStoreRecord) -> [&'a str; 22] {
    [
        &record.revision_sha256,
        &record.revision_object_sha256,
        &record.source_binding_sha256,
        &record.source_binding_object_sha256,
        &record.materialized_candidate_state_sha256,
        &record.materialized_program_sha256,
        &record.materialized_program_object_sha256,
        &record.materialized_artifact_sha256,
        &record.materialized_artifact_object_sha256,
        &record.materialized_artifact_readback_sha256,
        &record.materialized_artifact_readback_object_sha256,
        &record.representation_plan_sha256,
        &record.source_part_output_sha256,
        &record.high_execution_request_sha256,
        &record.high_evaluation_sha256,
        &record.high_result_sha256,
        &record.high_result_object_sha256,
        &record.high_readback_sha256,
        &record.high_readback_object_sha256,
        &record.high_worker_algorithm_sha256,
        &record.high_worker_build_cohort_sha256,
        &record.high_projected_source_mesh_sha256,
    ]
}

fn validate_record(record: &AuthoringMeshV2HighBridgeStoreRecord) -> Result<(), StoreError> {
    if record.schema_version != AUTHORING_MESH_V2_HIGH_BRIDGE_SCHEMA_VERSION
        || ids(record).iter().any(|id| !is_opaque_id(id))
        || hashes(record).iter().any(|hash| !is_sha256(hash))
        || !is_sha256(&record.canonical_sha256)
        || !is_sha256(&record.bridge_sha256)
        || !is_sha256(&record.bridge_object_sha256)
        || !is_sha256(&record.request_input_sha256)
        || record.bridge_sha256 != record.canonical_sha256
        || record.source_scope != AUTHORING_MESH_V2_HIGH_SOURCE_SCOPE
        || record.source_revision_schema_version != AUTHORING_MESH_V2_HIGH_REVISION_SCHEMA_VERSION
        || record.materialized_artifact_hash_policy != AUTHORING_MESH_V2_HIGH_ARTIFACT_HASH_POLICY
        || record.high_execution_request_schema_version
            != AUTHORING_MESH_V2_HIGH_EXECUTION_REQUEST_SCHEMA_VERSION
        || record.high_execution_operation != AUTHORING_MESH_V2_HIGH_EXECUTION_OPERATION
        || record.high_operation != AUTHORING_MESH_V2_HIGH_OPERATION
        || record.high_result_schema_version != AUTHORING_MESH_V2_HIGH_RESULT_SCHEMA_VERSION
        || record.high_readback_schema_version != AUTHORING_MESH_V2_HIGH_READBACK_SCHEMA_VERSION
        || record.high_evaluator_contract != AUTHORING_MESH_V2_HIGH_EVALUATOR_CONTRACT
        || record.high_subdivision_backend != AUTHORING_MESH_V2_HIGH_SUBDIVISION_BACKEND
        || record.high_subdivision_levels != 1
        || record.high_max_triangles_per_face != 32
        || record.high_max_output_vertices != 32_768
        || record.high_max_output_triangles != 600_000
        || record.high_replay_count != 2
        || !record.high_replay_byte_exact
        || !record.high_non_destructive
        || record.cohort_policy != AUTHORING_MESH_V2_HIGH_COHORT_POLICY
        || record.scope_limitations
            != AUTHORING_MESH_V2_HIGH_SCOPE_LIMITATIONS
                .iter()
                .map(|value| (*value).to_owned())
                .collect::<Vec<_>>()
        || record.high_structural_status != "PASS_SOURCE_STRUCTURAL"
        || record.high_status != "NOT_RUN"
        || record.quality_status != "structural_only"
        || record.visual_status != "NOT_RUN"
        || record.human_status != "NOT_RUN"
        || record.engine_status != "NOT_RUN"
        || record.high_mesh_created
        || record.high_stage_unlocked
        || record.production_stage_advanced
        || record.candidate_confirmed
        || record.version_created
        || record.export_performed
        || !record.runtime_write_performed
        || !record.persistent_user_data_touched
        || record.writer_policy != AUTHORING_MESH_V2_HIGH_WRITER_POLICY
        || record.canonicalization_policy != AUTHORING_MESH_V2_HIGH_CANONICALIZATION_POLICY
        || record.created_at.is_empty()
        || record.created_at.len() > 128
        || record.created_at.contains('/')
        || record.created_at.contains('\\')
        || record.preserved_part_ids.is_empty()
        || record.preserved_part_ids.len() > 128
        || record.preserved_part_ids.iter().any(|id| !is_opaque_id(id))
        || record
            .preserved_part_ids
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            != record.preserved_part_ids.len()
        || record
            .preserved_part_ids
            .iter()
            .any(|id| id == &record.part_id)
        || record.revision_index > 1_000_000
        || record.high_source_vertex_count == 0
        || record.high_source_vertex_count > 32_768
        || record.high_source_triangle_count == 0
        || record.high_source_triangle_count > 65_536
        || record.high_evaluated_part_count == 0
        || record.high_evaluated_part_count > 128
        || record.high_evaluated_triangle_count == 0
        || record.high_evaluated_triangle_count > 600_000
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_RECORD_INVALID",
            "High bridge identity, policy, status or bounded metadata is invalid",
        ));
    }
    if record_canonical_sha256(record)? != record.canonical_sha256 {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_CANONICAL_MISMATCH",
            "High bridge Main semantic canonical hash is invalid",
        ));
    }
    Ok(())
}

fn same_record(
    left: &AuthoringMeshV2HighBridgeStoreRecord,
    right: &AuthoringMeshV2HighBridgeStoreRecord,
) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    left.created_at.clear();
    right.created_at.clear();
    left == right
}

fn read_object_record(
    transaction: &Transaction<'_>,
    sha256: &str,
) -> Result<CasObjectRecord, StoreError> {
    transaction
        .query_row(
            "SELECT sha256, size_bytes, mime, kind, reachability, created_at FROM objects WHERE sha256 = ?1",
            params![sha256],
            |row| {
                let size: i64 = row.get(1)?;
                Ok(CasObjectRecord {
                    schema_version: "CasObject@1".to_owned(),
                    sha256: row.get(0)?,
                    size_bytes: u64::try_from(size).map_err(|_| rusqlite::Error::InvalidQuery)?,
                    mime: row.get(2)?,
                    kind: row.get(3)?,
                    reachability: row.get(4)?,
                    created_at: row.get(5)?,
                })
            },
        )
        .map_err(StoreError::from)
}

fn source_binding_payload_value(
    record: &KnifeSourceBindingStoreRecord,
) -> Result<Value, StoreError> {
    let mut object = serde_json::to_value(record)
        .map_err(|error| StoreError::InvalidData(error.to_string()))?
        .as_object()
        .cloned()
        .ok_or_else(|| {
            StoreError::InvalidData("SourceBinding record is not an object".to_owned())
        })?;
    object.insert(
        "schema_version".to_owned(),
        Value::String(KNIFE_SOURCE_BINDING_SCHEMA_VERSION.to_owned()),
    );
    object.remove("source_binding_sha256");
    object.remove("source_binding_object_sha256");
    object.remove("idempotency_key");
    object.insert(
        "canonical_sha256".to_owned(),
        Value::String(record.source_binding_sha256.clone()),
    );
    Ok(Value::Object(object))
}

fn validate_source_binding_record(
    record: &KnifeSourceBindingStoreRecord,
) -> Result<(), StoreError> {
    let ids = [
        record.source_binding_id.as_str(),
        record.project_id.as_str(),
        record.intent_bundle_id.as_str(),
        record.brief_id.as_str(),
        record.reference_id.as_str(),
        record.quality_contract_id.as_str(),
        record.source_candidate_id.as_str(),
        record.authoring_mesh_id.as_str(),
        record.authoring_mesh_lineage_id.as_str(),
        record.authoring_mesh_revision_id.as_str(),
        record.idempotency_key.as_str(),
    ];
    let hashes = [
        record.intent_bundle_sha256.as_str(),
        record.intent_bundle_object_sha256.as_str(),
        record.brief_sha256.as_str(),
        record.brief_object_sha256.as_str(),
        record.reference_object_sha256.as_str(),
        record.reference_evidence_sha256.as_str(),
        record.quality_contract_sha256.as_str(),
        record.quality_contract_object_sha256.as_str(),
        record.source_candidate_state_sha256.as_str(),
        record.authoring_mesh_revision_sha256.as_str(),
        record.authoring_mesh_revision_object_sha256.as_str(),
        record.authoring_mesh_identity_sha256.as_str(),
        record.source_binding_sha256.as_str(),
        record.source_binding_object_sha256.as_str(),
    ];
    let requirements = &record.downstream_binding_requirements;
    if record.schema_version != KNIFE_SOURCE_BINDING_RECORD_SCHEMA_VERSION
        || ids.iter().any(|id| !is_opaque_id(id))
        || hashes.iter().any(|hash| !is_sha256(hash))
        || record.binding_status != KNIFE_SOURCE_BINDING_BINDING_STATUS
        || record.authoring_eligibility != KNIFE_SOURCE_BINDING_AUTHORING_ELIGIBILITY
        || requirements.curve_modifier_graph != KNIFE_SOURCE_BINDING_DOWNSTREAM_POLICY
        || requirements.curve_evaluated_mesh != KNIFE_SOURCE_BINDING_DOWNSTREAM_POLICY
        || requirements.high != KNIFE_SOURCE_BINDING_DOWNSTREAM_POLICY
        || requirements.render != KNIFE_SOURCE_BINDING_DOWNSTREAM_POLICY
        || record.high_mesh_created
        || record.high_stage_unlocked
        || record.production_stage_advanced
        || record.candidate_confirmed
        || record.version_created
        || record.export_performed
        || record.quality_status != "source_binding_only"
        || record.visual_status != "NOT_RUN"
        || record.human_status != "NOT_RUN"
        || record.engine_status != "NOT_RUN"
        || record.binding_policy != KNIFE_SOURCE_BINDING_POLICY
        || record.canonicalization_policy != KNIFE_SOURCE_BINDING_CANONICALIZATION_POLICY
        || record.created_at.is_empty()
        || record.created_at.len() > 128
        || record.created_at.contains('/')
        || record.created_at.contains('\\')
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_SOURCE_BINDING_INVALID",
            "SourceBinding row is not a valid immutable source binding",
        ));
    }
    Ok(())
}

fn validate_canonical_root(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    object_sha256: &str,
    expected_kind: &str,
    expected_schema_version: &str,
    expected_semantic_sha256: &str,
    require_reachable: bool,
    role: &str,
) -> Result<Value, StoreError> {
    let object = read_object_record(transaction, object_sha256).map_err(|error| match error {
        StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_UPSTREAM_CAS_MISSING",
            format!("{role} CAS object is not registered"),
        ),
        other => other,
    })?;
    let bytes = validate_cas_metadata_and_bytes(
        transaction,
        cas,
        &object,
        object_sha256,
        expected_kind,
        AUTHORING_MESH_V2_HIGH_JSON_MIME,
        AUTHORING_MESH_V2_HIGH_MAX_JSON_BYTES,
        require_reachable,
        role,
    )?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|error| {
        contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_UPSTREAM_JSON_INVALID",
            format!("{role} CAS JSON is invalid: {error}"),
        )
    })?;
    if canonical_json_bytes(&value).map_err(|error| StoreError::InvalidData(error.to_string()))?
        != bytes
        || value.get("schema_version").and_then(Value::as_str) != Some(expected_schema_version)
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_UPSTREAM_CANONICAL_MISMATCH",
            format!("{role} CAS JSON is not canonical or has the wrong schema"),
        ));
    }
    validate_embedded_canonical_hash(&value, role)?;
    if value.get("canonical_sha256").and_then(Value::as_str) != Some(expected_semantic_sha256) {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_UPSTREAM_BINDING_MISMATCH",
            format!("{role} semantic hash differs from SourceBinding"),
        ));
    }
    Ok(value)
}

fn validate_source_binding_upstream(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    source_binding: &KnifeSourceBindingStoreRecord,
    require_reachable: bool,
) -> Result<Vec<String>, StoreError> {
    let intent: Option<(
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
    )> = transaction
        .query_row(
            "SELECT brief_id, brief_sha256, brief_object_sha256, reference_id, reference_object_sha256, reference_evidence_sha256, quality_contract_sha256, quality_contract_object_sha256, intent_bundle_object_sha256 FROM knife_reference_intent_bundle_records WHERE project_id = ?1 AND intent_bundle_id = ?2 AND intent_bundle_sha256 = ?3",
            params![
                source_binding.project_id,
                source_binding.intent_bundle_id,
                source_binding.intent_bundle_sha256
            ],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            },
        )
        .optional()?;
    let Some((
        brief_id,
        brief_sha256,
        brief_object_sha256,
        reference_id,
        reference_object_sha256,
        reference_evidence_sha256,
        quality_contract_sha256,
        quality_contract_object_sha256,
        intent_bundle_object_sha256,
    )) = intent
    else {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_INTENT_MISSING",
            "SourceBinding intent bundle is not durably registered",
        ));
    };
    if brief_id != source_binding.brief_id
        || brief_sha256 != source_binding.brief_sha256
        || brief_object_sha256 != source_binding.brief_object_sha256
        || reference_id != source_binding.reference_id
        || reference_object_sha256 != source_binding.reference_object_sha256
        || reference_evidence_sha256 != source_binding.reference_evidence_sha256
        || quality_contract_sha256 != source_binding.quality_contract_sha256
        || quality_contract_object_sha256 != source_binding.quality_contract_object_sha256
        || intent_bundle_object_sha256 != source_binding.intent_bundle_object_sha256
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_INTENT_BINDING_MISMATCH",
            "SourceBinding upstream intent fields differ from the durable intent row",
        ));
    }
    let intent_value = validate_canonical_root(
        transaction,
        cas,
        &source_binding.intent_bundle_object_sha256,
        KNIFE_REFERENCE_INTENT_BUNDLE_OBJECT_KIND,
        KNIFE_REFERENCE_INTENT_BUNDLE_SCHEMA_VERSION,
        &source_binding.intent_bundle_sha256,
        require_reachable,
        "intent bundle",
    )?;
    if intent_value.get("project_id").and_then(Value::as_str)
        != Some(source_binding.project_id.as_str())
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_INTENT_BINDING_MISMATCH",
            "intent bundle project differs from SourceBinding",
        ));
    }
    let quality_value = validate_canonical_root(
        transaction,
        cas,
        &source_binding.quality_contract_object_sha256,
        KNIFE_REFERENCE_INTENT_QUALITY_OBJECT_KIND,
        "KnifeQualityContract@1",
        &source_binding.quality_contract_sha256,
        require_reachable,
        "quality contract",
    )?;
    if quality_value.get("contract_id").and_then(Value::as_str)
        != Some(source_binding.quality_contract_id.as_str())
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_QUALITY_BINDING_MISMATCH",
            "quality contract id differs from SourceBinding",
        ));
    }
    let _brief_value = validate_canonical_root(
        transaction,
        cas,
        &source_binding.brief_object_sha256,
        WEAPONRY_KNIFE_PRODUCTION_BRIEF_OBJECT_KIND,
        "WeaponryKnifeProductionBrief@1",
        &source_binding.brief_sha256,
        require_reachable,
        "Brief",
    )?;

    let reference: Option<(String, String, String, i64, String)> = transaction
        .query_row(
            "SELECT project_id, object_sha256, canonical_sha256, size_bytes, mime FROM reference_evidence WHERE reference_id = ?1",
            params![source_binding.reference_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()?;
    let Some((project_id, object_sha256, evidence_sha256, size_bytes, mime)) = reference else {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_REFERENCE_MISSING",
            "SourceBinding ReferenceEvidence is not durably registered",
        ));
    };
    if project_id != source_binding.project_id
        || object_sha256 != source_binding.reference_object_sha256
        || evidence_sha256 != source_binding.reference_evidence_sha256
        || size_bytes <= 0
        || !matches!(mime.as_str(), "image/png" | "image/jpeg")
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_REFERENCE_BINDING_MISMATCH",
            "ReferenceEvidence fields differ from SourceBinding",
        ));
    }
    let reference_object = read_object_record(transaction, &object_sha256)?;
    let reference_bytes = validate_cas_metadata_and_bytes(
        transaction,
        cas,
        &reference_object,
        &object_sha256,
        "reference-image",
        &mime,
        64 * 1024 * 1024,
        require_reachable,
        "ReferenceEvidence",
    )?;
    if reference_bytes.len() as i64 != size_bytes {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_REFERENCE_BINDING_MISMATCH",
            "ReferenceEvidence CAS size differs from its durable row",
        ));
    }

    Ok(vec![
        source_binding.intent_bundle_object_sha256.clone(),
        source_binding.brief_object_sha256.clone(),
        source_binding.reference_object_sha256.clone(),
        source_binding.quality_contract_object_sha256.clone(),
    ])
}

fn validate_cas_metadata_and_bytes(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    supplied: &CasObjectRecord,
    expected_sha256: &str,
    expected_kind: &str,
    expected_mime: &str,
    max_bytes: u64,
    require_reachable: bool,
    role: &str,
) -> Result<Vec<u8>, StoreError> {
    if supplied.schema_version != "CasObject@1"
        || supplied.sha256 != expected_sha256
        || !is_sha256(expected_sha256)
        || supplied.mime != expected_mime
        || supplied.kind != expected_kind
        || supplied.size_bytes == 0
        || supplied.size_bytes > max_bytes
        || !matches!(supplied.reachability.as_str(), "temporary" | "reachable")
        || (require_reachable && supplied.reachability != "reachable")
        || supplied.created_at.is_empty()
        || supplied.created_at.len() > 128
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_CAS_METADATA_INVALID",
            format!("{role} CAS metadata is outside the bounded allowlist"),
        ));
    }
    let registered =
        read_object_record(transaction, expected_sha256).map_err(|error| match error {
            StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_CAS_MISSING",
                format!("{role} CAS object is not registered"),
            ),
            other => other,
        })?;
    let reachability_matches = supplied.reachability == registered.reachability
        || (supplied.reachability == "temporary" && registered.reachability == "reachable");
    if registered.schema_version != "CasObject@1"
        || registered.size_bytes != supplied.size_bytes
        || registered.mime != expected_mime
        || registered.kind != expected_kind
        || !matches!(registered.reachability.as_str(), "temporary" | "reachable")
        || !reachability_matches
        || (require_reachable && registered.reachability != "reachable")
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_CAS_METADATA_MISMATCH",
            format!("{role} CAS metadata differs from SQLite registration"),
        ));
    }
    let bytes = cas
        .read_verified_bounded(expected_sha256, max_bytes)
        .map_err(StoreError::from)?;
    if bytes.len() as u64 != supplied.size_bytes || sha256_hex(&bytes) != expected_sha256 {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_CAS_HASH_MISMATCH",
            format!("{role} CAS bytes do not match their content hash"),
        ));
    }
    Ok(bytes)
}

fn validate_main_payload(
    bytes: &[u8],
    record: &AuthoringMeshV2HighBridgeStoreRecord,
) -> Result<(), StoreError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_PAYLOAD_JSON_INVALID",
            format!("High bridge Main JSON is invalid: {error}"),
        )
    })?;
    let expected = main_value(record)?;
    let canonical =
        canonical_json_bytes(&value).map_err(|error| StoreError::InvalidData(error.to_string()))?;
    let expected_bytes = canonical_json_bytes(&expected)
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    if canonical != bytes
        || value != expected
        || bytes.len() as u64 > AUTHORING_MESH_V2_HIGH_MAX_BRIDGE_BYTES
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_PAYLOAD_BINDING_MISMATCH",
            "High bridge Main CAS bytes are not the exact canonical Main object",
        ));
    }
    if canonical_sha256(&value)? != record.canonical_sha256 {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_PAYLOAD_CANONICAL_MISMATCH",
            "High bridge Main CAS canonical hash differs from Store record",
        ));
    }
    // Keep this explicit: a future serializer change must not accidentally
    // make Store-only request identity part of the Main CAS preimage.
    if bytes != expected_bytes {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_PAYLOAD_SERIALIZATION_MISMATCH",
            "High bridge Main payload serialization is not canonical",
        ));
    }
    Ok(())
}

fn validate_semantic_json(
    bytes: &[u8],
    record: &AuthoringMeshV2HighBridgeStoreRecord,
    expected_semantic_sha256: &str,
    expected_schema_version: &str,
    role: &str,
) -> Result<Value, StoreError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_JSON_INVALID",
            format!("{role} JSON is invalid: {error}"),
        )
    })?;
    let canonical =
        canonical_json_bytes(&value).map_err(|error| StoreError::InvalidData(error.to_string()))?;
    if canonical != bytes
        || value.get("schema_version").and_then(Value::as_str) != Some(expected_schema_version)
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_CANONICAL_MISMATCH",
            format!("{role} is not canonical or has the wrong schema version"),
        ));
    }
    let semantic = value
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash));
    let computed = if let Some(semantic) = semantic {
        if canonical_sha256(&value)? != semantic {
            return Err(contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_CANONICAL_MISMATCH",
                format!("{role} canonical_sha256 preimage is invalid"),
            ));
        }
        semantic.to_owned()
    } else {
        canonical_json_hash(&value)
    };
    if computed != expected_semantic_sha256 {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_HASH_MISMATCH",
            format!("{role} semantic hash differs from the bridge record"),
        ));
    }

    // These fields are optional because the Store owns the closed bridge
    // envelope while the direct Worker result/readback are separate internal
    // contracts.  When present, they must still bind exactly to the durable
    // bridge; omission is not used to invent a positive quality claim.
    for (field, expected) in [
        (
            "high_evaluation_sha256",
            record.high_evaluation_sha256.as_str(),
        ),
        (
            "high_worker_algorithm_sha256",
            record.high_worker_algorithm_sha256.as_str(),
        ),
        (
            "high_worker_build_cohort_sha256",
            record.high_worker_build_cohort_sha256.as_str(),
        ),
        (
            "high_projected_source_mesh_sha256",
            record.high_projected_source_mesh_sha256.as_str(),
        ),
    ] {
        if let Some(actual) = value.get(field).and_then(Value::as_str) {
            if actual != expected {
                return Err(contract(
                    "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
                    format!("{role} {field} differs from the bridge record"),
                ));
            }
        }
    }
    if let Some(value) = value.get("replay_count").and_then(Value::as_u64) {
        if value != record.high_replay_count {
            return Err(contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
                format!("{role} replay_count differs from the bridge record"),
            ));
        }
    }
    Ok(value)
}

fn expect_string(value: &Value, field: &str, expected: &str, role: &str) -> Result<(), StoreError> {
    if value.get(field).and_then(Value::as_str) != Some(expected) {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
            format!("{role} {field} differs from the durable bridge"),
        ));
    }
    Ok(())
}

fn expect_u64(value: &Value, field: &str, expected: u64, role: &str) -> Result<(), StoreError> {
    if value.get(field).and_then(Value::as_u64) != Some(expected) {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
            format!("{role} {field} differs from the durable bridge"),
        ));
    }
    Ok(())
}

fn expect_bool(value: &Value, field: &str, expected: bool, role: &str) -> Result<(), StoreError> {
    if value.get(field).and_then(Value::as_bool) != Some(expected) {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
            format!("{role} {field} differs from the durable bridge"),
        ));
    }
    Ok(())
}

fn expect_array(value: &Value, field: &str, role: &str) -> Result<(), StoreError> {
    if !value.get(field).is_some_and(Value::is_array) {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
            format!("{role} {field} is not an array"),
        ));
    }
    Ok(())
}

fn expect_object(value: &Value, field: &str, role: &str) -> Result<(), StoreError> {
    if !value.get(field).is_some_and(Value::is_object) {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
            format!("{role} {field} is not an object"),
        ));
    }
    Ok(())
}

/// The direct Worker contracts are `deny_unknown_fields` Rust values.  Keep
/// that property at the Store boundary as well: a canonical JSON blob with a
/// valid hash is not sufficient if it has silently acquired a new field that
/// this repository does not understand.
fn expect_exact_fields(value: &Value, fields: &[&str], role: &str) -> Result<(), StoreError> {
    let object = value.as_object().ok_or_else(|| {
        contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
            format!("{role} is not an object"),
        )
    })?;
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
            format!("{role} has an unexpected or missing field"),
        ));
    }
    Ok(())
}

fn validate_embedded_canonical_hash(value: &Value, role: &str) -> Result<(), StoreError> {
    let actual = value
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| {
            contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_CANONICAL_MISMATCH",
                format!("{role} canonical_sha256 is missing or malformed"),
            )
        })?;
    let mut preimage = value.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != actual {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_CANONICAL_MISMATCH",
            format!("{role} canonical_sha256 preimage is invalid"),
        ));
    }
    Ok(())
}

fn validate_high_readback_shape(
    value: &Value,
    record: &AuthoringMeshV2HighBridgeStoreRecord,
) -> Result<(), StoreError> {
    let role = "High readback";
    expect_exact_fields(
        value,
        &[
            "schema_version",
            "mesh_id",
            "lineage_id",
            "revision_id",
            "revision_sha256",
            "projected_source_mesh_sha256",
            "source_vertex_count",
            "source_triangle_count",
            "evaluated_part_count",
            "evaluated_triangle_count",
            "high_evaluation_sha256",
            "high_worker_algorithm_sha256",
            "replay_count",
            "replay_byte_exact",
            "non_destructive",
            "runtime_write_performed",
            "production_stage_advanced",
            "candidate_confirmed",
            "version_created",
            "export_performed",
            "limitations",
            "canonical_sha256",
        ],
        role,
    )?;
    expect_string(
        value,
        "schema_version",
        &record.high_readback_schema_version,
        role,
    )?;
    for (field, expected) in [
        ("mesh_id", record.mesh_id.as_str()),
        ("lineage_id", record.lineage_id.as_str()),
        ("revision_id", record.revision_id.as_str()),
        ("revision_sha256", record.revision_sha256.as_str()),
        (
            "projected_source_mesh_sha256",
            record.high_projected_source_mesh_sha256.as_str(),
        ),
        (
            "high_evaluation_sha256",
            record.high_evaluation_sha256.as_str(),
        ),
        (
            "high_worker_algorithm_sha256",
            record.high_worker_algorithm_sha256.as_str(),
        ),
    ] {
        expect_string(value, field, expected, role)?;
    }
    for (field, expected) in [
        ("source_vertex_count", record.high_source_vertex_count),
        ("source_triangle_count", record.high_source_triangle_count),
        ("evaluated_part_count", record.high_evaluated_part_count),
        (
            "evaluated_triangle_count",
            record.high_evaluated_triangle_count,
        ),
        ("replay_count", record.high_replay_count),
    ] {
        expect_u64(value, field, expected, role)?;
    }
    for field in [
        "replay_byte_exact",
        "non_destructive",
        "runtime_write_performed",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
    ] {
        expect_bool(
            value,
            field,
            matches!(field, "replay_byte_exact" | "non_destructive"),
            role,
        )?;
    }
    expect_array(value, "limitations", role)?;
    validate_embedded_canonical_hash(value, role)
}

fn validate_high_result_shape(
    value: &Value,
    readback: &Value,
    record: &AuthoringMeshV2HighBridgeStoreRecord,
) -> Result<(), StoreError> {
    let role = "High result";
    expect_exact_fields(
        value,
        &[
            "schema_version",
            "operation",
            "mesh_id",
            "lineage_id",
            "revision_id",
            "revision_index",
            "revision_sha256",
            "high_worker_algorithm_sha256",
            "source_mesh",
            "evaluation",
            "readback",
            "replay_count",
            "replay_byte_exact",
            "non_destructive",
            "runtime_write_performed",
            "production_stage_advanced",
            "candidate_confirmed",
            "version_created",
            "export_performed",
            "quality_status",
            "limitations",
            "canonical_sha256",
        ],
        role,
    )?;
    expect_string(
        value,
        "schema_version",
        &record.high_result_schema_version,
        role,
    )?;
    expect_string(value, "operation", &record.high_operation, role)?;
    for (field, expected) in [
        ("mesh_id", record.mesh_id.as_str()),
        ("lineage_id", record.lineage_id.as_str()),
        ("revision_id", record.revision_id.as_str()),
        ("revision_sha256", record.revision_sha256.as_str()),
        (
            "high_worker_algorithm_sha256",
            record.high_worker_algorithm_sha256.as_str(),
        ),
        ("quality_status", "structural_only"),
    ] {
        expect_string(value, field, expected, role)?;
    }
    expect_u64(value, "revision_index", record.revision_index, role)?;
    for field in ["source_mesh", "evaluation", "readback"] {
        expect_object(value, field, role)?;
    }
    expect_array(value, "limitations", role)?;
    for field in [
        "replay_byte_exact",
        "non_destructive",
        "runtime_write_performed",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
    ] {
        expect_bool(
            value,
            field,
            matches!(field, "replay_byte_exact" | "non_destructive"),
            role,
        )?;
    }
    expect_u64(value, "replay_count", record.high_replay_count, role)?;

    let source_mesh = value.get("source_mesh").ok_or_else(|| {
        contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
            "High result source_mesh is missing",
        )
    })?;
    expect_exact_fields(
        source_mesh,
        &["schema_version", "parts"],
        "High result source_mesh",
    )?;
    expect_string(
        source_mesh,
        "schema_version",
        "HighEvaluatorSourceMesh@1",
        role,
    )?;
    let source_parts = source_mesh
        .get("parts")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
                "High result source_mesh.parts is missing",
            )
        })?;
    if source_parts.len() != record.preserved_part_ids.len().saturating_add(1) {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
            "High bridge source scope does not contain the complete materialized part set",
        ));
    }
    let expected_part_ids = record
        .preserved_part_ids
        .iter()
        .cloned()
        .chain(std::iter::once(record.part_id.clone()))
        .collect::<BTreeSet<_>>();
    let mut source_part_ids = BTreeSet::new();
    let mut source_vertex_count = 0_u64;
    let mut source_triangle_count = 0_u64;
    for source_part in source_parts {
        expect_exact_fields(
            source_part,
            &[
                "operand_id",
                "part_id",
                "source_node_ids",
                "source_node_id",
                "material_zone_id",
                "source_element_lineage",
                "positions_m",
                "indices",
            ],
            "High result source part",
        )?;
        let part_id = source_part
            .get("part_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                contract(
                    "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
                    "High result source part identity is missing",
                )
            })?;
        if !source_part_ids.insert(part_id.to_owned()) {
            return Err(contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
                "High result source part identity is duplicated",
            ));
        }
        let source_node_id = source_part
            .get("source_node_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                contract(
                    "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
                    "High result source node identity is missing",
                )
            })?;
        let source_node_ids = source_part
            .get("source_node_ids")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                contract(
                    "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
                    "High result source node identities are missing",
                )
            })?;
        if source_node_ids.is_empty()
            || source_node_ids.len() > 16
            || source_node_ids.first().and_then(Value::as_str) != Some(source_node_id)
            || source_node_ids
                .iter()
                .any(|value| value.as_str().is_none_or(|value| !is_opaque_id(value)))
            || source_node_ids
                .iter()
                .filter_map(Value::as_str)
                .collect::<BTreeSet<_>>()
                .len()
                != source_node_ids.len()
        {
            return Err(contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
                "High result source node identities are not closed and unique",
            ));
        }
        if part_id == record.part_id {
            expect_string(
                source_part,
                "material_zone_id",
                record.material_zone_id.as_str(),
                role,
            )?;
        }
        source_vertex_count = source_vertex_count.saturating_add(
            source_part
                .get("positions_m")
                .and_then(Value::as_array)
                .map_or(0, Vec::len) as u64,
        );
        source_triangle_count = source_triangle_count.saturating_add(
            source_part
                .get("indices")
                .and_then(Value::as_array)
                .map_or(0, Vec::len) as u64,
        );
    }
    if source_part_ids != expected_part_ids {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
            "High result source part identities differ from materialized lineage",
        ));
    }
    if source_vertex_count != record.high_source_vertex_count
        || source_triangle_count != record.high_source_triangle_count
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
            "High result source projection counts differ",
        ));
    }
    // The Worker source mesh hash is the canonical hash of this exact source
    // projection, not a caller-supplied label.
    if canonical_json_hash(source_mesh) != record.high_projected_source_mesh_sha256 {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
            "High result projected source mesh hash differs",
        ));
    }

    let evaluation = value.get("evaluation").ok_or_else(|| {
        contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
            "High result evaluation is missing",
        )
    })?;
    expect_exact_fields(
        evaluation,
        &[
            "schema_version",
            "operation",
            "source_mesh_sha256",
            "evaluator_contract",
            "module_descriptors",
            "base_parts",
            "evaluated_parts",
            "step_results",
            "base_triangle_count",
            "evaluated_triangle_count",
            "triangle_count",
            "replay_count",
            "replay_byte_exact",
            "non_destructive",
            "structural_status",
            "visual_status",
            "human_status",
            "quality_status",
            "runtime_write_performed",
            "production_stage_advanced",
            "candidate_confirmed",
            "version_created",
            "export_performed",
            "canonical_sha256",
        ],
        "High result evaluation",
    )?;
    for (field, expected) in [
        ("schema_version", "HighEvaluatorResult@1"),
        ("operation", AUTHORING_MESH_V2_HIGH_EVALUATOR_OPERATION),
        (
            "source_mesh_sha256",
            record.high_projected_source_mesh_sha256.as_str(),
        ),
        ("structural_status", "PASS_SOURCE_STRUCTURAL"),
        ("visual_status", "NOT_RUN"),
        ("human_status", "NOT_RUN"),
        ("quality_status", "structural_only"),
    ] {
        expect_string(evaluation, field, expected, role)?;
    }
    expect_u64(
        evaluation,
        "base_triangle_count",
        record.high_source_triangle_count,
        role,
    )?;
    expect_u64(
        evaluation,
        "evaluated_triangle_count",
        record.high_evaluated_triangle_count,
        role,
    )?;
    expect_u64(
        evaluation,
        "triangle_count",
        record
            .high_source_triangle_count
            .saturating_add(record.high_evaluated_triangle_count),
        role,
    )?;
    expect_u64(evaluation, "replay_count", record.high_replay_count, role)?;
    expect_object(evaluation, "evaluator_contract", role)?;
    for field in [
        "module_descriptors",
        "base_parts",
        "evaluated_parts",
        "step_results",
    ] {
        expect_array(evaluation, field, role)?;
    }
    let module_descriptors = evaluation
        .get("module_descriptors")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
                "High result module_descriptors is missing",
            )
        })?;
    for descriptor in module_descriptors {
        expect_exact_fields(
            descriptor,
            &[
                "schema_version",
                "module_id",
                "module_version",
                "availability",
                "backend",
                "source_revision",
                "license",
                "license_status",
                "operator_ids",
                "capabilities",
                "actual_third_party_link",
                "unavailable_reason",
                "module_sha256",
            ],
            "High result module descriptor",
        )?;
        expect_array(descriptor, "operator_ids", role)?;
        expect_object(descriptor, "capabilities", role)?;
        let capabilities = descriptor.get("capabilities").ok_or_else(|| {
            contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
                "High result module capabilities are missing",
            )
        })?;
        expect_exact_fields(
            capabilities,
            &[
                "network",
                "dynamic_plugin",
                "script",
                "direct_db_write",
                "direct_cas_write",
            ],
            "High result module capabilities",
        )?;
    }
    let step_results = evaluation
        .get("step_results")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
                "High result step_results is missing",
            )
        })?;
    for step_result in step_results {
        expect_exact_fields(
            step_result,
            &[
                "step_id",
                "kind",
                "module_id",
                "availability",
                "status",
                "output_part_id",
                "output_vertex_count",
                "output_triangle_count",
                "output_sha256",
                "error_code",
                "limitations",
            ],
            "High result evaluator step result",
        )?;
        expect_array(step_result, "limitations", role)?;
    }
    let evaluator_contract = evaluation.get("evaluator_contract").ok_or_else(|| {
        contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
            "High result evaluator contract is missing",
        )
    })?;
    expect_exact_fields(
        evaluator_contract,
        &[
            "schema_version",
            "policy",
            "topology",
            "continuity",
            "boundary_policy",
            "crease_policy",
            "adaptive_policy",
            "source_binding",
            "provenance",
            "deterministic_replay",
            "non_destructive",
            "max_subdivision_levels",
        ],
        "High result evaluator contract",
    )?;
    expect_string(
        evaluator_contract,
        "policy",
        &record.high_evaluator_contract,
        role,
    )?;
    expect_bool(evaluator_contract, "non_destructive", true, role)?;
    validate_embedded_canonical_hash(evaluation, role)?;
    let base_parts = evaluation
        .get("base_parts")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
                "High result evaluator base_parts is missing",
            )
        })?;
    if base_parts != source_parts {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
            "High result evaluator base_parts differ from source projection",
        ));
    }
    let evaluated_parts = evaluation
        .get("evaluated_parts")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
                "High result evaluator evaluated_parts is missing",
            )
        })?;
    if evaluated_parts.len() as u64 != record.high_evaluated_part_count {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
            "High result evaluated part count differs",
        ));
    }
    for (evaluated_part, source_part) in evaluated_parts.iter().zip(source_parts) {
        expect_exact_fields(
            evaluated_part,
            &[
                "output_part_id",
                "part_id",
                "source_node_ids",
                "source_node_id",
                "material_zone_id",
                "module_id",
                "source_operand_ids",
                "source_element_lineage",
                "positions_m",
                "indices",
            ],
            "High result evaluated part",
        )?;
        for field in [
            "part_id",
            "source_node_ids",
            "source_node_id",
            "material_zone_id",
        ] {
            if evaluated_part.get(field) != source_part.get(field) {
                return Err(contract(
                    "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
                    "High result evaluated part lineage differs from source projection",
                ));
            }
        }
        let source_lineage = source_part
            .get("source_element_lineage")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                contract(
                    "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
                    "High result source lineage is missing",
                )
            })?;
        let evaluated_lineage = evaluated_part
            .get("source_element_lineage")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                contract(
                    "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
                    "High result evaluated lineage is missing",
                )
            })?;
        let evaluated_lineage = evaluated_lineage
            .iter()
            .filter_map(Value::as_str)
            .collect::<BTreeSet<_>>();
        if evaluated_lineage.len()
            != evaluated_part["source_element_lineage"]
                .as_array()
                .map_or(0, Vec::len)
            || source_lineage.iter().any(|value| {
                value
                    .as_str()
                    .is_none_or(|value| !evaluated_lineage.contains(value))
            })
        {
            return Err(contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
                "High result evaluated lineage does not preserve source lineage",
            ));
        }
    }
    if value.get("readback") != Some(readback) {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_RESULT_BINDING_MISMATCH",
            "High result embedded readback differs from the separately persisted readback",
        ));
    }
    Ok(())
}

fn validate_source_binding_and_revision(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    record: &AuthoringMeshV2HighBridgeStoreRecord,
    require_reachable: bool,
) -> Result<Vec<String>, StoreError> {
    let source_binding_json: Option<String> = transaction
        .query_row(
            "SELECT record_json FROM knife_source_binding_records WHERE project_id = ?1 AND source_binding_id = ?2 AND source_binding_sha256 = ?3 AND source_binding_object_sha256 = ?4",
            params![
                record.project_id,
                record.source_binding_id,
                record.source_binding_sha256,
                record.source_binding_object_sha256
            ],
            |row| row.get(0),
        )
        .optional()?;
    let source_binding_json = source_binding_json.ok_or_else(|| {
        contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_SOURCE_BINDING_MISSING",
            "exact SourceBinding row is not durably registered",
        )
    })?;
    let source_binding: KnifeSourceBindingStoreRecord = serde_json::from_str(&source_binding_json)
        .map_err(|error| StoreError::InvalidData(format!("SourceBinding record JSON: {error}")))?;
    validate_source_binding_record(&source_binding)?;
    if source_binding.project_id != record.project_id
        || source_binding.source_binding_id != record.source_binding_id
        || source_binding.source_binding_sha256 != record.source_binding_sha256
        || source_binding.source_binding_object_sha256 != record.source_binding_object_sha256
        || source_binding.authoring_mesh_id != record.mesh_id
        || source_binding.authoring_mesh_lineage_id != record.lineage_id
        || source_binding.authoring_mesh_revision_id != record.revision_id
        || source_binding.authoring_mesh_revision_index != record.revision_index
        || source_binding.authoring_mesh_revision_sha256 != record.revision_sha256
        || source_binding.authoring_mesh_revision_object_sha256 != record.revision_object_sha256
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_SOURCE_BINDING_MISMATCH",
            "High bridge source binding fields differ from the immutable SourceBinding row",
        ));
    }

    let source_binding_object =
        read_object_record(transaction, &record.source_binding_object_sha256).map_err(|error| {
            match error {
                StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => contract(
                    "AUTHORING_MESH_V2_HIGH_BRIDGE_SOURCE_BINDING_CAS_MISSING",
                    "SourceBinding CAS object is not registered",
                ),
                other => other,
            }
        })?;
    let source_binding_bytes = validate_cas_metadata_and_bytes(
        transaction,
        cas,
        &source_binding_object,
        &record.source_binding_object_sha256,
        KNIFE_SOURCE_BINDING_OBJECT_KIND,
        KNIFE_SOURCE_BINDING_JSON_MIME,
        KNIFE_SOURCE_BINDING_MAX_JSON_BYTES,
        require_reachable,
        "SourceBinding",
    )?;
    let source_binding_value: Value = serde_json::from_slice(&source_binding_bytes)
        .map_err(|error| StoreError::InvalidData(format!("SourceBinding CAS JSON: {error}")))?;
    let expected_source_binding_payload = source_binding_payload_value(&source_binding)?;
    if canonical_json_bytes(&source_binding_value)
        .map_err(|error| StoreError::InvalidData(error.to_string()))?
        != source_binding_bytes
        || source_binding_value != expected_source_binding_payload
        || source_binding_value
            .get("schema_version")
            .and_then(Value::as_str)
            != Some(KNIFE_SOURCE_BINDING_SCHEMA_VERSION)
        || source_binding_value
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(record.source_binding_sha256.as_str())
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_SOURCE_BINDING_PAYLOAD_MISMATCH",
            "SourceBinding CAS bytes are not canonical and hash-bound",
        ));
    }
    let mut source_binding_preimage = source_binding_value.clone();
    source_binding_preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&source_binding_preimage) != record.source_binding_sha256 {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_SOURCE_BINDING_CANONICAL_MISMATCH",
            "SourceBinding semantic hash differs from its CAS preimage",
        ));
    }

    // The SourceBinding row is only an index.  Its intent, Brief, quality and
    // reference roots must also still exist and match before a High bridge can
    // retain them.  Keep this check inside the caller's transaction so a late
    // or tampered upstream root leaves no bridge row or reachability mutation.
    let upstream =
        validate_source_binding_upstream(transaction, cas, &source_binding, require_reachable)?;

    let source_idempotency_key = transaction
        .query_row(
            "SELECT idempotency_key FROM authoring_mesh_v2_durable_records WHERE project_id = ?1 AND mesh_id = ?2 AND lineage_id = ?3 AND revision_id = ?4 AND revision_index = ?5 AND revision_sha256 = ?6 AND revision_object_sha256 = ?7",
            params![
                record.project_id,
                record.mesh_id,
                record.lineage_id,
                record.revision_id,
                i64::try_from(record.revision_index).map_err(|_| StoreError::InvalidData("revision index too large".to_owned()))?,
                record.revision_sha256,
                record.revision_object_sha256,
            ],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let source_idempotency_key = source_idempotency_key.ok_or_else(|| {
        contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_REVISION_MISSING",
            "exact AuthoringMeshRevision@2 durable row is not registered",
        )
    })?;
    let revision_record = read_authoring_mesh_v2_record_in_transaction(
        transaction,
        &record.project_id,
        &source_idempotency_key,
    )?
    .ok_or_else(|| {
        contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_REVISION_MISSING",
            "AuthoringMeshRevision@2 durable row disappeared during validation",
        )
    })?;
    validate_authoring_mesh_v2_record_in_transaction(
        transaction,
        cas,
        &revision_record,
        require_reachable,
    )?;
    let revision_object = read_object_record(transaction, &record.revision_object_sha256)?;
    let revision_bytes = validate_authoring_mesh_v2_revision_object(
        cas,
        &revision_object,
        &record.revision_object_sha256,
        require_reachable,
    )?;
    let revision: AuthoringMeshRevision = serde_json::from_slice(&revision_bytes)
        .map_err(|error| StoreError::InvalidData(format!("V2 revision CAS JSON: {error}")))?;
    if revision.schema_version != AUTHORING_MESH_V2_HIGH_REVISION_SCHEMA_VERSION
        || revision.mesh_id.0 != record.mesh_id
        || revision.lineage_id.0 != record.lineage_id
        || revision.revision_id.0 != record.revision_id
        || revision.revision_index != record.revision_index
        || revision.canonical_sha256 != record.revision_sha256
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_REVISION_BINDING_MISMATCH",
            "Revision CAS identity differs from the High bridge",
        ));
    }
    let embedded = revision.source_binding.as_ref().ok_or_else(|| {
        contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_REVISION_SOURCE_MISSING",
            "High bridge revision has no embedded SourceBinding",
        )
    })?;
    if embedded.project_id != record.project_id
        || embedded.candidate_id != source_binding.source_candidate_id
        || embedded.candidate_state_sha256 != source_binding.source_candidate_state_sha256
        || embedded.source_node_id != record.source_node_id
        || embedded.part_id != record.part_id
        || embedded.material_zone_id != record.material_zone_id
        || embedded.solid != record.solid
        || embedded.part_output_sha256 != record.source_part_output_sha256
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_REVISION_SOURCE_MISMATCH",
            "embedded V2 SourceBinding does not match the High bridge target",
        ));
    }

    let mut roots = vec![
        record.source_binding_object_sha256.clone(),
        record.revision_object_sha256.clone(),
    ];
    roots.extend(upstream);
    Ok(roots)
}

/// Reproduce the bounded Runtime projection inputs that participate in the
/// materializer representation-plan hash.  This is intentionally derived
/// from the immutable revision and embedded SourceBinding, never from a
/// caller-supplied GeometryProgram or a GLB.  Keeping this calculation at the
/// Store boundary closes the otherwise dangerous gap where a valid-looking
/// `representation_plan_sha256` could be attached to an unrelated program.
fn materialization_geometry_parameters(
    revision: &AuthoringMeshRevision,
    position_m: [f64; 3],
    rotation_rad: [f64; 3],
) -> Result<Value, StoreError> {
    for (name, transform) in [("position_m", position_m), ("rotation_rad", rotation_rad)] {
        if transform
            .iter()
            .any(|component| !component.is_finite() || component.abs() > 10.0)
        {
            return Err(contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_REPRESENTATION_PLAN_INVALID",
                format!("materializer {name} is outside the bounded transform policy"),
            ));
        }
    }

    let edges_by_id = revision
        .original
        .edges
        .iter()
        .map(|edge| (edge.edge_id.0.as_str(), edge))
        .collect::<BTreeMap<_, _>>();
    let half_edges_by_id = revision
        .original
        .half_edges
        .iter()
        .map(|half_edge| (half_edge.half_edge_id.0.as_str(), half_edge))
        .collect::<BTreeMap<_, _>>();

    let mut loops = Vec::new();
    let mut faces = Vec::new();
    let mut referenced_vertex_ids = BTreeSet::new();
    let mut referenced_edge_ids = BTreeSet::new();
    for face in &revision.original.faces {
        if !(3..=4).contains(&face.half_edge_ids.len()) {
            return Err(contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_REPRESENTATION_PLAN_INVALID",
                "materializer projection only accepts triangle/quad faces",
            ));
        }
        let mut ordered = face
            .half_edge_ids
            .iter()
            .map(|id| {
                half_edges_by_id.get(id.0.as_str()).copied().ok_or_else(|| {
                    contract(
                        "AUTHORING_MESH_V2_HIGH_BRIDGE_REPRESENTATION_PLAN_INVALID",
                        "materializer projection references an unknown half-edge",
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let first = ordered
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| left.corner_id.0.cmp(&right.corner_id.0))
            .map(|(index, _)| index)
            .ok_or_else(|| {
                contract(
                    "AUTHORING_MESH_V2_HIGH_BRIDGE_REPRESENTATION_PLAN_INVALID",
                    "materializer projection encountered a face without half-edges",
                )
            })?;
        ordered.rotate_left(first);

        let mut face_loop_ids = Vec::with_capacity(ordered.len());
        for (ordinal, half_edge) in ordered.iter().enumerate() {
            let next = ordered[(ordinal + 1) % ordered.len()];
            let edge = edges_by_id
                .get(half_edge.edge_id.0.as_str())
                .copied()
                .ok_or_else(|| {
                    contract(
                        "AUTHORING_MESH_V2_HIGH_BRIDGE_REPRESENTATION_PLAN_INVALID",
                        "materializer projection references an unknown edge",
                    )
                })?;
            // Match the Runtime materializer's canonical edge orientation:
            // topology storage does not promise endpoint order, so direction
            // is defined against the lexicographically sorted stable IDs.
            let mut endpoints = [edge.vertex_ids[0].0.as_str(), edge.vertex_ids[1].0.as_str()];
            endpoints.sort();
            let origin = half_edge.origin_vertex_id.0.as_str();
            let target = next.origin_vertex_id.0.as_str();
            let edge_forward = if (origin == endpoints[0] && target == endpoints[1])
                || (origin == endpoints[1] && target == endpoints[0])
            {
                origin == endpoints[0] && target == endpoints[1]
            } else {
                return Err(contract(
                    "AUTHORING_MESH_V2_HIGH_BRIDGE_REPRESENTATION_PLAN_INVALID",
                    "materializer projection edge direction differs from its endpoints",
                ));
            };
            referenced_vertex_ids.insert(half_edge.origin_vertex_id.0.clone());
            referenced_edge_ids.insert(half_edge.edge_id.0.clone());
            face_loop_ids.push(half_edge.corner_id.0.clone());
            loops.push(json!({
                "element_id": half_edge.corner_id.0,
                "face_id": face.face_id.0,
                "ordinal": ordinal,
                "vertex_id": half_edge.origin_vertex_id.0,
                "edge_id": half_edge.edge_id.0,
                "edge_forward": edge_forward,
            }));
        }
        faces.push(json!({
            "element_id": face.face_id.0,
            "loop_ids": face_loop_ids,
        }));
    }
    loops.sort_by(|left, right| {
        left["element_id"]
            .as_str()
            .cmp(&right["element_id"].as_str())
    });

    let vertices = revision
        .original
        .vertices
        .iter()
        .filter(|vertex| referenced_vertex_ids.contains(&vertex.vertex_id.0))
        .map(|vertex| {
            json!({
                "element_id": vertex.vertex_id.0,
                "position_m": vertex.position_m,
            })
        })
        .collect::<Vec<_>>();
    let edges = revision
        .original
        .edges
        .iter()
        .filter(|edge| referenced_edge_ids.contains(&edge.edge_id.0))
        .map(|edge| {
            let mut endpoints = [edge.vertex_ids[0].0.clone(), edge.vertex_ids[1].0.clone()];
            endpoints.sort();
            json!({
                "element_id": edge.edge_id.0,
                "vertex_ids": endpoints,
            })
        })
        .collect::<Vec<_>>();

    Ok(json!({
        "shape": "authoring-mesh",
        "topology_policy": "triangle-quad-manifold-with-boundary@1",
        "vertices": vertices,
        "edges": edges,
        "loops": loops,
        "faces": faces,
        "position_m": position_m,
        "rotation_rad": rotation_rad,
    }))
}

fn materialization_projection_sha256(
    revision: &AuthoringMeshRevision,
    parameters: &Value,
) -> String {
    canonical_json_hash(&json!({
        "schema_version": "AuthoringMeshV2GeometryProjection@1",
        "revision_id": revision.revision_id.0,
        "revision_sha256": revision.canonical_sha256,
        "operator_id": AUTHORING_MESH_V2_MATERIALIZATION_OPERATOR_ID,
        "parameters": parameters,
    }))
}

fn expected_materialization_plan_sha256(
    record: &AuthoringMeshV2HighBridgeStoreRecord,
    source_binding: &AuthoringMeshV2SourceBinding,
    revision: &AuthoringMeshRevision,
    source_candidate_id: &str,
    source_candidate_state_sha256: &str,
    source_program: &Value,
    source_artifact_sha256: &str,
    source_artifact_readback_sha256: &str,
    source_program_sha256: &str,
    source_program_object_sha256: &str,
) -> Result<(String, String, Vec<String>), StoreError> {
    if source_binding.project_id != record.project_id
        || source_binding.candidate_id != source_candidate_id
        || source_binding.candidate_state_sha256 != source_candidate_state_sha256
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_REPRESENTATION_PLAN_INVALID",
            "embedded source binding candidate is outside the High bridge scope",
        ));
    }
    if revision.mesh_id.0 != record.mesh_id
        || revision.lineage_id.0 != record.lineage_id
        || revision.revision_id.0 != record.revision_id
        || revision.revision_index != record.revision_index
        || revision.canonical_sha256 != record.revision_sha256
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_REPRESENTATION_PLAN_INVALID",
            "replacement revision identity differs from the High bridge",
        ));
    }
    let parameters = materialization_geometry_parameters(
        revision,
        source_binding.position_m,
        source_binding.rotation_rad,
    )?;
    let replacement_projection_sha256 = materialization_projection_sha256(revision, &parameters);
    let replacement_identity = json!({
        "schema_version": "AuthoringMeshV2CandidateReplacementIdentity@1",
        "project_id": record.project_id,
        "mesh_id": record.mesh_id,
        "lineage_id": record.lineage_id,
        "materialization_mode": "source_binding_part_replacement",
        "revision_id": record.revision_id,
        "revision_sha256": record.revision_sha256,
        "revision_object_sha256": record.revision_object_sha256,
        "projection_sha256": replacement_projection_sha256,
        "source_binding_id": record.source_binding_id,
        "source_binding_sha256": record.source_binding_sha256,
        "source_node_id": source_binding.source_node_id,
        "source_part_id": source_binding.part_id,
    });
    let replacement_node_id = format!(
        "authoring-mesh-v2-{}",
        &canonical_json_hash(&replacement_identity)[..32]
    );

    let outputs = source_program
        .get("part_outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_REPRESENTATION_PLAN_INVALID",
                "source GeometryProgram part_outputs are missing",
            )
        })?;
    if outputs.is_empty() {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_REPRESENTATION_PLAN_INVALID",
            "source GeometryProgram has no Part outputs",
        ));
    }
    let mut output_ids = BTreeSet::new();
    for part in outputs {
        let part_id = part.get("part_id").and_then(Value::as_str).ok_or_else(|| {
            contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_REPRESENTATION_PLAN_INVALID",
                "source GeometryProgram Part id is missing",
            )
        })?;
        if !output_ids.insert(part_id.to_owned()) {
            return Err(contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_REPRESENTATION_PLAN_INVALID",
                "source GeometryProgram Part ids are duplicated",
            ));
        }
    }
    let mut preserved_part_ids = outputs
        .iter()
        .filter_map(|part| part.get("part_id").and_then(Value::as_str))
        .filter(|part_id| *part_id != record.part_id)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    preserved_part_ids.sort();
    preserved_part_ids.dedup();
    let plan = json!({
        "schema_version": AUTHORING_MESH_V2_MATERIALIZATION_PLAN_SCHEMA_VERSION,
        "project_id": record.project_id,
        "mesh_id": record.mesh_id,
        "lineage_id": record.lineage_id,
        "materialization_mode": "source_binding_part_replacement",
        "revision_id": record.revision_id,
        "revision_index": record.revision_index,
        "revision_sha256": record.revision_sha256,
        "revision_object_sha256": record.revision_object_sha256,
        "replacement_revision_id": record.revision_id,
        "replacement_revision_sha256": record.revision_sha256,
        "replacement_revision_object_sha256": record.revision_object_sha256,
        "replacement_projection_sha256": replacement_projection_sha256,
        "replacement_node_id": replacement_node_id,
        "source_candidate_id": source_candidate_id,
        "source_candidate_state_sha256": source_candidate_state_sha256,
        "source_artifact_sha256": source_artifact_sha256,
        "source_artifact_readback_sha256": source_artifact_readback_sha256,
        "source_program_sha256": source_program_sha256,
        "source_program_object_sha256": source_program_object_sha256,
        "source_binding_id": record.source_binding_id,
        "source_binding_sha256": record.source_binding_sha256,
        "source_binding_object_sha256": record.source_binding_object_sha256,
        "source_node_id": source_binding.source_node_id,
        "source_part_id": source_binding.part_id,
        "source_material_zone_id": source_binding.material_zone_id,
        "source_solid": source_binding.solid,
        "source_part_output_sha256": source_binding.part_output_sha256,
    });
    Ok((
        canonical_json_hash(&plan),
        replacement_node_id,
        preserved_part_ids,
    ))
}

/// The ArtifactReadback contract is keyed by the emitted `(Part, source node)`
/// pair, not merely by Part.  Keep that relation closed at this boundary so a
/// canonical readback cannot omit a source node or invent one while retaining
/// the same candidate/program hashes.
fn validate_artifact_readback_part_bindings(
    readback: &Value,
    outputs: &[Value],
    role: &str,
) -> Result<(), StoreError> {
    let bindings = readback
        .get("part_bindings")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_READBACK_BINDING_MISMATCH",
                format!("{role} part_bindings are missing"),
            )
        })?;
    let mut expected = BTreeSet::new();
    for part in outputs {
        let part_id = part.get("part_id").and_then(Value::as_str).ok_or_else(|| {
            contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_READBACK_BINDING_MISMATCH",
                format!("{role} Part id is missing"),
            )
        })?;
        let material_zone_id = part
            .get("material_zone_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                contract(
                    "AUTHORING_MESH_V2_HIGH_BRIDGE_READBACK_BINDING_MISMATCH",
                    format!("{role} Part material zone is missing"),
                )
            })?;
        let solid = part.get("solid").and_then(Value::as_bool).ok_or_else(|| {
            contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_READBACK_BINDING_MISMATCH",
                format!("{role} Part solid flag is missing"),
            )
        })?;
        let input_node_ids = part
            .get("input_node_ids")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                contract(
                    "AUTHORING_MESH_V2_HIGH_BRIDGE_READBACK_BINDING_MISMATCH",
                    format!("{role} Part input_node_ids are missing"),
                )
            })?;
        for node_id in input_node_ids {
            let node_id = node_id.as_str().ok_or_else(|| {
                contract(
                    "AUTHORING_MESH_V2_HIGH_BRIDGE_READBACK_BINDING_MISMATCH",
                    format!("{role} source node id is invalid"),
                )
            })?;
            if !expected.insert((
                part_id.to_owned(),
                node_id.to_owned(),
                material_zone_id.to_owned(),
                solid,
            )) {
                return Err(contract(
                    "AUTHORING_MESH_V2_HIGH_BRIDGE_READBACK_BINDING_MISMATCH",
                    format!("{role} source Part/node relation is duplicated"),
                ));
            }
        }
    }
    let mut actual = BTreeSet::new();
    for binding in bindings {
        expect_exact_fields(
            binding,
            &[
                "part_id",
                "source_node_id",
                "material_zone_id",
                "solid",
                "triangle_count",
            ],
            role,
        )?;
        let key = (
            binding
                .get("part_id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    contract(
                        "AUTHORING_MESH_V2_HIGH_BRIDGE_READBACK_BINDING_MISMATCH",
                        format!("{role} Part id is invalid"),
                    )
                })?
                .to_owned(),
            binding
                .get("source_node_id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    contract(
                        "AUTHORING_MESH_V2_HIGH_BRIDGE_READBACK_BINDING_MISMATCH",
                        format!("{role} source node id is invalid"),
                    )
                })?
                .to_owned(),
            binding
                .get("material_zone_id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    contract(
                        "AUTHORING_MESH_V2_HIGH_BRIDGE_READBACK_BINDING_MISMATCH",
                        format!("{role} material zone is invalid"),
                    )
                })?
                .to_owned(),
            binding
                .get("solid")
                .and_then(Value::as_bool)
                .ok_or_else(|| {
                    contract(
                        "AUTHORING_MESH_V2_HIGH_BRIDGE_READBACK_BINDING_MISMATCH",
                        format!("{role} solid flag is invalid"),
                    )
                })?,
        );
        if !actual.insert(key.clone()) || !expected.contains(&key) {
            return Err(contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_READBACK_BINDING_MISMATCH",
                format!("{role} Part/node relation is unknown or duplicated"),
            ));
        }
        if binding
            .get("triangle_count")
            .and_then(Value::as_u64)
            .is_none()
        {
            return Err(contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_READBACK_BINDING_MISMATCH",
                format!("{role} triangle_count is missing"),
            ));
        }
    }
    if actual != expected {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_READBACK_BINDING_MISMATCH",
            format!("{role} Part/node relation set is incomplete"),
        ));
    }
    Ok(())
}

struct SourceMaterializationInputs {
    embedded: AuthoringMeshV2SourceBinding,
    revision: AuthoringMeshRevision,
    program: Value,
    expected_plan_sha256: String,
    replacement_node_id: String,
    preserved_part_ids: Vec<String>,
    roots: Vec<String>,
}

/// Reload the source candidate owned by the immutable SourceBinding and prove
/// the exact inputs used by the materializer plan.  A High bridge may point at
/// a different, derived materialized candidate, but it may not skip this
/// source-side proof or treat the plan hash as a caller-supplied label.
fn load_source_materialization_inputs(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    record: &AuthoringMeshV2HighBridgeStoreRecord,
    require_reachable: bool,
) -> Result<SourceMaterializationInputs, StoreError> {
    let source_binding_json: Option<String> = transaction
        .query_row(
            "SELECT record_json FROM knife_source_binding_records WHERE project_id = ?1 AND source_binding_id = ?2 AND source_binding_sha256 = ?3 AND source_binding_object_sha256 = ?4",
            params![
                record.project_id,
                record.source_binding_id,
                record.source_binding_sha256,
                record.source_binding_object_sha256,
            ],
            |row| row.get(0),
        )
        .optional()?;
    let source_binding_json = source_binding_json.ok_or_else(|| {
        contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_SOURCE_BINDING_MISSING",
            "exact SourceBinding row is not durably registered",
        )
    })?;
    let binding: KnifeSourceBindingStoreRecord = serde_json::from_str(&source_binding_json)
        .map_err(|error| StoreError::InvalidData(format!("SourceBinding record JSON: {error}")))?;
    validate_source_binding_record(&binding)?;
    if binding.project_id != record.project_id
        || binding.source_binding_id != record.source_binding_id
        || binding.source_binding_sha256 != record.source_binding_sha256
        || binding.source_binding_object_sha256 != record.source_binding_object_sha256
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_SOURCE_BINDING_MISMATCH",
            "SourceBinding identity differs from the High bridge",
        ));
    }

    let source_binding_object =
        read_object_record(transaction, &binding.source_binding_object_sha256)?;
    let source_binding_bytes = validate_cas_metadata_and_bytes(
        transaction,
        cas,
        &source_binding_object,
        &binding.source_binding_object_sha256,
        KNIFE_SOURCE_BINDING_OBJECT_KIND,
        KNIFE_SOURCE_BINDING_JSON_MIME,
        KNIFE_SOURCE_BINDING_MAX_JSON_BYTES,
        require_reachable,
        "source materialization SourceBinding",
    )?;
    let source_binding_value: Value = serde_json::from_slice(&source_binding_bytes)
        .map_err(|error| StoreError::InvalidData(format!("SourceBinding CAS JSON: {error}")))?;
    let source_binding_preimage = {
        let mut value = source_binding_value.clone();
        value["canonical_sha256"] = Value::String(String::new());
        value
    };
    if canonical_json_bytes(&source_binding_value)
        .map_err(|error| StoreError::InvalidData(error.to_string()))?
        != source_binding_bytes
        || source_binding_value != source_binding_payload_value(&binding)?
        || canonical_json_hash(&source_binding_preimage) != record.source_binding_sha256
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_SOURCE_BINDING_PAYLOAD_MISMATCH",
            "SourceBinding CAS payload differs from its durable row",
        ));
    }

    let revision_idempotency_key: String = transaction
        .query_row(
            "SELECT idempotency_key FROM authoring_mesh_v2_durable_records WHERE project_id = ?1 AND mesh_id = ?2 AND lineage_id = ?3 AND revision_id = ?4 AND revision_index = ?5 AND revision_sha256 = ?6 AND revision_object_sha256 = ?7",
            params![
                record.project_id,
                record.mesh_id,
                record.lineage_id,
                record.revision_id,
                i64::try_from(record.revision_index).map_err(|_| StoreError::InvalidData("revision index too large".to_owned()))?,
                record.revision_sha256,
                record.revision_object_sha256,
            ],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| {
            contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_REVISION_MISSING",
                "exact source revision is not durably registered",
            )
        })?;
    let revision_record = read_authoring_mesh_v2_record_in_transaction(
        transaction,
        &record.project_id,
        &revision_idempotency_key,
    )?
    .ok_or_else(|| {
        contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_REVISION_MISSING",
            "source revision durable row disappeared during validation",
        )
    })?;
    let revision_object = read_object_record(transaction, &record.revision_object_sha256)?;
    validate_authoring_mesh_v2_record_in_transaction(
        transaction,
        cas,
        &revision_record,
        require_reachable,
    )?;
    let revision_bytes = validate_authoring_mesh_v2_revision_object(
        cas,
        &revision_object,
        &record.revision_object_sha256,
        require_reachable,
    )?;
    let revision: AuthoringMeshRevision = serde_json::from_slice(&revision_bytes)
        .map_err(|error| StoreError::InvalidData(format!("V2 revision JSON: {error}")))?;
    if revision.schema_version != AUTHORING_MESH_V2_HIGH_REVISION_SCHEMA_VERSION
        || revision.mesh_id.0 != record.mesh_id
        || revision.lineage_id.0 != record.lineage_id
        || revision.revision_id.0 != record.revision_id
        || revision.revision_index != record.revision_index
        || revision.canonical_sha256 != record.revision_sha256
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_REVISION_BINDING_MISMATCH",
            "source revision identity differs from the High bridge",
        ));
    }
    let embedded = revision.source_binding.clone().ok_or_else(|| {
        contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_REVISION_SOURCE_MISSING",
            "source revision has no embedded SourceBinding",
        )
    })?;
    if embedded.project_id != binding.project_id
        || embedded.candidate_id != binding.source_candidate_id
        || embedded.candidate_state_sha256 != binding.source_candidate_state_sha256
        || embedded.artifact_id.is_empty()
        || embedded.artifact_sha256.is_empty()
        || embedded.geometry_program_sha256.is_empty()
        || embedded.source_node_id != record.source_node_id
        || embedded.part_id != record.part_id
        || embedded.material_zone_id != record.material_zone_id
        || embedded.solid != record.solid
        || embedded.part_output_sha256 != record.source_part_output_sha256
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_REVISION_SOURCE_MISMATCH",
            "embedded source binding differs from the SourceBinding/High target",
        ));
    }

    let candidate: (String, String, String, Option<String>, bool) = transaction
        .query_row(
            "SELECT project_id, state, canonical_sha256, prepared_object_sha256, quality_hard_gate_passed FROM candidates WHERE candidate_id = ?1",
            params![binding.source_candidate_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get::<_, i64>(4)? != 0)),
        )
        .optional()?
        .ok_or_else(|| {
            contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_SOURCE_CANDIDATE_MISSING",
                "SourceBinding source candidate is not durably registered",
            )
        })?;
    let evidence: (String, Option<String>, Option<String>, String, String, String, String) =
        transaction
            .query_row(
                "SELECT project_id, reference_id, reference_sha256, geometry_program_sha256, geometry_program_object_sha256, artifact_object_sha256, artifact_readback_object_sha256 FROM geometry_candidate_evidence WHERE candidate_id = ?1",
                params![binding.source_candidate_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| {
                contract(
                    "AUTHORING_MESH_V2_HIGH_BRIDGE_SOURCE_EVIDENCE_MISSING",
                    "SourceBinding source candidate evidence is not durably registered",
                )
            })?;
    if candidate.0 != binding.project_id
        || !matches!(candidate.1.as_str(), "prepared" | "reviewable")
        || candidate.2 != binding.source_candidate_state_sha256
        || !candidate.4
        || candidate.3.as_deref() != Some(evidence.5.as_str())
        || evidence.0 != binding.project_id
        || evidence.1.as_deref() != Some(binding.reference_id.as_str())
        || evidence.2.as_deref() != Some(binding.reference_object_sha256.as_str())
        || evidence.3 != embedded.geometry_program_sha256
        || evidence.5 != embedded.artifact_sha256
        || embedded.artifact_readback_sha256.is_empty()
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_SOURCE_EVIDENCE_MISMATCH",
            "Source candidate state/evidence differs from embedded SourceBinding",
        ));
    }

    let program_object = read_object_record(transaction, &evidence.4)?;
    let program_bytes = validate_cas_metadata_and_bytes(
        transaction,
        cas,
        &program_object,
        &evidence.4,
        "geometry-program-v2",
        AUTHORING_MESH_V2_HIGH_JSON_MIME,
        64 * 1024 * 1024,
        require_reachable,
        "source materialization GeometryProgram",
    )?;
    let program: Value = serde_json::from_slice(&program_bytes).map_err(|error| {
        StoreError::InvalidData(format!("source GeometryProgram JSON: {error}"))
    })?;
    if !program.is_object()
        || program.get("canonical_sha256").is_some()
        || canonical_json_bytes(&program)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?
            != program_bytes
        || canonical_json_hash(&program) != evidence.3
        || program.get("schema_version").and_then(Value::as_str) != Some("GeometryProgram@2")
        || program.get("project_id").and_then(Value::as_str) != Some(record.project_id.as_str())
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_SOURCE_PROGRAM_MISMATCH",
            "source GeometryProgram is not canonical or has a drifted semantic hash",
        ));
    }
    let nodes = program
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_SOURCE_PROGRAM_MISMATCH",
                "source GeometryProgram nodes are missing",
            )
        })?;
    let matching_nodes = nodes
        .iter()
        .filter(|node| {
            node.get("node_id").and_then(Value::as_str) == Some(embedded.source_node_id.as_str())
        })
        .collect::<Vec<_>>();
    if matching_nodes.len() != 1
        || matching_nodes[0].get("operator_id").and_then(Value::as_str)
            != Some(embedded.source_operator_id.as_str())
        || matching_nodes[0]
            .get("inputs")
            .and_then(Value::as_array)
            .is_none_or(|inputs| !inputs.is_empty())
        || matching_nodes[0]
            .get("parameters")
            .is_none_or(|parameters| {
                canonical_json_hash(parameters) != embedded.source_parameters_sha256
            })
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_SOURCE_PROGRAM_MISMATCH",
            "source node is missing, duplicated or semantically drifted",
        ));
    }
    let outputs = program
        .get("part_outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_SOURCE_PROGRAM_MISMATCH",
                "source GeometryProgram part_outputs are missing",
            )
        })?;
    let mut seen_parts = BTreeSet::new();
    for part in outputs {
        let part_id = part.get("part_id").and_then(Value::as_str).ok_or_else(|| {
            contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_SOURCE_PROGRAM_MISMATCH",
                "source Part output id is missing",
            )
        })?;
        if !seen_parts.insert(part_id.to_owned()) {
            return Err(contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_SOURCE_PROGRAM_MISMATCH",
                "source Part output ids are duplicated",
            ));
        }
    }
    let selected_part = outputs
        .iter()
        .filter(|part| {
            part.get("part_id").and_then(Value::as_str) == Some(embedded.part_id.as_str())
        })
        .collect::<Vec<_>>();
    if selected_part.len() != 1
        || selected_part[0]
            .get("material_zone_id")
            .and_then(Value::as_str)
            != Some(embedded.material_zone_id.as_str())
        || selected_part[0].get("solid").and_then(Value::as_bool) != Some(embedded.solid)
        || selected_part[0]
            .get("input_node_ids")
            .and_then(Value::as_array)
            .is_none_or(|inputs| {
                inputs
                    .iter()
                    .filter(|id| id.as_str() == Some(embedded.source_node_id.as_str()))
                    .count()
                    != 1
            })
        || canonical_json_hash(selected_part[0]) != embedded.part_output_sha256
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_SOURCE_PROGRAM_MISMATCH",
            "source target Part output is missing, duplicated or semantically drifted",
        ));
    }

    let artifact_object = read_object_record(transaction, &evidence.5)?;
    let artifact_bytes = validate_cas_metadata_and_bytes(
        transaction,
        cas,
        &artifact_object,
        &evidence.5,
        "geometry-glb",
        "model/gltf-binary",
        64 * 1024 * 1024,
        require_reachable,
        "source materialization artifact",
    )?;
    if sha256_hex(&artifact_bytes) != evidence.5 {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_SOURCE_ARTIFACT_MISMATCH",
            "source candidate artifact bytes are not hash-bound",
        ));
    }
    let readback_object = read_object_record(transaction, &evidence.6)?;
    let readback_bytes = validate_cas_metadata_and_bytes(
        transaction,
        cas,
        &readback_object,
        &evidence.6,
        "geometry-artifact-readback-v2",
        AUTHORING_MESH_V2_HIGH_JSON_MIME,
        8 * 1024 * 1024,
        require_reachable,
        "source materialization artifact readback",
    )?;
    let readback: Value = serde_json::from_slice(&readback_bytes).map_err(|error| {
        StoreError::InvalidData(format!("source ArtifactReadback JSON: {error}"))
    })?;
    if canonical_json_bytes(&readback)
        .map_err(|error| StoreError::InvalidData(error.to_string()))?
        != readback_bytes
        || readback.get("schema_version").and_then(Value::as_str) != Some("ArtifactReadback@2")
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_SOURCE_READBACK_MISMATCH",
            "source ArtifactReadback is not canonical",
        ));
    }
    validate_embedded_canonical_hash(&readback, "source ArtifactReadback")?;
    let source_readback_sha256 = readback
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_SOURCE_READBACK_MISMATCH",
                "source ArtifactReadback semantic hash is missing",
            )
        })?;
    if source_readback_sha256 != embedded.artifact_readback_sha256
        || readback.get("candidate_id").and_then(Value::as_str)
            != Some(binding.source_candidate_id.as_str())
        || readback.get("object_sha256").and_then(Value::as_str) != Some(evidence.5.as_str())
        || readback.get("program_sha256").and_then(Value::as_str) != Some(evidence.3.as_str())
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_SOURCE_READBACK_MISMATCH",
            "source ArtifactReadback does not bind the exact candidate artifact/program",
        ));
    }
    validate_artifact_readback_part_bindings(&readback, outputs, "source ArtifactReadback")?;

    let (expected_plan_sha256, replacement_node_id, preserved_part_ids) =
        expected_materialization_plan_sha256(
            record,
            &embedded,
            &revision,
            &binding.source_candidate_id,
            &binding.source_candidate_state_sha256,
            &program,
            &evidence.5,
            source_readback_sha256,
            &evidence.3,
            &evidence.4,
        )?;
    if record.preserved_part_ids != preserved_part_ids {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_REPRESENTATION_PLAN_MISMATCH",
            "High bridge preserved_part_ids differ from the source program",
        ));
    }
    let mut roots = vec![evidence.4.clone(), evidence.5.clone(), evidence.6.clone()];
    roots.push(binding.source_binding_object_sha256.clone());
    roots.sort();
    roots.dedup();
    Ok(SourceMaterializationInputs {
        embedded,
        revision,
        program,
        expected_plan_sha256,
        replacement_node_id,
        preserved_part_ids,
        roots,
    })
}

fn validate_materialized_candidate_lineage(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    record: &AuthoringMeshV2HighBridgeStoreRecord,
    require_reachable: bool,
) -> Result<Vec<String>, StoreError> {
    let source_inputs =
        load_source_materialization_inputs(transaction, cas, record, require_reachable)?;
    if source_inputs.expected_plan_sha256 != record.representation_plan_sha256 {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_REPRESENTATION_PLAN_MISMATCH",
            format!(
                "representation_plan_sha256 is not derived from the exact source lineage: expected {}, received {}",
                source_inputs.expected_plan_sha256, record.representation_plan_sha256
            ),
        ));
    }
    let candidate: Option<(String, String, String, Option<String>, Option<String>, bool)> =
        transaction
            .query_row(
                "SELECT project_id, canonical_sha256, state, prepared_object_sha256, manifest_hash, quality_hard_gate_passed FROM candidates WHERE candidate_id = ?1",
                params![record.materialized_candidate_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get::<_, i64>(5)? != 0)),
            )
            .optional()?;
    let Some((project_id, state_sha256, state, prepared_object, manifest_hash, hard_gate)) =
        candidate
    else {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_CANDIDATE_MISSING",
            "materialized candidate is not durably registered",
        ));
    };
    if project_id != record.project_id
        || state_sha256 != record.materialized_candidate_state_sha256
        || !matches!(state.as_str(), "prepared" | "reviewable")
        || !hard_gate
        || prepared_object.as_deref() != Some(record.materialized_artifact_object_sha256.as_str())
        || manifest_hash
            .as_deref()
            .is_some_and(|hash| hash != record.materialized_artifact_object_sha256)
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_CANDIDATE_BINDING_MISMATCH",
            "materialized candidate state/artifact binding differs",
        ));
    }

    let evidence: Option<(String, Option<String>, Option<String>, String, String, String, String)> =
        transaction
            .query_row(
                "SELECT project_id, reference_id, reference_sha256, geometry_program_sha256, geometry_program_object_sha256, artifact_object_sha256, artifact_readback_object_sha256 FROM geometry_candidate_evidence WHERE candidate_id = ?1",
                params![record.materialized_candidate_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)),
            )
            .optional()?;
    let Some((
        evidence_project,
        _reference_id,
        _reference_sha256,
        program_sha256,
        program_object_sha256,
        artifact_object_sha256,
        readback_object_sha256,
    )) = evidence
    else {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_CANDIDATE_EVIDENCE_MISSING",
            "materialized candidate GeometryCandidateEvidence is not registered",
        ));
    };
    if evidence_project != record.project_id
        || program_sha256 != record.materialized_program_sha256
        || program_object_sha256 != record.materialized_program_object_sha256
        || artifact_object_sha256 != record.materialized_artifact_object_sha256
        || readback_object_sha256 != record.materialized_artifact_readback_object_sha256
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_CANDIDATE_EVIDENCE_BINDING_MISMATCH",
            "materialized candidate evidence hashes differ from the bridge",
        ));
    }

    let program_object =
        read_object_record(transaction, &record.materialized_program_object_sha256)?;
    let program_bytes = validate_cas_metadata_and_bytes(
        transaction,
        cas,
        &program_object,
        &record.materialized_program_object_sha256,
        "geometry-program-v2",
        AUTHORING_MESH_V2_HIGH_JSON_MIME,
        64 * 1024 * 1024,
        require_reachable,
        "materialized GeometryProgram",
    )?;
    let program: Value = serde_json::from_slice(&program_bytes).map_err(|error| {
        StoreError::InvalidData(format!("materialized GeometryProgram JSON: {error}"))
    })?;
    if !program.is_object()
        || program.get("schema_version").and_then(Value::as_str) != Some("GeometryProgram@2")
        || program.get("canonical_sha256").is_some()
        || canonical_json_bytes(&program)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?
            != program_bytes
        || canonical_json_hash(&program) != record.materialized_program_sha256
        || program.get("project_id").and_then(Value::as_str) != Some(record.project_id.as_str())
        || program
            .get("representation_plan_sha256")
            .and_then(Value::as_str)
            != Some(record.representation_plan_sha256.as_str())
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_PROGRAM_BINDING_MISMATCH",
            "materialized GeometryProgram draft or representation plan differs",
        ));
    }
    let outputs = program
        .get("part_outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_PROGRAM_PARTS_MISSING",
                "materialized GeometryProgram part_outputs are missing",
            )
        })?;
    let source_outputs = source_inputs
        .program
        .get("part_outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_PROGRAM_PARTS_MISSING",
                "source materialization GeometryProgram part_outputs are missing",
            )
        })?;
    if outputs.len() != source_outputs.len()
        || record.preserved_part_ids != source_inputs.preserved_part_ids
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_PROGRAM_PARTS_MISMATCH",
            "materialized GeometryProgram Part set differs from the source program",
        ));
    }
    let mut materialized_part_ids = BTreeSet::new();
    for part in outputs {
        let part_id = part.get("part_id").and_then(Value::as_str).ok_or_else(|| {
            contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_PROGRAM_PARTS_MISMATCH",
                "materialized GeometryProgram Part id is missing",
            )
        })?;
        if !materialized_part_ids.insert(part_id.to_owned()) {
            return Err(contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_PROGRAM_PARTS_MISMATCH",
                "materialized GeometryProgram Part ids are duplicated",
            ));
        }
        if part_id != record.part_id {
            let source_part = source_outputs
                .iter()
                .find(|source| source.get("part_id").and_then(Value::as_str) == Some(part_id))
                .ok_or_else(|| {
                    contract(
                        "AUTHORING_MESH_V2_HIGH_BRIDGE_PROGRAM_PARTS_MISMATCH",
                        "materialized GeometryProgram introduced an unknown preserved Part",
                    )
                })?;
            if part != source_part {
                return Err(contract(
                    "AUTHORING_MESH_V2_HIGH_BRIDGE_PROGRAM_PARTS_MISMATCH",
                    "untouched source Part output changed during materialization",
                ));
            }
        }
    }
    let source_part_ids = source_outputs
        .iter()
        .filter_map(|part| part.get("part_id").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    if materialized_part_ids.len() != source_part_ids.len()
        || materialized_part_ids
            .iter()
            .any(|part_id| !source_part_ids.contains(part_id.as_str()))
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_PROGRAM_PARTS_MISMATCH",
            "materialized GeometryProgram Part identity set is not exact",
        ));
    }
    let selected = outputs
        .iter()
        .filter(|part| part.get("part_id").and_then(Value::as_str) == Some(record.part_id.as_str()))
        .collect::<Vec<_>>();
    let source_selected = source_outputs
        .iter()
        .find(|part| part.get("part_id").and_then(Value::as_str) == Some(record.part_id.as_str()))
        .ok_or_else(|| {
            contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_PROGRAM_TARGET_MISMATCH",
                "source target Part output is missing",
            )
        })?;
    let source_input_node_ids = source_selected
        .get("input_node_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_PROGRAM_TARGET_MISMATCH",
                "source target Part input nodes are missing",
            )
        })?;
    let mut expected_target_inputs = source_input_node_ids.clone();
    let source_node_occurrences = expected_target_inputs
        .iter()
        .filter(|node| node.as_str() == Some(source_inputs.embedded.source_node_id.as_str()))
        .count();
    if source_node_occurrences != 1 {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_PROGRAM_TARGET_MISMATCH",
            "source target Part does not reference the selected source node exactly once",
        ));
    }
    for node in &mut expected_target_inputs {
        if node.as_str() == Some(source_inputs.embedded.source_node_id.as_str()) {
            *node = Value::String(source_inputs.replacement_node_id.clone());
        }
    }
    if selected.len() != 1
        || selected[0].get("material_zone_id").and_then(Value::as_str)
            != Some(record.material_zone_id.as_str())
        || selected[0].get("solid").and_then(Value::as_bool) != Some(record.solid)
        || selected[0].get("input_node_ids") != Some(&Value::Array(expected_target_inputs))
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_PROGRAM_TARGET_MISMATCH",
            "materialized GeometryProgram target Part is missing or drifted",
        ));
    }

    let source_nodes = source_inputs
        .program
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_PROGRAM_NODES_MISSING",
                "source materialization GeometryProgram nodes are missing",
            )
        })?;
    let materialized_nodes = program
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_PROGRAM_NODES_MISSING",
                "materialized GeometryProgram nodes are missing",
            )
        })?;
    if materialized_nodes.len() != source_nodes.len()
        || materialized_nodes
            .iter()
            .filter(|node| {
                node.get("node_id").and_then(Value::as_str)
                    == Some(source_inputs.replacement_node_id.as_str())
            })
            .count()
            != 1
        || materialized_nodes.iter().any(|node| {
            node.get("node_id").and_then(Value::as_str)
                == Some(source_inputs.embedded.source_node_id.as_str())
        })
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_PROGRAM_NODES_MISMATCH",
            "materialized GeometryProgram node replacement set is not exact",
        ));
    }
    for source_node in source_nodes {
        let source_node_id = source_node
            .get("node_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                contract(
                    "AUTHORING_MESH_V2_HIGH_BRIDGE_PROGRAM_NODES_MISMATCH",
                    "source node id is missing",
                )
            })?;
        if source_node_id == source_inputs.embedded.source_node_id {
            continue;
        }
        let matching = materialized_nodes
            .iter()
            .find(|node| node.get("node_id").and_then(Value::as_str) == Some(source_node_id));
        if matching != Some(source_node) {
            return Err(contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_PROGRAM_NODES_MISMATCH",
                "untouched source GeometryProgram node changed during materialization",
            ));
        }
    }
    let replacement_node = materialized_nodes
        .iter()
        .find(|node| {
            node.get("node_id").and_then(Value::as_str)
                == Some(source_inputs.replacement_node_id.as_str())
        })
        .ok_or_else(|| {
            contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_PROGRAM_NODES_MISMATCH",
                "materialized replacement node is missing",
            )
        })?;
    if replacement_node.get("operator_id").and_then(Value::as_str)
        != Some(AUTHORING_MESH_V2_MATERIALIZATION_OPERATOR_ID)
        || replacement_node
            .get("inputs")
            .and_then(Value::as_array)
            .is_none_or(|inputs| !inputs.is_empty())
        || replacement_node.get("parameters")
            != Some(&materialization_geometry_parameters(
                &source_inputs.revision,
                source_inputs.embedded.position_m,
                source_inputs.embedded.rotation_rad,
            )?)
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_PROGRAM_NODES_MISMATCH",
            "materialized replacement node policy differs",
        ));
    }
    let artifact_object =
        read_object_record(transaction, &record.materialized_artifact_object_sha256)?;
    let artifact_bytes = validate_cas_metadata_and_bytes(
        transaction,
        cas,
        &artifact_object,
        &record.materialized_artifact_object_sha256,
        "geometry-glb",
        "model/gltf-binary",
        64 * 1024 * 1024,
        require_reachable,
        "materialized geometry artifact",
    )?;
    if record.materialized_artifact_sha256 != record.materialized_artifact_object_sha256
        || sha256_hex(&artifact_bytes) != record.materialized_artifact_sha256
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_ARTIFACT_BINDING_MISMATCH",
            "materialized artifact semantic/object hash policy is violated",
        ));
    }

    let readback_object = read_object_record(
        transaction,
        &record.materialized_artifact_readback_object_sha256,
    )?;
    let readback_bytes = validate_cas_metadata_and_bytes(
        transaction,
        cas,
        &readback_object,
        &record.materialized_artifact_readback_object_sha256,
        "geometry-artifact-readback-v2",
        AUTHORING_MESH_V2_HIGH_JSON_MIME,
        8 * 1024 * 1024,
        require_reachable,
        "materialized artifact readback",
    )?;
    let readback: Value = serde_json::from_slice(&readback_bytes).map_err(|error| {
        StoreError::InvalidData(format!("materialized ArtifactReadback JSON: {error}"))
    })?;
    if canonical_json_bytes(&readback)
        .map_err(|error| StoreError::InvalidData(error.to_string()))?
        != readback_bytes
        || readback.get("canonical_sha256").and_then(Value::as_str)
            != Some(record.materialized_artifact_readback_sha256.as_str())
        || canonical_sha256(&readback)? != record.materialized_artifact_readback_sha256
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_BRIDGE_READBACK_BINDING_MISMATCH",
            "materialized ArtifactReadback semantic/object binding is invalid",
        ));
    }
    for (field, expected) in [
        ("candidate_id", record.materialized_candidate_id.as_str()),
        (
            "object_sha256",
            record.materialized_artifact_object_sha256.as_str(),
        ),
        (
            "program_sha256",
            record.materialized_program_sha256.as_str(),
        ),
    ] {
        if readback.get(field).and_then(Value::as_str) != Some(expected) {
            return Err(contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_READBACK_BINDING_MISMATCH",
                format!("materialized ArtifactReadback {field} is missing or differs"),
            ));
        }
    }
    validate_artifact_readback_part_bindings(&readback, outputs, "materialized ArtifactReadback")?;

    let mut roots = vec![
        record.materialized_program_object_sha256.clone(),
        record.materialized_artifact_object_sha256.clone(),
        record.materialized_artifact_readback_object_sha256.clone(),
    ];
    roots.extend(source_inputs.roots.iter().cloned());
    roots.sort();
    roots.dedup();
    Ok(roots)
}

fn high_roots(record: &AuthoringMeshV2HighBridgeStoreRecord, lineage: &[String]) -> Vec<String> {
    let mut roots = vec![
        record.bridge_object_sha256.clone(),
        record.high_result_object_sha256.clone(),
        record.high_readback_object_sha256.clone(),
        record.source_binding_object_sha256.clone(),
        // SourceBinding is the authoritative owner of the intent/brief/
        // reference/quality roots.  Carry those roots into the High row as
        // well so a High-only readback cannot make its upstream evidence
        // collectible.
        record.revision_object_sha256.clone(),
        record.materialized_program_object_sha256.clone(),
        record.materialized_artifact_object_sha256.clone(),
        record.materialized_artifact_readback_object_sha256.clone(),
    ];
    roots.extend(lineage.iter().cloned());
    roots.sort();
    roots.dedup();
    roots
}

fn read_record_in_transaction(
    transaction: &Transaction<'_>,
    project_id: &str,
    idempotency_key: &str,
) -> Result<Option<AuthoringMeshV2HighBridgeStoreRecord>, StoreError> {
    let record_json: Option<String> = transaction
        .query_row(
            "SELECT record_json FROM authoring_mesh_v2_high_bridge_records WHERE project_id = ?1 AND idempotency_key = ?2",
            params![project_id, idempotency_key],
            |row| row.get(0),
        )
        .optional()?;
    let Some(record_json) = record_json else {
        return Ok(None);
    };
    let record = serde_json::from_str(&record_json)
        .map_err(|error| StoreError::InvalidData(format!("High bridge record JSON: {error}")))?;
    Ok(Some(record))
}

pub(crate) fn ensure_table(transaction: &Transaction<'_>) -> Result<(), StoreError> {
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS authoring_mesh_v2_high_bridge_records (
             schema_version TEXT NOT NULL CHECK (schema_version = 'AuthoringMeshV2HighBridge@1'),
             project_id TEXT NOT NULL REFERENCES projects(project_id),
             bridge_id TEXT NOT NULL,
             bridge_sha256 TEXT NOT NULL,
             bridge_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             source_binding_id TEXT NOT NULL,
             source_binding_sha256 TEXT NOT NULL,
             source_binding_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             mesh_id TEXT NOT NULL,
             lineage_id TEXT NOT NULL,
             revision_id TEXT NOT NULL,
             revision_index INTEGER NOT NULL CHECK (revision_index BETWEEN 0 AND 1000000),
             revision_sha256 TEXT NOT NULL,
             revision_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             materialized_candidate_id TEXT NOT NULL REFERENCES candidates(candidate_id),
             materialized_candidate_state_sha256 TEXT NOT NULL,
             materialized_program_sha256 TEXT NOT NULL,
             materialized_program_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             materialized_artifact_id TEXT NOT NULL,
             materialized_artifact_sha256 TEXT NOT NULL,
             materialized_artifact_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             materialized_artifact_readback_sha256 TEXT NOT NULL,
             materialized_artifact_readback_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             representation_plan_sha256 TEXT NOT NULL,
             source_node_id TEXT NOT NULL,
             part_id TEXT NOT NULL,
             material_zone_id TEXT NOT NULL,
             source_part_output_sha256 TEXT NOT NULL,
             high_execution_request_sha256 TEXT NOT NULL,
             high_evaluation_sha256 TEXT NOT NULL,
             high_result_sha256 TEXT NOT NULL,
             high_result_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             high_readback_sha256 TEXT NOT NULL,
             high_readback_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             high_worker_algorithm_sha256 TEXT NOT NULL,
             high_worker_build_cohort_sha256 TEXT NOT NULL,
             request_input_sha256 TEXT NOT NULL,
             idempotency_key TEXT NOT NULL,
             canonical_sha256 TEXT NOT NULL,
             created_at TEXT NOT NULL,
             record_json TEXT NOT NULL,
             object_hashes_json TEXT NOT NULL,
             PRIMARY KEY (project_id, idempotency_key),
             UNIQUE (project_id, bridge_id),
             UNIQUE (project_id, bridge_sha256)
         );
         CREATE INDEX IF NOT EXISTS authoring_mesh_v2_high_bridge_source_idx
             ON authoring_mesh_v2_high_bridge_records(project_id, source_binding_id, revision_id, revision_index);
         CREATE INDEX IF NOT EXISTS authoring_mesh_v2_high_bridge_candidate_idx
             ON authoring_mesh_v2_high_bridge_records(project_id, materialized_candidate_id, materialized_artifact_id);
         CREATE INDEX IF NOT EXISTS authoring_mesh_v2_high_bridge_object_idx
             ON authoring_mesh_v2_high_bridge_records(bridge_object_sha256, high_result_object_sha256, high_readback_object_sha256);",
    )?;
    Ok(())
}

fn stored_object_hashes(
    record: &AuthoringMeshV2HighBridgeStoreRecord,
    lineage: &[String],
) -> Result<String, StoreError> {
    let mut roots = high_roots(record, lineage);
    roots.sort();
    roots.dedup();
    serde_json::to_string(&roots).map_err(|error| StoreError::InvalidData(error.to_string()))
}

impl Store {
    /// Atomically install one structural High bridge after validating all
    /// upstream roots and the three staged High CAS objects.  Exact replay
    /// returns `(record, true)`; a same-key mismatch fails before any row or
    /// reachability mutation.
    pub fn record_authoring_mesh_v2_high_bridge_with_replay(
        &self,
        commit: &AuthoringMeshV2HighBridgeCommit,
    ) -> Result<(AuthoringMeshV2HighBridgeStoreRecord, bool), StoreError> {
        validate_record(&commit.record)?;
        if commit.cas.bridge.sha256 != commit.record.bridge_object_sha256
            || commit.cas.high_result.sha256 != commit.record.high_result_object_sha256
            || commit.cas.high_readback.sha256 != commit.record.high_readback_object_sha256
        {
            return Err(contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_CAS_BINDING_MISMATCH",
                "staged High bridge CAS object hashes differ from the durable record",
            ));
        }

        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        ensure_table(&transaction)?;
        let bridge_bytes = validate_cas_metadata_and_bytes(
            &transaction,
            &self.cas,
            &commit.cas.bridge,
            &commit.record.bridge_object_sha256,
            AUTHORING_MESH_V2_HIGH_BRIDGE_OBJECT_KIND,
            AUTHORING_MESH_V2_HIGH_JSON_MIME,
            AUTHORING_MESH_V2_HIGH_MAX_BRIDGE_BYTES,
            false,
            "High bridge Main",
        )?;
        validate_main_payload(&bridge_bytes, &commit.record)?;
        let result_bytes = validate_cas_metadata_and_bytes(
            &transaction,
            &self.cas,
            &commit.cas.high_result,
            &commit.record.high_result_object_sha256,
            AUTHORING_MESH_V2_HIGH_RESULT_OBJECT_KIND,
            AUTHORING_MESH_V2_HIGH_JSON_MIME,
            AUTHORING_MESH_V2_HIGH_MAX_JSON_BYTES,
            false,
            "High result",
        )?;
        let result_value = validate_semantic_json(
            &result_bytes,
            &commit.record,
            &commit.record.high_result_sha256,
            &commit.record.high_result_schema_version,
            "High result",
        )?;
        let readback_bytes = validate_cas_metadata_and_bytes(
            &transaction,
            &self.cas,
            &commit.cas.high_readback,
            &commit.record.high_readback_object_sha256,
            AUTHORING_MESH_V2_HIGH_READBACK_OBJECT_KIND,
            AUTHORING_MESH_V2_HIGH_JSON_MIME,
            AUTHORING_MESH_V2_HIGH_MAX_JSON_BYTES,
            false,
            "High readback",
        )?;
        let readback_value = validate_semantic_json(
            &readback_bytes,
            &commit.record,
            &commit.record.high_readback_sha256,
            &commit.record.high_readback_schema_version,
            "High readback",
        )?;
        validate_high_readback_shape(&readback_value, &commit.record)?;
        validate_high_result_shape(&result_value, &readback_value, &commit.record)?;

        if let Some(existing) = read_record_in_transaction(
            &transaction,
            &commit.record.project_id,
            &commit.record.idempotency_key,
        )? {
            validate_record(&existing)?;
            if !same_record(&existing, &commit.record)
                || existing.bridge_object_sha256 != commit.record.bridge_object_sha256
                || existing.high_result_object_sha256 != commit.record.high_result_object_sha256
                || existing.high_readback_object_sha256 != commit.record.high_readback_object_sha256
            {
                return Err(contract(
                    "AUTHORING_MESH_V2_HIGH_BRIDGE_IDEMPOTENCY_CONFLICT",
                    "project and idempotency key are bound to different High bridge content",
                ));
            }
            let lineage =
                validate_source_binding_and_revision(&transaction, &self.cas, &existing, true)?;
            let materialized =
                validate_materialized_candidate_lineage(&transaction, &self.cas, &existing, true)?;
            let mut roots = lineage;
            roots.extend(materialized);
            roots.extend([
                existing.bridge_object_sha256.clone(),
                existing.high_result_object_sha256.clone(),
                existing.high_readback_object_sha256.clone(),
            ]);
            mark_reachable_in_transaction(&transaction, &roots)?;
            transaction.commit()?;
            return Ok((existing, true));
        }

        let project_exists: Option<String> = transaction
            .query_row(
                "SELECT project_id FROM projects WHERE project_id = ?1",
                params![commit.record.project_id],
                |row| row.get(0),
            )
            .optional()?;
        if project_exists.is_none() {
            return Err(contract(
                "PROJECT_SCOPE_DENIED",
                "High bridge project does not exist",
            ));
        }
        let bridge_conflict: Option<String> = transaction
            .query_row(
                "SELECT idempotency_key FROM authoring_mesh_v2_high_bridge_records WHERE project_id = ?1 AND bridge_id = ?2",
                params![commit.record.project_id, commit.record.bridge_id],
                |row| row.get(0),
            )
            .optional()?;
        if bridge_conflict.is_some() {
            return Err(contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_IDENTITY_CONFLICT",
                "bridge_id is already bound to another request",
            ));
        }

        // All lineage checks happen before INSERT.  Any late or tampered
        // upstream object therefore rolls back both rows and reachability.
        let lineage =
            validate_source_binding_and_revision(&transaction, &self.cas, &commit.record, false)?;
        let materialized = validate_materialized_candidate_lineage(
            &transaction,
            &self.cas,
            &commit.record,
            false,
        )?;
        let record_json = serde_json::to_string(&commit.record)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let mut all_lineage = lineage.clone();
        all_lineage.extend(materialized.iter().cloned());
        let object_hashes_json = stored_object_hashes(&commit.record, &all_lineage)?;
        transaction.execute(
            "INSERT INTO authoring_mesh_v2_high_bridge_records (schema_version, project_id, bridge_id, bridge_sha256, bridge_object_sha256, source_binding_id, source_binding_sha256, source_binding_object_sha256, mesh_id, lineage_id, revision_id, revision_index, revision_sha256, revision_object_sha256, materialized_candidate_id, materialized_candidate_state_sha256, materialized_program_sha256, materialized_program_object_sha256, materialized_artifact_id, materialized_artifact_sha256, materialized_artifact_object_sha256, materialized_artifact_readback_sha256, materialized_artifact_readback_object_sha256, representation_plan_sha256, source_node_id, part_id, material_zone_id, source_part_output_sha256, high_execution_request_sha256, high_evaluation_sha256, high_result_sha256, high_result_object_sha256, high_readback_sha256, high_readback_object_sha256, high_worker_algorithm_sha256, high_worker_build_cohort_sha256, request_input_sha256, idempotency_key, canonical_sha256, created_at, record_json, object_hashes_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37, ?38, ?39, ?40, ?41, ?42)",
            params![
                commit.record.schema_version,
                commit.record.project_id,
                commit.record.bridge_id,
                commit.record.bridge_sha256,
                commit.record.bridge_object_sha256,
                commit.record.source_binding_id,
                commit.record.source_binding_sha256,
                commit.record.source_binding_object_sha256,
                commit.record.mesh_id,
                commit.record.lineage_id,
                commit.record.revision_id,
                i64::try_from(commit.record.revision_index)
                    .map_err(|_| StoreError::InvalidData("revision index too large".to_owned()))?,
                commit.record.revision_sha256,
                commit.record.revision_object_sha256,
                commit.record.materialized_candidate_id,
                commit.record.materialized_candidate_state_sha256,
                commit.record.materialized_program_sha256,
                commit.record.materialized_program_object_sha256,
                commit.record.materialized_artifact_id,
                commit.record.materialized_artifact_sha256,
                commit.record.materialized_artifact_object_sha256,
                commit.record.materialized_artifact_readback_sha256,
                commit.record.materialized_artifact_readback_object_sha256,
                commit.record.representation_plan_sha256,
                commit.record.source_node_id,
                commit.record.part_id,
                commit.record.material_zone_id,
                commit.record.source_part_output_sha256,
                commit.record.high_execution_request_sha256,
                commit.record.high_evaluation_sha256,
                commit.record.high_result_sha256,
                commit.record.high_result_object_sha256,
                commit.record.high_readback_sha256,
                commit.record.high_readback_object_sha256,
                commit.record.high_worker_algorithm_sha256,
                commit.record.high_worker_build_cohort_sha256,
                commit.record.request_input_sha256,
                commit.record.idempotency_key,
                commit.record.canonical_sha256,
                commit.record.created_at,
                record_json,
                object_hashes_json,
            ],
        )?;
        let mut roots = lineage;
        roots.extend(materialized);
        roots.extend([
            commit.record.bridge_object_sha256.clone(),
            commit.record.high_result_object_sha256.clone(),
            commit.record.high_readback_object_sha256.clone(),
        ]);
        mark_reachable_in_transaction(&transaction, &roots)?;
        let stored = read_record_in_transaction(
            &transaction,
            &commit.record.project_id,
            &commit.record.idempotency_key,
        )?
        .ok_or_else(|| {
            contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_CORRUPT",
                "High bridge row disappeared before commit",
            )
        })?;
        validate_record(&stored)?;
        transaction.commit()?;
        Ok((stored, false))
    }

    pub fn get_authoring_mesh_v2_high_bridge(
        &self,
        project_id: &str,
        idempotency_key: &str,
    ) -> Result<Option<AuthoringMeshV2HighBridgeStoreRecord>, StoreError> {
        if !is_opaque_id(project_id) || !is_opaque_id(idempotency_key) {
            return Err(StoreError::InvalidData(
                "High bridge lookup identity is invalid".to_owned(),
            ));
        }
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        ensure_table(&transaction)?;
        let Some(record) = read_record_in_transaction(&transaction, project_id, idempotency_key)?
        else {
            transaction.commit()?;
            return Ok(None);
        };
        validate_record(&record)?;
        let lineage = validate_source_binding_and_revision(&transaction, &self.cas, &record, true)?;
        let materialized =
            validate_materialized_candidate_lineage(&transaction, &self.cas, &record, true)?;
        let bridge_object = read_object_record(&transaction, &record.bridge_object_sha256)?;
        let bridge_bytes = validate_cas_metadata_and_bytes(
            &transaction,
            &self.cas,
            &bridge_object,
            &record.bridge_object_sha256,
            AUTHORING_MESH_V2_HIGH_BRIDGE_OBJECT_KIND,
            AUTHORING_MESH_V2_HIGH_JSON_MIME,
            AUTHORING_MESH_V2_HIGH_MAX_BRIDGE_BYTES,
            true,
            "High bridge Main",
        )?;
        validate_main_payload(&bridge_bytes, &record)?;
        let result_object = read_object_record(&transaction, &record.high_result_object_sha256)?;
        let result_bytes = validate_cas_metadata_and_bytes(
            &transaction,
            &self.cas,
            &result_object,
            &record.high_result_object_sha256,
            AUTHORING_MESH_V2_HIGH_RESULT_OBJECT_KIND,
            AUTHORING_MESH_V2_HIGH_JSON_MIME,
            AUTHORING_MESH_V2_HIGH_MAX_JSON_BYTES,
            true,
            "High result",
        )?;
        let result_value = validate_semantic_json(
            &result_bytes,
            &record,
            &record.high_result_sha256,
            &record.high_result_schema_version,
            "High result",
        )?;
        let readback_object =
            read_object_record(&transaction, &record.high_readback_object_sha256)?;
        let readback_bytes = validate_cas_metadata_and_bytes(
            &transaction,
            &self.cas,
            &readback_object,
            &record.high_readback_object_sha256,
            AUTHORING_MESH_V2_HIGH_READBACK_OBJECT_KIND,
            AUTHORING_MESH_V2_HIGH_JSON_MIME,
            AUTHORING_MESH_V2_HIGH_MAX_JSON_BYTES,
            true,
            "High readback",
        )?;
        let readback_value = validate_semantic_json(
            &readback_bytes,
            &record,
            &record.high_readback_sha256,
            &record.high_readback_schema_version,
            "High readback",
        )?;
        validate_high_readback_shape(&readback_value, &record)?;
        validate_high_result_shape(&result_value, &readback_value, &record)?;
        let mut roots = lineage;
        roots.extend(materialized);
        roots.extend(high_roots(&record, &[]));
        mark_reachable_in_transaction(&transaction, &roots)?;
        transaction.commit()?;
        Ok(Some(record))
    }

    pub fn get_authoring_mesh_v2_high_bridge_by_id(
        &self,
        project_id: &str,
        bridge_id: &str,
    ) -> Result<Option<AuthoringMeshV2HighBridgeStoreRecord>, StoreError> {
        if !is_opaque_id(project_id) || !is_opaque_id(bridge_id) {
            return Err(StoreError::InvalidData(
                "High bridge lookup identity is invalid".to_owned(),
            ));
        }
        let idempotency_key = {
            let connection = self.lock_connection()?;
            connection
                .query_row(
                    "SELECT idempotency_key FROM authoring_mesh_v2_high_bridge_records WHERE project_id = ?1 AND bridge_id = ?2",
                    params![project_id, bridge_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
        };
        let Some(idempotency_key) = idempotency_key else {
            return Ok(None);
        };
        self.get_authoring_mesh_v2_high_bridge(project_id, &idempotency_key)
    }

    /// Strict all-field lookup used by Runtime restart/readback paths.  The
    /// function first resolves by the immutable request key and then compares
    /// every supplied semantic/object identity, so omitted or drifted fields
    /// cannot silently select a different upstream lineage.
    pub fn get_authoring_mesh_v2_high_bridge_exact(
        &self,
        project_id: &str,
        bridge_id: &str,
        bridge_sha256: &str,
        bridge_object_sha256: &str,
        source_binding_id: &str,
        source_binding_sha256: &str,
        source_binding_object_sha256: &str,
        mesh_id: &str,
        lineage_id: &str,
        revision_id: &str,
        revision_index: u64,
        revision_sha256: &str,
        revision_object_sha256: &str,
        materialized_candidate_id: &str,
        materialized_candidate_state_sha256: &str,
        materialized_program_sha256: &str,
        materialized_program_object_sha256: &str,
        materialized_artifact_id: &str,
        materialized_artifact_sha256: &str,
        materialized_artifact_object_sha256: &str,
        materialized_artifact_readback_sha256: &str,
        materialized_artifact_readback_object_sha256: &str,
        representation_plan_sha256: &str,
        source_node_id: &str,
        part_id: &str,
        material_zone_id: &str,
        source_part_output_sha256: &str,
        high_execution_request_sha256: &str,
        high_execution_operation: &str,
        high_operation: &str,
        high_result_sha256: &str,
        high_result_object_sha256: &str,
        high_readback_sha256: &str,
        high_readback_object_sha256: &str,
        high_worker_algorithm_sha256: &str,
        high_worker_build_cohort_sha256: &str,
    ) -> Result<Option<AuthoringMeshV2HighBridgeStoreRecord>, StoreError> {
        if !is_opaque_id(project_id)
            || !is_opaque_id(bridge_id)
            || !is_opaque_id(source_binding_id)
            || !is_opaque_id(mesh_id)
            || !is_opaque_id(lineage_id)
            || !is_opaque_id(revision_id)
            || !is_opaque_id(materialized_candidate_id)
            || !is_opaque_id(materialized_artifact_id)
            || !is_opaque_id(source_node_id)
            || !is_opaque_id(part_id)
            || !is_opaque_id(material_zone_id)
        {
            return Err(StoreError::InvalidData(
                "High bridge exact lookup identity is invalid".to_owned(),
            ));
        }
        let Some(record) = self.get_authoring_mesh_v2_high_bridge_by_id(project_id, bridge_id)?
        else {
            return Ok(None);
        };
        let equal = record.project_id == project_id
            && record.bridge_id == bridge_id
            && record.bridge_sha256 == bridge_sha256
            && record.bridge_object_sha256 == bridge_object_sha256
            && record.source_binding_id == source_binding_id
            && record.source_binding_sha256 == source_binding_sha256
            && record.source_binding_object_sha256 == source_binding_object_sha256
            && record.mesh_id == mesh_id
            && record.lineage_id == lineage_id
            && record.revision_id == revision_id
            && record.revision_index == revision_index
            && record.revision_sha256 == revision_sha256
            && record.revision_object_sha256 == revision_object_sha256
            && record.materialized_candidate_id == materialized_candidate_id
            && record.materialized_candidate_state_sha256 == materialized_candidate_state_sha256
            && record.materialized_program_sha256 == materialized_program_sha256
            && record.materialized_program_object_sha256 == materialized_program_object_sha256
            && record.materialized_artifact_id == materialized_artifact_id
            && record.materialized_artifact_sha256 == materialized_artifact_sha256
            && record.materialized_artifact_object_sha256 == materialized_artifact_object_sha256
            && record.materialized_artifact_readback_sha256
                == materialized_artifact_readback_sha256
            && record.materialized_artifact_readback_object_sha256
                == materialized_artifact_readback_object_sha256
            && record.representation_plan_sha256 == representation_plan_sha256
            && record.source_node_id == source_node_id
            && record.part_id == part_id
            && record.material_zone_id == material_zone_id
            && record.source_part_output_sha256 == source_part_output_sha256
            && record.high_execution_request_sha256 == high_execution_request_sha256
            && record.high_execution_operation == high_execution_operation
            && record.high_operation == high_operation
            && record.high_result_sha256 == high_result_sha256
            && record.high_result_object_sha256 == high_result_object_sha256
            && record.high_readback_sha256 == high_readback_sha256
            && record.high_readback_object_sha256 == high_readback_object_sha256
            && record.high_worker_algorithm_sha256 == high_worker_algorithm_sha256
            && record.high_worker_build_cohort_sha256 == high_worker_build_cohort_sha256;
        if !equal {
            return Err(contract(
                "AUTHORING_MESH_V2_HIGH_BRIDGE_EXACT_LOOKUP_MISMATCH",
                "High bridge exact lookup fields differ from the durable row",
            ));
        }
        Ok(Some(record))
    }

    pub fn read_authoring_mesh_v2_high_bridge_json(
        &self,
        project_id: &str,
        idempotency_key: &str,
    ) -> Result<Option<Value>, StoreError> {
        let Some(record) = self.get_authoring_mesh_v2_high_bridge(project_id, idempotency_key)?
        else {
            return Ok(None);
        };
        Ok(Some(main_value(&record)?))
    }

    // Short aliases for the Runtime domain service.
    pub fn record_authoring_mesh_v2_high_with_replay(
        &self,
        commit: &AuthoringMeshV2HighCommit,
    ) -> Result<(AuthoringMeshV2HighDurableRecord, bool), StoreError> {
        self.record_authoring_mesh_v2_high_bridge_with_replay(commit)
    }

    pub fn get_authoring_mesh_v2_high(
        &self,
        project_id: &str,
        idempotency_key: &str,
    ) -> Result<Option<AuthoringMeshV2HighDurableRecord>, StoreError> {
        self.get_authoring_mesh_v2_high_bridge(project_id, idempotency_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgecad_contracts::{
        AuthoringMeshRevision, AuthoringMeshV2SourceBinding, CandidateRecord, ProjectRecord,
        ReferenceAuthorization, ReferenceEvidenceRecord,
    };
    use serde_json::json;

    const NOW: &str = "2026-08-31T00:00:00Z";
    const PROJECT: &str = "high-bridge-test-project";
    const SOURCE_CANDIDATE: &str = "high-bridge-source-candidate";
    const MATERIALIZED_CANDIDATE: &str = "high-bridge-materialized-candidate";
    const REFERENCE: &str = "high-bridge-reference";
    const BRIEF: &str = "high-bridge-brief";
    const INTENT: &str = "high-bridge-intent";
    const MESH: &str = "high-bridge-mesh";
    const LINEAGE: &str = "high-bridge-lineage";
    const REVISION: &str = "high-bridge-revision";
    const SOURCE_BINDING: &str = "high-bridge-source-binding";
    const SOURCE_NODE: &str = "high-bridge-source-node";
    const PRESERVED_NODE: &str = "high-bridge-preserved-node";
    const TARGET_PART: &str = "high-bridge-blade";
    const PRESERVED_PART: &str = "high-bridge-grip";
    const TARGET_ZONE: &str = "high-bridge-blade-zone";
    const PRESERVED_ZONE: &str = "high-bridge-grip-zone";

    fn h(seed: char) -> String {
        sha256_hex(seed.to_string().as_bytes())
    }

    fn canonical_object(
        store: &Store,
        mut value: Value,
        kind: &str,
        mime: &str,
    ) -> (CasObjectRecord, String) {
        value["canonical_sha256"] = Value::String(String::new());
        let semantic = canonical_json_hash(&value);
        value["canonical_sha256"] = Value::String(semantic.clone());
        let bytes = canonical_json_bytes(&value).expect("canonical test object");
        let object = store
            .put_object(&bytes, None, mime, kind, NOW)
            .expect("test CAS object");
        (object.record, semantic)
    }

    fn project(store: &Store) {
        store
            .insert_project(&ProjectRecord {
                schema_version: "Project@1".to_owned(),
                project_id: PROJECT.to_owned(),
                name: "High bridge test project".to_owned(),
                policy: json!({"scope":"test"}),
                created_at: NOW.to_owned(),
                updated_at: NOW.to_owned(),
                active_snapshot_revision: 0,
                head_snapshot_id: None,
                canonical_sha256: h('p'),
            })
            .expect("test project");
    }

    fn source_program(source_part_output_sha256: &mut String) -> Value {
        let source_parameters = json!({
            "shape": "dragonfang-source",
            "scale": [1.0, 1.0, 1.0]
        });
        *source_part_output_sha256 = canonical_json_hash(&json!({
            "part_id": TARGET_PART,
            "input_node_ids": [SOURCE_NODE],
            "material_zone_id": TARGET_ZONE,
            "solid": true
        }));
        json!({
            "schema_version": "GeometryProgram@2",
            "project_id": PROJECT,
            "operator_catalog_sha256": h('o'),
            "nodes": [
                {
                    "node_id": SOURCE_NODE,
                    "operator_id": "forgecad.geometry.source@1",
                    "inputs": [],
                    "parameters": source_parameters
                },
                {
                    "node_id": PRESERVED_NODE,
                    "operator_id": "forgecad.geometry.source@1",
                    "inputs": [],
                    "parameters": {"shape":"grip"}
                }
            ],
            "part_outputs": [
                {
                    "part_id": TARGET_PART,
                    "input_node_ids": [SOURCE_NODE],
                    "material_zone_id": TARGET_ZONE,
                    "solid": true
                },
                {
                    "part_id": PRESERVED_PART,
                    "input_node_ids": [PRESERVED_NODE],
                    "material_zone_id": PRESERVED_ZONE,
                    "solid": true
                }
            ]
        })
    }

    fn artifact_readback(
        artifact_id: &str,
        candidate_id: &str,
        artifact_object_sha256: &str,
        program_sha256: &str,
        target_node_id: &str,
        part_bindings: Value,
    ) -> Value {
        json!({
            "schema_version": "ArtifactReadback@2",
            "artifact_id": artifact_id,
            "candidate_id": candidate_id,
            "object_sha256": artifact_object_sha256,
            "mime": "model/gltf-binary",
            "size_bytes": 16,
            "program_sha256": program_sha256,
            "operator_catalog_sha256": h('o'),
            "readback_config_sha256": h('c'),
            "triangle_count": 2,
            "part_ids": [TARGET_PART, PRESERVED_PART],
            "source_node_ids": [target_node_id, PRESERVED_NODE],
            "material_zone_ids": [TARGET_ZONE, PRESERVED_ZONE],
            "part_bindings": part_bindings,
            "validator_status": "passed",
            "hard_gate_passed": true,
            "integrity": {
                "glb_parse_status": "passed",
                "invalid_index_count": 0,
                "non_finite_count": 0,
                "degenerate_triangle_count": 0,
                "boundary_edge_count": 0,
                "non_manifold_edge_count": 0,
                "winding_error_count": 0,
                "uv_non_finite_count": 0,
                "zero_area_uv_triangle_count": 0,
                "tangent_non_finite_count": 0,
                "tangent_orthogonality_error_count": 0,
                "tangent_handedness_error_count": 0,
                "metadata_mismatch_count": 0,
                "external_uri_count": 0,
                "part_coverage": 1.0,
                "source_coverage": 1.0,
                "material_zone_coverage": 1.0
            },
            "canonical_sha256": ""
        })
    }

    fn part_bindings(source_node_id: &str) -> Value {
        json!([
            {
                "part_id": TARGET_PART,
                "source_node_id": source_node_id,
                "material_zone_id": TARGET_ZONE,
                "solid": true,
                "triangle_count": 1
            },
            {
                "part_id": PRESERVED_PART,
                "source_node_id": PRESERVED_NODE,
                "material_zone_id": PRESERVED_ZONE,
                "solid": true,
                "triangle_count": 1
            }
        ])
    }

    fn revision_value(binding: AuthoringMeshV2SourceBinding) -> AuthoringMeshRevision {
        let mut value = json!({
            "schema_version": "AuthoringMeshRevision@2",
            "mesh_id": MESH,
            "lineage_id": LINEAGE,
            "revision_id": REVISION,
            "parent_revision_ids": [],
            "revision_index": 0,
            "operation": null,
            "original": {
                "namespace": "original",
                "lineage_id": LINEAGE,
                "vertices": [
                    {"vertex_id":"v0", "position_m":[0.0,0.0,0.0]},
                    {"vertex_id":"v1", "position_m":[1.0,0.0,0.0]},
                    {"vertex_id":"v2", "position_m":[0.0,1.0,0.0]}
                ],
                "edges": [
                    {"edge_id":"e0", "vertex_ids":["v0","v1"], "half_edge_ids":["he0"], "boundary":true},
                    {"edge_id":"e1", "vertex_ids":["v1","v2"], "half_edge_ids":["he1"], "boundary":true},
                    {"edge_id":"e2", "vertex_ids":["v2","v0"], "half_edge_ids":["he2"], "boundary":true}
                ],
                "half_edges": [
                    {"half_edge_id":"he0", "origin_vertex_id":"v0", "edge_id":"e0", "face_id":"f0", "corner_id":"c0", "next_id":"he1", "prev_id":"he2", "twin_id":null, "boundary":true},
                    {"half_edge_id":"he1", "origin_vertex_id":"v1", "edge_id":"e1", "face_id":"f0", "corner_id":"c1", "next_id":"he2", "prev_id":"he0", "twin_id":null, "boundary":true},
                    {"half_edge_id":"he2", "origin_vertex_id":"v2", "edge_id":"e2", "face_id":"f0", "corner_id":"c2", "next_id":"he0", "prev_id":"he1", "twin_id":null, "boundary":true}
                ],
                "corners": [
                    {"corner_id":"c0", "half_edge_id":"he0", "vertex_id":"v0", "face_id":"f0", "ordinal":0, "uv0":null, "normal":null, "tangent":null, "seam":false},
                    {"corner_id":"c1", "half_edge_id":"he1", "vertex_id":"v1", "face_id":"f0", "ordinal":1, "uv0":null, "normal":null, "tangent":null, "seam":false},
                    {"corner_id":"c2", "half_edge_id":"he2", "vertex_id":"v2", "face_id":"f0", "ordinal":2, "uv0":null, "normal":null, "tangent":null, "seam":false}
                ],
                "faces": [
                    {"face_id":"f0", "half_edge_ids":["he0","he1","he2"], "loop_id":"l0", "boundary":true}
                ],
                "loops": [
                    {"loop_id":"l0", "face_id":"f0", "half_edge_ids":["he0","he1","he2"], "boundary":true}
                ],
                "rings": [],
                "tombstones": [],
                "canonical_sha256": ""
            },
            "evaluated": null,
            "source_binding": binding,
            "id_policy": "runtime-derived-lineage-operation-parent-stable-no-reuse@2",
            "canonical_sha256": ""
        });
        let original_hash = canonical_json_hash(&value["original"]);
        value["original"]["canonical_sha256"] = Value::String(original_hash);
        let revision_hash = canonical_json_hash(&value);
        value["canonical_sha256"] = Value::String(revision_hash);
        serde_json::from_value(value).expect("typed AuthoringMesh revision")
    }

    fn durable_revision_record(
        revision: &AuthoringMeshRevision,
        revision_object_sha256: &str,
    ) -> AuthoringMeshV2DurableRecord {
        let mut record = AuthoringMeshV2DurableRecord {
            schema_version: "AuthoringMeshV2DurableRecord@1".to_owned(),
            project_id: PROJECT.to_owned(),
            mesh_id: MESH.to_owned(),
            lineage_id: LINEAGE.to_owned(),
            revision_id: REVISION.to_owned(),
            parent_revision_ids: Vec::new(),
            revision_index: 0,
            revision_object_sha256: revision_object_sha256.to_owned(),
            revision_sha256: revision.canonical_sha256.clone(),
            operation_id: None,
            operation_kind: None,
            operation_lineage_sha256: None,
            request_input_sha256: h('q'),
            idempotency_key: "high-bridge-revision-key".to_owned(),
            materialization_status: "runtime-owned-store-authoring-mesh-v2-durable-record@1"
                .to_owned(),
            canonical_sha256: h('r'),
            created_at: NOW.to_owned(),
        };
        record.canonical_sha256 = authoring_mesh_v2_durable_record_canonical_sha256(&record)
            .expect("durable revision record hash");
        record
    }

    pub(crate) struct Fixture {
        pub(crate) store: Store,
        pub(crate) commit: AuthoringMeshV2HighBridgeCommit,
    }

    fn insert_intent_row(
        store: &Store,
        intent_sha256: &str,
        intent_object_sha256: &str,
        brief_sha256: &str,
        brief_object_sha256: &str,
        reference_object_sha256: &str,
        reference_evidence_sha256: &str,
        quality_sha256: &str,
        quality_object_sha256: &str,
    ) {
        let connection = store.connection.lock().expect("test connection");
        connection
            .execute(
                "INSERT INTO knife_reference_intent_bundle_records (schema_version, intent_bundle_id, project_id, brief_id, brief_sha256, brief_object_sha256, reference_id, reference_object_sha256, reference_evidence_sha256, intake_manifest_sha256, intake_manifest_object_sha256, detail_inventory_sha256, detail_inventory_object_sha256, quality_contract_sha256, quality_contract_object_sha256, intent_bundle_sha256, intent_bundle_object_sha256, idempotency_key, created_at, record_json) VALUES ('KnifeReferenceIntentBundleStoreRecord@1', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, 'high-intent-key', ?17, ?18)",
                params![
                    INTENT,
                    PROJECT,
                    BRIEF,
                    brief_sha256,
                    brief_object_sha256,
                    REFERENCE,
                    reference_object_sha256,
                    reference_evidence_sha256,
                    h('i'),
                    brief_object_sha256,
                    h('j'),
                    brief_object_sha256,
                    quality_sha256,
                    quality_object_sha256,
                    intent_sha256,
                    intent_object_sha256,
                    NOW,
                    "{}",
                ],
            )
            .expect("test intent row");
    }

    fn insert_source_binding_row(store: &Store, record: &KnifeSourceBindingStoreRecord) {
        let record_json = serde_json::to_string(record).expect("SourceBinding row JSON");
        let requirements = serde_json::to_string(&record.downstream_binding_requirements)
            .expect("SourceBinding requirements");
        let connection = store.connection.lock().expect("test connection");
        connection
            .execute(
                "INSERT INTO knife_source_binding_records (schema_version, source_binding_id, project_id, binding_status, authoring_eligibility, intent_bundle_id, intent_bundle_sha256, intent_bundle_object_sha256, brief_id, brief_sha256, brief_object_sha256, reference_id, reference_object_sha256, reference_evidence_sha256, quality_contract_id, quality_contract_sha256, quality_contract_object_sha256, source_candidate_id, source_candidate_state_sha256, authoring_mesh_id, authoring_mesh_lineage_id, authoring_mesh_revision_id, authoring_mesh_revision_index, authoring_mesh_revision_sha256, authoring_mesh_revision_object_sha256, authoring_mesh_identity_sha256, downstream_binding_requirements_json, high_mesh_created, high_stage_unlocked, production_stage_advanced, candidate_confirmed, version_created, export_performed, quality_status, visual_status, human_status, engine_status, binding_policy, canonicalization_policy, source_binding_sha256, source_binding_object_sha256, idempotency_key, created_at, record_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37, ?38, ?39, ?40, ?41, ?42, ?43, ?44)",
                params![
                    record.schema_version,
                    record.source_binding_id,
                    record.project_id,
                    record.binding_status,
                    record.authoring_eligibility,
                    record.intent_bundle_id,
                    record.intent_bundle_sha256,
                    record.intent_bundle_object_sha256,
                    record.brief_id,
                    record.brief_sha256,
                    record.brief_object_sha256,
                    record.reference_id,
                    record.reference_object_sha256,
                    record.reference_evidence_sha256,
                    record.quality_contract_id,
                    record.quality_contract_sha256,
                    record.quality_contract_object_sha256,
                    record.source_candidate_id,
                    record.source_candidate_state_sha256,
                    record.authoring_mesh_id,
                    record.authoring_mesh_lineage_id,
                    record.authoring_mesh_revision_id,
                    i64::try_from(record.authoring_mesh_revision_index).expect("revision index"),
                    record.authoring_mesh_revision_sha256,
                    record.authoring_mesh_revision_object_sha256,
                    record.authoring_mesh_identity_sha256,
                    requirements,
                    record.high_mesh_created,
                    record.high_stage_unlocked,
                    record.production_stage_advanced,
                    record.candidate_confirmed,
                    record.version_created,
                    record.export_performed,
                    record.quality_status,
                    record.visual_status,
                    record.human_status,
                    record.engine_status,
                    record.binding_policy,
                    record.canonicalization_policy,
                    record.source_binding_sha256,
                    record.source_binding_object_sha256,
                    record.idempotency_key,
                    record.created_at,
                    record_json,
                ],
            )
            .expect("test SourceBinding row");
    }

    fn insert_geometry_evidence_row(
        store: &Store,
        candidate_id: &str,
        reference_object_sha256: &str,
        program_sha256: &str,
        program_object_sha256: &str,
        artifact_object_sha256: &str,
        readback_object_sha256: &str,
        quality_object_sha256: &str,
    ) {
        let connection = store.connection.lock().expect("test connection");
        connection
            .execute(
                "INSERT INTO geometry_candidate_evidence (candidate_id, project_id, reference_id, reference_sha256, geometry_program_sha256, geometry_program_object_sha256, operator_catalog_sha256, readback_config_sha256, artifact_object_sha256, artifact_readback_object_sha256, quality_report_object_sha256, quality_report_id, canonical_sha256, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![
                    candidate_id,
                    PROJECT,
                    REFERENCE,
                    reference_object_sha256,
                    program_sha256,
                    program_object_sha256,
                    h('o'),
                    h('c'),
                    artifact_object_sha256,
                    readback_object_sha256,
                    quality_object_sha256,
                    "high-quality-report",
                    h('g'),
                    NOW,
                ],
            )
            .expect("test GeometryCandidateEvidence row");
    }

    fn insert_candidate(
        store: &Store,
        candidate_id: &str,
        state_sha256: &str,
        artifact_id: &str,
        artifact_object_sha256: &str,
        state: &str,
    ) {
        store
            .insert_candidate(&CandidateRecord {
                schema_version: "Candidate@1".to_owned(),
                candidate_id: candidate_id.to_owned(),
                project_id: PROJECT.to_owned(),
                base_version_id: None,
                source_version_id: None,
                prepared_object_id: Some(artifact_id.to_owned()),
                prepared_object_sha256: Some(artifact_object_sha256.to_owned()),
                state: state.to_owned(),
                request_sha256: h('q'),
                manifest_hash: None,
                quality_report_id: Some("high-quality-report".to_owned()),
                quality_hard_gate_passed: true,
                canonical_sha256: state_sha256.to_owned(),
                error_code: None,
                created_at: NOW.to_owned(),
                updated_at: NOW.to_owned(),
            })
            .expect("test Candidate row");
    }

    pub(crate) fn setup_fixture() -> Fixture {
        let store = Store::memory().expect("memory Store");
        project(&store);

        let reference = store
            .put_object(
                b"high-bridge-reference",
                None,
                "image/png",
                "reference-image",
                NOW,
            )
            .expect("reference CAS");
        store
            .insert_reference_evidence(&ReferenceEvidenceRecord {
                schema_version: "ReferenceEvidence@1".to_owned(),
                reference_id: REFERENCE.to_owned(),
                project_id: PROJECT.to_owned(),
                object_sha256: reference.record.sha256.clone(),
                mime: "image/png".to_owned(),
                size_bytes: reference.record.size_bytes,
                width: 1,
                height: 1,
                frame_count: 1,
                import_mode: "inline_content".to_owned(),
                authorization: ReferenceAuthorization {
                    user_authorized: true,
                    declaration: "authorized High bridge test source".to_owned(),
                },
                derived_object_sha256: None,
                canonical_sha256: h('e'),
                created_at: NOW.to_owned(),
            })
            .expect("ReferenceEvidence row");

        let (brief_object, brief_sha256) = canonical_object(
            &store,
            json!({
                "schema_version": "WeaponryKnifeProductionBrief@1",
                "brief_id": BRIEF,
                "project_id": PROJECT
            }),
            "weaponry-knife-production-brief",
            "application/json",
        );
        let (quality_object, quality_sha256) = canonical_object(
            &store,
            json!({
                "schema_version": "KnifeQualityContract@1",
                "contract_id": "high-bridge-quality"
            }),
            "knife-quality-contract",
            "application/json",
        );
        let (intent_object, intent_sha256) = canonical_object(
            &store,
            json!({
                "schema_version": "KnifeReferenceIntentBundle@1",
                "project_id": PROJECT
            }),
            "knife-reference-intent-bundle",
            "application/json",
        );
        insert_intent_row(
            &store,
            &intent_sha256,
            &intent_object.sha256,
            &brief_sha256,
            &brief_object.sha256,
            &reference.record.sha256,
            &h('e'),
            &quality_sha256,
            &quality_object.sha256,
        );
        // The High lineage validator only needs the immutable Brief CAS root;
        // this row makes the upstream source contract explicit without
        // coupling the test to the Brief repository implementation.
        {
            let connection = store.connection.lock().expect("test connection");
            connection
                .execute(
                    "INSERT INTO weaponry_knife_production_brief_records (schema_version, project_id, brief_id, brief_object_sha256, brief_canonical_sha256, reference_id, reference_object_sha256, reference_evidence_sha256, parent_brief_id, parent_brief_sha256, freeze_policy, source_reference_hashes_json, status, conflict_freeze_state, idempotency_key, created_at, record_json) VALUES ('WeaponryKnifeProductionBriefStoreRecord@1', ?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, NULL, 'initial-intake-no-parent@1', ?8, 'eligible', 'resolved', 'high-brief-key', ?9, ?10)",
                    params![
                        PROJECT,
                        BRIEF,
                        brief_object.sha256,
                        brief_sha256,
                        REFERENCE,
                        reference.record.sha256,
                        h('e'),
                        serde_json::to_string(&vec![reference.record.sha256.clone()]).unwrap(),
                        NOW,
                        "{}",
                    ],
                )
                .expect("Brief row");
        }

        let source_artifact = store
            .put_object(&[7u8; 16], None, "model/gltf-binary", "geometry-glb", NOW)
            .expect("source artifact");
        let mut source_part_output_sha256 = String::new();
        let source_program = source_program(&mut source_part_output_sha256);
        let source_program_bytes = canonical_json_bytes(&source_program).unwrap();
        let source_program_sha256 = canonical_json_hash(&source_program);
        assert_eq!(sha256_hex(&source_program_bytes), source_program_sha256);
        let source_program_object = store
            .put_object(
                &source_program_bytes,
                Some(&source_program_sha256),
                "application/json",
                "geometry-program-v2",
                NOW,
            )
            .expect("source program");
        let mut source_readback = artifact_readback(
            "high-bridge-source-artifact",
            SOURCE_CANDIDATE,
            &source_artifact.record.sha256,
            &source_program_sha256,
            SOURCE_NODE,
            part_bindings(SOURCE_NODE),
        );
        let source_readback_object_and_sha = canonical_object(
            &store,
            source_readback.clone(),
            "geometry-artifact-readback-v2",
            "application/json",
        );
        source_readback = serde_json::from_slice(
            &store
                .cas()
                .read_verified_bounded(&source_readback_object_and_sha.0.sha256, 8 * 1024 * 1024)
                .unwrap(),
        )
        .unwrap();
        let source_readback_sha256 = source_readback["canonical_sha256"]
            .as_str()
            .unwrap()
            .to_owned();
        let quality_report = store
            .put_object(
                b"high-bridge-quality-report",
                None,
                "application/json",
                "quality-report",
                NOW,
            )
            .expect("source quality report");
        let source_state_sha256 = h('s');
        insert_candidate(
            &store,
            SOURCE_CANDIDATE,
            &source_state_sha256,
            "high-bridge-source-artifact",
            &source_artifact.record.sha256,
            "prepared",
        );
        insert_geometry_evidence_row(
            &store,
            SOURCE_CANDIDATE,
            &reference.record.sha256,
            &source_program_sha256,
            &source_program_object.record.sha256,
            &source_artifact.record.sha256,
            &source_readback_object_and_sha.0.sha256,
            &quality_report.record.sha256,
        );

        let source_parameters_sha256 =
            canonical_json_hash(&source_program["nodes"][0]["parameters"]);
        let mut embedded_value = json!({
            "schema_version": "AuthoringMeshV2SourceBinding@1",
            "project_id": PROJECT,
            "candidate_id": SOURCE_CANDIDATE,
            "candidate_state_sha256": source_state_sha256,
            "artifact_id": "high-bridge-source-artifact",
            "artifact_sha256": source_artifact.record.sha256,
            "artifact_readback_sha256": source_readback_sha256,
            "geometry_program_sha256": source_program_sha256,
            "source_node_id": SOURCE_NODE,
            "part_id": TARGET_PART,
            "material_zone_id": TARGET_ZONE,
            "solid": true,
            "source_operator_id": "forgecad.geometry.source@1",
            "source_parameters_sha256": source_parameters_sha256,
            "part_output_sha256": source_part_output_sha256,
            "position_m": [0.0, 0.0, 0.0],
            "rotation_rad": [0.0, 0.0, 0.0],
            "canonical_sha256": ""
        });
        let embedded_sha256 = canonical_json_hash(&embedded_value);
        embedded_value["canonical_sha256"] = Value::String(embedded_sha256);
        let embedded: AuthoringMeshV2SourceBinding =
            serde_json::from_value(embedded_value).expect("embedded source binding");
        let revision = revision_value(embedded.clone());
        let revision_value = serde_json::to_value(&revision).unwrap();
        let revision_bytes = canonical_json_bytes(&revision_value).unwrap();
        let revision_object = store
            .put_object(
                &revision_bytes,
                None,
                "application/json",
                AUTHORING_MESH_V2_REVISION_OBJECT_KIND,
                NOW,
            )
            .expect("V2 revision object");
        let revision_record = durable_revision_record(&revision, &revision_object.record.sha256);
        store
            .record_authoring_mesh_v2_revision_with_replay(
                &revision_record,
                &revision,
                &revision_object.record,
            )
            .expect("V2 revision durable row");

        let identity_sha256 = canonical_json_hash(&json!({
            "schema_version": "AuthoringMeshSourceIdentity@1",
            "mesh_id": MESH,
            "lineage_id": LINEAGE,
            "revision_id": REVISION,
            "revision_index": 0,
            "revision_sha256": revision.canonical_sha256,
        }));
        let mut source_binding = KnifeSourceBindingStoreRecord {
            schema_version: KNIFE_SOURCE_BINDING_RECORD_SCHEMA_VERSION.to_owned(),
            source_binding_id: SOURCE_BINDING.to_owned(),
            project_id: PROJECT.to_owned(),
            binding_status: KNIFE_SOURCE_BINDING_BINDING_STATUS.to_owned(),
            authoring_eligibility: KNIFE_SOURCE_BINDING_AUTHORING_ELIGIBILITY.to_owned(),
            intent_bundle_id: INTENT.to_owned(),
            intent_bundle_sha256: intent_sha256,
            intent_bundle_object_sha256: intent_object.sha256,
            brief_id: BRIEF.to_owned(),
            brief_sha256,
            brief_object_sha256: brief_object.sha256,
            reference_id: REFERENCE.to_owned(),
            reference_object_sha256: reference.record.sha256.clone(),
            reference_evidence_sha256: h('e'),
            quality_contract_id: "high-bridge-quality".to_owned(),
            quality_contract_sha256: quality_sha256,
            quality_contract_object_sha256: quality_object.sha256,
            source_candidate_id: SOURCE_CANDIDATE.to_owned(),
            source_candidate_state_sha256: source_state_sha256.clone(),
            authoring_mesh_id: MESH.to_owned(),
            authoring_mesh_lineage_id: LINEAGE.to_owned(),
            authoring_mesh_revision_id: REVISION.to_owned(),
            authoring_mesh_revision_index: 0,
            authoring_mesh_revision_sha256: revision.canonical_sha256.clone(),
            authoring_mesh_revision_object_sha256: revision_object.record.sha256.clone(),
            authoring_mesh_identity_sha256: identity_sha256,
            downstream_binding_requirements: KnifeSourceBindingDownstreamBindingRequirements {
                curve_modifier_graph: KNIFE_SOURCE_BINDING_DOWNSTREAM_POLICY.to_owned(),
                curve_evaluated_mesh: KNIFE_SOURCE_BINDING_DOWNSTREAM_POLICY.to_owned(),
                high: KNIFE_SOURCE_BINDING_DOWNSTREAM_POLICY.to_owned(),
                render: KNIFE_SOURCE_BINDING_DOWNSTREAM_POLICY.to_owned(),
            },
            high_mesh_created: false,
            high_stage_unlocked: false,
            production_stage_advanced: false,
            candidate_confirmed: false,
            version_created: false,
            export_performed: false,
            quality_status: "source_binding_only".to_owned(),
            visual_status: "NOT_RUN".to_owned(),
            human_status: "NOT_RUN".to_owned(),
            engine_status: "NOT_RUN".to_owned(),
            binding_policy: KNIFE_SOURCE_BINDING_POLICY.to_owned(),
            canonicalization_policy: KNIFE_SOURCE_BINDING_CANONICALIZATION_POLICY.to_owned(),
            source_binding_sha256: h('b'),
            source_binding_object_sha256: h('c'),
            idempotency_key: "high-source-binding-key".to_owned(),
            created_at: NOW.to_owned(),
        };
        let mut source_binding_preimage = source_binding_payload_value(&source_binding).unwrap();
        source_binding_preimage["canonical_sha256"] = Value::String(String::new());
        source_binding.source_binding_sha256 = canonical_json_hash(&source_binding_preimage);
        let source_binding_payload = source_binding_payload_value(&source_binding).unwrap();
        let source_binding_bytes = canonical_json_bytes(&source_binding_payload).unwrap();
        let source_binding_object = store
            .put_object(
                &source_binding_bytes,
                None,
                KNIFE_SOURCE_BINDING_JSON_MIME,
                KNIFE_SOURCE_BINDING_OBJECT_KIND,
                NOW,
            )
            .expect("SourceBinding object");
        source_binding.source_binding_object_sha256 = source_binding_object.record.sha256.clone();
        insert_source_binding_row(&store, &source_binding);

        let mut record = AuthoringMeshV2HighBridgeStoreRecord {
            schema_version: AUTHORING_MESH_V2_HIGH_BRIDGE_SCHEMA_VERSION.to_owned(),
            bridge_id: "high-bridge-id".to_owned(),
            project_id: PROJECT.to_owned(),
            source_scope: AUTHORING_MESH_V2_HIGH_SOURCE_SCOPE.to_owned(),
            source_revision_schema_version: AUTHORING_MESH_V2_HIGH_REVISION_SCHEMA_VERSION
                .to_owned(),
            mesh_id: MESH.to_owned(),
            lineage_id: LINEAGE.to_owned(),
            revision_id: REVISION.to_owned(),
            revision_index: 0,
            revision_sha256: revision.canonical_sha256.clone(),
            revision_object_sha256: revision_object.record.sha256.clone(),
            source_binding_id: SOURCE_BINDING.to_owned(),
            source_binding_sha256: source_binding.source_binding_sha256.clone(),
            source_binding_object_sha256: source_binding.source_binding_object_sha256.clone(),
            materialized_candidate_id: MATERIALIZED_CANDIDATE.to_owned(),
            materialized_candidate_state_sha256: h('m'),
            materialized_program_sha256: h('n'),
            materialized_program_object_sha256: h('o'),
            materialized_artifact_id: "high-bridge-materialized-artifact".to_owned(),
            materialized_artifact_sha256: h('p'),
            materialized_artifact_object_sha256: h('p'),
            materialized_artifact_readback_sha256: h('q'),
            materialized_artifact_readback_object_sha256: h('r'),
            representation_plan_sha256: h('s'),
            source_node_id: SOURCE_NODE.to_owned(),
            part_id: TARGET_PART.to_owned(),
            material_zone_id: TARGET_ZONE.to_owned(),
            solid: true,
            source_part_output_sha256,
            preserved_part_ids: vec![PRESERVED_PART.to_owned()],
            materialized_artifact_hash_policy: AUTHORING_MESH_V2_HIGH_ARTIFACT_HASH_POLICY
                .to_owned(),
            high_execution_request_schema_version:
                AUTHORING_MESH_V2_HIGH_EXECUTION_REQUEST_SCHEMA_VERSION.to_owned(),
            high_execution_operation: AUTHORING_MESH_V2_HIGH_EXECUTION_OPERATION.to_owned(),
            high_operation: AUTHORING_MESH_V2_HIGH_OPERATION.to_owned(),
            high_result_schema_version: AUTHORING_MESH_V2_HIGH_RESULT_SCHEMA_VERSION.to_owned(),
            high_readback_schema_version: AUTHORING_MESH_V2_HIGH_READBACK_SCHEMA_VERSION.to_owned(),
            high_evaluator_contract: AUTHORING_MESH_V2_HIGH_EVALUATOR_CONTRACT.to_owned(),
            high_subdivision_backend: AUTHORING_MESH_V2_HIGH_SUBDIVISION_BACKEND.to_owned(),
            high_subdivision_levels: 1,
            high_max_triangles_per_face: 32,
            high_max_output_vertices: 32_768,
            high_max_output_triangles: 600_000,
            high_execution_request_sha256: h('t'),
            high_evaluation_sha256: h('u'),
            high_result_sha256: h('v'),
            high_result_object_sha256: h('w'),
            high_readback_sha256: h('x'),
            high_readback_object_sha256: h('y'),
            high_worker_algorithm_sha256: h('z'),
            high_worker_build_cohort_sha256: h('a'),
            high_replay_count: 2,
            high_replay_byte_exact: true,
            high_non_destructive: true,
            high_projected_source_mesh_sha256: h('b'),
            high_source_vertex_count: 3,
            high_source_triangle_count: 1,
            high_evaluated_part_count: 1,
            high_evaluated_triangle_count: 1,
            cohort_policy: AUTHORING_MESH_V2_HIGH_COHORT_POLICY.to_owned(),
            scope_limitations: AUTHORING_MESH_V2_HIGH_SCOPE_LIMITATIONS
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            high_structural_status: "PASS_SOURCE_STRUCTURAL".to_owned(),
            high_status: "NOT_RUN".to_owned(),
            quality_status: "structural_only".to_owned(),
            visual_status: "NOT_RUN".to_owned(),
            human_status: "NOT_RUN".to_owned(),
            engine_status: "NOT_RUN".to_owned(),
            high_mesh_created: false,
            high_stage_unlocked: false,
            production_stage_advanced: false,
            candidate_confirmed: false,
            version_created: false,
            export_performed: false,
            runtime_write_performed: true,
            persistent_user_data_touched: true,
            writer_policy: AUTHORING_MESH_V2_HIGH_WRITER_POLICY.to_owned(),
            canonicalization_policy: AUTHORING_MESH_V2_HIGH_CANONICALIZATION_POLICY.to_owned(),
            canonical_sha256: h('c'),
            created_at: NOW.to_owned(),
            bridge_sha256: h('d'),
            bridge_object_sha256: h('e'),
            request_input_sha256: h('f'),
            idempotency_key: "high-bridge-idempotency".to_owned(),
        };
        let (plan_sha256, replacement_node_id, preserved_part_ids) =
            expected_materialization_plan_sha256(
                &record,
                &embedded,
                &revision,
                SOURCE_CANDIDATE,
                &source_state_sha256,
                &source_program,
                &source_artifact.record.sha256,
                &source_readback_sha256,
                &source_program_sha256,
                &source_program_object.record.sha256,
            )
            .expect("materialization plan");
        record.representation_plan_sha256 = plan_sha256.clone();
        record.preserved_part_ids = preserved_part_ids;
        let replacement_parameters = materialization_geometry_parameters(
            &revision,
            embedded.position_m,
            embedded.rotation_rad,
        )
        .expect("replacement parameters");
        let materialized_program = json!({
            "schema_version": "GeometryProgram@2",
            "project_id": PROJECT,
            "operator_catalog_sha256": h('o'),
            "representation_plan_sha256": plan_sha256,
            "nodes": [
                {"node_id": replacement_node_id, "operator_id": AUTHORING_MESH_V2_MATERIALIZATION_OPERATOR_ID, "inputs": [], "parameters": replacement_parameters},
                {"node_id": PRESERVED_NODE, "operator_id": "forgecad.geometry.source@1", "inputs": [], "parameters": {"shape": "grip"}}
            ],
            "part_outputs": [
                {"part_id": TARGET_PART, "input_node_ids": [replacement_node_id], "material_zone_id": TARGET_ZONE, "solid": true},
                {"part_id": PRESERVED_PART, "input_node_ids": [PRESERVED_NODE], "material_zone_id": PRESERVED_ZONE, "solid": true}
            ]
        });
        let materialized_program_bytes = canonical_json_bytes(&materialized_program).unwrap();
        let materialized_program_sha256 = canonical_json_hash(&materialized_program);
        assert_eq!(
            sha256_hex(&materialized_program_bytes),
            materialized_program_sha256
        );
        let materialized_program_object = store
            .put_object(
                &materialized_program_bytes,
                Some(&materialized_program_sha256),
                "application/json",
                "geometry-program-v2",
                NOW,
            )
            .expect("materialized program");
        let materialized_artifact = store
            .put_object(&[8u8; 16], None, "model/gltf-binary", "geometry-glb", NOW)
            .expect("materialized artifact");
        let materialized_readback_value = artifact_readback(
            "high-bridge-materialized-artifact",
            MATERIALIZED_CANDIDATE,
            &materialized_artifact.record.sha256,
            &materialized_program_sha256,
            &replacement_node_id,
            part_bindings(&replacement_node_id),
        );
        let (materialized_readback_object, materialized_readback_sha256) = canonical_object(
            &store,
            materialized_readback_value,
            "geometry-artifact-readback-v2",
            "application/json",
        );
        let materialized_state_sha256 = h('m');
        insert_candidate(
            &store,
            MATERIALIZED_CANDIDATE,
            &materialized_state_sha256,
            "high-bridge-materialized-artifact",
            &materialized_artifact.record.sha256,
            "prepared",
        );
        insert_geometry_evidence_row(
            &store,
            MATERIALIZED_CANDIDATE,
            &reference.record.sha256,
            &materialized_program_sha256,
            &materialized_program_object.record.sha256,
            &materialized_artifact.record.sha256,
            &materialized_readback_object.sha256,
            &quality_report.record.sha256,
        );

        let source_part = json!({
            "operand_id": "high-bridge-operand",
            "part_id": TARGET_PART,
            "source_node_id": SOURCE_NODE,
            "material_zone_id": TARGET_ZONE,
            "source_element_lineage": [],
            "positions_m": [[0.0,0.0,0.0],[1.0,0.0,0.0],[0.0,1.0,0.0]],
            "indices": [[0,1,2]]
        });
        let source_mesh =
            json!({"schema_version":"HighEvaluatorSourceMesh@1", "parts":[source_part.clone()]});
        let projected_source_mesh_sha256 = canonical_json_hash(&source_mesh);
        let evaluator_contract = json!({
            "schema_version":"HighEvaluatorContract@1", "policy":AUTHORING_MESH_V2_HIGH_EVALUATOR_CONTRACT,
            "topology":"stitched-polygon", "continuity":"c1", "boundary_policy":"preserve",
            "crease_policy":"preserve", "adaptive_policy":"bounded", "source_binding":"exact",
            "provenance":"runtime", "deterministic_replay":"byte-exact", "non_destructive":true,
            "max_subdivision_levels":1
        });
        let mut evaluation = json!({
            "schema_version":"HighEvaluatorResult@1", "operation":AUTHORING_MESH_V2_HIGH_EVALUATOR_OPERATION,
            "source_mesh_sha256":projected_source_mesh_sha256, "evaluator_contract":evaluator_contract,
            "module_descriptors":[], "base_parts":[source_part.clone()], "evaluated_parts":[{
                "output_part_id":TARGET_PART, "part_id":TARGET_PART, "source_node_id":SOURCE_NODE,
                "material_zone_id":TARGET_ZONE, "module_id":"forgecad-owned-high", "source_operand_ids":["high-bridge-operand"],
                "source_element_lineage":[], "positions_m":[[0.0,0.0,0.0],[1.0,0.0,0.0],[0.0,1.0,0.0]], "indices":[[0,1,2]]
            }], "step_results":[], "base_triangle_count":1, "evaluated_triangle_count":1, "triangle_count":2,
            "replay_count":2, "replay_byte_exact":true, "non_destructive":true, "structural_status":"PASS_SOURCE_STRUCTURAL",
            "visual_status":"NOT_RUN", "human_status":"NOT_RUN", "quality_status":"structural_only",
            "runtime_write_performed":false, "production_stage_advanced":false, "candidate_confirmed":false,
            "version_created":false, "export_performed":false, "canonical_sha256":""
        });
        let evaluation_sha256 = canonical_json_hash(&evaluation);
        evaluation["canonical_sha256"] = Value::String(evaluation_sha256.clone());
        let mut high_readback = json!({
            "schema_version":"AuthoringMeshV2HighReadback@2", "mesh_id":MESH, "lineage_id":LINEAGE,
            "revision_id":REVISION, "revision_sha256":revision.canonical_sha256, "projected_source_mesh_sha256":projected_source_mesh_sha256,
            "source_vertex_count":3, "source_triangle_count":1, "evaluated_part_count":1, "evaluated_triangle_count":1,
            "high_evaluation_sha256":evaluation_sha256, "high_worker_algorithm_sha256":h('z'), "replay_count":2,
            "replay_byte_exact":true, "non_destructive":true, "runtime_write_performed":false,
            "production_stage_advanced":false, "candidate_confirmed":false, "version_created":false, "export_performed":false,
            "limitations":[], "canonical_sha256":""
        });
        let readback_sha256 = canonical_json_hash(&high_readback);
        high_readback["canonical_sha256"] = Value::String(readback_sha256.clone());
        let mut high_result = json!({
            "schema_version":"AuthoringMeshV2HighResult@2", "operation":AUTHORING_MESH_V2_HIGH_OPERATION,
            "mesh_id":MESH, "lineage_id":LINEAGE, "revision_id":REVISION, "revision_index":0,
            "revision_sha256":revision.canonical_sha256, "high_worker_algorithm_sha256":h('z'),
            "source_mesh":source_mesh, "evaluation":evaluation, "readback":high_readback.clone(), "replay_count":2,
            "replay_byte_exact":true, "non_destructive":true, "runtime_write_performed":false,
            "production_stage_advanced":false, "candidate_confirmed":false, "version_created":false,
            "export_performed":false, "quality_status":"structural_only", "limitations":[], "canonical_sha256":""
        });
        let high_result_sha256 = canonical_json_hash(&high_result);
        high_result["canonical_sha256"] = Value::String(high_result_sha256.clone());
        let high_result_bytes = canonical_json_bytes(&high_result).unwrap();
        let high_result_object = store
            .put_object(
                &high_result_bytes,
                None,
                AUTHORING_MESH_V2_HIGH_JSON_MIME,
                AUTHORING_MESH_V2_HIGH_RESULT_OBJECT_KIND,
                NOW,
            )
            .expect("High result");
        let high_readback_bytes = canonical_json_bytes(&high_readback).unwrap();
        let high_readback_object = store
            .put_object(
                &high_readback_bytes,
                None,
                AUTHORING_MESH_V2_HIGH_JSON_MIME,
                AUTHORING_MESH_V2_HIGH_READBACK_OBJECT_KIND,
                NOW,
            )
            .expect("High readback");
        record.materialized_candidate_state_sha256 = materialized_state_sha256;
        record.materialized_program_sha256 = materialized_program_sha256;
        record.materialized_program_object_sha256 =
            materialized_program_object.record.sha256.clone();
        record.materialized_artifact_sha256 = materialized_artifact.record.sha256.clone();
        record.materialized_artifact_object_sha256 = materialized_artifact.record.sha256.clone();
        record.materialized_artifact_readback_sha256 = materialized_readback_sha256;
        record.materialized_artifact_readback_object_sha256 = materialized_readback_object.sha256;
        record.high_evaluation_sha256 = evaluation_sha256;
        record.high_result_sha256 = high_result_sha256;
        record.high_result_object_sha256 = high_result_object.record.sha256.clone();
        record.high_readback_sha256 = readback_sha256;
        record.high_readback_object_sha256 = high_readback_object.record.sha256.clone();
        record.high_projected_source_mesh_sha256 = projected_source_mesh_sha256;
        let mut main = main_value(&record).expect("main preimage");
        main["canonical_sha256"] = Value::String(String::new());
        let bridge_sha256 = canonical_json_hash(&main);
        main["canonical_sha256"] = Value::String(bridge_sha256.clone());
        let bridge_bytes = canonical_json_bytes(&main).unwrap();
        let bridge_object = store
            .put_object(
                &bridge_bytes,
                None,
                AUTHORING_MESH_V2_HIGH_JSON_MIME,
                AUTHORING_MESH_V2_HIGH_BRIDGE_OBJECT_KIND,
                NOW,
            )
            .expect("High bridge Main");
        record.canonical_sha256 = bridge_sha256.clone();
        record.bridge_sha256 = bridge_sha256;
        record.bridge_object_sha256 = bridge_object.record.sha256.clone();
        Fixture {
            store,
            commit: AuthoringMeshV2HighBridgeCommit {
                record,
                cas: AuthoringMeshV2HighBridgeCasBundle {
                    bridge: bridge_object.record,
                    high_result: high_result_object.record,
                    high_readback: high_readback_object.record,
                },
            },
        }
    }

    fn restage_bridge_main(fixture: &mut Fixture) {
        fixture.commit.record.canonical_sha256.clear();
        fixture.commit.record.bridge_sha256.clear();
        fixture.commit.record.bridge_object_sha256.clear();
        let mut main = main_value(&fixture.commit.record).expect("High bridge Main preimage");
        main["canonical_sha256"] = Value::String(String::new());
        let semantic_sha256 = canonical_json_hash(&main);
        main["canonical_sha256"] = Value::String(semantic_sha256.clone());
        let bytes = canonical_json_bytes(&main).expect("High bridge Main bytes");
        let object = fixture
            .store
            .put_object(
                &bytes,
                None,
                AUTHORING_MESH_V2_HIGH_JSON_MIME,
                AUTHORING_MESH_V2_HIGH_BRIDGE_OBJECT_KIND,
                NOW,
            )
            .expect("restaged High bridge Main");
        fixture.commit.record.canonical_sha256 = semantic_sha256.clone();
        fixture.commit.record.bridge_sha256 = semantic_sha256;
        fixture.commit.record.bridge_object_sha256 = object.record.sha256.clone();
        fixture.commit.cas.bridge = object.record;
    }

    #[test]
    fn high_bridge_commit_replay_and_readback_validate_full_lineage() {
        let fixture = setup_fixture();
        let (stored, replayed) = fixture
            .store
            .record_authoring_mesh_v2_high_bridge_with_replay(&fixture.commit)
            .expect("first High bridge commit");
        assert!(!replayed);
        assert_eq!(stored, fixture.commit.record);

        let (replayed_record, replayed) = fixture
            .store
            .record_authoring_mesh_v2_high_bridge_with_replay(&fixture.commit)
            .expect("exact High bridge replay");
        assert!(replayed);
        assert_eq!(replayed_record, stored);

        let loaded = fixture
            .store
            .get_authoring_mesh_v2_high_bridge_by_id(PROJECT, &stored.bridge_id)
            .expect("High bridge readback")
            .expect("durable High bridge row");
        assert_eq!(loaded, stored);
        assert_eq!(
            fixture
                .store
                .read_authoring_mesh_v2_high_bridge_json(PROJECT, &stored.idempotency_key)
                .expect("High bridge Main readback")
                .expect("High bridge Main object"),
            main_value(&stored).expect("High bridge Main projection")
        );
    }

    #[test]
    fn high_bridge_rejects_representation_plan_drift_without_row() {
        let mut fixture = setup_fixture();
        fixture.commit.record.representation_plan_sha256 = h('!');
        restage_bridge_main(&mut fixture);

        let error = fixture
            .store
            .record_authoring_mesh_v2_high_bridge_with_replay(&fixture.commit)
            .expect_err("representation-plan drift must fail closed");
        assert!(format!("{error:?}").contains("REPRESENTATION_PLAN_MISMATCH"));
        assert!(fixture
            .store
            .get_authoring_mesh_v2_high_bridge(PROJECT, &fixture.commit.record.idempotency_key)
            .expect("post-failure High bridge lookup")
            .is_none());
    }

    #[test]
    fn high_bridge_rejects_materialized_readback_binding_drift_without_row() {
        let mut fixture = setup_fixture();
        let tampered_readback = artifact_readback(
            &fixture.commit.record.materialized_artifact_id,
            MATERIALIZED_CANDIDATE,
            &fixture.commit.record.materialized_artifact_object_sha256,
            &fixture.commit.record.materialized_program_sha256,
            "tampered-node",
            part_bindings("tampered-node"),
        );
        let (tampered_object, tampered_semantic) = canonical_object(
            &fixture.store,
            tampered_readback,
            "geometry-artifact-readback-v2",
            "application/json",
        );
        {
            let connection = fixture.store.connection.lock().expect("test connection");
            connection
                .execute(
                    "UPDATE geometry_candidate_evidence SET artifact_readback_object_sha256 = ?1 WHERE candidate_id = ?2",
                    params![tampered_object.sha256, MATERIALIZED_CANDIDATE],
                )
                .expect("tamper materialized evidence readback binding");
        }
        fixture.commit.record.materialized_artifact_readback_sha256 = tampered_semantic;
        fixture
            .commit
            .record
            .materialized_artifact_readback_object_sha256 = tampered_object.sha256;
        restage_bridge_main(&mut fixture);

        let error = fixture
            .store
            .record_authoring_mesh_v2_high_bridge_with_replay(&fixture.commit)
            .expect_err("materialized readback binding drift must fail closed");
        assert!(format!("{error:?}").contains("READBACK_BINDING_MISMATCH"));
        assert!(fixture
            .store
            .get_authoring_mesh_v2_high_bridge(PROJECT, &fixture.commit.record.idempotency_key)
            .expect("post-failure High bridge lookup")
            .is_none());
    }
}
