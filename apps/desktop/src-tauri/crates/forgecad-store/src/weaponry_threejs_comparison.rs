//! Durable Store/CAS boundary for the bounded Three.js knife comparison pass.
//!
//! The comparison is deliberately a measurement, not a promotion.  It binds
//! one fixed FRONT render to one authorized reference crop and to the worker's
//! semantic-id AOV.  The Store never interprets pixels and never runs the
//! browser worker; Runtime owns those operations and supplies a closed,
//! canonical receipt.

use super::{canonical_json_bytes, canonical_json_hash, mark_reachable_in_transaction, Store};
use forgecad_contracts::{is_opaque_id, is_sha256, CasObjectRecord};
use forgecad_core::sha256_hex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const WEAPONRY_THREEJS_COMPARISON_RECORD_SCHEMA: &str =
    "WeaponryThreeJsComparisonStoreRecord@1";
pub const WEAPONRY_THREEJS_COMPARISON_RECEIPT_SCHEMA: &str =
    "WeaponryThreeJsKnifeComparisonReceipt@1";
pub const WEAPONRY_THREEJS_COMPARISON_RECEIPT_KIND: &str =
    "weaponry-threejs-knife-comparison-receipt@1";
pub const WEAPONRY_THREEJS_COMPARISON_RECEIPT_MIME: &str = "application/json";
pub const WEAPONRY_THREEJS_COMPARISON_OPERATION: &str = "weaponry_threejs_knife_comparison";
pub const WEAPONRY_THREEJS_COMPARISON_VIEW_ID: &str = "FRONT";
pub const WEAPONRY_THREEJS_COMPARISON_AOV_ID: &str = "semantic-id";
pub const WEAPONRY_THREEJS_COMPARISON_HANDEDNESS_TRANSFORM: &str = "mirror-render-x-to-reference@1";
pub const WEAPONRY_THREEJS_COMPARISON_MAX_RECEIPT_BYTES: u64 = 4 * 1024 * 1024;
pub const WEAPONRY_THREEJS_COMPARISON_METRIC_POLICY: &str =
    "fixed-front-semantic-id-blade-only-metrics@1";
pub const WEAPONRY_THREEJS_COMPARISON_STATUS: &str = "MEASURED_NOT_APPROVED";
pub const WEAPONRY_THREEJS_COMPARISON_VISUAL_STATUS: &str = "NOT_RUN";
pub const WEAPONRY_THREEJS_COMPARISON_HUMAN_STATUS: &str = "NOT_RUN";
pub const WEAPONRY_THREEJS_COMPARISON_ENGINE_STATUS: &str = "NOT_RUN";
pub const WEAPONRY_THREEJS_COMPARISON_COMMERCIAL_STATUS: &str = "NOT_RUN";
pub const WEAPONRY_THREEJS_COMPARISON_MAX_CROP_PIXELS: u64 = 4096 * 4096;

/// The semantic-id AOV is generated from lexicographically sorted compiled
/// part ids.  Keep this mapping in the durable record so a later reader cannot
/// silently score guard/grip pixels as blade pixels.
pub const WEAPONRY_THREEJS_COMPARISON_SEMANTIC_PART_IDS: [&str; 2] = ["blade-body", "cutting-edge"];
pub const WEAPONRY_THREEJS_COMPARISON_EDITABLE_PART_IDS: [&str; 2] = ["blade-body", "cutting-edge"];
pub const WEAPONRY_THREEJS_COMPARISON_FROZEN_PART_IDS: [&str; 11] = [
    "fastener-grip-a",
    "fastener-grip-b",
    "fastener-grip-c",
    "fastener-grip-d",
    "gem-guard-eye",
    "gem-pommel",
    "grip",
    "guard",
    "pommel",
    "relief-dragon-belly",
    "relief-dragon-spine",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaponryThreeJsComparisonStoreRecord {
    pub schema_version: String,
    pub project_id: String,
    pub comparison_id: String,
    pub preview_execution_id: String,
    pub preview_receipt_sha256: String,
    pub preview_receipt_object_sha256: String,
    pub preview_worker_cohort_sha256: String,
    pub preview_view_id: String,
    pub preview_aov_id: String,
    pub handedness_transform: String,
    pub preview_aov_sha256: String,
    pub preview_aov_object_sha256: String,
    pub reference_id: String,
    pub reference_object_sha256: String,
    pub reference_evidence_sha256: String,
    pub reference_crop_x: u64,
    pub reference_crop_y: u64,
    pub reference_crop_width: u64,
    pub reference_crop_height: u64,
    pub semantic_part_ids: BTreeMap<String, u32>,
    pub editable_part_ids: Vec<String>,
    pub frozen_part_ids: Vec<String>,
    pub metric_policy: String,
    pub metrics: Value,
    pub comparison_status: String,
    pub visual_status: String,
    pub human_status: String,
    pub engine_status: String,
    pub commercial_status: String,
    pub parent_retained: bool,
    pub request_sha256: String,
    pub idempotency_key: String,
    pub comparison_receipt_sha256: String,
    pub comparison_receipt_object_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct WeaponryThreeJsComparisonCommit {
    pub record: WeaponryThreeJsComparisonStoreRecord,
    pub receipt: CasObjectRecord,
}

fn contract(code: &str, message: impl Into<String>) -> super::StoreError {
    super::StoreError::Contract {
        code: code.to_owned(),
        message: message.into(),
    }
}

fn validate_hash_fields(record: &WeaponryThreeJsComparisonStoreRecord) -> bool {
    [
        record.preview_receipt_sha256.as_str(),
        record.preview_receipt_object_sha256.as_str(),
        record.preview_worker_cohort_sha256.as_str(),
        record.preview_aov_sha256.as_str(),
        record.preview_aov_object_sha256.as_str(),
        record.reference_object_sha256.as_str(),
        record.reference_evidence_sha256.as_str(),
        record.request_sha256.as_str(),
        record.comparison_receipt_sha256.as_str(),
        record.comparison_receipt_object_sha256.as_str(),
    ]
    .into_iter()
    .all(is_sha256)
}

fn validate_record(record: &WeaponryThreeJsComparisonStoreRecord) -> Result<(), super::StoreError> {
    let expected_semantic = BTreeMap::from([
        ("blade-body".to_owned(), 1_u32),
        ("cutting-edge".to_owned(), 2_u32),
    ]);
    let expected_editable = WEAPONRY_THREEJS_COMPARISON_EDITABLE_PART_IDS
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    let expected_frozen = WEAPONRY_THREEJS_COMPARISON_FROZEN_PART_IDS
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    if record.schema_version != WEAPONRY_THREEJS_COMPARISON_RECORD_SCHEMA
        || !is_opaque_id(&record.project_id)
        || !is_opaque_id(&record.comparison_id)
        || !is_opaque_id(&record.preview_execution_id)
        || !validate_hash_fields(record)
        || record.preview_view_id != WEAPONRY_THREEJS_COMPARISON_VIEW_ID
        || record.preview_aov_id != WEAPONRY_THREEJS_COMPARISON_AOV_ID
        || record.handedness_transform != WEAPONRY_THREEJS_COMPARISON_HANDEDNESS_TRANSFORM
        || !is_opaque_id(&record.reference_id)
        || record.reference_crop_width == 0
        || record.reference_crop_height == 0
        || record
            .reference_crop_width
            .saturating_mul(record.reference_crop_height)
            > WEAPONRY_THREEJS_COMPARISON_MAX_CROP_PIXELS
        || record.semantic_part_ids != expected_semantic
        || record.editable_part_ids != expected_editable
        || record.frozen_part_ids != expected_frozen
        || record.metric_policy != WEAPONRY_THREEJS_COMPARISON_METRIC_POLICY
        || !record.metrics.is_object()
        || record.comparison_status != WEAPONRY_THREEJS_COMPARISON_STATUS
        || record.visual_status != WEAPONRY_THREEJS_COMPARISON_VISUAL_STATUS
        || record.human_status != WEAPONRY_THREEJS_COMPARISON_HUMAN_STATUS
        || record.engine_status != WEAPONRY_THREEJS_COMPARISON_ENGINE_STATUS
        || record.commercial_status != WEAPONRY_THREEJS_COMPARISON_COMMERCIAL_STATUS
        || !record.parent_retained
        || !is_opaque_id(&record.idempotency_key)
        || record.idempotency_key.len() > 128
        || record.created_at.is_empty()
        || record.created_at.len() > 64
    {
        return Err(contract(
            "WEAPONRY_THREEJS_COMPARISON_RECORD_INVALID",
            "comparison record identity, scope, status or hash is invalid",
        ));
    }
    Ok(())
}

fn receipt_fields() -> [&'static str; 29] {
    [
        "schema_version",
        "operation",
        "project_id",
        "comparison_id",
        "preview_execution_id",
        "preview_receipt_sha256",
        "preview_receipt_object_sha256",
        "preview_worker_cohort_sha256",
        "view_id",
        "aov_id",
        "handedness_transform",
        "preview_aov_sha256",
        "preview_aov_object_sha256",
        "reference_id",
        "reference_object_sha256",
        "reference_evidence_sha256",
        "reference_crop",
        "semantic_part_ids",
        "editable_part_ids",
        "frozen_part_ids",
        "metric_policy",
        "metrics",
        "comparison_status",
        "visual_status",
        "human_status",
        "engine_status",
        "commercial_status",
        "parent_retained",
        "canonical_sha256",
    ]
}

fn has_exact_keys(value: &Value, expected: &[&str]) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object.len() == expected.len() && expected.iter().all(|key| object.contains_key(*key))
}

fn validate_metrics(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    let expected = [
        "silhouette_iou_milli",
        "boundary_f1_milli",
        "sdf_chamfer_milli",
    ];
    object.len() == expected.len()
        && expected.iter().all(|key| {
            object
                .get(*key)
                .and_then(Value::as_u64)
                .is_some_and(|metric| metric <= 1_000_000)
        })
}

fn validate_receipt(
    record: &WeaponryThreeJsComparisonStoreRecord,
    bytes: &[u8],
) -> Result<Value, super::StoreError> {
    if bytes.is_empty() || bytes.len() as u64 > WEAPONRY_THREEJS_COMPARISON_MAX_RECEIPT_BYTES {
        return Err(contract(
            "WEAPONRY_THREEJS_COMPARISON_RECEIPT_BYTES_INVALID",
            "comparison receipt is empty or exceeds its bound",
        ));
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        contract(
            "WEAPONRY_THREEJS_COMPARISON_RECEIPT_JSON_INVALID",
            format!("comparison receipt is not JSON: {error}"),
        )
    })?;
    if !has_exact_keys(&value, &receipt_fields()) {
        return Err(contract(
            "WEAPONRY_THREEJS_COMPARISON_RECEIPT_FIELDS_INVALID",
            "comparison receipt fields are not closed",
        ));
    }
    let object = value.as_object().expect("closed receipt object");
    let crop = object.get("reference_crop").and_then(Value::as_object);
    let crop_matches = crop.is_some_and(|crop| {
        crop.len() == 4
            && crop.get("x").and_then(Value::as_u64) == Some(record.reference_crop_x)
            && crop.get("y").and_then(Value::as_u64) == Some(record.reference_crop_y)
            && crop.get("width").and_then(Value::as_u64) == Some(record.reference_crop_width)
            && crop.get("height").and_then(Value::as_u64) == Some(record.reference_crop_height)
    });
    let semantic_matches = object
        .get("semantic_part_ids")
        .is_some_and(|value| value == &json!(record.semantic_part_ids));
    let editable_matches =
        object.get("editable_part_ids") == Some(&json!(record.editable_part_ids));
    let frozen_matches = object.get("frozen_part_ids") == Some(&json!(record.frozen_part_ids));
    if object.get("schema_version").and_then(Value::as_str)
        != Some(WEAPONRY_THREEJS_COMPARISON_RECEIPT_SCHEMA)
        || object.get("operation").and_then(Value::as_str)
            != Some(WEAPONRY_THREEJS_COMPARISON_OPERATION)
        || object.get("project_id").and_then(Value::as_str) != Some(record.project_id.as_str())
        || object.get("comparison_id").and_then(Value::as_str)
            != Some(record.comparison_id.as_str())
        || object.get("preview_execution_id").and_then(Value::as_str)
            != Some(record.preview_execution_id.as_str())
        || object.get("preview_receipt_sha256").and_then(Value::as_str)
            != Some(record.preview_receipt_sha256.as_str())
        || object
            .get("preview_receipt_object_sha256")
            .and_then(Value::as_str)
            != Some(record.preview_receipt_object_sha256.as_str())
        || object
            .get("preview_worker_cohort_sha256")
            .and_then(Value::as_str)
            != Some(record.preview_worker_cohort_sha256.as_str())
        || object.get("view_id").and_then(Value::as_str) != Some(record.preview_view_id.as_str())
        || object.get("aov_id").and_then(Value::as_str) != Some(record.preview_aov_id.as_str())
        || object.get("handedness_transform").and_then(Value::as_str)
            != Some(record.handedness_transform.as_str())
        || object.get("preview_aov_sha256").and_then(Value::as_str)
            != Some(record.preview_aov_sha256.as_str())
        || object
            .get("preview_aov_object_sha256")
            .and_then(Value::as_str)
            != Some(record.preview_aov_object_sha256.as_str())
        || object.get("reference_id").and_then(Value::as_str) != Some(record.reference_id.as_str())
        || object
            .get("reference_object_sha256")
            .and_then(Value::as_str)
            != Some(record.reference_object_sha256.as_str())
        || object
            .get("reference_evidence_sha256")
            .and_then(Value::as_str)
            != Some(record.reference_evidence_sha256.as_str())
        || !crop_matches
        || !semantic_matches
        || !editable_matches
        || !frozen_matches
        || object.get("metric_policy").and_then(Value::as_str)
            != Some(record.metric_policy.as_str())
        || object.get("metrics") != Some(&record.metrics)
        || !validate_metrics(&record.metrics)
        || object.get("comparison_status").and_then(Value::as_str)
            != Some(WEAPONRY_THREEJS_COMPARISON_STATUS)
        || object.get("visual_status").and_then(Value::as_str)
            != Some(WEAPONRY_THREEJS_COMPARISON_VISUAL_STATUS)
        || object.get("human_status").and_then(Value::as_str)
            != Some(WEAPONRY_THREEJS_COMPARISON_HUMAN_STATUS)
        || object.get("engine_status").and_then(Value::as_str)
            != Some(WEAPONRY_THREEJS_COMPARISON_ENGINE_STATUS)
        || object.get("commercial_status").and_then(Value::as_str)
            != Some(WEAPONRY_THREEJS_COMPARISON_COMMERCIAL_STATUS)
        || object.get("parent_retained").and_then(Value::as_bool) != Some(true)
    {
        return Err(contract(
            "WEAPONRY_THREEJS_COMPARISON_RECEIPT_BINDING_MISMATCH",
            "comparison receipt differs from its frozen blade-only scope",
        ));
    }
    let supplied = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| {
            contract(
                "WEAPONRY_THREEJS_COMPARISON_RECEIPT_CANONICAL_MISSING",
                "comparison receipt canonical hash is missing",
            )
        })?;
    let mut preimage = value.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    let canonical = canonical_json_bytes(&value)
        .map_err(|error| super::StoreError::InvalidData(error.to_string()))?;
    if canonical != bytes
        || canonical_json_hash(&preimage) != supplied
        || supplied != record.comparison_receipt_sha256
        || sha256_hex(bytes) != record.comparison_receipt_object_sha256
    {
        return Err(contract(
            "WEAPONRY_THREEJS_COMPARISON_RECEIPT_CANONICAL_MISMATCH",
            "comparison receipt semantic or object hash differs",
        ));
    }
    Ok(value)
}

fn validate_object_metadata(
    connection: &Connection,
    object: &CasObjectRecord,
) -> Result<(), super::StoreError> {
    if object.schema_version != "CasObject@1"
        || !is_sha256(&object.sha256)
        || object.mime != WEAPONRY_THREEJS_COMPARISON_RECEIPT_MIME
        || object.kind != WEAPONRY_THREEJS_COMPARISON_RECEIPT_KIND
        || object.size_bytes == 0
        || object.size_bytes > WEAPONRY_THREEJS_COMPARISON_MAX_RECEIPT_BYTES
    {
        return Err(contract(
            "WEAPONRY_THREEJS_COMPARISON_RECEIPT_CAS_INVALID",
            "comparison receipt CAS metadata is outside its allowlist",
        ));
    }
    let stored: Option<(i64, String, String)> = connection
        .query_row(
            "SELECT size_bytes, mime, kind FROM objects WHERE sha256 = ?1",
            params![object.sha256],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    if stored
        != Some((
            object.size_bytes as i64,
            object.mime.clone(),
            object.kind.clone(),
        ))
    {
        return Err(contract(
            "WEAPONRY_THREEJS_COMPARISON_RECEIPT_CAS_MISSING",
            "comparison receipt CAS metadata is not registered exactly",
        ));
    }
    Ok(())
}

fn comparison_record_roots(
    transaction: &rusqlite::Transaction<'_>,
    record: &WeaponryThreeJsComparisonStoreRecord,
) -> Result<Vec<String>, super::StoreError> {
    let mut statement = transaction.prepare(
        "SELECT object_sha256 FROM weaponry_threejs_comparison_roots WHERE project_id = ?1 AND comparison_id = ?2 ORDER BY role",
    )?;
    let rows = statement.query_map(params![record.project_id, record.comparison_id], |row| {
        row.get::<_, String>(0)
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn read_comparison_record(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<WeaponryThreeJsComparisonStoreRecord> {
    let json: String = row.get(0)?;
    serde_json::from_str(&json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn same_request(
    left: &WeaponryThreeJsComparisonStoreRecord,
    right: &WeaponryThreeJsComparisonStoreRecord,
) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    left.created_at.clear();
    right.created_at.clear();
    left == right
}

pub(crate) fn ensure_table(connection: &Connection) -> Result<(), super::StoreError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS weaponry_threejs_comparison_records (
             schema_version TEXT NOT NULL CHECK (schema_version = 'WeaponryThreeJsComparisonStoreRecord@1'),
             project_id TEXT NOT NULL REFERENCES projects(project_id),
             comparison_id TEXT NOT NULL,
             preview_execution_id TEXT NOT NULL,
             comparison_receipt_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             idempotency_key TEXT NOT NULL UNIQUE,
             request_sha256 TEXT NOT NULL,
             record_json TEXT NOT NULL,
             created_at TEXT NOT NULL,
             PRIMARY KEY (project_id, comparison_id)
         );
         CREATE TABLE IF NOT EXISTS weaponry_threejs_comparison_roots (
             project_id TEXT NOT NULL,
             comparison_id TEXT NOT NULL,
             role TEXT NOT NULL,
             object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             PRIMARY KEY (project_id, comparison_id, role),
             FOREIGN KEY (project_id, comparison_id)
                 REFERENCES weaponry_threejs_comparison_records(project_id, comparison_id)
         );
         CREATE INDEX IF NOT EXISTS weaponry_threejs_comparison_preview_idx
             ON weaponry_threejs_comparison_records(project_id, preview_execution_id);
         CREATE INDEX IF NOT EXISTS weaponry_threejs_comparison_roots_object_idx
             ON weaponry_threejs_comparison_roots(object_sha256);",
    )?;
    Ok(())
}

impl Store {
    /// Read one exact AOV after validating the complete Preview record.  The
    /// comparison path uses FRONT/semantic-id; this general method remains
    /// useful for restart-safe readback and refuses a hash mismatch.
    pub fn read_weaponry_threejs_preview_aov_exact(
        &self,
        project_id: &str,
        execution_id: &str,
        view_id: &str,
        aov_id: &str,
        object_sha256: &str,
    ) -> Result<Option<(CasObjectRecord, Vec<u8>)>, super::StoreError> {
        if !is_opaque_id(project_id)
            || !is_opaque_id(execution_id)
            || !is_opaque_id(view_id)
            || !is_opaque_id(aov_id)
            || !is_sha256(object_sha256)
        {
            return Err(super::StoreError::InvalidData(
                "Three.js preview AOV exact lookup is invalid".to_owned(),
            ));
        }
        let Some(preview) = self.get_weaponry_threejs_preview_by_id(project_id, execution_id)?
        else {
            return Ok(None);
        };
        let connection = self.lock_connection()?;
        let found: Option<CasObjectRecord> = connection
            .query_row(
                "SELECT o.sha256, o.size_bytes, o.mime, o.kind, o.reachability, o.created_at FROM weaponry_threejs_preview_aov_refs r JOIN objects o ON o.sha256 = r.object_sha256 WHERE r.project_id = ?1 AND r.execution_id = ?2 AND r.view_id = ?3 AND r.aov_id = ?4",
                params![project_id, execution_id, view_id, aov_id],
                |row| {
                    let size: i64 = row.get(1)?;
                    Ok(CasObjectRecord {
                        schema_version: "CasObject@1".to_owned(),
                        sha256: row.get(0)?,
                        size_bytes: u64::try_from(size).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                1,
                                rusqlite::types::Type::Integer,
                                Box::new(error),
                            )
                        })?,
                        mime: row.get(2)?,
                        kind: row.get(3)?,
                        reachability: row.get(4)?,
                        created_at: row.get(5)?,
                    })
                },
            )
            .optional()?;
        let Some(object) = found else {
            return Ok(None);
        };
        if object.sha256 != object_sha256
            || object.mime != "image/png"
            || object.kind != "weaponry-threejs-preview-aov@1"
            || object.size_bytes == 0
            || object.size_bytes > 4 * 1024 * 1024
        {
            return Err(contract(
                "WEAPONRY_THREEJS_COMPARISON_AOV_BINDING_MISMATCH",
                "preview AOV metadata differs from the exact requested object",
            ));
        }
        if preview.preview_worker_cohort_sha256.is_empty() {
            return Err(contract(
                "WEAPONRY_THREEJS_COMPARISON_PREVIEW_INVALID",
                "preview worker cohort is missing",
            ));
        }
        let bytes = self
            .cas
            .read_verified_bounded(&object.sha256, 4 * 1024 * 1024)
            .map_err(super::StoreError::from)?;
        if sha256_hex(&bytes) != object.sha256 || bytes.len() as u64 != object.size_bytes {
            return Err(contract(
                "WEAPONRY_THREEJS_COMPARISON_AOV_BYTES_INVALID",
                "preview AOV bytes do not match its CAS identity",
            ));
        }
        Ok(Some((object, bytes)))
    }

    pub fn record_weaponry_threejs_comparison_with_replay(
        &self,
        commit: &WeaponryThreeJsComparisonCommit,
    ) -> Result<(WeaponryThreeJsComparisonStoreRecord, bool), super::StoreError> {
        validate_record(&commit.record)?;
        if commit.receipt.sha256 != commit.record.comparison_receipt_object_sha256 {
            return Err(contract(
                "WEAPONRY_THREEJS_COMPARISON_RECEIPT_OBJECT_MISMATCH",
                "comparison receipt object hash differs from the record",
            ));
        }
        let receipt_bytes = self
            .cas
            .read_verified_bounded(
                &commit.record.comparison_receipt_object_sha256,
                WEAPONRY_THREEJS_COMPARISON_MAX_RECEIPT_BYTES,
            )
            .map_err(super::StoreError::from)?;
        validate_receipt(&commit.record, &receipt_bytes)?;

        let Some(preview) = self.get_weaponry_threejs_preview_by_id(
            &commit.record.project_id,
            &commit.record.preview_execution_id,
        )?
        else {
            return Err(contract(
                "WEAPONRY_THREEJS_COMPARISON_PREVIEW_MISSING",
                "comparison is not bound to an existing preview execution",
            ));
        };
        if preview.preview_receipt_sha256 != commit.record.preview_receipt_sha256
            || preview.preview_receipt_object_sha256 != commit.record.preview_receipt_object_sha256
            || preview.preview_worker_cohort_sha256 != commit.record.preview_worker_cohort_sha256
        {
            return Err(contract(
                "WEAPONRY_THREEJS_COMPARISON_PREVIEW_BINDING_MISMATCH",
                "comparison preview receipt or worker cohort differs",
            ));
        }
        let Some(reference) = self.get_reference_evidence(&commit.record.reference_id)? else {
            return Err(contract(
                "WEAPONRY_THREEJS_COMPARISON_REFERENCE_MISSING",
                "comparison reference evidence row is missing",
            ));
        };
        if reference.project_id != commit.record.project_id
            || reference.object_sha256 != commit.record.reference_object_sha256
            || reference.canonical_sha256 != commit.record.reference_evidence_sha256
            || commit.record.reference_crop_x >= u64::from(reference.width)
            || commit.record.reference_crop_y >= u64::from(reference.height)
            || commit.record.reference_crop_width
                > u64::from(reference.width) - commit.record.reference_crop_x
            || commit.record.reference_crop_height
                > u64::from(reference.height) - commit.record.reference_crop_y
        {
            return Err(contract(
                "WEAPONRY_THREEJS_COMPARISON_REFERENCE_BINDING_MISMATCH",
                "reference evidence, project or crop bounds differ",
            ));
        }
        if reference.size_bytes > 64 * 1024 * 1024 {
            return Err(contract(
                "WEAPONRY_THREEJS_COMPARISON_REFERENCE_TOO_LARGE",
                "reference evidence exceeds the bounded comparison read size",
            ));
        }
        let reference_bytes = self
            .cas
            .read_verified_bounded(&reference.object_sha256, 64 * 1024 * 1024)
            .map_err(super::StoreError::from)?;
        if reference_bytes.len() as u64 != reference.size_bytes
            || sha256_hex(&reference_bytes) != reference.object_sha256
        {
            return Err(contract(
                "WEAPONRY_THREEJS_COMPARISON_REFERENCE_CAS_MISMATCH",
                "reference evidence bytes do not match the authorized CAS object",
            ));
        }
        let preview_receipt = self.read_weaponry_threejs_preview_receipt_json(&preview)?;
        let preview_pass = preview_receipt
            .get("views")
            .and_then(Value::as_array)
            .and_then(|views| {
                views.iter().find(|view| {
                    view.get("view_id").and_then(Value::as_str)
                        == Some(commit.record.preview_view_id.as_str())
                })
            })
            .and_then(|view| view.get("passes"))
            .and_then(Value::as_array)
            .and_then(|passes| {
                passes.iter().find(|pass| {
                    pass.get("aov_id").and_then(Value::as_str)
                        == Some(commit.record.preview_aov_id.as_str())
                })
            })
            .ok_or_else(|| {
                contract(
                    "WEAPONRY_THREEJS_COMPARISON_AOV_MISSING",
                    "preview receipt does not contain the requested semantic-id AOV",
                )
            })?;
        if preview_pass.get("sha256").and_then(Value::as_str)
            != Some(commit.record.preview_aov_sha256.as_str())
            || preview_pass.get("object_sha256").and_then(Value::as_str)
                != Some(commit.record.preview_aov_object_sha256.as_str())
        {
            return Err(contract(
                "WEAPONRY_THREEJS_COMPARISON_AOV_BINDING_MISMATCH",
                "comparison semantic-id AOV semantic/object hash differs from the preview receipt",
            ));
        }
        let Some((aov_object, _aov_bytes)) = self.read_weaponry_threejs_preview_aov_exact(
            &commit.record.project_id,
            &commit.record.preview_execution_id,
            &commit.record.preview_view_id,
            &commit.record.preview_aov_id,
            &commit.record.preview_aov_object_sha256,
        )?
        else {
            return Err(contract(
                "WEAPONRY_THREEJS_COMPARISON_AOV_MISSING",
                "comparison semantic-id AOV is missing",
            ));
        };
        if aov_object.sha256 != commit.record.preview_aov_object_sha256 {
            return Err(contract(
                "WEAPONRY_THREEJS_COMPARISON_AOV_BINDING_MISMATCH",
                "comparison semantic-id AOV object differs",
            ));
        }
        let mut connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let transaction = connection.transaction()?;
        if let Some(existing) = transaction
            .query_row(
                "SELECT record_json FROM weaponry_threejs_comparison_records WHERE idempotency_key = ?1",
                params![commit.record.idempotency_key],
                read_comparison_record,
            )
            .optional()?
        {
            if !same_request(&existing, &commit.record) {
                return Err(contract(
                    "WEAPONRY_THREEJS_COMPARISON_IDEMPOTENCY_CONFLICT",
                    "comparison idempotency key is already bound to another result",
                ));
            }
            let roots = comparison_record_roots(&transaction, &existing)?;
            mark_reachable_in_transaction(&transaction, &roots)?;
            transaction.commit()?;
            return Ok((existing, true));
        }
        let existing_id: Option<WeaponryThreeJsComparisonStoreRecord> = transaction
            .query_row(
                "SELECT record_json FROM weaponry_threejs_comparison_records WHERE project_id = ?1 AND comparison_id = ?2",
                params![commit.record.project_id, commit.record.comparison_id],
                read_comparison_record,
            )
            .optional()?;
        if existing_id.is_some() {
            return Err(contract(
                "WEAPONRY_THREEJS_COMPARISON_ID_CONFLICT",
                "comparison id is already bound to another immutable result",
            ));
        }
        let preview_ref: Option<String> = transaction
            .query_row(
                "SELECT object_sha256 FROM weaponry_threejs_preview_aov_refs WHERE project_id = ?1 AND execution_id = ?2 AND view_id = ?3 AND aov_id = ?4",
                params![commit.record.project_id, commit.record.preview_execution_id, commit.record.preview_view_id, commit.record.preview_aov_id],
                |row| row.get(0),
            )
            .optional()?;
        if preview_ref.as_deref() != Some(commit.record.preview_aov_object_sha256.as_str()) {
            return Err(contract(
                "WEAPONRY_THREEJS_COMPARISON_AOV_RELATION_MISMATCH",
                "preview AOV relation is not the exact semantic-id object",
            ));
        }
        validate_object_metadata(&transaction, &commit.receipt)?;
        let record_json = String::from_utf8(
            canonical_json_bytes(
                &serde_json::to_value(&commit.record)
                    .map_err(|error| super::StoreError::InvalidData(error.to_string()))?,
            )
            .map_err(|error| super::StoreError::InvalidData(error.to_string()))?,
        )
        .map_err(|error| super::StoreError::InvalidData(error.to_string()))?;
        transaction.execute(
            "INSERT INTO weaponry_threejs_comparison_records (schema_version, project_id, comparison_id, preview_execution_id, comparison_receipt_object_sha256, idempotency_key, request_sha256, record_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                commit.record.schema_version,
                commit.record.project_id,
                commit.record.comparison_id,
                commit.record.preview_execution_id,
                commit.record.comparison_receipt_object_sha256,
                commit.record.idempotency_key,
                commit.record.request_sha256,
                record_json,
                commit.record.created_at,
            ],
        )?;
        let mut roots = vec![
            (
                "comparison_receipt",
                commit.record.comparison_receipt_object_sha256.clone(),
            ),
            (
                "preview_receipt",
                commit.record.preview_receipt_object_sha256.clone(),
            ),
            (
                "preview_aov",
                commit.record.preview_aov_object_sha256.clone(),
            ),
            ("reference", commit.record.reference_object_sha256.clone()),
        ];
        if let Some(derived) = reference.derived_object_sha256 {
            roots.push(("reference_derived", derived));
        }
        let unique = roots
            .iter()
            .map(|(_, hash)| hash.as_str())
            .collect::<BTreeSet<_>>();
        if unique.len() != roots.len() {
            return Err(contract(
                "WEAPONRY_THREEJS_COMPARISON_ROOT_DUPLICATE",
                "comparison CAS roots must be distinct",
            ));
        }
        for (role, hash) in &roots {
            transaction.execute(
                "INSERT INTO weaponry_threejs_comparison_roots (project_id, comparison_id, role, object_sha256) VALUES (?1, ?2, ?3, ?4)",
                params![commit.record.project_id, commit.record.comparison_id, role, hash],
            )?;
        }
        mark_reachable_in_transaction(
            &transaction,
            &roots
                .iter()
                .map(|(_, hash)| hash.clone())
                .collect::<Vec<_>>(),
        )?;
        transaction.commit()?;
        Ok((commit.record.clone(), false))
    }

    pub fn get_weaponry_threejs_comparison(
        &self,
        project_id: &str,
        idempotency_key: &str,
    ) -> Result<Option<WeaponryThreeJsComparisonStoreRecord>, super::StoreError> {
        if !is_opaque_id(project_id) || !is_opaque_id(idempotency_key) {
            return Err(super::StoreError::InvalidData(
                "Three.js comparison lookup is invalid".to_owned(),
            ));
        }
        let record = {
            let connection = self.lock_connection()?;
            ensure_table(&connection)?;
            connection
                .query_row(
                    "SELECT record_json FROM weaponry_threejs_comparison_records WHERE project_id = ?1 AND idempotency_key = ?2",
                    params![project_id, idempotency_key],
                    read_comparison_record,
                )
                .optional()?
        };
        if let Some(record) = &record {
            self.verify_weaponry_threejs_comparison_record(record)?;
        }
        Ok(record)
    }

    pub fn get_weaponry_threejs_comparison_by_id(
        &self,
        project_id: &str,
        comparison_id: &str,
    ) -> Result<Option<WeaponryThreeJsComparisonStoreRecord>, super::StoreError> {
        if !is_opaque_id(project_id) || !is_opaque_id(comparison_id) {
            return Err(super::StoreError::InvalidData(
                "Three.js comparison identity lookup is invalid".to_owned(),
            ));
        }
        let record = {
            let connection = self.lock_connection()?;
            ensure_table(&connection)?;
            connection
                .query_row(
                    "SELECT record_json FROM weaponry_threejs_comparison_records WHERE project_id = ?1 AND comparison_id = ?2",
                    params![project_id, comparison_id],
                    read_comparison_record,
                )
                .optional()?
        };
        if let Some(record) = &record {
            self.verify_weaponry_threejs_comparison_record(record)?;
        }
        Ok(record)
    }

    pub fn get_weaponry_threejs_comparison_exact(
        &self,
        project_id: &str,
        comparison_id: &str,
        handedness_transform: &str,
        preview_receipt_sha256: &str,
        preview_receipt_object_sha256: &str,
        preview_aov_sha256: &str,
        preview_aov_object_sha256: &str,
        reference_object_sha256: &str,
        reference_evidence_sha256: &str,
        comparison_receipt_sha256: &str,
        comparison_receipt_object_sha256: &str,
        preview_worker_cohort_sha256: &str,
    ) -> Result<Option<WeaponryThreeJsComparisonStoreRecord>, super::StoreError> {
        if handedness_transform != WEAPONRY_THREEJS_COMPARISON_HANDEDNESS_TRANSFORM {
            return Err(super::StoreError::InvalidData(
                "Three.js comparison handedness transform is invalid".to_owned(),
            ));
        }
        for hash in [
            preview_receipt_sha256,
            preview_receipt_object_sha256,
            preview_aov_sha256,
            preview_aov_object_sha256,
            reference_object_sha256,
            reference_evidence_sha256,
            comparison_receipt_sha256,
            comparison_receipt_object_sha256,
            preview_worker_cohort_sha256,
        ] {
            if !is_sha256(hash) {
                return Err(super::StoreError::InvalidData(
                    "Three.js comparison exact lookup hash is invalid".to_owned(),
                ));
            }
        }
        let Some(record) = self.get_weaponry_threejs_comparison_by_id(project_id, comparison_id)?
        else {
            return Ok(None);
        };
        if record.handedness_transform == handedness_transform
            && record.preview_receipt_sha256 == preview_receipt_sha256
            && record.preview_receipt_object_sha256 == preview_receipt_object_sha256
            && record.preview_aov_sha256 == preview_aov_sha256
            && record.preview_aov_object_sha256 == preview_aov_object_sha256
            && record.reference_object_sha256 == reference_object_sha256
            && record.reference_evidence_sha256 == reference_evidence_sha256
            && record.comparison_receipt_sha256 == comparison_receipt_sha256
            && record.comparison_receipt_object_sha256 == comparison_receipt_object_sha256
            && record.preview_worker_cohort_sha256 == preview_worker_cohort_sha256
        {
            Ok(Some(record))
        } else {
            Err(contract(
                "WEAPONRY_THREEJS_COMPARISON_EXACT_BINDING_MISMATCH",
                "comparison exact lookup hashes differ from the durable record",
            ))
        }
    }

    fn verify_weaponry_threejs_comparison_record(
        &self,
        record: &WeaponryThreeJsComparisonStoreRecord,
    ) -> Result<Value, super::StoreError> {
        validate_record(record)?;
        let receipt_bytes = self
            .cas
            .read_verified_bounded(
                &record.comparison_receipt_object_sha256,
                WEAPONRY_THREEJS_COMPARISON_MAX_RECEIPT_BYTES,
            )
            .map_err(super::StoreError::from)?;
        let receipt = validate_receipt(record, &receipt_bytes)?;
        let Some(preview) = self
            .get_weaponry_threejs_preview_by_id(&record.project_id, &record.preview_execution_id)?
        else {
            return Err(contract(
                "WEAPONRY_THREEJS_COMPARISON_PREVIEW_MISSING",
                "comparison preview execution disappeared during readback",
            ));
        };
        if preview.preview_receipt_sha256 != record.preview_receipt_sha256
            || preview.preview_receipt_object_sha256 != record.preview_receipt_object_sha256
            || preview.preview_worker_cohort_sha256 != record.preview_worker_cohort_sha256
        {
            return Err(contract(
                "WEAPONRY_THREEJS_COMPARISON_PREVIEW_BINDING_MISMATCH",
                "comparison preview binding differs during readback",
            ));
        }
        let Some(reference) = self.get_reference_evidence(&record.reference_id)? else {
            return Err(contract(
                "WEAPONRY_THREEJS_COMPARISON_REFERENCE_MISSING",
                "comparison reference evidence disappeared during readback",
            ));
        };
        if reference.project_id != record.project_id
            || reference.object_sha256 != record.reference_object_sha256
            || reference.canonical_sha256 != record.reference_evidence_sha256
        {
            return Err(contract(
                "WEAPONRY_THREEJS_COMPARISON_REFERENCE_BINDING_MISMATCH",
                "comparison reference binding differs during readback",
            ));
        }
        let reference_bytes = self
            .cas
            .read_verified_bounded(&reference.object_sha256, 64 * 1024 * 1024)
            .map_err(super::StoreError::from)?;
        if reference_bytes.len() as u64 != reference.size_bytes
            || sha256_hex(&reference_bytes) != reference.object_sha256
        {
            return Err(contract(
                "WEAPONRY_THREEJS_COMPARISON_REFERENCE_CAS_MISMATCH",
                "reference evidence bytes differ during readback",
            ));
        }
        let preview_receipt = self.read_weaponry_threejs_preview_receipt_json(&preview)?;
        let preview_pass = preview_receipt
            .get("views")
            .and_then(Value::as_array)
            .and_then(|views| {
                views.iter().find(|view| {
                    view.get("view_id").and_then(Value::as_str)
                        == Some(record.preview_view_id.as_str())
                })
            })
            .and_then(|view| view.get("passes"))
            .and_then(Value::as_array)
            .and_then(|passes| {
                passes.iter().find(|pass| {
                    pass.get("aov_id").and_then(Value::as_str)
                        == Some(record.preview_aov_id.as_str())
                })
            })
            .ok_or_else(|| {
                contract(
                    "WEAPONRY_THREEJS_COMPARISON_AOV_MISSING",
                    "preview receipt does not contain the requested semantic-id AOV",
                )
            })?;
        if preview_pass.get("sha256").and_then(Value::as_str)
            != Some(record.preview_aov_sha256.as_str())
            || preview_pass.get("object_sha256").and_then(Value::as_str)
                != Some(record.preview_aov_object_sha256.as_str())
        {
            return Err(contract(
                "WEAPONRY_THREEJS_COMPARISON_AOV_BINDING_MISMATCH",
                "semantic-id AOV hash differs during readback",
            ));
        }
        self.read_weaponry_threejs_preview_aov_exact(
            &record.project_id,
            &record.preview_execution_id,
            &record.preview_view_id,
            &record.preview_aov_id,
            &record.preview_aov_object_sha256,
        )?
        .ok_or_else(|| {
            contract(
                "WEAPONRY_THREEJS_COMPARISON_AOV_MISSING",
                "semantic-id AOV disappeared during readback",
            )
        })?;
        if record.reference_crop_x >= u64::from(reference.width)
            || record.reference_crop_y >= u64::from(reference.height)
            || record.reference_crop_width > u64::from(reference.width) - record.reference_crop_x
            || record.reference_crop_height > u64::from(reference.height) - record.reference_crop_y
        {
            return Err(contract(
                "WEAPONRY_THREEJS_COMPARISON_REFERENCE_CROP_INVALID",
                "comparison crop is outside the authorized reference",
            ));
        }
        let connection = self.lock_connection()?;
        let transaction = connection.unchecked_transaction()?;
        let roots = comparison_record_roots(&transaction, record)?;
        transaction.rollback()?;
        if roots.is_empty() {
            return Err(contract(
                "WEAPONRY_THREEJS_COMPARISON_ROOTS_MISSING",
                "comparison durable roots are missing",
            ));
        }
        let mut expected_roots = BTreeSet::from([
            record.comparison_receipt_object_sha256.clone(),
            record.preview_receipt_object_sha256.clone(),
            record.preview_aov_object_sha256.clone(),
            record.reference_object_sha256.clone(),
        ]);
        if let Some(derived) = reference.derived_object_sha256 {
            expected_roots.insert(derived);
        }
        let actual_roots = roots.iter().cloned().collect::<BTreeSet<_>>();
        if actual_roots != expected_roots || actual_roots.len() != roots.len() {
            return Err(contract(
                "WEAPONRY_THREEJS_COMPARISON_ROOT_BINDING_MISMATCH",
                "comparison durable roots do not exactly cover its reference and preview inputs",
            ));
        }
        for hash in &roots {
            let registered: Option<i64> = connection
                .query_row(
                    "SELECT size_bytes FROM objects WHERE sha256 = ?1",
                    params![hash],
                    |row| row.get(0),
                )
                .optional()?;
            let Some(size) = registered else {
                return Err(contract(
                    "WEAPONRY_THREEJS_COMPARISON_ROOT_MISSING",
                    "comparison root is not registered in the CAS index",
                ));
            };
            if size < 0 || size as u64 > 64 * 1024 * 1024 {
                return Err(contract(
                    "WEAPONRY_THREEJS_COMPARISON_ROOT_INVALID",
                    "comparison root exceeds the bounded read size",
                ));
            }
            self.cas
                .read_verified_bounded(hash, 64 * 1024 * 1024)
                .map_err(super::StoreError::from)?;
        }
        Ok(receipt)
    }

    pub fn read_weaponry_threejs_comparison_receipt_json(
        &self,
        record: &WeaponryThreeJsComparisonStoreRecord,
    ) -> Result<Value, super::StoreError> {
        validate_record(record)?;
        let bytes = self
            .cas
            .read_verified_bounded(
                &record.comparison_receipt_object_sha256,
                WEAPONRY_THREEJS_COMPARISON_MAX_RECEIPT_BYTES,
            )
            .map_err(super::StoreError::from)?;
        validate_receipt(record, &bytes)
    }

    pub fn weaponry_threejs_comparison_cas_roots(
        &self,
        record: &WeaponryThreeJsComparisonStoreRecord,
    ) -> Result<Vec<String>, super::StoreError> {
        validate_record(record)?;
        let connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let transaction = connection.unchecked_transaction()?;
        let roots = comparison_record_roots(&transaction, record)?;
        transaction.rollback()?;
        Ok(roots)
    }
}
