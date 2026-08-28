use serde_json::{json, Value};

const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const TOPOLOGY_POLICY_SHA256: &str =
    "a6fb36a530e49537673b66d65ecb6e4fb4f51ffb3e7d01a0980be71f28cb367d";
const EDIT_POLICY_SHA256: &str = "fc76c6dffef2a41c05ff0a65ff160c8fce5eb37d312a3ef7f78043ef92539144";

const READ_NAMES: [&str; 2] = ["authoring_topology_get", "authoring_mesh_edit_preview"];
const WRITE_NAMES: [&str; 1] = ["authoring_mesh_edit_prepare"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoringMeshTopologyEditTool {
    TopologyGet,
    EditPreview,
    EditPrepare,
}

impl AuthoringMeshTopologyEditTool {
    pub const fn name(self) -> &'static str {
        match self {
            Self::TopologyGet => READ_NAMES[0],
            Self::EditPreview => READ_NAMES[1],
            Self::EditPrepare => WRITE_NAMES[0],
        }
    }

    pub const fn is_write(self) -> bool {
        matches!(self, Self::EditPrepare)
    }

    pub const fn runtime_method(self) -> &'static str {
        self.name()
    }
}

pub fn from_name(name: &str) -> Option<AuthoringMeshTopologyEditTool> {
    Some(match name {
        "authoring_topology_get" => AuthoringMeshTopologyEditTool::TopologyGet,
        "authoring_mesh_edit_preview" => AuthoringMeshTopologyEditTool::EditPreview,
        "authoring_mesh_edit_prepare" => AuthoringMeshTopologyEditTool::EditPrepare,
        _ => return None,
    })
}

pub fn is_tool(name: &str) -> bool {
    from_name(name).is_some()
}

pub fn is_write_tool(name: &str) -> bool {
    from_name(name).is_some_and(AuthoringMeshTopologyEditTool::is_write)
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    from_name(name).map(AuthoringMeshTopologyEditTool::runtime_method)
}

pub fn unavailable_error(name: &str) -> String {
    format!(
        "AUTHORING_MESH_TOPOLOGY_EDIT_RUNTIME_METHOD_UNAVAILABLE: {name} requires Runtime method {name}"
    )
}

pub fn read_tool_names() -> Vec<String> {
    READ_NAMES.iter().map(|name| (*name).to_owned()).collect()
}

pub fn write_tool_names() -> Vec<String> {
    WRITE_NAMES.iter().map(|name| (*name).to_owned()).collect()
}

pub fn read_tools() -> Vec<Value> {
    vec![
        read_tool(
            AuthoringMeshTopologyEditTool::TopologyGet,
            "Read exact candidate-bound source V/E/Loop/Face data from one direct authoring-mesh@1 Part. This is a bounded structural read model, not evaluated GLB topology, BMesh, persistent editing or visual-quality evidence.",
            topology_request_schema(),
        ),
        read_tool(
            AuthoringMeshTopologyEditTool::EditPreview,
            "Apply one bounded translate-vertices, single-face-extrude, split-edge, collapse-edge, or dissolve-edge edit to a transient candidate-bound authoring program and return deterministic Worker hashes/readback without writing CAS, candidates or versions. Typed topology operations expose source-element correspondence only; they do not materialize IdentityLineage.",
            edit_preview_request_schema(),
        ),
    ]
}

pub fn write_tools() -> Vec<Value> {
    vec![write_tool(
        "Explicitly replay one bounded candidate-bound authoring mesh edit, including typed split-edge, collapse-edge, or dissolve-edge operations, through the fixed Geometry Worker and atomically stage the exact derived program, GLB, strict readback, evidence, Job and reviewable candidate. This Runtime-owned write is idempotent, creates no version, performs no confirm or export, and accepts no Blender/Python/plugin payload; topology correspondence remains source-element-only and does not materialize IdentityLineage.",
        edit_prepare_request_schema(),
    )]
}

fn read_tool(tool: AuthoringMeshTopologyEditTool, description: &str, schema: Value) -> Value {
    json!({
        "name":tool.name(),
        "description":description,
        "inputSchema":schema,
        "annotations":{
            "readOnlyHint":true,
            "destructiveHint":false,
            "idempotentHint":true,
            "openWorldHint":false
        },
        "_meta":{"forgecad":{
            "availability":"available",
            "runtime_method":tool.runtime_method(),
            "requiresConfirmation":false,
            "transaction":"MCP010F"
        }}
    })
}

fn write_tool(description: &str, schema: Value) -> Value {
    json!({
        "name":"authoring_mesh_edit_prepare",
        "description":description,
        "inputSchema":schema,
        "annotations":{
            "readOnlyHint":false,
            "destructiveHint":false,
            "idempotentHint":true,
            "openWorldHint":false
        },
        "_meta":{"forgecad":{
            "availability":"available",
            "runtime_method":"authoring_mesh_edit_prepare",
            "requiresConfirmation":true,
            "transaction":"MCP010F"
        }}
    })
}

fn id_property() -> Value {
    json!({"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9_.-]{1,128}$"})
}

fn nullable_id_property() -> Value {
    json!({"type":["string","null"],"maxLength":128,"pattern":"^[A-Za-z0-9_.-]{1,128}$"})
}

fn sha256_property() -> Value {
    json!({"type":"string","pattern":"^[0-9a-f]{64}$"})
}

fn topology_request_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":[
            "schema_version","project_id","candidate_id","artifact_id",
            "artifact_readback_sha256",
            "program_sha256","operator_catalog_sha256","readback_config_sha256",
            "authoring_node_id","part_id","authoring_topology_policy_sha256",
            "max_response_bytes"
        ],
        "properties":{
            "schema_version":{"const":"AuthoringTopologyRequest@1"},
            "project_id":id_property(),
            "candidate_id":id_property(),
            "artifact_id":sha256_property(),
            "artifact_readback_sha256":sha256_property(),
            "program_sha256":sha256_property(),
            "operator_catalog_sha256":sha256_property(),
            "readback_config_sha256":sha256_property(),
            "authoring_node_id":id_property(),
            "part_id":id_property(),
            "authoring_topology_policy_sha256":{"const":TOPOLOGY_POLICY_SHA256},
            "max_response_bytes":{"const":MAX_RESPONSE_BYTES}
        }
    })
}

fn edit_preview_request_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["schema_version","topology_request","base_topology_sha256","edit","edit_policy_sha256","input_sha256"],
        "properties":{
            "schema_version":{"const":"AuthoringMeshEditPreviewRequest@1"},
            "topology_request":topology_request_schema(),
            "base_topology_sha256":sha256_property(),
            "edit":{
                "oneOf":[
                    {
                        "type":"object","additionalProperties":false,
                        "required":["operation","vertex_ids","delta_m"],
                        "properties":{
                            "operation":{"const":"translate_vertices"},
                            "vertex_ids":{"type":"array","minItems":1,"maxItems":64,"uniqueItems":true,"items":id_property()},
                            "delta_m":{"type":"array","minItems":3,"maxItems":3,"items":{"type":"number","minimum":-1.0,"maximum":1.0}}
                        }
                    },
                    {
                        "type":"object","additionalProperties":false,
                        "required":["operation","face_id","distance_m"],
                        "properties":{
                            "operation":{"const":"single_face_extrude"},
                            "face_id":id_property(),
                            "distance_m":{"type":"number","minimum":0.000001,"maximum":1.0}
                        }
                    },
                    {
                        "type":"object","additionalProperties":false,
                        "required":["operation","edge_id","parent_revision","operation_lineage_sha256"],
                        "properties":{
                            "operation":{"const":"split_edge"},
                            "edge_id":id_property(),
                            "parent_revision":{"type":"integer","minimum":0,"maximum":1000000},
                            "operation_lineage_sha256":sha256_property()
                        }
                    },
                    {
                        "type":"object","additionalProperties":false,
                        "required":["operation","edge_id","survivor_vertex_id","parent_revision","operation_lineage_sha256"],
                        "properties":{
                            "operation":{"const":"collapse_edge"},
                            "edge_id":id_property(),
                            "survivor_vertex_id":id_property(),
                            "parent_revision":{"type":"integer","minimum":0,"maximum":1000000},
                            "operation_lineage_sha256":sha256_property()
                        }
                    },
                    {
                        "type":"object","additionalProperties":false,
                        "required":["operation","edge_id","parent_revision","operation_lineage_sha256"],
                        "properties":{
                            "operation":{"const":"dissolve_edge"},
                            "edge_id":id_property(),
                            "parent_revision":{"type":"integer","minimum":0,"maximum":1000000},
                            "operation_lineage_sha256":sha256_property()
                        }
                    }
                ]
            },
            "edit_policy_sha256":{"const":EDIT_POLICY_SHA256},
            "input_sha256":sha256_property()
        }
    })
}

fn edit_prepare_request_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":[
            "schema_version","project_id","source_candidate_id","base_version_id",
            "preview_request","expected_preview_canonical_sha256","idempotency_key",
            "max_response_bytes","input_sha256"
        ],
        "properties":{
            "schema_version":{"const":"AuthoringMeshEditPrepareRequest@1"},
            "project_id":id_property(),
            "source_candidate_id":id_property(),
            "base_version_id":nullable_id_property(),
            "preview_request":edit_preview_request_schema(),
            "expected_preview_canonical_sha256":sha256_property(),
            "idempotency_key":{"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"},
            "max_response_bytes":{"const":MAX_RESPONSE_BYTES},
            "input_sha256":sha256_property()
        }
    })
}

pub fn summary(name: &str, value: &Value) -> Option<String> {
    let tool = from_name(name)?;
    let get = |field: &str| value.get(field).cloned().unwrap_or(Value::Null);
    let topology_operation_proof = || {
        value
            .get("edited_element_ids")
            .and_then(|ids| ids.get("typed_operation_proof"))
            .cloned()
            .unwrap_or(Value::Null)
    };
    let correspondence = || {
        value
            .get("correspondence")
            .cloned()
            .or_else(|| topology_operation_proof().get("correspondence").cloned())
            .unwrap_or(Value::Null)
    };
    let identity_namespace_status = || {
        topology_operation_proof()
            .get("identity_namespace_status")
            .cloned()
            .unwrap_or(Value::Null)
    };
    let summary = match tool {
        AuthoringMeshTopologyEditTool::TopologyGet => json!({
            "schema_version":"AuthoringTopologyMcpSummary@1",
            "project_id":get("project_id"),
            "candidate_id":get("candidate_id"),
            "artifact_id":get("artifact_id"),
            "artifact_readback_sha256":get("artifact_readback_sha256"),
            "authoring_node_id":get("authoring_node_id"),
            "part_id":get("part_id"),
            "counts":get("counts"),
            "topology_sha256":get("topology_sha256"),
            "canonical_sha256":get("canonical_sha256"),
            "structured_content_complete":true
        }),
        AuthoringMeshTopologyEditTool::EditPreview => json!({
            "schema_version":"AuthoringMeshEditPreviewMcpSummary@1",
            "project_id":get("project_id"),
            "candidate_id":get("candidate_id"),
            "source_artifact_id":get("source_artifact_id"),
            "source_program_sha256":get("source_program_sha256"),
            "derived_program_sha256":get("derived_program_sha256"),
            "operation":get("operation"),
            "edited_element_ids":get("edited_element_ids"),
            "counts":get("counts"),
            "source_replay":get("source_replay"),
            "derived_replay":get("derived_replay"),
            "topology_operation_proof":topology_operation_proof(),
            "correspondence":correspondence(),
            "correspondence_sha256":get("correspondence_sha256"),
            "identity_namespace_status":identity_namespace_status(),
            "geometry_materialization":get("geometry_materialization"),
            "canonical_sha256":get("canonical_sha256"),
            "structured_content_complete":true
        }),
        AuthoringMeshTopologyEditTool::EditPrepare => json!({
            "schema_version":"AuthoringMeshEditPrepareMcpSummary@1",
            "project_id":get("project_id"),
            "source_candidate_id":get("source_candidate_id"),
            "new_candidate_id":get("new_candidate_id"),
            "derived_artifact_sha256":get("derived_artifact_sha256"),
            "derived_program_sha256":get("derived_program_sha256"),
            "preview_canonical_sha256":get("preview_canonical_sha256"),
            "edit_lineage_sha256":get("edit_lineage_sha256"),
            "topology_operation_proof":topology_operation_proof(),
            "correspondence":correspondence(),
            "correspondence_sha256":get("correspondence_sha256"),
            "identity_namespace_status":identity_namespace_status(),
            "runtime_write_performed":get("runtime_write_performed"),
            "confirm_status":get("confirm_status"),
            "canonical_sha256":get("canonical_sha256"),
            "structured_content_complete":true
        }),
    };
    serde_json::to_string(&summary).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topology_edit_surface_is_closed_and_runtime_bound() {
        let read = read_tools();
        let write = write_tools();
        assert_eq!(read.len(), 2);
        assert_eq!(write.len(), 1);
        assert_eq!(read[0]["name"], "authoring_topology_get");
        assert_eq!(read[1]["name"], "authoring_mesh_edit_preview");
        assert_eq!(write[0]["name"], "authoring_mesh_edit_prepare");
        assert_eq!(
            read[1]["inputSchema"]["properties"]["edit"]["oneOf"]
                .as_array()
                .map(Vec::len),
            Some(5)
        );
        assert_eq!(
            read[1]["inputSchema"]["properties"]["edit"]["oneOf"][2]["properties"]["operation"]
                ["const"],
            "split_edge"
        );
        assert_eq!(
            read[1]["inputSchema"]["properties"]["edit"]["oneOf"][2]["required"],
            json!([
                "operation",
                "edge_id",
                "parent_revision",
                "operation_lineage_sha256"
            ])
        );
        assert!(read.iter().all(|tool| {
            tool["inputSchema"]["additionalProperties"] == false
                && tool["annotations"]["readOnlyHint"] == true
                && tool["_meta"]["forgecad"]["runtime_method"] == tool["name"]
        }));
        assert_eq!(write[0]["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            write[0]["inputSchema"]["properties"]["max_response_bytes"]["const"],
            MAX_RESPONSE_BYTES
        );
        assert_eq!(write[0]["annotations"]["readOnlyHint"], false);
        assert_eq!(write[0]["annotations"]["idempotentHint"], true);
        assert_eq!(write[0]["_meta"]["forgecad"]["requiresConfirmation"], true);
        assert_eq!(
            runtime_method("authoring_topology_get"),
            Some("authoring_topology_get")
        );
        assert_eq!(
            runtime_method("authoring_mesh_edit_preview"),
            Some("authoring_mesh_edit_preview")
        );
        assert_eq!(
            runtime_method("authoring_mesh_edit_prepare"),
            Some("authoring_mesh_edit_prepare")
        );
        assert!(!is_write_tool("authoring_mesh_edit_preview"));
        assert!(is_write_tool("authoring_mesh_edit_prepare"));
    }

    #[test]
    fn topology_edit_summary_is_hash_only_and_structural() {
        let value = json!({
            "project_id":"project-1",
            "candidate_id":"candidate-1",
            "source_artifact_id":"a".repeat(64),
            "source_program_sha256":"b".repeat(64),
            "derived_program_sha256":"c".repeat(64),
            "operation":"translate_vertices",
            "canonical_sha256":"d".repeat(64)
        });
        let summary = summary("authoring_mesh_edit_preview", &value)
            .and_then(|value| serde_json::from_str::<Value>(&value).ok())
            .expect("topology edit summary");
        assert_eq!(summary["structured_content_complete"], true);
        assert_eq!(summary["operation"], "translate_vertices");
        assert_eq!(summary["canonical_sha256"], "d".repeat(64));
    }

    #[test]
    fn topology_edit_summary_passes_explicit_correspondence_without_inventing_ids() {
        let value = json!({
            "operation":"split_edge",
            "edited_element_ids":{
                "typed_operation_proof":{
                    "correspondence":[{
                        "kind":"one-to-many",
                        "parent_source_element_ids":["e01"],
                        "child_source_element_ids":["xe-a","xe-b"],
                        "operation_lineage_sha256":"e".repeat(64)
                    }],
                    "identity_namespace_status":"source-element-only-not-materialized-to-identity-lineage@1"
                }
            }
        });
        let summary = summary("authoring_mesh_edit_preview", &value)
            .and_then(|value| serde_json::from_str::<Value>(&value).ok())
            .expect("topology edit summary");
        assert_eq!(
            summary["correspondence"][0]["parent_source_element_ids"],
            json!(["e01"])
        );
        assert_eq!(
            summary["topology_operation_proof"]["correspondence"].is_array(),
            true
        );
        assert_eq!(
            summary["identity_namespace_status"],
            "source-element-only-not-materialized-to-identity-lineage@1"
        );
        assert!(summary.get("topology_edit").is_none());
        assert!(summary["correspondence_sha256"].is_null());
    }
}
