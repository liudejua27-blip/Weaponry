//! Store-local durable index for the explicit Low quad-draft source slice.
//!
//! This module is intentionally kept out of `store::lib.rs` by the source
//! lane.  The integration owner should add one `mod` declaration after the
//! contract names are frozen.  The table is created lazily so an existing V1
//! database can be opened without a migration; the first prepare creates the
//! additive table in the same SQLite connection used by the Runtime.

use forgecad_contracts::{
    is_opaque_id, is_sha256, CasObjectRecord, LowQuadDraftDurableRecord,
    LOW_QUAD_DRAFT_DURABLE_ARTIFACT_KIND, LOW_QUAD_DRAFT_DURABLE_OBJECT_KIND,
    LOW_QUAD_DRAFT_DURABLE_READBACK_KIND, LOW_QUAD_DRAFT_DURABLE_RECORD_SCHEMA_VERSION,
    LOW_QUAD_DRAFT_DURABLE_WORKER_RESULT_KIND,
};
use forgecad_core::{canonical_json_bytes, canonical_json_hash, sha256_hex};
use rusqlite::{params, OptionalExtension};
use serde_json::Value;

use super::{Store, StoreError};

const TABLE: &str = "low_quad_draft_durable_links";
const JSON_MIME: &str = "application/json";
const GLB_MIME: &str = "model/gltf-binary";
const MAX_JSON_BYTES: u64 = 8 * 1024 * 1024;
const MAX_GLB_BYTES: u64 = 64 * 1024 * 1024;

fn contract(code: &str, message: impl Into<String>) -> StoreError {
    StoreError::Contract {
        code: code.to_owned(),
        message: message.into(),
    }
}

pub(crate) fn ensure_table(connection: &rusqlite::Connection) -> Result<(), StoreError> {
    connection.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {TABLE} (
            schema_version TEXT NOT NULL CHECK (schema_version = 'LowQuadDraftDurableRecord@1'),
            project_id TEXT NOT NULL,
            candidate_id TEXT NOT NULL,
            candidate_state_sha256 TEXT NOT NULL,
            base_version_id TEXT,
            source_high_artifact_id TEXT NOT NULL,
            source_high_artifact_object_sha256 TEXT NOT NULL,
            source_high_artifact_sha256 TEXT NOT NULL,
            source_high_artifact_readback_object_sha256 TEXT NOT NULL,
            source_high_artifact_readback_sha256 TEXT NOT NULL,
            worker_result_object_sha256 TEXT NOT NULL,
            worker_result_sha256 TEXT NOT NULL,
            artifact_object_sha256 TEXT NOT NULL,
            artifact_sha256 TEXT NOT NULL,
            artifact_size_bytes INTEGER NOT NULL,
            readback_object_sha256 TEXT NOT NULL,
            readback_sha256 TEXT NOT NULL,
            link_id TEXT NOT NULL UNIQUE,
            link_object_sha256 TEXT NOT NULL,
            request_sha256 TEXT NOT NULL,
            input_sha256 TEXT NOT NULL,
            idempotency_key TEXT NOT NULL,
            worker_build_cohort_sha256 TEXT NOT NULL,
            materialization_status TEXT NOT NULL,
            canonical_sha256 TEXT NOT NULL,
            created_at TEXT NOT NULL,
            record_json TEXT NOT NULL,
            PRIMARY KEY (project_id, idempotency_key),
            UNIQUE (candidate_id, artifact_sha256)
        );
        CREATE INDEX IF NOT EXISTS low_quad_draft_durable_candidate_idx
            ON {TABLE}(candidate_id, created_at DESC, link_id ASC);
        CREATE INDEX IF NOT EXISTS low_quad_draft_durable_artifact_idx
            ON {TABLE}(artifact_sha256, link_id ASC);"
    ))?;
    Ok(())
}

fn record_value(record: &LowQuadDraftDurableRecord) -> Result<Value, StoreError> {
    serde_json::to_value(record).map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn record_bytes(record: &LowQuadDraftDurableRecord) -> Result<Vec<u8>, StoreError> {
    let value = record_value(record)?;
    canonical_json_bytes(&value).map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn validate_record_shape(record: &LowQuadDraftDurableRecord) -> Result<(), StoreError> {
    if record.schema_version != LOW_QUAD_DRAFT_DURABLE_RECORD_SCHEMA_VERSION
        || !is_opaque_id(&record.project_id)
        || !is_opaque_id(&record.candidate_id)
        || !is_opaque_id(&record.source_high_artifact_id)
        || !is_opaque_id(&record.link_id)
        || !is_opaque_id(&record.idempotency_key)
        || !is_sha256(&record.candidate_state_sha256)
        || !is_sha256(&record.source_high_artifact_object_sha256)
        || !is_sha256(&record.source_high_artifact_sha256)
        || !is_sha256(&record.source_high_artifact_readback_object_sha256)
        || !is_sha256(&record.source_high_artifact_readback_sha256)
        || !is_sha256(&record.worker_result_object_sha256)
        || !is_sha256(&record.worker_result_sha256)
        || !is_sha256(&record.artifact_object_sha256)
        || !is_sha256(&record.artifact_sha256)
        || !is_sha256(&record.readback_object_sha256)
        || !is_sha256(&record.readback_sha256)
        || !is_sha256(&record.link_object_sha256)
        || !is_sha256(&record.request_sha256)
        || !is_sha256(&record.input_sha256)
        || !is_sha256(&record.worker_build_cohort_sha256)
        || !is_sha256(&record.canonical_sha256)
        || record.artifact_size_bytes == 0
        || record.materialization_status.is_empty()
    {
        return Err(contract(
            "LOW_QUAD_DRAFT_DURABLE_RECORD_INVALID",
            "durable Low quad record has invalid identity, hash, size or status",
        ));
    }
    if let Some(base_version_id) = &record.base_version_id {
        if !is_opaque_id(base_version_id) {
            return Err(contract(
                "LOW_QUAD_DRAFT_DURABLE_RECORD_INVALID",
                "base_version_id is not an opaque id",
            ));
        }
    }
    let value = record_value(record)?;
    let mut preimage = value;
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != record.canonical_sha256 {
        return Err(contract(
            "LOW_QUAD_DRAFT_DURABLE_RECORD_CANONICAL_MISMATCH",
            "durable record canonical hash differs",
        ));
    }
    Ok(())
}

fn validate_object(
    store: &Store,
    object: &CasObjectRecord,
    expected_sha256: &str,
    expected_mime: &str,
    expected_kind: &str,
    max_bytes: u64,
) -> Result<(), StoreError> {
    if object.sha256 != expected_sha256
        || !is_sha256(expected_sha256)
        || object.mime != expected_mime
        || object.kind != expected_kind
        || object.size_bytes == 0
        || object.size_bytes > max_bytes
        || !matches!(object.reachability.as_str(), "temporary" | "reachable")
    {
        return Err(contract(
            "LOW_QUAD_DRAFT_DURABLE_CAS_METADATA_MISMATCH",
            "CAS object metadata differs from the durable binding",
        ));
    }
    let current = store.get_object(expected_sha256)?.ok_or_else(|| {
        contract(
            "LOW_QUAD_DRAFT_DURABLE_CAS_MISSING",
            "CAS object is missing",
        )
    })?;
    if current.sha256 != object.sha256
        || current.size_bytes != object.size_bytes
        || current.mime != object.mime
        || current.kind != object.kind
    {
        return Err(contract(
            "LOW_QUAD_DRAFT_DURABLE_CAS_METADATA_MISMATCH",
            "registered CAS metadata differs from the supplied object",
        ));
    }
    let bytes = store
        .cas
        .read_verified_bounded(expected_sha256, max_bytes)
        .map_err(StoreError::from)?;
    if bytes.len() as u64 != object.size_bytes || sha256_hex(&bytes) != expected_sha256 {
        return Err(contract(
            "LOW_QUAD_DRAFT_DURABLE_CAS_HASH_MISMATCH",
            "CAS bytes do not match their content hash",
        ));
    }
    Ok(())
}

fn validate_candidate(store: &Store, record: &LowQuadDraftDurableRecord) -> Result<(), StoreError> {
    let candidate = store.get_candidate(&record.candidate_id)?.ok_or_else(|| {
        contract(
            "LOW_QUAD_DRAFT_DURABLE_CANDIDATE_UNAVAILABLE",
            "candidate is missing",
        )
    })?;
    if candidate.project_id != record.project_id
        || candidate.canonical_sha256 != record.candidate_state_sha256
        || candidate.base_version_id != record.base_version_id
    {
        return Err(contract(
            "LOW_QUAD_DRAFT_DURABLE_CANDIDATE_BINDING_MISMATCH",
            "candidate project/state/base-version binding differs",
        ));
    }
    Ok(())
}

fn validate_objects(
    store: &Store,
    record: &LowQuadDraftDurableRecord,
    source_object: &CasObjectRecord,
    source_readback: &CasObjectRecord,
    worker_result: &CasObjectRecord,
    artifact: &CasObjectRecord,
    readback: &CasObjectRecord,
    link: &CasObjectRecord,
) -> Result<(), StoreError> {
    validate_object(
        store,
        source_object,
        &record.source_high_artifact_object_sha256,
        GLB_MIME,
        "production-weapon-high-artifact-glb",
        MAX_GLB_BYTES,
    )?;
    validate_object(
        store,
        source_readback,
        &record.source_high_artifact_readback_object_sha256,
        JSON_MIME,
        "native-high-glb-materialize-result",
        MAX_JSON_BYTES,
    )?;
    validate_object(
        store,
        worker_result,
        &record.worker_result_object_sha256,
        JSON_MIME,
        LOW_QUAD_DRAFT_DURABLE_WORKER_RESULT_KIND,
        MAX_JSON_BYTES,
    )?;
    validate_object(
        store,
        artifact,
        &record.artifact_object_sha256,
        GLB_MIME,
        LOW_QUAD_DRAFT_DURABLE_ARTIFACT_KIND,
        MAX_GLB_BYTES,
    )?;
    validate_object(
        store,
        readback,
        &record.readback_object_sha256,
        JSON_MIME,
        LOW_QUAD_DRAFT_DURABLE_READBACK_KIND,
        MAX_JSON_BYTES,
    )?;
    validate_object(
        store,
        link,
        &record.link_object_sha256,
        JSON_MIME,
        LOW_QUAD_DRAFT_DURABLE_OBJECT_KIND,
        MAX_JSON_BYTES,
    )?;
    Ok(())
}

fn read_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<LowQuadDraftDurableRecord> {
    let payload: String = row.get(0)?;
    serde_json::from_str(&payload).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn same_record(left: &LowQuadDraftDurableRecord, right: &LowQuadDraftDurableRecord) -> bool {
    left == right
}

impl Store {
    /// Atomically bind the explicit quad draft CAS bundle to one candidate.
    /// The six CAS roots are registered by Runtime before this transaction;
    /// this method only verifies metadata/bytes, creates the additive SQLite
    /// row, marks all roots reachable, and enforces idempotency/conflict rules.
    pub fn record_low_quad_draft_durable_with_replay(
        &self,
        record: &LowQuadDraftDurableRecord,
        source_object: &CasObjectRecord,
        source_readback: &CasObjectRecord,
        worker_result: &CasObjectRecord,
        artifact: &CasObjectRecord,
        readback: &CasObjectRecord,
        link: &CasObjectRecord,
    ) -> Result<(LowQuadDraftDurableRecord, bool), StoreError> {
        validate_record_shape(record)?;
        validate_candidate(self, record)?;
        validate_objects(
            self,
            record,
            source_object,
            source_readback,
            worker_result,
            artifact,
            readback,
            link,
        )?;
        if artifact.size_bytes != record.artifact_size_bytes {
            return Err(contract(
                "LOW_QUAD_DRAFT_DURABLE_ARTIFACT_SIZE_MISMATCH",
                "artifact size differs from the durable record",
            ));
        }
        let payload_json = String::from_utf8(record_bytes(record)?).map_err(|error| {
            StoreError::InvalidData(format!("durable record JSON is not UTF-8: {error}"))
        })?;
        let mut connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let transaction = connection.transaction()?;
        let existing = transaction
            .query_row(
                &format!("SELECT record_json FROM {TABLE} WHERE project_id = ?1 AND idempotency_key = ?2"),
                params![record.project_id, record.idempotency_key],
                read_record,
            )
            .optional()?;
        let reachable = [
            record.source_high_artifact_object_sha256.clone(),
            record.source_high_artifact_readback_object_sha256.clone(),
            record.worker_result_object_sha256.clone(),
            record.artifact_object_sha256.clone(),
            record.readback_object_sha256.clone(),
            record.link_object_sha256.clone(),
        ];
        if let Some(existing) = existing {
            if !same_record(&existing, record) {
                return Err(contract(
                    "LOW_QUAD_DRAFT_DURABLE_RECORD_CONFLICT",
                    "project/idempotency key is bound to different Low quad metadata",
                ));
            }
            super::mark_reachable_in_transaction(&transaction, &reachable)?;
            transaction.commit()?;
            return Ok((existing, true));
        }
        let key_conflict: Option<String> = transaction
            .query_row(
                &format!("SELECT link_id FROM {TABLE} WHERE link_id = ?1 OR (candidate_id = ?2 AND artifact_sha256 = ?3)"),
                params![record.link_id, record.candidate_id, record.artifact_sha256],
                |row| row.get(0),
            )
            .optional()?;
        if key_conflict.is_some() {
            return Err(contract(
                "LOW_QUAD_DRAFT_DURABLE_RECORD_CONFLICT",
                "link or candidate/artifact identity is already bound",
            ));
        }
        transaction.execute(
            &format!("INSERT INTO {TABLE} (schema_version, project_id, candidate_id, candidate_state_sha256, base_version_id, source_high_artifact_id, source_high_artifact_object_sha256, source_high_artifact_sha256, source_high_artifact_readback_object_sha256, source_high_artifact_readback_sha256, worker_result_object_sha256, worker_result_sha256, artifact_object_sha256, artifact_sha256, artifact_size_bytes, readback_object_sha256, readback_sha256, link_id, link_object_sha256, request_sha256, input_sha256, idempotency_key, worker_build_cohort_sha256, materialization_status, canonical_sha256, created_at, record_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27)"),
            params![
                record.schema_version,
                record.project_id,
                record.candidate_id,
                record.candidate_state_sha256,
                record.base_version_id,
                record.source_high_artifact_id,
                record.source_high_artifact_object_sha256,
                record.source_high_artifact_sha256,
                record.source_high_artifact_readback_object_sha256,
                record.source_high_artifact_readback_sha256,
                record.worker_result_object_sha256,
                record.worker_result_sha256,
                record.artifact_object_sha256,
                record.artifact_sha256,
                i64::try_from(record.artifact_size_bytes).map_err(|_| {
                    StoreError::InvalidData("Low quad artifact is too large".to_owned())
                })?,
                record.readback_object_sha256,
                record.readback_sha256,
                record.link_id,
                record.link_object_sha256,
                record.request_sha256,
                record.input_sha256,
                record.idempotency_key,
                record.worker_build_cohort_sha256,
                record.materialization_status,
                record.canonical_sha256,
                record.created_at,
                payload_json,
            ],
        )?;
        super::mark_reachable_in_transaction(&transaction, &reachable)?;
        let stored = transaction.query_row(
            &format!(
                "SELECT record_json FROM {TABLE} WHERE project_id = ?1 AND idempotency_key = ?2"
            ),
            params![record.project_id, record.idempotency_key],
            read_record,
        )?;
        transaction.commit()?;
        Ok((stored, false))
    }

    pub fn get_low_quad_draft_durable(
        &self,
        project_id: &str,
        idempotency_key: &str,
    ) -> Result<Option<LowQuadDraftDurableRecord>, StoreError> {
        if !is_opaque_id(project_id) || !is_opaque_id(idempotency_key) {
            return Err(StoreError::InvalidData(
                "Low quad durable lookup identity is invalid".to_owned(),
            ));
        }
        let connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let Some(record) = connection
            .query_row(
                &format!("SELECT record_json FROM {TABLE} WHERE project_id = ?1 AND idempotency_key = ?2"),
                params![project_id, idempotency_key],
                read_record,
            )
            .optional()?
        else {
            return Ok(None);
        };
        drop(connection);
        if record.project_id != project_id || record.idempotency_key != idempotency_key {
            return Err(contract(
                "LOW_QUAD_DRAFT_DURABLE_RECORD_SCOPE_MISMATCH",
                "stored record scope differs",
            ));
        }
        validate_record_shape(&record)?;
        validate_candidate(self, &record)?;
        let objects = [
            (
                &record.source_high_artifact_object_sha256,
                GLB_MIME,
                "production-weapon-high-artifact-glb",
                MAX_GLB_BYTES,
            ),
            (
                &record.source_high_artifact_readback_object_sha256,
                JSON_MIME,
                "native-high-glb-materialize-result",
                MAX_JSON_BYTES,
            ),
            (
                &record.worker_result_object_sha256,
                JSON_MIME,
                LOW_QUAD_DRAFT_DURABLE_WORKER_RESULT_KIND,
                MAX_JSON_BYTES,
            ),
            (
                &record.artifact_object_sha256,
                GLB_MIME,
                LOW_QUAD_DRAFT_DURABLE_ARTIFACT_KIND,
                MAX_GLB_BYTES,
            ),
            (
                &record.readback_object_sha256,
                JSON_MIME,
                LOW_QUAD_DRAFT_DURABLE_READBACK_KIND,
                MAX_JSON_BYTES,
            ),
            (
                &record.link_object_sha256,
                JSON_MIME,
                LOW_QUAD_DRAFT_DURABLE_OBJECT_KIND,
                MAX_JSON_BYTES,
            ),
        ];
        for (hash, mime, kind, max_bytes) in objects {
            let object = self.get_object(hash)?.ok_or_else(|| {
                contract(
                    "LOW_QUAD_DRAFT_DURABLE_CAS_MISSING",
                    "CAS object is missing",
                )
            })?;
            validate_object(self, &object, hash, mime, kind, max_bytes)?;
        }
        Ok(Some(record))
    }

    pub fn get_low_quad_draft_durable_by_link_id(
        &self,
        link_id: &str,
    ) -> Result<Option<LowQuadDraftDurableRecord>, StoreError> {
        if !is_opaque_id(link_id) {
            return Err(StoreError::InvalidData(
                "Low quad durable link identity is invalid".to_owned(),
            ));
        }
        let connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let identity: Option<(String, String)> = connection
            .query_row(
                &format!("SELECT project_id, idempotency_key FROM {TABLE} WHERE link_id = ?1"),
                params![link_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        drop(connection);
        let Some((project_id, idempotency_key)) = identity else {
            return Ok(None);
        };
        self.get_low_quad_draft_durable(&project_id, &idempotency_key)
    }

    /// Resolve the exact Low durable provenance row for a candidate/artifact
    /// pair and then run the complete candidate/CAS/readback validation.
    pub fn get_low_quad_draft_durable_by_candidate_artifact(
        &self,
        candidate_id: &str,
        artifact_sha256: &str,
    ) -> Result<Option<LowQuadDraftDurableRecord>, StoreError> {
        if !is_opaque_id(candidate_id) || !is_sha256(artifact_sha256) {
            return Err(StoreError::InvalidData(
                "Low quad durable candidate/artifact lookup identity is invalid".to_owned(),
            ));
        }
        let connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let identity: Option<(String, String)> = connection
            .query_row(
                &format!(
                    "SELECT project_id, idempotency_key FROM {TABLE} WHERE candidate_id = ?1 AND artifact_sha256 = ?2"
                ),
                params![candidate_id, artifact_sha256],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        drop(connection);
        let Some((project_id, idempotency_key)) = identity else {
            return Ok(None);
        };
        self.get_low_quad_draft_durable(&project_id, &idempotency_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    #[test]
    fn table_name_is_additive_and_schema_kind_is_closed() {
        assert_eq!(TABLE, "low_quad_draft_durable_links");
        assert_eq!(
            forgecad_contracts::LOW_QUAD_DRAFT_DURABLE_LINK_SCHEMA_VERSION,
            "LowQuadDraftDurableLink@1"
        );
        assert_eq!(
            forgecad_contracts::LOW_QUAD_DRAFT_DURABLE_ARTIFACT_READBACK_SCHEMA_VERSION,
            "LowQuadDraftArtifactReadback@1"
        );
    }

    #[test]
    fn shared_linked_query_lazily_creates_table_and_recognizes_all_roots() {
        let store = Store::memory().expect("store");
        let objects = [
            (
                b"source-high".as_slice(),
                GLB_MIME,
                "production-weapon-high-artifact-glb",
            ),
            (
                b"source-readback".as_slice(),
                JSON_MIME,
                "native-high-glb-materialize-result",
            ),
            (
                b"worker-result".as_slice(),
                JSON_MIME,
                LOW_QUAD_DRAFT_DURABLE_WORKER_RESULT_KIND,
            ),
            (
                b"low-artifact".as_slice(),
                GLB_MIME,
                LOW_QUAD_DRAFT_DURABLE_ARTIFACT_KIND,
            ),
            (
                b"low-readback".as_slice(),
                JSON_MIME,
                LOW_QUAD_DRAFT_DURABLE_READBACK_KIND,
            ),
            (
                b"low-link".as_slice(),
                JSON_MIME,
                LOW_QUAD_DRAFT_DURABLE_OBJECT_KIND,
            ),
        ]
        .into_iter()
        .map(|(bytes, mime, kind)| {
            store
                .put_object(bytes, None, mime, kind, "1")
                .expect("CAS object")
        })
        .collect::<Vec<_>>();
        let hashes = objects
            .iter()
            .map(|object| object.record.sha256.clone())
            .collect::<Vec<_>>();
        let mut connection = store.lock_connection().expect("connection");
        let transaction = connection.transaction().expect("transaction");
        let table_count: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                params![TABLE],
                |row| row.get(0),
            )
            .expect("table probe");
        assert_eq!(
            table_count, 0,
            "Low table should remain lazy before linked query"
        );

        let unknown = "0".repeat(64);
        assert!(
            !super::super::authoring_mesh_edit_object_is_linked(&transaction, &unknown,)
                .expect("unknown linked query")
        );
        let table_count: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                params![TABLE],
                |row| row.get(0),
            )
            .expect("table probe after ensure");
        assert_eq!(
            table_count, 1,
            "linked query must ensure the lazy Low table"
        );

        transaction
            .execute(
                &format!(
                    "INSERT INTO {TABLE} (schema_version, project_id, candidate_id, candidate_state_sha256, base_version_id, source_high_artifact_id, source_high_artifact_object_sha256, source_high_artifact_sha256, source_high_artifact_readback_object_sha256, source_high_artifact_readback_sha256, worker_result_object_sha256, worker_result_sha256, artifact_object_sha256, artifact_sha256, artifact_size_bytes, readback_object_sha256, readback_sha256, link_id, link_object_sha256, request_sha256, input_sha256, idempotency_key, worker_build_cohort_sha256, materialization_status, canonical_sha256, created_at, record_json) VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)"
                ),
                params![
                    LOW_QUAD_DRAFT_DURABLE_RECORD_SCHEMA_VERSION,
                    "project-low-linked",
                    "candidate-low-linked",
                    "a".repeat(64),
                    "source-high",
                    hashes[0],
                    "b".repeat(64),
                    hashes[1],
                    "c".repeat(64),
                    hashes[2],
                    "d".repeat(64),
                    hashes[3],
                    "e".repeat(64),
                    objects[3].record.size_bytes as i64,
                    hashes[4],
                    "f".repeat(64),
                    "link-low-linked",
                    hashes[5],
                    "1".repeat(64),
                    "2".repeat(64),
                    "idempotency-low-linked",
                    "3".repeat(64),
                    "status",
                    "4".repeat(64),
                    "1",
                    "{}",
                ],
            )
            .expect("Low durable row");
        for hash in &hashes {
            assert!(
                super::super::authoring_mesh_edit_object_is_linked(&transaction, hash,)
                    .expect("root linked query")
            );
        }
        assert!(
            !super::super::authoring_mesh_edit_object_is_linked(&transaction, &unknown,)
                .expect("unknown linked query after row")
        );
        transaction.commit().expect("commit");
    }
}
