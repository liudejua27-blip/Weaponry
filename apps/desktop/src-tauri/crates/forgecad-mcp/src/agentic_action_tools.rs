use serde_json::{json, Map, Value};

const DESIGN_STAGES: [&str; 6] = [
    "reference-canvas",
    "primary-form",
    "secondary-structure",
    "tertiary-detail",
    "uv-pbr",
    "final-review",
];

const BOUNDED_ACTION_KINDS: [&str; 15] = [
    "reference-import",
    "coverage-annotation",
    "mark-unknown",
    "primary-blockout",
    "primary-form-adjustment",
    "secondary-structure",
    "tertiary-detail",
    "material-zone",
    "final-review",
    "request-reference",
    "bounded-repair",
    "checkpoint",
    "rollback",
    "human-review",
    "next-stage",
];

const ACTION_FIELDS: [&str; 8] = [
    "action_id",
    "action_kind",
    "scope_kind",
    "target_id",
    "operator_id",
    "parameter_changes",
    "bounded",
    "description",
];

const REQUIRED_ACTION_FIELDS: [&str; 7] = [
    "action_id",
    "action_kind",
    "scope_kind",
    "target_id",
    "operator_id",
    "parameter_changes",
    "bounded",
];

const OPERATOR_IDS: [&str; 12] = [
    "forgecad.geometry.primitive@2",
    "forgecad.geometry.profile-extrude@1",
    "forgecad.geometry.profile-loft@1",
    "forgecad.geometry.revolve@1",
    "forgecad.geometry.tube-sweep@1",
    "forgecad.geometry.transform@2",
    "forgecad.geometry.mirror@1",
    "forgecad.geometry.array@1",
    "forgecad.geometry.panel@1",
    "forgecad.geometry.vent-array@1",
    "forgecad.geometry.joint-stack@1",
    "forgecad.geometry.part-output@1",
];

const READ_FIELDS: [&str; 4] = ["project_id", "session_id", "candidate_id", "run_id"];

const WRITE_FIELDS: [&str; 14] = [
    "project_id",
    "session_id",
    "candidate_id",
    "run_id",
    "action",
    "input_sha256",
    "observation_sha256",
    "requested_stage",
    "approved",
    "approval_receipt_id",
    "approval_summary",
    "approval_expires_at",
    "approval_session_id",
    "idempotency_key",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgenticActionTool {
    DesignActionRunGet,
    DesignActionRunPrepare,
}

pub type AgenticTool = AgenticActionTool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    Read,
    Write,
}

pub type NameCategory = ToolKind;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Binding {
    pub session_id: Option<String>,
    pub project_id: Option<String>,
    pub candidate_id: Option<String>,
    pub run_id: Option<String>,
}

impl AgenticActionTool {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "design_action_run_get" => Self::DesignActionRunGet,
            "design_action_run_prepare" => Self::DesignActionRunPrepare,
            _ => return None,
        })
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::DesignActionRunGet => "design_action_run_get",
            Self::DesignActionRunPrepare => "design_action_run_prepare",
        }
    }

    pub const fn kind(self) -> ToolKind {
        match self {
            Self::DesignActionRunGet => ToolKind::Read,
            Self::DesignActionRunPrepare => ToolKind::Write,
        }
    }

    pub const fn is_write(self) -> bool {
        matches!(self, Self::DesignActionRunPrepare)
    }

    pub const fn read_only(self) -> bool {
        !self.is_write()
    }

    pub const fn requires_approval(self) -> bool {
        self.is_write()
    }

    pub const fn destructive(self) -> bool {
        false
    }

    pub const fn idempotent(self) -> bool {
        true
    }

    pub const fn runtime_method(self) -> &'static str {
        self.name()
    }

    pub const fn implemented(self) -> bool {
        true
    }
}

impl ToolKind {
    pub const fn is_read(self) -> bool {
        matches!(self, Self::Read)
    }

    pub const fn is_write(self) -> bool {
        matches!(self, Self::Write)
    }
}

impl Binding {
    pub fn is_bound(&self) -> bool {
        self.session_id.is_some()
            && self.project_id.is_some()
            && self.candidate_id.is_some()
            && self.run_id.is_some()
    }

    pub fn has_scope(&self) -> bool {
        self.session_id.is_some()
            || self.project_id.is_some()
            || self.candidate_id.is_some()
            || self.run_id.is_some()
    }

    pub fn has_session_scope(&self) -> bool {
        self.session_id.is_some() && self.project_id.is_some() && self.candidate_id.is_some()
    }
}

pub fn is_tool(name: &str) -> bool {
    AgenticActionTool::from_name(name).is_some()
}

pub fn is_read_tool(name: &str) -> bool {
    AgenticActionTool::from_name(name).is_some_and(|tool| tool.kind().is_read())
}

pub fn is_write_tool(name: &str) -> bool {
    AgenticActionTool::from_name(name).is_some_and(AgenticActionTool::is_write)
}

pub fn classify_name(name: &str) -> Option<ToolKind> {
    AgenticActionTool::from_name(name).map(AgenticActionTool::kind)
}

pub fn name_category(name: &str) -> Option<NameCategory> {
    classify_name(name)
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    AgenticActionTool::from_name(name).map(AgenticActionTool::runtime_method)
}

pub fn unavailable_error(name: &str) -> String {
    let tool = AgenticActionTool::from_name(name).expect("action tool name was checked");
    format!(
        "AGENTIC_ACTION_RUNTIME_METHOD_UNAVAILABLE: {} requires Runtime method {}",
        tool.name(),
        tool.runtime_method()
    )
}

pub fn sync_session_scope(source: &super::agentic_write_tools::Binding, target: &mut Binding) {
    target.session_id = source.session_id.clone();
    target.project_id = source.project_id.clone();
    target.candidate_id = source.candidate_id.clone();
}

pub fn read_tool_names() -> Vec<String> {
    [AgenticActionTool::DesignActionRunGet]
        .into_iter()
        .map(|tool| tool.name().to_owned())
        .collect()
}

pub fn write_tool_names() -> Vec<String> {
    [AgenticActionTool::DesignActionRunPrepare]
        .into_iter()
        .map(|tool| tool.name().to_owned())
        .collect()
}

pub fn all_tool_names() -> Vec<String> {
    read_tool_names()
        .into_iter()
        .chain(write_tool_names())
        .collect()
}

pub fn read_tools() -> Vec<Value> {
    [AgenticActionTool::DesignActionRunGet]
        .into_iter()
        .map(tool_definition)
        .collect()
}

pub fn write_tools() -> Vec<Value> {
    [AgenticActionTool::DesignActionRunPrepare]
        .into_iter()
        .map(tool_definition)
        .collect()
}

pub fn all_tools() -> Vec<Value> {
    read_tools().into_iter().chain(write_tools()).collect()
}

pub fn tool_definition_by_name(name: &str) -> Option<Value> {
    AgenticActionTool::from_name(name).map(tool_definition)
}

pub fn input_schema(name: &str) -> Option<Value> {
    AgenticActionTool::from_name(name).map(input_schema_for)
}

pub fn bounded_action_kinds() -> &'static [&'static str] {
    &BOUNDED_ACTION_KINDS
}

pub fn design_stages() -> &'static [&'static str] {
    &DESIGN_STAGES
}

pub fn operator_ids() -> &'static [&'static str] {
    &OPERATOR_IDS
}

fn tool_definition(tool: AgenticActionTool) -> Value {
    let description = match tool {
        AgenticActionTool::DesignActionRunGet => {
            "Read one exact-bound Runtime-owned DesignActionRun projection. The read returns the immutable action receipt and its stage, quality, and lock state."
        }
        AgenticActionTool::DesignActionRunPrepare => {
            "Execute one approved, bounded Primary Form DesignActionRun for a bound session and candidate. The Runtime owns the bounded search and staged result; the call never confirms, exports, or mutates a confirmed version."
        }
    };

    json!({
        "name": tool.name(),
        "description": description,
        "inputSchema": input_schema_for(tool),
        "annotations": {
            "readOnlyHint": tool.read_only(),
            "destructiveHint": tool.destructive(),
            "idempotentHint": tool.idempotent(),
            "openWorldHint": false,
            "writeIntent": tool.is_write(),
            "approvalRequired": tool.requires_approval()
        },
        "_meta": {"forgecad": {
            "availability": if tool.implemented() { "available" } else { "unavailable" },
            "runtime_method": tool.runtime_method(),
            "requiresConfirmation": tool.requires_approval(),
            "transaction": "ADR-0026",
            "definition_only": false
        }}
    })
}

fn input_schema_for(tool: AgenticActionTool) -> Value {
    match tool {
        AgenticActionTool::DesignActionRunGet => read_schema(),
        AgenticActionTool::DesignActionRunPrepare => write_schema(),
    }
}

fn read_schema() -> Value {
    object_schema(
        READ_FIELDS.to_vec(),
        Map::from_iter([
            ("project_id".to_owned(), id_property()),
            ("session_id".to_owned(), id_property()),
            ("candidate_id".to_owned(), id_property()),
            ("run_id".to_owned(), id_property()),
        ]),
    )
}

fn write_schema() -> Value {
    object_schema(
        vec![
            "project_id",
            "session_id",
            "candidate_id",
            "run_id",
            "action",
            "input_sha256",
            "observation_sha256",
            "requested_stage",
            "approved",
            "approval_receipt_id",
            "approval_summary",
            "idempotency_key",
        ],
        Map::from_iter([
            ("project_id".to_owned(), id_property()),
            ("session_id".to_owned(), id_property()),
            ("candidate_id".to_owned(), id_property()),
            ("run_id".to_owned(), id_property()),
            ("action".to_owned(), bounded_action_schema()),
            ("input_sha256".to_owned(), sha256_property()),
            ("observation_sha256".to_owned(), sha256_property()),
            ("requested_stage".to_owned(), stage_property()),
            ("approved".to_owned(), json!({"const": true})),
            ("approval_receipt_id".to_owned(), id_property()),
            ("approval_summary".to_owned(), safe_text_property(512)),
            ("approval_expires_at".to_owned(), safe_text_property(64)),
            ("approval_session_id".to_owned(), id_property()),
            ("idempotency_key".to_owned(), id_property()),
        ]),
    )
}

pub fn bounded_action_schema() -> Value {
    let mut schema = object_schema(
        ACTION_FIELDS.to_vec(),
        Map::from_iter([
            ("action_id".to_owned(), id_property()),
            (
                "action_kind".to_owned(),
                json!({"enum": BOUNDED_ACTION_KINDS}),
            ),
            (
                "scope_kind".to_owned(),
                json!({"enum":["session","part","material-zone","reference"]}),
            ),
            ("target_id".to_owned(), nullable_id_property()),
            ("operator_id".to_owned(), operator_id_property()),
            ("parameter_changes".to_owned(), parameter_changes_property()),
            ("bounded".to_owned(), json!({"const": true})),
            ("description".to_owned(), safe_text_property(512)),
        ]),
    );
    schema["allOf"] = json!([
        {
            "if": {"properties":{"scope_kind":{"const":"session"}},"required":["scope_kind"]},
            "then": {"properties":{"target_id":{"const":null}}}
        },
        {
            "if": {"properties":{"scope_kind":{"enum":["part","material-zone","reference"]}},"required":["scope_kind"]},
            "then": {"properties":{"target_id":id_property()}}
        },
        {
            "if": {"properties":{"action_kind":{"const":"request-reference"}},"required":["action_kind"]},
            "then": {"properties":{"scope_kind":{"const":"reference"}}}
        }
    ]);
    schema
}

fn object_schema(required: Vec<&str>, properties: Map<String, Value>) -> Value {
    json!({
        "type": "object",
        "required": required,
        "properties": properties,
        "additionalProperties": false
    })
}

fn id_property() -> Value {
    json!({
        "type": "string",
        "pattern": "^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"
    })
}

fn nullable_id_property() -> Value {
    json!({
        "type": ["string", "null"],
        "pattern": "^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"
    })
}

fn operator_id_property() -> Value {
    let mut allowed = vec![Value::Null];
    allowed.extend(
        OPERATOR_IDS
            .iter()
            .map(|operator_id| Value::String((*operator_id).to_owned())),
    );
    json!({
        "type": ["string", "null"],
        "enum": allowed
    })
}

fn parameter_changes_property() -> Value {
    json!({
        "type": "array",
        "maxItems": 8,
        "uniqueItems": true,
        "items": {
            "type": "object",
            "required": ["parameter_id", "before", "after", "minimum", "maximum", "unit"],
            "properties": {
                "parameter_id": id_property(),
                "before": {"type":"number","minimum":-1000,"maximum":1000},
                "after": {"type":"number","minimum":-1000,"maximum":1000},
                "minimum": {"type":"number","minimum":-1000,"maximum":1000},
                "maximum": {"type":"number","minimum":-1000,"maximum":1000},
                "unit": {"enum":["meter","radian","ratio","count"]}
            },
            "additionalProperties": false
        }
    })
}

fn sha256_property() -> Value {
    json!({"type":"string","pattern":"^[0-9a-f]{64}$"})
}

fn stage_property() -> Value {
    json!({"enum": DESIGN_STAGES})
}

fn safe_text_property(max_length: usize) -> Value {
    json!({"type":"string","minLength":1,"maxLength":max_length})
}

pub fn validate_call(name: &str, arguments: &Value, binding: &Binding) -> Result<(), String> {
    let Some(tool) = AgenticActionTool::from_name(name) else {
        return Ok(());
    };
    let object = arguments
        .as_object()
        .ok_or_else(|| "AGENTIC_ACTION_INVALID_INPUT: arguments must be an object".to_owned())?;
    let allowed = if tool.is_write() {
        &WRITE_FIELDS[..]
    } else {
        &READ_FIELDS[..]
    };
    reject_unknown_keys(object, allowed)?;

    validate_scope(object, binding)?;
    if tool.is_write() {
        if !binding.has_session_scope() {
            return Err(
                "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before an action run"
                    .to_owned(),
            );
        }
        validate_prepare(object)?;
    }
    Ok(())
}

pub fn validate_parameters(name: &str, arguments: &Value, binding: &Binding) -> Result<(), String> {
    validate_call(name, arguments, binding)
}

pub fn validate_action_run_call(
    name: &str,
    arguments: &Value,
    binding: &Binding,
) -> Result<(), String> {
    validate_call(name, arguments, binding)
}

pub fn validate_response(name: &str, value: &Value, binding: &Binding) -> Result<(), String> {
    let Some(tool) = AgenticActionTool::from_name(name) else {
        return Ok(());
    };
    if !value.is_object() {
        return Err("AGENTIC_ACTION_RUNTIME_OUTPUT_INVALID: response must be an object".to_owned());
    }
    if value.get("schema_version").and_then(Value::as_str) != Some("DesignActionRun@1") {
        return Err(
            "AGENTIC_ACTION_RUNTIME_OUTPUT_INVALID: response schema_version is not DesignActionRun@1"
                .to_owned(),
        );
    }
    let project_id = value.get("project_id").and_then(Value::as_str);
    let session_id = value.get("session_id").and_then(Value::as_str);
    let candidate_id = value.get("candidate_id").and_then(Value::as_str);
    let run_id = value.get("run_id").and_then(Value::as_str);
    if [project_id, session_id, candidate_id, run_id]
        .into_iter()
        .any(|value| value.is_none())
    {
        return Err(
            "AGENTIC_ACTION_RUNTIME_OUTPUT_INVALID: response binding is incomplete".to_owned(),
        );
    }
    for (key, expected, actual) in [
        ("project_id", binding.project_id.as_deref(), project_id.unwrap()),
        ("session_id", binding.session_id.as_deref(), session_id.unwrap()),
        (
            "candidate_id",
            binding.candidate_id.as_deref(),
            candidate_id.unwrap(),
        ),
        ("run_id", binding.run_id.as_deref(), run_id.unwrap()),
    ] {
        if let Some(expected) = expected {
            if expected != actual {
                return Err(format!(
                    "AGENTIC_ACTION_SCOPE_MISMATCH: Runtime response {key} differs from the bound action"
                ));
            }
        }
    }
    if tool.kind().is_read() && binding.run_id.is_none() {
        return Ok(());
    }
    Ok(())
}

pub fn bind_response(name: &str, value: &Value, binding: &mut Binding) -> Result<(), String> {
    validate_response(name, value, binding)?;
    if is_tool(name) {
        binding.run_id = value
            .get("run_id")
            .and_then(Value::as_str)
            .map(str::to_owned);
        binding.session_id = value
            .get("session_id")
            .and_then(Value::as_str)
            .map(str::to_owned);
        binding.project_id = value
            .get("project_id")
            .and_then(Value::as_str)
            .map(str::to_owned);
        binding.candidate_id = value
            .get("candidate_id")
            .and_then(Value::as_str)
            .map(str::to_owned);
    }
    Ok(())
}

fn validate_scope(object: &Map<String, Value>, binding: &Binding) -> Result<(), String> {
    let project_id = required_id(object, "project_id")?;
    let session_id = required_id(object, "session_id")?;
    let candidate_id = required_id(object, "candidate_id")?;
    let run_id = required_id(object, "run_id")?;

    for (key, expected, actual) in [
        ("project_id", binding.project_id.as_deref(), project_id),
        ("session_id", binding.session_id.as_deref(), session_id),
        (
            "candidate_id",
            binding.candidate_id.as_deref(),
            candidate_id,
        ),
        ("run_id", binding.run_id.as_deref(), run_id),
    ] {
        if let Some(expected) = expected {
            if expected != actual {
                return Err(format!(
                    "AGENTIC_ACTION_SCOPE_MISMATCH: {key} is outside the bound action run"
                ));
            }
        }
    }
    Ok(())
}

fn validate_prepare(object: &Map<String, Value>) -> Result<(), String> {
    let requested_stage = required_stage(object, "requested_stage")?;
    if requested_stage != "primary-form" {
        return Err(
            "AGENTIC_ACTION_STAGE_UNSUPPORTED: requested stage is not executable in this slice; only primary-form is supported"
                .to_owned(),
        );
    }
    let input_sha256 = required_sha256(object, "input_sha256")?;
    if input_sha256.is_empty() {
        return Err("AGENTIC_ACTION_INVALID_INPUT: input_sha256 is required".to_owned());
    }
    let observation_sha256 = required_sha256(object, "observation_sha256")?;
    if observation_sha256.is_empty() {
        return Err("AGENTIC_ACTION_INVALID_INPUT: observation_sha256 is required".to_owned());
    }

    if object.get("approved") != Some(&Value::Bool(true)) {
        return Err(
            "AGENTIC_ACTION_APPROVAL_REQUIRED: approved=true is required for action prepare"
                .to_owned(),
        );
    }
    for key in ["approval_receipt_id", "approval_summary", "idempotency_key"] {
        if object.get(key).is_none() {
            return Err(format!(
                "AGENTIC_ACTION_APPROVAL_REQUIRED: {key} is required"
            ));
        }
    }
    let approval_receipt_id = required_id(object, "approval_receipt_id")?;
    let approval_summary = required_safe_text(object, "approval_summary", 512)?;
    let idempotency_key = required_id(object, "idempotency_key")?;
    if approval_receipt_id.is_empty() || idempotency_key.is_empty() {
        return Err(
            "AGENTIC_ACTION_APPROVAL_REQUIRED: approval receipt and idempotency key are required"
                .to_owned(),
        );
    }
    validate_safe_text(approval_summary, "approval_summary")?;

    if let Some(expires_at) = object.get("approval_expires_at") {
        let expires_at = expires_at.as_str().ok_or_else(|| {
            "AGENTIC_ACTION_APPROVAL_REQUIRED: approval_expires_at must be a string".to_owned()
        })?;
        validate_safe_text_bounded(expires_at, "approval_expires_at", 64)?;
    }
    if let Some(approval_session_id) = object.get("approval_session_id") {
        let approval_session_id = approval_session_id.as_str().ok_or_else(|| {
            "AGENTIC_ACTION_APPROVAL_REQUIRED: approval_session_id must be a string".to_owned()
        })?;
        let session_id = required_id(object, "session_id")?;
        if approval_session_id != session_id {
            return Err(
                "AGENTIC_ACTION_SCOPE_MISMATCH: approval_session_id must match session_id"
                    .to_owned(),
            );
        }
        validate_opaque_id(approval_session_id, "approval_session_id")?;
    }
    let action = object
        .get("action")
        .and_then(Value::as_object)
        .ok_or_else(|| "AGENTIC_ACTION_INVALID_INPUT: action must be an object".to_owned())?;
    reject_unknown_keys(action, &ACTION_FIELDS)?;
    for key in REQUIRED_ACTION_FIELDS {
        if !action.contains_key(key) {
            return Err(format!(
                "AGENTIC_ACTION_INVALID_INPUT: action.{key} is required"
            ));
        }
    }

    validate_opaque_id(required_id(action, "action_id")?, "action.action_id")?;
    let action_kind = required_nonempty_string(action, "action_kind")?;
    if !BOUNDED_ACTION_KINDS.contains(&action_kind) {
        return Err(format!(
            "AGENTIC_ACTION_NOT_BOUNDED: action_kind {action_kind} is not allowlisted"
        ));
    }
    let scope_kind = required_nonempty_string(action, "scope_kind")?;
    if !matches!(
        scope_kind,
        "session" | "part" | "material-zone" | "reference"
    ) {
        return Err(
            "AGENTIC_ACTION_INVALID_INPUT: action.scope_kind is not a bounded scope".to_owned(),
        );
    }
    if scope_kind == "session" {
        if action.get("target_id") != Some(&Value::Null) {
            return Err(
                "AGENTIC_ACTION_SCOPE_MISMATCH: session action target_id must be null"
                    .to_owned(),
            );
        }
    } else {
        required_id(action, "target_id")?;
    }
    if action_kind == "request-reference" && scope_kind != "reference" {
        return Err(
            "AGENTIC_ACTION_SCOPE_MISMATCH: request-reference must target a reference"
                .to_owned(),
        );
    }
    if action.get("bounded") != Some(&Value::Bool(true)) {
        return Err("AGENTIC_ACTION_NOT_BOUNDED: action.bounded=true is required".to_owned());
    }
    validate_operator_id(action)?;
    validate_parameter_changes(action)?;
    if let Some(description) = action.get("description") {
        let description = description.as_str().ok_or_else(|| {
            "AGENTIC_ACTION_INVALID_INPUT: action.description must be a string".to_owned()
        })?;
        validate_safe_text_bounded(description, "action.description", 512)?;
    }
    let _ = requested_stage;
    Ok(())
}

fn validate_operator_id(action: &Map<String, Value>) -> Result<(), String> {
    let Some(operator_id) = action.get("operator_id") else {
        return Err("AGENTIC_ACTION_INVALID_INPUT: action.operator_id is required".to_owned());
    };
    if operator_id.is_null() {
        return Ok(());
    }
    let operator_id = operator_id.as_str().ok_or_else(|| {
        "AGENTIC_ACTION_INVALID_INPUT: action.operator_id must be a string or null".to_owned()
    })?;
    if !OPERATOR_IDS.contains(&operator_id) {
        return Err(format!(
            "AGENTIC_ACTION_NOT_BOUNDED: operator_id {operator_id} is not allowlisted"
        ));
    }
    Ok(())
}

fn validate_parameter_changes(action: &Map<String, Value>) -> Result<(), String> {
    let changes = action
        .get("parameter_changes")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "AGENTIC_ACTION_INVALID_INPUT: action.parameter_changes must be an array".to_owned()
        })?;
    if changes.len() > 8 {
        return Err(
            "AGENTIC_ACTION_NOT_BOUNDED: parameter_changes may contain at most 8 entries"
                .to_owned(),
        );
    }
    for (index, change) in changes.iter().enumerate() {
        let change = change.as_object().ok_or_else(|| {
            format!(
                "AGENTIC_ACTION_INVALID_INPUT: action.parameter_changes[{index}] must be an object"
            )
        })?;
        const FIELDS: [&str; 6] = [
            "parameter_id",
            "before",
            "after",
            "minimum",
            "maximum",
            "unit",
        ];
        reject_unknown_keys(change, &FIELDS)?;
        for field in FIELDS {
            if !change.contains_key(field) {
                return Err(format!(
                    "AGENTIC_ACTION_INVALID_INPUT: action.parameter_changes[{index}].{field} is required"
                ));
            }
        }
        validate_opaque_id(
            change
                .get("parameter_id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    format!(
                        "AGENTIC_ACTION_INVALID_INPUT: action.parameter_changes[{index}].parameter_id is invalid"
                    )
                })?,
            "action.parameter_changes.parameter_id",
        )?;
        let minimum = bounded_number(change, "minimum", index)?;
        let maximum = bounded_number(change, "maximum", index)?;
        let before = bounded_number(change, "before", index)?;
        let after = bounded_number(change, "after", index)?;
        if minimum > maximum || before < minimum || before > maximum || after < minimum || after > maximum {
            return Err(format!(
                "AGENTIC_ACTION_NOT_BOUNDED: action.parameter_changes[{index}] exceeds its declared bounds"
            ));
        }
        let unit = change.get("unit").and_then(Value::as_str).ok_or_else(|| {
            format!(
                "AGENTIC_ACTION_INVALID_INPUT: action.parameter_changes[{index}].unit is invalid"
            )
        })?;
        if !matches!(unit, "meter" | "radian" | "ratio" | "count") {
            return Err(format!(
                "AGENTIC_ACTION_NOT_BOUNDED: action.parameter_changes[{index}].unit is not allowlisted"
            ));
        }
    }
    Ok(())
}

fn bounded_number(
    object: &Map<String, Value>,
    key: &str,
    index: usize,
) -> Result<f64, String> {
    let value = object
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| {
            format!(
                "AGENTIC_ACTION_INVALID_INPUT: action.parameter_changes[{index}].{key} must be a number"
            )
        })?;
    if !value.is_finite() || !(-1000.0..=1000.0).contains(&value) {
        return Err(format!(
            "AGENTIC_ACTION_NOT_BOUNDED: action.parameter_changes[{index}].{key} is outside [-1000, 1000]"
        ));
    }
    Ok(value)
}

fn reject_unknown_keys(object: &Map<String, Value>, allowed: &[&str]) -> Result<(), String> {
    if let Some(key) = object
        .keys()
        .find(|key| !allowed.iter().any(|allowed_key| allowed_key == key))
    {
        return Err(format!("AGENTIC_ACTION_INVALID_INPUT: unknown field {key}"));
    }
    Ok(())
}

fn required_id<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    let value = required_nonempty_string(object, key)?;
    validate_opaque_id(value, key)?;
    Ok(value)
}

fn required_nonempty_string<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a str, String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("AGENTIC_ACTION_INVALID_INPUT: {key} is required"))
}

fn required_stage<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    let value = required_nonempty_string(object, key)?;
    if !DESIGN_STAGES.contains(&value) {
        return Err(format!(
            "AGENTIC_ACTION_INVALID_INPUT: {key} is not a valid DesignStage"
        ));
    }
    Ok(value)
}

fn required_sha256<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    let value = required_nonempty_string(object, key)?;
    if !is_sha256(value) {
        return Err(format!(
            "AGENTIC_ACTION_INVALID_INPUT: {key} must be a lowercase SHA-256"
        ));
    }
    Ok(value)
}

fn validate_sha256(object: &Map<String, Value>, key: &str) -> Result<(), String> {
    required_sha256(object, key).map(|_| ())
}

fn required_safe_text<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    max_length: usize,
) -> Result<&'a str, String> {
    let value = object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("AGENTIC_ACTION_INVALID_INPUT: {key} is required"))?;
    validate_safe_text_bounded(value, key, max_length)?;
    Ok(value)
}

fn validate_safe_text(value: &str, key: &str) -> Result<(), String> {
    validate_safe_text_bounded(value, key, 512)
}

fn validate_safe_text_bounded(value: &str, key: &str, max_length: usize) -> Result<(), String> {
    let lower = value.to_ascii_lowercase();
    if value.trim().is_empty()
        || value.len() > max_length
        || value.starts_with('/')
        || value.starts_with('\\')
        || lower.contains("://")
        || lower.starts_with("file:")
        || lower.contains("password")
        || lower.contains("api_key")
        || lower.contains("secret")
        || lower.contains("token")
    {
        return Err(format!(
            "AGENTIC_ACTION_INVALID_INPUT: {key} contains empty or unsafe text"
        ));
    }
    Ok(())
}

fn validate_opaque_id(value: &str, key: &str) -> Result<(), String> {
    if !is_opaque_id(value) {
        return Err(format!(
            "AGENTIC_ACTION_INVALID_INPUT: {key} must be an opaque identifier"
        ));
    }
    Ok(())
}

fn is_opaque_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding() -> Binding {
        Binding {
            session_id: Some("session-1".to_owned()),
            project_id: Some("project-1".to_owned()),
            candidate_id: Some("candidate-1".to_owned()),
            run_id: Some("run-1".to_owned()),
        }
    }

    fn action() -> Value {
        json!({
            "action_id": "action-1",
            "action_kind": "bounded-repair",
            "scope_kind": "session",
            "target_id": null,
            "operator_id": null,
            "parameter_changes": [],
            "bounded": true,
            "description": "Prepare one bounded repair for the current stage"
        })
    }

    fn prepare() -> Value {
        json!({
            "project_id": "project-1",
            "session_id": "session-1",
            "candidate_id": "candidate-1",
            "run_id": "run-1",
            "action": action(),
            "input_sha256": "a".repeat(64),
            "observation_sha256": "b".repeat(64),
            "requested_stage": "primary-form",
            "approved": true,
            "approval_receipt_id": "approval-1",
            "approval_summary": "Approve one bounded action run",
            "approval_expires_at": "2030-01-01T00:00:00Z",
            "approval_session_id": "session-1",
            "idempotency_key": "action-run-1"
        })
    }

    #[test]
    fn definitions_keep_read_and_approval_boundaries_distinct() {
        let read = read_tools();
        let write = write_tools();
        assert_eq!(read.len(), 1);
        assert_eq!(write.len(), 1);
        assert_eq!(read[0]["name"], "design_action_run_get");
        assert_eq!(write[0]["name"], "design_action_run_prepare");
        assert_eq!(read[0]["annotations"]["readOnlyHint"], true);
        assert_eq!(read[0]["annotations"]["writeIntent"], false);
        assert_eq!(read[0]["annotations"]["approvalRequired"], false);
        assert_eq!(read[0]["annotations"]["destructiveHint"], false);
        assert_eq!(write[0]["annotations"]["readOnlyHint"], false);
        assert_eq!(write[0]["annotations"]["writeIntent"], true);
        assert_eq!(write[0]["annotations"]["approvalRequired"], true);
        assert_eq!(write[0]["annotations"]["destructiveHint"], false);
    }

    #[test]
    fn schemas_require_scope_action_hash_stage_and_approval() {
        let read_tools = read_tools();
        let read_required = read_tools[0]["inputSchema"]["required"]
            .as_array()
            .expect("read required");
        for key in READ_FIELDS {
            assert!(read_required.iter().any(|value| value == key));
        }
        let write_schema = &write_tools()[0]["inputSchema"];
        assert_eq!(write_schema["additionalProperties"], false);
        for key in [
            "project_id",
            "session_id",
            "candidate_id",
            "run_id",
            "action",
            "input_sha256",
            "observation_sha256",
            "requested_stage",
            "approved",
            "approval_receipt_id",
            "approval_summary",
            "idempotency_key",
        ] {
            assert!(write_schema["required"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == key));
        }
        assert_eq!(
            write_schema["properties"]["action"]["additionalProperties"],
            false
        );
        assert_eq!(
            write_schema["properties"]["action"]["properties"]["bounded"]["const"],
            true
        );
    }

    #[test]
    fn valid_read_and_prepare_calls_pass_for_the_same_binding() {
        let read = json!({
            "project_id": "project-1",
            "session_id": "session-1",
            "candidate_id": "candidate-1",
            "run_id": "run-1"
        });
        assert!(validate_call("design_action_run_get", &read, &binding()).is_ok());
        assert!(validate_call("design_action_run_prepare", &prepare(), &binding()).is_ok());
    }

    #[test]
    fn unknown_fields_empty_values_and_scope_drift_fail_closed() {
        let mut unknown = prepare();
        unknown["unexpected"] = Value::String("nope".to_owned());
        assert!(
            validate_call("design_action_run_prepare", &unknown, &binding())
                .unwrap_err()
                .contains("unknown field")
        );

        let mut empty = prepare();
        empty["run_id"] = Value::String("   ".to_owned());
        assert!(validate_call("design_action_run_prepare", &empty, &binding()).is_err());

        let mut missing_observation = prepare();
        missing_observation
            .as_object_mut()
            .unwrap()
            .remove("observation_sha256");
        assert!(validate_call("design_action_run_prepare", &missing_observation, &binding())
            .unwrap_err()
            .contains("observation_sha256"));

        let mut cross_project = prepare();
        cross_project["project_id"] = Value::String("project-2".to_owned());
        assert!(
            validate_call("design_action_run_prepare", &cross_project, &binding())
                .unwrap_err()
                .contains("SCOPE_MISMATCH")
        );

        let mut cross_approval = prepare();
        cross_approval["approval_session_id"] = Value::String("session-2".to_owned());
        assert!(
            validate_call("design_action_run_prepare", &cross_approval, &binding())
                .unwrap_err()
                .contains("SCOPE_MISMATCH")
        );
    }

    #[test]
    fn stage_hash_approval_and_bounded_action_guards_fail_closed() {
        let mut stage_drift = prepare();
        stage_drift["requested_stage"] = Value::String("tertiary-detail".to_owned());
        assert!(
            validate_call("design_action_run_prepare", &stage_drift, &binding())
                .unwrap_err()
                .contains("stage")
        );

        let mut bad_hash = prepare();
        bad_hash["input_sha256"] = Value::String("A".repeat(64));
        assert!(validate_call("design_action_run_prepare", &bad_hash, &binding()).is_err());

        let mut not_approved = prepare();
        not_approved["approved"] = Value::Bool(false);
        assert!(
            validate_call("design_action_run_prepare", &not_approved, &binding())
                .unwrap_err()
                .contains("APPROVAL_REQUIRED")
        );

        let mut not_bounded = prepare();
        not_bounded["action"]["bounded"] = Value::Bool(false);
        assert!(
            validate_call("design_action_run_prepare", &not_bounded, &binding())
                .unwrap_err()
                .contains("NOT_BOUNDED")
        );

        let mut dangerous_kind = prepare();
        dangerous_kind["action"]["action_kind"] = Value::String("confirm".to_owned());
        assert!(
            validate_call("design_action_run_prepare", &dangerous_kind, &binding())
                .unwrap_err()
                .contains("NOT_BOUNDED")
        );
    }

    #[test]
    fn names_are_partitioned_without_unknown_aliases() {
        assert!(is_tool("design_action_run_get"));
        assert!(is_read_tool("design_action_run_get"));
        assert!(!is_write_tool("design_action_run_get"));
        assert!(is_write_tool("design_action_run_prepare"));
        assert_eq!(classify_name("unknown"), None);
        assert_eq!(
            all_tool_names(),
            vec![
                "design_action_run_get".to_owned(),
                "design_action_run_prepare".to_owned()
            ]
        );
    }
}
