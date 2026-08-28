use serde_json::{json, Map, Value};

const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const POLICY: &str = "runtime-owned-editable-weapon-arms-sockets-rig-clips-composite@2";
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const CANONICAL: &str = "canonical-json-sha256-excluding-canonical-sha256@1";

#[derive(Clone, Copy)]
enum Tool {
    Get,
    Prepare,
    ProductionPreflightGet,
}
impl Tool {
    fn name(self) -> &'static str {
        match self {
            Self::Get => "fps_presentation_package_v2_get",
            Self::Prepare => "fps_presentation_package_v2_prepare",
            Self::ProductionPreflightGet => "fps_presentation_package_v2_production_preflight_get",
        }
    }
    fn is_write(self) -> bool {
        matches!(self, Self::Prepare)
    }
}
fn from_name(name: &str) -> Option<Tool> {
    match name {
        "fps_presentation_package_v2_get" => Some(Tool::Get),
        "fps_presentation_package_v2_prepare" => Some(Tool::Prepare),
        "fps_presentation_package_v2_production_preflight_get" => {
            Some(Tool::ProductionPreflightGet)
        }
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
    format!("FPS_PRESENTATION_PACKAGE_V2_RUNTIME_METHOD_UNAVAILABLE: {name}")
}
pub fn read_tool_names() -> Vec<String> {
    vec![
        Tool::Get.name().to_owned(),
        Tool::ProductionPreflightGet.name().to_owned(),
    ]
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
            "weapon_materialization_id",
            "weapon_descriptor_sha256",
            "arms_materialization_id",
            "arms_descriptor_sha256",
            "animation_materialization_id",
            "animation_descriptor_sha256",
            "package_policy",
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
                json!({"const":"FpsPresentationPackageV2PrepareRequest@1"}),
            ),
            ("project_id".to_owned(), id()),
            ("weapon_materialization_id".to_owned(), id()),
            ("weapon_descriptor_sha256".to_owned(), sha()),
            ("arms_materialization_id".to_owned(), id()),
            ("arms_descriptor_sha256".to_owned(), sha()),
            ("animation_materialization_id".to_owned(), id()),
            ("animation_descriptor_sha256".to_owned(), sha()),
            ("package_policy".to_owned(), json!({"const":POLICY})),
            ("idempotency_key".to_owned(), id()),
            (
                "max_response_bytes".to_owned(),
                json!({"const":MAX_RESPONSE_BYTES}),
            ),
            ("runtime_write_performed".to_owned(), json!({"const":false})),
            ("writer_policy".to_owned(), json!({"const":WRITER_POLICY})),
            (
                "canonicalization_policy".to_owned(),
                json!({"const":CANONICAL}),
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
            "package_id",
            "runtime_write_performed",
            "persistent_user_data_touched",
            "input_sha256",
        ],
        Map::from_iter([
            (
                "schema_version".to_owned(),
                json!({"const":"FpsPresentationPackageV2GetRequest@1"}),
            ),
            ("project_id".to_owned(), id()),
            ("package_id".to_owned(), id()),
            (
                "package_sha256".to_owned(),
                json!({"type":["string","null"],"pattern":"^[0-9a-f]{64}$"}),
            ),
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
    let (description,input_schema) = match tool {
        Tool::Get => ("Read and restart-reverify one Runtime-owned editable composite FPS package.",get_schema()),
        Tool::Prepare => ("Atomically bind weapon, first-person arms, sockets, rig maps, animation source clips and AuthoringMesh@2 revisions into one structural-only FpsPresentationPackage@2.",prepare_schema()),
        Tool::ProductionPreflightGet => ("Read the exact High-Low-UV-Bake, FPS, engine and independent human-review gates for one composite package without advancing them.",get_schema()),
    };
    json!({"name":tool.name(),"description":description,"inputSchema":input_schema,
      "annotations":{"readOnlyHint":!tool.is_write(),"destructiveHint":false,"idempotentHint":true,"openWorldHint":false,"writeIntent":tool.is_write()},
      "_meta":{"forgecad":{"availability":"available","runtime_method":tool.name(),"requiresConfirmation":false,"transaction":"FpsPresentationPackageV2@1","maxResponseBytes":MAX_RESPONSE_BYTES,"definition_only":false}}})
}
pub fn read_tools() -> Vec<Value> {
    vec![
        definition(Tool::Get),
        definition(Tool::ProductionPreflightGet),
    ]
}
pub fn write_tools() -> Vec<Value> {
    vec![definition(Tool::Prepare)]
}
pub fn summary(name: &str, value: &Value) -> Option<String> {
    let tool = from_name(name)?;
    serde_json::to_string(&json!({
      "schema_version":"FpsPresentationPackageV2McpSummary@1","operation":name,"write_intent":tool.is_write(),
      "project_id":value.get("project_id"),"package_id":value.get("package_id"),
      "package_object_sha256":value.get("package_object_sha256"),"package_sha256":value.get("package_sha256"),
      "status":value.pointer("/package/status"),"quality_status":value.pointer("/package/quality_status"),
      "editable_composite_ready":value.get("editable_composite_ready"),"gates":value.get("gates"),
      "replayed":value.get("replayed"),"restart_hash_verified":value.get("restart_hash_verified"),
      "runtime_write_performed":value.get("runtime_write_performed"),"persistent_user_data_touched":value.get("persistent_user_data_touched"),
      "candidate_created":false,"version_created":false,"export_performed":false,"structured_content_complete":true
    })).ok()
}
