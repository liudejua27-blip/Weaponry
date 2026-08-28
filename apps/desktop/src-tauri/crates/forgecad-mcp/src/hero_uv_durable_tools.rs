use serde_json::{json, Map, Value};

const MAX_RESPONSE_BYTES: u64 = 8_388_608;
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-canonical-sha256@1";
const IDENTIFIER_PATTERN: &str = "^[A-Za-z0-9_.:-]{1,128}$";

const GET_FIELDS: [&str; 24] = [
    "schema_version",
    "operation",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "base_version_id",
    "source_low_artifact_id",
    "source_low_artifact_sha256",
    "layout_object_sha256",
    "layout_canonical_sha256",
    "link_id",
    "link_object_sha256",
    "resolution",
    "padding_texels",
    "min_mip_level",
    "hard_edge_angle_deg",
    "stretch_threshold",
    "visibility_weights_sha256",
    "idempotency_key",
    "source_only",
    "writer_policy",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "input_sha256",
];

const PREPARE_FIELDS: [&str; 23] = [
    "schema_version",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "base_version_id",
    "source_low_artifact_id",
    "source_low_artifact_object_sha256",
    "source_low_artifact_sha256",
    "source_low_artifact_readback_object_sha256",
    "source_low_artifact_readback_sha256",
    "resolution",
    "padding_texels",
    "min_mip_level",
    "hard_edge_angle_deg",
    "stretch_threshold",
    "visibility_weights",
    "idempotency_key",
    "max_response_bytes",
    "source_only",
    "runtime_write_performed",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeroUvDurableTool {
    Get,
    Prepare,
}

impl HeroUvDurableTool {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Get => "hero_uv_durable_get",
            Self::Prepare => "hero_uv_durable_prepare",
        }
    }

    pub const fn is_write(self) -> bool {
        matches!(self, Self::Prepare)
    }

    pub const fn runtime_method(self) -> &'static str {
        self.name()
    }
}

pub fn from_name(name: &str) -> Option<HeroUvDurableTool> {
    Some(match name {
        "hero_uv_durable_get" => HeroUvDurableTool::Get,
        "hero_uv_durable_prepare" => HeroUvDurableTool::Prepare,
        _ => return None,
    })
}

pub fn is_tool(name: &str) -> bool {
    from_name(name).is_some()
}

pub fn is_write_tool(name: &str) -> bool {
    from_name(name).is_some_and(HeroUvDurableTool::is_write)
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    from_name(name).map(HeroUvDurableTool::runtime_method)
}

pub fn unavailable_error(name: &str) -> String {
    format!("HERO_UV_DURABLE_RUNTIME_METHOD_UNAVAILABLE: {name} requires Runtime method {name}")
}

pub fn read_tool_names() -> Vec<String> {
    vec![HeroUvDurableTool::Get.name().to_owned()]
}

pub fn write_tool_names() -> Vec<String> {
    vec![HeroUvDurableTool::Prepare.name().to_owned()]
}

pub fn read_tools() -> Vec<Value> {
    vec![tool_definition(HeroUvDurableTool::Get)]
}

pub fn write_tools() -> Vec<Value> {
    vec![tool_definition(HeroUvDurableTool::Prepare)]
}

fn tool_definition(tool: HeroUvDurableTool) -> Value {
    let (description, schema) = match tool {
        HeroUvDurableTool::Get => (
            "Read one exact Runtime-owned HeroUvLayout structural diagnostic and its Low artifact/readback/CAS bindings after restart. This lookup never advances a production stage, confirms a candidate, creates a version or exports.",
            get_schema(),
        ),
        HeroUvDurableTool::Prepare => (
            "Prepare one Runtime-owned HeroUvLayout structural diagnostic from an exact candidate-bound Low artifact/readback. Runtime replays the bounded Worker twice and persists the layout/link through CAS and SQLite; explicit MCP write opt-in is required, and no stage, confirmation, version or export is performed.",
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
            "transaction":"HeroUvDurable@1",
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

fn visibility_weights_schema() -> Value {
    object_schema(
        &["part_id", "first_person", "world", "hidden"],
        Map::from_iter([
            ("part_id".to_owned(), identifier_property()),
            (
                "first_person".to_owned(),
                json!({"type":"number","minimum":0,"maximum":1}),
            ),
            (
                "world".to_owned(),
                json!({"type":"number","minimum":0,"maximum":1}),
            ),
            (
                "hidden".to_owned(),
                json!({"type":"number","minimum":0,"maximum":1}),
            ),
        ]),
    )
}

fn common_properties() -> Map<String, Value> {
    Map::from_iter([
        ("project_id".to_owned(), identifier_property()),
        ("candidate_id".to_owned(), identifier_property()),
        ("candidate_state_sha256".to_owned(), sha256_property()),
        ("base_version_id".to_owned(), nullable_identifier_property()),
        ("source_low_artifact_id".to_owned(), identifier_property()),
        ("source_low_artifact_sha256".to_owned(), sha256_property()),
        ("resolution".to_owned(), json!({"enum":[2048,4096]})),
        (
            "padding_texels".to_owned(),
            json!({"type":"integer","minimum":1,"maximum":128}),
        ),
        (
            "min_mip_level".to_owned(),
            json!({"type":"integer","minimum":0,"maximum":12}),
        ),
        (
            "hard_edge_angle_deg".to_owned(),
            json!({"type":"number","exclusiveMinimum":0.1,"exclusiveMaximum":89.9}),
        ),
        (
            "stretch_threshold".to_owned(),
            json!({"type":"number","minimum":1,"maximum":100}),
        ),
        ("idempotency_key".to_owned(), identifier_property()),
        ("source_only".to_owned(), json!({"const":true})),
        ("runtime_write_performed".to_owned(), json!({"const":false})),
        ("writer_policy".to_owned(), json!({"const":WRITER_POLICY})),
        ("input_sha256".to_owned(), sha256_property()),
    ])
}

fn get_schema() -> Value {
    let mut properties = common_properties();
    properties.extend(Map::from_iter([
        (
            "schema_version".to_owned(),
            json!({"const":"HeroUvDurableGetRequest@1"}),
        ),
        (
            "operation".to_owned(),
            json!({"const":"forgecad.production.hero-uv-durable-get@1"}),
        ),
        ("layout_object_sha256".to_owned(), sha256_property()),
        ("layout_canonical_sha256".to_owned(), sha256_property()),
        ("link_id".to_owned(), identifier_property()),
        ("link_object_sha256".to_owned(), sha256_property()),
        ("visibility_weights_sha256".to_owned(), sha256_property()),
        (
            "persistent_user_data_touched".to_owned(),
            json!({"const":false}),
        ),
    ]));
    object_schema(&GET_FIELDS, properties)
}

fn prepare_schema() -> Value {
    let mut properties = common_properties();
    properties.extend(Map::from_iter([
        (
            "source_low_artifact_object_sha256".to_owned(),
            sha256_property(),
        ),
        (
            "source_low_artifact_readback_object_sha256".to_owned(),
            sha256_property(),
        ),
        (
            "source_low_artifact_readback_sha256".to_owned(),
            sha256_property(),
        ),
        (
            "schema_version".to_owned(),
            json!({"const":"HeroUvDurablePrepareRequest@1"}),
        ),
        (
            "source_low_artifact_object_sha256".to_owned(),
            sha256_property(),
        ),
        (
            "visibility_weights".to_owned(),
            json!({
                "type":"array",
                "minItems":1,
                "maxItems":4096,
                "uniqueItems":true,
                "items":visibility_weights_schema()
            }),
        ),
        (
            "max_response_bytes".to_owned(),
            json!({"const":MAX_RESPONSE_BYTES}),
        ),
        (
            "canonicalization_policy".to_owned(),
            json!({"const":CANONICALIZATION_POLICY}),
        ),
    ]));
    object_schema(&PREPARE_FIELDS, properties)
}

pub fn summary(name: &str, value: &Value) -> Option<String> {
    let tool = from_name(name)?;
    let lookup = |field: &str| value.get(field).cloned().unwrap_or(Value::Null);
    let link_lookup = |field: &str| {
        value
            .get(field)
            .cloned()
            .or_else(|| value.pointer(&format!("/durable_link/{field}")).cloned())
            .unwrap_or(Value::Null)
    };
    let mut summary = Map::new();
    summary.insert(
        "schema_version".to_owned(),
        Value::String("HeroUvDurableMcpSummary@1".to_owned()),
    );
    summary.insert("tool".to_owned(), Value::String(tool.name().to_owned()));
    summary.insert(
        "operation".to_owned(),
        value
            .get("operation")
            .cloned()
            .unwrap_or_else(|| Value::String(name.to_owned())),
    );
    summary.insert(
        "runtime_method".to_owned(),
        Value::String(tool.runtime_method().to_owned()),
    );
    summary.insert(
        "write_intent".to_owned(),
        Value::String(
            if tool.is_write() {
                "explicit_runtime_hero_uv_durable_prepare_write"
            } else {
                "read_only_runtime_hero_uv_durable_lookup"
            }
            .to_owned(),
        ),
    );
    summary.insert("result_schema_version".to_owned(), lookup("schema_version"));
    for field in [
        "project_id",
        "candidate_id",
        "candidate_state_sha256",
        "base_version_id",
        "source_low_artifact_id",
        "source_low_artifact_object_sha256",
        "source_low_artifact_sha256",
        "source_low_artifact_readback_object_sha256",
        "source_low_artifact_readback_sha256",
        "layout_object_sha256",
        "layout_canonical_sha256",
        "worker_build_cohort_sha256",
        "link_id",
        "link_object_sha256",
        "resolution",
        "padding_texels",
        "min_mip_level",
        "hard_edge_angle_deg",
        "stretch_threshold",
        "visibility_weights_sha256",
        "request_sha256",
        "idempotency_key",
        "materialization_status",
        "link_policy",
        "idempotency_policy",
        "input_sha256",
        "created_at",
    ] {
        summary.insert(field.to_owned(), link_lookup(field));
    }
    summary.insert(
        "request_input_sha256".to_owned(),
        value
            .get("request_input_sha256")
            .cloned()
            .or_else(|| value.get("input_sha256").cloned())
            .unwrap_or(Value::Null),
    );
    for field in [
        "replayed",
        "replay_byte_exact",
        "restart_hash_verified",
        "runtime_write_performed",
        "persistent_user_data_touched",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
    ] {
        summary.insert(field.to_owned(), link_lookup(field));
    }
    summary.insert("replay_count".to_owned(), link_lookup("replay_count"));
    summary.insert(
        "quality_status".to_owned(),
        value
            .get("quality_status")
            .cloned()
            .or_else(|| value.pointer("/durable_link/quality_status").cloned())
            .unwrap_or_else(|| Value::String("structural_only".to_owned())),
    );
    for field in [
        "structural_status",
        "visual_status",
        "human_status",
        "engine_status",
        "distribution_status",
    ] {
        summary.insert(
            field.to_owned(),
            value
                .get(field)
                .cloned()
                .or_else(|| value.pointer(&format!("/durable_link/{field}")).cloned())
                .or_else(|| value.pointer(&format!("/layout/{field}")).cloned())
                .unwrap_or_else(|| Value::String("NOT_PROVEN".to_owned())),
        );
    }
    for field in [
        "source_only",
        "limitations",
        "canonicalization_policy",
        "canonical_sha256",
    ] {
        summary.insert(field.to_owned(), lookup(field));
    }
    summary.insert("structured_content_complete".to_owned(), Value::Bool(true));
    serde_json::to_string(&Value::Object(summary)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hero_uv_durable_tools_are_closed_and_classified() {
        let read = read_tools();
        let write = write_tools();
        assert_eq!(read.len(), 1);
        assert_eq!(write.len(), 1);
        assert_eq!(read[0]["name"], "hero_uv_durable_get");
        assert_eq!(write[0]["name"], "hero_uv_durable_prepare");
        assert_eq!(read[0]["annotations"]["readOnlyHint"], true);
        assert_eq!(write[0]["annotations"]["readOnlyHint"], false);
        assert_eq!(read[0]["annotations"]["writeIntent"], false);
        assert_eq!(write[0]["annotations"]["writeIntent"], true);
        assert_eq!(read[0]["inputSchema"]["additionalProperties"], false);
        assert_eq!(write[0]["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            read[0]["inputSchema"]["properties"]["schema_version"]["const"],
            "HeroUvDurableGetRequest@1"
        );
        assert_eq!(
            write[0]["inputSchema"]["properties"]["schema_version"]["const"],
            "HeroUvDurablePrepareRequest@1"
        );
        assert_eq!(
            write[0]["inputSchema"]["properties"]["max_response_bytes"]["const"],
            MAX_RESPONSE_BYTES
        );
        assert_eq!(
            write[0]["inputSchema"]["properties"]["resolution"]["enum"],
            json!([2048, 4096])
        );
        assert!(is_write_tool("hero_uv_durable_prepare"));
        assert!(!is_write_tool("hero_uv_durable_get"));
        assert_eq!(
            runtime_method("hero_uv_durable_get"),
            Some("hero_uv_durable_get")
        );
        assert_eq!(
            runtime_method("hero_uv_durable_prepare"),
            Some("hero_uv_durable_prepare")
        );
        let summary: Value = serde_json::from_str(
            &super::summary(
                "hero_uv_durable_prepare",
                &json!({
                "candidate_state_sha256":"a".repeat(64),
                    "schema_version":"HeroUvDurablePrepareResult@1",
                    "operation":"forgecad.production.hero-uv-durable-prepare@1",
                    "source_low_artifact_object_sha256":"b".repeat(64),
                    "source_low_artifact_readback_sha256":"c".repeat(64),
                    "worker_build_cohort_sha256":"d".repeat(64),
                    "request_sha256":"e".repeat(64),
                    "replayed":true,
                    "restart_hash_verified":true,
                    "runtime_write_performed":true,
                    "production_stage_advanced":false,
                    "candidate_confirmed":false,
                    "version_created":false,
                    "export_performed":false,
                    "source_only":true,
                    "quality_status":"structural_only"
                }),
            )
            .expect("Hero UV summary"),
        )
        .expect("Hero UV summary JSON");
        assert_eq!(summary["schema_version"], "HeroUvDurableMcpSummary@1");
        assert_eq!(summary["tool"], "hero_uv_durable_prepare");
        assert_eq!(summary["runtime_method"], "hero_uv_durable_prepare");
        assert_eq!(
            summary["operation"],
            "forgecad.production.hero-uv-durable-prepare@1"
        );
        assert_eq!(
            summary["result_schema_version"],
            "HeroUvDurablePrepareResult@1"
        );
        assert_eq!(summary["worker_build_cohort_sha256"], "d".repeat(64));
        assert_eq!(summary["restart_hash_verified"], true);
        assert_eq!(summary["production_stage_advanced"], false);
        assert_eq!(summary["structured_content_complete"], true);
    }
}
