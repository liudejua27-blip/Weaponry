//! Store-local durability for the first Weaponry curve authoring slice.
//!
//! This module is deliberately a metadata boundary.  It records the source
//! revision identity, curve/sample-set, modifier graph, dependency graph and
//! recompute-plan hashes; it does not evaluate a graph or persist mesh data.
//! Runtime stages the immutable JSON objects in CAS and this module verifies
//! their registered metadata before atomically installing the SQLite binding.

use super::{
    CasObjectRecord, CasStore, Store, StoreError, canonical_json_bytes, canonical_json_hash,
    is_opaque_id, is_sha256, mark_reachable_in_transaction,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const WEAPONRY_CURVE_MODIFIER_GRAPH_RECORD_SCHEMA: &str =
    "WeaponryCurveModifierGraphDurableRecord@1";
pub const WEAPONRY_CURVE_MODIFIER_GRAPH_STATUS: &str =
    "runtime-owned-store-weaponry-curve-modifier-graph@1";
pub const WEAPONRY_CURVE_SET_OBJECT_KIND: &str = "weaponry-curve-set";
pub const WEAPONRY_SAMPLE_SET_OBJECT_KIND: &str = "weaponry-curve-sample-set";
pub const WEAPONRY_MODIFIER_GRAPH_OBJECT_KIND: &str = "weaponry-modifier-graph";
pub const WEAPONRY_DEPENDENCY_GRAPH_OBJECT_KIND: &str = "weaponry-dependency-graph";
pub const WEAPONRY_RECOMPUTE_PLAN_OBJECT_KIND: &str = "weaponry-recompute-plan";
pub const WEAPONRY_CURVE_MODIFIER_GRAPH_JSON_MIME: &str = "application/json";
pub const WEAPONRY_CURVE_MODIFIER_GRAPH_MAX_JSON_BYTES: u64 = 8 * 1024 * 1024;

/// Store-local durable binding for the core's curve/sample-set and graph
/// hashes.  The semantic hashes may differ from the CAS object hashes: the
/// former identify canonical domain content while the latter identify the
/// immutable stored bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaponryCurveModifierGraphDurableRecord {
    pub schema_version: String,
    pub project_id: String,
    pub source_revision_id: String,
    pub source_revision_sha256: String,
    pub source_candidate_id: String,
    pub source_candidate_state_sha256: String,
    pub source_authoring_mesh_id: String,
    pub source_authoring_mesh_lineage_id: String,
    pub source_authoring_mesh_revision_index: u64,
    pub source_authoring_mesh_identity_sha256: String,
    pub curve_set_id: String,
    pub curve_set_sha256: String,
    pub curve_set_object_sha256: String,
    pub sample_set_id: String,
    pub sample_set_sha256: String,
    pub sample_set_object_sha256: String,
    pub modifier_graph_id: String,
    pub modifier_graph_sha256: String,
    pub modifier_graph_object_sha256: String,
    pub dependency_graph_sha256: String,
    pub dependency_graph_object_sha256: String,
    pub recompute_plan_sha256: String,
    pub recompute_plan_object_sha256: String,
    pub lookup_key_sha256: String,
    pub idempotency_key: String,
    pub input_sha256: String,
    pub materialization_status: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// The five immutable CAS objects that a Runtime caller stages before the
/// Store transaction.  Store does not put or delete these objects.
#[derive(Debug, Clone)]
pub struct WeaponryCurveModifierGraphCasBundle {
    pub curve_set: CasObjectRecord,
    pub sample_set: CasObjectRecord,
    pub modifier_graph: CasObjectRecord,
    pub dependency_graph: CasObjectRecord,
    pub recompute_plan: CasObjectRecord,
}

#[derive(Debug, Clone)]
pub struct WeaponryCurveModifierGraphCommit {
    pub record: WeaponryCurveModifierGraphDurableRecord,
    pub cas: WeaponryCurveModifierGraphCasBundle,
}

fn contract(code: &str, message: impl Into<String>) -> StoreError {
    StoreError::Contract {
        code: code.to_owned(),
        message: message.into(),
    }
}

fn record_value(record: &WeaponryCurveModifierGraphDurableRecord) -> Result<Value, StoreError> {
    serde_json::to_value(record).map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn canonical_record_value(
    record: &WeaponryCurveModifierGraphDurableRecord,
) -> Result<Value, StoreError> {
    let mut value = record_value(record)?;
    value["canonical_sha256"] = Value::String(String::new());
    Ok(value)
}

fn canonical_record_sha256(
    record: &WeaponryCurveModifierGraphDurableRecord,
) -> Result<String, StoreError> {
    Ok(canonical_json_hash(&canonical_record_value(record)?))
}

fn canonical_record_bytes(
    record: &WeaponryCurveModifierGraphDurableRecord,
) -> Result<Vec<u8>, StoreError> {
    canonical_json_bytes(&record_value(record)?)
        .map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn validate_record(record: &WeaponryCurveModifierGraphDurableRecord) -> Result<(), StoreError> {
    let ids = [
        record.project_id.as_str(),
        record.source_revision_id.as_str(),
        record.source_candidate_id.as_str(),
        record.source_authoring_mesh_id.as_str(),
        record.source_authoring_mesh_lineage_id.as_str(),
        record.curve_set_id.as_str(),
        record.sample_set_id.as_str(),
        record.modifier_graph_id.as_str(),
        record.idempotency_key.as_str(),
    ];
    let hashes = [
        record.source_revision_sha256.as_str(),
        record.source_candidate_state_sha256.as_str(),
        record.source_authoring_mesh_identity_sha256.as_str(),
        record.curve_set_sha256.as_str(),
        record.curve_set_object_sha256.as_str(),
        record.sample_set_sha256.as_str(),
        record.sample_set_object_sha256.as_str(),
        record.modifier_graph_sha256.as_str(),
        record.modifier_graph_object_sha256.as_str(),
        record.dependency_graph_sha256.as_str(),
        record.dependency_graph_object_sha256.as_str(),
        record.recompute_plan_sha256.as_str(),
        record.recompute_plan_object_sha256.as_str(),
        record.lookup_key_sha256.as_str(),
        record.input_sha256.as_str(),
        record.canonical_sha256.as_str(),
    ];
    if record.schema_version != WEAPONRY_CURVE_MODIFIER_GRAPH_RECORD_SCHEMA
        || ids.iter().any(|value| !is_opaque_id(value))
        || hashes.iter().any(|value| !is_sha256(value))
        || record.materialization_status != WEAPONRY_CURVE_MODIFIER_GRAPH_STATUS
        || record.source_authoring_mesh_revision_index > 1_000_000
        || record.idempotency_key.len() > 128
        || record.created_at.is_empty()
        || record.created_at.len() > 128
    {
        return Err(contract(
            "WEAPONRY_CURVE_MODIFIER_GRAPH_RECORD_INVALID",
            "curve/modifier-graph durable identity or hash is malformed",
        ));
    }
    if canonical_record_sha256(record)? != record.canonical_sha256 {
        return Err(contract(
            "WEAPONRY_CURVE_MODIFIER_GRAPH_CANONICAL_MISMATCH",
            "curve/modifier-graph durable record canonical hash differs",
        ));
    }
    Ok(())
}

fn roots(record: &WeaponryCurveModifierGraphDurableRecord) -> Vec<String> {
    let mut roots = vec![
        record.curve_set_object_sha256.clone(),
        record.sample_set_object_sha256.clone(),
        record.modifier_graph_object_sha256.clone(),
        record.dependency_graph_object_sha256.clone(),
        record.recompute_plan_object_sha256.clone(),
    ];
    roots.sort();
    roots.dedup();
    roots
}

fn validate_registered_object(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    object: &CasObjectRecord,
    expected_kind: &str,
    require_reachable: bool,
) -> Result<(), StoreError> {
    if object.schema_version != "CasObject@1"
        || !is_sha256(&object.sha256)
        || object.mime != WEAPONRY_CURVE_MODIFIER_GRAPH_JSON_MIME
        || object.kind != expected_kind
        || object.size_bytes == 0
        || object.size_bytes > WEAPONRY_CURVE_MODIFIER_GRAPH_MAX_JSON_BYTES
        || !matches!(object.reachability.as_str(), "temporary" | "reachable")
        || (require_reachable && object.reachability != "reachable")
        || object.created_at.is_empty()
        || object.created_at.len() > 128
    {
        return Err(contract(
            "WEAPONRY_CURVE_MODIFIER_GRAPH_CAS_METADATA_INVALID",
            "curve/modifier-graph CAS metadata is outside the bounded allowlist",
        ));
    }
    let stored: Option<(i64, String, String, String, String)> = transaction
        .query_row(
            "SELECT size_bytes, mime, kind, reachability, created_at FROM objects WHERE sha256 = ?1",
            params![object.sha256],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()?;
    let Some((size, mime, kind, reachability, created_at)) = stored else {
        return Err(contract(
            "WEAPONRY_CURVE_MODIFIER_GRAPH_CAS_MISSING",
            "curve/modifier-graph CAS object is not registered",
        ));
    };
    let reachability_matches = object.reachability == reachability
        || (object.reachability == "temporary" && reachability == "reachable");
    if size != i64::try_from(object.size_bytes).unwrap_or(i64::MAX)
        || mime != object.mime
        || kind != object.kind
        || !reachability_matches
        || created_at != object.created_at
    {
        return Err(contract(
            "WEAPONRY_CURVE_MODIFIER_GRAPH_CAS_METADATA_MISMATCH",
            "curve/modifier-graph CAS metadata differs from SQLite",
        ));
    }
    let bytes = cas
        .read_verified_bounded(&object.sha256, WEAPONRY_CURVE_MODIFIER_GRAPH_MAX_JSON_BYTES)
        .map_err(StoreError::from)?;
    serde_json::from_slice::<Value>(&bytes).map_err(|error| {
        contract(
            "WEAPONRY_CURVE_MODIFIER_GRAPH_CAS_JSON_INVALID",
            format!("curve/modifier-graph CAS object is not JSON: {error}"),
        )
    })?;
    let canonical = canonical_json_bytes(
        &serde_json::from_slice::<Value>(&bytes)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?,
    )
    .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    if canonical != bytes {
        return Err(contract(
            "WEAPONRY_CURVE_MODIFIER_GRAPH_CAS_NOT_CANONICAL",
            "curve/modifier-graph CAS JSON must use canonical encoding",
        ));
    }
    Ok(())
}

fn same_record(
    left: &WeaponryCurveModifierGraphDurableRecord,
    right: &WeaponryCurveModifierGraphDurableRecord,
) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    left.created_at.clear();
    right.created_at.clear();
    left.canonical_sha256.clear();
    right.canonical_sha256.clear();
    left == right
}

fn read_record(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<WeaponryCurveModifierGraphDurableRecord> {
    let payload: String = row.get(0)?;
    serde_json::from_str(&payload).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
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
                let size_bytes: i64 = row.get(1)?;
                Ok(CasObjectRecord {
                    schema_version: "CasObject@1".to_owned(),
                    sha256: row.get(0)?,
                    size_bytes: u64::try_from(size_bytes)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    mime: row.get(2)?,
                    kind: row.get(3)?,
                    reachability: row.get(4)?,
                    created_at: row.get(5)?,
                })
            },
        )
        .map_err(StoreError::from)
}

pub(crate) fn ensure_table(connection: &Connection) -> Result<(), StoreError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS weaponry_curve_modifier_graph_records (
             schema_version TEXT NOT NULL CHECK (schema_version = 'WeaponryCurveModifierGraphDurableRecord@1'),
             project_id TEXT NOT NULL REFERENCES projects(project_id),
             source_revision_id TEXT NOT NULL,
             source_revision_sha256 TEXT NOT NULL,
             source_candidate_id TEXT NOT NULL,
             source_candidate_state_sha256 TEXT NOT NULL,
             source_authoring_mesh_id TEXT NOT NULL,
             source_authoring_mesh_lineage_id TEXT NOT NULL,
             source_authoring_mesh_revision_index INTEGER NOT NULL CHECK (source_authoring_mesh_revision_index BETWEEN 0 AND 1000000),
             source_authoring_mesh_identity_sha256 TEXT NOT NULL,
             curve_set_id TEXT NOT NULL,
             curve_set_sha256 TEXT NOT NULL,
             curve_set_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             sample_set_id TEXT NOT NULL,
             sample_set_sha256 TEXT NOT NULL,
             sample_set_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             modifier_graph_id TEXT NOT NULL,
             modifier_graph_sha256 TEXT NOT NULL,
             modifier_graph_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             dependency_graph_sha256 TEXT NOT NULL,
             dependency_graph_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             recompute_plan_sha256 TEXT NOT NULL,
             recompute_plan_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             lookup_key_sha256 TEXT NOT NULL,
             idempotency_key TEXT NOT NULL,
             input_sha256 TEXT NOT NULL,
             materialization_status TEXT NOT NULL CHECK (materialization_status = 'runtime-owned-store-weaponry-curve-modifier-graph@1'),
             canonical_sha256 TEXT NOT NULL,
             created_at TEXT NOT NULL,
             record_json TEXT NOT NULL,
             PRIMARY KEY (project_id, lookup_key_sha256),
             UNIQUE (project_id, idempotency_key)
         );
         CREATE INDEX IF NOT EXISTS weaponry_curve_modifier_graph_source_idx
             ON weaponry_curve_modifier_graph_records(project_id, source_revision_id, created_at DESC);
         CREATE INDEX IF NOT EXISTS weaponry_curve_modifier_graph_object_idx
             ON weaponry_curve_modifier_graph_records(curve_set_object_sha256,
                                                       sample_set_object_sha256,
                                                       modifier_graph_object_sha256,
                                                       dependency_graph_object_sha256,
                                                       recompute_plan_object_sha256);",
    )?;
    Ok(())
}

impl Store {
    /// Read one of the five bounded curve/graph JSON roots after checking its
    /// SQLite registration, allowlisted role, reachable state and verified
    /// canonical bytes. The caller compares the returned canonical value's
    /// semantic hash with the durable record's semantic binding.
    pub fn read_weaponry_curve_modifier_graph_json(
        &self,
        sha256: &str,
        expected_kind: &str,
    ) -> Result<Value, StoreError> {
        if !is_sha256(sha256)
            || !matches!(
                expected_kind,
                WEAPONRY_CURVE_SET_OBJECT_KIND
                    | WEAPONRY_SAMPLE_SET_OBJECT_KIND
                    | WEAPONRY_MODIFIER_GRAPH_OBJECT_KIND
                    | WEAPONRY_DEPENDENCY_GRAPH_OBJECT_KIND
                    | WEAPONRY_RECOMPUTE_PLAN_OBJECT_KIND
            )
        {
            return Err(StoreError::InvalidData(
                "curve/modifier-graph JSON root identity or kind is invalid".to_owned(),
            ));
        }
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let object = read_object_record(&transaction, sha256).map_err(|error| match error {
            StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => contract(
                "WEAPONRY_CURVE_MODIFIER_GRAPH_CAS_MISSING",
                "curve/modifier-graph JSON root is not registered",
            ),
            other => other,
        })?;
        validate_registered_object(&transaction, &self.cas, &object, expected_kind, true)?;
        let bytes = self
            .cas
            .read_verified_bounded(sha256, WEAPONRY_CURVE_MODIFIER_GRAPH_MAX_JSON_BYTES)
            .map_err(StoreError::from)?;
        let value = serde_json::from_slice::<Value>(&bytes).map_err(|error| {
            contract(
                "WEAPONRY_CURVE_MODIFIER_GRAPH_CAS_JSON_INVALID",
                format!("curve/modifier-graph JSON root is invalid: {error}"),
            )
        })?;
        transaction.commit()?;
        Ok(value)
    }

    /// Atomically install the Store-local curve/graph binding and promote all
    /// five staged CAS roots. A matching idempotency key returns `(record,
    /// true)`; any binding difference is a typed conflict.
    pub fn record_weaponry_curve_modifier_graph_with_replay(
        &self,
        commit: &WeaponryCurveModifierGraphCommit,
    ) -> Result<(WeaponryCurveModifierGraphDurableRecord, bool), StoreError> {
        validate_record(&commit.record)?;
        let expected = [
            (&commit.cas.curve_set, WEAPONRY_CURVE_SET_OBJECT_KIND),
            (&commit.cas.sample_set, WEAPONRY_SAMPLE_SET_OBJECT_KIND),
            (
                &commit.cas.modifier_graph,
                WEAPONRY_MODIFIER_GRAPH_OBJECT_KIND,
            ),
            (
                &commit.cas.dependency_graph,
                WEAPONRY_DEPENDENCY_GRAPH_OBJECT_KIND,
            ),
            (
                &commit.cas.recompute_plan,
                WEAPONRY_RECOMPUTE_PLAN_OBJECT_KIND,
            ),
        ];
        let object_hashes = [
            commit.record.curve_set_object_sha256.as_str(),
            commit.record.sample_set_object_sha256.as_str(),
            commit.record.modifier_graph_object_sha256.as_str(),
            commit.record.dependency_graph_object_sha256.as_str(),
            commit.record.recompute_plan_object_sha256.as_str(),
        ];
        for ((object, kind), expected_hash) in expected.iter().zip(object_hashes) {
            if object.sha256 != expected_hash {
                return Err(contract(
                    "WEAPONRY_CURVE_MODIFIER_GRAPH_CAS_BINDING_MISMATCH",
                    "CAS object hash differs from durable record binding",
                ));
            }
            if object.kind != *kind {
                return Err(contract(
                    "WEAPONRY_CURVE_MODIFIER_GRAPH_CAS_METADATA_INVALID",
                    "CAS object kind differs from its typed root role",
                ));
            }
        }
        let mut connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let transaction = connection.transaction()?;
        let existing = transaction
            .query_row(
                "SELECT record_json FROM weaponry_curve_modifier_graph_records WHERE project_id = ?1 AND idempotency_key = ?2",
                params![commit.record.project_id, commit.record.idempotency_key],
                read_record,
            )
            .optional()?;
        if let Some(existing) = existing {
            validate_record(&existing)?;
            if !same_record(&existing, &commit.record) {
                return Err(contract(
                    "WEAPONRY_CURVE_MODIFIER_GRAPH_IDEMPOTENCY_CONFLICT",
                    "idempotency key is already bound to different curve/graph input",
                ));
            }
            for ((object, kind), _) in expected.iter().zip(object_hashes) {
                validate_registered_object(&transaction, &self.cas, object, kind, false)?;
            }
            mark_reachable_in_transaction(&transaction, &roots(&existing))?;
            transaction.commit()?;
            return Ok((existing, true));
        }
        let duplicate: Option<String> = transaction
            .query_row(
                "SELECT lookup_key_sha256 FROM weaponry_curve_modifier_graph_records WHERE project_id = ?1 AND lookup_key_sha256 = ?2",
                params![commit.record.project_id, commit.record.lookup_key_sha256],
                |row| row.get(0),
            )
            .optional()?;
        if duplicate.is_some() {
            return Err(contract(
                "WEAPONRY_CURVE_MODIFIER_GRAPH_LOOKUP_CONFLICT",
                "lookup key is already bound to another input",
            ));
        }
        for ((object, kind), _) in expected.iter().zip(object_hashes) {
            validate_registered_object(&transaction, &self.cas, object, kind, false)?;
        }
        let record_json =
            String::from_utf8(canonical_record_bytes(&commit.record)?).map_err(|error| {
                StoreError::InvalidData(format!(
                    "curve/modifier-graph durable record is not UTF-8: {error}"
                ))
            })?;
        transaction.execute(
            "INSERT INTO weaponry_curve_modifier_graph_records (schema_version, project_id, source_revision_id, source_revision_sha256, source_candidate_id, source_candidate_state_sha256, source_authoring_mesh_id, source_authoring_mesh_lineage_id, source_authoring_mesh_revision_index, source_authoring_mesh_identity_sha256, curve_set_id, curve_set_sha256, curve_set_object_sha256, sample_set_id, sample_set_sha256, sample_set_object_sha256, modifier_graph_id, modifier_graph_sha256, modifier_graph_object_sha256, dependency_graph_sha256, dependency_graph_object_sha256, recompute_plan_sha256, recompute_plan_object_sha256, lookup_key_sha256, idempotency_key, input_sha256, materialization_status, canonical_sha256, created_at, record_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30)",
            params![
                commit.record.schema_version,
                commit.record.project_id,
                commit.record.source_revision_id,
                commit.record.source_revision_sha256,
                commit.record.source_candidate_id,
                commit.record.source_candidate_state_sha256,
                commit.record.source_authoring_mesh_id,
                commit.record.source_authoring_mesh_lineage_id,
                i64::try_from(commit.record.source_authoring_mesh_revision_index).map_err(|_| StoreError::InvalidData("authoring mesh revision index is too large".to_owned()))?,
                commit.record.source_authoring_mesh_identity_sha256,
                commit.record.curve_set_id,
                commit.record.curve_set_sha256,
                commit.record.curve_set_object_sha256,
                commit.record.sample_set_id,
                commit.record.sample_set_sha256,
                commit.record.sample_set_object_sha256,
                commit.record.modifier_graph_id,
                commit.record.modifier_graph_sha256,
                commit.record.modifier_graph_object_sha256,
                commit.record.dependency_graph_sha256,
                commit.record.dependency_graph_object_sha256,
                commit.record.recompute_plan_sha256,
                commit.record.recompute_plan_object_sha256,
                commit.record.lookup_key_sha256,
                commit.record.idempotency_key,
                commit.record.input_sha256,
                commit.record.materialization_status,
                commit.record.canonical_sha256,
                commit.record.created_at,
                record_json,
            ],
        )?;
        mark_reachable_in_transaction(&transaction, &roots(&commit.record))?;
        let stored = transaction.query_row(
            "SELECT record_json FROM weaponry_curve_modifier_graph_records WHERE project_id = ?1 AND idempotency_key = ?2",
            params![commit.record.project_id, commit.record.idempotency_key],
            read_record,
        )?;
        transaction.commit()?;
        Ok((stored, false))
    }

    pub fn get_weaponry_curve_modifier_graph(
        &self,
        project_id: &str,
        lookup_key_sha256: &str,
    ) -> Result<Option<WeaponryCurveModifierGraphDurableRecord>, StoreError> {
        if !is_opaque_id(project_id) || !is_sha256(lookup_key_sha256) {
            return Err(StoreError::InvalidData(
                "curve/modifier-graph lookup identity is invalid".to_owned(),
            ));
        }
        let mut connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let transaction = connection.transaction()?;
        let record = transaction
            .query_row(
                "SELECT record_json FROM weaponry_curve_modifier_graph_records WHERE project_id = ?1 AND lookup_key_sha256 = ?2",
                params![project_id, lookup_key_sha256],
                read_record,
            )
            .optional()?;
        let Some(record) = record else {
            transaction.commit()?;
            return Ok(None);
        };
        if record.project_id != project_id || record.lookup_key_sha256 != lookup_key_sha256 {
            return Err(contract(
                "WEAPONRY_CURVE_MODIFIER_GRAPH_SCOPE_MISMATCH",
                "stored curve/modifier-graph record scope differs",
            ));
        }
        validate_record(&record)?;
        let expected = [
            (
                record.curve_set_object_sha256.as_str(),
                WEAPONRY_CURVE_SET_OBJECT_KIND,
            ),
            (
                record.sample_set_object_sha256.as_str(),
                WEAPONRY_SAMPLE_SET_OBJECT_KIND,
            ),
            (
                record.modifier_graph_object_sha256.as_str(),
                WEAPONRY_MODIFIER_GRAPH_OBJECT_KIND,
            ),
            (
                record.dependency_graph_object_sha256.as_str(),
                WEAPONRY_DEPENDENCY_GRAPH_OBJECT_KIND,
            ),
            (
                record.recompute_plan_object_sha256.as_str(),
                WEAPONRY_RECOMPUTE_PLAN_OBJECT_KIND,
            ),
        ];
        for (sha256, kind) in expected {
            let object = read_object_record(&transaction, sha256).map_err(|error| match error {
                StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => contract(
                    "WEAPONRY_CURVE_MODIFIER_GRAPH_CAS_MISSING",
                    "curve/modifier-graph CAS root disappeared before readback",
                ),
                other => other,
            })?;
            validate_registered_object(&transaction, &self.cas, &object, kind, true)?;
        }
        transaction.commit()?;
        Ok(Some(record))
    }

    /// Resolve the one structural Curve/ModifierGraph record that belongs to
    /// a source revision and the five structural semantic hashes. The lookup
    /// key is a derived structural identity and is intentionally not required
    /// from a caller-facing downstream request. Multiple rows are rejected
    /// rather than choosing an arbitrary structural parent.
    pub fn get_weaponry_curve_modifier_graph_by_source_revision_and_modifier_graph(
        &self,
        project_id: &str,
        source_revision_sha256: &str,
        modifier_graph_sha256: &str,
        curve_set_sha256: &str,
        sample_set_sha256: &str,
        dependency_graph_sha256: &str,
        recompute_plan_sha256: &str,
    ) -> Result<Option<WeaponryCurveModifierGraphDurableRecord>, StoreError> {
        if !is_opaque_id(project_id)
            || !is_sha256(source_revision_sha256)
            || !is_sha256(modifier_graph_sha256)
            || !is_sha256(curve_set_sha256)
            || !is_sha256(sample_set_sha256)
            || !is_sha256(dependency_graph_sha256)
            || !is_sha256(recompute_plan_sha256)
        {
            return Err(StoreError::InvalidData(
                "curve/modifier-graph source lookup identity is invalid".to_owned(),
            ));
        }
        let mut connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let transaction = connection.transaction()?;
        let mut statement = transaction.prepare(
            "SELECT record_json FROM weaponry_curve_modifier_graph_records WHERE project_id = ?1 AND source_revision_sha256 = ?2 AND modifier_graph_sha256 = ?3 AND curve_set_sha256 = ?4 AND sample_set_sha256 = ?5 AND dependency_graph_sha256 = ?6 AND recompute_plan_sha256 = ?7 ORDER BY created_at ASC, lookup_key_sha256 ASC",
        )?;
        let records = statement
            .query_map(
                params![
                    project_id,
                    source_revision_sha256,
                    modifier_graph_sha256,
                    curve_set_sha256,
                    sample_set_sha256,
                    dependency_graph_sha256,
                    recompute_plan_sha256,
                ],
                read_record,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        match records.len() {
            0 => {
                transaction.commit()?;
                Ok(None)
            }
            1 => {
                let record = records.into_iter().next().expect("one record");
                if record.project_id != project_id
                    || record.source_revision_sha256 != source_revision_sha256
                    || record.modifier_graph_sha256 != modifier_graph_sha256
                    || record.curve_set_sha256 != curve_set_sha256
                    || record.sample_set_sha256 != sample_set_sha256
                    || record.dependency_graph_sha256 != dependency_graph_sha256
                    || record.recompute_plan_sha256 != recompute_plan_sha256
                {
                    return Err(contract(
                        "WEAPONRY_CURVE_MODIFIER_GRAPH_SOURCE_SCOPE_MISMATCH",
                        "stored curve/modifier-graph source lookup scope differs",
                    ));
                }
                validate_record(&record)?;
                let expected = [
                    (
                        record.curve_set_object_sha256.as_str(),
                        WEAPONRY_CURVE_SET_OBJECT_KIND,
                    ),
                    (
                        record.sample_set_object_sha256.as_str(),
                        WEAPONRY_SAMPLE_SET_OBJECT_KIND,
                    ),
                    (
                        record.modifier_graph_object_sha256.as_str(),
                        WEAPONRY_MODIFIER_GRAPH_OBJECT_KIND,
                    ),
                    (
                        record.dependency_graph_object_sha256.as_str(),
                        WEAPONRY_DEPENDENCY_GRAPH_OBJECT_KIND,
                    ),
                    (
                        record.recompute_plan_object_sha256.as_str(),
                        WEAPONRY_RECOMPUTE_PLAN_OBJECT_KIND,
                    ),
                ];
                for (sha256, kind) in expected {
                    let object =
                        read_object_record(&transaction, sha256).map_err(|error| match error {
                            StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => contract(
                                "WEAPONRY_CURVE_MODIFIER_GRAPH_CAS_MISSING",
                                "curve/modifier-graph CAS root disappeared before source readback",
                            ),
                            other => other,
                        })?;
                    validate_registered_object(&transaction, &self.cas, &object, kind, true)?;
                }
                transaction.commit()?;
                Ok(Some(record))
            }
            _ => Err(contract(
                "WEAPONRY_CURVE_MODIFIER_GRAPH_SOURCE_AMBIGUOUS",
                "source revision and modifier graph identify multiple structural records",
            )),
        }
    }

    pub fn weaponry_curve_modifier_graph_cas_roots(
        record: &WeaponryCurveModifierGraphDurableRecord,
    ) -> Vec<String> {
        roots(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CasError;
    use forgecad_core::canonical_json_hash;
    use std::fs;

    fn hash(seed: &str) -> String {
        let value = serde_json::json!({"seed": seed});
        canonical_json_hash(&value)
    }

    fn project(store: &Store) {
        store
            .insert_project(&super::super::ProjectRecord {
                schema_version: "Project@1".to_owned(),
                project_id: "weaponry".to_owned(),
                name: "Weaponry test".to_owned(),
                policy: serde_json::json!({"scope":"test"}),
                created_at: "1".to_owned(),
                updated_at: "1".to_owned(),
                active_snapshot_revision: 0,
                head_snapshot_id: None,
                canonical_sha256: "a".repeat(64),
            })
            .expect("project");
    }

    fn object(store: &Store, kind: &str, name: &str) -> CasObjectRecord {
        let bytes = canonical_json_bytes(&serde_json::json!({"kind":kind,"name":name}))
            .expect("canonical json");
        store
            .put_object(
                &bytes,
                None,
                WEAPONRY_CURVE_MODIFIER_GRAPH_JSON_MIME,
                kind,
                "1",
            )
            .expect("object")
            .record
    }

    fn record(
        objects: &WeaponryCurveModifierGraphCasBundle,
    ) -> WeaponryCurveModifierGraphDurableRecord {
        let mut record = WeaponryCurveModifierGraphDurableRecord {
            schema_version: WEAPONRY_CURVE_MODIFIER_GRAPH_RECORD_SCHEMA.to_owned(),
            project_id: "weaponry".to_owned(),
            source_revision_id: "source-r1".to_owned(),
            source_revision_sha256: hash("source"),
            source_candidate_id: "candidate-r1".to_owned(),
            source_candidate_state_sha256: hash("candidate-state"),
            source_authoring_mesh_id: "mesh-r1".to_owned(),
            source_authoring_mesh_lineage_id: "lineage-r1".to_owned(),
            source_authoring_mesh_revision_index: 1,
            source_authoring_mesh_identity_sha256: hash("mesh-identity"),
            curve_set_id: "curve-set".to_owned(),
            curve_set_sha256: hash("curve"),
            curve_set_object_sha256: objects.curve_set.sha256.clone(),
            sample_set_id: "sample-set".to_owned(),
            sample_set_sha256: hash("sample"),
            sample_set_object_sha256: objects.sample_set.sha256.clone(),
            modifier_graph_id: "graph".to_owned(),
            modifier_graph_sha256: hash("graph"),
            modifier_graph_object_sha256: objects.modifier_graph.sha256.clone(),
            dependency_graph_sha256: hash("dependency"),
            dependency_graph_object_sha256: objects.dependency_graph.sha256.clone(),
            recompute_plan_sha256: hash("recompute"),
            recompute_plan_object_sha256: objects.recompute_plan.sha256.clone(),
            lookup_key_sha256: hash("source-r1-graph"),
            idempotency_key: "idem-1".to_owned(),
            input_sha256: hash("input"),
            materialization_status: WEAPONRY_CURVE_MODIFIER_GRAPH_STATUS.to_owned(),
            canonical_sha256: String::new(),
            created_at: "1".to_owned(),
        };
        record.canonical_sha256 = canonical_record_sha256(&record).expect("canonical");
        record
    }

    fn bundle(store: &Store) -> WeaponryCurveModifierGraphCasBundle {
        WeaponryCurveModifierGraphCasBundle {
            curve_set: object(store, WEAPONRY_CURVE_SET_OBJECT_KIND, "curve"),
            sample_set: object(store, WEAPONRY_SAMPLE_SET_OBJECT_KIND, "sample"),
            modifier_graph: object(store, WEAPONRY_MODIFIER_GRAPH_OBJECT_KIND, "graph"),
            dependency_graph: object(store, WEAPONRY_DEPENDENCY_GRAPH_OBJECT_KIND, "dependency"),
            recompute_plan: object(store, WEAPONRY_RECOMPUTE_PLAN_OBJECT_KIND, "recompute"),
        }
    }

    #[test]
    fn record_replay_get_and_roots_are_atomic_and_deterministic() {
        let store = Store::memory().expect("store");
        project(&store);
        let cas = bundle(&store);
        let record = record(&cas);
        let commit = WeaponryCurveModifierGraphCommit {
            record: record.clone(),
            cas: cas.clone(),
        };
        let (stored, replayed) = store
            .record_weaponry_curve_modifier_graph_with_replay(&commit)
            .expect("commit");
        assert!(!replayed);
        assert_eq!(stored, record);
        let (replayed_record, replayed) = store
            .record_weaponry_curve_modifier_graph_with_replay(&commit)
            .expect("replay");
        assert!(replayed);
        assert_eq!(replayed_record, record);
        assert_eq!(
            store
                .get_weaponry_curve_modifier_graph("weaponry", &hash("source-r1-graph"))
                .expect("get"),
            Some(record.clone())
        );
        assert_eq!(Store::weaponry_curve_modifier_graph_cas_roots(&record), {
            let mut roots = vec![
                cas.curve_set.sha256,
                cas.sample_set.sha256,
                cas.modifier_graph.sha256,
                cas.dependency_graph.sha256,
                cas.recompute_plan.sha256,
            ];
            roots.sort();
            roots
        });
        for root in Store::weaponry_curve_modifier_graph_cas_roots(&record) {
            assert_eq!(
                store
                    .get_object(&root)
                    .expect("object")
                    .expect("root")
                    .reachability,
                "reachable"
            );
        }
    }

    #[test]
    fn idempotency_conflict_does_not_replace_record() {
        let store = Store::memory().expect("store");
        project(&store);
        let cas = bundle(&store);
        let first = record(&cas);
        store
            .record_weaponry_curve_modifier_graph_with_replay(&WeaponryCurveModifierGraphCommit {
                record: first.clone(),
                cas: cas.clone(),
            })
            .expect("first");
        let mut conflict = first.clone();
        conflict.input_sha256 = hash("different-input");
        conflict.canonical_sha256 = canonical_record_sha256(&conflict).expect("canonical");
        let error = store
            .record_weaponry_curve_modifier_graph_with_replay(&WeaponryCurveModifierGraphCommit {
                record: conflict,
                cas,
            })
            .expect_err("conflict");
        assert!(
            matches!(error, StoreError::Contract { code, .. } if code == "WEAPONRY_CURVE_MODIFIER_GRAPH_IDEMPOTENCY_CONFLICT")
        );
        assert_eq!(
            store
                .get_weaponry_curve_modifier_graph("weaponry", &hash("source-r1-graph"))
                .expect("get"),
            Some(first)
        );
    }

    #[test]
    fn missing_or_wrong_cas_fails_before_row_install() {
        let store = Store::memory().expect("store");
        project(&store);
        let cas = bundle(&store);
        let mut record = record(&cas);
        record.curve_set_object_sha256 = "f".repeat(64);
        record.canonical_sha256 = canonical_record_sha256(&record).expect("canonical");
        let error = store
            .record_weaponry_curve_modifier_graph_with_replay(&WeaponryCurveModifierGraphCommit {
                record,
                cas,
            })
            .expect_err("binding mismatch");
        assert!(
            matches!(error, StoreError::Contract { code, .. } if code == "WEAPONRY_CURVE_MODIFIER_GRAPH_CAS_BINDING_MISMATCH")
        );
        assert_eq!(
            store
                .get_weaponry_curve_modifier_graph("weaponry", &hash("source-r1-graph"))
                .expect("get"),
            None
        );
    }

    #[test]
    fn bounded_json_read_rejects_tamper_and_exposes_semantic_mismatch() {
        let store = Store::memory().expect("store");
        project(&store);
        let cas = bundle(&store);
        let record = record(&cas);
        store
            .record_weaponry_curve_modifier_graph_with_replay(&WeaponryCurveModifierGraphCommit {
                record: record.clone(),
                cas: cas.clone(),
            })
            .expect("commit");

        let value = store
            .read_weaponry_curve_modifier_graph_json(
                &cas.curve_set.sha256,
                WEAPONRY_CURVE_SET_OBJECT_KIND,
            )
            .expect("read canonical root");
        // Semantic hashes are contract-owned bindings. A Runtime caller can
        // recompute this value and fail closed before using a mismatched root.
        assert_ne!(canonical_json_hash(&value), record.curve_set_sha256);

        let path = store
            .cas()
            .root()
            .join("objects")
            .join(&cas.curve_set.sha256[..2])
            .join(&cas.curve_set.sha256);
        fs::write(&path, b"{\"tampered\":true}").expect("tamper CAS");
        let error = store
            .read_weaponry_curve_modifier_graph_json(
                &cas.curve_set.sha256,
                WEAPONRY_CURVE_SET_OBJECT_KIND,
            )
            .expect_err("tampered root");
        assert!(matches!(
            error,
            StoreError::Cas(CasError::HashMismatch { .. })
        ));
    }

    #[test]
    fn source_revision_and_modifier_graph_lookup_resolves_one_record() {
        let store = Store::memory().expect("store");
        project(&store);
        let cas = bundle(&store);
        let record = record(&cas);
        store
            .record_weaponry_curve_modifier_graph_with_replay(&WeaponryCurveModifierGraphCommit {
                record: record.clone(),
                cas,
            })
            .expect("commit");
        assert_eq!(
            store
                .get_weaponry_curve_modifier_graph_by_source_revision_and_modifier_graph(
                    "weaponry",
                    &record.source_revision_sha256,
                    &record.modifier_graph_sha256,
                    &record.curve_set_sha256,
                    &record.sample_set_sha256,
                    &record.dependency_graph_sha256,
                    &record.recompute_plan_sha256,
                )
                .expect("source lookup"),
            Some(record)
        );
    }

    #[test]
    fn source_revision_and_modifier_graph_lookup_rejects_ambiguity() {
        let store = Store::memory().expect("store");
        project(&store);
        let cas = bundle(&store);
        let first = record(&cas);
        store
            .record_weaponry_curve_modifier_graph_with_replay(&WeaponryCurveModifierGraphCommit {
                record: first.clone(),
                cas: cas.clone(),
            })
            .expect("first commit");
        let mut second = first.clone();
        second.lookup_key_sha256 = hash("source-r1-graph-second");
        second.idempotency_key = "idem-2".to_owned();
        second.canonical_sha256 = canonical_record_sha256(&second).expect("canonical");
        store
            .record_weaponry_curve_modifier_graph_with_replay(&WeaponryCurveModifierGraphCommit {
                record: second,
                cas,
            })
            .expect("second commit");
        let error = store
            .get_weaponry_curve_modifier_graph_by_source_revision_and_modifier_graph(
                "weaponry",
                &first.source_revision_sha256,
                &first.modifier_graph_sha256,
                &first.curve_set_sha256,
                &first.sample_set_sha256,
                &first.dependency_graph_sha256,
                &first.recompute_plan_sha256,
            )
            .expect_err("ambiguous source lookup");
        assert!(matches!(
            error,
            StoreError::Contract { code, .. }
                if code == "WEAPONRY_CURVE_MODIFIER_GRAPH_SOURCE_AMBIGUOUS"
        ));
    }
}
