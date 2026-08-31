//! Store-local durable index for the Runtime-owned Hero UV source slice.
//!
//! Runtime writes the two immutable Hero UV JSON payloads to CAS before it
//! calls this boundary. This module verifies those registered objects, binds
//! them to the candidate in one SQLite transaction, and promotes every CAS
//! root together with the row. The public types mirror the narrow Runtime
//! persistence adapter contract; Store cannot implement the Runtime crate's
//! trait directly because Runtime already depends on Store.

use forgecad_contracts::{
    is_opaque_id, is_sha256, LOW_QUAD_DRAFT_DURABLE_ARTIFACT_KIND,
    LOW_QUAD_DRAFT_DURABLE_ARTIFACT_READBACK_SCHEMA_VERSION, LOW_QUAD_DRAFT_DURABLE_READBACK_KIND,
    PRODUCTION_WEAPON_LOW_ARTIFACT_KIND, PRODUCTION_WEAPON_LOW_ARTIFACT_RECEIPT_KIND,
};
use forgecad_core::{canonical_json_bytes, canonical_json_hash, sha256_hex};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Store, StoreError};

pub const HERO_UV_DURABLE_RECORD_SCHEMA: &str = "HeroUvDurableRecord@1";
pub const HERO_UV_DURABLE_LINK_SCHEMA: &str = "HeroUvDurableLink@1";
pub const HERO_UV_LAYOUT_SCHEMA: &str = "HeroUvLayout@1";
pub const HERO_UV_LAYOUT_CAS_KIND: &str = "production-weapon-hero-uv-layout";
pub const HERO_UV_LINK_CAS_KIND: &str = "production-weapon-hero-uv-durable-link";
pub const LOW_ARTIFACT_CAS_KIND: &str = LOW_QUAD_DRAFT_DURABLE_ARTIFACT_KIND;
pub const LOW_READBACK_CAS_KIND: &str = LOW_QUAD_DRAFT_DURABLE_READBACK_KIND;
pub const RETOPOLOGY_LOW_ARTIFACT_CAS_KIND: &str = PRODUCTION_WEAPON_LOW_ARTIFACT_KIND;
pub const RETOPOLOGY_LOW_READBACK_CAS_KIND: &str = PRODUCTION_WEAPON_LOW_ARTIFACT_RECEIPT_KIND;
pub const RETOPOLOGY_LOW_READBACK_SCHEMA: &str = "ProductionWeaponLowArtifactReadback@1";
pub const HERO_UV_MATERIALIZATION_STATUS: &str = "runtime-owned-durable-hero-uv-source-only@1";
pub const JSON_MIME: &str = "application/json";
pub const GLB_MIME: &str = "model/gltf-binary";
pub const MAX_GLB_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_JSON_BYTES: u64 = 8 * 1024 * 1024;

const TABLE: &str = "hero_uv_durable_links";
const HERO_UV_LINK_POLICY: &str = "low-artifact-to-hero-uv-layout-diagnostic@1";
const HERO_UV_IDEMPOTENCY_POLICY: &str = "same-input-hash-replays-without-new-record@1";
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-canonical-sha256@1";
const HERO_UV_LINK_FIELDS: &[&str] = &[
    "schema_version",
    "link_id",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "base_version_id",
    "source_low_artifact_id",
    "source_low_artifact_object_sha256",
    "source_low_artifact_sha256",
    "source_low_artifact_readback_object_sha256",
    "source_low_artifact_readback_sha256",
    "resolution",
    "padding_texels",
    "min_mip_level",
    "hard_edge_angle_deg",
    "stretch_threshold",
    "visibility_weights_sha256",
    "layout_object_sha256",
    "layout_canonical_sha256",
    "worker_build_cohort_sha256",
    "request_sha256",
    "input_sha256",
    "idempotency_key",
    "replay_count",
    "replay_byte_exact",
    "link_policy",
    "writer_policy",
    "materialization_status",
    "idempotency_policy",
    "source_only",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "production_stage_advanced",
    "candidate_confirmed",
    "version_created",
    "export_performed",
    "quality_status",
    "visual_status",
    "human_status",
    "engine_status",
    "distribution_status",
    "canonicalization_policy",
    "canonical_sha256",
    "created_at",
];

/// Store-side mirror of the Runtime persistence contract. Runtime currently
/// owns an identically shaped trait, but cannot be referenced from this crate
/// without creating a Store -> Runtime dependency cycle.
pub trait HeroUvDurablePersistence {
    fn get_hero_uv(
        &self,
        project_id: &str,
        idempotency_key: &str,
    ) -> Result<Option<HeroUvDurableRecord>, String>;

    fn commit_hero_uv(
        &self,
        record: &HeroUvDurableRecord,
        layout: &HeroUvDurableCasPayload,
        link: &HeroUvDurableCasPayload,
    ) -> Result<(HeroUvDurableRecord, bool), String>;

    fn read_hero_uv_bundle(
        &self,
        record: &HeroUvDurableRecord,
    ) -> Result<HeroUvDurableReadback, String>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HeroUvDurableRecord {
    pub schema_version: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub base_version_id: Option<String>,
    pub source_low_artifact_id: String,
    pub source_low_artifact_object_sha256: String,
    pub source_low_artifact_sha256: String,
    pub source_low_artifact_readback_object_sha256: String,
    pub source_low_artifact_readback_sha256: String,
    pub resolution: u64,
    pub padding_texels: u64,
    pub min_mip_level: u64,
    pub hard_edge_angle_deg: f64,
    pub stretch_threshold: f64,
    pub visibility_weights_sha256: String,
    pub layout_object_sha256: String,
    pub layout_canonical_sha256: String,
    pub worker_build_cohort_sha256: String,
    pub link_id: String,
    pub link_object_sha256: String,
    pub request_sha256: String,
    pub input_sha256: String,
    pub idempotency_key: String,
    pub materialization_status: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeroUvDurableCasPayload {
    pub bytes: Vec<u8>,
    pub object_sha256: String,
    pub mime: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeroUvDurableReadback {
    pub layout_bytes: Vec<u8>,
    pub link_bytes: Vec<u8>,
}

fn contract(code: &str, message: impl Into<String>) -> StoreError {
    StoreError::Contract {
        code: code.to_owned(),
        message: message.into(),
    }
}

pub(crate) fn ensure_table(connection: &rusqlite::Connection) -> Result<(), StoreError> {
    connection.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {TABLE} (
            schema_version TEXT NOT NULL CHECK (schema_version = 'HeroUvDurableRecord@1'),
            project_id TEXT NOT NULL,
            candidate_id TEXT NOT NULL,
            candidate_state_sha256 TEXT NOT NULL,
            base_version_id TEXT,
            source_low_artifact_id TEXT NOT NULL,
            source_low_artifact_object_sha256 TEXT NOT NULL,
            source_low_artifact_sha256 TEXT NOT NULL,
            source_low_artifact_readback_object_sha256 TEXT NOT NULL,
            source_low_artifact_readback_sha256 TEXT NOT NULL,
            resolution INTEGER NOT NULL,
            padding_texels INTEGER NOT NULL,
            min_mip_level INTEGER NOT NULL,
            hard_edge_angle_deg REAL NOT NULL,
            stretch_threshold REAL NOT NULL,
            visibility_weights_sha256 TEXT NOT NULL,
            layout_object_sha256 TEXT NOT NULL,
            layout_canonical_sha256 TEXT NOT NULL,
            worker_build_cohort_sha256 TEXT NOT NULL,
            link_id TEXT NOT NULL UNIQUE,
            link_object_sha256 TEXT NOT NULL,
            request_sha256 TEXT NOT NULL,
            input_sha256 TEXT NOT NULL,
            idempotency_key TEXT NOT NULL,
            materialization_status TEXT NOT NULL,
            canonical_sha256 TEXT NOT NULL,
            created_at TEXT NOT NULL,
            record_json TEXT NOT NULL,
            PRIMARY KEY (project_id, idempotency_key),
            UNIQUE (candidate_id, layout_object_sha256)
        );
        CREATE INDEX IF NOT EXISTS hero_uv_durable_candidate_idx
            ON {TABLE}(candidate_id, created_at DESC, link_id ASC);
        CREATE INDEX IF NOT EXISTS hero_uv_durable_object_idx
            ON {TABLE}(source_low_artifact_object_sha256,
                       source_low_artifact_readback_object_sha256,
                       layout_object_sha256,
                       link_object_sha256);"
    ))?;
    Ok(())
}

fn record_value(record: &HeroUvDurableRecord) -> Result<Value, StoreError> {
    serde_json::to_value(record).map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn record_bytes(record: &HeroUvDurableRecord) -> Result<Vec<u8>, StoreError> {
    canonical_json_bytes(&record_value(record)?)
        .map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn validate_config(record: &HeroUvDurableRecord) -> Result<(), StoreError> {
    if !matches!(record.resolution, 2048 | 4096)
        || !(1..=128).contains(&record.padding_texels)
        || record.min_mip_level > 12
        || !record.hard_edge_angle_deg.is_finite()
        || record.hard_edge_angle_deg <= 0.1
        || record.hard_edge_angle_deg >= 89.9
        || !record.stretch_threshold.is_finite()
        || !(1.0..=100.0).contains(&record.stretch_threshold)
    {
        return Err(contract(
            "HERO_UV_DURABLE_RECORD_INVALID",
            "Hero UV configuration is outside its bounded contract",
        ));
    }
    Ok(())
}

fn validate_record_shape(record: &HeroUvDurableRecord) -> Result<(), StoreError> {
    let identifiers = [
        record.project_id.as_str(),
        record.candidate_id.as_str(),
        record.source_low_artifact_id.as_str(),
        record.link_id.as_str(),
        record.idempotency_key.as_str(),
    ];
    let hashes = [
        record.candidate_state_sha256.as_str(),
        record.source_low_artifact_object_sha256.as_str(),
        record.source_low_artifact_sha256.as_str(),
        record.source_low_artifact_readback_object_sha256.as_str(),
        record.source_low_artifact_readback_sha256.as_str(),
        record.visibility_weights_sha256.as_str(),
        record.layout_object_sha256.as_str(),
        record.layout_canonical_sha256.as_str(),
        record.worker_build_cohort_sha256.as_str(),
        record.link_object_sha256.as_str(),
        record.request_sha256.as_str(),
        record.input_sha256.as_str(),
        record.canonical_sha256.as_str(),
    ];
    if record.schema_version != HERO_UV_DURABLE_RECORD_SCHEMA
        || identifiers.iter().any(|value| !is_opaque_id(value))
        || hashes.iter().any(|value| !is_sha256(value))
        || record
            .base_version_id
            .as_deref()
            .is_some_and(|value| !is_opaque_id(value))
        || record.materialization_status != HERO_UV_MATERIALIZATION_STATUS
        || record.created_at.is_empty()
        || record.created_at.len() > 128
    {
        return Err(contract(
            "HERO_UV_DURABLE_RECORD_INVALID",
            "Hero UV durable identity, hash, status or timestamp is malformed",
        ));
    }
    validate_config(record)?;
    let mut value = record_value(record)?;
    value["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&value) != record.canonical_sha256 {
        return Err(contract(
            "HERO_UV_DURABLE_RECORD_CANONICAL_MISMATCH",
            "Hero UV durable record canonical hash differs",
        ));
    }
    Ok(())
}

fn read_object(
    store: &Store,
    expected_sha256: &str,
    expected_mime: &str,
    expected_kind: &str,
    max_bytes: u64,
) -> Result<Vec<u8>, StoreError> {
    let object = store.get_object(expected_sha256)?.ok_or_else(|| {
        contract(
            "HERO_UV_DURABLE_CAS_MISSING",
            "Hero UV durable CAS object is missing",
        )
    })?;
    if object.schema_version != "CasObject@1"
        || object.sha256 != expected_sha256
        || !is_sha256(expected_sha256)
        || object.mime != expected_mime
        || object.kind != expected_kind
        || object.size_bytes == 0
        || object.size_bytes > max_bytes
        || object.size_bytes > i64::MAX as u64
        || !matches!(object.reachability.as_str(), "temporary" | "reachable")
    {
        return Err(contract(
            "HERO_UV_DURABLE_CAS_METADATA_MISMATCH",
            "Hero UV durable CAS metadata differs from its binding",
        ));
    }
    let bytes = store
        .cas
        .read_verified_bounded(expected_sha256, max_bytes)
        .map_err(StoreError::from)?;
    if bytes.len() as u64 != object.size_bytes || sha256_hex(&bytes) != expected_sha256 {
        return Err(contract(
            "HERO_UV_DURABLE_CAS_HASH_MISMATCH",
            "Hero UV durable CAS bytes do not match their content hash",
        ));
    }
    Ok(bytes)
}

fn validate_candidate(store: &Store, record: &HeroUvDurableRecord) -> Result<(), StoreError> {
    let candidate = store.get_candidate(&record.candidate_id)?.ok_or_else(|| {
        contract(
            "HERO_UV_DURABLE_CANDIDATE_UNAVAILABLE",
            "Hero UV candidate is missing",
        )
    })?;
    if candidate.project_id != record.project_id
        || candidate.canonical_sha256 != record.candidate_state_sha256
        || candidate.base_version_id != record.base_version_id
    {
        return Err(contract(
            "HERO_UV_DURABLE_CANDIDATE_BINDING_MISMATCH",
            "Hero UV candidate project/state/base-version binding differs",
        ));
    }
    Ok(())
}

fn validate_candidate_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    record: &HeroUvDurableRecord,
) -> Result<(), StoreError> {
    let candidate: Option<(String, String, Option<String>)> = transaction
        .query_row(
            "SELECT project_id, canonical_sha256, base_version_id FROM candidates WHERE candidate_id = ?1",
            params![record.candidate_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let Some((project_id, candidate_state_sha256, base_version_id)) = candidate else {
        return Err(contract(
            "HERO_UV_DURABLE_CANDIDATE_UNAVAILABLE",
            "Hero UV candidate is missing",
        ));
    };
    if project_id != record.project_id
        || candidate_state_sha256 != record.candidate_state_sha256
        || base_version_id != record.base_version_id
    {
        return Err(contract(
            "HERO_UV_DURABLE_CANDIDATE_BINDING_MISMATCH",
            "Hero UV candidate project/state/base-version changed before commit",
        ));
    }
    Ok(())
}

fn validate_source_readback(
    record: &HeroUvDurableRecord,
    bytes: &[u8],
    retopology_low: bool,
) -> Result<(), StoreError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        contract(
            "HERO_UV_DURABLE_SOURCE_READBACK_INVALID",
            format!("Low source readback JSON is invalid: {error}"),
        )
    })?;
    let valid_binding = if retopology_low {
        value.get("schema_version").and_then(Value::as_str) == Some(RETOPOLOGY_LOW_READBACK_SCHEMA)
            && value.get("artifact_sha256").and_then(Value::as_str)
                == Some(record.source_low_artifact_sha256.as_str())
            && value
                .get("worker_readback")
                .and_then(Value::as_object)
                .is_some_and(|worker| {
                    worker.get("glb_parse_status").and_then(Value::as_str) == Some("passed")
                        && worker
                            .get("failure_codes")
                            .and_then(Value::as_array)
                            .is_some_and(Vec::is_empty)
                        && worker.get("part_coverage").and_then(Value::as_f64) == Some(1.0)
                        && worker.get("material_zone_coverage").and_then(Value::as_f64) == Some(1.0)
                        && worker.get("source_coverage").and_then(Value::as_f64) == Some(1.0)
                        && [
                            "boundary_edge_count",
                            "degenerate_triangle_count",
                            "invalid_index_count",
                            "metadata_mismatch_count",
                            "non_finite_count",
                            "non_manifold_edge_count",
                            "tangent_handedness_error_count",
                            "tangent_non_finite_count",
                            "tangent_orthogonality_error_count",
                            "uv_non_finite_count",
                            "winding_error_count",
                            "zero_area_uv_triangle_count",
                        ]
                        .iter()
                        .all(|field| worker.get(*field).and_then(Value::as_u64) == Some(0))
                })
    } else {
        value.get("schema_version").and_then(Value::as_str)
            == Some(LOW_QUAD_DRAFT_DURABLE_ARTIFACT_READBACK_SCHEMA_VERSION)
            && value.get("artifact_sha256").and_then(Value::as_str)
                == Some(record.source_low_artifact_sha256.as_str())
            && value.get("artifact_object_sha256").and_then(Value::as_str)
                == Some(record.source_low_artifact_object_sha256.as_str())
            && value.get("validator_status").and_then(Value::as_str) == Some("passed")
            && value.get("hard_gate_passed") == Some(&Value::Bool(true))
            && value.get("quality_status").and_then(Value::as_str) == Some("structural_only")
            && value.get("edge_flow_status").and_then(Value::as_str) == Some("DRAFT_UNREVIEWED")
            && value.get("promotion_eligible") == Some(&Value::Bool(false))
            && value.get("production_stage_advanced") == Some(&Value::Bool(false))
            && value.get("candidate_confirmed") == Some(&Value::Bool(false))
            && value.get("version_created") == Some(&Value::Bool(false))
            && value.get("export_performed") == Some(&Value::Bool(false))
    };
    if !valid_binding {
        return Err(contract(
            "HERO_UV_DURABLE_SOURCE_READBACK_BINDING_MISMATCH",
            "Low source readback artifact binding differs",
        ));
    }
    let canonical = value
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            contract(
                "HERO_UV_DURABLE_SOURCE_READBACK_INVALID",
                "Low source readback canonical hash is missing",
            )
        })?;
    if !is_sha256(canonical) {
        return Err(contract(
            "HERO_UV_DURABLE_SOURCE_READBACK_INVALID",
            "Low source readback canonical hash is malformed",
        ));
    }
    let mut preimage = value.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != canonical
        || canonical != record.source_low_artifact_readback_sha256
        || canonical_json_bytes(&value)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?
            != bytes
    {
        return Err(contract(
            "HERO_UV_DURABLE_SOURCE_READBACK_CANONICAL_MISMATCH",
            "Low source readback canonical bytes differ",
        ));
    }
    Ok(())
}

fn validate_source_roots(store: &Store, record: &HeroUvDurableRecord) -> Result<(), StoreError> {
    let source_object = store
        .get_object(&record.source_low_artifact_object_sha256)?
        .ok_or_else(|| {
            contract(
                "HERO_UV_DURABLE_CAS_MISSING",
                "Hero UV durable CAS object is missing",
            )
        })?;
    let (artifact_kind, readback_kind, retopology_low) =
        if source_object.kind == RETOPOLOGY_LOW_ARTIFACT_CAS_KIND {
            (
                RETOPOLOGY_LOW_ARTIFACT_CAS_KIND,
                RETOPOLOGY_LOW_READBACK_CAS_KIND,
                true,
            )
        } else if source_object.kind == LOW_ARTIFACT_CAS_KIND {
            (LOW_ARTIFACT_CAS_KIND, LOW_READBACK_CAS_KIND, false)
        } else {
            return Err(contract(
                "HERO_UV_DURABLE_CAS_METADATA_MISMATCH",
                "Hero UV Low artifact kind is not a registered source kind",
            ));
        };
    let source = read_object(
        store,
        &record.source_low_artifact_object_sha256,
        GLB_MIME,
        artifact_kind,
        MAX_GLB_BYTES,
    )?;
    if sha256_hex(&source) != record.source_low_artifact_sha256 {
        return Err(contract(
            "HERO_UV_DURABLE_SOURCE_ARTIFACT_HASH_MISMATCH",
            "Low source artifact semantic hash differs from its CAS bytes",
        ));
    }
    let readback = read_object(
        store,
        &record.source_low_artifact_readback_object_sha256,
        JSON_MIME,
        readback_kind,
        MAX_JSON_BYTES,
    )?;
    if sha256_hex(&readback) != record.source_low_artifact_readback_object_sha256 {
        return Err(contract(
            "HERO_UV_DURABLE_SOURCE_READBACK_HASH_MISMATCH",
            "Low source readback bytes differ from their CAS object hash",
        ));
    }
    validate_source_readback(record, &readback, retopology_low)
}

fn validate_supplied_payload(
    store: &Store,
    payload: &HeroUvDurableCasPayload,
    expected_sha256: &str,
    expected_kind: &str,
) -> Result<Vec<u8>, StoreError> {
    if payload.object_sha256 != expected_sha256
        || payload.mime != JSON_MIME
        || payload.kind != expected_kind
        || payload.bytes.is_empty()
        || payload.bytes.len() as u64 > MAX_JSON_BYTES
        || !is_sha256(expected_sha256)
        || sha256_hex(&payload.bytes) != expected_sha256
    {
        return Err(contract(
            "HERO_UV_DURABLE_CAS_PAYLOAD_MISMATCH",
            "Hero UV supplied CAS payload metadata or bytes differ",
        ));
    }
    let stored = read_object(
        store,
        expected_sha256,
        JSON_MIME,
        expected_kind,
        MAX_JSON_BYTES,
    )?;
    if stored != payload.bytes {
        return Err(contract(
            "HERO_UV_DURABLE_CAS_PAYLOAD_MISMATCH",
            "Hero UV supplied CAS bytes differ from the registered object",
        ));
    }
    Ok(stored)
}

fn validate_json_canonical(bytes: &[u8], context: &str) -> Result<Value, StoreError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        contract(
            "HERO_UV_DURABLE_CAS_JSON_INVALID",
            format!("{context} JSON is invalid: {error}"),
        )
    })?;
    let canonical = value
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            contract(
                "HERO_UV_DURABLE_CAS_JSON_INVALID",
                format!("{context} canonical hash is missing"),
            )
        })?;
    if !is_sha256(canonical) {
        return Err(contract(
            "HERO_UV_DURABLE_CAS_JSON_INVALID",
            format!("{context} canonical hash is malformed"),
        ));
    }
    let mut preimage = value.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != canonical
        || canonical_json_bytes(&value)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?
            != bytes
    {
        return Err(contract(
            "HERO_UV_DURABLE_CAS_JSON_CANONICAL_MISMATCH",
            format!("{context} is not canonical JSON"),
        ));
    }
    Ok(value)
}

fn require_string(value: &Value, field: &str, expected: &str) -> Result<(), StoreError> {
    if value.get(field).and_then(Value::as_str) != Some(expected) {
        return Err(contract(
            "HERO_UV_DURABLE_LINK_BINDING_MISMATCH",
            format!("Hero UV link {field} differs"),
        ));
    }
    Ok(())
}

fn validate_layout_and_link(
    record: &HeroUvDurableRecord,
    layout_bytes: &[u8],
    link_bytes: &[u8],
) -> Result<(), StoreError> {
    let layout = validate_json_canonical(layout_bytes, "Hero UV layout")?;
    if layout.get("schema_version").and_then(Value::as_str) != Some(HERO_UV_LAYOUT_SCHEMA)
        || layout.get("canonical_sha256").and_then(Value::as_str)
            != Some(record.layout_canonical_sha256.as_str())
    {
        return Err(contract(
            "HERO_UV_DURABLE_LAYOUT_BINDING_MISMATCH",
            "Hero UV layout schema or canonical binding differs",
        ));
    }
    let link = validate_json_canonical(link_bytes, "Hero UV link")?;
    let Some(link_object) = link.as_object() else {
        return Err(contract(
            "HERO_UV_DURABLE_LINK_SCHEMA_MISMATCH",
            "Hero UV link is not a JSON object",
        ));
    };
    if link_object.len() != HERO_UV_LINK_FIELDS.len()
        || HERO_UV_LINK_FIELDS
            .iter()
            .any(|field| !link_object.contains_key(*field))
    {
        return Err(contract(
            "HERO_UV_DURABLE_LINK_SCHEMA_MISMATCH",
            "Hero UV link field set differs from HeroUvDurableLink@1",
        ));
    }
    if link.get("schema_version").and_then(Value::as_str) != Some(HERO_UV_DURABLE_LINK_SCHEMA)
        || link.get("link_id").and_then(Value::as_str) != Some(record.link_id.as_str())
    {
        return Err(contract(
            "HERO_UV_DURABLE_LINK_BINDING_MISMATCH",
            "Hero UV link schema or identity differs",
        ));
    }
    for (field, expected) in [
        ("project_id", record.project_id.as_str()),
        ("candidate_id", record.candidate_id.as_str()),
        (
            "candidate_state_sha256",
            record.candidate_state_sha256.as_str(),
        ),
        (
            "source_low_artifact_id",
            record.source_low_artifact_id.as_str(),
        ),
        (
            "source_low_artifact_object_sha256",
            record.source_low_artifact_object_sha256.as_str(),
        ),
        (
            "source_low_artifact_sha256",
            record.source_low_artifact_sha256.as_str(),
        ),
        (
            "source_low_artifact_readback_object_sha256",
            record.source_low_artifact_readback_object_sha256.as_str(),
        ),
        (
            "source_low_artifact_readback_sha256",
            record.source_low_artifact_readback_sha256.as_str(),
        ),
        (
            "visibility_weights_sha256",
            record.visibility_weights_sha256.as_str(),
        ),
        ("layout_object_sha256", record.layout_object_sha256.as_str()),
        (
            "layout_canonical_sha256",
            record.layout_canonical_sha256.as_str(),
        ),
        (
            "worker_build_cohort_sha256",
            record.worker_build_cohort_sha256.as_str(),
        ),
        ("request_sha256", record.request_sha256.as_str()),
        ("input_sha256", record.input_sha256.as_str()),
        ("idempotency_key", record.idempotency_key.as_str()),
        ("link_policy", HERO_UV_LINK_POLICY),
        ("writer_policy", WRITER_POLICY),
        ("materialization_status", HERO_UV_MATERIALIZATION_STATUS),
        ("idempotency_policy", HERO_UV_IDEMPOTENCY_POLICY),
        ("quality_status", "structural_only"),
        ("visual_status", "NOT_PROVEN"),
        ("human_status", "NOT_RUN"),
        ("engine_status", "NOT_RUN"),
        ("distribution_status", "NOT_RUN"),
        ("canonicalization_policy", CANONICALIZATION_POLICY),
        ("created_at", record.created_at.as_str()),
    ] {
        require_string(&link, field, expected)?;
    }
    if link.get("resolution").and_then(Value::as_u64) != Some(record.resolution)
        || link.get("padding_texels").and_then(Value::as_u64) != Some(record.padding_texels)
        || link.get("min_mip_level").and_then(Value::as_u64) != Some(record.min_mip_level)
        || link.get("hard_edge_angle_deg").and_then(Value::as_f64)
            != Some(record.hard_edge_angle_deg)
        || link.get("stretch_threshold").and_then(Value::as_f64) != Some(record.stretch_threshold)
        || link.get("replay_count").and_then(Value::as_u64) != Some(2)
        || link.get("replay_byte_exact") != Some(&Value::Bool(true))
    {
        return Err(contract(
            "HERO_UV_DURABLE_LINK_BINDING_MISMATCH",
            "Hero UV link numeric configuration differs",
        ));
    }
    let expected_base_version_id = record
        .base_version_id
        .as_ref()
        .map(|value| Value::String(value.clone()))
        .unwrap_or(Value::Null);
    if link.get("base_version_id") != Some(&expected_base_version_id)
        || link.get("materialization_status").and_then(Value::as_str)
            != Some(HERO_UV_MATERIALIZATION_STATUS)
        || link.get("source_only") != Some(&Value::Bool(true))
        || link.get("runtime_write_performed") != Some(&Value::Bool(true))
        || link.get("persistent_user_data_touched") != Some(&Value::Bool(true))
        || link.get("production_stage_advanced") != Some(&Value::Bool(false))
        || link.get("candidate_confirmed") != Some(&Value::Bool(false))
        || link.get("version_created") != Some(&Value::Bool(false))
        || link.get("export_performed") != Some(&Value::Bool(false))
    {
        return Err(contract(
            "HERO_UV_DURABLE_LINK_POLICY_MISMATCH",
            "Hero UV link materialization or promotion policy differs",
        ));
    }
    Ok(())
}

fn read_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<HeroUvDurableRecord> {
    let payload: String = row.get(0)?;
    serde_json::from_str(&payload).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn validate_committed_bundle(
    store: &Store,
    record: &HeroUvDurableRecord,
) -> Result<HeroUvDurableReadback, StoreError> {
    validate_record_shape(record)?;
    validate_candidate(store, record)?;
    validate_source_roots(store, record)?;
    let layout_bytes = read_object(
        store,
        &record.layout_object_sha256,
        JSON_MIME,
        HERO_UV_LAYOUT_CAS_KIND,
        MAX_JSON_BYTES,
    )?;
    let link_bytes = read_object(
        store,
        &record.link_object_sha256,
        JSON_MIME,
        HERO_UV_LINK_CAS_KIND,
        MAX_JSON_BYTES,
    )?;
    validate_layout_and_link(record, &layout_bytes, &link_bytes)?;
    Ok(HeroUvDurableReadback {
        layout_bytes,
        link_bytes,
    })
}

impl Store {
    /// Atomically bind the pre-registered source, layout and link objects to a
    /// candidate. Replay is exact on (project_id, idempotency_key); every
    /// replay also re-promotes all four CAS roots to close the GC race.
    pub fn commit_hero_uv(
        &self,
        record: &HeroUvDurableRecord,
        layout: &HeroUvDurableCasPayload,
        link: &HeroUvDurableCasPayload,
    ) -> Result<(HeroUvDurableRecord, bool), StoreError> {
        validate_record_shape(record)?;
        validate_candidate(self, record)?;
        validate_source_roots(self, record)?;
        let layout_bytes = validate_supplied_payload(
            self,
            layout,
            &record.layout_object_sha256,
            HERO_UV_LAYOUT_CAS_KIND,
        )?;
        let link_bytes = validate_supplied_payload(
            self,
            link,
            &record.link_object_sha256,
            HERO_UV_LINK_CAS_KIND,
        )?;
        let payload_json = String::from_utf8(record_bytes(record)?).map_err(|error| {
            StoreError::InvalidData(format!("Hero UV durable record JSON is not UTF-8: {error}"))
        })?;

        let mut connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let transaction = connection.transaction()?;
        validate_candidate_in_transaction(&transaction, record)?;
        let existing = transaction
            .query_row(
                &format!(
                    "SELECT record_json FROM {TABLE} WHERE project_id = ?1 AND idempotency_key = ?2"
                ),
                params![record.project_id, record.idempotency_key],
                read_record,
            )
            .optional()?;
        let reachable = vec![
            record.source_low_artifact_object_sha256.clone(),
            record.source_low_artifact_readback_object_sha256.clone(),
            record.layout_object_sha256.clone(),
            record.link_object_sha256.clone(),
        ];
        if let Some(existing) = existing {
            validate_record_shape(&existing)?;
            if existing.input_sha256 != record.input_sha256 {
                return Err(contract(
                    "HERO_UV_DURABLE_RECORD_CONFLICT",
                    "project and idempotency key are already bound to different Hero UV metadata",
                ));
            }
            super::mark_reachable_in_transaction(&transaction, &reachable)?;
            transaction.commit()?;
            return Ok((existing, true));
        }
        validate_layout_and_link(record, &layout_bytes, &link_bytes)?;
        let key_conflict: Option<String> = transaction
            .query_row(
                &format!(
                    "SELECT link_id FROM {TABLE} WHERE link_id = ?1 OR (candidate_id = ?2 AND layout_object_sha256 = ?3)"
                ),
                params![record.link_id, record.candidate_id, record.layout_object_sha256],
                |row| row.get(0),
            )
            .optional()?;
        if key_conflict.is_some() {
            return Err(contract(
                "HERO_UV_DURABLE_RECORD_CONFLICT",
                "link or candidate/layout identity is already bound",
            ));
        }
        transaction.execute(
            &format!(
                "INSERT INTO {TABLE} (schema_version, project_id, candidate_id, candidate_state_sha256, base_version_id, source_low_artifact_id, source_low_artifact_object_sha256, source_low_artifact_sha256, source_low_artifact_readback_object_sha256, source_low_artifact_readback_sha256, resolution, padding_texels, min_mip_level, hard_edge_angle_deg, stretch_threshold, visibility_weights_sha256, layout_object_sha256, layout_canonical_sha256, worker_build_cohort_sha256, link_id, link_object_sha256, request_sha256, input_sha256, idempotency_key, materialization_status, canonical_sha256, created_at, record_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28)"
            ),
            params![
                record.schema_version,
                record.project_id,
                record.candidate_id,
                record.candidate_state_sha256,
                record.base_version_id,
                record.source_low_artifact_id,
                record.source_low_artifact_object_sha256,
                record.source_low_artifact_sha256,
                record.source_low_artifact_readback_object_sha256,
                record.source_low_artifact_readback_sha256,
                i64::try_from(record.resolution).map_err(|_| {
                    StoreError::InvalidData("Hero UV resolution is too large".to_owned())
                })?,
                i64::try_from(record.padding_texels).map_err(|_| {
                    StoreError::InvalidData("Hero UV padding is too large".to_owned())
                })?,
                i64::try_from(record.min_mip_level).map_err(|_| {
                    StoreError::InvalidData("Hero UV mip level is too large".to_owned())
                })?,
                record.hard_edge_angle_deg,
                record.stretch_threshold,
                record.visibility_weights_sha256,
                record.layout_object_sha256,
                record.layout_canonical_sha256,
                record.worker_build_cohort_sha256,
                record.link_id,
                record.link_object_sha256,
                record.request_sha256,
                record.input_sha256,
                record.idempotency_key,
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

    pub fn get_hero_uv(
        &self,
        project_id: &str,
        idempotency_key: &str,
    ) -> Result<Option<HeroUvDurableRecord>, StoreError> {
        if !is_opaque_id(project_id) || !is_opaque_id(idempotency_key) {
            return Err(StoreError::InvalidData(
                "Hero UV durable lookup identity is invalid".to_owned(),
            ));
        }
        let connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let record = connection
            .query_row(
                &format!(
                    "SELECT record_json FROM {TABLE} WHERE project_id = ?1 AND idempotency_key = ?2"
                ),
                params![project_id, idempotency_key],
                read_record,
            )
            .optional()?;
        drop(connection);
        let Some(record) = record else {
            return Ok(None);
        };
        if record.project_id != project_id || record.idempotency_key != idempotency_key {
            return Err(contract(
                "HERO_UV_DURABLE_RECORD_SCOPE_MISMATCH",
                "stored Hero UV record scope differs",
            ));
        }
        validate_committed_bundle(self, &record)?;
        Ok(Some(record))
    }

    pub fn get_hero_uv_by_link_id(
        &self,
        link_id: &str,
    ) -> Result<Option<HeroUvDurableRecord>, StoreError> {
        if !is_opaque_id(link_id) {
            return Err(StoreError::InvalidData(
                "Hero UV durable link identity is invalid".to_owned(),
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
        self.get_hero_uv(&project_id, &idempotency_key)
    }

    /// Resolve the only durable Hero UV record for an exact project,
    /// candidate and Low-artifact semantic hash.  Formal High/Low/Cage/Bake
    /// preparation does not accept a Hero UV idempotency key from MCP, so the
    /// Runtime must use this closed reverse index and reject ambiguity rather
    /// than trusting caller-supplied replacement JSON.
    pub fn get_hero_uv_by_candidate_source_artifact(
        &self,
        project_id: &str,
        candidate_id: &str,
        source_low_artifact_sha256: &str,
    ) -> Result<Option<HeroUvDurableRecord>, StoreError> {
        if !is_opaque_id(project_id)
            || !is_opaque_id(candidate_id)
            || !is_sha256(source_low_artifact_sha256)
        {
            return Err(StoreError::InvalidData(
                "Hero UV candidate/artifact lookup identity is invalid".to_owned(),
            ));
        }
        let connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let identities = {
            let mut statement = connection.prepare(&format!(
                "SELECT project_id, idempotency_key FROM {TABLE} WHERE project_id = ?1 AND candidate_id = ?2 AND source_low_artifact_sha256 = ?3 ORDER BY created_at DESC, link_id ASC LIMIT 2"
            ))?;
            let rows = statement
                .query_map(
                    params![project_id, candidate_id, source_low_artifact_sha256],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };
        drop(connection);
        if identities.len() > 1 {
            return Err(contract(
                "HERO_UV_DURABLE_CANDIDATE_ARTIFACT_AMBIGUOUS",
                "candidate and Low artifact have multiple durable Hero UV records",
            ));
        }
        let Some((project_id, idempotency_key)) = identities.into_iter().next() else {
            return Ok(None);
        };
        self.get_hero_uv(&project_id, &idempotency_key)
    }

    pub fn read_hero_uv_bundle(
        &self,
        record: &HeroUvDurableRecord,
    ) -> Result<HeroUvDurableReadback, StoreError> {
        validate_record_shape(record)?;
        let connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let stored = connection
            .query_row(
                &format!(
                    "SELECT record_json FROM {TABLE} WHERE project_id = ?1 AND idempotency_key = ?2"
                ),
                params![record.project_id, record.idempotency_key],
                read_record,
            )
            .optional()?;
        drop(connection);
        let Some(stored) = stored else {
            return Err(contract(
                "HERO_UV_DURABLE_RECORD_UNAVAILABLE",
                "Hero UV durable record is unavailable",
            ));
        };
        if stored.input_sha256 != record.input_sha256 {
            return Err(contract(
                "HERO_UV_DURABLE_RECORD_CONFLICT",
                "Hero UV readback record differs from the stored idempotency row",
            ));
        }
        validate_committed_bundle(self, &stored)
    }
}

impl HeroUvDurablePersistence for Store {
    fn get_hero_uv(
        &self,
        project_id: &str,
        idempotency_key: &str,
    ) -> Result<Option<HeroUvDurableRecord>, String> {
        Store::get_hero_uv(self, project_id, idempotency_key).map_err(|error| error.to_string())
    }

    fn commit_hero_uv(
        &self,
        record: &HeroUvDurableRecord,
        layout: &HeroUvDurableCasPayload,
        link: &HeroUvDurableCasPayload,
    ) -> Result<(HeroUvDurableRecord, bool), String> {
        Store::commit_hero_uv(self, record, layout, link).map_err(|error| error.to_string())
    }

    fn read_hero_uv_bundle(
        &self,
        record: &HeroUvDurableRecord,
    ) -> Result<HeroUvDurableReadback, String> {
        Store::read_hero_uv_bundle(self, record).map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgecad_contracts::{CandidateRecord, ProjectRecord};
    use rusqlite::params;
    use serde_json::json;

    fn install_candidate(store: &Store) -> (String, String, String) {
        let project_id = "project-hero-uv".to_owned();
        let candidate_id = "candidate-hero-uv".to_owned();
        let candidate_state_sha256 = "a".repeat(64);
        store
            .insert_project(&ProjectRecord {
                schema_version: "Project@1".to_owned(),
                project_id: project_id.clone(),
                name: "Hero UV test".to_owned(),
                policy: json!({}),
                created_at: "1".to_owned(),
                updated_at: "1".to_owned(),
                active_snapshot_revision: 0,
                head_snapshot_id: None,
                canonical_sha256: "b".repeat(64),
            })
            .expect("project");
        store
            .insert_candidate(&CandidateRecord {
                schema_version: "Candidate@1".to_owned(),
                candidate_id: candidate_id.clone(),
                project_id: project_id.clone(),
                base_version_id: None,
                source_version_id: None,
                prepared_object_id: None,
                prepared_object_sha256: None,
                state: "prepared".to_owned(),
                request_sha256: "c".repeat(64),
                manifest_hash: None,
                quality_report_id: None,
                quality_hard_gate_passed: false,
                canonical_sha256: candidate_state_sha256.clone(),
                error_code: None,
                created_at: "1".to_owned(),
                updated_at: "1".to_owned(),
            })
            .expect("candidate");
        (project_id, candidate_id, candidate_state_sha256)
    }

    fn canonical_payload(mut value: Value) -> (Vec<u8>, String) {
        value["canonical_sha256"] = Value::String(String::new());
        let canonical = canonical_json_hash(&value);
        value["canonical_sha256"] = Value::String(canonical.clone());
        (
            canonical_json_bytes(&value).expect("canonical bytes"),
            canonical,
        )
    }

    fn fixture(
        store: &Store,
    ) -> (
        HeroUvDurableRecord,
        HeroUvDurableCasPayload,
        HeroUvDurableCasPayload,
    ) {
        let (project_id, candidate_id, candidate_state_sha256) = install_candidate(store);
        let source = store
            .put_object(b"low-glb", None, GLB_MIME, LOW_ARTIFACT_CAS_KIND, "1")
            .expect("source");
        let source_readback_value = json!({
            "schema_version": LOW_QUAD_DRAFT_DURABLE_ARTIFACT_READBACK_SCHEMA_VERSION,
            "artifact_sha256": source.record.sha256,
            "artifact_object_sha256": source.record.sha256,
            "validator_status": "passed",
            "hard_gate_passed": true,
            "quality_status": "structural_only",
            "edge_flow_status": "DRAFT_UNREVIEWED",
            "promotion_eligible": false,
            "production_stage_advanced": false,
            "candidate_confirmed": false,
            "version_created": false,
            "export_performed": false,
            "canonical_sha256": ""
        });
        let (source_readback_bytes, source_readback_sha256) =
            canonical_payload(source_readback_value);
        let source_readback = store
            .put_object(
                &source_readback_bytes,
                None,
                JSON_MIME,
                LOW_READBACK_CAS_KIND,
                "1",
            )
            .expect("source readback");
        let (layout_bytes, layout_canonical_sha256) = canonical_payload(json!({
            "schema_version": HERO_UV_LAYOUT_SCHEMA,
            "canonical_sha256": ""
        }));
        let layout = store
            .put_object(&layout_bytes, None, JSON_MIME, HERO_UV_LAYOUT_CAS_KIND, "1")
            .expect("layout");
        let mut link_map = serde_json::Map::new();
        link_map.insert(
            "schema_version".to_owned(),
            json!(HERO_UV_DURABLE_LINK_SCHEMA),
        );
        link_map.insert("link_id".to_owned(), json!("hero-uv-link"));
        link_map.insert("project_id".to_owned(), json!(project_id));
        link_map.insert("candidate_id".to_owned(), json!(candidate_id));
        link_map.insert(
            "candidate_state_sha256".to_owned(),
            json!(candidate_state_sha256),
        );
        link_map.insert("base_version_id".to_owned(), Value::Null);
        link_map.insert("source_low_artifact_id".to_owned(), json!("low-artifact"));
        link_map.insert(
            "source_low_artifact_object_sha256".to_owned(),
            json!(source.record.sha256),
        );
        link_map.insert(
            "source_low_artifact_sha256".to_owned(),
            json!(source.record.sha256),
        );
        link_map.insert(
            "source_low_artifact_readback_object_sha256".to_owned(),
            json!(source_readback.record.sha256),
        );
        link_map.insert(
            "source_low_artifact_readback_sha256".to_owned(),
            json!(source_readback_sha256),
        );
        link_map.insert("resolution".to_owned(), json!(2048));
        link_map.insert("padding_texels".to_owned(), json!(8));
        link_map.insert("min_mip_level".to_owned(), json!(3));
        link_map.insert("hard_edge_angle_deg".to_owned(), json!(45.0));
        link_map.insert("stretch_threshold".to_owned(), json!(4.0));
        link_map.insert(
            "visibility_weights_sha256".to_owned(),
            json!("d".repeat(64)),
        );
        link_map.insert(
            "layout_object_sha256".to_owned(),
            json!(layout.record.sha256),
        );
        link_map.insert(
            "layout_canonical_sha256".to_owned(),
            json!(layout_canonical_sha256),
        );
        link_map.insert(
            "worker_build_cohort_sha256".to_owned(),
            json!("e".repeat(64)),
        );
        link_map.insert("request_sha256".to_owned(), json!("f".repeat(64)));
        link_map.insert("input_sha256".to_owned(), json!("1".repeat(64)));
        link_map.insert("idempotency_key".to_owned(), json!("hero-uv-idempotency"));
        link_map.insert("replay_count".to_owned(), json!(2));
        link_map.insert("replay_byte_exact".to_owned(), json!(true));
        link_map.insert("link_policy".to_owned(), json!(HERO_UV_LINK_POLICY));
        link_map.insert("writer_policy".to_owned(), json!(WRITER_POLICY));
        link_map.insert(
            "materialization_status".to_owned(),
            json!(HERO_UV_MATERIALIZATION_STATUS),
        );
        link_map.insert(
            "idempotency_policy".to_owned(),
            json!(HERO_UV_IDEMPOTENCY_POLICY),
        );
        link_map.insert("source_only".to_owned(), json!(true));
        link_map.insert("runtime_write_performed".to_owned(), json!(true));
        link_map.insert("persistent_user_data_touched".to_owned(), json!(true));
        link_map.insert("production_stage_advanced".to_owned(), json!(false));
        link_map.insert("candidate_confirmed".to_owned(), json!(false));
        link_map.insert("version_created".to_owned(), json!(false));
        link_map.insert("export_performed".to_owned(), json!(false));
        link_map.insert("quality_status".to_owned(), json!("structural_only"));
        link_map.insert("visual_status".to_owned(), json!("NOT_PROVEN"));
        link_map.insert("human_status".to_owned(), json!("NOT_RUN"));
        link_map.insert("engine_status".to_owned(), json!("NOT_RUN"));
        link_map.insert("distribution_status".to_owned(), json!("NOT_RUN"));
        link_map.insert(
            "canonicalization_policy".to_owned(),
            json!(CANONICALIZATION_POLICY),
        );
        link_map.insert("canonical_sha256".to_owned(), json!(""));
        link_map.insert("created_at".to_owned(), json!("1"));
        let mut link_value = Value::Object(link_map);
        let link_canonical = canonical_json_hash(&link_value);
        link_value["canonical_sha256"] = Value::String(link_canonical);
        let link_bytes = canonical_json_bytes(&link_value).expect("link bytes");
        let link = store
            .put_object(&link_bytes, None, JSON_MIME, HERO_UV_LINK_CAS_KIND, "1")
            .expect("link");
        let mut record = HeroUvDurableRecord {
            schema_version: HERO_UV_DURABLE_RECORD_SCHEMA.to_owned(),
            project_id,
            candidate_id,
            candidate_state_sha256,
            base_version_id: None,
            source_low_artifact_id: "low-artifact".to_owned(),
            source_low_artifact_object_sha256: source.record.sha256.clone(),
            source_low_artifact_sha256: source.record.sha256,
            source_low_artifact_readback_object_sha256: source_readback.record.sha256,
            source_low_artifact_readback_sha256: source_readback_sha256,
            resolution: 2048,
            padding_texels: 8,
            min_mip_level: 3,
            hard_edge_angle_deg: 45.0,
            stretch_threshold: 4.0,
            visibility_weights_sha256: "d".repeat(64),
            layout_object_sha256: layout.record.sha256.clone(),
            layout_canonical_sha256,
            worker_build_cohort_sha256: "e".repeat(64),
            link_id: "hero-uv-link".to_owned(),
            link_object_sha256: link.record.sha256.clone(),
            request_sha256: "f".repeat(64),
            input_sha256: "1".repeat(64),
            idempotency_key: "hero-uv-idempotency".to_owned(),
            materialization_status: HERO_UV_MATERIALIZATION_STATUS.to_owned(),
            canonical_sha256: String::new(),
            created_at: "1".to_owned(),
        };
        let mut record_value = serde_json::to_value(&record).expect("record value");
        record_value["canonical_sha256"] = Value::String(String::new());
        record.canonical_sha256 = canonical_json_hash(&record_value);
        (
            record,
            HeroUvDurableCasPayload {
                bytes: layout_bytes,
                object_sha256: layout.record.sha256,
                mime: JSON_MIME.to_owned(),
                kind: HERO_UV_LAYOUT_CAS_KIND.to_owned(),
            },
            HeroUvDurableCasPayload {
                bytes: link_bytes,
                object_sha256: link.record.sha256,
                mime: JSON_MIME.to_owned(),
                kind: HERO_UV_LINK_CAS_KIND.to_owned(),
            },
        )
    }

    #[test]
    fn hero_uv_commit_replay_get_readback_and_gc_roots_are_exact() {
        let store = Store::memory().expect("store");
        let unknown = "0".repeat(64);
        {
            let mut connection = store.lock_connection().expect("connection");
            let transaction = connection.transaction().expect("transaction");
            let before: i64 = transaction
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    params![TABLE],
                    |row| row.get(0),
                )
                .expect("table probe");
            assert_eq!(before, 0);
            assert!(
                !super::super::authoring_mesh_edit_object_is_linked(&transaction, &unknown)
                    .expect("unknown linked query")
            );
            let after: i64 = transaction
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    params![TABLE],
                    |row| row.get(0),
                )
                .expect("table probe after ensure");
            assert_eq!(after, 1);
            transaction.commit().expect("commit");
        }
        let (record, layout, link) = fixture(&store);
        let source_hash = record.source_low_artifact_object_sha256.clone();
        let source_readback_hash = record.source_low_artifact_readback_object_sha256.clone();
        let layout_hash = record.layout_object_sha256.clone();
        let link_hash = record.link_object_sha256.clone();
        let (stored, replayed) = store
            .commit_hero_uv(&record, &layout, &link)
            .expect("first commit");
        assert!(!replayed);
        let (replayed_record, replayed) = store
            .commit_hero_uv(&record, &layout, &link)
            .expect("exact replay");
        assert!(replayed);
        assert_eq!(stored, replayed_record);
        assert_eq!(
            store
                .get_hero_uv(&record.project_id, &record.idempotency_key)
                .expect("get"),
            Some(record.clone())
        );
        assert_eq!(
            store
                .get_hero_uv_by_link_id(&record.link_id)
                .expect("link get"),
            Some(record.clone())
        );
        assert_eq!(
            store
                .get_hero_uv_by_candidate_source_artifact(
                    &record.project_id,
                    &record.candidate_id,
                    &record.source_low_artifact_sha256,
                )
                .expect("candidate/artifact get"),
            Some(record.clone())
        );
        assert_eq!(
            store
                .get_hero_uv_by_candidate_source_artifact(
                    &record.project_id,
                    &record.candidate_id,
                    &"9".repeat(64),
                )
                .expect("missing candidate/artifact get"),
            None
        );
        let bundle = store.read_hero_uv_bundle(&record).expect("readback");
        assert_eq!(bundle.layout_bytes, layout.bytes);
        assert_eq!(bundle.link_bytes, link.bytes);
        for hash in [source_hash, source_readback_hash, layout_hash, link_hash] {
            assert_eq!(
                store
                    .get_object(&hash)
                    .expect("object")
                    .expect("cas")
                    .reachability,
                "reachable"
            );
            let mut connection = store.lock_connection().expect("connection");
            let transaction = connection.transaction().expect("transaction");
            assert!(
                super::super::authoring_mesh_edit_object_is_linked(&transaction, &hash)
                    .expect("linked root")
            );
            transaction.commit().expect("commit");
        }
        let mut conflict = record.clone();
        conflict.input_sha256 = "2".repeat(64);
        let mut value = serde_json::to_value(&conflict).expect("conflict value");
        value["canonical_sha256"] = Value::String(String::new());
        conflict.canonical_sha256 = canonical_json_hash(&value);
        let error = store
            .commit_hero_uv(&conflict, &layout, &link)
            .expect_err("conflict");
        assert!(matches!(
            error,
            StoreError::Contract { code, .. } if code == "HERO_UV_DURABLE_RECORD_CONFLICT"
        ));
    }
}
