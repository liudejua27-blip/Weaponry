//! Thin MCP transport for the cumulative production-weapon FormArt proposal.
//!
//! The Runtime owns all Store/CAS reads and writes.  This adapter exposes the
//! same closed request envelopes as the first-party contract files, forwards
//! the request to the Runtime, and projects a bounded hash-only summary.  A
//! prepared value is a reviewable candidate only; it is never approval,
//! promotion, versioning, export, or a commercial-quality assertion.

use serde_json::{json, Value};

const PREPARE_TOOL_NAME: &str = "production_weapon_form_art_composite_proposal_prepare";
const GET_TOOL_NAME: &str = "production_weapon_form_art_composite_proposal_get";
const PREPARE_SCHEMA_VERSION: &str = "ProductionWeaponFormArtCompositeProposalPrepareRequest@1";
const GET_SCHEMA_VERSION: &str = "ProductionWeaponFormArtCompositeProposalGetRequest@1";
const PLAN_SCHEMA_VERSION: &str = "ProductionWeaponFormArtCompositeProposalPlan@1";
const OPERATION_SCHEMA_VERSION: &str = "ProductionWeaponFormArtCompositeProposalOperation@1";
const OPERATION_KIND: &str = "registered_profile_replace";
const TRIGGER_GUARD_SOURCE_NODE_ID: &str = "trigger-guard";
const TRIGGER_GUARD_PART_ID: &str = "trigger-guard";
const TRIGGER_GUARD_REGISTERED_PROFILE_ID: &str = "trigger-guard-side-aperture-xy@1";
const REAR_STOCK_SOURCE_NODE_ID: &str = "rear-stock";
const REAR_STOCK_PART_ID: &str = "rear-stock";
const REAR_STOCK_REGISTERED_PROFILE_ID: &str =
    "registered-boundary-bridge-half-y-flat-z-owner-void@1";
const SIDE_PANEL_A_SOURCE_NODE_ID: &str = "side-panel-a";
const SIDE_PANEL_A_PART_ID: &str = "side-panel-a";
const SIDE_PANEL_A_REGISTERED_PROFILE_IDS: [&str; 12] = [
    "side-panel-a-retract-min-x-20mm@1",
    "side-panel-a-retract-max-x-20mm@1",
    "side-panel-a-retract-min-x-40mm@1",
    "side-panel-a-retract-max-x-40mm@1",
    "side-panel-a-true-aperture-narrow@1",
    "side-panel-a-true-aperture-calibrated@1",
    "side-panel-a-true-aperture-forward@1",
    "side-panel-a-true-aperture-wide@1",
    "side-panel-a-camera-mapped-aperture-narrow@2",
    "side-panel-a-camera-mapped-aperture-calibrated@2",
    "side-panel-a-camera-mapped-aperture-raised@2",
    "side-panel-a-camera-mapped-aperture-wide@2",
];
const RECEIVER_UPPER_SOURCE_NODE_ID: &str = "receiver-upper";
const RECEIVER_UPPER_PART_ID: &str = "receiver-upper";
const RECEIVER_UPPER_REGISTERED_PROFILE_IDS: [&str; 12] = [
    "receiver-upper-retract-min-x-20mm@1",
    "receiver-upper-retract-max-x-20mm@1",
    "receiver-upper-retract-min-x-40mm@1",
    "receiver-upper-retract-max-x-40mm@1",
    "receiver-upper-target-notch-narrow@1",
    "receiver-upper-target-notch-calibrated@1",
    "receiver-upper-target-notch-raised@1",
    "receiver-upper-target-notch-wide@1",
    "receiver-upper-camera-target-notch-narrow@2",
    "receiver-upper-camera-target-notch-calibrated@2",
    "receiver-upper-camera-target-notch-raised@2",
    "receiver-upper-camera-target-notch-wide@2",
];
const COMPOSITION_POLICY: &str =
    "runtime-owned-original-baseline-current-base-registered-disjoint-replacements@1";
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-input-sha256@1";
const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const IDENTIFIER_PATTERN: &str = "^[A-Za-z0-9._:-]+$";
const SHA256_PATTERN: &str = "^[0-9a-f]{64}$";

#[derive(Clone, Copy)]
enum Tool {
    Get,
    Prepare,
}

impl Tool {
    fn name(self) -> &'static str {
        match self {
            Self::Get => GET_TOOL_NAME,
            Self::Prepare => PREPARE_TOOL_NAME,
        }
    }

    fn is_write(self) -> bool {
        matches!(self, Self::Prepare)
    }
}

fn from_name(name: &str) -> Option<Tool> {
    match name {
        GET_TOOL_NAME => Some(Tool::Get),
        PREPARE_TOOL_NAME => Some(Tool::Prepare),
        _ => None,
    }
}

pub fn is_tool(name: &str) -> bool {
    from_name(name).is_some()
}

pub fn is_write_tool(name: &str) -> bool {
    from_name(name).is_some_and(Tool::is_write)
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    from_name(name).map(Tool::name)
}

pub fn unavailable_error(name: &str) -> String {
    format!("PRODUCTION_WEAPON_FORM_ART_COMPOSITE_PROPOSAL_RUNTIME_METHOD_UNAVAILABLE: {name}")
}

pub fn read_tool_names() -> Vec<String> {
    vec![GET_TOOL_NAME.to_owned()]
}

pub fn write_tool_names() -> Vec<String> {
    vec![PREPARE_TOOL_NAME.to_owned()]
}

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

fn operation_schema() -> Value {
    let variant = |source_node_id: &str, part_id: &str, registered_profile_id: &str| {
        json!({
            "type":"object",
            "additionalProperties":false,
            "required":[
                "schema_version",
                "sequence_index",
                "operation_id",
                "operation_kind",
                "source_node_id",
                "part_id",
                "registered_profile_id",
                "canonical_sha256"
            ],
            "properties":{
                "schema_version":{"const":OPERATION_SCHEMA_VERSION},
                "sequence_index":{"type":"integer","minimum":0,"maximum":7},
                "operation_id":identifier_property(),
                "operation_kind":{"const":OPERATION_KIND},
                "source_node_id":{"const":source_node_id},
                "part_id":{"const":part_id},
                "registered_profile_id":{"const":registered_profile_id},
                "canonical_sha256":sha256_property()
            }
        })
    };
    let mut variants = vec![
        variant(
            TRIGGER_GUARD_SOURCE_NODE_ID,
            TRIGGER_GUARD_PART_ID,
            TRIGGER_GUARD_REGISTERED_PROFILE_ID,
        ),
        variant(
            REAR_STOCK_SOURCE_NODE_ID,
            REAR_STOCK_PART_ID,
            REAR_STOCK_REGISTERED_PROFILE_ID,
        ),
    ];
    variants.extend(
        SIDE_PANEL_A_REGISTERED_PROFILE_IDS
            .iter()
            .map(|profile| variant(SIDE_PANEL_A_SOURCE_NODE_ID, SIDE_PANEL_A_PART_ID, profile)),
    );
    variants.extend(RECEIVER_UPPER_REGISTERED_PROFILE_IDS.iter().map(|profile| {
        variant(
            RECEIVER_UPPER_SOURCE_NODE_ID,
            RECEIVER_UPPER_PART_ID,
            profile,
        )
    }));
    json!({"oneOf":variants})
}

fn plan_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":[
            "schema_version",
            "project_id",
            "original_source_candidate_id",
            "original_source_candidate_state_sha256",
            "original_source_artifact_sha256",
            "original_fresh_baseline_canonical_sha256",
            "current_base_candidate_id",
            "current_base_candidate_state_sha256",
            "current_base_artifact_sha256",
            "current_base_geometry_program_sha256",
            "current_base_proposal_evidence_sha256",
            "operations",
            "composition_policy",
            "canonical_sha256"
        ],
        "properties":{
            "schema_version":{"const":PLAN_SCHEMA_VERSION},
            "project_id":identifier_property(),
            "original_source_candidate_id":identifier_property(),
            "original_source_candidate_state_sha256":sha256_property(),
            "original_source_artifact_sha256":sha256_property(),
            "original_fresh_baseline_canonical_sha256":sha256_property(),
            "current_base_candidate_id":identifier_property(),
            "current_base_candidate_state_sha256":sha256_property(),
            "current_base_artifact_sha256":sha256_property(),
            "current_base_geometry_program_sha256":sha256_property(),
            "current_base_proposal_evidence_sha256":sha256_property(),
            "operations":{
                "type":"array",
                "minItems":1,
                "maxItems":8,
                "uniqueItems":true,
                "items":operation_schema()
            },
            "composition_policy":{"const":COMPOSITION_POLICY},
            "canonical_sha256":sha256_property()
        }
    })
}

fn prepare_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":[
            "schema_version",
            "proposal_id",
            "session_id",
            "project_id",
            "original_fresh_baseline_id",
            "plan",
            "idempotency_key",
            "max_response_bytes",
            "runtime_write_performed",
            "writer_policy",
            "canonicalization_policy",
            "input_sha256"
        ],
        "properties":{
            "schema_version":{"const":PREPARE_SCHEMA_VERSION},
            "proposal_id":identifier_property(),
            "session_id":identifier_property(),
            "project_id":identifier_property(),
            "original_fresh_baseline_id":identifier_property(),
            "plan":plan_schema(),
            "idempotency_key":identifier_property(),
            "max_response_bytes":{"const":MAX_RESPONSE_BYTES},
            "runtime_write_performed":{"const":false},
            "writer_policy":{"const":WRITER_POLICY},
            "canonicalization_policy":{"const":CANONICALIZATION_POLICY},
            "input_sha256":sha256_property()
        }
    })
}

fn get_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":[
            "schema_version",
            "project_id",
            "proposal_id",
            "max_response_bytes",
            "runtime_write_performed",
            "writer_policy",
            "canonicalization_policy",
            "input_sha256"
        ],
        "properties":{
            "schema_version":{"const":GET_SCHEMA_VERSION},
            "project_id":identifier_property(),
            "proposal_id":identifier_property(),
            "max_response_bytes":{"const":MAX_RESPONSE_BYTES},
            "runtime_write_performed":{"const":false},
            "writer_policy":{"const":WRITER_POLICY},
            "canonicalization_policy":{"const":CANONICALIZATION_POLICY},
            "input_sha256":sha256_property()
        }
    })
}

fn tool_definition(tool: Tool) -> Value {
    let (description, input_schema, transaction) = match tool {
        Tool::Prepare => (
            "Prepare one Runtime-owned cumulative FormArt reviewable candidate from the exact original fresh baseline, exact current proposal base and closed registered-operation plan. This requires explicit authenticated write opt-in, but it does not approve secondary form, advance ProductionStage, confirm a candidate, create a version, export, or establish commercial quality. Runtime owns all Store/CAS scope revalidation, composition, candidate materialization and six-view FormArt evidence.",
            prepare_schema(),
            "original-fresh-baseline@1→current-proposal-base@1→registered-composite-plan@1→reviewable-candidate",
        ),
        Tool::Get => (
            "Read and restart-revalidate one exact Runtime-owned cumulative FormArt proposal by project and proposal ID. This is read-only and does not compose geometry, invoke a Worker, alter candidate state, approve FormArt, advance ProductionStage, confirm, version or export.",
            get_schema(),
            "ProductionWeaponFormArtCompositeProposalStoreRecord@1",
        ),
    };
    json!({
        "name":tool.name(),
        "description":description,
        "inputSchema":input_schema,
        "annotations":{
            "readOnlyHint":!tool.is_write(),
            "destructiveHint":false,
            "idempotentHint":true,
            "openWorldHint":false,
            "writeIntent":tool.is_write(),
            "approvalRequired":false
        },
        "_meta":{"forgecad":{
            "availability":"available",
            "runtime_method":tool.name(),
            "requiresConfirmation":false,
            "writeOptInRequired":tool.is_write(),
            "transaction":transaction,
            "maxResponseBytes":MAX_RESPONSE_BYTES,
            "definition_only":false
        }}
    })
}

pub fn read_tools() -> Vec<Value> {
    vec![tool_definition(Tool::Get)]
}

pub fn write_tools() -> Vec<Value> {
    vec![tool_definition(Tool::Prepare)]
}

pub fn summary(name: &str, value: &Value) -> Option<String> {
    let tool = from_name(name)?;
    serde_json::to_string(&json!({
        "schema_version":"ProductionWeaponFormArtCompositeProposalMcpSummary@1",
        "operation":name,
        "write_intent":if tool.is_write() {"runtime_prepare_reviewable_composite_candidate"} else {"read_only_runtime_composite_proposal"},
        "project_id":value.get("project_id"),
        "proposal_id":value.get("proposal_id"),
        "session_id":value.get("session_id"),
        "original_fresh_baseline_id":value.get("original_fresh_baseline_id"),
        "original_source_candidate_id":value.get("original_source_candidate_id"),
        "current_base_candidate_id":value.get("current_base_candidate_id"),
        "current_base_geometry_program_sha256":value.get("current_base_geometry_program_sha256"),
        "composed_geometry_program_sha256":value.get("composed_geometry_program_sha256"),
        "proposal_candidate_id":value.get("proposal_candidate_id").or_else(|| value.get("candidate_id")),
        "candidate_id":value.get("candidate_id").or_else(|| value.get("proposal_candidate_id")),
        "proposal_artifact_sha256":value.get("proposal_artifact_sha256"),
        "candidate_artifact_sha256":value.get("candidate_artifact_sha256").or_else(|| value.get("proposal_artifact_sha256")),
        "plan_object_sha256":value.get("plan_object_sha256"),
        "plan_canonical_sha256":value.get("plan_canonical_sha256"),
        "current_base_proposal_evidence_receipt_object_sha256":value.get("current_base_proposal_evidence_receipt_object_sha256"),
        "cross_view_evidence_bundle_sha256":value.get("cross_view_evidence_bundle_sha256"),
        "proposal_form_art_evidence_receipt_object_sha256":value.get("proposal_form_art_evidence_receipt_object_sha256"),
        "receipt_object_sha256":value.get("receipt_object_sha256"),
        "request_sha256":value.get("request_sha256"),
        "input_sha256":value.get("input_sha256"),
        "status":value.get("status"),
        "quality_status":value.get("quality_status"),
        "visual_quality_status":value.get("visual_quality_status"),
        "artistic_quality_status":value.get("artistic_quality_status"),
        "commercial_fps_quality_status":value.get("commercial_fps_quality_status"),
        "commercial_quality_status":value.get("commercial_quality_status"),
        "commercial_quality":value.get("commercial_quality"),
        "commercial_engine_validation":value.get("commercial_engine_validation"),
        "proposal_status":value.get("proposal_status"),
        "six_view_evaluation":value.get("six_view_evaluation"),
        "promotion_status":value.pointer("/six_view_evaluation/promotion_status").or_else(|| value.get("promotion_status")),
        "form_art_evidence_ready":value.get("proposal_form_art_evidence_ready").or_else(|| value.get("form_art_evidence_ready")),
        "reviewable_candidate":value.get("reviewable_candidate").or_else(|| value.get("candidate_created")),
        "candidate_created":value.get("candidate_created"),
        "replayed":value.get("replayed"),
        "restart_hash_verified":value.get("restart_hash_verified"),
        "candidate_confirm_allowed":value.get("candidate_confirm_allowed"),
        "promotion_eligible":value.get("promotion_eligible"),
        "promotion_allowed":value.get("promotion_allowed"),
        "commercial_quality_claim_allowed":value.get("commercial_quality_claim_allowed"),
        "visual_quality_claim":value.get("visual_quality_claim"),
        "commercial_quality_claim":value.get("commercial_quality_claim"),
        "secondary_form_approved":value.get("secondary_form_approved"),
        "production_stage_advanced":value.get("production_stage_advanced"),
        "candidate_confirmed":value.get("candidate_confirmed"),
        "version_created":value.get("version_created"),
        "export_performed":value.get("export_performed"),
        "visual_review_status":value.get("visual_review_status"),
        "human_review_status":value.get("human_review_status"),
        "commercial_engine_status":value.get("commercial_engine_status"),
        "limitations":value.get("limitations"),
        "runtime_write_performed":value.get("runtime_write_performed"),
        "persistent_user_data_touched":value.get("persistent_user_data_touched"),
        "aov_bytes_in_summary":false,
        "structured_content_complete":true
    }))
    .ok()
}
