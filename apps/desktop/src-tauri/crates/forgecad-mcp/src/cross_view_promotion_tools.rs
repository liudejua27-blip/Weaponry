use serde_json::{json, Value};

const NAME: &str = "cross_view_promotion_confirm";
const FIELDS: [&str; 15] = [
    "project_id",
    "session_id",
    "source_candidate_id",
    "candidate_id",
    "bundle_sha256",
    "base_version_id",
    "prepared_object_id",
    "prepared_object_sha256",
    "quality_report_id",
    "approved",
    "approval_receipt_id",
    "approval_summary",
    "approval_session_id",
    "approval_expires_at",
    "idempotency_key",
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Binding {
    pub project_id: Option<String>,
    pub session_id: Option<String>,
    pub source_candidate_id: Option<String>,
    pub candidate_id: Option<String>,
}

pub fn is_tool(name: &str) -> bool {
    name == NAME
}

pub fn is_write_tool(name: &str) -> bool {
    is_tool(name)
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    is_tool(name).then_some(NAME)
}

pub fn unavailable_error(name: &str) -> String {
    format!(
        "CROSS_VIEW_PROMOTION_RUNTIME_METHOD_UNAVAILABLE: {name} requires Runtime method {NAME}"
    )
}

pub fn write_tool_names() -> Vec<String> {
    vec![NAME.to_owned()]
}

pub fn write_tools() -> Vec<Value> {
    vec![tool_definition()]
}

fn tool_definition() -> Value {
    json!({
        "name":NAME,
        "description":"Promote one complete, strictly improved CrossViewEvidenceBundle after explicit user approval. The bundle is evidence-only; Runtime revalidates the exact session, ReferenceCanvas, candidate artifact, per-view reports and stale-head guard before creating an immutable version and snapshot. It never exports.",
        "inputSchema":input_schema(),
        "annotations":{
            "readOnlyHint":false,
            "destructiveHint":false,
            "idempotentHint":true,
            "openWorldHint":false,
            "writeIntent":true,
            "approvalRequired":true
        },
        "_meta":{"forgecad":{
            "availability":"available",
            "runtime_method":NAME,
            "requiresConfirmation":true,
            "transaction":"ADR-0026-cross-view-promotion",
            "definition_only":false
        }}
    })
}

fn input_schema() -> Value {
    json!({
        "type":"object",
        "required":FIELDS,
        "properties":{
            "project_id":id_property(),
            "session_id":id_property(),
            "source_candidate_id":id_property(),
            "candidate_id":id_property(),
            "bundle_sha256":sha_property(),
            "base_version_id":{"type":["string","null"],"pattern":"^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"},
            "prepared_object_id":id_property(),
            "prepared_object_sha256":sha_property(),
            "quality_report_id":id_property(),
            "approved":{"const":true},
            "approval_receipt_id":id_property(),
            "approval_summary":{"type":"string","minLength":1,"maxLength":512},
            "approval_session_id":id_property(),
            "approval_expires_at":{"type":"string","minLength":1,"maxLength":64},
            "idempotency_key":id_property()
        },
        "additionalProperties":false
    })
}

pub fn validate_call(name: &str, arguments: &Value, binding: &Binding) -> Result<(), String> {
    if !is_tool(name) {
        return Ok(());
    }
    let object = arguments.as_object().ok_or_else(|| {
        "CROSS_VIEW_PROMOTION_INVALID_INPUT: arguments must be an object".to_owned()
    })?;
    if object.keys().any(|key| !FIELDS.contains(&key.as_str())) {
        return Err("CROSS_VIEW_PROMOTION_INVALID_INPUT: unsupported field".to_owned());
    }
    for (key, expected) in [
        ("project_id", binding.project_id.as_deref()),
        ("session_id", binding.session_id.as_deref()),
        (
            "source_candidate_id",
            binding.source_candidate_id.as_deref(),
        ),
        ("candidate_id", binding.candidate_id.as_deref()),
    ] {
        if let Some(expected) = expected {
            if object.get(key).and_then(Value::as_str) != Some(expected) {
                return Err(format!(
                    "CROSS_VIEW_PROMOTION_SCOPE_MISMATCH: {key} differs from bound promotion"
                ));
            }
        }
    }
    for key in [
        "project_id",
        "session_id",
        "source_candidate_id",
        "candidate_id",
        "bundle_sha256",
        "prepared_object_id",
        "prepared_object_sha256",
        "quality_report_id",
        "approval_receipt_id",
        "approval_summary",
        "approval_session_id",
        "approval_expires_at",
        "idempotency_key",
    ] {
        if object
            .get(key)
            .and_then(Value::as_str)
            .is_none_or(|value| value.is_empty())
        {
            return Err(format!(
                "CROSS_VIEW_PROMOTION_INVALID_INPUT: {key} is required"
            ));
        }
    }
    if object.get("approved") != Some(&Value::Bool(true)) {
        return Err("CROSS_VIEW_PROMOTION_APPROVAL_REQUIRED: approved=true is required".to_owned());
    }
    if object.get("approval_session_id") != object.get("session_id") {
        return Err(
            "CROSS_VIEW_PROMOTION_SCOPE_MISMATCH: approval_session_id must match session_id"
                .to_owned(),
        );
    }
    for key in ["bundle_sha256", "prepared_object_sha256"] {
        if !object
            .get(key)
            .and_then(Value::as_str)
            .is_some_and(is_sha256)
        {
            return Err(format!(
                "CROSS_VIEW_PROMOTION_INVALID_INPUT: {key} must be a lowercase SHA-256"
            ));
        }
    }
    Ok(())
}

pub fn validate_response(name: &str, value: &Value, binding: &Binding) -> Result<(), String> {
    if !is_tool(name) {
        return Ok(());
    }
    let object = value.as_object().ok_or_else(|| {
        "CROSS_VIEW_PROMOTION_RESPONSE_INVALID: response must be an object".to_owned()
    })?;
    if object.get("schema_version").and_then(Value::as_str) != Some("CrossViewPromotionResult@1") {
        return Err("CROSS_VIEW_PROMOTION_RESPONSE_INVALID: schema version differs".to_owned());
    }
    for (key, expected) in [
        ("project_id", binding.project_id.as_deref()),
        ("session_id", binding.session_id.as_deref()),
        (
            "source_candidate_id",
            binding.source_candidate_id.as_deref(),
        ),
        ("candidate_id", binding.candidate_id.as_deref()),
    ] {
        if let Some(expected) = expected {
            if object.get(key).and_then(Value::as_str) != Some(expected) {
                return Err(format!(
                    "CROSS_VIEW_PROMOTION_RESPONSE_SCOPE_MISMATCH: {key} differs"
                ));
            }
        }
    }
    Ok(())
}

pub fn bind_response(name: &str, value: &Value, binding: &mut Binding) -> Result<(), String> {
    validate_response(name, value, binding)?;
    let object = value.as_object().ok_or_else(|| {
        "CROSS_VIEW_PROMOTION_RESPONSE_INVALID: response must be an object".to_owned()
    })?;
    for (key, slot) in [
        ("project_id", &mut binding.project_id),
        ("session_id", &mut binding.session_id),
        ("source_candidate_id", &mut binding.source_candidate_id),
        ("candidate_id", &mut binding.candidate_id),
    ] {
        let value = object
            .get(key)
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| format!("CROSS_VIEW_PROMOTION_RESPONSE_INVALID: {key} is missing"))?;
        if slot.as_deref().is_some_and(|expected| expected != value) {
            return Err(format!(
                "CROSS_VIEW_PROMOTION_RESPONSE_SCOPE_MISMATCH: {key} cannot be rebound"
            ));
        }
        *slot = Some(value.to_owned());
    }
    Ok(())
}

fn id_property() -> Value {
    json!({"type":"string","pattern":"^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"})
}

fn sha_property() -> Value {
    json!({"type":"string","pattern":"^[0-9a-f]{64}$"})
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_opaque_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}
