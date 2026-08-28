use serde_json::{json, Map, Value};

const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-canonical-sha256@1";

const GET_FIELDS: [&str; 11] = [
    "schema_version",
    "project_id",
    "candidate_id",
    "canonical_mesh_id",
    "canonical_mesh_sha256",
    "artifact_id",
    "artifact_sha256",
    "writer_policy",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "input_sha256",
];

const PREPARE_FIELDS: [&str; 22] = [
    "schema_version",
    "project_id",
    "source_candidate_id",
    "source_candidate_state_sha256",
    "base_version_id",
    "authoring_node_id",
    "part_id",
    "source_program_object_sha256",
    "source_program_sha256",
    "source_artifact_id",
    "source_artifact_object_sha256",
    "source_artifact_sha256",
    "source_artifact_readback_object_sha256",
    "source_artifact_readback_sha256",
    "source_lineage_sha256",
    "expected_canonical_mesh_sha256",
    "idempotency_key",
    "max_response_bytes",
    "runtime_write_performed",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoringMeshDurableTool {
    Get,
    Prepare,
}

impl AuthoringMeshDurableTool {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Get => "authoring_mesh_durable_get",
            Self::Prepare => "authoring_mesh_durable_prepare",
        }
    }

    pub const fn is_write(self) -> bool {
        matches!(self, Self::Prepare)
    }

    pub const fn runtime_method(self) -> &'static str {
        self.name()
    }
}

pub fn from_name(name: &str) -> Option<AuthoringMeshDurableTool> {
    Some(match name {
        "authoring_mesh_durable_get" => AuthoringMeshDurableTool::Get,
        "authoring_mesh_durable_prepare" => AuthoringMeshDurableTool::Prepare,
        _ => return None,
    })
}

pub fn is_tool(name: &str) -> bool {
    from_name(name).is_some()
}

pub fn is_write_tool(name: &str) -> bool {
    from_name(name).is_some_and(AuthoringMeshDurableTool::is_write)
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    from_name(name).map(AuthoringMeshDurableTool::runtime_method)
}

pub fn unavailable_error(name: &str) -> String {
    format!(
        "AUTHORING_MESH_DURABLE_RUNTIME_METHOD_UNAVAILABLE: {name} requires Runtime method {name}"
    )
}

pub fn read_tool_names() -> Vec<String> {
    vec![AuthoringMeshDurableTool::Get.name().to_owned()]
}

pub fn write_tool_names() -> Vec<String> {
    vec![AuthoringMeshDurableTool::Prepare.name().to_owned()]
}

pub fn read_tools() -> Vec<Value> {
    vec![tool_definition(AuthoringMeshDurableTool::Get)]
}

pub fn write_tools() -> Vec<Value> {
    vec![tool_definition(AuthoringMeshDurableTool::Prepare)]
}

fn tool_definition(tool: AuthoringMeshDurableTool) -> Value {
    let (description, schema) = match tool {
        AuthoringMeshDurableTool::Get => (
            "Read one exact Runtime-owned durable AuthoringMesh canonical/original, evaluated artifact sidecar and lineage Link. This is a structural-only lookup; it performs no write, stage advancement, confirmation, version creation or export.",
            get_schema(),
        ),
        AuthoringMeshDurableTool::Prepare => (
            "Prepare one Runtime-owned durable AuthoringMesh canonical/original, evaluated artifact sidecar and lineage Link from an exact existing candidate/program/artifact/readback binding. This writes only through Runtime after explicit MCP write opt-in; it never advances a stage, confirms a candidate, creates a version or exports.",
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
            "transaction":"AuthoringMeshDurable@1",
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
        "pattern":"^[A-Za-z0-9._:-]+$"
    })
}

fn nullable_identifier_property() -> Value {
    json!({
        "type":["string","null"],
        "maxLength":128,
        "pattern":"^[A-Za-z0-9._:-]+$"
    })
}

fn sha256_property() -> Value {
    json!({"type":"string","pattern":"^[0-9a-f]{64}$"})
}

fn idempotency_key_property() -> Value {
    json!({
        "type":"string",
        "minLength":1,
        "maxLength":128,
        "pattern":"^[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}$"
    })
}

fn get_schema() -> Value {
    object_schema(
        &GET_FIELDS,
        Map::from_iter([
            (
                "schema_version".to_owned(),
                json!({"const":"AuthoringMeshGetRequest@1"}),
            ),
            ("project_id".to_owned(), identifier_property()),
            ("candidate_id".to_owned(), identifier_property()),
            ("canonical_mesh_id".to_owned(), identifier_property()),
            ("canonical_mesh_sha256".to_owned(), sha256_property()),
            ("artifact_id".to_owned(), identifier_property()),
            ("artifact_sha256".to_owned(), sha256_property()),
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
                json!({"const":"AuthoringMeshPrepareRequest@1"}),
            ),
            ("project_id".to_owned(), identifier_property()),
            ("source_candidate_id".to_owned(), identifier_property()),
            (
                "source_candidate_state_sha256".to_owned(),
                sha256_property(),
            ),
            ("base_version_id".to_owned(), nullable_identifier_property()),
            ("authoring_node_id".to_owned(), identifier_property()),
            ("part_id".to_owned(), identifier_property()),
            ("source_program_object_sha256".to_owned(), sha256_property()),
            ("source_program_sha256".to_owned(), sha256_property()),
            ("source_artifact_id".to_owned(), identifier_property()),
            (
                "source_artifact_object_sha256".to_owned(),
                sha256_property(),
            ),
            ("source_artifact_sha256".to_owned(), sha256_property()),
            (
                "source_artifact_readback_object_sha256".to_owned(),
                sha256_property(),
            ),
            (
                "source_artifact_readback_sha256".to_owned(),
                sha256_property(),
            ),
            ("source_lineage_sha256".to_owned(), sha256_property()),
            (
                "expected_canonical_mesh_sha256".to_owned(),
                sha256_property(),
            ),
            ("idempotency_key".to_owned(), idempotency_key_property()),
            (
                "max_response_bytes".to_owned(),
                json!({"const":MAX_RESPONSE_BYTES}),
            ),
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
    let summary = json!({
        "schema_version":"AuthoringMeshDurableMcpSummary@1",
        "operation":name,
        "write_intent":if tool.is_write() { "explicit_runtime_durable_authoring_mesh_prepare_write" } else { "read_only_runtime_durable_authoring_mesh_lookup" },
        "project_id":lookup("project_id"),
        "candidate_id":value.get("candidate_id").cloned().or_else(|| value.get("source_candidate_id").cloned()).unwrap_or(Value::Null),
        "canonical_mesh_id":lookup("canonical_mesh_id"),
        "canonical_mesh_sha256":lookup("canonical_mesh_sha256"),
        "artifact_id":lookup("artifact_id"),
        "artifact_sha256":lookup("artifact_sha256"),
        "link_id":lookup("link_id"),
        "request_input_sha256":value.get("request_input_sha256").cloned().or_else(|| value.get("input_sha256").cloned()).unwrap_or(Value::Null),
        "idempotency_key":lookup("idempotency_key"),
        "replayed":fallback_bool("replayed"),
        "restart_hash_verified":fallback_bool("restart_hash_verified"),
        "runtime_write_performed":fallback_bool("runtime_write_performed"),
        "persistent_user_data_touched":fallback_bool("persistent_user_data_touched"),
        "stage_advanced":value.get("stage_advanced").cloned().or_else(|| value.get("production_stage_advanced").cloned()).unwrap_or(Value::Bool(false)),
        "candidate_confirmed":fallback_bool("candidate_confirmed"),
        "version_created":fallback_bool("version_created"),
        "export_performed":fallback_bool("export_performed"),
        "quality_status":value.get("quality_status").cloned().unwrap_or_else(|| Value::String("structural_only".to_owned())),
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
    fn durable_tool_contracts_are_closed_and_classified() {
        let read = read_tools();
        let write = write_tools();
        assert_eq!(read.len(), 1);
        assert_eq!(write.len(), 1);
        assert_eq!(read[0]["name"], "authoring_mesh_durable_get");
        assert_eq!(write[0]["name"], "authoring_mesh_durable_prepare");
        assert_eq!(read[0]["annotations"]["readOnlyHint"], true);
        assert_eq!(write[0]["annotations"]["readOnlyHint"], false);
        assert_eq!(read[0]["inputSchema"]["additionalProperties"], false);
        assert_eq!(write[0]["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            read[0]["inputSchema"]["properties"]["schema_version"]["const"],
            "AuthoringMeshGetRequest@1"
        );
        assert_eq!(
            write[0]["inputSchema"]["properties"]["schema_version"]["const"],
            "AuthoringMeshPrepareRequest@1"
        );
        assert_eq!(
            write[0]["inputSchema"]["properties"]["max_response_bytes"]["const"],
            MAX_RESPONSE_BYTES
        );
        assert_eq!(
            runtime_method("authoring_mesh_durable_get"),
            Some("authoring_mesh_durable_get")
        );
        assert_eq!(
            runtime_method("authoring_mesh_durable_prepare"),
            Some("authoring_mesh_durable_prepare")
        );
        assert!(!is_write_tool("authoring_mesh_durable_get"));
        assert!(is_write_tool("authoring_mesh_durable_prepare"));
    }
}
