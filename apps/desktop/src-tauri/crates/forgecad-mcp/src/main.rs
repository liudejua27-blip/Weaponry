mod agentic_tools;
mod agentic_action_tools;
mod agentic_write_tools;
mod supervisor;

#[cfg(test)]
use forgecad_runtime::MCP_PROTOCOL_VERSION;
use forgecad_runtime::{
    build_cohort_sha256, canonical_json_hash, is_opaque_id, supports_mcp_protocol, IpcError,
    LocalIpcClient, LocalIpcEndpoint, Runtime, RuntimeCapabilities, MCP_PROTOCOL_VERSIONS,
};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use supervisor::MvpSupervisor;

const SERVER_NAME: &str = "forgecad";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
const INSTRUCTIONS: &str = "ForgeCAD is a local Codex-only 3D Runtime. Before any design tool or any Skill other than the preflight itself, call skill_get for ponytail-preflight@0.1.0 and follow its bounded planning rules. Permanent writes require a prepared candidate and user approval. Long work returns a RuntimeJob. Do not send arbitrary code, URLs, secrets, or unauthorized paths.";
const PONYTAIL_PREFLIGHT_SKILL_ID: &str = "ponytail-preflight";
const PONYTAIL_PREFLIGHT_VERSION: &str = "0.1.0";
const PONYTAIL_PREFLIGHT_REQUIRED: &str = "PONYTAIL_PREFLIGHT_REQUIRED: call skill_get with ponytail-preflight@0.1.0 before using ForgeCAD design tools or another Skill";

enum Backend {
    #[allow(dead_code)]
    InProcess(Runtime),
    // Retained only for focused in-memory IPC coverage.  All production
    // endpoint paths use DynamicIpc so no idle authenticated stream survives
    // between Codex tool calls.
    #[allow(dead_code)]
    AuthenticatedIpc(LocalIpcClient),
    DynamicIpc(DynamicIpcBackend),
    Unavailable(String),
}

struct DynamicIpcBackend {
    ready_file: Option<PathBuf>,
    fixed_endpoint: Option<LocalIpcEndpoint>,
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
    ponytail_preflight_read: bool,
    agentic_binding: agentic_write_tools::Binding,
    agentic_action_binding: agentic_action_tools::Binding,
}

impl Session {
    fn new() -> Self {
        Self {
            state: SessionState::New,
            negotiated_protocol_version: None,
            write_tools_enabled: false,
            ponytail_preflight_read: false,
            agentic_binding: agentic_write_tools::Binding::default(),
            agentic_action_binding: agentic_action_tools::Binding::default(),
        }
    }
}

fn main() {
    if std::env::args().skip(1).eq(["--build-identity"]) {
        print_build_identity("forgecad-mcp");
        return;
    }
    if std::env::args().skip(1).eq(["--tool-manifest-summary"]) {
        print_tool_manifest_summary();
        return;
    }
    if !valid_arguments() {
        eprintln!(
            "usage: forgecad-mcp [serve --stdio | --build-identity | --tool-manifest-summary]"
        );
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

fn print_tool_manifest_summary() {
    let summary = tool_manifest_summary().expect("MCP tool manifest invariants hold");
    println!(
        "{}",
        serde_json::to_string(&summary).expect("tool manifest summary serializes")
    );
}

fn tool_name_set(
    tools: &[Value],
    expected_read_only: Option<bool>,
) -> Result<BTreeSet<String>, String> {
    let mut names = BTreeSet::new();
    for entry in tools {
        let name = entry
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| "tool manifest entry is missing a string name".to_owned())?;
        if !names.insert(name.to_owned()) {
            return Err(format!("duplicate tool manifest name: {name}"));
        }
        if let Some(expected) = expected_read_only {
            let actual = entry
                .pointer("/annotations/readOnlyHint")
                .and_then(Value::as_bool)
                .ok_or_else(|| format!("tool {name} is missing annotations.readOnlyHint"))?;
            if actual != expected {
                return Err(format!(
                    "tool {name} readOnlyHint is {actual}, expected {expected}"
                ));
            }
        }
    }
    Ok(names)
}

fn tool_manifest_summary() -> Result<Value, String> {
    let read_tools = tools_with_writes(false);
    let enabled_tools = tools_with_writes(true);
    let read_names = tool_name_set(&read_tools, Some(true))?;
    let enabled_names = tool_name_set(&enabled_tools, None)?;
    if !read_names.is_subset(&enabled_names) {
        return Err("write-enabled manifest removed a read-only tool".to_owned());
    }
    let write_names = enabled_names
        .difference(&read_names)
        .cloned()
        .collect::<BTreeSet<_>>();
    for entry in &enabled_tools {
        let Some(name) = entry.get("name").and_then(Value::as_str) else {
            continue;
        };
        if write_names.contains(name) {
            let read_only = entry
                .pointer("/annotations/readOnlyHint")
                .and_then(Value::as_bool)
                .ok_or_else(|| format!("write tool {name} is missing annotations.readOnlyHint"))?;
            if read_only {
                return Err(format!("write tool {name} is marked read-only"));
            }
        }
    }
    let declared_write_names = all_write_tool_names();
    let declared_write_set = declared_write_names
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if declared_write_names.len() != declared_write_set.len() {
        return Err("all_write_tool_names contains a duplicate".to_owned());
    }
    if write_names != declared_write_set {
        return Err("write-enabled manifest differs from all_write_tool_names".to_owned());
    }

    let mut summary = json!({
        "schema_version":"ForgeCADMcpToolManifestSummary@1",
        "build_cohort_sha256":build_cohort_sha256(),
        "read_count":read_names.len(),
        "write_count":write_names.len(),
        "total_count":enabled_names.len(),
        "read_names":read_names.into_iter().collect::<Vec<_>>(),
        "write_names":write_names.into_iter().collect::<Vec<_>>(),
        "read_manifest_sha256":tool_manifest_hash(false),
        "write_enabled_manifest_sha256":tool_manifest_hash(true)
    });
    summary["canonical_sha256"] = Value::String(canonical_json_hash(&summary));
    Ok(summary)
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
        // An externally launched Runtime is intentionally dynamic too.  Codex
        // can spend more than one authenticated IPC request window reasoning
        // about an uploaded image before its first tool call.  Holding an
        // authenticated client open here would let the Runtime time it out,
        // then turn that first real write into RUNTIME_UNAVAILABLE.
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
            _ => match supervisor::runtime_data_root() {
                Ok(data_root) => match MvpSupervisor::new(supervisor::runtime_command(), data_root)
                {
                    Ok(mut runtime_supervisor) => {
                        // Always keep the default path dynamic. A probe client
                        // must be dropped immediately so one MCP adapter never
                        // monopolizes Runtime's sequential request connection.
                        let backend = Backend::DynamicIpc(DynamicIpcBackend::from_ready_file(
                            runtime_supervisor.ready_file().to_path_buf(),
                            Some(runtime_supervisor.status_file().to_path_buf()),
                        ));
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
            session,
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

fn mcp010c_write_tool_names() -> Vec<String> {
    [
        "reference_compare_prepare",
        "visual_review_submit",
        "human_visual_review_submit",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn mcp010f_write_tool_names() -> Vec<String> {
    [
        "reference_mask_prepare",
        "reference_mask_refine_prepare",
        "primary_form_repair_prepare",
    ]
        .into_iter()
        .map(str::to_owned)
        .collect()
}

fn agentic_write_tool_names() -> Vec<String> {
    agentic_write_tools::write_tool_names()
}

fn agentic_action_write_tool_names() -> Vec<String> {
    agentic_action_tools::write_tool_names()
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

fn is_mcp010c_write_tool(name: &str) -> bool {
    mcp010c_write_tool_names().iter().any(|tool| tool == name)
}

fn is_mcp010f_write_tool(name: &str) -> bool {
    mcp010f_write_tool_names().iter().any(|tool| tool == name)
}

fn is_write_tool(name: &str) -> bool {
    is_mcp004_write_tool(name)
        || is_mcp005_write_tool(name)
        || is_mcp007_write_tool(name)
        || is_mcp008_write_tool(name)
        || is_mcp009_write_tool(name)
        || is_mcp010c_write_tool(name)
        || is_mcp010f_write_tool(name)
        || agentic_write_tools::is_write_tool(name)
        || agentic_action_tools::is_write_tool(name)
}

fn all_write_tool_names() -> Vec<String> {
    let mut names = mcp004_write_tool_names();
    names.extend(mcp005_write_tool_names());
    names.extend(mcp007_write_tool_names());
    names.extend(mcp008_write_tool_names());
    names.extend(mcp009_write_tool_names());
    names.extend(mcp010c_write_tool_names());
    names.extend(mcp010f_write_tool_names());
    names.extend(agentic_write_tool_names());
    names.extend(agentic_action_write_tool_names());
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
        tools.extend(mcp010c_write_tools());
        tools.extend(mcp010f_write_tools());
        tools.extend(agentic_write_tools::write_tools());
        tools.extend(agentic_action_tools::write_tools());
    }
    tools.sort_by(|left, right| left["name"].as_str().cmp(&right["name"].as_str()));
    tools
}

fn read_only_tools() -> Vec<Value> {
    let mut tools = vec![
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
            "geometry_program_hash",
            "Validate a hash-free GeometryProgram@2 draft and return the Runtime-owned canonical hash without compiling or persisting a candidate",
            json!({
                "type":"object",
                "additionalProperties":false,
                "required":["schema_version","geometry_program_draft"],
                "properties":{
                    "schema_version":{"const":"GeometryProgramHashRequest@1"},
                    "geometry_program_draft":{"type":"object"}
                }
            }),
            true,
        ),
        tool(
            "silhouette_rig_hash",
            "Validate a hash-free SilhouetteRig@1 draft and return the Runtime-owned canonical hash without persisting anything",
            json!({
                "type":"object",
                "additionalProperties":false,
                "required":["schema_version","project_id","candidate_id","rig_draft"],
                "properties":{
                    "schema_version":{"const":"SilhouetteRigHashRequest@1"},
                    "project_id":id_property(),
                    "candidate_id":id_property(),
                    "rig_draft":{
                        "type":"object",
                        "additionalProperties":false,
                        "required":["schema_version","rig_id","candidate_id","parameters"],
                        "properties":{
                            "schema_version":{"const":"SilhouetteRig@1"},
                            "rig_id":id_property(),
                            "candidate_id":id_property(),
                            "parameters":{
                                "type":"array",
                                "minItems":1,
                                "maxItems":64,
                                "items":{
                                    "type":"object",
                                    "additionalProperties":false,
                                    "required":["parameter_id","part_id","semantic","value","min","max","step","unit"],
                                    "properties":{
                                        "parameter_id":id_property(),
                                        "part_id":id_property(),
                                        "semantic":{"enum":["width","height","depth","offset_x","offset_y","offset_z","scale"]},
                                        "value":{"type":"number"},
                                        "min":{"type":"number"},
                                        "max":{"type":"number"},
                                        "step":{"type":"number","minimum":0},
                                        "unit":{"enum":["meter","ratio"]}
                                    }
                                }
                            }
                        }
                    }
                }
            }),
            true,
        ),
        tool(
            "boundary_error_get",
            "Read candidate-bound directional silhouette boundary errors against a prepared reference target",
            json!({
                "type":"object",
                "required":["candidate_id","target_sha256"],
                "properties":{
                    "candidate_id":id_property(),
                    "target_sha256":sha256_property(),
                    "max_segments":{"type":"integer","minimum":1,"maximum":64}
                },
                "additionalProperties":false
            }),
            true,
        ),
        tool(
            "silhouette_part_error_get",
            "Read candidate-bound per-Part contour error, local envelope ratios and bounded repair priority",
            json!({
                "type":"object",
                "required":["project_id","candidate_id","target_sha256"],
                "properties":{
                    "project_id":id_property(),
                    "candidate_id":id_property(),
                    "target_sha256":sha256_property()
                },
                "additionalProperties":false
            }),
            true,
        ),
        tool(
            "camera_fit_prepare",
            "Search a small deterministic camera neighborhood using the reference silhouette target and candidate silhouette; this only returns camera evidence and never changes the candidate.",
            json!({
                "type":"object",
                "required":["project_id","candidate_id","target_sha256"],
                "properties":{
                    "project_id":id_property(),
                    "candidate_id":id_property(),
                    "target_sha256":sha256_property(),
                    "camera":{"type":["object","null"]}
                },
                "additionalProperties":false
            }),
            true,
        ),
        tool(
            "silhouette_fit_prepare",
            "Run a bounded deterministic camera and SilhouetteRig fit proposal against the reference mask; when a geometry trial strictly improves the authored baseline, return the exact Runtime-validated GeometryProgram proposal for a later user-approved geometry_prepare call. It never mutates or confirms the candidate.",
            json!({
                "type":"object",
                "required":["project_id","candidate_id","target_sha256","rig","base_camera","optimizer","canonical_sha256"],
                "properties":{
                    "project_id":id_property(),"candidate_id":id_property(),"target_sha256":sha256_property(),
                    "rig":{"type":"object"},"base_camera":{"type":"object"},
                    "optimizer":{"type":"object","required":["algorithm","max_iterations","max_evaluations","step_fraction"],"properties":{"algorithm":{"enum":["grid","coordinate_descent"]},"max_iterations":{"type":"integer","minimum":1,"maximum":8},"max_evaluations":{"type":"integer","minimum":1,"maximum":64},"step_fraction":{"type":"number","minimum":0.000001,"maximum":0.5}},"additionalProperties":false},
                    "canonical_sha256":sha256_property()
                },
                "additionalProperties":false
            }),
            true,
        ),
        tool(
            "part_contour_fit_prepare",
            "Return a bounded one-Part contour adjustment proposal from candidate-bound render evidence without changing the candidate.",
            json!({"type":"object","required":["project_id","candidate_id","target_sha256","part_id","rig"],"properties":{"project_id":id_property(),"candidate_id":id_property(),"target_sha256":sha256_property(),"part_id":id_property(),"rig":{"type":"object"}},"additionalProperties":false}),
            true,
        ),
        tool(
            "silhouette_candidate_compare",
            "Compare 2 to 8 candidate silhouettes against one immutable target and return the best bounded evidence.",
            json!({"type":"object","required":["project_id","target_sha256","candidate_ids"],"properties":{"project_id":id_property(),"target_sha256":sha256_property(),"candidate_ids":{"type":"array","minItems":2,"maxItems":8,"items":id_property()}},"additionalProperties":false}),
            true,
        ),
        tool(
            "operator_catalog_get",
            "Read the closed Runtime-owned OperatorCatalog@1 used to validate GeometryProgram@2 drafts",
            json!({"type":"object","additionalProperties":false}),
            true,
        ),
        tool(
            "material_pack_get",
            "Read the immutable offline forgecad-hard-surface-robot MaterialPack manifest, texture hashes and color-space rules",
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
            "render_pass_get",
            "Read one immutable 512x512 fixed-render PNG pass as an MCP image block; rendering is performed only by reference_compare_prepare",
            json!({
                "type":"object",
                "required":["render_set_hash","pass"],
                "properties":{
                    "render_set_hash":sha256_property(),
                    "pass":{"enum":["beauty","silhouette","depth","normal","ao","part-id","material-id","wireframe","uv-stretch"]}
                },
                "additionalProperties":false
            }),
            true,
        ),
        tool(
            "selection_get",
            "Read the ephemeral Viewer selection projection",
            json!({"type":"object","additionalProperties":false}),
            true,
        ),
        tool(
            "silhouette_target_get",
            "Read a hash-bound 512x512 reference silhouette target without returning original image bytes",
            json!({
                "type":"object",
                "required":["target_sha256"],
                "properties":{"target_sha256":sha256_property()},
                "additionalProperties":false
            }),
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
            "Read a first-party development Skill bundle manifest and its checked-in knowledge. This exact tool must first read ponytail-preflight@0.1.0 before any ForgeCAD design tool or another Skill.",
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
    ];
    tools.extend(agentic_tools::read_tools());
    tools.extend(agentic_write_tools::read_tools());
    tools.extend(agentic_action_tools::read_tools());
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
        "Compile a bounded typed GeometryProgram into a multi-part GLB candidate. First read ponytail-preflight@0.1.0 with skill_get in this MCP session. GeometryProgram@2 is catalog-hash-bound and returns strict BIN/accessor ArtifactReadback@2; GeometryProgram@1 remains the legacy-compatible MVP path. Read forgecad://operators/catalog before using V2. No permanent version is created until a later approval confirm.",
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

fn mcp010c_write_tools() -> Vec<Value> {
    vec![
        write_tool_with_transaction(
            "reference_compare_prepare",
            "Render a candidate with the fixed 512x512 perspective renderer, persist all nine AOVs, derive a local reference mask and comparison metrics, and return a reviewable visual evidence bundle without creating a version.",
            json!({
                "type":"object",
                "required":["project_id","candidate_id","reference_id","view_spec"],
                "properties":{
                    "project_id":id_property(),
                    "candidate_id":id_property(),
                    "reference_id":id_property(),
                    "view_spec":{"type":"object"},
                    "camera":{"type":["object","null"]},
                    "target_sha256":sha256_property()
                },
                "additionalProperties":false
            }),
            false,
            true,
            "MCP010C",
        ),
        write_tool_with_transaction(
            "visual_review_submit",
            "Persist Codex typed visual issues and a bounded stage review against one candidate-bound RenderSet and comparison report.",
            json!({
                "type":"object",
                "required":["candidate_id","reference_id","render_set_hash","comparison_report_hash","round","stage","issues","status"],
                "properties":{
                    "candidate_id":id_property(),
                    "reference_id":id_property(),
                    "render_set_hash":sha256_property(),
                    "comparison_report_hash":sha256_property(),
                    "round":{"type":"integer","minimum":1,"maximum":5},
                    "stage":{"enum":["silhouette","structure","form","material-surface","final"]},
                    "issues":{"type":"array","items":{"type":"object"},"maxItems":128},
                    "status":{"enum":["submitted","needs_revision","accepted"]}
                },
                "additionalProperties":false
            }),
            false,
            true,
            "MCP010C",
        ),
        write_tool_with_transaction(
            "human_visual_review_submit",
            "Record the user's four visual scores and approval against the candidate-bound RenderSet and comparison report; this is evidence only and does not confirm a version.",
            json!({
                "type":"object",
                "required":["candidate_id","reference_id","render_set_hash","comparison_report_hash","scores","approved"],
                "properties":{
                    "candidate_id":id_property(),
                    "reference_id":id_property(),
                    "render_set_hash":sha256_property(),
                    "comparison_report_hash":sha256_property(),
                    "scores":{"type":"object","required":["likeness","geometry_detail","material_fidelity","editability"],"properties":{"likeness":{"type":"integer","minimum":1,"maximum":5},"geometry_detail":{"type":"integer","minimum":1,"maximum":5},"material_fidelity":{"type":"integer","minimum":1,"maximum":5},"editability":{"type":"integer","minimum":1,"maximum":5}},"additionalProperties":false},
                    "approved":{"type":"boolean"}
                },
                "additionalProperties":false
            }),
            true,
            true,
            "MCP010C",
        ),
    ]
}

fn mcp010f_write_tools() -> Vec<Value> {
    vec![
        write_tool_with_transaction(
            "reference_mask_prepare",
            "Create a hash-bound 512x512 silhouette target from a user-authorized reference; an optional Codex contour is rasterized deterministically and no model candidate is created.",
            json!({
                "type":"object",
                "required":["project_id","reference_id"],
                "properties":{
                    "project_id":id_property(),
                    "reference_id":id_property(),
                    "contour_points":{"type":["array","null"],"maxItems":512,"items":{"type":"array","minItems":2,"maxItems":2,"items":{"type":"number","minimum":0,"maximum":1}}},
                    "landmarks":{"type":["array","null"],"maxItems":128,"items":{"type":"object"}},
                    "parts":{"type":["array","null"],"maxItems":64,"items":{"type":"object"}}
                },
                "additionalProperties":false
            }),
            false,
            false,
            "MCP010F",
        ),
        write_tool_with_transaction(
            "reference_mask_refine_prepare",
            "Create a new immutable silhouette target by refining an existing target with a bounded normalized contour; the source target remains unchanged.",
            json!({
                "type":"object",
                "required":["project_id","base_target_sha256","contour_points"],
                "properties":{
                    "project_id":id_property(),
                    "base_target_sha256":sha256_property(),
                    "contour_points":{"type":"array","minItems":3,"maxItems":512,"items":{"type":"array","minItems":2,"maxItems":2,"items":{"type":"number","minimum":0,"maximum":1}}},
                    "landmarks":{"type":["array","null"],"maxItems":128,"items":{"type":"object"}},
                    "parts":{"type":["array","null"],"maxItems":64,"items":{"type":"object"}}
                },
                "additionalProperties":false
            }),
            false,
            false,
            "MCP010F",
        ),
        write_tool_with_transaction(
            "primary_form_repair_prepare",
            "Run one Runtime-owned bounded Primary Form repair: fit continuous parameters, compile the winning typed GeometryProgram, perform strict readback, render fixed nine-AOV evidence through the isolated Render Worker, and evaluate the staged candidate against the same target. It never confirms a version or exports.",
            json!({
                "type":"object",
                "required":["project_id","candidate_id","target_sha256","rig","base_camera","optimizer","canonical_sha256"],
                "properties":{
                    "project_id":id_property(),
                    "candidate_id":id_property(),
                    "target_sha256":sha256_property(),
                    "rig":{"type":"object"},
                    "base_camera":{"type":"object"},
                    "optimizer":{"type":"object","required":["algorithm","max_iterations","max_evaluations","step_fraction"],"properties":{"algorithm":{"enum":["grid","coordinate_descent"]},"max_iterations":{"type":"integer","minimum":1,"maximum":8},"max_evaluations":{"type":"integer","minimum":1,"maximum":64},"step_fraction":{"type":"number","minimum":0.000001,"maximum":0.5}},"additionalProperties":false},
                    "base_version_id":nullable_id_property(),
                    "canonical_sha256":sha256_property()
                },
                "additionalProperties":false
            }),
            false,
            true,
            "MCP010F",
        ),
    ]
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
    vec![
        json!({
            "uri":"forgecad://capabilities",
            "name":"Runtime capabilities",
            "description":"Static MCP and Runtime health capability manifest",
            "mime_type":"application/json",
            "schema_version":"RuntimeResource@1",
            "read_only":true
        }),
        json!({
            "uri":"forgecad://operators/catalog",
            "name":"Geometry operator catalog",
            "description":"Runtime-owned GeometryProgram@2 operator catalog; requires a live Runtime",
            "mime_type":"application/json",
            "schema_version":"RuntimeResource@1",
            "read_only":true
        }),
    ]
}

// `tools/list` is a public contract, so stdio must enforce the same bounded
// input envelopes it advertises before forwarding a request to Runtime.  This
// is intentionally a small, fail-closed JSON Schema subset rather than a
// general-purpose schema engine: it covers every keyword used by the current
// tool schemas and has explicit recursion/work bounds.
const MAX_TOOL_SCHEMA_VALIDATION_DEPTH: usize = 32;
const MAX_TOOL_SCHEMA_VALIDATION_NODES: usize = 4_096;

#[derive(Clone, Copy)]
struct ToolSchemaValidationBudget {
    remaining_nodes: usize,
}

impl ToolSchemaValidationBudget {
    fn new() -> Self {
        Self {
            remaining_nodes: MAX_TOOL_SCHEMA_VALIDATION_NODES,
        }
    }

    fn consume(&mut self, depth: usize) -> Result<(), ()> {
        if depth > MAX_TOOL_SCHEMA_VALIDATION_DEPTH || self.remaining_nodes == 0 {
            return Err(());
        }
        self.remaining_nodes -= 1;
        Ok(())
    }
}

fn validate_tools_call_envelope(params: &Map<String, Value>) -> Result<(), ()> {
    if !params
        .keys()
        .all(|key| matches!(key.as_str(), "name" | "arguments" | "_meta"))
    {
        return Err(());
    }
    if !params
        .get("name")
        .and_then(Value::as_str)
        .is_some_and(|name| !name.is_empty())
    {
        return Err(());
    }
    if params
        .get("arguments")
        .is_some_and(|value| !value.is_object())
    {
        return Err(());
    }
    if params.get("_meta").is_some_and(|value| !value.is_object()) {
        return Err(());
    }
    Ok(())
}

fn validate_declared_tool_input(
    name: &str,
    arguments: &Value,
    write_tools_enabled: bool,
) -> Result<(), ()> {
    let tools = tools_with_writes(write_tools_enabled);
    let schema = tools
        .iter()
        .find(|tool| tool.get("name").and_then(Value::as_str) == Some(name))
        .and_then(|tool| tool.get("inputSchema"))
        .ok_or(())?;
    let mut schema_budget = ToolSchemaValidationBudget::new();
    validate_tool_schema_shape(schema, 0, &mut schema_budget)?;
    let mut value_budget = ToolSchemaValidationBudget::new();
    validate_value_against_tool_schema(schema, arguments, 0, &mut value_budget)
}

fn validate_tool_schema_shape(
    schema: &Value,
    depth: usize,
    budget: &mut ToolSchemaValidationBudget,
) -> Result<(), ()> {
    budget.consume(depth)?;
    let object = schema.as_object().ok_or(())?;
    if !object.keys().all(|key| {
        matches!(
            key.as_str(),
            "type"
                | "required"
                | "properties"
                | "additionalProperties"
                | "oneOf"
                | "const"
                | "enum"
                | "minLength"
                | "maxLength"
                | "pattern"
                | "minimum"
                | "maximum"
                | "maxProperties"
                | "items"
                | "minItems"
                | "maxItems"
        )
    }) {
        return Err(());
    }
    if let Some(value) = object.get("type") {
        validate_schema_type_shape(value)?;
    }
    if let Some(value) = object.get("required") {
        let values = value.as_array().ok_or(())?;
        if !values.iter().all(Value::is_string) {
            return Err(());
        }
    }
    if let Some(value) = object.get("properties") {
        let properties = value.as_object().ok_or(())?;
        for property_schema in properties.values() {
            validate_tool_schema_shape(property_schema, depth + 1, budget)?;
        }
    }
    if let Some(value) = object.get("additionalProperties") {
        if !value.is_boolean() {
            return Err(());
        }
    }
    if let Some(value) = object.get("oneOf") {
        let alternatives = value
            .as_array()
            .filter(|items| !items.is_empty())
            .ok_or(())?;
        for alternative in alternatives {
            validate_tool_schema_shape(alternative, depth + 1, budget)?;
        }
    }
    if let Some(value) = object.get("enum") {
        if value.as_array().filter(|items| !items.is_empty()).is_none() {
            return Err(());
        }
    }
    for key in [
        "minLength",
        "maxLength",
        "maxProperties",
        "minItems",
        "maxItems",
    ] {
        if object
            .get(key)
            .is_some_and(|value| schema_usize(value).is_err())
        {
            return Err(());
        }
    }
    if object
        .get("minimum")
        .is_some_and(|value| value.as_f64().is_none())
    {
        return Err(());
    }
    if object
        .get("maximum")
        .is_some_and(|value| value.as_f64().is_none())
    {
        return Err(());
    }
    if let Some(value) = object.get("pattern") {
        if value.as_str() != Some("^[0-9a-f]{64}$") {
            return Err(());
        }
    }
    if let Some(value) = object.get("items") {
        validate_tool_schema_shape(value, depth + 1, budget)?;
    }
    Ok(())
}

fn validate_value_against_tool_schema(
    schema: &Value,
    value: &Value,
    depth: usize,
    budget: &mut ToolSchemaValidationBudget,
) -> Result<(), ()> {
    budget.consume(depth)?;
    let object = schema.as_object().ok_or(())?;
    if let Some(schema_type) = object.get("type") {
        validate_schema_type(schema_type, value)?;
    }
    if let Some(expected) = object.get("const") {
        if value != expected {
            return Err(());
        }
    }
    if let Some(values) = object.get("enum").and_then(Value::as_array) {
        if !values.iter().any(|expected| value == expected) {
            return Err(());
        }
    }
    if let Some(required) = object.get("required").and_then(Value::as_array) {
        let values = value.as_object().ok_or(())?;
        if !required
            .iter()
            .all(|key| key.as_str().is_some_and(|key| values.contains_key(key)))
        {
            return Err(());
        }
    }
    if let Some(maximum) = object.get("maxProperties") {
        let values = value.as_object().ok_or(())?;
        if values.len() > schema_usize(maximum)? {
            return Err(());
        }
    }
    if let Some(properties) = object.get("properties").and_then(Value::as_object) {
        if let Some(values) = value.as_object() {
            if object.get("additionalProperties") == Some(&Value::Bool(false))
                && values.keys().any(|key| !properties.contains_key(key))
            {
                return Err(());
            }
            for (key, property_schema) in properties {
                if let Some(property_value) = values.get(key) {
                    validate_value_against_tool_schema(
                        property_schema,
                        property_value,
                        depth + 1,
                        budget,
                    )?;
                }
            }
        } else if object.get("additionalProperties") == Some(&Value::Bool(false)) {
            return Err(());
        }
    } else if object.get("additionalProperties") == Some(&Value::Bool(false)) {
        if value.as_object().map_or(true, |values| !values.is_empty()) {
            return Err(());
        }
    }
    if let Some(minimum) = object.get("minimum").and_then(Value::as_f64) {
        if value
            .as_f64()
            .filter(|candidate| *candidate >= minimum)
            .is_none()
        {
            return Err(());
        }
    }
    if let Some(maximum) = object.get("maximum").and_then(Value::as_f64) {
        if value
            .as_f64()
            .filter(|candidate| *candidate <= maximum)
            .is_none()
        {
            return Err(());
        }
    }
    if let Some(string) = value.as_str() {
        let character_count = string.chars().count();
        if let Some(minimum) = object.get("minLength") {
            if character_count < schema_usize(minimum)? {
                return Err(());
            }
        }
        if let Some(maximum) = object.get("maxLength") {
            if character_count > schema_usize(maximum)? {
                return Err(());
            }
        }
        if let Some(pattern) = object.get("pattern").and_then(Value::as_str) {
            if pattern != "^[0-9a-f]{64}$" || !is_lowercase_sha256(string) {
                return Err(());
            }
        }
    }
    if let Some(items) = object.get("items") {
        let values = value.as_array().ok_or(())?;
        if let Some(minimum) = object.get("minItems") {
            if values.len() < schema_usize(minimum)? {
                return Err(());
            }
        }
        if let Some(maximum) = object.get("maxItems") {
            if values.len() > schema_usize(maximum)? {
                return Err(());
            }
        }
        for item in values {
            validate_value_against_tool_schema(items, item, depth + 1, budget)?;
        }
    }
    if let Some(alternatives) = object.get("oneOf").and_then(Value::as_array) {
        let matches = alternatives
            .iter()
            .filter(|alternative| {
                let mut alternative_budget = *budget;
                validate_value_against_tool_schema(
                    alternative,
                    value,
                    depth + 1,
                    &mut alternative_budget,
                )
                .is_ok()
            })
            .count();
        if matches != 1 {
            return Err(());
        }
    }
    Ok(())
}

fn validate_schema_type_shape(schema_type: &Value) -> Result<(), ()> {
    match schema_type {
        Value::String(kind) => validate_schema_type_name(kind),
        Value::Array(kinds) if !kinds.is_empty() => {
            for kind in kinds {
                validate_schema_type_name(kind.as_str().ok_or(())?)?;
            }
            Ok(())
        }
        _ => Err(()),
    }
}

fn validate_schema_type(schema_type: &Value, value: &Value) -> Result<(), ()> {
    match schema_type {
        Value::String(kind) if value_matches_schema_type(kind, value) => Ok(()),
        Value::Array(kinds)
            if kinds.iter().any(|kind| {
                kind.as_str()
                    .is_some_and(|kind| value_matches_schema_type(kind, value))
            }) =>
        {
            Ok(())
        }
        _ => Err(()),
    }
}

fn validate_schema_type_name(kind: &str) -> Result<(), ()> {
    if matches!(
        kind,
        "object" | "array" | "string" | "number" | "integer" | "boolean" | "null"
    ) {
        Ok(())
    } else {
        Err(())
    }
}

fn value_matches_schema_type(kind: &str, value: &Value) -> bool {
    match kind {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => {
            value.as_i64().is_some()
                || value.as_u64().is_some()
                || value
                    .as_f64()
                    .is_some_and(|number| number.is_finite() && number.fract() == 0.0)
        }
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        _ => false,
    }
}

fn schema_usize(value: &Value) -> Result<usize, ()> {
    value
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(())
}

fn is_lowercase_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn call_tool(
    backend: &mut Backend,
    id: Option<Value>,
    params: Option<&Value>,
    session: &mut Session,
) -> Option<Value> {
    let write_tools_enabled = session.write_tools_enabled;
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
    if (agentic_write_tools::is_tool(name) || agentic_action_tools::is_tool(name))
        && !session.ponytail_preflight_read
    {
        return Some(json!({
            "jsonrpc":"2.0",
            "id":id,
            "result":{
                "isError":true,
                "content":[{"type":"text","text":serde_json::to_string(&runtime_error_value(PONYTAIL_PREFLIGHT_REQUIRED)).unwrap_or_else(|_| "{}".to_owned())}],
                "structuredContent":runtime_error_value(PONYTAIL_PREFLIGHT_REQUIRED)
            }
        }));
    }
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
        if is_mcp010c_write_tool(name) {
            return Some(json!({
                "jsonrpc":"2.0",
                "id":id,
                "result":{
                    "isError":true,
                    "content":[{"type":"text","text":serde_json::to_string(&runtime_error_value("MCP010C_VISUAL_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")).unwrap_or_else(|_| "{}".to_owned())}],
                    "structuredContent":runtime_error_value("MCP010C_VISUAL_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")
                }
            }));
        }
        if agentic_write_tools::is_write_tool(name) || agentic_action_tools::is_write_tool(name) {
            return Some(json!({
                "jsonrpc":"2.0",
                "id":id,
                "result":{
                    "isError":true,
                    "content":[{"type":"text","text":serde_json::to_string(&runtime_error_value("AGENTIC_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")).unwrap_or_else(|_| "{}".to_owned())}],
                    "structuredContent":runtime_error_value("AGENTIC_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")
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
    // Preserve the historic disabled-write response above: it is an
    // availability boundary, not a schema oracle.  For an exposed tool, all
    // declared envelope checks run before Runtime dispatch or any write.
    if validate_tools_call_envelope(params).is_err() {
        return Some(
            error_response(
                Some(id),
                -32602,
                "Tool call does not match the declared envelope",
                Some(json!({"code":"INVALID_TOOL_PARAMS"})),
            )
            .expect("response for request"),
        );
    }
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if validate_declared_tool_input(name, &arguments, write_tools_enabled).is_err() {
        return Some(
            error_response(
                Some(id),
                -32602,
                "Tool arguments do not match the declared input schema",
                Some(json!({"code":"INVALID_TOOL_PARAMS"})),
            )
            .expect("response for request"),
        );
    }
    if requires_ponytail_preflight(name, &arguments) && !session.ponytail_preflight_read {
        return Some(json!({
            "jsonrpc":"2.0",
            "id":id,
            "result":{
                "isError":true,
                "content":[{"type":"text","text":serde_json::to_string(&runtime_error_value(PONYTAIL_PREFLIGHT_REQUIRED)).unwrap_or_else(|_| "{}".to_owned())}],
                "structuredContent":runtime_error_value(PONYTAIL_PREFLIGHT_REQUIRED)
            }
        }));
    }
    if agentic_write_tools::is_tool(name) {
        if let Err(error) = agentic_write_tools::validate_call(
            name,
            &arguments,
            &session.agentic_binding,
        ) {
            return Some(json!({
                "jsonrpc":"2.0",
                "id":id,
                "result":{"isError":true,"content":[{"type":"text","text":serde_json::to_string(&runtime_error_value(&error)).unwrap_or_else(|_| "{}".to_owned())}],"structuredContent":runtime_error_value(&error)}
            }));
        }
    }
    if agentic_action_tools::is_tool(name) {
        if let Err(error) = agentic_action_tools::validate_call(
            name,
            &arguments,
            &session.agentic_action_binding,
        ) {
            return Some(json!({
                "jsonrpc":"2.0",
                "id":id,
                "result":{"isError":true,"content":[{"type":"text","text":serde_json::to_string(&runtime_error_value(&error)).unwrap_or_else(|_| "{}".to_owned())}],"structuredContent":runtime_error_value(&error)}
            }));
        }
    }
    match dispatch_tool(backend, name, &arguments, write_tools_enabled) {
        Ok(value) if name == "render_pass_get" => {
            let mut metadata = value.clone();
            let Some(png_base64) = metadata
                .as_object_mut()
                .and_then(|object| object.remove("png_base64"))
                .and_then(|value| value.as_str().map(str::to_owned))
            else {
                return Some(json!({
                    "jsonrpc":"2.0",
                    "id":id,
                    "result":{"isError":true,"content":[{"type":"text","text":"Runtime returned an invalid render pass"}],"structuredContent":runtime_error_value("RENDER_PASS_INVALID: PNG payload is missing")}
                }));
            };
            Some(json!({
                "jsonrpc":"2.0",
                "id":id,
                "result":{"content":[{"type":"image","data":png_base64,"mimeType":"image/png"},{"type":"text","text":serde_json::to_string(&metadata).unwrap_or_else(|_| "{}".to_owned())}],"structuredContent":metadata}
            }))
        }
        Ok(value) => {
            if agentic_write_tools::is_tool(name) {
                if let Err(error) = agentic_write_tools::bind_response(
                    name,
                    &value,
                    &mut session.agentic_binding,
                ) {
                    return Some(json!({
                        "jsonrpc":"2.0",
                        "id":id,
                        "result":{"isError":true,"content":[{"type":"text","text":serde_json::to_string(&runtime_error_value(&error)).unwrap_or_else(|_| "{}".to_owned())}],"structuredContent":runtime_error_value(&error)}
                    }));
                }
                agentic_action_tools::sync_session_scope(
                    &session.agentic_binding,
                    &mut session.agentic_action_binding,
                );
            }
            if agentic_action_tools::is_tool(name) {
                if let Err(error) = agentic_action_tools::bind_response(
                    name,
                    &value,
                    &mut session.agentic_action_binding,
                ) {
                    return Some(json!({
                        "jsonrpc":"2.0",
                        "id":id,
                        "result":{"isError":true,"content":[{"type":"text","text":serde_json::to_string(&runtime_error_value(&error)).unwrap_or_else(|_| "{}".to_owned())}],"structuredContent":runtime_error_value(&error)}
                    }));
                }
            }
            if is_ponytail_preflight_read(name, &arguments) {
                session.ponytail_preflight_read = true;
            }
            Some(json!({
                "jsonrpc":"2.0",
                "id":id,
                "result":{"content":[{"type":"text","text":serde_json::to_string(&value).unwrap_or_else(|_| "{}".to_owned())}],"structuredContent":value}
            }))
        }
        Err(error) => Some(json!({
            "jsonrpc":"2.0",
            "id":id,
            "result":{"isError":true,"content":[{"type":"text","text":serde_json::to_string(&runtime_error_value(&error)).unwrap_or_else(|_| "{}".to_owned())}],"structuredContent":runtime_error_value(&error)}
        })),
    }
}

fn is_ponytail_preflight_read(name: &str, arguments: &Value) -> bool {
    name == "skill_get"
        && arguments.get("skill_id").and_then(Value::as_str) == Some(PONYTAIL_PREFLIGHT_SKILL_ID)
        && arguments.get("version").and_then(Value::as_str) == Some(PONYTAIL_PREFLIGHT_VERSION)
}

fn requires_ponytail_preflight(name: &str, arguments: &Value) -> bool {
    !matches!(name, "capabilities_get" | "runtime_status" | "doctor")
        && !is_ponytail_preflight_read(name, arguments)
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
        if agentic_write_tools::is_write_tool(name) {
            return Err(
                "AGENTIC_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required"
                    .to_owned(),
            );
        }
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
        } else if is_mcp010c_write_tool(name) {
            "MCP010C_VISUAL_TOOLS_DISABLED: explicit authenticated IPC opt-in is required"
                .to_owned()
        } else if is_mcp010f_write_tool(name) {
            "MCP010F_PRIMARY_FORM_TOOLS_DISABLED: explicit authenticated IPC opt-in is required"
                .to_owned()
        } else {
            "MCP004_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required".to_owned()
        });
    }
    if is_write_tool(name) {
        if agentic_write_tools::is_write_tool(name) || agentic_action_tools::is_write_tool(name) {
            return backend_agentic_write_call(backend, name, arguments, local_build_cohort);
        }
        return backend_write_call(backend, name, arguments, local_build_cohort);
    }
    if agentic_write_tools::is_tool(name) || agentic_action_tools::is_tool(name) {
        return match agentic_write_tools::runtime_method(name) {
            Some(runtime_method) => backend_call(backend, runtime_method, arguments),
            None => match agentic_action_tools::runtime_method(name) {
                Some(runtime_method) => backend_call(backend, runtime_method, arguments),
                None => Err(agentic_action_tools::unavailable_error(name)),
            },
        };
    }
    if agentic_tools::is_tool(name) {
        return match agentic_tools::runtime_method(name) {
            Some(runtime_method) => backend_call(backend, runtime_method, arguments),
            None => Err(agentic_tools::unavailable_error(name)),
        };
    }
    match name {
        "capabilities_get" => capabilities_payload(backend, write_tools_enabled),
        "runtime_status" => runtime_status_payload(backend),
        "doctor" => doctor_payload(backend),
        "version_diff" | "quality_get" => backend_call(backend, name, arguments),
        _ => backend_call(backend, name, arguments),
    }
}

fn backend_agentic_write_call(
    backend: &mut Backend,
    name: &str,
    arguments: &Value,
    local_build_cohort: Option<&str>,
) -> Result<Value, String> {
    backend_write_call(backend, name, arguments, local_build_cohort).map_err(|error| {
        if error == "RUNTIME_UNAVAILABLE: Runtime request failed" {
            return if agentic_action_tools::is_tool(name) {
                agentic_action_tools::unavailable_error(name)
            } else {
                agentic_write_tools::unavailable_error(name)
            };
        }
        if error.starts_with("RUNTIME_UNAVAILABLE:") {
            return format!("AGENTIC_RUNTIME_UNAVAILABLE: {error}");
        }
        error
    })
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
            let endpoint = dynamic.endpoint()?;
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
            if let Some(agentic_code) = detail
                .split(':')
                .map(str::trim)
                .find(|value| value.starts_with("AGENTIC_"))
            {
                return format!("{agentic_code}: Runtime Agentic request rejected");
            }
            let code = detail.split(':').next().unwrap_or(detail.as_str()).trim();
            match code {
                "INVALID_INPUT" => {
                    // Keep the adapter free of user payloads and local paths,
                    // but preserve the stable stage code when Runtime has
                    // one.  Codex can then repair a fit envelope (or stop on
                    // a rejected quality gate) without guessing from the
                    // generic INVALID_INPUT bucket.
                    let stage = detail
                        .split(':')
                        .map(str::trim)
                        .find(|value| value.starts_with("AGENTIC_") || value.starts_with("GEOMETRY_PROGRAM_HASH_REJECTED") || value.starts_with("SILHOUETTE_") || value.starts_with("CAMERA_") || value.starts_with("PRIMARY_FORM_REPAIR_") || value.starts_with("CONTRACT_OUTPUT_INVALID"))
                        .unwrap_or("");
                    match stage {
                        "GEOMETRY_PROGRAM_HASH_REJECTED" => {
                            let reason = detail
                                .split_once("GEOMETRY_PROGRAM_HASH_REJECTED:")
                                .map(|(_, value)| value.trim())
                                .filter(|value| !value.is_empty())
                                .unwrap_or("request shape or worker contract validation")
                                .to_owned();
                            let reason = [
                                "request must be an object",
                                "request must contain only schema_version and geometry_program_draft",
                                "schema_version must be GeometryProgramHashRequest@1",
                                "GEOMETRY_WORKER_PROTOCOL",
                                "schema or project binding",
                            ]
                            .into_iter()
                            .find(|candidate| reason.starts_with(candidate))
                            .unwrap_or("request shape or worker contract validation");
                            format!("GEOMETRY_PROGRAM_HASH_REJECTED: Runtime geometry hash request rejected ({reason})")
                        }
                        _ if stage.starts_with("AGENTIC_") => {
                            let code = stage
                                .split_whitespace()
                                .next()
                                .unwrap_or("AGENTIC_RUNTIME_REJECTED");
                            format!("{code}: Runtime Agentic request rejected")
                        }
                        "SILHOUETTE_FIT_INVALID" => {
                            let reason = detail
                                .split_once("SILHOUETTE_FIT_INVALID:")
                                .map(|(_, value)| value.trim())
                                .unwrap_or("");
                            let reason = [
                                "canonical_sha256 does not bind intent",
                                "rig is required",
                                "base_camera is required",
                                "optimizer is required",
                                "unsupported optimizer",
                                "candidate not found",
                                "target not found",
                            ]
                            .into_iter()
                            .find(|candidate| reason.starts_with(candidate))
                            .unwrap_or("request shape or numeric canonicalization");
                            format!("SILHOUETTE_FIT_INVALID: Runtime fit intent rejected ({reason})")
                        }
                        "SILHOUETTE_FIT_REJECTED" => "SILHOUETTE_FIT_REJECTED: Runtime silhouette gate rejected the candidate".to_owned(),
                        "SILHOUETTE_FIT_RENDER_FAILED" => "SILHOUETTE_FIT_RENDER_FAILED: Runtime fit render failed".to_owned(),
                        "PRIMARY_FORM_REPAIR_INVALID" => {
                            let reason = detail
                                .split_once("PRIMARY_FORM_REPAIR_INVALID:")
                                .map(|(_, value)| value.trim())
                                .filter(|value| !value.is_empty())
                                .unwrap_or("request or target binding")
                                .to_owned();
                            let reason = [
                                "request must be an object",
                                "base_version_id argument is not bound to intent",
                                "canonical_sha256 does not bind intent",
                                "base_version_id must be an identifier or null",
                                "target landmarks are missing",
                            ]
                            .into_iter()
                            .find(|candidate| reason.starts_with(candidate))
                            .unwrap_or("request or target binding");
                            format!("PRIMARY_FORM_REPAIR_INVALID: Runtime Primary Form intent rejected ({reason})")
                        }
                        "PRIMARY_FORM_REPAIR_REJECTED" => "PRIMARY_FORM_REPAIR_REJECTED: Runtime Primary Form geometry proposal rejected".to_owned(),
                        "PRIMARY_FORM_REPAIR_CAS_HASH_MISMATCH" => "PRIMARY_FORM_REPAIR_CAS_HASH_MISMATCH: Runtime Primary Form staged GeometryProgram hash binding rejected".to_owned(),
                        "PRIMARY_FORM_REPAIR_CAS_INVALID_HASH" => "PRIMARY_FORM_REPAIR_CAS_INVALID_HASH: Runtime Primary Form staged hash is invalid".to_owned(),
                        "PRIMARY_FORM_REPAIR_CAS_CORRUPT" => "PRIMARY_FORM_REPAIR_CAS_CORRUPT: Runtime Primary Form staged CAS object is corrupt".to_owned(),
                        "PRIMARY_FORM_REPAIR_CAS_CAPACITY_EXCEEDED" => "PRIMARY_FORM_REPAIR_CAS_CAPACITY_EXCEEDED: Runtime Primary Form staged object exceeds the CAS limit".to_owned(),
                        "PRIMARY_FORM_REPAIR_CAS_UNSAFE_ROOT" => "PRIMARY_FORM_REPAIR_CAS_UNSAFE_ROOT: Runtime Primary Form CAS root is unsafe".to_owned(),
                        "PRIMARY_FORM_REPAIR_CAS_IO" => "PRIMARY_FORM_REPAIR_CAS_IO: Runtime Primary Form staged CAS file operation failed".to_owned(),
                        "SILHOUETTE_RIG_INVALID" => {
                            let reason = detail
                                .split_once("SILHOUETTE_RIG_INVALID:")
                                .map(|(_, value)| value.trim())
                                .filter(|value| !value.is_empty())
                                .unwrap_or("contract validation failed");
                            format!("SILHOUETTE_RIG_INVALID: {reason}")
                        }
                        "CAMERA_CALIBRATION_INVALID" => {
                            let reason = detail
                                .split_once("CAMERA_CALIBRATION_INVALID:")
                                .map(|(_, value)| value.trim())
                                .filter(|value| !value.is_empty())
                                .unwrap_or("contract validation failed");
                            format!("CAMERA_CALIBRATION_INVALID: {reason}")
                        }
                        "CONTRACT_OUTPUT_INVALID" => {
                            let reason = detail
                                .split_once("CONTRACT_OUTPUT_INVALID:")
                                .map(|(_, value)| value.trim())
                                .filter(|value| !value.is_empty())
                                .unwrap_or("contract validation failed");
                            format!("CONTRACT_OUTPUT_INVALID: {reason}")
                        }
                        "CANDIDATE_ARTIFACT_UNAVAILABLE" => "CANDIDATE_ARTIFACT_UNAVAILABLE: Runtime candidate has no readable artifact".to_owned(),
                        "SILHOUETTE_FIT_UNAVAILABLE" => "SILHOUETTE_FIT_UNAVAILABLE: Runtime fit could not produce an evaluation".to_owned(),
                        "SILHOUETTE_PART_ERROR_UNAVAILABLE" => "SILHOUETTE_PART_ERROR_UNAVAILABLE: Runtime Part contour evidence is unavailable".to_owned(),
                        "SILHOUETTE_PART_ERROR_INVALID" => {
                            let reason = detail
                                .split_once("SILHOUETTE_PART_ERROR_INVALID:")
                                .map(|(_, value)| value.trim())
                                .filter(|value| !value.is_empty())
                                .unwrap_or("Part contour evidence is invalid");
                            format!("SILHOUETTE_PART_ERROR_INVALID: {reason}")
                        }
                        "REFERENCE_BINDING_MISMATCH" => "REFERENCE_BINDING_MISMATCH: Runtime reference evidence is not bound to the candidate".to_owned(),
                        _ if detail.contains("SilhouettePartErrorResult@1") => {
                            "SILHOUETTE_PART_ERROR_INVALID: Runtime Part contour evidence failed its contract".to_owned()
                        }
                        _ if detail.contains("SILHOUETTE_PART_ERROR_UNAVAILABLE") => {
                            "SILHOUETTE_PART_ERROR_UNAVAILABLE: Runtime Part contour evidence is unavailable".to_owned()
                        }
                        _ => "INVALID_INPUT: Runtime request rejected".to_owned(),
                    }
                }
                "STORE_CONTRACT" => {
                    let code = detail
                        .split_once(':')
                        .map(|(_, value)| value.trim())
                        .filter(|value| !value.is_empty())
                        .unwrap_or("contract rejection");
                    format!("STORE_CONTRACT: Runtime store rejected the request ({code})")
                }
                "STORE_INVALID_DATA" => {
                    let reason = detail
                        .split_once(':')
                        .map(|(_, value)| value.trim())
                        .filter(|value| !value.is_empty())
                        .unwrap_or("record validation");
                    format!("STORE_INVALID_DATA: Runtime store rejected the record ({reason})")
                }
                "STORE_ERROR" => "STORE_ERROR: Runtime store rejected the request".to_owned(),
                "STORE_SQLITE" => "STORE_SQLITE: Runtime SQLite transaction rejected the request".to_owned(),
                "STORE_CAS" => "STORE_CAS: Runtime CAS operation rejected the request".to_owned(),
                "STORE_CAS_HASH_MISMATCH" => "STORE_CAS_HASH_MISMATCH: Runtime CAS content hash did not match the bound artifact".to_owned(),
                "STORE_CAS_INVALID_HASH" => "STORE_CAS_INVALID_HASH: Runtime CAS received an invalid content hash".to_owned(),
                "STORE_CAS_CORRUPT" => "STORE_CAS_CORRUPT: Runtime CAS object is corrupt".to_owned(),
                "STORE_CAS_CAPACITY_EXCEEDED" => "STORE_CAS_CAPACITY_EXCEEDED: Runtime CAS object exceeds the configured limit".to_owned(),
                "STORE_CAS_UNSAFE_ROOT" => "STORE_CAS_UNSAFE_ROOT: Runtime CAS root is unsafe".to_owned(),
                "STORE_CAS_IO" => "STORE_CAS_IO: Runtime CAS file operation failed".to_owned(),
                "STORE_IO" => "STORE_IO: Runtime store I/O failed".to_owned(),
                "STORE_BACKUP_UNAVAILABLE" => "STORE_BACKUP_UNAVAILABLE: Runtime store backup is unavailable".to_owned(),
                "STORE_MIGRATION_UNSUPPORTED" => "STORE_MIGRATION_UNSUPPORTED: Runtime store migration is unsupported".to_owned(),
                "STORE_LEGACY_DATABASE_REJECTED" => "STORE_LEGACY_DATABASE_REJECTED: Runtime rejected a legacy database".to_owned(),
                "STORE_LOCK_POISONED" => "STORE_LOCK_POISONED: Runtime store lock is poisoned".to_owned(),
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
        self.ready_file
            .as_deref()
            .ok_or_else(|| "RUNTIME_UNAVAILABLE: Runtime endpoint is unavailable".to_owned())
            .and_then(read_ready_endpoint)
    }

    fn call(&self, name: &str, arguments: &Value) -> Result<Value, String> {
        let endpoint = self.endpoint()?;
        let mut client = LocalIpcClient::connect(&endpoint).map_err(map_ipc_error)?;
        client.call(name, arguments.clone()).map_err(map_ipc_error)
    }

    fn status(&self) -> Value {
        let ready_probe = match self.endpoint() {
            Ok(endpoint) => probe_dynamic_endpoint(&endpoint),
            Err(_) => DynamicReadyProbe::Unavailable,
        };
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
        "session_create_or_resume"
        | "session_get"
        | "checkpoint_prepare"
        | "checkpoint_get"
        | "checkpoint_restore_prepare"
        | "design_action_run_prepare"
        | "design_action_run_get" => match name {
            "session_create_or_resume" => runtime
                .session_create_or_resume(arguments.clone())
                .map_err(|error| error.to_string()),
            "session_get" => runtime
                .session_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "checkpoint_prepare" => runtime
                .checkpoint_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "checkpoint_get" => runtime
                .checkpoint_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "checkpoint_restore_prepare" => runtime
                .checkpoint_restore_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "design_action_run_prepare" => runtime
                .design_action_run_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "design_action_run_get" => runtime
                .design_action_run_get(arguments.clone())
                .map_err(|error| error.to_string()),
            _ => unreachable!("agentic write tool dispatch arm is exhaustive"),
        },
        "capabilities_get" => {
            serde_json::to_value(runtime.capabilities()).map_err(|error| error.to_string())
        }
        "operator_catalog_get" => Ok(runtime.active_operator_catalog()),
        "material_pack_get" => Ok(runtime.material_pack_manifest()),
        "agentic_scene_observe" => {
            let project_id = required_id(arguments, "project_id")?;
            runtime
                .agentic_scene_observe(
                    project_id,
                    arguments.get("candidate_id").and_then(Value::as_str),
                )
                .map_err(|error| error.to_string())
        }
        "agentic_stage_plan" => {
            let project_id = required_id(arguments, "project_id")?;
            runtime
                .agentic_stage_plan(
                    project_id,
                    arguments.get("candidate_id").and_then(Value::as_str),
                )
                .map_err(|error| error.to_string())
        }
        "agentic_critic_projection" => {
            let project_id = required_id(arguments, "project_id")?;
            runtime
                .agentic_critic_projection(
                    project_id,
                    arguments.get("candidate_id").and_then(Value::as_str),
                )
                .map_err(|error| error.to_string())
        }
        "geometry_program_hash" => runtime
            .geometry_program_hash(arguments)
            .map_err(|error| error.to_string()),
        "silhouette_rig_hash" => {
            let project_id = required_id(arguments, "project_id")?;
            runtime
                .silhouette_rig_hash(project_id, arguments)
                .map_err(|error| error.to_string())
        }
        "silhouette_target_get" => {
            let target_sha256 = required_sha256(arguments, "target_sha256")?;
            runtime
                .silhouette_target_get(target_sha256)
                .map_err(|error| error.to_string())
        }
        "boundary_error_get" => {
            let candidate_id = required_id(arguments, "candidate_id")?;
            let target_sha256 = required_sha256(arguments, "target_sha256")?;
            runtime
                .boundary_error(
                    candidate_id,
                    target_sha256,
                    arguments.get("max_segments").and_then(Value::as_u64),
                )
                .map_err(|error| error.to_string())
        }
        "silhouette_part_error_get" => {
            let project_id = required_id(arguments, "project_id")?;
            runtime
                .silhouette_part_error(project_id, arguments.clone())
                .map_err(|error| error.to_string())
        }
        "render_pass_get" => {
            let render_set_hash = arguments
                .get("render_set_hash")
                .and_then(Value::as_str)
                .ok_or_else(|| "render_set_hash is required".to_owned())?;
            let pass = arguments
                .get("pass")
                .and_then(Value::as_str)
                .ok_or_else(|| "pass is required".to_owned())?;
            runtime
                .render_pass_get(render_set_hash, pass)
                .map_err(|error| error.to_string())
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
        "reference_compare_prepare" => {
            let project_id = required_id(arguments, "project_id")?;
            runtime
                .prepare_reference_comparison(project_id, arguments.clone())
                .map_err(|error| error.to_string())
        }
        "reference_mask_prepare" => {
            let project_id = required_id(arguments, "project_id")?;
            runtime
                .prepare_reference_mask(project_id, arguments.clone())
                .map_err(|error| error.to_string())
        }
        "reference_mask_refine_prepare" => {
            let project_id = required_id(arguments, "project_id")?;
            runtime
                .refine_reference_mask(project_id, arguments.clone())
                .map_err(|error| error.to_string())
        }
        "camera_fit_prepare" => {
            let project_id = required_id(arguments, "project_id")?;
            runtime
                .prepare_camera_fit(project_id, arguments.clone())
                .map_err(|error| error.to_string())
        }
        "silhouette_fit_prepare" => {
            let project_id = required_id(arguments, "project_id")?;
            let arguments = canonicalize_silhouette_fit_wire(arguments)?;
            runtime
                .silhouette_fit_prepare(project_id, arguments)
                .map_err(|error| error.to_string())
        }
        "primary_form_repair_prepare" => {
            let project_id = required_id(arguments, "project_id")?;
            let arguments = canonicalize_silhouette_fit_wire(arguments)?;
            let base_version_id = arguments
                .get("base_version_id")
                .and_then(Value::as_str)
                .map(str::to_owned);
            runtime
                .primary_form_repair_prepare(
                    project_id,
                    base_version_id.as_deref(),
                    arguments,
                )
                .map_err(|error| error.to_string())
        }
        "part_contour_fit_prepare" => {
            let project_id = required_id(arguments, "project_id")?;
            runtime
                .part_contour_fit_prepare(project_id, arguments.clone())
                .map_err(|error| error.to_string())
        }
        "silhouette_candidate_compare" => {
            let project_id = required_id(arguments, "project_id")?;
            runtime
                .silhouette_candidate_compare(project_id, arguments.clone())
                .map_err(|error| error.to_string())
        }
        "visual_review_submit" => runtime
            .submit_visual_review(arguments.clone())
            .map_err(|error| error.to_string()),
        "human_visual_review_submit" => runtime
            .submit_human_visual_review(arguments.clone())
            .map_err(|error| error.to_string()),
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
            runtime
                .skill_result(skill_id, version)
                .map_err(|error| error.to_owned())
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

/// Reconcile the harmless numeric spelling change that can occur when Codex
/// copies a JSON response (for example `1.0` becoming `1`).  Runtime keeps the
/// original typed contract and its own canonical hashes; the adapter only
/// restores continuous fields before dispatch and re-binds the outer intent.
/// A caller-provided hash must match either the exact wire payload or this
/// deterministic restoration.  Arbitrary/wrong hashes still fail closed.
fn canonicalize_silhouette_fit_wire(arguments: &Value) -> Result<Value, String> {
    let object = arguments
        .as_object()
        .ok_or_else(|| "INVALID_INPUT: silhouette fit arguments must be an object".to_owned())?;
    let supplied = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| "INVALID_INPUT: canonical_sha256 is required".to_owned())?;

    let mut wire_input = arguments.clone();
    wire_input["canonical_sha256"] = Value::String(String::new());
    let wire_hash = canonical_json_hash(&wire_input);

    let mut restored = arguments.clone();
    if let Some(camera) = restored.get("base_camera").cloned() {
        restored["base_camera"] = normalize_continuous_numbers(&camera, true);
    }
    if let Some(rig) = restored.get("rig").cloned() {
        restored["rig"] = normalize_continuous_numbers(&rig, false);
    }
    if let Some(optimizer) = restored.get("optimizer").cloned() {
        restored["optimizer"] = normalize_optimizer_numbers(&optimizer);
    }
    restored["canonical_sha256"] = Value::String(String::new());
    let restored_hash = canonical_json_hash(&restored);
    if supplied != wire_hash && supplied != restored_hash {
        return Err("SILHOUETTE_FIT_INVALID: canonical_sha256 does not bind intent".to_owned());
    }
    restored["canonical_sha256"] = Value::String(restored_hash);
    Ok(restored)
}

fn normalize_continuous_numbers(value: &Value, preserve_resolution: bool) -> Value {
    match value {
        Value::Number(number) => number
            .as_f64()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .unwrap_or_else(|| value.clone()),
        Value::Array(values) => Value::Array(
            values
                .iter()
                .map(|child| normalize_continuous_numbers(child, preserve_resolution))
                .collect(),
        ),
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, child)| {
                    let normalized = if preserve_resolution && key == "resolution" {
                        child.clone()
                    } else {
                        normalize_continuous_numbers(child, preserve_resolution)
                    };
                    (key.clone(), normalized)
                })
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn normalize_optimizer_numbers(value: &Value) -> Value {
    let Some(object) = value.as_object() else {
        return value.clone();
    };
    Value::Object(
        object
            .iter()
            .map(|(key, child)| {
                if matches!(key.as_str(), "max_iterations" | "max_evaluations") {
                    (key.clone(), child.clone())
                } else {
                    (key.clone(), normalize_continuous_numbers(child, false))
                }
            })
            .collect(),
    )
}

fn required_sha256<'a>(arguments: &'a Value, key: &str) -> Result<&'a str, String> {
    let sha256 = arguments
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default();
    if sha256.len() != 64 || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "INVALID_INPUT: {key} is required and must be a SHA-256"
        ));
    }
    Ok(sha256)
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
        // Most focused tests are about their target capability, not ordering.
        // Dedicated coverage below starts from a fresh session to verify the gate.
        session.ponytail_preflight_read = true;
        (backend, session)
    }

    #[test]
    fn checkpoint_get_schema_accepts_bound_readback() {
        let arguments = json!({
            "checkpoint_id": "checkpoint-agentic-probe",
            "session_id": "session-agentic-probe",
            "project_id": "project-agentic-probe",
            "candidate_id": "candidate-agentic-probe"
        });
        let schema = tools_with_writes(true)
            .into_iter()
            .find(|tool| tool["name"] == "checkpoint_get")
            .expect("checkpoint_get tool")["inputSchema"]
            .clone();
        let mut schema_budget = ToolSchemaValidationBudget::new();
        assert!(validate_tool_schema_shape(&schema, 0, &mut schema_budget).is_ok());
        let mut value_budget = ToolSchemaValidationBudget::new();
        assert!(validate_value_against_tool_schema(
            &schema,
            &arguments,
            0,
            &mut value_budget
        )
        .is_ok());
    }

    #[test]
    fn ponytail_preflight_must_be_read_before_design_tools_or_other_skills() {
        let mut backend = Backend::InProcess(Runtime::ephemeral().expect("runtime"));
        let mut session = Session::new();
        let initialize_response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":MCP_PROTOCOL_VERSION,"capabilities":{},"clientInfo":{"name":"test","version":"1"}}}),
        )
        .expect("initialize response");
        assert_eq!(initialize_response["result"]["protocolVersion"], MCP_PROTOCOL_VERSION);

        let blocked = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"operator_catalog_get","arguments":{}}}),
        )
        .expect("preflight block");
        assert_eq!(blocked["result"]["isError"], true);
        assert_eq!(blocked["result"]["structuredContent"]["code"], "PONYTAIL_PREFLIGHT_REQUIRED");

        let preflight = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"skill_get","arguments":{"skill_id":"ponytail-preflight","version":"0.1.0"}}}),
        )
        .expect("preflight skill");
        assert_eq!(preflight["result"]["structuredContent"]["skill"]["skill_id"], "ponytail-preflight");
        assert!(preflight["result"]["structuredContent"]["knowledge"]["overview"]
            .as_str()
            .expect("preflight overview")
            .contains("Ponytail preflight"));

        let catalog = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"operator_catalog_get","arguments":{}}}),
        )
        .expect("catalog after preflight");
        assert!(catalog["result"]["structuredContent"]["operators"].is_array());
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
        session.ponytail_preflight_read = true;

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
        session.ponytail_preflight_read = true;

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
    fn tool_manifest_summary_is_derived_from_the_actual_enabled_manifests() {
        let summary = tool_manifest_summary().expect("tool manifest summary");
        assert_eq!(
            summary["schema_version"],
            "ForgeCADMcpToolManifestSummary@1"
        );
        assert_eq!(summary["read_count"], 36);
        assert_eq!(summary["write_count"], 23);
        assert_eq!(summary["total_count"], 59);
        assert_eq!(summary["read_names"].as_array().unwrap().len(), 36);
        assert_eq!(summary["write_names"].as_array().unwrap().len(), 23);
        let mut hash_input = summary.clone();
        hash_input
            .as_object_mut()
            .expect("summary object")
            .remove("canonical_sha256");
        assert_eq!(
            summary["canonical_sha256"],
            canonical_json_hash(&hash_input)
        );
    }

    #[test]
    fn geometry_program_hash_is_a_default_read_only_tool() {
        let tools = tools_with_writes(false);
        let tool = tools
            .iter()
            .find(|tool| tool["name"] == "geometry_program_hash")
            .expect("geometry_program_hash tool");
        assert_eq!(tool["annotations"]["readOnlyHint"], true);
        assert_eq!(
            tool["inputSchema"]["properties"]["schema_version"]["const"],
            "GeometryProgramHashRequest@1"
        );
        assert_eq!(
            tool["inputSchema"]["additionalProperties"], false,
            "the public envelope must remain closed"
        );
    }

    #[test]
    fn silhouette_rig_hash_is_a_default_read_only_tool() {
        let tools = tools_with_writes(false);
        let tool = tools
            .iter()
            .find(|tool| tool["name"] == "silhouette_rig_hash")
            .expect("silhouette_rig_hash tool");
        assert_eq!(tool["annotations"]["readOnlyHint"], true);
        assert_eq!(
            tool["inputSchema"]["properties"]["schema_version"]["const"],
            "SilhouetteRigHashRequest@1"
        );
        assert_eq!(tool["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            tool["inputSchema"]["properties"]["rig_draft"]["additionalProperties"],
            false
        );
    }

    #[test]
    fn silhouette_hash_arguments_accept_sha256_values() {
        let sha256 = "a".repeat(64);
        assert_eq!(
            required_sha256(&json!({"target_sha256":sha256}), "target_sha256")
                .expect("target hash"),
            "a".repeat(64)
        );
        assert!(
            required_sha256(&json!({"target_sha256":"g".repeat(64)}), "target_sha256").is_err()
        );
    }

    #[test]
    fn silhouette_fit_wire_numbers_are_restored_before_hash_binding() {
        let mut request = json!({
            "project_id":"project-fit-wire",
            "candidate_id":"candidate-fit-wire",
            "target_sha256":"a".repeat(64),
            "rig":{"parameters":[{"value":1.0,"min":0.8,"max":1.2,"step":0.04}]},
            "base_camera":{"fov_y_degrees":33.0,"near_m":0.05,"far_m":20.0,"resolution":{"width":512,"height":512}},
            "optimizer":{"max_iterations":2,"max_evaluations":8,"step_fraction":0.1},
            "canonical_sha256":""
        });
        let wire = {
            fn integerize(value: &Value) -> Value {
                match value {
                    Value::Number(number) => number
                        .as_f64()
                        .filter(|value| value.is_finite() && value.fract() == 0.0)
                        .map(|value| serde_json::Number::from(value.round() as i64))
                        .map(Value::Number)
                        .unwrap_or_else(|| value.clone()),
                    Value::Array(values) => Value::Array(values.iter().map(integerize).collect()),
                    Value::Object(object) => Value::Object(
                        object
                            .iter()
                            .map(|(key, child)| (key.clone(), integerize(child)))
                            .collect(),
                    ),
                    _ => value.clone(),
                }
            }
            integerize(&request)
        };
        request = wire.clone();
        request["canonical_sha256"] = Value::String(canonical_json_hash(&wire));
        let restored = canonicalize_silhouette_fit_wire(&request).expect("wire hash accepted");
        assert_eq!(restored["base_camera"]["fov_y_degrees"], json!(33.0));
        assert_eq!(restored["base_camera"]["resolution"]["width"], json!(512));
        assert_ne!(restored["canonical_sha256"], Value::String(String::new()));
        request["canonical_sha256"] = Value::String("b".repeat(64));
        assert!(canonicalize_silhouette_fit_wire(&request).is_err());
    }

    #[test]
    fn every_advertised_tool_input_schema_uses_the_bounded_validator_subset() {
        for tool in tools_with_writes(true) {
            let mut budget = ToolSchemaValidationBudget::new();
            validate_tool_schema_shape(&tool["inputSchema"], 0, &mut budget)
                .unwrap_or_else(|_| panic!("{} has an unsupported input schema", tool["name"]));
        }
    }

    #[test]
    fn nullable_envelope_fields_remain_valid_when_declared() {
        assert!(validate_declared_tool_input(
            "geometry_prepare",
            &json!({
                "project_id":"project-envelope-fixture",
                "base_version_id":null,
                "request":{"typed":"geometry","geometry_program":{}}
            }),
            true,
        )
        .is_ok());
        assert!(validate_declared_tool_input(
            "reference_import",
            &json!({
                "project_id":"project-envelope-fixture",
                "source":{
                    "kind":"inline_content",
                    "mime":"image/png",
                    "content_base64":"fixture"
                },
                "authorization":{"user_authorized":true,"declaration":"test"},
                "expected_sha256":null
            }),
            true,
        )
        .is_ok());
    }

    #[test]
    fn tool_input_envelopes_fail_closed_before_geometry_runtime_dispatch() {
        let (mut backend, mut session) = initialized();
        let project = match &backend {
            Backend::InProcess(runtime) => runtime
                .create_project("MCP envelope fixture", json!({"scope":"test"}))
                .expect("project"),
            _ => unreachable!("test backend"),
        };

        let disabled = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":21,
                "method":"tools/call",
                "params":{"name":"geometry_prepare","arguments":{"unexpected":true}}
            }),
        )
        .expect("disabled response");
        assert_eq!(disabled["result"]["isError"], true);
        assert_eq!(
            disabled["result"]["structuredContent"]["code"], "MCP007_GEOMETRY_TOOLS_DISABLED",
            "disabled-write availability must remain ahead of input validation"
        );

        session.write_tools_enabled = true;
        let mut geometry_program = json!({
            "schema_version":"GeometryProgram@1",
            "project_id":project.project_id,
            "representation_plan_sha256":"c".repeat(64),
            "nodes":[{
                "node_id":"torso",
                "operator_id":"forgecad.geometry.primitive@1",
                "part_id":"torso",
                "parameters":{
                    "shape":"box",
                    "size":[1.0,1.4,0.5],
                    "position":[0.0,1.2,0.0],
                    "material_zone_id":"zone-white-shell"
                }
            }],
            "budgets":{"max_nodes":8,"max_triangles":10000,"max_runtime_ms":1000}
        });
        geometry_program["canonical_sha256"] =
            Value::String(canonical_json_hash(&geometry_program));
        let valid_geometry_request = json!({
            "project_id":project.project_id,
            "request":{"typed":"geometry","geometry_program":geometry_program}
        });

        let mut unknown_outer = valid_geometry_request.clone();
        unknown_outer["unexpected_outer"] = Value::Bool(true);
        let mut unknown_request = valid_geometry_request.clone();
        unknown_request["request"]["unexpected_request"] = Value::Bool(true);
        let mut wrong_project_id_type = valid_geometry_request.clone();
        wrong_project_id_type["project_id"] = Value::from(7);
        let mut missing_required_request_field = valid_geometry_request.clone();
        missing_required_request_field["request"]
            .as_object_mut()
            .expect("request object")
            .remove("geometry_program");

        for (id, arguments) in [
            (22, unknown_outer),
            (23, unknown_request),
            (24, wrong_project_id_type),
            (25, missing_required_request_field),
        ] {
            let response = handle(
                &mut backend,
                &mut session,
                &json!({
                    "jsonrpc":"2.0",
                    "id":id,
                    "method":"tools/call",
                    "params":{"name":"geometry_prepare","arguments":arguments}
                }),
            )
            .expect("invalid geometry envelope response");
            assert_eq!(response["error"]["data"]["code"], "INVALID_TOOL_PARAMS");
        }

        for (id, arguments) in [
            (
                26,
                json!({
                    "schema_version":"GeometryProgramHashRequest@1",
                    "geometry_program_draft":{},
                    "unexpected_outer":true
                }),
            ),
            (27, json!({"geometry_program_draft":{}})),
            (
                28,
                json!({
                    "schema_version":7,
                    "geometry_program_draft":[]
                }),
            ),
        ] {
            let response = handle(
                &mut backend,
                &mut session,
                &json!({
                    "jsonrpc":"2.0",
                    "id":id,
                    "method":"tools/call",
                    "params":{"name":"geometry_program_hash","arguments":arguments}
                }),
            )
            .expect("invalid hash envelope response");
            assert_eq!(response["error"]["data"]["code"], "INVALID_TOOL_PARAMS");
        }

        let outer_response = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":29,
                "method":"tools/call",
                "params":{
                    "name":"geometry_program_hash",
                    "arguments":{
                        "schema_version":"GeometryProgramHashRequest@1",
                        "geometry_program_draft":{}
                    },
                    "unexpected_call_field":true
                }
            }),
        )
        .expect("invalid tools/call envelope response");
        assert_eq!(
            outer_response["error"]["data"]["code"],
            "INVALID_TOOL_PARAMS"
        );

        let candidates = match &backend {
            Backend::InProcess(runtime) => runtime
                .candidates(&project.project_id)
                .expect("candidates after rejected envelopes"),
            _ => unreachable!("test backend"),
        };
        assert!(
            candidates.is_empty(),
            "invalid MCP envelopes must not reach candidate preparation"
        );
    }

    #[test]
    fn operator_catalog_get_is_a_default_read_only_tool() {
        let tools = tools_with_writes(false);
        let tool = tools
            .iter()
            .find(|tool| tool["name"] == "operator_catalog_get")
            .expect("operator_catalog_get tool");
        assert_eq!(tool["annotations"]["readOnlyHint"], true);
        assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    }

    #[test]
    fn agentic_projection_tools_are_read_only_and_do_not_enter_write_manifest() {
        let read_tools = tools_with_writes(false);
        let enabled_tools = tools_with_writes(true);
        for name in [
            "scene_observe_get",
            "design_stage_plan_get",
            "critic_report_get",
            "visual_evidence_bundle_get",
        ] {
            let read_tool = read_tools
                .iter()
                .find(|tool| tool["name"] == name)
                .expect("agentic read tool");
            assert_eq!(read_tool["annotations"]["readOnlyHint"], true);
            assert_eq!(read_tool["annotations"]["destructiveHint"], false);
            assert_eq!(read_tool["annotations"]["idempotentHint"], true);
            assert_eq!(read_tool["inputSchema"]["additionalProperties"], false);
            assert!(!is_write_tool(name));
            assert!(enabled_tools.iter().any(|tool| tool["name"] == name));
        }
        assert_eq!(
            read_tools
                .iter()
                .find(|tool| tool["name"] == "scene_observe_get")
                .expect("scene tool")
                .pointer("/_meta/forgecad/availability"),
            Some(&Value::String("available".to_owned()))
        );
        assert_eq!(
            read_tools
                .iter()
                .find(|tool| tool["name"] == "visual_evidence_bundle_get")
                .expect("evidence tool")
                .pointer("/_meta/forgecad/source_schema"),
            Some(&Value::String("VisualEvidenceBundle@1".to_owned()))
        );
    }

    #[test]
    fn agentic_targets_require_preflight_and_fail_closed_on_missing_project() {
        let mut backend = Backend::InProcess(Runtime::ephemeral().expect("runtime"));
        let mut session = Session::new();
        let initialized_response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":MCP_PROTOCOL_VERSION,"capabilities":{},"clientInfo":{"name":"agentic-test","version":"1"}}}),
        )
        .expect("initialize response");
        assert_eq!(
            initialized_response["result"]["protocolVersion"],
            MCP_PROTOCOL_VERSION
        );

        let blocked = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"scene_observe_get","arguments":{"project_id":"project-a","candidate_id":"candidate-a"}}}),
        )
        .expect("preflight response");
        assert_eq!(
            blocked["result"]["structuredContent"]["code"],
            "PONYTAIL_PREFLIGHT_REQUIRED"
        );

        session.ponytail_preflight_read = true;
        let unavailable = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"scene_observe_get","arguments":{"project_id":"project-a","candidate_id":"candidate-a"}}}),
        )
        .expect("unavailable response");
        assert_eq!(unavailable["result"]["isError"], true);
        assert_eq!(
            unavailable["result"]["structuredContent"]["code"],
            "invalid runtime input"
        );
        assert!(unavailable["result"]["structuredContent"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("project not found")));
    }

    #[test]
    fn agentic_write_tools_require_preflight_and_explicit_opt_in() {
        let mut backend = Backend::InProcess(Runtime::ephemeral().expect("runtime"));
        let mut session = Session::new();
        handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":MCP_PROTOCOL_VERSION,"capabilities":{},"clientInfo":{"name":"agentic-write-test","version":"1"}}}),
        )
        .expect("initialize response");
        let request = json!({
            "session_id": null,
            "project_id": "project-1",
            "candidate_id": "candidate-1",
            "idempotency_key": "idem-1",
            "approved": true,
            "approval_receipt_id": "approval-1",
            "approval_summary": "approved"
        });
        let blocked = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"session_create_or_resume","arguments":request.clone()}}),
        )
        .expect("preflight response");
        assert_eq!(
            blocked["result"]["structuredContent"]["code"],
            "PONYTAIL_PREFLIGHT_REQUIRED"
        );

        session.ponytail_preflight_read = true;
        let disabled = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"session_create_or_resume","arguments":request}}),
        )
        .expect("opt-in response");
        assert_eq!(
            disabled["result"]["structuredContent"]["code"],
            "AGENTIC_WRITE_TOOLS_DISABLED"
        );
    }

    #[test]
    fn mcp004_write_tools_are_explicit_and_confirmation_bound() {
        let disabled = tools_with_writes(false);
        assert_eq!(disabled.len(), 36);
        assert!(!disabled
            .iter()
            .any(|tool| { tool["name"].as_str().is_some_and(is_mcp004_write_tool) }));

        let enabled = tools_with_writes(true);
        assert_eq!(enabled.len(), 59);
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
    fn primary_form_repair_prepare_is_an_explicit_runtime_owned_write() {
        let disabled = tools_with_writes(false);
        assert!(!disabled
            .iter()
            .any(|tool| tool["name"] == "primary_form_repair_prepare"));

        let enabled = tools_with_writes(true);
        let tool = enabled
            .iter()
            .find(|tool| tool["name"] == "primary_form_repair_prepare")
            .expect("Primary Form repair tool");
        assert_eq!(tool["annotations"]["readOnlyHint"], false);
        assert_eq!(tool["annotations"]["destructiveHint"], false);
        assert_eq!(tool["annotations"]["idempotentHint"], true);
        assert_eq!(tool["_meta"]["forgecad"]["requiresConfirmation"], true);
        assert_eq!(tool["_meta"]["forgecad"]["transaction"], "MCP010F");
        assert_eq!(tool["inputSchema"]["additionalProperties"], false);
        for field in [
            "project_id",
            "candidate_id",
            "target_sha256",
            "rig",
            "base_camera",
            "optimizer",
            "canonical_sha256",
        ] {
            assert!(tool["inputSchema"]["required"]
                .as_array()
                .expect("Primary Form required schema")
                .iter()
                .any(|value| value == field));
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
        let mut backend = Backend::DynamicIpc(DynamicIpcBackend::from_ready_file(ready_file, None));

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
    fn fixed_endpoint_backend_does_not_hold_an_idle_authenticated_connection() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("fc-fe-{}", nonce % 1_000_000_000));
        fs::create_dir_all(&directory).expect("directory");
        let endpoint = LocalIpcEndpoint::new(&directory).expect("endpoint");
        let runtime = Arc::new(Runtime::ephemeral().expect("runtime"));
        let server = runtime.ipc_server(&endpoint).expect("server");
        let runtime_for_thread = runtime.clone();
        let server_thread = thread::spawn(move || server.serve_forever(&runtime_for_thread));

        let dynamic = DynamicIpcBackend::from_fixed_endpoint(endpoint.clone());
        assert_eq!(
            dynamic
                .call("project_list", &Value::Null)
                .expect("fixed endpoint call"),
            json!([])
        );

        // The Runtime accepts one connection at a time.  This independent
        // client can authenticate only if the call above dropped its client
        // instead of retaining an idle authenticated stream between Codex
        // tool calls.
        let mut independent = LocalIpcClient::connect(&endpoint).expect("independent client");
        assert_eq!(
            independent
                .call("project_list", Value::Null)
                .expect("independent call"),
            json!([])
        );
        independent
            .call("runtime_shutdown", Value::Null)
            .expect("shutdown");
        drop(independent);
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
        session.ponytail_preflight_read = true;

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
        assert_eq!(listed["result"]["tools"].as_array().unwrap().len(), 59);

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
                    "prepared_object_sha256":object.record.sha256
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
        let manifest: Value = serde_json::from_str(
            response["result"]["contents"][0]["text"]
                .as_str()
                .expect("resource text"),
        )
        .expect("skill resource JSON");
        assert_eq!(manifest["skill"]["execution_availability"], "unavailable");
        assert_eq!(
            manifest["skill"]["missing_operator_ids"],
            json!([
                "forgecad.reference.validate@1",
                "forgecad.reference.inventory@1"
            ])
        );

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

        let dynamic = DynamicIpcBackend::from_ready_file(ready_file, Some(status_file));
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
        let backend = Backend::DynamicIpc(DynamicIpcBackend::from_ready_file(
            ready_file,
            Some(status_file),
        ));

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
