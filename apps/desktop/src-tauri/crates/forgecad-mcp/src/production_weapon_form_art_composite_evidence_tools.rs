//! Thin MCP transport for the exact six-view evidence of a composite FormArt
//! proposal.
//!
//! The Runtime owns all candidate, baseline, camera, RenderSet, AOV and
//! FormArt Store/CAS validation. This module only publishes the closed
//! transport envelope and a bounded hash/status summary. Preparing evidence
//! is a Runtime write opt-in, but it is never approval, Stage advancement,
//! candidate confirmation, versioning, export or a commercial-quality claim.

use serde_json::{json, Map, Value};

const PREPARE_TOOL_NAME: &str = "production_weapon_form_art_composite_evidence_prepare";
const GET_TOOL_NAME: &str = "production_weapon_form_art_composite_evidence_get";
const PREPARE_SCHEMA_VERSION: &str = "ProductionWeaponFormArtCompositeEvidencePrepareRequest@1";
const GET_SCHEMA_VERSION: &str = "ProductionWeaponFormArtCompositeEvidenceGetRequest@1";
const STORE_RECORD_SCHEMA_VERSION: &str = "ProductionWeaponFormArtCompositeEvidenceRecord@1";
const PREPARE_OPERATION: &str = "forgecad.production.weapon.form-art-composite-evidence-prepare@1";
const GET_OPERATION: &str = "forgecad.production.weapon.form-art-composite-evidence-get@1";
const EVIDENCE_POLICY: &str = "composite-candidate-six-view-form-art@1";
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-input-sha256@1";
const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const IDENTIFIER_PATTERN: &str = "^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$";
const SHA256_PATTERN: &str = "^[0-9a-f]{64}$";
const VIEW_KINDS: [&str; 6] = [
    "front",
    "back",
    "left",
    "right",
    "top",
    "rear-three-quarter",
];

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "composite_evidence_id",
    "proposal_id",
    "session_id",
    "project_id",
    "composite_proposal_record_canonical_sha256",
    "composite_proposal_receipt_object_sha256",
    "original_fresh_baseline_id",
    "original_fresh_baseline_canonical_sha256",
    "source_form_art_evidence_id",
    "source_form_art_evidence_object_sha256",
    "source_form_art_evidence_canonical_sha256",
    "proposal_candidate_id",
    "proposal_candidate_state_sha256",
    "proposal_artifact_id",
    "proposal_artifact_sha256",
    "proposal_artifact_readback_object_sha256",
    "proposal_artifact_readback_sha256",
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
    "composite_evidence_id",
    "proposal_id",
    "session_id",
    "project_id",
    "composite_proposal_record_canonical_sha256",
    "composite_proposal_receipt_object_sha256",
    "original_fresh_baseline_id",
    "original_fresh_baseline_canonical_sha256",
    "source_form_art_evidence_id",
    "source_form_art_evidence_object_sha256",
    "source_form_art_evidence_canonical_sha256",
    "proposal_candidate_id",
    "proposal_candidate_state_sha256",
    "proposal_artifact_id",
    "proposal_artifact_sha256",
    "proposal_artifact_readback_object_sha256",
    "proposal_artifact_readback_sha256",
    "max_response_bytes",
    "runtime_write_performed",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];

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

    fn schema_version(self) -> &'static str {
        match self {
            Self::Get => GET_SCHEMA_VERSION,
            Self::Prepare => PREPARE_SCHEMA_VERSION,
        }
    }

    fn operation(self) -> &'static str {
        match self {
            Self::Get => GET_OPERATION,
            Self::Prepare => PREPARE_OPERATION,
        }
    }

    fn fields(self) -> &'static [&'static str] {
        match self {
            Self::Get => GET_FIELDS,
            Self::Prepare => PREPARE_FIELDS,
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

pub fn from_tool_name(name: &str) -> Option<&'static str> {
    from_name(name).map(Tool::name)
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    from_tool_name(name)
}

pub fn unavailable_error(name: &str) -> String {
    format!("PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_RUNTIME_METHOD_UNAVAILABLE: {name}")
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

fn input_schema(tool: Tool) -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":tool.schema_version()}),
    );
    properties.insert("operation".to_owned(), json!({"const":tool.operation()}));
    for field in [
        "composite_evidence_id",
        "proposal_id",
        "session_id",
        "project_id",
        "original_fresh_baseline_id",
        "source_form_art_evidence_id",
        "proposal_candidate_id",
        "proposal_artifact_id",
        "idempotency_key",
    ] {
        if tool.fields().contains(&field) {
            properties.insert(field.to_owned(), identifier_property());
        }
    }
    for field in [
        "composite_proposal_record_canonical_sha256",
        "composite_proposal_receipt_object_sha256",
        "original_fresh_baseline_canonical_sha256",
        "source_form_art_evidence_object_sha256",
        "source_form_art_evidence_canonical_sha256",
        "proposal_candidate_state_sha256",
        "proposal_artifact_sha256",
        "proposal_artifact_readback_object_sha256",
        "proposal_artifact_readback_sha256",
        "input_sha256",
    ] {
        if tool.fields().contains(&field) {
            properties.insert(field.to_owned(), sha256_property());
        }
    }
    properties.insert(
        "max_response_bytes".to_owned(),
        json!({"const":MAX_RESPONSE_BYTES}),
    );
    properties.insert("runtime_write_performed".to_owned(), json!({"const":false}));
    properties.insert("writer_policy".to_owned(), json!({"const":WRITER_POLICY}));
    properties.insert(
        "canonicalization_policy".to_owned(),
        json!({"const":CANONICALIZATION_POLICY}),
    );
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":tool.fields(),
        "properties":properties
    })
}

fn tool_definition(tool: Tool) -> Value {
    let (description, transaction) = if tool.is_write() {
        (
            "Prepare exact six-view, nine-AOV FormArt evidence for one Runtime-owned composite proposal candidate. Runtime revalidates the original fresh baseline, source FormArt, composite proposal receipt and final candidate, derives the approved six-camera RenderSets and proposal-side Part-ID/negative-space/line-flow/strict owner-void observations, and persists a reviewable evidence record. This write opt-in never approves FormArt, advances ProductionStage, confirms a candidate, creates a version, exports or claims commercial quality.",
            "composite-proposal@1→approved-fresh-baseline@1→6x9-AOV→composite-FormArt-evidence@1",
        )
    } else {
        (
            "Read and restart-revalidate one exact Runtime-owned composite FormArt evidence record by its closed proposal, baseline, source FormArt and candidate scope. This is read-only; it does not start a Worker, render, alter Store/CAS state, approve FormArt, advance ProductionStage, confirm, version or export.",
            STORE_RECORD_SCHEMA_VERSION,
        )
    };
    json!({
        "name":tool.name(),
        "description":description,
        "inputSchema":input_schema(tool),
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
            "evidence_policy":EVIDENCE_POLICY,
            "view_kinds":VIEW_KINDS,
            "aov_count_per_view":9,
            "aov_count":54,
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

/// Keep the human-readable transport summary hash-only and bounded. Complete
/// six-view/AOV rows remain in Runtime-owned structured content.
pub fn summary(name: &str, value: &Value) -> Option<String> {
    let tool = from_name(name)?;
    let lookup = |field: &str| value.get(field).cloned().unwrap_or(Value::Null);
    serde_json::to_string(&json!({
        "schema_version":"ProductionWeaponFormArtCompositeEvidenceMcpSummary@1",
        "operation":tool.name(),
        "runtime_method":tool.name(),
        "write_intent":if tool.is_write() {"runtime_prepare_composite_form_art_evidence"} else {"read_only_runtime_composite_form_art_evidence"},
        "composite_evidence_id":lookup("composite_evidence_id"),
        "proposal_id":lookup("proposal_id"),
        "session_id":lookup("session_id"),
        "project_id":lookup("project_id"),
        "composite_proposal_record_canonical_sha256":lookup("composite_proposal_record_canonical_sha256"),
        "composite_proposal_receipt_object_sha256":lookup("composite_proposal_receipt_object_sha256"),
        "original_fresh_baseline_id":lookup("original_fresh_baseline_id"),
        "original_fresh_baseline_canonical_sha256":lookup("original_fresh_baseline_canonical_sha256"),
        "source_form_art_evidence_id":lookup("source_form_art_evidence_id"),
        "source_form_art_evidence_object_sha256":lookup("source_form_art_evidence_object_sha256"),
        "source_form_art_evidence_canonical_sha256":lookup("source_form_art_evidence_canonical_sha256"),
        "proposal_candidate_id":value.get("proposal_candidate_id").cloned().or_else(|| value.get("candidate_id").cloned()).unwrap_or(Value::Null),
        "proposal_candidate_state_sha256":lookup("proposal_candidate_state_sha256"),
        "proposal_artifact_id":lookup("proposal_artifact_id"),
        "proposal_artifact_sha256":lookup("proposal_artifact_sha256"),
        "proposal_artifact_readback_object_sha256":lookup("proposal_artifact_readback_object_sha256"),
        "proposal_artifact_readback_sha256":lookup("proposal_artifact_readback_sha256"),
        "proposal_form_art_evidence_id":lookup("proposal_form_art_evidence_id"),
        "proposal_form_art_evidence_receipt_object_sha256":lookup("proposal_form_art_evidence_receipt_object_sha256"),
        "proposal_form_art_evidence_canonical_sha256":lookup("proposal_form_art_evidence_canonical_sha256"),
        "cross_view_evidence_bundle_sha256":lookup("cross_view_evidence_bundle_sha256"),
        "worker_build_cohort_sha256":lookup("worker_build_cohort_sha256"),
        "proposal_part_id_evidence_sha256":lookup("proposal_part_id_evidence_sha256"),
        "proposal_negative_space_evidence_sha256":lookup("proposal_negative_space_evidence_sha256"),
        "proposal_line_flow_evidence_sha256":lookup("proposal_line_flow_evidence_sha256"),
        "view_kinds":lookup("view_kinds"),
        "aov_count_per_view":lookup("aov_count_per_view"),
        "aov_count":lookup("aov_count"),
        "part_id_all_views_observed":lookup("part_id_all_views_observed"),
        "negative_space_all_views_resolved":lookup("negative_space_all_views_resolved"),
        "line_flow_all_views_resolved":lookup("line_flow_all_views_resolved"),
        "strict_owner_void_all_views_passed":lookup("strict_owner_void_all_views_passed"),
        "proposal_form_art_evidence_ready":lookup("proposal_form_art_evidence_ready"),
        "status":lookup("status"),
        "quality_status":lookup("quality_status"),
        "candidate_confirm_allowed":lookup("candidate_confirm_allowed"),
        "secondary_form_approved":lookup("secondary_form_approved"),
        "production_stage_advanced":lookup("production_stage_advanced"),
        "candidate_confirmed":lookup("candidate_confirmed"),
        "version_created":lookup("version_created"),
        "export_performed":lookup("export_performed"),
        "runtime_write_performed":lookup("runtime_write_performed"),
        "persistent_user_data_touched":lookup("persistent_user_data_touched"),
        "replayed":lookup("replayed"),
        "restart_hash_verified":lookup("restart_hash_verified"),
        "aov_bytes_in_summary":false,
        "structured_content_complete":true
    }))
    .ok()
}
