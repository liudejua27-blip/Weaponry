use serde_json::{json, Map, Value};

const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const POLICY: &str = "runtime-derived-package-weapon-authoring-mesh-reviewable-candidate@1";
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const CANONICAL: &str = "canonical-json-sha256-excluding-canonical-sha256@1";

#[derive(Clone, Copy)]
enum Tool {
    Get,
    Prepare,
}
impl Tool {
    fn name(self) -> &'static str {
        match self {
            Self::Get => "fps_presentation_package_v2_candidate_get",
            Self::Prepare => "fps_presentation_package_v2_candidate_prepare",
        }
    }
    fn is_write(self) -> bool {
        matches!(self, Self::Prepare)
    }
}
fn from_name(name: &str) -> Option<Tool> {
    match name {
        "fps_presentation_package_v2_candidate_get" => Some(Tool::Get),
        "fps_presentation_package_v2_candidate_prepare" => Some(Tool::Prepare),
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
    format!("FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_RUNTIME_METHOD_UNAVAILABLE: {name}")
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
            "package_id",
            "package_sha256",
            "policy",
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
                json!({"const":"FpsPresentationPackageV2CandidatePrepareRequest@1"}),
            ),
            ("project_id".to_owned(), id()),
            ("package_id".to_owned(), id()),
            ("package_sha256".to_owned(), sha()),
            ("policy".to_owned(), json!({"const":POLICY})),
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
                json!({"const":"FpsPresentationPackageV2CandidateGetRequest@1"}),
            ),
            ("project_id".to_owned(), id()),
            ("package_id".to_owned(), id()),
            (
                "binding_sha256".to_owned(),
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
        Tool::Get => ("Read and restart-reverify the exact package-to-reviewable-candidate derivation binding.",get_schema()),
        Tool::Prepare => ("Derive one reviewable weapon candidate from the package-owned AuthoringMesh revision. This does not approve Form, create Formal High, confirm, version or export.",prepare_schema()),
    };
    json!({"name":tool.name(),"description":description,"inputSchema":input_schema,
      "annotations":{"readOnlyHint":!tool.is_write(),"destructiveHint":false,"idempotentHint":true,"openWorldHint":false,"writeIntent":tool.is_write()},
      "_meta":{"forgecad":{"availability":"available","runtime_method":tool.name(),"requiresConfirmation":false,"transaction":"FpsPresentationPackageV2Candidate@1","maxResponseBytes":MAX_RESPONSE_BYTES,"definition_only":false}}})
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
      "schema_version":"FpsPresentationPackageV2CandidateMcpSummary@1","operation":name,"write_intent":tool.is_write(),
      "project_id":value.pointer("/binding/project_id"),"package_id":value.pointer("/binding/package_id"),
      "candidate_id":value.pointer("/binding/candidate_id"),"candidate_state":value.pointer("/binding/candidate_state"),
      "candidate_artifact_sha256":value.pointer("/binding/candidate_artifact_sha256"),
      "form_stage":value.pointer("/binding/form_stage"),"secondary_form_approved":value.pointer("/binding/secondary_form_approved"),
      "formal_high_status":value.pointer("/binding/formal_high_status"),"quality_status":value.pointer("/binding/quality_status"),
      "replayed":value.get("replayed"),"restart_hash_verified":value.get("restart_hash_verified"),
      "runtime_write_performed":value.get("runtime_write_performed"),"persistent_user_data_touched":value.get("persistent_user_data_touched"),
      "candidate_created":true,"candidate_confirmed":false,"version_created":false,"export_performed":false,"structured_content_complete":true
    })).ok()
}
