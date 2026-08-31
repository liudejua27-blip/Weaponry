//! Store boundary for the additive, source-only Knife UV/Cage/Bake aggregate.
//!
//! The existing Hero UV and Low durable records remain the authoritative
//! per-Part rows.  This table is only an immutable aggregate receipt: it
//! records the set of component links and the explicit High source proof so
//! a blade-body row can never be mistaken for a whole-blade result.  The
//! aggregate does not replace the formal seven-row High/Low/Cage/Bake
//! producer and never advances a production stage.

use forgecad_contracts::is_sha256;
use forgecad_core::{canonical_json_bytes, canonical_json_hash, sha256_hex};
use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{CasObjectRecord, Store, StoreError};

pub const SCHEMA_VERSION: &str = "WeaponryKnifeUvBakeV2Aggregate@1";
pub const RECORD_SCHEMA_VERSION: &str = "WeaponryKnifeUvBakeV2AggregateStoreRecord@1";
pub const RECEIPT_OBJECT_KIND: &str = "weaponry-knife-uv-bake-v2-aggregate-receipt@1";
pub const RECEIPT_MIME: &str = "application/json";
pub const MAX_COMPONENTS: usize = 32;
pub const MAX_JSON_BYTES: u64 = 8 * 1024 * 1024;
const TABLE: &str = "weaponry_knife_uv_bake_v2_aggregate_links";

/// The child is deliberately a value rather than a second copy of Low/UV
/// records.  Runtime validates its closed shape and each referenced record;
/// Store keeps the aggregate receipt lossless for later readback.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentRef {
    pub part_id: String,
    pub material_zone_id: String,
    pub source_high_part_id: String,
    pub source_high_node_id: String,
    pub source_high_material_zone_id: String,
    pub low_link_id: String,
    pub low_artifact_object_sha256: String,
    pub low_artifact_sha256: String,
    pub low_readback_object_sha256: String,
    pub low_readback_sha256: String,
    pub hero_uv_link_id: Option<String>,
    pub hero_uv_link_object_sha256: Option<String>,
    pub hero_uv_layout_object_sha256: Option<String>,
    pub cage_artifact_object_sha256: Option<String>,
    pub cage_artifact_sha256: Option<String>,
    pub cage_readback_object_sha256: Option<String>,
    pub cage_readback_sha256: Option<String>,
    pub bake_worker_result_object_sha256: Option<String>,
    pub bake_worker_result_sha256: Option<String>,
    pub bake_output_object_sha256s: Vec<String>,
    pub uv_status: String,
    pub cage_status: String,
    pub bake_status: String,
}

/// Immutable aggregate row.  `high_result_sha256` is the direct Worker
/// semantic hash; `high_artifact_sha256` and `high_artifact_object_sha256`
/// are the Runtime durable GLB hashes.  They intentionally remain separate
/// fields even when the current durable GLB policy makes the latter two
/// equal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AggregateRecord {
    pub schema_version: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub base_version_id: Option<String>,
    pub aggregate_id: String,
    pub source_high_artifact_id: String,
    pub source_high_result_sha256: String,
    pub source_high_result_object_sha256: String,
    pub source_high_readback_sha256: String,
    pub source_high_readback_object_sha256: String,
    pub source_high_artifact_sha256: String,
    pub source_high_artifact_object_sha256: String,
    /// Readback of the Runtime-owned durable High GLB.  This is deliberately
    /// separate from the direct Worker readback above: the two hashes are
    /// different identities even when both happen to describe the same
    /// source candidate.
    pub source_high_artifact_readback_sha256: String,
    pub source_high_artifact_readback_object_sha256: String,
    pub components: Vec<ComponentRef>,
    pub source_proof_sha256: String,
    pub uv_status: String,
    pub cage_status: String,
    pub bake_status: String,
    pub visual_status: String,
    pub human_status: String,
    pub engine_status: String,
    pub commercial_status: String,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub quality_status: String,
    pub request_sha256: String,
    pub input_sha256: String,
    pub idempotency_key: String,
    pub receipt_object_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

fn contract(code: &str, message: impl Into<String>) -> StoreError {
    StoreError::Contract {
        code: code.to_owned(),
        message: message.into(),
    }
}

fn value(record: &AggregateRecord) -> Result<Value, StoreError> {
    serde_json::to_value(record).map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn bytes(record: &AggregateRecord) -> Result<Vec<u8>, StoreError> {
    canonical_json_bytes(&value(record)?).map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn canonical(record: &AggregateRecord) -> Result<String, StoreError> {
    let mut value = value(record)?;
    value["canonical_sha256"] = Value::String(String::new());
    Ok(canonical_json_hash(&value))
}

fn validate_hashes(record: &AggregateRecord) -> Result<(), StoreError> {
    let hashes = [
        record.candidate_state_sha256.as_str(),
        record.source_high_result_sha256.as_str(),
        record.source_high_result_object_sha256.as_str(),
        record.source_high_readback_sha256.as_str(),
        record.source_high_readback_object_sha256.as_str(),
        record.source_high_artifact_sha256.as_str(),
        record.source_high_artifact_object_sha256.as_str(),
        record.source_high_artifact_readback_sha256.as_str(),
        record.source_high_artifact_readback_object_sha256.as_str(),
        record.source_proof_sha256.as_str(),
        record.request_sha256.as_str(),
        record.input_sha256.as_str(),
        record.receipt_object_sha256.as_str(),
        record.canonical_sha256.as_str(),
    ];
    if hashes.iter().any(|hash| !is_sha256(hash)) {
        return Err(contract("KNIFE_UV_BAKE_V2_HASH_INVALID", "aggregate hash is not SHA-256"));
    }
    for component in &record.components {
        let hashes = [
            component.low_artifact_object_sha256.as_str(),
            component.low_artifact_sha256.as_str(),
            component.low_readback_object_sha256.as_str(),
            component.low_readback_sha256.as_str(),
        ];
        if hashes.iter().any(|hash| !is_sha256(hash))
            || component
                .hero_uv_link_object_sha256
                .as_deref()
                .is_some_and(|hash| !is_sha256(hash))
            || component
                .hero_uv_layout_object_sha256
                .as_deref()
                .is_some_and(|hash| !is_sha256(hash))
            || component
                .cage_artifact_object_sha256
                .as_deref()
                .is_some_and(|hash| !is_sha256(hash))
            || component
                .cage_artifact_sha256
                .as_deref()
                .is_some_and(|hash| !is_sha256(hash))
            || component
                .cage_readback_object_sha256
                .as_deref()
                .is_some_and(|hash| !is_sha256(hash))
            || component
                .cage_readback_sha256
                .as_deref()
                .is_some_and(|hash| !is_sha256(hash))
            || component
                .bake_worker_result_object_sha256
                .as_deref()
                .is_some_and(|hash| !is_sha256(hash))
            || component
                .bake_worker_result_sha256
                .as_deref()
                .is_some_and(|hash| !is_sha256(hash))
            || component
                .bake_output_object_sha256s
                .iter()
                .any(|hash| !is_sha256(hash))
        {
            return Err(contract(
                "KNIFE_UV_BAKE_V2_COMPONENT_HASH_INVALID",
                "aggregate component hash is not SHA-256",
            ));
        }
    }
    Ok(())
}

fn validate_record(record: &AggregateRecord) -> Result<(), StoreError> {
    if record.schema_version != RECORD_SCHEMA_VERSION
        || record.project_id.is_empty()
        || record.candidate_id.is_empty()
        || record.aggregate_id.is_empty()
        || record.source_high_artifact_id.is_empty()
        || record.idempotency_key.is_empty()
        || record.created_at.is_empty()
        || record.components.len() < 2
        || record.components.len() > MAX_COMPONENTS
        || !record.runtime_write_performed
        || !record.persistent_user_data_touched
        || record.production_stage_advanced
        || record.candidate_confirmed
        || record.version_created
        || record.export_performed
        || record.visual_status != "NOT_PROVEN"
        || record.human_status != "NOT_RUN"
        || record.engine_status != "NOT_RUN"
        || record.commercial_status != "NOT_RUN"
        || record.quality_status != "structural_only"
        || record.receipt_object_sha256.is_empty()
    {
        return Err(contract(
            "KNIFE_UV_BAKE_V2_RECORD_INVALID",
            "aggregate identity, component count or status is invalid",
        ));
    }
    let mut parts = std::collections::BTreeSet::new();
    for component in &record.components {
        if component.part_id.is_empty()
            || component.material_zone_id.is_empty()
            || component.low_link_id.is_empty()
            || !parts.insert(component.part_id.as_str())
        {
            return Err(contract(
                "KNIFE_UV_BAKE_V2_COMPONENT_INVALID",
                "aggregate component Part identity is missing or duplicated",
            ));
        }
    }
    validate_hashes(record)?;
    if canonical(record)? != record.canonical_sha256 {
        return Err(contract(
            "KNIFE_UV_BAKE_V2_CANONICAL_MISMATCH",
            "aggregate canonical hash differs",
        ));
    }
    Ok(())
}

fn read_record(
    transaction: &rusqlite::Connection,
    project_id: &str,
    idempotency_key: &str,
) -> Result<Option<AggregateRecord>, StoreError> {
    transaction
        .query_row(
            &format!("SELECT record_json FROM {TABLE} WHERE project_id = ?1 AND idempotency_key = ?2"),
            params![project_id, idempotency_key],
            |row| {
                let json: String = row.get(0)?;
                serde_json::from_str(&json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        error.into(),
                    )
                })
            },
        )
        .optional()
        .map_err(StoreError::from)
}

pub fn ensure_table(transaction: &Transaction<'_>) -> Result<(), StoreError> {
    transaction.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {TABLE} (
            schema_version TEXT NOT NULL CHECK (schema_version = '{RECORD_SCHEMA_VERSION}'),
            project_id TEXT NOT NULL,
            candidate_id TEXT NOT NULL,
            candidate_state_sha256 TEXT NOT NULL,
            aggregate_id TEXT NOT NULL UNIQUE,
            source_high_artifact_id TEXT NOT NULL,
            source_high_artifact_sha256 TEXT NOT NULL,
            receipt_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
            idempotency_key TEXT NOT NULL,
            canonical_sha256 TEXT NOT NULL,
            created_at TEXT NOT NULL,
            record_json TEXT NOT NULL,
            PRIMARY KEY(project_id, idempotency_key)
        );
        CREATE INDEX IF NOT EXISTS weaponry_knife_uv_bake_v2_candidate_idx
            ON {TABLE}(project_id, candidate_id, created_at DESC);
        CREATE INDEX IF NOT EXISTS weaponry_knife_uv_bake_v2_receipt_idx
            ON {TABLE}(receipt_object_sha256);"
    ))?;
    Ok(())
}

pub fn commit(
    store: &Store,
    record: &AggregateRecord,
    receipt: &CasObjectRecord,
) -> Result<(AggregateRecord, bool), StoreError> {
    validate_record(record)?;
    if receipt.schema_version != "CasObject@1"
        || receipt.sha256 != record.receipt_object_sha256
        || receipt.mime != RECEIPT_MIME
        || receipt.kind != RECEIPT_OBJECT_KIND
        || receipt.size_bytes == 0
        || receipt.size_bytes > MAX_JSON_BYTES
    {
        return Err(contract(
            "KNIFE_UV_BAKE_V2_RECEIPT_METADATA_INVALID",
            "aggregate receipt CAS metadata differs",
        ));
    }
    let registered = store
        .get_object(&receipt.sha256)?
        .ok_or_else(|| contract("KNIFE_UV_BAKE_V2_RECEIPT_MISSING", "aggregate receipt CAS object is unavailable"))?;
    if registered.sha256 != receipt.sha256
        || registered.size_bytes != receipt.size_bytes
        || registered.mime != receipt.mime
        || registered.kind != receipt.kind
    {
        return Err(contract(
            "KNIFE_UV_BAKE_V2_RECEIPT_METADATA_INVALID",
            "registered aggregate receipt metadata differs",
        ));
    }
    let receipt_bytes = store
        .cas
        .read_verified_bounded(&receipt.sha256, MAX_JSON_BYTES)
        .map_err(StoreError::Cas)?;
    if receipt_bytes.len() as u64 != receipt.size_bytes
        || sha256_hex(&receipt_bytes) != receipt.sha256
    {
        return Err(contract(
            "KNIFE_UV_BAKE_V2_RECEIPT_HASH_MISMATCH",
            "aggregate receipt bytes do not match its CAS hash",
        ));
    }
    let mut connection = store.lock_connection()?;
    let transaction = connection.transaction()?;
    ensure_table(&transaction)?;
    let payload = String::from_utf8(bytes(record)?).map_err(|error| {
        StoreError::InvalidData(format!("aggregate record JSON is not UTF-8: {error}"))
    })?;
    if let Some(existing) = read_record(&transaction, &record.project_id, &record.idempotency_key)? {
        validate_record(&existing)?;
        if existing.input_sha256 != record.input_sha256 {
            return Err(contract(
                "KNIFE_UV_BAKE_V2_RECORD_CONFLICT",
                "aggregate idempotency key is bound to another input",
            ));
        }
        super::mark_reachable_in_transaction(&transaction, &object_roots(record))?;
        transaction.commit()?;
        return Ok((existing, true));
    }
    let conflict: Option<String> = transaction
        .query_row(
            &format!("SELECT aggregate_id FROM {TABLE} WHERE aggregate_id = ?1"),
            params![record.aggregate_id],
            |row| row.get(0),
        )
        .optional()?;
    if conflict.is_some() {
        return Err(contract(
            "KNIFE_UV_BAKE_V2_RECORD_CONFLICT",
            "aggregate identity is already bound",
        ));
    }
    transaction.execute(
        &format!(
            "INSERT INTO {TABLE} (schema_version, project_id, candidate_id, candidate_state_sha256, aggregate_id, source_high_artifact_id, source_high_artifact_sha256, receipt_object_sha256, idempotency_key, canonical_sha256, created_at, record_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"
        ),
        params![
            record.schema_version,
            record.project_id,
            record.candidate_id,
            record.candidate_state_sha256,
            record.aggregate_id,
            record.source_high_artifact_id,
            record.source_high_artifact_sha256,
            record.receipt_object_sha256,
            record.idempotency_key,
            record.canonical_sha256,
            record.created_at,
            payload,
        ],
    )?;
    super::mark_reachable_in_transaction(&transaction, &object_roots(record))?;
    transaction.commit()?;
    Ok((record.clone(), false))
}

/// Every child object is promoted together with the aggregate receipt.  The
/// aggregate row is intentionally only a compact index, so GC must not rely
/// on its SQL columns alone to discover the per-Part roots.
fn object_roots(record: &AggregateRecord) -> Vec<String> {
    let mut roots = vec![
        record.source_high_result_object_sha256.clone(),
        record.source_high_readback_object_sha256.clone(),
        record.source_high_artifact_object_sha256.clone(),
        record.source_high_artifact_readback_object_sha256.clone(),
        record.receipt_object_sha256.clone(),
    ];
    for component in &record.components {
        roots.extend([
            component.low_artifact_object_sha256.clone(),
            component.low_readback_object_sha256.clone(),
        ]);
        for hash in [
            component.hero_uv_link_object_sha256.as_ref(),
            component.hero_uv_layout_object_sha256.as_ref(),
            component.cage_artifact_object_sha256.as_ref(),
            component.cage_readback_object_sha256.as_ref(),
            component.bake_worker_result_object_sha256.as_ref(),
        ] {
            if let Some(hash) = hash {
                roots.push(hash.clone());
            }
        }
        roots.extend(component.bake_output_object_sha256s.iter().cloned());
    }
    roots.sort();
    roots.dedup();
    roots
}

pub fn get(
    store: &Store,
    project_id: &str,
    idempotency_key: &str,
) -> Result<Option<AggregateRecord>, StoreError> {
    let connection = store.lock_connection()?;
    let record = read_record(&connection, project_id, idempotency_key)?;
    drop(connection);
    if let Some(record) = &record {
        validate_record(record)?;
        let object = store
            .get_object(&record.receipt_object_sha256)?
            .ok_or_else(|| contract("KNIFE_UV_BAKE_V2_RECEIPT_MISSING", "aggregate receipt CAS object is unavailable"))?;
        if object.kind != RECEIPT_OBJECT_KIND || object.mime != RECEIPT_MIME {
            return Err(contract("KNIFE_UV_BAKE_V2_RECEIPT_METADATA_INVALID", "aggregate receipt CAS metadata differs"));
        }
        let bytes = store
            .cas
            .read_verified_bounded(&record.receipt_object_sha256, MAX_JSON_BYTES)
            .map_err(StoreError::Cas)?;
        if sha256_hex(&bytes) != record.receipt_object_sha256 {
            return Err(contract("KNIFE_UV_BAKE_V2_RECEIPT_HASH_MISMATCH", "aggregate receipt CAS hash differs"));
        }
        let receipt: Value = serde_json::from_slice(&bytes).map_err(|error| {
            StoreError::InvalidData(format!("aggregate receipt JSON is invalid: {error}"))
        })?;
        if receipt.get("schema_version").and_then(Value::as_str)
                != Some("WeaponryKnifeUvBakeV2AggregateReceipt@1")
            || receipt.get("aggregate_id").and_then(Value::as_str)
                != Some(record.aggregate_id.as_str())
            || receipt.get("project_id").and_then(Value::as_str)
                != Some(record.project_id.as_str())
            || receipt.get("candidate_id").and_then(Value::as_str)
                != Some(record.candidate_id.as_str())
            || receipt.get("request_sha256").and_then(Value::as_str)
                != Some(record.request_sha256.as_str())
        {
            return Err(contract(
                "KNIFE_UV_BAKE_V2_RECEIPT_BINDING_MISMATCH",
                "aggregate receipt does not bind its record",
            ));
        }
        let mut receipt_preimage = receipt.clone();
        receipt_preimage["canonical_sha256"] = Value::String(String::new());
        if canonical_json_hash(&receipt_preimage)
            != receipt.get("canonical_sha256").and_then(Value::as_str).unwrap_or_default()
        {
            return Err(contract(
                "KNIFE_UV_BAKE_V2_RECEIPT_CANONICAL_MISMATCH",
                "aggregate receipt canonical hash differs",
            ));
        }
    }
    Ok(record)
}

impl Store {
    /// Runtime-facing façade for the additive aggregate.  Per-Part Low and
    /// Hero UV rows remain authoritative; this method only commits their
    /// immutable receipt and promotes all referenced CAS roots.
    pub fn weaponry_knife_uv_bake_v2_commit(
        &self,
        record: &AggregateRecord,
        receipt: &CasObjectRecord,
    ) -> Result<(AggregateRecord, bool), StoreError> {
        commit(self, record, receipt)
    }

    /// Read the aggregate receipt and validate its self-contained binding.
    /// Runtime performs the child-record/CAS revalidation before returning a
    /// successful façade response.
    pub fn weaponry_knife_uv_bake_v2_get(
        &self,
        project_id: &str,
        idempotency_key: &str,
    ) -> Result<Option<AggregateRecord>, StoreError> {
        get(self, project_id, idempotency_key)
    }
}
