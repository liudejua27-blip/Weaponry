//! Durable Store/CAS boundary for the direct V2 High artifact.
//!
//! `authoring_mesh_v2_high_bridge` proves that a bounded V2 revision can be
//! evaluated by the first-party High worker.  It deliberately does not own a
//! GLB.  This module is the next, separate persistence seam: Runtime stages a
//! validated GLB, its strict readback and an aggregate receipt, and this
//! repository atomically binds those three objects to one immutable High
//! artifact row.  The existing High bridge remains the source of truth for
//! source/materialized revision and worker lineage; the artifact row is the
//! only durable High input that a later Low service may consume.
//!
//! This is Store-only by design.  It does not accept topology, invoke a
//! worker, advance a production stage, confirm a candidate or infer visual,
//! human, engine or commercial quality.

use forgecad_contracts::{is_opaque_id, is_sha256, CasObjectRecord};
use forgecad_core::{canonical_json_bytes, canonical_json_hash, sha256_hex};
use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{AuthoringMeshV2HighBridgeStoreRecord, Store, StoreError};

pub const AUTHORING_MESH_V2_HIGH_ARTIFACT_SCHEMA_VERSION: &str = "AuthoringMeshV2HighArtifact@1";
pub const AUTHORING_MESH_V2_HIGH_ARTIFACT_RECORD_SCHEMA_VERSION: &str =
    "AuthoringMeshV2HighArtifactStoreRecord@1";
pub const AUTHORING_MESH_V2_HIGH_ARTIFACT_GLB_OBJECT_KIND: &str =
    "authoring-mesh-v2-high-artifact-glb@1";
pub const AUTHORING_MESH_V2_HIGH_ARTIFACT_READBACK_OBJECT_KIND: &str =
    "authoring-mesh-v2-high-artifact-readback@1";
pub const AUTHORING_MESH_V2_HIGH_ARTIFACT_RECEIPT_OBJECT_KIND: &str =
    "authoring-mesh-v2-high-artifact-receipt@1";
pub const AUTHORING_MESH_V2_HIGH_ARTIFACT_GLB_MIME: &str = "model/gltf-binary";
pub const AUTHORING_MESH_V2_HIGH_ARTIFACT_JSON_MIME: &str = "application/json";
pub const AUTHORING_MESH_V2_HIGH_ARTIFACT_MAX_GLB_BYTES: u64 = 256 * 1024 * 1024;
pub const AUTHORING_MESH_V2_HIGH_ARTIFACT_MAX_JSON_BYTES: u64 = 8 * 1024 * 1024;
pub const AUTHORING_MESH_V2_HIGH_ARTIFACT_HASH_POLICY: &str =
    "artifact-sha256-equals-object-sha256-until-semantic-artifact-contract@1";
pub const AUTHORING_MESH_V2_HIGH_ARTIFACT_STATUS: &str =
    "runtime-owned-store-authoring-mesh-v2-high-artifact@1";
pub const AUTHORING_MESH_V2_HIGH_ARTIFACT_CANONICALIZATION_POLICY: &str =
    "canonical-json-sha256-excluding-canonical-sha256@1";
pub const AUTHORING_MESH_V2_HIGH_ARTIFACT_WRITER_POLICY: &str =
    "forgecad-runtime-only-state-writer@1";

/// Store-local immutable row.  The public Main shape can be introduced by a
/// later Contract slice without changing this persistence boundary.  Hashes
/// ending in `_object_sha256` identify CAS bytes; the corresponding semantic
/// hashes are kept independently even while the current GLB policy requires
/// them to be equal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthoringMeshV2HighArtifactStoreRecord {
    pub schema_version: String,
    pub project_id: String,
    pub artifact_id: String,
    pub bridge_id: String,
    pub bridge_sha256: String,
    pub bridge_object_sha256: String,
    pub source_binding_id: String,
    pub source_binding_sha256: String,
    pub source_binding_object_sha256: String,
    pub mesh_id: String,
    pub lineage_id: String,
    pub revision_id: String,
    pub revision_index: u64,
    pub revision_sha256: String,
    pub revision_object_sha256: String,
    pub materialized_candidate_id: String,
    pub materialized_candidate_state_sha256: String,
    pub materialized_program_sha256: String,
    pub materialized_program_object_sha256: String,
    pub representation_plan_sha256: String,
    pub source_node_id: String,
    pub part_id: String,
    pub material_zone_id: String,
    pub solid: bool,
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
    pub high_source_vertex_count: u64,
    pub high_source_triangle_count: u64,
    pub high_evaluated_part_count: u64,
    pub high_evaluated_triangle_count: u64,
    pub high_artifact_sha256: String,
    pub high_artifact_object_sha256: String,
    pub high_artifact_size_bytes: u64,
    pub high_artifact_readback_sha256: String,
    pub high_artifact_readback_object_sha256: String,
    pub receipt_sha256: String,
    pub receipt_object_sha256: String,
    pub materialized_artifact_hash_policy: String,
    pub materialization_status: String,
    pub structural_status: String,
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
    pub request_input_sha256: String,
    pub idempotency_key: String,
    pub created_at: String,
}

/// The three CAS objects are staged by Runtime before entering the SQLite
/// transaction.  The Store verifies their metadata and bytes, but never
/// synthesizes one object from another.
#[derive(Debug, Clone)]
pub struct AuthoringMeshV2HighArtifactCasBundle {
    pub artifact: CasObjectRecord,
    pub readback: CasObjectRecord,
    pub receipt: CasObjectRecord,
}

#[derive(Debug, Clone)]
pub struct AuthoringMeshV2HighArtifactCommit {
    pub record: AuthoringMeshV2HighArtifactStoreRecord,
    pub cas: AuthoringMeshV2HighArtifactCasBundle,
}

fn contract(code: &str, message: impl Into<String>) -> StoreError {
    StoreError::Contract {
        code: code.to_owned(),
        message: message.into(),
    }
}

fn record_value(record: &AuthoringMeshV2HighArtifactStoreRecord) -> Result<Value, StoreError> {
    serde_json::to_value(record).map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn record_canonical_sha256(
    record: &AuthoringMeshV2HighArtifactStoreRecord,
) -> Result<String, StoreError> {
    let mut value = record_value(record)?;
    value["canonical_sha256"] = Value::String(String::new());
    Ok(canonical_json_hash(&value))
}

fn id_fields(record: &AuthoringMeshV2HighArtifactStoreRecord) -> [&str; 14] {
    [
        &record.project_id,
        &record.artifact_id,
        &record.bridge_id,
        &record.source_binding_id,
        &record.mesh_id,
        &record.lineage_id,
        &record.revision_id,
        &record.materialized_candidate_id,
        &record.source_node_id,
        &record.part_id,
        &record.material_zone_id,
        &record.idempotency_key,
        &record.materialization_status,
        &record.structural_status,
    ]
}

fn hash_fields(record: &AuthoringMeshV2HighArtifactStoreRecord) -> [&str; 26] {
    [
        &record.bridge_sha256,
        &record.bridge_object_sha256,
        &record.source_binding_sha256,
        &record.source_binding_object_sha256,
        &record.revision_sha256,
        &record.revision_object_sha256,
        &record.materialized_candidate_state_sha256,
        &record.materialized_program_sha256,
        &record.materialized_program_object_sha256,
        &record.representation_plan_sha256,
        &record.high_execution_request_sha256,
        &record.high_evaluation_sha256,
        &record.high_result_sha256,
        &record.high_result_object_sha256,
        &record.high_readback_sha256,
        &record.high_readback_object_sha256,
        &record.high_worker_algorithm_sha256,
        &record.high_worker_build_cohort_sha256,
        &record.high_artifact_sha256,
        &record.high_artifact_object_sha256,
        &record.high_artifact_readback_sha256,
        &record.high_artifact_readback_object_sha256,
        &record.receipt_sha256,
        &record.receipt_object_sha256,
        &record.canonical_sha256,
        &record.request_input_sha256,
    ]
}

fn validate_record(record: &AuthoringMeshV2HighArtifactStoreRecord) -> Result<(), StoreError> {
    if record.schema_version != AUTHORING_MESH_V2_HIGH_ARTIFACT_RECORD_SCHEMA_VERSION
        || id_fields(record).iter().any(|value| !is_opaque_id(value))
        || hash_fields(record).iter().any(|value| !is_sha256(value))
        || record.revision_index > 1_000_000
        || record.high_replay_count != 2
        || !record.high_replay_byte_exact
        || !record.high_non_destructive
        || record.high_artifact_sha256 != record.high_artifact_object_sha256
        || record.high_artifact_size_bytes == 0
        || record.high_artifact_size_bytes > AUTHORING_MESH_V2_HIGH_ARTIFACT_MAX_GLB_BYTES
        || record.materialized_artifact_hash_policy != AUTHORING_MESH_V2_HIGH_ARTIFACT_HASH_POLICY
        || record.materialization_status != "prepared"
        || record.structural_status != "PASS_SOURCE_STRUCTURAL"
        || record.visual_status != "NOT_RUN"
        || record.human_status != "NOT_RUN"
        || record.engine_status != "NOT_RUN"
        || !record.high_mesh_created
        || record.high_stage_unlocked
        || record.production_stage_advanced
        || record.candidate_confirmed
        || record.version_created
        || record.export_performed
        || !record.runtime_write_performed
        || !record.persistent_user_data_touched
        || record.writer_policy != AUTHORING_MESH_V2_HIGH_ARTIFACT_WRITER_POLICY
        || record.canonicalization_policy != AUTHORING_MESH_V2_HIGH_ARTIFACT_CANONICALIZATION_POLICY
        || record.created_at.is_empty()
        || record.created_at.len() > 128
        || record.created_at.contains('/')
        || record.created_at.contains('\\')
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_ARTIFACT_RECORD_INVALID",
            "direct V2 High artifact record has invalid identity, policy or status",
        ));
    }
    if record_canonical_sha256(record)? != record.canonical_sha256 {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_ARTIFACT_CANONICAL_MISMATCH",
            "direct V2 High artifact record canonical hash differs",
        ));
    }
    Ok(())
}

fn same_record(
    left: &AuthoringMeshV2HighArtifactStoreRecord,
    right: &AuthoringMeshV2HighArtifactStoreRecord,
) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    left.created_at.clear();
    right.created_at.clear();
    left == right
}

fn read_object(transaction: &Transaction<'_>, sha256: &str) -> Result<CasObjectRecord, StoreError> {
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

fn validate_object(
    transaction: &Transaction<'_>,
    cas: &super::CasStore,
    supplied: &CasObjectRecord,
    expected_sha256: &str,
    expected_mime: &str,
    expected_kind: &str,
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
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_ARTIFACT_CAS_METADATA_INVALID",
            format!("{role} CAS metadata differs from the artifact binding"),
        ));
    }
    let registered = read_object(transaction, expected_sha256).map_err(|error| match error {
        StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => contract(
            "AUTHORING_MESH_V2_HIGH_ARTIFACT_CAS_MISSING",
            format!("{role} CAS object is not registered"),
        ),
        other => other,
    })?;
    if registered.size_bytes != supplied.size_bytes
        || registered.mime != supplied.mime
        || registered.kind != supplied.kind
        || !matches!(registered.reachability.as_str(), "temporary" | "reachable")
        || (require_reachable && registered.reachability != "reachable")
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_ARTIFACT_CAS_METADATA_INVALID",
            format!("registered {role} CAS metadata differs"),
        ));
    }
    let bytes = cas
        .read_verified_bounded(expected_sha256, max_bytes)
        .map_err(StoreError::from)?;
    if bytes.len() as u64 != supplied.size_bytes || sha256_hex(&bytes) != expected_sha256 {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_ARTIFACT_CAS_HASH_MISMATCH",
            format!("{role} CAS bytes do not match their object hash"),
        ));
    }
    Ok(bytes)
}

fn validate_glb(bytes: &[u8], expected_size: u64) -> Result<(), StoreError> {
    // A GLB header is enough for this Store seam.  Mesh topology and visual
    // quality belong to the Worker/readback/quality layers, not persistence.
    if bytes.len() as u64 != expected_size
        || bytes.len() < 12
        || &bytes[0..4] != b"glTF"
        || u32::from_le_bytes(bytes[4..8].try_into().expect("GLB version bytes")) != 2
        || u64::from(u32::from_le_bytes(
            bytes[8..12].try_into().expect("GLB length bytes"),
        )) != bytes.len() as u64
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_ARTIFACT_GLB_INVALID",
            "High artifact is not a bounded version-2 GLB with an exact length",
        ));
    }
    Ok(())
}

fn object_canonical_hash(value: &Value, role: &str) -> Result<String, StoreError> {
    let mut preimage = value.clone();
    let object = preimage.as_object_mut().ok_or_else(|| {
        contract(
            "AUTHORING_MESH_V2_HIGH_ARTIFACT_JSON_INVALID",
            format!("{role} is not a JSON object"),
        )
    })?;
    object.insert("canonical_sha256".to_owned(), Value::String(String::new()));
    // The readback carries its own semantic identity for downstream exact
    // lookup. Exclude that identity from the preimage as well, otherwise the
    // value would require an impossible fixed point. Receipts may carry the
    // same optional field; absent fields remain absent from the preimage.
    let semantic_field = if role.contains("readback") {
        Some("high_artifact_readback_sha256")
    } else if role.contains("receipt") {
        Some("receipt_sha256")
    } else {
        None
    };
    if let Some(field) = semantic_field {
        if object.contains_key(field) {
            object.insert(field.to_owned(), Value::String(String::new()));
        }
    }
    Ok(canonical_json_hash(&preimage))
}

fn json_string<'a>(value: &'a Value, field: &str, role: &str) -> Result<&'a str, StoreError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            contract(
                "AUTHORING_MESH_V2_HIGH_ARTIFACT_JSON_INVALID",
                format!("{role}.{field} is missing"),
            )
        })
}

fn expect_string(value: &Value, field: &str, expected: &str, role: &str) -> Result<(), StoreError> {
    if json_string(value, field, role)? != expected {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_ARTIFACT_BINDING_MISMATCH",
            format!("{role}.{field} differs from the durable artifact row"),
        ));
    }
    Ok(())
}

fn expect_u64(value: &Value, field: &str, expected: u64, role: &str) -> Result<(), StoreError> {
    if value.get(field).and_then(Value::as_u64) != Some(expected) {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_ARTIFACT_BINDING_MISMATCH",
            format!("{role}.{field} differs from the durable artifact row"),
        ));
    }
    Ok(())
}

fn validate_bound_json(
    bytes: &[u8],
    schema_version: &str,
    record: &AuthoringMeshV2HighArtifactStoreRecord,
    role: &str,
) -> Result<Value, StoreError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        contract(
            "AUTHORING_MESH_V2_HIGH_ARTIFACT_JSON_INVALID",
            format!("{role} JSON is invalid: {error}"),
        )
    })?;
    if canonical_json_bytes(&value).map_err(|error| StoreError::InvalidData(error.to_string()))?
        != bytes
        || json_string(&value, "schema_version", role)? != schema_version
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_ARTIFACT_JSON_CANONICAL_MISMATCH",
            format!("{role} is not canonical or has the wrong schema"),
        ));
    }
    let canonical = json_string(&value, "canonical_sha256", role)?;
    if !is_sha256(canonical) || object_canonical_hash(&value, role)? != canonical {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_ARTIFACT_JSON_CANONICAL_MISMATCH",
            format!("{role} canonical hash is invalid"),
        ));
    }
    for (field, expected) in [
        ("project_id", record.project_id.as_str()),
        ("artifact_id", record.artifact_id.as_str()),
        ("bridge_id", record.bridge_id.as_str()),
        ("bridge_sha256", record.bridge_sha256.as_str()),
        ("bridge_object_sha256", record.bridge_object_sha256.as_str()),
        ("revision_id", record.revision_id.as_str()),
        ("revision_sha256", record.revision_sha256.as_str()),
        (
            "revision_object_sha256",
            record.revision_object_sha256.as_str(),
        ),
        ("source_binding_id", record.source_binding_id.as_str()),
        (
            "source_binding_sha256",
            record.source_binding_sha256.as_str(),
        ),
        (
            "source_binding_object_sha256",
            record.source_binding_object_sha256.as_str(),
        ),
        ("high_result_sha256", record.high_result_sha256.as_str()),
        (
            "high_result_object_sha256",
            record.high_result_object_sha256.as_str(),
        ),
        ("high_readback_sha256", record.high_readback_sha256.as_str()),
        (
            "high_readback_object_sha256",
            record.high_readback_object_sha256.as_str(),
        ),
        (
            "high_worker_algorithm_sha256",
            record.high_worker_algorithm_sha256.as_str(),
        ),
        (
            "high_worker_build_cohort_sha256",
            record.high_worker_build_cohort_sha256.as_str(),
        ),
        ("revision_id", record.revision_id.as_str()),
    ] {
        expect_string(&value, field, expected, role)?;
    }
    expect_u64(&value, "revision_index", record.revision_index, role)?;
    expect_string(&value, "structural_status", &record.structural_status, role)?;
    for (field, expected) in [
        ("visual_status", "NOT_RUN"),
        ("human_status", "NOT_RUN"),
        ("engine_status", "NOT_RUN"),
    ] {
        expect_string(&value, field, expected, role)?;
    }
    Ok(value)
}

fn validate_readback(
    bytes: &[u8],
    record: &AuthoringMeshV2HighArtifactStoreRecord,
) -> Result<Value, StoreError> {
    let value = validate_bound_json(
        bytes,
        "AuthoringMeshV2HighArtifactStoreReadback@1",
        record,
        "High artifact readback",
    )?;
    expect_string(
        &value,
        "high_artifact_sha256",
        &record.high_artifact_sha256,
        "High artifact readback",
    )?;
    expect_string(
        &value,
        "high_artifact_object_sha256",
        &record.high_artifact_object_sha256,
        "High artifact readback",
    )?;
    expect_string(
        &value,
        "high_artifact_readback_sha256",
        &record.high_artifact_readback_sha256,
        "High artifact readback",
    )?;
    expect_u64(
        &value,
        "high_artifact_size_bytes",
        record.high_artifact_size_bytes,
        "High artifact readback",
    )?;
    if value.get("replay_count").and_then(Value::as_u64) != Some(2)
        || value.get("replay_byte_exact").and_then(Value::as_bool) != Some(true)
        || value.get("non_destructive").and_then(Value::as_bool) != Some(true)
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_ARTIFACT_READBACK_POLICY_INVALID",
            "High artifact readback does not prove deterministic non-destructive replay",
        ));
    }
    Ok(value)
}

fn validate_receipt(
    bytes: &[u8],
    record: &AuthoringMeshV2HighArtifactStoreRecord,
) -> Result<Value, StoreError> {
    let value = validate_bound_json(
        bytes,
        "AuthoringMeshV2HighArtifactReceipt@1",
        record,
        "High artifact receipt",
    )?;
    for (field, expected) in [
        (
            "high_artifact_readback_sha256",
            record.high_artifact_readback_sha256.as_str(),
        ),
        (
            "high_artifact_readback_object_sha256",
            record.high_artifact_readback_object_sha256.as_str(),
        ),
        ("receipt_status", "prepared"),
        ("materialization_status", "prepared"),
    ] {
        expect_string(&value, field, expected, "High artifact receipt")?;
    }
    Ok(value)
}

fn bridge_record(
    transaction: &Transaction<'_>,
    record: &AuthoringMeshV2HighArtifactStoreRecord,
) -> Result<AuthoringMeshV2HighBridgeStoreRecord, StoreError> {
    let record_json: Option<String> = transaction
        .query_row(
            "SELECT record_json FROM authoring_mesh_v2_high_bridge_records WHERE project_id = ?1 AND bridge_id = ?2 AND bridge_sha256 = ?3 AND bridge_object_sha256 = ?4",
            params![
                record.project_id,
                record.bridge_id,
                record.bridge_sha256,
                record.bridge_object_sha256
            ],
            |row| row.get(0),
        )
        .optional()?;
    let record_json = record_json.ok_or_else(|| {
        contract(
            "AUTHORING_MESH_V2_HIGH_ARTIFACT_BRIDGE_MISSING",
            "direct V2 High artifact requires an exact durable High bridge",
        )
    })?;
    let bridge: AuthoringMeshV2HighBridgeStoreRecord = serde_json::from_str(&record_json)
        .map_err(|error| StoreError::InvalidData(format!("High bridge record JSON: {error}")))?;
    let pairs = [
        (
            "project_id",
            record.project_id.as_str(),
            bridge.project_id.as_str(),
        ),
        (
            "bridge_id",
            record.bridge_id.as_str(),
            bridge.bridge_id.as_str(),
        ),
        (
            "bridge_sha256",
            record.bridge_sha256.as_str(),
            bridge.bridge_sha256.as_str(),
        ),
        (
            "bridge_object_sha256",
            record.bridge_object_sha256.as_str(),
            bridge.bridge_object_sha256.as_str(),
        ),
        (
            "source_binding_id",
            record.source_binding_id.as_str(),
            bridge.source_binding_id.as_str(),
        ),
        (
            "source_binding_sha256",
            record.source_binding_sha256.as_str(),
            bridge.source_binding_sha256.as_str(),
        ),
        (
            "source_binding_object_sha256",
            record.source_binding_object_sha256.as_str(),
            bridge.source_binding_object_sha256.as_str(),
        ),
        ("mesh_id", record.mesh_id.as_str(), bridge.mesh_id.as_str()),
        (
            "lineage_id",
            record.lineage_id.as_str(),
            bridge.lineage_id.as_str(),
        ),
        (
            "revision_id",
            record.revision_id.as_str(),
            bridge.revision_id.as_str(),
        ),
        (
            "revision_sha256",
            record.revision_sha256.as_str(),
            bridge.revision_sha256.as_str(),
        ),
        (
            "revision_object_sha256",
            record.revision_object_sha256.as_str(),
            bridge.revision_object_sha256.as_str(),
        ),
        (
            "materialized_candidate_id",
            record.materialized_candidate_id.as_str(),
            bridge.materialized_candidate_id.as_str(),
        ),
        (
            "materialized_candidate_state_sha256",
            record.materialized_candidate_state_sha256.as_str(),
            bridge.materialized_candidate_state_sha256.as_str(),
        ),
        (
            "materialized_program_sha256",
            record.materialized_program_sha256.as_str(),
            bridge.materialized_program_sha256.as_str(),
        ),
        (
            "materialized_program_object_sha256",
            record.materialized_program_object_sha256.as_str(),
            bridge.materialized_program_object_sha256.as_str(),
        ),
        (
            "representation_plan_sha256",
            record.representation_plan_sha256.as_str(),
            bridge.representation_plan_sha256.as_str(),
        ),
        (
            "source_node_id",
            record.source_node_id.as_str(),
            bridge.source_node_id.as_str(),
        ),
        ("part_id", record.part_id.as_str(), bridge.part_id.as_str()),
        (
            "material_zone_id",
            record.material_zone_id.as_str(),
            bridge.material_zone_id.as_str(),
        ),
        (
            "high_execution_request_sha256",
            record.high_execution_request_sha256.as_str(),
            bridge.high_execution_request_sha256.as_str(),
        ),
        (
            "high_evaluation_sha256",
            record.high_evaluation_sha256.as_str(),
            bridge.high_evaluation_sha256.as_str(),
        ),
        (
            "high_result_sha256",
            record.high_result_sha256.as_str(),
            bridge.high_result_sha256.as_str(),
        ),
        (
            "high_result_object_sha256",
            record.high_result_object_sha256.as_str(),
            bridge.high_result_object_sha256.as_str(),
        ),
        (
            "high_readback_sha256",
            record.high_readback_sha256.as_str(),
            bridge.high_readback_sha256.as_str(),
        ),
        (
            "high_readback_object_sha256",
            record.high_readback_object_sha256.as_str(),
            bridge.high_readback_object_sha256.as_str(),
        ),
        (
            "high_worker_algorithm_sha256",
            record.high_worker_algorithm_sha256.as_str(),
            bridge.high_worker_algorithm_sha256.as_str(),
        ),
        (
            "high_worker_build_cohort_sha256",
            record.high_worker_build_cohort_sha256.as_str(),
            bridge.high_worker_build_cohort_sha256.as_str(),
        ),
    ];
    if pairs.iter().any(|(_, left, right)| left != right)
        || record.revision_index != bridge.revision_index
        || record.solid != bridge.solid
    {
        return Err(contract(
            "AUTHORING_MESH_V2_HIGH_ARTIFACT_BRIDGE_BINDING_MISMATCH",
            "direct V2 High artifact fields differ from the High bridge",
        ));
    }
    Ok(bridge)
}

fn bridge_roots(bridge: &AuthoringMeshV2HighBridgeStoreRecord) -> Vec<String> {
    [
        bridge.bridge_object_sha256.clone(),
        bridge.source_binding_object_sha256.clone(),
        bridge.revision_object_sha256.clone(),
        bridge.materialized_program_object_sha256.clone(),
        bridge.materialized_artifact_object_sha256.clone(),
        bridge.materialized_artifact_readback_object_sha256.clone(),
        bridge.high_result_object_sha256.clone(),
        bridge.high_readback_object_sha256.clone(),
    ]
    .into_iter()
    .collect()
}

fn object_hashes(record: &AuthoringMeshV2HighArtifactStoreRecord) -> Vec<String> {
    let mut roots = vec![
        record.high_artifact_object_sha256.clone(),
        record.high_artifact_readback_object_sha256.clone(),
        record.receipt_object_sha256.clone(),
        record.bridge_object_sha256.clone(),
        record.source_binding_object_sha256.clone(),
        record.revision_object_sha256.clone(),
        record.materialized_program_object_sha256.clone(),
        record.high_result_object_sha256.clone(),
        record.high_readback_object_sha256.clone(),
    ];
    roots.sort();
    roots.dedup();
    roots
}

fn read_record(
    transaction: &Transaction<'_>,
    project_id: &str,
    idempotency_key: &str,
) -> Result<Option<AuthoringMeshV2HighArtifactStoreRecord>, StoreError> {
    let record_json: Option<String> = transaction
        .query_row(
            "SELECT record_json FROM authoring_mesh_v2_high_artifact_records WHERE project_id = ?1 AND idempotency_key = ?2",
            params![project_id, idempotency_key],
            |row| row.get(0),
        )
        .optional()?;
    record_json
        .map(|value| {
            serde_json::from_str(&value).map_err(|error| {
                StoreError::InvalidData(format!("High artifact record JSON: {error}"))
            })
        })
        .transpose()
}

pub(crate) fn ensure_table(transaction: &Transaction<'_>) -> Result<(), StoreError> {
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS authoring_mesh_v2_high_artifact_records (
             schema_version TEXT NOT NULL CHECK (schema_version = 'AuthoringMeshV2HighArtifactStoreRecord@1'),
             project_id TEXT NOT NULL REFERENCES projects(project_id),
             artifact_id TEXT NOT NULL,
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
             materialized_candidate_id TEXT NOT NULL,
             materialized_candidate_state_sha256 TEXT NOT NULL,
             materialized_program_sha256 TEXT NOT NULL,
             materialized_program_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             high_result_sha256 TEXT NOT NULL,
             high_result_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             high_readback_sha256 TEXT NOT NULL,
             high_readback_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             high_worker_algorithm_sha256 TEXT NOT NULL,
             high_worker_build_cohort_sha256 TEXT NOT NULL,
             high_artifact_sha256 TEXT NOT NULL,
             high_artifact_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             high_artifact_size_bytes INTEGER NOT NULL CHECK (high_artifact_size_bytes > 0 AND high_artifact_size_bytes <= 268435456),
             high_artifact_readback_sha256 TEXT NOT NULL,
             high_artifact_readback_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             receipt_sha256 TEXT NOT NULL,
             receipt_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             idempotency_key TEXT NOT NULL,
             request_input_sha256 TEXT NOT NULL,
             canonical_sha256 TEXT NOT NULL,
             created_at TEXT NOT NULL,
             record_json TEXT NOT NULL,
             object_hashes_json TEXT NOT NULL,
             PRIMARY KEY (project_id, idempotency_key),
             UNIQUE (project_id, artifact_id),
             UNIQUE (project_id, high_artifact_sha256)
         );
         CREATE INDEX IF NOT EXISTS authoring_mesh_v2_high_artifact_bridge_idx
             ON authoring_mesh_v2_high_artifact_records(project_id, bridge_id, bridge_sha256);
         CREATE INDEX IF NOT EXISTS authoring_mesh_v2_high_artifact_revision_idx
             ON authoring_mesh_v2_high_artifact_records(project_id, mesh_id, lineage_id, revision_id, revision_index);
         CREATE INDEX IF NOT EXISTS authoring_mesh_v2_high_artifact_object_idx
             ON authoring_mesh_v2_high_artifact_records(high_artifact_object_sha256, high_artifact_readback_object_sha256, receipt_object_sha256);",
    )?;
    Ok(())
}

impl Store {
    /// Atomically persist a direct V2 High GLB, its readback and aggregate
    /// receipt.  All upstream validation happens before INSERT; the single
    /// SQLite transaction also owns the reachability transition.
    pub fn record_authoring_mesh_v2_high_artifact_with_replay(
        &self,
        commit: &AuthoringMeshV2HighArtifactCommit,
    ) -> Result<(AuthoringMeshV2HighArtifactStoreRecord, bool), StoreError> {
        validate_record(&commit.record)?;
        if commit.cas.artifact.sha256 != commit.record.high_artifact_object_sha256
            || commit.cas.readback.sha256 != commit.record.high_artifact_readback_object_sha256
            || commit.cas.receipt.sha256 != commit.record.receipt_object_sha256
        {
            return Err(contract(
                "AUTHORING_MESH_V2_HIGH_ARTIFACT_CAS_BINDING_MISMATCH",
                "staged High artifact CAS objects differ from the durable record",
            ));
        }
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        ensure_table(&transaction)?;
        let bridge = bridge_record(&transaction, &commit.record)?;
        let bridge_object = read_object(&transaction, &commit.record.bridge_object_sha256)?;
        let bridge_bytes = validate_object(
            &transaction,
            &self.cas,
            &bridge_object,
            &commit.record.bridge_object_sha256,
            super::AUTHORING_MESH_V2_HIGH_JSON_MIME,
            super::AUTHORING_MESH_V2_HIGH_BRIDGE_OBJECT_KIND,
            super::AUTHORING_MESH_V2_HIGH_MAX_BRIDGE_BYTES,
            true,
            "High bridge",
        )?;
        let bridge_main = super::authoring_mesh_v2_high_bridge::main_value(&bridge)?;
        let bridge_value: Value = serde_json::from_slice(&bridge_bytes).map_err(|error| {
            contract(
                "AUTHORING_MESH_V2_HIGH_ARTIFACT_BRIDGE_INVALID",
                format!("High bridge CAS JSON is invalid: {error}"),
            )
        })?;
        if bridge_value != bridge_main {
            return Err(contract(
                "AUTHORING_MESH_V2_HIGH_ARTIFACT_BRIDGE_INVALID",
                "High bridge CAS object differs from its durable Main record",
            ));
        }
        let artifact_bytes = validate_object(
            &transaction,
            &self.cas,
            &commit.cas.artifact,
            &commit.record.high_artifact_object_sha256,
            AUTHORING_MESH_V2_HIGH_ARTIFACT_GLB_MIME,
            AUTHORING_MESH_V2_HIGH_ARTIFACT_GLB_OBJECT_KIND,
            AUTHORING_MESH_V2_HIGH_ARTIFACT_MAX_GLB_BYTES,
            false,
            "High artifact GLB",
        )?;
        validate_glb(&artifact_bytes, commit.record.high_artifact_size_bytes)?;
        if sha256_hex(&artifact_bytes) != commit.record.high_artifact_sha256 {
            return Err(contract(
                "AUTHORING_MESH_V2_HIGH_ARTIFACT_GLB_HASH_MISMATCH",
                "High artifact semantic hash differs from GLB bytes",
            ));
        }
        let readback_bytes = validate_object(
            &transaction,
            &self.cas,
            &commit.cas.readback,
            &commit.record.high_artifact_readback_object_sha256,
            AUTHORING_MESH_V2_HIGH_ARTIFACT_JSON_MIME,
            AUTHORING_MESH_V2_HIGH_ARTIFACT_READBACK_OBJECT_KIND,
            AUTHORING_MESH_V2_HIGH_ARTIFACT_MAX_JSON_BYTES,
            false,
            "High artifact readback",
        )?;
        validate_readback(&readback_bytes, &commit.record)?;
        let receipt_bytes = validate_object(
            &transaction,
            &self.cas,
            &commit.cas.receipt,
            &commit.record.receipt_object_sha256,
            AUTHORING_MESH_V2_HIGH_ARTIFACT_JSON_MIME,
            AUTHORING_MESH_V2_HIGH_ARTIFACT_RECEIPT_OBJECT_KIND,
            AUTHORING_MESH_V2_HIGH_ARTIFACT_MAX_JSON_BYTES,
            false,
            "High artifact receipt",
        )?;
        let receipt = validate_receipt(&receipt_bytes, &commit.record)?;
        if object_canonical_hash(&receipt, "High artifact receipt")? != commit.record.receipt_sha256
        {
            return Err(contract(
                "AUTHORING_MESH_V2_HIGH_ARTIFACT_RECEIPT_HASH_MISMATCH",
                "aggregate receipt semantic hash differs",
            ));
        }
        let _ = bridge;
        if let Some(existing) = read_record(
            &transaction,
            &commit.record.project_id,
            &commit.record.idempotency_key,
        )? {
            validate_record(&existing)?;
            if !same_record(&existing, &commit.record)
                || existing.high_artifact_object_sha256 != commit.record.high_artifact_object_sha256
                || existing.high_artifact_readback_object_sha256
                    != commit.record.high_artifact_readback_object_sha256
                || existing.receipt_object_sha256 != commit.record.receipt_object_sha256
            {
                return Err(contract(
                    "AUTHORING_MESH_V2_HIGH_ARTIFACT_IDEMPOTENCY_CONFLICT",
                    "project and idempotency key are bound to different High artifact content",
                ));
            }
            super::mark_reachable_in_transaction(&transaction, &object_hashes(&existing))?;
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
                "High artifact project does not exist",
            ));
        }
        let identity_conflict: Option<String> = transaction
            .query_row(
                "SELECT idempotency_key FROM authoring_mesh_v2_high_artifact_records WHERE project_id = ?1 AND (artifact_id = ?2 OR high_artifact_sha256 = ?3)",
                params![commit.record.project_id, commit.record.artifact_id, commit.record.high_artifact_sha256],
                |row| row.get(0),
            )
            .optional()?;
        if identity_conflict.is_some() {
            return Err(contract(
                "AUTHORING_MESH_V2_HIGH_ARTIFACT_IDENTITY_CONFLICT",
                "High artifact identity is already bound to another request",
            ));
        }
        let record_json = serde_json::to_string(&commit.record)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let object_hashes_json = serde_json::to_string(&object_hashes(&commit.record))
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        transaction.execute(
            "INSERT INTO authoring_mesh_v2_high_artifact_records (schema_version, project_id, artifact_id, bridge_id, bridge_sha256, bridge_object_sha256, source_binding_id, source_binding_sha256, source_binding_object_sha256, mesh_id, lineage_id, revision_id, revision_index, revision_sha256, revision_object_sha256, materialized_candidate_id, materialized_candidate_state_sha256, materialized_program_sha256, materialized_program_object_sha256, high_result_sha256, high_result_object_sha256, high_readback_sha256, high_readback_object_sha256, high_worker_algorithm_sha256, high_worker_build_cohort_sha256, high_artifact_sha256, high_artifact_object_sha256, high_artifact_size_bytes, high_artifact_readback_sha256, high_artifact_readback_object_sha256, receipt_sha256, receipt_object_sha256, idempotency_key, request_input_sha256, canonical_sha256, created_at, record_json, object_hashes_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37, ?38)",
            params![
                commit.record.schema_version,
                commit.record.project_id,
                commit.record.artifact_id,
                commit.record.bridge_id,
                commit.record.bridge_sha256,
                commit.record.bridge_object_sha256,
                commit.record.source_binding_id,
                commit.record.source_binding_sha256,
                commit.record.source_binding_object_sha256,
                commit.record.mesh_id,
                commit.record.lineage_id,
                commit.record.revision_id,
                i64::try_from(commit.record.revision_index).map_err(|_| StoreError::InvalidData("revision index too large".to_owned()))?,
                commit.record.revision_sha256,
                commit.record.revision_object_sha256,
                commit.record.materialized_candidate_id,
                commit.record.materialized_candidate_state_sha256,
                commit.record.materialized_program_sha256,
                commit.record.materialized_program_object_sha256,
                commit.record.high_result_sha256,
                commit.record.high_result_object_sha256,
                commit.record.high_readback_sha256,
                commit.record.high_readback_object_sha256,
                commit.record.high_worker_algorithm_sha256,
                commit.record.high_worker_build_cohort_sha256,
                commit.record.high_artifact_sha256,
                commit.record.high_artifact_object_sha256,
                i64::try_from(commit.record.high_artifact_size_bytes).map_err(|_| StoreError::InvalidData("High artifact is too large".to_owned()))?,
                commit.record.high_artifact_readback_sha256,
                commit.record.high_artifact_readback_object_sha256,
                commit.record.receipt_sha256,
                commit.record.receipt_object_sha256,
                commit.record.idempotency_key,
                commit.record.request_input_sha256,
                commit.record.canonical_sha256,
                commit.record.created_at,
                record_json,
                object_hashes_json,
            ],
        )?;
        let mut roots = bridge_roots(&bridge_record(&transaction, &commit.record)?);
        roots.extend(object_hashes(&commit.record));
        roots.sort();
        roots.dedup();
        super::mark_reachable_in_transaction(&transaction, &roots)?;
        let stored = read_record(
            &transaction,
            &commit.record.project_id,
            &commit.record.idempotency_key,
        )?
        .ok_or_else(|| {
            contract(
                "AUTHORING_MESH_V2_HIGH_ARTIFACT_CORRUPT",
                "High artifact row disappeared before commit",
            )
        })?;
        validate_record(&stored)?;
        transaction.commit()?;
        Ok((stored, false))
    }

    pub fn get_authoring_mesh_v2_high_artifact(
        &self,
        project_id: &str,
        idempotency_key: &str,
    ) -> Result<Option<AuthoringMeshV2HighArtifactStoreRecord>, StoreError> {
        if !is_opaque_id(project_id) || !is_opaque_id(idempotency_key) {
            return Err(StoreError::InvalidData(
                "High artifact lookup identity is invalid".to_owned(),
            ));
        }
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        ensure_table(&transaction)?;
        let Some(record) = read_record(&transaction, project_id, idempotency_key)? else {
            transaction.commit()?;
            return Ok(None);
        };
        validate_record(&record)?;
        let _bridge = bridge_record(&transaction, &record)?;
        let artifact = read_object(&transaction, &record.high_artifact_object_sha256)?;
        let artifact_bytes = validate_object(
            &transaction,
            &self.cas,
            &artifact,
            &record.high_artifact_object_sha256,
            AUTHORING_MESH_V2_HIGH_ARTIFACT_GLB_MIME,
            AUTHORING_MESH_V2_HIGH_ARTIFACT_GLB_OBJECT_KIND,
            AUTHORING_MESH_V2_HIGH_ARTIFACT_MAX_GLB_BYTES,
            true,
            "High artifact GLB",
        )?;
        validate_glb(&artifact_bytes, record.high_artifact_size_bytes)?;
        let readback = read_object(&transaction, &record.high_artifact_readback_object_sha256)?;
        let readback_bytes = validate_object(
            &transaction,
            &self.cas,
            &readback,
            &record.high_artifact_readback_object_sha256,
            AUTHORING_MESH_V2_HIGH_ARTIFACT_JSON_MIME,
            AUTHORING_MESH_V2_HIGH_ARTIFACT_READBACK_OBJECT_KIND,
            AUTHORING_MESH_V2_HIGH_ARTIFACT_MAX_JSON_BYTES,
            true,
            "High artifact readback",
        )?;
        validate_readback(&readback_bytes, &record)?;
        let receipt = read_object(&transaction, &record.receipt_object_sha256)?;
        let receipt_bytes = validate_object(
            &transaction,
            &self.cas,
            &receipt,
            &record.receipt_object_sha256,
            AUTHORING_MESH_V2_HIGH_ARTIFACT_JSON_MIME,
            AUTHORING_MESH_V2_HIGH_ARTIFACT_RECEIPT_OBJECT_KIND,
            AUTHORING_MESH_V2_HIGH_ARTIFACT_MAX_JSON_BYTES,
            true,
            "High artifact receipt",
        )?;
        let receipt_value = validate_receipt(&receipt_bytes, &record)?;
        if object_canonical_hash(&receipt_value, "High artifact receipt")? != record.receipt_sha256
        {
            return Err(contract(
                "AUTHORING_MESH_V2_HIGH_ARTIFACT_RECEIPT_HASH_MISMATCH",
                "aggregate receipt semantic hash differs during readback",
            ));
        }
        let mut roots = object_hashes(&record);
        roots.extend(bridge_roots(&_bridge));
        roots.sort();
        roots.dedup();
        super::mark_reachable_in_transaction(&transaction, &roots)?;
        transaction.commit()?;
        Ok(Some(record))
    }

    pub fn get_authoring_mesh_v2_high_artifact_by_id(
        &self,
        project_id: &str,
        artifact_id: &str,
    ) -> Result<Option<AuthoringMeshV2HighArtifactStoreRecord>, StoreError> {
        if !is_opaque_id(project_id) || !is_opaque_id(artifact_id) {
            return Err(StoreError::InvalidData(
                "High artifact lookup identity is invalid".to_owned(),
            ));
        }
        let key = {
            let connection = self.lock_connection()?;
            connection
                .query_row(
                    "SELECT idempotency_key FROM authoring_mesh_v2_high_artifact_records WHERE project_id = ?1 AND artifact_id = ?2",
                    params![project_id, artifact_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
        };
        let Some(key) = key else { return Ok(None) };
        self.get_authoring_mesh_v2_high_artifact(project_id, &key)
    }

    /// Strict lookup used by a future Low Runtime service.  It refuses to
    /// turn a semantic/object/hash drift into a different High input.
    pub fn get_authoring_mesh_v2_high_artifact_exact(
        &self,
        project_id: &str,
        artifact_id: &str,
        artifact_sha256: &str,
        artifact_object_sha256: &str,
        artifact_readback_sha256: &str,
        artifact_readback_object_sha256: &str,
        receipt_sha256: &str,
        receipt_object_sha256: &str,
        bridge_id: &str,
        bridge_sha256: &str,
        bridge_object_sha256: &str,
        revision_id: &str,
        revision_sha256: &str,
        revision_object_sha256: &str,
        high_result_sha256: &str,
        high_result_object_sha256: &str,
        high_readback_sha256: &str,
        high_readback_object_sha256: &str,
        high_worker_algorithm_sha256: &str,
        high_worker_build_cohort_sha256: &str,
    ) -> Result<Option<AuthoringMeshV2HighArtifactStoreRecord>, StoreError> {
        let Some(record) =
            self.get_authoring_mesh_v2_high_artifact_by_id(project_id, artifact_id)?
        else {
            return Ok(None);
        };
        let equal = record.high_artifact_sha256 == artifact_sha256
            && record.high_artifact_object_sha256 == artifact_object_sha256
            && record.high_artifact_readback_sha256 == artifact_readback_sha256
            && record.high_artifact_readback_object_sha256 == artifact_readback_object_sha256
            && record.receipt_sha256 == receipt_sha256
            && record.receipt_object_sha256 == receipt_object_sha256
            && record.bridge_id == bridge_id
            && record.bridge_sha256 == bridge_sha256
            && record.bridge_object_sha256 == bridge_object_sha256
            && record.revision_id == revision_id
            && record.revision_sha256 == revision_sha256
            && record.revision_object_sha256 == revision_object_sha256
            && record.high_result_sha256 == high_result_sha256
            && record.high_result_object_sha256 == high_result_object_sha256
            && record.high_readback_sha256 == high_readback_sha256
            && record.high_readback_object_sha256 == high_readback_object_sha256
            && record.high_worker_algorithm_sha256 == high_worker_algorithm_sha256
            && record.high_worker_build_cohort_sha256 == high_worker_build_cohort_sha256;
        if !equal {
            return Err(contract(
                "AUTHORING_MESH_V2_HIGH_ARTIFACT_EXACT_LOOKUP_MISMATCH",
                "High artifact exact lookup fields differ from the durable row",
            ));
        }
        Ok(Some(record))
    }

    pub fn read_authoring_mesh_v2_high_artifact_json(
        &self,
        project_id: &str,
        idempotency_key: &str,
    ) -> Result<Option<Value>, StoreError> {
        let Some(record) = self.get_authoring_mesh_v2_high_artifact(project_id, idempotency_key)?
        else {
            return Ok(None);
        };
        record_value(&record).map(Some)
    }

    /// Canonical source accessor reserved for Low.  A Low producer must use
    /// this identity rather than selecting an arbitrary GLB from CAS.
    pub fn get_authoring_mesh_v2_high_artifact_for_low(
        &self,
        project_id: &str,
        artifact_id: &str,
    ) -> Result<Option<AuthoringMeshV2HighArtifactStoreRecord>, StoreError> {
        self.get_authoring_mesh_v2_high_artifact_by_id(project_id, artifact_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgecad_contracts::ProjectRecord;
    use serde_json::json;

    const PROJECT: &str = "high-artifact-test-project";
    const NOW: &str = "2026-08-31T00:00:00Z";

    fn h(seed: u8) -> String {
        sha256_hex(&[seed])
    }

    fn project(store: &Store) {
        store
            .insert_project(&ProjectRecord {
                schema_version: "Project@1".to_owned(),
                project_id: PROJECT.to_owned(),
                name: "High artifact test".to_owned(),
                policy: json!({"scope":"test"}),
                created_at: NOW.to_owned(),
                updated_at: NOW.to_owned(),
                active_snapshot_revision: 0,
                head_snapshot_id: None,
                canonical_sha256: h(1),
            })
            .expect("project");
    }

    fn glb() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"glTF");
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&12u32.to_le_bytes());
        bytes
    }

    fn object(store: &Store, bytes: &[u8], kind: &str, mime: &str) -> CasObjectRecord {
        store
            .put_object(bytes, None, mime, kind, NOW)
            .expect("CAS object")
            .record
    }

    fn base_record(
        store: &Store,
    ) -> (
        AuthoringMeshV2HighArtifactStoreRecord,
        AuthoringMeshV2HighArtifactCasBundle,
    ) {
        project(store);
        let artifact_bytes = glb();
        let artifact = object(
            store,
            &artifact_bytes,
            AUTHORING_MESH_V2_HIGH_ARTIFACT_GLB_OBJECT_KIND,
            AUTHORING_MESH_V2_HIGH_ARTIFACT_GLB_MIME,
        );
        let mut record = AuthoringMeshV2HighArtifactStoreRecord {
            schema_version: AUTHORING_MESH_V2_HIGH_ARTIFACT_RECORD_SCHEMA_VERSION.to_owned(),
            project_id: PROJECT.to_owned(),
            artifact_id: "high-artifact".to_owned(),
            bridge_id: "high-bridge".to_owned(),
            bridge_sha256: h(2),
            bridge_object_sha256: h(3),
            source_binding_id: "source-binding".to_owned(),
            source_binding_sha256: h(4),
            source_binding_object_sha256: h(5),
            mesh_id: "mesh".to_owned(),
            lineage_id: "lineage".to_owned(),
            revision_id: "revision".to_owned(),
            revision_index: 0,
            revision_sha256: h(6),
            revision_object_sha256: h(7),
            materialized_candidate_id: "candidate".to_owned(),
            materialized_candidate_state_sha256: h(8),
            materialized_program_sha256: h(9),
            materialized_program_object_sha256: h(10),
            representation_plan_sha256: h(11),
            source_node_id: "source-node".to_owned(),
            part_id: "blade".to_owned(),
            material_zone_id: "blade-zone".to_owned(),
            solid: true,
            high_execution_request_sha256: h(12),
            high_evaluation_sha256: h(13),
            high_result_sha256: h(14),
            high_result_object_sha256: h(15),
            high_readback_sha256: h(16),
            high_readback_object_sha256: h(17),
            high_worker_algorithm_sha256: h(18),
            high_worker_build_cohort_sha256: h(19),
            high_replay_count: 2,
            high_replay_byte_exact: true,
            high_non_destructive: true,
            high_source_vertex_count: 3,
            high_source_triangle_count: 1,
            high_evaluated_part_count: 1,
            high_evaluated_triangle_count: 4,
            high_artifact_sha256: artifact.sha256.clone(),
            high_artifact_object_sha256: artifact.sha256.clone(),
            high_artifact_size_bytes: artifact.size_bytes,
            high_artifact_readback_sha256: h(20),
            high_artifact_readback_object_sha256: h(21),
            receipt_sha256: h(22),
            receipt_object_sha256: h(23),
            materialized_artifact_hash_policy: AUTHORING_MESH_V2_HIGH_ARTIFACT_HASH_POLICY
                .to_owned(),
            materialization_status: "prepared".to_owned(),
            structural_status: "PASS_SOURCE_STRUCTURAL".to_owned(),
            visual_status: "NOT_RUN".to_owned(),
            human_status: "NOT_RUN".to_owned(),
            engine_status: "NOT_RUN".to_owned(),
            high_mesh_created: true,
            high_stage_unlocked: false,
            production_stage_advanced: false,
            candidate_confirmed: false,
            version_created: false,
            export_performed: false,
            runtime_write_performed: true,
            persistent_user_data_touched: true,
            writer_policy: AUTHORING_MESH_V2_HIGH_ARTIFACT_WRITER_POLICY.to_owned(),
            canonicalization_policy: AUTHORING_MESH_V2_HIGH_ARTIFACT_CANONICALIZATION_POLICY
                .to_owned(),
            canonical_sha256: String::new(),
            request_input_sha256: h(24),
            idempotency_key: "high-artifact-key".to_owned(),
            created_at: NOW.to_owned(),
        };
        let mut value = record_value(&record).expect("record");
        value["canonical_sha256"] = Value::String(String::new());
        record.canonical_sha256 = canonical_json_hash(&value);
        // The unit tests focus on pre-transaction policy/replay invariants;
        // bridge and JSON object fixtures are supplied by the Runtime seam.
        (
            record,
            AuthoringMeshV2HighArtifactCasBundle {
                artifact,
                readback: CasObjectRecord {
                    schema_version: "CasObject@1".to_owned(),
                    sha256: h(21),
                    size_bytes: 1,
                    mime: AUTHORING_MESH_V2_HIGH_ARTIFACT_JSON_MIME.to_owned(),
                    kind: AUTHORING_MESH_V2_HIGH_ARTIFACT_READBACK_OBJECT_KIND.to_owned(),
                    reachability: "temporary".to_owned(),
                    created_at: NOW.to_owned(),
                },
                receipt: CasObjectRecord {
                    schema_version: "CasObject@1".to_owned(),
                    sha256: h(23),
                    size_bytes: 1,
                    mime: AUTHORING_MESH_V2_HIGH_ARTIFACT_JSON_MIME.to_owned(),
                    kind: AUTHORING_MESH_V2_HIGH_ARTIFACT_RECEIPT_OBJECT_KIND.to_owned(),
                    reachability: "temporary".to_owned(),
                    created_at: NOW.to_owned(),
                },
            },
        )
    }

    #[test]
    fn record_validation_requires_exact_glb_hash_policy_and_replay() {
        let store = Store::memory().expect("store");
        let (mut record, _) = base_record(&store);
        assert!(validate_record(&record).is_ok());
        record.high_replay_byte_exact = false;
        assert!(format!("{:?}", validate_record(&record)).contains("RECORD_INVALID"));
    }

    #[test]
    fn glb_header_is_bounded_and_exact() {
        assert!(validate_glb(&glb(), 12).is_ok());
        let mut bad = glb();
        bad[8] = 13;
        assert!(validate_glb(&bad, 12).is_err());
    }

    fn json_object(
        store: &Store,
        mut value: Value,
        kind: &str,
        semantic_field: Option<&str>,
    ) -> (CasObjectRecord, String) {
        value["canonical_sha256"] = Value::String(String::new());
        if let Some(field) = semantic_field {
            value[field] = Value::String(String::new());
        }
        let semantic = canonical_json_hash(&value);
        value["canonical_sha256"] = Value::String(semantic.clone());
        if let Some(field) = semantic_field {
            value[field] = Value::String(semantic.clone());
        }
        let bytes = canonical_json_bytes(&value).expect("canonical artifact JSON");
        let object = store
            .put_object(
                &bytes,
                None,
                AUTHORING_MESH_V2_HIGH_ARTIFACT_JSON_MIME,
                kind,
                NOW,
            )
            .expect("artifact JSON object");
        (object.record, semantic)
    }

    fn shared_proof(record: &AuthoringMeshV2HighArtifactStoreRecord, schema: &str) -> Value {
        json!({
            "schema_version": schema,
            "project_id": record.project_id,
            "artifact_id": record.artifact_id,
            "bridge_id": record.bridge_id,
            "bridge_sha256": record.bridge_sha256,
            "bridge_object_sha256": record.bridge_object_sha256,
            "revision_id": record.revision_id,
            "revision_index": record.revision_index,
            "revision_sha256": record.revision_sha256,
            "revision_object_sha256": record.revision_object_sha256,
            "source_binding_id": record.source_binding_id,
            "source_binding_sha256": record.source_binding_sha256,
            "source_binding_object_sha256": record.source_binding_object_sha256,
            "high_result_sha256": record.high_result_sha256,
            "high_result_object_sha256": record.high_result_object_sha256,
            "high_readback_sha256": record.high_readback_sha256,
            "high_readback_object_sha256": record.high_readback_object_sha256,
            "high_worker_algorithm_sha256": record.high_worker_algorithm_sha256,
            "high_worker_build_cohort_sha256": record.high_worker_build_cohort_sha256,
            "structural_status": record.structural_status,
            "visual_status": record.visual_status,
            "human_status": record.human_status,
            "engine_status": record.engine_status,
            "canonical_sha256": ""
        })
    }

    struct ArtifactFixture {
        store: Store,
        commit: AuthoringMeshV2HighArtifactCommit,
        bridge: AuthoringMeshV2HighBridgeStoreRecord,
    }

    fn setup_artifact_fixture() -> ArtifactFixture {
        let bridge_fixture = crate::authoring_mesh_v2_high_bridge::test_setup_fixture();
        let store = bridge_fixture.store;
        store
            .record_authoring_mesh_v2_high_bridge_with_replay(&bridge_fixture.commit)
            .expect("upstream High bridge");
        let bridge = bridge_fixture.commit.record;
        let artifact_bytes = glb();
        let artifact = store
            .put_object(
                &artifact_bytes,
                None,
                AUTHORING_MESH_V2_HIGH_ARTIFACT_GLB_MIME,
                AUTHORING_MESH_V2_HIGH_ARTIFACT_GLB_OBJECT_KIND,
                NOW,
            )
            .expect("direct High GLB");
        let mut record = AuthoringMeshV2HighArtifactStoreRecord {
            schema_version: AUTHORING_MESH_V2_HIGH_ARTIFACT_RECORD_SCHEMA_VERSION.to_owned(),
            project_id: bridge.project_id.clone(),
            artifact_id: "direct-high-artifact".to_owned(),
            bridge_id: bridge.bridge_id.clone(),
            bridge_sha256: bridge.bridge_sha256.clone(),
            bridge_object_sha256: bridge.bridge_object_sha256.clone(),
            source_binding_id: bridge.source_binding_id.clone(),
            source_binding_sha256: bridge.source_binding_sha256.clone(),
            source_binding_object_sha256: bridge.source_binding_object_sha256.clone(),
            mesh_id: bridge.mesh_id.clone(),
            lineage_id: bridge.lineage_id.clone(),
            revision_id: bridge.revision_id.clone(),
            revision_index: bridge.revision_index,
            revision_sha256: bridge.revision_sha256.clone(),
            revision_object_sha256: bridge.revision_object_sha256.clone(),
            materialized_candidate_id: bridge.materialized_candidate_id.clone(),
            materialized_candidate_state_sha256: bridge.materialized_candidate_state_sha256.clone(),
            materialized_program_sha256: bridge.materialized_program_sha256.clone(),
            materialized_program_object_sha256: bridge.materialized_program_object_sha256.clone(),
            representation_plan_sha256: bridge.representation_plan_sha256.clone(),
            source_node_id: bridge.source_node_id.clone(),
            part_id: bridge.part_id.clone(),
            material_zone_id: bridge.material_zone_id.clone(),
            solid: bridge.solid,
            high_execution_request_sha256: bridge.high_execution_request_sha256.clone(),
            high_evaluation_sha256: bridge.high_evaluation_sha256.clone(),
            high_result_sha256: bridge.high_result_sha256.clone(),
            high_result_object_sha256: bridge.high_result_object_sha256.clone(),
            high_readback_sha256: bridge.high_readback_sha256.clone(),
            high_readback_object_sha256: bridge.high_readback_object_sha256.clone(),
            high_worker_algorithm_sha256: bridge.high_worker_algorithm_sha256.clone(),
            high_worker_build_cohort_sha256: bridge.high_worker_build_cohort_sha256.clone(),
            high_replay_count: bridge.high_replay_count,
            high_replay_byte_exact: bridge.high_replay_byte_exact,
            high_non_destructive: bridge.high_non_destructive,
            high_source_vertex_count: bridge.high_source_vertex_count,
            high_source_triangle_count: bridge.high_source_triangle_count,
            high_evaluated_part_count: bridge.high_evaluated_part_count,
            high_evaluated_triangle_count: bridge.high_evaluated_triangle_count,
            high_artifact_sha256: artifact.record.sha256.clone(),
            high_artifact_object_sha256: artifact.record.sha256.clone(),
            high_artifact_size_bytes: artifact.record.size_bytes,
            high_artifact_readback_sha256: String::new(),
            high_artifact_readback_object_sha256: String::new(),
            receipt_sha256: String::new(),
            receipt_object_sha256: String::new(),
            materialized_artifact_hash_policy: AUTHORING_MESH_V2_HIGH_ARTIFACT_HASH_POLICY
                .to_owned(),
            materialization_status: "prepared".to_owned(),
            structural_status: "PASS_SOURCE_STRUCTURAL".to_owned(),
            visual_status: "NOT_RUN".to_owned(),
            human_status: "NOT_RUN".to_owned(),
            engine_status: "NOT_RUN".to_owned(),
            high_mesh_created: true,
            high_stage_unlocked: false,
            production_stage_advanced: false,
            candidate_confirmed: false,
            version_created: false,
            export_performed: false,
            runtime_write_performed: true,
            persistent_user_data_touched: true,
            writer_policy: AUTHORING_MESH_V2_HIGH_ARTIFACT_WRITER_POLICY.to_owned(),
            canonicalization_policy: AUTHORING_MESH_V2_HIGH_ARTIFACT_CANONICALIZATION_POLICY
                .to_owned(),
            canonical_sha256: String::new(),
            request_input_sha256: h(24),
            idempotency_key: "direct-high-artifact-key".to_owned(),
            created_at: NOW.to_owned(),
        };
        let readback_value = {
            let mut value = shared_proof(&record, "AuthoringMeshV2HighArtifactStoreReadback@1");
            value["high_artifact_sha256"] = Value::String(record.high_artifact_sha256.clone());
            value["high_artifact_object_sha256"] =
                Value::String(record.high_artifact_object_sha256.clone());
            value["high_artifact_readback_sha256"] = Value::String(String::new());
            value["high_artifact_readback_object_sha256"] = Value::String(String::new());
            value["high_artifact_size_bytes"] = Value::from(record.high_artifact_size_bytes);
            value["replay_count"] = Value::from(2u64);
            value["replay_byte_exact"] = Value::Bool(true);
            value["non_destructive"] = Value::Bool(true);
            value
        };
        let (readback, readback_sha256) = json_object(
            &store,
            readback_value,
            AUTHORING_MESH_V2_HIGH_ARTIFACT_READBACK_OBJECT_KIND,
            Some("high_artifact_readback_sha256"),
        );
        record.high_artifact_readback_sha256 = readback_sha256;
        record.high_artifact_readback_object_sha256 = readback.sha256.clone();
        let receipt_value = {
            let mut value = shared_proof(&record, "AuthoringMeshV2HighArtifactReceipt@1");
            value["high_artifact_readback_sha256"] =
                Value::String(record.high_artifact_readback_sha256.clone());
            value["high_artifact_readback_object_sha256"] =
                Value::String(record.high_artifact_readback_object_sha256.clone());
            value["receipt_status"] = Value::String("prepared".to_owned());
            value["materialization_status"] = Value::String("prepared".to_owned());
            value
        };
        let (receipt, receipt_sha256) = json_object(
            &store,
            receipt_value,
            AUTHORING_MESH_V2_HIGH_ARTIFACT_RECEIPT_OBJECT_KIND,
            None,
        );
        record.receipt_sha256 = receipt_sha256;
        record.receipt_object_sha256 = receipt.sha256.clone();
        record.canonical_sha256 = record_canonical_sha256(&record).expect("artifact row hash");
        ArtifactFixture {
            store,
            commit: AuthoringMeshV2HighArtifactCommit {
                record,
                cas: AuthoringMeshV2HighArtifactCasBundle {
                    artifact: artifact.record,
                    readback,
                    receipt,
                },
            },
            bridge,
        }
    }

    #[test]
    fn direct_high_artifact_commit_replays_and_marks_all_roots_reachable() {
        let fixture = setup_artifact_fixture();
        let (stored, replayed) = fixture
            .store
            .record_authoring_mesh_v2_high_artifact_with_replay(&fixture.commit)
            .expect("direct High artifact commit");
        assert!(!replayed);
        assert_eq!(stored, fixture.commit.record);
        let (replayed_record, replayed) = fixture
            .store
            .record_authoring_mesh_v2_high_artifact_with_replay(&fixture.commit)
            .expect("direct High artifact replay");
        assert!(replayed);
        assert_eq!(replayed_record, stored);
        let loaded = fixture
            .store
            .get_authoring_mesh_v2_high_artifact_by_id(&stored.project_id, &stored.artifact_id)
            .expect("direct High artifact get")
            .expect("direct High artifact row");
        assert_eq!(loaded, stored);
        let mut roots = object_hashes(&stored);
        roots.extend(bridge_roots(&fixture.bridge));
        roots.sort();
        roots.dedup();
        let connection = fixture.store.connection.lock().expect("connection");
        let temporary: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM objects WHERE sha256 IN (SELECT value FROM json_each(?1)) AND reachability != 'reachable'",
                params![serde_json::to_string(&roots).unwrap()],
                |row| row.get(0),
            )
            .expect("root reachability");
        assert_eq!(temporary, 0);
    }

    #[test]
    fn direct_high_artifact_rejects_cas_tamper_before_row_insert() {
        let fixture = setup_artifact_fixture();
        let receipt_path = fixture
            .store
            .cas()
            .root()
            .join("objects")
            .join(&fixture.commit.cas.receipt.sha256[..2])
            .join(&fixture.commit.cas.receipt.sha256);
        std::fs::write(&receipt_path, b"tampered").expect("tamper receipt bytes");
        let error = fixture
            .store
            .record_authoring_mesh_v2_high_artifact_with_replay(&fixture.commit)
            .expect_err("tampered receipt must fail closed");
        assert!(
            format!("{error:?}").contains("HashMismatch"),
            "unexpected tamper error: {error:?}"
        );
        let connection = fixture.store.connection.lock().expect("connection");
        let rows: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM authoring_mesh_v2_high_artifact_records",
                [],
                |row| row.get(0),
            )
            .expect("artifact row count");
        assert_eq!(rows, 0);
    }

    #[test]
    fn direct_high_artifact_rejects_same_key_content_conflict() {
        let fixture = setup_artifact_fixture();
        fixture
            .store
            .record_authoring_mesh_v2_high_artifact_with_replay(&fixture.commit)
            .expect("direct High artifact commit");
        let mut conflict = fixture.commit.clone();
        conflict.record.request_input_sha256 = h(25);
        conflict.record.canonical_sha256 = record_canonical_sha256(&conflict.record).unwrap();
        let error = fixture
            .store
            .record_authoring_mesh_v2_high_artifact_with_replay(&conflict)
            .expect_err("same key conflict must fail closed");
        assert!(format!("{error:?}").contains("IDEMPOTENCY_CONFLICT"));
    }

    #[test]
    fn direct_high_artifact_reopens_and_exact_get_rejects_hash_drift() {
        let suffix = h(26);
        let root = std::env::temp_dir().join(format!("weaponry-high-artifact-{suffix}"));
        let database = root.join("runtime.sqlite");
        let cas_root = root.join("cas");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("backup root");
        let fixture = setup_artifact_fixture();
        let (stored, _) = fixture
            .store
            .record_authoring_mesh_v2_high_artifact_with_replay(&fixture.commit)
            .expect("direct High artifact commit");
        {
            let connection = fixture.store.connection.lock().expect("connection");
            connection
                .execute(
                    "VACUUM INTO ?1",
                    params![database.to_string_lossy().to_string()],
                )
                .expect("SQLite backup");
        }
        fixture
            .store
            .cas()
            .copy_objects_to(&cas_root)
            .expect("CAS backup");
        drop(fixture);
        let reopened = Store::open_with_cas(&database, &cas_root).expect("reopen Store");
        let loaded = reopened
            .get_authoring_mesh_v2_high_artifact_by_id(&stored.project_id, &stored.artifact_id)
            .expect("reopened High artifact get")
            .expect("reopened High artifact row");
        assert_eq!(loaded, stored);
        let exact = reopened
            .get_authoring_mesh_v2_high_artifact_exact(
                &stored.project_id,
                &stored.artifact_id,
                &stored.high_artifact_sha256,
                &stored.high_artifact_object_sha256,
                &stored.high_artifact_readback_sha256,
                &stored.high_artifact_readback_object_sha256,
                &stored.receipt_sha256,
                &stored.receipt_object_sha256,
                &stored.bridge_id,
                &stored.bridge_sha256,
                &stored.bridge_object_sha256,
                &stored.revision_id,
                &stored.revision_sha256,
                &stored.revision_object_sha256,
                &stored.high_result_sha256,
                &stored.high_result_object_sha256,
                &stored.high_readback_sha256,
                &stored.high_readback_object_sha256,
                &stored.high_worker_algorithm_sha256,
                &stored.high_worker_build_cohort_sha256,
            )
            .expect("exact High artifact get")
            .expect("exact High artifact row");
        assert_eq!(exact, stored);
        let error = reopened
            .get_authoring_mesh_v2_high_artifact_exact(
                &stored.project_id,
                &stored.artifact_id,
                &h(27),
                &stored.high_artifact_object_sha256,
                &stored.high_artifact_readback_sha256,
                &stored.high_artifact_readback_object_sha256,
                &stored.receipt_sha256,
                &stored.receipt_object_sha256,
                &stored.bridge_id,
                &stored.bridge_sha256,
                &stored.bridge_object_sha256,
                &stored.revision_id,
                &stored.revision_sha256,
                &stored.revision_object_sha256,
                &stored.high_result_sha256,
                &stored.high_result_object_sha256,
                &stored.high_readback_sha256,
                &stored.high_readback_object_sha256,
                &stored.high_worker_algorithm_sha256,
                &stored.high_worker_build_cohort_sha256,
            )
            .expect_err("semantic hash drift must fail closed");
        assert!(format!("{error:?}").contains("EXACT_LOOKUP_MISMATCH"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
