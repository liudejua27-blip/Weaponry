//! Public MCP transport for the read-only owner-to-reviewed-void calibration
//! projection.
//!
//! This is deliberately only the transport envelope.  Runtime owns the
//! calibration implementation and resolves all Part-ID/depth/AOV
//! inputs from the hash-bound baseline and registration lineage.  The MCP
//! surface accepts no paths, URLs, bytes, masks or mesh arrays.

use serde_json::{json, Map, Value};

const TOOL_NAME: &str = "production_weapon_owner_reviewed_void_calibration_get";
const REQUEST_SCHEMA_VERSION: &str =
    "ProductionWeaponOwnerReviewedVoidCalibrationProjectionGetRequest@1";
const OPERATION: &str =
    "forgecad.production.weapon.owner-reviewed-void-calibration-projection-get@1";
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const IDENTIFIER_PATTERN: &str = "^[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}$";
const SHA256_PATTERN: &str = "^[0-9a-f]{64}$";

const REQUEST_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "projection_id",
    "registration_lineage_id",
    "registration_lineage_canonical_sha256",
    "session_id",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "artifact_id",
    "artifact_sha256",
    "artifact_readback_sha256",
    "form_art_evidence_id",
    "form_art_evidence_object_sha256",
    "form_art_evidence_canonical_sha256",
    "fresh_baseline_id",
    "fresh_baseline_canonical_sha256",
    "fresh_baseline_receipt_object_sha256",
    "registration_lineage_receipt_object_sha256",
    "registered_rig_v2_id",
    "registered_rig_v2_object_sha256",
    "registered_rig_v2_canonical_sha256",
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
        "projection_id",
        "registration_lineage_id",
        "session_id",
        "project_id",
        "candidate_id",
        "artifact_id",
        "form_art_evidence_id",
        "fresh_baseline_id",
        "registered_rig_v2_id",
    ] {
        properties.insert(field.to_owned(), identifier_property());
    }
    for field in [
        "registration_lineage_canonical_sha256",
        "candidate_state_sha256",
        "artifact_sha256",
        "artifact_readback_sha256",
        "form_art_evidence_object_sha256",
        "form_art_evidence_canonical_sha256",
        "fresh_baseline_canonical_sha256",
        "fresh_baseline_receipt_object_sha256",
        "registration_lineage_receipt_object_sha256",
        "registered_rig_v2_object_sha256",
        "registered_rig_v2_canonical_sha256",
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
        "description":"Read a Runtime-owned, hash-bound owner-to-reviewed-void calibration projection for the rear-stock Part across the exact left/right/rear-three-quarter registered views. Runtime derives the reviewed void, Part-ID owner and depth evidence from durable candidate, FormArt, fresh-baseline and camera-lineage records. It accepts no paths, URLs, bytes, masks, mesh arrays, camera matrices or caller-supplied AOV data, and performs no SQLite/CAS write, Worker start, stage advancement, confirmation, version creation or export.",
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
            "transaction":"ProductionWeaponOwnerReviewedVoidCalibrationProjection@1",
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
        "PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_RUNTIME_METHOD_UNAVAILABLE: {name} requires Runtime method {TOOL_NAME}"
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

/// Keep the human-readable MCP text hash-only and bounded.  The structured
/// Runtime projection remains the source of truth once its algorithm is
/// implemented.
pub fn summary(name: &str, value: &Value) -> Option<String> {
    if !is_tool(name) {
        return None;
    }
    let projection = value.get("projection").unwrap_or(value);
    let get = |field: &str| projection.get(field).cloned().unwrap_or(Value::Null);
    serde_json::to_string(&json!({
        "schema_version":"ProductionWeaponOwnerReviewedVoidCalibrationProjectionMcpSummary@1",
        "operation":TOOL_NAME,
        "projection_id":get("projection_id"),
        "registration_lineage_id":get("registration_lineage_id"),
        "registration_lineage_canonical_sha256":get("registration_lineage_canonical_sha256"),
        "session_id":get("session_id"),
        "project_id":get("project_id"),
        "candidate_id":get("candidate_id"),
        "candidate_state_sha256":get("candidate_state_sha256"),
        "artifact_id":get("artifact_id"),
        "artifact_sha256":get("artifact_sha256"),
        "fresh_baseline_id":get("fresh_baseline_id"),
        "fresh_baseline_canonical_sha256":get("fresh_baseline_canonical_sha256"),
        "form_art_evidence_id":get("form_art_evidence_id"),
        "form_art_evidence_canonical_sha256":get("form_art_evidence_canonical_sha256"),
        "owner_part_id":get("owner_part_id"),
        "view_kinds":get("view_kinds"),
        "calibration_status":get("calibration_status"),
        "eligible":get("eligible"),
        "all_views_passed":get("all_views_passed"),
        "strict_owner_void_all_views_passed":get("strict_owner_void_all_views_passed"),
        "strict_depth_all_views_passed":get("strict_depth_all_views_passed"),
        "blocker_codes":get("blocker_codes"),
        "quality_status":get("quality_status"),
        "depth_status":get("depth_status"),
        "runtime_write_performed":false,
        "persistent_user_data_touched":false,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "raw_aov_bytes":false,
        "raw_mask_bytes":false,
        "raw_mesh_arrays":false,
        "structured_content_complete":true
    }))
    .ok()
}
