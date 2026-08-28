use serde_json::{json, Map, Value};

const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-canonical-sha256@1";

#[derive(Clone, Copy)]
enum Tool {
    Get,
    Prepare,
}

impl Tool {
    fn name(self) -> &'static str {
        match self {
            Self::Get => "weapon_foundation_authoring_materialization_get",
            Self::Prepare => "weapon_foundation_authoring_materialization_prepare",
        }
    }

    fn is_write(self) -> bool {
        matches!(self, Self::Prepare)
    }
}

fn from_name(name: &str) -> Option<Tool> {
    match name {
        "weapon_foundation_authoring_materialization_get" => Some(Tool::Get),
        "weapon_foundation_authoring_materialization_prepare" => Some(Tool::Prepare),
        _ => None,
    }
}

pub fn is_tool(name: &str) -> bool {
    from_name(name).is_some()
}

pub fn is_write_tool(name: &str) -> bool {
    from_name(name).is_some_and(Tool::is_write)
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    from_name(name).map(Tool::name)
}

pub fn unavailable_error(name: &str) -> String {
    format!("WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_RUNTIME_METHOD_UNAVAILABLE: {name}")
}

pub fn read_tool_names() -> Vec<String> {
    vec![Tool::Get.name().to_owned()]
}

pub fn write_tool_names() -> Vec<String> {
    vec![Tool::Prepare.name().to_owned()]
}

fn id() -> Value {
    json!({"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"})
}

fn sha() -> Value {
    json!({"type":"string","pattern":"^[0-9a-f]{64}$"})
}

fn object(required: &[&str], properties: Map<String, Value>) -> Value {
    json!({"type":"object","required":required,"properties":properties,"additionalProperties":false})
}

fn prepare_schema() -> Value {
    object(
        &[
            "schema_version",
            "project_id",
            "foundation_request_id",
            "foundation_request_sha256",
            "foundation_result_object_sha256",
            "topology_object_sha256",
            "socket_map_object_sha256",
            "rig_map_object_sha256",
            "fps_presentation_package_object_sha256",
            "materialization_profile",
            "idempotency_key",
            "max_response_bytes",
            "runtime_write_performed",
            "writer_policy",
            "canonicalization_policy",
            "input_sha256",
        ],
        Map::from_iter([
            (
                "schema_version".to_owned(),
                json!({"const":"WeaponFoundationAuthoringMaterializationPrepareRequest@1"}),
            ),
            ("project_id".to_owned(), id()),
            ("foundation_request_id".to_owned(), id()),
            ("foundation_request_sha256".to_owned(), sha()),
            ("foundation_result_object_sha256".to_owned(), sha()),
            ("topology_object_sha256".to_owned(), sha()),
            ("socket_map_object_sha256".to_owned(), sha()),
            ("rig_map_object_sha256".to_owned(), sha()),
            ("fps_presentation_package_object_sha256".to_owned(), sha()),
            (
                "materialization_profile".to_owned(),
                json!({"const":"part-bounded-authoring-mesh-v2-genesis@1"}),
            ),
            ("idempotency_key".to_owned(), id()),
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
            ("input_sha256".to_owned(), sha()),
        ]),
    )
}

fn get_schema() -> Value {
    object(
        &[
            "schema_version",
            "project_id",
            "materialization_id",
            "writer_policy",
            "runtime_write_performed",
            "persistent_user_data_touched",
            "input_sha256",
        ],
        Map::from_iter([
            (
                "schema_version".to_owned(),
                json!({"const":"WeaponFoundationAuthoringMaterializationGetRequest@1"}),
            ),
            ("project_id".to_owned(), id()),
            ("materialization_id".to_owned(), id()),
            (
                "descriptor_sha256".to_owned(),
                json!({"type":["string","null"],"pattern":"^[0-9a-f]{64}$"}),
            ),
            ("writer_policy".to_owned(), json!({"const":WRITER_POLICY})),
            ("runtime_write_performed".to_owned(), json!({"const":false})),
            (
                "persistent_user_data_touched".to_owned(),
                json!({"const":false}),
            ),
            ("input_sha256".to_owned(), sha()),
        ]),
    )
}

fn definition(tool: Tool) -> Value {
    let (description, input_schema) = match tool {
        Tool::Get => (
            "Read and reverify one Runtime-owned foundation AuthoringMesh@2 materialization. Returns only a compact descriptor and per-Part hashes/counts.",
            get_schema(),
        ),
        Tool::Prepare => (
            "Atomically materialize every Part of one durable typed foundation import into ForgeCAD-owned AuthoringMesh@2 genesis revisions. Full topology remains in CAS; no candidate, version, export, or promotion is created.",
            prepare_schema(),
        ),
    };
    json!({
        "name":tool.name(),
        "description":description,
        "inputSchema":input_schema,
        "annotations":{
            "readOnlyHint":!tool.is_write(),"destructiveHint":false,
            "idempotentHint":true,"openWorldHint":false,"writeIntent":tool.is_write()
        },
        "_meta":{"forgecad":{
            "availability":"available","runtime_method":tool.name(),
            "requiresConfirmation":false,
            "transaction":"WeaponFoundationAuthoringMaterialization@1",
            "maxResponseBytes":MAX_RESPONSE_BYTES,"definition_only":false
        }}
    })
}

pub fn read_tools() -> Vec<Value> {
    vec![definition(Tool::Get)]
}

pub fn write_tools() -> Vec<Value> {
    vec![definition(Tool::Prepare)]
}

pub fn summary(name: &str, value: &Value) -> Option<String> {
    let tool = from_name(name)?;
    serde_json::to_string(&json!({
        "schema_version":"WeaponFoundationAuthoringMaterializationMcpSummary@1",
        "operation":name,
        "write_intent":if tool.is_write() { "explicit_runtime_atomic_foundation_authoring_write" } else { "read_only_runtime_foundation_authoring_lookup" },
        "project_id":value.get("project_id"),
        "materialization_id":value.get("materialization_id"),
        "descriptor_object_sha256":value.get("descriptor_object_sha256"),
        "descriptor_sha256":value.get("descriptor_sha256"),
        "record_sha256":value.get("record_sha256"),
        "part_count":value.pointer("/record/part_count"),
        "source_asset_id":value.pointer("/record/source_asset_id"),
        "materialization_status":value.get("materialization_status"),
        "quality_status":value.get("quality_status"),
        "review_status":value.get("review_status"),
        "replayed":value.get("replayed"),
        "restart_hash_verified":value.get("restart_hash_verified"),
        "runtime_write_performed":value.get("runtime_write_performed"),
        "persistent_user_data_touched":value.get("persistent_user_data_touched"),
        "candidate_created":false,"version_created":false,"export_performed":false,
        "structured_content_complete":true
    })).ok()
}
