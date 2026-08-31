//! Store/CAS durability for the knife source binding.
//!
//! This is a narrow, additive repository seam.  Runtime has already parsed
//! the closed source-binding contract and staged its canonical JSON in CAS;
//! this module only verifies the exact Brief/Reference/Intent/Quality and
//! candidate/AuthoringMesh lineage before installing one immutable SQLite
//! row.  It never accepts a path, URL, script, secret or inline topology.

use super::{
    canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256,
    mark_reachable_in_transaction, CasObjectRecord, CasStore, Store, StoreError,
    AUTHORING_MESH_V2_REVISION_OBJECT_KIND,
};
use forgecad_contracts::AuthoringMeshV2SourceBinding;
use forgecad_core::sha256_hex;
use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const KNIFE_SOURCE_BINDING_RECORD_SCHEMA_VERSION: &str = "KnifeSourceBindingStoreRecord@1";
pub const KNIFE_SOURCE_BINDING_SCHEMA_VERSION: &str = "KnifeSourceBinding@1";
pub const KNIFE_SOURCE_BINDING_STATUS: &str = "runtime-owned-store-knife-source-binding@1";
pub const KNIFE_SOURCE_BINDING_OBJECT_KIND: &str = "knife-source-binding";
pub const KNIFE_SOURCE_BINDING_JSON_MIME: &str = "application/json";
pub const KNIFE_SOURCE_BINDING_MAX_JSON_BYTES: u64 = 1024 * 1024;
pub const KNIFE_SOURCE_BINDING_BINDING_STATUS: &str = "runtime-bound";
pub const KNIFE_SOURCE_BINDING_AUTHORING_ELIGIBILITY: &str = "ELIGIBLE";
pub const KNIFE_SOURCE_BINDING_POLICY: &str =
    "intent-brief-reference-quality-to-authoring-mesh-exact@1";
pub const KNIFE_SOURCE_BINDING_CANONICALIZATION_POLICY: &str =
    "canonical-json-sha256-excluding-canonical-sha256@1";
pub const KNIFE_SOURCE_BINDING_DOWNSTREAM_POLICY: &str = "must-inherit-source-binding-sha256@1";

/// Store-local index for one immutable KnifeSourceBinding CAS object.
///
/// Semantic hashes and object hashes are deliberately separate.  The source
/// candidate and AuthoringMesh fields are all repeated here so a restart-safe
/// exact lookup cannot silently resolve a different source revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnifeSourceBindingStoreRecord {
    pub schema_version: String,
    pub source_binding_id: String,
    pub project_id: String,
    pub binding_status: String,
    pub authoring_eligibility: String,
    pub intent_bundle_id: String,
    pub intent_bundle_sha256: String,
    pub intent_bundle_object_sha256: String,
    pub brief_id: String,
    pub brief_sha256: String,
    pub brief_object_sha256: String,
    pub reference_id: String,
    pub reference_object_sha256: String,
    pub reference_evidence_sha256: String,
    pub quality_contract_id: String,
    pub quality_contract_sha256: String,
    pub quality_contract_object_sha256: String,
    pub source_candidate_id: String,
    pub source_candidate_state_sha256: String,
    pub authoring_mesh_id: String,
    pub authoring_mesh_lineage_id: String,
    pub authoring_mesh_revision_id: String,
    pub authoring_mesh_revision_index: u64,
    pub authoring_mesh_revision_sha256: String,
    pub authoring_mesh_revision_object_sha256: String,
    pub authoring_mesh_identity_sha256: String,
    pub downstream_binding_requirements: KnifeSourceBindingDownstreamBindingRequirements,
    pub high_mesh_created: bool,
    pub high_stage_unlocked: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub quality_status: String,
    pub visual_status: String,
    pub human_status: String,
    pub engine_status: String,
    pub binding_policy: String,
    pub canonicalization_policy: String,
    pub source_binding_sha256: String,
    pub source_binding_object_sha256: String,
    pub idempotency_key: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnifeSourceBindingDownstreamBindingRequirements {
    pub curve_modifier_graph: String,
    pub curve_evaluated_mesh: String,
    pub high: String,
    pub render: String,
}

/// The single canonical JSON object staged by Runtime before the Store
/// transaction.  Store does not put, rewrite or delete this object.
#[derive(Debug, Clone)]
pub struct KnifeSourceBindingCasBundle {
    pub source_binding: CasObjectRecord,
}

#[derive(Debug, Clone)]
pub struct KnifeSourceBindingCommit {
    pub record: KnifeSourceBindingStoreRecord,
    pub cas: KnifeSourceBindingCasBundle,
}

fn contract(code: &str, message: impl Into<String>) -> StoreError {
    StoreError::Contract {
        code: code.to_owned(),
        message: message.into(),
    }
}

fn record_value(record: &KnifeSourceBindingStoreRecord) -> Result<Value, StoreError> {
    serde_json::to_value(record).map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn record_bytes(record: &KnifeSourceBindingStoreRecord) -> Result<Vec<u8>, StoreError> {
    canonical_json_bytes(&record_value(record)?)
        .map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn validate_record(record: &KnifeSourceBindingStoreRecord) -> Result<(), StoreError> {
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
        || record.binding_status != KNIFE_SOURCE_BINDING_BINDING_STATUS
        || record.authoring_eligibility != KNIFE_SOURCE_BINDING_AUTHORING_ELIGIBILITY
        || ids.iter().any(|value| !is_opaque_id(value))
        || hashes.iter().any(|value| !is_sha256(value))
        || record.authoring_mesh_revision_index > 1_000_000
        || record.idempotency_key.len() > 128
        || record.created_at.is_empty()
        || record.created_at.len() > 128
        || record.created_at.contains('/')
        || record.created_at.contains('\\')
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
    {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_RECORD_INVALID",
            "source binding identity, hash, revision index or timestamp is malformed",
        ));
    }
    Ok(())
}

fn source_payload_value(record: &KnifeSourceBindingStoreRecord) -> Value {
    let mut object = record_value(record)
        .expect("KnifeSourceBindingStoreRecord is serializable")
        .as_object()
        .cloned()
        .expect("KnifeSourceBindingStoreRecord serializes to an object");
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
    Value::Object(object)
}

fn validate_source_payload(
    bytes: &[u8],
    record: &KnifeSourceBindingStoreRecord,
) -> Result<(), StoreError> {
    if bytes.is_empty() || bytes.len() as u64 > KNIFE_SOURCE_BINDING_MAX_JSON_BYTES {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_PAYLOAD_BYTES_INVALID",
            "source binding CAS JSON is empty or exceeds its bound",
        ));
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        contract(
            "KNIFE_SOURCE_BINDING_PAYLOAD_JSON_INVALID",
            format!("source binding CAS JSON is invalid: {error}"),
        )
    })?;
    let canonical =
        canonical_json_bytes(&value).map_err(|error| StoreError::InvalidData(error.to_string()))?;
    if canonical != bytes || value != source_payload_value(record) {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_PAYLOAD_BINDING_MISMATCH",
            "source binding CAS JSON is not the exact canonical binding",
        ));
    }
    let mut preimage = value.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != record.source_binding_sha256 {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_PAYLOAD_CANONICAL_MISMATCH",
            "source binding CAS JSON canonical hash differs from its Store binding",
        ));
    }
    Ok(())
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

fn valid_cas_created_at(created_at: &str) -> bool {
    !created_at.is_empty()
        && created_at.len() <= 128
        && !created_at.contains('/')
        && !created_at.contains('\\')
}

fn validate_source_cas_object(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    supplied: &CasObjectRecord,
    expected_sha256: &str,
    require_reachable: bool,
    role: &str,
) -> Result<Vec<u8>, StoreError> {
    if supplied.schema_version != "CasObject@1"
        || supplied.sha256 != expected_sha256
        || !is_sha256(expected_sha256)
        || supplied.mime != KNIFE_SOURCE_BINDING_JSON_MIME
        || supplied.kind != KNIFE_SOURCE_BINDING_OBJECT_KIND
        || supplied.size_bytes == 0
        || supplied.size_bytes > KNIFE_SOURCE_BINDING_MAX_JSON_BYTES
        || !matches!(supplied.reachability.as_str(), "temporary" | "reachable")
        || (require_reachable && supplied.reachability != "reachable")
        || !valid_cas_created_at(&supplied.created_at)
    {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_CAS_METADATA_INVALID",
            format!("{role} CAS metadata is outside the bounded allowlist"),
        ));
    }
    let registered =
        read_object_record(transaction, expected_sha256).map_err(|error| match error {
            StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => contract(
                "KNIFE_SOURCE_BINDING_CAS_MISSING",
                format!("{role} CAS object is not registered"),
            ),
            other => other,
        })?;
    let reachability_matches = supplied.reachability == registered.reachability
        || (supplied.reachability == "temporary" && registered.reachability == "reachable");
    if registered.size_bytes != supplied.size_bytes
        || registered.mime != supplied.mime
        || registered.kind != supplied.kind
        || !valid_cas_created_at(&registered.created_at)
        || !reachability_matches
        || (require_reachable && registered.reachability != "reachable")
    {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_CAS_METADATA_MISMATCH",
            format!("{role} CAS metadata differs from SQLite registration"),
        ));
    }
    let bytes = cas
        .read_verified_bounded(expected_sha256, KNIFE_SOURCE_BINDING_MAX_JSON_BYTES)
        .map_err(StoreError::from)?;
    if bytes.len() as u64 != supplied.size_bytes || sha256_hex(&bytes) != expected_sha256 {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_CAS_HASH_MISMATCH",
            format!("{role} CAS bytes do not match their content hash"),
        ));
    }
    Ok(bytes)
}

fn validate_json_object(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    object_sha256: &str,
    expected_kind: &str,
    expected_schema: &str,
    expected_semantic_sha256: Option<&str>,
    require_reachable: bool,
    role: &str,
) -> Result<Value, StoreError> {
    let object = read_object_record(transaction, object_sha256).map_err(|error| match error {
        StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => contract(
            "KNIFE_SOURCE_BINDING_LINEAGE_CAS_MISSING",
            format!("{role} CAS object is not registered"),
        ),
        other => other,
    })?;
    if object.schema_version != "CasObject@1"
        || object.sha256 != object_sha256
        || !is_sha256(object_sha256)
        || object.mime != "application/json"
        || object.kind != expected_kind
        || object.size_bytes == 0
        || object.size_bytes > 8 * 1024 * 1024
        || !matches!(object.reachability.as_str(), "temporary" | "reachable")
        || (require_reachable && object.reachability != "reachable")
    {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_LINEAGE_CAS_METADATA_INVALID",
            format!("{role} CAS metadata is invalid"),
        ));
    }
    let bytes = cas
        .read_verified_bounded(object_sha256, 8 * 1024 * 1024)
        .map_err(StoreError::from)?;
    if bytes.len() as u64 != object.size_bytes || sha256_hex(&bytes) != object_sha256 {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_LINEAGE_CAS_HASH_MISMATCH",
            format!("{role} CAS bytes do not match their content hash"),
        ));
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|error| {
        contract(
            "KNIFE_SOURCE_BINDING_LINEAGE_CAS_JSON_INVALID",
            format!("{role} CAS JSON is invalid: {error}"),
        )
    })?;
    if canonical_json_bytes(&value).map_err(|error| StoreError::InvalidData(error.to_string()))?
        != bytes
        || value.get("schema_version").and_then(Value::as_str) != Some(expected_schema)
    {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_LINEAGE_CAS_CANONICAL_MISMATCH",
            format!("{role} CAS JSON is not canonical or has the wrong schema"),
        ));
    }
    if let Some(expected) = expected_semantic_sha256 {
        let semantic = value
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .filter(|hash| is_sha256(hash))
            .ok_or_else(|| {
                contract(
                    "KNIFE_SOURCE_BINDING_LINEAGE_CANONICAL_MISMATCH",
                    format!("{role} canonical_sha256 is missing"),
                )
            })?;
        let mut preimage = value.clone();
        preimage["canonical_sha256"] = Value::String(String::new());
        if semantic != expected || canonical_json_hash(&preimage) != expected {
            return Err(contract(
                "KNIFE_SOURCE_BINDING_LINEAGE_CANONICAL_MISMATCH",
                format!("{role} semantic hash differs from the binding"),
            ));
        }
    }
    Ok(value)
}

fn validate_intent_lineage(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    record: &KnifeSourceBindingStoreRecord,
) -> Result<(), StoreError> {
    let intent: Option<(String, String, String, String, String, String, String, String, String)> =
        transaction
            .query_row(
                "SELECT brief_id, brief_sha256, brief_object_sha256, reference_id, reference_object_sha256, reference_evidence_sha256, quality_contract_sha256, quality_contract_object_sha256, intent_bundle_object_sha256 FROM knife_reference_intent_bundle_records WHERE project_id = ?1 AND intent_bundle_id = ?2 AND intent_bundle_sha256 = ?3",
                params![record.project_id, record.intent_bundle_id, record.intent_bundle_sha256],
                |row| {
                    Ok((
                        row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?,
                        row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?,
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
            "KNIFE_SOURCE_BINDING_INTENT_MISSING",
            "source binding intent bundle is not durably registered",
        ));
    };
    if brief_id != record.brief_id
        || brief_sha256 != record.brief_sha256
        || brief_object_sha256 != record.brief_object_sha256
        || reference_id != record.reference_id
        || reference_object_sha256 != record.reference_object_sha256
        || reference_evidence_sha256 != record.reference_evidence_sha256
        || quality_contract_sha256 != record.quality_contract_sha256
        || quality_contract_object_sha256 != record.quality_contract_object_sha256
        || intent_bundle_object_sha256 != record.intent_bundle_object_sha256
    {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_INTENT_BINDING_MISMATCH",
            "source binding intent bundle fields differ from the durable intent row",
        ));
    }
    validate_json_object(
        transaction,
        cas,
        &record.intent_bundle_object_sha256,
        "knife-reference-intent-bundle",
        "KnifeReferenceIntentBundle@1",
        Some(&record.intent_bundle_sha256),
        true,
        "intent bundle",
    )?;
    let quality = validate_json_object(
        transaction,
        cas,
        &record.quality_contract_object_sha256,
        "knife-quality-contract",
        "KnifeQualityContract@1",
        Some(&record.quality_contract_sha256),
        true,
        "quality contract",
    )?;
    if quality.get("contract_id").and_then(Value::as_str)
        != Some(record.quality_contract_id.as_str())
    {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_QUALITY_BINDING_MISMATCH",
            "source binding quality contract id differs from the intent child",
        ));
    }
    Ok(())
}

fn validate_brief_lineage(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    record: &KnifeSourceBindingStoreRecord,
) -> Result<(), StoreError> {
    let brief: Option<(String, String, String, String)> = transaction
        .query_row(
            "SELECT project_id, brief_object_sha256, brief_canonical_sha256, reference_id FROM weaponry_knife_production_brief_records WHERE project_id = ?1 AND brief_id = ?2 AND brief_canonical_sha256 = ?3",
            params![record.project_id, record.brief_id, record.brief_sha256],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    let Some((project_id, object_sha256, semantic_sha256, reference_id)) = brief else {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_BRIEF_MISSING",
            "source binding Brief is not durably registered",
        ));
    };
    if project_id != record.project_id
        || object_sha256 != record.brief_object_sha256
        || semantic_sha256 != record.brief_sha256
        || reference_id != record.reference_id
    {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_BRIEF_BINDING_MISMATCH",
            "source binding Brief differs from the eligible immutable Brief row",
        ));
    }
    validate_json_object(
        transaction,
        cas,
        &record.brief_object_sha256,
        "weaponry-knife-production-brief",
        "WeaponryKnifeProductionBrief@1",
        Some(&record.brief_sha256),
        true,
        "Brief",
    )?;
    Ok(())
}

fn validate_reference_lineage(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    record: &KnifeSourceBindingStoreRecord,
) -> Result<(), StoreError> {
    let source: Option<(String, String, String, i64, String)> = transaction
        .query_row(
            "SELECT project_id, object_sha256, canonical_sha256, size_bytes, mime FROM reference_evidence WHERE reference_id = ?1",
            params![record.reference_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()?;
    let Some((project_id, object_sha256, evidence_sha256, size_bytes, mime)) = source else {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_REFERENCE_MISSING",
            "source binding ReferenceEvidence is not durably registered",
        ));
    };
    if project_id != record.project_id
        || object_sha256 != record.reference_object_sha256
        || evidence_sha256 != record.reference_evidence_sha256
        || size_bytes <= 0
        || !matches!(mime.as_str(), "image/png" | "image/jpeg")
    {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_REFERENCE_BINDING_MISMATCH",
            "source binding ReferenceEvidence differs from the immutable source row",
        ));
    }
    let object =
        read_object_record(transaction, &record.reference_object_sha256).map_err(|error| {
            match error {
                StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => contract(
                    "KNIFE_SOURCE_BINDING_REFERENCE_CAS_MISSING",
                    "source binding reference CAS object is not registered",
                ),
                other => other,
            }
        })?;
    if object.kind != "reference-image"
        || object.mime != mime
        || object.size_bytes != u64::try_from(size_bytes).unwrap_or(u64::MAX)
        || !matches!(object.reachability.as_str(), "temporary" | "reachable")
    {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_REFERENCE_CAS_METADATA_MISMATCH",
            "source binding reference CAS metadata differs from ReferenceEvidence",
        ));
    }
    cas.verify(&record.reference_object_sha256, object.size_bytes)
        .map_err(StoreError::from)?;
    Ok(())
}

fn validate_lineage_cas_object(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    object_sha256: &str,
    require_reachable: bool,
    role: &str,
) -> Result<CasObjectRecord, StoreError> {
    let object = read_object_record(transaction, object_sha256).map_err(|error| match error {
        StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => contract(
            "KNIFE_SOURCE_BINDING_LINEAGE_CAS_MISSING",
            format!("{role} CAS object is not registered"),
        ),
        other => other,
    })?;
    if !is_sha256(object_sha256)
        || object.size_bytes == 0
        || !matches!(object.reachability.as_str(), "temporary" | "reachable")
        || (require_reachable && object.reachability != "reachable")
    {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_LINEAGE_CAS_METADATA_INVALID",
            format!("{role} CAS metadata is invalid"),
        ));
    }
    cas.verify(object_sha256, object.size_bytes)
        .map_err(StoreError::from)?;
    Ok(object)
}

fn validate_source_lineage(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    record: &KnifeSourceBindingStoreRecord,
) -> Result<Vec<String>, StoreError> {
    let candidate: Option<(String, String)> = transaction
        .query_row(
            "SELECT project_id, canonical_sha256 FROM candidates WHERE candidate_id = ?1",
            params![record.source_candidate_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((project_id, candidate_state_sha256)) = candidate else {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_CANDIDATE_MISSING",
            "source candidate is not durably registered",
        ));
    };
    if project_id != record.project_id
        || candidate_state_sha256 != record.source_candidate_state_sha256
    {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_CANDIDATE_BINDING_MISMATCH",
            "source candidate project or state hash differs",
        ));
    }

    let revision: Option<(String, String, i64, String, String)> = transaction
        .query_row(
            "SELECT lineage_id, revision_id, revision_index, revision_object_sha256, revision_sha256 FROM authoring_mesh_v2_durable_records WHERE project_id = ?1 AND mesh_id = ?2 AND revision_id = ?3",
            params![record.project_id, record.authoring_mesh_id, record.authoring_mesh_revision_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()?;
    let Some((lineage_id, revision_id, revision_index, revision_object_sha256, revision_sha256)) =
        revision
    else {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_REVISION_MISSING",
            "source AuthoringMesh revision is not durably registered",
        ));
    };
    if lineage_id != record.authoring_mesh_lineage_id
        || revision_id != record.authoring_mesh_revision_id
        || revision_index < 0
        || u64::try_from(revision_index).unwrap_or(u64::MAX) != record.authoring_mesh_revision_index
        || revision_object_sha256 != record.authoring_mesh_revision_object_sha256
        || revision_sha256 != record.authoring_mesh_revision_sha256
    {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_REVISION_BINDING_MISMATCH",
            "source AuthoringMesh revision identity differs",
        ));
    }

    let revision_value = validate_json_object(
        transaction,
        cas,
        &record.authoring_mesh_revision_object_sha256,
        AUTHORING_MESH_V2_REVISION_OBJECT_KIND,
        "AuthoringMeshRevision@2",
        Some(&record.authoring_mesh_revision_sha256),
        true,
        "AuthoringMesh revision",
    )?;
    let source_binding_value = revision_value
        .get("source_binding")
        .cloned()
        .ok_or_else(|| {
            contract(
                "KNIFE_SOURCE_BINDING_REVISION_SOURCE_MISSING",
                "AuthoringMesh revision has no embedded candidate source binding",
            )
        })?;
    let source_binding: AuthoringMeshV2SourceBinding =
        serde_json::from_value(source_binding_value.clone()).map_err(|error| {
            contract(
                "KNIFE_SOURCE_BINDING_REVISION_SOURCE_INVALID",
                format!("embedded AuthoringMesh source binding is invalid: {error}"),
            )
        })?;
    let mut source_preimage = source_binding_value.clone();
    source_preimage["canonical_sha256"] = Value::String(String::new());
    if source_binding.schema_version != "AuthoringMeshV2SourceBinding@1"
        || source_binding.project_id != record.project_id
        || source_binding.candidate_id != record.source_candidate_id
        || source_binding.candidate_state_sha256 != record.source_candidate_state_sha256
        || source_binding.artifact_sha256
            != candidate_prepared_object_sha256(transaction, &record.source_candidate_id)?
        || source_binding.canonical_sha256 != canonical_json_hash(&source_preimage)
    {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_REVISION_SOURCE_BINDING_MISMATCH",
            "embedded AuthoringMesh source binding differs from the candidate",
        ));
    }
    let expected_identity_sha256 = canonical_json_hash(&json!({
        "schema_version": "AuthoringMeshSourceIdentity@1",
        "mesh_id": record.authoring_mesh_id,
        "lineage_id": record.authoring_mesh_lineage_id,
        "revision_id": record.authoring_mesh_revision_id,
        "revision_index": record.authoring_mesh_revision_index,
        "revision_sha256": record.authoring_mesh_revision_sha256,
    }));
    if record.authoring_mesh_identity_sha256 != expected_identity_sha256 {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_IDENTITY_BINDING_MISMATCH",
            "source AuthoringMesh identity hash differs from the exact V2 revision",
        ));
    }

    let evidence: Option<(String, String, String, String, String)> = transaction
        .query_row(
            "SELECT project_id, geometry_program_sha256, geometry_program_object_sha256, artifact_object_sha256, artifact_readback_object_sha256 FROM geometry_candidate_evidence WHERE candidate_id = ?1",
            params![record.source_candidate_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()?;
    let Some((
        evidence_project,
        geometry_program_sha256,
        geometry_program_object_sha256,
        artifact_object_sha256,
        artifact_readback_object_sha256,
    )) = evidence
    else {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_GEOMETRY_LINEAGE_MISSING",
            "candidate-owned geometry evidence is not durably registered",
        ));
    };
    if evidence_project != record.project_id
        || source_binding.geometry_program_sha256 != geometry_program_sha256
        || source_binding.artifact_sha256 != artifact_object_sha256
        || !is_sha256(&geometry_program_object_sha256)
        || !is_sha256(&artifact_object_sha256)
        || !is_sha256(&artifact_readback_object_sha256)
    {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_GEOMETRY_LINEAGE_BINDING_MISMATCH",
            "embedded source binding differs from candidate-owned geometry evidence",
        ));
    }
    let geometry_program_object = validate_lineage_cas_object(
        transaction,
        cas,
        &geometry_program_object_sha256,
        true,
        "GeometryProgram",
    )?;
    let artifact_object = validate_lineage_cas_object(
        transaction,
        cas,
        &artifact_object_sha256,
        true,
        "candidate artifact",
    )?;
    let artifact_readback_object = validate_lineage_cas_object(
        transaction,
        cas,
        &artifact_readback_object_sha256,
        true,
        "candidate artifact readback",
    )?;
    let geometry_program_bytes = cas
        .read_verified_bounded(&geometry_program_object.sha256, 8 * 1024 * 1024)
        .map_err(StoreError::from)?;
    if geometry_program_bytes.len() as u64 != geometry_program_object.size_bytes
        || sha256_hex(&geometry_program_bytes) != geometry_program_sha256
    {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_GEOMETRY_LINEAGE_CANONICAL_MISMATCH",
            "candidate GeometryProgram draft object hash differs from its semantic hash",
        ));
    }
    let geometry_program_value: Value =
        serde_json::from_slice(&geometry_program_bytes).map_err(|error| {
            contract(
                "KNIFE_SOURCE_BINDING_GEOMETRY_LINEAGE_JSON_INVALID",
                format!("candidate GeometryProgram JSON is invalid: {error}"),
            )
        })?;
    let canonical_geometry_program = canonical_json_bytes(&geometry_program_value)
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    if !geometry_program_value.is_object()
        || geometry_program_value
            .get("schema_version")
            .and_then(Value::as_str)
            != Some("GeometryProgram@2")
        || geometry_program_value.get("canonical_sha256").is_some()
        || canonical_geometry_program != geometry_program_bytes
        || canonical_json_hash(&geometry_program_value) != geometry_program_sha256
    {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_GEOMETRY_LINEAGE_CANONICAL_MISMATCH",
            "candidate GeometryProgram CAS must be the canonical hash-bound draft",
        ));
    }
    let artifact_readback_bytes = cas
        .read_verified_bounded(&artifact_readback_object.sha256, 8 * 1024 * 1024)
        .map_err(StoreError::from)?;
    let artifact_readback_value: Value =
        serde_json::from_slice(&artifact_readback_bytes).map_err(|error| {
            contract(
                "KNIFE_SOURCE_BINDING_GEOMETRY_LINEAGE_JSON_INVALID",
                format!("candidate artifact readback JSON is invalid: {error}"),
            )
        })?;
    let mut artifact_readback_preimage = artifact_readback_value.clone();
    artifact_readback_preimage["canonical_sha256"] = Value::String(String::new());
    if artifact_readback_value
        .get("canonical_sha256")
        .and_then(Value::as_str)
        != Some(source_binding.artifact_readback_sha256.as_str())
        || canonical_json_hash(&artifact_readback_preimage)
            != source_binding.artifact_readback_sha256
    {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_GEOMETRY_LINEAGE_CANONICAL_MISMATCH",
            "candidate artifact readback semantic hash differs from its source binding",
        ));
    }
    let quality_report_object_sha256: String = transaction
        .query_row(
            "SELECT quality_report_object_sha256 FROM geometry_candidate_evidence WHERE candidate_id = ?1",
            params![record.source_candidate_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| {
            contract(
                "KNIFE_SOURCE_BINDING_GEOMETRY_LINEAGE_MISSING",
                "candidate quality report lineage is not durably registered",
            )
        })?;
    let mut roots = vec![
        record.authoring_mesh_revision_object_sha256.clone(),
        geometry_program_object.sha256,
        artifact_object.sha256,
        artifact_readback_object.sha256,
    ];
    roots.push(
        validate_lineage_cas_object(
            transaction,
            cas,
            &quality_report_object_sha256,
            true,
            "candidate quality report",
        )?
        .sha256,
    );
    Ok(roots)
}

fn candidate_prepared_object_sha256(
    transaction: &Transaction<'_>,
    candidate_id: &str,
) -> Result<String, StoreError> {
    let prepared: Option<String> = transaction
        .query_row(
            "SELECT prepared_object_sha256 FROM candidates WHERE candidate_id = ?1",
            params![candidate_id],
            |row| row.get(0),
        )
        .optional()?;
    prepared.ok_or_else(|| {
        contract(
            "KNIFE_SOURCE_BINDING_CANDIDATE_ARTIFACT_MISSING",
            "candidate has no prepared artifact object hash",
        )
    })
}

fn roots(record: &KnifeSourceBindingStoreRecord, source_lineage_roots: &[String]) -> Vec<String> {
    let mut roots = vec![
        record.source_binding_object_sha256.clone(),
        record.intent_bundle_object_sha256.clone(),
        record.quality_contract_object_sha256.clone(),
        record.brief_object_sha256.clone(),
        record.reference_object_sha256.clone(),
    ];
    roots.extend(source_lineage_roots.iter().cloned());
    roots.sort();
    roots.dedup();
    roots
}

fn same_record(
    left: &KnifeSourceBindingStoreRecord,
    right: &KnifeSourceBindingStoreRecord,
) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    left.created_at.clear();
    right.created_at.clear();
    left == right
}

fn read_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<KnifeSourceBindingStoreRecord> {
    let record_json: String = row.get(0)?;
    serde_json::from_str(&record_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

const KNIFE_SOURCE_BINDING_TABLE_SQL: &str =
    "CREATE TABLE IF NOT EXISTS knife_source_binding_records (
             schema_version TEXT NOT NULL CHECK (schema_version = 'KnifeSourceBindingStoreRecord@1'),
             source_binding_id TEXT NOT NULL,
             project_id TEXT NOT NULL REFERENCES projects(project_id),
             binding_status TEXT NOT NULL CHECK (binding_status = 'runtime-bound'),
             authoring_eligibility TEXT NOT NULL CHECK (authoring_eligibility = 'ELIGIBLE'),
             intent_bundle_id TEXT NOT NULL,
             intent_bundle_sha256 TEXT NOT NULL,
             intent_bundle_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             brief_id TEXT NOT NULL,
             brief_sha256 TEXT NOT NULL,
             brief_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             reference_id TEXT NOT NULL REFERENCES reference_evidence(reference_id),
             reference_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             reference_evidence_sha256 TEXT NOT NULL,
             quality_contract_id TEXT NOT NULL,
             quality_contract_sha256 TEXT NOT NULL,
             quality_contract_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             source_candidate_id TEXT NOT NULL REFERENCES candidates(candidate_id),
             source_candidate_state_sha256 TEXT NOT NULL,
             authoring_mesh_id TEXT NOT NULL,
             authoring_mesh_lineage_id TEXT NOT NULL,
             authoring_mesh_revision_id TEXT NOT NULL,
             authoring_mesh_revision_index INTEGER NOT NULL CHECK (authoring_mesh_revision_index BETWEEN 0 AND 1000000),
             authoring_mesh_revision_sha256 TEXT NOT NULL,
             authoring_mesh_revision_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             authoring_mesh_identity_sha256 TEXT NOT NULL,
             downstream_binding_requirements_json TEXT NOT NULL,
             high_mesh_created INTEGER NOT NULL CHECK (high_mesh_created = 0),
             high_stage_unlocked INTEGER NOT NULL CHECK (high_stage_unlocked = 0),
             production_stage_advanced INTEGER NOT NULL CHECK (production_stage_advanced = 0),
             candidate_confirmed INTEGER NOT NULL CHECK (candidate_confirmed = 0),
             version_created INTEGER NOT NULL CHECK (version_created = 0),
             export_performed INTEGER NOT NULL CHECK (export_performed = 0),
             quality_status TEXT NOT NULL CHECK (quality_status = 'source_binding_only'),
             visual_status TEXT NOT NULL CHECK (visual_status = 'NOT_RUN'),
             human_status TEXT NOT NULL CHECK (human_status = 'NOT_RUN'),
             engine_status TEXT NOT NULL CHECK (engine_status = 'NOT_RUN'),
             binding_policy TEXT NOT NULL CHECK (binding_policy = 'intent-brief-reference-quality-to-authoring-mesh-exact@1'),
             canonicalization_policy TEXT NOT NULL CHECK (canonicalization_policy = 'canonical-json-sha256-excluding-canonical-sha256@1'),
             source_binding_sha256 TEXT NOT NULL,
             source_binding_object_sha256 TEXT NOT NULL UNIQUE REFERENCES objects(sha256),
             idempotency_key TEXT NOT NULL,
             created_at TEXT NOT NULL,
             record_json TEXT NOT NULL,
             PRIMARY KEY (project_id, source_binding_id),
             UNIQUE (project_id, idempotency_key),
             UNIQUE (project_id, source_binding_sha256),
             UNIQUE (project_id, source_candidate_id, authoring_mesh_id, authoring_mesh_revision_id)
         );
         CREATE INDEX IF NOT EXISTS knife_source_binding_project_idx
             ON knife_source_binding_records(project_id, intent_bundle_id, source_binding_id);
         CREATE INDEX IF NOT EXISTS knife_source_binding_candidate_idx
             ON knife_source_binding_records(source_candidate_id, authoring_mesh_id, authoring_mesh_revision_id);
         CREATE INDEX IF NOT EXISTS knife_source_binding_object_idx
             ON knife_source_binding_records(source_binding_object_sha256, authoring_mesh_revision_object_sha256, quality_contract_object_sha256);";

fn create_table_and_indexes(transaction: &Transaction<'_>) -> Result<(), StoreError> {
    transaction.execute_batch(KNIFE_SOURCE_BINDING_TABLE_SQL)?;
    Ok(())
}

fn migrate_legacy_intent_uniqueness(transaction: &Transaction<'_>) -> Result<(), StoreError> {
    let table_sql: Option<String> = transaction
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'knife_source_binding_records'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let Some(table_sql) = table_sql else {
        return Ok(());
    };
    let normalized_table_sql = table_sql
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    if !normalized_table_sql.contains("unique (project_id, intent_bundle_id)") {
        return Ok(());
    }

    // SQLite cannot remove one UNIQUE constraint in place.  Rebuild only this
    // Store-local index table inside the caller's migration transaction.  The
    // row payload and every identity column are copied verbatim before the old
    // table is dropped; rollback therefore restores the legacy table intact.
    let legacy_count: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM knife_source_binding_records",
        [],
        |row| row.get(0),
    )?;
    transaction.execute_batch(
        "ALTER TABLE knife_source_binding_records RENAME TO knife_source_binding_records_legacy_intent_unique;
         DROP INDEX IF EXISTS knife_source_binding_project_idx;
         DROP INDEX IF EXISTS knife_source_binding_candidate_idx;
         DROP INDEX IF EXISTS knife_source_binding_object_idx;",
    )?;
    create_table_and_indexes(transaction)?;
    transaction.execute(
        "INSERT INTO knife_source_binding_records SELECT * FROM knife_source_binding_records_legacy_intent_unique",
        [],
    )?;
    let copied_count: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM knife_source_binding_records",
        [],
        |row| row.get(0),
    )?;
    if copied_count != legacy_count {
        return Err(contract(
            "KNIFE_SOURCE_BINDING_MIGRATION_ROW_COUNT_MISMATCH",
            format!("source binding migration copied {copied_count} of {legacy_count} rows"),
        ));
    }
    transaction.execute(
        "DROP TABLE knife_source_binding_records_legacy_intent_unique",
        [],
    )?;
    Ok(())
}

pub(crate) fn ensure_table(transaction: &Transaction<'_>) -> Result<(), StoreError> {
    migrate_legacy_intent_uniqueness(transaction)?;
    create_table_and_indexes(transaction)
}

fn read_by_exact_key(
    transaction: &Transaction<'_>,
    project_id: &str,
    source_binding_id: &str,
    source_binding_sha256: &str,
    source_binding_object_sha256: &str,
    intent_bundle_id: &str,
    intent_bundle_sha256: &str,
    intent_bundle_object_sha256: &str,
    brief_id: &str,
    brief_sha256: &str,
    brief_object_sha256: &str,
    reference_id: &str,
    reference_object_sha256: &str,
    reference_evidence_sha256: &str,
    quality_contract_id: &str,
    quality_contract_sha256: &str,
    quality_contract_object_sha256: &str,
    source_candidate_id: &str,
    source_candidate_state_sha256: &str,
    authoring_mesh_id: &str,
    authoring_mesh_lineage_id: &str,
    authoring_mesh_revision_id: &str,
    authoring_mesh_revision_index: u64,
    authoring_mesh_revision_sha256: &str,
    authoring_mesh_revision_object_sha256: &str,
    authoring_mesh_identity_sha256: &str,
) -> Result<Option<KnifeSourceBindingStoreRecord>, StoreError> {
    let record = transaction
        .query_row(
            "SELECT record_json FROM knife_source_binding_records WHERE project_id = ?1 AND source_binding_id = ?2 AND source_binding_sha256 = ?3 AND source_binding_object_sha256 = ?4 AND intent_bundle_id = ?5 AND intent_bundle_sha256 = ?6 AND intent_bundle_object_sha256 = ?7 AND brief_id = ?8 AND brief_sha256 = ?9 AND brief_object_sha256 = ?10 AND reference_id = ?11 AND reference_object_sha256 = ?12 AND reference_evidence_sha256 = ?13 AND quality_contract_id = ?14 AND quality_contract_sha256 = ?15 AND quality_contract_object_sha256 = ?16 AND source_candidate_id = ?17 AND source_candidate_state_sha256 = ?18 AND authoring_mesh_id = ?19 AND authoring_mesh_lineage_id = ?20 AND authoring_mesh_revision_id = ?21 AND authoring_mesh_revision_index = ?22 AND authoring_mesh_revision_sha256 = ?23 AND authoring_mesh_revision_object_sha256 = ?24 AND authoring_mesh_identity_sha256 = ?25",
            params![
                project_id,
                source_binding_id,
                source_binding_sha256,
                source_binding_object_sha256,
                intent_bundle_id,
                intent_bundle_sha256,
                intent_bundle_object_sha256,
                brief_id,
                brief_sha256,
                brief_object_sha256,
                reference_id,
                reference_object_sha256,
                reference_evidence_sha256,
                quality_contract_id,
                quality_contract_sha256,
                quality_contract_object_sha256,
                source_candidate_id,
                source_candidate_state_sha256,
                authoring_mesh_id,
                authoring_mesh_lineage_id,
                authoring_mesh_revision_id,
                i64::try_from(authoring_mesh_revision_index).map_err(|_| StoreError::InvalidData("source binding revision index is too large".to_owned()))?,
                authoring_mesh_revision_sha256,
                authoring_mesh_revision_object_sha256,
                authoring_mesh_identity_sha256,
            ],
            read_record,
        )
        .optional()?;
    Ok(record)
}

impl Store {
    fn get_knife_source_binding_in_transaction(
        &self,
        transaction: &Transaction<'_>,
        project_id: &str,
        source_binding_id: &str,
        source_binding_sha256: &str,
        role: &str,
    ) -> Result<Option<KnifeSourceBindingStoreRecord>, StoreError> {
        let Some(record) = transaction
            .query_row(
                "SELECT record_json FROM knife_source_binding_records WHERE project_id = ?1 AND source_binding_id = ?2 AND source_binding_sha256 = ?3",
                params![project_id, source_binding_id, source_binding_sha256],
                read_record,
            )
            .optional()?
        else {
            return Ok(None);
        };
        validate_record(&record)?;
        if record.source_binding_object_sha256.is_empty() {
            return Err(contract(
                "KNIFE_SOURCE_BINDING_CAS_MISSING",
                "source binding object hash is missing",
            ));
        }
        let object = read_object_record(transaction, &record.source_binding_object_sha256)?;
        let bytes = validate_source_cas_object(
            transaction,
            &self.cas,
            &object,
            &record.source_binding_object_sha256,
            true,
            role,
        )?;
        validate_source_payload(&bytes, &record)?;
        validate_intent_lineage(transaction, &self.cas, &record)?;
        validate_brief_lineage(transaction, &self.cas, &record)?;
        validate_reference_lineage(transaction, &self.cas, &record)?;
        validate_source_lineage(transaction, &self.cas, &record)?;
        Ok(Some(record))
    }

    /// Atomically install one immutable KnifeSourceBinding and promote its
    /// entire source lineage to reachable.  Exact idempotency replay returns
    /// `(record, true)`; same-key conflicts leave no new row or reachability
    /// mutation, while one intent bundle may own multiple source revisions.
    pub fn record_knife_source_binding_with_replay(
        &self,
        commit: &KnifeSourceBindingCommit,
    ) -> Result<(KnifeSourceBindingStoreRecord, bool), StoreError> {
        validate_record(&commit.record)?;
        if commit.cas.source_binding.sha256 != commit.record.source_binding_object_sha256 {
            return Err(contract(
                "KNIFE_SOURCE_BINDING_CAS_BINDING_MISMATCH",
                "source binding CAS object hash differs from its Store binding",
            ));
        }
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        ensure_table(&transaction)?;
        let payload_bytes = validate_source_cas_object(
            &transaction,
            &self.cas,
            &commit.cas.source_binding,
            &commit.record.source_binding_object_sha256,
            false,
            "source binding",
        )?;
        validate_source_payload(&payload_bytes, &commit.record)?;

        let existing = transaction
            .query_row(
                "SELECT record_json FROM knife_source_binding_records WHERE project_id = ?1 AND idempotency_key = ?2",
                params![commit.record.project_id, commit.record.idempotency_key],
                read_record,
            )
            .optional()?;
        if let Some(existing) = existing {
            validate_record(&existing)?;
            if !same_record(&existing, &commit.record) {
                return Err(contract(
                    "KNIFE_SOURCE_BINDING_IDEMPOTENCY_CONFLICT",
                    "project and idempotency key are already bound to different source metadata",
                ));
            }
            validate_intent_lineage(&transaction, &self.cas, &existing)?;
            validate_brief_lineage(&transaction, &self.cas, &existing)?;
            validate_reference_lineage(&transaction, &self.cas, &existing)?;
            let source_roots = validate_source_lineage(&transaction, &self.cas, &existing)?;
            let bytes = validate_source_cas_object(
                &transaction,
                &self.cas,
                &commit.cas.source_binding,
                &existing.source_binding_object_sha256,
                false,
                "source binding replay",
            )?;
            validate_source_payload(&bytes, &existing)?;
            mark_reachable_in_transaction(&transaction, &roots(&existing, &source_roots))?;
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
                "source binding project does not exist",
            ));
        }
        let source_conflict: Option<String> = transaction
            .query_row(
                "SELECT idempotency_key FROM knife_source_binding_records WHERE project_id = ?1 AND source_candidate_id = ?2 AND authoring_mesh_id = ?3 AND authoring_mesh_revision_id = ?4",
                params![commit.record.project_id, commit.record.source_candidate_id, commit.record.authoring_mesh_id, commit.record.authoring_mesh_revision_id],
                |row| row.get(0),
            )
            .optional()?;
        if source_conflict.is_some() {
            return Err(contract(
                "KNIFE_SOURCE_BINDING_SOURCE_CONFLICT",
                "source candidate and AuthoringMesh revision are already bound to another intent",
            ));
        }
        let source_roots = {
            validate_intent_lineage(&transaction, &self.cas, &commit.record)?;
            validate_brief_lineage(&transaction, &self.cas, &commit.record)?;
            validate_reference_lineage(&transaction, &self.cas, &commit.record)?;
            validate_source_lineage(&transaction, &self.cas, &commit.record)?
        };
        let record_json = String::from_utf8(record_bytes(&commit.record)?).map_err(|error| {
            StoreError::InvalidData(format!("source binding Store record is not UTF-8: {error}"))
        })?;
        transaction.execute(
            "INSERT INTO knife_source_binding_records (schema_version, source_binding_id, project_id, binding_status, authoring_eligibility, intent_bundle_id, intent_bundle_sha256, intent_bundle_object_sha256, brief_id, brief_sha256, brief_object_sha256, reference_id, reference_object_sha256, reference_evidence_sha256, quality_contract_id, quality_contract_sha256, quality_contract_object_sha256, source_candidate_id, source_candidate_state_sha256, authoring_mesh_id, authoring_mesh_lineage_id, authoring_mesh_revision_id, authoring_mesh_revision_index, authoring_mesh_revision_sha256, authoring_mesh_revision_object_sha256, authoring_mesh_identity_sha256, downstream_binding_requirements_json, high_mesh_created, high_stage_unlocked, production_stage_advanced, candidate_confirmed, version_created, export_performed, quality_status, visual_status, human_status, engine_status, binding_policy, canonicalization_policy, source_binding_sha256, source_binding_object_sha256, idempotency_key, created_at, record_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37, ?38, ?39, ?40, ?41, ?42, ?43, ?44)",
            params![
                commit.record.schema_version,
                commit.record.source_binding_id,
                commit.record.project_id,
                commit.record.binding_status,
                commit.record.authoring_eligibility,
                commit.record.intent_bundle_id,
                commit.record.intent_bundle_sha256,
                commit.record.intent_bundle_object_sha256,
                commit.record.brief_id,
                commit.record.brief_sha256,
                commit.record.brief_object_sha256,
                commit.record.reference_id,
                commit.record.reference_object_sha256,
                commit.record.reference_evidence_sha256,
                commit.record.quality_contract_id,
                commit.record.quality_contract_sha256,
                commit.record.quality_contract_object_sha256,
                commit.record.source_candidate_id,
                commit.record.source_candidate_state_sha256,
                commit.record.authoring_mesh_id,
                commit.record.authoring_mesh_lineage_id,
                commit.record.authoring_mesh_revision_id,
                i64::try_from(commit.record.authoring_mesh_revision_index).map_err(|_| StoreError::InvalidData("source binding revision index is too large".to_owned()))?,
                commit.record.authoring_mesh_revision_sha256,
                commit.record.authoring_mesh_revision_object_sha256,
                commit.record.authoring_mesh_identity_sha256,
                serde_json::to_string(&commit.record.downstream_binding_requirements)
                    .map_err(|error| StoreError::InvalidData(error.to_string()))?,
                commit.record.high_mesh_created,
                commit.record.high_stage_unlocked,
                commit.record.production_stage_advanced,
                commit.record.candidate_confirmed,
                commit.record.version_created,
                commit.record.export_performed,
                commit.record.quality_status,
                commit.record.visual_status,
                commit.record.human_status,
                commit.record.engine_status,
                commit.record.binding_policy,
                commit.record.canonicalization_policy,
                commit.record.source_binding_sha256,
                commit.record.source_binding_object_sha256,
                commit.record.idempotency_key,
                commit.record.created_at,
                record_json,
            ],
        )?;
        mark_reachable_in_transaction(&transaction, &roots(&commit.record, &source_roots))?;
        let stored = transaction
            .query_row(
                "SELECT record_json FROM knife_source_binding_records WHERE project_id = ?1 AND source_binding_id = ?2",
                params![commit.record.project_id, commit.record.source_binding_id],
                read_record,
            )?;
        validate_record(&stored)?;
        transaction.commit()?;
        Ok((stored, false))
    }

    pub fn get_knife_source_binding(
        &self,
        project_id: &str,
        source_binding_id: &str,
        source_binding_sha256: &str,
    ) -> Result<Option<KnifeSourceBindingStoreRecord>, StoreError> {
        if !is_opaque_id(project_id)
            || !is_opaque_id(source_binding_id)
            || !is_sha256(source_binding_sha256)
        {
            return Err(StoreError::InvalidData(
                "source binding lookup identity is invalid".to_owned(),
            ));
        }
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        ensure_table(&transaction)?;
        let record = self.get_knife_source_binding_in_transaction(
            &transaction,
            project_id,
            source_binding_id,
            source_binding_sha256,
            "source binding get",
        )?;
        transaction.commit()?;
        Ok(record)
    }

    pub fn get_knife_source_binding_exact(
        &self,
        project_id: &str,
        source_binding_id: &str,
        source_binding_sha256: &str,
        source_binding_object_sha256: &str,
        intent_bundle_id: &str,
        intent_bundle_sha256: &str,
        intent_bundle_object_sha256: &str,
        brief_id: &str,
        brief_sha256: &str,
        brief_object_sha256: &str,
        reference_id: &str,
        reference_object_sha256: &str,
        reference_evidence_sha256: &str,
        quality_contract_id: &str,
        quality_contract_sha256: &str,
        quality_contract_object_sha256: &str,
        source_candidate_id: &str,
        source_candidate_state_sha256: &str,
        authoring_mesh_id: &str,
        authoring_mesh_lineage_id: &str,
        authoring_mesh_revision_id: &str,
        authoring_mesh_revision_index: u64,
        authoring_mesh_revision_sha256: &str,
        authoring_mesh_revision_object_sha256: &str,
        authoring_mesh_identity_sha256: &str,
    ) -> Result<Option<KnifeSourceBindingStoreRecord>, StoreError> {
        let ids = [
            project_id,
            source_binding_id,
            intent_bundle_id,
            brief_id,
            reference_id,
            quality_contract_id,
            source_candidate_id,
            authoring_mesh_id,
            authoring_mesh_lineage_id,
            authoring_mesh_revision_id,
        ];
        let hashes = [
            source_binding_sha256,
            source_binding_object_sha256,
            intent_bundle_sha256,
            intent_bundle_object_sha256,
            brief_sha256,
            brief_object_sha256,
            reference_object_sha256,
            reference_evidence_sha256,
            quality_contract_sha256,
            quality_contract_object_sha256,
            source_candidate_state_sha256,
            authoring_mesh_revision_sha256,
            authoring_mesh_revision_object_sha256,
            authoring_mesh_identity_sha256,
        ];
        if ids.iter().any(|value| !is_opaque_id(value))
            || hashes.iter().any(|value| !is_sha256(value))
            || authoring_mesh_revision_index > 1_000_000
        {
            return Err(StoreError::InvalidData(
                "source binding exact lookup identity is invalid".to_owned(),
            ));
        }
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        ensure_table(&transaction)?;
        let Some(record) = read_by_exact_key(
            &transaction,
            project_id,
            source_binding_id,
            source_binding_sha256,
            source_binding_object_sha256,
            intent_bundle_id,
            intent_bundle_sha256,
            intent_bundle_object_sha256,
            brief_id,
            brief_sha256,
            brief_object_sha256,
            reference_id,
            reference_object_sha256,
            reference_evidence_sha256,
            quality_contract_id,
            quality_contract_sha256,
            quality_contract_object_sha256,
            source_candidate_id,
            source_candidate_state_sha256,
            authoring_mesh_id,
            authoring_mesh_lineage_id,
            authoring_mesh_revision_id,
            authoring_mesh_revision_index,
            authoring_mesh_revision_sha256,
            authoring_mesh_revision_object_sha256,
            authoring_mesh_identity_sha256,
        )?
        else {
            transaction.commit()?;
            return Ok(None);
        };
        validate_record(&record)?;
        let object = read_object_record(&transaction, &record.source_binding_object_sha256)?;
        let bytes = validate_source_cas_object(
            &transaction,
            &self.cas,
            &object,
            &record.source_binding_object_sha256,
            true,
            "source binding exact get",
        )?;
        validate_source_payload(&bytes, &record)?;
        validate_intent_lineage(&transaction, &self.cas, &record)?;
        validate_brief_lineage(&transaction, &self.cas, &record)?;
        validate_reference_lineage(&transaction, &self.cas, &record)?;
        validate_source_lineage(&transaction, &self.cas, &record)?;
        transaction.commit()?;
        Ok(Some(record))
    }

    pub fn get_knife_source_binding_by_idempotency(
        &self,
        project_id: &str,
        idempotency_key: &str,
    ) -> Result<Option<KnifeSourceBindingStoreRecord>, StoreError> {
        if !is_opaque_id(project_id) || !is_opaque_id(idempotency_key) {
            return Err(StoreError::InvalidData(
                "source binding idempotency identity is invalid".to_owned(),
            ));
        }
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        ensure_table(&transaction)?;
        let identity: Option<(String, String, String)> = transaction
            .query_row(
                "SELECT source_binding_id, source_binding_sha256, source_binding_object_sha256 FROM knife_source_binding_records WHERE project_id = ?1 AND idempotency_key = ?2",
                params![project_id, idempotency_key],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let Some((source_binding_id, source_binding_sha256, _)) = identity else {
            transaction.commit()?;
            return Ok(None);
        };
        let record = self.get_knife_source_binding_in_transaction(
            &transaction,
            project_id,
            &source_binding_id,
            &source_binding_sha256,
            "source binding idempotency get",
        )?;
        transaction.commit()?;
        Ok(record)
    }

    pub fn read_knife_source_binding_json(
        &self,
        project_id: &str,
        source_binding_id: &str,
        source_binding_sha256: &str,
    ) -> Result<Option<Value>, StoreError> {
        let Some(record) =
            self.get_knife_source_binding(project_id, source_binding_id, source_binding_sha256)?
        else {
            return Ok(None);
        };
        let bytes = self
            .cas
            .read_verified_bounded(
                &record.source_binding_object_sha256,
                KNIFE_SOURCE_BINDING_MAX_JSON_BYTES,
            )
            .map_err(StoreError::from)?;
        validate_source_payload(&bytes, &record)?;
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| StoreError::InvalidData(error.to_string()))
    }

    pub fn knife_source_binding_cas_roots(record: &KnifeSourceBindingStoreRecord) -> Vec<String> {
        roots(record, &[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CandidateRecord, ProjectRecord, ReferenceAuthorization, ReferenceEvidenceRecord};
    use forgecad_core::{canonical_json_hash, sha256_hex};
    use rusqlite::params;
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    const PROJECT: &str = "knife-source-binding-project";
    const BRIEF: &str = "knife-source-binding-brief";
    const REFERENCE: &str = "knife-source-binding-reference";
    const INTENT: &str = "knife-source-binding-intent";
    const CANDIDATE: &str = "knife-source-binding-candidate";
    const MESH: &str = "knife-source-binding-mesh";
    const LINEAGE: &str = "knife-source-binding-lineage";
    const REVISION: &str = "knife-source-binding-revision";
    const SOURCE: &str = "knife-source-binding-001";
    const NOW: &str = "2026-08-30T00:00:00Z";

    fn h(byte: char) -> String {
        std::iter::repeat_n(byte, 64).collect()
    }

    fn project(store: &Store) {
        store
            .insert_project(&ProjectRecord {
                schema_version: "Project@1".to_owned(),
                project_id: PROJECT.to_owned(),
                name: "Knife source binding test".to_owned(),
                policy: json!({"scope":"test"}),
                created_at: NOW.to_owned(),
                updated_at: NOW.to_owned(),
                active_snapshot_revision: 0,
                head_snapshot_id: None,
                canonical_sha256: h('a'),
            })
            .expect("project");
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
        let bytes = canonical_json_bytes(&value).expect("canonical object bytes");
        let object = store
            .put_object(&bytes, None, mime, kind, NOW)
            .expect("canonical object");
        (object.record, semantic)
    }

    struct FixtureLineage {
        reference_object_sha256: String,
        reference_evidence_sha256: String,
        brief_object_sha256: String,
        brief_sha256: String,
        intent_object_sha256: String,
        intent_sha256: String,
        quality_object_sha256: String,
        quality_sha256: String,
        revision_object_sha256: String,
        revision_sha256: String,
        identity_sha256: String,
    }

    fn setup_lineage(store: &Store) -> FixtureLineage {
        let reference_object = store
            .put_object(
                b"reference-image",
                None,
                "image/png",
                "reference-image",
                NOW,
            )
            .expect("reference object");
        let authorization = ReferenceAuthorization {
            user_authorized: true,
            declaration: "authorized test source".to_owned(),
        };
        let reference_evidence_sha256 = canonical_json_hash(&json!({
            "schema_version": "ReferenceEvidence@1",
            "reference_id": REFERENCE,
            "project_id": PROJECT,
            "object_sha256": reference_object.record.sha256,
            "mime": "image/png",
            "size_bytes": reference_object.record.size_bytes,
            "width": 1,
            "height": 1,
            "frame_count": 1,
            "import_mode": "inline_content",
            "authorization": authorization,
            "derived_object_sha256": Value::Null,
            "created_at": NOW,
        }));
        store
            .insert_reference_evidence(&ReferenceEvidenceRecord {
                schema_version: "ReferenceEvidence@1".to_owned(),
                reference_id: REFERENCE.to_owned(),
                project_id: PROJECT.to_owned(),
                object_sha256: reference_object.record.sha256.clone(),
                mime: "image/png".to_owned(),
                size_bytes: reference_object.record.size_bytes,
                width: 1,
                height: 1,
                frame_count: 1,
                import_mode: "inline_content".to_owned(),
                authorization,
                derived_object_sha256: None,
                canonical_sha256: reference_evidence_sha256.clone(),
                created_at: NOW.to_owned(),
            })
            .expect("reference evidence");

        let (brief_object, brief_sha256) = canonical_object(
            store,
            json!({"schema_version":"WeaponryKnifeProductionBrief@1"}),
            "weaponry-knife-production-brief",
            "application/json",
        );
        let brief_record_json = json!({
            "schema_version": "WeaponryKnifeProductionBriefStoreRecord@1",
            "project_id": PROJECT,
            "brief_id": BRIEF,
            "brief_canonical_sha256": brief_sha256,
        });
        let connection = store.connection.lock().expect("connection");
        connection
            .execute(
                "INSERT INTO weaponry_knife_production_brief_records (schema_version, project_id, brief_id, brief_object_sha256, brief_canonical_sha256, reference_id, reference_object_sha256, reference_evidence_sha256, parent_brief_id, parent_brief_sha256, freeze_policy, source_reference_hashes_json, status, conflict_freeze_state, idempotency_key, created_at, record_json) VALUES ('WeaponryKnifeProductionBriefStoreRecord@1', ?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, NULL, 'initial-intake-no-parent@1', ?8, 'eligible', 'resolved', 'brief-key', ?9, ?10)",
                params![
                    PROJECT,
                    BRIEF,
                    brief_object.sha256,
                    brief_sha256,
                    REFERENCE,
                    reference_object.record.sha256,
                    reference_evidence_sha256,
                    serde_json::to_string(&vec![reference_object.record.sha256.clone()]).expect("refs"),
                    NOW,
                    serde_json::to_string(&brief_record_json).expect("brief row"),
                ],
            )
            .expect("brief row");
        drop(connection);

        let (quality_object, quality_sha256) = canonical_object(
            store,
            json!({
                "schema_version":"KnifeQualityContract@1",
                "contract_id":"knife-quality-contract",
            }),
            "knife-quality-contract",
            "application/json",
        );
        let (intent_object, intent_sha256) = canonical_object(
            store,
            json!({"schema_version":"KnifeReferenceIntentBundle@1"}),
            "knife-reference-intent-bundle",
            "application/json",
        );
        let intent_record_json = json!({
            "schema_version": "KnifeReferenceIntentBundleStoreRecord@1",
            "intent_bundle_id": INTENT,
        });
        let connection = store.connection.lock().expect("connection");
        connection
            .execute(
                "INSERT INTO knife_reference_intent_bundle_records (schema_version, intent_bundle_id, project_id, brief_id, brief_sha256, brief_object_sha256, reference_id, reference_object_sha256, reference_evidence_sha256, intake_manifest_sha256, intake_manifest_object_sha256, detail_inventory_sha256, detail_inventory_object_sha256, quality_contract_sha256, quality_contract_object_sha256, intent_bundle_sha256, intent_bundle_object_sha256, idempotency_key, created_at, record_json) VALUES ('KnifeReferenceIntentBundleStoreRecord@1', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, 'intent-key', ?17, ?18)",
                params![
                    INTENT,
                    PROJECT,
                    BRIEF,
                    brief_sha256,
                    brief_object.sha256,
                    REFERENCE,
                    reference_object.record.sha256,
                    reference_evidence_sha256,
                    h('b'),
                    brief_object.sha256,
                    h('c'),
                    brief_object.sha256,
                    quality_sha256,
                    quality_object.sha256,
                    intent_sha256,
                    intent_object.sha256,
                    NOW,
                    serde_json::to_string(&intent_record_json).expect("intent row"),
                ],
            )
            .expect("intent row");
        drop(connection);

        let artifact_object = store
            .put_object(
                b"candidate-artifact",
                None,
                "model/gltf-binary",
                "geometry-glb",
                NOW,
            )
            .expect("candidate artifact");
        let (artifact_readback_object, artifact_readback_sha256) = canonical_object(
            store,
            json!({"schema_version":"ArtifactReadback@1"}),
            "artifact-readback",
            "application/json",
        );
        let geometry_program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":PROJECT,
        });
        let geometry_program_bytes =
            canonical_json_bytes(&geometry_program).expect("GeometryProgram draft bytes");
        let geometry_program_sha256 = sha256_hex(&geometry_program_bytes);
        let geometry_program_object = store
            .put_object(
                &geometry_program_bytes,
                None,
                "application/json",
                "geometry-program-v2",
                NOW,
            )
            .expect("GeometryProgram draft object");
        assert_eq!(
            geometry_program_object.record.sha256,
            geometry_program_sha256
        );
        let quality_report_object = store
            .put_object(
                b"candidate-quality-report",
                None,
                "application/json",
                "quality-report",
                NOW,
            )
            .expect("quality report");
        store
            .insert_candidate(&CandidateRecord {
                schema_version: "Candidate@1".to_owned(),
                candidate_id: CANDIDATE.to_owned(),
                project_id: PROJECT.to_owned(),
                base_version_id: None,
                source_version_id: None,
                prepared_object_id: Some("artifact-id".to_owned()),
                prepared_object_sha256: Some(artifact_object.record.sha256.clone()),
                state: "prepared".to_owned(),
                request_sha256: h('d'),
                manifest_hash: None,
                quality_report_id: None,
                quality_hard_gate_passed: false,
                canonical_sha256: h('e'),
                error_code: None,
                created_at: NOW.to_owned(),
                updated_at: NOW.to_owned(),
            })
            .expect("candidate");

        let mut embedded_source = json!({
            "schema_version": "AuthoringMeshV2SourceBinding@1",
            "project_id": PROJECT,
            "candidate_id": CANDIDATE,
            "candidate_state_sha256": h('e'),
            "artifact_id": "artifact-id",
            "artifact_sha256": artifact_object.record.sha256,
            "artifact_readback_sha256": artifact_readback_sha256,
            "geometry_program_sha256": geometry_program_sha256,
            "source_node_id": "source-node",
            "part_id": "part",
            "material_zone_id": "zone",
            "solid": true,
            "source_operator_id": "box",
            "source_parameters_sha256": h('p'),
            "part_output_sha256": h('o'),
            "position_m": [0.0, 0.0, 0.0],
            "rotation_rad": [0.0, 0.0, 0.0],
            "canonical_sha256": "",
        });
        let embedded_source_sha256 = canonical_json_hash(&embedded_source);
        embedded_source["canonical_sha256"] = Value::String(embedded_source_sha256);
        let (revision_object, revision_sha256) = canonical_object(
            store,
            json!({
                "schema_version": "AuthoringMeshRevision@2",
                "source_binding": embedded_source,
            }),
            AUTHORING_MESH_V2_REVISION_OBJECT_KIND,
            "application/json",
        );
        let connection = store.connection.lock().expect("connection");
        connection
            .execute(
                "INSERT INTO geometry_candidate_evidence (candidate_id, project_id, reference_id, reference_sha256, geometry_program_sha256, geometry_program_object_sha256, operator_catalog_sha256, readback_config_sha256, artifact_object_sha256, artifact_readback_object_sha256, quality_report_object_sha256, quality_report_id, canonical_sha256, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'quality-report', ?12, ?13)",
                params![
                    CANDIDATE,
                    PROJECT,
                    REFERENCE,
                    reference_evidence_sha256,
                    geometry_program_sha256,
                    geometry_program_object.record.sha256,
                    h('1'),
                    h('2'),
                    artifact_object.record.sha256,
                    artifact_readback_object.sha256,
                    quality_report_object.record.sha256,
                    h('3'),
                    NOW,
                ],
            )
            .expect("geometry evidence");
        connection
            .execute(
                "INSERT INTO authoring_mesh_v2_durable_records (schema_version, project_id, mesh_id, lineage_id, revision_id, parent_revision_ids_json, revision_index, revision_object_sha256, revision_sha256, operation_id, operation_kind, operation_lineage_sha256, request_input_sha256, idempotency_key, materialization_status, canonical_sha256, created_at) VALUES ('AuthoringMeshV2DurableRecord@1', ?1, ?2, ?3, ?4, '[]', 0, ?5, ?6, NULL, NULL, NULL, ?7, 'revision-key', 'runtime-owned-store-authoring-mesh-v2-durable-record@1', ?8, ?9)",
                params![
                    PROJECT,
                    MESH,
                    LINEAGE,
                    REVISION,
                    revision_object.sha256,
                    revision_sha256,
                    h('3'),
                    h('4'),
                    NOW,
                ],
            )
            .expect("mesh revision");
        drop(connection);

        // Lineage objects are inputs to this atomic Store commit and must be
        // readable before the source-binding transaction promotes its roots.
        let connection = store.connection.lock().expect("connection");
        connection
            .execute("UPDATE objects SET reachability = 'reachable'", [])
            .expect("lineage roots");
        drop(connection);

        FixtureLineage {
            reference_object_sha256: reference_object.record.sha256,
            reference_evidence_sha256,
            brief_object_sha256: brief_object.sha256,
            brief_sha256,
            intent_object_sha256: intent_object.sha256,
            intent_sha256,
            quality_object_sha256: quality_object.sha256,
            quality_sha256,
            revision_object_sha256: revision_object.sha256,
            revision_sha256: revision_sha256.clone(),
            identity_sha256: canonical_json_hash(&json!({
                "schema_version": "AuthoringMeshSourceIdentity@1",
                "mesh_id": MESH,
                "lineage_id": LINEAGE,
                "revision_id": REVISION,
                "revision_index": 0,
                "revision_sha256": revision_sha256,
            })),
        }
    }

    fn commit(store: &Store) -> KnifeSourceBindingCommit {
        let fixture = setup_lineage(store);
        let record = KnifeSourceBindingStoreRecord {
            schema_version: KNIFE_SOURCE_BINDING_RECORD_SCHEMA_VERSION.to_owned(),
            source_binding_id: SOURCE.to_owned(),
            project_id: PROJECT.to_owned(),
            binding_status: KNIFE_SOURCE_BINDING_BINDING_STATUS.to_owned(),
            authoring_eligibility: KNIFE_SOURCE_BINDING_AUTHORING_ELIGIBILITY.to_owned(),
            intent_bundle_id: INTENT.to_owned(),
            intent_bundle_sha256: fixture.intent_sha256,
            intent_bundle_object_sha256: fixture.intent_object_sha256,
            brief_id: BRIEF.to_owned(),
            brief_sha256: fixture.brief_sha256,
            brief_object_sha256: fixture.brief_object_sha256,
            reference_id: REFERENCE.to_owned(),
            reference_object_sha256: fixture.reference_object_sha256,
            reference_evidence_sha256: fixture.reference_evidence_sha256,
            quality_contract_id: "knife-quality-contract".to_owned(),
            quality_contract_sha256: fixture.quality_sha256,
            quality_contract_object_sha256: fixture.quality_object_sha256,
            source_candidate_id: CANDIDATE.to_owned(),
            source_candidate_state_sha256: h('e'),
            authoring_mesh_id: MESH.to_owned(),
            authoring_mesh_lineage_id: LINEAGE.to_owned(),
            authoring_mesh_revision_id: REVISION.to_owned(),
            authoring_mesh_revision_index: 0,
            authoring_mesh_revision_sha256: fixture.revision_sha256,
            authoring_mesh_revision_object_sha256: fixture.revision_object_sha256,
            authoring_mesh_identity_sha256: fixture.identity_sha256,
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
            source_binding_sha256: h('4'),
            source_binding_object_sha256: h('5'),
            idempotency_key: "source-binding-key".to_owned(),
            created_at: NOW.to_owned(),
        };
        let mut record = record;
        let mut preimage = source_payload_value(&record);
        preimage["canonical_sha256"] = Value::String(String::new());
        record.source_binding_sha256 = canonical_json_hash(&preimage);
        let bytes = canonical_json_bytes(&source_payload_value(&record)).expect("source payload");
        let source_object = store
            .put_object(
                &bytes,
                None,
                KNIFE_SOURCE_BINDING_JSON_MIME,
                KNIFE_SOURCE_BINDING_OBJECT_KIND,
                NOW,
            )
            .expect("source binding object");
        record.source_binding_object_sha256 = source_object.record.sha256.clone();
        KnifeSourceBindingCommit {
            record,
            cas: KnifeSourceBindingCasBundle {
                source_binding: source_object.record,
            },
        }
    }

    #[test]
    fn source_binding_payload_matches_contract_fixture_without_store_hashes() {
        let fixture: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../packages/forgecad-contracts/fixtures/knife-source-binding/positive/dragonfang-source-binding.json"
        )))
        .expect("source binding fixture");
        let mut store_value = fixture.clone();
        let object = store_value.as_object_mut().expect("fixture object");
        object.insert(
            "schema_version".to_owned(),
            Value::String(KNIFE_SOURCE_BINDING_RECORD_SCHEMA_VERSION.to_owned()),
        );
        object.insert(
            "source_binding_object_sha256".to_owned(),
            Value::String(h('f')),
        );
        object.insert(
            "source_binding_sha256".to_owned(),
            fixture["canonical_sha256"].clone(),
        );
        object.insert(
            "idempotency_key".to_owned(),
            Value::String("fixture-key".to_owned()),
        );
        let record: KnifeSourceBindingStoreRecord =
            serde_json::from_value(store_value).expect("Store record");
        assert_eq!(source_payload_value(&record), fixture);
        let mut preimage = fixture.clone();
        preimage["canonical_sha256"] = Value::String(String::new());
        assert_eq!(canonical_json_hash(&preimage), fixture["canonical_sha256"]);
        let bytes = canonical_json_bytes(&fixture).expect("fixture bytes");
        assert!(!bytes
            .windows(b"source_binding_object_sha256".len())
            .any(|window| { window == b"source_binding_object_sha256" }));
        assert!(!bytes
            .windows(b"source_binding_sha256".len())
            .any(|window| { window == b"source_binding_sha256" }));
    }

    #[test]
    fn source_binding_commit_replay_exact_get_and_roots_survive_restart() {
        let root = std::env::temp_dir().join(format!("forgecad-source-binding-{}", Uuid::new_v4()));
        let db = root.join("store.sqlite");
        let cas = root.join("cas");
        let first = Store::open_with_cas(&db, &cas).expect("store");
        project(&first);
        let commit = commit(&first);
        let (record, replayed) = first
            .record_knife_source_binding_with_replay(&commit)
            .expect("commit");
        assert!(!replayed);
        assert_eq!(record, commit.record);
        for hash in [
            record.source_binding_object_sha256.clone(),
            record.intent_bundle_object_sha256.clone(),
            record.brief_object_sha256.clone(),
            record.reference_object_sha256.clone(),
            record.quality_contract_object_sha256.clone(),
            record.authoring_mesh_revision_object_sha256.clone(),
        ] {
            assert_eq!(
                first
                    .get_object(&hash)
                    .expect("object")
                    .expect("metadata")
                    .reachability,
                "reachable"
            );
        }
        let mut replay_with_new_cas_timestamp = commit.clone();
        replay_with_new_cas_timestamp.cas.source_binding.created_at =
            "2026-08-30T00:00:01Z".to_owned();
        let (replayed_record, replayed) = first
            .record_knife_source_binding_with_replay(&replay_with_new_cas_timestamp)
            .expect("replay with a newly-staged CAS timestamp");
        assert!(replayed);
        assert_eq!(replayed_record, record);

        let mut replay_with_metadata_drift = commit.clone();
        replay_with_metadata_drift.cas.source_binding.size_bytes += 1;
        let error = first
            .record_knife_source_binding_with_replay(&replay_with_metadata_drift)
            .expect_err("non-content CAS metadata drift");
        assert!(matches!(
            error,
            StoreError::Contract { code, .. }
                if code == "KNIFE_SOURCE_BINDING_CAS_METADATA_MISMATCH"
        ));
        let by_idempotency = first
            .get_knife_source_binding_by_idempotency(PROJECT, &record.idempotency_key)
            .expect("idempotency get");
        assert_eq!(by_idempotency, Some(record.clone()));
        let (again, replayed) = first
            .record_knife_source_binding_with_replay(&commit)
            .expect("replay");
        assert!(replayed);
        assert_eq!(again, record);
        let exact = first
            .get_knife_source_binding_exact(
                PROJECT,
                SOURCE,
                &record.source_binding_sha256,
                &record.source_binding_object_sha256,
                INTENT,
                &record.intent_bundle_sha256,
                &record.intent_bundle_object_sha256,
                BRIEF,
                &record.brief_sha256,
                &record.brief_object_sha256,
                REFERENCE,
                &record.reference_object_sha256,
                &record.reference_evidence_sha256,
                &record.quality_contract_id,
                &record.quality_contract_sha256,
                &record.quality_contract_object_sha256,
                CANDIDATE,
                &record.source_candidate_state_sha256,
                MESH,
                LINEAGE,
                REVISION,
                0,
                &record.authoring_mesh_revision_sha256,
                &record.authoring_mesh_revision_object_sha256,
                &record.authoring_mesh_identity_sha256,
            )
            .expect("exact get");
        assert_eq!(exact, Some(record.clone()));
        drop(first);
        let restarted = Store::open_with_cas(&db, &cas).expect("restart");
        assert_eq!(
            restarted
                .get_knife_source_binding(PROJECT, SOURCE, &record.source_binding_sha256)
                .expect("restart get"),
            Some(record)
        );
        let _ = fs::remove_dir_all(PathBuf::from(root));
    }

    #[test]
    fn source_binding_late_rejection_and_tamper_leave_no_binding_row() {
        let store = Store::memory().expect("store");
        project(&store);
        let late_commit = commit(&store);
        store
            .connection
            .lock()
            .expect("connection")
            .execute(
                "UPDATE candidates SET canonical_sha256 = ?1 WHERE candidate_id = ?2",
                params![h('9'), CANDIDATE],
            )
            .expect("late candidate mutation");
        let error = store
            .record_knife_source_binding_with_replay(&late_commit)
            .expect_err("late candidate rejection");
        assert!(
            matches!(error, StoreError::Contract { code, .. } if code == "KNIFE_SOURCE_BINDING_CANDIDATE_BINDING_MISMATCH")
        );
        let count: i64 = store
            .connection
            .lock()
            .expect("connection")
            .query_row(
                "SELECT COUNT(*) FROM knife_source_binding_records",
                [],
                |row| row.get(0),
            )
            .expect("row count");
        assert_eq!(count, 0);

        let tamper_store = Store::memory().expect("tamper store");
        project(&tamper_store);
        let valid = commit(&tamper_store);
        let object_path = tamper_store
            .cas
            .root()
            .join("objects")
            .join(&valid.cas.source_binding.sha256[..2])
            .join(&valid.cas.source_binding.sha256);
        fs::write(&object_path, b"tampered").expect("tamper CAS");
        let error = tamper_store
            .record_knife_source_binding_with_replay(&valid)
            .expect_err("tamper rejection");
        assert!(matches!(
            error,
            StoreError::Cas(_) | StoreError::Contract { .. }
        ));
        let count: i64 = tamper_store
            .connection
            .lock()
            .expect("connection")
            .query_row(
                "SELECT COUNT(*) FROM knife_source_binding_records",
                [],
                |row| row.get(0),
            )
            .expect("row count after tamper");
        assert_eq!(count, 0);
    }
}
