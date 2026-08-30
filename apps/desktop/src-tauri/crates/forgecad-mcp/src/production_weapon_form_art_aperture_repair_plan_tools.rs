//! Thin default-read MCP surface for the hash-bound 04BE-H sequential
//! two-Part aperture sensitivity plan. Runtime owns all evidence derivation.

use serde_json::{json, Map, Value};

const TOOL_NAME: &str = "production_weapon_form_art_aperture_repair_plan_get";
const SCHEMA_VERSION: &str = "ProductionWeaponFormArtApertureRepairPlanGetRequest@1";
const OPERATION: &str = "forgecad.production.weapon.form-art-aperture-repair-plan-get@1";
const DERIVATION_POLICY: &str = "exact-raster-calibrated-sequential-aperture-sensitivity-plan@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-input-sha256@1";
const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const IDENTIFIER_PATTERN: &str = "^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$";
const SHA256_PATTERN: &str = "^[0-9a-f]{64}$";
const FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "aperture_repair_plan_id",
    "visibility_calibration_id",
    "visibility_calibration_canonical_sha256",
    "visibility_calibration_input_sha256",
    "failure_diagnostic_id",
    "failure_diagnostic_canonical_sha256",
    "failure_diagnostic_input_sha256",
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
    "derivation_policy",
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
    format!("PRODUCTION_WEAPON_FORM_ART_APERTURE_REPAIR_PLAN_RUNTIME_METHOD_UNAVAILABLE: {name}")
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
        "aperture_repair_plan_id",
        "visibility_calibration_id",
        "failure_diagnostic_id",
        "composite_evidence_id",
        "proposal_id",
        "session_id",
        "project_id",
    ] {
        properties.insert(field.to_owned(), identifier_property());
    }
    for field in [
        "visibility_calibration_canonical_sha256",
        "visibility_calibration_input_sha256",
        "failure_diagnostic_canonical_sha256",
        "failure_diagnostic_input_sha256",
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
        "derivation_policy".to_owned(),
        json!({"const":DERIVATION_POLICY}),
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
        "description":"Re-derive one exact raster visibility calibration and return two strictly ordered one-Part aperture sensitivity steps. Step 1 searches four bounded side-panel-a longitudinal boundary retractions; step 2 remains dependent on a retained six-view-non-regressing result before searching receiver-upper. The tool accepts no geometry, masks, raster bytes, cameras, paths, URLs or scripts and performs no mutation, candidate creation, Stage advancement, confirmation, version or export.",
        "inputSchema":input_schema(),
        "annotations":{"readOnlyHint":true,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false,"writeIntent":false,"approvalRequired":false},
        "_meta":{"forgecad":{"availability":"available","runtime_method":TOOL_NAME,"requiresConfirmation":false,"writeOptInRequired":false,"transaction":"read-only-hash-bound-sequential-aperture-plan@1","derivation_policy":DERIVATION_POLICY,"maxResponseBytes":MAX_RESPONSE_BYTES,"definition_only":false}}
    })]
}

pub fn summary(name: &str, value: &Value) -> Option<String> {
    if !is_tool(name) {
        return None;
    }
    let lookup = |field: &str| value.get(field).cloned().unwrap_or(Value::Null);
    serde_json::to_string(&json!({
        "schema_version":"ProductionWeaponFormArtApertureRepairPlanMcpSummary@1",
        "operation":TOOL_NAME,
        "runtime_method":TOOL_NAME,
        "write_intent":"read_only_hash_bound_sequential_aperture_plan",
        "aperture_repair_plan_id":lookup("aperture_repair_plan_id"),
        "visibility_calibration_id":lookup("visibility_calibration_id"),
        "visibility_calibration_canonical_sha256":lookup("visibility_calibration_canonical_sha256"),
        "project_id":lookup("project_id"),
        "proposal_candidate_id":lookup("proposal_candidate_id"),
        "calibrated_source_bindings":lookup("calibrated_source_bindings"),
        "plan_steps":lookup("plan_steps"),
        "total_trial_candidate_budget":lookup("total_trial_candidate_budget"),
        "plan_status":lookup("plan_status"),
        "next_trial_registration_authorized":lookup("next_trial_registration_authorized"),
        "repair_execution_allowed_by_this_tool":lookup("repair_execution_allowed_by_this_tool"),
        "geometry_repair_performed":lookup("geometry_repair_performed"),
        "next_atomic_action":lookup("next_atomic_action"),
        "quality_status":lookup("quality_status"),
        "form_quality_v2_status":lookup("form_quality_v2_status"),
        "production_stage_advanced":lookup("production_stage_advanced"),
        "runtime_write_performed":lookup("runtime_write_performed"),
        "persistent_user_data_touched":lookup("persistent_user_data_touched"),
        "canonical_sha256":lookup("canonical_sha256"),
        "structured_content_complete":true
    }))
    .ok()
}
