//! Runtime-owned reference-intent intake for the knife High slice.
//!
//! This module is deliberately an intake boundary.  It accepts a closed,
//! hash-bound observation bundle, proves that the supplied Brief is the exact
//! Runtime-owned eligible Brief and that its ReferenceEvidence is still the
//! same source object, then stages the four immutable JSON roots and asks
//! Store to install them in one transaction.  No geometry, High mesh,
//! candidate, version, export or human-review state is created here.

#[cfg(test)]
use super::sha256_hex;
use super::{
    canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256, Runtime, RuntimeError,
};
use forgecad_contracts::ReferenceEvidenceRecord;
use forgecad_store::{
    CasObject, KnifeReferenceIntentBundleCasBundle, KnifeReferenceIntentBundleCommit,
    KnifeReferenceIntentBundleStoreRecord, KNIFE_REFERENCE_INTENT_BUNDLE_OBJECT_KIND,
    KNIFE_REFERENCE_INTENT_BUNDLE_RECORD_SCHEMA_VERSION, KNIFE_REFERENCE_INTENT_DETAIL_OBJECT_KIND,
    KNIFE_REFERENCE_INTENT_INTAKE_OBJECT_KIND, KNIFE_REFERENCE_INTENT_JSON_MIME,
    KNIFE_REFERENCE_INTENT_MAX_JSON_BYTES, KNIFE_REFERENCE_INTENT_QUALITY_OBJECT_KIND,
};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

pub(crate) const BUNDLE_SCHEMA: &str = "KnifeReferenceIntentBundle@1";
pub(crate) const PREPARE_SCHEMA: &str = "KnifeReferenceIntentBundlePrepareRequest@1";
pub(crate) const GET_SCHEMA: &str = "KnifeReferenceIntentBundleGetRequest@1";
pub(crate) const RESULT_SCHEMA: &str = "KnifeReferenceIntentBundleResult@1";
pub(crate) const PREPARE_OPERATION: &str = "knife_reference_intent_bundle_prepare";
pub(crate) const GET_OPERATION: &str = "knife_reference_intent_bundle_get";
pub(crate) const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
pub(crate) const REQUEST_CANONICALIZATION: &str = "canonical-json-sha256-excluding-input-sha256@1";
pub(crate) const BUNDLE_CANONICALIZATION: &str =
    "canonical-json-sha256-excluding-canonical-sha256@1";
const MAX_RESPONSE_BYTES: usize = 1_048_576;
const MAX_REFERENCE_BYTES: u64 = 8 * 1024 * 1024;
const PENDING_THRESHOLD_FIXTURE_SHA256: &str =
    "cc99e2c26c147b27e59bb8544334ec24adfbd83f4e723d0c21599ebaf7304b4b";

const BUNDLE_FIELDS: &[&str] = &[
    "schema_version",
    "intent_bundle_id",
    "project_id",
    "brief_binding",
    "reference_binding",
    "route",
    "exactness",
    "intake_manifest",
    "detail_inventory",
    "quality_contract",
    "unknowns",
    "canonicalization_policy",
    "canonical_sha256",
    "created_at",
];
const BRIEF_BINDING_FIELDS: &[&str] = &[
    "brief_schema_version",
    "brief_id",
    "brief_sha256",
    "brief_object_sha256",
    "authoring_eligibility",
    "authorization_binding_status",
];
const REFERENCE_BINDING_FIELDS: &[&str] = &[
    "reference_id",
    "reference_object_sha256",
    "reference_evidence_sha256",
    "binding_status",
];
const INTAKE_FIELDS: &[&str] = &[
    "schema_version",
    "manifest_id",
    "records",
    "canonicalization_policy",
    "canonical_sha256",
];
const INTAKE_RECORD_FIELDS: &[&str] = &[
    "reference_id",
    "reference_object_sha256",
    "reference_evidence_sha256",
    "role",
    "resolution",
    "decode_status",
    "duplicate_status",
    "admission_status",
    "visible_coverage",
];
const COVERAGE_FIELDS: &[&str] = &["view", "status"];
const DIMENSION_FIELDS: &[&str] = &["width", "height"];
const DETAIL_INVENTORY_FIELDS: &[&str] = &[
    "schema_version",
    "inventory_id",
    "details",
    "canonicalization_policy",
    "canonical_sha256",
];
const DETAIL_FIELDS: &[&str] = &[
    "detail_id",
    "scale",
    "family",
    "label",
    "evidence_regions",
    "confidence",
    "observation_status",
    "target",
    "priority",
    "high_action",
];
const EVIDENCE_REGION_FIELDS: &[&str] = &[
    "reference_id",
    "view",
    "x",
    "y",
    "width",
    "height",
    "status",
];
const DETAIL_TARGET_FIELDS: &[&str] = &["target_kind", "target_id", "mapping_status"];
const QUALITY_FIELDS: &[&str] = &[
    "schema_version",
    "contract_id",
    "stage_order",
    "critical_features",
    "fixed_views",
    "blocking_failures",
    "threshold_fixture_sha256",
    "threshold_status",
    "correction_policy",
    "promotion_state",
    "canonicalization_policy",
    "canonical_sha256",
];
const CRITICAL_FEATURE_FIELDS: &[&str] = &[
    "feature_id",
    "feature_kind",
    "target_id",
    "source_status",
    "blocking",
    "evidence_region_ids",
];
const FIXED_VIEW_FIELDS: &[&str] = &["view_id", "view", "comparison_role", "reference_required"];
const BLOCKING_FAILURE_FIELDS: &[&str] =
    &["failure_id", "gate_id", "condition", "blocks_promotion"];
const CORRECTION_FIELDS: &[&str] = &[
    "max_iterations_per_pass",
    "max_iterations_total",
    "one_changed_scope_per_iteration",
    "baseline_preserved",
];
const UNKNOWN_FIELDS: &[&str] = &[
    "unknown_id",
    "topic",
    "view",
    "description",
    "impact",
    "resolution_status",
];
const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "project_id",
    "brief_id",
    "brief_sha256",
    "brief_object_sha256",
    "reference_id",
    "reference_object_sha256",
    "reference_evidence_sha256",
    "brief_authoring_eligibility",
    "intent_bundle",
    "idempotency_key",
    "max_response_bytes",
    "runtime_write_performed",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];
const GET_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "project_id",
    "brief_id",
    "brief_sha256",
    "brief_object_sha256",
    "reference_id",
    "reference_object_sha256",
    "reference_evidence_sha256",
    "brief_authoring_eligibility",
    "intent_bundle_id",
    "intent_bundle_sha256",
    "intent_bundle_object_sha256",
    "max_response_bytes",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];

const VIEWS: &[&str] = &[
    "front",
    "back",
    "left",
    "right",
    "front-three-quarter",
    "rear-three-quarter",
    "top",
    "bottom",
    "fps-hold",
    "fps-inspect",
];
const INTAKE_ROLES: &[&str] = &["primary", "secondary", "detail", "control"];
const DETAIL_SCALES: &[&str] = &["macro", "meso", "micro"];
const DETAIL_FAMILIES: &[&str] = &[
    "silhouette",
    "cross-section",
    "attachment",
    "negative-space",
    "identity",
    "surface",
    "wear",
    "unknown",
];
const DETAIL_TARGET_KINDS: &[&str] = &[
    "part",
    "edge-role",
    "material-zone",
    "surface-finish",
    "unknown",
];
const HIGH_ACTIONS: &[&str] = &[
    "geometry",
    "later-normal-bake",
    "material-override",
    "defer-unknown",
];
const FEATURE_KINDS: &[&str] = &[
    "silhouette",
    "proportion",
    "negative-space",
    "cross-section",
    "attachment",
    "topology",
    "normal",
    "identity",
    "material",
];
const CONDITIONS: &[&str] = &[
    "missing-lineage",
    "non-finite",
    "degenerate",
    "non-manifold",
    "self-intersection",
    "flipped-normal",
    "budget-exceeded",
    "silhouette-collapse",
    "attachment-invalid",
    "negative-space-collapse",
    "unknown",
];

#[derive(Debug, Clone)]
struct BundleValidation {
    bundle: Value,
    bundle_sha256: String,
    intake_sha256: String,
    detail_sha256: String,
    quality_sha256: String,
    intent_bundle_id: String,
    project_id: String,
    brief_id: String,
    brief_sha256: String,
    brief_object_sha256: String,
    reference_id: String,
    reference_object_sha256: String,
    reference_evidence_sha256: String,
}

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "KNIFE_REFERENCE_INTENT_BUNDLE_INVALID: {}",
        message.into()
    ))
}

fn mismatch(code: &str, message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!("{code}: {}", message.into()))
}

fn exact_object<'a>(
    value: &'a Value,
    fields: &[&str],
    context: &str,
) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(format!("{context} must be an object")))?;
    let expected = fields.iter().copied().collect::<BTreeSet<_>>();
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(invalid(format!(
            "{context} fields differ from the closed contract"
        )));
    }
    Ok(object)
}

fn text<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    context: &str,
) -> Result<&'a str, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{context}.{field} must be a string")))
}

fn id(object: &Map<String, Value>, field: &str, context: &str) -> Result<String, RuntimeError> {
    let value = text(object, field, context)?;
    if !is_opaque_id(value) {
        return Err(invalid(format!(
            "{context}.{field} is not an opaque identifier"
        )));
    }
    Ok(value.to_owned())
}

fn hash(object: &Map<String, Value>, field: &str, context: &str) -> Result<String, RuntimeError> {
    let value = text(object, field, context)?;
    if !is_sha256(value) {
        return Err(invalid(format!(
            "{context}.{field} is not a lowercase SHA-256"
        )));
    }
    Ok(value.to_owned())
}

fn enum_text(
    object: &Map<String, Value>,
    field: &str,
    allowed: &[&str],
    context: &str,
) -> Result<String, RuntimeError> {
    let value = text(object, field, context)?;
    if !allowed.contains(&value) {
        return Err(invalid(format!(
            "{context}.{field} is outside the closed enum"
        )));
    }
    Ok(value.to_owned())
}

fn bool_exact(
    object: &Map<String, Value>,
    field: &str,
    expected: bool,
    context: &str,
) -> Result<(), RuntimeError> {
    if object.get(field).and_then(Value::as_bool) != Some(expected) {
        return Err(invalid(format!("{context}.{field} must be {expected}")));
    }
    Ok(())
}

fn u64_field(object: &Map<String, Value>, field: &str, context: &str) -> Result<u64, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid(format!("{context}.{field} must be a non-negative integer")))
}

fn safe_text(value: &Value, context: &str) -> Result<(), RuntimeError> {
    let value = value
        .as_str()
        .ok_or_else(|| invalid(format!("{context} must be text")))?;
    if value.is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
        return Err(invalid(format!("{context} is not bounded text")));
    }
    let lower = value.to_ascii_lowercase();
    let suspicious = lower.contains('/')
        || lower.contains('\\')
        || lower.starts_with("file:")
        || lower.starts_with("data:")
        || lower.starts_with("http:")
        || lower.starts_with("https:")
        || lower.starts_with("ftp:")
        || lower.contains("blender --python")
        || lower.contains("plugin")
        || lower.contains("add-on")
        || lower.contains("password:")
        || lower.contains("api_key:")
        || lower.contains("api-key:")
        || lower.contains("secret:")
        || lower.contains("token:")
        || lower.contains("output:");
    if suspicious {
        return Err(invalid(format!(
            "{context} contains a path, URL, executable, secret or output locator"
        )));
    }
    Ok(())
}

fn validate_forbidden_values(value: &Value) -> Result<(), RuntimeError> {
    const FORBIDDEN_KEYS: &[&str] = &[
        "path",
        "url",
        "uri",
        "raw",
        "raw_bytes",
        "bytes",
        "contact",
        "contacts",
        "email",
        "phone",
        "logo",
        "trademark",
        "api_key",
        "secret",
        "token",
        "password",
        "prompt",
        "script",
        "shell",
        "environment",
        "signature",
        "signed_by",
        "image_bytes",
        "raw_image",
        "output",
    ];
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if FORBIDDEN_KEYS.contains(&key.to_ascii_lowercase().as_str()) {
                    return Err(invalid(
                        "bundle contains a forbidden path, URL, secret or byte field",
                    ));
                }
                validate_forbidden_values(child)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                validate_forbidden_values(child)?;
            }
        }
        Value::String(_) => {}
        _ => {}
    }
    Ok(())
}

fn valid_timestamp(value: &str) -> bool {
    let bytes = value.as_bytes();
    if !(bytes.len() == 20 || (bytes.len() >= 22 && bytes.len() <= 27))
        || !value.ends_with('Z')
        || bytes.get(4) != Some(&b'-')
        || bytes.get(7) != Some(&b'-')
        || bytes.get(10) != Some(&b'T')
        || bytes.get(13) != Some(&b':')
        || bytes.get(16) != Some(&b':')
    {
        return false;
    }
    if ![0..4, 5..7, 8..10, 11..13, 14..16, 17..19]
        .iter()
        .all(|range| bytes[range.clone()].iter().all(u8::is_ascii_digit))
    {
        return false;
    }
    bytes.len() == 20
        || (bytes[19] == b'.'
            && bytes[20..bytes.len() - 1].iter().all(u8::is_ascii_digit)
            && (1..=6).contains(&(bytes.len() - 21)))
}

fn valid_evidence_region_id(value: &str) -> bool {
    let Some((reference_id, view)) = value.split_once(':') else {
        return false;
    };
    is_opaque_id(reference_id) && VIEWS.contains(&view) && !view.contains(':')
}

fn canonical_field(value: &Value, field: &str, context: &str) -> Result<String, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(format!("{context} must be an object")))?;
    let supplied = hash(object, field, context)?;
    let mut preimage = value.clone();
    preimage[field] = Value::String(String::new());
    let expected = canonical_json_hash(&preimage);
    if expected != supplied {
        return Err(mismatch(
            "KNIFE_REFERENCE_INTENT_BUNDLE_CANONICAL_MISMATCH",
            format!("{context}.{field} differs from Runtime recomputation"),
        ));
    }
    Ok(supplied)
}

fn request_hash(request: &Value, object: &Map<String, Value>) -> Result<(), RuntimeError> {
    let supplied = hash(object, "input_sha256", "request")?;
    let mut preimage = request.clone();
    preimage["input_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != supplied {
        return Err(mismatch(
            "KNIFE_REFERENCE_INTENT_BUNDLE_INPUT_CANONICAL_MISMATCH",
            "input_sha256 differs from Runtime recomputation",
        ));
    }
    Ok(())
}

fn validate_request_header(
    object: &Map<String, Value>,
    schema: &str,
    operation: &str,
    context: &str,
) -> Result<(), RuntimeError> {
    if text(object, "schema_version", context)? != schema
        || text(object, "operation", context)? != operation
    {
        return Err(invalid(format!(
            "{context} schema_version/operation differs"
        )));
    }
    if u64_field(object, "max_response_bytes", context)? != MAX_RESPONSE_BYTES as u64 {
        return Err(invalid(format!(
            "{context}.max_response_bytes must be exactly 1048576"
        )));
    }
    bool_exact(object, "runtime_write_performed", false, context)?;
    if text(object, "writer_policy", context)? != WRITER_POLICY
        || text(object, "canonicalization_policy", context)? != REQUEST_CANONICALIZATION
    {
        return Err(invalid(format!(
            "{context} writer/canonicalization policy differs"
        )));
    }
    Ok(())
}

fn validate_dimensions(value: &Value, context: &str) -> Result<(u64, u64), RuntimeError> {
    let object = exact_object(value, DIMENSION_FIELDS, context)?;
    let width = u64_field(object, "width", context)?;
    let height = u64_field(object, "height", context)?;
    if !(1..=16_384).contains(&width) || !(1..=16_384).contains(&height) {
        return Err(invalid(format!(
            "{context} dimensions are outside 1..16384"
        )));
    }
    Ok((width, height))
}

fn validate_intake(
    value: &Value,
    reference_id: &str,
    reference_object_sha256: &str,
    reference_evidence_sha256: &str,
) -> Result<String, RuntimeError> {
    let object = exact_object(value, INTAKE_FIELDS, "intake_manifest")?;
    if text(object, "schema_version", "intake_manifest")? != "KnifeIntakeManifest@1"
        || text(object, "canonicalization_policy", "intake_manifest")? != BUNDLE_CANONICALIZATION
    {
        return Err(invalid(
            "intake_manifest schema or canonicalization policy differs",
        ));
    }
    id(object, "manifest_id", "intake_manifest")?;
    let records = object
        .get("records")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("intake_manifest.records must be an array"))?;
    if records.len() != 1 {
        return Err(invalid(
            "intake_manifest.records must contain exactly one primary record",
        ));
    }
    let mut ids = BTreeSet::new();
    let mut primary_count = 0;
    for (index, record) in records.iter().enumerate() {
        let context = format!("intake_manifest.records[{index}]");
        let record = exact_object(record, INTAKE_RECORD_FIELDS, &context)?;
        let record_reference_id = id(record, "reference_id", &context)?;
        if !ids.insert(record_reference_id.clone()) {
            return Err(invalid("intake manifest reference IDs are not unique"));
        }
        let object_hash = hash(record, "reference_object_sha256", &context)?;
        let evidence_hash = hash(record, "reference_evidence_sha256", &context)?;
        enum_text(record, "role", INTAKE_ROLES, &context)?;
        validate_dimensions(
            record.get("resolution").expect("closed field"),
            &format!("{context}.resolution"),
        )?;
        if enum_text(
            record,
            "decode_status",
            &["decoded", "rejected", "not-run"],
            &context,
        )? != "decoded"
            || enum_text(
                record,
                "duplicate_status",
                &["unique", "duplicate", "not-run"],
                &context,
            )? != "unique"
            || enum_text(
                record,
                "admission_status",
                &["admitted", "rejected", "blocked"],
                &context,
            )? != "admitted"
        {
            return Err(invalid(
                "intake record was not admitted through all fixed gates",
            ));
        }
        let coverage = record
            .get("visible_coverage")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid(format!("{context}.visible_coverage must be an array")))?;
        if coverage.is_empty() || coverage.len() > 10 {
            return Err(invalid(format!(
                "{context}.visible_coverage length is out of bounds"
            )));
        }
        let mut views = BTreeSet::new();
        for (coverage_index, item) in coverage.iter().enumerate() {
            let item_context = format!("{context}.visible_coverage[{coverage_index}]");
            let item = exact_object(item, COVERAGE_FIELDS, &item_context)?;
            let view = enum_text(item, "view", VIEWS, &item_context)?;
            if !views.insert(view) {
                return Err(invalid(format!("{item_context}.view is duplicated")));
            }
            enum_text(
                item,
                "status",
                &["observed", "inferred", "unknown"],
                &item_context,
            )?;
        }
        if record_reference_id == reference_id {
            primary_count += 1;
            if text(record, "role", &context)? != "primary"
                || object_hash != reference_object_sha256
                || evidence_hash != reference_evidence_sha256
            {
                return Err(mismatch(
                    "KNIFE_REFERENCE_INTENT_BUNDLE_INTAKE_BINDING_MISMATCH",
                    "primary intake record differs from the exact ReferenceEvidence binding",
                ));
            }
        }
    }
    if primary_count != 1 {
        return Err(invalid(
            "intake manifest must contain exactly one bound primary record",
        ));
    }
    canonical_field(value, "canonical_sha256", "intake_manifest")
}

fn finite_unit(
    object: &Map<String, Value>,
    field: &str,
    context: &str,
    allow_zero: bool,
) -> Result<f64, RuntimeError> {
    let value = object
        .get(field)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| invalid(format!("{context}.{field} must be finite")))?;
    if value < 0.0 || value > 1.0 || (!allow_zero && value == 0.0) {
        return Err(invalid(format!(
            "{context}.{field} is outside its unit interval"
        )));
    }
    Ok(value)
}

fn validate_detail_target(value: &Value, context: &str) -> Result<Option<String>, RuntimeError> {
    let object = exact_object(value, DETAIL_TARGET_FIELDS, context)?;
    let target_kind = enum_text(object, "target_kind", DETAIL_TARGET_KINDS, context)?;
    let mapping = enum_text(
        object,
        "mapping_status",
        &["mapped", "inferred", "unknown"],
        context,
    )?;
    let target_id = match object.get("target_id") {
        Some(Value::Null) => None,
        Some(Value::String(value)) if is_opaque_id(value) => Some(value.clone()),
        _ => {
            return Err(invalid(format!(
                "{context}.target_id must be null or an opaque identifier"
            )))
        }
    };
    if target_kind == "unknown" && (target_id.is_some() || mapping != "unknown") {
        return Err(invalid(
            "unknown detail target must retain null/unknown identity",
        ));
    }
    if mapping == "mapped" && (target_kind == "unknown" || target_id.is_none()) {
        return Err(invalid(
            "mapped detail target must identify a concrete target",
        ));
    }
    Ok(target_id)
}

fn primary_observed_views(
    value: &Value,
    reference_id: &str,
) -> Result<BTreeSet<String>, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("intake_manifest must be an object"))?;
    let records = object
        .get("records")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("intake_manifest.records must be an array"))?;
    let record = records
        .iter()
        .find(|record| record.get("reference_id") == Some(&Value::String(reference_id.to_owned())))
        .ok_or_else(|| invalid("intake manifest primary record is missing"))?;
    let coverage = record
        .get("visible_coverage")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("primary intake visible coverage is missing"))?;
    let mut observed = BTreeSet::new();
    for item in coverage {
        let item = item
            .as_object()
            .ok_or_else(|| invalid("primary intake coverage is not an object"))?;
        if item.get("status").and_then(Value::as_str) == Some("observed") {
            let view = item
                .get("view")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("primary intake observed coverage has no view"))?;
            observed.insert(view.to_owned());
        }
    }
    Ok(observed)
}

fn validate_detail_inventory(
    value: &Value,
    reference_id: &str,
    allowed_parts: &BTreeSet<String>,
    allowed_zones: &BTreeSet<String>,
    observed_views: &BTreeSet<String>,
) -> Result<String, RuntimeError> {
    let object = exact_object(value, DETAIL_INVENTORY_FIELDS, "detail_inventory")?;
    if text(object, "schema_version", "detail_inventory")? != "KnifeDetailInventory@1"
        || text(object, "canonicalization_policy", "detail_inventory")? != BUNDLE_CANONICALIZATION
    {
        return Err(invalid(
            "detail_inventory schema or canonicalization policy differs",
        ));
    }
    id(object, "inventory_id", "detail_inventory")?;
    let details = object
        .get("details")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("detail_inventory.details must be an array"))?;
    if details.is_empty() || details.len() > 128 {
        return Err(invalid("detail_inventory.details length is out of bounds"));
    }
    let mut detail_ids = BTreeSet::new();
    for (index, detail) in details.iter().enumerate() {
        let context = format!("detail_inventory.details[{index}]");
        let detail = exact_object(detail, DETAIL_FIELDS, &context)?;
        let detail_id = id(detail, "detail_id", &context)?;
        if !detail_ids.insert(detail_id) {
            return Err(invalid("detail inventory detail IDs are not unique"));
        }
        enum_text(detail, "scale", DETAIL_SCALES, &context)?;
        enum_text(detail, "family", DETAIL_FAMILIES, &context)?;
        safe_text(
            detail.get("label").expect("closed field"),
            &format!("{context}.label"),
        )?;
        enum_text(detail, "confidence", &["high", "medium", "low"], &context)?;
        let observation = enum_text(
            detail,
            "observation_status",
            &["observed", "inferred", "unknown"],
            &context,
        )?;
        let target_id = validate_detail_target(
            detail.get("target").expect("closed field"),
            &format!("{context}.target"),
        )?;
        if let Some(target_id) = target_id {
            let target = detail["target"].as_object().expect("validated target");
            match target["target_kind"].as_str().expect("validated kind") {
                "part" | "edge-role" if !allowed_parts.contains(&target_id) => {
                    return Err(invalid(format!(
                        "{context}.target references an undeclared Brief part"
                    )))
                }
                "material-zone" | "surface-finish" if !allowed_zones.contains(&target_id) => {
                    return Err(invalid(format!(
                        "{context}.target references an undeclared material zone"
                    )))
                }
                _ => {}
            }
        }
        let regions = detail
            .get("evidence_regions")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid(format!("{context}.evidence_regions must be an array")))?;
        if regions.is_empty() || regions.len() > 8 {
            return Err(invalid(format!(
                "{context}.evidence_regions length is out of bounds"
            )));
        }
        let mut observed_region_count = 0usize;
        let mut non_observed_region_count = 0usize;
        for (region_index, region) in regions.iter().enumerate() {
            let region_context = format!("{context}.evidence_regions[{region_index}]");
            let region = exact_object(region, EVIDENCE_REGION_FIELDS, &region_context)?;
            if id(region, "reference_id", &region_context)? != reference_id {
                return Err(invalid(format!(
                    "{region_context} introduces a second reference"
                )));
            }
            let view = enum_text(region, "view", VIEWS, &region_context)?;
            if !observed_views.contains(&view) {
                return Err(invalid(format!(
                    "{region_context}.view is not a supplied observed primary-reference view"
                )));
            }
            finite_unit(region, "x", &region_context, true)?;
            finite_unit(region, "y", &region_context, true)?;
            finite_unit(region, "width", &region_context, false)?;
            finite_unit(region, "height", &region_context, false)?;
            match enum_text(
                region,
                "status",
                &["observed", "inferred", "unknown"],
                &region_context,
            )?
            .as_str()
            {
                "observed" => observed_region_count += 1,
                _ => non_observed_region_count += 1,
            }
        }
        match observation.as_str() {
            "observed" if observed_region_count == 0 => {
                return Err(invalid(format!(
                    "{context} observed claim has no observed evidence region"
                )))
            }
            "unknown" if observed_region_count != 0 => {
                return Err(invalid(format!(
                    "{context} unknown claim contains an observed evidence region"
                )))
            }
            "inferred" if non_observed_region_count == 0 => {
                return Err(invalid(format!(
                    "{context} inferred claim is backed only by observed regions"
                )))
            }
            _ => {}
        }
        let priority = u64_field(detail, "priority", &context)?;
        if !(1..=16).contains(&priority) {
            return Err(invalid(format!("{context}.priority is outside 1..16")));
        }
        let action = enum_text(detail, "high_action", HIGH_ACTIONS, &context)?;
        if observation == "unknown"
            && action != "defer-unknown"
            && action != "later-normal-bake"
            && action != "material-override"
        {
            return Err(invalid(format!(
                "{context} overclaims an unknown observation"
            )));
        }
        if detail["scale"].as_str() == Some("micro")
            && matches!(detail["family"].as_str(), Some("surface") | Some("wear"))
            && action == "geometry"
        {
            return Err(invalid(format!(
                "{context} promotes a micro surface detail to geometry"
            )));
        }
    }
    canonical_field(value, "canonical_sha256", "detail_inventory")
}

fn validate_quality(value: &Value) -> Result<String, RuntimeError> {
    let object = exact_object(value, QUALITY_FIELDS, "quality_contract")?;
    if text(object, "schema_version", "quality_contract")? != "KnifeQualityContract@1"
        || text(object, "canonicalization_policy", "quality_contract")? != BUNDLE_CANONICALIZATION
    {
        return Err(invalid(
            "quality_contract schema or canonicalization policy differs",
        ));
    }
    id(object, "contract_id", "quality_contract")?;
    let stage_order = object
        .get("stage_order")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("quality_contract.stage_order must be an array"))?;
    let expected_stage_order = json!([
        "camera-lock",
        "silhouette-blockout",
        "structural-form",
        "secondary-form",
        "high-geometry"
    ]);
    if Value::Array(stage_order.clone()) != expected_stage_order {
        return Err(invalid("quality_contract stage order differs"));
    }
    let features = object
        .get("critical_features")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("quality_contract.critical_features must be an array"))?;
    if features.is_empty() || features.len() > 32 {
        return Err(invalid(
            "quality_contract.critical_features length is out of bounds",
        ));
    }
    let mut feature_ids = BTreeSet::new();
    for (index, feature) in features.iter().enumerate() {
        let context = format!("quality_contract.critical_features[{index}]");
        let feature = exact_object(feature, CRITICAL_FEATURE_FIELDS, &context)?;
        if !feature_ids.insert(id(feature, "feature_id", &context)?) {
            return Err(invalid("quality feature IDs are not unique"));
        }
        enum_text(feature, "feature_kind", FEATURE_KINDS, &context)?;
        id(feature, "target_id", &context)?;
        enum_text(
            feature,
            "source_status",
            &["observed", "inferred", "unknown"],
            &context,
        )?;
        feature
            .get("blocking")
            .and_then(Value::as_bool)
            .ok_or_else(|| invalid(format!("{context}.blocking must be boolean")))?;
        let region_ids = feature
            .get("evidence_region_ids")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid(format!("{context}.evidence_region_ids must be an array")))?;
        if region_ids.len() > 16 {
            return Err(invalid(format!(
                "{context}.evidence_region_ids exceeds its bound"
            )));
        }
        let mut seen = BTreeSet::new();
        for region_id in region_ids {
            let region_id = region_id.as_str().ok_or_else(|| {
                invalid(format!(
                    "{context}.evidence_region_ids must contain identifiers"
                ))
            })?;
            if !valid_evidence_region_id(region_id) || !seen.insert(region_id) {
                return Err(invalid(format!(
                    "{context}.evidence_region_ids contains an invalid/duplicate ID"
                )));
            }
        }
    }
    let fixed_views = object
        .get("fixed_views")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("quality_contract.fixed_views must be an array"))?;
    if fixed_views.is_empty() || fixed_views.len() > 10 {
        return Err(invalid(
            "quality_contract.fixed_views length is out of bounds",
        ));
    }
    let mut view_ids = BTreeSet::new();
    for (index, view) in fixed_views.iter().enumerate() {
        let context = format!("quality_contract.fixed_views[{index}]");
        let view = exact_object(view, FIXED_VIEW_FIELDS, &context)?;
        if !view_ids.insert(id(view, "view_id", &context)?) {
            return Err(invalid("quality fixed view IDs are not unique"));
        }
        enum_text(view, "view", VIEWS, &context)?;
        let role = enum_text(
            view,
            "comparison_role",
            &["primary-reference", "orbit-nonreference", "fps-inspect"],
            &context,
        )?;
        let required = view
            .get("reference_required")
            .and_then(Value::as_bool)
            .ok_or_else(|| invalid(format!("{context}.reference_required must be boolean")))?;
        if (role == "primary-reference" && !required) || (role == "orbit-nonreference" && required)
        {
            return Err(invalid(format!(
                "{context}.reference_required conflicts with comparison role"
            )));
        }
    }
    let failures = object
        .get("blocking_failures")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("quality_contract.blocking_failures must be an array"))?;
    if failures.is_empty() || failures.len() > 32 {
        return Err(invalid(
            "quality_contract.blocking_failures length is out of bounds",
        ));
    }
    let mut failure_ids = BTreeSet::new();
    for (index, failure) in failures.iter().enumerate() {
        let context = format!("quality_contract.blocking_failures[{index}]");
        let failure = exact_object(failure, BLOCKING_FAILURE_FIELDS, &context)?;
        if !failure_ids.insert(id(failure, "failure_id", &context)?) {
            return Err(invalid("quality blocking failure IDs are not unique"));
        }
        id(failure, "gate_id", &context)?;
        enum_text(failure, "condition", CONDITIONS, &context)?;
        bool_exact(failure, "blocks_promotion", true, &context)?;
    }
    let threshold_fixture_sha256 = hash(object, "threshold_fixture_sha256", "quality_contract")?;
    if threshold_fixture_sha256 != PENDING_THRESHOLD_FIXTURE_SHA256 {
        return Err(invalid(
            "quality_contract threshold fixture is not the fixed pending authority",
        ));
    }
    let threshold = enum_text(
        object,
        "threshold_status",
        &["CALIBRATION_PENDING", "CALIBRATED"],
        "quality_contract",
    )?;
    if threshold != "CALIBRATION_PENDING"
        || text(object, "promotion_state", "quality_contract")?
            != "HIGH_LOCKED_UNTIL_CALIBRATED_AND_REVIEWED@1"
    {
        return Err(invalid("quality promotion lock differs"));
    }
    let correction = exact_object(
        object.get("correction_policy").expect("closed field"),
        CORRECTION_FIELDS,
        "quality_contract.correction_policy",
    )?;
    if u64_field(
        correction,
        "max_iterations_per_pass",
        "quality_contract.correction_policy",
    )? != 3
        || u64_field(
            correction,
            "max_iterations_total",
            "quality_contract.correction_policy",
        )? != 6
    {
        return Err(invalid("quality correction policy differs"));
    }
    bool_exact(
        correction,
        "one_changed_scope_per_iteration",
        true,
        "quality_contract.correction_policy",
    )?;
    bool_exact(
        correction,
        "baseline_preserved",
        true,
        "quality_contract.correction_policy",
    )?;
    canonical_field(value, "canonical_sha256", "quality_contract")
}

fn validate_unknowns(value: &Value) -> Result<(), RuntimeError> {
    let unknowns = value
        .as_array()
        .ok_or_else(|| invalid("bundle.unknowns must be an array"))?;
    if unknowns.len() > 64 {
        return Err(invalid("bundle.unknowns exceeds its bound"));
    }
    let mut ids = BTreeSet::new();
    for (index, unknown) in unknowns.iter().enumerate() {
        let context = format!("bundle.unknowns[{index}]");
        let unknown = exact_object(unknown, UNKNOWN_FIELDS, &context)?;
        if !ids.insert(id(unknown, "unknown_id", &context)?) {
            return Err(invalid("unknown IDs are not unique"));
        }
        let topic = enum_text(
            unknown,
            "topic",
            &[
                "reference-view",
                "hidden-surface",
                "identity",
                "geometry",
                "material",
                "camera",
                "engine",
                "other",
            ],
            &context,
        )?;
        match unknown.get("view") {
            Some(Value::String(view))
                if topic == "reference-view" && VIEWS.contains(&view.as_str()) => {}
            Some(Value::Null) if topic != "reference-view" => {}
            _ => {
                return Err(invalid(format!(
                    "{context}.view must be a view for reference-view or null for other topics"
                )))
            }
        }
        safe_text(
            unknown.get("description").expect("closed field"),
            &format!("{context}.description"),
        )?;
        if topic == "reference-view" {
            enum_text(unknown, "impact", &["blocking"], &context)?;
        } else {
            enum_text(unknown, "impact", &["blocking", "non-blocking"], &context)?;
        }
        if text(unknown, "resolution_status", &context)? != "open" {
            return Err(invalid(
                "unknown observations must remain open until explicitly resolved",
            ));
        }
    }
    Ok(())
}

fn validate_unknown_bindings(value: &Value, brief: &Value) -> Result<(), RuntimeError> {
    let coverage = brief
        .get("reference_coverage")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("eligible Brief reference_coverage is missing"))?;
    let missing = coverage
        .get("missing_views")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("eligible Brief reference_coverage.missing_views is missing"))?;
    let expected = missing
        .iter()
        .map(|view| {
            view.as_str()
                .filter(|view| VIEWS.contains(view))
                .map(str::to_owned)
                .ok_or_else(|| invalid("eligible Brief missing_views contains an invalid view"))
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let unknowns = value
        .as_array()
        .ok_or_else(|| invalid("bundle.unknowns must be an array"))?;
    let mut actual = BTreeSet::new();
    for (index, unknown) in unknowns.iter().enumerate() {
        let context = format!("bundle.unknowns[{index}]");
        let object = exact_object(unknown, UNKNOWN_FIELDS, &context)?;
        if text(object, "topic", &context)? == "reference-view" {
            let view = object
                .get("view")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid(format!("{context}.view must identify the missing view")))?;
            if !actual.insert(view.to_owned()) {
                return Err(invalid("reference-view unknowns contain duplicate views"));
            }
            if !expected.contains(view) {
                return Err(invalid(format!(
                    "{context}.view is not listed in Brief reference_coverage.missing_views"
                )));
            }
        }
    }
    if actual != expected {
        return Err(invalid(
            "reference-view unknowns must exactly equal Brief missing_views",
        ));
    }
    Ok(())
}

fn brief_target_sets(brief: &Value) -> Result<(BTreeSet<String>, BTreeSet<String>), RuntimeError> {
    let root = brief
        .as_object()
        .ok_or_else(|| invalid("eligible Brief is not an object"))?;
    let parts = root
        .get("parts")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("eligible Brief parts are missing"))?;
    let zones = root
        .get("material_zones")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("eligible Brief material zones are missing"))?;
    let mut part_ids = BTreeSet::new();
    for part in parts {
        let part = part
            .as_object()
            .ok_or_else(|| invalid("eligible Brief part is not an object"))?;
        let part_id = part
            .get("part_id")
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| invalid("eligible Brief part identity is invalid"))?;
        part_ids.insert(part_id.to_owned());
    }
    let mut zone_ids = BTreeSet::new();
    for zone in zones {
        let zone = zone
            .as_object()
            .ok_or_else(|| invalid("eligible Brief material zone is not an object"))?;
        let zone_id = zone
            .get("zone_id")
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| invalid("eligible Brief material zone identity is invalid"))?;
        zone_ids.insert(zone_id.to_owned());
    }
    Ok((part_ids, zone_ids))
}

fn validate_quality_bindings(
    value: &Value,
    allowed_parts: &BTreeSet<String>,
    allowed_zones: &BTreeSet<String>,
    reference_id: &str,
    observed_views: &BTreeSet<String>,
) -> Result<(), RuntimeError> {
    let object = exact_object(value, QUALITY_FIELDS, "quality_contract")?;
    let features = object
        .get("critical_features")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("quality_contract.critical_features must be an array"))?;
    let allowed_region_ids = observed_views
        .iter()
        .map(|view| format!("{reference_id}:{view}"))
        .collect::<BTreeSet<_>>();
    for (index, feature) in features.iter().enumerate() {
        let context = format!("quality_contract.critical_features[{index}]");
        let feature = exact_object(feature, CRITICAL_FEATURE_FIELDS, &context)?;
        let target_id = id(feature, "target_id", &context)?;
        let feature_kind = text(feature, "feature_kind", &context)?;
        let target_is_declared = match feature_kind {
            "material" => allowed_zones.contains(&target_id),
            // Identity locks may name either a semantic part (for example the
            // dragon relief) or a Brief-owned material zone (for example the
            // antique-gold engraving language).  Both namespaces are exact
            // Runtime-read Brief truth; no caller-defined target is accepted.
            "identity" => allowed_parts.contains(&target_id) || allowed_zones.contains(&target_id),
            _ => allowed_parts.contains(&target_id),
        };
        if !target_is_declared {
            return Err(invalid(format!(
                "{context}.target_id is not declared by the exact eligible Brief"
            )));
        }
        let source_status = text(feature, "source_status", &context)?;
        let region_ids = feature
            .get("evidence_region_ids")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid(format!("{context}.evidence_region_ids must be an array")))?;
        if source_status == "observed" && region_ids.is_empty() {
            return Err(invalid(format!(
                "{context} observed feature has no evidence region"
            )));
        }
        for region_id in region_ids {
            let region_id = region_id.as_str().ok_or_else(|| {
                invalid(format!(
                    "{context}.evidence_region_ids must contain identifiers"
                ))
            })?;
            if !allowed_region_ids.contains(region_id) {
                return Err(invalid(format!(
                    "{context}.evidence_region_ids references a non-primary or unsupplied view"
                )));
            }
        }
    }

    let fixed_views = object
        .get("fixed_views")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("quality_contract.fixed_views must be an array"))?;
    let mut primary_count = 0usize;
    let mut orbit_count = 0usize;
    let mut orbit_views = BTreeSet::new();
    for (index, fixed_view) in fixed_views.iter().enumerate() {
        let context = format!("quality_contract.fixed_views[{index}]");
        let fixed_view = exact_object(fixed_view, FIXED_VIEW_FIELDS, &context)?;
        let view = text(fixed_view, "view", &context)?;
        let role = text(fixed_view, "comparison_role", &context)?;
        match role {
            "primary-reference" => {
                primary_count += 1;
                if !observed_views.contains(view) {
                    return Err(invalid(format!(
                        "{context}.view is not supplied as an observed primary-reference view"
                    )));
                }
            }
            "orbit-nonreference" => {
                orbit_count += 1;
                if !orbit_views.insert(view.to_owned()) {
                    return Err(invalid(
                        "quality_contract orbit-nonreference views must be distinct",
                    ));
                }
            }
            "fps-inspect" => {}
            _ => return Err(invalid(format!("{context}.comparison_role is invalid"))),
        }
    }
    if primary_count != 1 {
        return Err(invalid(
            "quality_contract must contain exactly one primary-reference view",
        ));
    }
    if orbit_count < 2 || orbit_views.len() < 2 {
        return Err(invalid(
            "quality_contract must contain at least two distinct orbit-nonreference views",
        ));
    }
    Ok(())
}

fn validate_bundle(value: Value) -> Result<BundleValidation, RuntimeError> {
    validate_forbidden_values(&value)?;
    let object = exact_object(&value, BUNDLE_FIELDS, BUNDLE_SCHEMA)?;
    if text(object, "schema_version", "bundle")? != BUNDLE_SCHEMA
        || text(object, "canonicalization_policy", "bundle")? != BUNDLE_CANONICALIZATION
    {
        return Err(invalid("bundle schema or canonicalization policy differs"));
    }
    let intent_bundle_id = id(object, "intent_bundle_id", "bundle")?;
    let project_id = id(object, "project_id", "bundle")?;
    let created_at = text(object, "created_at", "bundle")?;
    if !valid_timestamp(created_at) {
        return Err(invalid("bundle.created_at must be a UTC timestamp"));
    }
    let brief_binding = exact_object(
        object.get("brief_binding").expect("closed field"),
        BRIEF_BINDING_FIELDS,
        "bundle.brief_binding",
    )?;
    if text(
        brief_binding,
        "brief_schema_version",
        "bundle.brief_binding",
    )? != "WeaponryKnifeProductionBrief@1"
        || text(
            brief_binding,
            "authoring_eligibility",
            "bundle.brief_binding",
        )? != "ELIGIBLE"
        || text(
            brief_binding,
            "authorization_binding_status",
            "bundle.brief_binding",
        )? != "runtime-bound"
    {
        return Err(invalid(
            "bundle Brief binding is not exactly eligible/runtime-bound",
        ));
    }
    let brief_id = id(brief_binding, "brief_id", "bundle.brief_binding")?;
    let brief_sha256 = hash(brief_binding, "brief_sha256", "bundle.brief_binding")?;
    let brief_object_sha256 = hash(brief_binding, "brief_object_sha256", "bundle.brief_binding")?;
    let reference_binding = exact_object(
        object.get("reference_binding").expect("closed field"),
        REFERENCE_BINDING_FIELDS,
        "bundle.reference_binding",
    )?;
    if text(
        reference_binding,
        "binding_status",
        "bundle.reference_binding",
    )? != "runtime-bound"
    {
        return Err(invalid(
            "bundle ReferenceEvidence binding is not Runtime-bound",
        ));
    }
    let reference_id = id(
        reference_binding,
        "reference_id",
        "bundle.reference_binding",
    )?;
    let reference_object_sha256 = hash(
        reference_binding,
        "reference_object_sha256",
        "bundle.reference_binding",
    )?;
    let reference_evidence_sha256 = hash(
        reference_binding,
        "reference_evidence_sha256",
        "bundle.reference_binding",
    )?;
    enum_text(
        object,
        "route",
        &[
            "reference-projection",
            "authored-texture",
            "procedural-finish",
        ],
        "bundle",
    )?;
    enum_text(
        object,
        "exactness",
        &["image-only", "metadata-assisted", "exact-texture"],
        "bundle",
    )?;
    let intake_sha256 = validate_intake(
        object.get("intake_manifest").expect("closed field"),
        &reference_id,
        &reference_object_sha256,
        &reference_evidence_sha256,
    )?;
    // Target membership is checked after Runtime loads the exact Brief. The
    // structural validator still proves that the child is closed and hashed.
    let detail_inventory = exact_object(
        object.get("detail_inventory").expect("closed field"),
        DETAIL_INVENTORY_FIELDS,
        "detail_inventory",
    )?;
    let detail_sha256 = canonical_field(
        &Value::Object(detail_inventory.clone()),
        "canonical_sha256",
        "detail_inventory",
    )?;
    let quality_sha256 = validate_quality(object.get("quality_contract").expect("closed field"))?;
    validate_unknowns(object.get("unknowns").expect("closed field"))?;
    let bundle_sha256 = canonical_field(&value, "canonical_sha256", "bundle")?;
    Ok(BundleValidation {
        bundle: value,
        bundle_sha256,
        intake_sha256,
        detail_sha256,
        quality_sha256,
        intent_bundle_id,
        project_id,
        brief_id,
        brief_sha256,
        brief_object_sha256,
        reference_id,
        reference_object_sha256,
        reference_evidence_sha256,
    })
}

fn reference_canonical_hash(reference: &ReferenceEvidenceRecord) -> Result<String, RuntimeError> {
    let authorization = serde_json::to_value(&reference.authorization)
        .map_err(|error| invalid(error.to_string()))?;
    Ok(canonical_json_hash(&json!({
        "schema_version": "ReferenceEvidence@1",
        "reference_id": reference.reference_id,
        "project_id": reference.project_id,
        "object_sha256": reference.object_sha256,
        "mime": reference.mime,
        "size_bytes": reference.size_bytes,
        "width": reference.width,
        "height": reference.height,
        "frame_count": reference.frame_count,
        "import_mode": reference.import_mode,
        "authorization": authorization,
        "derived_object_sha256": reference.derived_object_sha256,
        "created_at": reference.created_at,
    })))
}

fn bind_brief_and_reference(
    runtime: &Runtime,
    validation: &BundleValidation,
) -> Result<Value, RuntimeError> {
    let record = runtime
        .store
        .get_weaponry_knife_production_brief_exact(
            &validation.project_id,
            &validation.reference_id,
            &validation.reference_object_sha256,
            &validation.reference_evidence_sha256,
            &validation.brief_id,
            &validation.brief_sha256,
            &validation.brief_object_sha256,
        )?
        .ok_or_else(|| {
            mismatch(
                "KNIFE_REFERENCE_INTENT_BUNDLE_BRIEF_NOT_FOUND",
                "exact eligible Brief is not durable",
            )
        })?;
    let brief_bytes = runtime.cas_read_bounded(&record.brief_object_sha256, 1024 * 1024)?;
    let brief: Value = serde_json::from_slice(&brief_bytes)
        .map_err(|error| invalid(format!("eligible Brief CAS JSON is invalid: {error}")))?;
    let brief_validation = super::weaponry_knife_production_brief::validate_brief(brief.clone())?;
    if brief_validation.project_id != validation.project_id
        || brief_validation.brief_id != validation.brief_id
        || brief_validation.brief_sha256 != validation.brief_sha256
        || brief_validation.authoring_eligibility != "ELIGIBLE"
        || brief_validation.authorization_binding_status != "runtime-bound"
        || record.brief_object_sha256 != validation.brief_object_sha256
        || record.status != "eligible"
        || record.conflict_freeze_state != "resolved"
    {
        return Err(mismatch(
            "KNIFE_REFERENCE_INTENT_BUNDLE_BRIEF_BINDING_MISMATCH",
            "Brief identity, object, eligibility or Runtime binding differs",
        ));
    }
    let reference = runtime
        .reference(&validation.reference_id)?
        .ok_or_else(|| {
            mismatch(
                "KNIFE_REFERENCE_INTENT_BUNDLE_REFERENCE_NOT_FOUND",
                "ReferenceEvidence is not registered",
            )
        })?;
    validate_reference(runtime, &reference, validation, &brief)?;
    let (parts, zones) = brief_target_sets(&brief)?;
    let observed_views = primary_observed_views(
        &validation.bundle["intake_manifest"],
        &validation.reference_id,
    )?;
    validate_detail_inventory(
        &validation.bundle["detail_inventory"],
        &validation.reference_id,
        &parts,
        &zones,
        &observed_views,
    )?;
    validate_quality_bindings(
        &validation.bundle["quality_contract"],
        &parts,
        &zones,
        &validation.reference_id,
        &observed_views,
    )?;
    validate_unknown_bindings(&validation.bundle["unknowns"], &brief)?;
    Ok(brief)
}

fn validate_reference(
    runtime: &Runtime,
    reference: &ReferenceEvidenceRecord,
    validation: &BundleValidation,
    brief: &Value,
) -> Result<(), RuntimeError> {
    if reference.schema_version != "ReferenceEvidence@1"
        || reference.project_id != validation.project_id
        || reference.reference_id != validation.reference_id
        || reference.object_sha256 != validation.reference_object_sha256
        || reference.canonical_sha256 != validation.reference_evidence_sha256
        || reference_canonical_hash(reference)? != validation.reference_evidence_sha256
        || !matches!(reference.mime.as_str(), "image/png" | "image/jpeg")
        || !reference.authorization.user_authorized
        || reference.size_bytes == 0
        || reference.size_bytes > MAX_REFERENCE_BYTES
    {
        return Err(mismatch(
            "KNIFE_REFERENCE_INTENT_BUNDLE_REFERENCE_BINDING_MISMATCH",
            "ReferenceEvidence identity, authorization or canonical hash differs",
        ));
    }
    let bytes = runtime.cas_read_bounded(&reference.object_sha256, MAX_REFERENCE_BYTES)?;
    if bytes.len() as u64 != reference.size_bytes
        || super::sha256_hex(&bytes) != reference.object_sha256
    {
        return Err(mismatch(
            "KNIFE_REFERENCE_INTENT_BUNDLE_REFERENCE_CAS_MISMATCH",
            "ReferenceEvidence CAS bytes differ from the exact source object",
        ));
    }
    let coverage = brief["reference_coverage"]
        .as_object()
        .expect("validated Brief coverage");
    let dimensions = coverage["source_dimensions"]
        .as_object()
        .expect("validated Brief dimensions");
    if dimensions["width"].as_u64() != Some(reference.width as u64)
        || dimensions["height"].as_u64() != Some(reference.height as u64)
    {
        return Err(mismatch(
            "KNIFE_REFERENCE_INTENT_BUNDLE_REFERENCE_DIMENSIONS_MISMATCH",
            "ReferenceEvidence dimensions differ from the eligible Brief",
        ));
    }
    let intake = validation.bundle["intake_manifest"]
        .as_object()
        .expect("validated intake");
    let records = intake["records"]
        .as_array()
        .expect("validated intake records");
    let primary = records
        .iter()
        .find(|record| record["reference_id"] == validation.reference_id)
        .expect("validated primary intake");
    let resolution = primary["resolution"]
        .as_object()
        .expect("validated resolution");
    if resolution["width"].as_u64() != Some(reference.width as u64)
        || resolution["height"].as_u64() != Some(reference.height as u64)
    {
        return Err(mismatch(
            "KNIFE_REFERENCE_INTENT_BUNDLE_INTAKE_DIMENSIONS_MISMATCH",
            "intake dimensions differ from ReferenceEvidence",
        ));
    }
    Ok(())
}

fn stage_object(
    runtime: &Runtime,
    reservation: &forgecad_store::CasReservation,
    value: &Value,
    kind: &str,
) -> Result<CasObject, RuntimeError> {
    let bytes = canonical_json_bytes(value)
        .map_err(|error| invalid(format!("{kind} canonical JSON failed: {error}")))?;
    if bytes.is_empty() || bytes.len() as u64 > KNIFE_REFERENCE_INTENT_MAX_JSON_BYTES {
        return Err(invalid(format!("{kind} exceeds the bounded CAS capacity")));
    }
    Ok(runtime.store.put_object_reserved(
        reservation,
        &bytes,
        None,
        KNIFE_REFERENCE_INTENT_JSON_MIME,
        kind,
        &super::now_string(),
    )?)
}

fn cleanup(
    runtime: &Runtime,
    reservation: &forgecad_store::CasReservation,
    objects: &[CasObject],
    remove: bool,
) {
    for object in objects {
        let _ = runtime
            .store
            .release_cas_reservation_object(reservation, object, remove);
    }
}

fn result_value(
    record: &KnifeReferenceIntentBundleStoreRecord,
    bundle: Value,
    request_kind: &str,
    operation: &str,
    replayed: bool,
    store_effect: &str,
    cas_effect: &str,
) -> Result<Value, RuntimeError> {
    let mut result = json!({
        "schema_version": RESULT_SCHEMA,
        "operation": operation,
        "request_kind": request_kind,
        "status": if request_kind == "get" { "found" } else if replayed { "replayed" } else { "stored" },
        "project_id": record.project_id,
        "brief_id": record.brief_id,
        "brief_sha256": record.brief_sha256,
        "brief_object_sha256": record.brief_object_sha256,
        "brief_authoring_eligibility": "ELIGIBLE",
        "reference_id": record.reference_id,
        "reference_object_sha256": record.reference_object_sha256,
        "reference_evidence_sha256": record.reference_evidence_sha256,
        "intent_bundle_id": record.intent_bundle_id,
        "intent_bundle_sha256": record.intent_bundle_sha256,
        "intent_bundle_object_sha256": record.intent_bundle_object_sha256,
        "intent_bundle": bundle,
        "idempotency_key": if request_kind == "get" { Value::Null } else { Value::String(record.idempotency_key.clone()) },
        "replayed": replayed,
        "store_effect": store_effect,
        "cas_effect": cas_effect,
        "runtime_write_performed": request_kind == "prepare" && !replayed,
        "persistent_user_data_touched": request_kind == "prepare" && !replayed,
        "production_stage_advanced": false,
        "candidate_confirmed": false,
        "version_created": false,
        "export_performed": false,
        "high_mesh_created": false,
        "high_stage_unlocked": false,
        "writer_policy": WRITER_POLICY,
        "canonicalization_policy": BUNDLE_CANONICALIZATION,
        "canonical_sha256": "",
    });
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    let bytes = canonical_json_bytes(&result).map_err(|error| invalid(error.to_string()))?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(invalid("result exceeds max_response_bytes"));
    }
    Ok(result)
}

pub(crate) fn prepare(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(request, PREPARE_FIELDS, PREPARE_SCHEMA)?;
    validate_request_header(object, PREPARE_SCHEMA, PREPARE_OPERATION, "prepare")?;
    request_hash(request, object)?;
    let project_id = id(object, "project_id", "prepare")?;
    let brief_id = id(object, "brief_id", "prepare")?;
    let brief_sha256 = hash(object, "brief_sha256", "prepare")?;
    let brief_object_sha256 = hash(object, "brief_object_sha256", "prepare")?;
    let reference_id = id(object, "reference_id", "prepare")?;
    let reference_object_sha256 = hash(object, "reference_object_sha256", "prepare")?;
    let reference_evidence_sha256 = hash(object, "reference_evidence_sha256", "prepare")?;
    if text(object, "brief_authoring_eligibility", "prepare")? != "ELIGIBLE" {
        return Err(invalid(
            "prepare.brief_authoring_eligibility must be ELIGIBLE",
        ));
    }
    let bundle = validate_bundle(object.get("intent_bundle").cloned().expect("closed field"))?;
    if bundle.project_id != project_id
        || bundle.brief_id != brief_id
        || bundle.brief_sha256 != brief_sha256
        || bundle.brief_object_sha256 != brief_object_sha256
        || bundle.reference_id != reference_id
        || bundle.reference_object_sha256 != reference_object_sha256
        || bundle.reference_evidence_sha256 != reference_evidence_sha256
    {
        return Err(mismatch(
            "KNIFE_REFERENCE_INTENT_BUNDLE_REQUEST_BINDING_MISMATCH",
            "prepare envelope and nested bundle bindings differ",
        ));
    }
    bind_brief_and_reference(runtime, &bundle)?;
    let idempotency_key = id(object, "idempotency_key", "prepare")?;
    let reservation = runtime.store.begin_cas_reservation();
    let mut staged = Vec::new();
    let result = (|| {
        let intake = stage_object(
            runtime,
            &reservation,
            &bundle.bundle["intake_manifest"],
            KNIFE_REFERENCE_INTENT_INTAKE_OBJECT_KIND,
        )?;
        staged.push(intake.clone());
        let detail = stage_object(
            runtime,
            &reservation,
            &bundle.bundle["detail_inventory"],
            KNIFE_REFERENCE_INTENT_DETAIL_OBJECT_KIND,
        )?;
        staged.push(detail.clone());
        let quality = stage_object(
            runtime,
            &reservation,
            &bundle.bundle["quality_contract"],
            KNIFE_REFERENCE_INTENT_QUALITY_OBJECT_KIND,
        )?;
        staged.push(quality.clone());
        let intent = stage_object(
            runtime,
            &reservation,
            &bundle.bundle,
            KNIFE_REFERENCE_INTENT_BUNDLE_OBJECT_KIND,
        )?;
        staged.push(intent.clone());
        let record = KnifeReferenceIntentBundleStoreRecord {
            schema_version: KNIFE_REFERENCE_INTENT_BUNDLE_RECORD_SCHEMA_VERSION.to_owned(),
            intent_bundle_id: bundle.intent_bundle_id.clone(),
            project_id: project_id.clone(),
            brief_id: brief_id.clone(),
            brief_sha256: brief_sha256.clone(),
            brief_object_sha256: brief_object_sha256.clone(),
            reference_id: reference_id.clone(),
            reference_object_sha256: reference_object_sha256.clone(),
            reference_evidence_sha256: reference_evidence_sha256.clone(),
            intake_manifest_sha256: bundle.intake_sha256.clone(),
            intake_manifest_object_sha256: intake.record.sha256.clone(),
            detail_inventory_sha256: bundle.detail_sha256.clone(),
            detail_inventory_object_sha256: detail.record.sha256.clone(),
            quality_contract_sha256: bundle.quality_sha256.clone(),
            quality_contract_object_sha256: quality.record.sha256.clone(),
            intent_bundle_sha256: bundle.bundle_sha256.clone(),
            intent_bundle_object_sha256: intent.record.sha256.clone(),
            idempotency_key,
            created_at: super::now_string(),
        };
        let commit = KnifeReferenceIntentBundleCommit {
            record,
            cas: KnifeReferenceIntentBundleCasBundle {
                intent_bundle: intent.record.clone(),
                intake_manifest: intake.record.clone(),
                detail_inventory: detail.record.clone(),
                quality_contract: quality.record.clone(),
            },
        };
        let (stored, replayed) = runtime
            .store
            .record_knife_reference_intent_bundle_with_replay(&commit)?;
        cleanup(runtime, &reservation, &staged, false);
        let stored_bundle = runtime
            .store
            .read_knife_reference_intent_bundle_json(
                &stored.project_id,
                &stored.brief_id,
                &stored.intent_bundle_id,
                &stored.intent_bundle_sha256,
            )?
            .ok_or_else(|| invalid("stored intent bundle disappeared before readback"))?;
        let stored_validation = validate_bundle(stored_bundle.clone())?;
        if stored_validation.bundle_sha256 != stored.intent_bundle_sha256
            || stored_validation.intent_bundle_id != stored.intent_bundle_id
        {
            return Err(invalid("stored intent bundle readback identity differs"));
        }
        result_value(
            &stored,
            stored_bundle,
            "prepare",
            PREPARE_OPERATION,
            replayed,
            if replayed { "not-touched" } else { "inserted" },
            if replayed { "not-touched" } else { "inserted" },
        )
    })();
    if result.is_err() {
        cleanup(runtime, &reservation, &staged, true);
    }
    result
}

pub(crate) fn get(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(request, GET_FIELDS, GET_SCHEMA)?;
    validate_request_header(object, GET_SCHEMA, GET_OPERATION, "get")?;
    request_hash(request, object)?;
    bool_exact(object, "persistent_user_data_touched", false, "get")?;
    if text(object, "brief_authoring_eligibility", "get")? != "ELIGIBLE" {
        return Err(invalid("get.brief_authoring_eligibility must be ELIGIBLE"));
    }
    let project_id = id(object, "project_id", "get")?;
    let brief_id = id(object, "brief_id", "get")?;
    let brief_sha256 = hash(object, "brief_sha256", "get")?;
    let brief_object_sha256 = hash(object, "brief_object_sha256", "get")?;
    let reference_id = id(object, "reference_id", "get")?;
    let reference_object_sha256 = hash(object, "reference_object_sha256", "get")?;
    let reference_evidence_sha256 = hash(object, "reference_evidence_sha256", "get")?;
    let intent_bundle_id = id(object, "intent_bundle_id", "get")?;
    let intent_bundle_sha256 = hash(object, "intent_bundle_sha256", "get")?;
    let intent_bundle_object_sha256 = hash(object, "intent_bundle_object_sha256", "get")?;
    let record = runtime
        .store
        .get_knife_reference_intent_bundle_exact(
            &project_id,
            &brief_id,
            &brief_sha256,
            &brief_object_sha256,
            &reference_id,
            &reference_object_sha256,
            &reference_evidence_sha256,
            &intent_bundle_id,
            &intent_bundle_sha256,
            &intent_bundle_object_sha256,
        )?
        .ok_or_else(|| {
            mismatch(
                "KNIFE_REFERENCE_INTENT_BUNDLE_NOT_FOUND",
                "no exact immutable bundle binding exists",
            )
        })?;
    let bundle = runtime
        .store
        .read_knife_reference_intent_bundle_json(
            &project_id,
            &brief_id,
            &intent_bundle_id,
            &intent_bundle_sha256,
        )?
        .ok_or_else(|| invalid("intent bundle CAS payload disappeared before readback"))?;
    let validation = validate_bundle(bundle.clone())?;
    if validation.bundle_sha256 != intent_bundle_sha256
        || validation.intent_bundle_id != intent_bundle_id
    {
        return Err(invalid("get bundle semantic identity differs from Store"));
    }
    bind_brief_and_reference(runtime, &validation)?;
    result_value(
        &record,
        bundle,
        "get",
        GET_OPERATION,
        false,
        "not-touched",
        "not-touched",
    )
}

impl Runtime {
    pub fn knife_reference_intent_bundle_prepare(
        &self,
        request: &Value,
    ) -> Result<Value, RuntimeError> {
        prepare(self, request)
    }

    pub fn knife_reference_intent_bundle_get(
        &self,
        request: &Value,
    ) -> Result<Value, RuntimeError> {
        get(self, request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgecad_contracts::{
        ReferenceAuthorization, ReferenceEvidenceRecord, ReferenceImportRequest,
        ReferenceImportSource,
    };
    use serde_json::json;
    use std::path::PathBuf;

    fn fixture(path: &str) -> Value {
        let root = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../packages/forgecad-contracts/fixtures/weaponry-knife-reference-intent-bundle/positive/"
        );
        let text = std::fs::read_to_string(format!("{root}{path}")).expect("fixture file");
        serde_json::from_str(&text).expect("fixture JSON")
    }

    fn brief_fixture(path: &str) -> Value {
        let root = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../packages/forgecad-contracts/fixtures/weaponry-knife-production-brief/positive/"
        );
        let text = std::fs::read_to_string(format!("{root}{path}")).expect("brief fixture file");
        serde_json::from_str(&text).expect("brief fixture JSON")
    }

    fn import_reference(runtime: &Runtime, project_id: &str) -> ReferenceEvidenceRecord {
        runtime
            .import_reference(&ReferenceImportRequest {
                project_id: project_id.to_owned(),
                source: ReferenceImportSource::InlineContent {
                    mime: "image/png".to_owned(),
                    content_base64: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=".to_owned(),
                },
                authorization: ReferenceAuthorization {
                    user_authorized: true,
                    declaration: "Runtime knife intent integration reference".to_owned(),
                },
                expected_sha256: None,
            })
            .expect("reference import")
            .reference
    }

    fn runtime_brief_from(
        path: &str,
        project_id: &str,
        reference: &ReferenceEvidenceRecord,
    ) -> Value {
        let mut brief = brief_fixture(path);
        brief["project_id"] = Value::String(project_id.to_owned());
        brief["authorization"]["source_reference_sha256"] =
            Value::String(reference.object_sha256.clone());
        if path == "dragonfang-kukri-brief-resolved-001.json" {
            brief["authorization"]["evidence_status"] = Value::String("runtime-bound".to_owned());
            brief["acceptance_constraints"]["gate_statuses"][0]["status"] =
                Value::String("pass".to_owned());
            if let Some(blockers) =
                brief["acceptance_constraints"]["blocking_reasons"].as_array_mut()
            {
                blockers.retain(|value| value.as_str() != Some("authorization-not-runtime-bound"));
            }
        }
        brief["reference_coverage"]["source_reference_sha256"] =
            Value::String(reference.object_sha256.clone());
        brief["reference_coverage"]["source_dimensions"] =
            json!({"width": reference.width, "height": reference.height});
        brief["canonical_sha256"] = Value::String(String::new());
        brief["canonical_sha256"] = Value::String(canonical_json_hash(&brief));
        brief
    }

    fn runtime_brief(project_id: &str, reference: &ReferenceEvidenceRecord) -> Value {
        runtime_brief_from(
            "dragonfang-kukri-brief-resolved-001.json",
            project_id,
            reference,
        )
    }

    fn brief_request(
        brief: Value,
        reference: &ReferenceEvidenceRecord,
        idempotency_key: &str,
    ) -> Value {
        let mut request = json!({
            "schema_version": "WeaponryKnifeProductionBriefPrepareRequest@1",
            "operation": "weaponry_knife_production_brief_prepare",
            "project_id": brief["project_id"],
            "brief": brief,
            "reference_id": reference.reference_id,
            "reference_object_sha256": reference.object_sha256,
            "reference_evidence_sha256": reference.canonical_sha256,
            "idempotency_key": idempotency_key,
            "max_response_bytes": MAX_RESPONSE_BYTES,
            "runtime_write_performed": false,
            "writer_policy": "forgecad-runtime-only-state-writer@1",
            "canonicalization_policy": "canonical-json-sha256-excluding-input-sha256@1",
            "input_sha256": ""
        });
        request["input_sha256"] = Value::String(canonical_json_hash(&request));
        request
    }

    fn prepare_brief(
        runtime: &Runtime,
        project_id: &str,
        reference: &ReferenceEvidenceRecord,
    ) -> Value {
        let parent = runtime_brief_from("dragonfang-kukri-brief.json", project_id, reference);
        let parent_request = brief_request(parent, reference, "knife-intent-parent");
        let parent_result = runtime
            .weaponry_knife_production_brief_prepare(&parent_request)
            .expect("parent Brief prepare");
        let mut successor = runtime_brief(project_id, reference);
        successor["parent_brief_id"] = parent_result["brief_id"].clone();
        successor["parent_brief_sha256"] = parent_result["brief_sha256"].clone();
        successor["canonical_sha256"] = Value::String(String::new());
        successor["canonical_sha256"] = Value::String(canonical_json_hash(&successor));
        let request = brief_request(successor, reference, "knife-intent-brief");
        runtime
            .weaponry_knife_production_brief_prepare(&request)
            .expect("eligible Brief prepare")
    }

    fn rewrite_reference_bindings(value: &mut Value, reference: &ReferenceEvidenceRecord) {
        match value {
            Value::Object(object) => {
                for (key, child) in object.iter_mut() {
                    match key.as_str() {
                        "reference_id" => *child = Value::String(reference.reference_id.clone()),
                        "reference_object_sha256" => {
                            *child = Value::String(reference.object_sha256.clone())
                        }
                        "reference_evidence_sha256" => {
                            *child = Value::String(reference.canonical_sha256.clone())
                        }
                        _ => rewrite_reference_bindings(child, reference),
                    }
                }
            }
            Value::Array(values) => {
                for child in values {
                    rewrite_reference_bindings(child, reference);
                }
            }
            _ => {}
        }
    }

    fn refill_hash(value: &mut Value) {
        value["canonical_sha256"] = Value::String(String::new());
        value["canonical_sha256"] = Value::String(canonical_json_hash(value));
    }

    fn runtime_bundle(
        brief_result: &Value,
        reference: &ReferenceEvidenceRecord,
        intent_bundle_id: &str,
    ) -> Value {
        let mut bundle = fixture("dragonfang-reference-intent-bundle.json");
        bundle["intent_bundle_id"] = Value::String(intent_bundle_id.to_owned());
        bundle["project_id"] = brief_result["project_id"].clone();
        bundle["brief_binding"]["brief_id"] = brief_result["brief_id"].clone();
        bundle["brief_binding"]["brief_sha256"] = brief_result["brief_sha256"].clone();
        bundle["brief_binding"]["brief_object_sha256"] =
            brief_result["brief_object_sha256"].clone();
        rewrite_reference_bindings(&mut bundle, reference);
        if let Some(features) = bundle["quality_contract"]["critical_features"].as_array_mut() {
            for feature in features {
                if let Some(region_ids) = feature["evidence_region_ids"].as_array_mut() {
                    for region_id in region_ids {
                        if let Some((_, view)) =
                            region_id.as_str().and_then(|value| value.split_once(':'))
                        {
                            *region_id =
                                Value::String(format!("{}:{view}", reference.reference_id));
                        }
                    }
                }
            }
        }
        bundle["intake_manifest"]["records"][0]["resolution"] =
            json!({"width": reference.width, "height": reference.height});
        refill_hash(&mut bundle["intake_manifest"]);
        refill_hash(&mut bundle["detail_inventory"]);
        refill_hash(&mut bundle["quality_contract"]);
        refill_hash(&mut bundle);
        bundle
    }

    fn prepare_request(bundle: Value, idempotency_key: &str) -> Value {
        let brief = &bundle["brief_binding"];
        let reference = &bundle["reference_binding"];
        let mut request = json!({
            "schema_version": PREPARE_SCHEMA,
            "operation": PREPARE_OPERATION,
            "project_id": bundle["project_id"],
            "brief_id": brief["brief_id"],
            "brief_sha256": brief["brief_sha256"],
            "brief_object_sha256": brief["brief_object_sha256"],
            "reference_id": reference["reference_id"],
            "reference_object_sha256": reference["reference_object_sha256"],
            "reference_evidence_sha256": reference["reference_evidence_sha256"],
            "brief_authoring_eligibility": "ELIGIBLE",
            "intent_bundle": bundle,
            "idempotency_key": idempotency_key,
            "max_response_bytes": MAX_RESPONSE_BYTES,
            "runtime_write_performed": false,
            "writer_policy": WRITER_POLICY,
            "canonicalization_policy": REQUEST_CANONICALIZATION,
            "input_sha256": ""
        });
        request["input_sha256"] = Value::String(canonical_json_hash(&request));
        request
    }

    fn get_request(result: &Value) -> Value {
        let mut request = json!({
            "schema_version": GET_SCHEMA,
            "operation": GET_OPERATION,
            "project_id": result["project_id"],
            "brief_id": result["brief_id"],
            "brief_sha256": result["brief_sha256"],
            "brief_object_sha256": result["brief_object_sha256"],
            "reference_id": result["reference_id"],
            "reference_object_sha256": result["reference_object_sha256"],
            "reference_evidence_sha256": result["reference_evidence_sha256"],
            "brief_authoring_eligibility": "ELIGIBLE",
            "intent_bundle_id": result["intent_bundle_id"],
            "intent_bundle_sha256": result["intent_bundle_sha256"],
            "intent_bundle_object_sha256": result["intent_bundle_object_sha256"],
            "max_response_bytes": MAX_RESPONSE_BYTES,
            "runtime_write_performed": false,
            "persistent_user_data_touched": false,
            "writer_policy": WRITER_POLICY,
            "canonicalization_policy": REQUEST_CANONICALIZATION,
            "input_sha256": ""
        });
        request["input_sha256"] = Value::String(canonical_json_hash(&request));
        request
    }

    fn setup_file_runtime() -> (Runtime, PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "forgecad-knife-reference-intent-{}",
            uuid::Uuid::new_v4()
        ));
        let database = root.join("runtime.sqlite");
        let cas = root.join("runtime.cas");
        let runtime = Runtime::open_with_cas(&database, &cas).expect("runtime");
        (runtime, database, cas)
    }

    #[test]
    fn knife_reference_intent_prepare_replay_conflict_and_exact_get() {
        let (runtime, database, cas) = setup_file_runtime();
        let project = runtime
            .create_project("knife reference intent", json!({"profile": "knife"}))
            .expect("project");
        let reference = import_reference(&runtime, &project.project_id);
        let brief = prepare_brief(&runtime, &project.project_id, &reference);
        assert_eq!(brief["authoring_eligibility"], "ELIGIBLE");
        let bundle = runtime_bundle(&brief, &reference, "dragonfang-intent-runtime-001");
        let request = prepare_request(bundle.clone(), "dragonfang-intent-runtime-key");
        let first = runtime
            .knife_reference_intent_bundle_prepare(&request)
            .expect("intent prepare");
        assert_eq!(first["status"], "stored");
        assert_eq!(first["replayed"], false);
        assert_eq!(first["runtime_write_performed"], true);
        assert_eq!(first["high_mesh_created"], false);
        assert_eq!(first["high_stage_unlocked"], false);
        assert_eq!(first["store_effect"], "inserted");
        assert_eq!(first["cas_effect"], "inserted");

        let replay = runtime
            .knife_reference_intent_bundle_prepare(&request)
            .expect("intent replay");
        assert_eq!(replay["status"], "replayed");
        assert_eq!(replay["replayed"], true);
        assert_eq!(replay["runtime_write_performed"], false);
        assert_eq!(replay["persistent_user_data_touched"], false);
        assert_eq!(replay["store_effect"], "not-touched");
        assert_eq!(replay["cas_effect"], "not-touched");

        let mut conflict_bundle = bundle;
        conflict_bundle["intent_bundle_id"] =
            Value::String("dragonfang-intent-runtime-002".to_owned());
        refill_hash(&mut conflict_bundle);
        let conflict_request = prepare_request(conflict_bundle, "dragonfang-intent-runtime-key");
        let conflict = runtime
            .knife_reference_intent_bundle_prepare(&conflict_request)
            .expect_err("same-key changed identity must conflict");
        assert!(conflict
            .to_string()
            .contains("KNIFE_REFERENCE_INTENT_BUNDLE_IDEMPOTENCY_CONFLICT"));

        let get = get_request(&first);
        let found = runtime
            .knife_reference_intent_bundle_get(&get)
            .expect("exact get");
        assert_eq!(found["status"], "found");
        assert_eq!(found["idempotency_key"], Value::Null);
        assert_eq!(found["store_effect"], "not-touched");
        assert_eq!(found["cas_effect"], "not-touched");
        assert_eq!(found["intent_bundle_sha256"], first["intent_bundle_sha256"]);
        drop(runtime);
        let reopened = Runtime::open_with_cas(&database, &cas).expect("reopen runtime");
        let reopened_found = reopened
            .knife_reference_intent_bundle_get(&get)
            .expect("reopened exact get");
        assert_eq!(reopened_found["status"], "found");
        assert_eq!(
            reopened_found["intent_bundle_object_sha256"],
            first["intent_bundle_object_sha256"]
        );
        drop(reopened);
        let _ = std::fs::remove_dir_all(database.parent().expect("test root"));
    }

    #[test]
    fn knife_reference_intent_late_store_rejection_cleans_cas_and_leaves_no_row() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project(
                "knife reference intent late reject",
                json!({"profile": "knife"}),
            )
            .expect("project");
        let reference = import_reference(&runtime, &project.project_id);
        let brief = prepare_brief(&runtime, &project.project_id, &reference);
        let first_bundle = runtime_bundle(&brief, &reference, "dragonfang-intent-initial");
        let first_request = prepare_request(first_bundle, "dragonfang-intent-initial-key");
        runtime
            .knife_reference_intent_bundle_prepare(&first_request)
            .expect("initial intent bundle prepare");

        let mut bundle = runtime_bundle(&brief, &reference, "dragonfang-intent-late-reject");
        bundle["intake_manifest"]["manifest_id"] =
            Value::String("dragonfang-intake-late-reject".to_owned());
        bundle["detail_inventory"]["inventory_id"] =
            Value::String("dragonfang-details-late-reject".to_owned());
        bundle["quality_contract"]["contract_id"] =
            Value::String("dragonfang-high-quality-late-reject".to_owned());
        refill_hash(&mut bundle["intake_manifest"]);
        refill_hash(&mut bundle["detail_inventory"]);
        refill_hash(&mut bundle["quality_contract"]);
        refill_hash(&mut bundle);
        let request = prepare_request(bundle.clone(), "dragonfang-intent-late-reject-key");
        let error = runtime
            .knife_reference_intent_bundle_prepare(&request)
            .expect_err("duplicate Brief binding must reject after staging");
        assert!(error
            .to_string()
            .contains("KNIFE_REFERENCE_INTENT_BUNDLE_BRIEF_CONFLICT"));
        let bundle_sha = bundle["canonical_sha256"].as_str().expect("bundle hash");
        let intake_bytes = canonical_json_bytes(&bundle["intake_manifest"]).expect("intake bytes");
        let detail_bytes = canonical_json_bytes(&bundle["detail_inventory"]).expect("detail bytes");
        let quality_bytes =
            canonical_json_bytes(&bundle["quality_contract"]).expect("quality bytes");
        for sha in [
            sha256_hex(&intake_bytes),
            sha256_hex(&detail_bytes),
            sha256_hex(&quality_bytes),
            sha256_hex(&canonical_json_bytes(&bundle).expect("bundle bytes")),
        ] {
            assert!(
                runtime
                    .store
                    .get_object(&sha)
                    .expect("CAS metadata lookup")
                    .is_none(),
                "rejected staged CAS root remained: {sha}"
            );
        }
        let wrong_object_sha = "f".repeat(64);
        let record = runtime
            .store
            .get_knife_reference_intent_bundle_exact(
                &project.project_id,
                brief["brief_id"].as_str().expect("brief id"),
                brief["brief_sha256"].as_str().expect("brief hash"),
                brief["brief_object_sha256"].as_str().expect("brief object"),
                &reference.reference_id,
                &reference.object_sha256,
                &reference.canonical_sha256,
                bundle["intent_bundle_id"].as_str().expect("bundle id"),
                bundle_sha,
                &wrong_object_sha,
            )
            .expect("exact rejected lookup");
        assert!(record.is_none());
    }

    #[test]
    fn knife_reference_intent_get_rejects_hash_or_binding_drift_without_write() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("knife reference intent drift", json!({"profile": "knife"}))
            .expect("project");
        let reference = import_reference(&runtime, &project.project_id);
        let brief = prepare_brief(&runtime, &project.project_id, &reference);
        let bundle = runtime_bundle(&brief, &reference, "dragonfang-intent-drift");
        let request = prepare_request(bundle, "dragonfang-intent-drift-key");
        let result = runtime
            .knife_reference_intent_bundle_prepare(&request)
            .expect("prepare");
        let mut get = get_request(&result);
        get["intent_bundle_object_sha256"] = Value::String("a".repeat(64));
        get["input_sha256"] = Value::String(canonical_json_hash(&get));
        let error = runtime
            .knife_reference_intent_bundle_get(&get)
            .expect_err("object hash substitution must fail closed");
        assert!(error.to_string().contains("KNIFE_REFERENCE_INTENT_BUNDLE"));
        assert_eq!(result["status"], "stored");
    }

    #[test]
    fn knife_reference_intent_rejects_unbound_records_targets_and_truth_drift() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project(
                "knife reference intent negatives",
                json!({"profile": "knife"}),
            )
            .expect("project");
        let reference = import_reference(&runtime, &project.project_id);
        let brief = prepare_brief(&runtime, &project.project_id, &reference);

        let mut extra_record = runtime_bundle(&brief, &reference, "intent-negative-records");
        let duplicate = extra_record["intake_manifest"]["records"][0].clone();
        extra_record["intake_manifest"]["records"]
            .as_array_mut()
            .expect("records")
            .push(duplicate);
        refill_hash(&mut extra_record["intake_manifest"]);
        refill_hash(&mut extra_record);
        let error = runtime
            .knife_reference_intent_bundle_prepare(&prepare_request(
                extra_record,
                "intent-negative-records-key",
            ))
            .expect_err("secondary records must not be accepted by a single-reference bundle");
        assert!(error
            .to_string()
            .contains("records must contain exactly one primary record"));

        let mut unknown_detail_target =
            runtime_bundle(&brief, &reference, "intent-negative-detail-target");
        unknown_detail_target["detail_inventory"]["details"][0]["target"]["target_id"] =
            Value::String("brief-part-that-does-not-exist".to_owned());
        refill_hash(&mut unknown_detail_target["detail_inventory"]);
        refill_hash(&mut unknown_detail_target);
        let error = runtime
            .knife_reference_intent_bundle_prepare(&prepare_request(
                unknown_detail_target,
                "intent-negative-detail-target-key",
            ))
            .expect_err("detail target outside the exact Brief must fail closed");
        assert!(error
            .to_string()
            .contains("detail_inventory.details[0].target references an undeclared Brief part"));

        let mut unknown_edge_role = runtime_bundle(&brief, &reference, "intent-negative-edge-role");
        unknown_edge_role["detail_inventory"]["details"][0]["target"]["target_kind"] =
            Value::String("edge-role".to_owned());
        unknown_edge_role["detail_inventory"]["details"][0]["target"]["target_id"] =
            Value::String("brief-edge-role-that-does-not-exist".to_owned());
        refill_hash(&mut unknown_edge_role["detail_inventory"]);
        refill_hash(&mut unknown_edge_role);
        let error = runtime
            .knife_reference_intent_bundle_prepare(&prepare_request(
                unknown_edge_role,
                "intent-negative-edge-role-key",
            ))
            .expect_err("edge-role target outside the exact Brief must fail closed");
        assert!(error
            .to_string()
            .contains("target references an undeclared Brief part"));

        let mut unknown_surface_finish =
            runtime_bundle(&brief, &reference, "intent-negative-surface-finish");
        unknown_surface_finish["detail_inventory"]["details"][0]["target"]["target_kind"] =
            Value::String("surface-finish".to_owned());
        unknown_surface_finish["detail_inventory"]["details"][0]["target"]["target_id"] =
            Value::String("brief-surface-finish-that-does-not-exist".to_owned());
        refill_hash(&mut unknown_surface_finish["detail_inventory"]);
        refill_hash(&mut unknown_surface_finish);
        let error = runtime
            .knife_reference_intent_bundle_prepare(&prepare_request(
                unknown_surface_finish,
                "intent-negative-surface-finish-key",
            ))
            .expect_err("surface-finish target outside the exact Brief must fail closed");
        assert!(error
            .to_string()
            .contains("target references an undeclared material zone"));

        let mut unknown_quality_target =
            runtime_bundle(&brief, &reference, "intent-negative-quality-target");
        unknown_quality_target["quality_contract"]["critical_features"][0]["target_id"] =
            Value::String("brief-part-that-does-not-exist".to_owned());
        refill_hash(&mut unknown_quality_target["quality_contract"]);
        refill_hash(&mut unknown_quality_target);
        let error = runtime
            .knife_reference_intent_bundle_prepare(&prepare_request(
                unknown_quality_target,
                "intent-negative-quality-target-key",
            ))
            .expect_err("quality feature outside the exact Brief must fail closed");
        assert!(error
            .to_string()
            .contains("quality_contract.critical_features[0].target_id is not declared"));

        let mut false_observation =
            runtime_bundle(&brief, &reference, "intent-negative-observation");
        false_observation["detail_inventory"]["details"][0]["observation_status"] =
            Value::String("unknown".to_owned());
        refill_hash(&mut false_observation["detail_inventory"]);
        refill_hash(&mut false_observation);
        let error = runtime
            .knife_reference_intent_bundle_prepare(&prepare_request(
                false_observation,
                "intent-negative-observation-key",
            ))
            .expect_err("unknown detail cannot retain observed evidence");
        assert!(error
            .to_string()
            .contains("unknown claim contains an observed evidence region"));

        let mut calibrated = runtime_bundle(&brief, &reference, "intent-negative-threshold");
        calibrated["quality_contract"]["threshold_status"] = Value::String("CALIBRATED".to_owned());
        refill_hash(&mut calibrated["quality_contract"]);
        refill_hash(&mut calibrated);
        let error = runtime
            .knife_reference_intent_bundle_prepare(&prepare_request(
                calibrated,
                "intent-negative-threshold-key",
            ))
            .expect_err("Slice A must not accept an uncalibrated threshold promotion");
        assert!(
            error
                .to_string()
                .contains("quality_contract threshold fixture is not the fixed pending authority")
                || error.to_string().contains("quality promotion lock differs")
        );
    }
}
