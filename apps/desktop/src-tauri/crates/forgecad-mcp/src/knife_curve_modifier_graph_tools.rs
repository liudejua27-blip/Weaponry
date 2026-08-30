//! MCP surface for the knife Curve + ModifierGraph vertical slice.
//!
//! These operations are deliberately façade-native.  They are not added to
//! the compatibility raw-tool manifest: the knife profile is the only public
//! route that exposes them.  Runtime remains the sole writer and owns all
//! canonicalization, curve sampling, graph planning and durable readback.

use serde_json::{json, Value};
use std::collections::BTreeSet;

const GET_NAME: &str = "knife_curve_modifier_graph_get";
const PREPARE_NAME: &str = "knife_curve_modifier_graph_prepare";
const GET_SCHEMA: &str = include_str!(
    "../../../../../../packages/forgecad-contracts/schemas/knife-curve-modifier-graph-get-request.schema.json"
);
const PREPARE_SCHEMA: &str = include_str!(
    "../../../../../../packages/forgecad-contracts/schemas/knife-curve-modifier-graph-prepare-request.schema.json"
);
const RESULT_SCHEMA: &str = include_str!(
    "../../../../../../packages/forgecad-contracts/schemas/knife-curve-modifier-graph-result.schema.json"
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnifeCurveModifierGraphTool {
    Get,
    Prepare,
}

impl KnifeCurveModifierGraphTool {
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

pub fn from_name(name: &str) -> Option<KnifeCurveModifierGraphTool> {
    Some(match name {
        GET_NAME => KnifeCurveModifierGraphTool::Get,
        PREPARE_NAME => KnifeCurveModifierGraphTool::Prepare,
        _ => return None,
    })
}

pub fn is_tool(name: &str) -> bool {
    from_name(name).is_some()
}

pub fn is_write_tool(name: &str) -> bool {
    from_name(name).is_some_and(KnifeCurveModifierGraphTool::is_write)
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    from_name(name).map(KnifeCurveModifierGraphTool::runtime_method)
}

pub fn unavailable_error(name: &str) -> String {
    format!(
        "KNIFE_CURVE_MODIFIER_GRAPH_RUNTIME_METHOD_UNAVAILABLE: {name} requires Runtime method {name}"
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

/// Return the closed contract schema used by the façade profile.  Parsing it
/// at startup makes a malformed checked-in contract fail closed instead of
/// silently widening the MCP surface.
pub fn input_schema(name: &str) -> Option<Value> {
    let source = match name {
        GET_NAME => GET_SCHEMA,
        PREPARE_NAME => PREPARE_SCHEMA,
        _ => return None,
    };
    serde_json::from_str(source).ok()
}

pub fn result_schema() -> Value {
    serde_json::from_str(RESULT_SCHEMA).expect("knife Curve+ModifierGraph result schema is valid")
}

/// Validate the exact public request envelope before Runtime dispatch. Runtime
/// remains responsible for the deep typed Curve/ModifierGraph validation;
/// this closes the MCP root without relying on the legacy shallow validator,
/// which intentionally does not implement draft-2020 `$ref` resolution.
pub fn validate_call(name: &str, arguments: &Value) -> Result<(), String> {
    let schema = input_schema(name).ok_or_else(|| {
        "KNIFE_CURVE_MODIFIER_GRAPH_INVALID: operation is not façade-native".to_owned()
    })?;
    let object = arguments.as_object().ok_or_else(|| {
        "KNIFE_CURVE_MODIFIER_GRAPH_INVALID: request must be an object".to_owned()
    })?;
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .ok_or_else(|| "KNIFE_CURVE_MODIFIER_GRAPH_SCHEMA_INVALID: required is missing".to_owned())?
        .iter()
        .map(|value| {
            value.as_str().ok_or_else(|| {
                "KNIFE_CURVE_MODIFIER_GRAPH_SCHEMA_INVALID: required contains a non-string"
                    .to_owned()
            })
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let declared = schema
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "KNIFE_CURVE_MODIFIER_GRAPH_SCHEMA_INVALID: properties are missing".to_owned()
        })?
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if required != declared || actual != declared {
        return Err(
            "KNIFE_CURVE_MODIFIER_GRAPH_INVALID: request fields differ from the closed contract"
                .to_owned(),
        );
    }
    let expected_version = if name == GET_NAME {
        "KnifeCurveModifierGraphGetRequest@1"
    } else {
        "KnifeCurveModifierGraphPrepareRequest@1"
    };
    if object.get("schema_version").and_then(Value::as_str) != Some(expected_version)
        || object.get("operation").and_then(Value::as_str) != Some(name)
    {
        return Err(
            "KNIFE_CURVE_MODIFIER_GRAPH_INVALID: schema_version or operation differs".to_owned(),
        );
    }
    Ok(())
}

pub fn read_tools() -> Vec<Value> {
    vec![tool_definition(KnifeCurveModifierGraphTool::Get)]
}

pub fn write_tools() -> Vec<Value> {
    vec![tool_definition(KnifeCurveModifierGraphTool::Prepare)]
}

fn tool_definition(tool: KnifeCurveModifierGraphTool) -> Value {
    let description = match tool {
        KnifeCurveModifierGraphTool::Get => {
            "Read one durable knife Curve + ModifierGraph dependency/recompute receipt. This is a read-only structural projection; it does not create a mesh, advance a stage, confirm a candidate, create a version or export."
        }
        KnifeCurveModifierGraphTool::Prepare => {
            "Prepare one bounded knife Curve + ModifierGraph dependency/recompute receipt. Runtime samples typed curves and plans dirty recomputation, then durably records the closed receipt; the current slice intentionally creates no evaluated mesh or geometry artifact. Explicit authenticated MCP write opt-in is required."
        }
    };
    json!({
        "name": tool.name(),
        "description": description,
        "inputSchema": input_schema(tool.name()).expect("knife operation schema is valid"),
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
            "transaction": "KnifeCurveModifierGraph@1",
            "facadeNative": true,
            "resultSchema": "KnifeCurveModifierGraphResult@1"
        }}
    })
}

/// Keep MCP text responses useful without copying curve points, graph nodes or
/// other potentially large request data onto the wire.  `structuredContent`
/// remains the exact Runtime result; this is only the hash/count summary.
pub fn summary(name: &str, value: &Value) -> Option<String> {
    if !is_tool(name) {
        return None;
    }
    serde_json::to_string(&json!({
        "schema_version": "KnifeCurveModifierGraphMcpSummary@1",
        "operation": name,
        "project_id": value.get("project_id"),
        "source_candidate_id": value.get("source_candidate_id"),
        "curve_ids": value.get("curve_ids"),
        "curve_sha256": value.get("curve_sha256"),
        "modifier_graph_sha256": value.get("modifier_graph_sha256"),
        "dependency_closure_sha256": value.get("dependency_closure_sha256"),
        "recompute_plan_sha256": value.get("recompute_plan_sha256"),
        "recomputed_node_ids": value.get("recomputed_node_ids"),
        "reused_node_ids": value.get("reused_node_ids"),
        "evaluation_status": value.get("evaluation_status"),
        "evaluated_mesh_created": value.get("evaluated_mesh_created"),
        "geometry_artifact_created": value.get("geometry_artifact_created"),
        "replayed": value.get("replayed"),
        "restart_hash_verified": value.get("restart_hash_verified"),
        "runtime_write_performed": value.get("runtime_write_performed"),
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
        }
        assert!(is_write_tool(PREPARE_NAME));
        assert!(!is_write_tool(GET_NAME));
        assert_eq!(read_tools().len(), 1);
        assert_eq!(write_tools().len(), 1);
        assert_eq!(result_schema()["additionalProperties"], false);
        assert!(validate_call(GET_NAME, &json!({})).is_err());
    }
}
