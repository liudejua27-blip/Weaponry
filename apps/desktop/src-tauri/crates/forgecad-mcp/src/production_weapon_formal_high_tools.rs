//! Closed MCP transport surface for the Runtime-owned Formal High slice.
//!
//! The public wrapper contracts deliberately contain no `operation` field.
//! The MCP adapter therefore keeps the exact contract field sets here and
//! uses the tool name only for dispatch/summary classification. Runtime owns
//! all derived candidate, artifact, readback and receipt identities.

use forgecad_runtime::{canonical_json_hash, sha256_hex};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const IDENTIFIER_PATTERN: &str = "^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$";
const SHA256_PATTERN: &str = "^[0-9a-f]{64}$";
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const TRANSACTION: &str = "ProductionWeaponFormalHigh@1";
const PREPARE_REQUEST_SCHEMA_VERSION: &str = "ProductionWeaponFormalHighPrepareRequest@1";
const PREPARE_RESULT_SCHEMA_VERSION: &str = "ProductionWeaponFormalHighPrepareResult@1";
const GET_REQUEST_SCHEMA_VERSION: &str = "ProductionWeaponFormalHighGetRequest@1";
const GET_RESULT_SCHEMA_VERSION: &str = "ProductionWeaponFormalHighGetResult@1";
const CANDIDATE_SCHEMA_VERSION: &str = "Candidate@1";
const HIGH_SCHEMA_VERSION: &str = "ProductionWeaponHighArtifact@1";
const HIGH_POLICY: &str = "production-weapon-independent-high-detail-graph@1";
const HIGH_ARTIFACT_KIND: &str = "production-weapon-high-artifact-glb";
const HIGH_MIME: &str = "model/gltf-binary";
const MAX_HIGH_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "source_stage_head_transition_id",
    "source_stage_head_transition_sha256",
    "source_stage_head_canonical_sha256",
    "high_candidate_id",
    "idempotency_key",
    "max_response_bytes",
    "writer_policy",
    "input_sha256",
];

const GET_FIELDS: &[&str] = &[
    "schema_version",
    "project_id",
    "session_id",
    "high_artifact_id",
    "high_candidate_id",
];

const RESULT_FIELDS: &[&str] = &[
    "schema_version",
    "candidate",
    "high",
    "replayed",
    "runtime_write",
    "restart_hash_verified",
    "production_stage_advanced",
    "candidate_confirmed",
    "version_created",
    "export_performed",
];

const CANDIDATE_FIELDS: &[&str] = &[
    "schema_version",
    "candidate_id",
    "project_id",
    "base_version_id",
    "source_version_id",
    "prepared_object_id",
    "prepared_object_sha256",
    "state",
    "request_sha256",
    "manifest_hash",
    "quality_report_id",
    "quality_hard_gate_passed",
    "canonical_sha256",
    "error_code",
    "created_at",
    "updated_at",
];

const HIGH_FIELDS: &[&str] = &[
    "schema_version",
    "high_artifact_id",
    "source_candidate_id",
    "source_candidate_state_sha256",
    "source_artifact_id",
    "source_artifact_sha256",
    "source_artifact_readback_sha256",
    "high_candidate_id",
    "high_candidate_state_sha256",
    "high_artifact_sha256",
    "high_artifact_readback_sha256",
    "high_artifact_readback_object_sha256",
    "high_geometry_program_sha256",
    "high_geometry_program_object_sha256",
    "high_geometry_candidate_evidence_sha256",
    "high_detail_graph_object_sha256",
    "high_detail_graph_canonical_sha256",
    "high_part_inventory_sha256",
    "high_part_ids",
    "high_material_zone_ids",
    "high_policy",
    "high_policy_sha256",
    "high_artifact_kind",
    "high_mime",
    "high_size_bytes",
    "high_worker_algorithm_sha256",
    "high_worker_build_cohort_sha256",
    "high_worker_replay_count",
    "high_replay_byte_exact",
    "high_topology_status",
    "high_authoring_topology_status",
    "high_uv_status",
    "high_tangent_status",
    "session_id",
    "project_id",
    "source_stage_head_transition_id",
    "source_stage_head_transition_sha256",
    "source_stage_head_canonical_sha256",
    "source_stage_head_stage",
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
pub enum ProductionWeaponFormalHighTool {
    Get,
    Prepare,
}

impl ProductionWeaponFormalHighTool {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Get => "production_weapon_formal_high_get",
            Self::Prepare => "production_weapon_formal_high_prepare",
        }
    }

    pub const fn is_write(self) -> bool {
        matches!(self, Self::Prepare)
    }

    pub const fn runtime_method(self) -> &'static str {
        self.name()
    }
}

pub fn from_name(name: &str) -> Option<ProductionWeaponFormalHighTool> {
    Some(match name {
        "production_weapon_formal_high_get" => ProductionWeaponFormalHighTool::Get,
        "production_weapon_formal_high_prepare" => ProductionWeaponFormalHighTool::Prepare,
        _ => return None,
    })
}

pub fn is_tool(name: &str) -> bool {
    from_name(name).is_some()
}

pub fn is_write_tool(name: &str) -> bool {
    from_name(name).is_some_and(ProductionWeaponFormalHighTool::is_write)
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    from_name(name).map(ProductionWeaponFormalHighTool::runtime_method)
}

pub fn unavailable_error(name: &str) -> String {
    format!(
        "PRODUCTION_WEAPON_FORMAL_HIGH_RUNTIME_METHOD_UNAVAILABLE: {name} requires Runtime method {name}"
    )
}

pub fn read_tool_names() -> Vec<String> {
    vec![ProductionWeaponFormalHighTool::Get.name().to_owned()]
}

pub fn write_tool_names() -> Vec<String> {
    vec![ProductionWeaponFormalHighTool::Prepare.name().to_owned()]
}

pub fn read_tools() -> Vec<Value> {
    vec![tool_definition(ProductionWeaponFormalHighTool::Get)]
}

pub fn write_tools() -> Vec<Value> {
    vec![tool_definition(ProductionWeaponFormalHighTool::Prepare)]
}

fn tool_definition(tool: ProductionWeaponFormalHighTool) -> Value {
    let (description, input_schema) = match tool {
        ProductionWeaponFormalHighTool::Get => (
            "Read one exact Runtime-owned Formal High candidate and High artifact record by project, session, artifact and candidate identities. This read is restart-verified and never advances a stage, confirms a candidate, creates a version or exports.",
            get_schema(),
        ),
        ProductionWeaponFormalHighTool::Prepare => (
            "Prepare one Runtime-owned Formal High candidate and High artifact from an exact secondary-form Stage head. Runtime derives the candidate state, High artifact, readback and receipt identities; explicit MCP write opt-in is required and no stage, confirmation, version or export is performed.",
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

fn get_schema() -> Value {
    object_schema(
        GET_FIELDS,
        Map::from_iter([
            (
                "schema_version".to_owned(),
                json!({"const":GET_REQUEST_SCHEMA_VERSION}),
            ),
            ("project_id".to_owned(), identifier_property()),
            ("session_id".to_owned(), identifier_property()),
            ("high_artifact_id".to_owned(), identifier_property()),
            ("high_candidate_id".to_owned(), identifier_property()),
        ]),
    )
}

fn prepare_schema() -> Value {
    object_schema(
        PREPARE_FIELDS,
        Map::from_iter([
            (
                "schema_version".to_owned(),
                json!({"const":PREPARE_REQUEST_SCHEMA_VERSION}),
            ),
            (
                "source_stage_head_transition_id".to_owned(),
                identifier_property(),
            ),
            (
                "source_stage_head_transition_sha256".to_owned(),
                sha256_property(),
            ),
            (
                "source_stage_head_canonical_sha256".to_owned(),
                sha256_property(),
            ),
            ("high_candidate_id".to_owned(), identifier_property()),
            ("idempotency_key".to_owned(), identifier_property()),
            (
                "max_response_bytes".to_owned(),
                json!({"const":MAX_RESPONSE_BYTES}),
            ),
            ("writer_policy".to_owned(), json!({"const":WRITER_POLICY})),
            ("input_sha256".to_owned(), sha256_property()),
        ]),
    )
}

fn exact_object<'a>(
    value: &'a Value,
    fields: &[&str],
    context: &str,
) -> Result<&'a Map<String, Value>, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{context} must be an object"))?;
    let expected = fields.iter().copied().collect::<BTreeSet<_>>();
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(format!("{context} contains an unknown or missing field"));
    }
    Ok(object)
}

fn contract_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 128
        && bytes[0].is_ascii_alphanumeric()
        && bytes[1..]
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
}

fn sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn require_id<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    context: &str,
) -> Result<&'a str, String> {
    let value = object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{context} {field} must be a string"))?;
    if !contract_id(value) {
        return Err(format!("{context} {field} identity is invalid"));
    }
    Ok(value)
}

fn require_sha<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    context: &str,
) -> Result<&'a str, String> {
    let value = object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{context} {field} must be a SHA-256"))?;
    if !sha256(value) {
        return Err(format!("{context} {field} SHA-256 is invalid"));
    }
    Ok(value)
}

fn require_optional_id(
    object: &Map<String, Value>,
    field: &str,
    context: &str,
) -> Result<(), String> {
    match object.get(field) {
        Some(Value::Null) => Ok(()),
        Some(Value::String(value)) if contract_id(value) => Ok(()),
        _ => Err(format!("{context} {field} optional identity is invalid")),
    }
}

fn require_optional_sha(
    object: &Map<String, Value>,
    field: &str,
    context: &str,
) -> Result<(), String> {
    match object.get(field) {
        Some(Value::Null) => Ok(()),
        Some(Value::String(value)) if sha256(value) => Ok(()),
        _ => Err(format!("{context} {field} optional SHA-256 is invalid")),
    }
}

fn require_bool(object: &Map<String, Value>, field: &str, context: &str) -> Result<bool, String> {
    object
        .get(field)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("{context} {field} must be boolean"))
}

fn require_const<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    expected: &str,
    context: &str,
) -> Result<&'a str, String> {
    let value = object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{context} {field} must be a string"))?;
    if value != expected {
        return Err(format!("{context} {field} differs"));
    }
    Ok(value)
}

fn require_enum<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    allowed: &[&str],
    context: &str,
) -> Result<&'a str, String> {
    let value = object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{context} {field} must be a string"))?;
    if !allowed.contains(&value) {
        return Err(format!("{context} {field} is invalid"));
    }
    Ok(value)
}

fn request_input_sha256(value: &Value) -> Result<String, String> {
    let mut preimage = value
        .as_object()
        .cloned()
        .map(Value::Object)
        .ok_or_else(|| "Formal High request must be an object".to_owned())?;
    preimage
        .as_object_mut()
        .expect("object was checked above")
        .remove("input_sha256");
    Ok(canonical_json_hash(&preimage))
}

/// Validate the exact public request before forwarding it to Runtime. The
/// main MCP envelope validator also consumes the advertised schema; this
/// helper is intentionally available to focused callers and tests.
pub fn validate_request(name: &str, value: &Value) -> Result<(), String> {
    let tool = from_name(name).ok_or_else(|| format!("unknown Formal High tool {name}"))?;
    if serde_json::to_vec(value)
        .map(|bytes| bytes.len() as u64 > MAX_RESPONSE_BYTES)
        .unwrap_or(true)
    {
        return Err("Formal High request exceeds the 1 MiB wire budget".to_owned());
    }
    let fields = if tool.is_write() {
        PREPARE_FIELDS
    } else {
        GET_FIELDS
    };
    let object = exact_object(value, fields, "Formal High request")?;
    if object.get("schema_version").and_then(Value::as_str)
        != Some(if tool.is_write() {
            PREPARE_REQUEST_SCHEMA_VERSION
        } else {
            GET_REQUEST_SCHEMA_VERSION
        })
    {
        return Err("Formal High request schema_version differs".to_owned());
    }
    for field in if tool.is_write() {
        &[
            "source_stage_head_transition_id",
            "high_candidate_id",
            "idempotency_key",
        ][..]
    } else {
        &[
            "project_id",
            "session_id",
            "high_artifact_id",
            "high_candidate_id",
        ][..]
    } {
        require_id(object, field, "Formal High request")?;
    }
    if tool.is_write() {
        for field in [
            "source_stage_head_transition_sha256",
            "source_stage_head_canonical_sha256",
            "input_sha256",
        ] {
            require_sha(object, field, "Formal High request")?;
        }
        if object.get("max_response_bytes").and_then(Value::as_u64) != Some(MAX_RESPONSE_BYTES) {
            return Err("Formal High request max_response_bytes must be 1048576".to_owned());
        }
        require_const(
            object,
            "writer_policy",
            WRITER_POLICY,
            "Formal High request",
        )?;
        let expected = request_input_sha256(value)?;
        if object.get("input_sha256").and_then(Value::as_str) != Some(expected.as_str()) {
            return Err("Formal High request input_sha256 does not bind the request".to_owned());
        }
    }
    Ok(())
}

/// Alias used by callers that treat MCP input validation as call validation.
pub fn validate_call(name: &str, value: &Value) -> Result<(), String> {
    validate_request(name, value)
}

fn validate_candidate(object: &Map<String, Value>) -> Result<(), String> {
    let context = "Formal High Candidate";
    let _ = exact_object(&Value::Object(object.clone()), CANDIDATE_FIELDS, context)?;
    require_const(object, "schema_version", CANDIDATE_SCHEMA_VERSION, context)?;
    for field in ["candidate_id", "project_id"] {
        require_id(object, field, context)?;
    }
    for field in [
        "base_version_id",
        "source_version_id",
        "prepared_object_id",
        "quality_report_id",
    ] {
        require_optional_id(object, field, context)?;
    }
    for field in ["prepared_object_sha256", "manifest_hash"] {
        require_optional_sha(object, field, context)?;
    }
    require_const(object, "state", "prepared", context)?;
    for field in ["request_sha256", "canonical_sha256"] {
        require_sha(object, field, context)?;
    }
    if require_bool(object, "quality_hard_gate_passed", context)? {
        return Err("Formal High Candidate quality_hard_gate_passed must be false".to_owned());
    }
    match object.get("error_code") {
        Some(Value::Null) => {}
        Some(Value::String(value)) if !value.is_empty() && value.len() <= 512 => {}
        _ => return Err("Formal High Candidate error_code is invalid".to_owned()),
    }
    for field in ["created_at", "updated_at"] {
        let timestamp = object
            .get(field)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty() && value.len() <= 128)
            .ok_or_else(|| format!("{context} {field} is invalid"))?;
        let _ = timestamp;
    }
    let canonical = require_sha(object, "canonical_sha256", context)?;
    let mut preimage = Value::Object(object.clone());
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != canonical {
        return Err("Formal High Candidate canonical_sha256 does not bind the record".to_owned());
    }
    Ok(())
}

fn require_id_array(object: &Map<String, Value>, field: &str, context: &str) -> Result<(), String> {
    let values = object
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{context} {field} must be an array"))?;
    if values.is_empty() || values.len() > 256 {
        return Err(format!("{context} {field} length is invalid"));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        let value = value
            .as_str()
            .filter(|value| contract_id(value))
            .ok_or_else(|| format!("{context} {field} contains an invalid identity"))?;
        if !unique.insert(value) {
            return Err(format!("{context} {field} contains a duplicate identity"));
        }
    }
    Ok(())
}

fn validate_high(
    object: &Map<String, Value>,
    candidate: &Map<String, Value>,
) -> Result<(), String> {
    let context = "Formal High Artifact";
    let _ = exact_object(&Value::Object(object.clone()), HIGH_FIELDS, context)?;
    require_const(object, "schema_version", HIGH_SCHEMA_VERSION, context)?;
    for field in [
        "high_artifact_id",
        "source_candidate_id",
        "source_artifact_id",
        "high_candidate_id",
        "session_id",
        "project_id",
        "source_stage_head_transition_id",
    ] {
        require_id(object, field, context)?;
    }
    for field in [
        "source_candidate_state_sha256",
        "source_artifact_sha256",
        "source_artifact_readback_sha256",
        "high_candidate_state_sha256",
        "high_artifact_sha256",
        "high_artifact_readback_sha256",
        "high_artifact_readback_object_sha256",
        "high_geometry_program_sha256",
        "high_geometry_program_object_sha256",
        "high_geometry_candidate_evidence_sha256",
        "high_detail_graph_object_sha256",
        "high_detail_graph_canonical_sha256",
        "high_part_inventory_sha256",
        "high_policy_sha256",
        "high_worker_algorithm_sha256",
        "high_worker_build_cohort_sha256",
        "source_stage_head_transition_sha256",
        "source_stage_head_canonical_sha256",
        "request_sha256",
        "input_sha256",
        "receipt_object_sha256",
        "canonical_sha256",
    ] {
        require_sha(object, field, context)?;
    }
    require_id_array(object, "high_part_ids", context)?;
    require_id_array(object, "high_material_zone_ids", context)?;
    require_const(object, "high_policy", HIGH_POLICY, context)?;
    require_const(object, "high_artifact_kind", HIGH_ARTIFACT_KIND, context)?;
    require_const(object, "high_mime", HIGH_MIME, context)?;
    let size = object
        .get("high_size_bytes")
        .and_then(Value::as_u64)
        .filter(|value| (1..=MAX_HIGH_ARTIFACT_BYTES).contains(value))
        .ok_or_else(|| format!("{context} high_size_bytes is invalid"))?;
    let _ = size;
    if object
        .get("high_worker_replay_count")
        .and_then(Value::as_u64)
        != Some(2)
        || object.get("high_replay_byte_exact") != Some(&Value::Bool(true))
    {
        return Err("Formal High Artifact Worker replay binding differs".to_owned());
    }
    for field in [
        "high_topology_status",
        "high_uv_status",
        "high_tangent_status",
    ] {
        let status = object
            .get(field)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty() && value.len() <= 64)
            .ok_or_else(|| format!("{context} {field} is invalid"))?;
        let _ = status;
    }
    require_enum(
        object,
        "high_authoring_topology_status",
        &["complete", "partial", "not-available"],
        context,
    )?;
    require_const(
        object,
        "source_stage_head_stage",
        "secondary-form-approved",
        context,
    )?;
    require_const(object, "validator_status", "passed", context)?;
    require_const(
        object,
        "structural_status",
        "PASS_SOURCE_STRUCTURAL",
        context,
    )?;
    require_const(object, "visual_status", "NOT_RUN", context)?;
    require_const(object, "human_status", "NOT_RUN", context)?;
    require_const(object, "engine_status", "NOT_RUN", context)?;
    require_const(object, "distribution_status", "NOT_RUN", context)?;
    require_const(object, "quality_status", "structural_only", context)?;
    if !require_bool(object, "hard_gate_passed", context)?
        || !require_bool(object, "runtime_write_performed", context)?
        || require_bool(object, "production_stage_advanced", context)?
        || require_bool(object, "candidate_confirmed", context)?
        || require_bool(object, "version_created", context)?
        || require_bool(object, "export_performed", context)?
    {
        return Err("Formal High Artifact side-effect or structural flags differ".to_owned());
    }
    let created_at = object
        .get("created_at")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 128)
        .ok_or_else(|| format!("{context} created_at is invalid"))?;
    let _ = created_at;

    let candidate_id = require_id(candidate, "candidate_id", "Formal High Candidate")?;
    let candidate_project = require_id(candidate, "project_id", "Formal High Candidate")?;
    if object.get("high_candidate_id").and_then(Value::as_str) != Some(candidate_id)
        || object
            .get("high_candidate_state_sha256")
            .and_then(Value::as_str)
            != candidate.get("canonical_sha256").and_then(Value::as_str)
        || object.get("project_id").and_then(Value::as_str) != Some(candidate_project)
        || object.get("session_id").and_then(Value::as_str).is_none()
    {
        return Err("Formal High Artifact candidate/project binding differs".to_owned());
    }
    if object.get("source_candidate_id").and_then(Value::as_str) == Some(candidate_id)
        || object.get("source_artifact_sha256").and_then(Value::as_str)
            == object.get("high_artifact_sha256").and_then(Value::as_str)
    {
        return Err(
            "Formal High Artifact source and derived identities must be distinct".to_owned(),
        );
    }
    if candidate.get("prepared_object_id").and_then(Value::as_str)
        != object.get("high_artifact_id").and_then(Value::as_str)
        || candidate
            .get("prepared_object_sha256")
            .and_then(Value::as_str)
            != object.get("high_artifact_sha256").and_then(Value::as_str)
        || candidate.get("request_sha256").and_then(Value::as_str)
            != object.get("request_sha256").and_then(Value::as_str)
    {
        return Err("Formal High Artifact prepared candidate binding differs".to_owned());
    }
    if object.get("high_policy_sha256").and_then(Value::as_str)
        != Some(sha256_hex(HIGH_POLICY.as_bytes()).as_str())
    {
        return Err("Formal High Artifact policy hash differs".to_owned());
    }
    let inventory = json!({
        "part_ids":object.get("high_part_ids"),
        "material_zone_ids":object.get("high_material_zone_ids")
    });
    if object
        .get("high_part_inventory_sha256")
        .and_then(Value::as_str)
        != Some(canonical_json_hash(&inventory).as_str())
    {
        return Err("Formal High Artifact part inventory hash differs".to_owned());
    }
    let canonical = require_sha(object, "canonical_sha256", context)?;
    let mut preimage = Value::Object(object.clone());
    preimage["receipt_object_sha256"] = Value::String(String::new());
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != canonical {
        return Err("Formal High Artifact canonical_sha256 does not bind the record".to_owned());
    }
    Ok(())
}

/// Validate Runtime output before MCP exposes it as structured content.
/// Nested Candidate and High records are checked as closed records, with all
/// derived side-effect flags kept fail-closed and the complete response under
/// the 1 MiB MCP wire budget.
pub fn validate_response(name: &str, value: &Value) -> Result<(), String> {
    let tool = from_name(name).ok_or_else(|| format!("unknown Formal High tool {name}"))?;
    let serialized_size = serde_json::to_vec(value)
        .map_err(|error| format!("Formal High response serialization failed: {error}"))?
        .len() as u64;
    if serialized_size > MAX_RESPONSE_BYTES {
        return Err("Formal High response exceeds the 1 MiB wire budget".to_owned());
    }
    let object = exact_object(value, RESULT_FIELDS, "Formal High response")?;
    let expected_schema = if tool.is_write() {
        PREPARE_RESULT_SCHEMA_VERSION
    } else {
        GET_RESULT_SCHEMA_VERSION
    };
    if object.get("schema_version").and_then(Value::as_str) != Some(expected_schema) {
        return Err("Formal High response schema_version differs".to_owned());
    }
    let candidate = object
        .get("candidate")
        .and_then(Value::as_object)
        .ok_or_else(|| "Formal High response candidate is missing".to_owned())?;
    validate_candidate(candidate)?;
    let high = object
        .get("high")
        .and_then(Value::as_object)
        .ok_or_else(|| "Formal High response high is missing".to_owned())?;
    validate_high(high, candidate)?;

    let replayed = require_bool(object, "replayed", "Formal High response")?;
    let runtime_write = require_bool(object, "runtime_write", "Formal High response")?;
    if runtime_write == replayed {
        return Err("Formal High response runtime_write must equal !replayed".to_owned());
    }
    if !require_bool(object, "restart_hash_verified", "Formal High response")? {
        return Err("Formal High response restart_hash_verified is not true".to_owned());
    }
    for field in [
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
    ] {
        if require_bool(object, field, "Formal High response")? {
            return Err(format!("Formal High response {field} must remain false"));
        }
    }
    if !tool.is_write() && runtime_write {
        return Err("Formal High get response runtime_write must be false".to_owned());
    }
    Ok(())
}

fn value_field(value: &Value, field: &str) -> Value {
    value.get(field).cloned().unwrap_or(Value::Null)
}

fn nested_field(value: &Value, object: &str, field: &str) -> Value {
    value
        .pointer(&format!("/{object}/{field}"))
        .cloned()
        .unwrap_or(Value::Null)
}

/// Return a compact text projection while leaving the complete typed result
/// in `structuredContent`. No operation field is read or invented from the
/// Formal High contracts, and no GLB/PNG/media bytes are copied into summary.
pub fn summary(name: &str, value: &Value) -> Option<String> {
    let tool = from_name(name)?;
    let summary = json!({
        "schema_version":"ProductionWeaponFormalHighMcpSummary@1",
        "tool":tool.name(),
        "runtime_method":tool.runtime_method(),
        "write_intent":if tool.is_write() { "explicit_runtime_production_weapon_formal_high_prepare_write" } else { "read_only_runtime_production_weapon_formal_high_get" },
        "result_schema_version":value_field(value, "schema_version"),
        "candidate_id":nested_field(value, "candidate", "candidate_id"),
        "candidate_project_id":nested_field(value, "candidate", "project_id"),
        "candidate_state":nested_field(value, "candidate", "state"),
        "candidate_state_sha256":nested_field(value, "candidate", "canonical_sha256"),
        "high_artifact_id":nested_field(value, "high", "high_artifact_id"),
        "high_artifact_sha256":nested_field(value, "high", "high_artifact_sha256"),
        "high_candidate_id":nested_field(value, "high", "high_candidate_id"),
        "high_candidate_state_sha256":nested_field(value, "high", "high_candidate_state_sha256"),
        "source_candidate_id":nested_field(value, "high", "source_candidate_id"),
        "source_artifact_id":nested_field(value, "high", "source_artifact_id"),
        "source_stage_head_transition_id":nested_field(value, "high", "source_stage_head_transition_id"),
        "source_stage_head_stage":nested_field(value, "high", "source_stage_head_stage"),
        "validator_status":nested_field(value, "high", "validator_status"),
        "structural_status":nested_field(value, "high", "structural_status"),
        "visual_status":nested_field(value, "high", "visual_status"),
        "human_status":nested_field(value, "high", "human_status"),
        "engine_status":nested_field(value, "high", "engine_status"),
        "distribution_status":nested_field(value, "high", "distribution_status"),
        "quality_status":nested_field(value, "high", "quality_status"),
        "hard_gate_passed":nested_field(value, "high", "hard_gate_passed"),
        "replayed":value_field(value, "replayed"),
        "runtime_write":value_field(value, "runtime_write"),
        "restart_hash_verified":value_field(value, "restart_hash_verified"),
        "production_stage_advanced":value_field(value, "production_stage_advanced"),
        "candidate_confirmed":value_field(value, "candidate_confirmed"),
        "version_created":value_field(value, "version_created"),
        "export_performed":value_field(value, "export_performed"),
        "glb_bytes_in_summary":false,
        "png_bytes_in_summary":false,
        "structured_content_complete":true
    });
    let encoded = serde_json::to_string(&summary).ok()?;
    (encoded.len() as u64 <= MAX_RESPONSE_BYTES).then_some(encoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(seed: char) -> String {
        seed.to_string().repeat(64)
    }

    fn prepare_request() -> Value {
        let mut value = json!({
            "schema_version":PREPARE_REQUEST_SCHEMA_VERSION,
            "source_stage_head_transition_id":"transition-1",
            "source_stage_head_transition_sha256":hash('a'),
            "source_stage_head_canonical_sha256":hash('b'),
            "high_candidate_id":"high-candidate-1",
            "idempotency_key":"formal-high-1",
            "max_response_bytes":MAX_RESPONSE_BYTES,
            "writer_policy":WRITER_POLICY,
            "input_sha256":""
        });
        let expected = request_input_sha256(&value).expect("request hash");
        value["input_sha256"] = Value::String(expected);
        value
    }

    fn candidate() -> Value {
        let mut value = json!({
            "schema_version":CANDIDATE_SCHEMA_VERSION,
            "candidate_id":"high-candidate-1",
            "project_id":"project-1",
            "base_version_id":null,
            "source_version_id":null,
            "prepared_object_id":"high-artifact-1",
            "prepared_object_sha256":hash('0'),
            "state":"prepared",
            "request_sha256":hash('c'),
            "manifest_hash":null,
            "quality_report_id":null,
            "quality_hard_gate_passed":false,
            "canonical_sha256":"",
            "error_code":null,
            "created_at":"2026-08-26T00:00:00Z",
            "updated_at":"2026-08-26T00:00:00Z"
        });
        let canonical = canonical_json_hash(&value);
        value["canonical_sha256"] = Value::String(canonical);
        value
    }

    fn high() -> Value {
        let candidate = candidate();
        let mut value = json!({
            "schema_version":HIGH_SCHEMA_VERSION,
            "high_artifact_id":"high-artifact-1",
            "source_candidate_id":"source-candidate-1",
            "source_candidate_state_sha256":hash('d'),
            "source_artifact_id":"source-artifact-1",
            "source_artifact_sha256":hash('e'),
            "source_artifact_readback_sha256":hash('f'),
            "high_candidate_id":"high-candidate-1",
            "high_candidate_state_sha256":candidate["canonical_sha256"],
            "high_artifact_sha256":hash('0'),
            "high_artifact_readback_sha256":hash('1'),
            "high_artifact_readback_object_sha256":hash('2'),
            "high_geometry_program_sha256":hash('3'),
            "high_geometry_program_object_sha256":hash('4'),
            "high_geometry_candidate_evidence_sha256":hash('5'),
            "high_detail_graph_object_sha256":hash('6'),
            "high_detail_graph_canonical_sha256":hash('7'),
            "high_part_inventory_sha256":"",
            "high_part_ids":["receiver","muzzle"],
            "high_material_zone_ids":["outer-shell"],
            "high_policy":HIGH_POLICY,
            "high_policy_sha256":sha256_hex(HIGH_POLICY.as_bytes()),
            "high_artifact_kind":HIGH_ARTIFACT_KIND,
            "high_mime":HIGH_MIME,
            "high_size_bytes":1024,
            "high_worker_algorithm_sha256":hash('8'),
            "high_worker_build_cohort_sha256":hash('9'),
            "high_worker_replay_count":2,
            "high_replay_byte_exact":true,
            "high_topology_status":"structural-readback",
            "high_authoring_topology_status":"not-available",
            "high_uv_status":"NOT_RUN",
            "high_tangent_status":"NOT_RUN",
            "session_id":"session-1",
            "project_id":"project-1",
            "source_stage_head_transition_id":"transition-1",
            "source_stage_head_transition_sha256":hash('a'),
            "source_stage_head_canonical_sha256":hash('b'),
            "source_stage_head_stage":"secondary-form-approved",
            "validator_status":"passed",
            "structural_status":"PASS_SOURCE_STRUCTURAL",
            "visual_status":"NOT_RUN",
            "human_status":"NOT_RUN",
            "engine_status":"NOT_RUN",
            "distribution_status":"NOT_RUN",
            "quality_status":"structural_only",
            "hard_gate_passed":true,
            "runtime_write_performed":true,
            "production_stage_advanced":false,
            "candidate_confirmed":false,
            "version_created":false,
            "export_performed":false,
            "request_sha256":candidate["request_sha256"],
            "input_sha256":hash('a'),
            "receipt_object_sha256":hash('b'),
            "canonical_sha256":"",
            "created_at":"2026-08-26T00:00:00Z"
        });
        value["high_part_inventory_sha256"] = Value::String(canonical_json_hash(&json!({
            "part_ids":value["high_part_ids"],
            "material_zone_ids":value["high_material_zone_ids"]
        })));
        let mut preimage = value.clone();
        preimage["receipt_object_sha256"] = Value::String(String::new());
        preimage["canonical_sha256"] = Value::String(String::new());
        value["canonical_sha256"] = Value::String(canonical_json_hash(&preimage));
        value
    }

    fn response(tool: ProductionWeaponFormalHighTool, replayed: bool) -> Value {
        json!({
            "schema_version":if tool.is_write() { PREPARE_RESULT_SCHEMA_VERSION } else { GET_RESULT_SCHEMA_VERSION },
            "candidate":candidate(),
            "high":high(),
            "replayed":replayed,
            "runtime_write":!replayed,
            "restart_hash_verified":true,
            "production_stage_advanced":false,
            "candidate_confirmed":false,
            "version_created":false,
            "export_performed":false
        })
    }

    #[test]
    fn definitions_match_closed_formal_high_requests() {
        let read = read_tools();
        let write = write_tools();
        assert_eq!(read.len(), 1);
        assert_eq!(write.len(), 1);
        assert_eq!(read[0]["name"], "production_weapon_formal_high_get");
        assert_eq!(write[0]["name"], "production_weapon_formal_high_prepare");
        assert_eq!(read[0]["annotations"]["readOnlyHint"], true);
        assert_eq!(write[0]["annotations"]["readOnlyHint"], false);
        assert_eq!(read[0]["inputSchema"]["required"], json!(GET_FIELDS));
        assert_eq!(write[0]["inputSchema"]["required"], json!(PREPARE_FIELDS));
        assert_eq!(read[0]["inputSchema"]["additionalProperties"], false);
        assert_eq!(write[0]["inputSchema"]["additionalProperties"], false);
        assert!(read[0]["inputSchema"]["properties"]
            .get("operation")
            .is_none());
        assert!(write[0]["inputSchema"]["properties"]
            .get("operation")
            .is_none());
        assert_eq!(
            write[0]["inputSchema"]["properties"]["max_response_bytes"]["const"],
            MAX_RESPONSE_BYTES
        );
        assert_eq!(write[0]["_meta"]["forgecad"]["transaction"], TRANSACTION);
    }

    #[test]
    fn request_validation_is_closed_and_hash_bound_without_operation() {
        let request = prepare_request();
        assert!(validate_request("production_weapon_formal_high_prepare", &request).is_ok());
        let mut unknown = request.clone();
        unknown["operation"] = Value::String("unexpected".to_owned());
        assert!(validate_request("production_weapon_formal_high_prepare", &unknown).is_err());
        let mut tampered = request;
        tampered["high_candidate_id"] = Value::String("other-candidate".to_owned());
        assert!(validate_request("production_weapon_formal_high_prepare", &tampered).is_err());
    }

    #[test]
    fn response_validation_checks_nested_bindings_and_budget() {
        let prepare = response(ProductionWeaponFormalHighTool::Prepare, false);
        validate_response("production_weapon_formal_high_prepare", &prepare)
            .unwrap_or_else(|error| panic!("{error}"));
        let get = response(ProductionWeaponFormalHighTool::Get, true);
        validate_response("production_weapon_formal_high_get", &get)
            .unwrap_or_else(|error| panic!("{error}"));
        let mut invalid = prepare;
        invalid["production_stage_advanced"] = Value::Bool(true);
        assert!(validate_response("production_weapon_formal_high_prepare", &invalid).is_err());
        let mut oversized = get;
        oversized["candidate"]["error_code"] =
            Value::String("x".repeat(MAX_RESPONSE_BYTES as usize));
        assert!(validate_response("production_weapon_formal_high_get", &oversized).is_err());
    }

    #[test]
    fn summary_is_hash_only_and_does_not_require_operation() {
        let value = response(ProductionWeaponFormalHighTool::Prepare, true);
        let text = summary("production_weapon_formal_high_prepare", &value).expect("summary");
        let summary: Value = serde_json::from_str(&text).expect("summary JSON");
        assert_eq!(
            summary["schema_version"],
            "ProductionWeaponFormalHighMcpSummary@1"
        );
        assert_eq!(summary["tool"], "production_weapon_formal_high_prepare");
        assert!(summary.get("operation").is_none());
        assert_eq!(summary["glb_bytes_in_summary"], false);
        assert_eq!(summary["structured_content_complete"], true);
    }
}
