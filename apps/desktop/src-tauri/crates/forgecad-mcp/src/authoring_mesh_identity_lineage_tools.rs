use serde_json::{json, Map, Value};

// Keep the MCP surface in lock-step with the four closed @2 contracts. The
// Runtime repeats these limits before touching Store/CAS; MCP must advertise
// the same bounded shape so a valid request reaches Runtime unchanged.
const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-canonical-sha256@1";
// This is the finite pattern understood by the MCP schema subset validator.
// Together with min/maxLength it is equivalent to the contract's
// `^[A-Za-z0-9_.:-]{1,128}$` pattern.
const IDENTIFIER_PATTERN: &str = "^[A-Za-z0-9._:-]+$";

const GET_FIELDS: [&str; 16] = [
    "schema_version",
    "project_id",
    "lineage_id",
    "revision_index",
    "candidate_id",
    "candidate_state_sha256",
    "canonical_mesh_id",
    "canonical_mesh_object_sha256",
    "canonical_mesh_sha256",
    "identity_lineage_object_sha256",
    "identity_lineage_sha256",
    "max_response_bytes",
    "writer_policy",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "input_sha256",
];

const PREPARE_FIELDS: [&str; 31] = [
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
    "canonical_mesh_id",
    "canonical_mesh_object_sha256",
    "canonical_mesh_sha256",
    "genesis_source_mesh_sha256",
    "current_source_mesh_sha256",
    "parent_lineage_object_sha256",
    "parent_lineage_sha256",
    "operation_lineage_sha256",
    "expected_lineage_id",
    "expected_lineage_sha256",
    "idempotency_key",
    "max_response_bytes",
    "runtime_write_performed",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoringMeshIdentityLineageTool {
    Get,
    Prepare,
}

impl AuthoringMeshIdentityLineageTool {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Get => "authoring_mesh_identity_lineage_get",
            Self::Prepare => "authoring_mesh_identity_lineage_prepare",
        }
    }

    pub const fn is_write(self) -> bool {
        matches!(self, Self::Prepare)
    }

    pub const fn runtime_method(self) -> &'static str {
        self.name()
    }
}

pub fn from_name(name: &str) -> Option<AuthoringMeshIdentityLineageTool> {
    Some(match name {
        "authoring_mesh_identity_lineage_get" => AuthoringMeshIdentityLineageTool::Get,
        "authoring_mesh_identity_lineage_prepare" => AuthoringMeshIdentityLineageTool::Prepare,
        _ => return None,
    })
}

pub fn is_tool(name: &str) -> bool {
    from_name(name).is_some()
}

pub fn is_write_tool(name: &str) -> bool {
    from_name(name).is_some_and(AuthoringMeshIdentityLineageTool::is_write)
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    from_name(name).map(AuthoringMeshIdentityLineageTool::runtime_method)
}

pub fn unavailable_error(name: &str) -> String {
    format!(
        "AUTHORING_MESH_IDENTITY_LINEAGE_RUNTIME_METHOD_UNAVAILABLE: {name} requires Runtime method {name}"
    )
}

pub fn read_tool_names() -> Vec<String> {
    vec![AuthoringMeshIdentityLineageTool::Get.name().to_owned()]
}

pub fn write_tool_names() -> Vec<String> {
    vec![AuthoringMeshIdentityLineageTool::Prepare.name().to_owned()]
}

pub fn read_tools() -> Vec<Value> {
    vec![tool_definition(AuthoringMeshIdentityLineageTool::Get)]
}

pub fn write_tools() -> Vec<Value> {
    vec![tool_definition(AuthoringMeshIdentityLineageTool::Prepare)]
}

fn tool_definition(tool: AuthoringMeshIdentityLineageTool) -> Value {
    let (description, schema) = match tool {
        AuthoringMeshIdentityLineageTool::Get => (
            "Read one exact Runtime-owned durable AuthoringMeshIdentityLineage@1 ledger entry. The lookup is structural-only, restart-hash-bound and never writes, advances a stage, confirms a candidate, creates a version or exports.",
            get_schema(),
        ),
        AuthoringMeshIdentityLineageTool::Prepare => (
            "Prepare one Runtime-owned durable AuthoringMeshIdentityLineage@1 ledger entry from exact current AuthoringMesh canonical/program/artifact/readback bindings plus optional Runtime-owned parent evidence. Runtime derives elements, tombstones and correspondence; this explicit write-opt-in never advances a stage, confirms a candidate, creates a version or exports.",
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
            "transaction":"AuthoringMeshIdentityLineage@2",
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

fn nullable_sha256_property() -> Value {
    json!({"type":["string","null"],"pattern":"^[0-9a-f]{64}$"})
}

fn idempotency_key_property() -> Value {
    json!({
        "type":"string",
        "minLength":1,
        "maxLength":128,
        "pattern":"^[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}$"
    })
}

fn revision_index_property() -> Value {
    json!({"type":"integer","minimum":0,"maximum":1_000_000})
}

fn get_schema() -> Value {
    object_schema(
        &GET_FIELDS,
        Map::from_iter([
            (
                "schema_version".to_owned(),
                json!({"const":"AuthoringMeshIdentityLineageGetRequest@2"}),
            ),
            ("project_id".to_owned(), identifier_property()),
            ("lineage_id".to_owned(), identifier_property()),
            ("revision_index".to_owned(), revision_index_property()),
            ("candidate_id".to_owned(), identifier_property()),
            ("candidate_state_sha256".to_owned(), sha256_property()),
            ("canonical_mesh_id".to_owned(), identifier_property()),
            ("canonical_mesh_object_sha256".to_owned(), sha256_property()),
            ("canonical_mesh_sha256".to_owned(), sha256_property()),
            (
                "identity_lineage_object_sha256".to_owned(),
                sha256_property(),
            ),
            ("identity_lineage_sha256".to_owned(), sha256_property()),
            (
                "max_response_bytes".to_owned(),
                json!({"const":MAX_RESPONSE_BYTES}),
            ),
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
                json!({"const":"AuthoringMeshIdentityLineagePrepareRequest@2"}),
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
            ("canonical_mesh_id".to_owned(), identifier_property()),
            ("canonical_mesh_object_sha256".to_owned(), sha256_property()),
            ("canonical_mesh_sha256".to_owned(), sha256_property()),
            ("genesis_source_mesh_sha256".to_owned(), sha256_property()),
            ("current_source_mesh_sha256".to_owned(), sha256_property()),
            (
                "parent_lineage_object_sha256".to_owned(),
                nullable_sha256_property(),
            ),
            (
                "parent_lineage_sha256".to_owned(),
                nullable_sha256_property(),
            ),
            ("operation_lineage_sha256".to_owned(), sha256_property()),
            (
                "expected_lineage_id".to_owned(),
                nullable_identifier_property(),
            ),
            (
                "expected_lineage_sha256".to_owned(),
                nullable_sha256_property(),
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
        "schema_version":"AuthoringMeshIdentityLineageMcpSummary@2",
        "operation":name,
        "write_intent":if tool.is_write() { "explicit_runtime_durable_identity_lineage_prepare_write" } else { "read_only_runtime_durable_identity_lineage_lookup" },
        "project_id":lookup("project_id"),
        "lineage_id":lookup("lineage_id"),
        "revision_index":lookup("revision_index"),
        "revision_kind":lookup("revision_kind"),
        "candidate_id":value.get("candidate_id").cloned().or_else(|| value.get("source_candidate_id").cloned()).unwrap_or(Value::Null),
        "canonical_mesh_id":lookup("canonical_mesh_id"),
        "canonical_mesh_object_sha256":lookup("canonical_mesh_object_sha256"),
        "canonical_mesh_sha256":lookup("canonical_mesh_sha256"),
        "current_source_mesh_sha256":lookup("current_source_mesh_sha256"),
        "identity_lineage_object_sha256":lookup("identity_lineage_object_sha256"),
        "identity_lineage_sha256":lookup("identity_lineage_sha256"),
        "parent_lineage_object_sha256":lookup("parent_lineage_object_sha256"),
        "parent_lineage_sha256":lookup("parent_lineage_sha256"),
        "operation_lineage_sha256":lookup("operation_lineage_sha256"),
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
    fn identity_lineage_v2_tools_match_all_four_closed_contracts() {
        let read = read_tools();
        let write = write_tools();
        assert_eq!(read.len(), 1);
        assert_eq!(write.len(), 1);
        assert_eq!(read[0]["name"], "authoring_mesh_identity_lineage_get");
        assert_eq!(write[0]["name"], "authoring_mesh_identity_lineage_prepare");

        assert_eq!(read[0]["annotations"]["readOnlyHint"], true);
        assert_eq!(read[0]["annotations"]["writeIntent"], false);
        assert_eq!(write[0]["annotations"]["readOnlyHint"], false);
        assert_eq!(write[0]["annotations"]["writeIntent"], true);
        assert_eq!(write[0]["annotations"]["approvalRequired"], true);
        assert_eq!(write[0]["_meta"]["forgecad"]["requiresConfirmation"], true);

        assert_eq!(read[0]["inputSchema"]["required"], json!(GET_FIELDS));
        assert_eq!(write[0]["inputSchema"]["required"], json!(PREPARE_FIELDS));
        assert_eq!(read[0]["inputSchema"]["additionalProperties"], false);
        assert_eq!(write[0]["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            read[0]["inputSchema"]["properties"]["schema_version"]["const"],
            "AuthoringMeshIdentityLineageGetRequest@2"
        );
        assert_eq!(
            write[0]["inputSchema"]["properties"]["schema_version"]["const"],
            "AuthoringMeshIdentityLineagePrepareRequest@2"
        );
        for tool in [&read[0], &write[0]] {
            assert_eq!(
                tool["inputSchema"]["properties"]["max_response_bytes"]["const"],
                MAX_RESPONSE_BYTES
            );
        }

        // The caller cannot smuggle derived identity truth into either closed
        // request. Runtime owns elements, tombstones and correspondence.
        for field in ["elements", "tombstones", "correspondence"] {
            assert!(write[0]["inputSchema"]["properties"].get(field).is_none());
        }
        assert_eq!(
            write[0]["inputSchema"]["properties"]["base_version_id"]["type"],
            json!(["string", "null"])
        );
        assert_eq!(
            write[0]["inputSchema"]["properties"]["expected_lineage_sha256"]["type"],
            json!(["string", "null"])
        );

        assert_eq!(
            runtime_method("authoring_mesh_identity_lineage_get"),
            Some("authoring_mesh_identity_lineage_get")
        );
        assert_eq!(
            runtime_method("authoring_mesh_identity_lineage_prepare"),
            Some("authoring_mesh_identity_lineage_prepare")
        );
        assert!(!is_write_tool("authoring_mesh_identity_lineage_get"));
        assert!(is_write_tool("authoring_mesh_identity_lineage_prepare"));
    }

    #[test]
    fn identity_lineage_summary_is_hash_only_and_does_not_invent_correspondence() {
        let summary = summary(
            "authoring_mesh_identity_lineage_get",
            &json!({
                "canonical_mesh_object_sha256":"a".repeat(64),
                "canonical_mesh_sha256":"b".repeat(64),
                "identity_lineage_object_sha256":"c".repeat(64),
                "identity_lineage_sha256":"d".repeat(64)
            }),
        )
        .and_then(|value| serde_json::from_str::<Value>(&value).ok())
        .expect("identity summary");
        assert_eq!(summary["structured_content_complete"], true);
        assert!(summary.get("correspondence_status").is_none());
        assert_eq!(
            summary["write_intent"],
            "read_only_runtime_durable_identity_lineage_lookup"
        );
        assert_eq!(summary["runtime_write_performed"], false);
        assert_eq!(summary["persistent_user_data_touched"], false);
    }
}
