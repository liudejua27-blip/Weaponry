mod supervisor;

#[cfg(test)]
use forgecad_runtime::MCP_PROTOCOL_VERSION;
use forgecad_runtime::{
    build_cohort_sha256, canonical_json_hash, is_opaque_id, supports_mcp_protocol, IpcError,
    LocalIpcClient, LocalIpcEndpoint, Runtime, RuntimeCapabilities, MCP_PROTOCOL_VERSIONS,
};
use serde_json::{json, Map, Value};
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use supervisor::MvpSupervisor;

const SERVER_NAME: &str = "forgecad";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
const INSTRUCTIONS: &str = "ForgeCAD is a local Codex-only 3D Runtime. Read capabilities and projects first; permanent writes require a prepared candidate and user approval. Long work returns a RuntimeJob. Do not send arbitrary code, URLs, secrets, or unauthorized paths.";

enum Backend {
    #[allow(dead_code)]
    InProcess(Runtime),
    AuthenticatedIpc(LocalIpcClient),
    DynamicIpc(DynamicIpcBackend),
    Unavailable(String),
}

struct DynamicIpcBackend {
    ready_file: PathBuf,
    status_file: Option<PathBuf>,
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
    write_tools_enabled: bool,
}

impl Session {
    fn new() -> Self {
        Self {
            state: SessionState::New,
            negotiated_protocol_version: None,
            write_tools_enabled: false,
        }
    }
}

fn main() {
    if std::env::args().skip(1).eq(["--build-identity"]) {
        print_build_identity("forgecad-mcp");
        return;
    }
    if !valid_arguments() {
        eprintln!("usage: forgecad-mcp [serve --stdio | --build-identity]");
        return;
    }
    let (mut backend, mut supervisor) = backend_from_environment();
    let mut session = Session::new();
    let mut stdout = io::BufWriter::new(io::stdout());
    for line in io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        if let Some(runtime_supervisor) = supervisor.as_mut() {
            runtime_supervisor.poll();
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
    drop(supervisor);
}

fn print_build_identity(component: &str) {
    println!(
        "{}",
        serde_json::to_string(&json!({
            "schema_version": "ForgeCADDevBuildIdentity@1",
            "component": component,
            "build_cohort_sha256": build_cohort_sha256()
        }))
        .expect("build identity serializes")
    );
}

fn valid_arguments() -> bool {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    args.is_empty() || args == ["serve".to_owned(), "--stdio".to_owned()]
}

fn backend_from_environment() -> (Backend, Option<MvpSupervisor>) {
    match (
        std::env::var("FORGECAD_RUNTIME_SOCKET").ok(),
        std::env::var("FORGECAD_RUNTIME_TOKEN").ok(),
    ) {
        (Some(socket), Some(token)) => {
            let backend = LocalIpcClient::connect(&LocalIpcEndpoint::from_parts(socket, token))
                .map(Backend::AuthenticatedIpc)
                .unwrap_or_else(|_| {
                    Backend::Unavailable("authenticated Runtime IPC is unavailable".to_owned())
                });
            (backend, None)
        }
        (None, None) => match std::env::var_os("FORGECAD_RUNTIME_READY_FILE") {
            Some(path) if !path.is_empty() => (
                Backend::DynamicIpc(DynamicIpcBackend {
                    ready_file: PathBuf::from(path),
                    status_file: std::env::var_os("FORGECAD_RUNTIME_STATUS_FILE")
                        .map(PathBuf::from),
                }),
                None,
            ),
            _ => match supervisor::runtime_data_root() {
                Ok(data_root) => match MvpSupervisor::new(supervisor::runtime_command(), data_root)
                {
                    Ok(mut runtime_supervisor) => {
                        // Always keep the default path dynamic. A probe client
                        // must be dropped immediately so one MCP adapter never
                        // monopolizes Runtime's sequential request connection.
                        let backend = Backend::DynamicIpc(DynamicIpcBackend {
                            ready_file: runtime_supervisor.ready_file().to_path_buf(),
                            status_file: Some(runtime_supervisor.status_file().to_path_buf()),
                        });
                        runtime_supervisor.start();
                        (backend, Some(runtime_supervisor))
                    }
                    Err(error) => (Backend::Unavailable(error), None),
                },
                Err(error) => (Backend::Unavailable(error), None),
            },
        },
        _ => (
            Backend::Unavailable("Runtime socket and token must be supplied together".to_owned()),
            None,
        ),
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
        "tools/list" => id.map(|id| {
            json!({"jsonrpc":"2.0","id":id,"result":{"tools":tools_with_writes(session.write_tools_enabled)}})
        }),
        "resources/list" => resources_list(backend, id),
        "resources/templates/list" => id.map(|id| {
            json!({"jsonrpc":"2.0","id":id,"result":{"resourceTemplates":resource_templates()}})
        }),
        "resources/read" => resources_read(
            backend,
            id,
            request.get("params"),
            session.write_tools_enabled,
        ),
        "tools/call" => call_tool(
            backend,
            id,
            request.get("params"),
            session.write_tools_enabled,
        ),
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
    // MCP protocol negotiation is deliberately independent from Runtime
    // readiness. Runtime health is observed by runtime_status/doctor and
    // dependent calls return a typed retryable error while degraded.
    session.write_tools_enabled = mcp004_write_opt_in(backend);
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

fn capabilities_payload(backend: &mut Backend, write_tools_enabled: bool) -> Result<Value, String> {
    let value = backend_call(backend, "capabilities_get", &json!({}))?;
    augment_capabilities_payload(value, write_tools_enabled)
}

fn static_capabilities_payload(
    backend: &Backend,
    write_tools_enabled: bool,
) -> Result<Value, String> {
    let mut capabilities = RuntimeCapabilities::default();
    capabilities.status = "runtime-unavailable".to_owned();
    capabilities.limitations.push(
        "Runtime is unavailable; this static capability manifest does not authorize dependent calls or writes."
            .to_owned(),
    );
    let mut value = serde_json::to_value(capabilities).map_err(|error| error.to_string())?;
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "runtime_supervisor_status".to_owned(),
            runtime_status_payload(backend)?,
        );
    }
    augment_capabilities_payload(value, write_tools_enabled)
}

fn augment_capabilities_payload(
    mut value: Value,
    write_tools_enabled: bool,
) -> Result<Value, String> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| "runtime capabilities are not an object".to_owned())?;
    let runtime_build_cohort = object
        .get("build_cohort_sha256")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let mcp_build_cohort = build_cohort_sha256();
    object.insert(
        "mcp_build_cohort_sha256".to_owned(),
        mcp_build_cohort
            .as_ref()
            .map_or(Value::Null, |value| Value::String(value.clone())),
    );
    object.insert(
        "build_cohort_match".to_owned(),
        Value::Bool(
            runtime_build_cohort.is_some()
                && runtime_build_cohort.as_ref() == mcp_build_cohort.as_ref(),
        ),
    );
    object.insert(
        "mcp_protocol_versions".to_owned(),
        json!(MCP_PROTOCOL_VERSIONS),
    );
    object.insert(
        "tool_manifest_hash".to_owned(),
        Value::String(tool_manifest_hash(write_tools_enabled)),
    );
    object.insert(
        "mcp_write_tools_enabled".to_owned(),
        Value::Bool(write_tools_enabled),
    );
    object.insert(
        "mcp_write_tool_names".to_owned(),
        if write_tools_enabled {
            Value::Array(
                all_write_tool_names()
                    .into_iter()
                    .map(Value::String)
                    .collect(),
            )
        } else {
            Value::Array(Vec::new())
        },
    );
    Ok(value)
}

fn mcp004_write_opt_in(backend: &Backend) -> bool {
    matches!(
        backend,
        Backend::AuthenticatedIpc(_) | Backend::DynamicIpc(_)
    ) && std::env::var("FORGECAD_MCP_ENABLE_MCP004_WRITES").as_deref() == Ok("1")
}

fn mcp004_write_tool_names() -> Vec<String> {
    [
        "project_create",
        "candidate_prepare",
        "candidate_confirm",
        "candidate_reject",
        "restore_prepare",
        "restore_confirm",
        "export_prepare",
        "export_confirm",
        "job_cancel",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn mcp005_write_tool_names() -> Vec<String> {
    vec!["reference_import".to_owned()]
}

fn mcp007_write_tool_names() -> Vec<String> {
    vec!["geometry_prepare".to_owned()]
}

fn mcp008_write_tool_names() -> Vec<String> {
    vec!["appearance_prepare".to_owned()]
}

fn mcp009_write_tool_names() -> Vec<String> {
    vec!["change_prepare".to_owned()]
}

fn is_mcp004_write_tool(name: &str) -> bool {
    mcp004_write_tool_names().iter().any(|tool| tool == name)
}

fn is_mcp005_write_tool(name: &str) -> bool {
    mcp005_write_tool_names().iter().any(|tool| tool == name)
}

fn is_mcp007_write_tool(name: &str) -> bool {
    mcp007_write_tool_names().iter().any(|tool| tool == name)
}

fn is_mcp008_write_tool(name: &str) -> bool {
    mcp008_write_tool_names().iter().any(|tool| tool == name)
}

fn is_mcp009_write_tool(name: &str) -> bool {
    mcp009_write_tool_names().iter().any(|tool| tool == name)
}

fn is_write_tool(name: &str) -> bool {
    is_mcp004_write_tool(name)
        || is_mcp005_write_tool(name)
        || is_mcp007_write_tool(name)
        || is_mcp008_write_tool(name)
        || is_mcp009_write_tool(name)
}

fn all_write_tool_names() -> Vec<String> {
    let mut names = mcp004_write_tool_names();
    names.extend(mcp005_write_tool_names());
    names.extend(mcp007_write_tool_names());
    names.extend(mcp008_write_tool_names());
    names.extend(mcp009_write_tool_names());
    names
}

fn tools_with_writes(writes_enabled: bool) -> Vec<Value> {
    let mut tools = read_only_tools();
    if writes_enabled {
        tools.extend(mcp004_write_tools());
        tools.extend(mcp005_write_tools());
        tools.extend(mcp007_write_tools());
        tools.extend(mcp008_write_tools());
        tools.extend(mcp009_write_tools());
    }
    tools.sort_by(|left, right| left["name"].as_str().cmp(&right["name"].as_str()));
    tools
}

fn read_only_tools() -> Vec<Value> {
    vec![
        tool(
            "artifact_readback_get",
            "Read strict hash-bound GLB metadata for a geometry candidate",
            json!({"type":"object","required":["artifact_id","candidate_id"],"properties":{"artifact_id":{"type":"string","minLength":1},"candidate_id":{"type":"string","minLength":1}},"additionalProperties":false}),
            true,
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
            "doctor",
            "Read bounded MCP and Runtime health diagnostics without changing project state",
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
            "reference_get",
            "Read one hash-bound reference image evidence record without returning its original path or bytes",
            json!({"type":"object","required":["reference_id"],"properties":{"reference_id":id_property()},"additionalProperties":false}),
            true,
        ),
        tool(
            "quality_get",
            "Read the Runtime-owned quality report; optionally include a bounded reference aspect comparison",
            json!({"type":"object","required":["candidate_id"],"properties":{"candidate_id":{"type":"string","minLength":1},"reference_id":{"type":"string","minLength":1}},"additionalProperties":false}),
            true,
        ),
        tool(
            "selection_get",
            "Read the ephemeral Viewer selection projection",
            json!({"type":"object","additionalProperties":false}),
            true,
        ),
        tool(
            "runtime_status",
            "Read the Runtime supervisor state and retryability",
            json!({"type":"object","additionalProperties":false}),
            true,
        ),
        tool(
            "skill_get",
            "Read a first-party development Skill bundle manifest when the Skill Registry is available",
            json!({"type":"object","required":["skill_id","version"],"properties":{"skill_id":{"type":"string","minLength":1},"version":{"type":"string","minLength":1}},"additionalProperties":false}),
            true,
        ),
        tool(
            "skill_list",
            "List first-party development Skill manifests when the Skill Registry is available",
            json!({"type":"object","additionalProperties":false}),
            true,
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
            true,
        ),
        tool(
            "version_list",
            "List immutable asset versions",
            json!({"type":"object","properties":{"project_id":{"type":"string","minLength":1}},"additionalProperties":false}),
            true,
        ),
    ]
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

fn write_tool(
    name: &str,
    description: &str,
    input_schema: Value,
    destructive: bool,
    idempotent: bool,
) -> Value {
    write_tool_with_transaction(
        name,
        description,
        input_schema,
        destructive,
        idempotent,
        "MCP004",
    )
}

fn write_tool_with_transaction(
    name: &str,
    description: &str,
    input_schema: Value,
    destructive: bool,
    idempotent: bool,
    transaction: &str,
) -> Value {
    json!({
        "name":name,
        "description":description,
        "inputSchema":input_schema,
        "annotations":{"readOnlyHint":false,"destructiveHint":destructive,"idempotentHint":idempotent,"openWorldHint":false},
        "_meta":{"forgecad":{"availability":"available","requiresConfirmation":true,"transaction":transaction}}
    })
}

fn id_property() -> Value {
    json!({"type":"string","minLength":1,"maxLength":128})
}

fn nullable_id_property() -> Value {
    json!({"type":["string","null"],"maxLength":128})
}

fn sha256_property() -> Value {
    json!({"type":"string","pattern":"^[0-9a-f]{64}$"})
}

fn request_property() -> Value {
    json!({"type":"object"})
}

fn mcp004_write_tools() -> Vec<Value> {
    vec![
        write_tool(
            "project_create",
            "Create a local ForgeCAD project. This only writes project metadata; model generation still requires a later typed candidate prepare.",
            json!({
                "type":"object",
                "required":["name"],
                "properties":{
                    "name":{"type":"string","minLength":1,"maxLength":200},
                    "policy":{"type":"object"}
                },
                "additionalProperties":false
            }),
            false,
            false,
        ),
        write_tool(
            "candidate_prepare",
            "Prepare a hash-bound candidate and durable Job; it accepts an existing CAS object or the bounded non-visual typed=diagnostic MVP path, and does not create a permanent version.",
            json!({
                "type":"object",
                "required":["project_id"],
                "properties":{
                    "project_id":id_property(),
                    "base_version_id":nullable_id_property(),
                    "prepared_object_id":id_property(),
                    "prepared_object_sha256":sha256_property(),
                    "request":{
                        "type":"object",
                        "properties":{
                            "typed":{"const":"diagnostic"},
                            "label":{"type":"string","minLength":1,"maxLength":128}
                        },
                        "additionalProperties":false
                    }
                },
                "additionalProperties":false,
                "oneOf":[
                    {"required":["prepared_object_id","prepared_object_sha256"]},
                    {"required":["request"],"properties":{"request":{"required":["typed"]}}}
                ]
            }),
            false,
            false,
        ),
        write_tool(
            "candidate_confirm",
            "Confirm a prepared candidate after user approval; Runtime mints the durable receipt and creates at most one immutable version by idempotency key.",
            json!({
                "type":"object",
                "required":["project_id","candidate_id","prepared_object_id","prepared_object_sha256","quality_report_id","approval_receipt_id","approval_summary","approval_session_id","approval_expires_at","idempotency_key"],
                "properties":{
                    "project_id":id_property(),
                    "candidate_id":id_property(),
                    "base_version_id":nullable_id_property(),
                    "prepared_object_id":id_property(),
                    "prepared_object_sha256":sha256_property(),
                    "quality_report_id":id_property(),
                    "approval_receipt_id":id_property(),
                    "approval_summary":{"type":"string","minLength":1,"maxLength":512},
                    "approval_session_id":id_property(),
                    "approval_expires_at":{"type":"string","minLength":1,"maxLength":64},
                    "idempotency_key":id_property()
                },
                "additionalProperties":false
            }),
            true,
            true,
        ),
        write_tool(
            "candidate_reject",
            "Reject a prepared candidate after user approval; Runtime mints the durable receipt without moving the confirmed head.",
            json!({
                "type":"object",
                "required":["project_id","candidate_id","approval_receipt_id","approval_summary","approval_session_id","approval_expires_at","idempotency_key"],
                "properties":{
                    "project_id":id_property(),
                    "candidate_id":id_property(),
                    "approval_receipt_id":id_property(),
                    "approval_summary":{"type":"string","minLength":1,"maxLength":512},
                    "approval_session_id":id_property(),
                    "approval_expires_at":{"type":"string","minLength":1,"maxLength":64},
                    "idempotency_key":id_property()
                },
                "additionalProperties":false
            }),
            true,
            true,
        ),
        write_tool(
            "restore_prepare",
            "Prepare a restore from a project-local confirmed historical version; confirmation creates a new child version.",
            json!({
                "type":"object",
                "required":["project_id","source_version_id","request"],
                "properties":{
                    "project_id":id_property(),
                    "base_version_id":nullable_id_property(),
                    "source_version_id":id_property(),
                    "request":request_property()
                },
                "additionalProperties":false
            }),
            false,
            false,
        ),
        write_tool(
            "restore_confirm",
            "Confirm an approved restore; Runtime mints the durable receipt, creates a new immutable child and never rewrites historical version pointers.",
            json!({
                "type":"object",
                "required":["project_id","candidate_id","source_version_id","prepared_object_id","prepared_object_sha256","quality_report_id","approval_receipt_id","approval_summary","approval_session_id","approval_expires_at","idempotency_key"],
                "properties":{
                    "project_id":id_property(),
                    "candidate_id":id_property(),
                    "source_version_id":id_property(),
                    "base_version_id":nullable_id_property(),
                    "prepared_object_id":id_property(),
                    "prepared_object_sha256":sha256_property(),
                    "quality_report_id":id_property(),
                    "approval_receipt_id":id_property(),
                    "approval_summary":{"type":"string","minLength":1,"maxLength":512},
                    "approval_session_id":id_property(),
                    "approval_expires_at":{"type":"string","minLength":1,"maxLength":64},
                    "idempotency_key":id_property()
                },
                "additionalProperties":false
            }),
            true,
            true,
        ),
        write_tool(
            "export_prepare",
            "Prepare a path-free diagnostic manifest or an approved-for-MVP GLB artifact export; it never creates a filesystem path.",
            json!({
                "type":"object",
                "required":["project_id","version_id","format","profile","request"],
                "properties":{
                    "project_id":id_property(),
                    "version_id":id_property(),
                    "format":{"enum":["manifest-json","glb"]},
                    "profile":{"enum":["diagnostic","mvp-glb"]},
                    "request":request_property()
                },
                "additionalProperties":false
            }),
            false,
            false,
        ),
        write_tool(
            "export_confirm",
            "Confirm an approved path-free diagnostic or MVP GLB export; Runtime mints the durable receipt with version, artifact and idempotency binding.",
            json!({
                "type":"object",
                "required":["project_id","export_id","version_id","format","profile","approval_receipt_id","approval_summary","approval_session_id","approval_expires_at","idempotency_key"],
                "properties":{
                    "project_id":id_property(),
                    "export_id":id_property(),
                    "version_id":id_property(),
                    "format":{"enum":["manifest-json","glb"]},
                    "profile":{"enum":["diagnostic","mvp-glb"]},
                    "approval_receipt_id":id_property(),
                    "approval_summary":{"type":"string","minLength":1,"maxLength":512},
                    "approval_session_id":id_property(),
                    "approval_expires_at":{"type":"string","minLength":1,"maxLength":64},
                    "idempotency_key":id_property()
                },
                "additionalProperties":false
            }),
            true,
            true,
        ),
        write_tool(
            "job_cancel",
            "Cancel a non-terminal Runtime Job; the durable terminal event is the only cancellation truth.",
            json!({
                "type":"object",
                "required":["job_id"],
                "properties":{"job_id":id_property()},
                "additionalProperties":false
            }),
            true,
            true,
        ),
    ]
}

fn mcp005_write_tools() -> Vec<Value> {
    vec![write_tool_with_transaction(
        "reference_import",
        "Import a user-authorized PNG/JPEG reference into Runtime CAS and create ReferenceEvidence; the original path is never persisted and no geometry is generated.",
        json!({
            "type":"object",
            "required":["project_id","source","authorization"],
            "properties":{
                "project_id":id_property(),
                "source":{
                    "oneOf":[
                        {"type":"object","required":["kind","mime","content_base64"],"properties":{"kind":{"const":"inline_content"},"mime":{"enum":["image/png","image/jpeg"]},"content_base64":{"type":"string","minLength":1,"maxLength":12582912}},"additionalProperties":false},
                        {"type":"object","required":["kind","path"],"properties":{"kind":{"const":"codex_local_file"},"path":{"type":"string","minLength":1,"maxLength":4096}},"additionalProperties":false}
                    ]
                },
                "authorization":{"type":"object","required":["user_authorized","declaration"],"properties":{"user_authorized":{"const":true},"declaration":{"type":"string","minLength":1,"maxLength":512}},"additionalProperties":false},
                "expected_sha256":{"type":["string","null"],"pattern":"^[0-9a-f]{64}$"}
            },
            "additionalProperties":false
        }),
        false,
        false,
        "MCP005",
    )]
}

fn mcp007_write_tools() -> Vec<Value> {
    vec![write_tool_with_transaction(
        "geometry_prepare",
        "Compile a bounded typed GeometryProgram into a real multi-part GLB candidate and return strict ArtifactReadback; no permanent version is created until a later approval confirm.",
        json!({
            "type":"object",
            "required":["project_id","request"],
            "properties":{
                "project_id":id_property(),
                "base_version_id":nullable_id_property(),
                "request":{
                    "type":"object",
                    "required":["typed","geometry_program"],
                    "properties":{"typed":{"const":"geometry"},"reference_id":id_property(),"geometry_program":{"type":"object"}},
                    "additionalProperties":false
                }
            },
            "additionalProperties":false
        }),
        false,
        false,
        "MCP007",
    )]
}

fn mcp008_write_tools() -> Vec<Value> {
    vec![write_tool_with_transaction(
        "appearance_prepare",
        "Compile a hash-bound AppearanceProgram with bounded UV/tangent/PBR lowering and fixed beauty, silhouette, normal and part-id render evidence; no permanent version is created until approval.",
        json!({
            "type":"object",
            "required":["project_id","request"],
            "properties":{
                "project_id":id_property(),
                "base_version_id":nullable_id_property(),
                "request":{
                    "type":"object",
                    "required":["typed","geometry_program","appearance_program"],
                    "properties":{"typed":{"const":"appearance"},"reference_id":id_property(),"geometry_program":{"type":"object"},"appearance_program":{"type":"object"}},
                    "additionalProperties":false
                }
            },
            "additionalProperties":false
        }),
        false,
        false,
        "MCP008",
    )]
}

fn mcp009_write_tools() -> Vec<Value> {
    vec![write_tool_with_transaction(
        "change_prepare",
        "Prepare one stable-Part local edit from an explicit typed GeometryProgram and AppearanceProgram; Runtime validates the base version and returns the same bounded GLB/render evidence without creating a permanent version.",
        json!({
            "type":"object",
            "required":["project_id","base_version_id","request"],
            "properties":{
                "project_id":id_property(),
                "base_version_id":id_property(),
                "request":{
                    "type":"object",
                    "required":["typed","change_set","geometry_program","appearance_program"],
                    "properties":{
                        "typed":{"const":"change"},
                        "reference_id":id_property(),
                        "change_set":{
                            "type":"object",
                            "required":["part_id","operation","parameters"],
                            "properties":{
                                "part_id":id_property(),
                                "operation":{"enum":["transform","material_update","replace_geometry"]},
                                "parameters":{"type":"object","maxProperties":16},
                                "reason":{"type":"string","maxLength":512}
                            },
                            "additionalProperties":false
                        },
                        "geometry_program":{"type":"object"},
                        "appearance_program":{"type":"object"}
                    },
                    "additionalProperties":false
                }
            },
            "additionalProperties":false
        }),
        false,
        false,
        "MCP009",
    )]
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
            "forgecad://references/{reference_id}",
            "Reference evidence",
            "Hash-bound reference image metadata without the original path or bytes",
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
        Err(error) if error.starts_with("RUNTIME_UNAVAILABLE") => Some(json!({
            "jsonrpc":"2.0",
            "id":id,
            "result":{"resources":static_resource_descriptors()}
        })),
        Err(error) => Some(
            error_response(
                Some(id),
                -32002,
                "Runtime resource listing failed",
                Some(json!({"code":if error.starts_with("RUNTIME_UNAVAILABLE") {"RUNTIME_UNAVAILABLE"} else {"RUNTIME_REQUEST_FAILED"},"retryable":error.starts_with("RUNTIME_UNAVAILABLE"),"detail":safe_error(&error)})),
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
    write_tools_enabled: bool,
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
        capabilities_payload(backend, write_tools_enabled)
            .or_else(|error| {
                if error.starts_with("RUNTIME_UNAVAILABLE") {
                    static_capabilities_payload(backend, write_tools_enabled)
                } else {
                    Err(error)
                }
            })
            .map(|value| {
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
            Some(json!({"code":if error.starts_with("RUNTIME_UNAVAILABLE") {"RUNTIME_UNAVAILABLE"} else if error.contains("not found") {"NOT_FOUND"} else {"CAPABILITY_UNAVAILABLE"},"retryable":error.starts_with("RUNTIME_UNAVAILABLE"),"detail":safe_error(&error)})),
        )
        .expect("response for request")),
    }
}

fn static_resource_descriptors() -> Vec<Value> {
    vec![json!({
        "uri":"forgecad://capabilities",
        "name":"Runtime capabilities",
        "description":"Static MCP and Runtime health capability manifest",
        "mime_type":"application/json",
        "schema_version":"RuntimeResource@1",
        "read_only":true
    })]
}

fn call_tool(
    backend: &mut Backend,
    id: Option<Value>,
    params: Option<&Value>,
    write_tools_enabled: bool,
) -> Option<Value> {
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
    if !tools_with_writes(write_tools_enabled)
        .iter()
        .any(|tool| tool["name"].as_str() == Some(name))
    {
        if is_mcp004_write_tool(name) {
            return Some(json!({
                "jsonrpc":"2.0",
                "id":id,
                "result":{
                    "isError":true,
                    "content":[{"type":"text","text":serde_json::to_string(&runtime_error_value("MCP004_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")).unwrap_or_else(|_| "{}".to_owned())}],
                    "structuredContent":runtime_error_value("MCP004_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")
                }
            }));
        }
        if is_mcp005_write_tool(name) {
            return Some(json!({
                "jsonrpc":"2.0",
                "id":id,
                "result":{
                    "isError":true,
                    "content":[{"type":"text","text":serde_json::to_string(&runtime_error_value("MCP005_REFERENCE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")).unwrap_or_else(|_| "{}".to_owned())}],
                    "structuredContent":runtime_error_value("MCP005_REFERENCE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")
                }
            }));
        }
        if is_mcp007_write_tool(name) {
            return Some(json!({
                "jsonrpc":"2.0",
                "id":id,
                "result":{
                    "isError":true,
                    "content":[{"type":"text","text":serde_json::to_string(&runtime_error_value("MCP007_GEOMETRY_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")).unwrap_or_else(|_| "{}".to_owned())}],
                    "structuredContent":runtime_error_value("MCP007_GEOMETRY_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")
                }
            }));
        }
        if is_mcp008_write_tool(name) {
            return Some(json!({
                "jsonrpc":"2.0",
                "id":id,
                "result":{
                    "isError":true,
                    "content":[{"type":"text","text":serde_json::to_string(&runtime_error_value("MCP008_APPEARANCE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")).unwrap_or_else(|_| "{}".to_owned())}],
                    "structuredContent":runtime_error_value("MCP008_APPEARANCE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")
                }
            }));
        }
        if is_mcp009_write_tool(name) {
            return Some(json!({
                "jsonrpc":"2.0",
                "id":id,
                "result":{
                    "isError":true,
                    "content":[{"type":"text","text":serde_json::to_string(&runtime_error_value("MCP009_CHANGE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")).unwrap_or_else(|_| "{}".to_owned())}],
                    "structuredContent":runtime_error_value("MCP009_CHANGE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")
                }
            }));
        }
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
    match dispatch_tool(backend, name, &arguments, write_tools_enabled) {
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

fn dispatch_tool(
    backend: &mut Backend,
    name: &str,
    arguments: &Value,
    write_tools_enabled: bool,
) -> Result<Value, String> {
    let local_build_cohort = build_cohort_sha256();
    dispatch_tool_with_build_cohort(
        backend,
        name,
        arguments,
        write_tools_enabled,
        local_build_cohort.as_deref(),
    )
}

fn dispatch_tool_with_build_cohort(
    backend: &mut Backend,
    name: &str,
    arguments: &Value,
    write_tools_enabled: bool,
    local_build_cohort: Option<&str>,
) -> Result<Value, String> {
    if is_write_tool(name) && !write_tools_enabled {
        return Err(if is_mcp005_write_tool(name) {
            "MCP005_REFERENCE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required"
                .to_owned()
        } else if is_mcp007_write_tool(name) {
            "MCP007_GEOMETRY_TOOLS_DISABLED: explicit authenticated IPC opt-in is required"
                .to_owned()
        } else if is_mcp008_write_tool(name) {
            "MCP008_APPEARANCE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required"
                .to_owned()
        } else if is_mcp009_write_tool(name) {
            "MCP009_CHANGE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required".to_owned()
        } else {
            "MCP004_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required".to_owned()
        });
    }
    if is_write_tool(name) {
        return backend_write_call(backend, name, arguments, local_build_cohort);
    }
    match name {
        "capabilities_get" => capabilities_payload(backend, write_tools_enabled),
        "runtime_status" => runtime_status_payload(backend),
        "doctor" => doctor_payload(backend),
        "version_diff" | "quality_get" => backend_call(backend, name, arguments),
        _ => backend_call(backend, name, arguments),
    }
}

fn backend_write_call(
    backend: &mut Backend,
    name: &str,
    arguments: &Value,
    local_build_cohort: Option<&str>,
) -> Result<Value, String> {
    let Some(local_build_cohort) = local_build_cohort else {
        // Ordinary and test builds intentionally omit a cohort and retain the
        // existing source-development behavior.
        return backend_call(backend, name, arguments);
    };
    match backend {
        Backend::AuthenticatedIpc(client) => {
            let runtime_capabilities = client
                .call("capabilities_get", json!({}))
                .map_err(map_ipc_error)?;
            require_matching_build_cohort(Some(local_build_cohort), &runtime_capabilities)?;
            client.call(name, arguments.clone()).map_err(map_ipc_error)
        }
        Backend::DynamicIpc(dynamic) => {
            let endpoint = read_ready_endpoint(&dynamic.ready_file)?;
            let mut client = LocalIpcClient::connect(&endpoint).map_err(map_ipc_error)?;
            let runtime_capabilities = client
                .call("capabilities_get", json!({}))
                .map_err(map_ipc_error)?;
            require_matching_build_cohort(Some(local_build_cohort), &runtime_capabilities)?;
            client.call(name, arguments.clone()).map_err(map_ipc_error)
        }
        Backend::InProcess(runtime) => {
            let runtime_capabilities =
                serde_json::to_value(runtime.capabilities()).map_err(|error| error.to_string())?;
            require_matching_build_cohort(Some(local_build_cohort), &runtime_capabilities)?;
            dispatch_in_process(runtime, name, arguments)
        }
        Backend::Unavailable(detail) => Err(format!("RUNTIME_UNAVAILABLE: {detail}")),
    }
}

fn require_matching_build_cohort(
    local_build_cohort: Option<&str>,
    runtime_capabilities: &Value,
) -> Result<(), String> {
    let Some(local_build_cohort) = local_build_cohort else {
        return Ok(());
    };
    let runtime_build_cohort = runtime_capabilities
        .get("build_cohort_sha256")
        .and_then(Value::as_str);
    if runtime_build_cohort == Some(local_build_cohort) {
        Ok(())
    } else {
        Err(
            "BUILD_COHORT_MISMATCH: Runtime and MCP development builds must match before writes"
                .to_owned(),
        )
    }
}

fn backend_call(backend: &mut Backend, name: &str, arguments: &Value) -> Result<Value, String> {
    match backend {
        Backend::AuthenticatedIpc(client) => {
            client.call(name, arguments.clone()).map_err(map_ipc_error)
        }
        Backend::DynamicIpc(dynamic) => dynamic.call(name, arguments),
        Backend::Unavailable(detail) => Err(format!("RUNTIME_UNAVAILABLE: {detail}")),
        Backend::InProcess(runtime) => dispatch_in_process(runtime, name, arguments),
    }
}

/// Preserve the distinction between an unavailable Runtime and a request that
/// the available Runtime rejected. The IPC server returns a typed error code;
/// details are intentionally not forwarded because they may contain user
/// input or local paths. Transport failures remain retryable.
fn map_ipc_error(error: IpcError) -> String {
    match error {
        IpcError::RuntimeRequest(detail) => {
            let code = detail.split(':').next().unwrap_or(detail.as_str()).trim();
            match code {
                "INVALID_INPUT" => "INVALID_INPUT: Runtime request rejected".to_owned(),
                "STORE_ERROR" => "STORE_ERROR: Runtime store rejected the request".to_owned(),
                "RUNTIME_BUSY" => "RUNTIME_BUSY: Runtime writer is busy".to_owned(),
                "IPC_ERROR" => "IPC_ERROR: Runtime IPC request failed".to_owned(),
                _ => "RUNTIME_UNAVAILABLE: Runtime request failed".to_owned(),
            }
        }
        IpcError::AuthenticationFailed => {
            "RUNTIME_UNAVAILABLE: Runtime IPC authentication failed".to_owned()
        }
        IpcError::Io(_) | IpcError::Protocol | IpcError::UnsupportedPlatform => {
            "RUNTIME_UNAVAILABLE: Runtime IPC is unavailable".to_owned()
        }
        IpcError::ShutdownRequested => "RUNTIME_UNAVAILABLE: Runtime is shutting down".to_owned(),
    }
}

fn runtime_status_payload(backend: &Backend) -> Result<Value, String> {
    let value = match backend {
        Backend::InProcess(_) => json!({
            "schema_version":"ForgeCADRuntimeSupervisorStatus@1",
            "state":"Ready",
            "retryable":false,
            "source":"in_process_test_backend"
        }),
        Backend::AuthenticatedIpc(_) => json!({
            "schema_version":"ForgeCADRuntimeSupervisorStatus@1",
            "state":"Ready",
            "retryable":false,
            "source":"authenticated_ipc"
        }),
        Backend::DynamicIpc(dynamic) => dynamic.status(),
        Backend::Unavailable(detail) => json!({
            "schema_version":"ForgeCADRuntimeSupervisorStatus@1",
            "state":"Degraded",
            "retryable":true,
            "source":"mcp_adapter",
            "detail":safe_error(detail)
        }),
    };
    Ok(value)
}

fn doctor_payload(backend: &Backend) -> Result<Value, String> {
    let runtime = runtime_status_payload(backend)?;
    let state = runtime
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("Degraded");
    let healthy = state == "Ready";
    let busy = state == "Busy";
    Ok(json!({
        "schema_version":"ForgeCADRuntimeDoctor@1",
        "state":state,
        "retryable":!healthy,
        "checks":{
            "mcp_protocol":"pass",
            "stdio_lifecycle":"pass",
            "runtime_supervisor":if healthy {"ready"} else if busy {"busy"} else {"degraded"},
            "runtime_endpoint":if healthy {"ready"} else if busy {"reachable_busy"} else {"unavailable"}
        },
        "runtime":runtime,
        "scope":"diagnostic status only; no fixture, confirmation, signing, or export was run"
    }))
}

impl DynamicIpcBackend {
    fn call(&self, name: &str, arguments: &Value) -> Result<Value, String> {
        let endpoint = read_ready_endpoint(&self.ready_file)?;
        let mut client = LocalIpcClient::connect(&endpoint).map_err(map_ipc_error)?;
        client.call(name, arguments.clone()).map_err(map_ipc_error)
    }

    fn status(&self) -> Value {
        let ready_probe = probe_dynamic_ready_endpoint(&self.ready_file);
        match ready_probe {
            DynamicReadyProbe::Authenticated => {
                return json!({
                    "schema_version":"ForgeCADRuntimeSupervisorStatus@1",
                    "state":"Ready",
                    "retryable":false,
                    "source":"authenticated_ready_handoff"
                })
            }
            DynamicReadyProbe::Busy => {
                return json!({
                    "schema_version":"ForgeCADRuntimeSupervisorStatus@1",
                    "state":"Busy",
                    "retryable":true,
                    "source":"authenticated_probe_timeout",
                    "code":"RUNTIME_BUSY",
                    "listener_reachable":true,
                    "detail":"Runtime endpoint is reachable but its bounded authentication probe timed out"
                })
            }
            DynamicReadyProbe::Unavailable => {}
        }
        if let Some(path) = &self.status_file {
            if let Ok(mut value) = read_bounded_json(path) {
                let state = value.get("state").and_then(Value::as_str);
                if state.is_some() && state != Some("Ready") {
                    return value;
                }
                if state == Some("Ready") {
                    value["state"] = Value::String("Degraded".to_owned());
                    value["retryable"] = Value::Bool(true);
                    value["code"] = Value::String("RUNTIME_HANDOFF_STALE".to_owned());
                    value["source"] = Value::String("authenticated_probe_failed".to_owned());
                    value["listener_reachable"] = Value::Bool(false);
                    return value;
                }
            }
        }
        json!({
            "schema_version":"ForgeCADRuntimeSupervisorStatus@1",
            "state":"Degraded",
            "retryable":true,
            "source":"mcp_adapter",
            "code":"RUNTIME_HANDOFF_STALE",
            "listener_reachable":false,
            "detail":"Runtime ready handoff did not pass authenticated endpoint probe"
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DynamicReadyProbe {
    Authenticated,
    Busy,
    Unavailable,
}

fn probe_dynamic_ready_endpoint(path: &std::path::Path) -> DynamicReadyProbe {
    let Ok(endpoint) = read_ready_endpoint(path) else {
        return DynamicReadyProbe::Unavailable;
    };
    match LocalIpcClient::connect(&endpoint) {
        Ok(_) => DynamicReadyProbe::Authenticated,
        Err(IpcError::Io(error))
            if matches!(
                error.kind(),
                std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
            ) && endpoint.listener_reachable() =>
        {
            DynamicReadyProbe::Busy
        }
        Err(_) => DynamicReadyProbe::Unavailable,
    }
}

fn read_ready_endpoint(path: &std::path::Path) -> Result<LocalIpcEndpoint, String> {
    let value = read_bounded_json(path)
        .map_err(|_| "RUNTIME_UNAVAILABLE: Runtime ready handoff is unavailable".to_owned())?;
    if value.get("status").and_then(Value::as_str) != Some("ready") {
        return Err("RUNTIME_UNAVAILABLE: Runtime is not ready".to_owned());
    }
    let socket = value
        .get("socket_path")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "RUNTIME_UNAVAILABLE: Runtime ready handoff has no socket".to_owned())?;
    let token = value
        .get("token")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "RUNTIME_UNAVAILABLE: Runtime ready handoff has no token".to_owned())?;
    Ok(LocalIpcEndpoint::from_parts(
        socket.to_owned(),
        token.to_owned(),
    ))
}

fn read_bounded_json(path: &std::path::Path) -> Result<Value, String> {
    let bytes = fs::read(path).map_err(|_| "file could not be read".to_owned())?;
    if bytes.len() > 64 * 1024 {
        return Err("file exceeds diagnostic response capacity".to_owned());
    }
    serde_json::from_slice(&bytes).map_err(|_| "file contains invalid JSON".to_owned())
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
        "reference_import" => {
            let request: forgecad_runtime::ReferenceImportRequest =
                serde_json::from_value(arguments.clone()).map_err(|error| error.to_string())?;
            serde_json::to_value(
                runtime
                    .import_reference(&request)
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())
        }
        "reference_get" => {
            let id = required_id(arguments, "reference_id")?;
            let reference = runtime
                .reference(id)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "NOT_FOUND: reference not found".to_owned())?;
            serde_json::to_value(forgecad_runtime::ReferenceGetResult {
                schema_version: "ReferenceGetResult@1".to_owned(),
                reference,
            })
            .map_err(|error| error.to_string())
        }
        "geometry_prepare" => {
            let project_id = required_id(arguments, "project_id")?;
            let base_version_id = arguments.get("base_version_id").and_then(Value::as_str);
            let request = arguments
                .get("request")
                .cloned()
                .unwrap_or_else(|| json!({}));
            runtime
                .prepare_geometry_candidate(project_id, base_version_id, request)
                .map_err(|error| error.to_string())
        }
        "appearance_prepare" => {
            let project_id = required_id(arguments, "project_id")?;
            let base_version_id = arguments.get("base_version_id").and_then(Value::as_str);
            let request = arguments
                .get("request")
                .cloned()
                .unwrap_or_else(|| json!({}));
            runtime
                .prepare_appearance_candidate(project_id, base_version_id, request)
                .map_err(|error| error.to_string())
        }
        "change_prepare" => {
            let project_id = required_id(arguments, "project_id")?;
            let base_version_id = arguments.get("base_version_id").and_then(Value::as_str);
            let request = arguments
                .get("request")
                .cloned()
                .unwrap_or_else(|| json!({}));
            runtime
                .prepare_change_candidate(project_id, base_version_id, request)
                .map_err(|error| error.to_string())
        }
        "artifact_readback_get" => {
            let artifact_id = required_id(arguments, "artifact_id")?;
            let candidate_id = required_id(arguments, "candidate_id")?;
            runtime
                .artifact_readback(artifact_id, candidate_id)
                .map_err(|error| error.to_string())
        }
        "quality_get" => {
            let candidate_id = required_id(arguments, "candidate_id")?;
            let reference_id = arguments.get("reference_id").and_then(Value::as_str);
            runtime
                .quality(candidate_id, reference_id)
                .map_err(|error| error.to_string())
        }
        "version_diff" => {
            let version_id = required_id(arguments, "version_id")?;
            let compare_to_version_id = required_id(arguments, "compare_to_version_id")?;
            runtime
                .version_diff(version_id, compare_to_version_id)
                .map_err(|error| error.to_string())
        }
        "skill_list" => serde_json::to_value(json!({
            "schema_version":"SkillListResult@1",
            "skills":runtime.skills().map_err(|error| error.to_string())?
        }))
        .map_err(|error| error.to_string()),
        "skill_get" => {
            let skill_id = required_id(arguments, "skill_id")?;
            let version = required_id(arguments, "version")?;
            let skill = runtime
                .skill(skill_id, version)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| {
                    "CAPABILITY_UNAVAILABLE: Skill version is not in the first-party registry"
                        .to_owned()
                })?;
            serde_json::to_value(json!({"schema_version":"SkillGetResult@1","skill":skill}))
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

fn tool_manifest_hash(write_tools_enabled: bool) -> String {
    canonical_json_hash(&json!({"tools":tools_with_writes(write_tools_enabled)}))
}

fn runtime_error_value(error: &str) -> Value {
    let (code, message) = error
        .split_once(':')
        .unwrap_or(("RUNTIME_REQUEST_FAILED", error));
    let retryable = code.trim() == "RUNTIME_UNAVAILABLE";
    json!({
        "schema_version":"RuntimeError@1",
        "code":code.trim(),
        "message":safe_error(message.trim()),
        "retryable":retryable,
        "next_action":if retryable {"Call runtime_status/doctor and retry after Runtime reaches Ready."} else {"Read capabilities_get and correct the request or wait for the required MCP task."},
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
    use std::fs;
    use std::sync::Arc;
    use std::thread;

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
    fn initialize_succeeds_without_runtime_and_dependent_calls_are_retryable() {
        let mut backend = Backend::Unavailable("Runtime child is absent".to_owned());
        let mut session = Session::new();
        let response = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":1,
                "method":"initialize",
                "params":{"protocolVersion":MCP_PROTOCOL_VERSION,"capabilities":{},"clientInfo":{"name":"test","version":"1"}}
            }),
        )
        .expect("initialize response");
        assert_eq!(response["result"]["protocolVersion"], MCP_PROTOCOL_VERSION);
        assert_eq!(session.state, SessionState::Ready);

        let status = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"runtime_status","arguments":{}}}),
        )
        .expect("runtime status response");
        assert_eq!(status["result"]["structuredContent"]["state"], "Degraded");

        let dependent = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"project_list","arguments":{}}}),
        )
        .expect("degraded response");
        assert_eq!(dependent["result"]["isError"], true);
        assert_eq!(
            dependent["result"]["structuredContent"]["code"],
            "RUNTIME_UNAVAILABLE"
        );
        assert_eq!(dependent["result"]["structuredContent"]["retryable"], true);
    }

    #[test]
    fn read_only_calls_do_not_modify_projects_or_versions() {
        let mut backend = Backend::InProcess(Runtime::ephemeral().expect("runtime"));
        let mut session = Session::new();
        let initialized_response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":MCP_PROTOCOL_VERSION,"capabilities":{},"clientInfo":{"name":"readonly-test","version":"1"}}}),
        )
        .expect("initialize response");
        assert!(initialized_response["result"].is_object());

        let before_projects = match &backend {
            Backend::InProcess(runtime) => {
                serde_json::to_value(runtime.projects().expect("projects before")).expect("json")
            }
            _ => unreachable!("test backend"),
        };
        let before_versions = match &backend {
            Backend::InProcess(runtime) => {
                serde_json::to_value(runtime.versions(None).expect("versions before"))
                    .expect("json")
            }
            _ => unreachable!("test backend"),
        };
        for (id, name, arguments) in [
            (2, "project_list", json!({})),
            (3, "version_list", json!({})),
        ] {
            let response = handle(
                &mut backend,
                &mut session,
                &json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":arguments}}),
            )
            .expect("read-only response");
            assert!(response["result"]["structuredContent"].is_array());
        }
        let after_projects = match &backend {
            Backend::InProcess(runtime) => {
                serde_json::to_value(runtime.projects().expect("projects after")).expect("json")
            }
            _ => unreachable!("test backend"),
        };
        let after_versions = match &backend {
            Backend::InProcess(runtime) => {
                serde_json::to_value(runtime.versions(None).expect("versions after")).expect("json")
            }
            _ => unreachable!("test backend"),
        };
        assert_eq!(before_projects, after_projects);
        assert_eq!(before_versions, after_versions);
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
    fn ipc_request_errors_keep_typed_codes_without_leaking_details() {
        assert_eq!(
            map_ipc_error(IpcError::RuntimeRequest(
                "INVALID_INPUT: /private/user/reference.png".to_owned(),
            )),
            "INVALID_INPUT: Runtime request rejected"
        );
        assert_eq!(
            map_ipc_error(IpcError::RuntimeRequest("RUNTIME_BUSY".to_owned())),
            "RUNTIME_BUSY: Runtime writer is busy"
        );
        assert!(map_ipc_error(IpcError::Io(std::io::Error::other("socket")))
            .starts_with("RUNTIME_UNAVAILABLE:"));
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
        let first = tools_with_writes(false);
        let second = tools_with_writes(false);
        assert_eq!(first, second);
        assert!(first
            .iter()
            .all(|tool| tool["annotations"]["readOnlyHint"] == true));
        assert!(first
            .iter()
            .all(|tool| tool["annotations"]["destructiveHint"] == false));
    }

    #[test]
    fn mcp004_write_tools_are_explicit_and_confirmation_bound() {
        let disabled = tools_with_writes(false);
        assert_eq!(disabled.len(), 17);
        assert!(!disabled
            .iter()
            .any(|tool| { tool["name"].as_str().is_some_and(is_mcp004_write_tool) }));

        let enabled = tools_with_writes(true);
        assert_eq!(enabled.len(), 30);
        for name in mcp004_write_tool_names() {
            let tool = enabled
                .iter()
                .find(|tool| tool["name"].as_str() == Some(name.as_str()))
                .expect("MCP004 write tool");
            assert_eq!(tool["annotations"]["readOnlyHint"], false);
            let expected_idempotent = !matches!(
                name.as_str(),
                "project_create" | "candidate_prepare" | "restore_prepare" | "export_prepare"
            );
            assert_eq!(tool["annotations"]["idempotentHint"], expected_idempotent);
            assert_eq!(tool["_meta"]["forgecad"]["requiresConfirmation"], true);
            assert_eq!(tool["_meta"]["forgecad"]["transaction"], "MCP004");
        }
        for name in ["restore_prepare", "export_prepare"] {
            let tool = enabled
                .iter()
                .find(|tool| tool["name"].as_str() == Some(name))
                .expect("prepare tool");
            assert!(tool["inputSchema"]["required"]
                .as_array()
                .expect("required schema")
                .iter()
                .any(|value| value == "request"));
        }
    }

    #[test]
    fn mcp004_write_call_is_typed_disabled_by_default() {
        let (mut backend, mut session) = initialized();
        let response = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":7,
                "method":"tools/call",
                "params":{"name":"candidate_prepare","arguments":{}}
            }),
        )
        .expect("typed disabled response");
        assert_eq!(response["result"]["isError"], true);
        assert_eq!(
            response["result"]["structuredContent"]["code"],
            "MCP004_WRITE_TOOLS_DISABLED"
        );
        assert_eq!(session.write_tools_enabled, false);
    }

    #[test]
    fn build_cohort_write_gate_is_exact_and_ordinary_builds_remain_compatible() {
        let local = "a".repeat(64);
        let other = "b".repeat(64);
        assert!(
            require_matching_build_cohort(None, &json!({"build_cohort_sha256":Value::Null}))
                .is_ok()
        );
        assert!(
            require_matching_build_cohort(Some(&local), &json!({"build_cohort_sha256":local}))
                .is_ok()
        );
        for capabilities in [
            json!({"build_cohort_sha256":Value::Null}),
            json!({"build_cohort_sha256":other}),
            json!({}),
        ] {
            let error = require_matching_build_cohort(Some(&local), &capabilities)
                .expect_err("missing or different Runtime cohort must fail closed");
            assert!(error.starts_with("BUILD_COHORT_MISMATCH:"));
            assert_eq!(runtime_error_value(&error)["code"], "BUILD_COHORT_MISMATCH");
        }
    }

    #[cfg(unix)]
    #[test]
    fn dynamic_build_cohort_mismatch_never_executes_the_write() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("fc-cm-{}", nonce % 1_000_000_000));
        fs::create_dir_all(&directory).expect("directory");
        let endpoint = LocalIpcEndpoint::new(&directory).expect("endpoint");
        let runtime = Arc::new(Runtime::ephemeral().expect("runtime"));
        let server = runtime.ipc_server(&endpoint).expect("server");
        let runtime_for_thread = runtime.clone();
        let server_thread = thread::spawn(move || server.serve_forever(&runtime_for_thread));
        let ready_file = directory.join("ready.json");
        fs::write(
            &ready_file,
            serde_json::to_vec(&json!({
                "status":"ready",
                "socket_path":endpoint.socket_path().to_string_lossy(),
                "token":endpoint.token()
            }))
            .expect("ready JSON"),
        )
        .expect("ready file");
        let runtime_cohort = runtime.capabilities().build_cohort_sha256.as_deref();
        let cohort_a = "a".repeat(64);
        let local_cohort = if runtime_cohort == Some(cohort_a.as_str()) {
            "b".repeat(64)
        } else {
            cohort_a
        };
        let mut backend = Backend::DynamicIpc(DynamicIpcBackend {
            ready_file,
            status_file: None,
        });

        let error = dispatch_tool_with_build_cohort(
            &mut backend,
            "project_create",
            &json!({"name":"must not be created","policy":{"profile":"mvp"}}),
            true,
            Some(&local_cohort),
        )
        .expect_err("mismatched package must reject write");
        assert!(error.starts_with("BUILD_COHORT_MISMATCH:"));
        assert!(runtime.projects().expect("projects").is_empty());

        let mut shutdown = LocalIpcClient::connect(&endpoint).expect("shutdown client");
        shutdown
            .call("runtime_shutdown", Value::Null)
            .expect("shutdown");
        drop(shutdown);
        assert!(server_thread.join().expect("server thread").is_ok());
        drop(runtime);
        let _ = fs::remove_dir_all(directory);
    }

    #[cfg(unix)]
    #[test]
    fn authenticated_ipc_opt_in_exposes_mcp004_prepare_without_enabling_in_process_runtime() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("fc4-{}", nonce % 1_000_000_000));
        fs::create_dir_all(&directory).expect("directory");
        let endpoint = LocalIpcEndpoint::new(&directory).expect("endpoint");
        let runtime = Arc::new(Runtime::ephemeral().expect("runtime"));
        let project = runtime
            .create_project("MCP004 adapter fixture", json!({"scope":"test"}))
            .expect("project");
        let object = runtime
            .put_object(
                b"MCP004 adapter prepared object",
                None,
                "application/octet-stream",
                "prepared-object",
            )
            .expect("object");
        let server = runtime.ipc_server(&endpoint).expect("server");
        let runtime_for_thread = runtime.clone();
        let server_thread = thread::spawn(move || runtime_for_thread.serve_ipc_once(&server));
        let client = LocalIpcClient::connect(&endpoint).expect("client");
        let prior_opt_in = std::env::var_os("FORGECAD_MCP_ENABLE_MCP004_WRITES");
        std::env::set_var("FORGECAD_MCP_ENABLE_MCP004_WRITES", "1");

        let mut backend = Backend::AuthenticatedIpc(client);
        let mut session = Session::new();
        let initialized_response = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":1,
                "method":"initialize",
                "params":{"protocolVersion":MCP_PROTOCOL_VERSION,"capabilities":{},"clientInfo":{"name":"mcp004-test","version":"1"}}
            }),
        )
        .expect("initialize response");
        assert_eq!(
            initialized_response["result"]["protocolVersion"],
            MCP_PROTOCOL_VERSION
        );
        assert!(session.write_tools_enabled);

        let created = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":2,
                "method":"tools/call",
                "params":{"name":"project_create","arguments":{"name":"Codex MVP project","policy":{"profile":"mvp"}}}
            }),
        )
        .expect("project create");
        let diagnostic_project_id = created["result"]["structuredContent"]["project_id"]
            .as_str()
            .expect("created project id")
            .to_owned();

        let listed = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
        )
        .expect("tools list");
        assert_eq!(listed["result"]["tools"].as_array().unwrap().len(), 30);

        let imported = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":2,
                "method":"tools/call",
                "params":{"name":"reference_import","arguments":{
                    "project_id":diagnostic_project_id.clone(),
                    "source":{"kind":"inline_content","mime":"image/png","content_base64":"iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="},
                    "authorization":{"user_authorized":true,"declaration":"MCP005 adapter fixture"}
                }}
            }),
        )
        .expect("reference import");
        assert_eq!(
            imported["result"]["structuredContent"]["reference"]["mime"],
            "image/png"
        );
        let reference_id = imported["result"]["structuredContent"]["reference"]["reference_id"]
            .as_str()
            .expect("reference id")
            .to_owned();
        let reference = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":2,
                "method":"tools/call",
                "params":{"name":"reference_get","arguments":{"reference_id":reference_id}}
            }),
        )
        .expect("reference get");
        assert_eq!(
            reference["result"]["structuredContent"]["reference"]["width"],
            1
        );

        let prepared = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":3,
                "method":"tools/call",
                "params":{"name":"candidate_prepare","arguments":{
                    "project_id":project.project_id,
                    "prepared_object_id":"mcp004-prepared-object",
                    "prepared_object_sha256":object.record.sha256,
                    "request":{"typed":"diagnostic"}
                }}
            }),
        )
        .expect("candidate prepare");
        assert!(
            prepared["result"]["structuredContent"]["candidate"]["candidate_id"]
                .as_str()
                .is_some()
        );

        let diagnostic = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":5,
                "method":"tools/call",
                "params":{"name":"candidate_prepare","arguments":{"project_id":diagnostic_project_id,"request":{"typed":"diagnostic","label":"codex-mvp"}}}
            }),
        )
        .expect("diagnostic candidate prepare");
        assert_eq!(
            diagnostic["result"]["structuredContent"]["candidate"]["state"],
            "reviewable"
        );
        assert_eq!(
            diagnostic["result"]["structuredContent"]["candidate"]["quality_hard_gate_passed"],
            true
        );

        let mut geometry_program = json!({
            "schema_version":"GeometryProgram@1",
            "project_id":diagnostic_project_id.clone(),
            "representation_plan_sha256":"c".repeat(64),
            "nodes":[
                {"node_id":"torso","operator_id":"forgecad.geometry.primitive@1","part_id":"torso","parameters":{"shape":"box","size":[1.0,1.4,0.5],"position":[0.0,1.2,0.0],"material_zone_id":"zone-white-shell"}},
                {"node_id":"core","operator_id":"forgecad.geometry.primitive@1","part_id":"core","parameters":{"shape":"cylinder","size":[0.45,1.0,0.45],"position":[0.0,1.2,0.0],"material_zone_id":"zone-black-mechanical"}}
            ],
            "budgets":{"max_nodes":8,"max_triangles":10000,"max_runtime_ms":1000}
        });
        let geometry_hash = canonical_json_hash(&geometry_program);
        geometry_program["canonical_sha256"] = Value::String(geometry_hash);
        let geometry = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":6,
                "method":"tools/call",
                "params":{"name":"geometry_prepare","arguments":{"project_id":diagnostic_project_id.clone(),"request":{"typed":"geometry","geometry_program":geometry_program}}}
            }),
        )
        .expect("geometry prepare");
        assert_eq!(
            geometry["result"]["structuredContent"]["schema_version"],
            "GeometryPrepareResult@1"
        );
        assert_eq!(
            geometry["result"]["structuredContent"]["candidate"]["state"],
            "reviewable"
        );
        let artifact_id = geometry["result"]["structuredContent"]["artifact"]["artifact_id"]
            .as_str()
            .expect("geometry artifact")
            .to_owned();
        let candidate_id = geometry["result"]["structuredContent"]["candidate"]["candidate_id"]
            .as_str()
            .expect("geometry candidate")
            .to_owned();
        let readback = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":7,
                "method":"tools/call",
                "params":{"name":"artifact_readback_get","arguments":{"artifact_id":artifact_id,"candidate_id":candidate_id}}
            }),
        )
        .expect("artifact readback");
        assert_eq!(
            readback["result"]["structuredContent"]["validator_status"],
            "passed"
        );
        assert_eq!(
            readback["result"]["structuredContent"]["part_ids"]
                .as_array()
                .unwrap()
                .len(),
            2
        );

        let mut appearance_program = json!({
            "schema_version":"AppearanceProgram@1",
            "project_id":diagnostic_project_id.clone(),
            "geometry_program_sha256":geometry_program["canonical_sha256"].clone(),
            "material_zones":[
                {"zone_id":"zone-white-shell","part_ids":["torso"],"base_color":[0.78,0.82,0.86,1.0],"metallic":0.72,"roughness":0.28,"emissive":[0.0,0.0,0.0]},
                {"zone_id":"zone-black-mechanical","part_ids":["core"],"base_color":[0.03,0.04,0.05,1.0],"metallic":0.75,"roughness":0.3,"emissive":[0.0,0.0,0.0]},
                {"zone_id":"zone-amber-emissive","part_ids":["core"],"base_color":[0.16,0.06,0.01,1.0],"metallic":0.2,"roughness":0.25,"emissive":[1.0,0.12,0.01]}
            ]
        });
        appearance_program["canonical_sha256"] =
            Value::String(canonical_json_hash(&appearance_program));
        let appearance = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":8,
                "method":"tools/call",
                "params":{"name":"appearance_prepare","arguments":{"project_id":diagnostic_project_id.clone(),"request":{"typed":"appearance","geometry_program":geometry_program,"appearance_program":appearance_program}}}
            }),
        )
        .expect("appearance prepare");
        assert_eq!(
            appearance["result"]["structuredContent"]["schema_version"],
            "AppearancePrepareResult@1"
        );
        assert_eq!(
            appearance["result"]["structuredContent"]["artifact"]["uv_status"],
            "passed"
        );
        assert_eq!(
            appearance["result"]["structuredContent"]["render_set"]["passes"]
                .as_array()
                .unwrap()
                .len(),
            4
        );

        drop(backend);
        assert!(server_thread.join().expect("server thread").is_ok());
        match prior_opt_in {
            Some(value) => std::env::set_var("FORGECAD_MCP_ENABLE_MCP004_WRITES", value),
            None => std::env::remove_var("FORGECAD_MCP_ENABLE_MCP004_WRITES"),
        }
        drop(runtime);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn skill_registry_resources_are_read_only_and_unknown_capabilities_are_typed() {
        let (mut backend, mut session) = initialized();
        let response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":3,"method":"resources/read","params":{"uri":"forgecad://skills/reference-intake/0.1.0"}}),
        )
        .expect("skill resource");
        assert_eq!(
            response["result"]["contents"][0]["mimeType"],
            "application/json"
        );
        assert!(response["result"]["contents"][0]["text"]
            .as_str()
            .expect("resource text")
            .contains("SkillGetResult@1"));

        let unknown = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"skill_get","arguments":{"skill_id":"not-a-skill","version":"0.1.0"}}}),
        )
        .expect("unknown skill error");
        assert_eq!(unknown["result"]["isError"], true);
        assert_eq!(
            unknown["result"]["structuredContent"]["code"],
            "CAPABILITY_UNAVAILABLE"
        );

        let response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"selection_get","arguments":{}}}),
        )
        .expect("selection");
        assert_eq!(response["result"]["structuredContent"]["available"], false);
    }

    #[cfg(unix)]
    #[test]
    fn stale_ready_and_ready_status_never_report_runtime_ready() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("fc-stale-status-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&root).expect("status root");
        let ready_file = root.join("ready.json");
        let status_file = root.join("status.json");
        fs::write(
            &ready_file,
            br#"{"status":"ready","socket_path":"/missing/forgecad.sock","token":"stale"}"#,
        )
        .expect("stale ready");
        fs::write(
            &status_file,
            br#"{"schema_version":"ForgeCADRuntimeSupervisorStatus@1","state":"Ready","retryable":false}"#,
        )
        .expect("stale status");

        let dynamic = DynamicIpcBackend {
            ready_file,
            status_file: Some(status_file),
        };
        let status = dynamic.status();
        assert_eq!(status["state"], "Degraded");
        assert_eq!(status["retryable"], true);
        assert_eq!(status["code"], "RUNTIME_HANDOFF_STALE");
        assert_eq!(status["listener_reachable"], false);
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn reachable_endpoint_with_bounded_auth_timeout_reports_busy() {
        use std::os::unix::net::UnixListener;

        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("fc-bs-{}", nonce % 1_000_000_000));
        fs::create_dir_all(&root).expect("status root");
        let endpoint = LocalIpcEndpoint::new(&root).expect("endpoint");
        let _listener = UnixListener::bind(endpoint.socket_path()).expect("reachable listener");
        let ready_file = root.join("ready.json");
        let status_file = root.join("status.json");
        fs::write(
            &ready_file,
            serde_json::to_vec(&json!({
                "status":"ready",
                "socket_path":endpoint.socket_path().to_string_lossy(),
                "token":endpoint.token()
            }))
            .expect("ready JSON"),
        )
        .expect("ready file");
        fs::write(
            &status_file,
            br#"{"schema_version":"ForgeCADRuntimeSupervisorStatus@1","state":"Ready","retryable":false}"#,
        )
        .expect("status file");
        let backend = Backend::DynamicIpc(DynamicIpcBackend {
            ready_file,
            status_file: Some(status_file),
        });

        let status = runtime_status_payload(&backend).expect("status");
        assert_eq!(status["state"], "Busy");
        assert_eq!(status["retryable"], true);
        assert_eq!(status["code"], "RUNTIME_BUSY");
        assert_eq!(status["listener_reachable"], true);
        let doctor = doctor_payload(&backend).expect("doctor");
        assert_eq!(doctor["checks"]["runtime_supervisor"], "busy");
        assert_eq!(doctor["checks"]["runtime_endpoint"], "reachable_busy");
        let _ = fs::remove_dir_all(root);
    }
}
