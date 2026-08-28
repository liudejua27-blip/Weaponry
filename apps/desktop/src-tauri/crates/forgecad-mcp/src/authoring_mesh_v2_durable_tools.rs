use serde_json::{json, Map, Value};

const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-canonical-sha256@1";

const PREPARE_FIELDS: [&str; 21] = [
    "schema_version",
    "project_id",
    "operation",
    "mesh_id",
    "lineage_id",
    "parent_revision_id",
    "operation_id",
    "edge_id",
    "split_ratio_milli",
    "vertex_ids",
    "delta_m",
    "operation_lineage_sha256",
    "positions_m",
    "faces",
    "evaluated",
    "idempotency_key",
    "max_response_bytes",
    "runtime_write_performed",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];

const GET_FIELDS: [&str; 10] = [
    "schema_version",
    "project_id",
    "mesh_id",
    "revision_id",
    "revision_sha256",
    "revision_object_sha256",
    "writer_policy",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "input_sha256",
];

const SOURCE_PREPARE_FIELDS: [&str; 15] = [
    "schema_version",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "geometry_program_sha256",
    "artifact_sha256",
    "artifact_readback_sha256",
    "part_id",
    "source_node_id",
    "idempotency_key",
    "max_response_bytes",
    "runtime_write_performed",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoringMeshV2DurableTool {
    Get,
    Prepare,
    SourcePrepare,
}

impl AuthoringMeshV2DurableTool {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Get => "authoring_mesh_v2_durable_get",
            Self::Prepare => "authoring_mesh_v2_durable_prepare",
            Self::SourcePrepare => "production_weapon_authoring_mesh_v2_source_prepare",
        }
    }

    pub const fn is_write(self) -> bool {
        matches!(self, Self::Prepare | Self::SourcePrepare)
    }

    pub const fn runtime_method(self) -> &'static str {
        self.name()
    }
}

pub fn from_name(name: &str) -> Option<AuthoringMeshV2DurableTool> {
    Some(match name {
        "authoring_mesh_v2_durable_get" => AuthoringMeshV2DurableTool::Get,
        "authoring_mesh_v2_durable_prepare" => AuthoringMeshV2DurableTool::Prepare,
        "production_weapon_authoring_mesh_v2_source_prepare" => {
            AuthoringMeshV2DurableTool::SourcePrepare
        }
        _ => return None,
    })
}

pub fn is_tool(name: &str) -> bool {
    from_name(name).is_some()
}

pub fn is_write_tool(name: &str) -> bool {
    from_name(name).is_some_and(AuthoringMeshV2DurableTool::is_write)
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    from_name(name).map(AuthoringMeshV2DurableTool::runtime_method)
}

pub fn unavailable_error(name: &str) -> String {
    format!(
        "AUTHORING_MESH_V2_DURABLE_RUNTIME_METHOD_UNAVAILABLE: {name} requires Runtime method {name}"
    )
}

pub fn read_tool_names() -> Vec<String> {
    vec![AuthoringMeshV2DurableTool::Get.name().to_owned()]
}

pub fn write_tool_names() -> Vec<String> {
    vec![
        AuthoringMeshV2DurableTool::Prepare.name().to_owned(),
        AuthoringMeshV2DurableTool::SourcePrepare.name().to_owned(),
    ]
}

pub fn read_tools() -> Vec<Value> {
    vec![tool_definition(AuthoringMeshV2DurableTool::Get)]
}

pub fn write_tools() -> Vec<Value> {
    vec![
        tool_definition(AuthoringMeshV2DurableTool::Prepare),
        tool_definition(AuthoringMeshV2DurableTool::SourcePrepare),
    ]
}

fn identifier_property() -> Value {
    json!({"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9._:-]+$"})
}

fn nullable_identifier_property() -> Value {
    json!({"type":["string","null"],"maxLength":128,"pattern":"^[A-Za-z0-9._:-]+$"})
}

fn nullable_sha_property() -> Value {
    json!({"type":["string","null"],"pattern":"^[0-9a-f]{64}$"})
}

fn sha_property() -> Value {
    json!({"type":"string","pattern":"^[0-9a-f]{64}$"})
}

fn idempotency_property() -> Value {
    json!({"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}$"})
}

fn object_schema(required: &[&str], properties: Map<String, Value>) -> Value {
    json!({"type":"object","required":required,"properties":properties,"additionalProperties":false})
}

fn get_schema() -> Value {
    object_schema(
        &GET_FIELDS,
        Map::from_iter([
            (
                "schema_version".to_owned(),
                json!({"const":"AuthoringMeshV2DurableGetRequest@1"}),
            ),
            ("project_id".to_owned(), identifier_property()),
            ("mesh_id".to_owned(), identifier_property()),
            ("revision_id".to_owned(), identifier_property()),
            ("revision_sha256".to_owned(), sha_property()),
            ("revision_object_sha256".to_owned(), sha_property()),
            ("writer_policy".to_owned(), json!({"const":WRITER_POLICY})),
            ("runtime_write_performed".to_owned(), json!({"const":false})),
            (
                "persistent_user_data_touched".to_owned(),
                json!({"const":false}),
            ),
            ("input_sha256".to_owned(), sha_property()),
        ]),
    )
}

fn prepare_schema() -> Value {
    object_schema(
        &PREPARE_FIELDS,
        Map::from_iter([
            (
                "schema_version".to_owned(),
                json!({"const":"AuthoringMeshV2DurablePrepareRequest@1"}),
            ),
            ("project_id".to_owned(), identifier_property()),
            (
                "operation".to_owned(),
                json!({"enum":["genesis","split_edge","move_vertices"]}),
            ),
            ("mesh_id".to_owned(), identifier_property()),
            ("lineage_id".to_owned(), identifier_property()),
            (
                "parent_revision_id".to_owned(),
                nullable_identifier_property(),
            ),
            ("operation_id".to_owned(), nullable_identifier_property()),
            ("edge_id".to_owned(), nullable_identifier_property()),
            (
                "split_ratio_milli".to_owned(),
                json!({
                    "oneOf":[
                        {"type":"null"},
                        {"type":"integer","minimum":1,"maximum":999}
                    ]
                }),
            ),
            (
                "vertex_ids".to_owned(),
                json!({
                    "type":["array","null"],
                    "minItems":1,
                    "maxItems":32,
                    "uniqueItems":true,
                    "items":identifier_property()
                }),
            ),
            (
                "delta_m".to_owned(),
                json!({
                    "type":["array","null"],
                    "minItems":1,
                    "maxItems":32,
                    "items":{
                        "type":"array",
                        "minItems":3,
                        "maxItems":3,
                        "items":{"type":"number","minimum":-1.0,"maximum":1.0}
                    }
                }),
            ),
            (
                "operation_lineage_sha256".to_owned(),
                nullable_sha_property(),
            ),
            (
                "positions_m".to_owned(),
                json!({"type":["array","null"],"maxItems":32768}),
            ),
            (
                "faces".to_owned(),
                json!({"type":["array","null"],"maxItems":32768}),
            ),
            (
                "evaluated".to_owned(),
                json!({
                    "oneOf":[
                        {"type":"null"},
                        {
                            "type":"object",
                            "required":["artifact_id","artifact_sha256","readback_sha256","correspondence_status"],
                            "properties":{
                                "artifact_id":identifier_property(),
                                "artifact_sha256":sha_property(),
                                "readback_sha256":sha_property(),
                                "correspondence_status":identifier_property()
                            },
                            "additionalProperties":false
                        }
                    ]
                }),
            ),
            ("idempotency_key".to_owned(), idempotency_property()),
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
            ("input_sha256".to_owned(), sha_property()),
        ]),
    )
}

fn source_prepare_schema() -> Value {
    object_schema(
        &SOURCE_PREPARE_FIELDS,
        Map::from_iter([
            (
                "schema_version".to_owned(),
                json!({"const":"ProductionWeaponAuthoringMeshV2SourcePrepareRequest@1"}),
            ),
            ("project_id".to_owned(), identifier_property()),
            ("candidate_id".to_owned(), identifier_property()),
            ("candidate_state_sha256".to_owned(), sha_property()),
            ("geometry_program_sha256".to_owned(), sha_property()),
            ("artifact_sha256".to_owned(), sha_property()),
            ("artifact_readback_sha256".to_owned(), sha_property()),
            ("part_id".to_owned(), identifier_property()),
            ("source_node_id".to_owned(), identifier_property()),
            ("idempotency_key".to_owned(), idempotency_property()),
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
            ("input_sha256".to_owned(), sha_property()),
        ]),
    )
}

fn tool_definition(tool: AuthoringMeshV2DurableTool) -> Value {
    let (description, schema) = match tool {
        AuthoringMeshV2DurableTool::Get => (
            "Read one immutable Runtime-owned AuthoringMesh@2 revision from Store/CAS and revalidate its half-edge topology, tombstones and parent DAG after restart.",
            get_schema(),
        ),
        AuthoringMeshV2DurableTool::Prepare => (
            "Prepare one Runtime-owned AuthoringMesh@2 genesis, local split_edge revision, or bounded move_vertices position edit. Explicit MCP write opt-in is required; no candidate, version, stage or export state is changed.",
            prepare_schema(),
        ),
        AuthoringMeshV2DurableTool::SourcePrepare => (
            "Derive and persist one AuthoringMesh@2 genesis from an exact candidate-owned fictional weapon source node. Runtime loads the GeometryProgram and creates stable topology; callers cannot provide mesh buffers or replacement programs.",
            source_prepare_schema(),
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
        "_meta":{"forgecad":{"availability":"available","runtime_method":tool.runtime_method(),"requiresConfirmation":false,"transaction":"AuthoringMesh@2","definition_only":false}}
    })
}

pub fn summary(name: &str, value: &Value) -> Option<String> {
    let tool = from_name(name)?;
    let lookup = |field: &str| value.get(field).cloned().unwrap_or(Value::Null);
    serde_json::to_string(&json!({
        "schema_version":"AuthoringMeshV2DurableMcpSummary@1",
        "operation":name,
        "write_intent":if tool.is_write() { "explicit_runtime_durable_authoring_mesh_v2_prepare_write" } else { "read_only_runtime_durable_authoring_mesh_v2_lookup" },
        "project_id":lookup("project_id"),
        "mesh_id":lookup("mesh_id"),
        "lineage_id":lookup("lineage_id"),
        "revision_id":lookup("revision_id"),
        "revision_sha256":lookup("revision_sha256"),
        "revision_object_sha256":lookup("revision_object_sha256"),
        "replayed":value.get("replayed").cloned().unwrap_or(Value::Bool(false)),
        "restart_hash_verified":value.get("restart_hash_verified").cloned().unwrap_or(Value::Bool(false)),
        "runtime_write_performed":value.get("runtime_write_performed").cloned().unwrap_or(Value::Bool(false)),
        "persistent_user_data_touched":value.get("persistent_user_data_touched").cloned().unwrap_or(Value::Bool(false)),
        "quality_status":value.get("quality_status").cloned().unwrap_or_else(|| Value::String("structural_only".to_owned())),
        "limitations":lookup("limitations"),
        "canonical_sha256":lookup("canonical_sha256"),
        "structured_content_complete":true
    }))
    .ok()
}
