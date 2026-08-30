//! Historical Agentic session/checkpoint/recovery compatibility contracts.
//!
//! This module owns only the session/checkpoint family that is exposed by the
//! explicit `forgecad-mcp-compat` binary.  It deliberately reuses the parent
//! module's shared schema helpers and `AgenticTool` enum; it does not declare a
//! second tool table or schema registry.

use super::{
    id_property, nullable_id_property, object_schema, required_string, scope_properties,
    sha256_property, stage_property, visual_state_property, with_approval, AgenticTool, Binding,
};
use serde_json::{json, Value};

pub(super) fn read_description(tool: AgenticTool) -> Option<&'static str> {
    Some(match tool {
        AgenticTool::SessionGet => {
            "Read one Runtime-owned DesignSession by its exact project and candidate binding. No local session state is created."
        }
        AgenticTool::CheckpointGet => {
            "Read one immutable Runtime-owned DesignCheckpoint by its exact session, project and candidate binding."
        }
        _ => return None,
    })
}

pub(super) fn read_schema(tool: AgenticTool) -> Option<Value> {
    Some(match tool {
        AgenticTool::SessionGet => scoped_schema("session_id"),
        AgenticTool::CheckpointGet => scoped_schema("checkpoint_id"),
        _ => return None,
    })
}

pub(super) fn write_definition(tool: AgenticTool) -> Option<(&'static str, Value, bool)> {
    Some(match tool {
        AgenticTool::SessionCreateOrResume => (
            "Create or resume a Runtime-owned DesignSession after explicit adapter opt-in and user approval. The Runtime owns the durable record; an optional typed authoring_context can provide hash-bound multi-view ReferenceCanvas and DesignSpec facts, while omitted context receives the conservative single-reference unknown model. The MCP adapter never fabricates a session.",
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
        _ => return None,
    })
}

pub(super) fn validate_scope(
    tool: AgenticTool,
    arguments: &Value,
    binding: &Binding,
) -> Option<Result<(), String>> {
    if !matches!(
        tool,
        AgenticTool::SessionCreateOrResume
            | AgenticTool::SessionGet
            | AgenticTool::CheckpointPrepare
            | AgenticTool::CheckpointGet
            | AgenticTool::CheckpointRestorePrepare
    ) {
        return None;
    }

    let project_id = match required_string(arguments, "project_id") {
        Ok(value) => value,
        Err(error) => return Some(Err(error)),
    };
    let candidate_id = match required_string(arguments, "candidate_id") {
        Ok(value) => value,
        Err(error) => return Some(Err(error)),
    };
    if tool == AgenticTool::SessionCreateOrResume && !binding.is_bound() {
        return Some(Ok(()));
    }
    if !binding.is_bound() {
        if matches!(tool, AgenticTool::SessionGet | AgenticTool::CheckpointGet) {
            // A fresh MCP process after Runtime restart may perform an exact,
            // read-only binding lookup before it has a local session state.
            return Some(Ok(()));
        }
        return Some(Err(
            "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before this tool"
                .to_owned(),
        ));
    }
    let session_id = match required_string(arguments, "session_id") {
        Ok(value) => value,
        Err(error) => return Some(Err(error)),
    };
    if binding.session_id.as_deref() != Some(session_id)
        || binding.project_id.as_deref() != Some(project_id)
        || binding.candidate_id.as_deref() != Some(candidate_id)
    {
        return Some(Err(
            "AGENTIC_SCOPE_MISMATCH: session, project and candidate must remain bound to one design session"
                .to_owned(),
        ));
    }
    Some(Ok(()))
}

/// `ReferenceCanvas@1` permits an exact typed, non-secret
/// `views[*].authorization` provenance claim.  The parent transport guard
/// treats the same key as a possible credential, so only this schema-owned
/// path is removed before the generic guard runs.
pub(super) fn contains_forbidden_transport_field_for_tool(
    tool: AgenticTool,
    value: &Value,
) -> Option<bool> {
    if tool != AgenticTool::SessionCreateOrResume {
        return None;
    }
    let mut guarded = value.clone();
    if let Some(views) = guarded
        .pointer_mut("/authoring_context/reference_canvas/views")
        .and_then(Value::as_array_mut)
    {
        for view in views {
            if let Some(view) = view.as_object_mut() {
                view.remove("authorization");
            }
        }
    }
    Some(super::contains_forbidden_transport_field(&guarded))
}

pub(super) fn validate_response_binding(
    tool: AgenticTool,
    value: &Value,
    binding: &Binding,
    session_id: Option<&str>,
) -> Option<Result<(), String>> {
    if tool != AgenticTool::SessionCreateOrResume || !binding.is_bound() {
        return None;
    }
    let requested_session = super::find_string(value, "session_id", 0);
    if requested_session != binding.session_id.as_deref() {
        return Some(Err(
            "AGENTIC_SCOPE_MISMATCH: resumed session does not match the bound session".to_owned(),
        ));
    }
    debug_assert_eq!(requested_session, session_id);
    Some(Ok(()))
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
        vec![extra_id, "session_id", "project_id", "candidate_id"]
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
    properties.insert(
        "authoring_context".to_owned(),
        json!({
            "type":"object",
            "required":["reference_canvas","design_spec"],
            "properties":{
                "reference_canvas":{"type":"object","maxProperties":16},
                "design_spec":{"type":"object","maxProperties":16}
            },
            "additionalProperties":false
        }),
    );
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

fn checkpoint_type_property() -> Value {
    json!({"enum":["stage-entry","stage-pass","stage-fail","manual-save","rollback-source","rollback-result"]})
}
