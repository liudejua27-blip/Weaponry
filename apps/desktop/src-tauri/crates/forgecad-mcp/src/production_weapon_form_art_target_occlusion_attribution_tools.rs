//! Thin read-only MCP surface for exact right trigger-void owner attribution.

use serde_json::{json, Value};

const TOOL_NAME: &str = "production_weapon_form_art_target_occlusion_attribution_get";
const SCHEMA_VERSION: &str = "ProductionWeaponFormArtTargetOcclusionAttributionGetRequest@1";
const OPERATION: &str = "forgecad.production.weapon.form-art-target-occlusion-attribution-get@1";
const ATTRIBUTION_POLICY: &str =
    "exact-parent-closed-receiver-upper-family-right-trigger-void-attribution@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-input-sha256@1";
const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const ID: &str = "^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$";
const SHA: &str = "^[0-9a-f]{64}$";

pub fn is_tool(name: &str) -> bool {
    name == TOOL_NAME
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    is_tool(name).then_some(TOOL_NAME)
}

pub fn unavailable_error(name: &str) -> String {
    format!("PRODUCTION_WEAPON_FORM_ART_TARGET_OCCLUSION_ATTRIBUTION_RUNTIME_METHOD_UNAVAILABLE: {name}")
}

pub fn read_tool_names() -> Vec<String> {
    vec![TOOL_NAME.to_owned()]
}

fn candidate_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["candidate_id","candidate_state_sha256","artifact_sha256","form_art_evidence_receipt_object_sha256"],
        "properties":{
            "candidate_id":{"type":"string","pattern":ID},
            "candidate_state_sha256":{"type":"string","pattern":SHA},
            "artifact_sha256":{"type":"string","pattern":SHA},
            "form_art_evidence_receipt_object_sha256":{"type":"string","pattern":SHA}
        }
    })
}

fn trial_schema() -> Value {
    let mut schema = candidate_schema();
    let object = schema.as_object_mut().expect("candidate schema object");
    object.insert(
        "required".to_owned(),
        json!([
            "registered_profile_id",
            "proposal_id",
            "composite_evidence_id",
            "candidate_id",
            "candidate_state_sha256",
            "artifact_sha256",
            "form_art_evidence_receipt_object_sha256"
        ]),
    );
    let properties = object
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .expect("candidate properties");
    properties.insert(
        "registered_profile_id".to_owned(),
        json!({"enum":["receiver-upper-retract-min-x-20mm@1","receiver-upper-retract-max-x-20mm@1","receiver-upper-retract-min-x-40mm@1","receiver-upper-retract-max-x-40mm@1","receiver-upper-target-notch-narrow@1","receiver-upper-target-notch-calibrated@1","receiver-upper-target-notch-raised@1","receiver-upper-target-notch-wide@1","receiver-upper-camera-target-notch-narrow@2","receiver-upper-camera-target-notch-calibrated@2","receiver-upper-camera-target-notch-raised@2","receiver-upper-camera-target-notch-wide@2"]}),
    );
    properties.insert(
        "proposal_id".to_owned(),
        json!({"type":"string","pattern":ID}),
    );
    properties.insert(
        "composite_evidence_id".to_owned(),
        json!({"type":"string","pattern":ID}),
    );
    schema
}

fn input_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["schema_version","operation","attribution_id","project_id","session_id","parent","trials","max_response_bytes","runtime_write_performed","persistent_user_data_touched","attribution_policy","canonicalization_policy","input_sha256"],
        "properties":{
            "schema_version":{"const":SCHEMA_VERSION},
            "operation":{"const":OPERATION},
            "attribution_id":{"type":"string","pattern":ID},
            "project_id":{"type":"string","pattern":ID},
            "session_id":{"type":"string","pattern":ID},
            "parent":candidate_schema(),
            "trials":{"type":"array","minItems":4,"maxItems":4,"items":trial_schema()},
            "max_response_bytes":{"const":MAX_RESPONSE_BYTES},
            "runtime_write_performed":{"const":false},
            "persistent_user_data_touched":{"const":false},
            "attribution_policy":{"const":ATTRIBUTION_POLICY},
            "canonicalization_policy":{"const":CANONICALIZATION_POLICY},
            "input_sha256":{"type":"string","pattern":SHA}
        }
    })
}

pub fn read_tools() -> Vec<Value> {
    vec![json!({
        "name":TOOL_NAME,
        "description":"Attribute the exact right.trigger-void target region across the retained parent and four closed receiver-upper retraction candidates. Runtime derives the registered camera, reference mask, triangle ownership and depth/Part-ID/silhouette deltas. No geometry, camera, mask, path, URL or script is accepted; no write or stage promotion occurs.",
        "inputSchema":input_schema(),
        "annotations":{"readOnlyHint":true,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false,"writeIntent":false,"approvalRequired":false},
        "_meta":{"forgecad":{"availability":"available","runtime_method":TOOL_NAME,"requiresConfirmation":false,"writeOptInRequired":false,"transaction":"read-only-target-region-owner-attribution@1","attribution_policy":ATTRIBUTION_POLICY,"maxResponseBytes":MAX_RESPONSE_BYTES,"definition_only":false}}
    })]
}

pub fn summary(name: &str, value: &Value) -> Option<String> {
    if !is_tool(name) {
        return None;
    }
    let get = |field: &str| value.get(field).cloned().unwrap_or(Value::Null);
    serde_json::to_string(&json!({
        "schema_version":"ProductionWeaponFormArtTargetOcclusionAttributionMcpSummary@1",
        "operation":TOOL_NAME,
        "runtime_method":TOOL_NAME,
        "write_intent":"read_only_exact_target_region_owner_attribution",
        "attribution_id":get("attribution_id"),
        "project_id":get("project_id"),
        "target_view_kind":get("target_view_kind"),
        "target_structure_id":get("target_structure_id"),
        "attributed_part_id":get("attributed_part_id"),
        "all_parent_and_trials_sealed":get("all_parent_and_trials_sealed"),
        "all_trials_zero_target_response":get("all_trials_zero_target_response"),
        "diagnostic_status":get("diagnostic_status"),
        "next_atomic_action":get("next_atomic_action"),
        "appearance_uv_pbr_write_authorized":get("appearance_uv_pbr_write_authorized"),
        "quality_status":get("quality_status"),
        "production_stage_advanced":get("production_stage_advanced"),
        "runtime_write_performed":get("runtime_write_performed"),
        "canonical_sha256":get("canonical_sha256"),
        "structured_content_complete":true
    }))
    .ok()
}
