//! Durable, additive foundation -> AuthoringMesh@2 materialization index.
//!
//! A foundation import is deliberately not a candidate, version, or export.
//! This module records the bounded transition from that source-only import to
//! one immutable AuthoringMesh@2 revision per Part.  The descriptor and every
//! revision payload are already in CAS when this boundary is called; Store
//! verifies their metadata and canonical bytes, then inserts the revision rows
//! and aggregate row in one SQLite transaction.

use forgecad_contracts::{AuthoringMeshRevision, CasObjectRecord, is_opaque_id, is_sha256};
use forgecad_core::{canonical_json_bytes, canonical_json_hash, sha256_hex};
use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

use super::weapon_foundation_import::WeaponFoundationImportRecord;
use super::{AuthoringMeshV2DurableRecord, Store, StoreError};

pub const RECORD_SCHEMA_VERSION: &str = "FoundationAuthoringMeshV2MaterializationRecord@1";
pub const DESCRIPTOR_SCHEMA_VERSION: &str = "WeaponFoundationAuthoringMaterializationDescriptor@1";
pub const TABLE: &str = "foundation_authoring_mesh_v2_materializations";
pub const DESCRIPTOR_OBJECT_KIND: &str = "forgecad-foundation-authoring-mesh-v2-descriptor";
pub const STATUS: &str = "runtime-owned-durable-authoring-mesh-v2-foundation@1";
pub const JSON_MIME: &str = "application/json";
pub const MAX_JSON_BYTES: u64 = 8 * 1024 * 1024;
/// A foundation mesh may contain all authored topology for a Part.  Keep the
/// wider bound local to this source-bound materialization path; the generic
/// AuthoringMesh durable API remains on its historical 1 MiB limit.
pub const FOUNDATION_REVISION_MAX_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_PARTS: usize = 4096;
pub const MAX_PART_ELEMENTS: u64 = 10_000_000;

fn contract(code: &str, message: impl Into<String>) -> StoreError {
    StoreError::Contract {
        code: code.to_owned(),
        message: message.into(),
    }
}

/// One Part entry in the immutable materialization descriptor.  The
/// idempotency key is the lookup key of the existing AuthoringMesh@2 durable
/// row; the object hash is the CAS root which the descriptor owns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FoundationAuthoringMeshV2PartRevision {
    pub part_id: String,
    pub mesh_id: String,
    pub lineage_id: String,
    pub revision_id: String,
    pub idempotency_key: String,
    pub revision_object_sha256: String,
    pub revision_sha256: String,
    pub vertex_count: u64,
    pub face_count: u64,
}

/// CAS payload describing the complete, bounded Part revision set.  It is
/// intentionally Store-local: the typed AuthoringMeshRevision remains the
/// topology authority, while this descriptor only closes the aggregate
/// lineage and reachability binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FoundationAuthoringMeshV2MaterializationDescriptor {
    pub schema_version: String,
    pub project_id: String,
    pub foundation_request_id: String,
    pub foundation_request_sha256: String,
    pub foundation_result_object_sha256: String,
    pub foundation_topology_object_sha256: String,
    pub foundation_socket_map_object_sha256: String,
    pub foundation_rig_map_object_sha256: String,
    pub foundation_fps_presentation_package_object_sha256: String,
    pub part_revisions: Vec<FoundationAuthoringMeshV2PartRevision>,
    pub part_revision_summary_sha256: String,
    pub part_count: u64,
    pub vertex_count: u64,
    pub face_count: u64,
    pub status: String,
    pub canonical_sha256: String,
}

/// The aggregate row.  It never carries source bytes, paths, URLs, scripts,
/// candidates, versions, or exports; all large data remains in CAS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FoundationAuthoringMeshV2MaterializationRecord {
    pub schema_version: String,
    pub project_id: String,
    pub idempotency_key: String,
    pub foundation_request_id: String,
    pub foundation_request_sha256: String,
    pub foundation_result_object_sha256: String,
    pub foundation_topology_object_sha256: String,
    pub foundation_socket_map_object_sha256: String,
    pub foundation_rig_map_object_sha256: String,
    pub foundation_fps_presentation_package_object_sha256: String,
    pub descriptor_object_sha256: String,
    pub descriptor_canonical_sha256: String,
    pub part_revision_summary_sha256: String,
    pub part_count: u64,
    pub vertex_count: u64,
    pub face_count: u64,
    pub status: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// One revision input to the atomic batch API.  Runtime has already written
/// `object` to CAS and registered it in the Store; this API only verifies and
/// durably binds it.
#[derive(Debug, Clone)]
pub struct FoundationAuthoringMeshV2RevisionInput {
    pub part_id: String,
    pub record: AuthoringMeshV2DurableRecord,
    pub revision: AuthoringMeshRevision,
    pub object: CasObjectRecord,
}

/// All inputs needed for one atomic materialization.  The descriptor is
/// supplied as its typed closed shape so Store can compare it byte-for-byte
/// with the descriptor CAS object without accepting arbitrary bytes.
#[derive(Debug, Clone)]
pub struct FoundationAuthoringMeshV2MaterializationBatch {
    pub record: FoundationAuthoringMeshV2MaterializationRecord,
    pub descriptor: FoundationAuthoringMeshV2MaterializationDescriptor,
    pub descriptor_object: CasObjectRecord,
    pub revisions: Vec<FoundationAuthoringMeshV2RevisionInput>,
}

pub(crate) fn ensure_table(connection: &rusqlite::Connection) -> Result<(), StoreError> {
    connection.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {TABLE} (
             schema_version TEXT NOT NULL CHECK (schema_version = 'FoundationAuthoringMeshV2MaterializationRecord@1'),
             project_id TEXT NOT NULL REFERENCES projects(project_id),
             idempotency_key TEXT NOT NULL,
             foundation_request_id TEXT NOT NULL,
             foundation_request_sha256 TEXT NOT NULL,
             foundation_result_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             foundation_topology_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             foundation_socket_map_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             foundation_rig_map_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             foundation_fps_presentation_package_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             descriptor_object_sha256 TEXT NOT NULL UNIQUE REFERENCES objects(sha256),
             descriptor_canonical_sha256 TEXT NOT NULL,
             part_revision_summary_sha256 TEXT NOT NULL,
             part_count INTEGER NOT NULL CHECK (part_count BETWEEN 1 AND 4096),
             vertex_count INTEGER NOT NULL CHECK (vertex_count >= 0),
             face_count INTEGER NOT NULL CHECK (face_count >= 0),
             status TEXT NOT NULL CHECK (status = 'runtime-owned-durable-authoring-mesh-v2-foundation@1'),
             canonical_sha256 TEXT NOT NULL,
             created_at TEXT NOT NULL,
             part_revision_object_sha256s_json TEXT NOT NULL,
             record_json TEXT NOT NULL,
             PRIMARY KEY (project_id, idempotency_key),
             UNIQUE (project_id, foundation_request_id)
         );
         CREATE INDEX IF NOT EXISTS foundation_authoring_mesh_v2_materializations_project_idx
             ON {TABLE}(project_id, created_at DESC, idempotency_key ASC);
         CREATE INDEX IF NOT EXISTS foundation_authoring_mesh_v2_materializations_foundation_idx
             ON {TABLE}(foundation_request_id, foundation_result_object_sha256);
         CREATE INDEX IF NOT EXISTS foundation_authoring_mesh_v2_materializations_roots_idx
             ON {TABLE}(descriptor_object_sha256, foundation_result_object_sha256);"
    ))?;
    Ok(())
}

fn to_value<T: Serialize>(input: &T) -> Result<Value, StoreError> {
    serde_json::to_value(input).map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn canonical_json<T: Serialize>(input: &T) -> Result<Vec<u8>, StoreError> {
    canonical_json_bytes(&to_value(input)?)
        .map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn canonical_hash_without_field<T: Serialize>(
    input: &T,
    field: &str,
) -> Result<String, StoreError> {
    let mut value = to_value(input)?;
    let object = value.as_object_mut().ok_or_else(|| {
        StoreError::InvalidData("materialization canonical payload must be an object".to_owned())
    })?;
    object.insert(field.to_owned(), Value::String(String::new()));
    Ok(canonical_json_hash(&value))
}

fn canonical_string<T: Serialize>(value: &T) -> Result<String, StoreError> {
    String::from_utf8(canonical_json(value)?)
        .map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn validate_hashes(values: &[(&str, &str)]) -> Result<(), StoreError> {
    if let Some((field, _)) = values.iter().find(|(_, value)| !is_sha256(value)) {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_HASH_INVALID",
            format!("{field} must be a lowercase SHA-256 hash"),
        ));
    }
    Ok(())
}

fn validate_record(
    record: &FoundationAuthoringMeshV2MaterializationRecord,
) -> Result<(), StoreError> {
    if record.schema_version != RECORD_SCHEMA_VERSION
        || !is_opaque_id(&record.project_id)
        || !is_opaque_id(&record.idempotency_key)
        || !is_opaque_id(&record.foundation_request_id)
        || record.status != STATUS
        || record.part_count == 0
        || record.part_count as usize > MAX_PARTS
        || record.vertex_count > MAX_PARTS as u64 * MAX_PART_ELEMENTS
        || record.face_count > MAX_PARTS as u64 * MAX_PART_ELEMENTS
        || record.created_at.is_empty()
        || record.created_at.len() > 128
    {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_RECORD_INVALID",
            "foundation AuthoringMesh@2 aggregate metadata is malformed",
        ));
    }
    validate_hashes(&[
        (
            "foundation_request_sha256",
            &record.foundation_request_sha256,
        ),
        (
            "foundation_result_object_sha256",
            &record.foundation_result_object_sha256,
        ),
        (
            "foundation_topology_object_sha256",
            &record.foundation_topology_object_sha256,
        ),
        (
            "foundation_socket_map_object_sha256",
            &record.foundation_socket_map_object_sha256,
        ),
        (
            "foundation_rig_map_object_sha256",
            &record.foundation_rig_map_object_sha256,
        ),
        (
            "foundation_fps_presentation_package_object_sha256",
            &record.foundation_fps_presentation_package_object_sha256,
        ),
        ("descriptor_object_sha256", &record.descriptor_object_sha256),
        (
            "descriptor_canonical_sha256",
            &record.descriptor_canonical_sha256,
        ),
        (
            "part_revision_summary_sha256",
            &record.part_revision_summary_sha256,
        ),
        ("canonical_sha256", &record.canonical_sha256),
    ])?;
    if canonical_hash_without_field(record, "canonical_sha256")? != record.canonical_sha256 {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_RECORD_CANONICAL_MISMATCH",
            "foundation AuthoringMesh@2 aggregate canonical hash differs",
        ));
    }
    Ok(())
}

fn validate_descriptor_shape(
    descriptor: &FoundationAuthoringMeshV2MaterializationDescriptor,
) -> Result<(), StoreError> {
    if descriptor.schema_version != DESCRIPTOR_SCHEMA_VERSION
        || !is_opaque_id(&descriptor.project_id)
        || !is_opaque_id(&descriptor.foundation_request_id)
        || descriptor.status != STATUS
        || descriptor.part_revisions.is_empty()
        || descriptor.part_revisions.len() > MAX_PARTS
        || descriptor.part_count != descriptor.part_revisions.len() as u64
        || descriptor.vertex_count > MAX_PARTS as u64 * MAX_PART_ELEMENTS
        || descriptor.face_count > MAX_PARTS as u64 * MAX_PART_ELEMENTS
    {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_DESCRIPTOR_INVALID",
            "foundation AuthoringMesh@2 descriptor shape is malformed",
        ));
    }
    validate_hashes(&[
        (
            "foundation_request_sha256",
            &descriptor.foundation_request_sha256,
        ),
        (
            "foundation_result_object_sha256",
            &descriptor.foundation_result_object_sha256,
        ),
        (
            "foundation_topology_object_sha256",
            &descriptor.foundation_topology_object_sha256,
        ),
        (
            "foundation_socket_map_object_sha256",
            &descriptor.foundation_socket_map_object_sha256,
        ),
        (
            "foundation_rig_map_object_sha256",
            &descriptor.foundation_rig_map_object_sha256,
        ),
        (
            "foundation_fps_presentation_package_object_sha256",
            &descriptor.foundation_fps_presentation_package_object_sha256,
        ),
        (
            "part_revision_summary_sha256",
            &descriptor.part_revision_summary_sha256,
        ),
        ("canonical_sha256", &descriptor.canonical_sha256),
    ])?;
    if canonical_hash_without_field(descriptor, "canonical_sha256")? != descriptor.canonical_sha256
    {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_DESCRIPTOR_CANONICAL_MISMATCH",
            "foundation AuthoringMesh@2 descriptor canonical hash differs",
        ));
    }

    let mut part_ids = BTreeSet::new();
    let mut revision_objects = BTreeSet::new();
    let mut vertex_total = 0_u64;
    let mut face_total = 0_u64;
    for pair in descriptor.part_revisions.windows(2) {
        if pair[0].part_id >= pair[1].part_id {
            return Err(contract(
                "FOUNDATION_AUTHORING_MESH_V2_DESCRIPTOR_ORDER_INVALID",
                "descriptor Part revisions must be sorted by unique part_id",
            ));
        }
    }
    for part in &descriptor.part_revisions {
        if !is_opaque_id(&part.part_id)
            || !is_opaque_id(&part.mesh_id)
            || !is_opaque_id(&part.lineage_id)
            || !is_opaque_id(&part.revision_id)
            || !is_opaque_id(&part.idempotency_key)
            || part.vertex_count > MAX_PART_ELEMENTS
            || part.face_count > MAX_PART_ELEMENTS
        {
            return Err(contract(
                "FOUNDATION_AUTHORING_MESH_V2_DESCRIPTOR_PART_INVALID",
                "descriptor Part revision identity or totals are malformed",
            ));
        }
        validate_hashes(&[
            ("revision_object_sha256", &part.revision_object_sha256),
            ("revision_sha256", &part.revision_sha256),
        ])?;
        if !part_ids.insert(&part.part_id) || !revision_objects.insert(&part.revision_object_sha256)
        {
            return Err(contract(
                "FOUNDATION_AUTHORING_MESH_V2_DESCRIPTOR_DUPLICATE",
                "descriptor contains a duplicate Part or revision CAS root",
            ));
        }
        vertex_total = vertex_total.checked_add(part.vertex_count).ok_or_else(|| {
            contract(
                "FOUNDATION_AUTHORING_MESH_V2_DESCRIPTOR_TOTALS_INVALID",
                "vertex total overflow",
            )
        })?;
        face_total = face_total.checked_add(part.face_count).ok_or_else(|| {
            contract(
                "FOUNDATION_AUTHORING_MESH_V2_DESCRIPTOR_TOTALS_INVALID",
                "face total overflow",
            )
        })?;
    }
    if vertex_total != descriptor.vertex_count || face_total != descriptor.face_count {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_DESCRIPTOR_TOTALS_MISMATCH",
            "descriptor Part totals do not equal aggregate totals",
        ));
    }
    let summary = canonical_json_hash(&to_value(&descriptor.part_revisions)?);
    if summary != descriptor.part_revision_summary_sha256 {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_DESCRIPTOR_SUMMARY_MISMATCH",
            "descriptor Part revision set summary differs",
        ));
    }
    Ok(())
}

fn same_record(
    left: &FoundationAuthoringMeshV2MaterializationRecord,
    right: &FoundationAuthoringMeshV2MaterializationRecord,
) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    left.created_at.clear();
    right.created_at.clear();
    left.canonical_sha256.clear();
    right.canonical_sha256.clear();
    left == right
}

fn validate_descriptor_binding(
    record: &FoundationAuthoringMeshV2MaterializationRecord,
    descriptor: &FoundationAuthoringMeshV2MaterializationDescriptor,
) -> Result<(), StoreError> {
    validate_descriptor_shape(descriptor)?;
    if descriptor.project_id != record.project_id
        || descriptor.foundation_request_id != record.foundation_request_id
        || descriptor.foundation_request_sha256 != record.foundation_request_sha256
        || descriptor.foundation_result_object_sha256 != record.foundation_result_object_sha256
        || descriptor.foundation_topology_object_sha256 != record.foundation_topology_object_sha256
        || descriptor.foundation_socket_map_object_sha256
            != record.foundation_socket_map_object_sha256
        || descriptor.foundation_rig_map_object_sha256 != record.foundation_rig_map_object_sha256
        || descriptor.foundation_fps_presentation_package_object_sha256
            != record.foundation_fps_presentation_package_object_sha256
        || descriptor.part_revision_summary_sha256 != record.part_revision_summary_sha256
        || descriptor.part_count != record.part_count
        || descriptor.vertex_count != record.vertex_count
        || descriptor.face_count != record.face_count
    {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_DESCRIPTOR_BINDING_MISMATCH",
            "descriptor does not match the aggregate foundation binding",
        ));
    }
    if descriptor.canonical_sha256 != record.descriptor_canonical_sha256 {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_DESCRIPTOR_CANONICAL_BINDING_MISMATCH",
            "aggregate descriptor canonical hash differs",
        ));
    }
    Ok(())
}

fn validate_descriptor_cas(
    store: &Store,
    transaction: &Transaction<'_>,
    object: &CasObjectRecord,
    expected_hash: &str,
    require_reachable: bool,
) -> Result<FoundationAuthoringMeshV2MaterializationDescriptor, StoreError> {
    if object.schema_version != "CasObject@1"
        || object.sha256 != expected_hash
        || !is_sha256(&object.sha256)
        || object.mime != JSON_MIME
        || object.kind != DESCRIPTOR_OBJECT_KIND
        || object.size_bytes == 0
        || object.size_bytes > MAX_JSON_BYTES
        || !matches!(object.reachability.as_str(), "temporary" | "reachable")
        || (require_reachable && object.reachability != "reachable")
        || object.created_at.is_empty()
        || object.created_at.len() > 128
    {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_DESCRIPTOR_CAS_METADATA_INVALID",
            "descriptor CAS metadata or binding differs",
        ));
    }
    let stored: Option<(i64, String, String, String)> = transaction
        .query_row(
            "SELECT size_bytes, mime, kind, reachability FROM objects WHERE sha256 = ?1",
            params![expected_hash],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    let Some((size, mime, kind, reachability)) = stored else {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_DESCRIPTOR_CAS_OBJECT_UNAVAILABLE",
            "descriptor CAS object is not registered",
        ));
    };
    if size != i64::try_from(object.size_bytes).unwrap_or(i64::MAX)
        || mime != object.mime
        || kind != object.kind
        || !matches!(reachability.as_str(), "temporary" | "reachable")
        || (require_reachable && reachability != "reachable")
    {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_DESCRIPTOR_CAS_METADATA_INVALID",
            "descriptor CAS metadata differs from SQLite",
        ));
    }
    let bytes = store
        .cas
        .read_verified_bounded(expected_hash, MAX_JSON_BYTES)
        .map_err(StoreError::Cas)?;
    if bytes.len() as u64 != object.size_bytes || sha256_hex(&bytes) != expected_hash {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_DESCRIPTOR_CAS_HASH_MISMATCH",
            "descriptor CAS bytes do not match their registered hash",
        ));
    }
    let descriptor: FoundationAuthoringMeshV2MaterializationDescriptor =
        serde_json::from_slice(&bytes).map_err(|error| {
            contract(
                "FOUNDATION_AUTHORING_MESH_V2_DESCRIPTOR_JSON_INVALID",
                format!("descriptor JSON is invalid: {error}"),
            )
        })?;
    validate_descriptor_shape(&descriptor)?;
    if canonical_json(&descriptor)? != bytes {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_DESCRIPTOR_CAS_NON_CANONICAL",
            "descriptor CAS bytes are not canonical JSON",
        ));
    }
    Ok(descriptor)
}

fn validate_foundation_revision_object(
    store: &Store,
    object: &CasObjectRecord,
    expected_hash: &str,
    require_reachable: bool,
) -> Result<Vec<u8>, StoreError> {
    if object.schema_version != "CasObject@1"
        || object.sha256 != expected_hash
        || !is_sha256(&object.sha256)
        || object.mime != super::AUTHORING_MESH_V2_DURABLE_OBJECT_MIME
        || object.kind != super::AUTHORING_MESH_V2_REVISION_OBJECT_KIND
        || object.size_bytes == 0
        || object.size_bytes > FOUNDATION_REVISION_MAX_BYTES
        || !matches!(object.reachability.as_str(), "temporary" | "reachable")
        || (require_reachable && object.reachability != "reachable")
        || object.created_at.is_empty()
        || object.created_at.len() > 128
    {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_REVISION_CAS_METADATA_INVALID",
            "foundation Part revision CAS metadata or binding differs",
        ));
    }
    let bytes = store
        .cas
        .read_verified_bounded(expected_hash, FOUNDATION_REVISION_MAX_BYTES)
        .map_err(StoreError::Cas)?;
    if bytes.len() as u64 != object.size_bytes || sha256_hex(&bytes) != expected_hash {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_REVISION_CAS_HASH_MISMATCH",
            "foundation Part revision CAS bytes do not match their registered hash",
        ));
    }
    Ok(bytes)
}

fn ensure_descriptor_object_row(
    transaction: &Transaction<'_>,
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
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_DESCRIPTOR_CAS_OBJECT_UNAVAILABLE",
            "descriptor CAS object is not registered",
        ));
    };
    if size != i64::try_from(object.size_bytes).unwrap_or(i64::MAX)
        || mime != object.mime
        || kind != object.kind
        || !matches!(reachability.as_str(), "temporary" | "reachable")
        || (require_reachable && reachability != "reachable")
    {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_DESCRIPTOR_CAS_METADATA_INVALID",
            "descriptor CAS metadata differs from SQLite",
        ));
    }
    Ok(())
}

fn foundation_record_json(
    transaction: &Transaction<'_>,
    request_id: &str,
) -> Result<Option<WeaponFoundationImportRecord>, StoreError> {
    let record_json: Option<String> = transaction
        .query_row(
            "SELECT record_json FROM weapon_foundation_imports WHERE request_id = ?1",
            params![request_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(record_json) = record_json else {
        return Ok(None);
    };
    serde_json::from_str(&record_json)
        .map(Some)
        .map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn validate_foundation_record(
    transaction: &Transaction<'_>,
    record: &WeaponFoundationImportRecord,
) -> Result<(), StoreError> {
    let Some(asset) = super::weapon_foundation_import::allowlisted_asset(&record.asset_id) else {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_FOUNDATION_INVALID",
            "foundation import asset is not allowlisted",
        ));
    };
    if record.schema_version != super::weapon_foundation_import::RECORD_SCHEMA_VERSION
        || !is_opaque_id(&record.request_id)
        || !is_sha256(&record.request_sha256)
        || record.foundation_pack_id != super::weapon_foundation_import::FOUNDATION_PACK_ID
        || record.foundation_pack_version
            != super::weapon_foundation_import::FOUNDATION_PACK_VERSION
        || record.foundation_manifest_sha256
            != super::weapon_foundation_import::FOUNDATION_MANIFEST_SHA256
        || record.asset_sha256 != asset.asset_sha256
        || record.asset_role != asset.asset_role
        || record.source_format != asset.source_format
        || !is_sha256(&record.coordinate_spec_sha256)
        || !is_sha256(&record.topology_object_sha256)
        || !is_sha256(&record.socket_map_object_sha256)
        || !is_sha256(&record.rig_map_object_sha256)
        || !is_sha256(&record.fps_presentation_package_object_sha256)
        || !is_sha256(&record.result_object_sha256)
        || !is_sha256(&record.link_object_sha256)
        || record.authoring_mesh_materialization_status
            != super::weapon_foundation_import::MATERIALIZATION_PENDING
        || record.import_status != "IMPORTED_DRAFT"
        || record.quality_status != "structural_only"
        || record.promotion_eligible
        || record.candidate_confirmed
        || record.version_created
        || record.export_performed
        || record.actual_engine_roundtrip
        || record.human_review_status != "NOT_RUN"
        || record.created_at.is_empty()
        || record.created_at.len() > 128
        || super::weapon_foundation_import::canonical_record_sha256(record)?
            != record.canonical_sha256
    {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_FOUNDATION_INVALID",
            "foundation import row is malformed or has an invalid canonical hash",
        ));
    }
    let stored_json: String = transaction.query_row(
        "SELECT record_json FROM weapon_foundation_imports WHERE request_id = ?1",
        params![record.request_id],
        |row| row.get(0),
    )?;
    if stored_json != canonical_string(record)? {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_FOUNDATION_NON_CANONICAL",
            "foundation import row JSON is not canonical",
        ));
    }
    Ok(())
}

fn validate_foundation_root(
    store: &Store,
    transaction: &Transaction<'_>,
    hash: &str,
    mime_expected: &str,
    kind_expected: &str,
    max_bytes: u64,
    require_reachable: bool,
) -> Result<Vec<u8>, StoreError> {
    let row: Option<(i64, String, String, String)> = transaction
        .query_row(
            "SELECT size_bytes, mime, kind, reachability FROM objects WHERE sha256 = ?1",
            params![hash],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    let Some((size, mime, kind, reachability)) = row else {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_FOUNDATION_CAS_OBJECT_MISSING",
            "foundation import references a missing CAS object",
        ));
    };
    if !is_sha256(hash)
        || size <= 0
        || u64::try_from(size).unwrap_or(u64::MAX) > max_bytes
        || mime != mime_expected
        || kind != kind_expected
        || !matches!(reachability.as_str(), "temporary" | "reachable")
        || (require_reachable && reachability != "reachable")
    {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_FOUNDATION_CAS_METADATA_INVALID",
            "foundation import CAS metadata is outside the closed binding",
        ));
    }
    let bytes = store
        .cas
        .read_verified_bounded(hash, max_bytes)
        .map_err(StoreError::Cas)?;
    if bytes.len() as i64 != size || sha256_hex(&bytes) != hash {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_FOUNDATION_CAS_HASH_MISMATCH",
            "foundation import CAS bytes do not match their registered hash",
        ));
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|error| {
        contract(
            "FOUNDATION_AUTHORING_MESH_V2_FOUNDATION_JSON_INVALID",
            format!("foundation JSON CAS is invalid: {error}"),
        )
    })?;
    if canonical_json_bytes(&value).map_err(|error| StoreError::InvalidData(error.to_string()))?
        != bytes
    {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_FOUNDATION_JSON_NON_CANONICAL",
            "foundation JSON CAS bytes are not canonical",
        ));
    }
    Ok(bytes)
}

fn validate_foundation_result_or_link(
    bytes: &[u8],
    record: &WeaponFoundationImportRecord,
    result: bool,
) -> Result<(), StoreError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        contract(
            if result {
                "FOUNDATION_AUTHORING_MESH_V2_FOUNDATION_RESULT_INVALID"
            } else {
                "FOUNDATION_AUTHORING_MESH_V2_FOUNDATION_LINK_INVALID"
            },
            format!("foundation JSON is invalid: {error}"),
        )
    })?;
    let schema = if result {
        super::weapon_foundation_import::RESULT_SCHEMA_VERSION
    } else {
        super::weapon_foundation_import::LINK_SCHEMA_VERSION
    };
    let matches = value.get("schema_version").and_then(Value::as_str) == Some(schema)
        && value.get("request_id").and_then(Value::as_str) == Some(record.request_id.as_str())
        && value.get("request_sha256").and_then(Value::as_str)
            == Some(record.request_sha256.as_str())
        && value.get("asset_id").and_then(Value::as_str) == Some(record.asset_id.as_str())
        && (!result
            || (value.get("asset_sha256").and_then(Value::as_str)
                == Some(record.asset_sha256.as_str())
                && value.get("topology_object_sha256").and_then(Value::as_str)
                    == Some(record.topology_object_sha256.as_str())
                && value
                    .get("socket_map_object_sha256")
                    .and_then(Value::as_str)
                    == Some(record.socket_map_object_sha256.as_str())
                && value.get("rig_map_object_sha256").and_then(Value::as_str)
                    == Some(record.rig_map_object_sha256.as_str())
                && value
                    .get("fps_presentation_package_object_sha256")
                    .and_then(Value::as_str)
                    == Some(record.fps_presentation_package_object_sha256.as_str())
                && value
                    .get("authoring_mesh_materialization_status")
                    .and_then(Value::as_str)
                    == Some(super::weapon_foundation_import::MATERIALIZATION_PENDING)
                && value.get("quality_status").and_then(Value::as_str) == Some("structural_only")
                && value.get("promotion_eligible").and_then(Value::as_bool) == Some(false)
                && value.get("candidate_confirmed").and_then(Value::as_bool) == Some(false)
                && value.get("version_created").and_then(Value::as_bool) == Some(false)
                && value.get("export_performed").and_then(Value::as_bool) == Some(false)
                && value
                    .get("actual_engine_roundtrip")
                    .and_then(Value::as_bool)
                    == Some(false)
                && value.get("human_review_status").and_then(Value::as_str) == Some("NOT_RUN")))
        && (!result
            || value
                .get("canonical_sha256")
                .and_then(Value::as_str)
                .is_some())
        && (!result
            || value
                .get("canonical_sha256")
                .and_then(Value::as_str)
                .is_some());
    if !matches {
        return Err(contract(
            if result {
                "FOUNDATION_AUTHORING_MESH_V2_FOUNDATION_RESULT_BINDING_MISMATCH"
            } else {
                "FOUNDATION_AUTHORING_MESH_V2_FOUNDATION_LINK_BINDING_MISMATCH"
            },
            "foundation result/link does not match its immutable import row",
        ));
    }
    if !result
        && (value.get("result_object_sha256").and_then(Value::as_str)
            != Some(record.result_object_sha256.as_str())
            || value
                .get("authoring_mesh_materialization_status")
                .and_then(Value::as_str)
                != Some(super::weapon_foundation_import::MATERIALIZATION_PENDING)
            || value.get("writer_policy").and_then(Value::as_str)
                != Some(super::weapon_foundation_import::WRITER_POLICY))
    {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_FOUNDATION_LINK_BINDING_MISMATCH",
            "foundation link does not match its immutable import row",
        ));
    }
    let supplied = value
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            contract(
                "FOUNDATION_AUTHORING_MESH_V2_FOUNDATION_CANONICAL_MISSING",
                "foundation result/link canonical hash is missing",
            )
        })?
        .to_owned();
    if !is_sha256(&supplied) {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_FOUNDATION_CANONICAL_INVALID",
            "foundation result/link canonical hash is invalid",
        ));
    }
    let mut without_hash = value;
    without_hash["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&without_hash) != supplied {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_FOUNDATION_CANONICAL_MISMATCH",
            "foundation result/link canonical hash differs",
        ));
    }
    Ok(())
}

fn validate_foundation_binding(
    store: &Store,
    transaction: &Transaction<'_>,
    aggregate: &FoundationAuthoringMeshV2MaterializationRecord,
    require_reachable: bool,
) -> Result<Vec<String>, StoreError> {
    let Some(foundation) = foundation_record_json(transaction, &aggregate.foundation_request_id)?
    else {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_FOUNDATION_UNAVAILABLE",
            "foundation import request is not durably materialized",
        ));
    };
    validate_foundation_record(transaction, &foundation)?;
    if foundation.request_sha256 != aggregate.foundation_request_sha256
        || foundation.result_object_sha256 != aggregate.foundation_result_object_sha256
        || foundation.topology_object_sha256 != aggregate.foundation_topology_object_sha256
        || foundation.socket_map_object_sha256 != aggregate.foundation_socket_map_object_sha256
        || foundation.rig_map_object_sha256 != aggregate.foundation_rig_map_object_sha256
        || foundation.fps_presentation_package_object_sha256
            != aggregate.foundation_fps_presentation_package_object_sha256
    {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_FOUNDATION_BINDING_MISMATCH",
            "aggregate foundation hashes differ from WeaponFoundationImport",
        ));
    }
    let expected_roots = [
        (
            foundation.topology_object_sha256.as_str(),
            super::weapon_foundation_import::TOPOLOGY_MIME,
            super::weapon_foundation_import::TOPOLOGY_OBJECT_KIND,
            super::weapon_foundation_import::MAX_TOPOLOGY_BYTES,
        ),
        (
            foundation.socket_map_object_sha256.as_str(),
            super::weapon_foundation_import::JSON_MIME,
            super::weapon_foundation_import::SOCKET_MAP_OBJECT_KIND,
            super::weapon_foundation_import::MAX_JSON_BYTES,
        ),
        (
            foundation.rig_map_object_sha256.as_str(),
            super::weapon_foundation_import::JSON_MIME,
            super::weapon_foundation_import::RIG_MAP_OBJECT_KIND,
            super::weapon_foundation_import::MAX_JSON_BYTES,
        ),
        (
            foundation.fps_presentation_package_object_sha256.as_str(),
            super::weapon_foundation_import::JSON_MIME,
            super::weapon_foundation_import::PRESENTATION_PACKAGE_OBJECT_KIND,
            super::weapon_foundation_import::MAX_JSON_BYTES,
        ),
        (
            foundation.result_object_sha256.as_str(),
            super::weapon_foundation_import::JSON_MIME,
            super::weapon_foundation_import::RESULT_OBJECT_KIND,
            super::weapon_foundation_import::MAX_JSON_BYTES,
        ),
        (
            foundation.link_object_sha256.as_str(),
            super::weapon_foundation_import::JSON_MIME,
            super::weapon_foundation_import::LINK_OBJECT_KIND,
            super::weapon_foundation_import::MAX_JSON_BYTES,
        ),
    ];
    let mut roots = Vec::with_capacity(expected_roots.len());
    let mut result_bytes = None;
    let mut link_bytes = None;
    for (hash, mime, kind, max_bytes) in expected_roots {
        let bytes = validate_foundation_root(
            store,
            transaction,
            hash,
            mime,
            kind,
            max_bytes,
            require_reachable,
        )?;
        if hash == foundation.result_object_sha256 {
            result_bytes = Some(bytes.clone());
        }
        if hash == foundation.link_object_sha256 {
            link_bytes = Some(bytes);
        }
        roots.push(hash.to_owned());
    }
    validate_foundation_result_or_link(
        &result_bytes.ok_or_else(|| {
            contract(
                "FOUNDATION_AUTHORING_MESH_V2_FOUNDATION_RESULT_MISSING",
                "foundation result root is missing",
            )
        })?,
        &foundation,
        true,
    )?;
    validate_foundation_result_or_link(
        &link_bytes.ok_or_else(|| {
            contract(
                "FOUNDATION_AUTHORING_MESH_V2_FOUNDATION_LINK_MISSING",
                "foundation link root is missing",
            )
        })?,
        &foundation,
        false,
    )?;
    Ok(roots)
}

fn revision_counts(revision: &AuthoringMeshRevision) -> (u64, u64) {
    (
        revision.original.vertices.len() as u64,
        revision.original.faces.len() as u64,
    )
}

fn validate_revision_input(
    store: &Store,
    aggregate: &FoundationAuthoringMeshV2MaterializationRecord,
    item: &FoundationAuthoringMeshV2RevisionInput,
    part: &FoundationAuthoringMeshV2PartRevision,
) -> Result<(), StoreError> {
    if item.part_id != part.part_id
        || item.record.project_id != aggregate.project_id
        || item.record.revision_index != 0
        || !item.record.parent_revision_ids.is_empty()
        || item.record.operation_id.is_some()
        || item.record.operation_kind.is_some()
        || item.record.operation_lineage_sha256.is_some()
        || item.record.revision_object_sha256 != item.object.sha256
        || item.record.mesh_id != part.mesh_id
        || item.record.lineage_id != part.lineage_id
        || item.record.revision_id != part.revision_id
        || item.record.idempotency_key != part.idempotency_key
        || item.record.revision_object_sha256 != part.revision_object_sha256
        || item.record.revision_sha256 != part.revision_sha256
    {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_REVISION_BINDING_MISMATCH",
            "Part revision input differs from the descriptor or aggregate project",
        ));
    }
    let (vertices, faces) = revision_counts(&item.revision);
    if vertices != part.vertex_count || faces != part.face_count {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_REVISION_TOTALS_MISMATCH",
            "Part revision topology totals differ from the descriptor",
        ));
    }
    let bytes = validate_foundation_revision_object(
        store,
        &item.object,
        &item.record.revision_object_sha256,
        false,
    )?;
    super::validate_authoring_mesh_v2_revision_payload(&bytes, &item.revision, &item.record)?;
    Ok(())
}

fn revision_object_hashes(
    descriptor: &FoundationAuthoringMeshV2MaterializationDescriptor,
) -> Result<String, StoreError> {
    let hashes = descriptor
        .part_revisions
        .iter()
        .map(|part| part.revision_object_sha256.clone())
        .collect::<Vec<_>>();
    canonical_string(&hashes)
}

fn read_aggregate(
    transaction: &Transaction<'_>,
    project_id: &str,
    idempotency_key: &str,
) -> Result<Option<FoundationAuthoringMeshV2MaterializationRecord>, StoreError> {
    let payload: Option<String> = transaction
        .query_row(
            "SELECT record_json FROM foundation_authoring_mesh_v2_materializations WHERE project_id = ?1 AND idempotency_key = ?2",
            params![project_id, idempotency_key],
            |row| row.get(0),
        )
        .optional()?;
    let Some(payload) = payload else {
        return Ok(None);
    };
    serde_json::from_str(&payload)
        .map(Some)
        .map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn read_descriptor_object(
    store: &Store,
    transaction: &Transaction<'_>,
    aggregate: &FoundationAuthoringMeshV2MaterializationRecord,
    require_reachable: bool,
) -> Result<FoundationAuthoringMeshV2MaterializationDescriptor, StoreError> {
    let object = transaction
        .query_row(
            "SELECT sha256, size_bytes, mime, kind, reachability, created_at FROM objects WHERE sha256 = ?1",
            params![aggregate.descriptor_object_sha256],
            |row| {
                let size_bytes: i64 = row.get(1)?;
                Ok(CasObjectRecord {
                    schema_version: "CasObject@1".to_owned(),
                    sha256: row.get(0)?,
                    size_bytes: u64::try_from(size_bytes).map_err(|_| {
                        rusqlite::Error::FromSqlConversionFailure(
                            1,
                            rusqlite::types::Type::Integer,
                            "negative descriptor size".into(),
                        )
                    })?,
                    mime: row.get(2)?,
                    kind: row.get(3)?,
                    reachability: row.get(4)?,
                    created_at: row.get(5)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| {
            contract(
                "FOUNDATION_AUTHORING_MESH_V2_DESCRIPTOR_CAS_OBJECT_UNAVAILABLE",
                "descriptor CAS object is unavailable",
            )
        })?;
    validate_descriptor_cas(
        store,
        transaction,
        &object,
        &aggregate.descriptor_object_sha256,
        require_reachable,
    )
}

fn validate_foundation_revision_record_in_transaction(
    transaction: &Transaction<'_>,
    store: &Store,
    record: &AuthoringMeshV2DurableRecord,
    require_reachable: bool,
) -> Result<AuthoringMeshRevision, StoreError> {
    let object = transaction
        .query_row(
            "SELECT sha256, size_bytes, mime, kind, reachability, created_at FROM objects WHERE sha256 = ?1",
            params![record.revision_object_sha256],
            |row| {
                let size_bytes: i64 = row.get(1)?;
                Ok(CasObjectRecord {
                    schema_version: "CasObject@1".to_owned(),
                    sha256: row.get(0)?,
                    size_bytes: u64::try_from(size_bytes).map_err(|_| {
                        rusqlite::Error::FromSqlConversionFailure(
                            1,
                            rusqlite::types::Type::Integer,
                            "negative revision size".into(),
                        )
                    })?,
                    mime: row.get(2)?,
                    kind: row.get(3)?,
                    reachability: row.get(4)?,
                    created_at: row.get(5)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| {
            contract(
                "FOUNDATION_AUTHORING_MESH_V2_REVISION_UNAVAILABLE",
                "descriptor-listed AuthoringMesh@2 revision object is unavailable",
            )
        })?;
    let bytes = validate_foundation_revision_object(
        store,
        &object,
        &record.revision_object_sha256,
        require_reachable,
    )?;
    let revision: AuthoringMeshRevision = serde_json::from_slice(&bytes)
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    super::validate_authoring_mesh_v2_revision_payload(&bytes, &revision, record)?;
    Ok(revision)
}

fn validate_listed_revisions(
    transaction: &Transaction<'_>,
    store: &Store,
    aggregate: &FoundationAuthoringMeshV2MaterializationRecord,
    descriptor: &FoundationAuthoringMeshV2MaterializationDescriptor,
    require_reachable: bool,
) -> Result<(), StoreError> {
    for part in &descriptor.part_revisions {
        let Some(record) = super::read_authoring_mesh_v2_record_in_transaction(
            transaction,
            &aggregate.project_id,
            &part.idempotency_key,
        )?
        else {
            return Err(contract(
                "FOUNDATION_AUTHORING_MESH_V2_REVISION_UNAVAILABLE",
                "descriptor-listed AuthoringMesh@2 revision row is unavailable",
            ));
        };
        let record = super::normalize_authoring_mesh_v2_durable_record(&record)?;
        if record.mesh_id != part.mesh_id
            || record.lineage_id != part.lineage_id
            || record.revision_id != part.revision_id
            || record.revision_object_sha256 != part.revision_object_sha256
            || record.revision_sha256 != part.revision_sha256
        {
            return Err(contract(
                "FOUNDATION_AUTHORING_MESH_V2_REVISION_BINDING_MISMATCH",
                "descriptor-listed AuthoringMesh@2 row differs",
            ));
        }
        super::validate_authoring_mesh_v2_parent_dag_in_transaction(transaction, &record)?;
        let revision = validate_foundation_revision_record_in_transaction(
            transaction,
            store,
            &record,
            require_reachable,
        )?;
        let (vertices, faces) = revision_counts(&revision);
        if vertices != part.vertex_count || faces != part.face_count {
            return Err(contract(
                "FOUNDATION_AUTHORING_MESH_V2_REVISION_TOTALS_MISMATCH",
                "descriptor-listed revision totals differ from CAS payload",
            ));
        }
    }
    Ok(())
}

fn ensure_project(transaction: &Transaction<'_>, project_id: &str) -> Result<(), StoreError> {
    let exists: Option<String> = transaction
        .query_row(
            "SELECT project_id FROM projects WHERE project_id = ?1",
            params![project_id],
            |row| row.get(0),
        )
        .optional()?;
    if exists.is_none() {
        return Err(contract(
            "PROJECT_SCOPE_DENIED",
            "materialization project does not exist",
        ));
    }
    Ok(())
}

fn insert_revision(
    transaction: &Transaction<'_>,
    record: &AuthoringMeshV2DurableRecord,
) -> Result<(), StoreError> {
    let parent_revision_ids_json = canonical_string(&record.parent_revision_ids)?;
    transaction.execute(
        "INSERT INTO authoring_mesh_v2_durable_records (schema_version, project_id, mesh_id, lineage_id, revision_id, parent_revision_ids_json, revision_index, revision_object_sha256, revision_sha256, operation_id, operation_kind, operation_lineage_sha256, request_input_sha256, idempotency_key, materialization_status, canonical_sha256, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
        params![
            record.schema_version,
            record.project_id,
            record.mesh_id,
            record.lineage_id,
            record.revision_id,
            parent_revision_ids_json,
            i64::try_from(record.revision_index).map_err(|_| {
                StoreError::InvalidData("AuthoringMesh@2 revision index is too large".to_owned())
            })?,
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
    Ok(())
}

fn validate_existing_revision_or_insert(
    transaction: &Transaction<'_>,
    store: &Store,
    item: &FoundationAuthoringMeshV2RevisionInput,
) -> Result<(), StoreError> {
    super::ensure_authoring_mesh_v2_object_row(transaction, &item.object, false)?;
    super::validate_authoring_mesh_v2_parent_dag_in_transaction(transaction, &item.record)?;
    let existing = super::read_authoring_mesh_v2_record_in_transaction(
        transaction,
        &item.record.project_id,
        &item.record.idempotency_key,
    )?;
    if let Some(existing) = existing {
        let existing = super::normalize_authoring_mesh_v2_durable_record(&existing)?;
        validate_foundation_revision_record_in_transaction(transaction, store, &existing, false)?;
        if !super::same_authoring_mesh_v2_durable_record(&existing, &item.record) {
            return Err(contract(
                "FOUNDATION_AUTHORING_MESH_V2_REVISION_CONFLICT",
                "revision idempotency key is already bound to different metadata",
            ));
        }
        return Ok(());
    }
    let duplicate_revision: Option<String> = transaction
        .query_row(
            "SELECT idempotency_key FROM authoring_mesh_v2_durable_records WHERE project_id = ?1 AND revision_id = ?2",
            params![item.record.project_id, item.record.revision_id],
            |row| row.get(0),
        )
        .optional()?;
    if duplicate_revision.is_some() {
        return Err(contract(
            "FOUNDATION_AUTHORING_MESH_V2_REVISION_CONFLICT",
            "revision_id is already bound to another idempotency key",
        ));
    }
    insert_revision(transaction, &item.record)
}

fn materialization_roots(
    record: &FoundationAuthoringMeshV2MaterializationRecord,
    descriptor: &FoundationAuthoringMeshV2MaterializationDescriptor,
    foundation_roots: &[String],
) -> Vec<String> {
    let mut roots = foundation_roots.to_vec();
    roots.push(record.descriptor_object_sha256.clone());
    roots.extend(
        descriptor
            .part_revisions
            .iter()
            .map(|part| part.revision_object_sha256.clone()),
    );
    roots.sort();
    roots.dedup();
    roots
}

impl Store {
    /// Atomically materialize every descriptor-listed Part revision and the
    /// aggregate row.  All CAS, descriptor, foundation, and revision checks
    /// happen before any insert; the transaction is rolled back on every
    /// failure, including a single Part or root promotion failure.
    pub fn record_foundation_authoring_mesh_v2_materialization_with_replay(
        &self,
        batch: &FoundationAuthoringMeshV2MaterializationBatch,
    ) -> Result<(FoundationAuthoringMeshV2MaterializationRecord, bool), StoreError> {
        validate_record(&batch.record)?;
        if batch.descriptor_object.sha256 != batch.record.descriptor_object_sha256 {
            return Err(contract(
                "FOUNDATION_AUTHORING_MESH_V2_DESCRIPTOR_OBJECT_BINDING_MISMATCH",
                "descriptor object hash differs from aggregate binding",
            ));
        }
        validate_descriptor_binding(&batch.record, &batch.descriptor)?;
        let descriptor_bytes = canonical_json(&batch.descriptor)?;
        if descriptor_bytes.len() as u64 > MAX_JSON_BYTES
            || descriptor_bytes.len() as u64 != batch.descriptor_object.size_bytes
            || sha256_hex(&descriptor_bytes) != batch.descriptor_object.sha256
        {
            return Err(contract(
                "FOUNDATION_AUTHORING_MESH_V2_DESCRIPTOR_CAS_HASH_MISMATCH",
                "descriptor metadata does not match its typed canonical bytes",
            ));
        }
        let mut input_by_part = BTreeMap::new();
        if batch.revisions.len() != batch.descriptor.part_revisions.len() {
            return Err(contract(
                "FOUNDATION_AUTHORING_MESH_V2_PART_SET_MISMATCH",
                "batch revision count differs from descriptor Part count",
            ));
        }
        for item in &batch.revisions {
            if !is_opaque_id(&item.part_id)
                || input_by_part.insert(item.part_id.clone(), item).is_some()
            {
                return Err(contract(
                    "FOUNDATION_AUTHORING_MESH_V2_PART_SET_INVALID",
                    "batch contains a duplicate or malformed Part id",
                ));
            }
        }
        for part in &batch.descriptor.part_revisions {
            let item = input_by_part.get(&part.part_id).ok_or_else(|| {
                contract(
                    "FOUNDATION_AUTHORING_MESH_V2_PART_SET_MISMATCH",
                    "descriptor Part has no revision input",
                )
            })?;
            validate_revision_input(self, &batch.record, item, part)?;
        }

        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        ensure_table(&transaction)?;
        super::weapon_foundation_import::ensure_table(&transaction)?;
        ensure_project(&transaction, &batch.record.project_id)?;

        let foundation_roots =
            validate_foundation_binding(self, &transaction, &batch.record, true)?;
        let descriptor = validate_descriptor_cas(
            self,
            &transaction,
            &batch.descriptor_object,
            &batch.record.descriptor_object_sha256,
            false,
        )?;
        validate_descriptor_binding(&batch.record, &descriptor)?;
        if descriptor != batch.descriptor {
            return Err(contract(
                "FOUNDATION_AUTHORING_MESH_V2_DESCRIPTOR_CAS_PAYLOAD_MISMATCH",
                "descriptor typed input differs from its CAS payload",
            ));
        }
        ensure_descriptor_object_row(&transaction, &batch.descriptor_object, false)?;

        let existing = read_aggregate(
            &transaction,
            &batch.record.project_id,
            &batch.record.idempotency_key,
        )?;
        if let Some(existing) = existing {
            validate_record(&existing)?;
            if !same_record(&existing, &batch.record) {
                return Err(contract(
                    "FOUNDATION_AUTHORING_MESH_V2_MATERIALIZATION_CONFLICT",
                    "project and idempotency key are already bound to different materialization metadata",
                ));
            }
            let existing_descriptor = read_descriptor_object(self, &transaction, &existing, true)?;
            validate_descriptor_binding(&existing, &existing_descriptor)?;
            validate_listed_revisions(&transaction, self, &existing, &existing_descriptor, true)?;
            let roots = materialization_roots(&existing, &existing_descriptor, &foundation_roots);
            super::mark_reachable_in_transaction(&transaction, &roots)?;
            transaction.commit()?;
            return Ok((existing, true));
        }

        let duplicate_foundation: Option<String> = transaction
            .query_row(
                "SELECT idempotency_key FROM foundation_authoring_mesh_v2_materializations WHERE project_id = ?1 AND foundation_request_id = ?2",
                params![batch.record.project_id, batch.record.foundation_request_id],
                |row| row.get(0),
            )
            .optional()?;
        if duplicate_foundation.is_some() {
            return Err(contract(
                "FOUNDATION_AUTHORING_MESH_V2_MATERIALIZATION_CONFLICT",
                "foundation request is already bound to another materialization idempotency key",
            ));
        }

        // Every revision is checked and inserted in this same transaction.
        // Descriptor order is deterministic, which also makes the root list
        // and replay readback deterministic.
        for part in &batch.descriptor.part_revisions {
            let item = input_by_part.get(&part.part_id).ok_or_else(|| {
                contract(
                    "FOUNDATION_AUTHORING_MESH_V2_PART_SET_MISMATCH",
                    "descriptor Part has no revision input",
                )
            })?;
            validate_existing_revision_or_insert(&transaction, self, item)?;
        }
        let roots = materialization_roots(&batch.record, &batch.descriptor, &foundation_roots);
        super::mark_reachable_in_transaction(&transaction, &roots)?;
        let roots_json = revision_object_hashes(&batch.descriptor)?;
        let payload = canonical_string(&batch.record)?;
        if payload.len() as u64 > MAX_JSON_BYTES {
            return Err(contract(
                "FOUNDATION_AUTHORING_MESH_V2_RECORD_TOO_LARGE",
                "materialization aggregate JSON exceeds the bounded limit",
            ));
        }
        transaction.execute(
            "INSERT INTO foundation_authoring_mesh_v2_materializations (schema_version, project_id, idempotency_key, foundation_request_id, foundation_request_sha256, foundation_result_object_sha256, foundation_topology_object_sha256, foundation_socket_map_object_sha256, foundation_rig_map_object_sha256, foundation_fps_presentation_package_object_sha256, descriptor_object_sha256, descriptor_canonical_sha256, part_revision_summary_sha256, part_count, vertex_count, face_count, status, canonical_sha256, created_at, part_revision_object_sha256s_json, record_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
            params![
                batch.record.schema_version,
                batch.record.project_id,
                batch.record.idempotency_key,
                batch.record.foundation_request_id,
                batch.record.foundation_request_sha256,
                batch.record.foundation_result_object_sha256,
                batch.record.foundation_topology_object_sha256,
                batch.record.foundation_socket_map_object_sha256,
                batch.record.foundation_rig_map_object_sha256,
                batch.record.foundation_fps_presentation_package_object_sha256,
                batch.record.descriptor_object_sha256,
                batch.record.descriptor_canonical_sha256,
                batch.record.part_revision_summary_sha256,
                i64::try_from(batch.record.part_count).map_err(|_| StoreError::InvalidData("materialization Part count is too large".to_owned()))?,
                i64::try_from(batch.record.vertex_count).map_err(|_| StoreError::InvalidData("materialization vertex count is too large".to_owned()))?,
                i64::try_from(batch.record.face_count).map_err(|_| StoreError::InvalidData("materialization face count is too large".to_owned()))?,
                batch.record.status,
                batch.record.canonical_sha256,
                batch.record.created_at,
                roots_json,
                payload,
            ],
        )?;
        let stored = read_aggregate(
            &transaction,
            &batch.record.project_id,
            &batch.record.idempotency_key,
        )?
        .ok_or_else(|| {
            contract(
                "FOUNDATION_AUTHORING_MESH_V2_RESTART_READBACK_FAILED",
                "materialization aggregate disappeared before commit",
            )
        })?;
        validate_record(&stored)?;
        let stored_descriptor = read_descriptor_object(self, &transaction, &stored, true)?;
        validate_descriptor_binding(&stored, &stored_descriptor)?;
        validate_listed_revisions(&transaction, self, &stored, &stored_descriptor, true)?;
        transaction.commit()?;
        Ok((stored, false))
    }

    pub fn commit_foundation_authoring_mesh_v2_materialization_with_replay(
        &self,
        batch: &FoundationAuthoringMeshV2MaterializationBatch,
    ) -> Result<(FoundationAuthoringMeshV2MaterializationRecord, bool), StoreError> {
        self.record_foundation_authoring_mesh_v2_materialization_with_replay(batch)
    }

    pub fn record_foundation_authoring_mesh_v2_materialization(
        &self,
        batch: &FoundationAuthoringMeshV2MaterializationBatch,
    ) -> Result<FoundationAuthoringMeshV2MaterializationRecord, StoreError> {
        self.record_foundation_authoring_mesh_v2_materialization_with_replay(batch)
            .map(|(record, _)| record)
    }

    pub fn get_foundation_authoring_mesh_v2_materialization(
        &self,
        project_id: &str,
        idempotency_key: &str,
    ) -> Result<Option<FoundationAuthoringMeshV2MaterializationRecord>, StoreError> {
        if !is_opaque_id(project_id) || !is_opaque_id(idempotency_key) {
            return Err(StoreError::InvalidData(
                "foundation AuthoringMesh@2 materialization lookup identity is invalid".to_owned(),
            ));
        }
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        ensure_table(&transaction)?;
        let Some(record) = read_aggregate(&transaction, project_id, idempotency_key)? else {
            transaction.rollback()?;
            return Ok(None);
        };
        validate_record(&record)?;
        let foundation_roots = validate_foundation_binding(self, &transaction, &record, true)?;
        let descriptor = read_descriptor_object(self, &transaction, &record, true)?;
        validate_descriptor_binding(&record, &descriptor)?;
        validate_listed_revisions(&transaction, self, &record, &descriptor, true)?;
        let roots = materialization_roots(&record, &descriptor, &foundation_roots);
        let roots_json = revision_object_hashes(&descriptor)?;
        let stored_roots_json: String = transaction.query_row(
            "SELECT part_revision_object_sha256s_json FROM foundation_authoring_mesh_v2_materializations WHERE project_id = ?1 AND idempotency_key = ?2",
            params![project_id, idempotency_key],
            |row| row.get(0),
        )?;
        if stored_roots_json != roots_json {
            return Err(contract(
                "FOUNDATION_AUTHORING_MESH_V2_ROOT_SET_MISMATCH",
                "aggregate revision root index differs from its descriptor",
            ));
        }
        let _ = roots;
        transaction.rollback()?;
        Ok(Some(record))
    }

    pub fn get_foundation_authoring_mesh_v2_materialization_by_descriptor(
        &self,
        descriptor_object_sha256: &str,
    ) -> Result<Option<FoundationAuthoringMeshV2MaterializationRecord>, StoreError> {
        if !is_sha256(descriptor_object_sha256) {
            return Err(StoreError::InvalidData(
                "foundation AuthoringMesh@2 descriptor hash is invalid".to_owned(),
            ));
        }
        let connection = self.lock_connection()?;
        let identity: Option<(String, String)> = connection
            .query_row(
                "SELECT project_id, idempotency_key FROM foundation_authoring_mesh_v2_materializations WHERE descriptor_object_sha256 = ?1",
                params![descriptor_object_sha256],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        drop(connection);
        let Some((project_id, idempotency_key)) = identity else {
            return Ok(None);
        };
        self.get_foundation_authoring_mesh_v2_materialization(&project_id, &idempotency_key)
    }
}
