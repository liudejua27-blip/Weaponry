use serde_json::{json, Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgenticTool {
    SessionCreateOrResume,
    SessionGet,
    CheckpointPrepare,
    CheckpointGet,
    CheckpointRestorePrepare,
}

impl AgenticTool {
    fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "session_create_or_resume" => Self::SessionCreateOrResume,
            "session_get" => Self::SessionGet,
            "checkpoint_prepare" => Self::CheckpointPrepare,
            "checkpoint_get" => Self::CheckpointGet,
            "checkpoint_restore_prepare" => Self::CheckpointRestorePrepare,
            _ => return None,
        })
    }

    const fn name(self) -> &'static str {
        match self {
            Self::SessionCreateOrResume => "session_create_or_resume",
            Self::SessionGet => "session_get",
            Self::CheckpointPrepare => "checkpoint_prepare",
            Self::CheckpointGet => "checkpoint_get",
            Self::CheckpointRestorePrepare => "checkpoint_restore_prepare",
        }
    }

    const fn runtime_method(self) -> &'static str {
        self.name()
    }

    const fn is_write(self) -> bool {
        matches!(
            self,
            Self::SessionCreateOrResume | Self::CheckpointPrepare | Self::CheckpointRestorePrepare
        )
    }

    const fn requires_visual_state(self) -> bool {
        matches!(
            self,
            Self::CheckpointPrepare | Self::CheckpointRestorePrepare
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Binding {
    pub session_id: Option<String>,
    pub project_id: Option<String>,
    pub candidate_id: Option<String>,
}

impl Binding {
    pub fn is_bound(&self) -> bool {
        self.session_id.is_some() && self.project_id.is_some() && self.candidate_id.is_some()
    }
}

pub fn is_tool(name: &str) -> bool {
    AgenticTool::from_name(name).is_some()
}

pub fn is_write_tool(name: &str) -> bool {
    AgenticTool::from_name(name).is_some_and(AgenticTool::is_write)
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    AgenticTool::from_name(name).map(AgenticTool::runtime_method)
}

pub fn unavailable_error(name: &str) -> String {
    let tool = AgenticTool::from_name(name).expect("agentic tool name was checked");
    format!(
        "AGENTIC_RUNTIME_METHOD_UNAVAILABLE: {} requires Runtime method {}",
        tool.name(),
        tool.runtime_method()
    )
}

pub fn read_tools() -> Vec<Value> {
    [AgenticTool::SessionGet, AgenticTool::CheckpointGet]
        .into_iter()
        .map(read_tool_definition)
        .collect()
}

pub fn write_tools() -> Vec<Value> {
    [
        AgenticTool::SessionCreateOrResume,
        AgenticTool::CheckpointPrepare,
        AgenticTool::CheckpointRestorePrepare,
    ]
    .into_iter()
    .map(write_tool_definition)
    .collect()
}

pub fn write_tool_names() -> Vec<String> {
    [
        AgenticTool::SessionCreateOrResume,
        AgenticTool::CheckpointPrepare,
        AgenticTool::CheckpointRestorePrepare,
    ]
    .into_iter()
    .map(|tool| tool.name().to_owned())
    .collect()
}

fn read_tool_definition(tool: AgenticTool) -> Value {
    debug_assert!(!tool.is_write());
    json!({
        "name": tool.name(),
        "description": read_description(tool),
        "inputSchema": read_schema(tool),
        "annotations": {
            "readOnlyHint": true,
            "destructiveHint": false,
            "idempotentHint": true,
            "openWorldHint": false,
            "writeIntent": false,
            "approvalRequired": false
        },
        "_meta": {"forgecad": {
            "availability": "available",
            "runtime_method": tool.runtime_method(),
            "requiresConfirmation": false,
            "transaction": "ADR-0026"
        }}
    })
}

fn write_tool_definition(tool: AgenticTool) -> Value {
    debug_assert!(tool.is_write());
    let (description, schema, idempotent) = match tool {
        AgenticTool::SessionCreateOrResume => (
            "Create or resume a Runtime-owned DesignSession after explicit adapter opt-in and user approval. The Runtime owns the durable record and the MCP adapter never fabricates a session.",
            session_create_schema(),
            true,
        ),
        AgenticTool::CheckpointPrepare => (
            "Prepare a Runtime-owned DesignCheckpoint for one bound session and candidate. This is a typed intent only; it is not a confirmed restore or version write.",
            checkpoint_prepare_schema(),
            true,
        ),
        AgenticTool::CheckpointRestorePrepare => (
            "Prepare a bounded restore intent for one bound checkpoint. It never moves a confirmed head and remains blocked until a separate candidate prepare and user approval.",
            checkpoint_restore_prepare_schema(),
            true,
        ),
        AgenticTool::SessionGet | AgenticTool::CheckpointGet => {
            unreachable!("read tool cannot be exposed as a write tool")
        }
    };
    json!({
        "name": tool.name(),
        "description": description,
        "inputSchema": schema,
        "annotations": {
            "readOnlyHint": false,
            "destructiveHint": false,
            "idempotentHint": idempotent,
            "openWorldHint": false,
            "writeIntent": true,
            "approvalRequired": true
        },
        "_meta": {"forgecad": {
            "availability": "available",
            "runtime_method": tool.runtime_method(),
            "requiresConfirmation": true,
            "transaction": "ADR-0026"
        }}
    })
}

fn read_description(tool: AgenticTool) -> &'static str {
    match tool {
        AgenticTool::SessionGet => {
            "Read one Runtime-owned DesignSession by its exact project and candidate binding. No local session state is created."
        }
        AgenticTool::CheckpointGet => {
            "Read one immutable Runtime-owned DesignCheckpoint by its exact session, project and candidate binding."
        }
        _ => unreachable!("write tool cannot use read description"),
    }
}

fn read_schema(tool: AgenticTool) -> Value {
    match tool {
        AgenticTool::SessionGet => scoped_schema("session_id"),
        AgenticTool::CheckpointGet => scoped_schema("checkpoint_id"),
        _ => unreachable!("write tool cannot use read schema"),
    }
}

fn scoped_schema(extra_id: &str) -> Value {
    let mut properties = scope_properties();
    if extra_id != "session_id" {
        properties.insert("session_id".to_owned(), id_property());
    }
    properties.insert(extra_id.to_owned(), id_property());
    let required = if extra_id == "session_id" {
        vec!["session_id", "project_id", "candidate_id"]
    } else {
        vec!["checkpoint_id", "session_id", "project_id", "candidate_id"]
    };
    object_schema(required, properties)
}

fn session_create_schema() -> Value {
    let mut properties = scope_properties();
    properties.insert("session_id".to_owned(), nullable_id_property());
    properties.insert("idempotency_key".to_owned(), id_property());
    properties.insert("reference_id".to_owned(), id_property());
    properties.insert("design_spec_id".to_owned(), id_property());
    properties.insert("reference_canvas_id".to_owned(), id_property());
    properties.insert("camera_hash".to_owned(), sha256_property());
    properties.insert("evidence_sha256".to_owned(), sha256_property());
    object_schema(
        vec![
            "session_id",
            "project_id",
            "candidate_id",
            "idempotency_key",
            "approved",
            "approval_receipt_id",
            "approval_summary",
        ],
        with_approval(properties),
    )
}

fn checkpoint_prepare_schema() -> Value {
    let mut properties = scope_properties();
    properties.insert("session_id".to_owned(), id_property());
    properties.insert("visual_state".to_owned(), visual_state_property());
    properties.insert("evidence_sha256".to_owned(), sha256_property());
    properties.insert("stage".to_owned(), stage_property());
    properties.insert("checkpoint_type".to_owned(), checkpoint_type_property());
    properties.insert("candidate_state_sha256".to_owned(), sha256_property());
    properties.insert("artifact_sha256".to_owned(), sha256_property());
    properties.insert("reference_id".to_owned(), id_property());
    properties.insert("reference_sha256".to_owned(), sha256_property());
    properties.insert("camera_hash".to_owned(), sha256_property());
    properties.insert("idempotency_key".to_owned(), id_property());
    object_schema(
        vec![
            "session_id",
            "project_id",
            "candidate_id",
            "visual_state",
            "evidence_sha256",
            "idempotency_key",
            "approved",
            "approval_receipt_id",
            "approval_summary",
        ],
        with_approval(properties),
    )
}

fn checkpoint_restore_prepare_schema() -> Value {
    let mut properties = scope_properties();
    properties.insert("session_id".to_owned(), id_property());
    properties.insert("checkpoint_id".to_owned(), id_property());
    properties.insert("checkpoint_sha256".to_owned(), sha256_property());
    properties.insert("visual_state".to_owned(), visual_state_property());
    properties.insert("idempotency_key".to_owned(), id_property());
    object_schema(
        vec![
            "session_id",
            "project_id",
            "candidate_id",
            "checkpoint_id",
            "visual_state",
            "idempotency_key",
            "approved",
            "approval_receipt_id",
            "approval_summary",
        ],
        with_approval(properties),
    )
}

fn scope_properties() -> Map<String, Value> {
    Map::from_iter([
        ("project_id".to_owned(), id_property()),
        ("candidate_id".to_owned(), id_property()),
    ])
}

fn with_approval(mut properties: Map<String, Value>) -> Map<String, Value> {
    properties.insert("approved".to_owned(), json!({"const": true}));
    properties.insert("approval_receipt_id".to_owned(), id_property());
    properties.insert(
        "approval_summary".to_owned(),
        json!({"type":"string","minLength":1,"maxLength":512}),
    );
    properties.insert(
        "approval_expires_at".to_owned(),
        json!({"type":"string","minLength":1,"maxLength":64}),
    );
    properties
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
    json!({"type":"string","minLength":1,"maxLength":128})
}

fn nullable_id_property() -> Value {
    json!({"type":["string","null"],"maxLength":128})
}

fn sha256_property() -> Value {
    json!({"type":"string","pattern":"^[0-9a-f]{64}$"})
}

fn visual_state_property() -> Value {
    json!({"enum":["pass","fail","unknown"]})
}

fn stage_property() -> Value {
    json!({"enum":["reference-canvas","primary-form","secondary-structure","tertiary-detail","uv-pbr","final-review"]})
}

fn checkpoint_type_property() -> Value {
    json!({"enum":["stage-entry","stage-pass","stage-fail","manual-save","rollback-source","rollback-result"]})
}

pub fn validate_call(name: &str, arguments: &Value, binding: &Binding) -> Result<(), String> {
    let Some(tool) = AgenticTool::from_name(name) else {
        return Ok(());
    };
    if tool.is_write() {
        if arguments.get("approved") != Some(&Value::Bool(true)) {
            return Err(
                "AGENTIC_APPROVAL_REQUIRED: approved=true is required for Agentic write tools"
                    .to_owned(),
            );
        }
        for key in ["approval_receipt_id", "approval_summary", "idempotency_key"] {
            if arguments
                .get(key)
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
            {
                return Err(format!(
                    "AGENTIC_APPROVAL_REQUIRED: {key} is required for Agentic writes"
                ));
            }
        }
    }
    validate_scope(tool, arguments, binding)?;
    if tool.requires_visual_state() {
        let state = arguments
            .get("visual_state")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                "AGENTIC_VISUAL_STATE_REQUIRED: checkpoint prepare requires known visual_state"
                    .to_owned()
            })?;
        if !matches!(state, "pass" | "fail") {
            return Err(
                "AGENTIC_VISUAL_STATE_UNKNOWN: unknown visual state cannot prepare or restore a checkpoint"
                    .to_owned(),
            );
        }
    }
    Ok(())
}

fn validate_scope(tool: AgenticTool, arguments: &Value, binding: &Binding) -> Result<(), String> {
    let project_id = required_string(arguments, "project_id")?;
    let candidate_id = required_string(arguments, "candidate_id")?;
    if tool == AgenticTool::SessionCreateOrResume && !binding.is_bound() {
        return Ok(());
    }
    if !binding.is_bound() {
        if matches!(tool, AgenticTool::SessionGet | AgenticTool::CheckpointGet) {
            // A fresh MCP process after Runtime restart may perform an exact,
            // read-only binding lookup before it has a local session state.
            return Ok(());
        }
        return Err(
            "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before this tool"
                .to_owned(),
        );
    }
    let session_id = required_string(arguments, "session_id")?;
    if binding.session_id.as_deref() != Some(session_id)
        || binding.project_id.as_deref() != Some(project_id)
        || binding.candidate_id.as_deref() != Some(candidate_id)
    {
        return Err(
            "AGENTIC_SCOPE_MISMATCH: session, project and candidate must remain bound to one design session"
                .to_owned(),
        );
    }
    Ok(())
}

fn required_string<'a>(arguments: &'a Value, key: &str) -> Result<&'a str, String> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("AGENTIC_INVALID_INPUT: {key} is required"))
}

/// Validate Runtime readback before it is exposed as a successful Agentic
/// response. This prevents a Runtime with a missing or divergent binding from
/// turning an unscoped payload into a usable checkpoint/session.
pub fn validate_response(name: &str, value: &Value, binding: &Binding) -> Result<(), String> {
    let Some(tool) = AgenticTool::from_name(name) else {
        return Ok(());
    };
    if !value.is_object() {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: Runtime response must be a typed object".to_owned(),
        );
    }
    let session_id = find_string(value, "session_id", 0);
    let project_id = find_string(value, "project_id", 0);
    let candidate_id = find_string(value, "candidate_id", 0);
    if session_id.is_none() || project_id.is_none() || candidate_id.is_none() {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: Runtime response is missing session/project/candidate binding"
                .to_owned(),
        );
    }
    if binding.is_bound()
        && (binding.session_id.as_deref() != session_id
            || binding.project_id.as_deref() != project_id
            || binding.candidate_id.as_deref() != candidate_id)
    {
        return Err(
            "AGENTIC_SCOPE_MISMATCH: Runtime response crossed the session project/candidate binding"
                .to_owned(),
        );
    }
    if tool == AgenticTool::SessionCreateOrResume && binding.is_bound() {
        let requested_session = find_string(value, "session_id", 0);
        if requested_session != binding.session_id.as_deref() {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: resumed session does not match the bound session"
                    .to_owned(),
            );
        }
    }
    Ok(())
}

pub fn bind_response(name: &str, value: &Value, binding: &mut Binding) -> Result<(), String> {
    validate_response(name, value, binding)?;
    if name == AgenticTool::SessionCreateOrResume.name() {
        binding.session_id = find_string(value, "session_id", 0).map(str::to_owned);
        binding.project_id = find_string(value, "project_id", 0).map(str::to_owned);
        binding.candidate_id = find_string(value, "candidate_id", 0).map(str::to_owned);
    }
    Ok(())
}

fn find_string<'a>(value: &'a Value, key: &str, depth: usize) -> Option<&'a str> {
    if depth > 4 {
        return None;
    }
    let object = value.as_object()?;
    if let Some(found) = object.get(key).and_then(Value::as_str) {
        return Some(found);
    }
    object
        .values()
        .filter(|child| child.is_object())
        .find_map(|child| find_string(child, key, depth + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approval() -> Value {
        json!({
            "approved": true,
            "approval_receipt_id": "approval-1",
            "approval_summary": "user approved checkpoint",
            "idempotency_key": "idem-1"
        })
    }

    fn bound() -> Binding {
        Binding {
            session_id: Some("session-1".to_owned()),
            project_id: Some("project-1".to_owned()),
            candidate_id: Some("candidate-1".to_owned()),
        }
    }

    #[test]
    fn annotations_keep_reads_and_prepares_distinct() {
        let reads = read_tools();
        assert_eq!(reads.len(), 2);
        assert!(reads.iter().all(|tool| {
            tool["annotations"]["readOnlyHint"] == true
                && tool["annotations"]["writeIntent"] == false
                && tool["annotations"]["approvalRequired"] == false
        }));
        for tool in write_tools() {
            assert_eq!(tool["annotations"]["readOnlyHint"], false);
            assert_eq!(tool["annotations"]["destructiveHint"], false);
            assert_eq!(tool["annotations"]["writeIntent"], true);
            assert_eq!(tool["annotations"]["approvalRequired"], true);
        }
    }

    #[test]
    fn new_session_requires_null_resume_and_explicit_approval() {
        let mut request = json!({
            "session_id": null,
            "project_id": "project-1",
            "candidate_id": "candidate-1",
            "idempotency_key": "idem-1"
        });
        assert!(validate_call("session_create_or_resume", &request, &Binding::default()).is_err());
        request["approved"] = Value::Bool(true);
        request["approval_receipt_id"] = Value::String("approval-1".to_owned());
        request["approval_summary"] = Value::String("approved".to_owned());
        assert!(validate_call("session_create_or_resume", &request, &Binding::default()).is_ok());
    }

    #[test]
    fn cross_project_candidate_and_unknown_visual_state_fail_closed() {
        let mut request = json!({
            "session_id": "session-1",
            "project_id": "project-other",
            "candidate_id": "candidate-other",
            "visual_state": "unknown",
            "evidence_sha256": "a".repeat(64),
            "idempotency_key": "idem-1"
        });
        request
            .as_object_mut()
            .unwrap()
            .extend(approval().as_object().unwrap().clone());
        let error = validate_call("checkpoint_prepare", &request, &bound()).unwrap_err();
        assert!(error.starts_with("AGENTIC_SCOPE_MISMATCH"));
        request["project_id"] = Value::String("project-1".to_owned());
        request["candidate_id"] = Value::String("candidate-1".to_owned());
        let error = validate_call("checkpoint_prepare", &request, &bound()).unwrap_err();
        assert!(error.starts_with("AGENTIC_VISUAL_STATE_UNKNOWN"));
    }

    #[test]
    fn runtime_response_must_keep_scope() {
        let response = json!({
            "session_id":"session-1",
            "project_id":"project-2",
            "candidate_id":"candidate-1"
        });
        let error = validate_response("session_get", &response, &bound()).unwrap_err();
        assert!(error.starts_with("AGENTIC_SCOPE_MISMATCH"));
    }

    #[test]
    fn readback_can_rebind_a_fresh_mcp_session() {
        let checkpoint_request = json!({
            "checkpoint_id": "checkpoint-1",
            "session_id": "session-1",
            "project_id": "project-1",
            "candidate_id": "candidate-1"
        });
        assert!(validate_call(
            "checkpoint_get",
            &checkpoint_request,
            &Binding::default()
        )
        .is_ok());
        let session_request = json!({
            "session_id": "session-1",
            "project_id": "project-1",
            "candidate_id": "candidate-1"
        });
        assert!(validate_call(
            "session_get",
            &session_request,
            &Binding::default()
        )
        .is_ok());
    }

    #[test]
    fn unavailable_error_names_assumed_runtime_method() {
        assert_eq!(
            unavailable_error("checkpoint_prepare"),
            "AGENTIC_RUNTIME_METHOD_UNAVAILABLE: checkpoint_prepare requires Runtime method checkpoint_prepare"
        );
    }
}
