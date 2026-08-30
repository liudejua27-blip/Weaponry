//! MCP façade for the bounded knife Curve-derived EvaluatedMesh slice.
//!
//! These operations are façade-native and intentionally absent from the
//! compatibility raw-tool manifest.  Runtime owns curve evaluation, durable
//! Store/CAS records and all canonical hashes; this module only closes the
//! MCP envelope and keeps the wire response bounded.

use serde_json::{json, Value};
use std::collections::BTreeSet;

const GET_NAME: &str = "knife_curve_evaluated_mesh_get";
const PREPARE_NAME: &str = "knife_curve_evaluated_mesh_prepare";
const GET_SCHEMA: &str = include_str!(
    "../../../../../../packages/forgecad-contracts/schemas/knife-curve-evaluated-mesh-get-request.schema.json"
);
const PREPARE_SCHEMA: &str = include_str!(
    "../../../../../../packages/forgecad-contracts/schemas/knife-curve-evaluated-mesh-prepare-request.schema.json"
);
const RESULT_SCHEMA: &str = include_str!(
    "../../../../../../packages/forgecad-contracts/schemas/knife-curve-evaluated-mesh-result.schema.json"
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnifeCurveEvaluatedMeshTool {
    Get,
    Prepare,
}

impl KnifeCurveEvaluatedMeshTool {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Get => GET_NAME,
            Self::Prepare => PREPARE_NAME,
        }
    }

    pub const fn is_write(self) -> bool {
        matches!(self, Self::Prepare)
    }

    pub const fn runtime_method(self) -> &'static str {
        self.name()
    }
}

pub fn from_name(name: &str) -> Option<KnifeCurveEvaluatedMeshTool> {
    Some(match name {
        GET_NAME => KnifeCurveEvaluatedMeshTool::Get,
        PREPARE_NAME => KnifeCurveEvaluatedMeshTool::Prepare,
        _ => return None,
    })
}

pub fn is_tool(name: &str) -> bool {
    from_name(name).is_some()
}

pub fn is_write_tool(name: &str) -> bool {
    from_name(name).is_some_and(KnifeCurveEvaluatedMeshTool::is_write)
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    from_name(name).map(KnifeCurveEvaluatedMeshTool::runtime_method)
}

pub fn unavailable_error(name: &str) -> String {
    format!(
        "KNIFE_CURVE_EVALUATED_MESH_RUNTIME_METHOD_UNAVAILABLE: {name} requires Runtime method {name}"
    )
}

pub fn read_tool_names() -> Vec<String> {
    vec![GET_NAME.to_owned()]
}

pub fn write_tool_names() -> Vec<String> {
    vec![PREPARE_NAME.to_owned()]
}

pub fn operation_names() -> [&'static str; 2] {
    [GET_NAME, PREPARE_NAME]
}

/// Return the checked-in closed request schema. Parsing at startup makes a
/// malformed contract fail closed rather than widening the public façade.
pub fn input_schema(name: &str) -> Option<Value> {
    let source = match name {
        GET_NAME => GET_SCHEMA,
        PREPARE_NAME => PREPARE_SCHEMA,
        _ => return None,
    };
    serde_json::from_str(source).ok()
}

pub fn result_schema() -> Value {
    serde_json::from_str(RESULT_SCHEMA).expect("knife Curve EvaluatedMesh result schema is valid")
}

/// Validate the exact public request envelope before Runtime dispatch. Deep
/// source/plan/hash semantics stay owned by the Runtime implementation.
pub fn validate_call(name: &str, arguments: &Value) -> Result<(), String> {
    let schema = input_schema(name).ok_or_else(|| {
        "KNIFE_CURVE_EVALUATED_MESH_INVALID: operation is not façade-native".to_owned()
    })?;
    let object = arguments.as_object().ok_or_else(|| {
        "KNIFE_CURVE_EVALUATED_MESH_INVALID: request must be an object".to_owned()
    })?;
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .ok_or_else(|| "KNIFE_CURVE_EVALUATED_MESH_SCHEMA_INVALID: required is missing".to_owned())?
        .iter()
        .map(|value| {
            value.as_str().ok_or_else(|| {
                "KNIFE_CURVE_EVALUATED_MESH_SCHEMA_INVALID: required contains a non-string"
                    .to_owned()
            })
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let declared = schema
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "KNIFE_CURVE_EVALUATED_MESH_SCHEMA_INVALID: properties are missing".to_owned()
        })?
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if required != declared || actual != declared {
        return Err(
            "KNIFE_CURVE_EVALUATED_MESH_INVALID: request fields differ from the closed contract"
                .to_owned(),
        );
    }
    let expected_version = if name == GET_NAME {
        "KnifeCurveEvaluatedMeshGetRequest@1"
    } else {
        "KnifeCurveEvaluatedMeshPrepareRequest@1"
    };
    if object.get("schema_version").and_then(Value::as_str) != Some(expected_version)
        || object.get("operation").and_then(Value::as_str) != Some(name)
    {
        return Err(
            "KNIFE_CURVE_EVALUATED_MESH_INVALID: schema_version or operation differs".to_owned(),
        );
    }
    Ok(())
}

pub fn read_tools() -> Vec<Value> {
    vec![tool_definition(KnifeCurveEvaluatedMeshTool::Get)]
}

pub fn write_tools() -> Vec<Value> {
    vec![tool_definition(KnifeCurveEvaluatedMeshTool::Prepare)]
}

fn tool_definition(tool: KnifeCurveEvaluatedMeshTool) -> Value {
    let description = match tool {
        KnifeCurveEvaluatedMeshTool::Get => {
            "Read one durable knife curve-derived EvaluatedMesh identity/link receipt. This is read-only and never returns mesh vertices or triangle buffers, creates geometry, advances a stage, confirms a candidate, creates a version or exports."
        }
        KnifeCurveEvaluatedMeshTool::Prepare => {
            "Prepare one bounded knife curve sweep-loft EvaluatedMesh receipt. Runtime evaluates typed curve/ModifierGraph bindings and durably records hash-only mesh identity/link data; no editable geometry artifact is created. Explicit authenticated MCP write opt-in is required."
        }
    };
    json!({
        "name": tool.name(),
        "description": description,
        "inputSchema": input_schema(tool.name()).expect("knife EvaluatedMesh schema is valid"),
        "annotations": {
            "readOnlyHint": !tool.is_write(),
            "destructiveHint": false,
            "idempotentHint": true,
            "openWorldHint": false,
            "writeIntent": tool.is_write(),
            "approvalRequired": false
        },
        "_meta": {"forgecad": {
            "availability": "available",
            "runtime_method": tool.runtime_method(),
            "requiresConfirmation": false,
            "transaction": "KnifeCurveEvaluatedMesh@1",
            "facadeNative": true,
            "resultSchema": "KnifeCurveEvaluatedMeshResult@1",
            "maxResponseBytes": 1048576,
            "meshBuffersOnWire": false
        }}
    })
}

/// Keep text content hash/count-only. `structuredContent` remains the exact
/// Runtime receipt, which has no vertex/triangle buffers by contract.
pub fn summary(name: &str, value: &Value) -> Option<String> {
    if !is_tool(name) {
        return None;
    }
    serde_json::to_string(&json!({
        "schema_version": "KnifeCurveEvaluatedMeshMcpSummary@1",
        "operation": name,
        "status": value.get("status"),
        "project_id": value.get("project_id"),
        "source_candidate_id": value.get("source_candidate_id"),
        "source_authoring_mesh_revision_sha256": value.get("source_authoring_mesh_revision_sha256"),
        "source_modifier_graph_sha256": value.get("source_modifier_graph_sha256"),
        "curve_graph_lookup_key_sha256": value.get("curve_graph_lookup_key_sha256"),
        "evaluated_mesh_lookup_key_sha256": value.get("evaluated_mesh_lookup_key_sha256"),
        "evaluation_plan_object_sha256": value.get("evaluation_plan_object_sha256"),
        "evaluation_plan_semantic_sha256": value.get("evaluation_plan_semantic_sha256"),
        "evaluated_mesh_id": value.get("evaluated_mesh_id"),
        "evaluated_mesh_object_sha256": value.get("evaluated_mesh_object_sha256"),
        "evaluated_mesh_semantic_sha256": value.get("evaluated_mesh_semantic_sha256"),
        "evaluated_mesh_identity_sha256": value.get("evaluated_mesh_identity_sha256"),
        "evaluated_mesh_link_sha256": value.get("evaluated_mesh_link_sha256"),
        "vertex_count": value.get("vertex_count"),
        "triangle_count": value.get("triangle_count"),
        "mesh_readback_status": value.get("mesh_readback_status"),
        "evaluation_status": value.get("evaluation_status"),
        "evaluated_mesh_created": value.get("evaluated_mesh_created"),
        "geometry_artifact_created": value.get("geometry_artifact_created"),
        "replayed": value.get("replayed"),
        "restart_hash_verified": value.get("restart_hash_verified"),
        "runtime_write_performed": value.get("runtime_write_performed"),
        "persistent_user_data_touched": value.get("persistent_user_data_touched"),
        "quality_status": value.get("quality_status"),
        "visual_status": value.get("visual_status"),
        "human_status": value.get("human_status"),
        "engine_status": value.get("engine_status"),
        "canonical_sha256": value.get("canonical_sha256"),
        "structured_content_complete": true
    }))
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_is_facade_native_and_schema_closed() {
        assert_eq!(operation_names(), [GET_NAME, PREPARE_NAME]);
        for name in operation_names() {
            let schema = input_schema(name).expect("schema");
            assert_eq!(schema["type"], "object");
            assert_eq!(schema["additionalProperties"], false);
            assert!(validate_call(name, &json!({})).is_err());
        }
        assert!(is_write_tool(PREPARE_NAME));
        assert!(!is_write_tool(GET_NAME));
        assert_eq!(read_tools().len(), 1);
        assert_eq!(write_tools().len(), 1);
        assert_eq!(result_schema()["additionalProperties"], false);
    }

    #[test]
    fn summary_does_not_copy_mesh_buffers() {
        let summary = summary(
            PREPARE_NAME,
            &json!({
                "evaluated_mesh_id":"mesh-1",
                "evaluated_mesh_object_sha256":"a".repeat(64),
                "evaluated_mesh_identity_sha256":"b".repeat(64),
                "evaluated_mesh_link_sha256":"c".repeat(64),
                "vertex_count":256,
                "triangle_count":512
            }),
        )
        .expect("summary");
        assert!(!summary.contains("vertices"));
        assert!(!summary.contains("faces"));
        assert!(!summary.contains("mesh_buffer"));
        assert!(summary.contains("vertex_count"));
        assert!(summary.contains("triangle_count"));
    }
}
