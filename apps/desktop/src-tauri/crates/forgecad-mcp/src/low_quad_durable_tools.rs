use serde_json::{json, Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LowQuadDurableTool {
    Get,
    Prepare,
}

impl LowQuadDurableTool {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Get => "low_quad_draft_durable_get",
            Self::Prepare => "low_quad_draft_durable_prepare",
        }
    }

    pub const fn is_write(self) -> bool {
        matches!(self, Self::Prepare)
    }

    pub const fn runtime_method(self) -> &'static str {
        self.name()
    }
}

pub fn from_name(name: &str) -> Option<LowQuadDurableTool> {
    Some(match name {
        "low_quad_draft_durable_get" => LowQuadDurableTool::Get,
        "low_quad_draft_durable_prepare" => LowQuadDurableTool::Prepare,
        _ => return None,
    })
}

pub fn is_tool(name: &str) -> bool {
    from_name(name).is_some()
}

pub fn is_write_tool(name: &str) -> bool {
    from_name(name).is_some_and(LowQuadDurableTool::is_write)
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    from_name(name).map(LowQuadDurableTool::runtime_method)
}

pub fn unavailable_error(name: &str) -> String {
    format!(
        "LOW_QUAD_DRAFT_DURABLE_RUNTIME_METHOD_UNAVAILABLE: {name} requires Runtime method {name}"
    )
}

pub fn read_tool_names() -> Vec<String> {
    vec![LowQuadDurableTool::Get.name().to_owned()]
}

pub fn write_tool_names() -> Vec<String> {
    vec![LowQuadDurableTool::Prepare.name().to_owned()]
}

pub fn read_tools() -> Vec<Value> {
    vec![tool_definition(LowQuadDurableTool::Get)]
}

pub fn write_tools() -> Vec<Value> {
    vec![tool_definition(LowQuadDurableTool::Prepare)]
}

fn request_schema(tool: LowQuadDurableTool) -> Value {
    let text = match tool {
        LowQuadDurableTool::Get => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../packages/forgecad-contracts/schemas/low-quad-draft-durable-get-request.schema.json"
        )),
        LowQuadDurableTool::Prepare => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../packages/forgecad-contracts/schemas/low-quad-draft-durable-prepare-request.schema.json"
        )),
    };
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("failed to parse embedded Low durable MCP schema: {error}"))
}

fn tool_definition(tool: LowQuadDurableTool) -> Value {
    let description = match tool {
        LowQuadDurableTool::Get => {
            "Read one exact Runtime-owned Low quad draft durable link and its source Native High, Worker result, artifact, strict readback and CAS bindings. This source-only lookup never advances a stage, confirms a candidate, creates a version or exports."
        }
        LowQuadDurableTool::Prepare => {
            "Prepare one Runtime-owned source-only explicit all-quad Low draft from exact candidate-bound Native High lineage and a bounded typed Worker request. The result remains DRAFT_UNREVIEWED, structural-only and promotion-ineligible; explicit MCP write opt-in is required and no stage, confirmation, version or export is performed."
        }
    };
    json!({
        "name":tool.name(),
        "description":description,
        "inputSchema":request_schema(tool),
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
            "transaction":"LowQuadDraftDurable@1",
            "definition_only":false
        }}
    })
}

pub fn summary(name: &str, value: &Value) -> Option<String> {
    let tool = from_name(name)?;
    let link_field = |field: &str| {
        value
            .get("durable_link")
            .and_then(|link| link.get(field))
            .cloned()
            .unwrap_or(Value::Null)
    };
    let mut summary = Map::new();
    let mut insert = |field: &str, field_value: Value| {
        summary.insert(field.to_owned(), field_value);
    };
    let value_field = |field: &str| value.get(field).cloned().unwrap_or(Value::Null);
    insert(
        "schema_version",
        Value::String("LowQuadDraftDurableMcpSummary@1".to_owned()),
    );
    insert("tool", Value::String(tool.name().to_owned()));
    insert("operation", value_field("operation"));
    insert(
        "runtime_method",
        Value::String(tool.runtime_method().to_owned()),
    );
    insert(
        "write_intent",
        Value::String(
            if tool.is_write() {
                "explicit_runtime_low_quad_durable_prepare_write"
            } else {
                "read_only_runtime_low_quad_durable_get"
            }
            .to_owned(),
        ),
    );
    insert("result_schema_version", value_field("schema_version"));
    insert("project_id", value_field("project_id"));
    insert("candidate_id", value_field("candidate_id"));
    insert(
        "candidate_state_sha256",
        value_field("candidate_state_sha256"),
    );
    insert("base_version_id", value_field("base_version_id"));
    insert("link_id", value_field("link_id"));
    insert("link_object_sha256", value_field("link_object_sha256"));
    insert(
        "source_high_artifact_id",
        link_field("source_high_artifact_id"),
    );
    insert(
        "source_high_artifact_object_sha256",
        link_field("source_high_artifact_object_sha256"),
    );
    insert(
        "source_high_artifact_sha256",
        link_field("source_high_artifact_sha256"),
    );
    insert(
        "source_high_artifact_readback_object_sha256",
        link_field("source_high_artifact_readback_object_sha256"),
    );
    insert(
        "source_high_artifact_readback_sha256",
        link_field("source_high_artifact_readback_sha256"),
    );
    insert(
        "worker_result_object_sha256",
        value_field("worker_result_object_sha256"),
    );
    insert("worker_result_sha256", value_field("worker_result_sha256"));
    insert(
        "artifact_object_sha256",
        value_field("artifact_object_sha256"),
    );
    insert("artifact_sha256", value_field("artifact_sha256"));
    insert(
        "readback_object_sha256",
        value_field("readback_object_sha256"),
    );
    insert("readback_sha256", value_field("readback_sha256"));
    insert("request_sha256", link_field("request_sha256"));
    insert("request_input_sha256", value_field("request_input_sha256"));
    insert("idempotency_key", value_field("idempotency_key"));
    insert("replayed", value_field("replayed"));
    insert(
        "restart_hash_verified",
        value_field("restart_hash_verified"),
    );
    insert(
        "persistent_user_data_touched",
        value_field("persistent_user_data_touched"),
    );
    insert("source_only", Value::Bool(true));
    insert("edge_flow_status", value_field("edge_flow_status"));
    insert("quality_status", value_field("quality_status"));
    insert("promotion_eligible", value_field("promotion_eligible"));
    insert(
        "runtime_write_performed",
        value_field("runtime_write_performed"),
    );
    insert(
        "production_stage_advanced",
        value_field("production_stage_advanced"),
    );
    insert("candidate_confirmed", value_field("candidate_confirmed"));
    insert("version_created", value_field("version_created"));
    insert("export_performed", value_field("export_performed"));
    insert("visual_status", link_field("visual_status"));
    insert("human_status", link_field("human_status"));
    insert("engine_status", link_field("engine_status"));
    insert("distribution_status", link_field("distribution_status"));
    insert("validator_status", link_field("validator_status"));
    insert("hard_gate_passed", link_field("hard_gate_passed"));
    insert("explicit_quad_faces", link_field("explicit_quad_faces"));
    insert(
        "artist_authored_quad_topology",
        link_field("artist_authored_quad_topology"),
    );
    insert("limitations", value_field("limitations"));
    insert(
        "canonicalization_policy",
        value_field("canonicalization_policy"),
    );
    insert("canonical_sha256", value_field("canonical_sha256"));
    insert("structured_content_complete", Value::Bool(true));
    serde_json::to_string(&Value::Object(summary)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_one_read_and_one_opt_in_write_tool() {
        assert_eq!(read_tool_names(), ["low_quad_draft_durable_get"]);
        assert_eq!(write_tool_names(), ["low_quad_draft_durable_prepare"]);
        assert!(!is_write_tool("low_quad_draft_durable_get"));
        assert!(is_write_tool("low_quad_draft_durable_prepare"));
        assert_eq!(
            read_tools()[0]["inputSchema"]["additionalProperties"],
            false
        );
        assert_eq!(
            write_tools()[0]["inputSchema"]["additionalProperties"],
            false
        );
    }

    #[test]
    fn contracts_are_closed_with_exact_runtime_routing_and_write_metadata() {
        let read = read_tools();
        let write = write_tools();
        let get = &read[0];
        let prepare = &write[0];

        assert_eq!(get["name"], "low_quad_draft_durable_get");
        assert_eq!(prepare["name"], "low_quad_draft_durable_prepare");
        assert_eq!(get["annotations"]["readOnlyHint"], true);
        assert_eq!(get["annotations"]["writeIntent"], false);
        assert_eq!(prepare["annotations"]["readOnlyHint"], false);
        assert_eq!(prepare["annotations"]["writeIntent"], true);
        assert_eq!(get["annotations"]["destructiveHint"], false);
        assert_eq!(prepare["annotations"]["destructiveHint"], false);
        assert_eq!(get["_meta"]["forgecad"]["runtime_method"], get["name"]);
        assert_eq!(
            prepare["_meta"]["forgecad"]["runtime_method"],
            prepare["name"]
        );
        assert_eq!(
            get["_meta"]["forgecad"]["transaction"],
            "LowQuadDraftDurable@1"
        );
        assert_eq!(
            prepare["_meta"]["forgecad"]["transaction"],
            "LowQuadDraftDurable@1"
        );

        for schema in [get["inputSchema"].clone(), prepare["inputSchema"].clone()] {
            assert_eq!(schema["type"], "object");
            assert_eq!(schema["additionalProperties"], false);
            assert!(schema["required"].as_array().is_some());
        }
        assert_eq!(
            get["inputSchema"]["properties"]["schema_version"]["const"],
            "LowQuadDraftDurableGetRequest@1"
        );
        assert_eq!(
            get["inputSchema"]["properties"]["operation"]["const"],
            "forgecad.production.low-quad-draft-durable-get@1"
        );
        assert_eq!(
            get["inputSchema"]["properties"]["source_only"]["const"],
            true
        );
        assert_eq!(
            get["inputSchema"]["properties"]["runtime_write_performed"]["const"],
            false
        );
        assert_eq!(
            prepare["inputSchema"]["properties"]["schema_version"]["const"],
            "LowQuadDraftDurablePrepareRequest@1"
        );
        assert_eq!(
            prepare["inputSchema"]["properties"]["low_quad_draft_worker_request"]["$ref"],
            "https://forgecad.local/contracts/low-quad-draft-worker-request.schema.json"
        );
        assert_eq!(
            prepare["inputSchema"]["properties"]["max_response_bytes"]["const"],
            1_048_576
        );
        assert_eq!(
            prepare["inputSchema"]["properties"]["canonicalization_policy"]["const"],
            "canonical-json-sha256-excluding-canonical-sha256@1"
        );

        assert_eq!(
            runtime_method("low_quad_draft_durable_get"),
            Some("low_quad_draft_durable_get")
        );
        assert_eq!(
            runtime_method("low_quad_draft_durable_prepare"),
            Some("low_quad_draft_durable_prepare")
        );
        assert_eq!(runtime_method("low_quad_draft_durable_unknown"), None);
        assert!(!is_tool("low_quad_draft_durable_unknown"));
        assert_eq!(
            unavailable_error("low_quad_draft_durable_get"),
            "LOW_QUAD_DRAFT_DURABLE_RUNTIME_METHOD_UNAVAILABLE: low_quad_draft_durable_get requires Runtime method low_quad_draft_durable_get"
        );
    }

    #[test]
    fn summary_preserves_prepare_restart_get_lineage_and_no_promotion_evidence() {
        let hash = "a".repeat(64);
        let value = json!({
            "schema_version":"LowQuadDraftDurablePrepareResult@1",
            "operation":"forgecad.production.low-quad-draft-durable-prepare@1",
            "project_id":"project-1",
            "candidate_id":"candidate-1",
            "candidate_state_sha256":hash,
            "base_version_id":null,
            "link_id":"link-1",
            "link_object_sha256":hash,
            "durable_link":{
                "source_high_artifact_id":"high-1",
                "source_high_artifact_object_sha256":hash,
                "source_high_artifact_sha256":hash,
                "source_high_artifact_readback_object_sha256":hash,
                "source_high_artifact_readback_sha256":hash,
                "worker_result_object_sha256":hash,
                "worker_result_sha256":hash,
                "artifact_object_sha256":hash,
                "artifact_sha256":hash,
                "readback_object_sha256":hash,
                "readback_sha256":hash,
                "request_sha256":hash,
                "visual_status":"NOT_PROVEN",
                "human_status":"NOT_RUN",
                "engine_status":"NOT_RUN",
                "distribution_status":"NOT_RUN",
                "validator_status":"PASS",
                "hard_gate_passed":false,
                "explicit_quad_faces":true,
                "artist_authored_quad_topology":false
            },
            "worker_result_object_sha256":hash,
            "worker_result_sha256":hash,
            "artifact_object_sha256":hash,
            "artifact_sha256":hash,
            "readback_object_sha256":hash,
            "readback_sha256":hash,
            "request_input_sha256":hash,
            "idempotency_key":"low-1",
            "replayed":true,
            "restart_hash_verified":true,
            "runtime_write_performed":true,
            "persistent_user_data_touched":true,
            "production_stage_advanced":false,
            "promotion_eligible":false,
            "candidate_confirmed":false,
            "version_created":false,
            "export_performed":false,
            "quality_status":"structural_only",
            "edge_flow_status":"DRAFT_UNREVIEWED",
            "limitations":["DRAFT_UNREVIEWED"],
            "canonicalization_policy":"canonical-json-sha256-excluding-canonical-sha256@1",
            "canonical_sha256":hash
        });

        let prepare_summary: Value = serde_json::from_str(
            &summary("low_quad_draft_durable_prepare", &value).expect("prepare summary"),
        )
        .expect("summary is JSON");
        assert_eq!(
            prepare_summary["schema_version"],
            "LowQuadDraftDurableMcpSummary@1"
        );
        assert_eq!(
            prepare_summary["operation"],
            "forgecad.production.low-quad-draft-durable-prepare@1"
        );
        assert_eq!(
            prepare_summary["write_intent"],
            "explicit_runtime_low_quad_durable_prepare_write"
        );
        assert_eq!(
            prepare_summary["runtime_method"],
            "low_quad_draft_durable_prepare"
        );
        assert_eq!(prepare_summary["source_high_artifact_id"], "high-1");
        assert_eq!(prepare_summary["source_high_artifact_object_sha256"], hash);
        assert_eq!(
            prepare_summary["source_high_artifact_readback_sha256"],
            hash
        );
        assert_eq!(prepare_summary["artifact_object_sha256"], hash);
        assert_eq!(prepare_summary["readback_object_sha256"], hash);
        assert_eq!(prepare_summary["request_input_sha256"], hash);
        assert_eq!(prepare_summary["idempotency_key"], "low-1");
        assert_eq!(prepare_summary["replayed"], true);
        assert_eq!(prepare_summary["restart_hash_verified"], true);
        assert_eq!(prepare_summary["persistent_user_data_touched"], true);
        assert_eq!(prepare_summary["source_only"], true);
        assert_eq!(prepare_summary["production_stage_advanced"], false);
        assert_eq!(prepare_summary["promotion_eligible"], false);
        assert_eq!(prepare_summary["candidate_confirmed"], false);
        assert_eq!(prepare_summary["version_created"], false);
        assert_eq!(prepare_summary["export_performed"], false);
        assert_eq!(prepare_summary["visual_status"], "NOT_PROVEN");
        assert_eq!(prepare_summary["human_status"], "NOT_RUN");
        assert_eq!(prepare_summary["structured_content_complete"], true);

        let get_summary: Value = serde_json::from_str(
            &summary(
                "low_quad_draft_durable_get",
                &json!({
                    "schema_version":"LowQuadDraftDurableGetResult@1",
                    "operation":"forgecad.production.low-quad-draft-durable-get@1",
                    "runtime_write_performed":false,
                    "persistent_user_data_touched":false,
                    "restart_hash_verified":true,
                    "durable_link":{"source_high_artifact_id":"high-1"}
                }),
            )
            .expect("get summary"),
        )
        .expect("get summary is JSON");
        assert_eq!(
            get_summary["write_intent"],
            "read_only_runtime_low_quad_durable_get"
        );
        assert_eq!(get_summary["runtime_method"], "low_quad_draft_durable_get");
        assert_eq!(get_summary["runtime_write_performed"], false);
        assert_eq!(get_summary["persistent_user_data_touched"], false);
        assert_eq!(get_summary["restart_hash_verified"], true);
        assert_eq!(summary("unknown_low_quad_tool", &value), None);
    }
}
