//! Store-owned append-only evidence binding for a cumulative FormArt proposal.
//!
//! The first composite proposal row is intentionally immutable and may be
//! created before the six-view/FormArt evidence exists.  This module records a
//! later, closed evidence binding as a separate immutable row; it never
//! updates the proposal row in place.  Runtime remains the only caller that
//! is allowed to assemble the record and CAS receipt.  The Store verifies the
//! parent/evidence identities and commits the typed index plus CAS roots in a
//! single SQLite transaction.

use super::{
    canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256,
    mark_reachable_in_transaction, ProductionWeaponFormArtCompositeProposalStoreRecord, Store,
    StoreError,
};
use forgecad_contracts::CasObjectRecord;
use forgecad_core::sha256_hex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const TABLE: &str = "production_weapon_form_art_composite_evidence_links";
pub const PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_RECORD_SCHEMA_VERSION: &str =
    "ProductionWeaponFormArtCompositeEvidenceRecord@1";
pub const PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_RECEIPT_SCHEMA_VERSION: &str =
    "ProductionWeaponFormArtCompositeEvidenceReceipt@1";
pub const PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_RECEIPT_OBJECT_KIND: &str =
    "production-weapon-form-art-composite-evidence-receipt";
const STATUS: &str = "SIX_VIEW_EVIDENCE_BOUND_NOT_PROMOTED";
const QUALITY_STATUS: &str = "QUALITY_TARGET_NOT_MET";
const JSON_MIME: &str = "application/json";
const MAX_JSON_BYTES: u64 = 1_048_576;

/// Immutable, typed index for one exact parent proposal plus its complete
/// CrossView/FormArt evidence binding.  `record_json` is deliberately kept
/// as the table payload rather than a struct field: the canonical record is
/// serialized once on insert and is the restart read authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionWeaponFormArtCompositeEvidenceRecord {
    pub schema_version: String,
    pub attachment_id: String,
    pub project_id: String,
    pub proposal_id: String,
    pub parent_record_canonical_sha256: String,
    pub parent_receipt_object_sha256: String,
    pub cross_view_evidence_bundle_sha256: String,
    pub proposal_form_art_evidence_receipt_object_sha256: String,
    pub attachment_receipt_object_sha256: String,
    pub request_sha256: String,
    pub input_sha256: String,
    pub idempotency_key: String,
    pub status: String,
    pub quality_status: String,
    pub candidate_confirm_allowed: bool,
    pub canonical_sha256: String,
    pub created_at: String,
}

fn contract(code: &str, message: impl Into<String>) -> StoreError {
    StoreError::Contract {
        code: code.to_owned(),
        message: message.into(),
    }
}

pub fn ensure_table(connection: &Connection) -> Result<(), StoreError> {
    connection.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {TABLE} (
            schema_version TEXT NOT NULL CHECK (schema_version = '{PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_RECORD_SCHEMA_VERSION}'),
            attachment_id TEXT NOT NULL PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(project_id),
            proposal_id TEXT NOT NULL,
            parent_record_canonical_sha256 TEXT NOT NULL,
            parent_receipt_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
            cross_view_evidence_bundle_sha256 TEXT NOT NULL REFERENCES objects(sha256),
            proposal_form_art_evidence_receipt_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
            attachment_receipt_object_sha256 TEXT NOT NULL UNIQUE REFERENCES objects(sha256),
            request_sha256 TEXT NOT NULL,
            input_sha256 TEXT NOT NULL,
            idempotency_key TEXT NOT NULL,
            status TEXT NOT NULL CHECK (status = '{STATUS}'),
            quality_status TEXT NOT NULL CHECK (quality_status = '{QUALITY_STATUS}'),
            candidate_confirm_allowed INTEGER NOT NULL CHECK (candidate_confirm_allowed = 0),
            canonical_sha256 TEXT NOT NULL,
            created_at TEXT NOT NULL,
            record_json TEXT NOT NULL,
            FOREIGN KEY (project_id, proposal_id)
                REFERENCES production_weapon_form_art_composite_proposal_links(project_id, proposal_id),
            UNIQUE (project_id, idempotency_key),
            UNIQUE (project_id, proposal_id, parent_record_canonical_sha256)
        );
        CREATE INDEX IF NOT EXISTS production_weapon_form_art_composite_evidence_proposal_idx
          ON {TABLE}(project_id, proposal_id, created_at DESC, attachment_id ASC);
        CREATE INDEX IF NOT EXISTS production_weapon_form_art_composite_evidence_parent_idx
          ON {TABLE}(parent_record_canonical_sha256, parent_receipt_object_sha256);
        CREATE INDEX IF NOT EXISTS production_weapon_form_art_composite_evidence_object_idx
          ON {TABLE}(cross_view_evidence_bundle_sha256,
                     proposal_form_art_evidence_receipt_object_sha256,
                     attachment_receipt_object_sha256);"
    ))?;
    Ok(())
}

fn record_value(
    record: &ProductionWeaponFormArtCompositeEvidenceRecord,
) -> Result<Value, StoreError> {
    serde_json::to_value(record).map_err(|error| StoreError::InvalidData(error.to_string()))
}

/// The attachment receipt hash is excluded to avoid a self-referential CAS
/// preimage.  The parent receipt hash remains part of the canonical identity.
pub fn record_canonical_sha256(
    record: &ProductionWeaponFormArtCompositeEvidenceRecord,
) -> Result<String, StoreError> {
    let mut value = record_value(record)?;
    value["attachment_receipt_object_sha256"] = Value::String(String::new());
    value["canonical_sha256"] = Value::String(String::new());
    Ok(canonical_json_hash(&value))
}

fn validate_record_shape(
    record: &ProductionWeaponFormArtCompositeEvidenceRecord,
) -> Result<(), StoreError> {
    if record.schema_version != PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_RECORD_SCHEMA_VERSION
        || !is_opaque_id(&record.attachment_id)
        || !is_opaque_id(&record.project_id)
        || !is_opaque_id(&record.proposal_id)
        || !is_opaque_id(&record.idempotency_key)
        || record.status != STATUS
        || record.quality_status != QUALITY_STATUS
        || record.candidate_confirm_allowed
        || record.created_at.is_empty()
        || record.created_at.len() > 128
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_RECORD_INVALID",
            "evidence binding identity or non-promotion boundary is invalid",
        ));
    }
    for hash in [
        &record.parent_record_canonical_sha256,
        &record.parent_receipt_object_sha256,
        &record.cross_view_evidence_bundle_sha256,
        &record.proposal_form_art_evidence_receipt_object_sha256,
        &record.attachment_receipt_object_sha256,
        &record.request_sha256,
        &record.input_sha256,
        &record.canonical_sha256,
    ] {
        if !is_sha256(hash) {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_HASH_INVALID",
                "evidence binding contains an invalid SHA-256 value",
            ));
        }
    }
    if record.canonical_sha256 != record_canonical_sha256(record)? {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_CANONICAL_MISMATCH",
            "evidence binding canonical hash differs",
        ));
    }
    Ok(())
}

fn validate_receipt_object(
    store: &Store,
    record: &ProductionWeaponFormArtCompositeEvidenceRecord,
    receipt_object: &CasObjectRecord,
) -> Result<(), StoreError> {
    if receipt_object.schema_version != "CasObject@1"
        || receipt_object.sha256 != record.attachment_receipt_object_sha256
        || receipt_object.mime != JSON_MIME
        || receipt_object.kind != PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_RECEIPT_OBJECT_KIND
        || receipt_object.size_bytes == 0
        || receipt_object.size_bytes > MAX_JSON_BYTES
        || !matches!(
            receipt_object.reachability.as_str(),
            "temporary" | "reachable"
        )
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_RECEIPT_INVALID",
            "attachment receipt CAS metadata differs",
        ));
    }
    let registered = store.get_object(&receipt_object.sha256)?.ok_or_else(|| {
        contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_RECEIPT_MISSING",
            "attachment receipt CAS object is unavailable",
        )
    })?;
    if registered.schema_version != receipt_object.schema_version
        || registered.sha256 != receipt_object.sha256
        || registered.size_bytes != receipt_object.size_bytes
        || registered.mime != receipt_object.mime
        || registered.kind != receipt_object.kind
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_RECEIPT_INVALID",
            "registered attachment receipt metadata differs",
        ));
    }
    let bytes = store
        .cas
        .read_verified_bounded(&receipt_object.sha256, MAX_JSON_BYTES)
        .map_err(StoreError::Cas)?;
    if bytes.len() as u64 != receipt_object.size_bytes
        || sha256_hex(&bytes) != receipt_object.sha256
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_RECEIPT_HASH_MISMATCH",
            "attachment receipt bytes do not match the registered CAS hash",
        ));
    }
    let receipt: Value = serde_json::from_slice(&bytes).map_err(|error| {
        contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_RECEIPT_INVALID",
            format!("attachment receipt JSON is invalid: {error}"),
        )
    })?;
    if receipt.get("schema_version").and_then(Value::as_str)
        != Some(PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_RECEIPT_SCHEMA_VERSION)
        || receipt.get("attachment_receipt_object_sha256").is_some()
        || receipt.get("attachment_id").and_then(Value::as_str)
            != Some(record.attachment_id.as_str())
        || receipt.get("project_id").and_then(Value::as_str) != Some(record.project_id.as_str())
        || receipt.get("proposal_id").and_then(Value::as_str) != Some(record.proposal_id.as_str())
        || receipt
            .get("parent_record_canonical_sha256")
            .and_then(Value::as_str)
            != Some(record.parent_record_canonical_sha256.as_str())
        || receipt
            .get("parent_receipt_object_sha256")
            .and_then(Value::as_str)
            != Some(record.parent_receipt_object_sha256.as_str())
        || receipt
            .get("cross_view_evidence_bundle_sha256")
            .and_then(Value::as_str)
            != Some(record.cross_view_evidence_bundle_sha256.as_str())
        || receipt
            .get("proposal_form_art_evidence_receipt_object_sha256")
            .and_then(Value::as_str)
            != Some(
                record
                    .proposal_form_art_evidence_receipt_object_sha256
                    .as_str(),
            )
        || receipt
            .get("record_canonical_sha256")
            .and_then(Value::as_str)
            != Some(record.canonical_sha256.as_str())
        || receipt.get("status").and_then(Value::as_str) != Some(STATUS)
        || receipt.get("quality_status").and_then(Value::as_str) != Some(QUALITY_STATUS)
        || receipt.get("candidate_confirm_allowed") != Some(&Value::Bool(false))
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_RECEIPT_BINDING_MISMATCH",
            "attachment receipt does not bind the exact evidence record",
        ));
    }
    let canonical_bytes = canonical_json_bytes(&receipt)
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    if canonical_bytes != bytes {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_RECEIPT_NON_CANONICAL",
            "attachment receipt bytes are not canonical JSON",
        ));
    }
    Ok(())
}

fn read_record(
    connection: &Connection,
    project_id: &str,
    field: &str,
    value: &str,
) -> Result<Option<ProductionWeaponFormArtCompositeEvidenceRecord>, StoreError> {
    let sql = format!("SELECT record_json FROM {TABLE} WHERE project_id = ?1 AND {field} = ?2");
    let payload: Option<String> = connection
        .query_row(&sql, params![project_id, value], |row| row.get(0))
        .optional()?;
    payload
        .map(|payload| {
            serde_json::from_str(&payload)
                .map_err(|error| StoreError::InvalidData(error.to_string()))
        })
        .transpose()
}

fn validate_parent(
    store: &Store,
    record: &ProductionWeaponFormArtCompositeEvidenceRecord,
) -> Result<ProductionWeaponFormArtCompositeProposalStoreRecord, StoreError> {
    let parent = store
        .get_production_weapon_form_art_composite_proposal(&record.project_id, &record.proposal_id)?
        .ok_or_else(|| {
            contract(
                "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_PARENT_MISSING",
                "parent composite proposal is unavailable",
            )
        })?;
    if parent.canonical_sha256 != record.parent_record_canonical_sha256
        || parent.receipt_object_sha256 != record.parent_receipt_object_sha256
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_PARENT_MISMATCH",
            "parent composite canonical or receipt binding differs",
        ));
    }
    if parent.cross_view_evidence_bundle_sha256.is_some()
        || parent
            .proposal_form_art_evidence_receipt_object_sha256
            .is_some()
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_PARENT_ALREADY_BOUND",
            "parent composite already contains an inline evidence binding",
        ));
    }
    Ok(parent)
}

fn validate_evidence_bindings(
    store: &Store,
    record: &ProductionWeaponFormArtCompositeEvidenceRecord,
    parent: &ProductionWeaponFormArtCompositeProposalStoreRecord,
) -> Result<(), StoreError> {
    let cross_view = store
        .get_cross_view_evidence(&record.cross_view_evidence_bundle_sha256)?
        .ok_or_else(|| {
            contract(
                "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_CROSS_VIEW_MISSING",
                "CrossView evidence bundle is unavailable",
            )
        })?;
    if cross_view.project_id != parent.project_id
        || cross_view.session_id != parent.session_id
        || cross_view.candidate_id != parent.proposal_candidate_id
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_CROSS_VIEW_MISMATCH",
            "CrossView evidence does not bind the proposal candidate",
        ));
    }
    let form_art = store
        .get_production_weapon_form_art_proposal_evidence(
            &record.proposal_form_art_evidence_receipt_object_sha256,
        )?
        .ok_or_else(|| {
            contract(
                "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_FORM_ART_MISSING",
                "proposal FormArt evidence receipt is unavailable",
            )
        })?;
    if form_art.project_id != parent.project_id
        || form_art.session_id != parent.session_id
        || form_art.proposal_candidate_id != parent.proposal_candidate_id
        || form_art.proposal_candidate_state_sha256 != parent.proposal_candidate_state_sha256
        || form_art.proposal_artifact_sha256 != parent.proposal_artifact_sha256
        || form_art.cross_view_evidence_bundle_sha256 != record.cross_view_evidence_bundle_sha256
        || form_art.receipt_object_sha256 != record.proposal_form_art_evidence_receipt_object_sha256
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_FORM_ART_MISMATCH",
            "proposal FormArt evidence does not bind the proposal candidate",
        ));
    }
    Ok(())
}

fn validate_parent_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    record: &ProductionWeaponFormArtCompositeEvidenceRecord,
) -> Result<(), StoreError> {
    let parent: Option<(String, String, String)> = transaction
        .query_row(
            "SELECT record_json, canonical_sha256, receipt_object_sha256 FROM production_weapon_form_art_composite_proposal_links WHERE project_id = ?1 AND proposal_id = ?2",
            params![record.project_id, record.proposal_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let Some((record_json, canonical_sha256, receipt_object_sha256)) = parent else {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_PARENT_MISSING",
            "parent composite proposal disappeared before evidence binding",
        ));
    };
    if canonical_sha256 != record.parent_record_canonical_sha256
        || receipt_object_sha256 != record.parent_receipt_object_sha256
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_PARENT_CHANGED",
            "parent composite proposal changed before evidence binding",
        ));
    }
    let parent: ProductionWeaponFormArtCompositeProposalStoreRecord =
        serde_json::from_str(&record_json)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    if parent.project_id != record.project_id
        || parent.proposal_id != record.proposal_id
        || parent.canonical_sha256 != record.parent_record_canonical_sha256
        || parent.receipt_object_sha256 != record.parent_receipt_object_sha256
        || parent.cross_view_evidence_bundle_sha256.is_some()
        || parent
            .proposal_form_art_evidence_receipt_object_sha256
            .is_some()
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_PARENT_CHANGED",
            "parent composite JSON binding differs before evidence commit",
        ));
    }
    Ok(())
}

fn validate_evidence_rows_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    record: &ProductionWeaponFormArtCompositeEvidenceRecord,
    parent: &ProductionWeaponFormArtCompositeProposalStoreRecord,
) -> Result<(), StoreError> {
    let cross_view: Option<(String, String, String)> = transaction
        .query_row(
            "SELECT project_id, session_id, candidate_id FROM cross_view_evidence WHERE bundle_object_sha256 = ?1",
            params![record.cross_view_evidence_bundle_sha256],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let Some((cross_project, cross_session, cross_candidate)) = cross_view else {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_CROSS_VIEW_MISSING",
            "CrossView row disappeared before evidence binding",
        ));
    };
    if cross_project != parent.project_id
        || cross_session != parent.session_id
        || cross_candidate != parent.proposal_candidate_id
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_CROSS_VIEW_CHANGED",
            "CrossView row changed before evidence binding",
        ));
    }
    let form_art: Option<(String, String, String, String, String, String)> = transaction
        .query_row(
            "SELECT project_id, session_id, proposal_candidate_id, proposal_candidate_state_sha256, proposal_artifact_sha256, cross_view_evidence_bundle_sha256 FROM production_weapon_form_art_proposal_evidence_links WHERE receipt_object_sha256 = ?1",
            params![record.proposal_form_art_evidence_receipt_object_sha256],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .optional()?;
    let Some((form_project, form_session, form_candidate, form_state, form_artifact, form_cross)) =
        form_art
    else {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_FORM_ART_MISSING",
            "proposal FormArt evidence row disappeared before evidence binding",
        ));
    };
    if form_project != parent.project_id
        || form_session != parent.session_id
        || form_candidate != parent.proposal_candidate_id
        || form_state != parent.proposal_candidate_state_sha256
        || form_artifact != parent.proposal_artifact_sha256
        || form_cross != record.cross_view_evidence_bundle_sha256
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_FORM_ART_CHANGED",
            "proposal FormArt evidence row changed before evidence binding",
        ));
    }
    Ok(())
}

impl Store {
    /// Insert one immutable evidence binding.  The parent composite row is
    /// never updated; replay is accepted only for byte-identical identity.
    pub fn record_production_weapon_form_art_composite_evidence_with_replay(
        &self,
        record: &ProductionWeaponFormArtCompositeEvidenceRecord,
        receipt_object: &CasObjectRecord,
    ) -> Result<(ProductionWeaponFormArtCompositeEvidenceRecord, bool), StoreError> {
        validate_record_shape(record)?;
        let parent = validate_parent(self, record)?;
        validate_evidence_bindings(self, record, &parent)?;
        validate_receipt_object(self, record, receipt_object)?;
        let record_json = String::from_utf8(
            canonical_json_bytes(&record_value(record)?)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?,
        )
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;

        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        super::production_weapon_form_art_composite_proposal::ensure_table(&transaction)?;
        ensure_table(&transaction)?;
        validate_parent_in_transaction(&transaction, record)?;
        validate_evidence_rows_in_transaction(&transaction, record, &parent)?;

        if let Some(existing) = read_record(
            &transaction,
            &record.project_id,
            "idempotency_key",
            &record.idempotency_key,
        )? {
            validate_record_shape(&existing)?;
            if existing != *record {
                return Err(contract(
                    "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_IDEMPOTENCY_CONFLICT",
                    "evidence binding idempotency key is already bound to different content",
                ));
            }
            let roots = [
                record.parent_receipt_object_sha256.clone(),
                record.cross_view_evidence_bundle_sha256.clone(),
                record
                    .proposal_form_art_evidence_receipt_object_sha256
                    .clone(),
                record.attachment_receipt_object_sha256.clone(),
            ];
            mark_reachable_in_transaction(&transaction, &roots)?;
            transaction.commit()?;
            return Ok((existing, true));
        }

        let conflict: Option<String> = transaction
            .query_row(
                &format!(
                    "SELECT attachment_id FROM {TABLE} WHERE attachment_id = ?1 OR (project_id = ?2 AND proposal_id = ?3 AND parent_record_canonical_sha256 = ?4)"
                ),
                params![
                    record.attachment_id,
                    record.project_id,
                    record.proposal_id,
                    record.parent_record_canonical_sha256,
                ],
                |row| row.get(0),
            )
            .optional()?;
        if conflict.is_some() {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_CONFLICT",
                "attachment or parent canonical identity is already bound",
            ));
        }

        transaction.execute(
            &format!(
                "INSERT INTO {TABLE} (schema_version, attachment_id, project_id, proposal_id, parent_record_canonical_sha256, parent_receipt_object_sha256, cross_view_evidence_bundle_sha256, proposal_form_art_evidence_receipt_object_sha256, attachment_receipt_object_sha256, request_sha256, input_sha256, idempotency_key, status, quality_status, candidate_confirm_allowed, canonical_sha256, created_at, record_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)"
            ),
            params![
                record.schema_version,
                record.attachment_id,
                record.project_id,
                record.proposal_id,
                record.parent_record_canonical_sha256,
                record.parent_receipt_object_sha256,
                record.cross_view_evidence_bundle_sha256,
                record.proposal_form_art_evidence_receipt_object_sha256,
                record.attachment_receipt_object_sha256,
                record.request_sha256,
                record.input_sha256,
                record.idempotency_key,
                record.status,
                record.quality_status,
                0_i64,
                record.canonical_sha256,
                record.created_at,
                record_json,
            ],
        )?;
        let roots = [
            record.parent_receipt_object_sha256.clone(),
            record.cross_view_evidence_bundle_sha256.clone(),
            record
                .proposal_form_art_evidence_receipt_object_sha256
                .clone(),
            record.attachment_receipt_object_sha256.clone(),
        ];
        mark_reachable_in_transaction(&transaction, &roots)?;
        transaction.commit()?;
        Ok((record.clone(), false))
    }

    pub fn record_production_weapon_form_art_composite_evidence_link_with_replay(
        &self,
        record: &ProductionWeaponFormArtCompositeEvidenceRecord,
        receipt_object: &CasObjectRecord,
    ) -> Result<(ProductionWeaponFormArtCompositeEvidenceRecord, bool), StoreError> {
        self.record_production_weapon_form_art_composite_evidence_with_replay(
            record,
            receipt_object,
        )
    }

    pub fn get_production_weapon_form_art_composite_evidence(
        &self,
        project_id: &str,
        proposal_id: &str,
    ) -> Result<Option<ProductionWeaponFormArtCompositeEvidenceRecord>, StoreError> {
        self.get_production_weapon_form_art_composite_evidence_by(
            project_id,
            "proposal_id",
            proposal_id,
        )
    }

    pub fn get_production_weapon_form_art_composite_evidence_by_idempotency(
        &self,
        project_id: &str,
        idempotency_key: &str,
    ) -> Result<Option<ProductionWeaponFormArtCompositeEvidenceRecord>, StoreError> {
        self.get_production_weapon_form_art_composite_evidence_by(
            project_id,
            "idempotency_key",
            idempotency_key,
        )
    }

    fn get_production_weapon_form_art_composite_evidence_by(
        &self,
        project_id: &str,
        field: &str,
        value: &str,
    ) -> Result<Option<ProductionWeaponFormArtCompositeEvidenceRecord>, StoreError> {
        if !is_opaque_id(project_id) || !is_opaque_id(value) {
            return Err(StoreError::InvalidData(
                "composite evidence lookup identity is invalid".to_owned(),
            ));
        }
        let connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let record = read_record(&connection, project_id, field, value)?;
        drop(connection);
        let Some(record) = record else {
            return Ok(None);
        };
        validate_record_shape(&record)?;
        let parent = validate_parent(self, &record)?;
        validate_evidence_bindings(self, &record, &parent)?;
        let receipt_object = self
            .get_object(&record.attachment_receipt_object_sha256)?
            .ok_or_else(|| {
                contract(
                    "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_RECEIPT_MISSING",
                    "attachment receipt CAS object is unavailable",
                )
            })?;
        validate_receipt_object(self, &record, &receipt_object)?;
        Ok(Some(record))
    }
}
