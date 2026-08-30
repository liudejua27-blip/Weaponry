//! Runtime-owned persistence for one bounded AuthoringMesh@2 transaction.
//!
//! The kernel is deliberately kept in `forgecad-runtime`.  This module owns
//! only the durable receipt/index and the atomic Store transaction which
//! installs the complete immutable revision chain.  CAS files are staged by
//! Runtime under a reservation; this module never trusts a caller supplied
//! digest and re-verifies every registered object before committing SQLite.

use super::*;

pub const AUTHORING_MESH_V2_TRANSACTION_RECORD_SCHEMA_VERSION: &str =
    "AuthoringMeshV2TransactionDurableRecord@1";
pub const AUTHORING_MESH_V2_TRANSACTION_PAYLOAD_SCHEMA_VERSION: &str = "AuthoringMeshTransaction@1";
pub const AUTHORING_MESH_V2_TRANSACTION_OBJECT_KIND: &str = "authoring-mesh-v2-transaction";
pub const AUTHORING_MESH_V2_TRANSACTION_STATUS: &str =
    "runtime-owned-store-authoring-mesh-v2-transaction@1";
pub const AUTHORING_MESH_V2_TRANSACTION_MAX_COMMANDS: usize = 32;
pub const AUTHORING_MESH_V2_TRANSACTION_MAX_BYTES: u64 = 8 * 1024 * 1024;

/// Store-local durable receipt.  The full immutable command journal remains
/// in `transaction_object_sha256`; this row is the restart-safe lookup and
/// binding index.  None of the hashes are accepted as truth without CAS
/// re-verification in `record_*`/`get_*`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthoringMeshV2TransactionDurableRecord {
    pub schema_version: String,
    pub project_id: String,
    pub transaction_id: String,
    pub mesh_id: String,
    pub lineage_id: String,
    pub base_revision_id: String,
    pub base_revision_index: u64,
    pub base_revision_sha256: String,
    pub final_revision_id: String,
    pub final_revision_index: u64,
    pub final_revision_sha256: String,
    pub final_revision_object_sha256: String,
    pub transaction_sha256: String,
    pub transaction_object_sha256: String,
    pub revision_ids: Vec<String>,
    pub revision_sha256s: Vec<String>,
    pub revision_object_sha256s: Vec<String>,
    pub operation_ids: Vec<String>,
    pub request_input_sha256: String,
    pub idempotency_key: String,
    pub materialization_status: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Canonical JSON persisted in the transaction CAS object.  It intentionally
/// contains command identity and output bindings, but not a second copy of
/// topology.  Topology is authoritative only in the immutable revision CAS
/// objects, and command inputs are bound by `request_input_sha256`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthoringMeshV2TransactionPayload {
    pub schema_version: String,
    pub transaction_id: String,
    pub mesh_id: String,
    pub lineage_id: String,
    pub base_revision_id: String,
    pub base_revision_index: u64,
    pub base_revision_sha256: String,
    pub commands: Vec<Value>,
    pub budgets: Value,
    pub execution_policy: Value,
    pub canonicalization_policy: String,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone)]
pub struct AuthoringMeshV2TransactionRevisionInput {
    pub record: AuthoringMeshV2DurableRecord,
    pub revision: AuthoringMeshRevision,
    pub object: CasObjectRecord,
}

#[derive(Debug, Clone)]
pub struct AuthoringMeshV2TransactionCommit {
    pub record: AuthoringMeshV2TransactionDurableRecord,
    pub payload: AuthoringMeshV2TransactionPayload,
    pub transaction_object: CasObjectRecord,
    pub revisions: Vec<AuthoringMeshV2TransactionRevisionInput>,
}

fn transaction_contract(code: &str, message: impl Into<String>) -> StoreError {
    StoreError::Contract {
        code: code.to_owned(),
        message: message.into(),
    }
}

fn transaction_record_canonical_sha256(
    record: &AuthoringMeshV2TransactionDurableRecord,
) -> Result<String, StoreError> {
    let mut value =
        serde_json::to_value(record).map_err(|error| StoreError::InvalidData(error.to_string()))?;
    value["canonical_sha256"] = Value::String(String::new());
    Ok(canonical_json_hash(&value))
}

fn transaction_payload_canonical_sha256(
    payload: &AuthoringMeshV2TransactionPayload,
) -> Result<String, StoreError> {
    let mut value = serde_json::to_value(payload)
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    value["canonical_sha256"] = Value::String(String::new());
    Ok(canonical_json_hash(&value))
}

fn validate_transaction_identity(
    schema_version: &str,
    project_id: &str,
    transaction_id: &str,
    mesh_id: &str,
    lineage_id: &str,
    base_revision_id: &str,
    base_revision_sha256: &str,
    final_revision_id: &str,
    final_revision_sha256: &str,
    request_input_sha256: &str,
    idempotency_key: &str,
) -> Result<(), StoreError> {
    if schema_version != AUTHORING_MESH_V2_TRANSACTION_RECORD_SCHEMA_VERSION
        || !is_opaque_id(project_id)
        || !is_opaque_id(transaction_id)
        || !is_opaque_id(mesh_id)
        || !is_opaque_id(lineage_id)
        || !is_opaque_id(base_revision_id)
        || !is_opaque_id(final_revision_id)
        || !is_sha256(base_revision_sha256)
        || !is_sha256(final_revision_sha256)
        || !is_sha256(request_input_sha256)
        || !is_opaque_id(idempotency_key)
        || idempotency_key.len() > 128
    {
        return Err(transaction_contract(
            "AUTHORING_TRANSACTION_INVALID",
            "transaction durable identity is malformed",
        ));
    }
    Ok(())
}

fn validate_transaction_record(
    record: &AuthoringMeshV2TransactionDurableRecord,
) -> Result<(), StoreError> {
    validate_transaction_identity(
        &record.schema_version,
        &record.project_id,
        &record.transaction_id,
        &record.mesh_id,
        &record.lineage_id,
        &record.base_revision_id,
        &record.base_revision_sha256,
        &record.final_revision_id,
        &record.final_revision_sha256,
        &record.request_input_sha256,
        &record.idempotency_key,
    )?;
    let lengths = [
        record.revision_ids.len(),
        record.revision_sha256s.len(),
        record.revision_object_sha256s.len(),
        record.operation_ids.len(),
    ];
    if lengths
        .iter()
        .any(|length| *length == 0 || *length > AUTHORING_MESH_V2_TRANSACTION_MAX_COMMANDS)
        || lengths.iter().any(|length| *length != lengths[0])
        || record.base_revision_index > 1_000_000
        || record.final_revision_index > 1_000_000
        || record.final_revision_index != record.base_revision_index + lengths[0] as u64
        || !is_sha256(&record.transaction_sha256)
        || !is_sha256(&record.transaction_object_sha256)
        || record.revision_ids.iter().any(|id| !is_opaque_id(id))
        || record.revision_sha256s.iter().any(|hash| !is_sha256(hash))
        || record
            .revision_object_sha256s
            .iter()
            .any(|hash| !is_sha256(hash))
        || record.operation_ids.iter().any(|id| !is_opaque_id(id))
        || record.materialization_status != AUTHORING_MESH_V2_TRANSACTION_STATUS
        || !is_sha256(&record.canonical_sha256)
        || record.created_at.is_empty()
        || record.created_at.len() > 64
    {
        return Err(transaction_contract(
            "AUTHORING_TRANSACTION_INVALID",
            "transaction durable record is malformed or exceeds its bound",
        ));
    }
    let expected = transaction_record_canonical_sha256(record)?;
    if expected != record.canonical_sha256 {
        return Err(transaction_contract(
            "AUTHORING_TRANSACTION_CORRUPT",
            "transaction durable record canonical hash is invalid",
        ));
    }
    Ok(())
}

fn validate_transaction_payload(
    payload: &AuthoringMeshV2TransactionPayload,
) -> Result<(), StoreError> {
    if payload.schema_version != AUTHORING_MESH_V2_TRANSACTION_PAYLOAD_SCHEMA_VERSION
        || !is_opaque_id(&payload.transaction_id)
        || !is_opaque_id(&payload.mesh_id)
        || !is_opaque_id(&payload.lineage_id)
        || !is_opaque_id(&payload.base_revision_id)
        || !is_sha256(&payload.base_revision_sha256)
        || payload.base_revision_index > 1_000_000
        || payload.commands.is_empty()
        || payload.commands.len() > AUTHORING_MESH_V2_TRANSACTION_MAX_COMMANDS
        || payload.canonicalization_policy != "canonical-json-sha256-excluding-canonical-sha256@1"
        || !is_sha256(&payload.canonical_sha256)
    {
        return Err(transaction_contract(
            "AUTHORING_TRANSACTION_INVALID",
            "transaction CAS payload is malformed",
        ));
    }
    let expected = transaction_payload_canonical_sha256(payload)?;
    if expected != payload.canonical_sha256 {
        return Err(transaction_contract(
            "AUTHORING_TRANSACTION_CORRUPT",
            "transaction CAS payload canonical hash is invalid",
        ));
    }
    Ok(())
}

fn same_transaction_record(
    left: &AuthoringMeshV2TransactionDurableRecord,
    right: &AuthoringMeshV2TransactionDurableRecord,
) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    left.created_at.clear();
    right.created_at.clear();
    left.canonical_sha256.clear();
    right.canonical_sha256.clear();
    left == right
}

fn validate_transaction_object(
    cas: &CasStore,
    object: &CasObjectRecord,
    expected_sha256: &str,
    require_reachable: bool,
) -> Result<Vec<u8>, StoreError> {
    if object.schema_version != "CasObject@1"
        || object.sha256 != expected_sha256
        || !is_sha256(&object.sha256)
        || object.mime != "application/json"
        || object.kind != AUTHORING_MESH_V2_TRANSACTION_OBJECT_KIND
        || object.size_bytes == 0
        || object.size_bytes > AUTHORING_MESH_V2_TRANSACTION_MAX_BYTES
        || !matches!(object.reachability.as_str(), "temporary" | "reachable")
        || (require_reachable && object.reachability != "reachable")
        || object.created_at.is_empty()
        || object.created_at.len() > 64
    {
        return Err(transaction_contract(
            "AUTHORING_TRANSACTION_CAS_FAILED",
            "transaction CAS metadata is invalid",
        ));
    }
    cas.read_verified_bounded(expected_sha256, AUTHORING_MESH_V2_TRANSACTION_MAX_BYTES)
        .map_err(StoreError::from)
}

fn ensure_transaction_object_row(
    transaction: &rusqlite::Transaction<'_>,
    object: &CasObjectRecord,
    require_reachable: bool,
) -> Result<(), StoreError> {
    let stored: Option<(i64, String, String, String)> = transaction
        .query_row(
            "SELECT size_bytes, mime, kind, reachability FROM objects WHERE sha256 = ?1",
            params![object.sha256],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    let Some((size, mime, kind, reachability)) = stored else {
        return Err(transaction_contract(
            "AUTHORING_TRANSACTION_CAS_FAILED",
            "transaction CAS object is not registered",
        ));
    };
    if size != i64::try_from(object.size_bytes).unwrap_or(i64::MAX)
        || mime != object.mime
        || kind != object.kind
        || !matches!(reachability.as_str(), "temporary" | "reachable")
        || (require_reachable && reachability != "reachable")
    {
        return Err(transaction_contract(
            "AUTHORING_TRANSACTION_CAS_FAILED",
            "transaction CAS metadata differs from SQLite",
        ));
    }
    Ok(())
}

fn read_transaction_record_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    project_id: &str,
    idempotency_key: &str,
) -> Result<Option<AuthoringMeshV2TransactionDurableRecord>, StoreError> {
    let record_json: Option<String> = transaction
        .query_row(
            "SELECT record_json FROM authoring_mesh_v2_transactions WHERE project_id = ?1 AND idempotency_key = ?2",
            params![project_id, idempotency_key],
            |row| row.get(0),
        )
        .optional()?;
    let Some(record_json) = record_json else {
        return Ok(None);
    };
    let record = serde_json::from_str(&record_json)
        .map_err(|error| StoreError::InvalidData(format!("transaction record JSON: {error}")))?;
    Ok(Some(record))
}

fn validate_registered_object(
    transaction: &rusqlite::Transaction<'_>,
    object: &CasObjectRecord,
    require_reachable: bool,
) -> Result<(), StoreError> {
    if object.kind == AUTHORING_MESH_V2_TRANSACTION_OBJECT_KIND {
        ensure_transaction_object_row(transaction, object, require_reachable)
    } else {
        ensure_authoring_mesh_v2_object_row(transaction, object, require_reachable)
    }
}

fn operation_kind(revision: &AuthoringMeshRevision) -> Option<&'static str> {
    revision
        .operation
        .as_ref()
        .map(|operation| match operation.kind {
            AuthoringMeshTopologyOperationKind::SplitEdge => "split_edge",
            AuthoringMeshTopologyOperationKind::FaceExtrude => "face_extrude",
            AuthoringMeshTopologyOperationKind::MoveVertices => "move_vertices",
            AuthoringMeshTopologyOperationKind::OpenFrameNotch => "open_frame_notch",
            AuthoringMeshTopologyOperationKind::RearStockVoidRailBow => "rear_stock_void_rail_bow",
            AuthoringMeshTopologyOperationKind::RearStockVoidBoundaryBridge => {
                "rear_stock_void_boundary_bridge"
            }
        })
}

fn validate_revision_input(
    transaction: &rusqlite::Transaction<'_>,
    cas: &CasStore,
    input: &AuthoringMeshV2TransactionRevisionInput,
    parent_revision_id: &str,
    parent_revision_index: u64,
    batch: &AuthoringMeshV2TransactionCommit,
    index: usize,
    require_reachable: bool,
) -> Result<(), StoreError> {
    let record = &input.record;
    validate_authoring_mesh_v2_durable_record(record)?;
    if record.project_id != batch.record.project_id
        || record.mesh_id != batch.record.mesh_id
        || record.lineage_id != batch.record.lineage_id
        || record.revision_id != batch.record.revision_ids[index]
        || record.revision_sha256 != batch.record.revision_sha256s[index]
        || record.revision_object_sha256 != batch.record.revision_object_sha256s[index]
        || record.operation_id.as_deref() != Some(batch.record.operation_ids[index].as_str())
        || record.parent_revision_ids != vec![parent_revision_id.to_owned()]
        || record.revision_index != parent_revision_index + 1
        || record.request_input_sha256 != batch.record.request_input_sha256
        || record.idempotency_key != format!("{}-revision-{}", batch.record.idempotency_key, index)
    {
        return Err(transaction_contract(
            "AUTHORING_TRANSACTION_CORRUPT",
            "revision chain binding differs from transaction receipt",
        ));
    }
    if input.revision.mesh_id.0 != record.mesh_id
        || input.revision.lineage_id.0 != record.lineage_id
        || input.revision.revision_id.0 != record.revision_id
        || input.revision.revision_index != record.revision_index
        || input
            .revision
            .parent_revision_ids
            .iter()
            .map(|id| id.0.clone())
            .collect::<Vec<_>>()
            != record.parent_revision_ids
        || input.revision.canonical_sha256 != record.revision_sha256
        || input
            .revision
            .operation
            .as_ref()
            .map(|operation| operation.operation_id.as_str())
            != record.operation_id.as_deref()
        || operation_kind(&input.revision) != record.operation_kind.as_deref()
        || input
            .revision
            .operation
            .as_ref()
            .map(|operation| operation.operation_lineage_sha256.as_str())
            != record.operation_lineage_sha256.as_deref()
    {
        return Err(transaction_contract(
            "AUTHORING_TRANSACTION_CORRUPT",
            "typed revision differs from its durable binding",
        ));
    }
    if input.object.sha256 != record.revision_object_sha256 {
        return Err(transaction_contract(
            "AUTHORING_TRANSACTION_CAS_FAILED",
            "revision object hash differs from durable binding",
        ));
    }
    let bytes = validate_authoring_mesh_v2_revision_object(
        cas,
        &input.object,
        &record.revision_object_sha256,
        require_reachable,
    )?;
    validate_authoring_mesh_v2_revision_payload(&bytes, &input.revision, record)?;
    validate_registered_object(transaction, &input.object, require_reachable)
}

fn validate_transaction_binding(
    batch: &AuthoringMeshV2TransactionCommit,
    payload_bytes: &[u8],
) -> Result<(), StoreError> {
    validate_transaction_record(&batch.record)?;
    validate_transaction_payload(&batch.payload)?;
    let expected_payload_bytes = canonical_json_bytes(
        &serde_json::to_value(&batch.payload)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?,
    )
    .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    if expected_payload_bytes != payload_bytes
        || sha256_hex(payload_bytes) != batch.record.transaction_object_sha256
        || batch.payload.transaction_id != batch.record.transaction_id
        || batch.payload.mesh_id != batch.record.mesh_id
        || batch.payload.lineage_id != batch.record.lineage_id
        || batch.payload.base_revision_id != batch.record.base_revision_id
        || batch.payload.base_revision_index != batch.record.base_revision_index
        || batch.payload.base_revision_sha256 != batch.record.base_revision_sha256
        || batch.payload.canonical_sha256 != batch.record.transaction_sha256
    {
        return Err(transaction_contract(
            "AUTHORING_TRANSACTION_CORRUPT",
            "transaction payload and durable receipt disagree",
        ));
    }
    Ok(())
}

/// Ensure the additive migration exists.  Keeping this in the module makes
/// the table available to both file-backed restart probes and Store::memory.
pub(crate) fn ensure_schema(transaction: &rusqlite::Transaction<'_>) -> Result<(), StoreError> {
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS authoring_mesh_v2_transactions (
             schema_version TEXT NOT NULL CHECK (schema_version = 'AuthoringMeshV2TransactionDurableRecord@1'),
             project_id TEXT NOT NULL REFERENCES projects(project_id),
             transaction_id TEXT NOT NULL,
             mesh_id TEXT NOT NULL,
             lineage_id TEXT NOT NULL,
             base_revision_id TEXT NOT NULL,
             base_revision_index INTEGER NOT NULL CHECK (base_revision_index >= 0 AND base_revision_index <= 1000000),
             base_revision_sha256 TEXT NOT NULL,
             final_revision_id TEXT NOT NULL,
             final_revision_index INTEGER NOT NULL CHECK (final_revision_index >= 0 AND final_revision_index <= 1000000),
             final_revision_sha256 TEXT NOT NULL,
             final_revision_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             transaction_sha256 TEXT NOT NULL,
             transaction_object_sha256 TEXT NOT NULL UNIQUE REFERENCES objects(sha256),
             revision_ids_json TEXT NOT NULL,
             revision_sha256s_json TEXT NOT NULL,
             revision_object_sha256s_json TEXT NOT NULL,
             operation_ids_json TEXT NOT NULL,
             request_input_sha256 TEXT NOT NULL,
             idempotency_key TEXT NOT NULL,
             materialization_status TEXT NOT NULL CHECK (materialization_status = 'runtime-owned-store-authoring-mesh-v2-transaction@1'),
             canonical_sha256 TEXT NOT NULL,
             record_json TEXT NOT NULL,
             created_at TEXT NOT NULL,
             PRIMARY KEY (project_id, idempotency_key),
             UNIQUE (project_id, transaction_id),
             UNIQUE (project_id, final_revision_id)
         );
         CREATE INDEX IF NOT EXISTS authoring_mesh_v2_transactions_lineage_idx
             ON authoring_mesh_v2_transactions(project_id, lineage_id, final_revision_index);
         CREATE INDEX IF NOT EXISTS authoring_mesh_v2_transactions_object_idx
             ON authoring_mesh_v2_transactions(transaction_object_sha256, final_revision_object_sha256);",
    )?;
    Ok(())
}

impl Store {
    /// Persist all immutable child revisions and the aggregate receipt in one
    /// SQLite transaction.  A caller may have staged several CAS files, but
    /// no Store row or reachable root is visible until every validation and
    /// the final readback have succeeded.
    pub fn record_authoring_mesh_v2_transaction_with_replay(
        &self,
        batch: &AuthoringMeshV2TransactionCommit,
    ) -> Result<(AuthoringMeshV2TransactionDurableRecord, bool), StoreError> {
        let transaction_bytes = validate_transaction_object(
            &self.cas,
            &batch.transaction_object,
            &batch.record.transaction_object_sha256,
            false,
        )?;
        validate_transaction_binding(batch, &transaction_bytes)?;
        if batch.revisions.len() != batch.record.revision_ids.len() {
            return Err(transaction_contract(
                "AUTHORING_TRANSACTION_INVALID",
                "revision input count differs from transaction chain",
            ));
        }

        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let existing = read_transaction_record_in_transaction(
            &transaction,
            &batch.record.project_id,
            &batch.record.idempotency_key,
        )?;
        if let Some(existing) = existing {
            validate_transaction_record(&existing)?;
            if !same_transaction_record(&existing, &batch.record) {
                return Err(transaction_contract(
                    "AUTHORING_TRANSACTION_IDEMPOTENCY_CONFLICT",
                    "idempotency key is already bound to different transaction input",
                ));
            }
            let stored_object: CasObjectRecord = transaction
                .query_row(
                    "SELECT sha256, size_bytes, mime, kind, reachability, created_at FROM objects WHERE sha256 = ?1",
                    params![existing.transaction_object_sha256],
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
                .map_err(StoreError::from)?;
            let stored_bytes = validate_transaction_object(
                &self.cas,
                &stored_object,
                &existing.transaction_object_sha256,
                true,
            )?;
            if sha256_hex(&stored_bytes) != existing.transaction_object_sha256 {
                return Err(transaction_contract(
                    "AUTHORING_TRANSACTION_CORRUPT",
                    "stored transaction object hash does not match receipt",
                ));
            }
            transaction.commit()?;
            return Ok((existing, true));
        }

        ensure_transaction_object_row(&transaction, &batch.transaction_object, false)?;
        let base: Option<(String, String, u64, String, String)> = transaction
            .query_row(
                "SELECT mesh_id, lineage_id, revision_index, revision_sha256, idempotency_key FROM authoring_mesh_v2_durable_records WHERE project_id = ?1 AND revision_id = ?2",
                params![batch.record.project_id, batch.record.base_revision_id],
                |row| {
                    let index: i64 = row.get(2)?;
                    Ok((row.get(0)?, row.get(1)?, u64::try_from(index).map_err(|_| rusqlite::Error::InvalidQuery)?, row.get(3)?, row.get(4)?))
                },
            )
            .optional()?;
        let Some((base_mesh_id, base_lineage_id, base_index, base_sha256, base_key)) = base else {
            return Err(transaction_contract(
                "AUTHORING_TRANSACTION_BASE_REVISION_MISMATCH",
                "base revision is not durably materialized",
            ));
        };
        if base_mesh_id != batch.record.mesh_id
            || base_lineage_id != batch.record.lineage_id
            || base_index != batch.record.base_revision_index
            || base_sha256 != batch.record.base_revision_sha256
        {
            return Err(transaction_contract(
                "AUTHORING_TRANSACTION_BASE_REVISION_MISMATCH",
                "base revision identity differs from the transaction",
            ));
        }
        let base_record = read_authoring_mesh_v2_record_in_transaction(
            &transaction,
            &batch.record.project_id,
            &base_key,
        )?
        .ok_or_else(|| {
            transaction_contract(
                "AUTHORING_TRANSACTION_BASE_REVISION_MISMATCH",
                "base revision durable row cannot be read back",
            )
        })?;
        validate_authoring_mesh_v2_record_in_transaction(
            &transaction,
            &self.cas,
            &base_record,
            true,
        )?;

        let mut previous_id = batch.record.base_revision_id.clone();
        let mut previous_index = batch.record.base_revision_index;
        for (index, input) in batch.revisions.iter().enumerate() {
            validate_revision_input(
                &transaction,
                &self.cas,
                input,
                &previous_id,
                previous_index,
                batch,
                index,
                false,
            )?;
            previous_id = input.record.revision_id.clone();
            previous_index = input.record.revision_index;
        }

        let parent_ids_json = |record: &AuthoringMeshV2DurableRecord| {
            serde_json::to_string(&record.parent_revision_ids)
                .map_err(|error| StoreError::InvalidData(error.to_string()))
        };
        for input in &batch.revisions {
            let record = &input.record;
            let duplicate: Option<String> = transaction
                .query_row(
                    "SELECT idempotency_key FROM authoring_mesh_v2_durable_records WHERE project_id = ?1 AND revision_id = ?2",
                    params![record.project_id, record.revision_id],
                    |row| row.get(0),
                )
                .optional()?;
            if duplicate.is_some() {
                return Err(transaction_contract(
                    "AUTHORING_TRANSACTION_IDEMPOTENCY_CONFLICT",
                    "a revision in this transaction is already durably bound",
                ));
            }
            validate_authoring_mesh_v2_parent_dag_in_transaction(&transaction, record)?;
            transaction.execute(
                "INSERT INTO authoring_mesh_v2_durable_records (schema_version, project_id, mesh_id, lineage_id, revision_id, parent_revision_ids_json, revision_index, revision_object_sha256, revision_sha256, operation_id, operation_kind, operation_lineage_sha256, request_input_sha256, idempotency_key, materialization_status, canonical_sha256, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
                params![
                    record.schema_version,
                    record.project_id,
                    record.mesh_id,
                    record.lineage_id,
                    record.revision_id,
                    parent_ids_json(record)?,
                    i64::try_from(record.revision_index).map_err(|_| StoreError::InvalidData("revision index too large".to_owned()))?,
                    record.revision_object_sha256,
                    record.revision_sha256,
                    record.operation_id,
                    record.operation_kind,
                    record.operation_lineage_sha256,
                    record.request_input_sha256,
                    record.idempotency_key,
                    record.materialization_status,
                    record.canonical_sha256,
                    record.created_at,
                ],
            )?;
        }
        let record_json = serde_json::to_string(&batch.record)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        transaction.execute(
            "INSERT INTO authoring_mesh_v2_transactions (schema_version, project_id, transaction_id, mesh_id, lineage_id, base_revision_id, base_revision_index, base_revision_sha256, final_revision_id, final_revision_index, final_revision_sha256, final_revision_object_sha256, transaction_sha256, transaction_object_sha256, revision_ids_json, revision_sha256s_json, revision_object_sha256s_json, operation_ids_json, request_input_sha256, idempotency_key, materialization_status, canonical_sha256, record_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24)",
            params![
                batch.record.schema_version,
                batch.record.project_id,
                batch.record.transaction_id,
                batch.record.mesh_id,
                batch.record.lineage_id,
                batch.record.base_revision_id,
                i64::try_from(batch.record.base_revision_index).map_err(|_| StoreError::InvalidData("base revision index too large".to_owned()))?,
                batch.record.base_revision_sha256,
                batch.record.final_revision_id,
                i64::try_from(batch.record.final_revision_index).map_err(|_| StoreError::InvalidData("final revision index too large".to_owned()))?,
                batch.record.final_revision_sha256,
                batch.record.final_revision_object_sha256,
                batch.record.transaction_sha256,
                batch.record.transaction_object_sha256,
                serde_json::to_string(&batch.record.revision_ids).map_err(|error| StoreError::InvalidData(error.to_string()))?,
                serde_json::to_string(&batch.record.revision_sha256s).map_err(|error| StoreError::InvalidData(error.to_string()))?,
                serde_json::to_string(&batch.record.revision_object_sha256s).map_err(|error| StoreError::InvalidData(error.to_string()))?,
                serde_json::to_string(&batch.record.operation_ids).map_err(|error| StoreError::InvalidData(error.to_string()))?,
                batch.record.request_input_sha256,
                batch.record.idempotency_key,
                batch.record.materialization_status,
                batch.record.canonical_sha256,
                record_json,
                batch.record.created_at,
            ],
        )?;

        let mut reachable = vec![batch.record.transaction_object_sha256.clone()];
        reachable.extend(batch.record.revision_object_sha256s.iter().cloned());
        reachable.sort();
        reachable.dedup();
        mark_reachable_in_transaction(&transaction, &reachable)?;
        let stored = read_transaction_record_in_transaction(
            &transaction,
            &batch.record.project_id,
            &batch.record.idempotency_key,
        )?
        .ok_or_else(|| {
            transaction_contract(
                "AUTHORING_TRANSACTION_CORRUPT",
                "transaction receipt disappeared before commit",
            )
        })?;
        validate_transaction_record(&stored)?;
        transaction.commit()?;
        Ok((stored, false))
    }

    pub fn get_authoring_mesh_v2_transaction(
        &self,
        project_id: &str,
        idempotency_key: &str,
    ) -> Result<Option<AuthoringMeshV2TransactionDurableRecord>, StoreError> {
        if !is_opaque_id(project_id) || !is_opaque_id(idempotency_key) {
            return Err(StoreError::InvalidData(
                "transaction lookup identity is invalid".to_owned(),
            ));
        }
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let Some(record) =
            read_transaction_record_in_transaction(&transaction, project_id, idempotency_key)?
        else {
            transaction.commit()?;
            return Ok(None);
        };
        validate_transaction_record(&record)?;
        let object: CasObjectRecord = transaction
            .query_row(
                "SELECT sha256, size_bytes, mime, kind, reachability, created_at FROM objects WHERE sha256 = ?1",
                params![record.transaction_object_sha256],
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
            )?;
        let bytes = validate_transaction_object(
            &self.cas,
            &object,
            &record.transaction_object_sha256,
            true,
        )?;
        if sha256_hex(&bytes) != record.transaction_object_sha256 {
            return Err(transaction_contract(
                "AUTHORING_TRANSACTION_CORRUPT",
                "stored transaction object hash differs from receipt",
            ));
        }
        for object_sha256 in &record.revision_object_sha256s {
            let revision_object: CasObjectRecord = transaction
                .query_row(
                    "SELECT sha256, size_bytes, mime, kind, reachability, created_at FROM objects WHERE sha256 = ?1",
                    params![object_sha256],
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
                )?;
            let bytes = validate_authoring_mesh_v2_revision_object(
                &self.cas,
                &revision_object,
                object_sha256,
                true,
            )?;
            let revision: AuthoringMeshRevision = serde_json::from_slice(&bytes)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?;
            let record_for_revision = AuthoringMeshV2DurableRecord {
                schema_version: AUTHORING_MESH_V2_DURABLE_RECORD_SCHEMA_VERSION.to_owned(),
                project_id: record.project_id.clone(),
                mesh_id: revision.mesh_id.0.clone(),
                lineage_id: revision.lineage_id.0.clone(),
                revision_id: revision.revision_id.0.clone(),
                parent_revision_ids: revision
                    .parent_revision_ids
                    .iter()
                    .map(|id| id.0.clone())
                    .collect(),
                revision_index: revision.revision_index,
                revision_object_sha256: object_sha256.clone(),
                revision_sha256: revision.canonical_sha256.clone(),
                operation_id: revision
                    .operation
                    .as_ref()
                    .map(|operation| operation.operation_id.clone()),
                operation_kind: operation_kind(&revision).map(str::to_owned),
                operation_lineage_sha256: revision
                    .operation
                    .as_ref()
                    .map(|operation| operation.operation_lineage_sha256.clone()),
                request_input_sha256: record.request_input_sha256.clone(),
                idempotency_key: format!(
                    "{}-revision-{}",
                    record.idempotency_key,
                    record
                        .revision_object_sha256s
                        .iter()
                        .position(|hash| hash == object_sha256)
                        .unwrap_or(usize::MAX)
                ),
                materialization_status: AUTHORING_MESH_V2_DURABLE_RECORD_STATUS.to_owned(),
                canonical_sha256: String::new(),
                created_at: record.created_at.clone(),
            };
            let mut record_for_revision = record_for_revision;
            record_for_revision.canonical_sha256 =
                authoring_mesh_v2_durable_record_canonical_sha256(&record_for_revision)?;
            validate_authoring_mesh_v2_revision_payload(&bytes, &revision, &record_for_revision)?;
        }
        transaction.commit()?;
        Ok(Some(record))
    }

    pub fn get_authoring_mesh_v2_transaction_by_id(
        &self,
        project_id: &str,
        transaction_id: &str,
    ) -> Result<Option<AuthoringMeshV2TransactionDurableRecord>, StoreError> {
        if !is_opaque_id(project_id) || !is_opaque_id(transaction_id) {
            return Err(StoreError::InvalidData(
                "transaction lookup identity is invalid".to_owned(),
            ));
        }
        let connection = self.lock_connection()?;
        let idempotency_key: Option<String> = connection
            .query_row(
                "SELECT idempotency_key FROM authoring_mesh_v2_transactions WHERE project_id = ?1 AND transaction_id = ?2",
                params![project_id, transaction_id],
                |row| row.get(0),
            )
            .optional()?;
        drop(connection);
        let Some(idempotency_key) = idempotency_key else {
            return Ok(None);
        };
        self.get_authoring_mesh_v2_transaction(project_id, &idempotency_key)
    }
}
