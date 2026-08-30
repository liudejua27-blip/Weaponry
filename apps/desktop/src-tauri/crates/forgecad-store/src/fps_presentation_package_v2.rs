//! Durable index for one Runtime-owned editable composite FPS package.

use forgecad_contracts::{
    CasObjectRecord, FpsPresentationPackageV2, FpsPresentationPackageV2CandidateBinding,
    is_opaque_id, is_sha256,
};
use forgecad_core::{canonical_json_bytes, canonical_json_hash, sha256_hex};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Store, StoreError};

pub const TABLE: &str = "fps_presentation_packages_v2";
pub const RECORD_SCHEMA_VERSION: &str = "FpsPresentationPackageV2StoreRecord@1";
pub const OBJECT_KIND: &str = "forgecad-fps-presentation-package-v2";
pub const JSON_MIME: &str = "application/json";
pub const MAX_JSON_BYTES: u64 = 8 * 1024 * 1024;
pub const CANDIDATE_TABLE: &str = "fps_presentation_package_v2_candidates";
pub const CANDIDATE_RECORD_SCHEMA_VERSION: &str = "FpsPresentationPackageV2CandidateStoreRecord@1";
pub const CANDIDATE_OBJECT_KIND: &str = "forgecad-fps-presentation-package-v2-candidate-binding";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FpsPresentationPackageV2CandidateStoreRecord {
    pub schema_version: String,
    pub project_id: String,
    pub package_id: String,
    pub package_sha256: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub binding_object_sha256: String,
    pub binding_canonical_sha256: String,
    pub idempotency_key: String,
    pub request_input_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FpsPresentationPackageV2StoreRecord {
    pub schema_version: String,
    pub project_id: String,
    pub package_id: String,
    pub idempotency_key: String,
    pub package_object_sha256: String,
    pub package_canonical_sha256: String,
    pub weapon_materialization_id: String,
    pub weapon_descriptor_sha256: String,
    pub arms_materialization_id: String,
    pub arms_descriptor_sha256: String,
    pub animation_materialization_id: String,
    pub animation_descriptor_sha256: String,
    pub request_input_sha256: String,
    pub status: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

fn contract(code: &str, message: impl Into<String>) -> StoreError {
    StoreError::Contract {
        code: code.to_owned(),
        message: message.into(),
    }
}

fn canonical_hash_without<T: Serialize>(value: &T, field: &str) -> Result<String, StoreError> {
    let mut value =
        serde_json::to_value(value).map_err(|error| StoreError::InvalidData(error.to_string()))?;
    value
        .as_object_mut()
        .ok_or_else(|| StoreError::InvalidData("package record must be an object".to_owned()))?
        .insert(field.to_owned(), Value::String(String::new()));
    Ok(canonical_json_hash(&value))
}

pub(crate) fn ensure_table(connection: &rusqlite::Connection) -> Result<(), StoreError> {
    connection.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {TABLE} (
           schema_version TEXT NOT NULL CHECK (schema_version = 'FpsPresentationPackageV2StoreRecord@1'),
           project_id TEXT NOT NULL REFERENCES projects(project_id),
           package_id TEXT NOT NULL,
           idempotency_key TEXT NOT NULL,
           package_object_sha256 TEXT NOT NULL UNIQUE REFERENCES objects(sha256),
           package_canonical_sha256 TEXT NOT NULL,
           weapon_materialization_id TEXT NOT NULL,
           weapon_descriptor_sha256 TEXT NOT NULL,
           arms_materialization_id TEXT NOT NULL,
           arms_descriptor_sha256 TEXT NOT NULL,
           animation_materialization_id TEXT NOT NULL,
           animation_descriptor_sha256 TEXT NOT NULL,
           request_input_sha256 TEXT NOT NULL,
           status TEXT NOT NULL CHECK (status = 'EDITABLE_COMPOSITE_BOUND'),
           canonical_sha256 TEXT NOT NULL,
           created_at TEXT NOT NULL,
           record_json TEXT NOT NULL,
           PRIMARY KEY (project_id, package_id),
           UNIQUE (project_id, idempotency_key)
         );
         CREATE INDEX IF NOT EXISTS fps_presentation_packages_v2_project_idx
           ON {TABLE}(project_id, created_at DESC, package_id ASC);
         CREATE TABLE IF NOT EXISTS {CANDIDATE_TABLE} (
           schema_version TEXT NOT NULL CHECK (schema_version = 'FpsPresentationPackageV2CandidateStoreRecord@1'),
           project_id TEXT NOT NULL REFERENCES projects(project_id),
           package_id TEXT NOT NULL,
           package_sha256 TEXT NOT NULL,
           candidate_id TEXT NOT NULL UNIQUE REFERENCES candidates(candidate_id),
           candidate_state_sha256 TEXT NOT NULL,
           binding_object_sha256 TEXT NOT NULL UNIQUE REFERENCES objects(sha256),
           binding_canonical_sha256 TEXT NOT NULL,
           idempotency_key TEXT NOT NULL,
           request_input_sha256 TEXT NOT NULL,
           canonical_sha256 TEXT NOT NULL,
           created_at TEXT NOT NULL,
           record_json TEXT NOT NULL,
           PRIMARY KEY (project_id, package_id),
           UNIQUE (project_id, idempotency_key),
           FOREIGN KEY (project_id, package_id) REFERENCES {TABLE}(project_id, package_id)
         );
         CREATE INDEX IF NOT EXISTS fps_presentation_package_v2_candidates_project_idx
           ON {CANDIDATE_TABLE}(project_id, created_at DESC, package_id ASC);"
    ))?;
    Ok(())
}

fn validate_candidate_record(
    record: &FpsPresentationPackageV2CandidateStoreRecord,
) -> Result<(), StoreError> {
    if record.schema_version != CANDIDATE_RECORD_SCHEMA_VERSION
        || !is_opaque_id(&record.project_id)
        || !is_opaque_id(&record.package_id)
        || !is_opaque_id(&record.candidate_id)
        || !is_opaque_id(&record.idempotency_key)
        || record.created_at.is_empty()
        || record.created_at.len() > 128
        || [
            &record.package_sha256,
            &record.candidate_state_sha256,
            &record.binding_object_sha256,
            &record.binding_canonical_sha256,
            &record.request_input_sha256,
            &record.canonical_sha256,
        ]
        .iter()
        .any(|value| !is_sha256(value))
        || canonical_hash_without(record, "canonical_sha256")? != record.canonical_sha256
    {
        return Err(contract(
            "FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_RECORD_INVALID",
            "candidate binding record is invalid",
        ));
    }
    Ok(())
}

fn validate_candidate_binding(
    record: &FpsPresentationPackageV2CandidateStoreRecord,
    binding: &FpsPresentationPackageV2CandidateBinding,
) -> Result<Vec<u8>, StoreError> {
    if binding.schema_version
        != forgecad_contracts::FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_BINDING_SCHEMA_VERSION
        || binding.project_id != record.project_id
        || binding.package_id != record.package_id
        || binding.package_sha256 != record.package_sha256
        || binding.candidate_id != record.candidate_id
        || binding.candidate_state_sha256 != record.candidate_state_sha256
        || binding.canonical_sha256 != record.binding_canonical_sha256
        || binding.candidate_state != "reviewable"
        || binding.geometry_integrity_status != "PASS_SOURCE_STRUCTURAL"
        || binding.form_stage != "candidate-reviewable"
        || binding.secondary_form_approved
        || binding.formal_high_status != "BLOCKED_SECONDARY_FORM_APPROVAL"
        || binding.quality_status != "structural_only"
        || binding.visual_review_status != "NOT_RUN"
        || binding.engine_validation_status != "NOT_RUN"
        || binding.human_review_status != "NOT_RUN"
        || binding.promotion_eligible
        || binding.candidate_confirmed
        || binding.version_created
        || binding.export_performed
        || binding.policy != forgecad_contracts::FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_POLICY
        || binding.canonicalization_policy
            != forgecad_contracts::FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_CANONICALIZATION_POLICY
        || canonical_hash_without(binding, "canonical_sha256")? != binding.canonical_sha256
    {
        return Err(contract(
            "FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_BINDING_INVALID",
            "candidate binding does not preserve the fail-closed Form boundary",
        ));
    }
    canonical_json_bytes(
        &serde_json::to_value(binding)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?,
    )
    .map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn validate_record(record: &FpsPresentationPackageV2StoreRecord) -> Result<(), StoreError> {
    if record.schema_version != RECORD_SCHEMA_VERSION
        || !is_opaque_id(&record.project_id)
        || !is_opaque_id(&record.package_id)
        || !is_opaque_id(&record.idempotency_key)
        || !is_opaque_id(&record.weapon_materialization_id)
        || !is_opaque_id(&record.arms_materialization_id)
        || !is_opaque_id(&record.animation_materialization_id)
        || record.status != forgecad_contracts::FPS_PRESENTATION_PACKAGE_V2_STATUS
        || record.created_at.is_empty()
        || record.created_at.len() > 128
    {
        return Err(contract(
            "FPS_PRESENTATION_PACKAGE_V2_RECORD_INVALID",
            "composite package record identity is invalid",
        ));
    }
    for hash in [
        &record.package_object_sha256,
        &record.package_canonical_sha256,
        &record.weapon_descriptor_sha256,
        &record.arms_descriptor_sha256,
        &record.animation_descriptor_sha256,
        &record.request_input_sha256,
        &record.canonical_sha256,
    ] {
        if !is_sha256(hash) {
            return Err(contract(
                "FPS_PRESENTATION_PACKAGE_V2_HASH_INVALID",
                "composite package record contains an invalid hash",
            ));
        }
    }
    if canonical_hash_without(record, "canonical_sha256")? != record.canonical_sha256 {
        return Err(contract(
            "FPS_PRESENTATION_PACKAGE_V2_RECORD_HASH_MISMATCH",
            "composite package record canonical hash differs",
        ));
    }
    Ok(())
}

fn validate_package(
    record: &FpsPresentationPackageV2StoreRecord,
    package: &FpsPresentationPackageV2,
) -> Result<Vec<u8>, StoreError> {
    if package.schema_version != forgecad_contracts::FPS_PRESENTATION_PACKAGE_V2_SCHEMA_VERSION
        || package.package_id != record.package_id
        || package.project_id != record.project_id
        || package.status != forgecad_contracts::FPS_PRESENTATION_PACKAGE_V2_STATUS
        || package.quality_status != forgecad_contracts::FPS_PRESENTATION_PACKAGE_V2_QUALITY_STATUS
        || package.review_status != forgecad_contracts::FPS_PRESENTATION_PACKAGE_V2_REVIEW_STATUS
        || package.promotion_eligible
        || package.candidate_created
        || package.candidate_confirmed
        || package.version_created
        || package.export_performed
        || package.actual_engine_roundtrip
        || package.human_review_performed
        || package.canonical_sha256 != record.package_canonical_sha256
        || canonical_hash_without(package, "canonical_sha256")? != package.canonical_sha256
    {
        return Err(contract(
            "FPS_PRESENTATION_PACKAGE_V2_PAYLOAD_INVALID",
            "composite package payload is not a structural-only exact binding",
        ));
    }
    canonical_json_bytes(
        &serde_json::to_value(package)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?,
    )
    .map_err(|error| StoreError::InvalidData(error.to_string()))
}

impl Store {
    pub fn record_fps_presentation_package_v2_candidate_with_replay(
        &self,
        record: &FpsPresentationPackageV2CandidateStoreRecord,
        binding: &FpsPresentationPackageV2CandidateBinding,
        object: &CasObjectRecord,
    ) -> Result<(FpsPresentationPackageV2CandidateStoreRecord, bool), StoreError> {
        validate_candidate_record(record)?;
        let bytes = validate_candidate_binding(record, binding)?;
        if object.schema_version != "CasObject@1"
            || object.sha256 != record.binding_object_sha256
            || object.mime != JSON_MIME
            || object.kind != CANDIDATE_OBJECT_KIND
            || object.size_bytes == 0
            || object.size_bytes > MAX_JSON_BYTES
            || sha256_hex(&bytes) != object.sha256
        {
            return Err(contract(
                "FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_CAS_INVALID",
                "candidate binding CAS object differs",
            ));
        }
        let package = self
            .get_fps_presentation_package_v2(&record.project_id, &record.package_id)?
            .ok_or_else(|| {
                contract(
                    "FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_PACKAGE_MISSING",
                    "composite package is not durable",
                )
            })?;
        if package.package_canonical_sha256 != record.package_sha256
            || package.package_object_sha256 != binding.package_object_sha256
            || package.weapon_materialization_id != binding.weapon_materialization_id
            || package.weapon_descriptor_sha256 != binding.weapon_materialization_descriptor_sha256
        {
            return Err(contract(
                "FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_PACKAGE_MISMATCH",
                "package source binding differs",
            ));
        }
        let candidate = self.get_candidate(&record.candidate_id)?.ok_or_else(|| {
            contract(
                "FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_MISSING",
                "reviewable candidate is not durable",
            )
        })?;
        if candidate.project_id != record.project_id
            || candidate.state != "reviewable"
            || candidate.canonical_sha256 != record.candidate_state_sha256
            || candidate.prepared_object_sha256.as_deref()
                != Some(binding.candidate_artifact_sha256.as_str())
            || !candidate.quality_hard_gate_passed
        {
            return Err(contract(
                "FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_STATE_MISMATCH",
                "candidate state or artifact differs",
            ));
        }
        let evidence = self
            .get_geometry_candidate_evidence(&record.candidate_id)?
            .ok_or_else(|| {
                contract(
                    "FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_EVIDENCE_MISSING",
                    "geometry evidence is not durable",
                )
            })?;
        if evidence.project_id != record.project_id
            || evidence.canonical_sha256 != binding.geometry_candidate_evidence_sha256
            || evidence.artifact_object_sha256 != binding.candidate_artifact_sha256
            || evidence.geometry_program_object_sha256 != binding.geometry_program_object_sha256
            || evidence.geometry_program_sha256 != binding.geometry_program_sha256
        {
            return Err(contract(
                "FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_EVIDENCE_MISMATCH",
                "geometry evidence differs",
            ));
        }
        for hash in [
            &binding.weapon_authoring_mesh_revision_object_sha256,
            &binding.candidate_artifact_sha256,
            &binding.geometry_program_object_sha256,
            &record.binding_object_sha256,
        ] {
            self.get_object(hash)?.ok_or_else(|| {
                contract(
                    "FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_OBJECT_MISSING",
                    "candidate lineage CAS root is missing",
                )
            })?;
        }

        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        ensure_table(&transaction)?;
        let existing: Option<String> = transaction.query_row(
            &format!("SELECT record_json FROM {CANDIDATE_TABLE} WHERE project_id=?1 AND idempotency_key=?2"),
            params![record.project_id, record.idempotency_key], |row| row.get(0),
        ).optional()?;
        if let Some(existing) = existing {
            let existing: FpsPresentationPackageV2CandidateStoreRecord =
                serde_json::from_str(&existing)
                    .map_err(|error| StoreError::InvalidData(error.to_string()))?;
            validate_candidate_record(&existing)?;
            if existing != *record {
                return Err(contract(
                    "FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_IDEMPOTENCY_CONFLICT",
                    "idempotency key is bound to another candidate",
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
        transaction.execute(&format!("INSERT INTO {CANDIDATE_TABLE} (schema_version,project_id,package_id,package_sha256,candidate_id,candidate_state_sha256,binding_object_sha256,binding_canonical_sha256,idempotency_key,request_input_sha256,canonical_sha256,created_at,record_json) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)"), params![
            record.schema_version,record.project_id,record.package_id,record.package_sha256,record.candidate_id,
            record.candidate_state_sha256,record.binding_object_sha256,record.binding_canonical_sha256,
            record.idempotency_key,record.request_input_sha256,record.canonical_sha256,record.created_at,record_json
        ])?;
        transaction.execute(
            "UPDATE objects SET reachability='reachable' WHERE sha256=?1",
            params![record.binding_object_sha256],
        )?;
        transaction.commit()?;
        Ok((record.clone(), false))
    }

    pub fn get_fps_presentation_package_v2_candidate(
        &self,
        project_id: &str,
        package_id: &str,
    ) -> Result<Option<FpsPresentationPackageV2CandidateStoreRecord>, StoreError> {
        if !is_opaque_id(project_id) || !is_opaque_id(package_id) {
            return Err(StoreError::InvalidData(
                "candidate binding lookup identity is invalid".to_owned(),
            ));
        }
        let connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let json: Option<String> = connection.query_row(
            &format!("SELECT record_json FROM {CANDIDATE_TABLE} WHERE project_id=?1 AND package_id=?2"),
            params![project_id, package_id], |row| row.get(0),
        ).optional()?;
        let Some(json) = json else {
            return Ok(None);
        };
        let record: FpsPresentationPackageV2CandidateStoreRecord = serde_json::from_str(&json)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        validate_candidate_record(&record)?;
        drop(connection);
        let bytes = self
            .cas
            .read_verified_bounded(&record.binding_object_sha256, MAX_JSON_BYTES)
            .map_err(StoreError::Cas)?;
        let binding: FpsPresentationPackageV2CandidateBinding = serde_json::from_slice(&bytes)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        if sha256_hex(&bytes) != record.binding_object_sha256
            || validate_candidate_binding(&record, &binding)? != bytes
        {
            return Err(contract(
                "FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_RESTART_MISMATCH",
                "candidate binding restart readback differs",
            ));
        }
        Ok(Some(record))
    }

    pub fn record_fps_presentation_package_v2_with_replay(
        &self,
        record: &FpsPresentationPackageV2StoreRecord,
        package: &FpsPresentationPackageV2,
        object: &CasObjectRecord,
    ) -> Result<(FpsPresentationPackageV2StoreRecord, bool), StoreError> {
        validate_record(record)?;
        let bytes = validate_package(record, package)?;
        if object.schema_version != "CasObject@1"
            || object.sha256 != record.package_object_sha256
            || object.mime != JSON_MIME
            || object.kind != OBJECT_KIND
            || object.size_bytes == 0
            || object.size_bytes > MAX_JSON_BYTES
            || sha256_hex(&bytes) != object.sha256
        {
            return Err(contract(
                "FPS_PRESENTATION_PACKAGE_V2_CAS_INVALID",
                "package CAS object does not match typed payload",
            ));
        }
        for (materialization_id, descriptor_sha256) in [
            (
                &record.weapon_materialization_id,
                &record.weapon_descriptor_sha256,
            ),
            (
                &record.arms_materialization_id,
                &record.arms_descriptor_sha256,
            ),
            (
                &record.animation_materialization_id,
                &record.animation_descriptor_sha256,
            ),
        ] {
            let source = self
                .get_foundation_authoring_mesh_v2_materialization(
                    &record.project_id,
                    materialization_id,
                )?
                .ok_or_else(|| {
                    contract(
                        "FPS_PRESENTATION_PACKAGE_V2_SOURCE_MISSING",
                        "source materialization is not durable",
                    )
                })?;
            if source.descriptor_canonical_sha256 != *descriptor_sha256 {
                return Err(contract(
                    "FPS_PRESENTATION_PACKAGE_V2_SOURCE_MISMATCH",
                    "source descriptor hash differs",
                ));
            }
        }
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        ensure_table(&transaction)?;
        let existing: Option<String> = transaction
            .query_row(
                &format!(
                    "SELECT record_json FROM {TABLE} WHERE project_id=?1 AND idempotency_key=?2"
                ),
                params![record.project_id, record.idempotency_key],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(existing) = existing {
            let existing: FpsPresentationPackageV2StoreRecord = serde_json::from_str(&existing)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?;
            validate_record(&existing)?;
            if existing != *record {
                return Err(contract(
                    "FPS_PRESENTATION_PACKAGE_V2_IDEMPOTENCY_CONFLICT",
                    "idempotency key is already bound to a different package",
                ));
            }
            transaction.rollback()?;
            return Ok((existing, true));
        }
        let object_row: Option<(i64, String, String, String)> = transaction
            .query_row(
                "SELECT size_bytes,mime,kind,reachability FROM objects WHERE sha256=?1",
                params![object.sha256],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        if object_row
            != Some((
                object.size_bytes as i64,
                object.mime.clone(),
                object.kind.clone(),
                object.reachability.clone(),
            ))
            || !matches!(object.reachability.as_str(), "temporary" | "reachable")
        {
            return Err(contract(
                "FPS_PRESENTATION_PACKAGE_V2_CAS_ROW_INVALID",
                "package CAS registration differs",
            ));
        }
        let record_json = String::from_utf8(
            canonical_json_bytes(
                &serde_json::to_value(record)
                    .map_err(|error| StoreError::InvalidData(error.to_string()))?,
            )
            .map_err(|error| StoreError::InvalidData(error.to_string()))?,
        )
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        transaction.execute(&format!("INSERT INTO {TABLE} (schema_version,project_id,package_id,idempotency_key,package_object_sha256,package_canonical_sha256,weapon_materialization_id,weapon_descriptor_sha256,arms_materialization_id,arms_descriptor_sha256,animation_materialization_id,animation_descriptor_sha256,request_input_sha256,status,canonical_sha256,created_at,record_json) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)"), params![
            record.schema_version,record.project_id,record.package_id,record.idempotency_key,
            record.package_object_sha256,record.package_canonical_sha256,
            record.weapon_materialization_id,record.weapon_descriptor_sha256,
            record.arms_materialization_id,record.arms_descriptor_sha256,
            record.animation_materialization_id,record.animation_descriptor_sha256,
            record.request_input_sha256,record.status,record.canonical_sha256,record.created_at,record_json
        ])?;
        transaction.execute(
            "UPDATE objects SET reachability='reachable' WHERE sha256=?1",
            params![record.package_object_sha256],
        )?;
        transaction.commit()?;
        Ok((record.clone(), false))
    }

    pub fn get_fps_presentation_package_v2(
        &self,
        project_id: &str,
        package_id: &str,
    ) -> Result<Option<FpsPresentationPackageV2StoreRecord>, StoreError> {
        if !is_opaque_id(project_id) || !is_opaque_id(package_id) {
            return Err(StoreError::InvalidData(
                "composite package lookup identity is invalid".to_owned(),
            ));
        }
        let connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let json: Option<String> = connection
            .query_row(
                &format!("SELECT record_json FROM {TABLE} WHERE project_id=?1 AND package_id=?2"),
                params![project_id, package_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(json) = json else {
            return Ok(None);
        };
        let record: FpsPresentationPackageV2StoreRecord = serde_json::from_str(&json)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        validate_record(&record)?;
        drop(connection);
        for (materialization_id, descriptor_sha256) in [
            (
                &record.weapon_materialization_id,
                &record.weapon_descriptor_sha256,
            ),
            (
                &record.arms_materialization_id,
                &record.arms_descriptor_sha256,
            ),
            (
                &record.animation_materialization_id,
                &record.animation_descriptor_sha256,
            ),
        ] {
            let source = self
                .get_foundation_authoring_mesh_v2_materialization(project_id, materialization_id)?
                .ok_or_else(|| {
                    contract(
                        "FPS_PRESENTATION_PACKAGE_V2_SOURCE_MISSING",
                        "source materialization disappeared",
                    )
                })?;
            if source.descriptor_canonical_sha256 != *descriptor_sha256 {
                return Err(contract(
                    "FPS_PRESENTATION_PACKAGE_V2_SOURCE_MISMATCH",
                    "source descriptor changed",
                ));
            }
        }
        let bytes = self
            .cas
            .read_verified_bounded(&record.package_object_sha256, MAX_JSON_BYTES)
            .map_err(StoreError::Cas)?;
        let package: FpsPresentationPackageV2 = serde_json::from_slice(&bytes)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        if sha256_hex(&bytes) != record.package_object_sha256
            || validate_package(&record, &package)? != bytes
        {
            return Err(contract(
                "FPS_PRESENTATION_PACKAGE_V2_RESTART_MISMATCH",
                "package restart readback differs",
            ));
        }
        Ok(Some(record))
    }
}
