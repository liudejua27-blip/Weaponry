#![recursion_limit = "256"]

//! Default Knife-only Weaponry MCP adapter with typed five-domain routing.
//! Historical raw handlers live in the explicit `forgecad-mcp-compat` binary.

mod active_manifest;
mod active_schema;
mod active_session;
mod domain_router;
mod knife_curve_evaluated_mesh_tools;
mod knife_curve_modifier_graph_tools;
mod knife_tool_profile;
mod result_adapter;
mod supervisor;
mod compatibility_registry {
    //! Typed fail-closed shim; the raw registry is not part of this binary.
    //!
    //! This module deliberately has no feature gate. Cargo may be asked to
    //! check every package binary with `--all-features`; the default binary
    //! must remain Knife-only even when the compatibility feature is enabled
    //! for the separate `forgecad-mcp-compat` target.
    use serde_json::Value;

    pub(crate) const UNAVAILABLE_ERROR: &str =
        "WEAPONRY_COMPATIBILITY_PROFILE_UNAVAILABLE: rebuild the explicit forgecad-mcp-compat binary with --features legacy-compatibility-registry";

    pub(crate) fn ensure_enabled() -> Result<(), String> {
        Err(UNAVAILABLE_ERROR.to_owned())
    }

    #[allow(dead_code)]
    pub(crate) fn tools_with_writes(_writes_enabled: bool) -> Vec<Value> {
        Vec::new()
    }
}

use active_session::{InitializeError, Session, SessionState};
use forgecad_contracts::weaponry_domain_map::WeaponryOperationExecutionTarget;
use forgecad_runtime::{
    build_cohort_sha256, canonical_json_hash, IpcError, LocalIpcClient, LocalIpcEndpoint, Runtime,
    RuntimeCapabilities, MCP_PROTOCOL_VERSIONS,
};
use result_adapter::{
    apply_mcp_response_budget, error_response, safe_error, tool_error, MCP_RESPONSE_MAX_BYTES,
};
use serde_json::{json, Value};
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use supervisor::MvpSupervisor;

const SERVER_NAME: &str = "forgecad";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
const INSTRUCTIONS: &str = "ForgeCAD is a local Codex-only Weaponry Runtime. Call weapon_preflight.skill_get with ponytail-preflight@0.1.0 before other design operations. Permanent writes require explicit authenticated MCP opt-in. Do not send arbitrary code, URLs, secrets, or unauthorized paths.";
const PREFLIGHT_SKILL_ID: &str = "ponytail-preflight";
const PREFLIGHT_VERSION: &str = "0.1.0";
const PREFLIGHT_REQUIRED: &str = "PONYTAIL_PREFLIGHT_REQUIRED: call skill_get with ponytail-preflight@0.1.0 before using ForgeCAD design tools or another Skill";
enum Backend {
    InProcess(Runtime),
    AuthenticatedIpc(LocalIpcClient),
    DynamicIpc(DynamicIpcBackend),
    Unavailable(String),
}

struct DynamicIpcBackend {
    ready_file: Option<PathBuf>,
    fixed_endpoint: Option<LocalIpcEndpoint>,
    status_file: Option<PathBuf>,
}

fn main() {
    if std::env::args().skip(1).eq(["--build-identity"]) {
        print_build_identity();
        return;
    }
    if std::env::args()
        .skip(1)
        .eq(["--knife-tool-manifest-summary"])
    {
        print_knife_manifest_summary();
        return;
    }
    if !valid_arguments() {
        eprintln!("usage: forgecad-mcp [serve --stdio | --build-identity | --knife-tool-manifest-summary]");
        return;
    }

    let (mut backend, mut runtime_supervisor) = backend_from_environment();
    let mut session = Session::new();
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout());
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        if let Some(supervisor) = runtime_supervisor.as_mut() {
            supervisor.poll();
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

fn print_build_identity() {
    println!(
        "{}",
        serde_json::to_string(&json!({
            "schema_version":"ForgeCADDevBuildIdentity@1",
            "component":"forgecad-mcp",
            "profile":knife_tool_profile::KNIFE_PROFILE_ID,
            "compatibility_binary":"forgecad-mcp-compat",
            "compatibility_feature_default":false,
            "build_cohort_sha256":build_cohort_sha256()
        }))
        .expect("build identity serializes")
    );
}

fn print_knife_manifest_summary() {
    match active_manifest::build_manifest_summary() {
        Ok(summary) => println!(
            "{}",
            serde_json::to_string(&summary).expect("manifest summary serializes")
        ),
        Err(error) => eprintln!("{error}"),
    }
}

fn valid_arguments() -> bool {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    args.is_empty() || args == ["serve".to_owned(), "--stdio".to_owned()]
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
            Some(json!({"code":"INVALID_REQUEST"})),
        );
    };
    match method {
        "initialize" => initialize(backend, session, id, request.get("params")),
        "notifications/initialized" | "notifications/cancelled" => None,
        "ping" => id.map(|id| json!({"jsonrpc":"2.0","id":id,"result":{}})),
        "server/discover" if session.state() == SessionState::New => error_response(
            id,
            -32022,
            "Modern MCP protocol is not enabled for this stdio endpoint",
            Some(json!({"code":"CONTRACT_VERSION_UNSUPPORTED","supported":MCP_PROTOCOL_VERSIONS})),
        ),
        _ if session.state() == SessionState::New => error_response(
            id,
            -32000,
            "Server is not initialized",
            Some(json!({"code":"SERVER_NOT_INITIALIZED"})),
        ),
        _ if session.state() == SessionState::Failed => error_response(
            id,
            -32001,
            "Server initialization failed; restart after correcting the contract",
            Some(json!({"code":"CONTRACT_VERSION_UNSUPPORTED"})),
        ),
        "tools/list" => tools_list(id),
        "resources/list" => resources_list(id),
        "resources/templates/list" => {
            id.map(|id| json!({"jsonrpc":"2.0","id":id,"result":{"resourceTemplates":[]}}))
        }
        "resources/read" => resources_read(backend, session, id, request.get("params")),
        "tools/call" => call_tool(backend, session, id, request.get("params")),
        _ => error_response(
            id,
            -32601,
            "Method not found",
            Some(json!({"code":"METHOD_NOT_FOUND","method":method})),
        ),
    }
}

fn initialize(
    backend: &Backend,
    session: &mut Session,
    id: Option<Value>,
    params: Option<&Value>,
) -> Option<Value> {
    let Some(id) = id else {
        session.mark_failed();
        return None;
    };
    let requested = match session.try_initialize(
        params,
        MCP_PROTOCOL_VERSIONS,
        write_opt_in(backend),
    ) {
        Ok(requested) => requested,
        Err(InitializeError::AlreadyInitialized) => {
            return error_response(
                Some(id),
                -32600,
                "Initialize may only be called once",
                Some(json!({"code":"ALREADY_INITIALIZED"})),
            );
        }
        Err(InitializeError::UnsupportedProtocol {
            requested,
            supported,
        }) => {
            return error_response(
                Some(id),
                -32602,
                "Unsupported protocol version",
                Some(
                    json!({"code":"CONTRACT_VERSION_UNSUPPORTED","requested":requested,"supported":supported}),
                ),
            );
        }
        Err(InitializeError::InvalidParams) => {
            return error_response(
                Some(id),
                -32602,
                "Invalid initialize params",
                Some(json!({"code":"INVALID_INITIALIZE_PARAMS"})),
            );
        }
        Err(InitializeError::MissingCapabilitiesOrClientInfo) => {
            return error_response(
                Some(id),
                -32602,
                "Initialize requires capabilities and clientInfo",
                Some(json!({"code":"INVALID_INITIALIZE_PARAMS"})),
            );
        }
    };
    Some(json!({
        "jsonrpc":"2.0",
        "id":id,
        "result":{
            "protocolVersion":requested,
            "capabilities":{"resources":{"listChanged":false,"subscribe":false},"tools":{"listChanged":false}},
            "serverInfo":{"name":SERVER_NAME,"version":SERVER_VERSION},
            "instructions":INSTRUCTIONS
        }
    }))
}

fn tools_list(id: Option<Value>) -> Option<Value> {
    let id = id?;
    match active_manifest::build_tools_list() {
        Ok(result) => Some(json!({"jsonrpc":"2.0","id":id,"result":result})),
        Err(error) => error_response(
            Some(id),
            -32603,
            "Weaponry tool profile is invalid",
            Some(json!({"code":"WEAPONRY_KNIFE_PROFILE_INVALID","detail":safe_error(&error)})),
        ),
    }
}

fn resources_list(id: Option<Value>) -> Option<Value> {
    let id = id?;
    Some(json!({"jsonrpc":"2.0","id":id,"result":active_manifest::build_resources_list()}))
}

fn resources_read(
    backend: &mut Backend,
    session: &Session,
    id: Option<Value>,
    params: Option<&Value>,
) -> Option<Value> {
    let id = id?;
    let Some(uri) = params
        .and_then(Value::as_object)
        .and_then(|value| value.get("uri"))
        .and_then(Value::as_str)
    else {
        return error_response(
            Some(id),
            -32602,
            "resources/read requires a URI",
            Some(json!({"code":"INVALID_RESOURCE_URI"})),
        );
    };
    if uri != active_manifest::CAPABILITIES_RESOURCE_URI {
        return error_response(
            Some(id),
            -32602,
            "Unknown resource URI",
            Some(json!({"code":"INVALID_RESOURCE_URI"})),
        );
    }
    let value = capabilities(backend, session.write_tools_enabled())
        .unwrap_or_else(|_| static_capabilities(backend, session.write_tools_enabled()));
    match active_manifest::build_capabilities_read(uri, &value) {
        Ok(result) => Some(json!({"jsonrpc":"2.0","id":id,"result":result})),
        Err(error) => error_response(
            Some(id),
            -32603,
            "Capabilities resource could not be serialized",
            Some(
                json!({"code":error.split(':').next().unwrap_or("MCP_RESOURCE_RESPONSE_FAILED"),"detail":safe_error(&error)}),
            ),
        ),
    }
}

fn call_tool(
    backend: &mut Backend,
    session: &mut Session,
    id: Option<Value>,
    params: Option<&Value>,
) -> Option<Value> {
    let id = id?;
    let Some(params) = params.and_then(Value::as_object) else {
        return error_response(
            Some(id),
            -32602,
            "tools/call requires an object params value",
            Some(json!({"code":"INVALID_TOOL_PARAMS"})),
        );
    };
    let Some(requested_name) = params.get("name").and_then(Value::as_str) else {
        return error_response(
            Some(id),
            -32602,
            "tools/call requires a tool name",
            Some(json!({"code":"INVALID_TOOL_PARAMS"})),
        );
    };
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let (operation, request) = match knife_tool_profile::unwrap_facade_call(
        knife_tool_profile::ToolProfile::Knife,
        requested_name,
        &arguments,
    ) {
        Ok(route) => route,
        Err(error) => {
            return error_response(
                Some(id),
                -32602,
                "Tool is outside the active Weaponry profile",
                Some(
                    json!({"code":error.split(':').next().unwrap_or("WEAPONRY_KNIFE_PROFILE_INVALID"),"detail":safe_error(&error)}),
                ),
            );
        }
    };
    if let Err(error) = knife_tool_profile::validate_active_operation_request(&operation, &request)
    {
        return tool_error(id, &error);
    }
    if requires_preflight(&operation, &request) && !session.preflight_read() {
        return tool_error(id, PREFLIGHT_REQUIRED);
    }
    if operation == "knife_curve_modifier_graph_get"
        || operation == "knife_curve_modifier_graph_prepare"
    {
        if let Err(error) = knife_curve_modifier_graph_tools::validate_call(&operation, &request) {
            return tool_error(id, &error);
        }
    }
    if operation == "knife_curve_evaluated_mesh_get"
        || operation == "knife_curve_evaluated_mesh_prepare"
    {
        if let Err(error) = knife_curve_evaluated_mesh_tools::validate_call(&operation, &request) {
            return tool_error(id, &error);
        }
    }
    let route = match domain_router::resolve(requested_name, &operation) {
        Ok(route) => route,
        Err(error) => return tool_error(id, &error),
    };
    let write = knife_tool_profile::is_write_operation(&operation);
    let dispatched = if route.execution_target == WeaponryOperationExecutionTarget::McpAdapter {
        match operation.as_str() {
            "runtime_status" => runtime_status_payload(backend),
            "doctor" => doctor_payload(backend),
            _ => {
                Err("RUNTIME_OPERATION_TARGET_MISMATCH: unsupported MCP-local operation".to_owned())
            }
        }
    } else if write && !effective_write_tools_enabled(backend, session.write_tools_enabled()) {
        Err(
            "WEAPONRY_KNIFE_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required"
                .to_owned(),
        )
    } else if write {
        backend_domain_write_call(backend, route, &request)
    } else {
        backend_domain_call(backend, route, &request)
    };
    if operation == "skill_get"
        && request.get("skill_id").and_then(Value::as_str) == Some(PREFLIGHT_SKILL_ID)
        && request.get("version").and_then(Value::as_str) == Some(PREFLIGHT_VERSION)
        && dispatched.is_ok()
    {
        session.mark_preflight_read();
    }
    match dispatched {
        Ok(value) => result_adapter::tool_success(
            id,
            &operation,
            value,
            &[
                knife_curve_modifier_graph_tools::summary,
                knife_curve_evaluated_mesh_tools::summary,
            ],
        ),
        Err(error) => tool_error(id, &error),
    }
}

fn requires_preflight(operation: &str, request: &Value) -> bool {
    !matches!(operation, "capabilities_get" | "runtime_status" | "doctor")
        && !(operation == "skill_get"
            && request.get("skill_id").and_then(Value::as_str) == Some(PREFLIGHT_SKILL_ID)
            && request.get("version").and_then(Value::as_str) == Some(PREFLIGHT_VERSION))
}

fn capabilities(backend: &mut Backend, write_requested: bool) -> Result<Value, String> {
    let route = domain_router::resolve("weapon_preflight", "capabilities_get")?;
    let mut value = backend_domain_call(backend, route, &json!({}))?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| "Runtime capabilities are not an object".to_owned())?;
    object.insert(
        "mcp_tool_profile".to_owned(),
        Value::String(knife_tool_profile::KNIFE_PROFILE_ID.to_owned()),
    );
    let tools = knife_tool_profile::active_tools()?;
    object.insert(
        "mcp_public_tool_count".to_owned(),
        Value::from(tools.len() as u64),
    );
    object.insert(
        "tool_manifest_hash".to_owned(),
        Value::String(canonical_json_hash(&json!({"tools":tools}))),
    );
    object.insert(
        "mcp_write_tools_enabled".to_owned(),
        Value::Bool(effective_write_tools_enabled(backend, write_requested)),
    );
    object.insert(
        "mcp_build_cohort_sha256".to_owned(),
        build_cohort_sha256()
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    object.insert(
        "compatibility_profile_available".to_owned(),
        Value::Bool(false),
    );
    object.insert(
        "compatibility_profile_id".to_owned(),
        Value::String(knife_tool_profile::COMPATIBILITY_PROFILE_ID.to_owned()),
    );
    object.insert(
        "compatibility_requires_explicit_profile".to_owned(),
        Value::Bool(true),
    );
    Ok(value)
}

fn static_capabilities(backend: &Backend, write_requested: bool) -> Value {
    let mut capabilities =
        serde_json::to_value(RuntimeCapabilities::default()).unwrap_or_else(|_| json!({}));
    if let Some(object) = capabilities.as_object_mut() {
        object.insert(
            "status".to_owned(),
            Value::String("runtime-unavailable".to_owned()),
        );
        object.insert(
            "mcp_tool_profile".to_owned(),
            Value::String(knife_tool_profile::KNIFE_PROFILE_ID.to_owned()),
        );
        object.insert(
            "mcp_public_tool_count".to_owned(),
            Value::from(knife_tool_profile::FACADE_NAMES.len() as u64),
        );
        // A degraded static projection has no Runtime cohort to compare. A
        // source build may retain the explicit opt-in bit; packaged builds
        // stay fail-closed until a live capabilities response is available.
        object.insert(
            "mcp_write_tools_enabled".to_owned(),
            Value::Bool(write_requested && build_cohort_sha256().is_none()),
        );
        object.insert(
            "compatibility_profile_available".to_owned(),
            Value::Bool(false),
        );
        object.insert(
            "runtime_supervisor_status".to_owned(),
            runtime_status_payload(backend).unwrap_or_else(|_| json!({"state":"Degraded"})),
        );
    }
    capabilities
}

fn write_opt_in(backend: &Backend) -> bool {
    matches!(
        backend,
        Backend::AuthenticatedIpc(_) | Backend::DynamicIpc(_) | Backend::InProcess(_)
    ) && std::env::var("FORGECAD_MCP_ENABLE_MCP004_WRITES").as_deref() == Ok("1")
}

fn effective_write_tools_enabled(backend: &mut Backend, requested: bool) -> bool {
    if !requested {
        return false;
    }
    let Some(local) = build_cohort_sha256() else {
        return true;
    };
    let Ok(runtime) = backend_capabilities_call(backend) else {
        return false;
    };
    runtime.get("build_cohort_sha256").and_then(Value::as_str) == Some(local.as_str())
}

fn backend_capabilities_call(backend: &mut Backend) -> Result<Value, String> {
    let route = domain_router::resolve("weapon_preflight", "capabilities_get")?;
    backend_domain_call(backend, route, &json!({}))
}

fn backend_domain_call(
    backend: &mut Backend,
    route: domain_router::ResolvedRoute<'_>,
    arguments: &Value,
) -> Result<Value, String> {
    let payload = domain_router::ipc_payload(route, arguments);
    match backend {
        // These two bootstrap reads predate the typed Runtime operation
        // envelope.  Keep their public route/domain resolution above, then
        // use the Runtime's existing read-only methods so preflight and the
        // capability/cohort gate remain live after the physical extraction.
        Backend::AuthenticatedIpc(client)
            if route.operation == "capabilities_get" || route.operation == "skill_get" =>
        {
            client
                .call(route.operation, arguments.clone())
                .map_err(map_ipc_error)
        }
        Backend::DynamicIpc(dynamic)
            if route.operation == "capabilities_get" || route.operation == "skill_get" =>
        {
            dynamic.call_raw(route.operation, arguments)
        }
        Backend::InProcess(runtime) if route.operation == "capabilities_get" => {
            serde_json::to_value(runtime.capabilities()).map_err(|error| error.to_string())
        }
        Backend::InProcess(runtime) if route.operation == "skill_get" => {
            let skill_id = arguments
                .get("skill_id")
                .and_then(Value::as_str)
                .ok_or_else(|| "skill_id is required".to_owned())?;
            let version = arguments
                .get("version")
                .and_then(Value::as_str)
                .ok_or_else(|| "version is required".to_owned())?;
            runtime.skill_result(skill_id, version)
        }
        Backend::AuthenticatedIpc(client) => client
            .call_weaponry_operation(route.domain, route.operation, arguments.clone())
            .map_err(map_ipc_error),
        Backend::DynamicIpc(dynamic) => dynamic.call_weaponry_operation(&payload),
        Backend::InProcess(runtime) => runtime
            .invoke_weaponry_operation(route.domain, route.operation, arguments)
            .map_err(|error| error.to_string()),
        Backend::Unavailable(detail) => Err(format!("RUNTIME_UNAVAILABLE: {detail}")),
    }
}

fn backend_domain_write_call(
    backend: &mut Backend,
    route: domain_router::ResolvedRoute<'_>,
    arguments: &Value,
) -> Result<Value, String> {
    let Some(local) = build_cohort_sha256() else {
        return backend_domain_call(backend, route, arguments);
    };
    let runtime = backend_capabilities_call(backend)?;
    if runtime.get("build_cohort_sha256").and_then(Value::as_str) != Some(local.as_str()) {
        return Err(
            "BUILD_COHORT_MISMATCH: Runtime and MCP development builds must match before writes"
                .to_owned(),
        );
    }
    backend_domain_call(backend, route, arguments)
}

impl DynamicIpcBackend {
    fn from_ready_file(ready_file: PathBuf, status_file: Option<PathBuf>) -> Self {
        Self {
            ready_file: Some(ready_file),
            fixed_endpoint: None,
            status_file,
        }
    }

    fn from_fixed_endpoint(endpoint: LocalIpcEndpoint) -> Self {
        Self {
            ready_file: None,
            fixed_endpoint: Some(endpoint),
            status_file: None,
        }
    }

    fn endpoint(&self) -> Result<LocalIpcEndpoint, String> {
        if let Some(endpoint) = &self.fixed_endpoint {
            return Ok(endpoint.clone());
        }
        let path = self
            .ready_file
            .as_deref()
            .ok_or_else(|| "RUNTIME_UNAVAILABLE: Runtime endpoint is unavailable".to_owned())?;
        let value = read_bounded_json(path)?;
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

    fn call_weaponry_operation(&self, payload: &Value) -> Result<Value, String> {
        self.call_raw("weaponry_domain_operation", payload)
    }

    fn call_raw(&self, method: &str, payload: &Value) -> Result<Value, String> {
        let endpoint = self.endpoint()?;
        let mut client = LocalIpcClient::connect(&endpoint).map_err(map_ipc_error)?;
        client.call(method, payload.clone()).map_err(map_ipc_error)
    }

    fn status(&self) -> Value {
        let ready_probe = match self.endpoint() {
            Ok(endpoint) => probe_dynamic_endpoint(&endpoint),
            Err(_) => DynamicReadyProbe::Unavailable,
        };
        match ready_probe {
            DynamicReadyProbe::Authenticated => {
                return json!({"schema_version":"ForgeCADRuntimeSupervisorStatus@1","state":"Ready","retryable":false,"source":"authenticated_ready_handoff"});
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
                });
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

fn probe_dynamic_endpoint(endpoint: &LocalIpcEndpoint) -> DynamicReadyProbe {
    match LocalIpcClient::connect(endpoint) {
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

fn backend_from_environment() -> (Backend, Option<MvpSupervisor>) {
    match (
        std::env::var("FORGECAD_RUNTIME_SOCKET").ok(),
        std::env::var("FORGECAD_RUNTIME_TOKEN").ok(),
    ) {
        (Some(socket), Some(token)) => (
            Backend::DynamicIpc(DynamicIpcBackend::from_fixed_endpoint(
                LocalIpcEndpoint::from_parts(socket, token),
            )),
            None,
        ),
        (None, None) => match std::env::var_os("FORGECAD_RUNTIME_READY_FILE") {
            Some(path) if !path.is_empty() => (
                Backend::DynamicIpc(DynamicIpcBackend::from_ready_file(
                    PathBuf::from(path),
                    std::env::var_os("FORGECAD_RUNTIME_STATUS_FILE").map(PathBuf::from),
                )),
                None,
            ),
            _ => match supervisor::runtime_data_root()
                .and_then(|root| MvpSupervisor::new(supervisor::runtime_command(), root))
            {
                Ok(mut supervisor) => {
                    let backend = Backend::DynamicIpc(DynamicIpcBackend::from_ready_file(
                        supervisor.ready_file().to_path_buf(),
                        Some(supervisor.status_file().to_path_buf()),
                    ));
                    supervisor.start();
                    (backend, Some(supervisor))
                }
                Err(error) => (Backend::Unavailable(error), None),
            },
        },
        _ => (
            Backend::Unavailable("Runtime socket and token must be supplied together".to_owned()),
            None,
        ),
    }
}

fn runtime_status_payload(backend: &Backend) -> Result<Value, String> {
    Ok(match backend {
        Backend::InProcess(_) => {
            json!({"schema_version":"ForgeCADRuntimeSupervisorStatus@1","state":"Ready","retryable":false,"source":"in_process_test_backend"})
        }
        Backend::AuthenticatedIpc(_) => {
            json!({"schema_version":"ForgeCADRuntimeSupervisorStatus@1","state":"Ready","retryable":false,"source":"authenticated_ipc"})
        }
        Backend::DynamicIpc(dynamic) => dynamic.status(),
        Backend::Unavailable(detail) => {
            json!({"schema_version":"ForgeCADRuntimeSupervisorStatus@1","state":"Degraded","retryable":true,"source":"mcp_adapter","detail":safe_error(detail)})
        }
    })
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

fn map_ipc_error(error: IpcError) -> String {
    match error {
        IpcError::RuntimeRequest(detail) => format!(
            "{}: Runtime request rejected",
            detail.split(':').next().unwrap_or("RUNTIME_REQUEST_FAILED")
        ),
        IpcError::AuthenticationFailed => {
            "RUNTIME_UNAVAILABLE: Runtime IPC authentication failed".to_owned()
        }
        IpcError::Io(_) | IpcError::Protocol | IpcError::UnsupportedPlatform => {
            "RUNTIME_UNAVAILABLE: Runtime IPC is unavailable".to_owned()
        }
        IpcError::ShutdownRequested => "RUNTIME_UNAVAILABLE: Runtime is shutting down".to_owned(),
    }
}

fn read_bounded_json(path: &std::path::Path) -> Result<Value, String> {
    let bytes = fs::read(path)
        .map_err(|_| "RUNTIME_UNAVAILABLE: Runtime ready handoff is unavailable".to_owned())?;
    if bytes.len() > 64 * 1024 {
        return Err("RUNTIME_UNAVAILABLE: Runtime ready handoff is too large".to_owned());
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| "RUNTIME_UNAVAILABLE: Runtime ready handoff is invalid".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn initialize_unavailable() -> (Backend, Session) {
        let mut backend = Backend::Unavailable("test runtime is intentionally absent".to_owned());
        let mut session = Session::new();
        let response = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":1,
                "method":"initialize",
                "params":{
                    "protocolVersion":MCP_PROTOCOL_VERSIONS[0],
                    "capabilities":{},
                    "clientInfo":{"name":"codex-test","version":"1"}
                }
            }),
        )
        .expect("initialize responds");
        assert!(response.get("result").is_some());
        assert_eq!(session.state(), SessionState::Ready);
        (backend, session)
    }

    #[test]
    fn default_protocol_exposes_only_eleven_facades_and_hides_raw_compatibility() {
        let (mut backend, mut session) = initialize_unavailable();
        let listed = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
        )
        .expect("tools/list responds");
        let tools = listed["result"]["tools"].as_array().expect("tool array");
        assert_eq!(tools.len(), 11);
        assert_eq!(
            tools
                .iter()
                .filter_map(|tool| tool.get("name").and_then(Value::as_str))
                .collect::<Vec<_>>(),
            knife_tool_profile::FACADE_NAMES
        );

        let hidden = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":3,
                "method":"tools/call",
                "params":{"name":"candidate_get","arguments":{}}
            }),
        )
        .expect("hidden compatibility call responds");
        assert_eq!(
            hidden["error"]["data"]["code"],
            "WEAPONRY_KNIFE_PROFILE_TOOL_HIDDEN"
        );
    }

    #[test]
    fn default_protocol_keeps_doctor_local_and_preflight_fail_closed() {
        let (mut backend, mut session) = initialize_unavailable();
        let doctor = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":4,
                "method":"tools/call",
                "params":{
                    "name":"weapon_preflight",
                    "arguments":{"operation":"doctor","request":{}}
                }
            }),
        )
        .expect("doctor responds");
        assert_eq!(
            doctor["result"]["structuredContent"]["schema_version"],
            "ForgeCADRuntimeDoctor@1"
        );
        assert_eq!(doctor["result"]["structuredContent"]["state"], "Degraded");

        let preflight = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":5,
                "method":"tools/call",
                "params":{
                    "name":"weapon_preflight",
                    "arguments":{
                        "operation":"skill_get",
                        "request":{"skill_id":PREFLIGHT_SKILL_ID,"version":PREFLIGHT_VERSION}
                    }
                }
            }),
        )
        .expect("preflight responds");
        assert_eq!(
            preflight["result"]["structuredContent"]["code"],
            "RUNTIME_UNAVAILABLE"
        );
        assert!(!session.preflight_read());
    }

    #[test]
    fn default_protocol_bounds_wire_responses_and_preserves_supervisor_state() {
        let oversized = json!({
            "jsonrpc":"2.0",
            "id":6,
            "result":{"structuredContent":{"payload":"x".repeat(MCP_RESPONSE_MAX_BYTES)}}
        });
        let bounded = apply_mcp_response_budget("candidate_get", oversized);
        assert_eq!(
            bounded["result"]["structuredContent"]["code"],
            "MCP_READ_MODEL_RESPONSE_BUDGET_EXCEEDED"
        );
        assert!(
            serde_json::to_vec(&bounded)
                .expect("bounded response serializes")
                .len()
                < MCP_RESPONSE_MAX_BYTES
        );

        let status_path = std::env::temp_dir().join(format!(
            "weaponry-mcp-slim-status-{}.json",
            std::process::id()
        ));
        fs::write(
            &status_path,
            serde_json::to_vec(&json!({
                "schema_version":"ForgeCADRuntimeSupervisorStatus@1",
                "state":"Starting",
                "retryable":true
            }))
            .expect("status serializes"),
        )
        .expect("status fixture writes");
        let dynamic = DynamicIpcBackend::from_ready_file(
            status_path.with_extension("missing-ready"),
            Some(status_path.clone()),
        );
        assert_eq!(dynamic.status()["state"], "Starting");
        fs::remove_file(status_path).expect("status fixture removes");
    }
}
