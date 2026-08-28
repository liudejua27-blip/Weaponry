//! MCP transport for the Runtime-owned fresh FormArt baseline producer.
//!
//! The adapter exposes a closed prepare/get envelope only.  It never accepts
//! camera matrices, RenderSet/AOV hashes, image bytes, paths or external
//! FormEvidence/FormArt content; Runtime derives those values from the
//! approved registration lineage and durable ReferenceCanvas.

use serde_json::{json, Map, Value};

const PREPARE_TOOL_NAME: &str = "production_weapon_form_art_baseline_prepare";
const GET_TOOL_NAME: &str = "production_weapon_form_art_baseline_get";
const PREPARE_SCHEMA_VERSION: &str = "ProductionWeaponFormArtBaselinePrepareRequest@1";
const GET_SCHEMA_VERSION: &str = "ProductionWeaponFormArtBaselineGetRequest@1";
const PREPARE_OPERATION: &str = "forgecad.production.weapon.form-art-baseline-prepare@1";
const GET_OPERATION: &str = "forgecad.production.weapon.form-art-baseline-get@1";
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-canonical-sha256@1";
const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const IDENTIFIER_PATTERN: &str = "^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$";
const SHA256_PATTERN: &str = "^[0-9a-f]{64}$";

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "baseline_id",
    "registration_lineage_id",
    "registration_lineage_canonical_sha256",
    "session_id",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "artifact_id",
    "artifact_sha256",
    "base_version_id",
    "idempotency_key",
    "max_response_bytes",
    "writer_policy",
    "canonicalization_policy",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "input_sha256",
];

const GET_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "baseline_id",
    "registration_lineage_id",
    "registration_lineage_canonical_sha256",
    "session_id",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "artifact_id",
    "artifact_sha256",
    "base_version_id",
    "idempotency_key",
    "max_response_bytes",
    "writer_policy",
    "canonicalization_policy",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "input_sha256",
];

fn identifier_property() -> Value {
    json!({
        "type":"string",
        "minLength":1,
        "maxLength":128,
        "pattern":IDENTIFIER_PATTERN
    })
}

fn sha256_property() -> Value {
    json!({"type":"string","pattern":SHA256_PATTERN})
}

fn input_schema(fields: &[&str], schema_version: &str, operation: &str) -> Value {
    let mut properties = Map::new();
    properties.insert("schema_version".to_owned(), json!({"const":schema_version}));
    properties.insert("operation".to_owned(), json!({"const":operation}));
    for field in [
        "baseline_id",
        "registration_lineage_id",
        "session_id",
        "project_id",
        "candidate_id",
        "artifact_id",
        "idempotency_key",
    ] {
        if fields.contains(&field) {
            properties.insert(field.to_owned(), identifier_property());
        }
    }
    for field in [
        "registration_lineage_canonical_sha256",
        "candidate_state_sha256",
        "artifact_sha256",
        "input_sha256",
    ] {
        if fields.contains(&field) {
            properties.insert(field.to_owned(), sha256_property());
        }
    }
    properties.insert(
        "max_response_bytes".to_owned(),
        json!({"const":MAX_RESPONSE_BYTES}),
    );
    properties.insert(
        "base_version_id".to_owned(),
        json!({"type":["string","null"],"pattern":IDENTIFIER_PATTERN}),
    );
    properties.insert("writer_policy".to_owned(), json!({"const":WRITER_POLICY}));
    properties.insert(
        "canonicalization_policy".to_owned(),
        json!({"const":CANONICALIZATION_POLICY}),
    );
    properties.insert("runtime_write_performed".to_owned(), json!({"const":false}));
    properties.insert(
        "persistent_user_data_touched".to_owned(),
        json!({"const":false}),
    );
    json!({
        "type":"object",
        "required":fields,
        "properties":properties,
        "additionalProperties":false
    })
}

fn tool_definition(name: &str) -> Value {
    let prepare = name == PREPARE_TOOL_NAME;
    let (schema, description, transaction) = if prepare {
        (
            input_schema(&PREPARE_FIELDS, PREPARE_SCHEMA_VERSION, PREPARE_OPERATION),
            "Prepare a fresh Runtime-owned six-view FormArt baseline from one approved CameraLock registration lineage. Runtime derives the six RigV2 cameras, invokes the fixed 512x512 nine-AOV Render Worker once per view in one cohort, and persists only the new baseline receipt. Existing FormEvidence/FormArt@1 is never reused; this does not advance ProductionStage, confirm a candidate, create a version or export.",
            "RegisteredCameraRigCalibration@2→six RenderSet@2→ProductionWeaponFormArtBaseline@1",
        )
    } else {
        (
            input_schema(&GET_FIELDS, GET_SCHEMA_VERSION, GET_OPERATION),
            "Read one exact Runtime-owned fresh FormArt baseline by its closed scope and idempotency binding. This is a read-only restart/hash check and does not start a Worker or change candidate, stage, confirmation, version or export state.",
            "ProductionWeaponFormArtBaseline@1",
        )
    };
    json!({
        "name":name,
        "description":description,
        "inputSchema":schema,
        "annotations":{
            "readOnlyHint":!prepare,
            "destructiveHint":false,
            "idempotentHint":true,
            "openWorldHint":false,
            "writeIntent":prepare,
            "approvalRequired":prepare
        },
        "_meta":{"forgecad":{
            "availability":"available",
            "runtime_method":name,
            "requiresConfirmation":prepare,
            "transaction":transaction,
            "definition_only":false
        }}
    })
}

pub fn is_tool(name: &str) -> bool {
    matches!(name, PREPARE_TOOL_NAME | GET_TOOL_NAME)
}

pub fn is_write_tool(name: &str) -> bool {
    name == PREPARE_TOOL_NAME
}

pub fn from_name(name: &str) -> Option<&'static str> {
    match name {
        PREPARE_TOOL_NAME => Some(PREPARE_TOOL_NAME),
        GET_TOOL_NAME => Some(GET_TOOL_NAME),
        _ => None,
    }
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    from_name(name)
}

pub fn unavailable_error(name: &str) -> String {
    format!("PRODUCTION_WEAPON_FORM_ART_BASELINE_RUNTIME_METHOD_UNAVAILABLE: {name}")
}

pub fn read_tool_names() -> Vec<String> {
    vec![GET_TOOL_NAME.to_owned()]
}

pub fn write_tool_names() -> Vec<String> {
    vec![PREPARE_TOOL_NAME.to_owned()]
}

pub fn read_tools() -> Vec<Value> {
    vec![tool_definition(GET_TOOL_NAME)]
}

pub fn write_tools() -> Vec<Value> {
    vec![tool_definition(PREPARE_TOOL_NAME)]
}

pub fn summary(name: &str, value: &Value) -> Option<String> {
    if !is_tool(name) {
        return None;
    }
    serde_json::to_string(&json!({
        "schema_version":"ProductionWeaponFormArtBaselineMcpSummary@1",
        "operation":name,
        "runtime_method":name,
        "write_intent":if is_write_tool(name) {"runtime_prepare_fresh_form_art_baseline"} else {"read_only_fresh_form_art_baseline"},
        "baseline_id":value.pointer("/baseline/baseline_id"),
        "baseline_object_sha256":value.pointer("/baseline/receipt_object_sha256"),
        "baseline_canonical_sha256":value.pointer("/baseline/canonical_sha256"),
        "registration_lineage_id":value.pointer("/baseline/registration_lineage_id"),
        "registered_rig_v2_id":value.pointer("/baseline/registered_rig_v2_id"),
        "view_kinds":value.pointer("/baseline/view_kinds"),
        "view_count":value.pointer("/baseline/views").and_then(Value::as_array).map(Vec::len),
        "runtime_build_cohort_sha256":value.pointer("/baseline/runtime_build_cohort_sha256"),
        "quality_status":value.pointer("/baseline/quality_status"),
        "runtime_write_performed":value.get("runtime_write_performed"),
        "persistent_user_data_touched":value.get("persistent_user_data_touched"),
        "production_stage_advanced":value.get("production_stage_advanced"),
        "candidate_confirmed":value.get("candidate_confirmed"),
        "version_created":value.get("version_created"),
        "export_performed":value.get("export_performed"),
        "restart_hash_verified":value.get("restart_hash_verified"),
        "aov_bytes_in_summary":false,
        "structured_content_complete":true
    }))
    .ok()
}
