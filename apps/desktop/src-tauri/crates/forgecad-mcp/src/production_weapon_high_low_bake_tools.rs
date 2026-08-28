//! MCP transport definitions for the formal High/Low/HeroUV/Cage bake gate.
//!
//! This module deliberately owns only the closed MCP surface.  Runtime is
//! still the sole writer and validates the Stage@3/CAS/worker cohort bindings;
//! this adapter does not synthesize a bake receipt or downgrade to preflight.

use forgecad_runtime::is_opaque_id;
use serde_json::{json, Map, Value};

const IDENTIFIER_PATTERN: &str = "^[A-Za-z0-9_.-]{1,128}$";
const SHA256_PATTERN: &str = "^[0-9a-f]{64}$";
const TRANSACTION: &str = "ProductionWeaponHighLowBake@1";
const PREPARE_REQUEST_SCHEMA_VERSION: &str = "ProductionWeaponHighLowBakePrepareRequest@1";
const PREPARE_RESULT_SCHEMA_VERSION: &str = "ProductionWeaponHighLowBakePrepareResult@1";
const GET_REQUEST_SCHEMA_VERSION: &str = "ProductionWeaponHighLowBakeGetRequest@1";
const GET_RESULT_SCHEMA_VERSION: &str = "ProductionWeaponHighLowBakeGetResult@1";
const RECEIPT_SCHEMA_VERSION: &str = "ProductionWeaponHighLowBakeReceipt@1";
const BAKE_POLICY: &str = "production-weapon-high-low-cage-bake-gate@1";
const STAGES: &[&str] = &[
    "reference-intake",
    "reference-coverage-reviewed",
    "camera-calibrated",
    "blockout-reviewed",
    "primary-form-approved",
    "secondary-form-approved",
    "high-poly-approved",
    "low-poly-approved",
    "uv-approved",
    "cage-approved",
    "bake-approved",
    "material-approved",
    "rig-socket-approved",
    "animation-approved",
    "vfx-approved",
    "lod-collision-approved",
    "hero-art-review-approved",
    "engine-validated",
    "export-confirmed",
];
const GATE_SCOPES: &[&str] = &[
    "high-artifact",
    "low-artifact",
    "cage-artifact",
    "high-low-bake",
];
const SOURCE_STAGES: &[&str] = &[
    "secondary-form-approved",
    "high-poly-approved",
    "low-poly-approved",
    "cage-approved",
];
const TARGET_STAGES: &[&str] = &[
    "high-poly-approved",
    "low-poly-approved",
    "cage-approved",
    "bake-approved",
];
const STRUCTURAL_STATUSES: &[&str] = &["NOT_RUN", "BLOCKED", "PASS_SOURCE_STRUCTURAL"];
const VISUAL_STATUSES: &[&str] = &["NOT_RUN", "BLOCKED", "QUALITY_TARGET_NOT_MET", "NOT_PROVEN"];
const HUMAN_STATUSES: &[&str] = &["NOT_RUN", "BLOCKED", "REJECTED", "PASS_HUMAN_ART_REVIEW"];
const ENGINE_STATUSES: &[&str] = &["NOT_RUN", "BLOCKED", "FAILED", "PASS_ENGINE_VALIDATION"];
const DISTRIBUTION_STATUSES: &[&str] = &["NOT_RUN", "BLOCKED", "FAILED", "PASS_DISTRIBUTION"];
const BAKE_STATUSES: &[&str] = &[
    "NOT_HIGH_LOW_BAKE",
    "DIAGNOSTIC_ONLY",
    "PASS_SOURCE_STRUCTURAL",
];

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "bake_receipt_id",
    "session_id",
    "project_id",
    "gate_scope",
    "source_stage",
    "target_stage",
    "source_stage_head_transition_id",
    "source_stage_head_transition_sha256",
    "source_stage_head_canonical_sha256",
    "source_stage_head_stage",
    "high_candidate_id",
    "high_candidate_state_sha256",
    "high_artifact_id",
    "high_artifact_sha256",
    "high_artifact_readback_sha256",
    "low_candidate_id",
    "low_candidate_state_sha256",
    "low_artifact_id",
    "low_artifact_sha256",
    "low_artifact_readback_sha256",
    "cage_artifact_id",
    "cage_artifact_sha256",
    "cage_artifact_readback_sha256",
    "correspondence_id",
    "correspondence_object_sha256",
    "correspondence_canonical_sha256",
    "bake_plan_id",
    "bake_plan_object_sha256",
    "bake_plan_canonical_sha256",
    "bake_policy",
    "bake_policy_sha256",
    "input_sha256",
    "idempotency_key",
];

const GET_FIELDS: &[&str] = &[
    "schema_version",
    "bake_receipt_id",
    "session_id",
    "project_id",
    "gate_scope",
];

const RESULT_FIELDS: &[&str] = &[
    "schema_version",
    "bake_receipt_id",
    "bake_receipt_object_sha256",
    "bake_receipt",
    "replayed",
    "restart_hash_verified",
    "runtime_write",
    "production_stage_advanced",
    "candidate_confirmed",
    "version_created",
    "export_performed",
];

const RECEIPT_FIELDS: &[&str] = &[
    "schema_version",
    "bake_receipt_id",
    "session_id",
    "project_id",
    "gate_scope",
    "source_stage",
    "target_stage",
    "source_stage_head_transition_id",
    "source_stage_head_transition_sha256",
    "source_stage_head_canonical_sha256",
    "source_stage_head_stage",
    "high_candidate_id",
    "high_candidate_state_sha256",
    "high_artifact_id",
    "high_artifact_sha256",
    "high_artifact_readback_sha256",
    "low_candidate_id",
    "low_candidate_state_sha256",
    "low_artifact_id",
    "low_artifact_sha256",
    "low_artifact_readback_sha256",
    "cage_artifact_id",
    "cage_artifact_sha256",
    "cage_artifact_readback_sha256",
    "correspondence_id",
    "correspondence_object_sha256",
    "correspondence_canonical_sha256",
    "bake_plan_id",
    "bake_plan_object_sha256",
    "bake_plan_canonical_sha256",
    "diagnostic_id",
    "diagnostic_object_sha256",
    "diagnostic_canonical_sha256",
    "bake_policy",
    "bake_policy_sha256",
    "high_status",
    "low_status",
    "cage_status",
    "correspondence_status",
    "diagnostic_status",
    "high_low_bake_status",
    "bake_output_object_sha256s",
    "hard_gate",
    "stage_advance_allowed",
    "limitations",
    "validator_status",
    "structural_status",
    "visual_status",
    "human_status",
    "engine_status",
    "distribution_status",
    "quality_status",
    "hard_gate_passed",
    "runtime_write_performed",
    "production_stage_advanced",
    "candidate_confirmed",
    "version_created",
    "export_performed",
    "request_sha256",
    "input_sha256",
    "receipt_object_sha256",
    "canonical_sha256",
    "created_at",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionWeaponHighLowBakeTool {
    Get,
    Prepare,
}

impl ProductionWeaponHighLowBakeTool {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Get => "production_weapon_high_low_bake_get",
            Self::Prepare => "production_weapon_high_low_bake_prepare",
        }
    }

    pub const fn is_write(self) -> bool {
        matches!(self, Self::Prepare)
    }

    pub const fn runtime_method(self) -> &'static str {
        self.name()
    }
}

pub fn from_name(name: &str) -> Option<ProductionWeaponHighLowBakeTool> {
    Some(match name {
        "production_weapon_high_low_bake_get" => ProductionWeaponHighLowBakeTool::Get,
        "production_weapon_high_low_bake_prepare" => ProductionWeaponHighLowBakeTool::Prepare,
        _ => return None,
    })
}

pub fn is_tool(name: &str) -> bool {
    from_name(name).is_some()
}

pub fn is_write_tool(name: &str) -> bool {
    from_name(name).is_some_and(ProductionWeaponHighLowBakeTool::is_write)
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    from_name(name).map(ProductionWeaponHighLowBakeTool::runtime_method)
}

pub fn unavailable_error(name: &str) -> String {
    format!(
        "PRODUCTION_WEAPON_HIGH_LOW_BAKE_RUNTIME_METHOD_UNAVAILABLE: {name} requires Runtime method {name}"
    )
}

pub fn read_tool_names() -> Vec<String> {
    vec![ProductionWeaponHighLowBakeTool::Get.name().to_owned()]
}

pub fn write_tool_names() -> Vec<String> {
    vec![ProductionWeaponHighLowBakeTool::Prepare.name().to_owned()]
}

pub fn read_tools() -> Vec<Value> {
    vec![tool_definition(ProductionWeaponHighLowBakeTool::Get)]
}

pub fn write_tools() -> Vec<Value> {
    vec![tool_definition(ProductionWeaponHighLowBakeTool::Prepare)]
}

fn tool_definition(tool: ProductionWeaponHighLowBakeTool) -> Value {
    let (description, input_schema) = match tool {
        ProductionWeaponHighLowBakeTool::Get => (
            "Read one exact Runtime-owned High/Low/HeroUV/Cage bake receipt by session, project and gate scope. This is read-only and never invokes a Worker, writes CAS/SQLite, advances Stage@3, confirms a candidate, creates a version or exports.",
            get_schema(),
        ),
        ProductionWeaponHighLowBakeTool::Prepare => (
            "Prepare one Runtime-owned formal High/Low/HeroUV/Cage bake receipt from existing hash-bound artifacts and fixed Worker/CAS/canonical inputs. Explicit MCP write opt-in is required; this prepare never advances Stage@3, confirms a candidate, creates a version or exports, and does not claim visual, human, engine or distribution quality.",
            prepare_schema(),
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
            "runtime_method":tool.runtime_method(),
            "requiresConfirmation":false,
            "transaction":TRANSACTION,
            "definition_only":false
        }}
    })
}

fn object_schema(required: &[&str], properties: Map<String, Value>) -> Value {
    json!({
        "type":"object",
        "required":required,
        "properties":properties,
        "additionalProperties":false
    })
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

fn enum_property(values: &[&str]) -> Value {
    json!({"type":"string","enum":values})
}

fn prepare_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":PREPARE_REQUEST_SCHEMA_VERSION}),
    );
    for field in [
        "bake_receipt_id",
        "session_id",
        "project_id",
        "source_stage_head_transition_id",
        "high_candidate_id",
        "high_artifact_id",
        "low_candidate_id",
        "low_artifact_id",
        "cage_artifact_id",
        "correspondence_id",
        "bake_plan_id",
        "idempotency_key",
    ] {
        properties.insert(field.to_owned(), identifier_property());
    }
    for field in [
        "source_stage_head_transition_sha256",
        "source_stage_head_canonical_sha256",
        "high_candidate_state_sha256",
        "high_artifact_sha256",
        "high_artifact_readback_sha256",
        "low_candidate_state_sha256",
        "low_artifact_sha256",
        "low_artifact_readback_sha256",
        "cage_artifact_sha256",
        "cage_artifact_readback_sha256",
        "correspondence_object_sha256",
        "correspondence_canonical_sha256",
        "bake_plan_object_sha256",
        "bake_plan_canonical_sha256",
        "bake_policy_sha256",
        "input_sha256",
    ] {
        properties.insert(field.to_owned(), sha256_property());
    }
    properties.insert("gate_scope".to_owned(), enum_property(GATE_SCOPES));
    properties.insert("source_stage".to_owned(), enum_property(SOURCE_STAGES));
    properties.insert("target_stage".to_owned(), enum_property(TARGET_STAGES));
    properties.insert("source_stage_head_stage".to_owned(), enum_property(STAGES));
    properties.insert("bake_policy".to_owned(), json!({"const":BAKE_POLICY}));
    object_schema(PREPARE_FIELDS, properties)
}

fn get_schema() -> Value {
    object_schema(
        GET_FIELDS,
        Map::from_iter([
            (
                "schema_version".to_owned(),
                json!({"const":GET_REQUEST_SCHEMA_VERSION}),
            ),
            ("bake_receipt_id".to_owned(), identifier_property()),
            ("session_id".to_owned(), identifier_property()),
            ("project_id".to_owned(), identifier_property()),
            ("gate_scope".to_owned(), enum_property(GATE_SCOPES)),
        ]),
    )
}

fn exact_object<'a>(
    value: &'a Value,
    fields: &[&str],
    kind: &str,
) -> Result<&'a Map<String, Value>, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{kind} response must be an object"))?;
    if let Some(field) = object
        .keys()
        .find(|field| !fields.contains(&field.as_str()))
    {
        return Err(format!(
            "{kind} response contains unsupported field {field}"
        ));
    }
    if let Some(field) = fields.iter().find(|field| !object.contains_key(**field)) {
        return Err(format!("{kind} response is missing {field}"));
    }
    Ok(object)
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn require_id<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    let value = value
        .as_str()
        .ok_or_else(|| format!("ProductionWeaponHighLowBake response {field} must be a string"))?;
    if !is_opaque_id(value) {
        return Err(format!(
            "ProductionWeaponHighLowBake response {field} identity is invalid"
        ));
    }
    Ok(value)
}

fn require_sha<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    let value = value
        .as_str()
        .ok_or_else(|| format!("ProductionWeaponHighLowBake response {field} must be a string"))?;
    if !is_sha256(value) {
        return Err(format!(
            "ProductionWeaponHighLowBake response {field} SHA-256 is invalid"
        ));
    }
    Ok(value)
}

fn validate_receipt(
    value: &Value,
    result_receipt_id: &str,
    result_receipt_sha256: &str,
) -> Result<(), String> {
    let receipt = exact_object(
        value,
        RECEIPT_FIELDS,
        "ProductionWeaponHighLowBake bake_receipt",
    )?;
    let schema_version = receipt
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "ProductionWeaponHighLowBake receipt schema_version is missing".to_owned()
        })?;
    if schema_version != RECEIPT_SCHEMA_VERSION {
        return Err("ProductionWeaponHighLowBake receipt schema differs".to_owned());
    }
    let receipt_id = require_id(
        receipt.get("bake_receipt_id").ok_or_else(|| {
            "ProductionWeaponHighLowBake receipt bake_receipt_id is missing".to_owned()
        })?,
        "bake_receipt_id",
    )?;
    if receipt_id != result_receipt_id {
        return Err("ProductionWeaponHighLowBake response receipt id differs".to_owned());
    }
    require_sha(
        &Value::String(result_receipt_sha256.to_owned()),
        "bake_receipt_object_sha256",
    )?;
    if receipt.get("receipt_object_sha256").and_then(Value::as_str) != Some(result_receipt_sha256) {
        return Err("ProductionWeaponHighLowBake response receipt object hash differs".to_owned());
    }
    for field in [
        "session_id",
        "project_id",
        "source_stage_head_transition_id",
        "high_candidate_id",
        "high_artifact_id",
        "low_candidate_id",
        "low_artifact_id",
        "cage_artifact_id",
        "correspondence_id",
        "bake_plan_id",
        "diagnostic_id",
    ] {
        require_id(
            receipt
                .get(field)
                .ok_or_else(|| format!("ProductionWeaponHighLowBake receipt {field} is missing"))?,
            field,
        )?;
    }
    for field in [
        "source_stage_head_transition_sha256",
        "source_stage_head_canonical_sha256",
        "high_candidate_state_sha256",
        "high_artifact_sha256",
        "high_artifact_readback_sha256",
        "low_candidate_state_sha256",
        "low_artifact_sha256",
        "low_artifact_readback_sha256",
        "cage_artifact_sha256",
        "cage_artifact_readback_sha256",
        "correspondence_object_sha256",
        "correspondence_canonical_sha256",
        "bake_plan_object_sha256",
        "bake_plan_canonical_sha256",
        "diagnostic_object_sha256",
        "diagnostic_canonical_sha256",
        "bake_policy_sha256",
        "request_sha256",
        "input_sha256",
        "receipt_object_sha256",
        "canonical_sha256",
    ] {
        require_sha(
            receipt
                .get(field)
                .ok_or_else(|| format!("ProductionWeaponHighLowBake receipt {field} is missing"))?,
            field,
        )?;
    }
    let gate_scope = receipt
        .get("gate_scope")
        .and_then(Value::as_str)
        .ok_or_else(|| "ProductionWeaponHighLowBake receipt gate_scope is missing".to_owned())?;
    if !GATE_SCOPES.contains(&gate_scope) {
        return Err("ProductionWeaponHighLowBake receipt gate_scope is invalid".to_owned());
    }
    for (field, allowed) in [
        ("source_stage", SOURCE_STAGES),
        ("target_stage", TARGET_STAGES),
        ("source_stage_head_stage", STAGES),
    ] {
        let stage = receipt
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("ProductionWeaponHighLowBake receipt {field} is missing"))?;
        if !allowed.contains(&stage) {
            return Err(format!(
                "ProductionWeaponHighLowBake receipt {field} is invalid"
            ));
        }
    }
    let gate_index = GATE_SCOPES
        .iter()
        .position(|scope| *scope == gate_scope)
        .expect("gate scope was checked above");
    if receipt.get("source_stage").and_then(Value::as_str) != Some(SOURCE_STAGES[gate_index])
        || receipt.get("target_stage").and_then(Value::as_str) != Some(TARGET_STAGES[gate_index])
        || receipt
            .get("source_stage_head_stage")
            .and_then(Value::as_str)
            != Some(SOURCE_STAGES[gate_index])
    {
        return Err("ProductionWeaponHighLowBake receipt gate/stage binding differs".to_owned());
    }
    let bake_policy = receipt
        .get("bake_policy")
        .and_then(Value::as_str)
        .ok_or_else(|| "ProductionWeaponHighLowBake receipt bake_policy is missing".to_owned())?;
    if bake_policy != BAKE_POLICY {
        return Err("ProductionWeaponHighLowBake receipt policy differs".to_owned());
    }
    for (field, allowed) in [
        ("high_status", STRUCTURAL_STATUSES),
        ("low_status", STRUCTURAL_STATUSES),
        ("cage_status", STRUCTURAL_STATUSES),
        ("correspondence_status", STRUCTURAL_STATUSES),
        ("diagnostic_status", STRUCTURAL_STATUSES),
        ("structural_status", STRUCTURAL_STATUSES),
        ("visual_status", VISUAL_STATUSES),
        ("human_status", HUMAN_STATUSES),
        ("engine_status", ENGINE_STATUSES),
        ("distribution_status", DISTRIBUTION_STATUSES),
        ("high_low_bake_status", BAKE_STATUSES),
    ] {
        let status = receipt
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("ProductionWeaponHighLowBake receipt {field} is missing"))?;
        if !allowed.contains(&status) {
            return Err(format!(
                "ProductionWeaponHighLowBake receipt {field} is invalid"
            ));
        }
    }
    let hard_gate = receipt
        .get("hard_gate")
        .and_then(Value::as_object)
        .ok_or_else(|| "ProductionWeaponHighLowBake receipt hard_gate is missing".to_owned())?;
    for field in [
        "distinct_high_low_cage_bindings",
        "high_readback_verified",
        "low_readback_verified",
        "cage_readback_verified",
        "low_authoring_topology_verified",
        "correspondence_verified",
        "uv_tangent_binding_verified",
        "ray_diagnostic_verified",
        "no_candidate_surface_bake_reuse",
        "same_cohort_replay_verified",
        "output_byte_exact",
    ] {
        if hard_gate.get(field).and_then(Value::as_bool).is_none() {
            return Err(format!(
                "ProductionWeaponHighLowBake receipt hard_gate {field} is missing"
            ));
        }
    }
    if hard_gate
        .get("distinct_high_low_cage_bindings")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Err(
            "ProductionWeaponHighLowBake response lacks distinct High/Low/Cage binding proof"
                .to_owned(),
        );
    }
    if hard_gate
        .get("no_candidate_surface_bake_reuse")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Err(
            "ProductionWeaponHighLowBake response permits candidate-surface bake reuse".to_owned(),
        );
    }
    for field in [
        "hard_gate_passed",
        "runtime_write_performed",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
    ] {
        if receipt.get(field).and_then(Value::as_bool).is_none() {
            return Err(format!(
                "ProductionWeaponHighLowBake receipt {field} is missing"
            ));
        }
    }
    let output_objects = receipt
        .get("bake_output_object_sha256s")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "ProductionWeaponHighLowBake receipt bake output objects are missing".to_owned()
        })?;
    for output in output_objects {
        require_sha(output, "bake_output_object_sha256")?;
    }
    if receipt
        .get("limitations")
        .and_then(Value::as_array)
        .is_none_or(|limitations| limitations.is_empty())
    {
        return Err("ProductionWeaponHighLowBake receipt limitations are missing".to_owned());
    }
    for field in [
        "stage_advance_allowed",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
    ] {
        if receipt.get(field).and_then(Value::as_bool) != Some(false) {
            return Err(format!(
                "ProductionWeaponHighLowBake receipt {field} must remain false"
            ));
        }
    }
    Ok(())
}

/// Validate the closed Runtime result before MCP exposes it as structured
/// content.  This keeps stage/confirm/version/export invariants at the wire
/// boundary even when an older Runtime is accidentally selected.
pub fn validate_response(name: &str, value: &Value) -> Result<(), String> {
    let tool = from_name(name).ok_or_else(|| format!("unknown HighLowBake tool {name}"))?;
    let object = exact_object(value, RESULT_FIELDS, "ProductionWeaponHighLowBake")?;
    let expected_schema = if tool.is_write() {
        PREPARE_RESULT_SCHEMA_VERSION
    } else {
        GET_RESULT_SCHEMA_VERSION
    };
    if object.get("schema_version").and_then(Value::as_str) != Some(expected_schema) {
        return Err("ProductionWeaponHighLowBake response schema differs".to_owned());
    }
    let result_receipt_id = require_id(
        object
            .get("bake_receipt_id")
            .expect("closed response has bake_receipt_id"),
        "bake_receipt_id",
    )?;
    let result_receipt_sha256 = require_sha(
        object
            .get("bake_receipt_object_sha256")
            .expect("closed response has bake_receipt_object_sha256"),
        "bake_receipt_object_sha256",
    )?;
    validate_receipt(
        object
            .get("bake_receipt")
            .expect("closed response has bake_receipt"),
        result_receipt_id,
        result_receipt_sha256,
    )?;
    if object.get("restart_hash_verified").and_then(Value::as_bool) != Some(true) {
        return Err("ProductionWeaponHighLowBake response restart hash is not verified".to_owned());
    }
    for field in [
        "replayed",
        "restart_hash_verified",
        "runtime_write",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
    ] {
        if object.get(field).and_then(Value::as_bool).is_none() {
            return Err(format!(
                "ProductionWeaponHighLowBake response {field} must be boolean"
            ));
        }
    }
    if !tool.is_write() && object.get("runtime_write").and_then(Value::as_bool) != Some(false) {
        return Err(
            "ProductionWeaponHighLowBake get response runtime_write must be false".to_owned(),
        );
    }
    for field in [
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
    ] {
        if object.get(field).and_then(Value::as_bool) != Some(false) {
            return Err(format!(
                "ProductionWeaponHighLowBake response {field} must remain false"
            ));
        }
    }
    if !tool.is_write()
        && object
            .get("bake_receipt")
            .and_then(|value| value.get("runtime_write_performed"))
            .and_then(Value::as_bool)
            != Some(false)
    {
        return Err(
            "ProductionWeaponHighLowBake get receipt runtime_write_performed must be false"
                .to_owned(),
        );
    }
    if object.get("runtime_write").and_then(Value::as_bool)
        != object
            .get("bake_receipt")
            .and_then(|value| value.get("runtime_write_performed"))
            .and_then(Value::as_bool)
    {
        return Err(
            "ProductionWeaponHighLowBake response runtime-write binding differs".to_owned(),
        );
    }
    Ok(())
}

pub fn summary(name: &str, value: &Value) -> Option<String> {
    let tool = from_name(name)?;
    let lookup = |field: &str| value.get(field).cloned().unwrap_or(Value::Null);
    let receipt_lookup = |field: &str| {
        value
            .get(field)
            .cloned()
            .or_else(|| value.pointer(&format!("/bake_receipt/{field}")).cloned())
            .unwrap_or(Value::Null)
    };
    let summary = json!({
        "schema_version":"ProductionWeaponHighLowBakeMcpSummary@1",
        "tool":tool.name(),
        "runtime_method":tool.runtime_method(),
        "write_intent":if tool.is_write() { "explicit_runtime_production_weapon_high_low_bake_prepare_write" } else { "read_only_runtime_production_weapon_high_low_bake_get" },
        "result_schema_version":lookup("schema_version"),
        "bake_receipt_id":receipt_lookup("bake_receipt_id"),
        "bake_receipt_object_sha256":lookup("bake_receipt_object_sha256"),
        "session_id":receipt_lookup("session_id"),
        "project_id":receipt_lookup("project_id"),
        "gate_scope":receipt_lookup("gate_scope"),
        "source_stage":receipt_lookup("source_stage"),
        "target_stage":receipt_lookup("target_stage"),
        "source_stage_head_transition_id":receipt_lookup("source_stage_head_transition_id"),
        "source_stage_head_transition_sha256":receipt_lookup("source_stage_head_transition_sha256"),
        "source_stage_head_canonical_sha256":receipt_lookup("source_stage_head_canonical_sha256"),
        "high_candidate_id":receipt_lookup("high_candidate_id"),
        "low_candidate_id":receipt_lookup("low_candidate_id"),
        "high_artifact_id":receipt_lookup("high_artifact_id"),
        "low_artifact_id":receipt_lookup("low_artifact_id"),
        "cage_artifact_id":receipt_lookup("cage_artifact_id"),
        "high_low_bake_status":receipt_lookup("high_low_bake_status"),
        "structural_status":receipt_lookup("structural_status"),
        "visual_status":receipt_lookup("visual_status"),
        "human_status":receipt_lookup("human_status"),
        "engine_status":receipt_lookup("engine_status"),
        "distribution_status":receipt_lookup("distribution_status"),
        "quality_status":receipt_lookup("quality_status"),
        "hard_gate_passed":receipt_lookup("hard_gate_passed"),
        "replayed":lookup("replayed"),
        "restart_hash_verified":lookup("restart_hash_verified"),
        "runtime_write":lookup("runtime_write"),
        "production_stage_advanced":lookup("production_stage_advanced"),
        "candidate_confirmed":lookup("candidate_confirmed"),
        "version_created":lookup("version_created"),
        "export_performed":lookup("export_performed"),
        "glb_bytes_in_summary":false,
        "png_bytes_in_summary":false,
        "structured_content_complete":true
    });
    serde_json::to_string(&summary).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definitions_are_closed_and_split_read_write() {
        let read = read_tools();
        let write = write_tools();
        assert_eq!(read.len(), 1);
        assert_eq!(write.len(), 1);
        assert_eq!(read[0]["name"], "production_weapon_high_low_bake_get");
        assert_eq!(write[0]["name"], "production_weapon_high_low_bake_prepare");
        assert_eq!(read[0]["annotations"]["readOnlyHint"], true);
        assert_eq!(write[0]["annotations"]["readOnlyHint"], false);
        assert_eq!(read[0]["_meta"]["forgecad"]["requiresConfirmation"], false);
        assert_eq!(write[0]["_meta"]["forgecad"]["requiresConfirmation"], false);
        assert_eq!(read[0]["inputSchema"]["additionalProperties"], false);
        assert_eq!(write[0]["inputSchema"]["additionalProperties"], false);
        assert!(is_write_tool("production_weapon_high_low_bake_prepare"));
        assert!(!is_write_tool("production_weapon_high_low_bake_get"));
    }

    #[test]
    fn response_validation_rejects_stage_mutation_and_unknown_fields() {
        let mut response = json!({
            "schema_version":GET_RESULT_SCHEMA_VERSION,
            "bake_receipt_id":"bake-receipt-1",
            "bake_receipt_object_sha256":"e".repeat(64),
            "bake_receipt": {
            "schema_version":RECEIPT_SCHEMA_VERSION,
                "bake_receipt_id":"bake-receipt-1",
                "session_id":"session-1","project_id":"project-1","gate_scope":"high-artifact",
                "source_stage":"secondary-form-approved","target_stage":"high-poly-approved",
                "source_stage_head_transition_id":"transition-1","source_stage_head_transition_sha256":"b".repeat(64),"source_stage_head_canonical_sha256":"c".repeat(64),"source_stage_head_stage":"secondary-form-approved",
                "high_candidate_id":"high-candidate-1","high_candidate_state_sha256":"d".repeat(64),"high_artifact_id":"high-artifact-1","high_artifact_sha256":"e".repeat(64),"high_artifact_readback_sha256":"f".repeat(64),
                "low_candidate_id":"low-candidate-1","low_candidate_state_sha256":"0".repeat(64),"low_artifact_id":"low-artifact-1","low_artifact_sha256":"1".repeat(64),"low_artifact_readback_sha256":"2".repeat(64),
                "cage_artifact_id":"cage-artifact-1","cage_artifact_sha256":"3".repeat(64),"cage_artifact_readback_sha256":"4".repeat(64),
                "correspondence_id":"correspondence-1","correspondence_object_sha256":"5".repeat(64),"correspondence_canonical_sha256":"6".repeat(64),
                "bake_plan_id":"bake-plan-1","bake_plan_object_sha256":"7".repeat(64),"bake_plan_canonical_sha256":"8".repeat(64),"diagnostic_id":"diagnostic-1","diagnostic_object_sha256":"9".repeat(64),"diagnostic_canonical_sha256":"a".repeat(64),
                "bake_policy":BAKE_POLICY,"bake_policy_sha256":"b".repeat(64),
                "high_status":"PASS_SOURCE_STRUCTURAL","low_status":"PASS_SOURCE_STRUCTURAL","cage_status":"PASS_SOURCE_STRUCTURAL","correspondence_status":"PASS_SOURCE_STRUCTURAL","diagnostic_status":"PASS_SOURCE_STRUCTURAL","high_low_bake_status":"PASS_SOURCE_STRUCTURAL",
                "bake_output_object_sha256s":[],"hard_gate":{"distinct_high_low_cage_bindings":true,"high_readback_verified":true,"low_readback_verified":true,"cage_readback_verified":true,"low_authoring_topology_verified":true,"correspondence_verified":true,"uv_tangent_binding_verified":true,"ray_diagnostic_verified":true,"no_candidate_surface_bake_reuse":true,"same_cohort_replay_verified":true,"output_byte_exact":true},"hard_gate_passed":true,"validator_status":"PASS","structural_status":"PASS_SOURCE_STRUCTURAL","visual_status":"NOT_PROVEN","human_status":"NOT_RUN","engine_status":"NOT_RUN","distribution_status":"NOT_RUN","quality_status":"structural_only","runtime_write_performed":false,"stage_advance_allowed":false,"production_stage_advanced":false,"candidate_confirmed":false,"version_created":false,"export_performed":false,"limitations":["structural-only"],"request_sha256":"c".repeat(64),"input_sha256":"d".repeat(64),"receipt_object_sha256":"e".repeat(64),"canonical_sha256":"f".repeat(64),"created_at":"2026-01-01T00:00:00Z"
            },
            "replayed":false,"restart_hash_verified":true,"runtime_write":false,"production_stage_advanced":false,"candidate_confirmed":false,"version_created":false,"export_performed":false
        });
        assert!(validate_response("production_weapon_high_low_bake_get", &response).is_ok());
        response["unexpected"] = Value::Bool(true);
        assert!(validate_response("production_weapon_high_low_bake_get", &response).is_err());
    }
}
