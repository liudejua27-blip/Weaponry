use serde_json::{json, Map, Value};

const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-canonical-sha256@1";

const GET_NAME: &str = "authoring_mesh_transaction_get";
const PREPARE_NAME: &str = "authoring_mesh_transaction_prepare";

const GET_FIELDS: [&str; 11] = [
    "schema_version",
    "project_id",
    "transaction_id",
    "transaction_sha256",
    "transaction_object_sha256",
    "max_response_bytes",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];

const PREPARE_FIELDS: [&str; 9] = [
    "schema_version",
    "project_id",
    "transaction",
    "idempotency_key",
    "max_response_bytes",
    "runtime_write_performed",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoringMeshTransactionTool {
    Get,
    Prepare,
}

impl AuthoringMeshTransactionTool {
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

pub fn from_name(name: &str) -> Option<AuthoringMeshTransactionTool> {
    Some(match name {
        GET_NAME => AuthoringMeshTransactionTool::Get,
        PREPARE_NAME => AuthoringMeshTransactionTool::Prepare,
        _ => return None,
    })
}

pub fn is_tool(name: &str) -> bool {
    from_name(name).is_some()
}

pub fn is_write_tool(name: &str) -> bool {
    from_name(name).is_some_and(AuthoringMeshTransactionTool::is_write)
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    from_name(name).map(AuthoringMeshTransactionTool::runtime_method)
}

pub fn unavailable_error(name: &str) -> String {
    format!(
        "AUTHORING_MESH_TRANSACTION_RUNTIME_METHOD_UNAVAILABLE: {name} requires Runtime method {name}"
    )
}

pub fn read_tool_names() -> Vec<String> {
    vec![GET_NAME.to_owned()]
}

pub fn write_tool_names() -> Vec<String> {
    vec![PREPARE_NAME.to_owned()]
}

pub fn read_tools() -> Vec<Value> {
    vec![tool_definition(AuthoringMeshTransactionTool::Get)]
}

pub fn write_tools() -> Vec<Value> {
    vec![tool_definition(AuthoringMeshTransactionTool::Prepare)]
}

fn identifier_property() -> Value {
    json!({
        "type":"string",
        "minLength":1,
        "maxLength":128,
        "pattern":"^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"
    })
}

fn sha256_property() -> Value {
    json!({"type":"string","pattern":"^[0-9a-f]{64}$"})
}

fn revision_index_property() -> Value {
    json!({"type":"integer","minimum":0,"maximum":1_000_000})
}

fn idempotency_key_property() -> Value {
    json!({
        "type":"string",
        "minLength":1,
        "maxLength":128,
        "pattern":"^[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}$"
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

fn get_schema() -> Value {
    object_schema(
        &GET_FIELDS,
        Map::from_iter([
            (
                "schema_version".to_owned(),
                json!({"const":"AuthoringMeshTransactionGetRequest@1"}),
            ),
            ("project_id".to_owned(), identifier_property()),
            ("transaction_id".to_owned(), identifier_property()),
            ("transaction_sha256".to_owned(), sha256_property()),
            ("transaction_object_sha256".to_owned(), sha256_property()),
            (
                "max_response_bytes".to_owned(),
                json!({"const":MAX_RESPONSE_BYTES}),
            ),
            ("runtime_write_performed".to_owned(), json!({"const":false})),
            (
                "persistent_user_data_touched".to_owned(),
                json!({"const":false}),
            ),
            ("writer_policy".to_owned(), json!({"const":WRITER_POLICY})),
            (
                "canonicalization_policy".to_owned(),
                json!({"const":CANONICALIZATION_POLICY}),
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
                json!({"const":"AuthoringMeshTransactionPrepareRequest@1"}),
            ),
            ("project_id".to_owned(), identifier_property()),
            ("transaction".to_owned(), transaction_schema()),
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

fn element_kind_property() -> Value {
    json!({
        "enum":["vertex","edge","half_edge","corner","face","loop","ring"]
    })
}

fn stable_element_ref_schema() -> Value {
    object_schema(
        &["kind", "id"],
        Map::from_iter([
            ("kind".to_owned(), element_kind_property()),
            ("id".to_owned(), identifier_property()),
        ]),
    )
}

fn generated_element_ref_schema() -> Value {
    object_schema(
        &["kind", "command_index", "output_index"],
        Map::from_iter([
            ("kind".to_owned(), element_kind_property()),
            (
                "command_index".to_owned(),
                json!({"type":"integer","minimum":0,"maximum":31}),
            ),
            (
                "output_index".to_owned(),
                json!({"type":"integer","minimum":0,"maximum":131_071}),
            ),
        ]),
    )
}

fn element_ref_schema() -> Value {
    json!({
        "oneOf":[stable_element_ref_schema(),generated_element_ref_schema()]
    })
}

fn operation_lineage_property() -> Value {
    sha256_property()
}

fn split_edge_command_schema() -> Value {
    object_schema(
        &[
            "command_index",
            "operation",
            "operation_id",
            "edge",
            "split_ratio_milli",
            "operation_lineage_sha256",
        ],
        Map::from_iter([
            (
                "command_index".to_owned(),
                json!({"type":"integer","minimum":0,"maximum":31}),
            ),
            ("operation".to_owned(), json!({"const":"split_edge"})),
            ("operation_id".to_owned(), identifier_property()),
            ("edge".to_owned(), element_ref_schema()),
            (
                "split_ratio_milli".to_owned(),
                json!({"type":"integer","minimum":1,"maximum":999}),
            ),
            (
                "operation_lineage_sha256".to_owned(),
                operation_lineage_property(),
            ),
        ]),
    )
}

fn move_vertices_command_schema() -> Value {
    object_schema(
        &[
            "command_index",
            "operation",
            "operation_id",
            "vertices",
            "delta_m",
            "operation_lineage_sha256",
        ],
        Map::from_iter([
            (
                "command_index".to_owned(),
                json!({"type":"integer","minimum":0,"maximum":31}),
            ),
            ("operation".to_owned(), json!({"const":"move_vertices"})),
            ("operation_id".to_owned(), identifier_property()),
            (
                "vertices".to_owned(),
                json!({
                    "type":"array",
                    "minItems":1,
                    "maxItems":32,
                    "uniqueItems":true,
                    "items":element_ref_schema()
                }),
            ),
            (
                "delta_m".to_owned(),
                json!({
                    "type":"array",
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
                operation_lineage_property(),
            ),
        ]),
    )
}

fn face_extrude_command_schema() -> Value {
    object_schema(
        &[
            "command_index",
            "operation",
            "operation_id",
            "face",
            "distance_m",
            "operation_lineage_sha256",
        ],
        Map::from_iter([
            (
                "command_index".to_owned(),
                json!({"type":"integer","minimum":0,"maximum":31}),
            ),
            ("operation".to_owned(), json!({"const":"face_extrude"})),
            ("operation_id".to_owned(), identifier_property()),
            ("face".to_owned(), element_ref_schema()),
            (
                "distance_m".to_owned(),
                json!({
                    "oneOf":[
                        {"type":"number","minimum":-10.0,"maximum":-0.0000001},
                        {"type":"number","minimum":0.0000001,"maximum":10.0}
                    ]
                }),
            ),
            (
                "operation_lineage_sha256".to_owned(),
                operation_lineage_property(),
            ),
        ]),
    )
}

fn transaction_schema() -> Value {
    object_schema(
        &[
            "schema_version",
            "transaction_id",
            "mesh_id",
            "lineage_id",
            "base_revision_id",
            "base_revision_index",
            "base_revision_sha256",
            "commands",
            "budgets",
            "execution_policy",
            "canonicalization_policy",
            "canonical_sha256",
        ],
        Map::from_iter([
            (
                "schema_version".to_owned(),
                json!({"const":"AuthoringMeshTransaction@1"}),
            ),
            ("transaction_id".to_owned(), identifier_property()),
            ("mesh_id".to_owned(), identifier_property()),
            ("lineage_id".to_owned(), identifier_property()),
            ("base_revision_id".to_owned(), identifier_property()),
            ("base_revision_index".to_owned(), revision_index_property()),
            ("base_revision_sha256".to_owned(), sha256_property()),
            (
                "commands".to_owned(),
                json!({
                    "type":"array",
                    "minItems":1,
                    "maxItems":32,
                    "uniqueItems":true,
                    "items":{
                        "oneOf":[
                            split_edge_command_schema(),
                            move_vertices_command_schema(),
                            face_extrude_command_schema()
                        ]
                    }
                }),
            ),
            ("budgets".to_owned(), budgets_schema()),
            ("execution_policy".to_owned(), execution_policy_schema()),
            (
                "canonicalization_policy".to_owned(),
                json!({"const":CANONICALIZATION_POLICY}),
            ),
            ("canonical_sha256".to_owned(), sha256_property()),
        ]),
    )
}

fn budgets_schema() -> Value {
    object_schema(
        &[
            "max_commands",
            "max_move_vertices_per_command",
            "max_face_degree",
            "max_vertex_delta_m",
            "max_face_extrude_distance_m",
            "overflow_policy",
        ],
        Map::from_iter([
            ("max_commands".to_owned(), json!({"const":32})),
            (
                "max_move_vertices_per_command".to_owned(),
                json!({"const":32}),
            ),
            ("max_face_degree".to_owned(), json!({"const":32})),
            ("max_vertex_delta_m".to_owned(), json!({"const":1})),
            (
                "max_face_extrude_distance_m".to_owned(),
                json!({"const":10}),
            ),
            (
                "overflow_policy".to_owned(),
                json!({"const":"reject-entire-transaction@1"}),
            ),
        ]),
    )
}

fn execution_policy_schema() -> Value {
    object_schema(
        &[
            "writer_policy",
            "source_of_truth",
            "reference_policy",
            "atomicity_policy",
            "replay_policy",
            "evaluation_policy",
            "identity_policy",
        ],
        Map::from_iter([
            ("writer_policy".to_owned(), json!({"const":WRITER_POLICY})),
            (
                "source_of_truth".to_owned(),
                json!({"const":"original-authoring-mesh@2"}),
            ),
            (
                "reference_policy".to_owned(),
                json!({"const":"stable-or-earlier-generated-element-by-kind@1"}),
            ),
            (
                "atomicity_policy".to_owned(),
                json!({"const":"clone-before-first-command-no-partial-result@1"}),
            ),
            (
                "replay_policy".to_owned(),
                json!({"const":"same-input-same-base-deterministic-revision-chain@1"}),
            ),
            (
                "evaluation_policy".to_owned(),
                json!({"const":"authored-edit-invalidates-evaluated-sidecar@2"}),
            ),
            (
                "identity_policy".to_owned(),
                json!({"const":"runtime-derived-lineage-operation-parent-stable-no-reuse@2"}),
            ),
        ]),
    )
}

fn tool_definition(tool: AuthoringMeshTransactionTool) -> Value {
    let (description, schema) = match tool {
        AuthoringMeshTransactionTool::Get => (
            "Read one Runtime-owned AuthoringMeshTransaction@1 receipt from Store/CAS and verify its transaction hash, complete revision-chain identity and restart readback. This is read-only and exposes no mesh buffers.",
            get_schema(),
        ),
        AuthoringMeshTransactionTool::Prepare => (
            "Prepare one bounded, ordered AuthoringMeshTransaction@1 through the pure Runtime kernel and atomically persist its revision chain in Store/CAS. Explicit MCP write opt-in is required; the operation never confirms a candidate, creates a version, advances a stage or exports.",
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
            "approvalRequired":tool.is_write()
        },
        "_meta":{"forgecad":{
            "availability":"available",
            "runtime_method":tool.runtime_method(),
            "requiresConfirmation":tool.is_write(),
            "transaction":"AuthoringMeshTransaction@1",
            "definition_only":false
        }}
    })
}

/// Keep MCP text bounded and useful for Codex. The complete revision chain
/// remains in structuredContent/Runtime; no mesh buffers, command arrays or
/// CAS payloads are copied into the human-readable MCP text summary.
pub fn summary(name: &str, value: &Value) -> Option<String> {
    let tool = from_name(name)?;
    let field = |key: &str| value.get(key).cloned().unwrap_or(Value::Null);
    serde_json::to_string(&json!({
        "schema_version":"AuthoringMeshTransactionMcpSummary@1",
        "operation":name,
        "write_intent":if tool.is_write() { "explicit_runtime_durable_authoring_mesh_transaction_prepare_write" } else { "read_only_runtime_durable_authoring_mesh_transaction_lookup" },
        "project_id":field("project_id"),
        "transaction_id":field("transaction_id"),
        "transaction_sha256":field("transaction_sha256"),
        "transaction_object_sha256":field("transaction_object_sha256"),
        "mesh_id":field("mesh_id"),
        "lineage_id":field("lineage_id"),
        "base_revision_id":field("base_revision_id"),
        "base_revision_sha256":field("base_revision_sha256"),
        "final_revision_id":field("final_revision_id"),
        "final_revision_sha256":field("final_revision_sha256"),
        "replayed":field("replayed"),
        "restart_hash_verified":field("restart_hash_verified"),
        "store_commit_status":field("store_commit_status"),
        "cas_commit_status":field("cas_commit_status"),
        "runtime_write_performed":field("runtime_write_performed"),
        "persistent_user_data_touched":field("persistent_user_data_touched"),
        "stage_advanced":field("stage_advanced"),
        "candidate_confirmed":field("candidate_confirmed"),
        "version_created":field("version_created"),
        "export_performed":field("export_performed"),
        "quality_status":field("quality_status"),
        "error":field("error"),
        "canonical_sha256":field("canonical_sha256"),
        "structured_content_complete":true
    }))
    .ok()
}
