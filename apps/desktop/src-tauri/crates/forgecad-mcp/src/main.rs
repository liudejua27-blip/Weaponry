#[cfg(test)]
use forgecad_runtime::MCP_PROTOCOL_VERSION;
use forgecad_runtime::{
    canonical_json_hash, is_opaque_id, supports_mcp_protocol, LocalIpcClient, LocalIpcEndpoint,
    Runtime, CONTRACT_SET, MCP_PROTOCOL_VERSIONS,
};
use serde_json::{json, Map, Value};
use std::io::{self, BufRead, Write};

const SERVER_NAME: &str = "forgecad";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
const INSTRUCTIONS: &str = "ForgeCAD is a local Codex-only 3D Runtime. Read capabilities and projects first; permanent writes require a prepared candidate and user approval. Long work returns a RuntimeJob. Do not send arbitrary code, URLs, secrets, or unauthorized paths.";

enum Backend {
    InProcess(Runtime),
    AuthenticatedIpc(LocalIpcClient),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionState {
    New,
    Ready,
    Failed,
}

#[derive(Debug, Clone)]
struct Session {
    state: SessionState,
    negotiated_protocol_version: Option<String>,
}

impl Session {
    fn new() -> Self {
        Self {
            state: SessionState::New,
            negotiated_protocol_version: None,
        }
    }
}

fn main() {
    if !valid_arguments() {
        eprintln!("usage: forgecad-mcp [serve --stdio]");
        return;
    }
    let Ok(mut backend) = backend_from_environment() else {
        return;
    };
    let mut session = Session::new();
    let mut stdout = io::BufWriter::new(io::stdout());
    for line in io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(&line) {
            Ok(request) => handle(&mut backend, &mut session, &request),
            Err(error) => error_response(
                Some(Value::Null),
                -32700,
                "Parse error",
                Some(json!({"code":"PARSE_ERROR","detail":safe_error(&error.to_string())})),
            ),
        };
        if let Some(response) = response {
            serde_json::to_writer(&mut stdout, &response).expect("MCP response serializes");
            stdout.write_all(b"\n").expect("MCP response writes");
            stdout.flush().expect("MCP response flushes");
        }
    }
}

fn valid_arguments() -> bool {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    args.is_empty() || args == ["serve".to_owned(), "--stdio".to_owned()]
}

fn backend_from_environment() -> Result<Backend, ()> {
    match (
        std::env::var("FORGECAD_RUNTIME_SOCKET").ok(),
        std::env::var("FORGECAD_RUNTIME_TOKEN").ok(),
    ) {
        (Some(socket), Some(token)) => {
            LocalIpcClient::connect(&LocalIpcEndpoint::from_parts(socket, token))
                .map(Backend::AuthenticatedIpc)
                .map_err(|_| ())
        }
        (None, None) => Runtime::ephemeral().map(Backend::InProcess).map_err(|_| ()),
        _ => Err(()),
    }
}

fn handle(backend: &mut Backend, session: &mut Session, request: &Value) -> Option<Value> {
    let id = request.get("id").cloned();
    if request.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return error_response(
            id,
            -32600,
            "Invalid Request",
            Some(json!({"code":"INVALID_REQUEST"})),
        );
    }
    let Some(method) = request.get("method").and_then(Value::as_str) else {
        return error_response(
            id,
            -32600,
            "Invalid Request",
            Some(json!({"code":"INVALID_REQUEST","detail":"method is required"})),
        );
    };

    match method {
        "initialize" => initialize(backend, session, id, request.get("params")),
        "server/discover" if session.state == SessionState::New => error_response(
            id,
            -32022,
            "Modern MCP protocol is not enabled for this stdio endpoint",
            Some(json!({
                "code":"CONTRACT_VERSION_UNSUPPORTED",
                "supported":MCP_PROTOCOL_VERSIONS,
                "modern_protocol":"2026-07-28",
                "next_action":"Remove CODEX_MCP_PROTOCOL_VERSION=2026-07-28 or use a future ForgeCAD modern adapter."
            })),
        ),
        "notifications/initialized" => {
            if session.state == SessionState::Ready {
                None
            } else {
                None
            }
        }
        "notifications/cancelled" => None,
        "ping" => id.map(|id| json!({"jsonrpc":"2.0","id":id,"result":{}})),
        _ if session.state == SessionState::New => error_response(
            id,
            -32000,
            "Server is not initialized",
            Some(json!({"code":"SERVER_NOT_INITIALIZED"})),
        ),
        _ if session.state == SessionState::Failed => error_response(
            id,
            -32001,
            "Server initialization failed; restart after correcting the contract",
            Some(json!({"code":"CONTRACT_VERSION_UNSUPPORTED"})),
        ),
        "tools/list" => id.map(|id| json!({"jsonrpc":"2.0","id":id,"result":{"tools":tools()}})),
        "resources/list" => resources_list(backend, id),
        "resources/templates/list" => id.map(|id| {
            json!({"jsonrpc":"2.0","id":id,"result":{"resourceTemplates":resource_templates()}})
        }),
        "resources/read" => resources_read(backend, id, request.get("params")),
        "tools/call" => call_tool(backend, id, request.get("params")),
        some_method => error_response(
            id,
            -32601,
            "Method not found",
            Some(json!({"code":"METHOD_NOT_FOUND","method":some_method})),
        ),
    }
}

fn initialize(
    backend: &mut Backend,
    session: &mut Session,
    id: Option<Value>,
    params: Option<&Value>,
) -> Option<Value> {
    let Some(id) = id else {
        session.state = SessionState::Failed;
        return None;
    };
    if session.state != SessionState::New {
        return Some(
            error_response(
                Some(id),
                -32600,
                "Initialize may only be called once",
                Some(json!({"code":"ALREADY_INITIALIZED"})),
            )
            .expect("response for request"),
        );
    }
    let Some(params) = params.and_then(Value::as_object) else {
        session.state = SessionState::Failed;
        return Some(
            error_response(
                Some(id),
                -32602,
                "Invalid initialize params",
                Some(json!({"code":"INVALID_INITIALIZE_PARAMS"})),
            )
            .expect("response for request"),
        );
    };
    let requested = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !supports_mcp_protocol(requested) {
        session.state = SessionState::Failed;
        return Some(
            error_response(
                Some(id),
                -32602,
                "Unsupported protocol version",
                Some(json!({
                    "code":"CONTRACT_VERSION_UNSUPPORTED",
                    "requested":requested,
                    "supported":MCP_PROTOCOL_VERSIONS
                })),
            )
            .expect("response for request"),
        );
    }
    if params
        .get("capabilities")
        .and_then(Value::as_object)
        .is_none()
        || params
            .get("clientInfo")
            .and_then(Value::as_object)
            .is_none()
    {
        session.state = SessionState::Failed;
        return Some(
            error_response(
                Some(id),
                -32602,
                "Initialize requires capabilities and clientInfo",
                Some(json!({"code":"INVALID_INITIALIZE_PARAMS"})),
            )
            .expect("response for request"),
        );
    }
    if let Err(error) = validate_runtime_capabilities(backend) {
        session.state = SessionState::Failed;
        return Some(
            error_response(
                Some(id),
                -32602,
                "Runtime contract is incompatible",
                Some(json!({"code":"CONTRACT_VERSION_UNSUPPORTED","detail":safe_error(&error)})),
            )
            .expect("response for request"),
        );
    }
    session.state = SessionState::Ready;
    session.negotiated_protocol_version = Some(requested.to_owned());
    Some(json!({
        "jsonrpc":"2.0",
        "id":id,
        "result":{
            "protocolVersion":requested,
            "capabilities":{
                "resources":{"listChanged":false,"subscribe":false},
                "tools":{"listChanged":false}
            },
            "serverInfo":{"name":SERVER_NAME,"version":SERVER_VERSION},
            "instructions":INSTRUCTIONS
        }
    }))
}

fn validate_runtime_capabilities(backend: &mut Backend) -> Result<(), String> {
    let value = backend_call(backend, "capabilities_get", &json!({}))?;
    let object = value
        .as_object()
        .ok_or_else(|| "runtime capabilities are not an object".to_owned())?;
    if object.get("contract_set").and_then(Value::as_str) != Some(CONTRACT_SET)
        || object.get("mcp_transport").and_then(Value::as_str) != Some("stdio-json-rpc")
        || object.get("ipc_transport").and_then(Value::as_str) != Some("authenticated-local")
    {
        return Err("runtime contract_set or transport is incompatible".to_owned());
    }
    Ok(())
}

fn capabilities_payload(backend: &mut Backend) -> Result<Value, String> {
    let mut value = backend_call(backend, "capabilities_get", &json!({}))?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| "runtime capabilities are not an object".to_owned())?;
    object.insert(
        "mcp_protocol_versions".to_owned(),
        json!(MCP_PROTOCOL_VERSIONS),
    );
    object.insert(
        "tool_manifest_hash".to_owned(),
        Value::String(tool_manifest_hash()),
    );
    Ok(value)
}

fn tools() -> Vec<Value> {
    let mut tools = vec![
        tool(
            "artifact_readback_get",
            "Read artifact readback metadata when the artifact capability is available",
            json!({"type":"object","required":["artifact_id"],"properties":{"artifact_id":{"type":"string","minLength":1}},"additionalProperties":false}),
            false,
        ),
        tool(
            "candidate_get",
            "Read one prepared candidate",
            json!({"type":"object","required":["candidate_id"],"properties":{"candidate_id":{"type":"string","minLength":1}},"additionalProperties":false}),
            true,
        ),
        tool(
            "capabilities_get",
            "Read live Runtime and MCP capabilities",
            json!({"type":"object","additionalProperties":false}),
            true,
        ),
        tool(
            "job_events_read",
            "Read durable events after a job sequence",
            json!({"type":"object","required":["job_id"],"properties":{"job_id":{"type":"string","minLength":1},"after_sequence":{"type":"integer","minimum":0}},"additionalProperties":false}),
            true,
        ),
        tool(
            "job_get",
            "Read a durable Runtime job receipt",
            json!({"type":"object","required":["job_id"],"properties":{"job_id":{"type":"string","minLength":1}},"additionalProperties":false}),
            true,
        ),
        tool(
            "project_get",
            "Read one Runtime project",
            json!({"type":"object","required":["project_id"],"properties":{"project_id":{"type":"string","minLength":1}},"additionalProperties":false}),
            true,
        ),
        tool(
            "project_list",
            "List Runtime projects",
            json!({"type":"object","additionalProperties":false}),
            true,
        ),
        tool(
            "quality_get",
            "Read a quality report when the Quality Compiler is available",
            json!({"type":"object","required":["candidate_id"],"properties":{"candidate_id":{"type":"string","minLength":1}},"additionalProperties":false}),
            false,
        ),
        tool(
            "selection_get",
            "Read the ephemeral Viewer selection projection",
            json!({"type":"object","additionalProperties":false}),
            true,
        ),
        tool(
            "skill_get",
            "Read a signed Skill bundle manifest when the Skill Registry is available",
            json!({"type":"object","required":["skill_id","version"],"properties":{"skill_id":{"type":"string","minLength":1},"version":{"type":"string","minLength":1}},"additionalProperties":false}),
            false,
        ),
        tool(
            "skill_list",
            "List installed signed Skills when the Skill Registry is available",
            json!({"type":"object","additionalProperties":false}),
            false,
        ),
        tool(
            "snapshot_get",
            "Read an immutable ActiveDesignSnapshot",
            json!({"type":"object","required":["snapshot_id"],"properties":{"snapshot_id":{"type":"string","minLength":1}},"additionalProperties":false}),
            true,
        ),
        tool(
            "version_diff",
            "Compare two immutable versions when version diff is available",
            json!({"type":"object","required":["version_id","compare_to_version_id"],"properties":{"version_id":{"type":"string","minLength":1},"compare_to_version_id":{"type":"string","minLength":1}},"additionalProperties":false}),
            false,
        ),
        tool(
            "version_list",
            "List immutable asset versions",
            json!({"type":"object","properties":{"project_id":{"type":"string","minLength":1}},"additionalProperties":false}),
            true,
        ),
    ];
    tools.sort_by(|left, right| left["name"].as_str().cmp(&right["name"].as_str()));
    tools
}

fn tool(name: &str, description: &str, input_schema: Value, available: bool) -> Value {
    json!({
        "name":name,
        "description":description,
        "inputSchema":input_schema,
        "annotations":{"readOnlyHint":true,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false},
        "_meta":{"forgecad":{"availability":if available {"available"} else {"unavailable"}}}
    })
}

fn resource_templates() -> Vec<Value> {
    [
        (
            "forgecad://projects/{project_id}/snapshot",
            "Project snapshot",
            "Current ActiveDesignSnapshot projection for a project",
        ),
        (
            "forgecad://projects/{project_id}/selection",
            "Viewer selection",
            "Ephemeral selection; never a version truth",
        ),
        (
            "forgecad://candidates/{candidate_id}",
            "Candidate",
            "Prepared candidate and hash-bound state",
        ),
        (
            "forgecad://jobs/{job_id}",
            "Job",
            "Durable job receipt and bounded recent events",
        ),
        (
            "forgecad://versions/{version_id}",
            "Asset version",
            "Immutable asset version and manifest pointers",
        ),
        (
            "forgecad://renders/{render_set_id}/{pass}",
            "Render pass",
            "CAS-backed fixed-view render when the Render Compiler is available",
        ),
        (
            "forgecad://skills/{skill_id}/{version}",
            "Skill bundle",
            "Signed Skill manifest when the Skill Registry is available",
        ),
        (
            "forgecad://artifacts/{artifact_id}",
            "Artifact",
            "Hash-bound artifact metadata when artifact readback is available",
        ),
    ]
    .into_iter()
    .map(|(uri_template, name, description)| {
        json!({"uriTemplate":uri_template,"name":name,"description":description,"mimeType":"application/json"})
    })
    .collect()
}

fn resources_list(backend: &mut Backend, id: Option<Value>) -> Option<Value> {
    let Some(id) = id else { return None };
    match backend_call(backend, "resources_list", &json!({})) {
        Ok(value) => {
            let resources = value
                .as_array()
                .map(|items| items.iter().map(resource_descriptor).collect::<Vec<_>>())
                .unwrap_or_default();
            Some(json!({"jsonrpc":"2.0","id":id,"result":{"resources":resources}}))
        }
        Err(error) => Some(
            error_response(
                Some(id),
                -32002,
                "Runtime resource listing failed",
                Some(json!({"code":"RUNTIME_REQUEST_FAILED","detail":safe_error(&error)})),
            )
            .expect("response for request"),
        ),
    }
}

fn resource_descriptor(value: &Value) -> Value {
    let object = value.as_object().cloned().unwrap_or_default();
    let mut result = Map::new();
    for key in ["uri", "name", "description"] {
        if let Some(value) = object.get(key) {
            result.insert(key.to_owned(), value.clone());
        }
    }
    if let Some(value) = object.get("mime_type") {
        result.insert("mimeType".to_owned(), value.clone());
    }
    result.insert(
        "_meta".to_owned(),
        json!({"forgecad":{"schemaVersion":object.get("schema_version").cloned().unwrap_or(Value::Null),"readOnly":object.get("read_only").cloned().unwrap_or(Value::Bool(true))}}),
    );
    Value::Object(result)
}

fn resources_read(
    backend: &mut Backend,
    id: Option<Value>,
    params: Option<&Value>,
) -> Option<Value> {
    let Some(id) = id else { return None };
    let Some(uri) = params
        .and_then(Value::as_object)
        .and_then(|params| params.get("uri"))
        .and_then(Value::as_str)
    else {
        return Some(
            error_response(
                Some(id),
                -32602,
                "resources/read requires a URI",
                Some(json!({"code":"INVALID_RESOURCE_URI"})),
            )
            .expect("response for request"),
        );
    };
    let result = if uri == "forgecad://capabilities" {
        capabilities_payload(backend).map(|value| {
            json!({
                "schema_version":"RuntimeResourceContents@1",
                "uri":uri,
                "mime_type":"application/json",
                "text":serde_json::to_string(&value).unwrap_or_else(|_| "{}".to_owned())
            })
        })
    } else {
        backend_call(backend, "resource_read", &json!({"uri":uri}))
    };
    match result {
        Ok(value) => {
            let object = value.as_object().cloned().unwrap_or_default();
            let Some(text) = object.get("text").and_then(Value::as_str) else {
                return Some(error_response(
                    Some(id),
                    -32002,
                    "Runtime returned an invalid resource",
                    Some(json!({"code":"RUNTIME_REQUEST_FAILED"})),
                )
                .expect("response for request"));
            };
            if text.len() > 1024 * 1024 {
                return Some(error_response(
                    Some(id),
                    -32003,
                    "Resource exceeds MCP response capacity",
                    Some(json!({"code":"WORKER_BUDGET_EXCEEDED"})),
                )
                .expect("response for request"));
            }
            Some(json!({
                "jsonrpc":"2.0",
                "id":id,
                "result":{"contents":[{"uri":uri,"mimeType":object.get("mime_type").and_then(Value::as_str).unwrap_or("application/json"),"text":text}]}
            }))
        }
        Err(error) => Some(error_response(
            Some(id),
            -32002,
            "Resource is unavailable",
            Some(json!({"code":if error.contains("not found") {"NOT_FOUND"} else {"CAPABILITY_UNAVAILABLE"},"detail":safe_error(&error)})),
        )
        .expect("response for request")),
    }
}

fn call_tool(backend: &mut Backend, id: Option<Value>, params: Option<&Value>) -> Option<Value> {
    let Some(id) = id else { return None };
    let Some(params) = params.and_then(Value::as_object) else {
        return Some(
            error_response(
                Some(id),
                -32602,
                "tools/call requires an object params value",
                Some(json!({"code":"INVALID_TOOL_PARAMS"})),
            )
            .expect("response for request"),
        );
    };
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return Some(
            error_response(
                Some(id),
                -32602,
                "tools/call requires a tool name",
                Some(json!({"code":"INVALID_TOOL_PARAMS"})),
            )
            .expect("response for request"),
        );
    };
    if !tools()
        .iter()
        .any(|tool| tool["name"].as_str() == Some(name))
    {
        return Some(
            error_response(
                Some(id),
                -32602,
                "Unknown tool",
                Some(json!({"code":"METHOD_NOT_FOUND","tool":name})),
            )
            .expect("response for request"),
        );
    }
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if !arguments.is_object() {
        return Some(
            error_response(
                Some(id),
                -32602,
                "Tool arguments must be an object",
                Some(json!({"code":"INVALID_TOOL_PARAMS"})),
            )
            .expect("response for request"),
        );
    }
    match dispatch_tool(backend, name, &arguments) {
        Ok(value) => Some(json!({
            "jsonrpc":"2.0",
            "id":id,
            "result":{"content":[{"type":"text","text":serde_json::to_string(&value).unwrap_or_else(|_| "{}".to_owned())}],"structuredContent":value}
        })),
        Err(error) => Some(json!({
            "jsonrpc":"2.0",
            "id":id,
            "result":{"isError":true,"content":[{"type":"text","text":serde_json::to_string(&runtime_error_value(&error)).unwrap_or_else(|_| "{}".to_owned())}],"structuredContent":runtime_error_value(&error)}
        })),
    }
}

fn dispatch_tool(backend: &mut Backend, name: &str, arguments: &Value) -> Result<Value, String> {
    match name {
        "capabilities_get" => capabilities_payload(backend),
        "version_diff" | "skill_list" | "skill_get" | "quality_get" | "artifact_readback_get" => {
            Err("CAPABILITY_UNAVAILABLE: this MCP003 read model has no quality, Skill, diff, or artifact readback implementation".to_owned())
        }
        _ => backend_call(backend, name, arguments),
    }
}

fn backend_call(backend: &mut Backend, name: &str, arguments: &Value) -> Result<Value, String> {
    match backend {
        Backend::AuthenticatedIpc(client) => client
            .call(name, arguments.clone())
            .map_err(|error| error.to_string()),
        Backend::InProcess(runtime) => dispatch_in_process(runtime, name, arguments),
    }
}

fn dispatch_in_process(runtime: &Runtime, name: &str, arguments: &Value) -> Result<Value, String> {
    match name {
        "capabilities_get" => {
            serde_json::to_value(runtime.capabilities()).map_err(|error| error.to_string())
        }
        "project_list" => {
            serde_json::to_value(runtime.projects().map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())
        }
        "project_get" => {
            let id = required_id(arguments, "project_id")?;
            serde_json::to_value(runtime.project(id).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())
        }
        "snapshot_get" => {
            let id = required_id(arguments, "snapshot_id")?;
            serde_json::to_value(runtime.snapshot(id).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())
        }
        "selection_get" => {
            serde_json::to_value(runtime.selection()).map_err(|error| error.to_string())
        }
        "candidate_get" => {
            let id = required_id(arguments, "candidate_id")?;
            serde_json::to_value(runtime.candidate(id).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())
        }
        "job_get" => {
            let id = required_id(arguments, "job_id")?;
            serde_json::to_value(runtime.job(id).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())
        }
        "job_events_read" => {
            let id = required_id(arguments, "job_id")?;
            let after_sequence = arguments
                .get("after_sequence")
                .and_then(Value::as_i64)
                .unwrap_or(0);
            serde_json::to_value(
                runtime
                    .job_events(id, after_sequence)
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())
        }
        "version_list" => {
            let project_id = arguments.get("project_id").and_then(Value::as_str);
            serde_json::to_value(
                runtime
                    .versions(project_id)
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())
        }
        "resources_list" => serde_json::to_value(
            runtime
                .resource_descriptors()
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string()),
        "resource_read" => {
            let uri = arguments
                .get("uri")
                .and_then(Value::as_str)
                .ok_or_else(|| "uri is required".to_owned())?;
            serde_json::to_value(
                runtime
                    .read_resource(uri)
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())
        }
        _ => Err(format!(
            "CAPABILITY_UNAVAILABLE: unsupported Runtime read method {name}"
        )),
    }
}

fn required_id<'a>(arguments: &'a Value, key: &str) -> Result<&'a str, String> {
    let id = arguments
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !is_opaque_id(id) {
        return Err(format!(
            "INVALID_INPUT: {key} is required and must be an opaque id"
        ));
    }
    Ok(id)
}

fn tool_manifest_hash() -> String {
    canonical_json_hash(&json!({"tools":tools()}))
}

fn runtime_error_value(error: &str) -> Value {
    let (code, message) = error
        .split_once(':')
        .unwrap_or(("RUNTIME_REQUEST_FAILED", error));
    json!({
        "schema_version":"RuntimeError@1",
        "code":code.trim(),
        "message":safe_error(message.trim()),
        "retryable":false,
        "next_action":"Read capabilities_get and correct the request or wait for the required MCP task.",
        "evidence_ids":[]
    })
}

fn safe_error(error: &str) -> String {
    error
        .chars()
        .filter(|character| *character != '\n' && *character != '\r')
        .take(512)
        .collect()
}

fn error_response(
    id: Option<Value>,
    code: i64,
    message: &str,
    data: Option<Value>,
) -> Option<Value> {
    let id = id?;
    let mut error = json!({"code":code,"message":message});
    if let Some(data) = data {
        error["data"] = data;
    }
    Some(json!({"jsonrpc":"2.0","id":id,"error":error}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn initialized() -> (Backend, Session) {
        let mut backend = Backend::InProcess(Runtime::ephemeral().expect("runtime"));
        let mut session = Session::new();
        let response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":MCP_PROTOCOL_VERSION,"capabilities":{},"clientInfo":{"name":"test","version":"1"}}}),
        )
        .expect("initialize response");
        assert_eq!(response["result"]["protocolVersion"], MCP_PROTOCOL_VERSION);
        assert_eq!(session.state, SessionState::Ready);
        (backend, session)
    }

    #[test]
    fn initialize_and_initialized_follow_mcp_lifecycle() {
        let (mut backend, mut session) = initialized();
        assert!(handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","method":"notifications/initialized"})
        )
        .is_none());
        let response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":2,"method":"resources/list"}),
        )
        .expect("resources");
        assert!(response["result"]["resources"].is_array());
    }

    #[test]
    fn codex_legacy_revision_negotiates_without_downgrading_unknown_versions() {
        let mut backend = Backend::InProcess(Runtime::ephemeral().expect("runtime"));
        let mut session = Session::new();
        let response = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":1,
                "method":"initialize",
                "params":{
                    "protocolVersion":"2025-06-18",
                    "capabilities":{},
                    "clientInfo":{"name":"codex-host-probe","version":"0.147.0"}
                }
            }),
        )
        .expect("compatibility initialize response");
        assert_eq!(response["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(
            session.negotiated_protocol_version.as_deref(),
            Some("2025-06-18")
        );
        assert_eq!(session.state, SessionState::Ready);
    }

    #[test]
    fn incompatible_protocol_fails_closed() {
        let mut backend = Backend::InProcess(Runtime::ephemeral().expect("runtime"));
        let mut session = Session::new();
        let response = handle(&mut backend, &mut session, &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"0.0.0","capabilities":{},"clientInfo":{"name":"test","version":"1"}}})).expect("error");
        assert_eq!(
            response["error"]["data"]["code"],
            "CONTRACT_VERSION_UNSUPPORTED"
        );
        assert_eq!(session.state, SessionState::Failed);
    }

    #[test]
    fn modern_discovery_fails_closed_without_legacy_downgrade() {
        let mut backend = Backend::InProcess(Runtime::ephemeral().expect("runtime"));
        let mut session = Session::new();
        let response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":1,"method":"server/discover","params":{}}),
        )
        .expect("modern protocol error");
        assert_eq!(
            response["error"]["data"]["code"],
            "CONTRACT_VERSION_UNSUPPORTED"
        );
        assert_eq!(response["error"]["data"]["modern_protocol"], "2026-07-28");
        assert_eq!(session.state, SessionState::New);
    }

    #[test]
    fn missing_initialize_fields_lock_the_session_closed() {
        let mut backend = Backend::InProcess(Runtime::ephemeral().expect("runtime"));
        let mut session = Session::new();
        let response = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":1,
                "method":"initialize",
                "params":{"protocolVersion":MCP_PROTOCOL_VERSION,"capabilities":{}
            }}),
        )
        .expect("initialize error");
        assert_eq!(
            response["error"]["data"]["code"],
            "INVALID_INITIALIZE_PARAMS"
        );
        assert_eq!(session.state, SessionState::Failed);

        let response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
        )
        .expect("failed session error");
        assert_eq!(
            response["error"]["data"]["code"],
            "CONTRACT_VERSION_UNSUPPORTED"
        );
    }

    #[test]
    fn duplicate_initialize_is_rejected_without_resetting_ready_state() {
        let (mut backend, mut session) = initialized();
        let response = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":2,
                "method":"initialize",
                "params":{"protocolVersion":MCP_PROTOCOL_VERSION,"capabilities":{},"clientInfo":{"name":"test","version":"1"}}
            }),
        )
        .expect("duplicate initialize error");
        assert_eq!(response["error"]["data"]["code"], "ALREADY_INITIALIZED");
        assert_eq!(session.state, SessionState::Ready);
    }

    #[test]
    fn unknown_methods_and_tools_fail_closed() {
        let mut backend = Backend::InProcess(Runtime::ephemeral().expect("runtime"));
        let mut session = Session::new();
        let response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
        )
        .expect("not initialized error");
        assert_eq!(response["error"]["data"]["code"], "SERVER_NOT_INITIALIZED");

        let (mut backend, mut session) = initialized();
        let response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"not_a_tool","arguments":{}}}),
        )
        .expect("unknown tool error");
        assert_eq!(response["error"]["data"]["code"], "METHOD_NOT_FOUND");

        let response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"selection_get","arguments":[]}}),
        )
        .expect("invalid arguments error");
        assert_eq!(response["error"]["data"]["code"], "INVALID_TOOL_PARAMS");
    }

    #[test]
    fn invalid_resource_uris_fail_closed() {
        let (mut backend, mut session) = initialized();
        for uri in [
            "file:///tmp/secret",
            "forgecad://../capabilities",
            "forgecad://capabilities?raw=1",
        ] {
            let response = handle(
                &mut backend,
                &mut session,
                &json!({"jsonrpc":"2.0","id":4,"method":"resources/read","params":{"uri":uri}}),
            )
            .expect("resource error");
            assert_eq!(response["error"]["data"]["code"], "CAPABILITY_UNAVAILABLE");
        }
    }

    #[test]
    fn tools_are_read_only_and_deterministic() {
        let first = tools();
        let second = tools();
        assert_eq!(first, second);
        assert!(first
            .iter()
            .all(|tool| tool["annotations"]["readOnlyHint"] == true));
        assert!(first
            .iter()
            .all(|tool| tool["annotations"]["destructiveHint"] == false));
    }

    #[test]
    fn resource_reads_are_bounded_and_unavailable_capabilities_are_typed() {
        let (mut backend, mut session) = initialized();
        let response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":3,"method":"resources/read","params":{"uri":"forgecad://skills/example/1"}}),
        )
        .expect("resource error");
        assert_eq!(response["error"]["data"]["code"], "CAPABILITY_UNAVAILABLE");

        let response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"selection_get","arguments":{}}}),
        )
        .expect("selection");
        assert_eq!(response["result"]["structuredContent"]["available"], false);
    }
}
