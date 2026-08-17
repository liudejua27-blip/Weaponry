use crate::optimization_tools::intent_property;
use serde_json::{json, Value};

const NAME: &str = "design_stage_run_prepare";
const COMPOSITION_NAME: &str = "design_composition_prepare";
const STAGES: [&str; 6] = [
    "reference-canvas",
    "primary-form",
    "secondary-structure",
    "tertiary-detail",
    "uv-pbr",
    "final-review",
];
const ACTION_KINDS: [&str; 6] = [
    "checkpoint",
    "primary-blockout",
    "primary-form-adjustment",
    "secondary-structure",
    "tertiary-detail",
    "bounded-repair",
];
const STAGE_FIELDS: [&str; 14] = [
    "project_id",
    "session_id",
    "candidate_id",
    "batch_id",
    "requested_stage",
    "actions",
    "observation_sha256",
    "input_sha256",
    "approved",
    "approval_receipt_id",
    "approval_summary",
    "approval_expires_at",
    "approval_session_id",
    "idempotency_key",
];
const COMPOSITION_FIELDS: [&str; 15] = [
    "project_id",
    "session_id",
    "candidate_id",
    "composition_id",
    "requested_stage",
    "actions",
    "observation_sha256",
    "input_sha256",
    "approved",
    "approval_receipt_id",
    "approval_summary",
    "approval_expires_at",
    "approval_session_id",
    "idempotency_key",
    "merge",
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Binding {
    pub project_id: Option<String>,
    pub session_id: Option<String>,
    pub candidate_id: Option<String>,
    pub batch_id: Option<String>,
    pub composition_id: Option<String>,
}

pub fn is_tool(name: &str) -> bool {
    matches!(name, NAME | COMPOSITION_NAME)
}

pub fn is_write_tool(name: &str) -> bool {
    is_tool(name)
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    match name {
        NAME => Some(NAME),
        COMPOSITION_NAME => Some(COMPOSITION_NAME),
        _ => None,
    }
}

pub fn write_tool_names() -> Vec<String> {
    vec![NAME.to_owned(), COMPOSITION_NAME.to_owned()]
}

pub fn write_tools() -> Vec<Value> {
    vec![stage_tool_definition(), composition_tool_definition()]
}

fn stage_tool_definition() -> Value {
    json!({
        "name":NAME,
        "description":"Execute a bounded ordered stage batch of independent Runtime-owned DesignActionRun receipts. A geometry action may carry typed parameter_changes plus a candidate-bound view_spec so Runtime can materialize a constrained parameter patch; the batch resumes by exact batch_id/input_sha256 through RuntimeJob events, stops at the first blocked quality gate, and never promotes, confirms or exports proposal candidates.",
        "inputSchema":input_schema(),
        "annotations":{
            "readOnlyHint":false,
            "destructiveHint":false,
            "idempotentHint":true,
            "openWorldHint":false,
            "writeIntent":true,
            "approvalRequired":true
        },
        "_meta":{"forgecad":{
            "availability":"available",
            "runtime_method":NAME,
            "requiresConfirmation":true,
            "transaction":"ADR-0026",
            "execution_mode":"independent-reviewable-actions",
            "definition_only":false
        }}
    })
}

fn composition_tool_definition() -> Value {
    json!({
        "name":COMPOSITION_NAME,
        "description":"Prepare an explicit ordered composition proposal from 2-6 typed geometry actions. Each action is independently evidenced; an optional cumulative-program merge envelope hash-links complete GeometryProgram@2 states and compiles the final state into a distinct review candidate. The tool never confirms, versions or exports.",
        "inputSchema":composition_input_schema(),
        "annotations":{
            "readOnlyHint":false,
            "destructiveHint":false,
            "idempotentHint":true,
            "openWorldHint":false,
            "writeIntent":true,
            "approvalRequired":true
        },
        "_meta":{"forgecad":{
            "availability":"available",
            "runtime_method":COMPOSITION_NAME,
            "requiresConfirmation":true,
            "transaction":"ADR-0026",
            "execution_mode":"ordered-independent-proposal-with-optional-cumulative-merge",
            "definition_only":false,
            "promotion":"separate-explicit-transaction"
        }}
    })
}

fn input_schema() -> Value {
    json!({
        "type":"object",
        "required":[
            "project_id","session_id","candidate_id","batch_id","requested_stage","actions",
            "observation_sha256","input_sha256","approved","approval_receipt_id","approval_summary","approval_expires_at","idempotency_key"
        ],
        "properties":{
            "project_id":id_property(),
            "session_id":id_property(),
            "candidate_id":id_property(),
            "batch_id":id_property(),
            "requested_stage":{"enum":STAGES},
            "actions":{
                "type":"array",
                "minItems":1,
                "maxItems":6,
                "items":{
                    "type":"object",
                    "required":["run_id","action"],
                "properties":{
                    "run_id":id_property(),
                    "action":action_schema(),
                    "proposal":{"type":["object","null"]},
                    "optimization_intent":intent_property(),
                    "view_spec":{"type":"object"}
                },
                    "additionalProperties":false
                }
            },
            "observation_sha256":sha256_property(),
            "input_sha256":sha256_property(),
            "approved":{"const":true},
            "approval_receipt_id":id_property(),
            "approval_summary":{"type":"string","minLength":1,"maxLength":512},
            "approval_expires_at":{"type":"string","minLength":1,"maxLength":64},
            "approval_session_id":id_property(),
            "idempotency_key":id_property()
        },
        "additionalProperties":false
    })
}

fn composition_merge_schema() -> Value {
    json!({
        "type":"object",
        "required":["mode","steps","final_step_index"],
        "properties":{
            "mode":{"const":"cumulative-program"},
            "steps":{
                "type":"array",
                "minItems":2,
                "maxItems":6,
                "items":{
                    "type":"object",
                    "required":["run_id","parent_program_sha256","program_sha256"],
                    "properties":{
                        "run_id":id_property(),
                        "parent_program_sha256":sha256_property(),
                        "program_sha256":sha256_property()
                    },
                    "additionalProperties":false
                }
            },
            "final_step_index":{"type":"integer","minimum":1,"maximum":5}
        },
        "additionalProperties":false
    })
}

fn action_schema() -> Value {
    json!({
        "type":"object",
        "required":["action_id","action_kind","scope_kind","target_id","operator_id","parameter_changes","bounded"],
        "properties":{
            "action_id":id_property(),
            "action_kind":{"enum":ACTION_KINDS},
            "scope_kind":{"enum":["session","part","material-zone","reference"]},
            "target_id":{"type":["string","null"]},
            "operator_id":{"type":["string","null"]},
            "parameter_changes":{"type":"array","maxItems":8},
            "bounded":{"const":true},
            "description":{"type":"string","minLength":1,"maxLength":512}
        },
        "additionalProperties":false
    })
}

fn composition_input_schema() -> Value {
    json!({
        "type":"object",
        "required":[
            "project_id","session_id","candidate_id","composition_id","requested_stage","actions",
            "observation_sha256","input_sha256","approved","approval_receipt_id","approval_summary","approval_expires_at","idempotency_key"
        ],
        "properties":{
            "project_id":id_property(),
            "session_id":id_property(),
            "candidate_id":id_property(),
            "composition_id":id_property(),
            "requested_stage":{"enum":STAGES},
            "actions":{
                "type":"array",
                "minItems":2,
                "maxItems":6,
                "items":{
                    "type":"object",
                    "required":["run_id","depends_on","action","proposal"],
                    "properties":{
                        "run_id":id_property(),
                        "depends_on":{"type":"array","maxItems":1,"items":id_property()},
                        "action":composition_action_schema(),
                        "proposal":composition_proposal_schema()
                    },
                    "additionalProperties":false
                }
            },
            "observation_sha256":sha256_property(),
            "input_sha256":sha256_property(),
            "approved":{"const":true},
            "approval_receipt_id":id_property(),
            "approval_summary":{"type":"string","minLength":1,"maxLength":512},
            "approval_expires_at":{"type":"string","minLength":1,"maxLength":64},
            "approval_session_id":id_property(),
            "idempotency_key":id_property(),
            "merge":{
                "oneOf":[
                    {"type":"null"},
                    composition_merge_schema()
                ]
            }
        },
        "additionalProperties":false
    })
}

fn composition_action_schema() -> Value {
    json!({
        "type":"object",
        "required":["action_id","action_kind","scope_kind","target_id","operator_id","parameter_changes","bounded"],
        "properties":{
            "action_id":id_property(),
            "action_kind":{"enum":["primary-blockout","primary-form-adjustment","secondary-structure","tertiary-detail","bounded-repair"]},
            "scope_kind":{"const":"part"},
            "target_id":id_property(),
            "operator_id":{"type":"string","minLength":1,"maxLength":128},
            "parameter_changes":{"type":"array","maxItems":8},
            "bounded":{"const":true},
            "description":{"type":"string","minLength":1,"maxLength":512}
        },
        "additionalProperties":false
    })
}

fn composition_proposal_schema() -> Value {
    json!({
        "type":"object",
        "required":["repair_intent","geometry_program","view_spec","camera"],
        "properties":{
            "repair_intent":{"type":"object","additionalProperties":true},
            "geometry_program":{"type":"object","additionalProperties":true},
            "view_spec":{"type":"object","additionalProperties":true},
            "camera":{"type":"object","additionalProperties":true},
            "view_evaluations":{"type":"array","minItems":2,"maxItems":8,"items":{"type":"object","additionalProperties":true}}
        },
        "additionalProperties":false
    })
}

fn id_property() -> Value {
    json!({"type":"string","pattern":"^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"})
}

fn sha256_property() -> Value {
    json!({"type":"string","pattern":"^[0-9a-f]{64}$"})
}

pub fn validate_call(name: &str, arguments: &Value, binding: &Binding) -> Result<(), String> {
    if !is_tool(name) {
        return Ok(());
    }
    let object = arguments
        .as_object()
        .ok_or_else(|| "DESIGN_STAGE_INVALID_INPUT: arguments must be an object".to_owned())?;
    let fields = if name == NAME {
        &STAGE_FIELDS[..]
    } else {
        &COMPOSITION_FIELDS[..]
    };
    if let Some(key) = object.keys().find(|key| !fields.contains(&key.as_str())) {
        return Err(format!(
            "DESIGN_ORCHESTRATOR_INVALID_INPUT: unsupported field {key}"
        ));
    }
    let mut scoped = vec![
        ("project_id", binding.project_id.as_deref()),
        ("session_id", binding.session_id.as_deref()),
        ("candidate_id", binding.candidate_id.as_deref()),
    ];
    if name == NAME {
        scoped.push(("batch_id", binding.batch_id.as_deref()));
    } else {
        scoped.push(("composition_id", binding.composition_id.as_deref()));
    }
    for (key, expected) in scoped {
        if let Some(expected) = expected {
            if object.get(key).and_then(Value::as_str) != Some(expected) {
                return Err(format!(
                    "DESIGN_ORCHESTRATOR_SCOPE_MISMATCH: {key} differs from bound request"
                ));
            }
        }
    }
    let required = if name == NAME {
        [
            "project_id",
            "session_id",
            "candidate_id",
            "batch_id",
            "requested_stage",
            "observation_sha256",
            "input_sha256",
            "approval_receipt_id",
            "approval_summary",
            "approval_expires_at",
            "idempotency_key",
        ]
    } else {
        [
            "project_id",
            "session_id",
            "candidate_id",
            "composition_id",
            "requested_stage",
            "observation_sha256",
            "input_sha256",
            "approval_receipt_id",
            "approval_summary",
            "approval_expires_at",
            "idempotency_key",
        ]
    };
    for key in required {
        if object
            .get(key)
            .and_then(Value::as_str)
            .is_none_or(|value| value.is_empty())
        {
            return Err(format!(
                "DESIGN_ORCHESTRATOR_INVALID_INPUT: {key} is required"
            ));
        }
    }
    if object.get("approved") != Some(&Value::Bool(true)) {
        return Err("DESIGN_ORCHESTRATOR_APPROVAL_REQUIRED: approved=true is required".to_owned());
    }
    let actions = object
        .get("actions")
        .and_then(Value::as_array)
        .ok_or_else(|| "DESIGN_STAGE_INVALID_INPUT: actions is required".to_owned())?;
    if name == NAME && (actions.is_empty() || actions.len() > 6) {
        return Err("DESIGN_STAGE_ACTION_COUNT_OUT_OF_BOUNDS".to_owned());
    }
    if name == COMPOSITION_NAME && !(2..=6).contains(&actions.len()) {
        return Err("DESIGN_COMPOSITION_ACTION_COUNT_OUT_OF_BOUNDS".to_owned());
    }
    if name == COMPOSITION_NAME
        && object
            .get("merge")
            .is_some_and(|merge| !merge.is_null() && !merge.is_object())
    {
        return Err("DESIGN_COMPOSITION_MERGE_INVALID: merge must be an object or null".to_owned());
    }
    Ok(())
}

pub fn validate_response(name: &str, value: &Value, binding: &Binding) -> Result<(), String> {
    if !is_tool(name) {
        return Ok(());
    }
    let object = value
        .as_object()
        .ok_or_else(|| "DESIGN_STAGE_RESPONSE_INVALID: response must be an object".to_owned())?;
    let expected_schema = if name == NAME {
        "DesignActionBatchResult@1"
    } else {
        "DesignCompositionResult@1"
    };
    if object.get("schema_version").and_then(Value::as_str) != Some(expected_schema) {
        return Err("DESIGN_ORCHESTRATOR_RESPONSE_INVALID: schema version differs".to_owned());
    }
    let mut scoped = vec![
        ("project_id", binding.project_id.as_deref()),
        ("session_id", binding.session_id.as_deref()),
        ("candidate_id", binding.candidate_id.as_deref()),
    ];
    if name == NAME {
        scoped.push(("batch_id", binding.batch_id.as_deref()));
    } else {
        scoped.push(("composition_id", binding.composition_id.as_deref()));
    }
    for (key, expected) in scoped {
        if let Some(expected) = expected {
            if object.get(key).and_then(Value::as_str) != Some(expected) {
                return Err(format!(
                    "DESIGN_ORCHESTRATOR_RESPONSE_SCOPE_MISMATCH: {key} differs"
                ));
            }
        }
    }
    Ok(())
}

pub fn bind_response(name: &str, value: &Value, binding: &mut Binding) -> Result<(), String> {
    validate_response(name, value, binding)?;
    let object = value
        .as_object()
        .ok_or_else(|| "DESIGN_STAGE_RESPONSE_INVALID: response must be an object".to_owned())?;
    let keys = if name == NAME {
        ["project_id", "session_id", "candidate_id", "batch_id"]
    } else {
        ["project_id", "session_id", "candidate_id", "composition_id"]
    };
    for key in keys {
        let value = object
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("DESIGN_ORCHESTRATOR_RESPONSE_INVALID: {key} is missing"))?;
        let slot = match key {
            "project_id" => &mut binding.project_id,
            "session_id" => &mut binding.session_id,
            "candidate_id" => &mut binding.candidate_id,
            "batch_id" => &mut binding.batch_id,
            "composition_id" => &mut binding.composition_id,
            _ => unreachable!(),
        };
        if slot.as_deref().is_some_and(|expected| expected != value) {
            return Err(format!(
                "DESIGN_ORCHESTRATOR_RESPONSE_SCOPE_MISMATCH: {key} cannot rebind"
            ));
        }
        *slot = Some(value.to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_batch_tool_is_approval_gated_and_idempotent() {
        let tools = write_tools();
        assert_eq!(tools.len(), 2);
        let stage = tools
            .iter()
            .find(|tool| tool["name"] == NAME)
            .expect("stage tool");
        assert_eq!(stage["annotations"]["approvalRequired"], true);
        assert_eq!(stage["annotations"]["destructiveHint"], false);
        assert_eq!(stage["inputSchema"]["properties"]["merge"], Value::Null);
        assert_eq!(
            stage["inputSchema"]["properties"]["actions"]["items"]["properties"]["view_spec"]
                ["type"],
            "object"
        );
        assert!(stage["inputSchema"]["required"]
            .as_array()
            .expect("stage required")
            .iter()
            .any(|value| value == "approval_expires_at"));
        assert!(tools.iter().any(|tool| tool["name"] == COMPOSITION_NAME));
        let arguments = json!({
            "project_id":"project-1",
            "session_id":"session-1",
            "candidate_id":"candidate-1",
            "batch_id":"batch-1",
            "requested_stage":"primary-form",
            "actions":[],
            "input_sha256":"a".repeat(64),
            "approved":true,
            "approval_receipt_id":"approval-1",
            "approval_summary":"Run bounded stage actions",
            "idempotency_key":"batch-idempotency"
        });
        assert!(validate_call(NAME, &arguments, &Binding::default()).is_err());
    }

    #[test]
    fn composition_tool_requires_ordered_dependencies_and_is_not_destructive() {
        let tools = write_tools();
        let composition = tools
            .iter()
            .find(|tool| tool["name"] == COMPOSITION_NAME)
            .expect("composition tool");
        assert_eq!(composition["annotations"]["destructiveHint"], false);
        assert_eq!(
            composition["_meta"]["forgecad"]["promotion"],
            "separate-explicit-transaction"
        );
        assert!(composition["inputSchema"]["properties"]["merge"].is_object());
        assert!(composition["inputSchema"]["required"]
            .as_array()
            .expect("composition required")
            .iter()
            .any(|value| value == "approval_expires_at"));
        let arguments = json!({
            "project_id":"project-1",
            "session_id":"session-1",
            "candidate_id":"candidate-1",
            "composition_id":"composition-1",
            "requested_stage":"primary-form",
            "actions":[{"run_id":"run-1","depends_on":[],"action":{},"proposal":{}}],
            "input_sha256":"a".repeat(64),
            "approved":true,
            "approval_receipt_id":"approval-1",
            "approval_summary":"Compose bounded actions",
            "approval_expires_at":"9999999999",
            "idempotency_key":"composition-idempotency"
        });
        assert!(validate_call(COMPOSITION_NAME, &arguments, &Binding::default()).is_err());
    }

    #[test]
    fn runtime_methods_preserve_stage_and_composition_names() {
        assert_eq!(runtime_method(NAME), Some(NAME));
        assert_eq!(runtime_method(COMPOSITION_NAME), Some(COMPOSITION_NAME));
        assert_eq!(runtime_method("unknown"), None);
    }
}
