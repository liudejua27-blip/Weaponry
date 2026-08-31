//! Runtime-owned typed intake for `WeaponryKnifeProductionBrief@1`.
//!
//! This is deliberately an intake boundary.  It validates a closed brief,
//! verifies the already imported reference through Runtime/CAS, and delegates
//! the only durable write to Store.  It never creates a candidate, advances a
//! production stage, confirms a version, or exports an artifact.

use super::{
    canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256, sha256_hex, Runtime,
    RuntimeError,
};
use forgecad_contracts::ReferenceEvidenceRecord;
use forgecad_store::{
    WeaponryKnifeProductionBriefCasBundle, WeaponryKnifeProductionBriefCommit,
    WeaponryKnifeProductionBriefStoreRecord, WEAPONRY_KNIFE_PRODUCTION_BRIEF_JSON_MIME,
    WEAPONRY_KNIFE_PRODUCTION_BRIEF_MAX_JSON_BYTES, WEAPONRY_KNIFE_PRODUCTION_BRIEF_OBJECT_KIND,
    WEAPONRY_KNIFE_PRODUCTION_BRIEF_RECORD_SCHEMA_VERSION,
};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const BRIEF_SCHEMA: &str = "WeaponryKnifeProductionBrief@1";
pub(crate) const PREPARE_SCHEMA: &str = "WeaponryKnifeProductionBriefPrepareRequest@1";
pub(crate) const GET_SCHEMA: &str = "WeaponryKnifeProductionBriefGetRequest@1";
pub(crate) const RESULT_SCHEMA: &str = "WeaponryKnifeProductionBriefResult@1";
pub(crate) const PREPARE_OPERATION: &str = "weaponry_knife_production_brief_prepare";
pub(crate) const GET_OPERATION: &str = "weaponry_knife_production_brief_get";
pub(crate) const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
pub(crate) const REQUEST_CANONICALIZATION: &str = "canonical-json-sha256-excluding-input-sha256@1";
pub(crate) const BRIEF_CANONICALIZATION: &str =
    "canonical-json-sha256-excluding-canonical-sha256@1";
pub(crate) const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const MAX_REFERENCE_BYTES: u64 = 8 * 1024 * 1024;

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "project_id",
    "brief",
    "reference_id",
    "reference_object_sha256",
    "reference_evidence_sha256",
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
    "reference_id",
    "reference_object_sha256",
    "reference_evidence_sha256",
    "brief_id",
    "brief_sha256",
    "brief_object_sha256",
    "max_response_bytes",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];
const BRIEF_FIELDS: &[&str] = &[
    "schema_version",
    "brief_id",
    "project_id",
    "parent_brief_id",
    "parent_brief_sha256",
    "freeze_policy",
    "subject",
    "asset_identity",
    "authorization",
    "reference_coverage",
    "silhouette_priorities",
    "parts",
    "material_zones",
    "surface_constraints",
    "presentation_constraints",
    "engine_constraints",
    "source_conflicts",
    "acceptance_constraints",
    "canonicalization_policy",
    "canonical_sha256",
    "created_at",
];
const IDENTITY_FIELDS: &[&str] = &[
    "working_asset_id",
    "category",
    "functional_scope",
    "source_labels",
    "identity_claims",
    "name_status",
    "selected_label",
];
const CLAIM_FIELDS: &[&str] = &[
    "claim_id",
    "source_kind",
    "label",
    "evidence_sha256",
    "confidence",
];
const AUTHORIZATION_FIELDS: &[&str] = &[
    "status",
    "rights_scope",
    "restrictions",
    "territory",
    "term",
    "revocation",
    "sublicense",
    "source_reference_sha256",
    "evidence_status",
    "user_confirmation_required",
];
const COVERAGE_FIELDS: &[&str] = &[
    "source_reference_sha256",
    "source_dimensions",
    "required_views",
    "supplied_views",
    "missing_views",
    "detail_views",
    "coverage_status",
    "hq_360_status",
    "camera_status",
];
const DIMENSION_FIELDS: &[&str] = &["width", "height"];
const SILHOUETTE_FIELDS: &[&str] = &["rank", "focus", "source_status"];
const PART_FIELDS: &[&str] = &[
    "part_id",
    "role",
    "parent_id",
    "material_zone_ids",
    "source_status",
    "fps_priority",
];
const MATERIAL_FIELDS: &[&str] = &[
    "zone_id",
    "zone_role",
    "surface_language",
    "channels",
    "target_share_percent",
    "roughness_range",
    "emissive_allowed",
    "source_status",
];
const RANGE_FIELDS: &[&str] = &["min", "max"];
const SURFACE_FIELDS: &[&str] = &[
    "hero_budget",
    "lod_levels",
    "texture_policy",
    "topology_policy",
];
const HERO_FIELDS: &[&str] = &[
    "status",
    "resolved_min_triangles",
    "resolved_max_triangles",
    "claims",
    "blocks",
];
const TRIANGLE_CLAIM_FIELDS: &[&str] = &[
    "claim_id",
    "source_kind",
    "value_kind",
    "min_triangles",
    "max_triangles",
    "evidence_sha256",
    "confidence",
];
const LOD_FIELDS: &[&str] = &["level_id", "target_percent"];
const TEXTURE_FIELDS: &[&str] = &[
    "resolution_status",
    "resolved_width",
    "resolved_height",
    "shipping_width",
    "shipping_height",
    "resolution_claims",
    "udim_tile_max",
    "material_slot_count",
    "layout",
    "channels",
];
const RESOLUTION_CLAIM_FIELDS: &[&str] = &[
    "claim_id",
    "source_kind",
    "width",
    "height",
    "usage",
    "evidence_sha256",
    "confidence",
];
const TOPOLOGY_FIELDS: &[&str] = &[
    "non_manifold_allowed",
    "duplicate_faces_allowed",
    "floating_vertices_allowed",
    "edge_highlight_policy",
    "low_editable_required",
    "hidden_fallback_allowed",
];
const PRESENTATION_FIELDS: &[&str] = &[
    "hand_side",
    "grip_region",
    "blade_direction",
    "inspect_focus_order",
    "socket_ids",
    "animation_clips",
    "lighting_modes",
];
const ENGINE_FIELDS: &[&str] = &[
    "profile_status",
    "preferred_engine",
    "preferred_engine_version",
    "target_claims",
    "export_formats",
    "optional_export_formats",
    "unit_status",
    "unit_options",
    "selected_unit",
    "axis_status",
    "selected_axis_profile",
    "pivot_policy",
    "validation_requirements",
];
const ENGINE_CLAIM_FIELDS: &[&str] = &[
    "claim_id",
    "engine_family",
    "version_requirement",
    "source_kind",
    "evidence_sha256",
    "confidence",
];
const CONFLICT_FIELDS: &[&str] = &[
    "conflict_id",
    "kind",
    "observed_claim_ids",
    "resolution_status",
    "blocking",
];
const ACCEPTANCE_FIELDS: &[&str] = &[
    "status",
    "required_gates",
    "gate_statuses",
    "blocking_reasons",
    "promotion_labels",
    "runtime_sole_writer",
    "prototype_not_truth",
    "human_artist_required",
    "user_approval_required",
    "confirm_export_requires_all_gates",
];
const GATE_FIELDS: &[&str] = &["gate_id", "status"];

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
const DETAIL_VIEWS: &[&str] = &[
    "blade-detail",
    "guard-detail",
    "handle-detail",
    "engraving-detail",
    "wear-detail",
];
const CORE_AUTHORING_VIEWS: &[&str] = &["front", "back", "left", "right"];
const CHANNELS: &[&str] = &[
    "base-color",
    "normal",
    "roughness",
    "metallic",
    "ao",
    "emissive",
    "clearcoat",
];

#[derive(Debug, Clone)]
pub(crate) struct BriefValidation {
    pub brief: Value,
    pub brief_id: String,
    pub project_id: String,
    pub brief_sha256: String,
    pub source_reference_sha256: String,
    pub conflict_status: &'static str,
    pub authorization_binding_status: &'static str,
    pub authoring_eligibility: &'static str,
    pub blocking_reasons: Vec<String>,
    pub payload_claims_runtime_bound: bool,
    pub core_coverage: bool,
    pub parent_brief_id: Option<String>,
    pub parent_brief_sha256: Option<String>,
    pub freeze_policy: String,
}

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "WEAPONRY_KNIFE_PRODUCTION_BRIEF_INVALID: {}",
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
    let expected: BTreeSet<&str> = fields.iter().copied().collect();
    let actual: BTreeSet<&str> = object.keys().map(String::as_str).collect();
    if expected != actual {
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

fn opaque(object: &Map<String, Value>, field: &str, context: &str) -> Result<String, RuntimeError> {
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

fn enum_value(
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

fn bool_is(
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

fn string_array<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    context: &str,
    min: usize,
    max: usize,
) -> Result<Vec<&'a str>, RuntimeError> {
    let values = object
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(format!("{context}.{field} must be an array")))?;
    if values.len() < min || values.len() > max {
        return Err(invalid(format!(
            "{context}.{field} length is out of bounds"
        )));
    }
    let mut seen = BTreeSet::new();
    let mut result = Vec::with_capacity(values.len());
    for value in values {
        let value = value
            .as_str()
            .ok_or_else(|| invalid(format!("{context}.{field} must contain strings")))?;
        if !seen.insert(value) {
            return Err(invalid(format!("{context}.{field} contains duplicates")));
        }
        result.push(value);
    }
    Ok(result)
}

fn enum_array<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    context: &str,
    allowed: &[&str],
    min: usize,
    max: usize,
) -> Result<Vec<&'a str>, RuntimeError> {
    let result = string_array(object, field, context, min, max)?;
    if result.iter().any(|value| !allowed.contains(value)) {
        return Err(invalid(format!(
            "{context}.{field} contains an unknown enum value"
        )));
    }
    Ok(result)
}

fn unsigned(value: Option<&Value>, context: &str) -> Result<u64, RuntimeError> {
    value
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid(format!("{context} must be a non-negative integer")))
}

fn finite(value: Option<&Value>, context: &str) -> Result<f64, RuntimeError> {
    value
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| invalid(format!("{context} must be finite")))
}

fn safe_text(value: Option<&Value>, context: &str) -> Result<String, RuntimeError> {
    let value = value
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{context} must be text")))?;
    if value.is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
        return Err(invalid(format!("{context} is not bounded text")));
    }
    let lower = value.to_ascii_lowercase();
    let digit_count = value.chars().filter(|value| value.is_ascii_digit()).count();
    let contains_secret_assignment = ["password", "api_key", "api-key", "secret", "token"]
        .iter()
        .any(|name| lower.contains(&format!("{name}:")));
    let suspicious = lower.contains('/')
        || lower.contains('\\')
        || lower.contains("file:")
        || lower.contains("data:")
        || lower.contains("http:")
        || lower.contains("https:")
        || lower.contains("ftp:")
        || lower.contains("../")
        || lower.contains("..\\")
        || lower.contains("blender --python")
        || lower.contains("plugin")
        || lower.contains("add-on")
        || contains_secret_assignment
        || lower.contains("bearer ")
        || lower.contains('@')
        || digit_count >= 8;
    if suspicious {
        return Err(invalid(format!(
            "{context} contains a path, URL, executable or secret"
        )));
    }
    Ok(value.to_owned())
}

fn register_claim(
    claims: &mut BTreeMap<String, &'static str>,
    claim_id: String,
    family: &'static str,
) -> Result<(), RuntimeError> {
    if claims.insert(claim_id, family).is_some() {
        return Err(invalid(
            "source claim_id must be unique across identity, surface and engine claims",
        ));
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
        .all(|range| {
            value.as_bytes()[range.clone()]
                .iter()
                .all(u8::is_ascii_digit)
        })
    {
        return false;
    }
    if bytes.len() > 20 {
        bytes[19] == b'.'
            && bytes[20..bytes.len() - 1].iter().all(u8::is_ascii_digit)
            && (1..=6).contains(&(bytes.len() - 21))
    } else {
        true
    }
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
    ];
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if FORBIDDEN_KEYS.contains(&key.to_ascii_lowercase().as_str()) {
                    return Err(invalid(
                        "brief contains a forbidden source or executable field",
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
        _ => {}
    }
    Ok(())
}

fn validate_request_hash(request: &Value, object: &Map<String, Value>) -> Result<(), RuntimeError> {
    let supplied = hash(object, "input_sha256", "request")?;
    let mut preimage = request.clone();
    preimage["input_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != supplied {
        return Err(mismatch(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_INPUT_CANONICAL_MISMATCH",
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
    if object.get("max_response_bytes").and_then(Value::as_u64) != Some(MAX_RESPONSE_BYTES) {
        return Err(invalid(format!(
            "{context}.max_response_bytes must be exactly 1048576"
        )));
    }
    if object
        .get("runtime_write_performed")
        .and_then(Value::as_bool)
        != Some(false)
    {
        return Err(invalid(format!(
            "{context}.runtime_write_performed must be false"
        )));
    }
    if object.get("writer_policy").and_then(Value::as_str) != Some(WRITER_POLICY)
        || object
            .get("canonicalization_policy")
            .and_then(Value::as_str)
            != Some(REQUEST_CANONICALIZATION)
    {
        return Err(invalid(format!(
            "{context} writer/canonicalization policy differs"
        )));
    }
    Ok(())
}

fn validate_identity(
    value: &Value,
    claims: &mut BTreeMap<String, &'static str>,
) -> Result<String, RuntimeError> {
    let object = exact_object(value, IDENTITY_FIELDS, "asset_identity")?;
    opaque(object, "working_asset_id", "asset_identity")?;
    enum_value(
        object,
        "category",
        &[
            "knife",
            "kukri",
            "fixed-blade-knife",
            "original-control-knife",
        ],
        "asset_identity",
    )?;
    if text(object, "functional_scope", "asset_identity")? != "nonfunctional-fps-game-visual" {
        return Err(invalid("asset_identity.functional_scope differs"));
    }
    let labels = string_array(object, "source_labels", "asset_identity", 1, 8)?;
    for label in &labels {
        safe_text(
            Some(&Value::String((*label).to_owned())),
            "asset_identity.source_labels",
        )?;
    }
    let items = object
        .get("identity_claims")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("asset_identity.identity_claims must be an array"))?;
    if items.is_empty() || items.len() > 8 {
        return Err(invalid(
            "asset_identity.identity_claims length is out of bounds",
        ));
    }
    let mut ids = BTreeSet::new();
    for item in items {
        let item = exact_object(item, CLAIM_FIELDS, "identity_claim")?;
        let claim_id = opaque(item, "claim_id", "identity_claim")?;
        if !ids.insert(claim_id.clone()) {
            return Err(invalid("duplicate identity claim_id"));
        }
        enum_value(
            item,
            "source_kind",
            &["image-panel", "text-brief", "author", "benchmark"],
            "identity_claim",
        )?;
        let label = safe_text(item.get("label"), "identity_claim.label")?;
        if !labels.contains(&label.as_str()) {
            return Err(mismatch(
                "WEAPONRY_KNIFE_PRODUCTION_BRIEF_IDENTITY_CLAIM_MISMATCH",
                "identity claim label is not retained in source_labels",
            ));
        }
        hash(item, "evidence_sha256", "identity_claim")?;
        enum_value(
            item,
            "confidence",
            &["high", "medium", "ambiguous"],
            "identity_claim",
        )?;
        register_claim(claims, claim_id, "identity")?;
    }
    let status = enum_value(
        object,
        "name_status",
        &["resolved", "unresolved", "not-applicable"],
        "asset_identity",
    )?;
    let selected = match object.get("selected_label") {
        Some(Value::Null) => None,
        Some(value) => Some(safe_text(Some(value), "asset_identity.selected_label")?),
        None => return Err(invalid("asset_identity.selected_label is missing")),
    };
    match status.as_str() {
        "resolved"
            if selected
                .as_deref()
                .is_some_and(|selected| labels.iter().any(|label| *label == selected)) => {}
        "unresolved" if labels.len() >= 2 && selected.is_none() => {}
        "not-applicable" if selected.is_none() => {}
        _ => {
            return Err(mismatch(
                "WEAPONRY_KNIFE_PRODUCTION_BRIEF_IDENTITY_RESOLUTION_INVALID",
                "identity resolution must explicitly retain or select labels",
            ))
        }
    }
    Ok(status)
}

fn validate_authorization(value: &Value) -> Result<(String, String), RuntimeError> {
    let object = exact_object(value, AUTHORIZATION_FIELDS, "authorization")?;
    let status = enum_value(
        object,
        "status",
        &[
            "source-asserted",
            "user-confirmed",
            "unavailable",
            "conflicted",
        ],
        "authorization",
    )?;
    enum_array(
        object,
        "rights_scope",
        "authorization",
        &[
            "commercial-use",
            "noncommercial-use",
            "in-game-display",
            "promotional-media",
            "ui-and-marketing",
            "internal-training",
            "derivative-editing",
            "format-conversion",
            "single-project-use",
            "multi-project-use",
        ],
        1,
        12,
    )?;
    enum_array(
        object,
        "restrictions",
        "authorization",
        &["illegal-content", "political-content", "religious-content"],
        0,
        8,
    )?;
    enum_value(
        object,
        "territory",
        &["worldwide", "project-scoped", "unknown"],
        "authorization",
    )?;
    enum_value(
        object,
        "term",
        &["perpetual", "time-limited", "unknown"],
        "authorization",
    )?;
    enum_value(
        object,
        "revocation",
        &["irrevocable", "revocable", "unknown"],
        "authorization",
    )?;
    enum_value(
        object,
        "sublicense",
        &["internal", "none", "unknown"],
        "authorization",
    )?;
    let source = hash(object, "source_reference_sha256", "authorization")?;
    let evidence = enum_value(
        object,
        "evidence_status",
        &[
            "source-asserted-not-runtime-bound",
            "runtime-bound",
            "unavailable",
        ],
        "authorization",
    )?;
    let confirmation_required = object
        .get("user_confirmation_required")
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid("authorization.user_confirmation_required must be boolean"))?;
    if confirmation_required != (status != "user-confirmed") {
        return Err(mismatch(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_AUTHORIZATION_CONFIRMATION_INVALID",
            "user-confirmed authorization must close the confirmation prompt; all other states must remain pending",
        ));
    }
    let coherent = matches!(
        (status.as_str(), evidence.as_str()),
        ("source-asserted", "source-asserted-not-runtime-bound")
            | ("user-confirmed", "runtime-bound")
            | ("unavailable", "unavailable")
            | ("conflicted", "unavailable")
    );
    if !coherent {
        return Err(mismatch(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_AUTHORIZATION_BINDING_INVALID",
            "authorization status and evidence_status disagree",
        ));
    }
    Ok((source, evidence))
}

fn validate_coverage(value: &Value, source: &str) -> Result<bool, RuntimeError> {
    let object = exact_object(value, COVERAGE_FIELDS, "reference_coverage")?;
    if hash(object, "source_reference_sha256", "reference_coverage")? != source {
        return Err(mismatch(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_SOURCE_HASH_MISMATCH",
            "authorization and coverage source hashes differ",
        ));
    }
    let dimensions = exact_object(
        object
            .get("source_dimensions")
            .ok_or_else(|| invalid("source_dimensions missing"))?,
        DIMENSION_FIELDS,
        "source_dimensions",
    )?;
    let width = unsigned(dimensions.get("width"), "source_dimensions.width")?;
    let height = unsigned(dimensions.get("height"), "source_dimensions.height")?;
    if width == 0 || width > 16_384 || height == 0 || height > 16_384 {
        return Err(invalid("source_dimensions exceed bounds"));
    }
    let required = enum_array(object, "required_views", "reference_coverage", VIEWS, 1, 10)?;
    let supplied = enum_array(object, "supplied_views", "reference_coverage", VIEWS, 0, 10)?;
    let missing = enum_array(object, "missing_views", "reference_coverage", VIEWS, 0, 10)?;
    enum_array(
        object,
        "detail_views",
        "reference_coverage",
        DETAIL_VIEWS,
        0,
        5,
    )?;
    let required: BTreeSet<&str> = required.into_iter().collect();
    let supplied: BTreeSet<&str> = supplied.into_iter().collect();
    let missing: BTreeSet<&str> = missing.into_iter().collect();
    if !supplied.is_disjoint(&missing)
        || supplied.union(&missing).copied().collect::<BTreeSet<_>>() != required
    {
        return Err(mismatch(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_COVERAGE_PARTITION_INVALID",
            "supplied/missing views do not partition required views",
        ));
    }
    let status = enum_value(
        object,
        "coverage_status",
        &["complete", "partial", "blocked"],
        "reference_coverage",
    )?;
    let hq = enum_value(
        object,
        "hq_360_status",
        &["eligible", "BLOCKED_REFERENCE_COVERAGE"],
        "reference_coverage",
    )?;
    enum_value(
        object,
        "camera_status",
        &["observed", "inferred", "unknown"],
        "reference_coverage",
    )?;
    if status == "complete" && (!missing.is_empty() || hq != "eligible") {
        return Err(mismatch(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_COVERAGE_STATUS_INVALID",
            "complete coverage requires no missing views and eligible HQ360",
        ));
    }
    if status != "complete" && (missing.is_empty() || hq != "BLOCKED_REFERENCE_COVERAGE") {
        return Err(mismatch(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_COVERAGE_STATUS_INVALID",
            "partial/blocked coverage must remain HQ360 blocked",
        ));
    }
    let core = CORE_AUTHORING_VIEWS
        .iter()
        .all(|view| supplied.contains(view))
        && status != "blocked";
    Ok(core)
}

fn validate_silhouette(value: &Value) -> Result<(), RuntimeError> {
    let values = value
        .as_array()
        .ok_or_else(|| invalid("silhouette_priorities must be an array"))?;
    if values.is_empty() || values.len() > 16 {
        return Err(invalid("silhouette_priorities length is out of bounds"));
    }
    let mut ranks = BTreeSet::new();
    for item in values {
        let object = exact_object(item, SILHOUETTE_FIELDS, "silhouette_priority")?;
        let rank = unsigned(object.get("rank"), "silhouette_priority.rank")?;
        if rank == 0 || rank > 16 || !ranks.insert(rank) {
            return Err(invalid(
                "silhouette priority ranks must be unique and bounded",
            ));
        }
        opaque(object, "focus", "silhouette_priority")?;
        enum_value(
            object,
            "source_status",
            &["observed", "inferred", "unknown"],
            "silhouette_priority",
        )?;
    }
    if ranks.iter().copied().collect::<Vec<_>>() != (1..=values.len() as u64).collect::<Vec<_>>() {
        return Err(invalid("silhouette priority ranks must be contiguous"));
    }
    Ok(())
}

fn validate_parts_materials(value: &Value) -> Result<BTreeSet<String>, RuntimeError> {
    let parts = value
        .get("parts")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("parts must be an array"))?;
    if parts.is_empty() || parts.len() > 128 {
        return Err(invalid("parts length is out of bounds"));
    }
    let mut part_ids = BTreeSet::new();
    let mut parents = Vec::new();
    for item in parts {
        let object = exact_object(item, PART_FIELDS, "part")?;
        let part_id = opaque(object, "part_id", "part")?;
        if !part_ids.insert(part_id) {
            return Err(invalid("duplicate part_id"));
        }
        enum_value(
            object,
            "role",
            &[
                "blade",
                "cutting-edge",
                "blade-body",
                "relief",
                "guard",
                "gem",
                "grip",
                "fastener",
                "pommel",
                "handle",
                "component",
                "other",
            ],
            "part",
        )?;
        match object.get("parent_id") {
            Some(Value::Null) => {}
            Some(Value::String(value)) if is_opaque_id(value) => parents.push(value.clone()),
            _ => return Err(invalid("part.parent_id must be null or an opaque id")),
        }
        let _ = string_array(object, "material_zone_ids", "part", 1, 16)?;
        enum_value(
            object,
            "source_status",
            &["observed", "inferred", "unknown"],
            "part",
        )?;
        enum_value(
            object,
            "fps_priority",
            &["hero", "support", "detail"],
            "part",
        )?;
    }
    if parents.iter().any(|parent| !part_ids.contains(parent)) {
        return Err(mismatch(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_PART_PARENT_MISMATCH",
            "part parent is unknown",
        ));
    }
    let zones = value
        .get("material_zones")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("material_zones must be an array"))?;
    if zones.is_empty() || zones.len() > 64 {
        return Err(invalid("material_zones length is out of bounds"));
    }
    let mut zone_ids = BTreeSet::new();
    let mut total = 0.0;
    for item in zones {
        let object = exact_object(item, MATERIAL_FIELDS, "material_zone")?;
        let zone_id = opaque(object, "zone_id", "material_zone")?;
        if !zone_ids.insert(zone_id) {
            return Err(invalid("duplicate zone_id"));
        }
        enum_value(
            object,
            "zone_role",
            &[
                "metal",
                "coating",
                "grip",
                "gem",
                "wood",
                "composite",
                "other",
            ],
            "material_zone",
        )?;
        safe_text(
            object.get("surface_language"),
            "material_zone.surface_language",
        )?;
        let channels = enum_array(object, "channels", "material_zone", CHANNELS, 1, 7)?;
        let share = finite(
            object.get("target_share_percent"),
            "material_zone.target_share_percent",
        )?;
        if !(0.0..=100.0).contains(&share) {
            return Err(invalid("material zone target share is out of bounds"));
        }
        total += share;
        let range = exact_object(
            object
                .get("roughness_range")
                .ok_or_else(|| invalid("roughness_range missing"))?,
            RANGE_FIELDS,
            "roughness_range",
        )?;
        let min = finite(range.get("min"), "roughness_range.min")?;
        let max = finite(range.get("max"), "roughness_range.max")?;
        if !(0.0..=1.0).contains(&min) || !(0.0..=1.0).contains(&max) || min > max {
            return Err(invalid("roughness range is invalid"));
        }
        let emissive = object
            .get("emissive_allowed")
            .and_then(Value::as_bool)
            .ok_or_else(|| invalid("emissive_allowed must be boolean"))?;
        if emissive != channels.contains(&"emissive") {
            return Err(mismatch(
                "WEAPONRY_KNIFE_PRODUCTION_BRIEF_MATERIAL_CHANNEL_MISMATCH",
                "emissive policy differs from channels",
            ));
        }
        enum_value(
            object,
            "source_status",
            &["observed", "inferred", "unknown"],
            "material_zone",
        )?;
    }
    if (total - 100.0).abs() > 1e-6 {
        return Err(mismatch(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_MATERIAL_SHARE_INVALID",
            "material zone shares must total 100",
        ));
    }
    for item in parts {
        let object = item.as_object().expect("validated part");
        for zone in object["material_zone_ids"]
            .as_array()
            .expect("validated ids")
        {
            if !zone_ids.contains(zone.as_str().expect("zone id")) {
                return Err(mismatch(
                    "WEAPONRY_KNIFE_PRODUCTION_BRIEF_MATERIAL_ZONE_MISMATCH",
                    "part references unknown material zone",
                ));
            }
        }
    }
    Ok(part_ids)
}

fn optional_u64(
    object: &Map<String, Value>,
    field: &str,
    context: &str,
    max: u64,
) -> Result<Option<u64>, RuntimeError> {
    match object.get(field) {
        Some(Value::Null) => Ok(None),
        Some(value) => {
            let value = unsigned(Some(value), &format!("{context}.{field}"))?;
            if value == 0 || value > max {
                return Err(invalid(format!("{context}.{field} is out of bounds")));
            }
            Ok(Some(value))
        }
        None => Err(invalid(format!("{context}.{field} is missing"))),
    }
}

fn validate_surface(
    value: &Value,
    claims: &mut BTreeMap<String, &'static str>,
) -> Result<(String, String), RuntimeError> {
    let object = exact_object(value, SURFACE_FIELDS, "surface_constraints")?;
    let hero = exact_object(
        object
            .get("hero_budget")
            .ok_or_else(|| invalid("hero_budget missing"))?,
        HERO_FIELDS,
        "hero_budget",
    )?;
    let hero_status = enum_value(
        hero,
        "status",
        &["resolved", "conflicted", "not-run"],
        "hero_budget",
    )?;
    let min = optional_u64(hero, "resolved_min_triangles", "hero_budget", 1_000_000)?;
    let max = optional_u64(hero, "resolved_max_triangles", "hero_budget", 1_000_000)?;
    if min.zip(max).is_some_and(|(min, max)| min > max) {
        return Err(invalid("hero triangle range is inverted"));
    }
    let hero_claims = hero
        .get("claims")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("hero_budget.claims must be an array"))?;
    if hero_claims.len() > 8 {
        return Err(invalid("hero_budget.claims exceeds bound"));
    }
    let mut ids = BTreeSet::new();
    let mut selected = Vec::new();
    for item in hero_claims {
        let item = exact_object(item, TRIANGLE_CLAIM_FIELDS, "triangle_claim")?;
        let id = opaque(item, "claim_id", "triangle_claim")?;
        if !ids.insert(id.clone()) {
            return Err(invalid("duplicate triangle claim_id"));
        }
        enum_value(
            item,
            "source_kind",
            &["image-panel", "text-brief", "author", "benchmark"],
            "triangle_claim",
        )?;
        enum_value(item, "value_kind", &["exact", "range"], "triangle_claim")?;
        let min = unsigned(item.get("min_triangles"), "triangle_claim.min_triangles")?;
        let max = unsigned(item.get("max_triangles"), "triangle_claim.max_triangles")?;
        if min == 0 || max == 0 || min > max || max > 1_000_000 {
            return Err(invalid("triangle claim range is invalid"));
        }
        hash(item, "evidence_sha256", "triangle_claim")?;
        enum_value(
            item,
            "confidence",
            &["high", "medium", "ambiguous"],
            "triangle_claim",
        )?;
        register_claim(claims, id.clone(), "hero")?;
        selected.push((id, min, max));
    }
    let blocks = string_array(hero, "blocks", "hero_budget", 0, 8)?;
    if blocks.iter().any(|value| !is_opaque_id(value)) {
        return Err(invalid("hero_budget block is not an opaque identifier"));
    }
    match hero_status.as_str() {
        "resolved" => {
            let (min, max) = min
                .zip(max)
                .ok_or_else(|| invalid("resolved hero budget needs selected values"))?;
            if hero_claims.is_empty()
                || !blocks.is_empty()
                || selected
                    .iter()
                    .filter(|(_, claim_min, claim_max)| *claim_min == min && *claim_max == max)
                    .count()
                    == 0
            {
                return Err(mismatch(
                    "WEAPONRY_KNIFE_PRODUCTION_BRIEF_RESOLUTION_NOT_EXPLICIT",
                    "resolved hero budget must select a retained claim value",
                ));
            }
        }
        "conflicted" => {
            if min.is_some() || max.is_some() || hero_claims.len() < 2 || blocks.is_empty() {
                return Err(mismatch(
                    "WEAPONRY_KNIFE_PRODUCTION_BRIEF_CONFLICT_FREEZE_INVALID",
                    "conflicted hero budget must retain claims and blockers",
                ));
            }
        }
        "not-run" => {
            if min.is_some() || max.is_some() {
                return Err(invalid("not-run hero budget cannot carry resolved values"));
            }
        }
        _ => unreachable!(),
    }
    let texture = exact_object(
        object
            .get("texture_policy")
            .ok_or_else(|| invalid("texture_policy missing"))?,
        TEXTURE_FIELDS,
        "texture_policy",
    )?;
    let texture_status = enum_value(
        texture,
        "resolution_status",
        &["resolved", "conflicted", "not-run"],
        "texture_policy",
    )?;
    let width = optional_u64(texture, "resolved_width", "texture_policy", 16_384)?;
    let height = optional_u64(texture, "resolved_height", "texture_policy", 16_384)?;
    let shipping_width = optional_u64(texture, "shipping_width", "texture_policy", 16_384)?;
    let shipping_height = optional_u64(texture, "shipping_height", "texture_policy", 16_384)?;
    let resolution_claims = texture
        .get("resolution_claims")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("texture_policy.resolution_claims must be an array"))?;
    if resolution_claims.len() > 8 {
        return Err(invalid("too many texture claims"));
    }
    let mut resolution_ids = BTreeSet::new();
    let mut resolution_values = Vec::new();
    for item in resolution_claims {
        let item = exact_object(item, RESOLUTION_CLAIM_FIELDS, "resolution_claim")?;
        let id = opaque(item, "claim_id", "resolution_claim")?;
        if !resolution_ids.insert(id.clone()) {
            return Err(invalid("duplicate resolution claim_id"));
        }
        enum_value(
            item,
            "source_kind",
            &["image-panel", "text-brief", "author", "benchmark"],
            "resolution_claim",
        )?;
        let width = unsigned(item.get("width"), "resolution_claim.width")?;
        let height = unsigned(item.get("height"), "resolution_claim.height")?;
        if width == 0 || height == 0 || width > 16_384 || height > 16_384 {
            return Err(invalid("resolution claim exceeds bounds"));
        }
        let usage = enum_value(
            item,
            "usage",
            &["hero", "production", "unspecified"],
            "resolution_claim",
        )?;
        hash(item, "evidence_sha256", "resolution_claim")?;
        enum_value(
            item,
            "confidence",
            &["high", "medium", "ambiguous"],
            "resolution_claim",
        )?;
        register_claim(claims, id.clone(), "texture")?;
        resolution_values.push((id, width, height, usage));
    }
    match texture_status.as_str() {
        "resolved" => {
            let (width, height) = width
                .zip(height)
                .ok_or_else(|| invalid("resolved texture policy needs selected values"))?;
            let (shipping_width, shipping_height) = shipping_width
                .zip(shipping_height)
                .ok_or_else(|| invalid("resolved texture policy needs shipping values"))?;
            if resolution_claims.is_empty()
                || resolution_values
                    .iter()
                    .filter(|(_, claim_width, claim_height, _)| {
                        *claim_width == width && *claim_height == height
                    })
                    .count()
                    == 0
            {
                return Err(mismatch(
                    "WEAPONRY_KNIFE_PRODUCTION_BRIEF_RESOLUTION_NOT_EXPLICIT",
                    "resolved texture policy must select a retained claim value",
                ));
            }
            if !resolution_values
                .iter()
                .any(|(_, claim_width, claim_height, usage)| {
                    *claim_width == shipping_width
                        && *claim_height == shipping_height
                        && usage == "production"
                })
            {
                return Err(mismatch(
                    "WEAPONRY_KNIFE_PRODUCTION_BRIEF_SHIPPING_RESOLUTION_NOT_EXPLICIT",
                    "resolved shipping texture must select a retained production claim value",
                ));
            }
        }
        "conflicted" => {
            if width.is_some()
                || height.is_some()
                || shipping_width.is_some()
                || shipping_height.is_some()
                || resolution_claims.len() < 2
            {
                return Err(mismatch(
                    "WEAPONRY_KNIFE_PRODUCTION_BRIEF_CONFLICT_FREEZE_INVALID",
                    "conflicted texture policy must retain claims",
                ));
            }
        }
        "not-run" => {
            if width.is_some()
                || height.is_some()
                || shipping_width.is_some()
                || shipping_height.is_some()
            {
                return Err(invalid(
                    "not-run texture policy cannot carry resolved values",
                ));
            }
        }
        _ => unreachable!(),
    }
    let udim = unsigned(texture.get("udim_tile_max"), "texture_policy.udim_tile_max")?;
    if udim > 16 {
        return Err(invalid("texture_policy.udim_tile_max is out of bounds"));
    }
    let slots = unsigned(
        texture.get("material_slot_count"),
        "texture_policy.material_slot_count",
    )?;
    if slots == 0 || slots > 32 {
        return Err(invalid(
            "texture_policy.material_slot_count is out of bounds",
        ));
    }
    for value in string_array(texture, "layout", "texture_policy", 1, 32)? {
        if !is_opaque_id(value) {
            return Err(invalid(
                "texture_policy.layout contains an invalid identifier",
            ));
        }
    }
    enum_array(texture, "channels", "texture_policy", CHANNELS, 1, 7)?;
    let topology = exact_object(
        object
            .get("topology_policy")
            .ok_or_else(|| invalid("topology_policy missing"))?,
        TOPOLOGY_FIELDS,
        "topology_policy",
    )?;
    bool_is(topology, "non_manifold_allowed", false, "topology_policy")?;
    bool_is(
        topology,
        "duplicate_faces_allowed",
        false,
        "topology_policy",
    )?;
    bool_is(
        topology,
        "floating_vertices_allowed",
        false,
        "topology_policy",
    )?;
    enum_value(
        topology,
        "edge_highlight_policy",
        &[
            "screen-space-controlled",
            "profile-relative-bounded",
            "not-specified",
        ],
        "topology_policy",
    )?;
    bool_is(topology, "low_editable_required", true, "topology_policy")?;
    bool_is(
        topology,
        "hidden_fallback_allowed",
        false,
        "topology_policy",
    )?;
    let lods = object
        .get("lod_levels")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("lod_levels must be an array"))?;
    if lods.is_empty() || lods.len() > 8 {
        return Err(invalid("lod_levels length is out of bounds"));
    }
    let mut levels = BTreeSet::new();
    let mut previous = 101.0;
    for item in lods {
        let item = exact_object(item, LOD_FIELDS, "lod_level")?;
        let level = opaque(item, "level_id", "lod_level")?;
        if !levels.insert(level) {
            return Err(invalid("duplicate LOD level_id"));
        }
        let target = finite(item.get("target_percent"), "lod_level.target_percent")?;
        if !(0.0..=100.0).contains(&target) || target > previous {
            return Err(invalid("LOD targets must monotonically decrease"));
        }
        previous = target;
    }
    Ok((hero_status, texture_status))
}

fn validate_presentation(value: &Value, part_ids: &BTreeSet<String>) -> Result<(), RuntimeError> {
    let object = exact_object(value, PRESENTATION_FIELDS, "presentation_constraints")?;
    enum_value(
        object,
        "hand_side",
        &["left", "right", "ambidextrous", "unknown"],
        "presentation_constraints",
    )?;
    enum_value(
        object,
        "grip_region",
        &["lower-left", "lower-right", "center", "unknown"],
        "presentation_constraints",
    )?;
    enum_value(
        object,
        "blade_direction",
        &[
            "upper-left",
            "upper-right",
            "horizontal",
            "vertical",
            "unknown",
        ],
        "presentation_constraints",
    )?;
    let focus = string_array(
        object,
        "inspect_focus_order",
        "presentation_constraints",
        1,
        16,
    )?;
    if focus.iter().any(|value| !part_ids.contains(*value)) {
        return Err(mismatch(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_PRESENTATION_PART_MISMATCH",
            "inspect focus references unknown part",
        ));
    }
    for field in ["socket_ids", "animation_clips", "lighting_modes"] {
        for value in string_array(object, field, "presentation_constraints", 1, 32)? {
            if !is_opaque_id(value) {
                return Err(invalid(format!(
                    "presentation_constraints.{field} contains an invalid identifier"
                )));
            }
        }
    }
    Ok(())
}

fn validate_engine(
    value: &Value,
    claims: &mut BTreeMap<String, &'static str>,
) -> Result<(String, bool, bool), RuntimeError> {
    let object = exact_object(value, ENGINE_FIELDS, "engine_constraints")?;
    let status = enum_value(
        object,
        "profile_status",
        &["resolved", "conflicted", "not-run"],
        "engine_constraints",
    )?;
    let preferred = match object.get("preferred_engine") {
        Some(Value::Null) => None,
        Some(Value::String(value))
            if ["unreal", "unity", "godot", "custom"].contains(&value.as_str()) =>
        {
            Some(value.as_str())
        }
        _ => return Err(invalid("engine_constraints.preferred_engine is invalid")),
    };
    let preferred_version = match object.get("preferred_engine_version") {
        Some(Value::Null) => None,
        Some(value) => Some(safe_text(
            Some(value),
            "engine_constraints.preferred_engine_version",
        )?),
        None => {
            return Err(invalid(
                "engine_constraints.preferred_engine_version is missing",
            ))
        }
    };
    let items = object
        .get("target_claims")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("target_claims must be an array"))?;
    if items.len() > 8 {
        return Err(invalid("too many engine claims"));
    }
    let mut ids = BTreeSet::new();
    let mut targets = Vec::new();
    for item in items {
        let item = exact_object(item, ENGINE_CLAIM_FIELDS, "engine_claim")?;
        let id = opaque(item, "claim_id", "engine_claim")?;
        if !ids.insert(id.clone()) {
            return Err(invalid("duplicate engine claim_id"));
        }
        let family = enum_value(
            item,
            "engine_family",
            &["unreal", "unity", "godot", "custom"],
            "engine_claim",
        )?;
        let version = safe_text(
            item.get("version_requirement"),
            "engine_claim.version_requirement",
        )?;
        enum_value(
            item,
            "source_kind",
            &["image-panel", "text-brief", "author", "benchmark"],
            "engine_claim",
        )?;
        hash(item, "evidence_sha256", "engine_claim")?;
        enum_value(
            item,
            "confidence",
            &["high", "medium", "ambiguous"],
            "engine_claim",
        )?;
        register_claim(claims, id, "engine")?;
        targets.push((family, version));
    }
    match status.as_str() {
        "resolved" => {
            let preferred = preferred
                .ok_or_else(|| invalid("resolved engine profile needs preferred_engine"))?;
            let preferred_version = preferred_version
                .as_deref()
                .ok_or_else(|| invalid("resolved engine profile needs preferred_engine_version"))?;
            if items.is_empty()
                || targets
                    .iter()
                    .filter(|(family, version)| {
                        family.as_str() == preferred && version == preferred_version
                    })
                    .count()
                    == 0
            {
                return Err(mismatch(
                    "WEAPONRY_KNIFE_PRODUCTION_BRIEF_RESOLUTION_NOT_EXPLICIT",
                    "resolved engine must select a retained claim value",
                ));
            }
        }
        "conflicted" => {
            if preferred.is_some() || preferred_version.is_some() || items.len() < 2 {
                return Err(mismatch(
                    "WEAPONRY_KNIFE_PRODUCTION_BRIEF_CONFLICT_FREEZE_INVALID",
                    "conflicted engine profile must retain claims",
                ));
            }
        }
        "not-run" => {
            if preferred.is_some() || preferred_version.is_some() {
                return Err(invalid(
                    "not-run engine profile cannot carry preferred_engine",
                ));
            }
        }
        _ => unreachable!(),
    }
    enum_array(
        object,
        "export_formats",
        "engine_constraints",
        &["glb", "fbx", "usd", "gltf", "obj"],
        1,
        8,
    )?;
    enum_array(
        object,
        "optional_export_formats",
        "engine_constraints",
        &["glb", "fbx", "usd", "gltf", "obj"],
        0,
        8,
    )?;
    let unit = enum_value(
        object,
        "unit_status",
        &["resolved", "unresolved", "not-run"],
        "engine_constraints",
    )?;
    let axis = enum_value(
        object,
        "axis_status",
        &["resolved", "unresolved", "not-run"],
        "engine_constraints",
    )?;
    let unit_options = enum_array(
        object,
        "unit_options",
        "engine_constraints",
        &["meter", "centimeter", "engine-default", "normalized"],
        1,
        4,
    )?;
    let selected_unit = match object.get("selected_unit") {
        Some(Value::Null) => None,
        Some(Value::String(value))
            if ["meter", "centimeter", "engine-default", "normalized"]
                .contains(&value.as_str()) =>
        {
            Some(value.as_str())
        }
        _ => return Err(invalid("engine_constraints.selected_unit is invalid")),
    };
    let selected_axis = match object.get("selected_axis_profile") {
        Some(Value::Null) => None,
        Some(Value::String(value)) if is_opaque_id(value) => Some(value.as_str()),
        _ => {
            return Err(invalid(
                "engine_constraints.selected_axis_profile is invalid",
            ))
        }
    };
    match unit.as_str() {
        "resolved"
            if selected_unit.is_none()
                || !unit_options.contains(&selected_unit.expect("checked selected unit")) =>
        {
            return Err(mismatch(
                "WEAPONRY_KNIFE_PRODUCTION_BRIEF_ENGINE_UNIT_SELECTION_INVALID",
                "resolved engine unit must select a retained unit option",
            ));
        }
        "resolved" => {}
        _ if selected_unit.is_some() => {
            return Err(invalid("unresolved engine unit cannot carry selected_unit"))
        }
        _ => {}
    }
    match axis.as_str() {
        "resolved" if selected_axis.is_none() => {
            return Err(mismatch(
                "WEAPONRY_KNIFE_PRODUCTION_BRIEF_ENGINE_AXIS_SELECTION_INVALID",
                "resolved engine axis needs selected_axis_profile",
            ))
        }
        "resolved" => {}
        _ if selected_axis.is_some() => {
            return Err(invalid(
                "unresolved engine axis cannot carry selected_axis_profile",
            ))
        }
        _ => {}
    }
    opaque(object, "pivot_policy", "engine_constraints")?;
    for value in string_array(
        object,
        "validation_requirements",
        "engine_constraints",
        1,
        16,
    )? {
        if !is_opaque_id(value) {
            return Err(invalid("engine validation requirement is malformed"));
        }
    }
    Ok((status, unit == "resolved", axis == "resolved"))
}

fn validate_conflicts(
    value: &Value,
    claims: &BTreeMap<String, &'static str>,
    identity: &str,
    hero: &str,
    texture: &str,
    engine: &str,
) -> Result<bool, RuntimeError> {
    let values = value
        .get("source_conflicts")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("source_conflicts must be an array"))?;
    if values.len() > 32 {
        return Err(invalid("too many source conflicts"));
    }
    let mut ids = BTreeSet::new();
    let mut kinds = BTreeSet::new();
    let mut unresolved = false;
    let mut seen_expected = BTreeSet::new();
    for item in values {
        let object = exact_object(item, CONFLICT_FIELDS, "source_conflict")?;
        let id = opaque(object, "conflict_id", "source_conflict")?;
        if !ids.insert(id) {
            return Err(invalid("duplicate conflict_id"));
        }
        let kind = opaque(object, "kind", "source_conflict")?;
        if !kinds.insert(kind.clone()) {
            return Err(invalid("duplicate source conflict kind"));
        }
        let observed = string_array(object, "observed_claim_ids", "source_conflict", 2, 16)?;
        if observed.iter().collect::<BTreeSet<_>>().len() != observed.len() {
            return Err(invalid("source conflict observed_claim_ids must be unique"));
        }
        if observed.iter().any(|claim| !claims.contains_key(*claim)) {
            return Err(mismatch(
                "WEAPONRY_KNIFE_PRODUCTION_BRIEF_CONFLICT_CLAIM_MISMATCH",
                "conflict references an unknown claim",
            ));
        }
        let status = enum_value(
            object,
            "resolution_status",
            &["resolved", "unresolved"],
            "source_conflict",
        )?;
        let blocking = object
            .get("blocking")
            .and_then(Value::as_bool)
            .ok_or_else(|| invalid("source_conflict.blocking must be boolean"))?;
        if blocking != (status == "unresolved") {
            return Err(mismatch(
                "WEAPONRY_KNIFE_PRODUCTION_BRIEF_CONFLICT_FREEZE_INVALID",
                "blocking must follow resolution_status",
            ));
        }
        if status == "unresolved" {
            unresolved = true;
        }
        let expected = match kind.as_str() {
            "identity-label" => Some(identity != "resolved"),
            "hero-triangle-budget" => Some(hero != "resolved"),
            "texture-resolution" => Some(texture != "resolved"),
            "engine-profile" => Some(engine != "resolved"),
            _ => None,
        };
        if let Some(expected_unresolved) = expected {
            seen_expected.insert(kind.clone());
            if expected_unresolved != (status == "unresolved") {
                return Err(mismatch(
                    "WEAPONRY_KNIFE_PRODUCTION_BRIEF_CONFLICT_STATUS_MISMATCH",
                    "source conflict disagrees with selected field state",
                ));
            }
        }
    }
    for (kind, state) in [
        ("identity-label", identity),
        ("hero-triangle-budget", hero),
        ("texture-resolution", texture),
        ("engine-profile", engine),
    ] {
        if state != "resolved" && !seen_expected.contains(kind) {
            return Err(mismatch(
                "WEAPONRY_KNIFE_PRODUCTION_BRIEF_CONFLICT_FREEZE_INVALID",
                format!("unresolved {kind} has no frozen conflict record"),
            ));
        }
    }
    Ok(unresolved)
}

fn validate_acceptance(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(value, ACCEPTANCE_FIELDS, "acceptance_constraints")?;
    let status = enum_value(
        object,
        "status",
        &["ready", "blocked", "not-run"],
        "acceptance_constraints",
    )?;
    let required = string_array(object, "required_gates", "acceptance_constraints", 1, 32)?;
    let gates = object
        .get("gate_statuses")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("gate_statuses must be an array"))?;
    if gates.is_empty() || gates.len() > 32 {
        return Err(invalid("gate_statuses length is out of bounds"));
    }
    let mut ids = Vec::new();
    let mut gate_states = Vec::new();
    for item in gates {
        let item = exact_object(item, GATE_FIELDS, "gate_status")?;
        ids.push(opaque(item, "gate_id", "gate_status")?);
        gate_states.push(enum_value(
            item,
            "status",
            &["not-run", "blocked", "pass", "fail"],
            "gate_status",
        )?);
    }
    if ids.iter().map(String::as_str).collect::<Vec<_>>() != required {
        return Err(mismatch(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_ACCEPTANCE_DEPENDENCY_INVALID",
            "gate statuses differ from required_gates",
        ));
    }
    for field in ["blocking_reasons", "promotion_labels"] {
        for value in string_array(object, field, "acceptance_constraints", 0, 32)? {
            if !is_opaque_id(value) {
                return Err(invalid(format!(
                    "acceptance_constraints.{field} contains an invalid identifier"
                )));
            }
        }
    }
    bool_is(
        object,
        "runtime_sole_writer",
        true,
        "acceptance_constraints",
    )?;
    bool_is(
        object,
        "prototype_not_truth",
        true,
        "acceptance_constraints",
    )?;
    bool_is(
        object,
        "human_artist_required",
        true,
        "acceptance_constraints",
    )?;
    bool_is(
        object,
        "user_approval_required",
        true,
        "acceptance_constraints",
    )?;
    bool_is(
        object,
        "confirm_export_requires_all_gates",
        true,
        "acceptance_constraints",
    )?;
    let reasons = object["blocking_reasons"]
        .as_array()
        .expect("validated blockers");
    if status == "blocked" && reasons.is_empty() {
        return Err(invalid("blocked acceptance needs blocking_reasons"));
    }
    if status == "ready" && (!reasons.is_empty() || gate_states.iter().any(|value| value != "pass"))
    {
        return Err(invalid(
            "ready acceptance requires all gates pass and no blockers",
        ));
    }
    Ok(())
}

fn validate_acceptance_binding(
    value: &Value,
    evidence: &str,
    identity: &str,
    hero: &str,
    texture: &str,
    engine: &str,
) -> Result<(), RuntimeError> {
    let object = value
        .as_object()
        .expect("acceptance constraints were validated");
    if object["status"] != "blocked" {
        return Ok(());
    }
    let blockers = object["blocking_reasons"]
        .as_array()
        .expect("acceptance blockers were validated")
        .iter()
        .map(|value| value.as_str().expect("validated blocker"))
        .collect::<BTreeSet<_>>();
    let k0 = object["gate_statuses"]
        .as_array()
        .expect("acceptance gates were validated")
        .iter()
        .find(|gate| gate["gate_id"] == "K0_AUTH_REFERENCE")
        .and_then(|gate| gate["status"].as_str())
        .ok_or_else(|| invalid("acceptance K0_AUTH_REFERENCE gate is missing"))?;
    let runtime_bound = evidence == "runtime-bound";
    if (runtime_bound && (k0 != "pass" || blockers.contains("authorization-not-runtime-bound")))
        || (!runtime_bound
            && (k0 != "blocked" || !blockers.contains("authorization-not-runtime-bound")))
    {
        return Err(mismatch(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_ACCEPTANCE_AUTHORIZATION_STALE",
            "K0 and authorization blocker must reflect the current binding evidence",
        ));
    }
    for (state, blocker) in [
        (identity, "identity-label-conflict"),
        (hero, "hero-budget-conflict"),
        (texture, "texture-resolution-conflict"),
        (engine, "engine-profile-conflict"),
    ] {
        if blockers.contains(blocker) != (state != "resolved") {
            return Err(mismatch(
                "WEAPONRY_KNIFE_PRODUCTION_BRIEF_ACCEPTANCE_CONFLICT_STALE",
                format!("acceptance blocker {blocker} disagrees with frozen resolution state"),
            ));
        }
    }
    Ok(())
}

fn nullable_id(
    object: &Map<String, Value>,
    field: &str,
    context: &str,
) -> Result<Option<String>, RuntimeError> {
    match object.get(field) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if is_opaque_id(value) => Ok(Some(value.clone())),
        _ => Err(invalid(format!(
            "{context}.{field} must be null or an opaque identifier"
        ))),
    }
}

fn nullable_hash(
    object: &Map<String, Value>,
    field: &str,
    context: &str,
) -> Result<Option<String>, RuntimeError> {
    match object.get(field) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if is_sha256(value) => Ok(Some(value.clone())),
        _ => Err(invalid(format!(
            "{context}.{field} must be null or a lowercase SHA-256"
        ))),
    }
}

pub(crate) fn validate_brief(value: Value) -> Result<BriefValidation, RuntimeError> {
    validate_forbidden_values(&value)?;
    let object = exact_object(&value, BRIEF_FIELDS, "WeaponryKnifeProductionBrief@1")?;
    if text(object, "schema_version", "brief")? != BRIEF_SCHEMA {
        return Err(invalid("schema_version differs"));
    }
    let brief_id = opaque(object, "brief_id", "brief")?;
    let project_id = opaque(object, "project_id", "brief")?;
    let parent_brief_id = nullable_id(object, "parent_brief_id", "brief")?;
    let parent_brief_sha256 = nullable_hash(object, "parent_brief_sha256", "brief")?;
    let freeze_policy = enum_value(
        object,
        "freeze_policy",
        &[
            "initial-intake-no-parent@1",
            "immutable-successor-preserve-source-claims@1",
        ],
        "brief",
    )?;
    if matches!(freeze_policy.as_str(), "initial-intake-no-parent@1")
        && (parent_brief_id.is_some() || parent_brief_sha256.is_some())
    {
        return Err(mismatch(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_PARENT_POLICY_INVALID",
            "initial intake cannot bind a parent",
        ));
    }
    if matches!(
        freeze_policy.as_str(),
        "immutable-successor-preserve-source-claims@1"
    ) && (parent_brief_id.is_none() || parent_brief_sha256.is_none())
    {
        return Err(mismatch(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_PARENT_POLICY_INVALID",
            "successor intake must bind both parent fields",
        ));
    }
    if parent_brief_id.as_deref() == Some(brief_id.as_str()) {
        return Err(mismatch(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_PARENT_SELF_REFERENCE",
            "brief cannot parent itself",
        ));
    }
    enum_value(
        object,
        "subject",
        &["knife", "crossfire-knife", "original-control-knife"],
        "brief",
    )?;
    let mut claims = BTreeMap::new();
    let identity = validate_identity(
        object
            .get("asset_identity")
            .ok_or_else(|| invalid("asset_identity missing"))?,
        &mut claims,
    )?;
    let (source, evidence) = validate_authorization(
        object
            .get("authorization")
            .ok_or_else(|| invalid("authorization missing"))?,
    )?;
    let core = validate_coverage(
        object
            .get("reference_coverage")
            .ok_or_else(|| invalid("reference_coverage missing"))?,
        &source,
    )?;
    validate_silhouette(
        object
            .get("silhouette_priorities")
            .ok_or_else(|| invalid("silhouette_priorities missing"))?,
    )?;
    let part_ids = validate_parts_materials(&value)?;
    validate_presentation(
        object
            .get("presentation_constraints")
            .ok_or_else(|| invalid("presentation_constraints missing"))?,
        &part_ids,
    )?;
    let (hero, texture) = validate_surface(
        object
            .get("surface_constraints")
            .ok_or_else(|| invalid("surface_constraints missing"))?,
        &mut claims,
    )?;
    let (engine, unit_ok, axis_ok) = validate_engine(
        object
            .get("engine_constraints")
            .ok_or_else(|| invalid("engine_constraints missing"))?,
        &mut claims,
    )?;
    let conflicts = validate_conflicts(&value, &claims, &identity, &hero, &texture, &engine)?;
    validate_acceptance(
        object
            .get("acceptance_constraints")
            .ok_or_else(|| invalid("acceptance_constraints missing"))?,
    )?;
    validate_acceptance_binding(
        object
            .get("acceptance_constraints")
            .expect("acceptance constraints were validated"),
        &evidence,
        &identity,
        &hero,
        &texture,
        &engine,
    )?;
    if text(object, "canonicalization_policy", "brief")? != BRIEF_CANONICALIZATION {
        return Err(invalid("brief canonicalization policy differs"));
    }
    let supplied = hash(object, "canonical_sha256", "brief")?;
    let mut preimage = value.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != supplied {
        return Err(mismatch(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_CANONICAL_MISMATCH",
            "canonical_sha256 differs from Runtime recomputation",
        ));
    }
    let created_at = text(object, "created_at", "brief")?;
    if !valid_timestamp(created_at) {
        return Err(invalid("brief.created_at must be a UTC timestamp"));
    }
    let mut blocking = Vec::new();
    if evidence != "runtime-bound" {
        blocking.push("AUTHORIZATION_NOT_RUNTIME_BOUND".to_owned());
    }
    if conflicts {
        blocking.push("SOURCE_CONFLICT_UNRESOLVED".to_owned());
    }
    if !core {
        blocking.push("REFERENCE_CORE_COVERAGE_INCOMPLETE".to_owned());
    }
    if hero != "resolved" {
        blocking.push("HERO_BUDGET_UNRESOLVED".to_owned());
    }
    if texture != "resolved" {
        blocking.push("TEXTURE_RESOLUTION_UNRESOLVED".to_owned());
    }
    if engine != "resolved" || !unit_ok || !axis_ok {
        blocking.push("ENGINE_PROFILE_UNRESOLVED".to_owned());
    }
    let runtime_eligible = evidence == "runtime-bound"
        && !conflicts
        && core
        && hero == "resolved"
        && texture == "resolved"
        && engine == "resolved"
        && unit_ok
        && axis_ok;
    if runtime_eligible {
        blocking.clear();
    }
    Ok(BriefValidation {
        brief: value,
        brief_id,
        project_id,
        brief_sha256: supplied,
        source_reference_sha256: source,
        conflict_status: if conflicts { "conflicted" } else { "resolved" },
        authorization_binding_status: if evidence == "runtime-bound" {
            "runtime-bound"
        } else {
            "source-asserted-not-runtime-bound"
        },
        authoring_eligibility: if runtime_eligible {
            "ELIGIBLE"
        } else {
            "BLOCKED"
        },
        blocking_reasons: blocking,
        payload_claims_runtime_bound: evidence == "runtime-bound",
        core_coverage: core,
        parent_brief_id,
        parent_brief_sha256,
        freeze_policy,
    })
}

fn apply_reference_binding(validation: &mut BriefValidation, reference_verified: bool) {
    if !(reference_verified && validation.payload_claims_runtime_bound) {
        validation.authorization_binding_status = "source-asserted-not-runtime-bound";
        if !validation
            .blocking_reasons
            .iter()
            .any(|reason| reason == "AUTHORIZATION_NOT_RUNTIME_BOUND")
        {
            validation
                .blocking_reasons
                .push("AUTHORIZATION_NOT_RUNTIME_BOUND".to_owned());
        }
        validation.authoring_eligibility = "BLOCKED";
    } else if validation.blocking_reasons.is_empty() {
        validation.authorization_binding_status = "runtime-bound";
        validation.authoring_eligibility = "ELIGIBLE";
    }
}

fn source_claim_values(brief: &Value) -> BTreeMap<String, Value> {
    let mut claims = BTreeMap::new();
    let root = brief.as_object().expect("validated brief");
    let identity = root["asset_identity"]
        .as_object()
        .expect("validated identity");
    for claim in identity["identity_claims"]
        .as_array()
        .expect("validated identity claims")
    {
        let object = claim.as_object().expect("validated claim");
        claims.insert(
            object["claim_id"].as_str().expect("claim id").to_owned(),
            claim.clone(),
        );
    }
    let surface = root["surface_constraints"]
        .as_object()
        .expect("validated surface");
    let hero = surface["hero_budget"].as_object().expect("validated hero");
    for claim in hero["claims"].as_array().expect("validated hero claims") {
        let object = claim.as_object().expect("validated claim");
        claims.insert(
            object["claim_id"].as_str().expect("claim id").to_owned(),
            claim.clone(),
        );
    }
    let texture = surface["texture_policy"]
        .as_object()
        .expect("validated texture");
    for claim in texture["resolution_claims"]
        .as_array()
        .expect("validated texture claims")
    {
        let object = claim.as_object().expect("validated claim");
        claims.insert(
            object["claim_id"].as_str().expect("claim id").to_owned(),
            claim.clone(),
        );
    }
    let engine = root["engine_constraints"]
        .as_object()
        .expect("validated engine");
    for claim in engine["target_claims"]
        .as_array()
        .expect("validated engine claims")
    {
        let object = claim.as_object().expect("validated claim");
        claims.insert(
            object["claim_id"].as_str().expect("claim id").to_owned(),
            claim.clone(),
        );
    }
    claims
}

fn source_conflict_structure(brief: &Value) -> BTreeMap<String, Value> {
    let mut conflicts = BTreeMap::new();
    for item in brief["source_conflicts"]
        .as_array()
        .expect("validated conflicts")
    {
        let object = item.as_object().expect("validated conflict");
        let id = object["conflict_id"]
            .as_str()
            .expect("conflict id")
            .to_owned();
        conflicts.insert(
            id,
            serde_json::json!({
                "kind": object["kind"],
                "observed_claim_ids": object["observed_claim_ids"],
            }),
        );
    }
    conflicts
}

/// Return the part of a Brief that an immutable successor is not permitted to
/// change.  The source claim objects stay in this projection byte-for-byte;
/// only explicit selection/resolution fields, source authorization/coverage,
/// and the conflict ledger are omitted as the allowed advancing surface.
fn successor_immutable_projection(brief: &Value) -> Value {
    let mut value = brief.clone();
    let root = value.as_object_mut().expect("validated brief");
    for field in [
        "brief_id",
        "parent_brief_id",
        "parent_brief_sha256",
        "freeze_policy",
        "authorization",
        "reference_coverage",
        "source_conflicts",
        "canonical_sha256",
        "created_at",
    ] {
        root.insert(field.to_owned(), Value::Null);
    }
    let identity = root["asset_identity"]
        .as_object_mut()
        .expect("validated identity");
    identity.insert("name_status".to_owned(), Value::Null);
    identity.insert("selected_label".to_owned(), Value::Null);
    let surface = root["surface_constraints"]
        .as_object_mut()
        .expect("validated surface");
    let hero = surface["hero_budget"]
        .as_object_mut()
        .expect("validated hero");
    for field in [
        "status",
        "resolved_min_triangles",
        "resolved_max_triangles",
        "blocks",
    ] {
        hero.insert(field.to_owned(), Value::Null);
    }
    let texture = surface["texture_policy"]
        .as_object_mut()
        .expect("validated texture");
    for field in [
        "resolution_status",
        "resolved_width",
        "resolved_height",
        "shipping_width",
        "shipping_height",
    ] {
        texture.insert(field.to_owned(), Value::Null);
    }
    let engine = root["engine_constraints"]
        .as_object_mut()
        .expect("validated engine");
    for field in [
        "profile_status",
        "preferred_engine",
        "preferred_engine_version",
        "unit_status",
        "selected_unit",
        "axis_status",
        "selected_axis_profile",
    ] {
        engine.insert(field.to_owned(), Value::Null);
    }
    let acceptance = root["acceptance_constraints"]
        .as_object_mut()
        .expect("validated acceptance constraints");
    for field in ["status", "gate_statuses", "blocking_reasons"] {
        acceptance.insert(field.to_owned(), Value::Null);
    }
    value
}

fn validate_successor(runtime: &Runtime, validation: &BriefValidation) -> Result<(), RuntimeError> {
    if validation.freeze_policy == "initial-intake-no-parent@1" {
        return Ok(());
    }
    let parent_id = validation
        .parent_brief_id
        .as_deref()
        .ok_or_else(|| invalid("successor parent id is missing"))?;
    let parent_sha = validation
        .parent_brief_sha256
        .as_deref()
        .ok_or_else(|| invalid("successor parent hash is missing"))?;
    let parent_record = runtime
        .store
        .get_weaponry_knife_production_brief(&validation.project_id, parent_id, parent_sha)?
        .ok_or_else(|| RuntimeError::InvalidInput("WEAPONRY_KNIFE_PRODUCTION_BRIEF_PARENT_NOT_FOUND: exact parent brief is not durable".to_owned()))?;
    let parent = runtime.read_brief_object(&parent_record.brief_object_sha256)?;
    let parent_validation = validate_brief(parent.clone())?;
    if parent_validation.project_id != validation.project_id
        || parent_validation.brief_id != parent_id
        || parent_validation.brief_sha256 != parent_sha
    {
        return Err(mismatch(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_PARENT_BINDING_MISMATCH",
            "parent identity or canonical hash differs from the exact Store record",
        ));
    }
    if validation.brief_id == parent_validation.brief_id
        || validation.brief_sha256 == parent_validation.brief_sha256
    {
        return Err(mismatch(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_PARENT_SUCCESSOR_NOT_NEW",
            "successor must use a new brief id and canonical hash",
        ));
    }
    if source_claim_values(&parent) != source_claim_values(&validation.brief) {
        return Err(mismatch(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_PARENT_SOURCE_CLAIM_DROPPED",
            "successor did not preserve the exact parent source claim set, values and evidence hashes",
        ));
    }
    if source_conflict_structure(&parent) != source_conflict_structure(&validation.brief) {
        return Err(mismatch(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_PARENT_CONFLICT_LEDGER_CHANGED",
            "successor changed the historical conflict identity or observed claims",
        ));
    }
    if successor_immutable_projection(&parent) != successor_immutable_projection(&validation.brief)
    {
        return Err(mismatch("WEAPONRY_KNIFE_PRODUCTION_BRIEF_PARENT_IMMUTABLE_FIELD_CHANGED", "successor changed a field outside resolution, authorization, coverage or conflict freeze"));
    }
    Ok(())
}

fn reference_canonical_hash(reference: &ReferenceEvidenceRecord) -> Result<String, RuntimeError> {
    let auth = serde_json::to_value(&reference.authorization)
        .map_err(|error| invalid(error.to_string()))?;
    Ok(canonical_json_hash(&serde_json::json!({
        "schema_version": "ReferenceEvidence@1", "reference_id": reference.reference_id,
        "project_id": reference.project_id, "object_sha256": reference.object_sha256,
        "mime": reference.mime, "size_bytes": reference.size_bytes, "width": reference.width,
        "height": reference.height, "frame_count": reference.frame_count,
        "import_mode": reference.import_mode, "authorization": auth,
        "derived_object_sha256": reference.derived_object_sha256, "created_at": reference.created_at,
    })))
}

fn verify_reference(
    runtime: &Runtime,
    project_id: &str,
    brief: &BriefValidation,
    object: &Map<String, Value>,
) -> Result<bool, RuntimeError> {
    let reference_id = opaque(object, "reference_id", "prepare")?;
    let reference_object = hash(object, "reference_object_sha256", "prepare")?;
    let reference_evidence = hash(object, "reference_evidence_sha256", "prepare")?;
    let reference = runtime.reference(&reference_id)?.ok_or_else(|| {
        RuntimeError::InvalidInput(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_REFERENCE_NOT_FOUND: reference is not registered"
                .to_owned(),
        )
    })?;
    if reference.project_id != project_id {
        return Err(mismatch(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_REFERENCE_PROJECT_MISMATCH",
            "reference belongs to another project",
        ));
    }
    if !matches!(reference.mime.as_str(), "image/png" | "image/jpeg") {
        return Err(invalid(
            "Runtime reference MIME is outside the image allowlist",
        ));
    }
    safe_text(
        Some(&Value::String(reference.authorization.declaration.clone())),
        "reference.authorization.declaration",
    )?;
    if reference.object_sha256 != reference_object
        || reference.object_sha256 != brief.source_reference_sha256
    {
        return Err(mismatch(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_REFERENCE_HASH_MISMATCH",
            "reference object hash differs from Brief source hash",
        ));
    }
    if reference.canonical_sha256 != reference_evidence
        || reference_canonical_hash(&reference)? != reference.canonical_sha256
    {
        return Err(mismatch(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_REFERENCE_EVIDENCE_MISMATCH",
            "reference evidence canonical hash is invalid",
        ));
    }
    let coverage = brief
        .brief
        .get("reference_coverage")
        .and_then(Value::as_object)
        .expect("validated coverage");
    let dimensions = coverage["source_dimensions"]
        .as_object()
        .expect("validated dimensions");
    if dimensions["width"].as_u64() != Some(reference.width as u64)
        || dimensions["height"].as_u64() != Some(reference.height as u64)
    {
        return Err(mismatch(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_REFERENCE_DIMENSIONS_MISMATCH",
            "Brief dimensions differ from Runtime reference",
        ));
    }
    if reference.size_bytes == 0 || reference.size_bytes > MAX_REFERENCE_BYTES {
        return Err(invalid("reference bytes exceed bounded intake capacity"));
    }
    let bytes = runtime.cas_read_bounded(&reference.object_sha256, MAX_REFERENCE_BYTES)?;
    if bytes.len() as u64 != reference.size_bytes || sha256_hex(&bytes) != reference.object_sha256 {
        return Err(mismatch(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_REFERENCE_CAS_MISMATCH",
            "reference CAS bytes do not match Runtime evidence",
        ));
    }
    Ok(reference.authorization.user_authorized)
}

fn verify_reference_for_get(
    runtime: &Runtime,
    project_id: &str,
    reference_id: &str,
    source: &str,
    evidence: &str,
) -> Result<bool, RuntimeError> {
    let Some(reference) = runtime.reference(reference_id)? else {
        return Ok(false);
    };
    if reference.project_id != project_id
        || reference.object_sha256 != source
        || reference.canonical_sha256 != evidence
    {
        return Ok(false);
    }
    if !matches!(reference.mime.as_str(), "image/png" | "image/jpeg")
        || safe_text(
            Some(&Value::String(reference.authorization.declaration.clone())),
            "reference.authorization.declaration",
        )
        .is_err()
    {
        return Ok(false);
    }
    if !reference.authorization.user_authorized
        || reference.size_bytes == 0
        || reference.size_bytes > MAX_REFERENCE_BYTES
        || reference_canonical_hash(&reference)? != evidence
    {
        return Ok(false);
    }
    let bytes = runtime.cas_read_bounded(source, MAX_REFERENCE_BYTES)?;
    Ok(bytes.len() as u64 == reference.size_bytes && sha256_hex(&bytes) == source)
}

fn result_value(
    record: &WeaponryKnifeProductionBriefStoreRecord,
    validation: &BriefValidation,
    request_kind: &str,
    replayed: bool,
    store_effect: &str,
    cas_effect: &str,
) -> Result<Value, RuntimeError> {
    let mut result = serde_json::json!({
        "schema_version": RESULT_SCHEMA, "request_kind": request_kind,
        "status": if request_kind == "get" { "found" } else if replayed { "replayed" } else { "stored" },
        "project_id": record.project_id, "reference_id": record.reference_id,
        "reference_object_sha256": record.reference_object_sha256,
        "reference_evidence_sha256": record.reference_evidence_sha256,
        "parent_brief_id": record.parent_brief_id,
        "parent_brief_sha256": record.parent_brief_sha256,
        "freeze_policy": record.freeze_policy,
        "brief_id": record.brief_id, "brief": validation.brief,
        "brief_sha256": record.brief_canonical_sha256, "brief_object_sha256": record.brief_object_sha256,
        "idempotency_key": if request_kind == "get" { Value::Null } else { Value::String(record.idempotency_key.clone()) },
        "replayed": replayed, "authorization_binding_status": validation.authorization_binding_status,
        "conflict_status": validation.conflict_status, "authoring_eligibility": validation.authoring_eligibility,
        "blocking_reasons": validation.blocking_reasons, "store_effect": store_effect, "cas_effect": cas_effect,
        "runtime_write_performed": request_kind == "prepare" && !replayed,
        "persistent_user_data_touched": request_kind == "prepare" && !replayed,
        "production_stage_advanced": false, "candidate_confirmed": false, "version_created": false,
        "export_performed": false, "writer_policy": WRITER_POLICY,
        "canonicalization_policy": BRIEF_CANONICALIZATION, "canonical_sha256": "",
    });
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    if canonical_json_bytes(&result)
        .map_err(|error| invalid(error.to_string()))?
        .len() as u64
        > MAX_RESPONSE_BYTES
    {
        return Err(invalid("result exceeds max_response_bytes"));
    }
    Ok(result)
}

pub(crate) fn prepare(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(request, PREPARE_FIELDS, PREPARE_SCHEMA)?;
    validate_request_header(object, PREPARE_SCHEMA, PREPARE_OPERATION, "prepare")?;
    validate_request_hash(request, object)?;
    let project_id = opaque(object, "project_id", "prepare")?;
    let idempotency_key = opaque(object, "idempotency_key", "prepare")?;
    let validation = validate_brief(
        object
            .get("brief")
            .cloned()
            .ok_or_else(|| invalid("brief missing"))?,
    )?;
    if validation.project_id != project_id {
        return Err(mismatch(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_PROJECT_SCOPE_MISMATCH",
            "request project differs from Brief project",
        ));
    }
    if runtime.project(&project_id)?.is_none() {
        return Err(RuntimeError::InvalidInput(
            "WEAPONRY_KNIFE_PRODUCTION_BRIEF_PROJECT_NOT_FOUND: project does not exist".to_owned(),
        ));
    }
    validate_successor(runtime, &validation)?;
    let mut validation = validation;
    let reference_verified = verify_reference(runtime, &project_id, &validation, object)?;
    apply_reference_binding(&mut validation, reference_verified);
    let bytes =
        canonical_json_bytes(&validation.brief).map_err(|error| invalid(error.to_string()))?;
    if bytes.len() as u64 > WEAPONRY_KNIFE_PRODUCTION_BRIEF_MAX_JSON_BYTES {
        return Err(invalid("brief exceeds bounded CAS capacity"));
    }
    let reservation = runtime.store.begin_cas_reservation();
    let cas = match runtime.store.put_object_reserved(
        &reservation,
        &bytes,
        None,
        WEAPONRY_KNIFE_PRODUCTION_BRIEF_JSON_MIME,
        WEAPONRY_KNIFE_PRODUCTION_BRIEF_OBJECT_KIND,
        &super::now_string(),
    ) {
        Ok(cas) => cas,
        Err(error) => return Err(error.into()),
    };
    let source_refs = vec![validation.source_reference_sha256.clone()];
    let record = WeaponryKnifeProductionBriefStoreRecord {
        schema_version: WEAPONRY_KNIFE_PRODUCTION_BRIEF_RECORD_SCHEMA_VERSION.to_owned(),
        project_id: project_id.clone(),
        brief_id: validation.brief_id.clone(),
        brief_object_sha256: cas.record.sha256.clone(),
        brief_canonical_sha256: validation.brief_sha256.clone(),
        reference_id: opaque(object, "reference_id", "prepare")?,
        reference_object_sha256: hash(object, "reference_object_sha256", "prepare")?,
        reference_evidence_sha256: hash(object, "reference_evidence_sha256", "prepare")?,
        parent_brief_id: validation.parent_brief_id.clone(),
        parent_brief_sha256: validation.parent_brief_sha256.clone(),
        freeze_policy: validation.freeze_policy.clone(),
        source_reference_hashes: source_refs,
        status: if validation.authoring_eligibility == "ELIGIBLE" {
            // Store's intent-bundle lineage gate consumes this exact
            // normalized status.  The public result still exposes the richer
            // `authoring_eligibility` value; the durable index uses the
            // stable enum expected by every downstream repository.
            "eligible"
        } else {
            "blocked"
        }
        .to_owned(),
        conflict_freeze_state: if validation.conflict_status == "conflicted" {
            "frozen"
        } else {
            "resolved"
        }
        .to_owned(),
        idempotency_key,
        created_at: super::now_string(),
    };
    let commit = WeaponryKnifeProductionBriefCommit {
        record,
        cas: WeaponryKnifeProductionBriefCasBundle {
            brief: cas.record.clone(),
        },
    };
    let commit_result = runtime
        .store
        .record_weaponry_knife_production_brief_with_replay(&commit);
    let (stored, replayed) = match commit_result {
        Ok(value) => {
            let _ = runtime
                .store
                .release_cas_reservation_object(&reservation, &cas, false);
            value
        }
        Err(error) => {
            let _ = runtime
                .store
                .release_cas_reservation_object(&reservation, &cas, true);
            return Err(error.into());
        }
    };
    let stored_brief = runtime.read_brief_object(&stored.brief_object_sha256)?;
    let mut stored_validation = validate_brief(stored_brief)?;
    apply_reference_binding(&mut stored_validation, reference_verified);
    result_value(
        &stored,
        &stored_validation,
        "prepare",
        replayed,
        if replayed { "not-touched" } else { "inserted" },
        if replayed { "not-touched" } else { "inserted" },
    )
}

pub(crate) fn get(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(request, GET_FIELDS, GET_SCHEMA)?;
    validate_request_header(object, GET_SCHEMA, GET_OPERATION, "get")?;
    validate_request_hash(request, object)?;
    let project_id = opaque(object, "project_id", "get")?;
    if object
        .get("persistent_user_data_touched")
        .and_then(Value::as_bool)
        != Some(false)
    {
        return Err(invalid("get.persistent_user_data_touched must be false"));
    }
    let reference_id = opaque(object, "reference_id", "get")?;
    let reference_object_sha256 = hash(object, "reference_object_sha256", "get")?;
    let reference_evidence_sha256 = hash(object, "reference_evidence_sha256", "get")?;
    let brief_id = opaque(object, "brief_id", "get")?;
    let brief_sha256 = hash(object, "brief_sha256", "get")?;
    let brief_object_sha256 = hash(object, "brief_object_sha256", "get")?;
    let record = runtime
        .store
        .get_weaponry_knife_production_brief_exact(
            &project_id,
            &reference_id,
            &reference_object_sha256,
            &reference_evidence_sha256,
            &brief_id,
            &brief_sha256,
            &brief_object_sha256,
        )?
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "WEAPONRY_KNIFE_PRODUCTION_BRIEF_NOT_FOUND: no exact Brief binding".to_owned(),
            )
        })?;
    let brief = runtime.read_brief_object(&record.brief_object_sha256)?;
    let mut validation = validate_brief(brief)?;
    validate_successor(runtime, &validation)?;
    let reference_verified = verify_reference_for_get(
        runtime,
        &project_id,
        &record.reference_id,
        &record.reference_object_sha256,
        &record.reference_evidence_sha256,
    )?;
    apply_reference_binding(&mut validation, reference_verified);
    result_value(
        &record,
        &validation,
        "get",
        false,
        "not-touched",
        "not-touched",
    )
}

impl Runtime {
    pub fn weaponry_knife_production_brief_prepare(
        &self,
        request: &Value,
    ) -> Result<Value, RuntimeError> {
        prepare(self, request)
    }
    pub fn weaponry_knife_production_brief_get(
        &self,
        request: &Value,
    ) -> Result<Value, RuntimeError> {
        get(self, request)
    }

    fn read_brief_object(&self, sha256: &str) -> Result<Value, RuntimeError> {
        let bytes =
            self.cas_read_bounded(sha256, WEAPONRY_KNIFE_PRODUCTION_BRIEF_MAX_JSON_BYTES)?;
        serde_json::from_slice(&bytes)
            .map_err(|error| invalid(format!("brief CAS JSON is invalid: {error}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgecad_contracts::{
        ReferenceAuthorization, ReferenceImportRequest, ReferenceImportSource,
    };
    use serde_json::json;
    use std::path::PathBuf;

    fn fixture(path: &str) -> Value {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../../../packages/forgecad-contracts/fixtures/weaponry-knife-production-brief/positive/");
        let text = std::fs::read_to_string(format!("{root}{path}")).expect("fixture file");
        serde_json::from_str(&text).expect("fixture JSON")
    }

    fn import_test_reference(runtime: &Runtime, project_id: &str) -> ReferenceEvidenceRecord {
        runtime
            .import_reference(&ReferenceImportRequest {
                project_id: project_id.to_owned(),
                source: ReferenceImportSource::InlineContent {
                    mime: "image/png".to_owned(),
                    content_base64: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=".to_owned(),
                },
                authorization: ReferenceAuthorization {
                    user_authorized: true,
                    declaration: "Runtime brief integration test reference".to_owned(),
                },
                expected_sha256: None,
            })
            .expect("reference import")
            .reference
    }

    fn runtime_brief(
        fixture_name: &str,
        project_id: &str,
        reference: &ReferenceEvidenceRecord,
    ) -> Value {
        let mut brief = fixture(fixture_name);
        brief["project_id"] = Value::String(project_id.to_owned());
        brief["authorization"]["source_reference_sha256"] =
            Value::String(reference.object_sha256.clone());
        brief["reference_coverage"]["source_reference_sha256"] =
            Value::String(reference.object_sha256.clone());
        brief["reference_coverage"]["source_dimensions"] =
            json!({"width": reference.width, "height": reference.height});
        brief["canonical_sha256"] = Value::String(String::new());
        brief["canonical_sha256"] = Value::String(canonical_json_hash(&brief));
        brief
    }

    fn prepare_request(
        brief: Value,
        reference: &ReferenceEvidenceRecord,
        idempotency_key: &str,
    ) -> Value {
        let mut request = json!({
            "schema_version": PREPARE_SCHEMA,
            "operation": PREPARE_OPERATION,
            "project_id": brief["project_id"].clone(),
            "brief": brief,
            "reference_id": reference.reference_id,
            "reference_object_sha256": reference.object_sha256,
            "reference_evidence_sha256": reference.canonical_sha256,
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
            "reference_id": result["reference_id"],
            "reference_object_sha256": result["reference_object_sha256"],
            "reference_evidence_sha256": result["reference_evidence_sha256"],
            "brief_id": result["brief_id"],
            "brief_sha256": result["brief_sha256"],
            "brief_object_sha256": result["brief_object_sha256"],
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

    #[test]
    fn dragonfang_conflict_is_valid_but_blocked() {
        let validation =
            validate_brief(fixture("dragonfang-kukri-brief.json")).expect("brief contract");
        assert_eq!(validation.conflict_status, "conflicted");
        assert_eq!(validation.authoring_eligibility, "BLOCKED");
        assert!(validation
            .blocking_reasons
            .iter()
            .any(|reason| reason == "SOURCE_CONFLICT_UNRESOLVED"));
        assert_eq!(validation.freeze_policy, "initial-intake-no-parent@1");
    }

    #[test]
    fn generic_resolved_brief_requires_runtime_reference_before_eligibility() {
        let validation = validate_brief(fixture("generic-resolved-original-control.json"))
            .expect("brief contract");
        assert_eq!(validation.conflict_status, "resolved");
        assert_eq!(validation.authoring_eligibility, "ELIGIBLE");
        let mut validation = validation;
        apply_reference_binding(&mut validation, false);
        assert_eq!(validation.authoring_eligibility, "BLOCKED");
        assert!(validation
            .blocking_reasons
            .iter()
            .any(|reason| reason == "AUTHORIZATION_NOT_RUNTIME_BOUND"));
    }

    #[test]
    fn canonical_mutation_and_unknown_nested_field_fail_closed() {
        let mut value = fixture("generic-resolved-original-control.json");
        value["presentation_constraints"]["unexpected"] = Value::String("nope".to_owned());
        assert!(validate_brief(value).is_err());
        let mut value = fixture("generic-resolved-original-control.json");
        value["subject"] = Value::String("knife".to_owned());
        assert!(validate_brief(value).is_err());
    }

    #[test]
    fn runtime_prepare_replay_get_and_reopen_preserve_blocked_or_eligible_truth() {
        let root =
            std::env::temp_dir().join(format!("forgecad-brief-runtime-{}", uuid::Uuid::new_v4()));
        let database = root.join("runtime.sqlite");
        let cas_root = root.join("runtime.cas");
        let (project_id, get_payload) = {
            let runtime = Runtime::open_with_cas(&database, &cas_root).expect("open runtime");
            let project = runtime
                .create_project("brief runtime integration", json!({"profile":"knife"}))
                .expect("project");
            let reference = import_test_reference(&runtime, &project.project_id);
            let brief = runtime_brief(
                "generic-resolved-original-control.json",
                &project.project_id,
                &reference,
            );
            let request = prepare_request(brief, &reference, "brief-runtime-replay");
            let first = runtime
                .weaponry_knife_production_brief_prepare(&request)
                .expect("first prepare");
            assert_eq!(first["status"], "stored");
            assert_eq!(first["replayed"], false);
            assert_eq!(first["store_effect"], "inserted");
            assert_eq!(first["cas_effect"], "inserted");
            assert_eq!(first["authoring_eligibility"], "ELIGIBLE");
            assert_eq!(first["authorization_binding_status"], "runtime-bound");
            assert_eq!(first["production_stage_advanced"], false);

            let replay = runtime
                .weaponry_knife_production_brief_prepare(&request)
                .expect("idempotent replay");
            assert_eq!(replay["status"], "replayed");
            assert_eq!(replay["replayed"], true);
            assert_eq!(replay["store_effect"], "not-touched");
            assert_eq!(replay["cas_effect"], "not-touched");
            assert_eq!(replay["runtime_write_performed"], false);
            assert_eq!(replay["persistent_user_data_touched"], false);

            let get = get_request(&first);
            let found = runtime
                .weaponry_knife_production_brief_get(&get)
                .expect("get readback");
            assert_eq!(found["status"], "found");
            assert_eq!(found["store_effect"], "not-touched");
            assert_eq!(found["cas_effect"], "not-touched");
            assert_eq!(found["runtime_write_performed"], false);
            assert_eq!(found["persistent_user_data_touched"], false);
            assert_eq!(found["reference_id"], first["reference_id"]);
            assert_eq!(found["brief_object_sha256"], first["brief_object_sha256"]);
            (project.project_id, get)
        };

        let reopened = Runtime::open_with_cas(&database, &cas_root).expect("reopen runtime");
        let reopened_result = reopened
            .weaponry_knife_production_brief_get(&get_payload)
            .expect("reopened get");
        assert_eq!(reopened_result["status"], "found");
        assert_eq!(reopened_result["project_id"], project_id);
        assert_eq!(reopened_result["authoring_eligibility"], "ELIGIBLE");
        drop(reopened);
        let _ = std::fs::remove_dir_all(PathBuf::from(database).parent().expect("test root"));
    }

    #[test]
    fn runtime_persists_dragonfang_conflict_as_blocked_without_stage_progression() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("dragonfang brief integration", json!({"profile":"knife"}))
            .expect("project");
        let reference = import_test_reference(&runtime, &project.project_id);
        let brief = runtime_brief(
            "dragonfang-kukri-brief.json",
            &project.project_id,
            &reference,
        );
        let request = prepare_request(brief, &reference, "dragonfang-blocked");
        let result = runtime
            .weaponry_knife_production_brief_prepare(&request)
            .expect("conflicted intake is durable");
        assert_eq!(result["status"], "stored");
        assert_eq!(result["conflict_status"], "conflicted");
        assert_eq!(result["authoring_eligibility"], "BLOCKED");
        assert_eq!(
            result["authorization_binding_status"],
            "source-asserted-not-runtime-bound"
        );
        assert_eq!(result["production_stage_advanced"], false);
        assert_eq!(result["candidate_confirmed"], false);
        assert_eq!(result["version_created"], false);
        assert_eq!(result["export_performed"], false);

        let record = runtime
            .store
            .get_weaponry_knife_production_brief(
                &project.project_id,
                result["brief_id"].as_str().expect("brief id"),
                result["brief_sha256"].as_str().expect("brief hash"),
            )
            .expect("blocked record lookup")
            .expect("blocked record");
        assert_eq!(record.status, "blocked");
        assert_eq!(record.conflict_freeze_state, "frozen");
    }

    #[test]
    fn runtime_accepts_only_an_immutable_resolved_successor_for_authoring() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project(
                "dragonfang successor integration",
                json!({"profile":"knife"}),
            )
            .expect("project");
        let reference = import_test_reference(&runtime, &project.project_id);
        let parent = runtime_brief(
            "dragonfang-kukri-brief.json",
            &project.project_id,
            &reference,
        );
        let parent_request = prepare_request(parent.clone(), &reference, "dragonfang-parent");
        let parent_result = runtime
            .weaponry_knife_production_brief_prepare(&parent_request)
            .expect("parent prepare");

        let mut successor = runtime_brief(
            "dragonfang-kukri-brief-resolved-001.json",
            &project.project_id,
            &reference,
        );
        successor["parent_brief_id"] = parent_result["brief_id"].clone();
        successor["parent_brief_sha256"] = parent_result["brief_sha256"].clone();
        successor["authorization"]["evidence_status"] = Value::String("runtime-bound".to_owned());
        successor["acceptance_constraints"]["gate_statuses"][0]["status"] =
            Value::String("pass".to_owned());
        successor["acceptance_constraints"]["blocking_reasons"] = json!([
            "missing-reference-views",
            "engine-validation-not-run",
            "independent-human-review-missing"
        ]);
        successor["canonical_sha256"] = Value::String(String::new());
        successor["canonical_sha256"] = Value::String(canonical_json_hash(&successor));

        let mut stale_acceptance = successor.clone();
        stale_acceptance["brief_id"] = Value::String("dragonfang-stale-acceptance".to_owned());
        stale_acceptance["acceptance_constraints"]["blocking_reasons"]
            .as_array_mut()
            .expect("acceptance blockers")
            .push(Value::String("identity-label-conflict".to_owned()));
        stale_acceptance["canonical_sha256"] = Value::String(String::new());
        stale_acceptance["canonical_sha256"] =
            Value::String(canonical_json_hash(&stale_acceptance));
        let stale_request =
            prepare_request(stale_acceptance, &reference, "dragonfang-stale-acceptance");
        let stale_error = runtime
            .weaponry_knife_production_brief_prepare(&stale_request)
            .expect_err("stale acceptance blocker must fail before persistence");
        assert!(stale_error
            .to_string()
            .contains("WEAPONRY_KNIFE_PRODUCTION_BRIEF_ACCEPTANCE_CONFLICT_STALE"));
        assert_eq!(
            runtime
                .store
                .get_weaponry_knife_production_brief(
                    &project.project_id,
                    "dragonfang-stale-acceptance",
                    stale_request["brief"]["canonical_sha256"]
                        .as_str()
                        .expect("stale hash"),
                )
                .expect("stale lookup"),
            None
        );

        let request = prepare_request(successor, &reference, "dragonfang-successor");
        let result = runtime
            .weaponry_knife_production_brief_prepare(&request)
            .expect("immutable successor prepare");
        assert_eq!(result["status"], "stored");
        assert_eq!(result["conflict_status"], "resolved");
        assert_eq!(result["authorization_binding_status"], "runtime-bound");
        assert_eq!(result["authoring_eligibility"], "ELIGIBLE");
        assert_eq!(
            result["brief"]["surface_constraints"]["hero_budget"]["resolved_min_triangles"],
            25_000
        );
        assert_eq!(
            result["brief"]["surface_constraints"]["hero_budget"]["resolved_max_triangles"],
            45_000
        );
        assert_eq!(
            result["brief"]["surface_constraints"]["texture_policy"]["resolved_width"],
            4096
        );
        assert_eq!(
            result["brief"]["surface_constraints"]["texture_policy"]["shipping_width"],
            2048
        );
        assert_eq!(
            result["brief"]["engine_constraints"]["preferred_engine_version"],
            "5.6-or-later"
        );
        assert_eq!(
            result["freeze_policy"],
            "immutable-successor-preserve-source-claims@1"
        );
        assert_eq!(result["production_stage_advanced"], false);
    }

    #[test]
    fn successor_claim_and_conflict_ledger_mutations_fail_before_any_child_write() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("brief successor integration", json!({"profile":"knife"}))
            .expect("project");
        let reference = import_test_reference(&runtime, &project.project_id);
        let parent_brief = runtime_brief(
            "generic-resolved-original-control.json",
            &project.project_id,
            &reference,
        );
        let parent_request = prepare_request(parent_brief.clone(), &reference, "brief-parent");
        let parent_result = runtime
            .weaponry_knife_production_brief_prepare(&parent_request)
            .expect("parent prepare");
        let parent_id = parent_result["brief_id"].as_str().expect("parent id");
        let parent_sha = parent_result["brief_sha256"].as_str().expect("parent hash");
        let parent_record = runtime
            .store
            .get_weaponry_knife_production_brief(&project.project_id, parent_id, parent_sha)
            .expect("parent lookup")
            .expect("parent record");
        let parent_roots =
            forgecad_store::Store::weaponry_knife_production_brief_cas_roots(&parent_record);

        let mut cases = Vec::new();
        let mut dropped_claim = parent_brief.clone();
        dropped_claim["brief_id"] = Value::String("successor-dropped-claim".to_owned());
        dropped_claim["surface_constraints"]["hero_budget"]["claims"][0]["claim_id"] =
            Value::String("control-hero-replaced".to_owned());
        dropped_claim["source_conflicts"][0]["observed_claim_ids"][1] =
            Value::String("control-hero-replaced".to_owned());
        cases.push(("dropped-claim", dropped_claim));

        let mut changed_claim = parent_brief.clone();
        changed_claim["brief_id"] = Value::String("successor-changed-claim".to_owned());
        changed_claim["asset_identity"]["source_labels"] = json!(["control-knife-rewritten"]);
        changed_claim["asset_identity"]["identity_claims"][0]["label"] =
            Value::String("control-knife-rewritten".to_owned());
        changed_claim["asset_identity"]["selected_label"] =
            Value::String("control-knife-rewritten".to_owned());
        cases.push(("changed-claim", changed_claim));

        let mut added_claim = parent_brief.clone();
        added_claim["brief_id"] = Value::String("successor-added-claim".to_owned());
        added_claim["surface_constraints"]["hero_budget"]["claims"]
            .as_array_mut()
            .expect("hero claims")
            .push(json!({
                "claim_id":"control-extra-hero",
                "source_kind":"author",
                "value_kind":"range",
                "min_triangles":1200,
                "max_triangles":4800,
                "evidence_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "confidence":"medium"
            }));
        cases.push(("added-claim", added_claim));

        let mut dropped_conflict = parent_brief.clone();
        dropped_conflict["brief_id"] = Value::String("successor-dropped-conflict".to_owned());
        dropped_conflict["source_conflicts"] = json!([]);
        cases.push(("dropped-conflict", dropped_conflict));

        let mut changed_conflict = parent_brief.clone();
        changed_conflict["brief_id"] = Value::String("successor-changed-conflict".to_owned());
        changed_conflict["source_conflicts"][0]["observed_claim_ids"] =
            json!(["control-hero-budget", "control-identity"]);
        cases.push(("changed-conflict-observed-claims", changed_conflict));

        for (label, mut child) in cases {
            child["parent_brief_id"] = Value::String(parent_id.to_owned());
            child["parent_brief_sha256"] = Value::String(parent_sha.to_owned());
            child["freeze_policy"] =
                Value::String("immutable-successor-preserve-source-claims@1".to_owned());
            child["canonical_sha256"] = Value::String(String::new());
            child["canonical_sha256"] = Value::String(canonical_json_hash(&child));
            let request = prepare_request(child, &reference, &format!("brief-{label}"));
            let error = runtime
                .weaponry_knife_production_brief_prepare(&request)
                .expect_err("invalid successor must not write");
            let detail = error.to_string();
            assert!(
                detail.contains("WEAPONRY_KNIFE_PRODUCTION_BRIEF_PARENT_"),
                "{label} did not fail at immutable parent boundary: {detail}"
            );
            let child_id = request["brief"]["brief_id"].as_str().expect("child id");
            assert_eq!(
                runtime
                    .store
                    .get_weaponry_knife_production_brief(
                        &project.project_id,
                        child_id,
                        &request["brief"]["canonical_sha256"]
                            .as_str()
                            .expect("child hash"),
                    )
                    .expect("child lookup"),
                None,
                "{label} left a durable child row"
            );
            assert_eq!(
                forgecad_store::Store::weaponry_knife_production_brief_cas_roots(&parent_record),
                parent_roots,
                "{label} changed parent CAS roots"
            );
        }
    }
}
