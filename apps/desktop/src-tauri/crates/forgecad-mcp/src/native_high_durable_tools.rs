use serde_json::{json, Map, Value};

const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-canonical-sha256@1";
const IDENTIFIER_PATTERN: &str = "^[A-Za-z0-9_.:-]{1,128}$";

const GET_FIELDS: [&str; 20] = [
    "schema_version",
    "operation",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "base_version_id",
    "link_id",
    "link_object_sha256",
    "source_authoring_mesh_id",
    "source_authoring_mesh_sha256",
    "detail_graph_canonical_sha256",
    "artifact_id",
    "artifact_sha256",
    "glb_sha256",
    "idempotency_key",
    "source_only",
    "writer_policy",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "input_sha256",
];

const PREPARE_FIELDS: [&str; 17] = [
    "schema_version",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "base_version_id",
    "source_authoring_mesh_id",
    "source_authoring_mesh_object_sha256",
    "source_authoring_mesh_sha256",
    "high_mesh_request",
    "high_mesh_request_sha256",
    "idempotency_key",
    "max_response_bytes",
    "source_only",
    "runtime_write_performed",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeHighDurableTool {
    Get,
    Prepare,
}

impl NativeHighDurableTool {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Get => "native_high_durable_get",
            Self::Prepare => "native_high_durable_prepare",
        }
    }

    pub const fn is_write(self) -> bool {
        matches!(self, Self::Prepare)
    }

    pub const fn runtime_method(self) -> &'static str {
        self.name()
    }
}

pub fn from_name(name: &str) -> Option<NativeHighDurableTool> {
    Some(match name {
        "native_high_durable_get" => NativeHighDurableTool::Get,
        "native_high_durable_prepare" => NativeHighDurableTool::Prepare,
        _ => return None,
    })
}

pub fn is_tool(name: &str) -> bool {
    from_name(name).is_some()
}

pub fn is_write_tool(name: &str) -> bool {
    from_name(name).is_some_and(NativeHighDurableTool::is_write)
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    from_name(name).map(NativeHighDurableTool::runtime_method)
}

pub fn unavailable_error(name: &str) -> String {
    format!("NATIVE_HIGH_DURABLE_RUNTIME_METHOD_UNAVAILABLE: {name} requires Runtime method {name}")
}

pub fn read_tool_names() -> Vec<String> {
    vec![NativeHighDurableTool::Get.name().to_owned()]
}

pub fn write_tool_names() -> Vec<String> {
    vec![NativeHighDurableTool::Prepare.name().to_owned()]
}

pub fn read_tools() -> Vec<Value> {
    vec![tool_definition(NativeHighDurableTool::Get)]
}

pub fn write_tools() -> Vec<Value> {
    vec![tool_definition(NativeHighDurableTool::Prepare)]
}

fn tool_definition(tool: NativeHighDurableTool) -> Value {
    let (description, schema) = match tool {
        NativeHighDurableTool::Get => (
            "Read one exact Runtime-owned source-only Native High durable link and its GLB/CAS bindings. This structural lookup never advances a stage, confirms a candidate, creates a version or exports.",
            get_schema(),
        ),
        NativeHighDurableTool::Prepare => (
            "Prepare one Runtime-owned source-only Native High durable link from an exact candidate-bound AuthoringMeshCanonical@1 and HighMeshWorkerRequest@1. Runtime alone materializes the HighMeshArtifact, GLB and strict readback; explicit MCP write opt-in is required and no stage, confirmation, version or export is performed.",
            prepare_schema(),
        ),
    };
    json!({
        "name":tool.name(),
        "description":description,
        "inputSchema":schema,
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
            "transaction":"NativeHighDurable@1",
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

fn nullable_identifier_property() -> Value {
    json!({
        "type":["string","null"],
        "maxLength":128,
        "pattern":IDENTIFIER_PATTERN
    })
}

fn sha256_property() -> Value {
    json!({"type":"string","pattern":"^[0-9a-f]{64}$"})
}

fn high_mesh_request_schema() -> Value {
    object_schema(
        &[
            "schema_version",
            "operation",
            "source_authoring_mesh",
            "source_authoring_mesh_sha256",
            "detail_graph",
            "detail_graph_canonical_sha256",
            "budgets",
            "canonical_sha256",
        ],
        Map::from_iter([
            (
                "schema_version".to_owned(),
                json!({"const":"HighMeshWorkerRequest@1"}),
            ),
            (
                "operation".to_owned(),
                json!({"const":"forgecad.production.high-mesh-prepare@1"}),
            ),
            // The nested canonical source and DetailGraph have their own
            // closed public contracts. Runtime performs their complete
            // validation; this adapter keeps the transport object opaque
            // rather than inventing a second partial contract here.
            ("source_authoring_mesh".to_owned(), json!({"type":"object"})),
            ("source_authoring_mesh_sha256".to_owned(), sha256_property()),
            ("detail_graph".to_owned(), json!({"type":"object"})),
            (
                "detail_graph_canonical_sha256".to_owned(),
                sha256_property(),
            ),
            (
                "budgets".to_owned(),
                object_schema(
                    &[
                        "max_detail_nodes",
                        "max_output_vertices",
                        "max_output_triangles",
                    ],
                    Map::from_iter([
                        (
                            "max_detail_nodes".to_owned(),
                            json!({"type":"integer","minimum":1,"maximum":256}),
                        ),
                        (
                            "max_output_vertices".to_owned(),
                            json!({"type":"integer","minimum":1,"maximum":300000}),
                        ),
                        (
                            "max_output_triangles".to_owned(),
                            json!({"type":"integer","minimum":1,"maximum":600000}),
                        ),
                    ]),
                ),
            ),
            ("canonical_sha256".to_owned(), sha256_property()),
        ]),
    )
}

fn get_schema() -> Value {
    object_schema(
        &GET_FIELDS,
        Map::from_iter([
            (
                "schema_version".to_owned(),
                json!({"const":"NativeHighDurableGetRequest@1"}),
            ),
            (
                "operation".to_owned(),
                json!({"const":"forgecad.production.native-high-durable-get@1"}),
            ),
            ("project_id".to_owned(), identifier_property()),
            ("candidate_id".to_owned(), identifier_property()),
            ("candidate_state_sha256".to_owned(), sha256_property()),
            ("base_version_id".to_owned(), nullable_identifier_property()),
            ("link_id".to_owned(), identifier_property()),
            ("link_object_sha256".to_owned(), sha256_property()),
            ("source_authoring_mesh_id".to_owned(), identifier_property()),
            ("source_authoring_mesh_sha256".to_owned(), sha256_property()),
            (
                "detail_graph_canonical_sha256".to_owned(),
                sha256_property(),
            ),
            ("artifact_id".to_owned(), identifier_property()),
            ("artifact_sha256".to_owned(), sha256_property()),
            ("glb_sha256".to_owned(), sha256_property()),
            ("idempotency_key".to_owned(), identifier_property()),
            ("source_only".to_owned(), json!({"const":true})),
            ("writer_policy".to_owned(), json!({"const":WRITER_POLICY})),
            ("runtime_write_performed".to_owned(), json!({"const":false})),
            (
                "persistent_user_data_touched".to_owned(),
                json!({"const":false}),
            ),
            ("input_sha256".to_owned(), sha256_property()),
        ]),
    )
}

fn prepare_schema() -> Value {
    object_schema(
        &PREPARE_FIELDS,
        Map::from_iter([
            (
                "schema_version".to_owned(),
                json!({"const":"NativeHighDurablePrepareRequest@1"}),
            ),
            ("project_id".to_owned(), identifier_property()),
            ("candidate_id".to_owned(), identifier_property()),
            ("candidate_state_sha256".to_owned(), sha256_property()),
            ("base_version_id".to_owned(), nullable_identifier_property()),
            ("source_authoring_mesh_id".to_owned(), identifier_property()),
            (
                "source_authoring_mesh_object_sha256".to_owned(),
                sha256_property(),
            ),
            ("source_authoring_mesh_sha256".to_owned(), sha256_property()),
            ("high_mesh_request".to_owned(), high_mesh_request_schema()),
            ("high_mesh_request_sha256".to_owned(), sha256_property()),
            ("idempotency_key".to_owned(), identifier_property()),
            (
                "max_response_bytes".to_owned(),
                json!({"const":MAX_RESPONSE_BYTES}),
            ),
            ("source_only".to_owned(), json!({"const":true})),
            ("runtime_write_performed".to_owned(), json!({"const":false})),
            ("writer_policy".to_owned(), json!({"const":WRITER_POLICY})),
            (
                "canonicalization_policy".to_owned(),
                json!({"const":CANONICALIZATION_POLICY}),
            ),
            ("input_sha256".to_owned(), sha256_property()),
        ]),
    )
}

pub fn summary(name: &str, value: &Value) -> Option<String> {
    let tool = from_name(name)?;
    let lookup = |field: &str| value.get(field).cloned().unwrap_or(Value::Null);
    let fallback_bool = |field: &str| value.get(field).cloned().unwrap_or(Value::Bool(false));
    let source_mesh_id = value
        .pointer("/source_authoring_mesh/canonical_mesh_id")
        .cloned()
        .or_else(|| value.get("source_authoring_mesh_id").cloned())
        .unwrap_or(Value::Null);
    let summary = json!({
        "schema_version":"NativeHighDurableMcpSummary@1",
        "operation":name,
        "write_intent":if tool.is_write() { "explicit_runtime_native_high_durable_prepare_write" } else { "read_only_runtime_native_high_durable_lookup" },
        "project_id":lookup("project_id"),
        "candidate_id":lookup("candidate_id"),
        "source_authoring_mesh_id":source_mesh_id,
        "source_authoring_mesh_sha256":lookup("source_authoring_mesh_sha256"),
        "detail_graph_canonical_sha256":lookup("detail_graph_canonical_sha256"),
        "artifact_id":lookup("artifact_id"),
        "artifact_sha256":lookup("artifact_sha256"),
        "glb_sha256":lookup("glb_sha256"),
        "glb_object_sha256":lookup("glb_object_sha256"),
        "link_id":lookup("link_id"),
        "request_sha256":lookup("request_sha256"),
        "request_input_sha256":value.get("request_input_sha256").cloned().or_else(|| value.get("input_sha256").cloned()).unwrap_or(Value::Null),
        "idempotency_key":lookup("idempotency_key"),
        "replayed":fallback_bool("replayed"),
        "replay_count":lookup("replay_count"),
        "replay_byte_exact":fallback_bool("replay_byte_exact"),
        "restart_hash_verified":fallback_bool("restart_hash_verified"),
        "runtime_write_performed":fallback_bool("runtime_write_performed"),
        "persistent_user_data_touched":fallback_bool("persistent_user_data_touched"),
        "production_stage_advanced":fallback_bool("production_stage_advanced"),
        "candidate_confirmed":fallback_bool("candidate_confirmed"),
        "version_created":fallback_bool("version_created"),
        "export_performed":fallback_bool("export_performed"),
        "quality_status":value.get("quality_status").cloned().unwrap_or_else(|| Value::String("structural_only".to_owned())),
        "source_only":lookup("source_only"),
        "limitations":lookup("limitations"),
        "canonicalization_policy":lookup("canonicalization_policy"),
        "canonical_sha256":lookup("canonical_sha256"),
        "structured_content_complete":true
    });
    serde_json::to_string(&summary).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_high_durable_tools_are_closed_and_classified() {
        let read = read_tools();
        let write = write_tools();
        assert_eq!(read.len(), 1);
        assert_eq!(write.len(), 1);
        assert_eq!(read[0]["name"], "native_high_durable_get");
        assert_eq!(write[0]["name"], "native_high_durable_prepare");
        assert_eq!(read[0]["annotations"]["readOnlyHint"], true);
        assert_eq!(write[0]["annotations"]["readOnlyHint"], false);
        assert_eq!(read[0]["annotations"]["writeIntent"], false);
        assert_eq!(write[0]["annotations"]["writeIntent"], true);
        assert_eq!(read[0]["inputSchema"]["additionalProperties"], false);
        assert_eq!(write[0]["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            read[0]["inputSchema"]["properties"]["schema_version"]["const"],
            "NativeHighDurableGetRequest@1"
        );
        assert_eq!(
            write[0]["inputSchema"]["properties"]["schema_version"]["const"],
            "NativeHighDurablePrepareRequest@1"
        );
        assert_eq!(
            write[0]["inputSchema"]["properties"]["max_response_bytes"]["const"],
            MAX_RESPONSE_BYTES
        );
        assert!(write[0]["inputSchema"]["required"]
            .as_array()
            .expect("prepare required fields")
            .iter()
            .any(|field| field == "high_mesh_request"));
        assert!(is_write_tool("native_high_durable_prepare"));
        assert!(!is_write_tool("native_high_durable_get"));
        assert_eq!(
            runtime_method("native_high_durable_get"),
            Some("native_high_durable_get")
        );
        assert_eq!(
            runtime_method("native_high_durable_prepare"),
            Some("native_high_durable_prepare")
        );
    }
}
