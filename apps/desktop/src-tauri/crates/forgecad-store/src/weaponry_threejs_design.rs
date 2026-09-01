//! Durable Store/CAS boundary for the lightweight Three.js knife workbench.
//!
//! This repository stores an already validated `KnifeSceneProgram@1` as
//! immutable canonical JSON. It deliberately does not execute JavaScript or
//! accept paths/URLs; execution remains a later fixed-worker concern.

use super::{
    canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256,
    mark_reachable_in_transaction, CasObjectRecord, Store, StoreError,
};
use forgecad_core::sha256_hex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const WEAPONRY_THREEJS_DESIGN_RECORD_SCHEMA: &str = "WeaponryThreeJsDesignStoreRecord@1";
pub const WEAPONRY_THREEJS_PROGRAM_SCHEMA: &str = "KnifeSceneProgram@1";
pub const WEAPONRY_THREEJS_PROGRAM_MIME: &str = "application/json";
pub const WEAPONRY_THREEJS_PROGRAM_OBJECT_KIND: &str = "weaponry-threejs-knife-scene-program@1";
pub const WEAPONRY_THREEJS_MAX_PROGRAM_BYTES: u64 = 1024 * 1024;
pub const WEAPONRY_THREEJS_EXECUTION_RECORD_SCHEMA: &str = "WeaponryThreeJsExecutionStoreRecord@1";
pub const WEAPONRY_THREEJS_WORKER_RESULT_MIME: &str = "application/json";
pub const WEAPONRY_THREEJS_WORKER_RESULT_KIND: &str = "weaponry-threejs-fixed-worker-result@1";
pub const WEAPONRY_THREEJS_GLB_MIME: &str = "model/gltf-binary";
pub const WEAPONRY_THREEJS_GLB_KIND: &str = "weaponry-threejs-knife-glb@1";
pub const WEAPONRY_THREEJS_MAX_WORKER_RESULT_BYTES: u64 = 64 * 1024 * 1024;
pub const WEAPONRY_THREEJS_MAX_GLB_BYTES: u64 = 32 * 1024 * 1024;
pub const WEAPONRY_THREEJS_PREVIEW_RECORD_SCHEMA: &str = "WeaponryThreeJsPreviewStoreRecord@1";
pub const WEAPONRY_THREEJS_PREVIEW_RECEIPT_SCHEMA: &str = "WeaponryThreeJsPreviewReceipt@1";
pub const WEAPONRY_THREEJS_PREVIEW_RUNTIME_ID: &str = "weaponry-threejs-fixed-preview-worker@1";
pub const WEAPONRY_THREEJS_PREVIEW_AOV_KIND: &str = "weaponry-threejs-preview-aov@1";
pub const WEAPONRY_THREEJS_PREVIEW_RECEIPT_KIND: &str = "weaponry-threejs-preview-receipt@1";
pub const WEAPONRY_THREEJS_PREVIEW_AOV_MIME: &str = "image/png";
pub const WEAPONRY_THREEJS_PREVIEW_RECEIPT_MIME: &str = "application/json";
pub const WEAPONRY_THREEJS_PREVIEW_VIEW_COUNT: usize = 8;
pub const WEAPONRY_THREEJS_PREVIEW_AOVS_PER_VIEW: usize = 6;
pub const WEAPONRY_THREEJS_PREVIEW_AOV_COUNT: usize =
    WEAPONRY_THREEJS_PREVIEW_VIEW_COUNT * WEAPONRY_THREEJS_PREVIEW_AOVS_PER_VIEW;
pub const WEAPONRY_THREEJS_PREVIEW_MAX_AOV_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaponryThreeJsDesignStoreRecord {
    pub schema_version: String,
    pub project_id: String,
    pub design_id: String,
    pub asset_id: String,
    pub family: String,
    pub program_sha256: String,
    pub program_object_sha256: String,
    pub part_count: u64,
    pub material_zone_count: u64,
    pub request_sha256: String,
    pub idempotency_key: String,
    pub execution_status: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct WeaponryThreeJsDesignCommit {
    pub record: WeaponryThreeJsDesignStoreRecord,
    pub program: CasObjectRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaponryThreeJsExecutionStoreRecord {
    pub schema_version: String,
    pub project_id: String,
    pub execution_id: String,
    pub design_id: String,
    pub operation: String,
    pub action: String,
    pub program_sha256: String,
    pub program_object_sha256: String,
    pub worker_result_sha256: String,
    pub worker_result_object_sha256: String,
    pub glb_sha256: Option<String>,
    pub glb_object_sha256: Option<String>,
    pub glb_bytes: u64,
    pub triangle_count: u64,
    pub part_count: u64,
    pub request_sha256: String,
    pub idempotency_key: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct WeaponryThreeJsExecutionCommit {
    pub record: WeaponryThreeJsExecutionStoreRecord,
    pub worker_result: CasObjectRecord,
    pub glb: Option<CasObjectRecord>,
}

/// Durable preview index. PNG/AOV bytes are never stored inline in SQLite;
/// only their content-addressed identities and the canonical receipt are
/// indexed here. The receipt itself contains the fixed eight-view manifest,
/// while `weaponry_threejs_preview_aov_refs` gives GC an explicit root for
/// every referenced image.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaponryThreeJsPreviewStoreRecord {
    pub schema_version: String,
    pub project_id: String,
    pub execution_id: String,
    pub design_id: String,
    pub operation: String,
    pub action: String,
    pub program_sha256: String,
    pub program_object_sha256: String,
    pub worker_id: String,
    pub preview_runtime_id: String,
    pub preview_runtime_sha256: String,
    pub preview_dependency_lock_sha256: String,
    pub preview_worker_cohort_sha256: String,
    pub worker_result_sha256: String,
    pub worker_result_object_sha256: String,
    pub preview_receipt_sha256: String,
    pub preview_receipt_object_sha256: String,
    pub view_count: u64,
    pub aov_count: u64,
    pub request_sha256: String,
    pub idempotency_key: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct WeaponryThreeJsPreviewCommit {
    pub record: WeaponryThreeJsPreviewStoreRecord,
    pub worker_result: CasObjectRecord,
    pub receipt: CasObjectRecord,
    pub aov_objects: Vec<CasObjectRecord>,
}

fn contract(code: &str, message: impl Into<String>) -> StoreError {
    StoreError::Contract {
        code: code.to_owned(),
        message: message.into(),
    }
}

fn validate_record(record: &WeaponryThreeJsDesignStoreRecord) -> Result<(), StoreError> {
    if record.schema_version != WEAPONRY_THREEJS_DESIGN_RECORD_SCHEMA
        || !is_opaque_id(&record.project_id)
        || !is_opaque_id(&record.design_id)
        || !is_opaque_id(&record.asset_id)
        || !is_opaque_id(&record.family)
        || !is_sha256(&record.program_sha256)
        || !is_sha256(&record.program_object_sha256)
        || !is_sha256(&record.request_sha256)
        || !is_opaque_id(&record.idempotency_key)
        || record.idempotency_key.len() > 128
        || !(2..=64).contains(&record.part_count)
        || !(1..=32).contains(&record.material_zone_count)
        || record.execution_status != "NOT_RUN_FIXED_WORKER"
        || record.created_at.is_empty()
        || record.created_at.len() > 64
    {
        return Err(contract(
            "WEAPONRY_THREEJS_DESIGN_RECORD_INVALID",
            "Three.js design record identity, counts, policy or hash is invalid",
        ));
    }
    Ok(())
}

fn validate_program(
    record: &WeaponryThreeJsDesignStoreRecord,
    bytes: &[u8],
) -> Result<Value, StoreError> {
    if bytes.is_empty() || bytes.len() as u64 > WEAPONRY_THREEJS_MAX_PROGRAM_BYTES {
        return Err(contract(
            "WEAPONRY_THREEJS_PROGRAM_BYTES_INVALID",
            "KnifeSceneProgram bytes are empty or exceed the bounded capacity",
        ));
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        contract(
            "WEAPONRY_THREEJS_PROGRAM_JSON_INVALID",
            format!("KnifeSceneProgram is not valid JSON: {error}"),
        )
    })?;
    let object = value.as_object().ok_or_else(|| {
        contract(
            "WEAPONRY_THREEJS_PROGRAM_JSON_INVALID",
            "KnifeSceneProgram must be a JSON object",
        )
    })?;
    let required_keys = [
        "asset_id",
        "blade_surface",
        "budgets",
        "canonical_sha256",
        "coordinate_convention",
        "design_basis",
        "family",
        "material_zones",
        "parts",
        "presentation",
        "schema_version",
        "unknowns",
    ];
    let allowed_keys: std::collections::BTreeSet<&str> = required_keys
        .iter()
        .copied()
        .chain(std::iter::once("assembly"))
        .collect();
    if required_keys.iter().any(|key| !object.contains_key(*key))
        || object
            .keys()
            .any(|key| !allowed_keys.contains(key.as_str()))
        || object.get("schema_version").and_then(Value::as_str)
            != Some(WEAPONRY_THREEJS_PROGRAM_SCHEMA)
        || object.get("asset_id").and_then(Value::as_str) != Some(record.asset_id.as_str())
        || object.get("family").and_then(Value::as_str) != Some(record.family.as_str())
        || object.get("coordinate_convention").and_then(Value::as_str)
            != Some("weapon-front-z-up-right-handed@1")
    {
        return Err(contract(
            "WEAPONRY_THREEJS_PROGRAM_BINDING_MISMATCH",
            "KnifeSceneProgram root shape or durable identity differs from the record",
        ));
    }
    let canonical =
        canonical_json_bytes(&value).map_err(|error| StoreError::InvalidData(error.to_string()))?;
    if canonical != bytes || sha256_hex(bytes) != record.program_object_sha256 {
        return Err(contract(
            "WEAPONRY_THREEJS_PROGRAM_OBJECT_MISMATCH",
            "KnifeSceneProgram CAS bytes are not canonical or differ from the object hash",
        ));
    }
    let payload_hash = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| {
            contract(
                "WEAPONRY_THREEJS_PROGRAM_CANONICAL_MISMATCH",
                "KnifeSceneProgram canonical_sha256 is missing",
            )
        })?;
    let mut preimage = value.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != payload_hash || payload_hash != record.program_sha256 {
        return Err(contract(
            "WEAPONRY_THREEJS_PROGRAM_CANONICAL_MISMATCH",
            "KnifeSceneProgram semantic hash differs from the durable binding",
        ));
    }
    let part_count = object
        .get("parts")
        .and_then(Value::as_array)
        .map(|items| items.len() as u64);
    let material_count = object
        .get("material_zones")
        .and_then(Value::as_array)
        .map(|items| items.len() as u64);
    if part_count != Some(record.part_count) || material_count != Some(record.material_zone_count) {
        return Err(contract(
            "WEAPONRY_THREEJS_PROGRAM_COUNT_MISMATCH",
            "KnifeSceneProgram part/material counts differ from the durable index",
        ));
    }
    Ok(value)
}

fn read_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<WeaponryThreeJsDesignStoreRecord> {
    let json: String = row.get(0)?;
    serde_json::from_str(&json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn same_request(
    left: &WeaponryThreeJsDesignStoreRecord,
    right: &WeaponryThreeJsDesignStoreRecord,
) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    left.created_at.clear();
    right.created_at.clear();
    left == right
}

fn validate_registered_object(
    connection: &Connection,
    record: &WeaponryThreeJsDesignStoreRecord,
    expected_size: u64,
) -> Result<(), StoreError> {
    let metadata: Option<(i64, String, String, String)> = connection
        .query_row(
            "SELECT size_bytes, mime, kind, reachability FROM objects WHERE sha256 = ?1",
            params![record.program_object_sha256],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    let Some((size, mime, kind, reachability)) = metadata else {
        return Err(contract(
            "WEAPONRY_THREEJS_PROGRAM_CAS_MISSING",
            "KnifeSceneProgram CAS object is not registered",
        ));
    };
    if size != expected_size as i64
        || mime != WEAPONRY_THREEJS_PROGRAM_MIME
        || kind != WEAPONRY_THREEJS_PROGRAM_OBJECT_KIND
        || !matches!(reachability.as_str(), "temporary" | "reachable")
    {
        return Err(contract(
            "WEAPONRY_THREEJS_PROGRAM_CAS_METADATA_MISMATCH",
            "KnifeSceneProgram CAS registration differs from the immutable binding",
        ));
    }
    Ok(())
}

pub(crate) fn ensure_table(connection: &Connection) -> Result<(), StoreError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS weaponry_threejs_design_records (
             schema_version TEXT NOT NULL CHECK (schema_version = 'WeaponryThreeJsDesignStoreRecord@1'),
             project_id TEXT NOT NULL REFERENCES projects(project_id),
             design_id TEXT NOT NULL,
             program_sha256 TEXT NOT NULL,
             program_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             idempotency_key TEXT NOT NULL UNIQUE,
             request_sha256 TEXT NOT NULL,
             record_json TEXT NOT NULL,
             created_at TEXT NOT NULL,
             PRIMARY KEY (project_id, design_id),
             UNIQUE (project_id, program_sha256)
         );
         CREATE INDEX IF NOT EXISTS weaponry_threejs_design_object_idx
             ON weaponry_threejs_design_records(program_object_sha256);
         CREATE TABLE IF NOT EXISTS weaponry_threejs_execution_records (
             schema_version TEXT NOT NULL CHECK (schema_version = 'WeaponryThreeJsExecutionStoreRecord@1'),
             project_id TEXT NOT NULL REFERENCES projects(project_id),
             execution_id TEXT NOT NULL,
             design_id TEXT NOT NULL,
             operation TEXT NOT NULL CHECK (operation = 'weaponry_threejs_knife_design_execute'),
             action TEXT NOT NULL CHECK (action IN ('build', 'preview', 'export')),
             program_sha256 TEXT NOT NULL,
             program_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             worker_result_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             glb_object_sha256 TEXT REFERENCES objects(sha256),
             idempotency_key TEXT NOT NULL UNIQUE,
             request_sha256 TEXT NOT NULL,
             record_json TEXT NOT NULL,
             created_at TEXT NOT NULL,
             PRIMARY KEY (project_id, execution_id),
             FOREIGN KEY (project_id, design_id) REFERENCES weaponry_threejs_design_records(project_id, design_id)
         );
         CREATE INDEX IF NOT EXISTS weaponry_threejs_execution_design_idx
             ON weaponry_threejs_execution_records(project_id, design_id);",
    )?;
    Ok(())
}

fn validate_execution_record(
    record: &WeaponryThreeJsExecutionStoreRecord,
) -> Result<(), StoreError> {
    let glb_pair_valid = match (
        &record.glb_sha256,
        &record.glb_object_sha256,
        record.glb_bytes,
    ) {
        (None, None, 0) => record.action == "preview",
        (Some(semantic), Some(object), size) => {
            record.action != "preview"
                && is_sha256(semantic)
                && is_sha256(object)
                && size > 0
                && size <= WEAPONRY_THREEJS_MAX_GLB_BYTES
        }
        _ => false,
    };
    if record.schema_version != WEAPONRY_THREEJS_EXECUTION_RECORD_SCHEMA
        || !is_opaque_id(&record.project_id)
        || !is_opaque_id(&record.execution_id)
        || !is_opaque_id(&record.design_id)
        || record.operation != "weaponry_threejs_knife_design_execute"
        || !matches!(record.action.as_str(), "build" | "preview" | "export")
        || !is_sha256(&record.program_sha256)
        || !is_sha256(&record.program_object_sha256)
        || !is_sha256(&record.worker_result_sha256)
        || !is_sha256(&record.worker_result_object_sha256)
        || !is_sha256(&record.request_sha256)
        || !is_opaque_id(&record.idempotency_key)
        || record.idempotency_key.len() > 128
        || record.triangle_count == 0
        || !(2..=64).contains(&record.part_count)
        || !glb_pair_valid
        || record.created_at.is_empty()
        || record.created_at.len() > 64
    {
        return Err(contract(
            "WEAPONRY_THREEJS_EXECUTION_RECORD_INVALID",
            "fixed Worker execution record is outside the closed policy",
        ));
    }
    Ok(())
}

fn validate_worker_result(
    record: &WeaponryThreeJsExecutionStoreRecord,
    bytes: &[u8],
) -> Result<Value, StoreError> {
    if bytes.is_empty() || bytes.len() as u64 > WEAPONRY_THREEJS_MAX_WORKER_RESULT_BYTES {
        return Err(contract(
            "WEAPONRY_THREEJS_WORKER_RESULT_BYTES_INVALID",
            "fixed Worker result is empty or exceeds the bounded capacity",
        ));
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        contract(
            "WEAPONRY_THREEJS_WORKER_RESULT_JSON_INVALID",
            format!("fixed Worker result is not JSON: {error}"),
        )
    })?;
    let object = value.as_object().ok_or_else(|| {
        contract(
            "WEAPONRY_THREEJS_WORKER_RESULT_JSON_INVALID",
            "fixed Worker result must be an object",
        )
    })?;
    let expected: std::collections::BTreeSet<&str> = [
        "schema_version",
        "operation",
        "status",
        "worker_id",
        "program_sha256",
        "program_object_sha256",
        "deterministic_fingerprint",
        "triangle_count",
        "part_ids",
        "preview_manifest",
        "glb_sha256",
        "glb_bytes",
        "glb_base64",
        "renderer_invoked",
        "visual_status",
        "human_status",
        "commercial_status",
        "canonical_sha256",
    ]
    .into_iter()
    .collect();
    let actual: std::collections::BTreeSet<&str> = object.keys().map(String::as_str).collect();
    let expected_status = match record.action.as_str() {
        "build" => "built",
        "preview" => "preview-ready",
        "export" => "exported",
        _ => "",
    };
    let part_ids = object.get("part_ids").and_then(Value::as_array);
    let part_id_set: std::collections::BTreeSet<&str> = part_ids
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if actual != expected
        || object.get("schema_version").and_then(Value::as_str)
            != Some("WeaponryThreeJsFixedWorkerResult@1")
        || object.get("operation").and_then(Value::as_str) != Some(record.action.as_str())
        || object.get("status").and_then(Value::as_str) != Some(expected_status)
        || object.get("worker_id").and_then(Value::as_str)
            != Some("weaponry-threejs-fixed-knife-worker@1")
        || object.get("program_sha256").and_then(Value::as_str)
            != Some(record.program_sha256.as_str())
        || object.get("program_object_sha256").and_then(Value::as_str)
            != Some(record.program_object_sha256.as_str())
        || object.get("triangle_count").and_then(Value::as_u64) != Some(record.triangle_count)
        || part_ids.map(|items| items.len() as u64) != Some(record.part_count)
        || part_id_set.len() as u64 != record.part_count
        || object.get("renderer_invoked").and_then(Value::as_bool) != Some(false)
        || object.get("visual_status").and_then(Value::as_str) != Some("NOT_RUN")
        || object.get("human_status").and_then(Value::as_str) != Some("NOT_RUN")
        || object.get("commercial_status").and_then(Value::as_str) != Some("NOT_RUN")
    {
        return Err(contract(
            "WEAPONRY_THREEJS_WORKER_RESULT_BINDING_MISMATCH",
            "fixed Worker result differs from its durable execution record",
        ));
    }
    let glb_shape_matches = if record.action == "preview" {
        object.get("glb_sha256") == Some(&Value::Null)
            && object.get("glb_base64") == Some(&Value::Null)
            && object.get("glb_bytes").and_then(Value::as_u64) == Some(0)
    } else {
        object.get("glb_sha256").and_then(Value::as_str) == record.glb_sha256.as_deref()
            && object.get("glb_base64").and_then(Value::as_str).is_some()
            && object.get("glb_bytes").and_then(Value::as_u64) == Some(record.glb_bytes)
    };
    if !glb_shape_matches {
        return Err(contract(
            "WEAPONRY_THREEJS_WORKER_RESULT_GLB_MISMATCH",
            "fixed Worker result GLB fields differ from the durable action",
        ));
    }
    let supplied = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| {
            contract(
                "WEAPONRY_THREEJS_WORKER_RESULT_CANONICAL_MISMATCH",
                "fixed Worker result canonical hash is missing",
            )
        })?;
    let mut preimage = value.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    let canonical =
        canonical_json_bytes(&value).map_err(|error| StoreError::InvalidData(error.to_string()))?;
    if canonical != bytes
        || canonical_json_hash(&preimage) != supplied
        || supplied != record.worker_result_sha256
        || sha256_hex(bytes) != record.worker_result_object_sha256
    {
        return Err(contract(
            "WEAPONRY_THREEJS_WORKER_RESULT_CANONICAL_MISMATCH",
            "fixed Worker result semantic or object hash differs",
        ));
    }
    Ok(value)
}

fn read_execution_record(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<WeaponryThreeJsExecutionStoreRecord> {
    let json: String = row.get(0)?;
    serde_json::from_str(&json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn same_execution_request(
    left: &WeaponryThreeJsExecutionStoreRecord,
    right: &WeaponryThreeJsExecutionStoreRecord,
) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    left.created_at.clear();
    right.created_at.clear();
    left == right
}

impl Store {
    pub fn get_weaponry_threejs_execution(
        &self,
        project_id: &str,
        idempotency_key: &str,
    ) -> Result<Option<WeaponryThreeJsExecutionStoreRecord>, StoreError> {
        if !is_opaque_id(project_id) || !is_opaque_id(idempotency_key) {
            return Err(StoreError::InvalidData(
                "Three.js execution lookup is invalid".to_owned(),
            ));
        }
        let connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let record = connection
            .query_row(
                "SELECT record_json FROM weaponry_threejs_execution_records WHERE project_id = ?1 AND idempotency_key = ?2",
                params![project_id, idempotency_key],
                read_execution_record,
            )
            .optional()?;
        if let Some(record) = &record {
            validate_execution_record(record)?;
        }
        Ok(record)
    }

    pub fn read_weaponry_threejs_worker_result_json(
        &self,
        record: &WeaponryThreeJsExecutionStoreRecord,
    ) -> Result<Value, StoreError> {
        validate_execution_record(record)?;
        let bytes = self
            .cas
            .read_verified_bounded(
                &record.worker_result_object_sha256,
                WEAPONRY_THREEJS_MAX_WORKER_RESULT_BYTES,
            )
            .map_err(StoreError::from)?;
        validate_worker_result(record, &bytes)
    }

    pub fn record_weaponry_threejs_execution_with_replay(
        &self,
        commit: &WeaponryThreeJsExecutionCommit,
    ) -> Result<(WeaponryThreeJsExecutionStoreRecord, bool), StoreError> {
        validate_execution_record(&commit.record)?;
        if commit.worker_result.sha256 != commit.record.worker_result_object_sha256
            || commit.worker_result.mime != WEAPONRY_THREEJS_WORKER_RESULT_MIME
            || commit.worker_result.kind != WEAPONRY_THREEJS_WORKER_RESULT_KIND
            || commit.worker_result.size_bytes == 0
            || commit.worker_result.size_bytes > WEAPONRY_THREEJS_MAX_WORKER_RESULT_BYTES
        {
            return Err(contract(
                "WEAPONRY_THREEJS_WORKER_RESULT_CAS_INVALID",
                "fixed Worker result CAS metadata differs",
            ));
        }
        match (&commit.glb, &commit.record.glb_object_sha256) {
            (None, None) => {}
            (Some(glb), Some(expected))
                if glb.sha256 == *expected
                    && glb.mime == WEAPONRY_THREEJS_GLB_MIME
                    && glb.kind == WEAPONRY_THREEJS_GLB_KIND
                    && glb.size_bytes == commit.record.glb_bytes => {}
            _ => {
                return Err(contract(
                    "WEAPONRY_THREEJS_GLB_CAS_INVALID",
                    "fixed Worker GLB CAS metadata differs",
                ))
            }
        }
        let worker_bytes = self
            .cas
            .read_verified_bounded(
                &commit.record.worker_result_object_sha256,
                WEAPONRY_THREEJS_MAX_WORKER_RESULT_BYTES,
            )
            .map_err(StoreError::from)?;
        if sha256_hex(&worker_bytes) != commit.record.worker_result_object_sha256 {
            return Err(contract(
                "WEAPONRY_THREEJS_WORKER_RESULT_CAS_INVALID",
                "fixed Worker result bytes differ",
            ));
        }
        validate_worker_result(&commit.record, &worker_bytes)?;
        if let Some(glb_hash) = &commit.record.glb_object_sha256 {
            let bytes = self
                .cas
                .read_verified_bounded(glb_hash, WEAPONRY_THREEJS_MAX_GLB_BYTES)
                .map_err(StoreError::from)?;
            if sha256_hex(&bytes) != *glb_hash
                || Some(sha256_hex(&bytes)) != commit.record.glb_sha256
            {
                return Err(contract(
                    "WEAPONRY_THREEJS_GLB_BYTES_INVALID",
                    "fixed Worker GLB bytes differ from the durable binding",
                ));
            }
        }

        let mut connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let transaction = connection.transaction()?;
        if let Some(existing) = transaction
            .query_row(
                "SELECT record_json FROM weaponry_threejs_execution_records WHERE idempotency_key = ?1",
                params![commit.record.idempotency_key],
                read_execution_record,
            )
            .optional()?
        {
            if !same_execution_request(&existing, &commit.record) {
                return Err(contract(
                    "WEAPONRY_THREEJS_EXECUTION_IDEMPOTENCY_CONFLICT",
                    "idempotency key is bound to another fixed Worker result",
                ));
            }
            let mut roots = vec![existing.program_object_sha256.clone(), existing.worker_result_object_sha256.clone()];
            if let Some(hash) = &existing.glb_object_sha256 { roots.push(hash.clone()); }
            mark_reachable_in_transaction(&transaction, &roots)?;
            transaction.commit()?;
            return Ok((existing, true));
        }
        let design_exists: Option<String> = transaction
            .query_row(
                "SELECT design_id FROM weaponry_threejs_design_records WHERE project_id = ?1 AND design_id = ?2 AND program_sha256 = ?3 AND program_object_sha256 = ?4",
                params![commit.record.project_id, commit.record.design_id, commit.record.program_sha256, commit.record.program_object_sha256],
                |row| row.get(0),
            )
            .optional()?;
        if design_exists.is_none() {
            return Err(contract(
                "WEAPONRY_THREEJS_EXECUTION_SOURCE_MISSING",
                "fixed Worker execution is not bound to an exact durable design",
            ));
        }
        let record_json = String::from_utf8(
            canonical_json_bytes(
                &serde_json::to_value(&commit.record)
                    .map_err(|error| StoreError::InvalidData(error.to_string()))?,
            )
            .map_err(|error| StoreError::InvalidData(error.to_string()))?,
        )
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        transaction.execute(
            "INSERT INTO weaponry_threejs_execution_records (schema_version, project_id, execution_id, design_id, operation, action, program_sha256, program_object_sha256, worker_result_object_sha256, glb_object_sha256, idempotency_key, request_sha256, record_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![commit.record.schema_version, commit.record.project_id, commit.record.execution_id, commit.record.design_id, commit.record.operation, commit.record.action, commit.record.program_sha256, commit.record.program_object_sha256, commit.record.worker_result_object_sha256, commit.record.glb_object_sha256, commit.record.idempotency_key, commit.record.request_sha256, record_json, commit.record.created_at],
        )?;
        let mut roots = vec![
            commit.record.program_object_sha256.clone(),
            commit.record.worker_result_object_sha256.clone(),
        ];
        if let Some(hash) = &commit.record.glb_object_sha256 {
            roots.push(hash.clone());
        }
        mark_reachable_in_transaction(&transaction, &roots)?;
        transaction.commit()?;
        Ok((commit.record.clone(), false))
    }

    pub fn record_weaponry_threejs_design_with_replay(
        &self,
        commit: &WeaponryThreeJsDesignCommit,
    ) -> Result<(WeaponryThreeJsDesignStoreRecord, bool), StoreError> {
        validate_record(&commit.record)?;
        if commit.program.schema_version != "CasObject@1"
            || commit.program.sha256 != commit.record.program_object_sha256
            || commit.program.mime != WEAPONRY_THREEJS_PROGRAM_MIME
            || commit.program.kind != WEAPONRY_THREEJS_PROGRAM_OBJECT_KIND
            || commit.program.size_bytes == 0
            || commit.program.size_bytes > WEAPONRY_THREEJS_MAX_PROGRAM_BYTES
            || !matches!(
                commit.program.reachability.as_str(),
                "temporary" | "reachable"
            )
        {
            return Err(contract(
                "WEAPONRY_THREEJS_PROGRAM_CAS_METADATA_INVALID",
                "KnifeSceneProgram CAS metadata is outside the fixed allowlist",
            ));
        }
        let bytes = self
            .cas
            .read_verified_bounded(
                &commit.record.program_object_sha256,
                WEAPONRY_THREEJS_MAX_PROGRAM_BYTES,
            )
            .map_err(StoreError::from)?;
        validate_program(&commit.record, &bytes)?;

        let mut connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let transaction = connection.transaction()?;
        let existing = transaction
            .query_row(
                "SELECT record_json FROM weaponry_threejs_design_records WHERE idempotency_key = ?1",
                params![commit.record.idempotency_key],
                read_record,
            )
            .optional()?;
        if let Some(existing) = existing {
            validate_record(&existing)?;
            if !same_request(&existing, &commit.record) {
                return Err(contract(
                    "WEAPONRY_THREEJS_DESIGN_IDEMPOTENCY_CONFLICT",
                    "idempotency key is already bound to a different design",
                ));
            }
            validate_registered_object(&transaction, &existing, commit.program.size_bytes)?;
            mark_reachable_in_transaction(&transaction, &[existing.program_object_sha256.clone()])?;
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
                "design project does not exist",
            ));
        }
        let duplicate: Option<String> = transaction
            .query_row(
                "SELECT idempotency_key FROM weaponry_threejs_design_records WHERE project_id = ?1 AND (design_id = ?2 OR program_sha256 = ?3)",
                params![commit.record.project_id, commit.record.design_id, commit.record.program_sha256],
                |row| row.get(0),
            )
            .optional()?;
        if duplicate.is_some() {
            return Err(contract(
                "WEAPONRY_THREEJS_DESIGN_IDENTITY_CONFLICT",
                "project design identity is already bound by another request",
            ));
        }
        validate_registered_object(&transaction, &commit.record, commit.program.size_bytes)?;
        let record_json = String::from_utf8(
            canonical_json_bytes(
                &serde_json::to_value(&commit.record)
                    .map_err(|error| StoreError::InvalidData(error.to_string()))?,
            )
            .map_err(|error| StoreError::InvalidData(error.to_string()))?,
        )
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        transaction.execute(
            "INSERT INTO weaponry_threejs_design_records (schema_version, project_id, design_id, program_sha256, program_object_sha256, idempotency_key, request_sha256, record_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                commit.record.schema_version,
                commit.record.project_id,
                commit.record.design_id,
                commit.record.program_sha256,
                commit.record.program_object_sha256,
                commit.record.idempotency_key,
                commit.record.request_sha256,
                record_json,
                commit.record.created_at,
            ],
        )?;
        mark_reachable_in_transaction(
            &transaction,
            &[commit.record.program_object_sha256.clone()],
        )?;
        transaction.commit()?;
        Ok((commit.record.clone(), false))
    }

    pub fn get_weaponry_threejs_design_exact(
        &self,
        project_id: &str,
        design_id: &str,
        program_sha256: &str,
        program_object_sha256: &str,
    ) -> Result<Option<WeaponryThreeJsDesignStoreRecord>, StoreError> {
        if !is_opaque_id(project_id)
            || !is_opaque_id(design_id)
            || !is_sha256(program_sha256)
            || !is_sha256(program_object_sha256)
        {
            return Err(StoreError::InvalidData(
                "Three.js design exact lookup is invalid".to_owned(),
            ));
        }
        let connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let record = connection
            .query_row(
                "SELECT record_json FROM weaponry_threejs_design_records WHERE project_id = ?1 AND design_id = ?2 AND program_sha256 = ?3 AND program_object_sha256 = ?4",
                params![project_id, design_id, program_sha256, program_object_sha256],
                read_record,
            )
            .optional()?;
        let Some(record) = record else {
            return Ok(None);
        };
        validate_record(&record)?;
        let object_size = connection
            .query_row(
                "SELECT size_bytes FROM objects WHERE sha256 = ?1",
                params![program_object_sha256],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .and_then(|size| u64::try_from(size).ok())
            .ok_or_else(|| {
                contract(
                    "WEAPONRY_THREEJS_PROGRAM_CAS_MISSING",
                    "KnifeSceneProgram CAS object is not registered",
                )
            })?;
        validate_registered_object(&connection, &record, object_size)?;
        let bytes = self
            .cas
            .read_verified_bounded(program_object_sha256, WEAPONRY_THREEJS_MAX_PROGRAM_BYTES)
            .map_err(StoreError::from)?;
        validate_program(&record, &bytes)?;
        Ok(Some(record))
    }

    pub fn read_weaponry_threejs_program_json(
        &self,
        record: &WeaponryThreeJsDesignStoreRecord,
    ) -> Result<Value, StoreError> {
        validate_record(record)?;
        let bytes = self
            .cas
            .read_verified_bounded(
                &record.program_object_sha256,
                WEAPONRY_THREEJS_MAX_PROGRAM_BYTES,
            )
            .map_err(StoreError::from)?;
        validate_program(record, &bytes)
    }
}

fn validate_preview_record(record: &WeaponryThreeJsPreviewStoreRecord) -> Result<(), StoreError> {
    if record.schema_version != WEAPONRY_THREEJS_PREVIEW_RECORD_SCHEMA
        || !is_opaque_id(&record.project_id)
        || !is_opaque_id(&record.execution_id)
        || !is_opaque_id(&record.design_id)
        || record.operation != "weaponry_threejs_knife_design_execute"
        || record.action != "preview"
        || !is_sha256(&record.program_sha256)
        || !is_sha256(&record.program_object_sha256)
        || record.worker_id != "weaponry-threejs-fixed-knife-worker@1"
        || record.preview_runtime_id != WEAPONRY_THREEJS_PREVIEW_RUNTIME_ID
        || !is_sha256(&record.preview_runtime_sha256)
        || !is_sha256(&record.preview_dependency_lock_sha256)
        || !is_sha256(&record.preview_worker_cohort_sha256)
        || !is_sha256(&record.worker_result_sha256)
        || !is_sha256(&record.worker_result_object_sha256)
        || !is_sha256(&record.preview_receipt_sha256)
        || !is_sha256(&record.preview_receipt_object_sha256)
        || record.view_count != WEAPONRY_THREEJS_PREVIEW_VIEW_COUNT as u64
        || record.aov_count != WEAPONRY_THREEJS_PREVIEW_AOV_COUNT as u64
        || !is_sha256(&record.request_sha256)
        || !is_opaque_id(&record.idempotency_key)
        || record.idempotency_key.len() > 128
        || record.created_at.is_empty()
        || record.created_at.len() > 64
    {
        return Err(contract(
            "WEAPONRY_THREEJS_PREVIEW_RECORD_INVALID",
            "Three.js preview record identity, runtime, view or hash is invalid",
        ));
    }
    Ok(())
}

fn exact_keys(value: &Value, expected: &[&str]) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object.len() == expected.len() && expected.iter().all(|key| object.contains_key(*key))
}

fn preview_worker_fields() -> [&'static str; 25] {
    [
        "schema_version",
        "operation",
        "status",
        "worker_id",
        "program_sha256",
        "program_object_sha256",
        "deterministic_fingerprint",
        "triangle_count",
        "part_ids",
        "preview_manifest",
        "glb_sha256",
        "glb_bytes",
        "glb_base64",
        "renderer_invoked",
        "visual_status",
        "human_status",
        "commercial_status",
        "preview_runtime_id",
        "preview_runtime_sha256",
        "preview_dependency_lock_sha256",
        "preview_worker_cohort_sha256",
        "preview_view_count",
        "preview_aov_count",
        "preview_views",
        "canonical_sha256",
    ]
}

fn validate_preview_worker_result(
    record: &WeaponryThreeJsPreviewStoreRecord,
    bytes: &[u8],
) -> Result<Value, StoreError> {
    if bytes.is_empty() || bytes.len() as u64 > WEAPONRY_THREEJS_MAX_WORKER_RESULT_BYTES {
        return Err(contract(
            "WEAPONRY_THREEJS_PREVIEW_WORKER_RESULT_BYTES_INVALID",
            "preview Worker result is empty or exceeds the bounded capacity",
        ));
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        contract(
            "WEAPONRY_THREEJS_PREVIEW_WORKER_RESULT_JSON_INVALID",
            format!("preview Worker result is not JSON: {error}"),
        )
    })?;
    if !exact_keys(&value, &preview_worker_fields()) {
        return Err(contract(
            "WEAPONRY_THREEJS_PREVIEW_WORKER_RESULT_FIELDS_INVALID",
            "preview Worker result fields are not closed",
        ));
    }
    let object = value.as_object().expect("exact_keys checked object");
    let part_ids = object
        .get("part_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "WEAPONRY_THREEJS_PREVIEW_WORKER_RESULT_BINDING_MISMATCH",
                "preview Worker result part_ids is not an array",
            )
        })?;
    let part_id_set: std::collections::BTreeSet<&str> =
        part_ids.iter().filter_map(Value::as_str).collect();
    let glb_null = object.get("glb_sha256") == Some(&Value::Null)
        && object.get("glb_base64") == Some(&Value::Null)
        && object.get("glb_bytes").and_then(Value::as_u64) == Some(0);
    if object.get("schema_version").and_then(Value::as_str)
        != Some("WeaponryThreeJsFixedWorkerResult@1")
        || object.get("operation").and_then(Value::as_str) != Some("preview")
        || object.get("status").and_then(Value::as_str) != Some("preview-ready")
        || object.get("worker_id").and_then(Value::as_str) != Some(record.worker_id.as_str())
        || object.get("program_sha256").and_then(Value::as_str)
            != Some(record.program_sha256.as_str())
        || object.get("program_object_sha256").and_then(Value::as_str)
            != Some(record.program_object_sha256.as_str())
        || object
            .get("triangle_count")
            .and_then(Value::as_u64)
            .is_none()
        || part_ids.len() < 2
        || part_ids.len() > 64
        || part_id_set.len() != part_ids.len()
        || object.get("renderer_invoked").and_then(Value::as_bool) != Some(true)
        || object.get("visual_status").and_then(Value::as_str) != Some("NOT_RUN")
        || object.get("human_status").and_then(Value::as_str) != Some("NOT_RUN")
        || object.get("commercial_status").and_then(Value::as_str) != Some("NOT_RUN")
        || object.get("preview_runtime_id").and_then(Value::as_str)
            != Some(record.preview_runtime_id.as_str())
        || object.get("preview_runtime_sha256").and_then(Value::as_str)
            != Some(record.preview_runtime_sha256.as_str())
        || object
            .get("preview_dependency_lock_sha256")
            .and_then(Value::as_str)
            != Some(record.preview_dependency_lock_sha256.as_str())
        || object
            .get("preview_worker_cohort_sha256")
            .and_then(Value::as_str)
            != Some(record.preview_worker_cohort_sha256.as_str())
        || object.get("preview_view_count").and_then(Value::as_u64) != Some(record.view_count)
        || object.get("preview_aov_count").and_then(Value::as_u64) != Some(record.aov_count)
        || !glb_null
    {
        return Err(contract(
            "WEAPONRY_THREEJS_PREVIEW_WORKER_RESULT_BINDING_MISMATCH",
            "preview Worker result differs from its durable runtime binding",
        ));
    }
    let supplied = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| {
            contract(
                "WEAPONRY_THREEJS_PREVIEW_WORKER_RESULT_CANONICAL_MISMATCH",
                "preview Worker result canonical hash is missing",
            )
        })?;
    let mut preimage = value.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    let canonical =
        canonical_json_bytes(&value).map_err(|error| StoreError::InvalidData(error.to_string()))?;
    if canonical != bytes
        || canonical_json_hash(&preimage) != supplied
        || supplied != record.worker_result_sha256
        || sha256_hex(bytes) != record.worker_result_object_sha256
    {
        return Err(contract(
            "WEAPONRY_THREEJS_PREVIEW_WORKER_RESULT_CANONICAL_MISMATCH",
            "preview Worker result semantic or object hash differs",
        ));
    }
    Ok(value)
}

fn preview_receipt_fields() -> [&'static str; 16] {
    [
        "schema_version",
        "operation",
        "project_id",
        "execution_id",
        "design_id",
        "program_sha256",
        "program_object_sha256",
        "worker_result_sha256",
        "worker_result_object_sha256",
        "preview_runtime_id",
        "preview_runtime_sha256",
        "preview_dependency_lock_sha256",
        "preview_worker_cohort_sha256",
        "view_count",
        "aov_count",
        "views",
    ]
}

fn validate_preview_receipt(
    record: &WeaponryThreeJsPreviewStoreRecord,
    bytes: &[u8],
    worker_result: &Value,
) -> Result<Value, StoreError> {
    if bytes.is_empty() || bytes.len() as u64 > WEAPONRY_THREEJS_MAX_WORKER_RESULT_BYTES {
        return Err(contract(
            "WEAPONRY_THREEJS_PREVIEW_RECEIPT_BYTES_INVALID",
            "preview receipt is empty or exceeds the bounded capacity",
        ));
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        contract(
            "WEAPONRY_THREEJS_PREVIEW_RECEIPT_JSON_INVALID",
            format!("preview receipt is not JSON: {error}"),
        )
    })?;
    let mut expected = preview_receipt_fields().to_vec();
    expected.push("canonical_sha256");
    if !exact_keys(&value, &expected) {
        return Err(contract(
            "WEAPONRY_THREEJS_PREVIEW_RECEIPT_FIELDS_INVALID",
            "preview receipt fields are not closed",
        ));
    }
    let object = value.as_object().expect("exact_keys checked object");
    if object.get("schema_version").and_then(Value::as_str)
        != Some(WEAPONRY_THREEJS_PREVIEW_RECEIPT_SCHEMA)
        || object.get("operation").and_then(Value::as_str)
            != Some("weaponry_threejs_knife_design_preview")
        || object.get("project_id").and_then(Value::as_str) != Some(record.project_id.as_str())
        || object.get("execution_id").and_then(Value::as_str) != Some(record.execution_id.as_str())
        || object.get("design_id").and_then(Value::as_str) != Some(record.design_id.as_str())
        || object.get("program_sha256").and_then(Value::as_str)
            != Some(record.program_sha256.as_str())
        || object.get("program_object_sha256").and_then(Value::as_str)
            != Some(record.program_object_sha256.as_str())
        || object.get("worker_result_sha256").and_then(Value::as_str)
            != Some(record.worker_result_sha256.as_str())
        || object
            .get("worker_result_object_sha256")
            .and_then(Value::as_str)
            != Some(record.worker_result_object_sha256.as_str())
        || object.get("preview_runtime_id").and_then(Value::as_str)
            != Some(record.preview_runtime_id.as_str())
        || object.get("preview_runtime_sha256").and_then(Value::as_str)
            != Some(record.preview_runtime_sha256.as_str())
        || object
            .get("preview_dependency_lock_sha256")
            .and_then(Value::as_str)
            != Some(record.preview_dependency_lock_sha256.as_str())
        || object
            .get("preview_worker_cohort_sha256")
            .and_then(Value::as_str)
            != Some(record.preview_worker_cohort_sha256.as_str())
        || object.get("view_count").and_then(Value::as_u64) != Some(record.view_count)
        || object.get("aov_count").and_then(Value::as_u64) != Some(record.aov_count)
    {
        return Err(contract(
            "WEAPONRY_THREEJS_PREVIEW_RECEIPT_BINDING_MISMATCH",
            "preview receipt differs from its durable runtime binding",
        ));
    }
    let views = object
        .get("views")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "WEAPONRY_THREEJS_PREVIEW_RECEIPT_VIEWS_INVALID",
                "preview receipt views is not an array",
            )
        })?;
    let view_ids = [
        "FRONT",
        "BACK",
        "TOP",
        "BOTTOM",
        "LEFT",
        "RIGHT",
        "REAR_THREE_QUARTER",
        "FPS_HOLD",
    ];
    let aov_ids = [
        "beauty",
        "alpha-silhouette",
        "semantic-id",
        "depth",
        "normal",
        "roughness-material-id",
    ];
    if views.len() != WEAPONRY_THREEJS_PREVIEW_VIEW_COUNT {
        return Err(contract(
            "WEAPONRY_THREEJS_PREVIEW_RECEIPT_VIEW_COUNT_MISMATCH",
            "preview receipt must contain exactly eight fixed views",
        ));
    }
    for (index, view) in views.iter().enumerate() {
        let Some(view_object) = view.as_object() else {
            return Err(contract(
                "WEAPONRY_THREEJS_PREVIEW_RECEIPT_VIEW_INVALID",
                "preview view must be an object",
            ));
        };
        let expected_view_fields = [
            "view_id",
            "camera_sha256",
            "worker_cohort_sha256",
            "width",
            "height",
            "passes",
        ];
        if view_object.len() != expected_view_fields.len()
            || !expected_view_fields
                .iter()
                .all(|field| view_object.contains_key(*field))
            || view_object.get("view_id").and_then(Value::as_str) != Some(view_ids[index])
            || view_object
                .get("camera_sha256")
                .and_then(Value::as_str)
                .is_none_or(|hash| !is_sha256(hash))
            || view_object
                .get("worker_cohort_sha256")
                .and_then(Value::as_str)
                != Some(record.preview_worker_cohort_sha256.as_str())
            || view_object.get("width").and_then(Value::as_u64) != Some(512)
            || view_object.get("height").and_then(Value::as_u64) != Some(512)
        {
            return Err(contract(
                "WEAPONRY_THREEJS_PREVIEW_RECEIPT_VIEW_BINDING_MISMATCH",
                "preview view camera, cohort or dimensions differ",
            ));
        }
        let passes = view_object
            .get("passes")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                contract(
                    "WEAPONRY_THREEJS_PREVIEW_RECEIPT_PASSES_INVALID",
                    "preview view passes is not an array",
                )
            })?;
        if passes.len() != WEAPONRY_THREEJS_PREVIEW_AOVS_PER_VIEW {
            return Err(contract(
                "WEAPONRY_THREEJS_PREVIEW_RECEIPT_AOV_COUNT_MISMATCH",
                "preview view must contain exactly six AOV passes",
            ));
        }
        for (pass_index, pass) in passes.iter().enumerate() {
            let Some(pass_object) = pass.as_object() else {
                return Err(contract(
                    "WEAPONRY_THREEJS_PREVIEW_RECEIPT_PASS_INVALID",
                    "preview AOV pass must be an object",
                ));
            };
            let pass_fields = ["aov_id", "sha256", "object_sha256", "bytes", "mime"];
            if pass_object.len() != pass_fields.len()
                || !pass_fields
                    .iter()
                    .all(|field| pass_object.contains_key(*field))
                || pass_object.get("aov_id").and_then(Value::as_str) != Some(aov_ids[pass_index])
                || pass_object
                    .get("sha256")
                    .and_then(Value::as_str)
                    .is_none_or(|hash| !is_sha256(hash))
                || pass_object
                    .get("object_sha256")
                    .and_then(Value::as_str)
                    .is_none_or(|hash| !is_sha256(hash))
                || pass_object
                    .get("bytes")
                    .and_then(Value::as_u64)
                    .is_none_or(|size| size == 0 || size > WEAPONRY_THREEJS_PREVIEW_MAX_AOV_BYTES)
                || pass_object.get("mime").and_then(Value::as_str)
                    != Some(WEAPONRY_THREEJS_PREVIEW_AOV_MIME)
            {
                return Err(contract(
                    "WEAPONRY_THREEJS_PREVIEW_RECEIPT_PASS_BINDING_MISMATCH",
                    "preview AOV pass hash, size, mime or order differs",
                ));
            }
        }
    }
    if worker_result.get("preview_views") != Some(object.get("views").expect("views checked")) {
        return Err(contract(
            "WEAPONRY_THREEJS_PREVIEW_RECEIPT_WORKER_MISMATCH",
            "preview receipt view descriptors differ from Worker result",
        ));
    }
    let supplied = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| {
            contract(
                "WEAPONRY_THREEJS_PREVIEW_RECEIPT_CANONICAL_MISMATCH",
                "preview receipt canonical hash is missing",
            )
        })?;
    let mut preimage = value.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    let canonical =
        canonical_json_bytes(&value).map_err(|error| StoreError::InvalidData(error.to_string()))?;
    if canonical != bytes
        || canonical_json_hash(&preimage) != supplied
        || supplied != record.preview_receipt_sha256
        || sha256_hex(bytes) != record.preview_receipt_object_sha256
    {
        return Err(contract(
            "WEAPONRY_THREEJS_PREVIEW_RECEIPT_CANONICAL_MISMATCH",
            "preview receipt semantic or object hash differs",
        ));
    }
    Ok(value)
}

fn validate_preview_object_metadata(
    connection: &Connection,
    object: &CasObjectRecord,
    expected_kind: &str,
    expected_mime: &str,
    max_bytes: u64,
) -> Result<(), StoreError> {
    if object.schema_version != "CasObject@1"
        || !is_sha256(&object.sha256)
        || object.mime != expected_mime
        || object.kind != expected_kind
        || object.size_bytes == 0
        || object.size_bytes > max_bytes
    {
        return Err(contract(
            "WEAPONRY_THREEJS_PREVIEW_CAS_METADATA_INVALID",
            "preview CAS metadata is outside the fixed allowlist",
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
            expected_mime.to_owned(),
            expected_kind.to_owned(),
        ))
    {
        return Err(contract(
            "WEAPONRY_THREEJS_PREVIEW_CAS_METADATA_MISMATCH",
            "preview CAS metadata is not registered exactly",
        ));
    }
    Ok(())
}

fn preview_reference_hashes(receipt: &Value) -> Result<Vec<(String, String, u64)>, StoreError> {
    let views = receipt["views"].as_array().ok_or_else(|| {
        contract(
            "WEAPONRY_THREEJS_PREVIEW_RECEIPT_VIEWS_INVALID",
            "preview receipt views is not an array",
        )
    })?;
    let mut references = Vec::with_capacity(WEAPONRY_THREEJS_PREVIEW_AOV_COUNT);
    for view in views {
        let view_id = view["view_id"].as_str().ok_or_else(|| {
            contract(
                "WEAPONRY_THREEJS_PREVIEW_RECEIPT_VIEW_INVALID",
                "preview view id is missing",
            )
        })?;
        for pass in view["passes"].as_array().ok_or_else(|| {
            contract(
                "WEAPONRY_THREEJS_PREVIEW_RECEIPT_PASSES_INVALID",
                "preview view passes is not an array",
            )
        })? {
            let hash = pass["object_sha256"].as_str().ok_or_else(|| {
                contract(
                    "WEAPONRY_THREEJS_PREVIEW_RECEIPT_PASS_INVALID",
                    "preview pass object hash is missing",
                )
            })?;
            let aov_id = pass["aov_id"].as_str().ok_or_else(|| {
                contract(
                    "WEAPONRY_THREEJS_PREVIEW_RECEIPT_PASS_INVALID",
                    "preview pass id is missing",
                )
            })?;
            let bytes = pass["bytes"].as_u64().ok_or_else(|| {
                contract(
                    "WEAPONRY_THREEJS_PREVIEW_RECEIPT_PASS_INVALID",
                    "preview pass byte count is missing",
                )
            })?;
            references.push((format!("{view_id}:{aov_id}"), hash.to_owned(), bytes));
        }
    }
    if references.len() != WEAPONRY_THREEJS_PREVIEW_AOV_COUNT {
        return Err(contract(
            "WEAPONRY_THREEJS_PREVIEW_RECEIPT_AOV_COUNT_MISMATCH",
            "preview receipt must reference exactly 48 AOV objects",
        ));
    }
    Ok(references)
}

fn validate_preview_aov_objects(
    store: &Store,
    connection: &Connection,
    receipt: &Value,
    supplied_objects: &[CasObjectRecord],
) -> Result<Vec<String>, StoreError> {
    let references = preview_reference_hashes(receipt)?;
    let supplied: std::collections::BTreeMap<&str, &CasObjectRecord> = supplied_objects
        .iter()
        .map(|object| (object.sha256.as_str(), object))
        .collect();
    let mut hashes = Vec::with_capacity(references.len());
    for (_, object_hash, expected_size) in references {
        let object = if let Some(object) = supplied.get(object_hash.as_str()) {
            *object
        } else {
            let Some((size, mime, kind, reachability)) = connection
                .query_row(
                    "SELECT size_bytes, mime, kind, reachability FROM objects WHERE sha256 = ?1",
                    params![object_hash],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                        ))
                    },
                )
                .optional()?
            else {
                return Err(contract(
                    "WEAPONRY_THREEJS_PREVIEW_AOV_CAS_MISSING",
                    "preview AOV CAS object is not registered",
                ));
            };
            if size != expected_size as i64
                || mime != WEAPONRY_THREEJS_PREVIEW_AOV_MIME
                || kind != WEAPONRY_THREEJS_PREVIEW_AOV_KIND
                || !matches!(reachability.as_str(), "temporary" | "reachable")
            {
                return Err(contract(
                    "WEAPONRY_THREEJS_PREVIEW_AOV_CAS_METADATA_MISMATCH",
                    "preview AOV CAS metadata differs from receipt",
                ));
            }
            let bytes = store
                .cas
                .read_verified_bounded(&object_hash, WEAPONRY_THREEJS_PREVIEW_MAX_AOV_BYTES)
                .map_err(StoreError::from)?;
            if bytes.len() as u64 != expected_size || sha256_hex(&bytes) != object_hash {
                return Err(contract(
                    "WEAPONRY_THREEJS_PREVIEW_AOV_BYTES_INVALID",
                    "preview AOV bytes differ from receipt object hash",
                ));
            }
            hashes.push(object_hash);
            continue;
        };
        validate_preview_object_metadata(
            connection,
            object,
            WEAPONRY_THREEJS_PREVIEW_AOV_KIND,
            WEAPONRY_THREEJS_PREVIEW_AOV_MIME,
            WEAPONRY_THREEJS_PREVIEW_MAX_AOV_BYTES,
        )?;
        let bytes = store
            .cas
            .read_verified_bounded(&object.sha256, WEAPONRY_THREEJS_PREVIEW_MAX_AOV_BYTES)
            .map_err(StoreError::from)?;
        if bytes.len() as u64 != expected_size || sha256_hex(&bytes) != object.sha256 {
            return Err(contract(
                "WEAPONRY_THREEJS_PREVIEW_AOV_BYTES_INVALID",
                "preview AOV bytes differ from receipt object hash",
            ));
        }
        hashes.push(object.sha256.clone());
    }
    Ok(hashes)
}

fn read_preview_record(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<WeaponryThreeJsPreviewStoreRecord> {
    let json: String = row.get(0)?;
    serde_json::from_str(&json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn same_preview_request(
    left: &WeaponryThreeJsPreviewStoreRecord,
    right: &WeaponryThreeJsPreviewStoreRecord,
) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    left.created_at.clear();
    right.created_at.clear();
    left == right
}

fn preview_record_roots(
    transaction: &rusqlite::Transaction<'_>,
    record: &WeaponryThreeJsPreviewStoreRecord,
) -> Result<Vec<String>, StoreError> {
    let mut roots = vec![
        record.program_object_sha256.clone(),
        record.worker_result_object_sha256.clone(),
        record.preview_receipt_object_sha256.clone(),
    ];
    let mut statement = transaction.prepare(
        "SELECT object_sha256 FROM weaponry_threejs_preview_aov_refs WHERE project_id = ?1 AND execution_id = ?2 ORDER BY view_id, aov_id",
    )?;
    let rows = statement.query_map(params![record.project_id, record.execution_id], |row| {
        row.get::<_, String>(0)
    })?;
    for row in rows {
        roots.push(row?);
    }
    Ok(roots)
}

pub(crate) fn ensure_preview_table(connection: &Connection) -> Result<(), StoreError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS weaponry_threejs_preview_records (
             schema_version TEXT NOT NULL CHECK (schema_version = 'WeaponryThreeJsPreviewStoreRecord@1'),
             project_id TEXT NOT NULL REFERENCES projects(project_id),
             execution_id TEXT NOT NULL,
             design_id TEXT NOT NULL,
             operation TEXT NOT NULL CHECK (operation = 'weaponry_threejs_knife_design_execute'),
             action TEXT NOT NULL CHECK (action = 'preview'),
             program_sha256 TEXT NOT NULL,
             program_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             worker_result_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             preview_receipt_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             idempotency_key TEXT NOT NULL UNIQUE,
             request_sha256 TEXT NOT NULL,
             record_json TEXT NOT NULL,
             created_at TEXT NOT NULL,
             PRIMARY KEY (project_id, execution_id),
             FOREIGN KEY (project_id, design_id) REFERENCES weaponry_threejs_design_records(project_id, design_id)
         );
         CREATE TABLE IF NOT EXISTS weaponry_threejs_preview_aov_refs (
             project_id TEXT NOT NULL,
             execution_id TEXT NOT NULL,
             view_id TEXT NOT NULL,
             aov_id TEXT NOT NULL,
             object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             PRIMARY KEY (project_id, execution_id, view_id, aov_id),
             FOREIGN KEY (project_id, execution_id) REFERENCES weaponry_threejs_preview_records(project_id, execution_id)
         );
         CREATE INDEX IF NOT EXISTS weaponry_threejs_preview_records_design_idx
             ON weaponry_threejs_preview_records(project_id, design_id);
         CREATE INDEX IF NOT EXISTS weaponry_threejs_preview_aov_refs_object_idx
             ON weaponry_threejs_preview_aov_refs(object_sha256);",
    )?;
    Ok(())
}

impl Store {
    fn verify_weaponry_threejs_preview_record(
        &self,
        connection: &Connection,
        record: &WeaponryThreeJsPreviewStoreRecord,
    ) -> Result<Value, StoreError> {
        validate_preview_record(record)?;
        let worker_bytes = self
            .cas
            .read_verified_bounded(
                &record.worker_result_object_sha256,
                WEAPONRY_THREEJS_MAX_WORKER_RESULT_BYTES,
            )
            .map_err(StoreError::from)?;
        let worker_result = validate_preview_worker_result(record, &worker_bytes)?;
        let receipt_bytes = self
            .cas
            .read_verified_bounded(
                &record.preview_receipt_object_sha256,
                WEAPONRY_THREEJS_MAX_WORKER_RESULT_BYTES,
            )
            .map_err(StoreError::from)?;
        let receipt = validate_preview_receipt(record, &receipt_bytes, &worker_result)?;
        validate_preview_aov_objects(self, connection, &receipt, &[])?;
        Ok(receipt)
    }

    pub fn record_weaponry_threejs_preview_with_replay(
        &self,
        commit: &WeaponryThreeJsPreviewCommit,
    ) -> Result<(WeaponryThreeJsPreviewStoreRecord, bool), StoreError> {
        validate_preview_record(&commit.record)?;
        if commit.worker_result.sha256 != commit.record.worker_result_object_sha256
            || commit.worker_result.mime != WEAPONRY_THREEJS_WORKER_RESULT_MIME
            || commit.worker_result.kind != WEAPONRY_THREEJS_WORKER_RESULT_KIND
            || commit.worker_result.size_bytes == 0
            || commit.worker_result.size_bytes > WEAPONRY_THREEJS_MAX_WORKER_RESULT_BYTES
            || commit.receipt.sha256 != commit.record.preview_receipt_object_sha256
            || commit.receipt.mime != WEAPONRY_THREEJS_PREVIEW_RECEIPT_MIME
            || commit.receipt.kind != WEAPONRY_THREEJS_PREVIEW_RECEIPT_KIND
            || commit.receipt.size_bytes == 0
            || commit.receipt.size_bytes > WEAPONRY_THREEJS_MAX_WORKER_RESULT_BYTES
        {
            return Err(contract(
                "WEAPONRY_THREEJS_PREVIEW_CAS_INVALID",
                "preview Worker result or receipt CAS metadata differs",
            ));
        }
        let worker_bytes = self
            .cas
            .read_verified_bounded(
                &commit.record.worker_result_object_sha256,
                WEAPONRY_THREEJS_MAX_WORKER_RESULT_BYTES,
            )
            .map_err(StoreError::from)?;
        let worker_result = validate_preview_worker_result(&commit.record, &worker_bytes)?;
        let receipt_bytes = self
            .cas
            .read_verified_bounded(
                &commit.record.preview_receipt_object_sha256,
                WEAPONRY_THREEJS_MAX_WORKER_RESULT_BYTES,
            )
            .map_err(StoreError::from)?;
        let receipt = validate_preview_receipt(&commit.record, &receipt_bytes, &worker_result)?;
        let mut connection = self.lock_connection()?;
        ensure_table(&connection)?;
        ensure_preview_table(&connection)?;
        validate_preview_object_metadata(
            &connection,
            &commit.worker_result,
            WEAPONRY_THREEJS_WORKER_RESULT_KIND,
            WEAPONRY_THREEJS_WORKER_RESULT_MIME,
            WEAPONRY_THREEJS_MAX_WORKER_RESULT_BYTES,
        )?;
        validate_preview_object_metadata(
            &connection,
            &commit.receipt,
            WEAPONRY_THREEJS_PREVIEW_RECEIPT_KIND,
            WEAPONRY_THREEJS_PREVIEW_RECEIPT_MIME,
            WEAPONRY_THREEJS_MAX_WORKER_RESULT_BYTES,
        )?;
        let aov_hashes =
            validate_preview_aov_objects(self, &connection, &receipt, &commit.aov_objects)?;
        let supplied_hashes: std::collections::BTreeSet<&str> = commit
            .aov_objects
            .iter()
            .map(|object| object.sha256.as_str())
            .collect();
        let referenced_hashes: std::collections::BTreeSet<&str> =
            aov_hashes.iter().map(String::as_str).collect();
        if supplied_hashes != referenced_hashes {
            return Err(contract(
                "WEAPONRY_THREEJS_PREVIEW_AOV_SUPPLY_MISMATCH",
                "preview commit must supply every distinct AOV CAS object referenced by 48 passes",
            ));
        }
        let transaction = connection.transaction()?;
        if let Some(existing) = transaction
            .query_row(
                "SELECT record_json FROM weaponry_threejs_preview_records WHERE idempotency_key = ?1",
                params![commit.record.idempotency_key],
                read_preview_record,
            )
            .optional()?
        {
            if !same_preview_request(&existing, &commit.record) {
                return Err(contract(
                    "WEAPONRY_THREEJS_PREVIEW_IDEMPOTENCY_CONFLICT",
                    "preview idempotency key is already bound to another result",
                ));
            }
            let roots = preview_record_roots(&transaction, &existing)?;
            mark_reachable_in_transaction(&transaction, &roots)?;
            transaction.commit()?;
            return Ok((existing, true));
        }
        let design_exists: Option<String> = transaction
            .query_row(
                "SELECT design_id FROM weaponry_threejs_design_records WHERE project_id = ?1 AND design_id = ?2 AND program_sha256 = ?3 AND program_object_sha256 = ?4",
                params![commit.record.project_id, commit.record.design_id, commit.record.program_sha256, commit.record.program_object_sha256],
                |row| row.get(0),
            )
            .optional()?;
        if design_exists.is_none() {
            return Err(contract(
                "WEAPONRY_THREEJS_PREVIEW_SOURCE_MISSING",
                "preview is not bound to an exact durable design",
            ));
        }
        let record_json = String::from_utf8(
            canonical_json_bytes(
                &serde_json::to_value(&commit.record)
                    .map_err(|error| StoreError::InvalidData(error.to_string()))?,
            )
            .map_err(|error| StoreError::InvalidData(error.to_string()))?,
        )
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        transaction.execute(
            "INSERT INTO weaponry_threejs_preview_records (schema_version, project_id, execution_id, design_id, operation, action, program_sha256, program_object_sha256, worker_result_object_sha256, preview_receipt_object_sha256, idempotency_key, request_sha256, record_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![commit.record.schema_version, commit.record.project_id, commit.record.execution_id, commit.record.design_id, commit.record.operation, commit.record.action, commit.record.program_sha256, commit.record.program_object_sha256, commit.record.worker_result_object_sha256, commit.record.preview_receipt_object_sha256, commit.record.idempotency_key, commit.record.request_sha256, record_json, commit.record.created_at],
        )?;
        let views = receipt["views"].as_array().expect("receipt validation");
        let mut index = 0usize;
        for view in views {
            let view_id = view["view_id"].as_str().expect("receipt validation");
            for pass in view["passes"].as_array().expect("receipt validation") {
                let aov_id = pass["aov_id"].as_str().expect("receipt validation");
                transaction.execute(
                    "INSERT INTO weaponry_threejs_preview_aov_refs (project_id, execution_id, view_id, aov_id, object_sha256) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![commit.record.project_id, commit.record.execution_id, view_id, aov_id, aov_hashes[index]],
                )?;
                index += 1;
            }
        }
        let roots = preview_record_roots(&transaction, &commit.record)?;
        mark_reachable_in_transaction(&transaction, &roots)?;
        transaction.commit()?;
        Ok((commit.record.clone(), false))
    }

    pub fn get_weaponry_threejs_preview(
        &self,
        project_id: &str,
        idempotency_key: &str,
    ) -> Result<Option<WeaponryThreeJsPreviewStoreRecord>, StoreError> {
        if !is_opaque_id(project_id) || !is_opaque_id(idempotency_key) {
            return Err(StoreError::InvalidData(
                "Three.js preview lookup is invalid".to_owned(),
            ));
        }
        let connection = self.lock_connection()?;
        ensure_table(&connection)?;
        ensure_preview_table(&connection)?;
        let record = connection
            .query_row(
                "SELECT record_json FROM weaponry_threejs_preview_records WHERE project_id = ?1 AND idempotency_key = ?2",
                params![project_id, idempotency_key],
                read_preview_record,
            )
            .optional()?;
        if let Some(record) = &record {
            self.verify_weaponry_threejs_preview_record(&connection, record)?;
        }
        Ok(record)
    }

    pub fn get_weaponry_threejs_preview_by_id(
        &self,
        project_id: &str,
        execution_id: &str,
    ) -> Result<Option<WeaponryThreeJsPreviewStoreRecord>, StoreError> {
        if !is_opaque_id(project_id) || !is_opaque_id(execution_id) {
            return Err(StoreError::InvalidData(
                "Three.js preview identity lookup is invalid".to_owned(),
            ));
        }
        let connection = self.lock_connection()?;
        ensure_table(&connection)?;
        ensure_preview_table(&connection)?;
        let record = connection
            .query_row(
                "SELECT record_json FROM weaponry_threejs_preview_records WHERE project_id = ?1 AND execution_id = ?2",
                params![project_id, execution_id],
                read_preview_record,
            )
            .optional()?;
        if let Some(record) = &record {
            self.verify_weaponry_threejs_preview_record(&connection, record)?;
        }
        Ok(record)
    }

    pub fn get_weaponry_threejs_preview_exact(
        &self,
        project_id: &str,
        execution_id: &str,
        program_sha256: &str,
        program_object_sha256: &str,
        worker_result_sha256: &str,
        worker_result_object_sha256: &str,
        preview_receipt_sha256: &str,
        preview_receipt_object_sha256: &str,
        preview_runtime_sha256: &str,
        preview_dependency_lock_sha256: &str,
        preview_worker_cohort_sha256: &str,
    ) -> Result<Option<WeaponryThreeJsPreviewStoreRecord>, StoreError> {
        for hash in [
            program_sha256,
            program_object_sha256,
            worker_result_sha256,
            worker_result_object_sha256,
            preview_receipt_sha256,
            preview_receipt_object_sha256,
            preview_runtime_sha256,
            preview_dependency_lock_sha256,
            preview_worker_cohort_sha256,
        ] {
            if !is_sha256(hash) {
                return Err(StoreError::InvalidData(
                    "Three.js preview exact lookup hash is invalid".to_owned(),
                ));
            }
        }
        let Some(record) = self.get_weaponry_threejs_preview_by_id(project_id, execution_id)?
        else {
            return Ok(None);
        };
        if record.program_sha256 == program_sha256
            && record.program_object_sha256 == program_object_sha256
            && record.worker_result_sha256 == worker_result_sha256
            && record.worker_result_object_sha256 == worker_result_object_sha256
            && record.preview_receipt_sha256 == preview_receipt_sha256
            && record.preview_receipt_object_sha256 == preview_receipt_object_sha256
            && record.preview_runtime_sha256 == preview_runtime_sha256
            && record.preview_dependency_lock_sha256 == preview_dependency_lock_sha256
            && record.preview_worker_cohort_sha256 == preview_worker_cohort_sha256
        {
            Ok(Some(record))
        } else {
            Err(contract(
                "WEAPONRY_THREEJS_PREVIEW_EXACT_BINDING_MISMATCH",
                "preview exact lookup hashes differ from the durable record",
            ))
        }
    }

    pub fn read_weaponry_threejs_preview_receipt_json(
        &self,
        record: &WeaponryThreeJsPreviewStoreRecord,
    ) -> Result<Value, StoreError> {
        let connection = self.lock_connection()?;
        ensure_table(&connection)?;
        ensure_preview_table(&connection)?;
        validate_preview_record(record)?;
        let worker_bytes = self
            .cas
            .read_verified_bounded(
                &record.worker_result_object_sha256,
                WEAPONRY_THREEJS_MAX_WORKER_RESULT_BYTES,
            )
            .map_err(StoreError::from)?;
        let worker_result = validate_preview_worker_result(record, &worker_bytes)?;
        let receipt_bytes = self
            .cas
            .read_verified_bounded(
                &record.preview_receipt_object_sha256,
                WEAPONRY_THREEJS_MAX_WORKER_RESULT_BYTES,
            )
            .map_err(StoreError::from)?;
        let receipt = validate_preview_receipt(record, &receipt_bytes, &worker_result)?;
        validate_preview_aov_objects(self, &connection, &receipt, &[])?;
        Ok(receipt)
    }

    pub fn read_weaponry_threejs_preview_worker_result_json(
        &self,
        record: &WeaponryThreeJsPreviewStoreRecord,
    ) -> Result<Value, StoreError> {
        validate_preview_record(record)?;
        let bytes = self
            .cas
            .read_verified_bounded(
                &record.worker_result_object_sha256,
                WEAPONRY_THREEJS_MAX_WORKER_RESULT_BYTES,
            )
            .map_err(StoreError::from)?;
        validate_preview_worker_result(record, &bytes)
    }

    pub fn weaponry_threejs_preview_cas_roots(
        &self,
        record: &WeaponryThreeJsPreviewStoreRecord,
    ) -> Result<Vec<String>, StoreError> {
        validate_preview_record(record)?;
        let connection = self.lock_connection()?;
        ensure_table(&connection)?;
        ensure_preview_table(&connection)?;
        let transaction = connection.unchecked_transaction()?;
        let roots = preview_record_roots(&transaction, record)?;
        transaction.rollback()?;
        Ok(roots)
    }
}
