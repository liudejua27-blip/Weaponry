//! Public MCP transport for the read-only FormArt baseline preflight.
//!
//! The Runtime owns the lineage, camera rig, RenderSet and FormArt records.
//! This adapter exposes only the hash-bound scope needed to ask Runtime for a
//! preflight projection; it never accepts camera matrices, camera hashes,
//! RenderSet hashes, AOV bytes, image bytes, paths or external FormArt data.

use serde_json::{json, Map, Value};

const TOOL_NAME: &str = "production_weapon_form_art_baseline_preflight_get";
const REQUEST_SCHEMA_VERSION: &str = "ProductionWeaponFormArtBaselinePreflightRequest@1";
const OPERATION: &str = "forgecad.production.weapon.form-art-baseline-preflight-get@1";
const TRANSACTION: &str = "ProductionWeaponFormArtBaselineRefresh@1";
const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const IDENTIFIER_PATTERN: &str = "^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$";
const SHA256_PATTERN: &str = "^[0-9a-f]{64}$";

const REQUEST_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "preflight_id",
    "registration_lineage_id",
    "registration_lineage_canonical_sha256",
    "session_id",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "artifact_id",
    "artifact_sha256",
    "max_response_bytes",
    "writer_policy",
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

fn input_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":REQUEST_SCHEMA_VERSION}),
    );
    properties.insert("operation".to_owned(), json!({"const":OPERATION}));
    for field in [
        "preflight_id",
        "registration_lineage_id",
        "session_id",
        "project_id",
        "candidate_id",
        "artifact_id",
    ] {
        properties.insert(field.to_owned(), identifier_property());
    }
    for field in [
        "registration_lineage_canonical_sha256",
        "candidate_state_sha256",
        "artifact_sha256",
        "input_sha256",
    ] {
        properties.insert(field.to_owned(), sha256_property());
    }
    properties.insert(
        "max_response_bytes".to_owned(),
        json!({"const":MAX_RESPONSE_BYTES}),
    );
    properties.insert("writer_policy".to_owned(), json!({"const":WRITER_POLICY}));
    properties.insert("runtime_write_performed".to_owned(), json!({"const":false}));
    properties.insert(
        "persistent_user_data_touched".to_owned(),
        json!({"const":false}),
    );
    json!({
        "type":"object",
        "required":REQUEST_FIELDS,
        "properties":properties,
        "additionalProperties":false
    })
}

fn tool_definition() -> Value {
    json!({
        "name":TOOL_NAME,
        "description":"Read a Runtime-owned FormArt baseline preflight bound to one approved CameraLock registration lineage, session, candidate, artifact and baseline identity. Runtime verifies the RegisteredCameraRigCalibration@2 source and reports whether a fresh same-cohort six-view FormArt baseline producer is available. This read performs no SQLite/CAS write, Worker start, stage advancement, confirmation, version creation or export; callers cannot provide camera matrices, camera hashes, RenderSet hashes, AOVs, image bytes, paths or external FormArt content.",
        "inputSchema":input_schema(),
        "annotations":{
            "readOnlyHint":true,
            "destructiveHint":false,
            "idempotentHint":true,
            "openWorldHint":false,
            "writeIntent":false,
            "approvalRequired":false
        },
        "_meta":{"forgecad":{
            "availability":"available",
            "runtime_method":TOOL_NAME,
            "requiresConfirmation":false,
            "transaction":TRANSACTION,
            "definition_only":false
        }}
    })
}

pub fn is_tool(name: &str) -> bool {
    name == TOOL_NAME
}

pub fn is_write_tool(_name: &str) -> bool {
    false
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    is_tool(name).then_some(TOOL_NAME)
}

pub fn unavailable_error(name: &str) -> String {
    format!(
        "PRODUCTION_WEAPON_FORM_ART_BASELINE_PREFLIGHT_RUNTIME_METHOD_UNAVAILABLE: {name} requires Runtime method {TOOL_NAME}"
    )
}

pub fn read_tool_names() -> Vec<String> {
    vec![TOOL_NAME.to_owned()]
}

pub fn write_tool_names() -> Vec<String> {
    Vec::new()
}

pub fn read_tools() -> Vec<Value> {
    vec![tool_definition()]
}

pub fn write_tools() -> Vec<Value> {
    Vec::new()
}

/// Keep human-readable MCP text hash-only and bounded. The complete
/// Runtime-owned projection remains in structuredContent.
pub fn summary(name: &str, value: &Value) -> Option<String> {
    if !is_tool(name) {
        return None;
    }
    let lookup = |field: &str| value.get(field).cloned().unwrap_or(Value::Null);
    let bool_or_false = |field: &str| value.get(field).cloned().unwrap_or(Value::Bool(false));
    let view_kinds = value
        .get("views")
        .and_then(Value::as_array)
        .map(|views| {
            views
                .iter()
                .filter_map(|view| view.get("view_kind").and_then(Value::as_str))
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let view_count = view_kinds.len();
    serde_json::to_string(&json!({
        "schema_version":"ProductionWeaponFormArtBaselinePreflightMcpSummary@1",
        "operation":TOOL_NAME,
        "runtime_method":TOOL_NAME,
        "write_intent":"read_only_runtime_form_art_baseline_preflight",
        "preflight_id":lookup("preflight_id"),
        "registration_lineage_id":lookup("registration_lineage_id"),
        "registration_lineage_canonical_sha256":lookup("registration_lineage_canonical_sha256"),
        "registered_rig_v2_object_sha256":lookup("registered_rig_v2_object_sha256"),
        "registered_rig_v2_canonical_sha256":lookup("registered_rig_v2_canonical_sha256"),
        "session_id":lookup("session_id"),
        "project_id":lookup("project_id"),
        "candidate_id":lookup("candidate_id"),
        "candidate_state_sha256":lookup("candidate_state_sha256"),
        "artifact_id":lookup("artifact_id"),
        "artifact_sha256":lookup("artifact_sha256"),
        "lineage_status":lookup("lineage_status"),
        "lineage_promotable":bool_or_false("lineage_promotable"),
        "rig_v2_status":lookup("rig_v2_status"),
        "artifact_binding_status":lookup("artifact_binding_status"),
        "camera_source":if view_count > 0 {Value::String("registered-rig-v2.renderer_views".to_owned())} else {Value::Null},
        "rear_three_quarter_camera_source":lookup("rear_three_quarter_camera_source"),
        "fresh_baseline_materialized":bool_or_false("fresh_baseline_materialized"),
        "ready_for_fresh_baseline":bool_or_false("ready_for_fresh_baseline"),
        "view_count":view_count,
        "view_kinds":view_kinds,
        "blocking_reasons":lookup("blocking_reasons"),
        "restart_hash_verified":bool_or_false("restart_hash_verified"),
        "runtime_write":false,
        "runtime_write_performed":false,
        "worker_started":false,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "aov_bytes_in_summary":false,
        "structured_content_complete":true
    }))
    .ok()
}
