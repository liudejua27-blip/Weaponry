//! Durable Store/CAS boundary for `WeaponryKnifeProductionBrief@1`.
//!
//! Runtime owns validation and all writes.  This module only accepts the
//! already validated, canonical JSON envelope and a registered CAS object,
//! then verifies the bytes again before installing one immutable SQLite row.
//! The brief contains references by SHA-256 only; this record never stores an
//! image, local path, contact, signature or other source PII.

use super::{
    canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256,
    mark_reachable_in_transaction, CasObjectRecord, CasStore, Store, StoreError,
};
use forgecad_core::sha256_hex;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub const WEAPONRY_KNIFE_PRODUCTION_BRIEF_RECORD_SCHEMA_VERSION: &str =
    "WeaponryKnifeProductionBriefStoreRecord@1";
pub const WEAPONRY_KNIFE_PRODUCTION_BRIEF_STATUS: &str =
    "runtime-owned-store-weaponry-knife-production-brief@1";
pub const WEAPONRY_KNIFE_PRODUCTION_BRIEF_OBJECT_KIND: &str = "weaponry-knife-production-brief";
pub const WEAPONRY_KNIFE_PRODUCTION_BRIEF_JSON_MIME: &str = "application/json";
// Keep the durable payload below the public Runtime/MCP response ceiling so
// every accepted Brief can be returned through the same readback surface.
pub const WEAPONRY_KNIFE_PRODUCTION_BRIEF_MAX_JSON_BYTES: u64 = 1024 * 1024;
const MAX_SOURCE_REFERENCE_HASHES: usize = 64;
const MAX_STATUS_BYTES: usize = 128;

/// Store-local immutable index for one Runtime-verified production brief.
///
/// `brief_object_sha256` is the content hash of the canonical JSON bytes in
/// CAS.  `brief_canonical_sha256` is the semantic hash carried by the brief
/// payload itself (the payload's `canonical_sha256` with that field blanked).
/// The two hashes intentionally remain distinct, matching the rest of the
/// Store/CAS lineage model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaponryKnifeProductionBriefStoreRecord {
    pub schema_version: String,
    pub project_id: String,
    pub brief_id: String,
    pub brief_object_sha256: String,
    pub brief_canonical_sha256: String,
    /// The Runtime-bound ReferenceEvidence identity for the source image.
    pub reference_id: String,
    /// The source image CAS object hash.  The source object remains owned by
    /// `reference_evidence`; this row keeps the exact lineage visible.
    pub reference_object_sha256: String,
    /// The canonical hash of the `ReferenceEvidence@1` record.
    pub reference_evidence_sha256: String,
    /// Optional immutable parent link for a successor intake.  Initial
    /// intake records must keep both parent fields null.
    pub parent_brief_id: Option<String>,
    pub parent_brief_sha256: Option<String>,
    /// Frozen lineage policy: initial records cannot have a parent; successor
    /// records preserve the parent's source claims without replacement.
    pub freeze_policy: String,
    pub source_reference_hashes: Vec<String>,
    /// Runtime-derived intake status.  Store preserves blocked/conflicted
    /// truth and does not use this field to authorize modeling.
    pub status: String,
    /// Runtime-derived conflict freeze state.  Store persists this state but
    /// never decides whether an unresolved brief may enter authoring.
    pub conflict_freeze_state: String,
    pub idempotency_key: String,
    pub created_at: String,
}

/// The CAS object staged by Runtime before this Store transaction.
#[derive(Debug, Clone)]
pub struct WeaponryKnifeProductionBriefCasBundle {
    pub brief: CasObjectRecord,
}

#[derive(Debug, Clone)]
pub struct WeaponryKnifeProductionBriefCommit {
    pub record: WeaponryKnifeProductionBriefStoreRecord,
    pub cas: WeaponryKnifeProductionBriefCasBundle,
}

fn contract(code: &str, message: impl Into<String>) -> StoreError {
    StoreError::Contract {
        code: code.to_owned(),
        message: message.into(),
    }
}

fn record_value(record: &WeaponryKnifeProductionBriefStoreRecord) -> Result<Value, StoreError> {
    serde_json::to_value(record).map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn record_bytes(record: &WeaponryKnifeProductionBriefStoreRecord) -> Result<Vec<u8>, StoreError> {
    canonical_json_bytes(&record_value(record)?)
        .map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn safe_status(value: &str) -> bool {
    value.len() <= MAX_STATUS_BYTES
        && !value.is_empty()
        && is_opaque_id(value)
        && !value.contains("path")
        && !value.to_ascii_lowercase().contains("secret")
        && !value.to_ascii_lowercase().contains("token")
}

fn validate_record(record: &WeaponryKnifeProductionBriefStoreRecord) -> Result<(), StoreError> {
    if record.schema_version != WEAPONRY_KNIFE_PRODUCTION_BRIEF_RECORD_SCHEMA_VERSION
        || !is_opaque_id(&record.project_id)
        || !is_opaque_id(&record.brief_id)
        || !is_sha256(&record.brief_object_sha256)
        || !is_sha256(&record.brief_canonical_sha256)
        || !is_opaque_id(&record.reference_id)
        || !is_sha256(&record.reference_object_sha256)
        || !is_sha256(&record.reference_evidence_sha256)
        || record
            .parent_brief_id
            .as_deref()
            .is_some_and(|value| !is_opaque_id(value) || value == record.brief_id)
        || record
            .parent_brief_sha256
            .as_deref()
            .is_some_and(|value| !is_sha256(value) || value == record.brief_canonical_sha256)
        || !is_opaque_id(&record.idempotency_key)
        || record.idempotency_key.len() > 128
        || !safe_status(&record.status)
        || !safe_status(&record.conflict_freeze_state)
        || record.created_at.is_empty()
        || record.created_at.len() > 64
        || record.created_at.contains('/')
        || record.created_at.contains('\\')
        || record.source_reference_hashes.is_empty()
        || record.source_reference_hashes.len() > MAX_SOURCE_REFERENCE_HASHES
        || record
            .source_reference_hashes
            .iter()
            .any(|hash| !is_sha256(hash))
        || record
            .source_reference_hashes
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || !matches!(
            (
                &record.parent_brief_id,
                &record.parent_brief_sha256,
                record.freeze_policy.as_str()
            ),
            (None, None, "initial-intake-no-parent@1")
                | (
                    Some(_),
                    Some(_),
                    "immutable-successor-preserve-source-claims@1"
                )
        )
    {
        return Err(contract(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_RECORD_INVALID",
            "brief durable identity, status, timestamp or source hash list is malformed",
        ));
    }
    Ok(())
}

fn validate_no_forbidden_payload(value: &Value) -> Result<(), StoreError> {
    const FORBIDDEN_KEYS: &[&str] = &[
        "api_key",
        "bytes",
        "contact",
        "contacts",
        "email",
        "file_path",
        "image_bytes",
        "local_path",
        "password",
        "path",
        "phone",
        "pii",
        "raw_image",
        "secret",
        "signature",
        "signed_by",
        "token",
        "url",
    ];
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if FORBIDDEN_KEYS.contains(&key.to_ascii_lowercase().as_str()) {
                    return Err(contract(
                        "WEAPONRY_KNIFE_PRODUCTION_BRIEF_FORBIDDEN_PAYLOAD",
                        "brief payload contains a forbidden image/path/contact/signature field",
                    ));
                }
                validate_no_forbidden_payload(child)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                validate_no_forbidden_payload(child)?;
            }
        }
        Value::String(text) => {
            let lower = text.to_ascii_lowercase();
            let contains_secret_assignment = ["api key", "secret", "password", "token"]
                .iter()
                .any(|name| lower.contains(&format!("{name}:")));
            if text.chars().any(char::is_control)
                || text.starts_with('/')
                || text.starts_with('\\')
                || lower.starts_with("file:")
                || lower.starts_with("data:")
                || lower.starts_with("http:")
                || lower.starts_with("https:")
                || lower.starts_with("ftp:")
                || contains_secret_assignment
            {
                return Err(contract(
                    "WEAPONRY_KNIFE_PRODUCTION_BRIEF_FORBIDDEN_PAYLOAD",
                    "brief payload contains a path, URL, secret or control character",
                ));
            }
        }
        _ => {}
    }
    Ok(())
}

fn source_reference_hashes_in_payload(value: &Value) -> BTreeSet<String> {
    fn visit(value: &Value, hashes: &mut BTreeSet<String>) {
        match value {
            Value::Object(object) => {
                for (key, child) in object {
                    if key == "source_reference_sha256" {
                        if let Some(hash) = child.as_str().filter(|hash| is_sha256(hash)) {
                            hashes.insert(hash.to_owned());
                        }
                    }
                    visit(child, hashes);
                }
            }
            Value::Array(values) => values.iter().for_each(|child| visit(child, hashes)),
            _ => {}
        }
    }
    let mut hashes = BTreeSet::new();
    visit(value, &mut hashes);
    hashes
}

fn validate_brief_payload(
    record: &WeaponryKnifeProductionBriefStoreRecord,
    bytes: &[u8],
) -> Result<Value, StoreError> {
    if bytes.is_empty() || bytes.len() as u64 > WEAPONRY_KNIFE_PRODUCTION_BRIEF_MAX_JSON_BYTES {
        return Err(contract(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_BYTES_INVALID",
            "brief CAS bytes are empty or exceed the bounded capacity",
        ));
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        contract(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_JSON_INVALID",
            format!("brief CAS object is not valid JSON: {error}"),
        )
    })?;
    if !value.is_object() {
        return Err(contract(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_JSON_INVALID",
            "brief CAS payload must be a JSON object",
        ));
    }
    validate_no_forbidden_payload(&value)?;
    let canonical =
        canonical_json_bytes(&value).map_err(|error| StoreError::InvalidData(error.to_string()))?;
    if canonical != bytes {
        return Err(contract(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_NOT_CANONICAL",
            "brief CAS JSON bytes are not canonical",
        ));
    }
    let payload_parent_brief_id = match value.get("parent_brief_id") {
        Some(Value::Null) => None,
        Some(Value::String(value)) => Some(value.clone()),
        _ => {
            return Err(contract(
                "WEAPONRY_KNIFE_PRODUCTION_BRIEF_PARENT_BINDING_INVALID",
                "brief parent_brief_id must be an explicit string or null",
            ));
        }
    };
    let payload_parent_brief_sha256 = match value.get("parent_brief_sha256") {
        Some(Value::Null) => None,
        Some(Value::String(value)) => Some(value.clone()),
        _ => {
            return Err(contract(
                "WEAPONRY_KNIFE_PRODUCTION_BRIEF_PARENT_BINDING_INVALID",
                "brief parent_brief_sha256 must be an explicit hash or null",
            ));
        }
    };
    if value.get("schema_version").and_then(Value::as_str) != Some("WeaponryKnifeProductionBrief@1")
        || value.get("brief_id").and_then(Value::as_str) != Some(record.brief_id.as_str())
        || value.get("project_id").and_then(Value::as_str) != Some(record.project_id.as_str())
        || value.get("canonicalization_policy").and_then(Value::as_str)
            != Some("canonical-json-sha256-excluding-canonical-sha256@1")
        || payload_parent_brief_id != record.parent_brief_id
        || payload_parent_brief_sha256 != record.parent_brief_sha256
        || value.get("freeze_policy").and_then(Value::as_str) != Some(record.freeze_policy.as_str())
    {
        return Err(contract(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_BINDING_MISMATCH",
            "brief payload identity or canonicalization policy differs from Store input",
        ));
    }
    let payload_canonical = value
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| {
            contract(
                "WEAPONRY_KNIFE_PRODUCTION_BRIEF_CANONICAL_MISMATCH",
                "brief payload canonical_sha256 is missing or malformed",
            )
        })?;
    let mut preimage = value.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != payload_canonical
        || payload_canonical != record.brief_canonical_sha256
    {
        return Err(contract(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_CANONICAL_MISMATCH",
            "brief payload canonical hash differs from its verified Store binding",
        ));
    }
    let payload_refs = source_reference_hashes_in_payload(&value);
    if payload_refs
        .iter()
        .any(|hash| !record.source_reference_hashes.binary_search(hash).is_ok())
    {
        return Err(contract(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_SOURCE_BINDING_MISMATCH",
            "brief payload source reference hash is absent from the Store binding",
        ));
    }
    Ok(value)
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
        .map_err(StoreError::from)
}

fn valid_cas_created_at(created_at: &str) -> bool {
    !created_at.is_empty() && created_at.len() <= 128
}

fn validate_registered_object(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    supplied: &CasObjectRecord,
    expected_sha256: &str,
    require_reachable: bool,
) -> Result<Vec<u8>, StoreError> {
    if supplied.schema_version != "CasObject@1"
        || supplied.sha256 != expected_sha256
        || !is_sha256(expected_sha256)
        || supplied.mime != WEAPONRY_KNIFE_PRODUCTION_BRIEF_JSON_MIME
        || supplied.kind != WEAPONRY_KNIFE_PRODUCTION_BRIEF_OBJECT_KIND
        || supplied.size_bytes == 0
        || supplied.size_bytes > WEAPONRY_KNIFE_PRODUCTION_BRIEF_MAX_JSON_BYTES
        || !matches!(supplied.reachability.as_str(), "temporary" | "reachable")
        || (require_reachable && supplied.reachability != "reachable")
        || !valid_cas_created_at(&supplied.created_at)
    {
        return Err(contract(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_CAS_METADATA_INVALID",
            "brief CAS metadata is outside the bounded allowlist",
        ));
    }
    let registered =
        read_object_record(transaction, expected_sha256).map_err(|error| match error {
            StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => contract(
                "WEAPONRY_KNIFE_PRODUCTION_BRIEF_CAS_MISSING",
                "brief CAS object is not registered",
            ),
            other => other,
        })?;
    let reachability_matches = supplied.reachability == registered.reachability
        || (supplied.reachability == "temporary" && registered.reachability == "reachable");
    if registered.sha256 != supplied.sha256
        || registered.size_bytes != supplied.size_bytes
        || registered.mime != supplied.mime
        || registered.kind != supplied.kind
        || !valid_cas_created_at(&registered.created_at)
        || !reachability_matches
        || (require_reachable && registered.reachability != "reachable")
    {
        return Err(contract(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_CAS_METADATA_MISMATCH",
            "brief CAS metadata differs from SQLite registration",
        ));
    }
    let bytes = cas
        .read_verified_bounded(
            expected_sha256,
            WEAPONRY_KNIFE_PRODUCTION_BRIEF_MAX_JSON_BYTES,
        )
        .map_err(StoreError::from)?;
    if bytes.len() as u64 != supplied.size_bytes || sha256_hex(&bytes) != expected_sha256 {
        return Err(contract(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_CAS_HASH_MISMATCH",
            "brief CAS bytes do not match their content hash",
        ));
    }
    Ok(bytes)
}

/// Revalidate the immutable ReferenceEvidence binding without copying or
/// exposing the source image.  The source object is already a GC root through
/// `reference_evidence`; this link only makes the exact lineage auditable and
/// does not change its reachability here.
fn validate_reference_lineage(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    record: &WeaponryKnifeProductionBriefStoreRecord,
) -> Result<(), StoreError> {
    let source: Option<(String, String, String, i64, String)> = transaction
        .query_row(
            "SELECT project_id, object_sha256, canonical_sha256, size_bytes, mime FROM reference_evidence WHERE reference_id = ?1",
            params![record.reference_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()?;
    let Some((project_id, object_sha256, evidence_sha256, size_bytes, mime)) = source else {
        return Err(contract(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_REFERENCE_MISSING",
            "brief reference_id is not bound to ReferenceEvidence",
        ));
    };
    if project_id != record.project_id
        || object_sha256 != record.reference_object_sha256
        || evidence_sha256 != record.reference_evidence_sha256
        || !is_sha256(&object_sha256)
        || !is_sha256(&evidence_sha256)
        || size_bytes <= 0
        || !matches!(mime.as_str(), "image/png" | "image/jpeg")
    {
        return Err(contract(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_REFERENCE_BINDING_MISMATCH",
            "brief ReferenceEvidence binding differs from the immutable source row",
        ));
    }
    let expected_size = u64::try_from(size_bytes).map_err(|_| {
        contract(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_REFERENCE_BINDING_MISMATCH",
            "brief ReferenceEvidence size is outside the valid range",
        )
    })?;
    let object =
        read_object_record(transaction, &record.reference_object_sha256).map_err(|error| {
            match error {
                StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => contract(
                    "WEAPONRY_KNIFE_PRODUCTION_BRIEF_REFERENCE_CAS_MISSING",
                    "brief source reference CAS object is not registered",
                ),
                other => other,
            }
        })?;
    if object.sha256 != record.reference_object_sha256
        || object.size_bytes != expected_size
        || object.mime != mime
        || object.kind != "reference-image"
        || !matches!(object.reachability.as_str(), "temporary" | "reachable")
    {
        return Err(contract(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_REFERENCE_CAS_METADATA_MISMATCH",
            "brief source reference CAS metadata differs from ReferenceEvidence",
        ));
    }
    cas.verify(&record.reference_object_sha256, expected_size)
        .map_err(StoreError::from)?;
    if !record
        .source_reference_hashes
        .binary_search(&record.reference_object_sha256)
        .is_ok()
    {
        return Err(contract(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_SOURCE_BINDING_MISMATCH",
            "brief source reference hash list omits the bound ReferenceEvidence object",
        ));
    }
    Ok(())
}

/// Load and fully verify the immutable parent inside the caller's write
/// transaction.  A successor can never replace an existing brief identity;
/// it is a new row whose source claims must remain byte-for-byte identical to
/// this parent.
fn validate_parent_brief_lineage(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    record: &WeaponryKnifeProductionBriefStoreRecord,
) -> Result<(), StoreError> {
    let (Some(parent_brief_id), Some(parent_brief_sha256)) = (
        record.parent_brief_id.as_deref(),
        record.parent_brief_sha256.as_deref(),
    ) else {
        return Ok(());
    };
    if parent_brief_id == record.brief_id || parent_brief_sha256 == record.brief_canonical_sha256 {
        return Err(contract(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_PARENT_SELF_REFERENCE",
            "successor brief cannot parent itself",
        ));
    }
    let parent = transaction
        .query_row(
            "SELECT record_json, source_reference_hashes_json, schema_version, project_id, brief_id, brief_object_sha256, brief_canonical_sha256, reference_id, reference_object_sha256, reference_evidence_sha256, parent_brief_id, parent_brief_sha256, freeze_policy, status, conflict_freeze_state, idempotency_key, created_at FROM weaponry_knife_production_brief_records WHERE project_id = ?1 AND brief_id = ?2",
            params![record.project_id, parent_brief_id],
            read_record,
        )
        .optional()?;
    let Some(parent) = parent else {
        return Err(contract(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_PARENT_MISSING",
            "successor brief parent exact identity is not durable",
        ));
    };
    validate_record(&parent)?;
    if parent.project_id != record.project_id
        || parent.brief_id != parent_brief_id
        || parent.brief_canonical_sha256 != parent_brief_sha256
    {
        return Err(contract(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_PARENT_HASH_MISMATCH",
            "successor parent semantic hash differs from the loaded parent",
        ));
    }
    let parent_object = read_object_record(transaction, &parent.brief_object_sha256).map_err(
        |error| match error {
            StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => contract(
                "WEAPONRY_KNIFE_PRODUCTION_BRIEF_PARENT_CAS_MISSING",
                "successor parent CAS object is not registered",
            ),
            other => other,
        },
    )?;
    let parent_bytes = validate_registered_object(
        transaction,
        cas,
        &parent_object,
        &parent.brief_object_sha256,
        true,
    )?;
    validate_brief_payload(&parent, &parent_bytes)?;
    validate_reference_lineage(transaction, cas, &parent)?;
    if record.reference_id != parent.reference_id
        || record.reference_object_sha256 != parent.reference_object_sha256
        || record.reference_evidence_sha256 != parent.reference_evidence_sha256
        || record.source_reference_hashes != parent.source_reference_hashes
    {
        return Err(contract(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_SOURCE_CLAIMS_CHANGED",
            "successor brief must preserve the parent's source claims",
        ));
    }
    Ok(())
}

fn read_record(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<WeaponryKnifeProductionBriefStoreRecord> {
    let payload: String = row.get(0)?;
    let mut record: WeaponryKnifeProductionBriefStoreRecord = serde_json::from_str(&payload)
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    let refs_json: String = row.get(1)?;
    let source_reference_hashes: Vec<String> =
        serde_json::from_str(&refs_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                1,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    if record.source_reference_hashes != source_reference_hashes {
        return Err(rusqlite::Error::FromSqlConversionFailure(
            1,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "brief record source-reference index disagrees with canonical record JSON",
            )),
        ));
    }
    let stored_columns = [
        (2usize, row.get::<_, String>(2)?, &record.schema_version),
        (3usize, row.get::<_, String>(3)?, &record.project_id),
        (4usize, row.get::<_, String>(4)?, &record.brief_id),
        (
            5usize,
            row.get::<_, String>(5)?,
            &record.brief_object_sha256,
        ),
        (
            6usize,
            row.get::<_, String>(6)?,
            &record.brief_canonical_sha256,
        ),
        (7usize, row.get::<_, String>(7)?, &record.reference_id),
        (
            8usize,
            row.get::<_, String>(8)?,
            &record.reference_object_sha256,
        ),
        (
            9usize,
            row.get::<_, String>(9)?,
            &record.reference_evidence_sha256,
        ),
        (12usize, row.get::<_, String>(12)?, &record.freeze_policy),
        (13usize, row.get::<_, String>(13)?, &record.status),
        (
            14usize,
            row.get::<_, String>(14)?,
            &record.conflict_freeze_state,
        ),
        (15usize, row.get::<_, String>(15)?, &record.idempotency_key),
        (16usize, row.get::<_, String>(16)?, &record.created_at),
    ];
    if row.get::<_, Option<String>>(10)? != record.parent_brief_id {
        return Err(rusqlite::Error::FromSqlConversionFailure(
            10,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "brief parent id index disagrees with canonical record JSON",
            )),
        ));
    }
    if row.get::<_, Option<String>>(11)? != record.parent_brief_sha256 {
        return Err(rusqlite::Error::FromSqlConversionFailure(
            11,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "brief parent hash index disagrees with canonical record JSON",
            )),
        ));
    }
    if let Some((index, _, _)) = stored_columns
        .iter()
        .find(|(_, stored, record_value)| stored != *record_value)
    {
        return Err(rusqlite::Error::FromSqlConversionFailure(
            *index,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "brief record index column disagrees with canonical record JSON",
            )),
        ));
    }
    record.source_reference_hashes = source_reference_hashes;
    Ok(record)
}

pub(crate) fn ensure_table(connection: &Connection) -> Result<(), StoreError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS weaponry_knife_production_brief_records (
             schema_version TEXT NOT NULL CHECK (schema_version = 'WeaponryKnifeProductionBriefStoreRecord@1'),
             project_id TEXT NOT NULL REFERENCES projects(project_id),
             brief_id TEXT NOT NULL,
             brief_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             brief_canonical_sha256 TEXT NOT NULL,
             reference_id TEXT NOT NULL REFERENCES reference_evidence(reference_id),
             reference_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             reference_evidence_sha256 TEXT NOT NULL,
             parent_brief_id TEXT,
             parent_brief_sha256 TEXT,
             freeze_policy TEXT NOT NULL CHECK (freeze_policy IN ('initial-intake-no-parent@1', 'immutable-successor-preserve-source-claims@1')),
             source_reference_hashes_json TEXT NOT NULL,
             status TEXT NOT NULL,
             conflict_freeze_state TEXT NOT NULL,
             idempotency_key TEXT NOT NULL UNIQUE,
             created_at TEXT NOT NULL,
             record_json TEXT NOT NULL,
             PRIMARY KEY (project_id, brief_id),
             UNIQUE (project_id, brief_canonical_sha256)
         );
         CREATE INDEX IF NOT EXISTS weaponry_knife_production_brief_project_idx
             ON weaponry_knife_production_brief_records(project_id, created_at DESC, brief_id ASC);
         CREATE INDEX IF NOT EXISTS weaponry_knife_production_brief_hash_idx
             ON weaponry_knife_production_brief_records(project_id, brief_canonical_sha256, brief_id ASC);
         CREATE INDEX IF NOT EXISTS weaponry_knife_production_brief_object_idx
             ON weaponry_knife_production_brief_records(brief_object_sha256);",
    )?;
    Ok(())
}

/// Additive migration for a development database created before the explicit
/// ReferenceEvidence lineage fields existed.  Empty defaults intentionally
/// make old rows fail closed on read; Store never invents a source binding.
pub(crate) fn ensure_columns(transaction: &Transaction<'_>) -> Result<(), StoreError> {
    super::ensure_column(
        transaction,
        "weaponry_knife_production_brief_records",
        "reference_id",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    super::ensure_column(
        transaction,
        "weaponry_knife_production_brief_records",
        "reference_object_sha256",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    super::ensure_column(
        transaction,
        "weaponry_knife_production_brief_records",
        "reference_evidence_sha256",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    super::ensure_column(
        transaction,
        "weaponry_knife_production_brief_records",
        "parent_brief_id",
        "TEXT",
    )?;
    super::ensure_column(
        transaction,
        "weaponry_knife_production_brief_records",
        "parent_brief_sha256",
        "TEXT",
    )?;
    super::ensure_column(
        transaction,
        "weaponry_knife_production_brief_records",
        "freeze_policy",
        "TEXT NOT NULL DEFAULT 'initial-intake-no-parent@1'",
    )?;
    transaction.execute_batch(
        "CREATE INDEX IF NOT EXISTS weaponry_knife_production_brief_reference_idx
             ON weaponry_knife_production_brief_records(reference_id, reference_object_sha256, reference_evidence_sha256);",
    )?;
    Ok(())
}

fn roots(record: &WeaponryKnifeProductionBriefStoreRecord) -> Vec<String> {
    vec![record.brief_object_sha256.clone()]
}

fn same_record(
    left: &WeaponryKnifeProductionBriefStoreRecord,
    right: &WeaponryKnifeProductionBriefStoreRecord,
) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    // Runtime may regenerate the intake timestamp while replaying the same
    // immutable idempotency request.  All content, status and bindings must
    // remain byte-for-byte equivalent.
    left.created_at.clear();
    right.created_at.clear();
    left == right
}

impl Store {
    /// Atomically install one Runtime-verified immutable knife production
    /// brief and promote its canonical JSON object to a GC root.  Exact
    /// idempotency replay returns `(record, true)`; any same-key, brief-id or
    /// project/hash binding change fails closed before another row is written.
    pub fn record_weaponry_knife_production_brief_with_replay(
        &self,
        commit: &WeaponryKnifeProductionBriefCommit,
    ) -> Result<(WeaponryKnifeProductionBriefStoreRecord, bool), StoreError> {
        validate_record(&commit.record)?;
        if commit.cas.brief.sha256 != commit.record.brief_object_sha256 {
            return Err(contract(
                "WEAPONRY_KNIFE_PRODUCTION_BRIEF_CAS_BINDING_MISMATCH",
                "brief CAS object hash differs from durable binding",
            ));
        }
        if commit.cas.brief.kind != WEAPONRY_KNIFE_PRODUCTION_BRIEF_OBJECT_KIND
            || commit.cas.brief.mime != WEAPONRY_KNIFE_PRODUCTION_BRIEF_JSON_MIME
        {
            return Err(contract(
                "WEAPONRY_KNIFE_PRODUCTION_BRIEF_CAS_METADATA_INVALID",
                "brief CAS object kind or MIME is invalid",
            ));
        }
        let payload_bytes = self
            .cas
            .read_verified_bounded(
                &commit.record.brief_object_sha256,
                WEAPONRY_KNIFE_PRODUCTION_BRIEF_MAX_JSON_BYTES,
            )
            .map_err(StoreError::from)?;
        validate_brief_payload(&commit.record, &payload_bytes)?;
        if sha256_hex(&payload_bytes) != commit.record.brief_object_sha256 {
            return Err(contract(
                "WEAPONRY_KNIFE_PRODUCTION_BRIEF_CAS_HASH_MISMATCH",
                "brief canonical bytes do not match their object hash",
            ));
        }

        let mut connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let transaction = connection.transaction()?;
        let existing = transaction
            .query_row(
                "SELECT record_json, source_reference_hashes_json, schema_version, project_id, brief_id, brief_object_sha256, brief_canonical_sha256, reference_id, reference_object_sha256, reference_evidence_sha256, parent_brief_id, parent_brief_sha256, freeze_policy, status, conflict_freeze_state, idempotency_key, created_at FROM weaponry_knife_production_brief_records WHERE idempotency_key = ?1",
                params![commit.record.idempotency_key],
                read_record,
            )
            .optional()?;
        if let Some(existing) = existing {
            validate_record(&existing)?;
            if !same_record(&existing, &commit.record) {
                return Err(contract(
                    "WEAPONRY_KNIFE_PRODUCTION_BRIEF_IDEMPOTENCY_CONFLICT",
                    "idempotency key is already bound to a different brief",
                ));
            }
            validate_parent_brief_lineage(&transaction, &self.cas, &existing)?;
            validate_reference_lineage(&transaction, &self.cas, &existing)?;
            let bytes = validate_registered_object(
                &transaction,
                &self.cas,
                &commit.cas.brief,
                &existing.brief_object_sha256,
                false,
            )?;
            validate_brief_payload(&existing, &bytes)?;
            mark_reachable_in_transaction(&transaction, &roots(&existing))?;
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
                "brief project does not exist",
            ));
        }
        let duplicate_brief: Option<String> = transaction
            .query_row(
                "SELECT idempotency_key FROM weaponry_knife_production_brief_records WHERE project_id = ?1 AND brief_id = ?2",
                params![commit.record.project_id, commit.record.brief_id],
                |row| row.get(0),
            )
            .optional()?;
        if duplicate_brief.is_some() {
            return Err(contract(
                "WEAPONRY_KNIFE_PRODUCTION_BRIEF_IDENTITY_CONFLICT",
                "project/brief id is already bound to a different immutable payload",
            ));
        }
        let duplicate_hash: Option<String> = transaction
            .query_row(
                "SELECT idempotency_key FROM weaponry_knife_production_brief_records WHERE project_id = ?1 AND brief_canonical_sha256 = ?2",
                params![commit.record.project_id, commit.record.brief_canonical_sha256],
                |row| row.get(0),
            )
            .optional()?;
        if duplicate_hash.is_some() {
            return Err(contract(
                "WEAPONRY_KNIFE_PRODUCTION_BRIEF_CANONICAL_CONFLICT",
                "project/canonical brief hash is already bound to another brief",
            ));
        }
        validate_parent_brief_lineage(&transaction, &self.cas, &commit.record)?;
        validate_reference_lineage(&transaction, &self.cas, &commit.record)?;
        validate_registered_object(
            &transaction,
            &self.cas,
            &commit.cas.brief,
            &commit.record.brief_object_sha256,
            false,
        )?;
        let record_json = String::from_utf8(record_bytes(&commit.record)?).map_err(|error| {
            StoreError::InvalidData(format!("brief durable record is not UTF-8: {error}"))
        })?;
        let source_refs_json = String::from_utf8(
            canonical_json_bytes(&json!(commit.record.source_reference_hashes))
                .map_err(|error| StoreError::InvalidData(error.to_string()))?,
        )
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        transaction.execute(
            "INSERT INTO weaponry_knife_production_brief_records (schema_version, project_id, brief_id, brief_object_sha256, brief_canonical_sha256, reference_id, reference_object_sha256, reference_evidence_sha256, parent_brief_id, parent_brief_sha256, freeze_policy, source_reference_hashes_json, status, conflict_freeze_state, idempotency_key, created_at, record_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                commit.record.schema_version,
                commit.record.project_id,
                commit.record.brief_id,
                commit.record.brief_object_sha256,
                commit.record.brief_canonical_sha256,
                commit.record.reference_id,
                commit.record.reference_object_sha256,
                commit.record.reference_evidence_sha256,
                commit.record.parent_brief_id,
                commit.record.parent_brief_sha256,
                commit.record.freeze_policy,
                source_refs_json,
                commit.record.status,
                commit.record.conflict_freeze_state,
                commit.record.idempotency_key,
                commit.record.created_at,
                record_json,
            ],
        )?;
        mark_reachable_in_transaction(&transaction, &roots(&commit.record))?;
        let stored = transaction
            .query_row(
                "SELECT record_json, source_reference_hashes_json, schema_version, project_id, brief_id, brief_object_sha256, brief_canonical_sha256, reference_id, reference_object_sha256, reference_evidence_sha256, parent_brief_id, parent_brief_sha256, freeze_policy, status, conflict_freeze_state, idempotency_key, created_at FROM weaponry_knife_production_brief_records WHERE project_id = ?1 AND brief_id = ?2",
                params![commit.record.project_id, commit.record.brief_id],
                read_record,
            )?;
        validate_record(&stored)?;
        transaction.commit()?;
        Ok((stored, false))
    }

    /// Exact project/brief/semantic-hash lookup.  The row and its canonical
    /// CAS payload are revalidated before returning, so a SQLite-only or
    /// corrupted-CAS projection cannot be mistaken for a durable brief.
    pub fn get_weaponry_knife_production_brief(
        &self,
        project_id: &str,
        brief_id: &str,
        brief_canonical_sha256: &str,
    ) -> Result<Option<WeaponryKnifeProductionBriefStoreRecord>, StoreError> {
        if !is_opaque_id(project_id)
            || !is_opaque_id(brief_id)
            || !is_sha256(brief_canonical_sha256)
        {
            return Err(StoreError::InvalidData(
                "brief lookup identity is invalid".to_owned(),
            ));
        }
        let mut connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let transaction = connection.transaction()?;
        let Some(record) = transaction
            .query_row(
                "SELECT record_json, source_reference_hashes_json, schema_version, project_id, brief_id, brief_object_sha256, brief_canonical_sha256, reference_id, reference_object_sha256, reference_evidence_sha256, parent_brief_id, parent_brief_sha256, freeze_policy, status, conflict_freeze_state, idempotency_key, created_at FROM weaponry_knife_production_brief_records WHERE project_id = ?1 AND brief_id = ?2 AND brief_canonical_sha256 = ?3",
                params![project_id, brief_id, brief_canonical_sha256],
                read_record,
            )
            .optional()?
        else {
            transaction.commit()?;
            return Ok(None);
        };
        validate_record(&record)?;
        if record.project_id != project_id
            || record.brief_id != brief_id
            || record.brief_canonical_sha256 != brief_canonical_sha256
        {
            return Err(contract(
                "WEAPONRY_KNIFE_PRODUCTION_BRIEF_SCOPE_MISMATCH",
                "stored brief scope differs from the exact lookup",
            ));
        }
        let object =
            read_object_record(&transaction, &record.brief_object_sha256).map_err(|error| {
                match error {
                    StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => contract(
                        "WEAPONRY_KNIFE_PRODUCTION_BRIEF_CAS_MISSING",
                        "brief CAS root disappeared before readback",
                    ),
                    other => other,
                }
            })?;
        let bytes = validate_registered_object(
            &transaction,
            &self.cas,
            &object,
            &record.brief_object_sha256,
            true,
        )?;
        validate_brief_payload(&record, &bytes)?;
        validate_reference_lineage(&transaction, &self.cas, &record)?;
        transaction.commit()?;
        Ok(Some(record))
    }

    /// Fully bound exact lookup used by the Runtime `get` operation.  The
    /// compact project/brief/semantic-hash lookup above remains useful for
    /// internal recovery, while this surface additionally prevents a caller
    /// from substituting a different ReferenceEvidence or CAS object.
    pub fn get_weaponry_knife_production_brief_exact(
        &self,
        project_id: &str,
        reference_id: &str,
        reference_object_sha256: &str,
        reference_evidence_sha256: &str,
        brief_id: &str,
        brief_sha256: &str,
        brief_object_sha256: &str,
    ) -> Result<Option<WeaponryKnifeProductionBriefStoreRecord>, StoreError> {
        if !is_opaque_id(reference_id)
            || !is_sha256(reference_object_sha256)
            || !is_sha256(reference_evidence_sha256)
            || !is_sha256(brief_object_sha256)
        {
            return Err(StoreError::InvalidData(
                "brief exact lookup binding is invalid".to_owned(),
            ));
        }
        let Some(record) =
            self.get_weaponry_knife_production_brief(project_id, brief_id, brief_sha256)?
        else {
            return Ok(None);
        };
        if record.reference_id != reference_id
            || record.reference_object_sha256 != reference_object_sha256
            || record.reference_evidence_sha256 != reference_evidence_sha256
            || record.brief_object_sha256 != brief_object_sha256
        {
            return Err(contract(
                "WEAPONRY_KNIFE_PRODUCTION_BRIEF_EXACT_BINDING_MISMATCH",
                "brief exact lookup binding differs from the immutable record",
            ));
        }
        Ok(Some(record))
    }

    /// Restart-safe lookup by the Runtime idempotency key.
    pub fn get_weaponry_knife_production_brief_by_idempotency(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<WeaponryKnifeProductionBriefStoreRecord>, StoreError> {
        if !is_opaque_id(idempotency_key) {
            return Err(StoreError::InvalidData(
                "brief idempotency key is invalid".to_owned(),
            ));
        }
        let mut connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let transaction = connection.transaction()?;
        let record = transaction
            .query_row(
                "SELECT record_json, source_reference_hashes_json, schema_version, project_id, brief_id, brief_object_sha256, brief_canonical_sha256, reference_id, reference_object_sha256, reference_evidence_sha256, parent_brief_id, parent_brief_sha256, freeze_policy, status, conflict_freeze_state, idempotency_key, created_at FROM weaponry_knife_production_brief_records WHERE idempotency_key = ?1",
                params![idempotency_key],
                read_record,
            )
            .optional()?;
        let Some(record) = record else {
            transaction.commit()?;
            return Ok(None);
        };
        validate_record(&record)?;
        let object = read_object_record(&transaction, &record.brief_object_sha256)?;
        let bytes = validate_registered_object(
            &transaction,
            &self.cas,
            &object,
            &record.brief_object_sha256,
            true,
        )?;
        validate_brief_payload(&record, &bytes)?;
        validate_reference_lineage(&transaction, &self.cas, &record)?;
        transaction.commit()?;
        Ok(Some(record))
    }

    /// Return the canonical Brief JSON after the same exact scope and CAS
    /// checks used by `get_weaponry_knife_production_brief`.
    pub fn read_weaponry_knife_production_brief_json(
        &self,
        project_id: &str,
        brief_id: &str,
        brief_canonical_sha256: &str,
    ) -> Result<Option<Value>, StoreError> {
        let Some(record) =
            self.get_weaponry_knife_production_brief(project_id, brief_id, brief_canonical_sha256)?
        else {
            return Ok(None);
        };
        let bytes = self
            .cas
            .read_verified_bounded(
                &record.brief_object_sha256,
                WEAPONRY_KNIFE_PRODUCTION_BRIEF_MAX_JSON_BYTES,
            )
            .map_err(StoreError::from)?;
        Ok(Some(validate_brief_payload(&record, &bytes)?))
    }

    /// The sole CAS root owned by a durable brief row.  Source reference
    /// hashes are intentionally metadata-only and are not treated as local
    /// image objects by this repository.
    pub fn weaponry_knife_production_brief_cas_roots(
        record: &WeaponryKnifeProductionBriefStoreRecord,
    ) -> Vec<String> {
        roots(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CasError, CasObject, ProjectRecord};
    use forgecad_contracts::{ReferenceAuthorization, ReferenceEvidenceRecord};
    use forgecad_core::canonical_json_hash;
    use std::fs;
    use uuid::Uuid;

    const PROJECT: &str = "brief-store-project";
    const BRIEF: &str = "dragonfang-brief";

    fn project(store: &Store) {
        store
            .insert_project(&ProjectRecord {
                schema_version: "Project@1".to_owned(),
                project_id: PROJECT.to_owned(),
                name: "Brief Store test".to_owned(),
                policy: json!({"scope":"test"}),
                created_at: "2026-08-30T00:00:00Z".to_owned(),
                updated_at: "2026-08-30T00:00:00Z".to_owned(),
                active_snapshot_revision: 0,
                head_snapshot_id: None,
                canonical_sha256: "a".repeat(64),
            })
            .expect("project");
    }

    fn reference_record(store: &Store) -> ReferenceEvidenceRecord {
        let bytes = b"brief-store-reference";
        let object = store
            .put_object(
                bytes,
                None,
                "image/png",
                "reference-image",
                "2026-08-30T00:00:00Z",
            )
            .expect("reference object");
        let reference_id = "reference-brief-store".to_owned();
        let project_id = PROJECT.to_owned();
        let authorization = ReferenceAuthorization {
            user_authorized: true,
            declaration: "authorized test source".to_owned(),
        };
        let canonical_sha256 = canonical_json_hash(&json!({
            "schema_version": "ReferenceEvidence@1",
            "reference_id": reference_id.clone(),
            "project_id": project_id.clone(),
            "object_sha256": object.record.sha256.clone(),
            "mime": "image/png",
            "size_bytes": object.record.size_bytes,
            "width": 1,
            "height": 1,
            "frame_count": 1,
            "import_mode": "inline_content",
            "authorization": authorization.clone(),
            "derived_object_sha256": Value::Null,
            "created_at": "2026-08-30T00:00:00Z",
        }));
        let reference = ReferenceEvidenceRecord {
            schema_version: "ReferenceEvidence@1".to_owned(),
            reference_id,
            project_id,
            object_sha256: object.record.sha256,
            mime: "image/png".to_owned(),
            size_bytes: object.record.size_bytes,
            width: 1,
            height: 1,
            frame_count: 1,
            import_mode: "inline_content".to_owned(),
            authorization,
            derived_object_sha256: None,
            canonical_sha256,
            created_at: "2026-08-30T00:00:00Z".to_owned(),
        };
        store
            .insert_reference_evidence(&reference)
            .expect("reference evidence");
        reference
    }

    fn brief_payload_for(
        brief_id: &str,
        source: &str,
        parent_brief_id: Option<&str>,
        parent_brief_sha256: Option<&str>,
        freeze_policy: &str,
    ) -> Vec<u8> {
        let mut value = json!({
            "schema_version": "WeaponryKnifeProductionBrief@1",
            "brief_id": brief_id,
            "project_id": PROJECT,
            "parent_brief_id": parent_brief_id,
            "parent_brief_sha256": parent_brief_sha256,
            "freeze_policy": freeze_policy,
            "authorization": {"source_reference_sha256": source},
            "reference_coverage": {"source_reference_sha256": source},
            "canonicalization_policy": "canonical-json-sha256-excluding-canonical-sha256@1",
            "canonical_sha256": "",
        });
        let canonical = canonical_json_hash(&value);
        value["canonical_sha256"] = Value::String(canonical);
        canonical_json_bytes(&value).expect("canonical brief")
    }

    fn make_commit(
        store: &Store,
        reference: &ReferenceEvidenceRecord,
        key: &str,
    ) -> WeaponryKnifeProductionBriefCommit {
        make_commit_for(store, reference, key, BRIEF, None)
    }

    fn make_commit_for(
        store: &Store,
        reference: &ReferenceEvidenceRecord,
        key: &str,
        brief_id: &str,
        parent: Option<(&str, &str)>,
    ) -> WeaponryKnifeProductionBriefCommit {
        let (parent_brief_id, parent_brief_sha256) = parent
            .map(|(id, sha256)| (Some(id), Some(sha256)))
            .unwrap_or((None, None));
        let freeze_policy = if parent.is_some() {
            "immutable-successor-preserve-source-claims@1"
        } else {
            "initial-intake-no-parent@1"
        };
        let bytes = brief_payload_for(
            brief_id,
            &reference.object_sha256,
            parent_brief_id,
            parent_brief_sha256,
            freeze_policy,
        );
        let semantic = {
            let value: Value = serde_json::from_slice(&bytes).expect("brief value");
            value["canonical_sha256"]
                .as_str()
                .expect("canonical")
                .to_owned()
        };
        let object = store
            .put_object(
                &bytes,
                None,
                WEAPONRY_KNIFE_PRODUCTION_BRIEF_JSON_MIME,
                WEAPONRY_KNIFE_PRODUCTION_BRIEF_OBJECT_KIND,
                "2026-08-30T00:00:00Z",
            )
            .expect("brief object");
        WeaponryKnifeProductionBriefCommit {
            record: WeaponryKnifeProductionBriefStoreRecord {
                schema_version: WEAPONRY_KNIFE_PRODUCTION_BRIEF_RECORD_SCHEMA_VERSION.to_owned(),
                project_id: PROJECT.to_owned(),
                brief_id: brief_id.to_owned(),
                brief_object_sha256: object.record.sha256.clone(),
                brief_canonical_sha256: semantic,
                reference_id: reference.reference_id.clone(),
                reference_object_sha256: reference.object_sha256.clone(),
                reference_evidence_sha256: reference.canonical_sha256.clone(),
                parent_brief_id: parent_brief_id.map(str::to_owned),
                parent_brief_sha256: parent_brief_sha256.map(str::to_owned),
                freeze_policy: freeze_policy.to_owned(),
                source_reference_hashes: vec![reference.object_sha256.clone()],
                status: "blocked".to_owned(),
                conflict_freeze_state: "frozen".to_owned(),
                idempotency_key: key.to_owned(),
                created_at: "2026-08-30T00:00:00Z".to_owned(),
            },
            cas: WeaponryKnifeProductionBriefCasBundle {
                brief: object.record,
            },
        }
    }

    #[test]
    fn brief_store_is_atomic_replayed_exactly_and_roots_only_canonical_brief() {
        let store = Store::memory().expect("store");
        project(&store);
        let reference = reference_record(&store);
        let commit = make_commit(&store, &reference, "brief-idem-1");
        let (stored, replayed) = store
            .record_weaponry_knife_production_brief_with_replay(&commit)
            .expect("commit");
        assert!(!replayed);
        assert_eq!(stored, commit.record);
        assert_eq!(
            Store::weaponry_knife_production_brief_cas_roots(&stored),
            vec![stored.brief_object_sha256.clone()]
        );
        assert_eq!(
            store
                .get_object(&stored.brief_object_sha256)
                .expect("object")
                .expect("metadata")
                .reachability,
            "reachable"
        );
        let (again, replayed) = store
            .record_weaponry_knife_production_brief_with_replay(&commit)
            .expect("replay");
        assert!(replayed);
        assert_eq!(again, stored);
        assert_eq!(
            store
                .get_weaponry_knife_production_brief(PROJECT, BRIEF, &stored.brief_canonical_sha256)
                .expect("get"),
            Some(stored)
        );
        assert_eq!(
            store
                .get_weaponry_knife_production_brief_exact(
                    PROJECT,
                    &reference.reference_id,
                    &reference.object_sha256,
                    &reference.canonical_sha256,
                    BRIEF,
                    &commit.record.brief_canonical_sha256,
                    &commit.record.brief_object_sha256,
                )
                .expect("fully bound get"),
            Some(commit.record)
        );
    }

    #[test]
    fn brief_store_exact_replay_accepts_distinct_cas_created_at_and_rejects_metadata_drift() {
        let store = Store::memory().expect("store");
        project(&store);
        let reference = reference_record(&store);
        let original = make_commit(&store, &reference, "brief-cas-replay-timestamp");
        let (record, replayed) = store
            .record_weaponry_knife_production_brief_with_replay(&original)
            .expect("commit");
        assert!(!replayed);

        let replay_created_at = "2026-08-31T00:00:00Z";
        let mut replay = original.clone();
        replay.record.created_at = replay_created_at.to_owned();
        replay.cas.brief.created_at = replay_created_at.to_owned();
        let (again, replayed) = store
            .record_weaponry_knife_production_brief_with_replay(&replay)
            .expect("exact replay with a new CAS timestamp");
        assert!(replayed);
        assert_eq!(again, record);
        assert_eq!(replay.cas.brief.created_at, replay_created_at);
        assert_ne!(replay.cas.brief.created_at, original.cas.brief.created_at);
        let registered = store
            .get_object(&record.brief_object_sha256)
            .expect("registered metadata")
            .expect("brief CAS object");
        assert_eq!(registered.created_at, original.cas.brief.created_at);
        assert_eq!(registered.reachability, "reachable");

        let mut size_drift = replay.clone();
        size_drift.cas.brief.size_bytes += 1;
        let error = store
            .record_weaponry_knife_production_brief_with_replay(&size_drift)
            .expect_err("CAS size drift must fail closed");
        assert!(matches!(
            error,
            StoreError::Contract { code, .. }
                if code == "WEAPONRY_KNIFE_PRODUCTION_BRIEF_CAS_METADATA_MISMATCH"
        ));

        let mut hash_drift = replay;
        hash_drift.cas.brief.sha256 = "b".repeat(64);
        let error = store
            .record_weaponry_knife_production_brief_with_replay(&hash_drift)
            .expect_err("CAS hash drift must fail closed");
        assert!(matches!(
            error,
            StoreError::Contract { code, .. }
                if code == "WEAPONRY_KNIFE_PRODUCTION_BRIEF_CAS_BINDING_MISMATCH"
        ));
    }

    #[test]
    fn brief_store_same_key_conflict_and_late_invalid_write_leave_zero_rows() {
        let store = Store::memory().expect("store");
        project(&store);
        let reference = reference_record(&store);
        let original = make_commit(&store, &reference, "brief-idem-conflict");
        store
            .record_weaponry_knife_production_brief_with_replay(&original)
            .expect("original");

        let mut conflict = make_commit(&store, &reference, "brief-idem-conflict");
        conflict.record.status = "ready".to_owned();
        let error = store
            .record_weaponry_knife_production_brief_with_replay(&conflict)
            .expect_err("same key conflict");
        assert!(
            matches!(error, StoreError::Contract { code, .. } if code == "WEAPONRY_KNIFE_PRODUCTION_BRIEF_IDEMPOTENCY_CONFLICT")
        );

        let mut invalid = make_commit(&store, &reference, "brief-idem-invalid");
        invalid.record.brief_canonical_sha256 = "b".repeat(64);
        let error = store
            .record_weaponry_knife_production_brief_with_replay(&invalid)
            .expect_err("invalid late write");
        assert!(
            matches!(error, StoreError::Contract { code, .. } if code == "WEAPONRY_KNIFE_PRODUCTION_BRIEF_CANONICAL_MISMATCH")
        );
        assert_eq!(
            store
                .get_weaponry_knife_production_brief_by_idempotency("brief-idem-invalid")
                .expect("lookup"),
            None
        );
        let count: i64 = store
            .connection
            .lock()
            .expect("connection")
            .query_row(
                "SELECT COUNT(*) FROM weaponry_knife_production_brief_records",
                [],
                |row| row.get(0),
            )
            .expect("row count");
        assert_eq!(count, 1);
    }

    #[test]
    fn brief_store_freeze_parent_and_replacement_failures_leave_zero_rows() {
        let store = Store::memory().expect("store");
        project(&store);
        let reference = reference_record(&store);
        let parent_commit = make_commit(&store, &reference, "brief-parent");
        let (parent, _) = store
            .record_weaponry_knife_production_brief_with_replay(&parent_commit)
            .expect("parent");

        let missing_parent = make_commit_for(
            &store,
            &reference,
            "brief-missing-parent",
            "dragonfang-successor-missing",
            Some(("missing-parent", &"b".repeat(64))),
        );
        let error = store
            .record_weaponry_knife_production_brief_with_replay(&missing_parent)
            .expect_err("missing parent");
        assert!(matches!(
            error,
            StoreError::Contract { code, .. }
                if code == "WEAPONRY_KNIFE_PRODUCTION_BRIEF_PARENT_MISSING"
        ));

        let valid_successor = make_commit_for(
            &store,
            &reference,
            "brief-successor",
            "dragonfang-successor",
            Some((&parent.brief_id, &parent.brief_canonical_sha256)),
        );
        let (successor, replayed) = store
            .record_weaponry_knife_production_brief_with_replay(&valid_successor)
            .expect("successor");
        assert!(!replayed);
        assert_eq!(
            successor.parent_brief_id.as_deref(),
            Some(parent.brief_id.as_str())
        );
        assert_eq!(
            successor.parent_brief_sha256.as_deref(),
            Some(parent.brief_canonical_sha256.as_str())
        );
        assert_eq!(
            successor.freeze_policy,
            "immutable-successor-preserve-source-claims@1"
        );

        let drifted_parent_hash = make_commit_for(
            &store,
            &reference,
            "brief-drifted-parent",
            "dragonfang-successor-drifted",
            Some((&parent.brief_id, &"c".repeat(64))),
        );
        let error = store
            .record_weaponry_knife_production_brief_with_replay(&drifted_parent_hash)
            .expect_err("parent hash drift");
        assert!(matches!(
            error,
            StoreError::Contract { code, .. }
                if code == "WEAPONRY_KNIFE_PRODUCTION_BRIEF_PARENT_HASH_MISMATCH"
        ));

        let replacement = make_commit(&store, &reference, "brief-replacement");
        let error = store
            .record_weaponry_knife_production_brief_with_replay(&replacement)
            .expect_err("old brief replacement");
        assert!(matches!(
            error,
            StoreError::Contract { code, .. }
                if code == "WEAPONRY_KNIFE_PRODUCTION_BRIEF_IDENTITY_CONFLICT"
        ));

        let count: i64 = store
            .connection
            .lock()
            .expect("connection")
            .query_row(
                "SELECT COUNT(*) FROM weaponry_knife_production_brief_records",
                [],
                |row| row.get(0),
            )
            .expect("row count");
        assert_eq!(count, 2);
        assert_eq!(
            store
                .get_weaponry_knife_production_brief(
                    PROJECT,
                    &successor.brief_id,
                    &successor.brief_canonical_sha256,
                )
                .expect("successor readback"),
            Some(successor)
        );
    }

    #[test]
    fn brief_store_failed_reserved_prepare_can_cleanup_orphan_without_rows() {
        let store = Store::memory().expect("store");
        project(&store);
        let reference = reference_record(&store);
        let brief_id = "dragonfang-successor-orphan";
        let parent_brief_id = "missing-parent";
        let parent_brief_sha256 = "b".repeat(64);
        let bytes = brief_payload_for(
            brief_id,
            &reference.object_sha256,
            Some(parent_brief_id),
            Some(&parent_brief_sha256),
            "immutable-successor-preserve-source-claims@1",
        );
        let value: Value = serde_json::from_slice(&bytes).expect("brief value");
        let semantic = value["canonical_sha256"]
            .as_str()
            .expect("canonical")
            .to_owned();
        let reservation = store.begin_cas_reservation();
        let object = store
            .put_object_reserved(
                &reservation,
                &bytes,
                None,
                WEAPONRY_KNIFE_PRODUCTION_BRIEF_JSON_MIME,
                WEAPONRY_KNIFE_PRODUCTION_BRIEF_OBJECT_KIND,
                "2026-08-30T00:00:00Z",
            )
            .expect("reserved brief object");
        assert!(object.created_new);
        assert!(object.path.exists());
        assert_eq!(
            store
                .get_object(&object.record.sha256)
                .expect("object metadata")
                .expect("temporary object")
                .reachability,
            "temporary"
        );

        let commit = WeaponryKnifeProductionBriefCommit {
            record: WeaponryKnifeProductionBriefStoreRecord {
                schema_version: WEAPONRY_KNIFE_PRODUCTION_BRIEF_RECORD_SCHEMA_VERSION.to_owned(),
                project_id: PROJECT.to_owned(),
                brief_id: brief_id.to_owned(),
                brief_object_sha256: object.record.sha256.clone(),
                brief_canonical_sha256: semantic,
                reference_id: reference.reference_id.clone(),
                reference_object_sha256: reference.object_sha256.clone(),
                reference_evidence_sha256: reference.canonical_sha256.clone(),
                parent_brief_id: Some(parent_brief_id.to_owned()),
                parent_brief_sha256: Some(parent_brief_sha256),
                freeze_policy: "immutable-successor-preserve-source-claims@1".to_owned(),
                source_reference_hashes: vec![reference.object_sha256.clone()],
                status: "blocked".to_owned(),
                conflict_freeze_state: "frozen".to_owned(),
                idempotency_key: "brief-orphan-failure".to_owned(),
                created_at: "2026-08-30T00:00:00Z".to_owned(),
            },
            cas: WeaponryKnifeProductionBriefCasBundle {
                brief: object.record.clone(),
            },
        };
        let error = store
            .record_weaponry_knife_production_brief_with_replay(&commit)
            .expect_err("missing parent must reject before insert");
        assert!(matches!(
            error,
            StoreError::Contract { code, .. }
                if code == "WEAPONRY_KNIFE_PRODUCTION_BRIEF_PARENT_MISSING"
        ));
        assert_eq!(
            store
                .get_weaponry_knife_production_brief_by_idempotency("brief-orphan-failure")
                .expect("idempotency lookup"),
            None
        );
        let count: i64 = store
            .connection
            .lock()
            .expect("connection")
            .query_row(
                "SELECT COUNT(*) FROM weaponry_knife_production_brief_records",
                [],
                |row| row.get(0),
            )
            .expect("row count");
        assert_eq!(count, 0);

        let removed = store
            .release_cas_reservation_object(&reservation, &object, true)
            .expect("failed prepare cleanup");
        assert!(removed);
        assert!(store
            .get_object(&object.record.sha256)
            .expect("metadata")
            .is_none());
        assert!(!object.path.exists());
    }

    #[test]
    fn brief_store_cleanup_never_removes_durable_brief_root() {
        let store = Store::memory().expect("store");
        project(&store);
        let reference = reference_record(&store);
        let commit = make_commit(&store, &reference, "brief-durable-root");
        let (stored, _) = store
            .record_weaponry_knife_production_brief_with_replay(&commit)
            .expect("durable brief");
        let path = store
            .cas()
            .root()
            .join("objects")
            .join(&stored.brief_object_sha256[..2])
            .join(&stored.brief_object_sha256);
        assert!(path.exists());

        // Simulate a failed operation holding a stale `created_new` view of
        // the same object.  SQLite reachability is authoritative, so the
        // temporary cleanup allowlist must not delete this durable root.
        let stale_object = CasObject {
            record: commit.cas.brief.clone(),
            path: path.clone(),
            created_new: true,
        };
        let reservation = store.begin_cas_reservation();
        assert!(!store
            .release_cas_reservation_object(&reservation, &stale_object, true)
            .expect("durable root cleanup is a no-op"));
        assert_eq!(
            store
                .get_object(&stored.brief_object_sha256)
                .expect("root metadata")
                .expect("durable root")
                .reachability,
            "reachable"
        );
        assert!(path.exists());
        assert_eq!(
            store
                .get_weaponry_knife_production_brief(
                    PROJECT,
                    BRIEF,
                    &stored.brief_canonical_sha256,
                )
                .expect("durable root readback"),
            Some(stored)
        );
    }

    #[test]
    fn brief_store_drop_reopen_and_corrupt_cas_fail_closed() {
        let root = std::env::temp_dir().join(format!("forgecad-brief-store-{}", Uuid::new_v4()));
        let database = root.join("runtime.sqlite");
        let cas_root = root.join("runtime.cas");
        let commit_record;
        {
            let store = Store::open_with_cas(&database, &cas_root).expect("open");
            project(&store);
            let reference = reference_record(&store);
            let commit = make_commit(&store, &reference, "brief-idem-reopen");
            let (stored, _) = store
                .record_weaponry_knife_production_brief_with_replay(&commit)
                .expect("commit");
            commit_record = stored;
        }
        let reopened = Store::open_with_cas(&database, &cas_root).expect("reopen");
        assert_eq!(
            reopened
                .get_weaponry_knife_production_brief(
                    PROJECT,
                    BRIEF,
                    &commit_record.brief_canonical_sha256
                )
                .expect("readback"),
            Some(commit_record.clone())
        );
        let path = reopened
            .cas()
            .root()
            .join("objects")
            .join(&commit_record.brief_object_sha256[..2])
            .join(&commit_record.brief_object_sha256);
        fs::write(&path, b"corrupt").expect("corrupt CAS");
        let error = reopened
            .get_weaponry_knife_production_brief(
                PROJECT,
                BRIEF,
                &commit_record.brief_canonical_sha256,
            )
            .expect_err("corrupt CAS must fail closed");
        assert!(matches!(
            error,
            StoreError::Cas(CasError::HashMismatch { .. }) | StoreError::Contract { .. }
        ));
        let _ = fs::remove_dir_all(root);
    }
}
