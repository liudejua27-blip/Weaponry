//! Store-owned durable index for one cumulative production-weapon FormArt
//! review candidate. CAS JSON remains authoritative; this table only makes the
//! original/current/final lineage restart-queryable and reachable.

use super::{
    Store, StoreError, canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256,
    mark_reachable_in_transaction,
};
use forgecad_contracts::CasObjectRecord;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const TABLE: &str = "production_weapon_form_art_composite_proposal_links";
const SCHEMA_VERSION: &str = "ProductionWeaponFormArtCompositeProposalStoreRecord@1";
const RECEIPT_SCHEMA_VERSION: &str = "ProductionWeaponFormArtCompositeProposalReceipt@1";
const JSON_MIME: &str = "application/json";
const RECEIPT_KIND: &str = "production-weapon-form-art-composite-proposal-receipt";
const MAX_JSON_BYTES: u64 = 1_048_576;
const STATUS: &str = "PREPARED_REVIEWABLE_CANDIDATE_AWAITING_SIX_VIEW";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionWeaponFormArtCompositeProposalStoreRecord {
    pub schema_version: String,
    pub project_id: String,
    pub proposal_id: String,
    pub session_id: String,
    pub idempotency_key: String,
    pub plan_object_sha256: String,
    pub plan_canonical_sha256: String,
    pub current_base_candidate_id: String,
    pub current_base_candidate_state_sha256: String,
    pub current_base_artifact_sha256: String,
    pub current_base_geometry_program_sha256: String,
    pub current_base_geometry_program_object_sha256: String,
    pub current_base_proposal_evidence_receipt_object_sha256: String,
    pub composed_geometry_program_sha256: String,
    pub composed_geometry_program_object_sha256: String,
    pub proposal_candidate_id: String,
    pub proposal_candidate_state_sha256: String,
    pub proposal_artifact_sha256: String,
    pub proposal_artifact_readback_object_sha256: String,
    pub proposal_artifact_readback_sha256: String,
    pub cross_view_evidence_bundle_sha256: Option<String>,
    pub proposal_form_art_evidence_receipt_object_sha256: Option<String>,
    pub receipt_object_sha256: String,
    pub request_sha256: String,
    pub input_sha256: String,
    pub status: String,
    pub quality_status: String,
    pub candidate_confirm_allowed: bool,
    pub secondary_form_approved: String,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
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
            schema_version TEXT NOT NULL CHECK (schema_version = '{SCHEMA_VERSION}'),
            project_id TEXT NOT NULL REFERENCES projects(project_id),
            proposal_id TEXT NOT NULL,
            session_id TEXT NOT NULL REFERENCES agentic_design_sessions(session_id),
            idempotency_key TEXT NOT NULL,
            plan_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
            plan_canonical_sha256 TEXT NOT NULL,
            current_base_candidate_id TEXT NOT NULL REFERENCES candidates(candidate_id),
            current_base_candidate_state_sha256 TEXT NOT NULL,
            current_base_artifact_sha256 TEXT NOT NULL REFERENCES objects(sha256),
            current_base_geometry_program_sha256 TEXT NOT NULL,
            current_base_geometry_program_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
            current_base_proposal_evidence_receipt_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
            composed_geometry_program_sha256 TEXT NOT NULL,
            composed_geometry_program_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
            proposal_candidate_id TEXT NOT NULL REFERENCES candidates(candidate_id),
            proposal_candidate_state_sha256 TEXT NOT NULL,
            proposal_artifact_sha256 TEXT NOT NULL REFERENCES objects(sha256),
            proposal_artifact_readback_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
            proposal_artifact_readback_sha256 TEXT NOT NULL,
            cross_view_evidence_bundle_sha256 TEXT REFERENCES objects(sha256),
            proposal_form_art_evidence_receipt_object_sha256 TEXT REFERENCES objects(sha256),
            receipt_object_sha256 TEXT NOT NULL UNIQUE REFERENCES objects(sha256),
            request_sha256 TEXT NOT NULL,
            input_sha256 TEXT NOT NULL,
            status TEXT NOT NULL CHECK (status = '{STATUS}'),
            quality_status TEXT NOT NULL CHECK (quality_status = 'QUALITY_TARGET_NOT_MET'),
            candidate_confirm_allowed INTEGER NOT NULL CHECK (candidate_confirm_allowed = 0),
            secondary_form_approved TEXT NOT NULL CHECK (secondary_form_approved = 'NOT_CREATED'),
            production_stage_advanced INTEGER NOT NULL CHECK (production_stage_advanced = 0),
            candidate_confirmed INTEGER NOT NULL CHECK (candidate_confirmed = 0),
            version_created INTEGER NOT NULL CHECK (version_created = 0),
            export_performed INTEGER NOT NULL CHECK (export_performed = 0),
            canonical_sha256 TEXT NOT NULL,
            created_at TEXT NOT NULL,
            record_json TEXT NOT NULL,
            PRIMARY KEY(project_id, proposal_id),
            UNIQUE(project_id, idempotency_key)
        );
        CREATE INDEX IF NOT EXISTS production_weapon_form_art_composite_proposal_base_idx
          ON {TABLE}(project_id,current_base_candidate_id,current_base_proposal_evidence_receipt_object_sha256);
        CREATE INDEX IF NOT EXISTS production_weapon_form_art_composite_proposal_result_idx
          ON {TABLE}(project_id,proposal_candidate_id,created_at DESC);"
    ))?;
    Ok(())
}

/// The receipt object hash is deliberately excluded from this canonical value
/// so the immutable receipt can bind the record without a self-referential hash.
pub fn record_canonical_sha256(
    record: &ProductionWeaponFormArtCompositeProposalStoreRecord,
) -> Result<String, StoreError> {
    let mut value =
        serde_json::to_value(record).map_err(|error| StoreError::InvalidData(error.to_string()))?;
    value["receipt_object_sha256"] = Value::String(String::new());
    value["canonical_sha256"] = Value::String(String::new());
    Ok(canonical_json_hash(&value))
}

fn validate_record(
    record: &ProductionWeaponFormArtCompositeProposalStoreRecord,
) -> Result<(), StoreError> {
    if record.schema_version != SCHEMA_VERSION
        || !is_opaque_id(&record.project_id)
        || !is_opaque_id(&record.proposal_id)
        || !is_opaque_id(&record.session_id)
        || !is_opaque_id(&record.idempotency_key)
        || record.status != STATUS
        || record.quality_status != "QUALITY_TARGET_NOT_MET"
        || record.candidate_confirm_allowed
        || record.secondary_form_approved != "NOT_CREATED"
        || record.production_stage_advanced
        || record.candidate_confirmed
        || record.version_created
        || record.export_performed
        || record.created_at.is_empty()
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_RECORD_INVALID",
            "record identity or non-promotion boundary differs",
        ));
    }
    for value in [
        &record.plan_object_sha256,
        &record.plan_canonical_sha256,
        &record.current_base_candidate_state_sha256,
        &record.current_base_artifact_sha256,
        &record.current_base_geometry_program_sha256,
        &record.current_base_geometry_program_object_sha256,
        &record.current_base_proposal_evidence_receipt_object_sha256,
        &record.composed_geometry_program_sha256,
        &record.composed_geometry_program_object_sha256,
        &record.proposal_candidate_state_sha256,
        &record.proposal_artifact_sha256,
        &record.proposal_artifact_readback_object_sha256,
        &record.proposal_artifact_readback_sha256,
        &record.receipt_object_sha256,
        &record.request_sha256,
        &record.input_sha256,
        &record.canonical_sha256,
    ] {
        if !is_sha256(value) {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_HASH_INVALID",
                "record contains an invalid hash",
            ));
        }
    }
    if record
        .cross_view_evidence_bundle_sha256
        .as_ref()
        .is_some_and(|value| !is_sha256(value))
        || record
            .proposal_form_art_evidence_receipt_object_sha256
            .as_ref()
            .is_some_and(|value| !is_sha256(value))
        || record.canonical_sha256 != record_canonical_sha256(record)?
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_CANONICAL_MISMATCH",
            "record canonical hash differs",
        ));
    }
    Ok(())
}

fn read_json_object(store: &Store, sha256: &str, expected_kind: &str) -> Result<Value, StoreError> {
    let object = store.get_object(sha256)?.ok_or_else(|| {
        contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_OBJECT_MISSING",
            "CAS object is unavailable",
        )
    })?;
    if object.mime != JSON_MIME
        || object.kind != expected_kind
        || object.size_bytes == 0
        || object.size_bytes > MAX_JSON_BYTES
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_OBJECT_INVALID",
            "CAS object metadata differs",
        ));
    }
    let bytes = store
        .cas
        .read_verified_bounded(sha256, MAX_JSON_BYTES)
        .map_err(StoreError::Cas)?;
    serde_json::from_slice(&bytes).map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn validate_bindings(
    store: &Store,
    record: &ProductionWeaponFormArtCompositeProposalStoreRecord,
) -> Result<(), StoreError> {
    let session = store
        .get_agentic_session(&record.session_id)?
        .ok_or_else(|| {
            contract(
                "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_SESSION_MISSING",
                "session is unavailable",
            )
        })?;
    if session.project_id != record.project_id {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_SESSION_MISMATCH",
            "session project differs",
        ));
    }
    let current = store
        .get_candidate(&record.current_base_candidate_id)?
        .ok_or_else(|| {
            contract(
                "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_BASE_MISSING",
                "current-base candidate is unavailable",
            )
        })?;
    let current_evidence = store
        .get_geometry_candidate_evidence(&record.current_base_candidate_id)?
        .ok_or_else(|| {
            contract(
                "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_BASE_EVIDENCE_MISSING",
                "current-base evidence is unavailable",
            )
        })?;
    if current.project_id != record.project_id
        || current.canonical_sha256 != record.current_base_candidate_state_sha256
        || current_evidence.artifact_object_sha256 != record.current_base_artifact_sha256
        || current_evidence.geometry_program_sha256 != record.current_base_geometry_program_sha256
        || current_evidence.geometry_program_object_sha256
            != record.current_base_geometry_program_object_sha256
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_BASE_MISMATCH",
            "current-base binding differs",
        ));
    }
    let base_proof = store
        .get_production_weapon_form_art_proposal_evidence(
            &record.current_base_proposal_evidence_receipt_object_sha256,
        )?
        .ok_or_else(|| {
            contract(
                "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_BASE_PROOF_MISSING",
                "current-base proposal evidence is unavailable",
            )
        })?;
    if base_proof.project_id != record.project_id
        || base_proof.proposal_candidate_id != record.current_base_candidate_id
        || base_proof.proposal_candidate_state_sha256 != record.current_base_candidate_state_sha256
        || base_proof.proposal_artifact_sha256 != record.current_base_artifact_sha256
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_BASE_PROOF_MISMATCH",
            "current-base proposal evidence differs",
        ));
    }
    let proposal = store
        .get_candidate(&record.proposal_candidate_id)?
        .ok_or_else(|| {
            contract(
                "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_PROPOSAL_MISSING",
                "proposal candidate is unavailable",
            )
        })?;
    let proposal_evidence = store
        .get_geometry_candidate_evidence(&record.proposal_candidate_id)?
        .ok_or_else(|| {
            contract(
                "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_PROPOSAL_EVIDENCE_MISSING",
                "proposal evidence is unavailable",
            )
        })?;
    if proposal.project_id != record.project_id
        || proposal.canonical_sha256 != record.proposal_candidate_state_sha256
        || proposal_evidence.artifact_object_sha256 != record.proposal_artifact_sha256
        || proposal_evidence.artifact_readback_object_sha256
            != record.proposal_artifact_readback_object_sha256
        || proposal_evidence.geometry_program_sha256 != record.composed_geometry_program_sha256
        || proposal_evidence.geometry_program_object_sha256
            != record.composed_geometry_program_object_sha256
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_PROPOSAL_MISMATCH",
            "proposal candidate binding differs",
        ));
    }
    let plan = read_json_object(
        store,
        &record.plan_object_sha256,
        "production-weapon-form-art-composite-proposal-plan",
    )?;
    if plan.get("canonical_sha256").and_then(Value::as_str)
        != Some(record.plan_canonical_sha256.as_str())
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_PLAN_MISMATCH",
            "plan canonical hash differs",
        ));
    }
    let receipt = read_json_object(store, &record.receipt_object_sha256, RECEIPT_KIND)?;
    if receipt.get("schema_version").and_then(Value::as_str) != Some(RECEIPT_SCHEMA_VERSION)
        || receipt.get("project_id").and_then(Value::as_str) != Some(record.project_id.as_str())
        || receipt.get("proposal_id").and_then(Value::as_str) != Some(record.proposal_id.as_str())
        || receipt
            .get("record_canonical_sha256")
            .and_then(Value::as_str)
            != Some(record.canonical_sha256.as_str())
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_RECEIPT_MISMATCH",
            "receipt binding differs",
        ));
    }
    Ok(())
}

fn read_record(
    connection: &Connection,
    project_id: &str,
    field: &str,
    value: &str,
) -> Result<Option<ProductionWeaponFormArtCompositeProposalStoreRecord>, StoreError> {
    let sql = format!("SELECT record_json FROM {TABLE} WHERE project_id=?1 AND {field}=?2");
    let json: Option<String> = connection
        .query_row(&sql, params![project_id, value], |row| row.get(0))
        .optional()?;
    json.map(|json| {
        serde_json::from_str(&json).map_err(|error| StoreError::InvalidData(error.to_string()))
    })
    .transpose()
}

impl Store {
    pub fn record_production_weapon_form_art_composite_proposal_with_replay(
        &self,
        record: &ProductionWeaponFormArtCompositeProposalStoreRecord,
        receipt_object: &CasObjectRecord,
    ) -> Result<(ProductionWeaponFormArtCompositeProposalStoreRecord, bool), StoreError> {
        validate_record(record)?;
        if receipt_object.sha256 != record.receipt_object_sha256
            || receipt_object.mime != JSON_MIME
            || receipt_object.kind != RECEIPT_KIND
        {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_RECEIPT_INVALID",
                "receipt CAS metadata differs",
            ));
        }
        validate_bindings(self, record)?;
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        ensure_table(&transaction)?;
        if let Some(existing) = read_record(
            &transaction,
            &record.project_id,
            "idempotency_key",
            &record.idempotency_key,
        )? {
            validate_record(&existing)?;
            if existing != *record {
                return Err(contract(
                    "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_IDEMPOTENCY_CONFLICT",
                    "idempotency key is already bound to different lineage",
                ));
            }
            transaction.rollback()?;
            return Ok((existing, true));
        }
        let record_json = String::from_utf8(
            canonical_json_bytes(
                &serde_json::to_value(record)
                    .map_err(|error| StoreError::InvalidData(error.to_string()))?,
            )
            .map_err(|error| StoreError::InvalidData(error.to_string()))?,
        )
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        transaction.execute(&format!("INSERT INTO {TABLE} (schema_version,project_id,proposal_id,session_id,idempotency_key,plan_object_sha256,plan_canonical_sha256,current_base_candidate_id,current_base_candidate_state_sha256,current_base_artifact_sha256,current_base_geometry_program_sha256,current_base_geometry_program_object_sha256,current_base_proposal_evidence_receipt_object_sha256,composed_geometry_program_sha256,composed_geometry_program_object_sha256,proposal_candidate_id,proposal_candidate_state_sha256,proposal_artifact_sha256,proposal_artifact_readback_object_sha256,proposal_artifact_readback_sha256,cross_view_evidence_bundle_sha256,proposal_form_art_evidence_receipt_object_sha256,receipt_object_sha256,request_sha256,input_sha256,status,quality_status,candidate_confirm_allowed,secondary_form_approved,production_stage_advanced,candidate_confirmed,version_created,export_performed,canonical_sha256,created_at,record_json) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,0,?28,0,0,0,0,?29,?30,?31)"), params![
            record.schema_version,record.project_id,record.proposal_id,record.session_id,record.idempotency_key,record.plan_object_sha256,record.plan_canonical_sha256,
            record.current_base_candidate_id,record.current_base_candidate_state_sha256,record.current_base_artifact_sha256,record.current_base_geometry_program_sha256,
            record.current_base_geometry_program_object_sha256,record.current_base_proposal_evidence_receipt_object_sha256,record.composed_geometry_program_sha256,
            record.composed_geometry_program_object_sha256,record.proposal_candidate_id,record.proposal_candidate_state_sha256,record.proposal_artifact_sha256,
            record.proposal_artifact_readback_object_sha256,record.proposal_artifact_readback_sha256,record.cross_view_evidence_bundle_sha256,
            record.proposal_form_art_evidence_receipt_object_sha256,record.receipt_object_sha256,record.request_sha256,record.input_sha256,record.status,record.quality_status,
            record.secondary_form_approved,record.canonical_sha256,record.created_at,record_json])?;
        let mut roots = vec![
            record.plan_object_sha256.clone(),
            record.current_base_artifact_sha256.clone(),
            record.current_base_geometry_program_object_sha256.clone(),
            record
                .current_base_proposal_evidence_receipt_object_sha256
                .clone(),
            record.composed_geometry_program_object_sha256.clone(),
            record.proposal_artifact_sha256.clone(),
            record.proposal_artifact_readback_object_sha256.clone(),
            record.receipt_object_sha256.clone(),
        ];
        roots.extend(record.cross_view_evidence_bundle_sha256.clone());
        roots.extend(
            record
                .proposal_form_art_evidence_receipt_object_sha256
                .clone(),
        );
        roots.sort();
        roots.dedup();
        mark_reachable_in_transaction(&transaction, &roots)?;
        transaction.commit()?;
        Ok((record.clone(), false))
    }

    pub fn get_production_weapon_form_art_composite_proposal(
        &self,
        project_id: &str,
        proposal_id: &str,
    ) -> Result<Option<ProductionWeaponFormArtCompositeProposalStoreRecord>, StoreError> {
        if !is_opaque_id(project_id) || !is_opaque_id(proposal_id) {
            return Err(StoreError::InvalidData(
                "composite proposal lookup identity is invalid".to_owned(),
            ));
        }
        let connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let record = read_record(&connection, project_id, "proposal_id", proposal_id)?;
        drop(connection);
        if let Some(record) = &record {
            validate_record(record)?;
            validate_bindings(self, record)?;
        }
        Ok(record)
    }

    pub fn get_production_weapon_form_art_composite_proposal_by_idempotency(
        &self,
        project_id: &str,
        idempotency_key: &str,
    ) -> Result<Option<ProductionWeaponFormArtCompositeProposalStoreRecord>, StoreError> {
        if !is_opaque_id(project_id) || !is_opaque_id(idempotency_key) {
            return Err(StoreError::InvalidData(
                "composite proposal idempotency identity is invalid".to_owned(),
            ));
        }
        let connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let record = read_record(&connection, project_id, "idempotency_key", idempotency_key)?;
        drop(connection);
        if let Some(record) = &record {
            validate_record(record)?;
            validate_bindings(self, record)?;
        }
        Ok(record)
    }
}
