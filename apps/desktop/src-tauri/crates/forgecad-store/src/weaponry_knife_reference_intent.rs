//! Durable Store/CAS ownership for the knife reference convergence intent.
//!
//! This is intentionally a Store-local record.  Runtime owns the contract
//! parser and is the only product writer; this module accepts only the
//! already-canonical JSON objects staged by Runtime and installs their
//! immutable lineage in one SQLite transaction.  No path, URL, secret or
//! image bytes are copied into the durable record.

use super::{
    canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256,
    mark_reachable_in_transaction, CasObjectRecord, CasStore, Store, StoreError,
};
use forgecad_core::sha256_hex;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const KNIFE_REFERENCE_INTENT_BUNDLE_RECORD_SCHEMA_VERSION: &str =
    "KnifeReferenceIntentBundleStoreRecord@1";
pub const KNIFE_REFERENCE_INTENT_BUNDLE_SCHEMA_VERSION: &str = "KnifeReferenceIntentBundle@1";
pub const KNIFE_REFERENCE_INTENT_BUNDLE_STATUS: &str =
    "runtime-owned-store-knife-reference-intent-bundle@1";
pub const KNIFE_REFERENCE_INTENT_BUNDLE_OBJECT_KIND: &str = "knife-reference-intent-bundle";
pub const KNIFE_REFERENCE_INTENT_INTAKE_OBJECT_KIND: &str = "knife-intake-manifest";
pub const KNIFE_REFERENCE_INTENT_DETAIL_OBJECT_KIND: &str = "knife-detail-inventory";
pub const KNIFE_REFERENCE_INTENT_QUALITY_OBJECT_KIND: &str = "knife-quality-contract";
pub const KNIFE_REFERENCE_INTENT_JSON_MIME: &str = "application/json";
pub const KNIFE_REFERENCE_INTENT_MAX_JSON_BYTES: u64 = 1024 * 1024;

/// Store-local immutable binding for the eligible Brief, its exact source
/// ReferenceEvidence, and the three canonical intent children.
///
/// Semantic hashes identify canonical child content.  Object hashes identify
/// the CAS bytes and are kept separate because the canonical hash field in a
/// JSON object is populated after the semantic preimage is hashed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnifeReferenceIntentBundleStoreRecord {
    pub schema_version: String,
    pub intent_bundle_id: String,
    pub project_id: String,
    pub brief_id: String,
    pub brief_sha256: String,
    pub brief_object_sha256: String,
    pub reference_id: String,
    pub reference_object_sha256: String,
    pub reference_evidence_sha256: String,
    pub intake_manifest_sha256: String,
    pub intake_manifest_object_sha256: String,
    pub detail_inventory_sha256: String,
    pub detail_inventory_object_sha256: String,
    pub quality_contract_sha256: String,
    pub quality_contract_object_sha256: String,
    pub intent_bundle_sha256: String,
    pub intent_bundle_object_sha256: String,
    pub idempotency_key: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct KnifeReferenceIntentBundleCasBundle {
    pub intent_bundle: CasObjectRecord,
    pub intake_manifest: CasObjectRecord,
    pub detail_inventory: CasObjectRecord,
    pub quality_contract: CasObjectRecord,
}

#[derive(Debug, Clone)]
pub struct KnifeReferenceIntentBundleCommit {
    pub record: KnifeReferenceIntentBundleStoreRecord,
    pub cas: KnifeReferenceIntentBundleCasBundle,
}

fn contract(code: &str, message: impl Into<String>) -> StoreError {
    StoreError::Contract {
        code: code.to_owned(),
        message: message.into(),
    }
}

fn record_value(record: &KnifeReferenceIntentBundleStoreRecord) -> Result<Value, StoreError> {
    serde_json::to_value(record).map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn record_bytes(record: &KnifeReferenceIntentBundleStoreRecord) -> Result<Vec<u8>, StoreError> {
    canonical_json_bytes(&record_value(record)?)
        .map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn validate_record(record: &KnifeReferenceIntentBundleStoreRecord) -> Result<(), StoreError> {
    if record.schema_version != KNIFE_REFERENCE_INTENT_BUNDLE_RECORD_SCHEMA_VERSION
        || !is_opaque_id(&record.project_id)
        || !is_opaque_id(&record.brief_id)
        || !is_sha256(&record.brief_sha256)
        || !is_sha256(&record.brief_object_sha256)
        || !is_opaque_id(&record.reference_id)
        || !is_sha256(&record.reference_object_sha256)
        || !is_sha256(&record.reference_evidence_sha256)
        || !is_sha256(&record.intake_manifest_sha256)
        || !is_sha256(&record.intake_manifest_object_sha256)
        || !is_sha256(&record.detail_inventory_sha256)
        || !is_sha256(&record.detail_inventory_object_sha256)
        || !is_sha256(&record.quality_contract_sha256)
        || !is_sha256(&record.quality_contract_object_sha256)
        || !is_opaque_id(&record.intent_bundle_id)
        || !is_sha256(&record.intent_bundle_sha256)
        || !is_sha256(&record.intent_bundle_object_sha256)
        || !is_opaque_id(&record.idempotency_key)
        || record.idempotency_key.len() > 128
        || record.created_at.is_empty()
        || record.created_at.len() > 128
        || record.created_at.contains('/')
        || record.created_at.contains('\\')
    {
        return Err(contract(
            "KNIFE_REFERENCE_INTENT_BUNDLE_RECORD_INVALID",
            "reference intent durable identity or hash is malformed",
        ));
    }
    if record.brief_object_sha256 == record.intent_bundle_object_sha256 {
        return Err(contract(
            "KNIFE_REFERENCE_INTENT_BUNDLE_OBJECT_COLLISION",
            "brief and intent bundle CAS objects must be distinct",
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
        "output",
    ];
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if FORBIDDEN_KEYS.contains(&key.to_ascii_lowercase().as_str()) {
                    return Err(contract(
                        "KNIFE_REFERENCE_INTENT_BUNDLE_FORBIDDEN_PAYLOAD",
                        "intent payload contains a forbidden path, URL, secret, contact or byte field",
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
            let windows_path = text.len() >= 3
                && text.as_bytes()[1] == b':'
                && matches!(text.as_bytes()[2], b'/' | b'\\');
            let dot_segment = text == "."
                || text == ".."
                || text.contains("/./")
                || text.contains("/../")
                || text.contains("\\.\\")
                || text.contains("\\..\\");
            if text.chars().any(char::is_control)
                || text.starts_with('/')
                || text.starts_with('\\')
                || windows_path
                || dot_segment
                || lower.starts_with("file:")
                || lower.starts_with("data:")
                || lower.starts_with("http:")
                || lower.starts_with("https:")
                || lower.starts_with("ftp:")
                || ["api key:", "api_key:", "secret:", "password:", "token:"]
                    .iter()
                    .any(|prefix| lower.contains(prefix))
            {
                return Err(contract(
                    "KNIFE_REFERENCE_INTENT_BUNDLE_FORBIDDEN_PAYLOAD",
                    "intent payload contains a path, URL, secret or control character",
                ));
            }
        }
        _ => {}
    }
    Ok(())
}

fn canonical_payload(
    bytes: &[u8],
    expected_schema: &str,
    expected_semantic_sha256: &str,
    code_prefix: &str,
) -> Result<Value, StoreError> {
    if bytes.is_empty() || bytes.len() as u64 > KNIFE_REFERENCE_INTENT_MAX_JSON_BYTES {
        return Err(contract(
            &format!("{code_prefix}_BYTES_INVALID"),
            "intent CAS JSON is empty or exceeds the bounded capacity",
        ));
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        contract(
            &format!("{code_prefix}_JSON_INVALID"),
            format!("intent CAS object is not valid JSON: {error}"),
        )
    })?;
    if !value.is_object() {
        return Err(contract(
            &format!("{code_prefix}_JSON_INVALID"),
            "intent CAS payload must be a JSON object",
        ));
    }
    validate_no_forbidden_payload(&value)?;
    let canonical =
        canonical_json_bytes(&value).map_err(|error| StoreError::InvalidData(error.to_string()))?;
    if canonical != bytes {
        return Err(contract(
            &format!("{code_prefix}_NOT_CANONICAL"),
            "intent CAS JSON bytes are not canonical",
        ));
    }
    if value.get("schema_version").and_then(Value::as_str) != Some(expected_schema) {
        return Err(contract(
            &format!("{code_prefix}_SCHEMA_INVALID"),
            "intent CAS JSON schema_version is invalid",
        ));
    }
    let semantic = value
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| {
            contract(
                &format!("{code_prefix}_CANONICAL_MISMATCH"),
                "intent CAS JSON canonical_sha256 is missing or malformed",
            )
        })?;
    let mut preimage = value.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != semantic || semantic != expected_semantic_sha256 {
        return Err(contract(
            &format!("{code_prefix}_CANONICAL_MISMATCH"),
            "intent CAS JSON semantic hash differs from its Store binding",
        ));
    }
    Ok(value)
}

fn value_hash(value: &Value, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        value
            .get(*name)
            .and_then(Value::as_str)
            .filter(|hash| is_sha256(hash))
            .map(str::to_owned)
    })
}

fn value_id<'a>(value: &'a Value, name: &str) -> Option<&'a str> {
    value.get(name).and_then(Value::as_str)
}

fn validate_bundle_payload(
    record: &KnifeReferenceIntentBundleStoreRecord,
    bytes: &[u8],
) -> Result<Value, StoreError> {
    let value = canonical_payload(
        bytes,
        KNIFE_REFERENCE_INTENT_BUNDLE_SCHEMA_VERSION,
        &record.intent_bundle_sha256,
        "KNIFE_REFERENCE_INTENT_BUNDLE",
    )?;
    let brief = value.get("brief_binding").ok_or_else(|| {
        contract(
            "KNIFE_REFERENCE_INTENT_BUNDLE_BINDING_MISMATCH",
            "intent bundle brief_binding is missing",
        )
    })?;
    let reference = value.get("reference_binding").ok_or_else(|| {
        contract(
            "KNIFE_REFERENCE_INTENT_BUNDLE_BINDING_MISMATCH",
            "intent bundle reference_binding is missing",
        )
    })?;
    if value_id(&value, "intent_bundle_id") != Some(record.intent_bundle_id.as_str())
        || value_id(&value, "project_id") != Some(record.project_id.as_str())
        || value_id(brief, "brief_schema_version") != Some("WeaponryKnifeProductionBrief@1")
        || value_id(brief, "brief_id") != Some(record.brief_id.as_str())
        || value_hash(brief, &["brief_sha256"]).as_deref() != Some(record.brief_sha256.as_str())
        || value_hash(brief, &["brief_object_sha256"]).as_deref()
            != Some(record.brief_object_sha256.as_str())
        || value_id(brief, "authoring_eligibility") != Some("ELIGIBLE")
        || value_id(brief, "authorization_binding_status") != Some("runtime-bound")
        || value_id(reference, "reference_id") != Some(record.reference_id.as_str())
        || value_hash(reference, &["reference_object_sha256"]).as_deref()
            != Some(record.reference_object_sha256.as_str())
        || value_hash(reference, &["reference_evidence_sha256"]).as_deref()
            != Some(record.reference_evidence_sha256.as_str())
        || value_id(reference, "binding_status") != Some("runtime-bound")
    {
        return Err(contract(
            "KNIFE_REFERENCE_INTENT_BUNDLE_BINDING_MISMATCH",
            "intent bundle payload does not exactly bind its brief, reference or child records",
        ));
    }
    for (field, expected_schema, expected_sha256) in [
        (
            "intake_manifest",
            "KnifeIntakeManifest@1",
            record.intake_manifest_sha256.as_str(),
        ),
        (
            "detail_inventory",
            "KnifeDetailInventory@1",
            record.detail_inventory_sha256.as_str(),
        ),
        (
            "quality_contract",
            "KnifeQualityContract@1",
            record.quality_contract_sha256.as_str(),
        ),
    ] {
        let child = value.get(field).ok_or_else(|| {
            contract(
                "KNIFE_REFERENCE_INTENT_BUNDLE_BINDING_MISMATCH",
                format!("intent bundle {field} child is missing"),
            )
        })?;
        if value_id(child, "schema_version") != Some(expected_schema)
            || value_hash(child, &["canonical_sha256"]).as_deref() != Some(expected_sha256)
        {
            return Err(contract(
                "KNIFE_REFERENCE_INTENT_BUNDLE_CHILD_BINDING_MISMATCH",
                format!("intent bundle {field} child semantic hash differs"),
            ));
        }
        let mut preimage = child.clone();
        preimage["canonical_sha256"] = Value::String(String::new());
        if canonical_json_hash(&preimage) != expected_sha256 {
            return Err(contract(
                "KNIFE_REFERENCE_INTENT_BUNDLE_CHILD_CANONICAL_MISMATCH",
                format!("intent bundle {field} child canonical hash is invalid"),
            ));
        }
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
    expected_kind: &str,
    expected_schema: &str,
    expected_semantic_sha256: &str,
    require_reachable: bool,
    role: &str,
) -> Result<Value, StoreError> {
    if supplied.schema_version != "CasObject@1"
        || !is_sha256(&supplied.sha256)
        || supplied.mime != KNIFE_REFERENCE_INTENT_JSON_MIME
        || supplied.kind != expected_kind
        || supplied.size_bytes == 0
        || supplied.size_bytes > KNIFE_REFERENCE_INTENT_MAX_JSON_BYTES
        || !matches!(supplied.reachability.as_str(), "temporary" | "reachable")
        || (require_reachable && supplied.reachability != "reachable")
        || !valid_cas_created_at(&supplied.created_at)
    {
        return Err(contract(
            "KNIFE_REFERENCE_INTENT_BUNDLE_CAS_METADATA_INVALID",
            format!("{role} CAS metadata is outside the bounded allowlist"),
        ));
    }
    let registered =
        read_object_record(transaction, &supplied.sha256).map_err(|error| match error {
            StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => contract(
                "KNIFE_REFERENCE_INTENT_BUNDLE_CAS_MISSING",
                format!("{role} CAS object is not registered"),
            ),
            other => other,
        })?;
    let reachability_matches = supplied.reachability == registered.reachability
        || (supplied.reachability == "temporary" && registered.reachability == "reachable");
    if !valid_cas_created_at(&registered.created_at)
        || registered.size_bytes != supplied.size_bytes
        || registered.mime != supplied.mime
        || registered.kind != supplied.kind
        || !reachability_matches
        || (require_reachable && registered.reachability != "reachable")
    {
        return Err(contract(
            "KNIFE_REFERENCE_INTENT_BUNDLE_CAS_METADATA_MISMATCH",
            format!("{role} CAS metadata differs from SQLite registration"),
        ));
    }
    let bytes = cas
        .read_verified_bounded(&supplied.sha256, KNIFE_REFERENCE_INTENT_MAX_JSON_BYTES)
        .map_err(StoreError::from)?;
    if bytes.len() as u64 != supplied.size_bytes || sha256_hex(&bytes) != supplied.sha256 {
        return Err(contract(
            "KNIFE_REFERENCE_INTENT_BUNDLE_CAS_HASH_MISMATCH",
            format!("{role} CAS bytes do not match their content hash"),
        ));
    }
    canonical_payload(&bytes, expected_schema, expected_semantic_sha256, role)
}

fn validate_reference_lineage(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    record: &KnifeReferenceIntentBundleStoreRecord,
) -> Result<(), StoreError> {
    let source: Option<(String, String, String, i64, String)> = transaction
        .query_row(
            "SELECT project_id, object_sha256, canonical_sha256, size_bytes, mime FROM reference_evidence WHERE reference_id = ?1",
            params![record.reference_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()?;
    let Some((project_id, object_sha256, evidence_sha256, size_bytes, mime)) = source else {
        return Err(contract(
            "KNIFE_REFERENCE_INTENT_BUNDLE_REFERENCE_MISSING",
            "intent reference_id is not bound to ReferenceEvidence",
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
            "KNIFE_REFERENCE_INTENT_BUNDLE_REFERENCE_BINDING_MISMATCH",
            "intent ReferenceEvidence binding differs from the immutable source row",
        ));
    }
    let expected_size = u64::try_from(size_bytes).map_err(|_| {
        contract(
            "KNIFE_REFERENCE_INTENT_BUNDLE_REFERENCE_BINDING_MISMATCH",
            "intent ReferenceEvidence size is outside the valid range",
        )
    })?;
    let object =
        read_object_record(transaction, &record.reference_object_sha256).map_err(|error| {
            match error {
                StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => contract(
                    "KNIFE_REFERENCE_INTENT_BUNDLE_REFERENCE_CAS_MISSING",
                    "intent source ReferenceEvidence CAS object is not registered",
                ),
                other => other,
            }
        })?;
    if object.size_bytes != expected_size
        || object.mime != mime
        || object.kind != "reference-image"
        || !matches!(object.reachability.as_str(), "temporary" | "reachable")
    {
        return Err(contract(
            "KNIFE_REFERENCE_INTENT_BUNDLE_REFERENCE_CAS_METADATA_MISMATCH",
            "intent source ReferenceEvidence CAS metadata differs from its source row",
        ));
    }
    cas.verify(&record.reference_object_sha256, expected_size)
        .map_err(StoreError::from)?;
    Ok(())
}

fn validate_brief_lineage(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    record: &KnifeReferenceIntentBundleStoreRecord,
) -> Result<(), StoreError> {
    let brief: Option<(String, String, String, String, String, String, String, String, String)> = transaction
        .query_row(
            "SELECT project_id, brief_id, brief_object_sha256, brief_canonical_sha256, reference_id, reference_object_sha256, reference_evidence_sha256, status, conflict_freeze_state FROM weaponry_knife_production_brief_records WHERE project_id = ?1 AND brief_id = ?2",
            params![record.project_id, record.brief_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?)),
        )
        .optional()?;
    let Some((
        project_id,
        brief_id,
        object_sha256,
        semantic_sha256,
        reference_id,
        reference_object_sha256,
        reference_evidence_sha256,
        status,
        conflict_freeze_state,
    )) = brief
    else {
        return Err(contract(
            "KNIFE_REFERENCE_INTENT_BUNDLE_BRIEF_MISSING",
            "intent brief_id is not bound to a durable eligible Brief",
        ));
    };
    if project_id != record.project_id
        || brief_id != record.brief_id
        || object_sha256 != record.brief_object_sha256
        || semantic_sha256 != record.brief_sha256
        || reference_id != record.reference_id
        || reference_object_sha256 != record.reference_object_sha256
        || reference_evidence_sha256 != record.reference_evidence_sha256
        || status != "eligible"
        || conflict_freeze_state != "resolved"
        || !is_sha256(&object_sha256)
        || !is_sha256(&semantic_sha256)
    {
        return Err(contract(
            "KNIFE_REFERENCE_INTENT_BUNDLE_BRIEF_BINDING_MISMATCH",
            "intent Brief identity, semantic hash, object or reference differs from the durable Brief",
        ));
    }
    let object =
        read_object_record(transaction, &record.brief_object_sha256).map_err(
            |error| match error {
                StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => contract(
                    "KNIFE_REFERENCE_INTENT_BUNDLE_BRIEF_CAS_MISSING",
                    "intent Brief CAS object is not registered",
                ),
                other => other,
            },
        )?;
    if object.mime != KNIFE_REFERENCE_INTENT_JSON_MIME
        || object.kind != "weaponry-knife-production-brief"
        || object.reachability != "reachable"
    {
        return Err(contract(
            "KNIFE_REFERENCE_INTENT_BUNDLE_BRIEF_CAS_METADATA_MISMATCH",
            "intent Brief CAS object is not a reachable production brief",
        ));
    }
    cas.verify(&record.brief_object_sha256, object.size_bytes)
        .map_err(StoreError::from)?;
    Ok(())
}

fn roots(record: &KnifeReferenceIntentBundleStoreRecord) -> Vec<String> {
    let mut roots = vec![
        record.intent_bundle_object_sha256.clone(),
        record.intake_manifest_object_sha256.clone(),
        record.detail_inventory_object_sha256.clone(),
        record.quality_contract_object_sha256.clone(),
    ];
    roots.sort();
    roots.dedup();
    roots
}

fn same_record(
    left: &KnifeReferenceIntentBundleStoreRecord,
    right: &KnifeReferenceIntentBundleStoreRecord,
) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    // Runtime may regenerate an intake timestamp while retrying the same
    // immutable idempotency request.  All bindings and payload identities are
    // still exact; only creation time is ignored for replay identity.
    left.created_at.clear();
    right.created_at.clear();
    left == right
}

fn read_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<KnifeReferenceIntentBundleStoreRecord> {
    let payload: String = row.get(0)?;
    let record: KnifeReferenceIntentBundleStoreRecord =
        serde_json::from_str(&payload).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    let indexed: [&str; 19] = [
        row.get_ref(1)?.as_str().unwrap_or_default(),
        row.get_ref(2)?.as_str().unwrap_or_default(),
        row.get_ref(3)?.as_str().unwrap_or_default(),
        row.get_ref(4)?.as_str().unwrap_or_default(),
        row.get_ref(5)?.as_str().unwrap_or_default(),
        row.get_ref(6)?.as_str().unwrap_or_default(),
        row.get_ref(7)?.as_str().unwrap_or_default(),
        row.get_ref(8)?.as_str().unwrap_or_default(),
        row.get_ref(9)?.as_str().unwrap_or_default(),
        row.get_ref(10)?.as_str().unwrap_or_default(),
        row.get_ref(11)?.as_str().unwrap_or_default(),
        row.get_ref(12)?.as_str().unwrap_or_default(),
        row.get_ref(13)?.as_str().unwrap_or_default(),
        row.get_ref(14)?.as_str().unwrap_or_default(),
        row.get_ref(15)?.as_str().unwrap_or_default(),
        row.get_ref(16)?.as_str().unwrap_or_default(),
        row.get_ref(17)?.as_str().unwrap_or_default(),
        row.get_ref(18)?.as_str().unwrap_or_default(),
        row.get_ref(19)?.as_str().unwrap_or_default(),
    ];
    let expected = [
        record.schema_version.as_str(),
        record.intent_bundle_id.as_str(),
        record.project_id.as_str(),
        record.brief_id.as_str(),
        record.brief_sha256.as_str(),
        record.brief_object_sha256.as_str(),
        record.reference_id.as_str(),
        record.reference_object_sha256.as_str(),
        record.reference_evidence_sha256.as_str(),
        record.intake_manifest_sha256.as_str(),
        record.intake_manifest_object_sha256.as_str(),
        record.detail_inventory_sha256.as_str(),
        record.detail_inventory_object_sha256.as_str(),
        record.quality_contract_sha256.as_str(),
        record.quality_contract_object_sha256.as_str(),
        record.intent_bundle_sha256.as_str(),
        record.intent_bundle_object_sha256.as_str(),
        record.idempotency_key.as_str(),
        record.created_at.as_str(),
    ];
    if indexed
        .iter()
        .zip(expected)
        .any(|(actual, expected)| *actual != expected)
    {
        return Err(rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "reference intent index column disagrees with canonical record JSON",
            )),
        ));
    }
    Ok(record)
}

pub(crate) fn ensure_table(connection: &Connection) -> Result<(), StoreError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS knife_reference_intent_bundle_records (
             schema_version TEXT NOT NULL CHECK (schema_version = 'KnifeReferenceIntentBundleStoreRecord@1'),
             intent_bundle_id TEXT NOT NULL,
             project_id TEXT NOT NULL REFERENCES projects(project_id),
             brief_id TEXT NOT NULL,
             brief_sha256 TEXT NOT NULL,
             brief_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             reference_id TEXT NOT NULL REFERENCES reference_evidence(reference_id),
             reference_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             reference_evidence_sha256 TEXT NOT NULL,
             intake_manifest_sha256 TEXT NOT NULL,
             intake_manifest_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             detail_inventory_sha256 TEXT NOT NULL,
             detail_inventory_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             quality_contract_sha256 TEXT NOT NULL,
             quality_contract_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             intent_bundle_sha256 TEXT NOT NULL,
             intent_bundle_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             idempotency_key TEXT NOT NULL,
             created_at TEXT NOT NULL,
             record_json TEXT NOT NULL,
             PRIMARY KEY (project_id, intent_bundle_id),
             UNIQUE (project_id, brief_id),
             UNIQUE (project_id, idempotency_key),
             UNIQUE (project_id, intent_bundle_sha256)
         );",
    )?;
    // Older Slice A development databases predate the explicit bundle id.
    // Recover it only from the stored canonical record JSON; new rows use the
    // full schema and retain the one-bundle-per-Brief V1 policy below.
    let has_identity: Option<String> = connection
        .query_row(
            "SELECT name FROM pragma_table_info('knife_reference_intent_bundle_records') WHERE name = 'intent_bundle_id'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if has_identity.is_none() {
        connection.execute(
            "ALTER TABLE knife_reference_intent_bundle_records ADD COLUMN intent_bundle_id TEXT",
            [],
        )?;
        connection.execute(
            "UPDATE knife_reference_intent_bundle_records SET intent_bundle_id = json_extract(record_json, '$.intent_bundle_id') WHERE intent_bundle_id IS NULL",
            [],
        )?;
    }
    connection.execute_batch(
        "CREATE INDEX IF NOT EXISTS knife_reference_intent_bundle_project_idx
             ON knife_reference_intent_bundle_records(project_id, created_at DESC, brief_id ASC, intent_bundle_id ASC);
         CREATE INDEX IF NOT EXISTS knife_reference_intent_bundle_object_idx
             ON knife_reference_intent_bundle_records(intent_bundle_object_sha256,
                                                       intake_manifest_object_sha256,
                                                       detail_inventory_object_sha256,
                                                       quality_contract_object_sha256);",
    )?;
    Ok(())
}

fn cas_objects(
    commit: &KnifeReferenceIntentBundleCommit,
) -> [(&CasObjectRecord, &str, &str, &str); 4] {
    [
        (
            &commit.cas.intent_bundle,
            KNIFE_REFERENCE_INTENT_BUNDLE_OBJECT_KIND,
            KNIFE_REFERENCE_INTENT_BUNDLE_SCHEMA_VERSION,
            &commit.record.intent_bundle_sha256,
        ),
        (
            &commit.cas.intake_manifest,
            KNIFE_REFERENCE_INTENT_INTAKE_OBJECT_KIND,
            "KnifeIntakeManifest@1",
            &commit.record.intake_manifest_sha256,
        ),
        (
            &commit.cas.detail_inventory,
            KNIFE_REFERENCE_INTENT_DETAIL_OBJECT_KIND,
            "KnifeDetailInventory@1",
            &commit.record.detail_inventory_sha256,
        ),
        (
            &commit.cas.quality_contract,
            KNIFE_REFERENCE_INTENT_QUALITY_OBJECT_KIND,
            "KnifeQualityContract@1",
            &commit.record.quality_contract_sha256,
        ),
    ]
}

impl Store {
    /// Atomically install one Runtime-verified ReferenceIntentBundle.  A
    /// matching key returns `(record, true)` and marks the same roots
    /// reachable; any identity, brief, reference or child binding change is
    /// rejected before another row is written.
    pub fn record_knife_reference_intent_bundle_with_replay(
        &self,
        commit: &KnifeReferenceIntentBundleCommit,
    ) -> Result<(KnifeReferenceIntentBundleStoreRecord, bool), StoreError> {
        validate_record(&commit.record)?;
        let object_hashes = [
            commit.record.intent_bundle_object_sha256.as_str(),
            commit.record.intake_manifest_object_sha256.as_str(),
            commit.record.detail_inventory_object_sha256.as_str(),
            commit.record.quality_contract_object_sha256.as_str(),
        ];
        for ((object, kind, _, _), expected_hash) in cas_objects(commit).iter().zip(object_hashes) {
            if object.sha256 != expected_hash || object.kind != *kind {
                return Err(contract(
                    "KNIFE_REFERENCE_INTENT_BUNDLE_CAS_BINDING_MISMATCH",
                    "intent CAS object hash or kind differs from its typed Store binding",
                ));
            }
        }
        let mut connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let transaction = connection.transaction()?;
        let existing = transaction
            .query_row(
                "SELECT record_json, schema_version, intent_bundle_id, project_id, brief_id, brief_sha256, brief_object_sha256, reference_id, reference_object_sha256, reference_evidence_sha256, intake_manifest_sha256, intake_manifest_object_sha256, detail_inventory_sha256, detail_inventory_object_sha256, quality_contract_sha256, quality_contract_object_sha256, intent_bundle_sha256, intent_bundle_object_sha256, idempotency_key, created_at FROM knife_reference_intent_bundle_records WHERE project_id = ?1 AND idempotency_key = ?2",
                params![commit.record.project_id, commit.record.idempotency_key],
                read_record,
            )
            .optional()?;
        if let Some(existing) = existing {
            validate_record(&existing)?;
            if !same_record(&existing, &commit.record) {
                return Err(contract(
                    "KNIFE_REFERENCE_INTENT_BUNDLE_IDEMPOTENCY_CONFLICT",
                    "intent idempotency key is already bound to a different immutable bundle",
                ));
            }
            validate_brief_lineage(&transaction, &self.cas, &existing)?;
            validate_reference_lineage(&transaction, &self.cas, &existing)?;
            for ((object, kind, schema, semantic), _) in
                cas_objects(commit).iter().zip(object_hashes)
            {
                validate_registered_object(
                    &transaction,
                    &self.cas,
                    object,
                    kind,
                    schema,
                    semantic,
                    false,
                    "KNIFE_REFERENCE_INTENT_BUNDLE_REPLAY",
                )?;
            }
            let bundle_bytes = self.cas.read_verified_bounded(
                &existing.intent_bundle_object_sha256,
                KNIFE_REFERENCE_INTENT_MAX_JSON_BYTES,
            )?;
            validate_bundle_payload(&existing, &bundle_bytes)?;
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
                "intent bundle project does not exist",
            ));
        }
        let duplicate_brief: Option<String> = transaction
            .query_row(
                "SELECT idempotency_key FROM knife_reference_intent_bundle_records WHERE project_id = ?1 AND brief_id = ?2",
                params![commit.record.project_id, commit.record.brief_id],
                |row| row.get(0),
            )
            .optional()?;
        if duplicate_brief.is_some() {
            return Err(contract(
                "KNIFE_REFERENCE_INTENT_BUNDLE_BRIEF_CONFLICT",
                "project/brief identity is already bound to another immutable intent bundle",
            ));
        }
        let duplicate_bundle: Option<String> = transaction
            .query_row(
                "SELECT idempotency_key FROM knife_reference_intent_bundle_records WHERE project_id = ?1 AND intent_bundle_sha256 = ?2",
                params![commit.record.project_id, commit.record.intent_bundle_sha256],
                |row| row.get(0),
            )
            .optional()?;
        if duplicate_bundle.is_some() {
            return Err(contract(
                "KNIFE_REFERENCE_INTENT_BUNDLE_CANONICAL_CONFLICT",
                "project/bundle semantic hash is already bound to another immutable bundle",
            ));
        }
        validate_brief_lineage(&transaction, &self.cas, &commit.record)?;
        validate_reference_lineage(&transaction, &self.cas, &commit.record)?;
        for ((object, kind, schema, semantic), _) in cas_objects(commit).iter().zip(object_hashes) {
            validate_registered_object(
                &transaction,
                &self.cas,
                object,
                kind,
                schema,
                semantic,
                false,
                "KNIFE_REFERENCE_INTENT_BUNDLE",
            )?;
        }
        let bundle_bytes = self.cas.read_verified_bounded(
            &commit.record.intent_bundle_object_sha256,
            KNIFE_REFERENCE_INTENT_MAX_JSON_BYTES,
        )?;
        validate_bundle_payload(&commit.record, &bundle_bytes)?;
        let record_json = String::from_utf8(record_bytes(&commit.record)?).map_err(|error| {
            StoreError::InvalidData(format!("intent durable record is not UTF-8: {error}"))
        })?;
        transaction.execute(
            "INSERT INTO knife_reference_intent_bundle_records (schema_version, intent_bundle_id, project_id, brief_id, brief_sha256, brief_object_sha256, reference_id, reference_object_sha256, reference_evidence_sha256, intake_manifest_sha256, intake_manifest_object_sha256, detail_inventory_sha256, detail_inventory_object_sha256, quality_contract_sha256, quality_contract_object_sha256, intent_bundle_sha256, intent_bundle_object_sha256, idempotency_key, created_at, record_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
            params![
                commit.record.schema_version,
                commit.record.intent_bundle_id,
                commit.record.project_id,
                commit.record.brief_id,
                commit.record.brief_sha256,
                commit.record.brief_object_sha256,
                commit.record.reference_id,
                commit.record.reference_object_sha256,
                commit.record.reference_evidence_sha256,
                commit.record.intake_manifest_sha256,
                commit.record.intake_manifest_object_sha256,
                commit.record.detail_inventory_sha256,
                commit.record.detail_inventory_object_sha256,
                commit.record.quality_contract_sha256,
                commit.record.quality_contract_object_sha256,
                commit.record.intent_bundle_sha256,
                commit.record.intent_bundle_object_sha256,
                commit.record.idempotency_key,
                commit.record.created_at,
                record_json,
            ],
        )?;
        mark_reachable_in_transaction(&transaction, &roots(&commit.record))?;
        let stored = transaction.query_row(
            "SELECT record_json, schema_version, intent_bundle_id, project_id, brief_id, brief_sha256, brief_object_sha256, reference_id, reference_object_sha256, reference_evidence_sha256, intake_manifest_sha256, intake_manifest_object_sha256, detail_inventory_sha256, detail_inventory_object_sha256, quality_contract_sha256, quality_contract_object_sha256, intent_bundle_sha256, intent_bundle_object_sha256, idempotency_key, created_at FROM knife_reference_intent_bundle_records WHERE project_id = ?1 AND brief_id = ?2 AND intent_bundle_id = ?3",
            params![commit.record.project_id, commit.record.brief_id, commit.record.intent_bundle_id],
            read_record,
        )?;
        validate_record(&stored)?;
        transaction.commit()?;
        Ok((stored, false))
    }

    /// Exact project/brief/semantic bundle lookup.  All four JSON roots and
    /// the Brief/Reference lineage are revalidated in this read transaction.
    pub fn get_knife_reference_intent_bundle(
        &self,
        project_id: &str,
        brief_id: &str,
        intent_bundle_id: &str,
        intent_bundle_sha256: &str,
    ) -> Result<Option<KnifeReferenceIntentBundleStoreRecord>, StoreError> {
        if !is_opaque_id(project_id)
            || !is_opaque_id(brief_id)
            || !is_opaque_id(intent_bundle_id)
            || !is_sha256(intent_bundle_sha256)
        {
            return Err(StoreError::InvalidData(
                "intent bundle lookup identity is invalid".to_owned(),
            ));
        }
        let mut connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let transaction = connection.transaction()?;
        let record = transaction
            .query_row(
                "SELECT record_json, schema_version, intent_bundle_id, project_id, brief_id, brief_sha256, brief_object_sha256, reference_id, reference_object_sha256, reference_evidence_sha256, intake_manifest_sha256, intake_manifest_object_sha256, detail_inventory_sha256, detail_inventory_object_sha256, quality_contract_sha256, quality_contract_object_sha256, intent_bundle_sha256, intent_bundle_object_sha256, idempotency_key, created_at FROM knife_reference_intent_bundle_records WHERE project_id = ?1 AND brief_id = ?2 AND intent_bundle_id = ?3 AND intent_bundle_sha256 = ?4",
                params![project_id, brief_id, intent_bundle_id, intent_bundle_sha256],
                read_record,
            )
            .optional()?;
        let Some(record) = record else {
            transaction.commit()?;
            return Ok(None);
        };
        validate_record(&record)?;
        validate_brief_lineage(&transaction, &self.cas, &record)?;
        validate_reference_lineage(&transaction, &self.cas, &record)?;
        for (object_sha256, kind, schema, semantic) in [
            (
                record.intent_bundle_object_sha256.as_str(),
                KNIFE_REFERENCE_INTENT_BUNDLE_OBJECT_KIND,
                KNIFE_REFERENCE_INTENT_BUNDLE_SCHEMA_VERSION,
                record.intent_bundle_sha256.as_str(),
            ),
            (
                record.intake_manifest_object_sha256.as_str(),
                KNIFE_REFERENCE_INTENT_INTAKE_OBJECT_KIND,
                "KnifeIntakeManifest@1",
                record.intake_manifest_sha256.as_str(),
            ),
            (
                record.detail_inventory_object_sha256.as_str(),
                KNIFE_REFERENCE_INTENT_DETAIL_OBJECT_KIND,
                "KnifeDetailInventory@1",
                record.detail_inventory_sha256.as_str(),
            ),
            (
                record.quality_contract_object_sha256.as_str(),
                KNIFE_REFERENCE_INTENT_QUALITY_OBJECT_KIND,
                "KnifeQualityContract@1",
                record.quality_contract_sha256.as_str(),
            ),
        ] {
            let object =
                read_object_record(&transaction, object_sha256).map_err(|error| match error {
                    StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => contract(
                        "KNIFE_REFERENCE_INTENT_BUNDLE_CAS_MISSING",
                        "intent CAS root disappeared before restart readback",
                    ),
                    other => other,
                })?;
            let value = validate_registered_object(
                &transaction,
                &self.cas,
                &object,
                kind,
                schema,
                semantic,
                true,
                "KNIFE_REFERENCE_INTENT_BUNDLE_GET",
            )?;
            if kind == KNIFE_REFERENCE_INTENT_BUNDLE_OBJECT_KIND {
                let bytes = canonical_json_bytes(&value)
                    .map_err(|error| StoreError::InvalidData(error.to_string()))?;
                validate_bundle_payload(&record, &bytes)?;
            }
        }
        transaction.commit()?;
        Ok(Some(record))
    }

    pub fn get_knife_reference_intent_bundle_exact(
        &self,
        project_id: &str,
        brief_id: &str,
        brief_sha256: &str,
        brief_object_sha256: &str,
        reference_id: &str,
        reference_object_sha256: &str,
        reference_evidence_sha256: &str,
        intent_bundle_id: &str,
        intent_bundle_sha256: &str,
        intent_bundle_object_sha256: &str,
    ) -> Result<Option<KnifeReferenceIntentBundleStoreRecord>, StoreError> {
        if !is_opaque_id(project_id)
            || !is_opaque_id(brief_id)
            || !is_sha256(brief_sha256)
            || !is_sha256(brief_object_sha256)
            || !is_opaque_id(reference_id)
            || !is_sha256(reference_object_sha256)
            || !is_sha256(reference_evidence_sha256)
            || !is_opaque_id(intent_bundle_id)
            || !is_sha256(intent_bundle_sha256)
            || !is_sha256(intent_bundle_object_sha256)
        {
            return Err(StoreError::InvalidData(
                "intent bundle exact lookup binding is invalid".to_owned(),
            ));
        }
        let Some(record) = self.get_knife_reference_intent_bundle(
            project_id,
            brief_id,
            intent_bundle_id,
            intent_bundle_sha256,
        )?
        else {
            return Ok(None);
        };
        if record.brief_sha256 != brief_sha256
            || record.brief_object_sha256 != brief_object_sha256
            || record.reference_id != reference_id
            || record.reference_object_sha256 != reference_object_sha256
            || record.reference_evidence_sha256 != reference_evidence_sha256
            || record.intent_bundle_id != intent_bundle_id
            || record.intent_bundle_sha256 != intent_bundle_sha256
            || record.intent_bundle_object_sha256 != intent_bundle_object_sha256
        {
            return Err(contract(
                "KNIFE_REFERENCE_INTENT_BUNDLE_EXACT_BINDING_MISMATCH",
                "intent exact lookup binding differs from the immutable record",
            ));
        }
        Ok(Some(record))
    }

    pub fn get_knife_reference_intent_bundle_by_idempotency(
        &self,
        project_id: &str,
        idempotency_key: &str,
    ) -> Result<Option<KnifeReferenceIntentBundleStoreRecord>, StoreError> {
        if !is_opaque_id(project_id) || !is_opaque_id(idempotency_key) {
            return Err(StoreError::InvalidData(
                "intent bundle idempotency lookup identity is invalid".to_owned(),
            ));
        }
        let mut connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let transaction = connection.transaction()?;
        let record = transaction
            .query_row(
                "SELECT record_json, schema_version, intent_bundle_id, project_id, brief_id, brief_sha256, brief_object_sha256, reference_id, reference_object_sha256, reference_evidence_sha256, intake_manifest_sha256, intake_manifest_object_sha256, detail_inventory_sha256, detail_inventory_object_sha256, quality_contract_sha256, quality_contract_object_sha256, intent_bundle_sha256, intent_bundle_object_sha256, idempotency_key, created_at FROM knife_reference_intent_bundle_records WHERE project_id = ?1 AND idempotency_key = ?2",
                params![project_id, idempotency_key],
                read_record,
            )
            .optional()?;
        let Some(record) = record else {
            transaction.commit()?;
            return Ok(None);
        };
        validate_record(&record)?;
        validate_brief_lineage(&transaction, &self.cas, &record)?;
        validate_reference_lineage(&transaction, &self.cas, &record)?;
        transaction.commit()?;
        self.get_knife_reference_intent_bundle(
            &record.project_id,
            &record.brief_id,
            &record.intent_bundle_id,
            &record.intent_bundle_sha256,
        )
    }

    pub fn read_knife_reference_intent_bundle_json(
        &self,
        project_id: &str,
        brief_id: &str,
        intent_bundle_id: &str,
        intent_bundle_sha256: &str,
    ) -> Result<Option<Value>, StoreError> {
        let Some(record) = self.get_knife_reference_intent_bundle(
            project_id,
            brief_id,
            intent_bundle_id,
            intent_bundle_sha256,
        )?
        else {
            return Ok(None);
        };
        let bytes = self.cas.read_verified_bounded(
            &record.intent_bundle_object_sha256,
            KNIFE_REFERENCE_INTENT_MAX_JSON_BYTES,
        )?;
        Ok(Some(validate_bundle_payload(&record, &bytes)?))
    }

    pub fn knife_reference_intent_bundle_cas_roots(
        record: &KnifeReferenceIntentBundleStoreRecord,
    ) -> Vec<String> {
        roots(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProjectRecord, ReferenceAuthorization, ReferenceEvidenceRecord};
    use forgecad_core::canonical_json_hash;
    use serde_json::json;
    use std::fs;
    use uuid::Uuid;

    const PROJECT: &str = "knife-intent-store-project";
    const BRIEF: &str = "dragonfang-intent-brief";
    const REFERENCE: &str = "dragonfang-intent-reference";

    fn project(store: &Store) {
        store
            .insert_project(&ProjectRecord {
                schema_version: "Project@1".to_owned(),
                project_id: PROJECT.to_owned(),
                name: "Knife intent Store test".to_owned(),
                policy: json!({"scope":"test"}),
                created_at: "2026-08-30T00:00:00Z".to_owned(),
                updated_at: "2026-08-30T00:00:00Z".to_owned(),
                active_snapshot_revision: 0,
                head_snapshot_id: None,
                canonical_sha256: "a".repeat(64),
            })
            .expect("project");
    }

    fn brief_and_reference(store: &Store) -> (String, String, String) {
        let source = store
            .put_object(
                b"knife-intent-reference",
                None,
                "image/png",
                "reference-image",
                "2026-08-30T00:00:00Z",
            )
            .expect("reference CAS");
        let authorization = ReferenceAuthorization {
            user_authorized: true,
            declaration: "authorized test source".to_owned(),
        };
        let evidence_hash = canonical_json_hash(&json!({
            "schema_version": "ReferenceEvidence@1",
            "reference_id": REFERENCE,
            "project_id": PROJECT,
            "object_sha256": source.record.sha256,
            "mime": "image/png",
            "size_bytes": source.record.size_bytes,
            "width": 1,
            "height": 1,
            "frame_count": 1,
            "import_mode": "inline_content",
            "authorization": authorization,
            "derived_object_sha256": Value::Null,
            "created_at": "2026-08-30T00:00:00Z",
        }));
        store
            .insert_reference_evidence(&ReferenceEvidenceRecord {
                schema_version: "ReferenceEvidence@1".to_owned(),
                reference_id: REFERENCE.to_owned(),
                project_id: PROJECT.to_owned(),
                object_sha256: source.record.sha256.clone(),
                mime: "image/png".to_owned(),
                size_bytes: source.record.size_bytes,
                width: 1,
                height: 1,
                frame_count: 1,
                import_mode: "inline_content".to_owned(),
                authorization,
                derived_object_sha256: None,
                canonical_sha256: evidence_hash.clone(),
                created_at: "2026-08-30T00:00:00Z".to_owned(),
            })
            .expect("reference evidence");
        let brief_value = json!({
            "schema_version": "WeaponryKnifeProductionBrief@1",
            "brief_id": BRIEF,
            "project_id": PROJECT,
            "canonical_sha256": "brief-semantic-placeholder"
        });
        let brief_bytes = canonical_json_bytes(&brief_value).expect("brief JSON");
        let brief = store
            .put_object(
                &brief_bytes,
                None,
                KNIFE_REFERENCE_INTENT_JSON_MIME,
                "weaponry-knife-production-brief",
                "2026-08-30T00:00:00Z",
            )
            .expect("brief CAS");
        let connection = store.connection.lock().expect("connection");
        connection
            .execute(
                "UPDATE objects SET reachability = 'reachable' WHERE sha256 = ?1",
                params![brief.record.sha256],
            )
            .expect("brief root");
        connection
            .execute(
                "INSERT INTO weaponry_knife_production_brief_records (schema_version, project_id, brief_id, brief_object_sha256, brief_canonical_sha256, reference_id, reference_object_sha256, reference_evidence_sha256, parent_brief_id, parent_brief_sha256, freeze_policy, source_reference_hashes_json, status, conflict_freeze_state, idempotency_key, created_at, record_json) VALUES ('WeaponryKnifeProductionBriefStoreRecord@1', ?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, NULL, 'initial-intake-no-parent@1', ?8, 'eligible', 'resolved', 'brief-key', '2026-08-30T00:00:00Z', ?9)",
                params![
                    PROJECT,
                    BRIEF,
                    brief.record.sha256,
                    "b".repeat(64),
                    REFERENCE,
                    source.record.sha256,
                    evidence_hash,
                    serde_json::to_string(&vec![source.record.sha256.clone()]).expect("refs"),
                    serde_json::to_string(&json!({"brief_id": BRIEF})).expect("record"),
                ],
            )
            .expect("brief row");
        (source.record.sha256, evidence_hash, brief.record.sha256)
    }

    #[test]
    fn intent_bundle_id_migration_recovers_existing_record_identity() {
        let connection = Connection::open_in_memory().expect("connection");
        connection
            .execute_batch(
                "CREATE TABLE knife_reference_intent_bundle_records (
                    schema_version TEXT, project_id TEXT, brief_id TEXT,
                    brief_sha256 TEXT, brief_object_sha256 TEXT, reference_id TEXT,
                    reference_object_sha256 TEXT, reference_evidence_sha256 TEXT,
                    intake_manifest_sha256 TEXT, intake_manifest_object_sha256 TEXT,
                    detail_inventory_sha256 TEXT, detail_inventory_object_sha256 TEXT,
                    quality_contract_sha256 TEXT, quality_contract_object_sha256 TEXT,
                    intent_bundle_sha256 TEXT, intent_bundle_object_sha256 TEXT,
                    idempotency_key TEXT, created_at TEXT, record_json TEXT
                );
                INSERT INTO knife_reference_intent_bundle_records
                    (record_json) VALUES ('{\"intent_bundle_id\":\"migrated-intent\"}');",
            )
            .expect("legacy table");
        ensure_table(&connection).expect("migrate identity");
        let migrated: String = connection
            .query_row(
                "SELECT intent_bundle_id FROM knife_reference_intent_bundle_records",
                [],
                |row| row.get(0),
            )
            .expect("identity");
        assert_eq!(migrated, "migrated-intent");
    }

    fn child(
        store: &Store,
        schema: &str,
        kind: &str,
        name: &str,
    ) -> (CasObjectRecord, String, Value) {
        let mut value = json!({"schema_version": schema, "name": name, "canonical_sha256": ""});
        let semantic = canonical_json_hash(&value);
        value["canonical_sha256"] = Value::String(semantic.clone());
        let bytes = canonical_json_bytes(&value).expect("child JSON");
        let object = store
            .put_object(
                &bytes,
                None,
                KNIFE_REFERENCE_INTENT_JSON_MIME,
                kind,
                "2026-08-30T00:00:00Z",
            )
            .expect("child CAS");
        (object.record, semantic, value)
    }

    fn commit(store: &Store) -> KnifeReferenceIntentBundleCommit {
        let (reference_object, evidence, brief_object) = brief_and_reference(store);
        let intake = child(
            store,
            "KnifeIntakeManifest@1",
            KNIFE_REFERENCE_INTENT_INTAKE_OBJECT_KIND,
            "intake",
        );
        let detail = child(
            store,
            "KnifeDetailInventory@1",
            KNIFE_REFERENCE_INTENT_DETAIL_OBJECT_KIND,
            "detail",
        );
        let quality = child(
            store,
            "KnifeQualityContract@1",
            KNIFE_REFERENCE_INTENT_QUALITY_OBJECT_KIND,
            "quality",
        );
        let mut bundle_value = json!({
            "schema_version": KNIFE_REFERENCE_INTENT_BUNDLE_SCHEMA_VERSION,
            "intent_bundle_id": "intent-bundle-1",
            "project_id": PROJECT,
            "brief_binding": {
                "brief_schema_version": "WeaponryKnifeProductionBrief@1",
                "brief_id": BRIEF,
                "brief_sha256": "b".repeat(64),
                "brief_object_sha256": brief_object,
                "authoring_eligibility": "ELIGIBLE",
                "authorization_binding_status": "runtime-bound"
            },
            "reference_binding": {
                "reference_id": REFERENCE,
                "reference_object_sha256": reference_object,
                "reference_evidence_sha256": evidence,
                "binding_status": "runtime-bound"
            },
            "route": "reference-projection",
            "exactness": "image-only",
            "intake_manifest": intake.2,
            "detail_inventory": detail.2,
            "quality_contract": quality.2,
            "unknowns": [],
            "canonicalization_policy": "canonical-json-sha256-excluding-canonical-sha256@1",
            "canonical_sha256": ""
        });
        let bundle_semantic = canonical_json_hash(&bundle_value);
        bundle_value["canonical_sha256"] = Value::String(bundle_semantic.clone());
        let bundle_bytes = canonical_json_bytes(&bundle_value).expect("bundle JSON");
        let bundle = store
            .put_object(
                &bundle_bytes,
                None,
                KNIFE_REFERENCE_INTENT_JSON_MIME,
                KNIFE_REFERENCE_INTENT_BUNDLE_OBJECT_KIND,
                "2026-08-30T00:00:00Z",
            )
            .expect("bundle CAS");
        KnifeReferenceIntentBundleCommit {
            record: KnifeReferenceIntentBundleStoreRecord {
                schema_version: KNIFE_REFERENCE_INTENT_BUNDLE_RECORD_SCHEMA_VERSION.to_owned(),
                intent_bundle_id: "intent-bundle-1".to_owned(),
                project_id: PROJECT.to_owned(),
                brief_id: BRIEF.to_owned(),
                brief_sha256: "b".repeat(64),
                brief_object_sha256: brief_object,
                reference_id: REFERENCE.to_owned(),
                reference_object_sha256: reference_object,
                reference_evidence_sha256: evidence,
                intake_manifest_sha256: intake.1,
                intake_manifest_object_sha256: intake.0.sha256.clone(),
                detail_inventory_sha256: detail.1,
                detail_inventory_object_sha256: detail.0.sha256.clone(),
                quality_contract_sha256: quality.1,
                quality_contract_object_sha256: quality.0.sha256.clone(),
                intent_bundle_sha256: bundle_semantic,
                intent_bundle_object_sha256: bundle.record.sha256.clone(),
                idempotency_key: "intent-key".to_owned(),
                created_at: "2026-08-30T00:00:00Z".to_owned(),
            },
            cas: KnifeReferenceIntentBundleCasBundle {
                intent_bundle: bundle.record,
                intake_manifest: intake.0,
                detail_inventory: detail.0,
                quality_contract: quality.0,
            },
        }
    }

    #[test]
    fn intent_bundle_commit_replay_get_and_roots_are_exact() {
        let store = Store::memory().expect("store");
        project(&store);
        let commit = commit(&store);
        let (record, replayed) = store
            .record_knife_reference_intent_bundle_with_replay(&commit)
            .expect("commit");
        assert!(!replayed);
        assert_eq!(record, commit.record);
        for hash in Store::knife_reference_intent_bundle_cas_roots(&record) {
            assert_eq!(
                store
                    .get_object(&hash)
                    .expect("object")
                    .expect("metadata")
                    .reachability,
                "reachable"
            );
        }
        let (again, replayed) = store
            .record_knife_reference_intent_bundle_with_replay(&commit)
            .expect("replay");
        assert!(replayed);
        assert_eq!(again, record);
        assert_eq!(
            store
                .get_knife_reference_intent_bundle(
                    PROJECT,
                    BRIEF,
                    &record.intent_bundle_id,
                    &record.intent_bundle_sha256,
                )
                .expect("get"),
            Some(record.clone())
        );
        assert!(store
            .read_knife_reference_intent_bundle_json(
                PROJECT,
                BRIEF,
                &record.intent_bundle_id,
                &record.intent_bundle_sha256,
            )
            .expect("json")
            .is_some());
        assert!(store
            .get_knife_reference_intent_bundle_exact(
                PROJECT,
                BRIEF,
                &record.brief_sha256,
                &record.brief_object_sha256,
                REFERENCE,
                &record.reference_object_sha256,
                &record.reference_evidence_sha256,
                "unknown-intent-bundle",
                &record.intent_bundle_sha256,
                &record.intent_bundle_object_sha256,
            )
            .expect("wrong id lookup")
            .is_none());
    }

    #[test]
    fn intent_bundle_exact_replay_accepts_distinct_cas_created_at() {
        let store = Store::memory().expect("store");
        project(&store);
        let original = commit(&store);
        let (record, replayed) = store
            .record_knife_reference_intent_bundle_with_replay(&original)
            .expect("commit");
        assert!(!replayed);

        let replay_created_at = "2026-08-31T00:00:00Z";
        let mut replay = original.clone();
        replay.record.created_at = replay_created_at.to_owned();
        replay.cas.intent_bundle.created_at = replay_created_at.to_owned();
        replay.cas.intake_manifest.created_at = replay_created_at.to_owned();
        replay.cas.detail_inventory.created_at = replay_created_at.to_owned();
        replay.cas.quality_contract.created_at = replay_created_at.to_owned();

        let (again, replayed) = store
            .record_knife_reference_intent_bundle_with_replay(&replay)
            .expect("exact replay with a new CAS timestamp");
        assert!(replayed);
        assert_eq!(again, record);

        for object in [
            &replay.cas.intent_bundle,
            &replay.cas.intake_manifest,
            &replay.cas.detail_inventory,
            &replay.cas.quality_contract,
        ] {
            assert_eq!(object.created_at, replay_created_at);
            assert_ne!(object.created_at, record.created_at);
            let registered = store
                .get_object(&object.sha256)
                .expect("registered metadata")
                .expect("CAS object");
            assert_eq!(registered.created_at, original.cas.intent_bundle.created_at);
            assert_eq!(registered.reachability, "reachable");
        }
    }

    #[test]
    fn intent_bundle_replay_validates_supplied_and_registered_cas_created_at_bounds() {
        let store = Store::memory().expect("store");
        project(&store);
        let original = commit(&store);
        store
            .record_knife_reference_intent_bundle_with_replay(&original)
            .expect("commit");

        for supplied_created_at in [String::new(), "x".repeat(129)] {
            let mut replay = original.clone();
            replay.cas.detail_inventory.created_at = supplied_created_at;
            let error = store
                .record_knife_reference_intent_bundle_with_replay(&replay)
                .expect_err("invalid supplied CAS timestamp");
            assert!(matches!(
                error,
                StoreError::Contract { code, .. }
                    if code == "KNIFE_REFERENCE_INTENT_BUNDLE_CAS_METADATA_INVALID"
            ));
        }

        for registered_created_at in [String::new(), "x".repeat(129)] {
            {
                let connection = store.connection.lock().expect("connection");
                connection
                    .execute(
                        "UPDATE objects SET created_at = ?1 WHERE sha256 = ?2",
                        params![registered_created_at, original.cas.detail_inventory.sha256],
                    )
                    .expect("registered timestamp");
            }
            let error = store
                .record_knife_reference_intent_bundle_with_replay(&original)
                .expect_err("invalid registered CAS timestamp");
            assert!(matches!(
                error,
                StoreError::Contract { code, .. }
                    if code == "KNIFE_REFERENCE_INTENT_BUNDLE_CAS_METADATA_MISMATCH"
            ));
        }
    }

    #[test]
    fn intent_bundle_same_key_conflict_and_exact_binding_fail_closed() {
        let store = Store::memory().expect("store");
        project(&store);
        let original = commit(&store);
        store
            .record_knife_reference_intent_bundle_with_replay(&original)
            .expect("original");
        let mut conflict = original.clone();
        conflict.record.brief_sha256 = "c".repeat(64);
        let error = store
            .record_knife_reference_intent_bundle_with_replay(&conflict)
            .expect_err("same-key conflict");
        assert!(
            matches!(error, StoreError::Contract { code, .. } if code == "KNIFE_REFERENCE_INTENT_BUNDLE_IDEMPOTENCY_CONFLICT")
        );
        let error = store
            .get_knife_reference_intent_bundle_exact(
                PROJECT,
                BRIEF,
                &"c".repeat(64),
                &original.record.brief_object_sha256,
                REFERENCE,
                &original.record.reference_object_sha256,
                &original.record.reference_evidence_sha256,
                &original.record.intent_bundle_id,
                &original.record.intent_bundle_sha256,
                &original.record.intent_bundle_object_sha256,
            )
            .expect_err("exact mismatch");
        assert!(
            matches!(error, StoreError::Contract { code, .. } if code == "KNIFE_REFERENCE_INTENT_BUNDLE_EXACT_BINDING_MISMATCH")
        );
    }

    #[test]
    fn intent_bundle_restart_readback_and_cas_tamper_fail_closed() {
        let root = std::env::temp_dir().join(format!("forgecad-knife-intent-{}", Uuid::new_v4()));
        let database = root.join("runtime.sqlite");
        let cas_root = root.join("runtime.cas");
        let record;
        {
            let store = Store::open_with_cas(&database, &cas_root).expect("open");
            project(&store);
            let commit = commit(&store);
            record = store
                .record_knife_reference_intent_bundle_with_replay(&commit)
                .expect("commit")
                .0;
        }
        let reopened = Store::open_with_cas(&database, &cas_root).expect("reopen");
        assert_eq!(
            reopened
                .get_knife_reference_intent_bundle(
                    PROJECT,
                    BRIEF,
                    &record.intent_bundle_id,
                    &record.intent_bundle_sha256,
                )
                .expect("readback"),
            Some(record.clone())
        );
        let path = reopened
            .cas()
            .root()
            .join("objects")
            .join(&record.intent_bundle_object_sha256[..2])
            .join(&record.intent_bundle_object_sha256);
        fs::write(&path, b"tampered").expect("tamper");
        let error = reopened
            .get_knife_reference_intent_bundle(
                PROJECT,
                BRIEF,
                &record.intent_bundle_id,
                &record.intent_bundle_sha256,
            )
            .expect_err("tampered CAS");
        assert!(matches!(
            error,
            StoreError::Cas(_) | StoreError::Contract { .. }
        ));
        let _ = fs::remove_dir_all(root);
    }
}
