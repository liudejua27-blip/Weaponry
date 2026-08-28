//! Thin read-only MCP surface for the exact rejected-repair FormArt failure
//! diagnostic. Runtime owns all durable evidence lookup and diagnosis.

use serde_json::{json, Map, Value};

const TOOL_NAME: &str = "production_weapon_form_art_failure_diagnostic_get";
const SCHEMA_VERSION: &str = "ProductionWeaponFormArtFailureDiagnosticGetRequest@1";
const OPERATION: &str = "forgecad.production.weapon.form-art-failure-diagnostic-get@1";
const DIAGNOSTIC_POLICY: &str = "exact-parent-proposal-cross-view-form-art-delta-diagnostic@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-input-sha256@1";
const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const IDENTIFIER_PATTERN: &str = "^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$";
const SHA256_PATTERN: &str = "^[0-9a-f]{64}$";
const FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "diagnostic_id",
    "composite_evidence_id",
    "proposal_id",
    "session_id",
    "project_id",
    "composite_evidence_record_canonical_sha256",
    "composite_evidence_receipt_object_sha256",
    "cross_view_evidence_bundle_sha256",
    "proposal_form_art_evidence_receipt_object_sha256",
    "max_response_bytes",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "diagnostic_policy",
    "canonicalization_policy",
    "input_sha256",
];

pub fn is_tool(name: &str) -> bool {
    name == TOOL_NAME
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    is_tool(name).then_some(TOOL_NAME)
}

pub fn unavailable_error(name: &str) -> String {
    format!("PRODUCTION_WEAPON_FORM_ART_FAILURE_DIAGNOSTIC_RUNTIME_METHOD_UNAVAILABLE: {name}")
}

pub fn read_tool_names() -> Vec<String> {
    vec![TOOL_NAME.to_owned()]
}

fn identifier_property() -> Value {
    json!({"type":"string","minLength":1,"maxLength":128,"pattern":IDENTIFIER_PATTERN})
}

fn sha256_property() -> Value {
    json!({"type":"string","pattern":SHA256_PATTERN})
}

fn input_schema() -> Value {
    let mut properties = Map::new();
    properties.insert("schema_version".to_owned(), json!({"const":SCHEMA_VERSION}));
    properties.insert("operation".to_owned(), json!({"const":OPERATION}));
    for field in [
        "diagnostic_id",
        "composite_evidence_id",
        "proposal_id",
        "session_id",
        "project_id",
    ] {
        properties.insert(field.to_owned(), identifier_property());
    }
    for field in [
        "composite_evidence_record_canonical_sha256",
        "composite_evidence_receipt_object_sha256",
        "cross_view_evidence_bundle_sha256",
        "proposal_form_art_evidence_receipt_object_sha256",
        "input_sha256",
    ] {
        properties.insert(field.to_owned(), sha256_property());
    }
    properties.insert(
        "max_response_bytes".to_owned(),
        json!({"const":MAX_RESPONSE_BYTES}),
    );
    properties.insert("runtime_write_performed".to_owned(), json!({"const":false}));
    properties.insert(
        "persistent_user_data_touched".to_owned(),
        json!({"const":false}),
    );
    properties.insert(
        "diagnostic_policy".to_owned(),
        json!({"const":DIAGNOSTIC_POLICY}),
    );
    properties.insert(
        "canonicalization_policy".to_owned(),
        json!({"const":CANONICALIZATION_POLICY}),
    );
    json!({"type":"object","additionalProperties":false,"required":FIELDS,"properties":properties})
}

pub fn read_tools() -> Vec<Value> {
    vec![json!({
        "name":TOOL_NAME,
        "description":"Diagnose one exact rejected composite FormArt repair from durable parent/proposal GeometryPrograms, CrossView and before/after FormArt evidence. Runtime separates source-local geometry response, registered-camera owner attribution, side trigger aperture visibility and line-flow status. It performs no write, Worker execution, mesh mutation, Stage advancement, confirmation, version or export.",
        "inputSchema":input_schema(),
        "annotations":{"readOnlyHint":true,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false,"writeIntent":false,"approvalRequired":false},
        "_meta":{"forgecad":{"availability":"available","runtime_method":TOOL_NAME,"requiresConfirmation":false,"writeOptInRequired":false,"transaction":"read-only-exact-rejected-form-art-diagnostic@1","diagnostic_policy":DIAGNOSTIC_POLICY,"maxResponseBytes":MAX_RESPONSE_BYTES,"definition_only":false}}
    })]
}

pub fn summary(name: &str, value: &Value) -> Option<String> {
    if !is_tool(name) {
        return None;
    }
    let lookup = |field: &str| value.get(field).cloned().unwrap_or(Value::Null);
    serde_json::to_string(&json!({
        "schema_version":"ProductionWeaponFormArtFailureDiagnosticMcpSummary@1",
        "operation":TOOL_NAME,
        "runtime_method":TOOL_NAME,
        "write_intent":"read_only_exact_rejected_form_art_diagnostic",
        "diagnostic_id":lookup("diagnostic_id"),
        "composite_evidence_id":lookup("composite_evidence_id"),
        "proposal_id":lookup("proposal_id"),
        "project_id":lookup("project_id"),
        "current_base_candidate_id":lookup("current_base_candidate_id"),
        "proposal_candidate_id":lookup("proposal_candidate_id"),
        "current_base_geometry_program_sha256":lookup("current_base_geometry_program_sha256"),
        "proposal_geometry_program_sha256":lookup("proposal_geometry_program_sha256"),
        "cross_view_evidence_bundle_sha256":lookup("cross_view_evidence_bundle_sha256"),
        "proposal_form_art_evidence_receipt_object_sha256":lookup("proposal_form_art_evidence_receipt_object_sha256"),
        "next_atomic_action":lookup("next_atomic_action"),
        "diagnostic_status":lookup("diagnostic_status"),
        "quality_status":lookup("quality_status"),
        "form_quality_v2_status":lookup("form_quality_v2_status"),
        "candidate_confirm_allowed":lookup("candidate_confirm_allowed"),
        "secondary_form_approved":lookup("secondary_form_approved"),
        "production_stage_advanced":lookup("production_stage_advanced"),
        "runtime_write_performed":lookup("runtime_write_performed"),
        "persistent_user_data_touched":lookup("persistent_user_data_touched"),
        "canonical_sha256":lookup("canonical_sha256"),
        "structured_content_complete":true
    })).ok()
}
