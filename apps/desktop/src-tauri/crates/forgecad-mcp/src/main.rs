mod agentic_action_tools;
mod agentic_orchestrator_tools;
mod agentic_tools;
mod agentic_write_tools;
mod cross_view_promotion_tools;
mod optimization_tools;
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
const READ_MODEL_MCP_WIRE_MAX_BYTES: usize = 1024 * 1024;

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
    action_binding: agentic_action_tools::Binding,
    orchestrator_binding: agentic_orchestrator_tools::Binding,
    optimization_binding: optimization_tools::Binding,
    cross_view_promotion_binding: cross_view_promotion_tools::Binding,
}

impl Session {
    fn new() -> Self {
        Self {
            state: SessionState::New,
            negotiated_protocol_version: None,
            write_tools_enabled: false,
            ponytail_preflight_read: false,
            agentic_binding: agentic_write_tools::Binding::default(),
            action_binding: agentic_action_tools::Binding::default(),
            orchestrator_binding: agentic_orchestrator_tools::Binding::default(),
            optimization_binding: optimization_tools::Binding::default(),
            cross_view_promotion_binding: cross_view_promotion_tools::Binding::default(),
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
            let write_tools_enabled = advertised_write_tools_enabled(
                backend,
                session.write_tools_enabled,
            );
            json!({"jsonrpc":"2.0","id":id,"result":{"tools":tools_with_writes(write_tools_enabled)}})
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
    let effective_write_tools_enabled = effective_write_tools_enabled(
        write_tools_enabled,
        runtime_build_cohort.as_deref(),
        mcp_build_cohort.as_deref(),
    );
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
        Value::String(tool_manifest_hash(effective_write_tools_enabled)),
    );
    object.insert(
        "mcp_write_tools_enabled".to_owned(),
        Value::Bool(effective_write_tools_enabled),
    );
    if !effective_write_tools_enabled
        && mcp_build_cohort.is_some()
        && runtime_build_cohort.as_ref() != mcp_build_cohort.as_ref()
    {
        object.insert(
            "mcp_write_tools_disabled_reason".to_owned(),
            Value::String("BUILD_COHORT_MISMATCH".to_owned()),
        );
    }
    object.insert(
        "mcp_write_tool_names".to_owned(),
        if effective_write_tools_enabled {
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

/// A packaged MCP adapter must not advertise write tools when its embedded
/// cohort cannot be proven equal to the Runtime cohort.  Source/test builds
/// may omit the compile-time cohort and retain their existing opt-in behavior;
/// once the adapter carries a cohort, a missing or different Runtime cohort is
/// a fail-closed read-only state.
fn effective_write_tools_enabled(
    requested: bool,
    runtime_build_cohort: Option<&str>,
    mcp_build_cohort: Option<&str>,
) -> bool {
    if !requested {
        return false;
    }
    match mcp_build_cohort {
        Some(local) => runtime_build_cohort == Some(local),
        None => true,
    }
}

fn advertised_write_tools_enabled(backend: &mut Backend, requested: bool) -> bool {
    if !requested {
        return false;
    }
    let Some(local_build_cohort) = build_cohort_sha256() else {
        // Ordinary source/test builds intentionally omit a cohort and retain
        // the existing explicit opt-in behavior.
        return true;
    };
    let Ok(runtime_capabilities) = backend_call(backend, "capabilities_get", &json!({})) else {
        // A packaged adapter cannot prove a matching Runtime while degraded.
        return false;
    };
    let runtime_build_cohort = runtime_capabilities
        .get("build_cohort_sha256")
        .and_then(Value::as_str);
    effective_write_tools_enabled(
        true,
        runtime_build_cohort,
        Some(local_build_cohort.as_str()),
    )
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
        "authoring_mesh_edit_prepare",
        "mechanical_animation_clip_prepare",
        "mechanical_animation_glb_prepare",
        "game_asset_delivery_prepare",
        "game_weapon_anchor_prepare",
        "game_weapon_glb_socket_prepare",
        "game_weapon_animated_glb_socket_prepare",
        "appearance_source_lineage_prepare",
        "fictional_energy_vfx_prepare",
        "fictional_energy_vfx_rendered_frame_prepare",
        "fictional_energy_vfx_rendered_sequence_prepare",
        "fictional_energy_vfx_hdr_bloom_prepare",
        "fictional_energy_vfx_particles_prepare",
        "fictional_energy_vfx_trails_prepare",
        "fictional_energy_vfx_trails_bloom_prepare",
        "subdivision_artifact_lineage_prepare",
        "primary_form_repair_prepare",
        "primary_form_repair_job_prepare",
        "reference_mask_prepare",
        "reference_mask_refine_prepare",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn silhouette_target_part_property() -> Value {
    json!({
        "type":"object",
        "required":["part_id","start_index","end_index","visibility"],
        "properties":{
            "part_id":id_property(),
            "start_index":{"type":"integer","minimum":0,"maximum":511},
            "end_index":{"type":"integer","minimum":0,"maximum":511},
            "visibility":{"enum":["observed","inferred","unknown"]},
            "region":{
                "type":"object",
                "required":["region_id","x","y","width","height"],
                "properties":{
                    "region_id":id_property(),
                    "x":{"type":"number","minimum":0,"maximum":1},
                    "y":{"type":"number","minimum":0,"maximum":1},
                    "width":{"type":"number","minimum":0,"maximum":1},
                    "height":{"type":"number","minimum":0,"maximum":1}
                },
                "additionalProperties":false
            }
        },
        "additionalProperties":false
    })
}

fn reference_visual_structure_property() -> Value {
    json!({
        "type":["object","null"],
        "required":["regions","line_flows"],
        "properties":{
            "regions":{
                "type":"array",
                "minItems":1,
                "maxItems":64,
                "items":{
                    "type":"object",
                    "required":["structure_id","visual_role","continuity_group_id","layer_index","boundary_relationship","visibility","depth_policy","profile_policy","contour_points"],
                    "properties":{
                        "structure_id":id_property(),
                        "visual_role":{"enum":["outer-flowing-shell","open-frame","primary-volume","floating-shell","layered-body","terminal-assembly","luminous-core","internal-channel","material-transition","unknown-visual-region"]},
                        "continuity_group_id":id_property(),
                        "layer_index":{"type":"integer","minimum":-16,"maximum":16},
                        "boundary_relationship":{"enum":["shared","overlap","independent","enclosed"]},
                        "visibility":{"enum":["observed","inferred","unknown"]},
                        "depth_policy":{"enum":["from-multiview","bounded-inference","unknown"]},
                        "profile_policy":{"enum":["preserve-continuity","closed-profile","revolved-profile","material-only"]},
                        "mask_operation":{"enum":["none","subtract"]},
                        "contour_points":{"type":"array","minItems":3,"maxItems":256,"items":{"type":"array","minItems":2,"maxItems":2,"items":{"type":"number","minimum":0,"maximum":1}}}
                    },
                    "additionalProperties":false
                }
            },
            "line_flows":{
                "type":"array",
                "maxItems":128,
                "items":{
                    "type":"object",
                    "required":["line_flow_id","continuity_group_id","kind","visibility","points"],
                    "properties":{
                        "line_flow_id":id_property(),
                        "continuity_group_id":id_property(),
                        "kind":{"enum":["ridge","valley","seam","light-channel","occlusion-edge"]},
                        "visibility":{"enum":["observed","inferred","unknown"]},
                        "points":{"type":"array","minItems":2,"maxItems":256,"items":{"type":"array","minItems":2,"maxItems":2,"items":{"type":"number","minimum":0,"maximum":1}}}
                    },
                    "additionalProperties":false
                }
            }
        },
        "additionalProperties":false
    })
}

fn agentic_write_tool_names() -> Vec<String> {
    agentic_write_tools::write_tool_names()
}

fn agentic_action_write_tool_names() -> Vec<String> {
    agentic_action_tools::write_tool_names()
}

fn optimization_write_tool_names() -> Vec<String> {
    optimization_tools::write_tool_names()
}

fn agentic_orchestrator_write_tool_names() -> Vec<String> {
    agentic_orchestrator_tools::write_tool_names()
}

fn cross_view_promotion_write_tool_names() -> Vec<String> {
    cross_view_promotion_tools::write_tool_names()
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
        || optimization_tools::is_write_tool(name)
        || agentic_orchestrator_tools::is_write_tool(name)
        || agentic_action_tools::is_write_tool(name)
        || cross_view_promotion_tools::is_write_tool(name)
        || agentic_write_tools::is_write_tool(name)
}

fn all_write_tool_names() -> Vec<String> {
    let mut names = mcp004_write_tool_names();
    names.extend(mcp005_write_tool_names());
    names.extend(mcp007_write_tool_names());
    names.extend(mcp008_write_tool_names());
    names.extend(mcp009_write_tool_names());
    names.extend(mcp010c_write_tool_names());
    names.extend(mcp010f_write_tool_names());
    names.extend(optimization_write_tool_names());
    names.extend(agentic_orchestrator_write_tool_names());
    names.extend(agentic_action_write_tool_names());
    names.extend(cross_view_promotion_write_tool_names());
    names.extend(agentic_write_tool_names());
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
        tools.extend(optimization_tools::write_tools());
        tools.extend(agentic_orchestrator_tools::write_tools());
        tools.extend(agentic_action_tools::write_tools());
        tools.extend(cross_view_promotion_tools::write_tools());
        tools.extend(agentic_write_tools::write_tools());
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
            "topology_snapshot_get",
            "Read one complete, bounded Part topology projection from an admitted ArtifactReadback@2. IDs are artifact-bound only; this is evaluated triangulated GLB topology, not an authoring cage or visual-quality claim.",
            json!({
                "type":"object",
                "additionalProperties":false,
                "required":["schema_version","project_id","artifact_id","candidate_id","part_id","artifact_readback_sha256","program_sha256","operator_catalog_sha256","readback_config_sha256","snapshot_policy_sha256","max_face_count"],
                "properties":{
                    "schema_version":{"const":"TopologySnapshotRequest@1"},
                    "project_id":id_property(),
                    "artifact_id":{"type":"string","pattern":"^[0-9a-f]{64}$"},
                    "candidate_id":id_property(),
                    "part_id":id_property(),
                    "artifact_readback_sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"},
                    "program_sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"},
                    "operator_catalog_sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"},
                    "readback_config_sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"},
                    "snapshot_policy_sha256":{"const":"7d6b64a92c00841d80ec887542ff11b968fd387f7b5bdf5b4b4522a52ff1af28"},
                    "max_face_count":{"type":"integer","minimum":1,"maximum":512}
                }
            }),
            true,
        ),
        tool(
            "authoring_topology_get",
            "Read exact candidate-bound source V/E/Loop/Face data from one direct authoring-mesh@1 Part. This is a bounded structural read model, not evaluated GLB topology, BMesh, persistent editing or visual-quality evidence.",
            authoring_topology_request_schema(),
            true,
        ),
        tool(
            "authoring_mesh_edit_preview",
            "Apply one bounded translate-vertices or single-face-extrude edit to a transient candidate-bound authoring program and return deterministic Worker hashes/readback without writing CAS, candidates or versions.",
            authoring_mesh_edit_preview_schema(),
            true,
        ),
        tool(
            "mechanical_pose_evaluate",
            "Evaluate one candidate-bound rigid mechanical RestFrame and bounded scalar PoseAction at one integer tick or preview at most 16 ordered ticks. Returns structural local/world poses only; it never materializes geometry, skinning, a timeline or an animation asset.",
            mechanical_pose_evaluate_schema(),
            true,
        ),
        tool(
            "mechanical_pose_geometry_preview",
            "Compile one candidate-bound authored rigid-link pose as a transient derived GeometryProgram and strict fixed-Worker GLB readback. It writes no CAS, candidate or version and does not prove an original asset rig, animation system or visual quality.",
            mechanical_pose_geometry_preview_schema(),
            true,
        ),
        tool(
            "mechanical_animation_clip_get",
            "Read one immutable Runtime-owned rigid MechanicalAnimationClip and its exact candidate/artifact/source-Worker binding from SQLite and CAS. This is read-only and does not evaluate a frame or claim armature, skinning, timeline, NLA, F-Curve, driver or Python parity.",
            mechanical_animation_clip_get_schema(),
            true,
        ),
        tool(
            "game_asset_delivery_get",
            "Read and re-verify one Runtime-owned durable game delivery link and its exact LOD, collision, readiness and manifest CAS objects after restart. This is structural evidence only and does not claim automatic LOD generation, export or a Unity, Unreal or Godot round-trip.",
            game_asset_delivery_get_schema(),
            true,
        ),
        tool(
            "game_asset_lod_derive",
            "Derive deterministic LOD1 and LOD2 GeometryProgram variants from one exact durable geometry candidate by lowering only allowlisted typed tessellation parameters. Runtime compiles each level twice through the fixed Worker, writes no state, and fails when the 75/50 percent triangle targets are unreachable.",
            game_asset_lod_derive_schema(),
            true,
        ),
        tool(
            "appearance_source_lineage_get",
            "Read and re-verify one Runtime-owned durable Appearance source lineage sidecar, including the exact AppearanceProgram, GeometryProgram evidence, MaterialPack provenance, TextureBuild and optional surface-bake receipt, and three candidate-bound LOD ArtifactReadback inventories after restart.",
            appearance_source_lineage_get_schema(),
            true,
        ),
        tool(
            "game_weapon_anchor_get",
            "Read and re-verify one Runtime-owned fictional-weapon anchor metadata sidecar bound to an exact durable LOD delivery. This proves typed transforms and bindings only; it does not claim real GLB anchor nodes, pivots, hitboxes, ballistics or a commercial-engine import.",
            game_weapon_anchor_get_schema(),
            true,
        ),
        tool(
            "game_weapon_glb_socket_get",
            "Read and re-verify one Runtime-owned derived GLB socket materialization across exactly three LODs. The summary exposes only hash-bound readback, six named empty-node counts and structural truth flags; it never returns GLB bytes, claims a commercial-engine round-trip or reports visual quality.",
            game_weapon_glb_socket_get_schema(),
            true,
        ),
        tool(
            "game_weapon_animated_glb_socket_get",
            "Read and re-verify one Runtime-owned animated GLB socket materialization bound to the source MechanicalAnimationGlbReceipt, delivery LOD0 and AnchorSet. The summary exposes only animation/readable-content hash bindings, six named-node counts and structural truth flags; it never returns GLB bytes, claims a commercial-engine round-trip or reports visual quality.",
            game_weapon_animated_glb_socket_get_schema(),
            true,
        ),
        tool(
            "fictional_energy_vfx_get",
            "Read and re-verify one Runtime-owned fictional energy VFX intent profile bound to an exact delivery, anchor sidecar and allowlisted MaterialPack. This is structural sampled-emissive intent only; no material animation, bloom, particles, trails or commercial-engine round-trip has executed.",
            fictional_energy_vfx_get_schema(),
            true,
        ),
        tool(
            "fictional_energy_vfx_frame_sample",
            "Sample the two exact durable fictional energy emissive intent curves at one bounded integer tick using LINEAR interpolation, once-clamp and loop-modulo semantics. This read-only result does not prove a GLB MaterialZone binding, render a frame, write CAS or claim bloom, particles, trails or engine execution.",
            fictional_energy_vfx_frame_sample_schema(),
            true,
        ),
        tool(
            "fictional_energy_vfx_appearance_frame_sample",
            "Re-read three exact durable AppearanceProgram GLBs and sample the bound fictional energy emissive intent curves only after their MaterialPack, MaterialZone and stable material IDs match across every LOD. This read-only structural proof does not render a frame, write CAS or claim bloom, particles, trails or engine execution.",
            fictional_energy_vfx_appearance_frame_sample_schema(),
            true,
        ),
        tool(
            "fictional_energy_vfx_rendered_frame_get",
            "Re-read one Runtime-owned durable sampled-emissive LOD0 frame, its dedicated RenderSet and nine fixed AOV PNG bindings after restart. This proves deterministic structural rendering only, not animation sequence, bloom, particles, trails, engine execution or visual quality.",
            fictional_energy_vfx_rendered_frame_get_schema(),
            true,
        ),
        tool(
            "fictional_energy_vfx_rendered_sequence_get",
            "Re-read one Runtime-owned bounded sampled-emissive LOD0 sequence, its ordered independent frame links, fixed camera and nine-AOV receipts after restart. This proves same-cohort structural sequence evidence only, not engine material animation, bloom, particles, trails or visual quality.",
            fictional_energy_vfx_rendered_sequence_get_schema(),
            true,
        ),
        tool(
            "fictional_energy_vfx_hdr_bloom_get",
            "Re-read one Runtime-owned fixed-profile HDR bloom frame, its independent emissive-source and bloom-contribution PNGs, and the exact durable nine-AOV base-frame hash binding after restart. This proves bounded post-process evidence only; it does not claim particles, trails, engine execution or visual quality.",
            fictional_energy_vfx_hdr_bloom_get_schema(),
            true,
        ),
        tool(
            "fictional_energy_vfx_particles_get",
            "Re-read one Runtime-owned typed-particle frame, its three independent particle PNGs, exact LOD0 owner-node transforms, and unchanged base-nine-AOV plus Bloom hash bindings after restart. This is structural evidence only; it does not claim trails, GLB sockets, engine execution or visual quality.",
            fictional_energy_vfx_particles_get_schema(),
            true,
        ),
        tool(
            "fictional_energy_vfx_trails_get",
            "Re-read one Runtime-owned particle-history typed-trail frame, its independent trail color/ID/depth PNGs, ordered particle receipts and unchanged base-nine-AOV, Bloom and particle hash bindings after restart. V1 does not feed trails into Bloom and does not claim GLB sockets, engine execution or visual quality.",
            fictional_energy_vfx_trails_get_schema(),
            true,
        ),
        tool(
            "fictional_energy_vfx_trails_bloom_get",
            "Re-read one Runtime-owned fixed-profile typed-trail HDR Bloom frame with independent trail-emissive-source and trail-bloom-contribution PNGs. Inputs are the existing trail color/depth and current base opaque depth; base AOV, base Bloom, particle and source-trail passes are byte-exact reused. This does not report the original bloom_rendered flag, rerender particles or trails, invoke a commercial engine or claim visual quality.",
            fictional_energy_vfx_trails_bloom_get_schema(),
            true,
        ),
        tool(
            "mechanical_animation_clip_preview_get",
            "Read one scheduled tick from an immutable MechanicalAnimationClip and compile it twice through the fixed Geometry Worker as transient exact replay evidence. This writes no Runtime state and remains rigid-part structural evidence only.",
            mechanical_animation_clip_preview_schema(),
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
            "Validate a hash-free GeometryProgram@2 draft, expand one bounded ParametricDesignKitRequest@1 or immutable first-party ParametricDesignKitRequest@2 geometry-group template, lower or compare one ordered modifier stack, or evaluate one bounded regular quad-grid smooth/crease subdivision request; return Runtime-owned structural hashes without compiling, caching or persisting a candidate",
            json!({
                "type":"object",
                "additionalProperties":false,
                "properties":{
                    "schema_version":{"type":"string"},
                    "geometry_program_draft":{"type":"object"},
                    "project_id":id_property(),
                    "representation_plan_sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"},
                    "kit_id":{"type":"string"},
                    "template_id":{"type":"string"},
                    "instance_id":id_property(),
                    "part_id":id_property(),
                    "material_zone_id":id_property(),
                    "solid":{"type":"boolean"},
                    "base_node":{"type":"object"},
                    "modifiers":{"type":"array"},
                    "previous_evaluation":{"type":["object","null"]},
                    "intent":{"type":"object"},
                    "parameters":{"type":"object"},
                    "control_cage":{"type":"object"},
                    "crease_edges":{"type":"array"},
                    "policy":{"type":"object"},
                    "transform":{"type":"object"},
                    "budgets":{"type":"object"},
                    "input_sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"}
                },
                "oneOf":[
                    {
                        "type":"object",
                        "additionalProperties":false,
                        "required":["schema_version","geometry_program_draft"],
                        "properties":{
                            "schema_version":{"const":"GeometryProgramHashRequest@1"},
                            "geometry_program_draft":{"type":"object"}
                        }
                    },
                    parametric_group_request_branch_schema(),
                    {
                        "type":"object",
                        "additionalProperties":false,
                        "required":["schema_version","project_id","representation_plan_sha256","kit_id","part_id","material_zone_id","intent","input_sha256"],
                        "properties":{
                            "schema_version":{"const":"ParametricDesignKitRequest@1"},
                            "project_id":id_property(),
                            "representation_plan_sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"},
                            "kit_id":{"enum":["forgecad.kit.housing@1","forgecad.kit.panel@1","forgecad.kit.vent@1","forgecad.kit.joint@1","forgecad.kit.sensor@1","forgecad.kit.frame@1"]},
                            "part_id":id_property(),
                            "material_zone_id":id_property(),
                            "intent":{"type":"object"},
                            "input_sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"}
                        }
                    },
                    {
                        "type":"object",
                        "additionalProperties":false,
                        "required":["schema_version","project_id","representation_plan_sha256","part_id","material_zone_id","solid","base_node","modifiers","input_sha256"],
                        "properties":{
                            "schema_version":{"const":"GeometryModifierStackRequest@1"},
                            "project_id":id_property(),
                            "representation_plan_sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"},
                            "part_id":id_property(),
                            "material_zone_id":id_property(),
                            "solid":{"type":"boolean"},
                            "base_node":modifier_stack_base_node_schema(),
                            "modifiers":{
                                "type":"array",
                                "minItems":1,
                                "maxItems":8,
                                "items":modifier_stack_item_schema()
                            },
                            "input_sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"}
                        }
                    },
                    modifier_evaluation_request_schema(),
                    {
                        "type":"object",
                        "additionalProperties":false,
                        "required":["schema_version","project_id","representation_plan_sha256","part_id","material_zone_id","solid","control_cage","policy","transform","budgets","input_sha256"],
                        "properties":{
                            "schema_version":{"const":"SubdivisionEvaluationRequest@2"},
                            "project_id":id_property(),
                            "representation_plan_sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"},
                            "part_id":id_property(),
                            "material_zone_id":id_property(),
                            "solid":{"const":false},
                            "control_cage":{
                                "type":"object",
                                "additionalProperties":false,
                                "required":["u_points","v_points","control_points"],
                                "properties":{
                                    "u_points":{"type":"integer","minimum":2,"maximum":16},
                                    "v_points":{"type":"integer","minimum":2,"maximum":16},
                                    "control_points":{
                                        "type":"array","minItems":4,"maxItems":256,
                                        "items":{"type":"array","minItems":3,"maxItems":3,"items":{"type":"number","minimum":-10.0,"maximum":10.0}}
                                    }
                                }
                            },
                            "policy":{
                                "type":"object",
                                "additionalProperties":false,
                                "required":["scheme","subdivision_levels","boundary_interpolation","crease_mode","face_varying_interpolation","limit_surface","adaptive"],
                                "properties":{
                                    "scheme":{"const":"catmull-clark-uniform-regular-quad-grid"},
                                    "subdivision_levels":{"type":"integer","minimum":0,"maximum":2},
                                    "boundary_interpolation":{"const":"edge-and-corner"},
                                    "crease_mode":{"const":"unsupported"},
                                    "face_varying_interpolation":{"const":"worker-triangle-chart-postprocess"},
                                    "limit_surface":{"const":false},
                                    "adaptive":{"const":false}
                                }
                            },
                            "transform":{
                                "type":"object","additionalProperties":false,
                                "required":["position_m","rotation_rad"],
                                "properties":{
                                    "position_m":{"type":"array","minItems":3,"maxItems":3,"items":{"type":"number","minimum":-10.0,"maximum":10.0}},
                                    "rotation_rad":{"type":"array","minItems":3,"maxItems":3,"items":{"type":"number","minimum":-6.283185307179586,"maximum":6.283185307179586}}
                                }
                            },
                            "budgets":{
                                "type":"object","additionalProperties":false,
                                "required":["max_nodes","max_triangles","max_glb_bytes","max_worker_memory_bytes","max_runtime_ms"],
                                "properties":{
                                    "max_nodes":{"type":"integer","minimum":1,"maximum":512},
                                    "max_triangles":{"type":"integer","minimum":1,"maximum":250000},
                                    "max_glb_bytes":{"type":"integer","minimum":1,"maximum":67108864},
                                    "max_worker_memory_bytes":{"type":"integer","minimum":1,"maximum":536870912},
                                    "max_runtime_ms":{"type":"integer","minimum":1,"maximum":10000}
                                }
                            },
                            "input_sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"}
                        }
                    },
                    {
                        "type":"object",
                        "additionalProperties":false,
                        "required":["schema_version","project_id","representation_plan_sha256","part_id","material_zone_id","solid","control_cage","crease_edges","policy","transform","budgets","input_sha256"],
                        "properties":{
                            "schema_version":{"const":"SubdivisionCreaseEvaluationRequest@1"},
                            "project_id":id_property(),
                            "representation_plan_sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"},
                            "part_id":id_property(),
                            "material_zone_id":id_property(),
                            "solid":{"const":false},
                            "control_cage":{
                                "type":"object",
                                "additionalProperties":false,
                                "required":["u_points","v_points","control_points"],
                                "properties":{
                                    "u_points":{"type":"integer","minimum":3,"maximum":16},
                                    "v_points":{"type":"integer","minimum":3,"maximum":16},
                                    "control_points":{
                                        "type":"array","minItems":9,"maxItems":256,
                                        "items":{"type":"array","minItems":3,"maxItems":3,"items":{"type":"number","minimum":-10.0,"maximum":10.0}}
                                    }
                                }
                            },
                            "crease_edges":{
                                "description":"Runtime validates each edge against the regular-grid interior adjacency rules, then lexicographically normalizes the complete edge set before checking input_sha256.",
                                "type":"array","minItems":1,"maxItems":128,
                                "items":{
                                    "type":"object","additionalProperties":false,
                                    "required":["vertex_a","vertex_b","sharpness_levels"],
                                    "properties":{
                                        "vertex_a":{"type":"integer","minimum":0,"maximum":254},
                                        "vertex_b":{"type":"integer","minimum":1,"maximum":255},
                                        "sharpness_levels":{"type":"integer","minimum":1,"maximum":2}
                                    }
                                }
                            },
                            "policy":{
                                "type":"object",
                                "additionalProperties":false,
                                "required":["scheme","subdivision_levels","boundary_interpolation","crease_method","sharpness_domain","face_varying_interpolation","limit_surface","adaptive"],
                                "properties":{
                                    "scheme":{"const":"catmull-clark-uniform-regular-quad-grid"},
                                    "subdivision_levels":{"type":"integer","minimum":1,"maximum":2},
                                    "boundary_interpolation":{"const":"edge-only"},
                                    "crease_method":{"const":"uniform-integer-level-decay@1"},
                                    "sharpness_domain":{"const":"integer-levels-1-to-2"},
                                    "face_varying_interpolation":{"const":"worker-triangle-chart-postprocess"},
                                    "limit_surface":{"const":false},
                                    "adaptive":{"const":false}
                                }
                            },
                            "transform":{
                                "type":"object","additionalProperties":false,
                                "required":["position_m","rotation_rad"],
                                "properties":{
                                    "position_m":{"type":"array","minItems":3,"maxItems":3,"items":{"type":"number","minimum":-10.0,"maximum":10.0}},
                                    "rotation_rad":{"type":"array","minItems":3,"maxItems":3,"items":{"type":"number","minimum":-6.283185307179586,"maximum":6.283185307179586}}
                                }
                            },
                            "budgets":{
                                "type":"object","additionalProperties":false,
                                "required":["max_nodes","max_triangles","max_glb_bytes","max_worker_memory_bytes","max_runtime_ms"],
                                "properties":{
                                    "max_nodes":{"type":"integer","minimum":1,"maximum":512},
                                    "max_triangles":{"type":"integer","minimum":1,"maximum":250000},
                                    "max_glb_bytes":{"type":"integer","minimum":1,"maximum":67108864},
                                    "max_worker_memory_bytes":{"type":"integer","minimum":1,"maximum":536870912},
                                    "max_runtime_ms":{"type":"integer","minimum":1,"maximum":10000}
                                }
                            },
                            "input_sha256":{"description":"SHA-256 of the closed request without input_sha256 after Runtime lexicographically normalizes crease_edges.","type":"string","pattern":"^[0-9a-f]{64}$"}
                        }
                    }
                ]
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
                                    "allOf":[{"if":{"properties":{"semantic":{"const":"surface_control_point"}}},"then":{"required":["control_point_index","axis"]}}],
                                    "properties":{
                                        "parameter_id":id_property(),
                                        "part_id":id_property(),
                                        "semantic":{"enum":["width","height","depth","offset_x","offset_y","offset_z","scale","rotation_x","rotation_y","rotation_z","surface_control_point"]},
                                        "control_point_index":{"type":"integer","minimum":0,"maximum":255},
                                        "axis":{"enum":["x","y","z"]},
                                        "value":{"type":"number"},
                                        "min":{"type":"number"},
                                        "max":{"type":"number"},
                                        "step":{"type":"number","minimum":0},
                                        "unit":{"enum":["meter","radian","ratio"]}
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
            "Run a bounded deterministic camera and SilhouetteRig fit proposal against the reference mask; it never mutates the candidate.",
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
            "silhouette_evaluation_objective_prepare",
            "Create immutable Runtime-owned objective evidence binding the global target, refined Part target, PartError source, baseline candidate and fixed camera.",
            json!({
                "type":"object",
                "required":["project_id","baseline_candidate_id","global_target_sha256","part_target_sha256","part_id","source_part_error_sha256","camera"],
                "properties":{
                    "project_id":id_property(),
                    "baseline_candidate_id":id_property(),
                    "global_target_sha256":sha256_property(),
                    "part_target_sha256":sha256_property(),
                    "part_id":id_property(),
                    "source_part_error_sha256":sha256_property(),
                    "camera":{"type":"object"}
                },
                "additionalProperties":false
            }),
            true,
        ),
        tool(
            "silhouette_objective_compare",
            "Compare candidates under one unified global non-regression plus single-Part PartError promotion objective; never confirms or mutates a candidate.",
            json!({
                "type":"object",
                "required":["project_id","objective_sha256","candidate_ids"],
                "properties":{
                    "project_id":id_property(),
                    "objective_sha256":sha256_property(),
                    "candidate_ids":{"type":"array","minItems":2,"maxItems":8,"items":id_property()}
                },
                "additionalProperties":false
            }),
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
            "Read one compile-time allowlisted immutable offline MaterialPack manifest, texture hashes and color-space rules; omitting pack_id preserves the historical robot-pack default",
            json!({
                "type":"object",
                "properties":{
                    "pack_id":{
                        "type":"string",
                        "enum":["forgecad-hard-surface-robot","forgecad-fictional-energy-weapon","forgecad-fictional-energy-weapon-2k"]
                    }
                },
                "additionalProperties":false
            }),
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
            "job_result_get",
            "Read the completed result of a durable Runtime job from CAS; returns JOB_RESULT_PENDING while the worker is still running",
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
            "boolean_operand_lineage_preview",
            "Read a bounded fixed-Worker projection of evaluated Boolean triangle runs and their left/right operand source. Face IDs are evaluated identities, not original authoring faces, are not persisted in the current GLB, and do not prove visual quality.",
            json!({
                "type":"object",
                "additionalProperties":false,
                "required":["schema_version","geometry_program","boolean_node_id","max_lineage_runs","canonical_sha256"],
                "properties":{
                    "schema_version":{"const":"BooleanOperandLineageRequest@1"},
                    "geometry_program":{"type":"object","maxProperties":9},
                    "boolean_node_id":id_property(),
                    "max_lineage_runs":{"type":"integer","minimum":1,"maximum":4096},
                    "canonical_sha256":sha256_property()
                }
            }),
            true,
        ),
        tool(
            "subdivision_topology_lineage_preview",
            "Read a complete-within-scope fixed-Worker mapping from one exact subd-cage@2 control vertex/edge/quad root to evaluated quad topology. IDs are program/evaluation-bound, not GLB or artifact IDs; corner paths, weights, persistence and visual quality are explicitly unavailable.",
            json!({
                "type":"object",
                "additionalProperties":false,
                "required":["schema_version","geometry_program","subdivision_node_id","max_lineage_elements","canonical_sha256"],
                "properties":{
                    "schema_version":{"const":"SubdivisionTopologyLineageRequest@1"},
                    "geometry_program":{"type":"object","maxProperties":9},
                    "subdivision_node_id":id_property(),
                    "max_lineage_elements":{"type":"integer","minimum":1,"maximum":25000},
                    "canonical_sha256":sha256_property()
                }
            }),
            true,
        ),
        tool(
            "subdivision_artifact_lineage_get",
            "Read an exact candidate/artifact-bound Subdivision lineage projection. Runtime revalidates durable V2 evidence, replays the persisted program to byte-identical GLB, and maps evaluated quads to source-primitive-local triangles. The projection is not a persisted sidecar, exposes no glTF vertex/edge/corner IDs, and proves structural lineage only.",
            json!({
                "type":"object",
                "additionalProperties":false,
                "required":["schema_version","project_id","candidate_id","artifact_id","artifact_readback_sha256","subdivision_node_id","max_lineage_elements","canonical_sha256"],
                "properties":{
                    "schema_version":{"const":"SubdivisionArtifactLineageRequest@1"},
                    "project_id":id_property(),
                    "candidate_id":id_property(),
                    "artifact_id":sha256_property(),
                    "artifact_readback_sha256":sha256_property(),
                    "subdivision_node_id":id_property(),
                    "max_lineage_elements":{"type":"integer","minimum":1,"maximum":25000},
                    "canonical_sha256":sha256_property()
                }
            }),
            true,
        ),
        tool(
            "subdivision_artifact_lineage_sidecar_get",
            "Read one Runtime-owned Link@1 for the exact candidate/artifact-bound Subdivision lineage sidecar. This is a default read-only lookup: it does not write SQLite, CAS, candidate, version or sidecar state, and it does not promote reconstructed lineage into visual or package quality.",
            subdivision_artifact_lineage_sidecar_request_schema(),
            true,
        ),
        tool(
            "render_evidence_integrity_get",
            "Deeply verify one exact current candidate-bound render evidence cohort: camera and JSON CAS objects plus all nine 512x512 RGBA8 AOV hashes, sizes, order and color semantics. Read-only structural integrity only; historical receipts and visual quality are not repaired or promoted.",
            render_evidence_integrity_request_schema(),
            true,
        ),
        tool(
            "render_evidence_replay_get",
            "Re-run the fixed Render Worker twice for one exact integrity-bound artifact and camera, require the same Worker cohort, and compare all nine persisted AOV PNG and decoded RGBA8 bytes. Read-only structural replay only; no image bytes are returned and no visual-quality or Blender renderer parity is inferred.",
            json!({
                "type":"object",
                "additionalProperties":false,
                "required":["schema_version","candidate_state_sha256","integrity_request","replay_policy","canonical_sha256"],
                "properties":{
                    "schema_version":{"const":"RenderEvidenceReplayRequest@1"},
                    "candidate_state_sha256":sha256_property(),
                    "integrity_request":render_evidence_integrity_request_schema(),
                    "replay_policy":{"const":"fixed-worker-nine-aov-byte-replay-read-only@1"},
                    "canonical_sha256":sha256_property()
                }
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
    tools.extend(agentic_action_tools::read_tools());
    tools.extend(optimization_tools::read_tools());
    tools.extend(agentic_write_tools::read_tools());
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

fn render_evidence_integrity_request_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":[
            "schema_version","project_id","candidate_id","artifact_sha256","artifact_readback_object_sha256","program_sha256",
            "reference_id","reference_sha256","camera_hash","camera_object_sha256",
            "render_set_object_sha256","comparison_report_object_sha256",
            "quality_report_object_sha256","canonical_sha256"
        ],
        "properties":{
            "schema_version":{"const":"RenderEvidenceIntegrityRequest@1"},
            "project_id":id_property(),
            "candidate_id":id_property(),
            "artifact_sha256":sha256_property(),
            "artifact_readback_object_sha256":sha256_property(),
            "program_sha256":sha256_property(),
            "reference_id":id_property(),
            "reference_sha256":sha256_property(),
            "camera_hash":sha256_property(),
            "camera_object_sha256":sha256_property(),
            "render_set_object_sha256":sha256_property(),
            "comparison_report_object_sha256":sha256_property(),
            "quality_report_object_sha256":sha256_property(),
            "canonical_sha256":sha256_property()
        }
    })
}

fn subdivision_artifact_lineage_sidecar_request_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":[
            "schema_version","project_id","candidate_id","artifact_id",
            "artifact_readback_sha256","subdivision_node_id","max_lineage_elements",
            "canonical_sha256"
        ],
        "properties":{
            "schema_version":{"const":"SubdivisionArtifactLineageSidecarRequest@1"},
            "project_id":id_property(),
            "candidate_id":id_property(),
            "artifact_id":sha256_property(),
            "artifact_readback_sha256":sha256_property(),
            "subdivision_node_id":id_property(),
            "max_lineage_elements":{"type":"integer","minimum":1,"maximum":25000},
            "canonical_sha256":sha256_property()
        }
    })
}

fn authoring_topology_request_schema() -> Value {
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
            "authoring_topology_policy_sha256":{"const":"a6fb36a530e49537673b66d65ecb6e4fb4f51ffb3e7d01a0980be71f28cb367d"},
            "max_response_bytes":{"const":1048576}
        }
    })
}

fn authoring_mesh_edit_preview_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["schema_version","topology_request","base_topology_sha256","edit","edit_policy_sha256","input_sha256"],
        "properties":{
            "schema_version":{"const":"AuthoringMeshEditPreviewRequest@1"},
            "topology_request":authoring_topology_request_schema(),
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
                    }
                ]
            },
            "edit_policy_sha256":{"const":"1d050226b13848902f44bddb1b88c240cdfa86759703f804443b03964f8ddaae"},
            "input_sha256":sha256_property()
        }
    })
}

fn authoring_mesh_edit_prepare_schema() -> Value {
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
            "preview_request":authoring_mesh_edit_preview_schema(),
            "expected_preview_canonical_sha256":sha256_property(),
            "idempotency_key":{
                "type":"string",
                "pattern":"^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"
            },
            "max_response_bytes":{"const":1048576},
            "input_sha256":sha256_property()
        }
    })
}

fn mechanical_pose_evaluate_schema() -> Value {
    json!({
        "oneOf":[
            mechanical_pose_single_request_schema(),
            mechanical_pose_sequence_request_schema()
        ]
    })
}

fn mechanical_pose_geometry_preview_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["schema_version","pose_evaluation_request","preview_policy","input_sha256"],
        "properties":{
            "schema_version":{"const":"MechanicalPoseGeometryPreviewRequest@1"},
            "pose_evaluation_request":mechanical_pose_single_request_schema(),
            "preview_policy":{"const":"transient-derived-program-worker-readback@1"},
            "input_sha256":sha256_property()
        }
    })
}

fn mechanical_animation_clip_prepare_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["schema_version","clip_id","pose_sequence_request","clip_policy","input_sha256"],
        "properties":{
            "schema_version":{"const":"MechanicalAnimationClipPrepareRequest@1"},
            "clip_id":opaque_id_property(),
            "pose_sequence_request":mechanical_pose_sequence_request_schema(),
            "clip_policy":{"const":"runtime-owned-immutable-cas-rigid-mechanical-action@1"},
            "input_sha256":sha256_property()
        }
    })
}

fn mechanical_animation_clip_get_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["schema_version","project_id","candidate_id","clip_id","canonical_sha256"],
        "properties":{
            "schema_version":{"const":"MechanicalAnimationClipGetRequest@1"},
            "project_id":opaque_id_property(),
            "candidate_id":opaque_id_property(),
            "clip_id":opaque_id_property(),
            "canonical_sha256":sha256_property()
        }
    })
}

fn mechanical_animation_glb_prepare_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["schema_version","project_id","candidate_id","candidate_state_sha256","clip_id","materialization_policy","canonical_sha256"],
        "properties":{
            "schema_version":{"const":"MechanicalAnimationGlbPrepareRequest@1"},
            "project_id":opaque_id_property(),
            "candidate_id":opaque_id_property(),
            "candidate_state_sha256":sha256_property(),
            "clip_id":opaque_id_property(),
            "materialization_policy":{"const":"rigid-node-trs-gltf-linear-scheduled-samples@1"},
            "canonical_sha256":sha256_property()
        }
    })
}

fn game_asset_delivery_prepare_schema() -> Value {
    let lod = json!({
        "type":"object",
        "additionalProperties":false,
        "required":["level","candidate_id","candidate_state_sha256","artifact_sha256","artifact_readback_sha256"],
        "properties":{
            "level":{"type":"integer","minimum":0,"maximum":2},
            "candidate_id":opaque_id_property(),
            "candidate_state_sha256":sha256_property(),
            "artifact_sha256":sha256_property(),
            "artifact_readback_sha256":sha256_property()
        }
    });
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["schema_version","project_id","lods","animation","lod_policy","collision_policy","readiness_policy","canonical_sha256"],
        "properties":{
            "schema_version":{"const":"GameAssetDeliveryPrepareRequest@1"},
            "project_id":opaque_id_property(),
            "lods":{"type":"array","minItems":3,"maxItems":3,"items":lod},
            "animation":{"oneOf":[
                {"type":"null"},
                {
                    "type":"object",
                    "additionalProperties":false,
                    "required":["clip_id","animated_artifact_sha256","receipt_object_sha256"],
                    "properties":{
                        "clip_id":opaque_id_property(),
                        "animated_artifact_sha256":sha256_property(),
                        "receipt_object_sha256":sha256_property()
                    }
                }
            ]},
            "lod_policy":{"const":"authored-three-level-part-stable-progressive-triangles@1"},
            "collision_policy":{"const":"per-part-aabb-box-from-lod2-visual-geometry@1"},
            "readiness_policy":{"const":"engine-neutral-gltf2-embedded-assets-stable-names@1"},
            "canonical_sha256":sha256_property()
        }
    })
}

fn game_asset_delivery_get_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["schema_version","project_id","delivery_manifest_object_sha256"],
        "properties":{
            "schema_version":{"const":"GameAssetDeliveryGetRequest@1"},
            "project_id":opaque_id_property(),
            "delivery_manifest_object_sha256":sha256_property()
        }
    })
}

fn appearance_source_lineage_prepare_schema() -> Value {
    let lod = json!({
        "type":"object",
        "additionalProperties":false,
        "required":["level","candidate_id","candidate_state_sha256","artifact_sha256","artifact_readback_sha256"],
        "properties":{
            "level":{"type":"integer","minimum":0,"maximum":2},
            "candidate_id":opaque_id_property(),
            "candidate_state_sha256":sha256_property(),
            "artifact_sha256":sha256_property(),
            "artifact_readback_sha256":sha256_property()
        }
    });
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["schema_version","project_id","candidate_id","candidate_state_sha256","source_replay_worker_cohort_sha256","appearance_program","geometry_program_object_sha256","material_pack_manifest_sha256","texture_build_receipt_sha256","candidate_surface_bake_receipt_sha256","uv_binding_sha256","lods","canonical_sha256"],
        "properties":{
            "schema_version":{"const":"AppearanceSourceLineagePrepareRequest@1"},
            "project_id":opaque_id_property(),
            "candidate_id":opaque_id_property(),
            "candidate_state_sha256":sha256_property(),
            "source_replay_worker_cohort_sha256":sha256_property(),
            "appearance_program":{"type":"object"},
            "geometry_program_object_sha256":sha256_property(),
            "material_pack_manifest_sha256":sha256_property(),
            "texture_build_receipt_sha256":sha256_property(),
            "candidate_surface_bake_receipt_sha256":{"type":["string","null"],"pattern":"^[0-9a-f]{64}$"},
            "uv_binding_sha256":sha256_property(),
            "lods":{"type":"array","minItems":3,"maxItems":3,"items":lod},
            "canonical_sha256":sha256_property()
        }
    })
}

fn appearance_source_lineage_get_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["schema_version","project_id","candidate_id","appearance_program_sha256","canonical_sha256"],
        "properties":{
            "schema_version":{"const":"AppearanceSourceLineageGetRequest@1"},
            "project_id":opaque_id_property(),
            "candidate_id":opaque_id_property(),
            "appearance_program_sha256":sha256_property(),
            "canonical_sha256":sha256_property()
        }
    })
}

fn game_asset_lod_derive_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":[
            "schema_version","project_id","source_candidate_id",
            "source_candidate_state_sha256","source_artifact_sha256",
            "source_artifact_readback_sha256","source_geometry_program_sha256",
            "source_operator_catalog_sha256","source_readback_config_sha256",
            "derive_policy","canonical_sha256"
        ],
        "properties":{
            "schema_version":{"const":"GameAssetLodDeriveRequest@1"},
            "project_id":opaque_id_property(),
            "source_candidate_id":opaque_id_property(),
            "source_candidate_state_sha256":sha256_property(),
            "source_artifact_sha256":sha256_property(),
            "source_artifact_readback_sha256":sha256_property(),
            "source_geometry_program_sha256":sha256_property(),
            "source_operator_catalog_sha256":sha256_property(),
            "source_readback_config_sha256":sha256_property(),
            "derive_policy":{"const":"runtime-owned-typed-segment-lowering-lod1-half-lod2-quarter@1"},
            "canonical_sha256":sha256_property()
        }
    })
}

fn game_weapon_anchor_prepare_schema() -> Value {
    let anchor = json!({
        "type":"object",
        "additionalProperties":false,
        "required":["anchor_id","role","parent_kind","owner_part_id","local_translation_m","local_rotation_quat_xyzw","local_scale_xyz"],
        "properties":{
            "anchor_id":opaque_id_property(),
            "role":{"enum":["weapon-root","grip-primary","muzzle-vfx","magazine-well","sight-primary","energy-core-vfx"]},
            "parent_kind":{"enum":["synthetic-scene-root","part-node"]},
            "owner_part_id":{"oneOf":[{"type":"null"},opaque_id_property()]},
            "local_translation_m":{"type":"array","minItems":3,"maxItems":3,"items":{"type":"number","minimum":-10.0,"maximum":10.0}},
            "local_rotation_quat_xyzw":{"type":"array","minItems":4,"maxItems":4,"items":{"type":"number","minimum":-1.0,"maximum":1.0}},
            "local_scale_xyz":{"const":[1.0,1.0,1.0]}
        }
    });
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["schema_version","project_id","delivery_manifest_object_sha256","anchor_policy","anchors","canonical_sha256"],
        "properties":{
            "schema_version":{"const":"GameWeaponAnchorPrepareRequest@1"},
            "project_id":opaque_id_property(),
            "delivery_manifest_object_sha256":sha256_property(),
            "anchor_policy":{"const":"weapon-rh-x-forward-y-up-model-space-six-role@1"},
            "anchors":{"type":"array","minItems":6,"maxItems":6,"items":anchor},
            "canonical_sha256":sha256_property()
        }
    })
}

fn game_weapon_anchor_get_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["schema_version","project_id","delivery_manifest_object_sha256"],
        "properties":{
            "schema_version":{"const":"GameWeaponAnchorGetRequest@1"},
            "project_id":opaque_id_property(),
            "delivery_manifest_object_sha256":sha256_property()
        }
    })
}

fn game_weapon_glb_socket_prepare_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":[
            "schema_version",
            "project_id",
            "delivery_manifest_object_sha256",
            "anchor_set_object_sha256",
            "materialization_policy",
            "lod_scope",
            "canonical_sha256"
        ],
        "properties":{
            "schema_version":{"const":"GameWeaponGlbSocketMaterializationPrepareRequest@1"},
            "project_id":opaque_id_property(),
            "delivery_manifest_object_sha256":sha256_property(),
            "anchor_set_object_sha256":sha256_property(),
            "materialization_policy":{"const":"gltf-anchor-node-materialization-preserve-renderable-content@1"},
            "lod_scope":{"const":"lod0-lod1-lod2@1"},
            "canonical_sha256":sha256_property()
        }
    })
}

fn game_weapon_glb_socket_get_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["schema_version","project_id","socket_materialization_key_sha256"],
        "properties":{
            "schema_version":{"const":"GameWeaponGlbSocketMaterializationGetRequest@1"},
            "project_id":opaque_id_property(),
            "socket_materialization_key_sha256":sha256_property()
        }
    })
}

fn game_weapon_animated_glb_socket_prepare_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":[
            "schema_version",
            "project_id",
            "delivery_manifest_object_sha256",
            "anchor_set_object_sha256",
            "source_candidate_id",
            "source_candidate_state_sha256",
            "source_animated_artifact_sha256",
            "source_animation_receipt_object_sha256",
            "materialization_policy",
            "canonical_sha256"
        ],
        "properties":{
            "schema_version":{"const":"GameWeaponAnimatedGlbSocketMaterializationPrepareRequest@1"},
            "project_id":opaque_id_property(),
            "delivery_manifest_object_sha256":sha256_property(),
            "anchor_set_object_sha256":sha256_property(),
            "source_candidate_id":opaque_id_property(),
            "source_candidate_state_sha256":sha256_property(),
            "source_animated_artifact_sha256":sha256_property(),
            "source_animation_receipt_object_sha256":sha256_property(),
            "materialization_policy":{"const":"gltf-animated-anchor-node-materialization-preserve-animations-renderable-content@1"},
            "canonical_sha256":sha256_property()
        }
    })
}

fn game_weapon_animated_glb_socket_get_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["schema_version","project_id","animated_socket_materialization_key_sha256"],
        "properties":{
            "schema_version":{"const":"GameWeaponAnimatedGlbSocketMaterializationGetRequest@1"},
            "project_id":opaque_id_property(),
            "animated_socket_materialization_key_sha256":sha256_property()
        }
    })
}

fn fictional_energy_vfx_effect_schema() -> Value {
    json!({
        "type":"object","additionalProperties":false,
        "required":["effect_id","anchor_id","effect_kind","material_id","color_linear_rgb","duration_ticks","sample_time_ticks","emissive_strength_samples","loop_mode","lod_visibility"],
        "allOf":[
            {"if":{"properties":{"effect_id":{"const":"muzzle-pulse"}},"required":["effect_id"]},"then":{"properties":{"material_id":{"const":"energy-cyan-muzzle-emissive"}}}},
            {"if":{"properties":{"effect_id":{"const":"energy-core-breathe"}},"required":["effect_id"]},"then":{"properties":{"material_id":{"const":"energy-cyan-emissive"}}}}
        ],
        "properties":{
            "effect_id":opaque_id_property(),
            "anchor_id":{"enum":["socket-muzzle-vfx","socket-energy-core-vfx"]},
            "effect_kind":{"enum":["muzzle-emissive-pulse","energy-core-emissive-breathe"]},
            "material_id":{"enum":["energy-cyan-muzzle-emissive","energy-cyan-emissive"]},
            "color_linear_rgb":{"type":"array","minItems":3,"maxItems":3,"items":{"type":"number","minimum":0.0,"maximum":1.0}},
            "duration_ticks":{"type":"integer","minimum":1,"maximum":10000},
            "sample_time_ticks":{"type":"array","minItems":2,"maxItems":16,"uniqueItems":true,"items":{"type":"integer","minimum":0,"maximum":10000}},
            "emissive_strength_samples":{"type":"array","minItems":2,"maxItems":16,"items":{"type":"number","minimum":0.0,"maximum":16.0}},
            "loop_mode":{"enum":["once","loop"]},
            "lod_visibility":{"type":"array","minItems":3,"maxItems":3,"items":{"type":"boolean"}}
        }
    })
}

fn fictional_energy_vfx_prepare_schema() -> Value {
    json!({
        "type":"object","additionalProperties":false,
        "required":["schema_version","project_id","delivery_manifest_object_sha256","anchor_set_object_sha256","material_pack_id","material_pack_manifest_sha256","vfx_policy","effects","canonical_sha256"],
        "properties":{
            "schema_version":{"const":"FictionalEnergyVfxPrepareRequest@1"},
            "project_id":opaque_id_property(),
            "delivery_manifest_object_sha256":sha256_property(),
            "anchor_set_object_sha256":sha256_property(),
            "material_pack_id":{"const":"forgecad-fictional-energy-weapon-2k"},
            "material_pack_manifest_sha256":sha256_property(),
            "vfx_policy":{"const":"fictional-energy-two-effect-time-sampled-emissive-intent@1"},
            "effects":{"type":"array","minItems":2,"maxItems":2,"items":fictional_energy_vfx_effect_schema()},
            "canonical_sha256":sha256_property()
        }
    })
}

fn fictional_energy_vfx_get_schema() -> Value {
    json!({
        "type":"object","additionalProperties":false,
        "required":["schema_version","project_id","delivery_manifest_object_sha256"],
        "properties":{
            "schema_version":{"const":"FictionalEnergyVfxGetRequest@1"},
            "project_id":opaque_id_property(),
            "delivery_manifest_object_sha256":sha256_property()
        }
    })
}

fn fictional_energy_vfx_frame_sample_schema() -> Value {
    json!({
        "type":"object","additionalProperties":false,
        "required":["schema_version","project_id","delivery_manifest_object_sha256","vfx_profile_object_sha256","sample_time_ticks","sampling_policy","canonical_sha256"],
        "properties":{
            "schema_version":{"const":"FictionalEnergyVfxFrameSampleRequest@1"},
            "project_id":opaque_id_property(),
            "delivery_manifest_object_sha256":sha256_property(),
            "vfx_profile_object_sha256":sha256_property(),
            "sample_time_ticks":{"type":"integer","minimum":0,"maximum":1000000},
            "sampling_policy":{"const":"integer-tick-linear-once-clamp-loop-modulo-duration@1"},
            "canonical_sha256":sha256_property()
        }
    })
}

fn fictional_energy_vfx_appearance_frame_sample_schema() -> Value {
    json!({
        "type":"object","additionalProperties":false,
        "required":["schema_version","project_id","delivery_manifest_object_sha256","vfx_profile_object_sha256","sample_time_ticks","sampling_policy","appearance_binding_policy","canonical_sha256"],
        "properties":{
            "schema_version":{"const":"FictionalEnergyVfxAppearanceFrameSampleRequest@1"},
            "project_id":opaque_id_property(),
            "delivery_manifest_object_sha256":sha256_property(),
            "vfx_profile_object_sha256":sha256_property(),
            "sample_time_ticks":{"type":"integer","minimum":0,"maximum":1000000},
            "sampling_policy":{"const":"integer-tick-linear-once-clamp-loop-modulo-duration@1"},
            "appearance_binding_policy":{"const":"three-lod-appearance-program-glb-material-zone-stable-id@1"},
            "canonical_sha256":sha256_property()
        }
    })
}

fn fictional_energy_vfx_rendered_frame_prepare_schema() -> Value {
    json!({
        "type":"object","additionalProperties":false,
        "required":["schema_version","project_id","delivery_manifest_object_sha256","vfx_profile_object_sha256","sample_time_ticks","sampling_policy","appearance_binding_policy","effect_materialization_policy","lod_level","camera_policy","render_policy","canonical_sha256"],
        "properties":{
            "schema_version":{"const":"FictionalEnergyVfxFrameRenderPrepareRequest@1"},
            "project_id":opaque_id_property(),
            "delivery_manifest_object_sha256":sha256_property(),
            "vfx_profile_object_sha256":sha256_property(),
            "sample_time_ticks":{"type":"integer","minimum":0,"maximum":1000000},
            "sampling_policy":{"const":"integer-tick-linear-once-clamp-loop-modulo-duration@1"},
            "appearance_binding_policy":{"const":"three-lod-appearance-program-glb-material-zone-stable-id@1"},
            "effect_materialization_policy":{"const":"independent-effect-material-zone@1"},
            "lod_level":{"const":0},
            "camera_policy":{"const":"runtime-fixed-default-camera-calibration@1"},
            "render_policy":{"const":"lod0-nine-aov-double-worker-byte-exact-reservation-safe@1"},
            "canonical_sha256":sha256_property()
        }
    })
}

fn fictional_energy_vfx_rendered_frame_get_schema() -> Value {
    json!({
        "type":"object","additionalProperties":false,
        "required":["schema_version","project_id","frame_key_sha256"],
        "properties":{
            "schema_version":{"const":"FictionalEnergyVfxRenderedFrameGetRequest@1"},
            "project_id":opaque_id_property(),
            "frame_key_sha256":sha256_property()
        }
    })
}

fn fictional_energy_vfx_rendered_sequence_prepare_schema() -> Value {
    json!({
        "type":"object","additionalProperties":false,
        "required":["schema_version","project_id","delivery_manifest_object_sha256","vfx_profile_object_sha256","sample_time_ticks","sampling_policy","appearance_binding_policy","effect_materialization_policy","lod_level","camera_policy","render_policy","canonical_sha256"],
        "properties":{
            "schema_version":{"const":"FictionalEnergyVfxSequenceRenderPrepareRequest@1"},
            "project_id":opaque_id_property(),
            "delivery_manifest_object_sha256":sha256_property(),
            "vfx_profile_object_sha256":sha256_property(),
            "sample_time_ticks":{"type":"array","minItems":2,"maxItems":16,"items":{"type":"integer","minimum":0,"maximum":1000000}},
            "sampling_policy":{"const":"integer-tick-linear-once-clamp-loop-modulo-duration@1"},
            "appearance_binding_policy":{"const":"three-lod-appearance-program-glb-material-zone-stable-id@1"},
            "effect_materialization_policy":{"const":"independent-effect-material-zone@1"},
            "lod_level":{"const":0},
            "camera_policy":{"const":"runtime-fixed-default-camera-calibration@1"},
            "render_policy":{"const":"lod0-nine-aov-sequence-same-cohort-byte-exact-reservation-safe@1"},
            "canonical_sha256":sha256_property()
        }
    })
}

fn fictional_energy_vfx_rendered_sequence_get_schema() -> Value {
    json!({
        "type":"object","additionalProperties":false,
        "required":["schema_version","project_id","sequence_key_sha256"],
        "properties":{
            "schema_version":{"const":"FictionalEnergyVfxRenderedSequenceGetRequest@1"},
            "project_id":opaque_id_property(),
            "sequence_key_sha256":sha256_property()
        }
    })
}

fn fictional_energy_vfx_hdr_bloom_prepare_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["schema_version","project_id","delivery_manifest_object_sha256","vfx_profile_object_sha256","base_frame_key_sha256","bloom_profile","bloom_policy","canonical_sha256"],
        "properties":{
            "schema_version":{"const":"FictionalEnergyVfxHdrBloomFrameRenderPrepareRequest@1"},
            "project_id":opaque_id_property(),
            "delivery_manifest_object_sha256":sha256_property(),
            "vfx_profile_object_sha256":sha256_property(),
            "base_frame_key_sha256":sha256_property(),
            "bloom_profile":{
                "type":"object",
                "additionalProperties":false,
                "required":["threshold","radius_px","intensity","hdr_clamp"],
                "properties":{
                    "threshold":{"const":1.0},
                    "radius_px":{"const":8},
                    "intensity":{"const":4.0},
                    "hdr_clamp":{"const":16.0}
                }
            },
            "bloom_policy":{"const":"lod0-hdr-emissive-source-two-pass-fixed-kernel-base-aov-byte-exact@1"},
            "canonical_sha256":sha256_property()
        }
    })
}

fn fictional_energy_vfx_hdr_bloom_get_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["schema_version","project_id","bloom_key_sha256"],
        "properties":{
            "schema_version":{"const":"FictionalEnergyVfxHdrBloomFrameGetRequest@1"},
            "project_id":opaque_id_property(),
            "bloom_key_sha256":sha256_property()
        }
    })
}

fn fictional_energy_vfx_particles_prepare_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["schema_version","project_id","delivery_manifest_object_sha256","vfx_profile_object_sha256","anchor_set_object_sha256","base_frame_key_sha256","bloom_key_sha256","sample_time_ticks","particle_policy","emitter_policy","render_policy","canonical_sha256"],
        "properties":{
            "schema_version":{"const":"FictionalEnergyVfxParticlesFrameRenderPrepareRequest@1"},
            "project_id":opaque_id_property(),
            "delivery_manifest_object_sha256":sha256_property(),
            "vfx_profile_object_sha256":sha256_property(),
            "anchor_set_object_sha256":sha256_property(),
            "base_frame_key_sha256":sha256_property(),
            "bloom_key_sha256":sha256_property(),
            "sample_time_ticks":{"type":"integer","minimum":0,"maximum":1000000},
            "particle_policy":{"const":"two-closed-emitters-hash-seeded-typed-attributes@1"},
            "emitter_policy":{"const":"muzzle-burst-24-energy-core-sparks-32@1"},
            "render_policy":{"const":"lod0-three-typed-particle-aov-depth-tested-base-bloom-byte-exact@1"},
            "canonical_sha256":sha256_property()
        }
    })
}

fn fictional_energy_vfx_particles_get_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["schema_version","project_id","particle_key_sha256"],
        "properties":{
            "schema_version":{"const":"FictionalEnergyVfxParticlesFrameGetRequest@1"},
            "project_id":opaque_id_property(),
            "particle_key_sha256":sha256_property()
        }
    })
}

fn fictional_energy_vfx_trails_prepare_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["schema_version","project_id","delivery_manifest_object_sha256","vfx_profile_object_sha256","anchor_set_object_sha256","base_frame_key_sha256","bloom_key_sha256","current_particle_key_sha256","particle_history_key_sha256s","sample_time_ticks","trail_policy","history_policy","render_policy","bloom_input","canonical_sha256"],
        "properties":{
            "schema_version":{"const":"FictionalEnergyVfxTrailsFrameRenderPrepareRequest@1"},
            "project_id":opaque_id_property(),
            "delivery_manifest_object_sha256":sha256_property(),
            "vfx_profile_object_sha256":sha256_property(),
            "anchor_set_object_sha256":sha256_property(),
            "base_frame_key_sha256":sha256_property(),
            "bloom_key_sha256":sha256_property(),
            "current_particle_key_sha256":sha256_property(),
            "particle_history_key_sha256s":{"type":"array","minItems":1,"maxItems":4,"uniqueItems":true,"items":sha256_property()},
            "sample_time_ticks":{"type":"integer","minimum":0,"maximum":1000000},
            "trail_policy":{"const":"two-closed-history-bound-polyline-trails@1"},
            "history_policy":{"const":"one-to-four-strictly-earlier-particle-frames@1"},
            "render_policy":{"const":"lod0-three-typed-trail-aov-depth-tested-base-bloom-particles-byte-exact-no-bloom-input@1"},
            "bloom_input":{"const":false},
            "canonical_sha256":sha256_property()
        }
    })
}

fn fictional_energy_vfx_trails_get_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["schema_version","project_id","trail_key_sha256"],
        "properties":{
            "schema_version":{"const":"FictionalEnergyVfxTrailsFrameGetRequest@1"},
            "project_id":opaque_id_property(),
            "trail_key_sha256":sha256_property()
        }
    })
}

fn fictional_energy_vfx_trails_bloom_prepare_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":[
            "schema_version",
            "project_id",
            "delivery_manifest_object_sha256",
            "vfx_profile_object_sha256",
            "anchor_set_object_sha256",
            "base_frame_key_sha256",
            "bloom_key_sha256",
            "source_trail_key_sha256",
            "trail_bloom_profile",
            "trail_bloom_policy",
            "input_policy",
            "occlusion_policy",
            "render_policy",
            "canonical_sha256"
        ],
        "properties":{
            "schema_version":{"const":"FictionalEnergyVfxTrailsBloomFrameRenderPrepareRequest@1"},
            "project_id":opaque_id_property(),
            "delivery_manifest_object_sha256":sha256_property(),
            "vfx_profile_object_sha256":sha256_property(),
            "anchor_set_object_sha256":sha256_property(),
            "base_frame_key_sha256":sha256_property(),
            "bloom_key_sha256":sha256_property(),
            "source_trail_key_sha256":sha256_property(),
            "trail_bloom_profile":{
                "type":"object",
                "additionalProperties":false,
                "required":["threshold","source_gain","radius_px","intensity","hdr_clamp","blur_passes","kernel"],
                "properties":{
                    "threshold":{"const":1.0},
                    "source_gain":{"const":8.0},
                    "radius_px":{"const":8},
                    "intensity":{"const":4.0},
                    "hdr_clamp":{"const":16.0},
                    "blur_passes":{"const":2},
                    "kernel":{"const":"separable-box-two-pass-fixed-radius@1"}
                }
            },
            "trail_bloom_policy":{"const":"lod0-typed-trails-hdr-source-two-pass-fixed-kernel@1"},
            "input_policy":{"const":"existing-trail-color-depth-plus-current-base-opaque-depth-byte-exact@1"},
            "occlusion_policy":{"const":"current-base-opaque-depth-before-trail-depth-reversed-normalized-u8-epsilon-1e-4@1"},
            "render_policy":{"const":"lod0-trail-bloom-two-new-passes-base-bloom-particles-trails-byte-exact-reused@1"},
            "canonical_sha256":sha256_property()
        }
    })
}

fn fictional_energy_vfx_trails_bloom_get_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["schema_version","project_id","trail_bloom_key_sha256"],
        "properties":{
            "schema_version":{"const":"FictionalEnergyVfxTrailsBloomFrameGetRequest@1"},
            "project_id":opaque_id_property(),
            "trail_bloom_key_sha256":sha256_property()
        }
    })
}

fn mechanical_animation_clip_preview_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["schema_version","project_id","candidate_id","clip_id","sample_time_ticks","preview_policy","canonical_sha256"],
        "properties":{
            "schema_version":{"const":"MechanicalAnimationClipPreviewRequest@1"},
            "project_id":opaque_id_property(),
            "candidate_id":opaque_id_property(),
            "clip_id":opaque_id_property(),
            "sample_time_ticks":{"type":"integer","minimum":0,"maximum":1000000},
            "preview_policy":{"const":"single-tick-transient-double-worker-replay@1"},
            "canonical_sha256":sha256_property()
        }
    })
}

fn mechanical_pose_single_request_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":[
            "schema_version","project_id","artifact_id","candidate_id",
            "artifact_readback_sha256","program_sha256","operator_catalog_sha256",
            "readback_config_sha256","rest_frame_draft","pose_action_draft",
            "sample_time_ticks","input_sha256"
        ],
        "properties":{
            "schema_version":{"const":"MechanicalPoseEvaluationRequest@1"},
            "project_id":opaque_id_property(),
            "artifact_id":sha256_property(),
            "candidate_id":opaque_id_property(),
            "artifact_readback_sha256":sha256_property(),
            "program_sha256":sha256_property(),
            "operator_catalog_sha256":sha256_property(),
            "readback_config_sha256":sha256_property(),
            "rest_frame_draft":mechanical_rest_frame_draft_schema(),
            "pose_action_draft":{"oneOf":[{"type":"null"},mechanical_pose_action_draft_schema()]},
            "sample_time_ticks":{"type":"integer","minimum":0,"maximum":1000000},
            "input_sha256":sha256_property()
        }
    })
}

fn mechanical_pose_sequence_request_schema() -> Value {
    let mut schema = mechanical_pose_single_request_schema();
    schema["properties"]["schema_version"] =
        json!({"const":"MechanicalPoseSequencePreviewRequest@1"});
    schema["properties"]["sample_time_ticks"] = json!({
        "type":"array",
        "minItems":1,
        "maxItems":16,
        "uniqueItems":true,
        "items":{"type":"integer","minimum":0,"maximum":1000000}
    });
    schema
}

fn opaque_id_property() -> Value {
    json!({"type":"string","pattern":"^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"})
}

fn mechanical_rest_frame_draft_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":[
            "schema_version","rest_frame_id","coordinate_system","transform_convention",
            "root_link_id","links","parent_map"
        ],
        "properties":{
            "schema_version":{"const":"MechanicalRestFrameDraft@1"},
            "rest_frame_id":opaque_id_property(),
            "coordinate_system":{"const":"forgecad-rh-y-up-m@1"},
            "transform_convention":{"const":"column-vector-trs-quaternion@1"},
            "root_link_id":opaque_id_property(),
            "links":{
                "type":"array","minItems":1,"maxItems":64,
                "items":mechanical_link_draft_schema()
            },
            "parent_map":{
                "type":"array","minItems":0,"maxItems":63,
                "items":{
                    "type":"object","additionalProperties":false,
                    "required":["child_link_id","parent_link_id"],
                    "properties":{
                        "child_link_id":opaque_id_property(),
                        "parent_link_id":opaque_id_property()
                    }
                }
            }
        }
    })
}

fn mechanical_link_draft_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":[
            "link_id","part_id","source_node_ids","joint_type","rest_translation_m",
            "rest_rotation_quat_xyzw","axis_local","limit_min","limit_max","value_unit"
        ],
        "properties":{
            "link_id":opaque_id_property(),
            "part_id":opaque_id_property(),
            "source_node_ids":{
                "type":"array","minItems":1,"maxItems":16,"items":opaque_id_property()
            },
            "joint_type":{"enum":["fixed","revolute","prismatic"]},
            "rest_translation_m":bounded_array_schema(3,-10.0,10.0),
            "rest_rotation_quat_xyzw":bounded_array_schema(4,-1.0,1.0),
            "axis_local":{"oneOf":[{"type":"null"},bounded_array_schema(3,-1.0,1.0)]},
            "limit_min":{"oneOf":[{"type":"null"},{"type":"number","minimum":-3.141592653589793,"maximum":3.141592653589793}]},
            "limit_max":{"oneOf":[{"type":"null"},{"type":"number","minimum":-3.141592653589793,"maximum":3.141592653589793}]},
            "value_unit":{"enum":["none","radian","meter"]}
        }
    })
}

fn mechanical_pose_action_draft_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":[
            "schema_version","action_id","timebase_hz","duration_ticks","interpolation",
            "extrapolation","unkeyed_policy","channels"
        ],
        "properties":{
            "schema_version":{"const":"MechanicalPoseActionDraft@1"},
            "action_id":opaque_id_property(),
            "timebase_hz":{"const":1000},
            "duration_ticks":{"type":"integer","minimum":1,"maximum":1000000},
            "interpolation":{"const":"linear@1"},
            "extrapolation":{"const":"clamp@1"},
            "unkeyed_policy":{"const":"rest@1"},
            "channels":{
                "type":"array","minItems":1,"maxItems":64,
                "items":{
                    "type":"object","additionalProperties":false,
                    "required":["link_id","value_unit","keys"],
                    "properties":{
                        "link_id":opaque_id_property(),
                        "value_unit":{"enum":["radian","meter"]},
                        "keys":{
                            "type":"array","minItems":1,"maxItems":32,
                            "items":{
                                "type":"object","additionalProperties":false,
                                "required":["time_ticks","value"],
                                "properties":{
                                    "time_ticks":{"type":"integer","minimum":0,"maximum":1000000},
                                    "value":{"type":"number","minimum":-3.141592653589793,"maximum":3.141592653589793}
                                }
                            }
                        }
                    }
                }
            }
        }
    })
}

fn bounded_array_schema(length: usize, minimum: f64, maximum: f64) -> Value {
    json!({
        "type":"array","minItems":length,"maxItems":length,
        "items":{"type":"number","minimum":minimum,"maximum":maximum}
    })
}

fn parametric_group_request_branch_schema() -> Value {
    let common = |template_id: &str, parameters: Value| {
        json!({
            "type":"object",
            "additionalProperties":false,
            "required":["schema_version","project_id","representation_plan_sha256","template_id","instance_id","part_id","material_zone_id","parameters","input_sha256"],
            "properties":{
                "schema_version":{"const":"ParametricDesignKitRequest@2"},
                "project_id":opaque_id_property(),
                "representation_plan_sha256":sha256_property(),
                "template_id":{"const":template_id},
                "instance_id":opaque_id_property(),
                "part_id":opaque_id_property(),
                "material_zone_id":opaque_id_property(),
                "parameters":parameters,
                "input_sha256":sha256_property()
            }
        })
    };
    json!({
        "oneOf":[
            common("forgecad.group.rounded-box@1", json!({
                "type":"object","additionalProperties":false,
                "required":["size_m","position_m","rotation_rad","bevel_width_m","bevel_segments","bevel_profile","crease_angle_rad"],
                "properties":{
                    "size_m":bounded_vec3_schema(0.0,10.0,true),
                    "position_m":bounded_vec3_schema(-10.0,10.0,false),
                    "rotation_rad":bounded_vec3_schema(-6.283185307179586,6.283185307179586,false),
                    "bevel_width_m":{"type":"number","exclusiveMinimum":0.0,"maximum":5.0},
                    "bevel_segments":{"type":"integer","minimum":1,"maximum":4},
                    "bevel_profile":{"type":"number","minimum":0.25,"maximum":0.75},
                    "crease_angle_rad":{"type":"number","minimum":0.0,"maximum":3.141592653589793}
                }
            })),
            common("forgecad.group.mirrored-box@1", json!({
                "type":"object","additionalProperties":false,
                "required":["size_m","position_m","rotation_rad","mirror_axis","mirror_offset_m","crease_angle_rad"],
                "properties":{
                    "size_m":bounded_vec3_schema(0.0,10.0,true),
                    "position_m":bounded_vec3_schema(-10.0,10.0,false),
                    "rotation_rad":bounded_vec3_schema(-6.283185307179586,6.283185307179586,false),
                    "mirror_axis":{"enum":["x","y","z"]},
                    "mirror_offset_m":{"type":"number","minimum":-10.0,"maximum":10.0},
                    "crease_angle_rad":{"type":"number","minimum":0.0,"maximum":3.141592653589793}
                }
            })),
            common("forgecad.group.arrayed-cylinder@1", json!({
                "type":"object","additionalProperties":false,
                "required":["radius_m","height_m","radial_segments","position_m","rotation_rad","array_count","array_offset_m","crease_angle_rad"],
                "properties":{
                    "radius_m":{"type":"number","exclusiveMinimum":0.0,"maximum":5.0},
                    "height_m":{"type":"number","exclusiveMinimum":0.0,"maximum":10.0},
                    "radial_segments":{"type":"integer","minimum":8,"maximum":64},
                    "position_m":bounded_vec3_schema(-10.0,10.0,false),
                    "rotation_rad":bounded_vec3_schema(-6.283185307179586,6.283185307179586,false),
                    "array_count":{"type":"integer","minimum":1,"maximum":32},
                    "array_offset_m":bounded_vec3_schema(-10.0,10.0,false),
                    "crease_angle_rad":{"type":"number","minimum":0.0,"maximum":3.141592653589793}
                }
            }))
        ]
    })
}

fn modifier_stack_item_schema() -> Value {
    json!({
        "oneOf":[
            modifier_stack_variant_schema(
                "forgecad.geometry.transform@2",
                json!({
                    "type":"object","additionalProperties":false,
                    "required":["shape","translation_m","rotation_rad","scale"],
                    "properties":{
                        "shape":{"const":"transform"},
                        "translation_m":bounded_vec3_schema(-10.0, 10.0, false),
                        "rotation_rad":bounded_vec3_schema(-6.283185307179586, 6.283185307179586, false),
                        "scale":bounded_vec3_schema(0.0, 10.0, true)
                    }
                })
            ),
            modifier_stack_variant_schema(
                "forgecad.geometry.mirror@1",
                json!({
                    "type":"object","additionalProperties":false,
                    "required":["shape","axis","offset_m"],
                    "properties":{
                        "shape":{"const":"mirror"},
                        "axis":{"enum":["x","y","z"]},
                        "offset_m":{"type":"number","minimum":-10.0,"maximum":10.0}
                    }
                })
            ),
            modifier_stack_variant_schema(
                "forgecad.geometry.array@1",
                json!({
                    "type":"object","additionalProperties":false,
                    "required":["shape","count","offset_m"],
                    "properties":{
                        "shape":{"const":"array"},
                        "count":{"type":"integer","minimum":1,"maximum":32},
                        "offset_m":bounded_vec3_schema(-10.0, 10.0, false)
                    }
                })
            ),
            modifier_stack_variant_schema(
                "forgecad.geometry.bevel@1",
                json!({
                    "type":"object","additionalProperties":false,
                    "required":["shape","width_m","segments","profile","edge_scope","clamp_overlap"],
                    "properties":{
                        "shape":{"const":"bevel"},
                        "width_m":bounded_number_schema(0.0, 5.0, true),
                        "segments":{"type":"integer","minimum":1,"maximum":4},
                        "profile":{"type":"number","minimum":0.25,"maximum":0.75},
                        "edge_scope":{"const":"all-source-box-edges"},
                        "clamp_overlap":{"type":"boolean"}
                    }
                })
            ),
            modifier_stack_variant_schema(
                "forgecad.geometry.normal-policy@1",
                json!({
                    "type":"object","additionalProperties":false,
                    "required":["shape","weighting","crease_angle_rad","keep_sharp","output_domain"],
                    "properties":{
                        "shape":{"const":"normal-policy"},
                        "weighting":{"const":"face-area-x-corner-angle"},
                        "crease_angle_rad":{"type":"number","minimum":0.0,"maximum":3.141592653589793},
                        "keep_sharp":{"const":true},
                        "output_domain":{"const":"corner"}
                    }
                })
            )
        ]
    })
}

fn modifier_evaluation_request_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["schema_version","project_id","representation_plan_sha256","part_id","material_zone_id","solid","base_node","modifiers","previous_evaluation","input_sha256"],
        "properties":{
            "schema_version":{"const":"GeometryModifierEvaluationRequest@2"},
            "project_id":id_property(),
            "representation_plan_sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"},
            "part_id":id_property(),
            "material_zone_id":id_property(),
            "solid":{"type":"boolean"},
            "base_node":modifier_stack_base_node_schema(),
            "modifiers":{
                "type":"array",
                "minItems":1,
                "maxItems":8,
                "items":modifier_stack_item_schema()
            },
            "previous_evaluation":{
                "oneOf":[
                    {"type":"null"},
                    modifier_evaluation_signature_schema()
                ]
            },
            "input_sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"}
        }
    })
}

fn modifier_apply_request_schema() -> Value {
    json!({
        "oneOf":[
            modifier_apply_request_v1_schema(),
            modifier_apply_request_v2_schema()
        ]
    })
}

fn modifier_apply_request_v1_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":[
            "schema_version","project_id","source_candidate_id",
            "source_candidate_canonical_sha256","source_artifact_sha256",
            "source_artifact_readback_sha256",
            "source_geometry_program_sha256","source_operator_catalog_sha256",
            "source_readback_config_sha256","source_part_id","base_version_id",
            "modifiers","idempotency_key","max_response_bytes","input_sha256"
        ],
        "properties":{
            "schema_version":{"const":"GeometryModifierApplyRequest@1"},
            "project_id":id_property(),
            "source_candidate_id":id_property(),
            "source_candidate_canonical_sha256":sha256_property(),
            "source_artifact_sha256":sha256_property(),
            "source_artifact_readback_sha256":sha256_property(),
            "source_geometry_program_sha256":sha256_property(),
            "source_operator_catalog_sha256":sha256_property(),
            "source_readback_config_sha256":sha256_property(),
            "source_part_id":id_property(),
            "base_version_id":nullable_id_property(),
            "modifiers":{
                "type":"array",
                "minItems":1,
                "maxItems":8,
                "items":modifier_stack_item_schema()
            },
            "idempotency_key":id_property(),
            "max_response_bytes":{"const":1048576},
            "input_sha256":sha256_property()
        }
    })
}

fn modifier_apply_request_v2_schema() -> Value {
    // This is deliberately a separate closed request rather than an additive
    // widening of @1.  @2 is the candidate-bound single-edge bevel slice:
    // exactly one stable source edge on one direct authoring-mesh@1 Part.
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":[
            "schema_version","project_id","source_candidate_id",
            "source_candidate_canonical_sha256","source_artifact_sha256",
            "source_artifact_readback_sha256","source_geometry_program_sha256",
            "source_operator_catalog_sha256","source_readback_config_sha256",
            "source_part_id","source_terminal_node_id",
            "source_authoring_topology_sha256","source_edge_id","bevel_m",
            "segments","profile","clamp_overlap","base_version_id",
            "idempotency_key","max_response_bytes","input_sha256"
        ],
        "properties":{
            "schema_version":{"const":"GeometryModifierApplyRequest@2"},
            "project_id":id_property(),
            "source_candidate_id":id_property(),
            "source_candidate_canonical_sha256":sha256_property(),
            "source_artifact_sha256":sha256_property(),
            "source_artifact_readback_sha256":sha256_property(),
            "source_geometry_program_sha256":sha256_property(),
            "source_operator_catalog_sha256":sha256_property(),
            "source_readback_config_sha256":sha256_property(),
            "source_part_id":id_property(),
            "source_terminal_node_id":id_property(),
            "source_authoring_topology_sha256":sha256_property(),
            "source_edge_id":id_property(),
            "bevel_m":{"type":"number","exclusiveMinimum":0.0,"maximum":0.25},
            "segments":{"type":"integer","minimum":1,"maximum":4},
            "profile":{"type":"number","minimum":0.25,"maximum":0.75},
            "clamp_overlap":{"type":"boolean"},
            "base_version_id":nullable_id_property(),
            "idempotency_key":id_property(),
            "max_response_bytes":{"const":1048576},
            "input_sha256":sha256_property()
        }
    })
}

fn modifier_evaluation_signature_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":[
            "schema_version","project_id","representation_plan_sha256","part_id",
            "material_zone_id","solid","source_input_sha256","stack_definition_sha256",
            "evaluation_sha256","output_sha256","evaluation_policy_sha256",
            "operator_catalog_sha256","catalog_cohort_sha256","cache_key_sha256",
            "stages","canonical_sha256"
        ],
        "properties":{
            "schema_version":{"const":"GeometryModifierEvaluationSignature@1"},
            "project_id":id_property(),
            "representation_plan_sha256":sha256_property(),
            "part_id":id_property(),
            "material_zone_id":id_property(),
            "solid":{"type":"boolean"},
            "source_input_sha256":sha256_property(),
            "stack_definition_sha256":sha256_property(),
            "evaluation_sha256":sha256_property(),
            "output_sha256":sha256_property(),
            "evaluation_policy_sha256":sha256_property(),
            "operator_catalog_sha256":sha256_property(),
            "catalog_cohort_sha256":sha256_property(),
            "cache_key_sha256":sha256_property(),
            "stages":{
                "type":"array",
                "minItems":1,
                "maxItems":8,
                "items":{
                    "type":"object",
                    "additionalProperties":false,
                    "required":["order_index","modifier_id","enabled","operator_id","parameters_sha256","definition_sha256","input_evaluation_sha256","output_evaluation_sha256","stage_cache_key_sha256"],
                    "properties":{
                        "order_index":{"type":"integer","minimum":0,"maximum":7},
                        "modifier_id":id_property(),
                        "enabled":{"type":"boolean"},
                        "operator_id":{"enum":["forgecad.geometry.transform@2","forgecad.geometry.mirror@1","forgecad.geometry.array@1","forgecad.geometry.bevel@1","forgecad.geometry.normal-policy@1"]},
                        "parameters_sha256":sha256_property(),
                        "definition_sha256":sha256_property(),
                        "input_evaluation_sha256":sha256_property(),
                        "output_evaluation_sha256":sha256_property(),
                        "stage_cache_key_sha256":sha256_property()
                    }
                }
            },
            "canonical_sha256":sha256_property()
        }
    })
}

fn modifier_stack_variant_schema(operator_id: &str, parameters: Value) -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["modifier_id","enabled","operator_id","parameters"],
        "properties":{
            "modifier_id":id_property(),
            "enabled":{"type":"boolean"},
            "operator_id":{"const":operator_id},
            "parameters":parameters
        }
    })
}

fn modifier_stack_base_node_schema() -> Value {
    json!({
        "oneOf":[
            modifier_stack_base_node_variant_schema("forgecad.geometry.primitive@2", modifier_stack_primitive_parameters_schema()),
            modifier_stack_base_node_variant_schema("forgecad.geometry.profile-extrude@1", modifier_stack_profile_extrude_parameters_schema()),
            modifier_stack_base_node_variant_schema("forgecad.geometry.profile-loft@1", modifier_stack_profile_loft_parameters_schema()),
            modifier_stack_base_node_variant_schema("forgecad.geometry.longitudinal-section-loft@1", modifier_stack_longitudinal_loft_parameters_schema()),
            modifier_stack_base_node_variant_schema("forgecad.geometry.subd-cage@1", modifier_stack_subd_cage_parameters_schema()),
            modifier_stack_base_node_variant_schema("forgecad.geometry.surface-patch@1", modifier_stack_surface_patch_parameters_schema()),
            modifier_stack_base_node_variant_schema("forgecad.geometry.revolve@1", modifier_stack_revolve_parameters_schema()),
            modifier_stack_base_node_variant_schema("forgecad.geometry.tube-sweep@1", modifier_stack_tube_sweep_parameters_schema()),
            modifier_stack_base_node_variant_schema("forgecad.geometry.panel@1", modifier_stack_panel_parameters_schema()),
            modifier_stack_base_node_variant_schema("forgecad.geometry.vent-array@1", modifier_stack_vent_array_parameters_schema()),
            modifier_stack_base_node_variant_schema("forgecad.geometry.vent-array@2", modifier_stack_vent_array_v2_parameters_schema()),
            modifier_stack_base_node_variant_schema("forgecad.geometry.recessed-channel@1", modifier_stack_recessed_channel_parameters_schema()),
            modifier_stack_base_node_variant_schema("forgecad.geometry.joint-stack@1", modifier_stack_joint_stack_parameters_schema())
        ]
    })
}

fn modifier_stack_base_node_variant_schema(operator_id: &str, parameters: Value) -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["node_id","operator_id","inputs","parameters"],
        "properties":{
            "node_id":id_property(),
            "operator_id":{"const":operator_id},
            "inputs":{"type":"array","maxItems":0},
            "parameters":parameters
        }
    })
}

fn modifier_stack_primitive_parameters_schema() -> Value {
    json!({
        "oneOf":[
            closed_parameters_schema(
                &["shape","size_m","position_m","rotation_rad"],
                json!({
                    "shape":{"const":"box"},
                    "size_m":bounded_vec3_schema(0.0, 10.0, true),
                    "position_m":bounded_vec3_schema(-10.0, 10.0, false),
                    "rotation_rad":bounded_vec3_schema(-6.283185307179586, 6.283185307179586, false)
                })
            ),
            closed_parameters_schema(
                &["shape","radius_m","height_m","radial_segments","position_m","rotation_rad"],
                json!({
                    "shape":{"const":"cylinder"},
                    "radius_m":bounded_number_schema(0.0, 5.0, true),
                    "height_m":bounded_number_schema(0.0, 10.0, true),
                    "radial_segments":{"type":"integer","minimum":8,"maximum":64},
                    "position_m":bounded_vec3_schema(-10.0, 10.0, false),
                    "rotation_rad":bounded_vec3_schema(-6.283185307179586, 6.283185307179586, false)
                })
            ),
            closed_parameters_schema(
                &["shape","radii_m","longitude_segments","latitude_segments","position_m","rotation_rad"],
                json!({
                    "shape":{"const":"ellipsoid"},
                    "radii_m":bounded_vec3_schema(0.0, 5.0, true),
                    "longitude_segments":{"type":"integer","minimum":8,"maximum":64},
                    "latitude_segments":{"type":"integer","minimum":4,"maximum":64},
                    "position_m":bounded_vec3_schema(-10.0, 10.0, false),
                    "rotation_rad":bounded_vec3_schema(-6.283185307179586, 6.283185307179586, false)
                })
            ),
            closed_parameters_schema(
                &["shape","radius_m","longitude_segments","latitude_segments","position_m","rotation_rad"],
                json!({
                    "shape":{"const":"sphere"},
                    "radius_m":bounded_number_schema(0.0, 5.0, true),
                    "longitude_segments":{"type":"integer","minimum":8,"maximum":64},
                    "latitude_segments":{"type":"integer","minimum":4,"maximum":64},
                    "position_m":bounded_vec3_schema(-10.0, 10.0, false),
                    "rotation_rad":bounded_vec3_schema(-6.283185307179586, 6.283185307179586, false)
                })
            )
        ]
    })
}

fn modifier_stack_profile_extrude_parameters_schema() -> Value {
    closed_parameters_schema(
        &["shape", "profile", "depth_m", "position_m", "rotation_rad"],
        json!({
            "shape":{"const":"profile-extrude"},
            "profile":profile_points_schema(3, 64),
            "depth_m":bounded_number_schema(0.0, 10.0, true),
            "position_m":bounded_vec3_schema(-10.0, 10.0, false),
            "rotation_rad":bounded_vec3_schema(-6.283185307179586, 6.283185307179586, false)
        }),
    )
}

fn modifier_stack_profile_loft_parameters_schema() -> Value {
    closed_parameters_schema(
        &["shape", "profiles", "position_m", "rotation_rad"],
        json!({
            "shape":{"const":"profile-loft"},
            "profiles":{
                "type":"array","minItems":2,"maxItems":16,
                "items":closed_parameters_schema(
                    &["height_m","points"],
                    json!({
                        "height_m":bounded_number_schema(-10.0, 10.0, false),
                        "points":profile_points_schema(3, 64)
                    })
                )
            },
            "position_m":bounded_vec3_schema(-10.0, 10.0, false),
            "rotation_rad":bounded_vec3_schema(-6.283185307179586, 6.283185307179586, false)
        }),
    )
}

fn modifier_stack_longitudinal_loft_parameters_schema() -> Value {
    closed_parameters_schema(
        &["shape", "sections", "position_m", "rotation_rad"],
        json!({
            "shape":{"const":"longitudinal-section-loft"},
            "sections":{
                "type":"array","minItems":2,"maxItems":16,
                "items":closed_parameters_schema(
                    &["station_m","points"],
                    json!({
                        "station_m":bounded_number_schema(-10.0, 10.0, false),
                        "points":profile_points_schema(3, 64)
                    })
                )
            },
            "position_m":bounded_vec3_schema(-10.0, 10.0, false),
            "rotation_rad":bounded_vec3_schema(-6.283185307179586, 6.283185307179586, false)
        }),
    )
}

fn modifier_stack_subd_cage_parameters_schema() -> Value {
    closed_parameters_schema(
        &[
            "shape",
            "control_points",
            "u_points",
            "v_points",
            "subdivision_levels",
            "position_m",
            "rotation_rad",
        ],
        json!({
            "shape":{"const":"subd-cage"},
            "control_points":{"type":"array","minItems":4,"maxItems":256,"items":bounded_vec3_schema(-10.0, 10.0, false)},
            "u_points":{"type":"integer","minimum":2,"maximum":16},
            "v_points":{"type":"integer","minimum":2,"maximum":16},
            "subdivision_levels":{"type":"integer","minimum":0,"maximum":2},
            "position_m":bounded_vec3_schema(-10.0, 10.0, false),
            "rotation_rad":bounded_vec3_schema(-6.283185307179586, 6.283185307179586, false)
        }),
    )
}

fn modifier_stack_surface_patch_parameters_schema() -> Value {
    closed_parameters_schema(
        &[
            "shape",
            "control_points",
            "u_segments",
            "v_segments",
            "position_m",
            "rotation_rad",
        ],
        json!({
            "shape":{"const":"surface-patch"},
            "control_points":{"type":"array","minItems":16,"maxItems":16,"items":bounded_vec3_schema(-10.0, 10.0, false)},
            "u_segments":{"type":"integer","minimum":4,"maximum":32},
            "v_segments":{"type":"integer","minimum":4,"maximum":32},
            "position_m":bounded_vec3_schema(-10.0, 10.0, false),
            "rotation_rad":bounded_vec3_schema(-6.283185307179586, 6.283185307179586, false)
        }),
    )
}

fn modifier_stack_revolve_parameters_schema() -> Value {
    closed_parameters_schema(
        &[
            "shape",
            "profile",
            "radial_segments",
            "position_m",
            "rotation_rad",
        ],
        json!({
            "shape":{"const":"revolve"},
            "profile":profile_points_schema(2, 64),
            "radial_segments":{"type":"integer","minimum":8,"maximum":64},
            "position_m":bounded_vec3_schema(-10.0, 10.0, false),
            "rotation_rad":bounded_vec3_schema(-6.283185307179586, 6.283185307179586, false)
        }),
    )
}

fn modifier_stack_tube_sweep_parameters_schema() -> Value {
    closed_parameters_schema(
        &[
            "shape",
            "path",
            "radius_m",
            "radial_segments",
            "cap_ends",
            "position_m",
            "rotation_rad",
        ],
        json!({
            "shape":{"const":"tube-sweep"},
            "path":{"type":"array","minItems":2,"maxItems":128,"items":bounded_vec3_schema(-10.0, 10.0, false)},
            "radius_m":bounded_number_schema(0.0, 5.0, true),
            "radial_segments":{"type":"integer","minimum":8,"maximum":64},
            "cap_ends":{"type":"boolean"},
            "position_m":bounded_vec3_schema(-10.0, 10.0, false),
            "rotation_rad":bounded_vec3_schema(-6.283185307179586, 6.283185307179586, false)
        }),
    )
}

fn modifier_stack_panel_parameters_schema() -> Value {
    closed_parameters_schema(
        &[
            "shape",
            "size_m",
            "thickness_m",
            "bevel_m",
            "position_m",
            "rotation_rad",
        ],
        json!({
            "shape":{"const":"panel"},
            "size_m":bounded_vec3_schema(0.0, 10.0, true),
            "thickness_m":bounded_number_schema(0.0, 10.0, true),
            "bevel_m":bounded_number_schema(0.0, 10.0, true),
            "position_m":bounded_vec3_schema(-10.0, 10.0, false),
            "rotation_rad":bounded_vec3_schema(-6.283185307179586, 6.283185307179586, false)
        }),
    )
}

fn modifier_stack_vent_array_parameters_schema() -> Value {
    closed_parameters_schema(
        &[
            "shape",
            "width_m",
            "height_m",
            "depth_m",
            "slot_count",
            "slot_width_m",
            "slot_spacing_m",
            "position_m",
            "rotation_rad",
        ],
        json!({
            "shape":{"const":"vent-array"},
            "width_m":bounded_number_schema(0.0, 10.0, true),
            "height_m":bounded_number_schema(0.0, 10.0, true),
            "depth_m":bounded_number_schema(0.0, 10.0, true),
            "slot_count":{"type":"integer","minimum":1,"maximum":32},
            "slot_width_m":bounded_number_schema(0.0, 10.0, true),
            "slot_spacing_m":bounded_number_schema(0.0, 10.0, true),
            "position_m":bounded_vec3_schema(-10.0, 10.0, false),
            "rotation_rad":bounded_vec3_schema(-6.283185307179586, 6.283185307179586, false)
        }),
    )
}

fn modifier_stack_vent_array_v2_parameters_schema() -> Value {
    closed_parameters_schema(
        &[
            "shape",
            "width_m",
            "height_m",
            "depth_m",
            "face_thickness_m",
            "backing_depth_m",
            "backing_gap_m",
            "slot_count",
            "slot_width_m",
            "slot_spacing_m",
            "slot_margin_m",
            "slot_edge_bevel_m",
            "bevel_segments",
            "position_m",
            "rotation_rad",
        ],
        json!({
            "shape":{"const":"vent-array"},
            "width_m":bounded_number_schema(0.0, 10.0, true),
            "height_m":bounded_number_schema(0.0, 10.0, true),
            "depth_m":bounded_number_schema(0.0, 10.0, true),
            "face_thickness_m":bounded_number_schema(0.0, 10.0, true),
            "backing_depth_m":bounded_number_schema(0.0, 10.0, true),
            "backing_gap_m":bounded_number_schema(0.0, 10.0, true),
            "slot_count":{"type":"integer","minimum":1,"maximum":32},
            "slot_width_m":bounded_number_schema(0.0, 10.0, true),
            "slot_spacing_m":bounded_number_schema(0.0, 10.0, true),
            "slot_margin_m":bounded_number_schema(0.0, 10.0, true),
            "slot_edge_bevel_m":bounded_number_schema(0.0, 10.0, true),
            "bevel_segments":{"type":"integer","minimum":1,"maximum":4},
            "position_m":bounded_vec3_schema(-10.0, 10.0, false),
            "rotation_rad":bounded_vec3_schema(-6.283185307179586, 6.283185307179586, false)
        }),
    )
}

fn modifier_stack_recessed_channel_parameters_schema() -> Value {
    closed_parameters_schema(
        &[
            "shape",
            "stations",
            "path_frame",
            "floor_width_ratio",
            "edge_bevel_m",
            "start_transition_m",
            "end_transition_m",
            "transition_segments",
            "position_m",
            "rotation_rad",
        ],
        json!({
            "shape":{"const":"recessed-channel"},
            "stations":{
                "type":"array","minItems":2,"maxItems":32,
                "items":closed_parameters_schema(
                    &["point_m","width_m","depth_m"],
                    json!({
                        "point_m":bounded_vec3_schema(-10.0, 10.0, false),
                        "width_m":bounded_number_schema(0.0, 10.0, true),
                        "depth_m":bounded_number_schema(0.0, 10.0, true)
                    })
                )
            },
            "path_frame":{"const":"planar-xy-z-up@1"},
            "floor_width_ratio":{"type":"number","exclusiveMinimum":0.1,"maximum":0.8},
            "edge_bevel_m":{"type":"number","minimum":0.0,"maximum":5.0},
            "start_transition_m":{"type":"number","minimum":0.0,"maximum":5.0},
            "end_transition_m":{"type":"number","minimum":0.0,"maximum":5.0},
            "transition_segments":{"type":"integer","minimum":1,"maximum":4},
            "position_m":bounded_vec3_schema(-10.0, 10.0, false),
            "rotation_rad":bounded_vec3_schema(-6.283185307179586, 6.283185307179586, false)
        }),
    )
}

fn modifier_stack_joint_stack_parameters_schema() -> Value {
    closed_parameters_schema(
        &[
            "shape",
            "radius_m",
            "depth_m",
            "ring_count",
            "ring_spacing_m",
            "radial_segments",
            "position_m",
            "rotation_rad",
        ],
        json!({
            "shape":{"const":"joint-stack"},
            "radius_m":bounded_number_schema(0.0, 5.0, true),
            "depth_m":bounded_number_schema(0.0, 10.0, true),
            "ring_count":{"type":"integer","minimum":1,"maximum":16},
            "ring_spacing_m":bounded_number_schema(0.0, 10.0, true),
            "radial_segments":{"type":"integer","minimum":8,"maximum":64},
            "position_m":bounded_vec3_schema(-10.0, 10.0, false),
            "rotation_rad":bounded_vec3_schema(-6.283185307179586, 6.283185307179586, false)
        }),
    )
}

fn closed_parameters_schema(required: &[&str], properties: Value) -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":required,
        "properties":properties
    })
}

fn profile_points_schema(min_items: usize, max_items: usize) -> Value {
    json!({
        "type":"array",
        "minItems":min_items,
        "maxItems":max_items,
        "items":{
            "type":"array","minItems":2,"maxItems":2,
            "items":bounded_number_schema(-10.0, 10.0, false)
        }
    })
}

fn bounded_number_schema(minimum: f64, maximum: f64, exclusive_minimum: bool) -> Value {
    if exclusive_minimum {
        json!({"type":"number","exclusiveMinimum":minimum,"maximum":maximum})
    } else {
        json!({"type":"number","minimum":minimum,"maximum":maximum})
    }
}

fn bounded_vec3_schema(minimum: f64, maximum: f64, exclusive_minimum: bool) -> Value {
    let item = if exclusive_minimum {
        json!({"type":"number","exclusiveMinimum":minimum,"maximum":maximum})
    } else {
        json!({"type":"number","minimum":minimum,"maximum":maximum})
    };
    json!({"type":"array","minItems":3,"maxItems":3,"items":item})
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
        "Compile a bounded typed GeometryProgram into a multi-part GLB candidate. First read ponytail-preflight@0.1.0 with skill_get in this MCP session. Legacy calls omit idempotency_key. Exact calls add idempotency_key, explicitly bind base_version_id to the current version or null for an empty head, and accept exactly one direct GeometryProgram@2, one closed GeometryModifierEvaluationRequest@2, one legacy candidate-bound GeometryModifierApplyRequest@1, or one exact GeometryModifierApplyRequest@2. The @2 path is limited to one stable source edge on one direct authoring-mesh@1 Part, is exposed only through the authenticated explicit write opt-in, and stages a reviewable candidate only: it does not confirm, create a version, or export. Exact paths use byte-exact same-cohort Worker replay and one Runtime transaction; MCP responses contain readback metadata, never raw GLB bytes. No permanent version is created until a later approval confirm.",
        json!({
            "oneOf":[
                {
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
                },
                {
                    "type":"object",
                    "required":["project_id","base_version_id","idempotency_key","request"],
                    "properties":{
                        "project_id":id_property(),
                        "base_version_id":nullable_id_property(),
                        "idempotency_key":id_property(),
                        "request":{
                            "oneOf":[
                                {
                                    "type":"object",
                                    "required":["typed","geometry_program"],
                                    "properties":{
                                        "typed":{"const":"geometry"},
                                        "reference_id":id_property(),
                                        "geometry_program":{
                                            "type":"object",
                                            "required":["schema_version"],
                                            "properties":{"schema_version":{"const":"GeometryProgram@2"}}
                                        }
                                    },
                                    "additionalProperties":false
                                },
                                {
                                    "type":"object",
                                    "required":["typed","modifier_evaluation_request","modifier_evaluation_sha256"],
                                    "properties":{
                                        "typed":{"const":"geometry"},
                                        "reference_id":id_property(),
                                        "modifier_evaluation_request":modifier_evaluation_request_schema(),
                                        "modifier_evaluation_sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"}
                                    },
                                    "additionalProperties":false
                                },
                                {
                                    "type":"object",
                                    "required":["typed","modifier_apply_request","modifier_apply_sha256"],
                                    "properties":{
                                        "typed":{"const":"geometry"},
                                        "modifier_apply_request":modifier_apply_request_schema(),
                                        "modifier_apply_sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"}
                                    },
                                    "additionalProperties":false
                                }
                            ]
                        }
                    },
                    "additionalProperties":false
                }
            ]
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
            "authoring_mesh_edit_prepare",
            "Explicitly replay one bounded candidate-bound authoring mesh edit through the fixed Geometry Worker and atomically stage the exact derived program, GLB, strict readback, evidence, Job and reviewable candidate. This Runtime-owned write is idempotent, creates no version, performs no confirm or export, and accepts no Blender/Python/plugin payload.",
            authoring_mesh_edit_prepare_schema(),
            false,
            true,
            "MCP010F",
        ),
        write_tool_with_transaction(
            "mechanical_animation_clip_prepare",
            "Explicitly validate and materialize one bounded rigid MechanicalPose sequence as an immutable Runtime-owned CAS clip plus exact SQLite Link. Runtime replays the source artifact twice through the fixed Geometry Worker and requires one non-null same-build cohort before writing; this never confirms a candidate/version or claims Blender armature, skinning, timeline, NLA, F-Curve, driver or Python parity.",
            mechanical_animation_clip_prepare_schema(),
            false,
            true,
            "MCP010F",
        ),
        write_tool_with_transaction(
            "mechanical_animation_glb_prepare",
            "Materialize one immutable Runtime-owned rigid MechanicalAnimationClip into a standard bounded glTF 2.0 animation in CAS. Every Part receives LINEAR translation and quaternion rotation channels from scheduled double-Worker-verified frames; strict readback preserves the exact static source projection and rejects skinning, morph targets, scripts and custom animation payloads. This prepare never confirms a candidate/version and does not perform an external export.",
            mechanical_animation_glb_prepare_schema(),
            false,
            true,
            "MCP010F",
        ),
        write_tool_with_transaction(
            "game_asset_delivery_prepare",
            "Validate exactly three independently authored and compiled candidate-bound LOD GLBs, require stable Part/MaterialZone coverage, progressive 75/50 percent triangle budgets and bounded spatial envelopes, then derive one conservative gameplay-only AABB collision box per Part from actual LOD2 POSITION bytes. Runtime stores immutable CAS receipts only; this never confirms, exports, adds physical properties or claims a Unity, Unreal, Godot or Three.js round-trip.",
            game_asset_delivery_prepare_schema(),
            false,
            true,
            "MCP010F",
        ),
        write_tool_with_transaction(
            "appearance_source_lineage_prepare",
            "Atomically materialize one Runtime-owned durable Appearance source lineage sidecar bound to one candidate/project/Worker cohort, allowlisted AppearanceProgram and MaterialPack, TextureBuild receipt, optional CandidateSurfaceBake receipt, exact GeometryProgram evidence and three strict LOD GLB/ArtifactReadback/Part inventory bindings. This prepare never confirms, exports or changes quality gates; missing or tampered source objects fail closed.",
            appearance_source_lineage_prepare_schema(),
            false,
            true,
            "MCP010F",
        ),
        write_tool_with_transaction(
            "game_weapon_anchor_prepare",
            "Persist one closed six-role fictional-weapon metadata anchor sidecar for an exact durable LOD delivery. Runtime validates the synthetic identity root, five unique Part-bound helpers, finite unit-quaternion TRS, +X muzzle placement and all LOD bindings. It does not rewrite GLB nodes, prove pivots, define hitboxes or invoke a commercial engine.",
            game_weapon_anchor_prepare_schema(),
            false,
            true,
            "MCP010F",
        ),
        write_tool_with_transaction(
            "game_weapon_glb_socket_prepare",
            "Materialize six Runtime-owned named empty socket nodes into derived LOD0, LOD1 and LOD2 GLBs while preserving source renderable content, meshes, materials, animations and BIN bytes by exact hash-bound readback. This prepare never confirms or exports a candidate, invokes a commercial engine, returns GLB bytes or claims functional weapon semantics or visual quality.",
            game_weapon_glb_socket_prepare_schema(),
            false,
            true,
            "MCP010F",
        ),
        write_tool_with_transaction(
            "game_weapon_animated_glb_socket_prepare",
            "Materialize six Runtime-owned named empty socket nodes into the animated LOD0 GLB while preserving the source MechanicalAnimationGlb renderable projection, animations, channels, samplers and BIN bytes by exact hash-bound readback. This prepare never confirms or exports a candidate, invokes a commercial engine, returns GLB bytes or claims functional weapon semantics or visual quality.",
            game_weapon_animated_glb_socket_prepare_schema(),
            false,
            true,
            "MCP010F",
        ),
        write_tool_with_transaction(
            "fictional_energy_vfx_prepare",
            "Persist one closed fictional energy VFX intent profile bound to an exact durable delivery, anchor sidecar and allowlisted 2K MaterialPack. Runtime validates two bounded emissive sample curves, but does not execute material animation, bloom, particles, trails, confirmation, export or a commercial-engine round-trip.",
            fictional_energy_vfx_prepare_schema(),
            false,
            true,
            "MCP010F",
        ),
        write_tool_with_transaction(
            "fictional_energy_vfx_rendered_frame_prepare",
            "Render one exact LOD0 sampled-emissive frame twice through the fixed same-cohort Render Worker, require byte-identical nine-AOV replay, and atomically persist the dedicated RenderSet, nine PNGs and receipt through reservation-protected CAS plus one SQLite link. This never confirms, versions or exports the candidate and does not claim a full animation sequence, bloom, particles, trails, engine execution or visual quality.",
            fictional_energy_vfx_rendered_frame_prepare_schema(),
            false,
            true,
            "MCP010F",
        ),
        write_tool_with_transaction(
            "fictional_energy_vfx_rendered_sequence_prepare",
            "Render an ordered bounded sequence of exact LOD0 sampled-emissive frames through the fixed same-cohort Render Worker and durably persist one sequence receipt/link over its independent fixed-camera nine-AOV frame links. This never confirms, versions or exports the candidate and does not claim engine material animation, bloom, particles, trails or visual quality.",
            fictional_energy_vfx_rendered_sequence_prepare_schema(),
            false,
            true,
            "MCP010F",
        ),
        write_tool_with_transaction(
            "fictional_energy_vfx_hdr_bloom_prepare",
            "Render one exact durable sampled-emissive LOD0 base frame through the fixed two-pass HDR emissive-source and bloom-contribution Worker operation, require same-cohort byte-exact replay, verify the existing nine-AOV base frame by hash, and atomically persist two independent PNGs plus a RenderSet, receipt and SQLite Link. This never rerenders or mutates the base AOVs, confirms a candidate, exports or invokes a commercial engine.",
            fictional_energy_vfx_hdr_bloom_prepare_schema(),
            false,
            true,
            "MCP010F",
        ),
        write_tool_with_transaction(
            "fictional_energy_vfx_particles_prepare",
            "Derive two closed typed-particle emitters from exact durable hashes and resolved LOD0 Part-node world transforms, render three independent depth-tested particle passes twice through the same-cohort Render Worker, verify the base nine AOV and Bloom hashes remain byte-exact, and atomically persist the receipt, RenderSet, PNGs and SQLite link. This does not create GLB sockets, trails, engine execution, confirmation or export.",
            fictional_energy_vfx_particles_prepare_schema(),
            false,
            true,
            "MCP010F",
        ),
        write_tool_with_transaction(
            "fictional_energy_vfx_trails_prepare",
            "Derive two closed typed trails from one to four ordered durable particle-history frames and the current particle frame, render independent depth-tested trail color/ID/depth passes twice through the same-cohort Render Worker, verify base AOV, Bloom and particle pass hashes remain byte-exact, and atomically persist CAS plus one SQLite link. V1 explicitly excludes trails from Bloom and does not create GLB sockets, engine execution, confirmation or export.",
            fictional_energy_vfx_trails_prepare_schema(),
            false,
            true,
            "MCP010F",
        ),
        write_tool_with_transaction(
            "fictional_energy_vfx_trails_bloom_prepare",
            "Render one fixed-profile typed-trail HDR source and trail-bloom contribution pair from existing trail color/depth plus the current base opaque depth. Runtime verifies the base AOV, base Bloom, particle and source-trail pass objects are byte-exact reused, writes only the two new independent PNG passes and their receipt/link, and never reports the original bloom_rendered flag or rerenders particles/trails.",
            fictional_energy_vfx_trails_bloom_prepare_schema(),
            false,
            true,
            "MCP010F",
        ),
        write_tool_with_transaction(
            "subdivision_artifact_lineage_prepare",
            "Explicitly prepare and write one Runtime-owned Subdivision lineage sidecar Link@1 for the exact candidate/artifact binding. This is a write operation exposed only through the authenticated explicit write opt-in; MCP never writes CAS or SQLite directly, and the returned structuredContent is the Runtime Link@1 result.",
            subdivision_artifact_lineage_sidecar_request_schema(),
            false,
            true,
            "MCP010F",
        ),
        write_tool_with_transaction(
            "primary_form_repair_prepare",
            "Run one Runtime-owned bounded Primary Form repair: fit the typed SilhouetteRig, compile the selected GeometryProgram through the Geometry Worker, validate strict readback, render the same camera through the isolated Render Worker, compare source and proposal, and return only a staged candidate when strict same-camera improvement is proven. It never confirms a version or exports an asset.",
            json!({
                "type":"object",
                "required":["project_id","candidate_id","target_sha256","rig","base_camera","optimizer","canonical_sha256"],
                "properties":{
                    "project_id":id_property(),
                    "candidate_id":id_property(),
                    "target_sha256":sha256_property(),
                    "part_id":id_property(),
                    "rig":{"type":"object"},
                    "base_camera":{"type":"object"},
                    "optimizer":{
                        "type":"object",
                        "required":["algorithm","max_iterations","max_evaluations","step_fraction"],
                        "properties":{
                            "algorithm":{"enum":["grid","coordinate_descent"]},
                            "max_iterations":{"type":"integer","minimum":1,"maximum":8},
                            "max_evaluations":{"type":"integer","minimum":1,"maximum":64},
                            "step_fraction":{"type":"number","exclusiveMinimum":0,"maximum":0.5}
                        },
                        "additionalProperties":false
                    },
                    "view_spec":{"type":"object"},
                    "base_version_id":nullable_id_property(),
                    "canonical_sha256":sha256_property()
                },
                "additionalProperties":false
            }),
            false,
            true,
            "MCP010F",
        ),
        write_tool_with_transaction(
            "primary_form_repair_job_prepare",
            "Queue one Runtime-owned asynchronous Primary Form repair. The bounded Geometry/Render Worker search runs outside the MCP request deadline; poll job_get and then job_result_get. It creates only a staged candidate when the same-camera acceptance gate passes, never confirms a version or exports an asset.",
            json!({
                "type":"object",
                "required":["project_id","candidate_id","target_sha256","rig","base_camera","optimizer","canonical_sha256"],
                "properties":{
                    "project_id":id_property(),
                    "candidate_id":id_property(),
                    "target_sha256":sha256_property(),
                    "part_id":id_property(),
                    "rig":{"type":"object"},
                    "base_camera":{"type":"object"},
                    "view_spec":{"type":"object"},
                    "optimizer":{
                        "type":"object",
                        "required":["algorithm","max_iterations","max_evaluations","step_fraction"],
                        "properties":{
                            "algorithm":{"enum":["grid","coordinate_descent"]},
                            "max_iterations":{"type":"integer","minimum":1,"maximum":8},
                            "max_evaluations":{"type":"integer","minimum":1,"maximum":64},
                            "step_fraction":{"type":"number","exclusiveMinimum":0,"maximum":0.5}
                        },
                        "additionalProperties":false
                    },
                    "base_version_id":nullable_id_property(),
                    "canonical_sha256":sha256_property()
                },
                "additionalProperties":false
            }),
            false,
            false,
            "MCP010F",
        ),
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
                    "parts":{"type":["array","null"],"maxItems":64,"items":silhouette_target_part_property()},
                    "visual_structure":reference_visual_structure_property(),
                    "user_confirmed":{"type":"boolean","description":"Explicit user confirmation that the contour and observed annotations are ready for a partial-view benchmark; automatic flood-fill remains exploratory."}
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
                    "parts":{"type":["array","null"],"maxItems":64,"items":silhouette_target_part_property()},
                    "visual_structure":reference_visual_structure_property(),
                    "user_confirmed":{"type":"boolean","description":"Explicit user confirmation that the refined contour and observed annotations are ready for a partial-view benchmark."}
                },
                "additionalProperties":false
            }),
            false,
            false,
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
                | "allOf"
                | "if"
                | "then"
                | "const"
                | "enum"
                | "minLength"
                | "maxLength"
                | "pattern"
                | "minimum"
                | "exclusiveMinimum"
                | "maximum"
                | "maxProperties"
                | "items"
                | "minItems"
                | "maxItems"
                | "uniqueItems"
                | "description"
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
    if let Some(value) = object.get("uniqueItems") {
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
    if let Some(value) = object.get("allOf") {
        let alternatives = value
            .as_array()
            .filter(|items| !items.is_empty())
            .ok_or(())?;
        for alternative in alternatives {
            validate_tool_schema_shape(alternative, depth + 1, budget)?;
        }
    }
    for key in ["if", "then"] {
        if let Some(value) = object.get(key) {
            validate_tool_schema_shape(value, depth + 1, budget)?;
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
    if object
        .get("exclusiveMinimum")
        .is_some_and(|value| value.as_f64().is_none())
    {
        return Err(());
    }
    if let Some(value) = object.get("pattern") {
        if !matches!(
            value.as_str(),
            Some(
                "^[0-9a-f]{64}$"
                    | "^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"
                    | "^[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}$"
                    | "^[A-Za-z0-9._:-]+$"
                    | "^[0-9]{1,10}$",
            )
        ) {
            return Err(());
        }
    }
    if object
        .get("description")
        .is_some_and(|value| !value.is_string())
    {
        return Err(());
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
    if let Some(minimum) = object.get("exclusiveMinimum").and_then(Value::as_f64) {
        if value
            .as_f64()
            .filter(|candidate| *candidate > minimum)
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
            let matches = match pattern {
                "^[0-9a-f]{64}$" => is_lowercase_sha256(string),
                "^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$" => {
                    is_opaque_id(string)
                        && string
                            .chars()
                            .next()
                            .is_some_and(|first| first.is_ascii_alphanumeric())
                }
                "^[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}$" => {
                    !string.is_empty()
                        && string.chars().count() <= 128
                        && string
                            .chars()
                            .next()
                            .is_some_and(|first| first.is_ascii_alphanumeric())
                        && string.chars().all(|character| {
                            character.is_ascii_alphanumeric()
                                || matches!(character, '.' | '_' | ':' | '-')
                        })
                }
                "^[A-Za-z0-9._:-]+$" => {
                    !string.is_empty()
                        && string.chars().count() <= 128
                        && string.chars().all(|character| {
                            character.is_ascii_alphanumeric()
                                || matches!(character, '.' | '_' | ':' | '-')
                        })
                }
                "^[0-9]{1,10}$" => {
                    !string.is_empty()
                        && string.len() <= 10
                        && string.bytes().all(|byte| byte.is_ascii_digit())
                }
                _ => false,
            };
            if !matches {
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

fn fictional_energy_vfx_mcp_summary(name: &str, value: &Value) -> Option<String> {
    if !matches!(
        name,
        "fictional_energy_vfx_prepare"
            | "fictional_energy_vfx_get"
            | "fictional_energy_vfx_frame_sample"
            | "fictional_energy_vfx_appearance_frame_sample"
            | "fictional_energy_vfx_rendered_frame_prepare"
            | "fictional_energy_vfx_rendered_frame_get"
            | "fictional_energy_vfx_rendered_sequence_prepare"
            | "fictional_energy_vfx_rendered_sequence_get"
            | "fictional_energy_vfx_hdr_bloom_prepare"
            | "fictional_energy_vfx_hdr_bloom_get"
            | "fictional_energy_vfx_particles_prepare"
            | "fictional_energy_vfx_particles_get"
            | "fictional_energy_vfx_trails_prepare"
            | "fictional_energy_vfx_trails_get"
            | "fictional_energy_vfx_trails_bloom_prepare"
            | "fictional_energy_vfx_trails_bloom_get"
    ) {
        return None;
    }
    let is_hdr_bloom = matches!(
        name,
        "fictional_energy_vfx_hdr_bloom_prepare" | "fictional_energy_vfx_hdr_bloom_get"
    );
    let is_particles = matches!(
        name,
        "fictional_energy_vfx_particles_prepare" | "fictional_energy_vfx_particles_get"
    );
    let is_trails = matches!(
        name,
        "fictional_energy_vfx_trails_prepare" | "fictional_energy_vfx_trails_get"
    );
    let is_trails_bloom = matches!(
        name,
        "fictional_energy_vfx_trails_bloom_prepare" | "fictional_energy_vfx_trails_bloom_get"
    );
    let value_or_null = |candidate: Option<&Value>| candidate.cloned().unwrap_or(Value::Null);
    let mut summary = Map::new();
    summary.insert(
        "schema_version".to_owned(),
        Value::String("FictionalEnergyVfxMcpSummary@1".to_owned()),
    );
    summary.insert("operation".to_owned(), Value::String(name.to_owned()));
    summary.insert(
        "vfx_profile_object_sha256".to_owned(),
        value_or_null(
            value
                .get("vfx_profile_object_sha256")
                .or_else(|| value.pointer("/link/vfx_profile_object_sha256")),
        ),
    );
    summary.insert(
        "delivery_manifest_object_sha256".to_owned(),
        value_or_null(
            value
                .pointer("/durable_link/delivery_manifest_object_sha256")
                .or_else(|| value.pointer("/link/delivery_manifest_object_sha256")),
        ),
    );
    summary.insert(
        "effect_ids".to_owned(),
        value
            .pointer("/vfx_profile/effects")
            .or_else(|| value.get("effects"))
            .and_then(Value::as_array)
            .map(|effects| {
                Value::Array(
                    effects
                        .iter()
                        .map(|effect| value_or_null(effect.get("effect_id")))
                        .collect(),
                )
            })
            .unwrap_or(Value::Null),
    );
    summary.insert(
        "execution_mode".to_owned(),
        value_or_null(
            value
                .pointer("/vfx_profile/execution_mode")
                .or_else(|| value.get("sampling_policy")),
        ),
    );
    summary.insert(
        "glb_material_zone_binding_verified".to_owned(),
        value
            .get("glb_material_zone_binding_verified")
            .cloned()
            .unwrap_or(Value::Bool(false)),
    );
    summary.insert(
        "lod_appearance_binding_count".to_owned(),
        value
            .get("lod_appearance_bindings")
            .and_then(Value::as_array)
            .map(|values| Value::from(values.len() as u64))
            .unwrap_or(Value::Null),
    );
    summary.insert(
        "sequence_key_sha256".to_owned(),
        value_or_null(value.get("sequence_key_sha256")),
    );
    summary.insert(
        "frame_count".to_owned(),
        value_or_null(value.get("frame_count")),
    );
    summary.insert(
        "frame_key_sha256s".to_owned(),
        value_or_null(
            value
                .pointer("/durable_link/frame_key_sha256s")
                .or_else(|| value.pointer("/link/frame_key_sha256s"))
                .or_else(|| value.pointer("/receipt/frame_key_sha256s")),
        ),
    );
    summary.insert(
        "same_camera_verified".to_owned(),
        value_or_null(value.pointer("/receipt/fixed_camera_verified")),
    );
    summary.insert(
        "same_worker_cohort_verified".to_owned(),
        value_or_null(value.pointer("/receipt/same_worker_cohort_verified")),
    );
    summary.insert(
        "independent_effect_material_zones_verified".to_owned(),
        value_or_null(value.pointer("/receipt/independent_effect_material_zones_verified")),
    );
    summary.insert("emissive_animation_rendered".to_owned(), Value::Bool(false));
    summary.insert(
        "bloom_rendered".to_owned(),
        if is_hdr_bloom {
            value
                .pointer("/receipt/bloom_rendered")
                .cloned()
                .unwrap_or(Value::Bool(true))
        } else {
            Value::Bool(false)
        },
    );
    summary.insert(
        "particles_rendered".to_owned(),
        if is_particles {
            value
                .pointer("/receipt/typed_particles_rendered")
                .cloned()
                .unwrap_or(Value::Bool(true))
        } else {
            Value::Bool(false)
        },
    );
    summary.insert(
        "trails_rendered".to_owned(),
        if is_trails {
            value
                .pointer("/receipt/typed_trails_rendered")
                .cloned()
                .unwrap_or(Value::Bool(true))
        } else {
            Value::Bool(false)
        },
    );
    summary.insert(
        "trail_bloom_key_sha256".to_owned(),
        if is_trails_bloom {
            value_or_null(
                value
                    .get("trail_bloom_key_sha256")
                    .or_else(|| value.pointer("/receipt/trail_bloom_key_sha256"))
                    .or_else(|| value.pointer("/link/trail_bloom_key_sha256")),
            )
        } else {
            Value::Null
        },
    );
    summary.insert(
        "source_trail_key_sha256".to_owned(),
        if is_trails_bloom {
            value_or_null(
                value
                    .get("source_trail_key_sha256")
                    .or_else(|| value.pointer("/receipt/source_trail_key_sha256"))
                    .or_else(|| value.pointer("/link/source_trail_key_sha256")),
            )
        } else {
            Value::Null
        },
    );
    summary.insert(
        "trail_bloom_rendered".to_owned(),
        if is_trails_bloom {
            value
                .pointer("/receipt/trail_bloom_rendered")
                .or_else(|| value.pointer("/render_set/trail_bloom_rendered"))
                .cloned()
                .unwrap_or(Value::Bool(false))
        } else {
            Value::Bool(false)
        },
    );
    summary.insert(
        "trail_bloom_source_rendered".to_owned(),
        if is_trails_bloom {
            value
                .pointer("/receipt/trail_bloom_source_rendered")
                .or_else(|| value.pointer("/render_set/trail_bloom_source_rendered"))
                .cloned()
                .unwrap_or(Value::Bool(false))
        } else {
            Value::Bool(false)
        },
    );
    summary.insert(
        "trail_bloom_contribution_rendered".to_owned(),
        if is_trails_bloom {
            value
                .pointer("/receipt/trail_bloom_contribution_rendered")
                .or_else(|| value.pointer("/render_set/trail_bloom_contribution_rendered"))
                .cloned()
                .unwrap_or(Value::Bool(false))
        } else {
            Value::Bool(false)
        },
    );
    let input = if is_trails_bloom {
        let mut input = Map::new();
        input.insert(
            "input_policy".to_owned(),
            value_or_null(
                value
                    .pointer("/receipt/input_policy")
                    .or_else(|| value.pointer("/render_set/input_policy")),
            ),
        );
        input.insert(
            "source_trail_key_sha256".to_owned(),
            value_or_null(
                value
                    .get("source_trail_key_sha256")
                    .or_else(|| value.pointer("/receipt/source_trail_key_sha256"))
                    .or_else(|| value.pointer("/link/source_trail_key_sha256")),
            ),
        );
        input.insert(
            "source_trail_color_object_sha256".to_owned(),
            value_or_null(
                value
                    .pointer("/receipt/source_trail_color_object_sha256")
                    .or_else(|| value.pointer("/link/source_trail_color_object_sha256")),
            ),
        );
        input.insert(
            "source_trail_id_object_sha256".to_owned(),
            value_or_null(
                value
                    .pointer("/receipt/source_trail_id_object_sha256")
                    .or_else(|| value.pointer("/link/source_trail_id_object_sha256")),
            ),
        );
        input.insert(
            "source_trail_depth_object_sha256".to_owned(),
            value_or_null(
                value
                    .pointer("/receipt/source_trail_depth_object_sha256")
                    .or_else(|| value.pointer("/link/source_trail_depth_object_sha256")),
            ),
        );
        input.insert(
            "base_opaque_depth_object_sha256".to_owned(),
            value_or_null(
                value
                    .pointer("/receipt/base_opaque_depth_object_sha256")
                    .or_else(|| value.pointer("/link/base_opaque_depth_object_sha256")),
            ),
        );
        input.insert(
            "base_opaque_depth_byte_exact_reused".to_owned(),
            value_or_null(value.pointer("/receipt/base_opaque_depth_byte_exact_reused")),
        );
        Value::Object(input)
    } else {
        Value::Null
    };
    summary.insert("input".to_owned(), input);
    let trail_bloom_input = if is_trails_bloom {
        let mut input = Map::new();
        input.insert(
            "source_trail_color_depth_and_current_base_opaque_depth".to_owned(),
            Value::Bool(true),
        );
        input.insert(
            "input_policy".to_owned(),
            value_or_null(
                value
                    .pointer("/receipt/input_policy")
                    .or_else(|| value.pointer("/render_set/input_policy")),
            ),
        );
        input.insert(
            "base_opaque_depth_object_sha256".to_owned(),
            value_or_null(
                value
                    .pointer("/receipt/base_opaque_depth_object_sha256")
                    .or_else(|| value.pointer("/link/base_opaque_depth_object_sha256")),
            ),
        );
        input.insert(
            "source_trail_key_sha256".to_owned(),
            value_or_null(
                value
                    .get("source_trail_key_sha256")
                    .or_else(|| value.pointer("/receipt/source_trail_key_sha256"))
                    .or_else(|| value.pointer("/link/source_trail_key_sha256")),
            ),
        );
        Value::Object(input)
    } else {
        Value::Null
    };
    summary.insert("trail_bloom_input".to_owned(), trail_bloom_input);
    let trail_bloom_pass_artifacts =
        if is_trails_bloom {
            let mut artifacts = Map::new();
            artifacts.insert(
                "source".to_owned(),
                value_or_null(
                    value.pointer("/receipt/source_pass").or_else(|| {
                        value.pointer("/render_set/pass_artifacts/trail-emissive-source")
                    }),
                ),
            );
            artifacts.insert(
                "contribution".to_owned(),
                value_or_null(value.pointer("/receipt/contribution_pass").or_else(|| {
                    value.pointer("/render_set/pass_artifacts/trail-bloom-contribution")
                })),
            );
            Value::Object(artifacts)
        } else {
            Value::Null
        };
    summary.insert(
        "trail_bloom_pass_artifacts".to_owned(),
        trail_bloom_pass_artifacts,
    );
    summary.insert(
        "trail_bloom_passes".to_owned(),
        if is_trails_bloom {
            value_or_null(
                value
                    .pointer("/receipt/pass_artifacts")
                    .or_else(|| value.pointer("/render_set/pass_artifacts")),
            )
        } else {
            Value::Null
        },
    );
    summary.insert(
        "trail_count".to_owned(),
        if is_trails {
            value_or_null(value.pointer("/receipt/trail_count"))
        } else {
            Value::Null
        },
    );
    summary.insert(
        "segment_count".to_owned(),
        if is_trails {
            value_or_null(value.pointer("/receipt/segment_count"))
        } else {
            Value::Null
        },
    );
    summary.insert(
        "current_particle_key_sha256".to_owned(),
        if is_trails {
            value_or_null(
                value
                    .pointer("/receipt/current_particle_key_sha256")
                    .or_else(|| value.pointer("/link/current_particle_key_sha256")),
            )
        } else {
            Value::Null
        },
    );
    summary.insert(
        "particle_history_key_sha256s".to_owned(),
        if is_trails {
            value_or_null(
                value
                    .pointer("/receipt/particle_history_key_sha256s")
                    .or_else(|| value.pointer("/link/particle_history_key_sha256s")),
            )
        } else {
            Value::Null
        },
    );
    summary.insert(
        "history_time_ticks".to_owned(),
        if is_trails {
            value_or_null(value.pointer("/receipt/history_time_ticks"))
        } else {
            Value::Null
        },
    );
    summary.insert(
        "trail_passes".to_owned(),
        if is_trails {
            value_or_null(value.pointer("/receipt/pass_artifacts"))
        } else {
            Value::Null
        },
    );
    summary.insert(
        "source_trail_passes".to_owned(),
        if is_trails_bloom {
            value_or_null(
                value
                    .pointer("/receipt/source_trail_passes")
                    .or_else(|| value.pointer("/render_set/source_trail_passes")),
            )
        } else {
            Value::Null
        },
    );
    summary.insert(
        "base_aov_byte_exact_verified".to_owned(),
        if is_trails || is_trails_bloom {
            value_or_null(
                value
                    .pointer("/receipt/base_aov_byte_exact_verified")
                    .or_else(|| value.pointer("/render_set/base_aov_byte_exact_verified")),
            )
        } else {
            Value::Null
        },
    );
    summary.insert(
        "base_opaque_depth_byte_exact_reused".to_owned(),
        if is_trails_bloom {
            value_or_null(
                value
                    .pointer("/receipt/base_opaque_depth_byte_exact_reused")
                    .or_else(|| value.pointer("/render_set/base_opaque_depth_byte_exact_reused")),
            )
        } else {
            Value::Null
        },
    );
    summary.insert(
        "bloom_pass_byte_exact_reused".to_owned(),
        if is_trails || is_trails_bloom {
            value_or_null(
                value
                    .pointer("/receipt/bloom_pass_byte_exact_reused")
                    .or_else(|| value.pointer("/render_set/bloom_pass_byte_exact_reused")),
            )
        } else {
            Value::Null
        },
    );
    summary.insert(
        "particle_passes_byte_exact_reused".to_owned(),
        if is_trails || is_trails_bloom {
            value_or_null(
                value
                    .pointer("/receipt/particle_passes_byte_exact_reused")
                    .or_else(|| value.pointer("/render_set/particle_passes_byte_exact_reused")),
            )
        } else {
            Value::Null
        },
    );
    summary.insert(
        "source_trail_passes_byte_exact_reused".to_owned(),
        if is_trails_bloom {
            value_or_null(
                value
                    .pointer("/receipt/source_trail_passes_byte_exact_reused")
                    .or_else(|| value.pointer("/render_set/source_trail_passes_byte_exact_reused")),
            )
        } else {
            Value::Null
        },
    );
    summary.insert(
        "base_bloom_mutated".to_owned(),
        if is_trails_bloom {
            value_or_null(
                value
                    .pointer("/receipt/base_bloom_mutated")
                    .or_else(|| value.pointer("/render_set/base_bloom_mutated")),
            )
        } else {
            Value::Null
        },
    );
    summary.insert(
        "particle_passes_mutated".to_owned(),
        if is_trails_bloom {
            value_or_null(
                value
                    .pointer("/receipt/particle_passes_mutated")
                    .or_else(|| value.pointer("/render_set/particle_passes_mutated")),
            )
        } else {
            Value::Null
        },
    );
    summary.insert(
        "trail_passes_mutated".to_owned(),
        if is_trails_bloom {
            value_or_null(
                value
                    .pointer("/receipt/trail_passes_mutated")
                    .or_else(|| value.pointer("/render_set/trail_passes_mutated")),
            )
        } else {
            Value::Null
        },
    );
    summary.insert(
        "runtime_write_performed".to_owned(),
        Value::Bool(matches!(
            name,
            "fictional_energy_vfx_prepare"
                | "fictional_energy_vfx_rendered_frame_prepare"
                | "fictional_energy_vfx_rendered_sequence_prepare"
                | "fictional_energy_vfx_hdr_bloom_prepare"
                | "fictional_energy_vfx_particles_prepare"
                | "fictional_energy_vfx_trails_prepare"
                | "fictional_energy_vfx_trails_bloom_prepare"
        )),
    );
    summary.insert("candidate_confirmed".to_owned(), Value::Bool(false));
    summary.insert("export_performed".to_owned(), Value::Bool(false));
    summary.insert("actual_engine_roundtrip".to_owned(), Value::Bool(false));
    summary.insert(
        "quality_status".to_owned(),
        Value::String("structural_only".to_owned()),
    );
    summary.insert("structured_content_complete".to_owned(), Value::Bool(true));
    Some(serde_json::to_string(&Value::Object(summary)).unwrap_or_else(|_| "{}".to_owned()))
}

fn game_weapon_glb_socket_mcp_summary(name: &str, value: &Value) -> Option<String> {
    if !matches!(
        name,
        "game_weapon_glb_socket_prepare" | "game_weapon_glb_socket_get"
    ) {
        return None;
    }
    let is_prepare = name == "game_weapon_glb_socket_prepare";
    let receipt = value
        .get("receipt")
        .or_else(|| value.pointer("/durable_link/receipt"))
        .or_else(|| value.pointer("/link/receipt"));
    let link = value.get("link").or_else(|| value.get("durable_link"));
    let value_or_null = |candidate: Option<&Value>| candidate.cloned().unwrap_or(Value::Null);
    let lookup = |field: &str| {
        value
            .get(field)
            .or_else(|| receipt.and_then(|object| object.get(field)))
            .or_else(|| link.and_then(|object| object.get(field)))
    };
    let levels = value
        .get("levels")
        .or_else(|| receipt.and_then(|object| object.get("levels")))
        .and_then(Value::as_array);

    let mut lod_summaries = Vec::with_capacity(3);
    let mut all_renderable_exact = true;
    let mut all_bin_exact = true;
    let mut all_nodes_materialized = true;
    for lod_level in 0..3usize {
        let lod = levels.and_then(|items| {
            items.iter().find(|item| {
                item.get("lod_level").and_then(Value::as_u64) == Some(lod_level as u64)
            })
        });
        let lookup_lod = |field: &str| lod.and_then(|item| item.get(field));
        let source_node_count = value_or_null(lookup_lod("source_node_count"));
        let derived_node_count = value_or_null(lookup_lod("derived_node_count"));
        let socket_nodes_materialized = lookup_lod("socket_nodes_materialized")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let renderable_exact = lookup_lod("source_renderable_projection_exact")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let bin_exact = lookup_lod("source_bin_byte_exact")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        all_renderable_exact &= renderable_exact;
        all_bin_exact &= bin_exact;
        all_nodes_materialized &= socket_nodes_materialized;

        let mut summary = Map::new();
        summary.insert(
            "lod_level".to_owned(),
            lod.and_then(|item| item.get("lod_level"))
                .cloned()
                .unwrap_or_else(|| Value::from(lod_level as u64)),
        );
        for field in [
            "source_artifact_sha256",
            "source_artifact_readback_sha256",
            "derived_artifact_sha256",
            "derived_artifact_readback_sha256",
            "source_renderable_inventory_sha256",
            "derived_renderable_inventory_sha256",
            "socket_node_inventory_sha256",
            "source_bin_sha256",
            "derived_bin_sha256",
        ] {
            summary.insert(field.to_owned(), value_or_null(lookup_lod(field)));
        }
        summary.insert("source_node_count".to_owned(), source_node_count);
        summary.insert("derived_node_count".to_owned(), derived_node_count);
        summary.insert("socket_node_count".to_owned(), Value::from(6_u64));
        summary.insert(
            "source_renderable_projection_exact".to_owned(),
            Value::Bool(renderable_exact),
        );
        summary.insert("source_bin_byte_exact".to_owned(), Value::Bool(bin_exact));
        summary.insert(
            "socket_nodes_materialized".to_owned(),
            Value::Bool(socket_nodes_materialized),
        );
        lod_summaries.push(Value::Object(summary));
    }

    let mut summary = Map::new();
    summary.insert(
        "schema_version".to_owned(),
        Value::String("GameWeaponGlbSocketMcpSummary@1".to_owned()),
    );
    summary.insert("operation".to_owned(), Value::String(name.to_owned()));
    for field in [
        "socket_materialization_key_sha256",
        "receipt_object_sha256",
        "project_id",
        "delivery_manifest_object_sha256",
        "anchor_set_object_sha256",
        "anchor_set_canonical_sha256",
        "request_sha256",
        "socket_materialization_policy",
        "lod_scope",
        "socket_node_id_encoding_sha256",
    ] {
        summary.insert(field.to_owned(), value_or_null(lookup(field)));
    }
    summary.insert("lod_readback".to_owned(), Value::Array(lod_summaries));
    summary.insert("socket_node_count".to_owned(), Value::from(6_u64));
    summary.insert(
        "socket_node_counts".to_owned(),
        Value::Array(vec![
            Value::from(6_u64),
            Value::from(6_u64),
            Value::from(6_u64),
        ]),
    );
    summary.insert(
        "source_renderable_projection_exact".to_owned(),
        Value::Bool(all_renderable_exact),
    );
    summary.insert(
        "source_bin_byte_exact".to_owned(),
        Value::Bool(all_bin_exact),
    );
    summary.insert(
        "socket_nodes_materialized".to_owned(),
        Value::Bool(all_nodes_materialized),
    );
    summary.insert("restart_hash_verified".to_owned(), Value::Bool(!is_prepare));
    summary.insert(
        "runtime_write_performed".to_owned(),
        Value::Bool(is_prepare),
    );
    summary.insert("candidate_confirmed".to_owned(), Value::Bool(false));
    summary.insert("export_performed".to_owned(), Value::Bool(false));
    summary.insert("actual_engine_roundtrip".to_owned(), Value::Bool(false));
    summary.insert(
        "quality_status".to_owned(),
        Value::String("structural_only".to_owned()),
    );
    summary.insert("glb_bytes_in_summary".to_owned(), Value::Bool(false));
    summary.insert("commercial_engine_roundtrip".to_owned(), Value::Bool(false));
    summary.insert("structured_content_complete".to_owned(), Value::Bool(true));
    Some(serde_json::to_string(&Value::Object(summary)).unwrap_or_else(|_| "{}".to_owned()))
}

fn game_weapon_animated_glb_socket_mcp_summary(name: &str, value: &Value) -> Option<String> {
    if !matches!(
        name,
        "game_weapon_animated_glb_socket_prepare" | "game_weapon_animated_glb_socket_get"
    ) {
        return None;
    }
    let is_prepare = name == "game_weapon_animated_glb_socket_prepare";
    let receipt = value
        .get("receipt")
        .or_else(|| value.pointer("/durable_link/receipt"))
        .or_else(|| value.pointer("/link/receipt"));
    let link = value.get("link").or_else(|| value.get("durable_link"));
    let value_or_null = |candidate: Option<&Value>| candidate.cloned().unwrap_or(Value::Null);
    let lookup = |field: &str| {
        value
            .get(field)
            .or_else(|| receipt.and_then(|object| object.get(field)))
            .or_else(|| link.and_then(|object| object.get(field)))
    };
    let bool_lookup =
        |field: &str| Value::Bool(lookup(field).and_then(Value::as_bool).unwrap_or(false));
    let mut summary = Map::new();
    summary.insert(
        "schema_version".to_owned(),
        Value::String("GameWeaponAnimatedGlbSocketMcpSummary@1".to_owned()),
    );
    summary.insert("operation".to_owned(), Value::String(name.to_owned()));
    for field in [
        "animated_socket_materialization_key_sha256",
        "source_artifact_sha256",
        "source_artifact_readback_sha256",
        "animated_artifact_sha256",
        "animated_artifact_readback_sha256",
        "derived_animated_socket_artifact_sha256",
        "derived_animated_socket_artifact_readback_sha256",
        "receipt_object_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "project_id",
        "candidate_id",
        "candidate_state_sha256",
        "delivery_manifest_object_sha256",
        "lod0_artifact_sha256",
        "anchor_set_object_sha256",
        "anchor_set_canonical_sha256",
        "request_sha256",
        "socket_materialization_policy",
        "lod_scope",
        "socket_node_id_encoding_sha256",
        "source_animation_projection_sha256",
        "derived_animation_projection_sha256",
        "source_animation_validation_sha256",
        "derived_animation_validation_sha256",
        "source_renderable_inventory_sha256",
        "derived_renderable_inventory_sha256",
        "source_bin_sha256",
        "derived_bin_sha256",
        "socket_node_inventory_sha256",
    ] {
        summary.insert(field.to_owned(), value_or_null(lookup(field)));
    }
    for field in [
        "sampler_count",
        "channel_count",
        "node_count",
        "source_node_count",
        "derived_node_count",
    ] {
        summary.insert(field.to_owned(), value_or_null(lookup(field)));
    }
    summary.insert(
        "socket_node_count".to_owned(),
        value_or_null(lookup("socket_node_count")),
    );
    for field in [
        "animations_preserved",
        "channels_preserved",
        "samplers_preserved",
        "renderable_projection_exact",
        "bin_byte_exact",
        "source_static_projection_exact",
        "no_skinning",
        "no_morph_targets",
        "socket_nodes_materialized",
    ] {
        summary.insert(field.to_owned(), bool_lookup(field));
    }
    summary.insert(
        "restart_hash_verified".to_owned(),
        lookup("restart_hash_verified")
            .cloned()
            .unwrap_or_else(|| Value::Bool(!is_prepare)),
    );
    summary.insert(
        "runtime_write_performed".to_owned(),
        lookup("runtime_write_performed")
            .cloned()
            .unwrap_or_else(|| Value::Bool(is_prepare)),
    );
    summary.insert("candidate_confirmed".to_owned(), Value::Bool(false));
    summary.insert("export_performed".to_owned(), Value::Bool(false));
    summary.insert("actual_engine_roundtrip".to_owned(), Value::Bool(false));
    summary.insert("functional_semantics".to_owned(), Value::Bool(false));
    summary.insert(
        "quality_status".to_owned(),
        Value::String("structural_only".to_owned()),
    );
    summary.insert("structural_only".to_owned(), Value::Bool(true));
    summary.insert("glb_bytes_in_summary".to_owned(), Value::Bool(false));
    summary.insert("commercial_engine_roundtrip".to_owned(), Value::Bool(false));
    summary.insert("structured_content_complete".to_owned(), Value::Bool(true));
    Some(serde_json::to_string(&Value::Object(summary)).unwrap_or_else(|_| "{}".to_owned()))
}

fn game_weapon_animated_glb_socket_v2_mcp_summary(name: &str, value: &Value) -> Option<String> {
    if !matches!(
        name,
        "game_weapon_animated_glb_socket_v2_prepare" | "game_weapon_animated_glb_socket_v2_get"
    ) {
        return None;
    }
    let is_prepare = name == "game_weapon_animated_glb_socket_v2_prepare";
    let receipt = value.get("receipt");
    let durable_link = value.get("durable_link");
    let lookup = |field: &str| {
        value
            .get(field)
            .or_else(|| receipt.and_then(|record| record.get(field)))
            .or_else(|| durable_link.and_then(|record| record.get(field)))
            .cloned()
            .unwrap_or(Value::Null)
    };
    let mut summary = Map::new();
    summary.insert(
        "schema_version".to_owned(),
        Value::String("GameWeaponAnimatedGlbSocketMaterializationMcpSummary@2".to_owned()),
    );
    summary.insert("operation".to_owned(), Value::String(name.to_owned()));
    for field in [
        "animated_socket_materialization_key_sha256",
        "derived_animated_socket_artifact_sha256",
        "receipt_object_sha256",
        "project_id",
        "appearance_candidate_id",
        "appearance_candidate_state_sha256",
        "appearance_delivery_manifest_object_sha256",
        "clip_id",
        "clip_object_sha256",
        "clip_sha256",
        "anchor_set_object_sha256",
        "anchor_set_canonical_sha256",
        "socket_materialization_policy",
        "lod_scope",
        "materialization_status",
        "validator_status",
    ] {
        summary.insert(field.to_owned(), lookup(field));
    }
    for field in [
        "animations_preserved",
        "channels_preserved",
        "samplers_preserved",
        "renderable_projection_exact",
        "bin_byte_exact",
        "source_static_projection_exact",
        "appearance_material_projection_exact",
        "material_pack_identity_exact",
        "socket_nodes_materialized",
        "hard_gate_passed",
    ] {
        summary.insert(
            field.to_owned(),
            lookup(field)
                .as_bool()
                .map(Value::Bool)
                .unwrap_or(Value::Bool(false)),
        );
    }
    summary.insert(
        "replayed".to_owned(),
        value.get("replayed").cloned().unwrap_or(Value::Bool(false)),
    );
    summary.insert(
        "restart_hash_verified".to_owned(),
        value
            .get("restart_hash_verified")
            .cloned()
            .unwrap_or(Value::Bool(true)),
    );
    summary.insert(
        "runtime_write_performed".to_owned(),
        value
            .get("runtime_write_performed")
            .cloned()
            .unwrap_or(Value::Bool(is_prepare)),
    );
    summary.insert("candidate_confirmed".to_owned(), Value::Bool(false));
    summary.insert("version_created".to_owned(), Value::Bool(false));
    summary.insert("export_performed".to_owned(), Value::Bool(false));
    summary.insert("production_stage_advanced".to_owned(), Value::Bool(false));
    summary.insert("actual_engine_roundtrip".to_owned(), Value::Bool(false));
    summary.insert(
        "quality_status".to_owned(),
        Value::String("structural_only".to_owned()),
    );
    summary.insert(
        "visual_quality_status".to_owned(),
        Value::String("NOT_PROVEN".to_owned()),
    );
    summary.insert(
        "commercial_fps_quality_status".to_owned(),
        Value::String("NOT_PROVEN".to_owned()),
    );
    summary.insert(
        "human_review_status".to_owned(),
        Value::String("NOT_RUN".to_owned()),
    );
    summary.insert(
        "commercial_engine_status".to_owned(),
        Value::String("NOT_RUN".to_owned()),
    );
    summary.insert("structured_content_complete".to_owned(), Value::Bool(true));
    Some(serde_json::to_string(&Value::Object(summary)).unwrap_or_else(|_| "{}".to_owned()))
}

fn fictional_energy_vfx_animated_socket_attachment_mcp_summary(
    name: &str,
    value: &Value,
) -> Option<String> {
    if !matches!(
        name,
        "fictional_energy_vfx_animated_socket_attachment_prepare"
            | "fictional_energy_vfx_animated_socket_attachment_get"
    ) {
        return None;
    }
    let is_prepare = name == "fictional_energy_vfx_animated_socket_attachment_prepare";
    let attachment = value.get("attachment").and_then(Value::as_object);
    let value_or_null = |field: &str| {
        attachment
            .and_then(|object| object.get(field))
            .cloned()
            .unwrap_or(Value::Null)
    };
    let frames = attachment
        .and_then(|object| object.get("frames"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|frame| {
                    let mut summary = Map::new();
                    for field in [
                        "frame_index",
                        "sample_time_ticks",
                        "animation_pose_readback_sha256",
                        "socket_transform_inventory_sha256",
                        "socket_transform_readback_sha256",
                        "emitter_socket_bindings_sha256",
                        "trail_socket_bindings_sha256",
                        "base_frame_key_sha256",
                        "bloom_key_sha256",
                        "particle_key_sha256",
                        "trail_key_sha256",
                        "trail_bloom_key_sha256",
                        "canonical_sha256",
                    ] {
                        summary.insert(
                            field.to_owned(),
                            frame.get(field).cloned().unwrap_or(Value::Null),
                        );
                    }
                    Value::Object(summary)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut summary = Map::new();
    summary.insert(
        "schema_version".to_owned(),
        Value::String("FictionalEnergyVfxAnimatedSocketAttachmentMcpSummary@1".to_owned()),
    );
    summary.insert("operation".to_owned(), Value::String(name.to_owned()));
    for field in [
        "attachment_key_sha256",
        "project_id",
        "delivery_manifest_object_sha256",
        "candidate_id",
        "candidate_state_sha256",
        "source_artifact_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_socket_anchor_set_object_sha256",
        "animated_socket_anchor_set_canonical_sha256",
        "animation_clip_id",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animated_artifact_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "vfx_sequence_key_sha256",
        "vfx_sequence_canonical_sha256",
        "attachment_policy",
        "socket_node_id_encoding_sha256",
        "socket_roles_sha256",
        "frame_scope",
        "attachment_status",
        "canonical_sha256",
    ] {
        summary.insert(field.to_owned(), value_or_null(field));
    }
    summary.insert("frame_count".to_owned(), Value::from(frames.len() as u64));
    summary.insert("frames".to_owned(), Value::Array(frames));
    summary.insert(
        "replayed".to_owned(),
        value.get("replayed").cloned().unwrap_or(Value::Bool(false)),
    );
    summary.insert(
        "restart_hash_verified".to_owned(),
        value
            .get("restart_hash_verified")
            .cloned()
            .unwrap_or(Value::Bool(!is_prepare)),
    );
    summary.insert(
        "runtime_write_performed".to_owned(),
        value
            .get("runtime_write")
            .cloned()
            .unwrap_or(Value::Bool(is_prepare)),
    );
    for field in [
        "quality_status",
        "visual_quality_status",
        "commercial_fps_quality_status",
        "human_review_status",
        "commercial_engine_status",
    ] {
        summary.insert(
            field.to_owned(),
            value
                .get(field)
                .cloned()
                .unwrap_or_else(|| Value::String("NOT_PROVEN".to_owned())),
        );
    }
    summary.insert("actual_engine_roundtrip".to_owned(), Value::Bool(false));
    summary.insert("production_stage_advanced".to_owned(), Value::Bool(false));
    summary.insert("candidate_confirmed".to_owned(), Value::Bool(false));
    summary.insert("version_created".to_owned(), Value::Bool(false));
    summary.insert("export_performed".to_owned(), Value::Bool(false));
    summary.insert("glb_bytes_in_summary".to_owned(), Value::Bool(false));
    summary.insert("png_bytes_in_summary".to_owned(), Value::Bool(false));
    summary.insert("aov_bytes_in_summary".to_owned(), Value::Bool(false));
    summary.insert("structured_content_complete".to_owned(), Value::Bool(true));
    Some(serde_json::to_string(&Value::Object(summary)).unwrap_or_else(|_| "{}".to_owned()))
}

fn fictional_energy_vfx_animated_socket_attachment_v2_mcp_summary(
    name: &str,
    value: &Value,
) -> Option<String> {
    if !matches!(
        name,
        "fictional_energy_vfx_animated_socket_attachment_v2_prepare"
            | "fictional_energy_vfx_animated_socket_attachment_v2_get"
    ) {
        return None;
    }
    let is_prepare = name == "fictional_energy_vfx_animated_socket_attachment_v2_prepare";
    let attachment = value.get("attachment").and_then(Value::as_object);
    let value_or_null = |field: &str| {
        attachment
            .and_then(|object| object.get(field))
            .cloned()
            .unwrap_or(Value::Null)
    };
    let frames = attachment
        .and_then(|object| object.get("frames"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|frame| {
                    let mut summary = Map::new();
                    for field in [
                        "frame_index",
                        "projection_frame_index",
                        "particle_sequence_frame_index",
                        "sample_time_ticks",
                        "animation_pose_readback_sha256",
                        "socket_transform_inventory_sha256",
                        "socket_transform_readback_sha256",
                        "emitter_socket_bindings_sha256",
                        "trail_socket_bindings_sha256",
                        "base_frame_key_sha256",
                        "bloom_key_sha256",
                        "particle_key_sha256",
                        "trail_key_sha256",
                        "trail_bloom_key_sha256",
                        "projection_frame_canonical_sha256",
                        "particle_sequence_frame_canonical_sha256",
                        "trail_sequence_frame_canonical_sha256",
                        "trail_bloom_sequence_frame_canonical_sha256",
                        "canonical_sha256",
                    ] {
                        summary.insert(
                            field.to_owned(),
                            frame.get(field).cloned().unwrap_or(Value::Null),
                        );
                    }
                    Value::Object(summary)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut summary = Map::new();
    summary.insert(
        "schema_version".to_owned(),
        Value::String("FictionalEnergyVfxAnimatedSocketAttachmentMcpSummary@2".to_owned()),
    );
    summary.insert("operation".to_owned(), Value::String(name.to_owned()));
    for field in [
        "attachment_key_sha256",
        "project_id",
        "delivery_manifest_object_sha256",
        "candidate_id",
        "candidate_state_sha256",
        "source_artifact_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_socket_anchor_set_object_sha256",
        "animated_socket_anchor_set_canonical_sha256",
        "animation_clip_id",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animated_artifact_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "projection_key_sha256",
        "projection_object_sha256",
        "projection_canonical_sha256",
        "particle_sequence_key_sha256",
        "particle_sequence_canonical_sha256",
        "trail_sequence_key_sha256",
        "trail_sequence_canonical_sha256",
        "trail_bloom_sequence_key_sha256",
        "trail_bloom_sequence_canonical_sha256",
        "attachment_policy",
        "socket_node_id_encoding_sha256",
        "socket_roles_sha256",
        "frame_scope",
        "attachment_status",
        "canonical_sha256",
    ] {
        summary.insert(field.to_owned(), value_or_null(field));
    }
    summary.insert("frame_count".to_owned(), Value::from(frames.len() as u64));
    summary.insert("frames".to_owned(), Value::Array(frames));
    summary.insert(
        "replayed".to_owned(),
        value
            .get("replayed")
            .cloned()
            .unwrap_or(Value::Bool(!is_prepare)),
    );
    summary.insert(
        "restart_hash_verified".to_owned(),
        value
            .get("restart_hash_verified")
            .cloned()
            .unwrap_or(Value::Bool(!is_prepare)),
    );
    summary.insert(
        "runtime_write_performed".to_owned(),
        value
            .get("runtime_write")
            .cloned()
            .unwrap_or(Value::Bool(is_prepare)),
    );
    for field in [
        "quality_status",
        "visual_quality_status",
        "commercial_fps_quality_status",
        "human_review_status",
        "commercial_engine_status",
    ] {
        summary.insert(
            field.to_owned(),
            value
                .get(field)
                .cloned()
                .unwrap_or_else(|| Value::String("NOT_PROVEN".to_owned())),
        );
    }
    summary.insert("actual_engine_roundtrip".to_owned(), Value::Bool(false));
    summary.insert("production_stage_advanced".to_owned(), Value::Bool(false));
    summary.insert("candidate_confirmed".to_owned(), Value::Bool(false));
    summary.insert("version_created".to_owned(), Value::Bool(false));
    summary.insert("export_performed".to_owned(), Value::Bool(false));
    summary.insert("glb_bytes_in_summary".to_owned(), Value::Bool(false));
    summary.insert("png_bytes_in_summary".to_owned(), Value::Bool(false));
    summary.insert("aov_bytes_in_summary".to_owned(), Value::Bool(false));
    summary.insert("structured_content_complete".to_owned(), Value::Bool(true));
    Some(serde_json::to_string(&Value::Object(summary)).unwrap_or_else(|_| "{}".to_owned()))
}

fn fictional_energy_vfx_animated_socket_attachment_v3_mcp_summary(
    name: &str,
    value: &Value,
) -> Option<String> {
    if !matches!(
        name,
        "fictional_energy_vfx_animated_socket_attachment_v3_prepare"
            | "fictional_energy_vfx_animated_socket_attachment_v3_get"
    ) {
        return None;
    }
    let is_prepare = name == "fictional_energy_vfx_animated_socket_attachment_v3_prepare";
    let attachment = value.get("attachment").and_then(Value::as_object);
    let value_or_null = |field: &str| {
        attachment
            .and_then(|object| object.get(field))
            .cloned()
            .unwrap_or(Value::Null)
    };
    let frames = attachment
        .and_then(|object| object.get("frames"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|frame| {
                    let mut summary = Map::new();
                    for field in [
                        "frame_index",
                        "sample_time_ticks",
                        "projection_frame_index",
                        "particle_sequence_frame_index",
                        "trail_frame_index",
                        "trail_bloom_frame_index",
                        "projection_frame_canonical_sha256",
                        "projection_socket_transform_inventory_sha256",
                        "projection_socket_transform_readback_sha256",
                        "particle_sequence_key_sha256",
                        "particle_sequence_frame_canonical_sha256",
                        "trail_sequence_key_sha256",
                        "trail_sequence_frame_canonical_sha256",
                        "trail_key_sha256",
                        "trail_inventory_sha256",
                        "trail_id_encoding_sha256",
                        "emitter_binding_sha256",
                        "trail_bloom_sequence_key_sha256",
                        "trail_bloom_sequence_frame_canonical_sha256",
                        "trail_bloom_key_sha256",
                        "trail_bloom_seed_sha256",
                        "base_frame_key_sha256",
                        "bloom_key_sha256",
                        "camera_object_sha256",
                        "camera_identity_sha256",
                        "render_profile_sha256",
                        "render_worker_build_cohort_sha256",
                        "canonical_sha256",
                    ] {
                        summary.insert(
                            field.to_owned(),
                            frame.get(field).cloned().unwrap_or(Value::Null),
                        );
                    }
                    Value::Object(summary)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut summary = Map::new();
    summary.insert(
        "schema_version".to_owned(),
        Value::String("FictionalEnergyVfxAnimatedSocketAttachmentMcpSummary@3".to_owned()),
    );
    summary.insert("operation".to_owned(), Value::String(name.to_owned()));
    for field in [
        "attachment_key_sha256",
        "project_id",
        "geometry_candidate_id",
        "geometry_candidate_state_sha256",
        "geometry_delivery_manifest_object_sha256",
        "geometry_artifact_sha256",
        "appearance_candidate_id",
        "appearance_candidate_state_sha256",
        "appearance_delivery_manifest_object_sha256",
        "appearance_artifact_sha256",
        "material_surface_quality_id",
        "material_surface_quality_report_object_sha256",
        "material_surface_quality_canonical_sha256",
        "geometry_preservation_projection_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_artifact_sha256",
        "animated_socket_anchor_set_object_sha256",
        "animated_socket_anchor_set_canonical_sha256",
        "appearance_anchor_set_object_sha256",
        "appearance_anchor_set_canonical_sha256",
        "anchor_binding_sha256",
        "animation_clip_id",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "projection_key_sha256",
        "projection_object_sha256",
        "projection_canonical_sha256",
        "particle_sequence_key_sha256",
        "particle_sequence_canonical_sha256",
        "trail_sequence_key_sha256",
        "trail_sequence_canonical_sha256",
        "trail_bloom_sequence_key_sha256",
        "trail_bloom_sequence_canonical_sha256",
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "trail_bloom_profile_sha256",
        "socket_node_id_encoding_sha256",
        "socket_roles_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
        "sample_schedule_sha256",
        "sample_count",
        "attachment_policy",
        "frame_scope",
        "attachment_receipt_object_sha256",
        "attachment_receipt_canonical_sha256",
        "attachment_status",
        "canonical_sha256",
    ] {
        summary.insert(field.to_owned(), value_or_null(field));
    }
    summary.insert("frame_count".to_owned(), Value::from(frames.len() as u64));
    summary.insert("frames".to_owned(), Value::Array(frames));
    summary.insert(
        "replayed".to_owned(),
        value
            .get("replayed")
            .cloned()
            .unwrap_or(Value::Bool(!is_prepare)),
    );
    summary.insert(
        "restart_hash_verified".to_owned(),
        value
            .get("restart_hash_verified")
            .cloned()
            .unwrap_or(Value::Bool(true)),
    );
    summary.insert(
        "runtime_write_performed".to_owned(),
        value
            .get("runtime_write")
            .cloned()
            .unwrap_or(Value::Bool(is_prepare)),
    );
    for field in [
        "quality_status",
        "visual_quality_status",
        "commercial_fps_quality_status",
        "human_review_status",
        "commercial_engine_status",
    ] {
        summary.insert(
            field.to_owned(),
            value
                .get(field)
                .cloned()
                .unwrap_or_else(|| Value::String("NOT_PROVEN".to_owned())),
        );
    }
    summary.insert("actual_engine_roundtrip".to_owned(), Value::Bool(false));
    summary.insert("production_stage_advanced".to_owned(), Value::Bool(false));
    summary.insert("candidate_confirmed".to_owned(), Value::Bool(false));
    summary.insert("version_created".to_owned(), Value::Bool(false));
    summary.insert("export_performed".to_owned(), Value::Bool(false));
    summary.insert("glb_bytes_in_summary".to_owned(), Value::Bool(false));
    summary.insert("png_bytes_in_summary".to_owned(), Value::Bool(false));
    summary.insert("aov_bytes_in_summary".to_owned(), Value::Bool(false));
    summary.insert("structured_content_complete".to_owned(), Value::Bool(true));
    Some(serde_json::to_string(&Value::Object(summary)).unwrap_or_else(|_| "{}".to_owned()))
}

fn call_tool(
    backend: &mut Backend,
    id: Option<Value>,
    params: Option<&Value>,
    session: &mut Session,
) -> Option<Value> {
    let write_tools_enabled = advertised_write_tools_enabled(backend, session.write_tools_enabled);
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
    if (agentic_write_tools::is_tool(name)
        || agentic_action_tools::is_tool(name)
        || agentic_orchestrator_tools::is_tool(name)
        || cross_view_promotion_tools::is_tool(name)
        || optimization_tools::is_tool(name))
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
        if name == "subdivision_artifact_lineage_prepare" {
            return Some(json!({
                "jsonrpc":"2.0",
                "id":id,
                "result":{
                    "isError":true,
                    "content":[{"type":"text","text":serde_json::to_string(&runtime_error_value("MCP010F_SUBDIVISION_ARTIFACT_LINEAGE_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")).unwrap_or_else(|_| "{}".to_owned())}],
                    "structuredContent":runtime_error_value("MCP010F_SUBDIVISION_ARTIFACT_LINEAGE_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")
                }
            }));
        }
        if name == "mechanical_animation_clip_prepare" {
            return Some(json!({
                "jsonrpc":"2.0",
                "id":id,
                "result":{
                    "isError":true,
                    "content":[{"type":"text","text":serde_json::to_string(&runtime_error_value("MCP010F_MECHANICAL_ANIMATION_CLIP_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")).unwrap_or_else(|_| "{}".to_owned())}],
                    "structuredContent":runtime_error_value("MCP010F_MECHANICAL_ANIMATION_CLIP_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")
                }
            }));
        }
        if name == "mechanical_animation_glb_prepare" {
            return Some(json!({
                "jsonrpc":"2.0",
                "id":id,
                "result":{
                    "isError":true,
                    "content":[{"type":"text","text":serde_json::to_string(&runtime_error_value("MCP010F_MECHANICAL_ANIMATION_GLB_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")).unwrap_or_else(|_| "{}".to_owned())}],
                    "structuredContent":runtime_error_value("MCP010F_MECHANICAL_ANIMATION_GLB_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")
                }
            }));
        }
        if name == "game_asset_delivery_prepare" {
            return Some(json!({
                "jsonrpc":"2.0",
                "id":id,
                "result":{
                    "isError":true,
                    "content":[{"type":"text","text":serde_json::to_string(&runtime_error_value("MCP010F_GAME_ASSET_DELIVERY_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")).unwrap_or_else(|_| "{}".to_owned())}],
                    "structuredContent":runtime_error_value("MCP010F_GAME_ASSET_DELIVERY_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")
                }
            }));
        }
        if name == "appearance_source_lineage_prepare" {
            return Some(json!({
                "jsonrpc":"2.0",
                "id":id,
                "result":{
                    "isError":true,
                    "content":[{"type":"text","text":serde_json::to_string(&runtime_error_value("MCP010F_APPEARANCE_SOURCE_LINEAGE_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")).unwrap_or_else(|_| "{}".to_owned())}],
                    "structuredContent":runtime_error_value("MCP010F_APPEARANCE_SOURCE_LINEAGE_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")
                }
            }));
        }
        if name == "game_weapon_anchor_prepare" {
            return Some(json!({
                "jsonrpc":"2.0",
                "id":id,
                "result":{
                    "isError":true,
                    "content":[{"type":"text","text":serde_json::to_string(&runtime_error_value("MCP010F_GAME_WEAPON_ANCHOR_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")).unwrap_or_else(|_| "{}".to_owned())}],
                    "structuredContent":runtime_error_value("MCP010F_GAME_WEAPON_ANCHOR_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")
                }
            }));
        }
        if name == "game_weapon_animated_glb_socket_prepare" {
            return Some(json!({
                "jsonrpc":"2.0",
                "id":id,
                "result":{
                    "isError":true,
                    "content":[{"type":"text","text":serde_json::to_string(&runtime_error_value("MCP010F_GAME_WEAPON_ANIMATED_GLB_SOCKET_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")).unwrap_or_else(|_| "{}".to_owned())}],
                    "structuredContent":runtime_error_value("MCP010F_GAME_WEAPON_ANIMATED_GLB_SOCKET_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")
                }
            }));
        }
        if name == "fictional_energy_vfx_prepare" {
            return Some(json!({
                "jsonrpc":"2.0","id":id,
                "result":{
                    "isError":true,
                    "content":[{"type":"text","text":serde_json::to_string(&runtime_error_value("MCP010F_FICTIONAL_ENERGY_VFX_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")).unwrap_or_else(|_| "{}".to_owned())}],
                    "structuredContent":runtime_error_value("MCP010F_FICTIONAL_ENERGY_VFX_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")
                }
            }));
        }
        if is_mcp010f_write_tool(name) {
            return Some(json!({
                "jsonrpc":"2.0","id":id,
                "result":{
                    "isError":true,
                    "content":[{"type":"text","text":serde_json::to_string(&runtime_error_value("MCP010F_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")).unwrap_or_else(|_| "{}".to_owned())}],
                    "structuredContent":runtime_error_value("MCP010F_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")
                }
            }));
        }
        if name == "authoring_mesh_edit_prepare" {
            return Some(json!({
                "jsonrpc":"2.0",
                "id":id,
                "result":{
                    "isError":true,
                    "content":[{"type":"text","text":serde_json::to_string(&runtime_error_value("MCP010F_AUTHORING_MESH_EDIT_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")).unwrap_or_else(|_| "{}".to_owned())}],
                    "structuredContent":runtime_error_value("MCP010F_AUTHORING_MESH_EDIT_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")
                }
            }));
        }
        if agentic_action_tools::is_write_tool(name) {
            return Some(json!({
                "jsonrpc":"2.0",
                "id":id,
                "result":{
                    "isError":true,
                    "content":[{"type":"text","text":serde_json::to_string(&runtime_error_value("AGENTIC_ACTION_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")).unwrap_or_else(|_| "{}".to_owned())}],
                    "structuredContent":runtime_error_value("AGENTIC_ACTION_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")
                }
            }));
        }
        if agentic_orchestrator_tools::is_write_tool(name) {
            return Some(json!({
                "jsonrpc":"2.0",
                "id":id,
                "result":{
                    "isError":true,
                    "content":[{"type":"text","text":serde_json::to_string(&runtime_error_value("AGENTIC_ORCHESTRATOR_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")).unwrap_or_else(|_| "{}".to_owned())}],
                    "structuredContent":runtime_error_value("AGENTIC_ORCHESTRATOR_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")
                }
            }));
        }
        if cross_view_promotion_tools::is_write_tool(name) {
            return Some(json!({
                "jsonrpc":"2.0",
                "id":id,
                "result":{
                    "isError":true,
                    "content":[{"type":"text","text":serde_json::to_string(&runtime_error_value("CROSS_VIEW_PROMOTION_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")).unwrap_or_else(|_| "{}".to_owned())}],
                    "structuredContent":runtime_error_value("CROSS_VIEW_PROMOTION_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")
                }
            }));
        }
        if optimization_tools::is_write_tool(name) {
            return Some(json!({
                "jsonrpc":"2.0",
                "id":id,
                "result":{
                    "isError":true,
                    "content":[{"type":"text","text":serde_json::to_string(&runtime_error_value("OPTIMIZATION_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")).unwrap_or_else(|_| "{}".to_owned())}],
                    "structuredContent":runtime_error_value("OPTIMIZATION_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required")
                }
            }));
        }
        if agentic_write_tools::is_write_tool(name) {
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
        if let Err(error) =
            agentic_write_tools::validate_call(name, &arguments, &session.agentic_binding)
        {
            return Some(json!({
                "jsonrpc":"2.0",
                "id":id,
                "result":{"isError":true,"content":[{"type":"text","text":serde_json::to_string(&runtime_error_value(&error)).unwrap_or_else(|_| "{}".to_owned())}],"structuredContent":runtime_error_value(&error)}
            }));
        }
    }
    if agentic_action_tools::is_tool(name) {
        if let Err(error) =
            agentic_action_tools::validate_call(name, &arguments, &session.action_binding)
        {
            return Some(json!({
                "jsonrpc":"2.0",
                "id":id,
                "result":{"isError":true,"content":[{"type":"text","text":serde_json::to_string(&runtime_error_value(&error)).unwrap_or_else(|_| "{}".to_owned())}],"structuredContent":runtime_error_value(&error)}
            }));
        }
    }
    if agentic_orchestrator_tools::is_tool(name) {
        if let Err(error) = agentic_orchestrator_tools::validate_call(
            name,
            &arguments,
            &session.orchestrator_binding,
        ) {
            return Some(json!({
                "jsonrpc":"2.0",
                "id":id,
                "result":{"isError":true,"content":[{"type":"text","text":serde_json::to_string(&runtime_error_value(&error)).unwrap_or_else(|_| "{}".to_owned())}],"structuredContent":runtime_error_value(&error)}
            }));
        }
    }
    if optimization_tools::is_tool(name) {
        if let Err(error) =
            optimization_tools::validate_call(name, &arguments, &session.optimization_binding)
        {
            return Some(json!({
                "jsonrpc":"2.0",
                "id":id,
                "result":{"isError":true,"content":[{"type":"text","text":serde_json::to_string(&runtime_error_value(&error)).unwrap_or_else(|_| "{}".to_owned())}],"structuredContent":runtime_error_value(&error)}
            }));
        }
    }
    if cross_view_promotion_tools::is_tool(name) {
        if let Err(error) = cross_view_promotion_tools::validate_call(
            name,
            &arguments,
            &session.cross_view_promotion_binding,
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
                if let Err(error) =
                    agentic_write_tools::bind_response(name, &value, &mut session.agentic_binding)
                {
                    return Some(json!({
                        "jsonrpc":"2.0",
                        "id":id,
                        "result":{"isError":true,"content":[{"type":"text","text":serde_json::to_string(&runtime_error_value(&error)).unwrap_or_else(|_| "{}".to_owned())}],"structuredContent":runtime_error_value(&error)}
                    }));
                }
            }
            if agentic_action_tools::is_tool(name) {
                if let Err(error) =
                    agentic_action_tools::validate_response(name, &value, &session.action_binding)
                        .and_then(|_| {
                            agentic_action_tools::bind_response(
                                name,
                                &value,
                                &mut session.action_binding,
                            )
                        })
                {
                    return Some(json!({
                        "jsonrpc":"2.0",
                        "id":id,
                        "result":{"isError":true,"content":[{"type":"text","text":serde_json::to_string(&runtime_error_value(&error)).unwrap_or_else(|_| "{}".to_owned())}],"structuredContent":runtime_error_value(&error)}
                    }));
                }
            }
            if agentic_orchestrator_tools::is_tool(name) {
                if let Err(error) = agentic_orchestrator_tools::validate_response(
                    name,
                    &value,
                    &session.orchestrator_binding,
                )
                .and_then(|_| {
                    agentic_orchestrator_tools::bind_response(
                        name,
                        &value,
                        &mut session.orchestrator_binding,
                    )
                }) {
                    return Some(json!({
                        "jsonrpc":"2.0",
                        "id":id,
                        "result":{"isError":true,"content":[{"type":"text","text":serde_json::to_string(&runtime_error_value(&error)).unwrap_or_else(|_| "{}".to_owned())}],"structuredContent":runtime_error_value(&error)}
                    }));
                }
            }
            if optimization_tools::is_tool(name) {
                if let Err(error) = optimization_tools::validate_response(
                    name,
                    &value,
                    &session.optimization_binding,
                )
                .and_then(|_| {
                    optimization_tools::bind_response(
                        name,
                        &value,
                        &mut session.optimization_binding,
                    )
                }) {
                    return Some(json!({
                        "jsonrpc":"2.0",
                        "id":id,
                        "result":{"isError":true,"content":[{"type":"text","text":serde_json::to_string(&runtime_error_value(&error)).unwrap_or_else(|_| "{}".to_owned())}],"structuredContent":runtime_error_value(&error)}
                    }));
                }
            }
            if cross_view_promotion_tools::is_tool(name) {
                if let Err(error) = cross_view_promotion_tools::validate_response(
                    name,
                    &value,
                    &session.cross_view_promotion_binding,
                )
                .and_then(|_| {
                    cross_view_promotion_tools::bind_response(
                        name,
                        &value,
                        &mut session.cross_view_promotion_binding,
                    )
                }) {
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
            let content_text = if name == "topology_snapshot_get" {
                serde_json::to_string(&json!({
                    "schema_version":"TopologySnapshotMcpSummary@1",
                    "artifact_id":value.get("artifact_id"),
                    "candidate_id":value.get("candidate_id"),
                    "part_id":value.get("part_id"),
                    "counts":value.get("counts"),
                    "topology":value.get("topology"),
                    "canonical_sha256":value.get("canonical_sha256"),
                    "structured_content_complete":true
                }))
                .unwrap_or_else(|_| "{}".to_owned())
            } else if name == "authoring_topology_get" {
                serde_json::to_string(&json!({
                    "schema_version":"AuthoringTopologyMcpSummary@1",
                    "artifact_id":value.get("artifact_id"),
                    "candidate_id":value.get("candidate_id"),
                    "authoring_node_id":value.get("authoring_node_id"),
                    "part_id":value.get("part_id"),
                    "counts":value.get("counts"),
                    "topology_sha256":value.get("topology_sha256"),
                    "canonical_sha256":value.get("canonical_sha256"),
                    "structured_content_complete":true
                }))
                .unwrap_or_else(|_| "{}".to_owned())
            } else if name == "authoring_mesh_edit_preview" {
                serde_json::to_string(&json!({
                    "schema_version":"AuthoringMeshEditPreviewMcpSummary@1",
                    "candidate_id":value.get("candidate_id"),
                    "source_artifact_id":value.get("source_artifact_id"),
                    "source_program_sha256":value.get("source_program_sha256"),
                    "derived_program_sha256":value.get("derived_program_sha256"),
                    "operation":value.get("operation"),
                    "counts":value.get("counts"),
                    "derived_replay":value.get("derived_replay"),
                    "canonical_sha256":value.get("canonical_sha256"),
                    "structured_content_complete":true
                }))
                .unwrap_or_else(|_| "{}".to_owned())
            } else if name == "authoring_mesh_edit_prepare" {
                serde_json::to_string(&json!({
                    "schema_version":"AuthoringMeshEditPrepareMcpSummary@1",
                    "source_candidate_id":value.get("source_candidate_id"),
                    "new_candidate_id":value.get("new_candidate_id"),
                    "derived_artifact_sha256":value.get("derived_artifact_sha256"),
                    "derived_program_sha256":value.get("derived_program_sha256"),
                    "preview_canonical_sha256":value.get("preview_canonical_sha256"),
                    "edit_lineage_sha256":value.get("edit_lineage_sha256"),
                    "runtime_write_performed":value.get("runtime_write_performed"),
                    "confirm_status":value.get("confirm_status"),
                    "canonical_sha256":value.get("canonical_sha256"),
                    "structured_content_complete":true
                }))
                .unwrap_or_else(|_| "{}".to_owned())
            } else if name == "mechanical_pose_evaluate" {
                serde_json::to_string(&json!({
                    "schema_version":"MechanicalPoseMcpSummary@1",
                    "result_schema_version":value.get("schema_version"),
                    "artifact_id":value.get("artifact_id"),
                    "candidate_id":value.get("candidate_id"),
                    "sample_time_ticks":value.get("sample_time_ticks"),
                    "evaluated_pose_sha256":value.get("evaluated_pose_sha256"),
                    "sequence_sha256":value.get("sequence_sha256"),
                    "canonical_sha256":value.get("canonical_sha256"),
                    "structured_content_complete":true
                }))
                .unwrap_or_else(|_| "{}".to_owned())
            } else if name == "mechanical_pose_geometry_preview" {
                serde_json::to_string(&json!({
                    "schema_version":"MechanicalPoseGeometryPreviewMcpSummary@1",
                    "candidate_id":value.get("candidate_id"),
                    "source_artifact_id":value.get("source_artifact_id"),
                    "source_program_sha256":value.get("source_program_sha256"),
                    "posed_program_sha256":value.get("posed_program_sha256"),
                    "part_deltas_sha256":value.get("part_deltas_sha256"),
                    "transient_artifact":value.get("transient_artifact"),
                    "runtime_write_performed":false,
                    "quality_status":value.get("quality_status"),
                    "canonical_sha256":value.get("canonical_sha256"),
                    "structured_content_complete":true
                }))
                .unwrap_or_else(|_| "{}".to_owned())
            } else if matches!(
                name,
                "mechanical_animation_clip_v2_prepare" | "mechanical_animation_clip_v2_get"
            ) {
                let is_prepare = name == "mechanical_animation_clip_v2_prepare";
                let clip = value.get("clip");
                let link = value.get("durable_link");
                serde_json::to_string(&json!({
                    "schema_version":"MechanicalAnimationClipV2McpSummary@2",
                    "operation":name,
                    "clip_id":clip.and_then(|record| record.get("clip_id")),
                    "project_id":clip.and_then(|record| record.get("project_id")),
                    "appearance_candidate_id":clip.and_then(|record| record.get("appearance_candidate_id")),
                    "appearance_artifact_sha256":clip.and_then(|record| record.get("appearance_artifact_sha256")),
                    "source_geometry_artifact_sha256":clip.and_then(|record| record.get("source_geometry_artifact_sha256")),
                    "clip_object_sha256":link.and_then(|record| record.get("clip_object_sha256")),
                    "clip_sha256":clip.and_then(|record| record.get("clip_sha256")),
                    "source_replay_worker_cohort_sha256":clip.and_then(|record| record.get("source_replay_worker_cohort_sha256")),
                    "write_intent":if is_prepare {"explicit_runtime_appearance_aware_clip_prepare_write"} else {"read_only_durable_appearance_aware_clip_lookup"},
                    "runtime_write_performed":value.get("runtime_write_performed"),
                    "restart_hash_verified":value.get("restart_hash_verified"),
                    "replayed":value.get("replayed"),
                    "quality_status":"structural_only",
                    "visual_quality_status":"NOT_PROVEN",
                    "commercial_fps_quality_status":"NOT_PROVEN",
                    "human_review_status":"NOT_RUN",
                    "commercial_engine_status":"NOT_RUN",
                    "candidate_confirmed":false,
                    "version_created":false,
                    "export_performed":false,
                    "raw_glb_bytes":false,
                    "raw_png_bytes":false,
                    "structured_content_complete":true
                }))
                .unwrap_or_else(|_| "{}".to_owned())
            } else if name == "mechanical_animation_clip_v2_preview" {
                serde_json::to_string(&json!({
                    "schema_version":"MechanicalAnimationClipV2PreviewMcpSummary@2",
                    "operation":name,
                    "project_id":value.get("project_id"),
                    "appearance_candidate_id":value.get("appearance_candidate_id"),
                    "clip_id":value.get("clip_id"),
                    "sample_time_ticks":value.get("sample_time_ticks"),
                    "frame_sha256":value.get("frame_sha256"),
                    "source_replay_worker_cohort_sha256":value.get("source_replay_worker_cohort_sha256"),
                    "appearance_transient_artifact_sha256":value.get("appearance_transient_artifact_sha256"),
                    "appearance_transient_artifact_readback_sha256":value.get("appearance_transient_artifact_readback_sha256"),
                    "appearance_transient_program_sha256":value.get("appearance_transient_program_sha256"),
                    "appearance_replay_worker_cohort_sha256":value.get("appearance_replay_worker_cohort_sha256"),
                    "appearance_program_sha256":value.get("appearance_program_sha256"),
                    "material_pack_manifest_sha256":value.get("material_pack_manifest_sha256"),
                    "geometry_preservation_projection_sha256":value.get("geometry_preservation_projection_sha256"),
                    "geometry_materialization":value.get("geometry_materialization"),
                    "appearance_materialization":value.get("appearance_materialization"),
                    "runtime_write_performed":false,
                    "persistent_user_data_touched":false,
                    "quality_status":"structural_only",
                    "visual_quality_status":"NOT_PROVEN",
                    "commercial_fps_quality_status":"NOT_PROVEN",
                    "human_review_status":"NOT_RUN",
                    "commercial_engine_status":"NOT_RUN",
                    "raw_glb_bytes":false,
                    "raw_png_bytes":false,
                    "structured_content_complete":true
                }))
                .unwrap_or_else(|_| "{}".to_owned())
            } else if matches!(
                name,
                "mechanical_animation_glb_v2_prepare" | "mechanical_animation_glb_v2_get"
            ) {
                let is_prepare = name == "mechanical_animation_glb_v2_prepare";
                serde_json::to_string(&json!({
                    "schema_version":"MechanicalAnimationGlbV2McpSummary@2",
                    "operation":name,
                    "animation_glb_key_sha256":value.get("animation_glb_key_sha256"),
                    "animated_artifact_sha256":value.get("animated_artifact_sha256"),
                    "animated_artifact_size_bytes":value.get("animated_artifact_size_bytes"),
                    "receipt_object_sha256":value.get("receipt_object_sha256"),
                    "project_id":value.pointer("/receipt/project_id"),
                    "appearance_candidate_id":value.pointer("/receipt/appearance_candidate_id"),
                    "clip_id":value.pointer("/receipt/clip_id"),
                    "write_intent":if is_prepare {"explicit_runtime_appearance_aware_animated_glb_prepare_write"} else {"read_only_durable_appearance_aware_animated_glb_lookup"},
                    "runtime_write_performed":value.get("runtime_write_performed"),
                    "restart_hash_verified":value.get("restart_hash_verified"),
                    "replayed":value.get("replayed"),
                    "quality_status":"structural_only",
                    "visual_quality_status":"NOT_PROVEN",
                    "commercial_fps_quality_status":"NOT_PROVEN",
                    "human_review_status":"NOT_RUN",
                    "commercial_engine_status":"NOT_RUN",
                    "candidate_confirmed":false,
                    "version_created":false,
                    "export_performed":false,
                    "raw_glb_bytes":false,
                    "raw_png_bytes":false,
                    "structured_content_complete":true
                }))
                .unwrap_or_else(|_| "{}".to_owned())
            } else if matches!(
                name,
                "mechanical_animation_clip_prepare" | "mechanical_animation_clip_get"
            ) {
                let is_prepare = name == "mechanical_animation_clip_prepare";
                serde_json::to_string(&json!({
                    "schema_version":"MechanicalAnimationClipMcpSummary@1",
                    "operation":name,
                    "clip_id":value.get("clip_id"),
                    "candidate_id":value.get("candidate_id"),
                    "artifact_id":value.get("artifact_id"),
                    "clip_object_sha256":value.get("clip_object_sha256"),
                    "source_replay_worker_cohort_sha256":value.get("source_replay_worker_cohort_sha256"),
                    "write_intent":if is_prepare {"explicit_runtime_clip_prepare_write"} else {"read_only_durable_clip_lookup"},
                    "runtime_write_performed":is_prepare,
                    "canonical_sha256":value.get("canonical_sha256"),
                    "structured_content_complete":true
                }))
                .unwrap_or_else(|_| "{}".to_owned())
            } else if matches!(
                name,
                "game_weapon_anchor_prepare" | "game_weapon_anchor_get"
            ) {
                serde_json::to_string(&json!({
                    "schema_version":"GameWeaponAnchorMcpSummary@1",
                    "operation":name,
                    "anchor_set_object_sha256":value.get("anchor_set_object_sha256").or_else(|| value.pointer("/link/anchor_set_object_sha256")),
                    "delivery_manifest_object_sha256":value.pointer("/durable_link/delivery_manifest_object_sha256").or_else(|| value.pointer("/link/delivery_manifest_object_sha256")),
                    "anchor_roles":value.pointer("/anchor_set/anchors").and_then(Value::as_array).map(|anchors| anchors.iter().map(|anchor| anchor.get("role")).collect::<Vec<_>>()),
                    "animation_follow_status":value.pointer("/anchor_set/animation_follow_status"),
                    "pivot_status":value.pointer("/anchor_set/pivot_status"),
                    "runtime_write_performed":if name == "game_weapon_anchor_prepare" { Value::Bool(true) } else { value.get("runtime_write_performed").cloned().unwrap_or(Value::Bool(false)) },
                    "candidate_confirmed":false,
                    "export_performed":false,
                    "actual_engine_roundtrip":false,
                    "quality_status":"structural_only",
                    "structured_content_complete":true
                }))
                .unwrap_or_else(|_| "{}".to_owned())
            } else if let Some(summary) = game_weapon_glb_socket_mcp_summary(name, &value) {
                summary
            } else if let Some(summary) = game_weapon_animated_glb_socket_mcp_summary(name, &value)
            {
                summary
            } else if let Some(summary) =
                game_weapon_animated_glb_socket_v2_mcp_summary(name, &value)
            {
                summary
            } else if let Some(summary) =
                fictional_energy_vfx_animated_socket_attachment_mcp_summary(name, &value)
            {
                summary
            } else if let Some(summary) =
                fictional_energy_vfx_animated_socket_attachment_v2_mcp_summary(name, &value)
            {
                summary
            } else if let Some(summary) =
                fictional_energy_vfx_animated_socket_attachment_v3_mcp_summary(name, &value)
            {
                summary
            } else if let Some(summary) = fictional_energy_vfx_mcp_summary(name, &value) {
                summary
            } else if name == "mechanical_animation_clip_preview_get" {
                let pose_geometry_preview = value.get("pose_geometry_preview");
                serde_json::to_string(&json!({
                    "schema_version":"MechanicalAnimationClipPreviewMcpSummary@1",
                    "clip_id":value.get("clip_id"),
                    "candidate_id":value.get("candidate_id"),
                    "sample_time_ticks":value.get("sample_time_ticks"),
                    "transient_artifact":pose_geometry_preview.and_then(|preview| preview.get("transient_artifact")),
                    "worker_replay":pose_geometry_preview.and_then(|preview| preview.get("worker_replay")),
                    "runtime_write_performed":false,
                    "quality_status":value.get("quality_status"),
                    "canonical_sha256":value.get("canonical_sha256"),
                    "structured_content_complete":true
                }))
                .unwrap_or_else(|_| "{}".to_owned())
            } else if name == "mechanical_animation_glb_prepare" {
                serde_json::to_string(&json!({
                    "schema_version":"MechanicalAnimationGlbMcpSummary@1",
                    "animated_artifact_sha256":value.get("animated_artifact_sha256"),
                    "animated_artifact_size_bytes":value.get("animated_artifact_size_bytes"),
                    "receipt_object_sha256":value.get("receipt_object_sha256"),
                    "candidate_id":value.pointer("/receipt/candidate_id"),
                    "clip_id":value.pointer("/receipt/clip_id"),
                    "channel_count":value.pointer("/receipt/channel_count"),
                    "validator_status":value.pointer("/receipt/validator_status"),
                    "quality_status":value.pointer("/receipt/quality_status"),
                    "candidate_confirmed":false,
                    "export_performed":false,
                    "structured_content_complete":true
                }))
                .unwrap_or_else(|_| "{}".to_owned())
            } else if matches!(
                name,
                "game_asset_delivery_prepare" | "game_asset_delivery_get" | "game_asset_lod_derive"
            ) {
                serde_json::to_string(&json!({
                    "schema_version":"GameAssetDeliveryMcpSummary@1",
                    "operation":name,
                    "lod_receipt_object_sha256":value.get("lod_receipt_object_sha256").or_else(|| value.pointer("/link/lod_receipt_object_sha256")),
                    "collision_proxy_object_sha256":value.get("collision_proxy_object_sha256").or_else(|| value.pointer("/link/collision_proxy_object_sha256")),
                    "readiness_object_sha256":value.get("readiness_object_sha256").or_else(|| value.pointer("/link/readiness_object_sha256")),
                    "delivery_manifest_object_sha256":value.get("delivery_manifest_object_sha256").or_else(|| value.pointer("/link/delivery_manifest_object_sha256")),
                    "triangle_counts":value.pointer("/lod_receipt/levels").and_then(Value::as_array).map(|levels| levels.iter().map(|level| level.get("triangle_count")).collect::<Vec<_>>()),
                    "collision_proxy_count":value.pointer("/collision_proxy_set/proxies").and_then(Value::as_array).map(Vec::len),
                    "threejs_status":value.pointer("/readiness/engine_results/threejs"),
                    "durable_restart_hash_verified":value.get("restart_hash_verified"),
                    "derive_levels":value.get("levels").and_then(Value::as_array).map(|levels| levels.iter().map(|level| json!({"level":level.get("level"),"triangle_count":level.get("triangle_count"),"geometry_program_sha256":level.get("geometry_program_sha256")})).collect::<Vec<_>>()),
                    "runtime_write_performed":value.get("runtime_write_performed"),
                    "candidate_confirmed":false,
                    "export_performed":false,
                    "actual_engine_roundtrip":false,
                    "quality_status":"structural_only",
                    "structured_content_complete":true
                }))
                .unwrap_or_else(|_| "{}".to_owned())
            } else if matches!(
                name,
                "appearance_source_lineage_prepare" | "appearance_source_lineage_get"
            ) {
                serde_json::to_string(&json!({
                    "schema_version":"AppearanceSourceLineageMcpSummary@1",
                    "operation":name,
                    "sidecar_object_sha256":value.get("sidecar_object_sha256"),
                    "appearance_program_sha256":value.pointer("/durable_link/appearance_program_sha256"),
                    "geometry_program_sha256":value.pointer("/durable_link/geometry_program_sha256"),
                    "material_pack_manifest_sha256":value.pointer("/durable_link/material_pack_manifest_sha256"),
                    "texture_build_receipt_sha256":value.pointer("/durable_link/texture_build_receipt_sha256"),
                    "candidate_surface_bake_receipt_sha256":value.pointer("/durable_link/candidate_surface_bake_receipt_sha256"),
                    "lod_count":value.pointer("/durable_link/lod_candidate_ids").and_then(Value::as_array).map(Vec::len),
                    "lod_artifact_readback_object_sha256s":value.pointer("/durable_link/lod_artifact_readback_object_sha256s"),
                    "lod_part_binding_inventory_sha256s":value.pointer("/durable_link/lod_part_binding_inventory_sha256s"),
                    "runtime_write_performed":value.get("runtime_write_performed"),
                    "candidate_confirmed":false,
                    "export_performed":false,
                    "quality_status":"structural_only",
                    "structured_content_complete":true
                }))
                .unwrap_or_else(|_| "{}".to_owned())
            } else if name == "render_evidence_replay_get" {
                serde_json::to_string(&json!({
                    "schema_version":"RenderEvidenceReplayMcpSummary@1",
                    "candidate_id":value.get("candidate_id"),
                    "artifact_sha256":value.get("artifact_sha256"),
                    "camera_hash":value.get("camera_hash"),
                    "source_render_set_object_sha256":value.get("source_render_set_object_sha256"),
                    "replay_status":value.get("replay_status"),
                    "determinism_claim":value.get("determinism_claim"),
                    "worker_cohort_binding":value.get("worker_cohort_binding"),
                    "runtime_write_performed":false,
                    "canonical_sha256":value.get("canonical_sha256"),
                    "structured_content_complete":true
                }))
                .unwrap_or_else(|_| "{}".to_owned())
            } else if name == "boolean_operand_lineage_preview" {
                serde_json::to_string(&json!({
                    "schema_version":"BooleanOperandLineageMcpSummary@1",
                    "program_sha256":value.get("program_sha256"),
                    "boolean_node_id":value.get("boolean_node_id"),
                    "operation":value.get("operation"),
                    "output_triangle_count":value.get("output_triangle_count"),
                    "lineage_run_count":value.get("lineage_run_count"),
                    "lineage_sha256":value.get("lineage_sha256"),
                    "canonical_sha256":value.get("canonical_sha256"),
                    "structured_content_complete":true
                }))
                .unwrap_or_else(|_| "{}".to_owned())
            } else if name == "subdivision_topology_lineage_preview" {
                serde_json::to_string(&json!({
                    "schema_version":"SubdivisionTopologyLineageMcpSummary@1",
                    "program_sha256":value.get("program_sha256"),
                    "subdivision_node_id":value.get("subdivision_node_id"),
                    "lineage_kind":value.get("lineage_kind"),
                    "lineage_element_count":value.get("lineage_element_count"),
                    "lineage_sha256":value.get("lineage_sha256"),
                    "canonical_sha256":value.get("canonical_sha256"),
                    "structured_content_complete":true
                }))
                .unwrap_or_else(|_| "{}".to_owned())
            } else if name == "subdivision_artifact_lineage_get" {
                serde_json::to_string(&json!({
                    "schema_version":"SubdivisionArtifactLineageMcpSummary@1",
                    "project_id":value.get("project_id"),
                    "candidate_id":value.get("candidate_id"),
                    "artifact_id":value.get("artifact_id"),
                    "subdivision_node_id":value.get("subdivision_node_id"),
                    "part_id":value.get("part_id"),
                    "max_lineage_elements":value.get("max_lineage_elements"),
                    "lineage_element_count":value.get("lineage_element_count"),
                    "lineage_sha256":value.get("lineage_sha256"),
                    "artifact_binding_sha256":value.get("artifact_binding_sha256"),
                    "canonical_sha256":value.get("canonical_sha256"),
                    "structured_content_complete":true
                }))
                .unwrap_or_else(|_| "{}".to_owned())
            } else if matches!(
                name,
                "subdivision_artifact_lineage_sidecar_get" | "subdivision_artifact_lineage_prepare"
            ) {
                let is_prepare = name == "subdivision_artifact_lineage_prepare";
                serde_json::to_string(&json!({
                    "schema_version":"SubdivisionArtifactLineageMcpSummary@1",
                    "operation":name,
                    "write_intent":if is_prepare {"explicit_runtime_prepare_write"} else {"read_only_sidecar_lookup"},
                    "runtime_write_performed":is_prepare,
                    "text":if is_prepare {"Runtime explicitly writes the Runtime-owned Subdivision lineage sidecar Link."} else {"Read-only sidecar Link lookup; this call performs no write."},
                    "structured_content_complete":true
                }))
                .unwrap_or_else(|_| "{}".to_owned())
            } else {
                serde_json::to_string(&value).unwrap_or_else(|_| "{}".to_owned())
            };
            let response = json!({
                "jsonrpc":"2.0",
                "id":id,
                "result":{"content":[{"type":"text","text":content_text}],"structuredContent":value}
            });
            Some(apply_read_model_mcp_wire_budget(name, response))
        }
        Err(error) => Some(json!({
            "jsonrpc":"2.0",
            "id":id,
            "result":{"isError":true,"content":[{"type":"text","text":serde_json::to_string(&runtime_error_value(&error)).unwrap_or_else(|_| "{}".to_owned())}],"structuredContent":runtime_error_value(&error)}
        })),
    }
}

fn apply_read_model_mcp_wire_budget(name: &str, response: Value) -> Value {
    let bounded = matches!(
        name,
        "geometry_prepare"
            | "topology_snapshot_get"
            | "authoring_topology_get"
            | "authoring_mesh_edit_preview"
            | "authoring_mesh_edit_prepare"
            | "mechanical_pose_evaluate"
            | "mechanical_pose_geometry_preview"
            | "mechanical_animation_clip_prepare"
            | "mechanical_animation_clip_get"
            | "mechanical_animation_clip_preview_get"
            | "mechanical_animation_clip_v2_prepare"
            | "mechanical_animation_clip_v2_get"
            | "mechanical_animation_clip_v2_preview"
            | "mechanical_animation_glb_v2_prepare"
            | "mechanical_animation_glb_v2_get"
            | "mechanical_animation_glb_prepare"
            | "game_asset_delivery_prepare"
            | "game_asset_delivery_get"
            | "game_asset_lod_derive"
            | "appearance_source_lineage_prepare"
            | "appearance_source_lineage_get"
            | "candidate_material_surface_quality_prepare"
            | "candidate_material_surface_quality_get"
            | "candidate_animation_vfx_quality_prepare"
            | "candidate_animation_vfx_quality_get"
            | "candidate_animation_vfx_quality_v2_prepare"
            | "candidate_animation_vfx_quality_v2_get"
            | "production_stage_transition_v2_prepare"
            | "production_stage_transition_v2_get"
            | "game_weapon_anchor_prepare"
            | "game_weapon_anchor_get"
            | "game_weapon_glb_socket_prepare"
            | "game_weapon_glb_socket_get"
            | "game_weapon_animated_glb_socket_prepare"
            | "game_weapon_animated_glb_socket_get"
            | "game_weapon_animated_glb_socket_v2_prepare"
            | "game_weapon_animated_glb_socket_v2_get"
            | "fictional_energy_vfx_animated_socket_attachment_prepare"
            | "fictional_energy_vfx_animated_socket_attachment_get"
            | "fictional_energy_vfx_animated_socket_attachment_v2_prepare"
            | "fictional_energy_vfx_animated_socket_attachment_v2_get"
            | "fictional_energy_vfx_animated_socket_attachment_v3_prepare"
            | "fictional_energy_vfx_animated_socket_attachment_v3_get"
            | "game_weapon_animated_glb_socket_transform_projection_prepare"
            | "game_weapon_animated_glb_socket_transform_projection_get"
            | "game_weapon_animated_glb_socket_transform_projection_v2_prepare"
            | "game_weapon_animated_glb_socket_transform_projection_v2_get"
            | "fictional_energy_vfx_animated_socket_particles_sequence_prepare"
            | "fictional_energy_vfx_animated_socket_particles_sequence_get"
            | "fictional_energy_vfx_animated_socket_particles_sequence_v2_prepare"
            | "fictional_energy_vfx_animated_socket_particles_sequence_v2_get"
            | "fictional_energy_vfx_animated_socket_trails_sequence_prepare"
            | "fictional_energy_vfx_animated_socket_trails_sequence_get"
            | "fictional_energy_vfx_animated_socket_trails_sequence_v2_prepare"
            | "fictional_energy_vfx_animated_socket_trails_sequence_v2_get"
            | "fictional_energy_vfx_animated_socket_trails_bloom_sequence_prepare"
            | "fictional_energy_vfx_animated_socket_trails_bloom_sequence_get"
            | "fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_prepare"
            | "fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_get"
            | "fictional_energy_vfx_prepare"
            | "fictional_energy_vfx_get"
            | "fictional_energy_vfx_frame_sample"
            | "fictional_energy_vfx_appearance_frame_sample"
            | "fictional_energy_vfx_rendered_frame_prepare"
            | "fictional_energy_vfx_rendered_frame_get"
            | "fictional_energy_vfx_rendered_sequence_prepare"
            | "fictional_energy_vfx_rendered_sequence_get"
            | "fictional_energy_vfx_hdr_bloom_prepare"
            | "fictional_energy_vfx_hdr_bloom_get"
            | "fictional_energy_vfx_particles_prepare"
            | "fictional_energy_vfx_particles_get"
            | "fictional_energy_vfx_trails_prepare"
            | "fictional_energy_vfx_trails_get"
            | "fictional_energy_vfx_trails_bloom_prepare"
            | "fictional_energy_vfx_trails_bloom_get"
            | "render_evidence_integrity_get"
            | "render_evidence_replay_get"
            | "boolean_operand_lineage_preview"
            | "subdivision_topology_lineage_preview"
            | "subdivision_artifact_lineage_get"
            | "subdivision_artifact_lineage_sidecar_get"
            | "subdivision_artifact_lineage_prepare"
            | "geometry_program_hash"
    );
    if !bounded {
        return response;
    }
    let exceeds_budget = serde_json::to_vec(&response)
        .map(|bytes| bytes.len() > READ_MODEL_MCP_WIRE_MAX_BYTES)
        .unwrap_or(true);
    if !exceeds_budget {
        return response;
    }
    let error = runtime_error_value(
        "MCP_READ_MODEL_RESPONSE_BUDGET_EXCEEDED: serialized tools/call response exceeds 1 MiB",
    );
    json!({
        "jsonrpc":"2.0",
        "id":response["id"].clone(),
        "result":{"isError":true,"content":[{"type":"text","text":serde_json::to_string(&error).unwrap_or_else(|_| "{}".to_owned())}],"structuredContent":error}
    })
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
        if optimization_tools::is_write_tool(name) {
            return Err(
                "OPTIMIZATION_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required"
                    .to_owned(),
            );
        }
        if agentic_action_tools::is_write_tool(name) {
            return Err(
                "AGENTIC_ACTION_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required"
                    .to_owned(),
            );
        }
        if agentic_orchestrator_tools::is_write_tool(name) {
            return Err(
                "AGENTIC_ORCHESTRATOR_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required"
                    .to_owned(),
            );
        }
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
        } else if name == "subdivision_artifact_lineage_prepare" {
            "MCP010F_SUBDIVISION_ARTIFACT_LINEAGE_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required"
                .to_owned()
        } else if name == "mechanical_animation_clip_prepare" {
            "MCP010F_MECHANICAL_ANIMATION_CLIP_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required"
                .to_owned()
        } else if name == "mechanical_animation_glb_prepare" {
            "MCP010F_MECHANICAL_ANIMATION_GLB_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required"
                .to_owned()
        } else if name == "game_asset_delivery_prepare" {
            "MCP010F_GAME_ASSET_DELIVERY_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required"
                .to_owned()
        } else if name == "appearance_source_lineage_prepare" {
            "MCP010F_APPEARANCE_SOURCE_LINEAGE_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required"
                .to_owned()
        } else if name == "game_weapon_anchor_prepare" {
            "MCP010F_GAME_WEAPON_ANCHOR_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required"
                .to_owned()
        } else if name == "game_weapon_animated_glb_socket_prepare" {
            "MCP010F_GAME_WEAPON_ANIMATED_GLB_SOCKET_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required"
                .to_owned()
        } else if name == "authoring_mesh_edit_prepare" {
            "MCP010F_AUTHORING_MESH_EDIT_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required"
                .to_owned()
        } else if is_mcp010f_write_tool(name) {
            "MCP010F_PRIMARY_FORM_TOOLS_DISABLED: explicit authenticated IPC opt-in is required"
                .to_owned()
        } else {
            "MCP004_WRITE_TOOLS_DISABLED: explicit authenticated IPC opt-in is required".to_owned()
        });
    }
    if is_write_tool(name) {
        if optimization_tools::is_write_tool(name) {
            let arguments = if name == "optimization_job_prepare" {
                canonicalize_optimization_job_wire(arguments)?
            } else {
                arguments.clone()
            };
            return backend_write_call(backend, name, &arguments, local_build_cohort);
        }
        if agentic_orchestrator_tools::is_write_tool(name) {
            return backend_write_call(backend, name, arguments, local_build_cohort);
        }
        if agentic_action_tools::is_write_tool(name) {
            return backend_agentic_action_call(backend, name, arguments, local_build_cohort);
        }
        if cross_view_promotion_tools::is_write_tool(name) {
            return backend_write_call(backend, name, arguments, local_build_cohort);
        }
        if agentic_write_tools::is_write_tool(name) {
            return backend_agentic_write_call(backend, name, arguments, local_build_cohort);
        }
        return backend_write_call(backend, name, arguments, local_build_cohort);
    }
    if agentic_write_tools::is_tool(name) {
        return match agentic_write_tools::runtime_method(name) {
            Some(runtime_method) => backend_call(backend, runtime_method, arguments),
            None => Err(agentic_write_tools::unavailable_error(name)),
        };
    }
    if agentic_action_tools::is_tool(name) {
        return match agentic_action_tools::runtime_method(name) {
            Some(runtime_method) => backend_call(backend, runtime_method, arguments),
            None => Err(agentic_action_tools::unavailable_error(name)),
        };
    }
    if optimization_tools::is_tool(name) {
        return match optimization_tools::runtime_method(name) {
            Some(runtime_method) => backend_call(backend, runtime_method, arguments),
            None => Err(optimization_tools::unavailable_error(name)),
        };
    }
    if cross_view_promotion_tools::is_tool(name) {
        return match cross_view_promotion_tools::runtime_method(name) {
            Some(runtime_method) => backend_call(backend, runtime_method, arguments),
            None => Err(cross_view_promotion_tools::unavailable_error(name)),
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
            return agentic_write_tools::unavailable_error(name);
        }
        if error.starts_with("RUNTIME_UNAVAILABLE:") {
            return format!("AGENTIC_RUNTIME_UNAVAILABLE: {error}");
        }
        error
    })
}

fn backend_agentic_action_call(
    backend: &mut Backend,
    name: &str,
    arguments: &Value,
    local_build_cohort: Option<&str>,
) -> Result<Value, String> {
    backend_write_call(backend, name, arguments, local_build_cohort).map_err(|error| {
        if error == "RUNTIME_UNAVAILABLE: Runtime request failed" {
            return agentic_action_tools::unavailable_error(name);
        }
        if error.starts_with("RUNTIME_UNAVAILABLE:") {
            return format!("AGENTIC_ACTION_RUNTIME_UNAVAILABLE: {error}");
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
                "GEOMETRY_PROGRAM_HASH_REJECTED" => {
                    let reason = detail
                        .split_once("GEOMETRY_PROGRAM_HASH_REJECTED:")
                        .map(|(_, value)| value.trim())
                        .filter(|value| !value.is_empty())
                        .map(|value| {
                            [
                                "request shape",
                                "schema or project binding",
                                "GEOMETRY_WORKER_PROTOCOL",
                                "GEOMETRY_WORKER_REJECTED",
                            ]
                            .into_iter()
                            .find(|candidate| value.starts_with(candidate))
                            .unwrap_or("request shape or worker contract validation")
                        })
                        .unwrap_or("request shape or worker contract validation");
                    format!(
                        "GEOMETRY_PROGRAM_HASH_REJECTED: Runtime geometry hash request rejected ({reason})"
                    )
                }
                "INVALID_INPUT" => {
                    // Keep the adapter free of user payloads and local paths,
                    // but preserve the stable stage code when Runtime has
                    // one.  Codex can then repair a fit envelope (or stop on
                    // a rejected quality gate) without guessing from the
                    // generic INVALID_INPUT bucket.
                    let stage = detail
                        .split(':')
                        .map(str::trim)
                        .find(|value| {
                            value.starts_with("AGENTIC_")
                                || value.starts_with("DESIGN_ACTION_")
                                || value.starts_with("DESIGN_STAGE_")
                                || value.starts_with("DESIGN_COMPOSITION_")
                                || value.starts_with("REPAIR_")
                                || value.starts_with("GEOMETRY_PROGRAM_HASH_REJECTED")
                                || value.starts_with("PRIMARY_FORM_REPAIR_")
                                || value.starts_with("SILHOUETTE_")
                                || value.starts_with("CAMERA_")
                                || value.starts_with("APPEARANCE_")
                                || value.starts_with("RENDER_REJECTED")
                                || value.starts_with("CONTRACT_OUTPUT_INVALID")
                                || value.starts_with("OPTIMIZATION_")
                        })
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
                        _ if stage.starts_with("DESIGN_ACTION_") || stage.starts_with("DESIGN_STAGE_") || stage.starts_with("DESIGN_COMPOSITION_") || stage.starts_with("REPAIR_") => {
                            let code = stage
                                .split_whitespace()
                                .next()
                                .unwrap_or("DESIGN_ORCHESTRATOR_RUNTIME_REJECTED");
                            if matches!(
                                code,
                                "DESIGN_ACTION_INPUT_HASH_MISMATCH"
                                    | "DESIGN_STAGE_INPUT_HASH_MISMATCH"
                                    | "DESIGN_COMPOSITION_INPUT_HASH_MISMATCH"
                            ) {
                                let detail = detail
                                    .split_once(&format!("{code}:"))
                                    .map(|(_, value)| value.trim())
                                    .unwrap_or("");
                                let safe_hash_detail = detail
                                    .split_whitespace()
                                    .filter(|part| {
                                        let Some((key, value)) = part.split_once('=') else {
                                            return false;
                                        };
                                        matches!(key, "expected" | "numeric_compatibility" | "actual")
                                            && value.len() == 64
                                            && value.bytes().all(|byte| byte.is_ascii_hexdigit())
                                    })
                                    .collect::<Vec<_>>();
                                if safe_hash_detail.len() >= 2 {
                                    return format!(
                                        "{code}: Runtime design request rejected ({})",
                                        safe_hash_detail.join(" ")
                                    );
                                }
                            }
                            format!("{code}: Runtime design request rejected")
                        }
                        _ if stage.starts_with("ACTION_")
                            || stage.starts_with("GEOMETRY_")
                            || stage.starts_with("REFERENCE_")
                            || stage.starts_with("VISUAL_")
                            || stage.starts_with("QUALITY_")
                            || stage.starts_with("ARTIFACT_")
                            || stage.starts_with("PROJECT_SCOPE_")
                            || stage == "NOT_FOUND" => {
                            let code = stage
                                .split_whitespace()
                                .next()
                                .unwrap_or("DESIGN_RUNTIME_REJECTED");
                            format!("{code}: Runtime design request rejected")
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
                        _ if stage.starts_with("PRIMARY_FORM_REPAIR_") => {
                            let code = stage
                                .split_whitespace()
                                .next()
                                .unwrap_or("PRIMARY_FORM_REPAIR_REJECTED");
                            format!(
                                "{code}: Runtime Primary Form request rejected"
                            )
                        }
                        _ if stage.starts_with("SILHOUETTE_OBJECTIVE_") => {
                            let code = stage
                                .split_whitespace()
                                .next()
                                .unwrap_or("SILHOUETTE_OBJECTIVE_REJECTED");
                            format!(
                                "{code}: Runtime silhouette evaluation objective request rejected"
                            )
                        }
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
                        "CAMERA_FIT_INVALID" => "CAMERA_FIT_INVALID: Runtime camera-fit request shape was rejected".to_owned(),
                        "CAMERA_FIT_REJECTED" => "CAMERA_FIT_REJECTED: Runtime camera-fit readback gate rejected the candidate".to_owned(),
                        "CAMERA_FIT_RENDER_FAILED" => "CAMERA_FIT_RENDER_FAILED: Runtime camera-fit render failed".to_owned(),
                        "CAMERA_FIT_UNAVAILABLE" => "CAMERA_FIT_UNAVAILABLE: Runtime camera-fit did not produce a candidate".to_owned(),
                        "APPEARANCE_V2_REFERENCE_REQUIRED" => "APPEARANCE_V2_REFERENCE_REQUIRED: Runtime appearance preparation requires a project-bound reference".to_owned(),
                        "APPEARANCE_REJECTED" => {
                            let reason = detail
                                .split_once("APPEARANCE_REJECTED:")
                                .map(|(_, value)| value.trim())
                                .filter(|value| !value.is_empty())
                                .unwrap_or("appearance contract or GLB readback validation failed");
                            let reason = [
                                "physical GLB readback failed",
                                "appearance program must be an object",
                                "appearance schema_version",
                                "appearance project_id",
                                "appearance geometry_program_sha256",
                                "appearance is not bound",
                                "appearance canonical_sha256",
                                "appearance material_zones",
                                "AppearanceProgram@2",
                            ]
                            .into_iter()
                            .find(|candidate| reason.starts_with(candidate))
                            .unwrap_or("appearance contract or GLB readback validation failed");
                            format!("APPEARANCE_REJECTED: Runtime appearance stage rejected ({reason})")
                        }
                        "RENDER_REJECTED" => "RENDER_REJECTED: Runtime render stage rejected the artifact".to_owned(),
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
                        _ if stage.starts_with("OPTIMIZATION_") => {
                            format!("{stage}: Runtime optimization request rejected")
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
                "RUNTIME_BUSY" => "RUNTIME_BUSY: Runtime writer is busy".to_owned(),
                "IPC_ERROR" => "IPC_ERROR: Runtime IPC request failed".to_owned(),
                _ if code.starts_with("PRIMARY_FORM_REPAIR_") => {
                    format!("{code}: Runtime Primary Form request rejected")
                }
                _ if code.starts_with("SILHOUETTE_OBJECTIVE_") => {
                    format!("{code}: Runtime silhouette evaluation objective request rejected")
                }
                _ if code.starts_with("SILHOUETTE_PART_ERROR_") => {
                    format!("{code}: Runtime Part contour evidence request rejected")
                }
                _ if code.starts_with("OPTIMIZATION_") => {
                    format!("{code}: Runtime optimization request rejected")
                }
                // Geometry compile/readback failures are typed Runtime
                // rejections, not transport outages. Preserve the bounded
                // machine-readable family so Codex can correct one profile
                // or Part instead of repeatedly restarting a healthy Runtime.
                _ if code.starts_with("GEOMETRY_") => {
                    format!("{code}: Runtime geometry request rejected")
                }
                // Runtime emits stable REFERENCE_* codes for authorization,
                // attachment, image inspection and project-binding failures.
                // Preserve only the machine-readable family across IPC; the
                // detail is deliberately not forwarded because it may contain
                // user input or local paths.  These are request failures, not
                // transport outages, so runtime_error_value must not mark
                // them retryable as RUNTIME_UNAVAILABLE.
                _ if code.starts_with("REFERENCE_") => {
                    format!("{code}: Runtime reference request rejected")
                }
                // The same distinction is needed when CAS/SQLite validation
                // rejects an import.  Keep the code bounded and path-free so
                // a live package can report the real storage boundary without
                // weakening the Runtime/CAS ownership model.
                _ if code.starts_with("STORE_") => {
                    format!("{code}: Runtime store rejected the request")
                }
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
        "design_stage_run_prepare" => runtime
            .design_stage_run_prepare(arguments.clone())
            .map_err(|error| error.to_string()),
        "design_composition_prepare" => runtime
            .design_composition_prepare(arguments.clone())
            .map_err(|error| error.to_string()),
        "cross_view_promotion_confirm" => runtime
            .cross_view_promotion_confirm(arguments.clone())
            .map_err(|error| error.to_string()),
        "optimization_job_get" | "optimization_job_prepare" | "optimization_job_resume" => {
            match name {
                "optimization_job_get" => runtime
                    .optimization_job_get(arguments.clone())
                    .map_err(|error| error.to_string()),
                "optimization_job_prepare" => runtime
                    .optimization_job_prepare(arguments.clone())
                    .map_err(|error| error.to_string()),
                "optimization_job_resume" => runtime
                    .optimization_job_resume(arguments.clone())
                    .map_err(|error| error.to_string()),
                _ => unreachable!("OptimizationJob dispatch arm is exhaustive"),
            }
        }
        "design_action_run_get"
        | "design_action_run_prepare"
        | "design_action_optimization_proposal_prepare"
        | "repair_intent_run_prepare"
        | "repair_apply_prepare"
        | "repair_apply_confirm" => match name {
            "design_action_run_get" => runtime
                .design_action_run_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "design_action_run_prepare" => runtime
                .design_action_run_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "design_action_optimization_proposal_prepare" => runtime
                .design_action_optimization_proposal_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "repair_intent_run_prepare" => runtime
                .repair_intent_run_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "repair_apply_prepare" => runtime
                .repair_apply_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "repair_apply_confirm" => runtime
                .repair_apply_confirm(arguments.clone())
                .map_err(|error| error.to_string()),
            _ => unreachable!("DesignActionRun dispatch arm is exhaustive"),
        },
        "session_create_or_resume"
        | "session_get"
        | "checkpoint_prepare"
        | "checkpoint_get"
        | "checkpoint_restore_prepare"
        | "production_stage_transition_prepare"
        | "production_stage_transition_get"
        | "production_stage_transition_v2_prepare"
        | "production_stage_transition_v2_get"
        | "candidate_topology_quality_prepare"
        | "candidate_topology_quality_get"
        | "candidate_material_surface_quality_prepare"
        | "candidate_material_surface_quality_get"
        | "candidate_animation_vfx_quality_prepare"
        | "candidate_animation_vfx_quality_get"
        | "candidate_animation_vfx_quality_v2_prepare"
        | "candidate_animation_vfx_quality_v2_get"
        | "mechanical_animation_clip_v2_prepare"
        | "mechanical_animation_clip_v2_get"
        | "mechanical_animation_clip_v2_preview"
        | "mechanical_animation_glb_v2_prepare"
        | "mechanical_animation_glb_v2_get"
        | "game_weapon_animated_glb_socket_v2_prepare"
        | "game_weapon_animated_glb_socket_v2_get"
        | "fictional_energy_vfx_animated_socket_attachment_prepare"
        | "fictional_energy_vfx_animated_socket_attachment_get"
        | "fictional_energy_vfx_animated_socket_attachment_v2_prepare"
        | "fictional_energy_vfx_animated_socket_attachment_v2_get"
        | "fictional_energy_vfx_animated_socket_attachment_v3_prepare"
        | "fictional_energy_vfx_animated_socket_attachment_v3_get"
        | "game_weapon_animated_glb_socket_transform_projection_prepare"
        | "game_weapon_animated_glb_socket_transform_projection_get"
        | "game_weapon_animated_glb_socket_transform_projection_v2_prepare"
        | "game_weapon_animated_glb_socket_transform_projection_v2_get"
        | "fictional_energy_vfx_animated_socket_particles_sequence_prepare"
        | "fictional_energy_vfx_animated_socket_particles_sequence_get"
        | "fictional_energy_vfx_animated_socket_particles_sequence_v2_prepare"
        | "fictional_energy_vfx_animated_socket_particles_sequence_v2_get"
        | "fictional_energy_vfx_animated_socket_trails_sequence_prepare"
        | "fictional_energy_vfx_animated_socket_trails_sequence_get"
        | "fictional_energy_vfx_animated_socket_trails_sequence_v2_prepare"
        | "fictional_energy_vfx_animated_socket_trails_sequence_v2_get"
        | "fictional_energy_vfx_animated_socket_trails_bloom_sequence_prepare"
        | "fictional_energy_vfx_animated_socket_trails_bloom_sequence_get"
        | "fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_prepare"
        | "fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_get" => match name {
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
            "production_stage_transition_prepare" => runtime
                .production_stage_transition_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_stage_transition_get" => runtime
                .production_stage_transition_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_stage_transition_v2_prepare" => runtime
                .production_stage_transition_v2_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_stage_transition_v2_get" => runtime
                .production_stage_transition_v2_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "candidate_topology_quality_prepare" => runtime
                .candidate_topology_quality_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "candidate_topology_quality_get" => runtime
                .candidate_topology_quality_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "candidate_material_surface_quality_prepare" => runtime
                .candidate_material_surface_quality_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "candidate_material_surface_quality_get" => runtime
                .candidate_material_surface_quality_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "candidate_animation_vfx_quality_prepare" => runtime
                .candidate_animation_vfx_quality_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "candidate_animation_vfx_quality_get" => runtime
                .candidate_animation_vfx_quality_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "candidate_animation_vfx_quality_v2_prepare" => runtime
                .candidate_animation_vfx_quality_v2_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "candidate_animation_vfx_quality_v2_get" => runtime
                .candidate_animation_vfx_quality_v2_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "mechanical_animation_clip_v2_prepare" => runtime
                .mechanical_animation_clip_v2_prepare(arguments)
                .map_err(|error| error.to_string()),
            "mechanical_animation_clip_v2_get" => runtime
                .mechanical_animation_clip_v2_get(arguments)
                .map_err(|error| error.to_string()),
            "mechanical_animation_clip_v2_preview" => runtime
                .mechanical_animation_clip_v2_preview_get(arguments)
                .map_err(|error| error.to_string()),
            "mechanical_animation_glb_v2_prepare" => runtime
                .mechanical_animation_glb_v2_prepare(arguments)
                .map_err(|error| error.to_string()),
            "mechanical_animation_glb_v2_get" => runtime
                .mechanical_animation_glb_v2_get(arguments)
                .map_err(|error| error.to_string()),
            "game_weapon_animated_glb_socket_v2_prepare" => runtime
                .game_weapon_animated_glb_socket_v2_prepare(arguments)
                .map_err(|error| error.to_string()),
            "game_weapon_animated_glb_socket_v2_get" => runtime
                .game_weapon_animated_glb_socket_v2_get(arguments)
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_attachment_prepare" => runtime
                .fictional_energy_vfx_animated_socket_attachment_prepare(&arguments.clone())
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_attachment_get" => runtime
                .fictional_energy_vfx_animated_socket_attachment_get(&arguments.clone())
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_attachment_v2_prepare" => runtime
                .fictional_energy_vfx_animated_socket_attachment_v2_prepare(&arguments.clone())
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_attachment_v2_get" => runtime
                .fictional_energy_vfx_animated_socket_attachment_v2_get(&arguments.clone())
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_attachment_v3_prepare" => runtime
                .fictional_energy_vfx_animated_socket_attachment_v3_prepare(&arguments.clone())
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_attachment_v3_get" => runtime
                .fictional_energy_vfx_animated_socket_attachment_v3_get(&arguments.clone())
                .map_err(|error| error.to_string()),
            "game_weapon_animated_glb_socket_transform_projection_prepare" => runtime
                .game_weapon_animated_glb_socket_transform_projection_prepare(&arguments.clone())
                .map_err(|error| error.to_string()),
            "game_weapon_animated_glb_socket_transform_projection_get" => runtime
                .game_weapon_animated_glb_socket_transform_projection_get(&arguments.clone())
                .map_err(|error| error.to_string()),
            "game_weapon_animated_glb_socket_transform_projection_v2_prepare" => runtime
                .game_weapon_animated_glb_socket_transform_projection_v2_prepare(&arguments.clone())
                .map_err(|error| error.to_string()),
            "game_weapon_animated_glb_socket_transform_projection_v2_get" => runtime
                .game_weapon_animated_glb_socket_transform_projection_v2_get(&arguments.clone())
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_particles_sequence_prepare" => runtime
                .fictional_energy_vfx_animated_socket_particles_sequence_prepare(&arguments)
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_particles_sequence_get" => runtime
                .fictional_energy_vfx_animated_socket_particles_sequence_get(&arguments)
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_particles_sequence_v2_prepare" => runtime
                .fictional_energy_vfx_animated_socket_particles_sequence_v2_prepare(&arguments)
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_particles_sequence_v2_get" => runtime
                .fictional_energy_vfx_animated_socket_particles_sequence_v2_get(&arguments)
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_trails_sequence_prepare" => runtime
                .fictional_energy_vfx_animated_socket_trails_sequence_prepare(&arguments)
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_trails_sequence_get" => runtime
                .fictional_energy_vfx_animated_socket_trails_sequence_get(&arguments)
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_trails_sequence_v2_prepare" => runtime
                .fictional_energy_vfx_animated_socket_trails_sequence_v2_prepare(&arguments)
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_trails_sequence_v2_get" => runtime
                .fictional_energy_vfx_animated_socket_trails_sequence_v2_get(&arguments)
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_trails_bloom_sequence_prepare" => runtime
                .fictional_energy_vfx_animated_socket_trails_bloom_sequence_prepare(&arguments)
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_trails_bloom_sequence_get" => runtime
                .fictional_energy_vfx_animated_socket_trails_bloom_sequence_get(&arguments)
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_prepare" => runtime
                .fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_prepare(&arguments)
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_get" => runtime
                .fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_get(&arguments)
                .map_err(|error| error.to_string()),
            _ => unreachable!("agentic write tool dispatch arm is exhaustive"),
        },
        "capabilities_get" => {
            serde_json::to_value(runtime.capabilities()).map_err(|error| error.to_string())
        }
        "operator_catalog_get" => Ok(runtime.active_operator_catalog()),
        "material_pack_get" => runtime
            .material_pack_get(arguments)
            .map_err(|error| error.to_string()),
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
            let observation_sha256 = required_sha256(arguments, "observation_sha256")?;
            runtime
                .agentic_stage_plan_bound(
                    project_id,
                    arguments.get("candidate_id").and_then(Value::as_str),
                    observation_sha256,
                )
                .map_err(|error| error.to_string())
        }
        "agentic_critic_projection" => {
            let project_id = required_id(arguments, "project_id")?;
            let observation_sha256 = required_sha256(arguments, "observation_sha256")?;
            runtime
                .agentic_critic_projection_bound(
                    project_id,
                    arguments.get("candidate_id").and_then(Value::as_str),
                    observation_sha256,
                )
                .map_err(|error| error.to_string())
        }
        "agentic_visual_evidence_bundle" => {
            let project_id = required_id(arguments, "project_id")?;
            let candidate_id = required_id(arguments, "candidate_id")?;
            let observation_sha256 = required_sha256(arguments, "observation_sha256")?;
            runtime
                .agentic_visual_evidence_bundle_bound(project_id, candidate_id, observation_sha256)
                .map_err(|error| error.to_string())
        }
        "visual_surface_get" => runtime
            .visual_surface_get(arguments.clone())
            .map_err(|error| error.to_string()),
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
        "render_evidence_integrity_get" => runtime
            .render_evidence_integrity_get(arguments.clone())
            .map_err(|error| error.to_string()),
        "render_evidence_replay_get" => runtime
            .render_evidence_replay_get(arguments.clone())
            .map_err(|error| error.to_string()),
        "boolean_operand_lineage_preview" => runtime
            .boolean_operand_lineage_preview(arguments.clone())
            .map_err(|error| error.to_string()),
        "subdivision_topology_lineage_preview" => runtime
            .subdivision_topology_lineage_preview(arguments.clone())
            .map_err(|error| error.to_string()),
        "subdivision_artifact_lineage_get" => runtime
            .subdivision_artifact_lineage_get(arguments.clone())
            .map_err(|error| error.to_string()),
        "subdivision_artifact_lineage_sidecar_get" => runtime
            .subdivision_artifact_lineage_sidecar_get(arguments.clone())
            .map_err(|error| error.to_string()),
        "subdivision_artifact_lineage_prepare" => runtime
            .subdivision_artifact_lineage_prepare(arguments.clone())
            .map_err(|error| error.to_string()),
        "mechanical_animation_clip_prepare" => runtime
            .mechanical_animation_clip_prepare(arguments)
            .map_err(|error| error.to_string()),
        "mechanical_animation_clip_get" => runtime
            .mechanical_animation_clip_get(arguments)
            .map_err(|error| error.to_string()),
        "mechanical_animation_clip_preview_get" => runtime
            .mechanical_animation_clip_preview_get(arguments)
            .map_err(|error| error.to_string()),
        "mechanical_animation_glb_prepare" => runtime
            .mechanical_animation_glb_prepare(arguments)
            .map_err(|error| error.to_string()),
        "game_asset_delivery_prepare" => runtime
            .game_asset_delivery_prepare(arguments)
            .map_err(|error| error.to_string()),
        "game_asset_delivery_get" => runtime
            .game_asset_delivery_get(arguments)
            .map_err(|error| error.to_string()),
        "game_asset_lod_derive" => runtime
            .game_asset_lod_derive(arguments)
            .map_err(|error| error.to_string()),
        "appearance_source_lineage_prepare" => runtime
            .appearance_source_lineage_prepare(arguments)
            .map_err(|error| error.to_string()),
        "appearance_source_lineage_get" => runtime
            .appearance_source_lineage_get(arguments)
            .map_err(|error| error.to_string()),
        "game_weapon_anchor_prepare" => runtime
            .game_weapon_anchor_prepare(arguments)
            .map_err(|error| error.to_string()),
        "game_weapon_anchor_get" => runtime
            .game_weapon_anchor_get(arguments)
            .map_err(|error| error.to_string()),
        "game_weapon_glb_socket_prepare" => runtime
            .game_weapon_glb_socket_prepare(arguments)
            .map_err(|error| error.to_string()),
        "game_weapon_glb_socket_get" => runtime
            .game_weapon_glb_socket_get(arguments)
            .map_err(|error| error.to_string()),
        "game_weapon_animated_glb_socket_prepare" => runtime
            .game_weapon_animated_glb_socket_prepare(arguments)
            .map_err(|error| error.to_string()),
        "game_weapon_animated_glb_socket_get" => runtime
            .game_weapon_animated_glb_socket_get(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_prepare" => runtime
            .fictional_energy_vfx_prepare(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_get" => runtime
            .fictional_energy_vfx_get(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_frame_sample" => runtime
            .fictional_energy_vfx_frame_sample(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_appearance_frame_sample" => runtime
            .fictional_energy_vfx_appearance_frame_sample(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_rendered_frame_prepare" => runtime
            .fictional_energy_vfx_rendered_frame_prepare(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_rendered_frame_get" => runtime
            .fictional_energy_vfx_rendered_frame_get(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_rendered_sequence_prepare" => runtime
            .fictional_energy_vfx_rendered_sequence_prepare(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_rendered_sequence_get" => runtime
            .fictional_energy_vfx_rendered_sequence_get(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_hdr_bloom_prepare" => runtime
            .fictional_energy_vfx_hdr_bloom_prepare(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_hdr_bloom_get" => runtime
            .fictional_energy_vfx_hdr_bloom_get(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_particles_prepare" => runtime
            .fictional_energy_vfx_particles_prepare(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_particles_get" => runtime
            .fictional_energy_vfx_particles_get(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_trails_prepare" => runtime
            .fictional_energy_vfx_trails_prepare(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_trails_get" => runtime
            .fictional_energy_vfx_trails_get(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_trails_bloom_prepare" => runtime
            .fictional_energy_vfx_trails_bloom_prepare(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_trails_bloom_get" => runtime
            .fictional_energy_vfx_trails_bloom_get(arguments)
            .map_err(|error| error.to_string()),
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
            let request = arguments
                .get("request")
                .cloned()
                .unwrap_or_else(|| json!({}));
            if let Some(idempotency_value) = arguments.get("idempotency_key") {
                let idempotency_key = idempotency_value
                    .as_str()
                    .ok_or_else(|| "idempotency_key must be a non-null identifier".to_owned())?;
                let base_version_id = match arguments.get("base_version_id") {
                    Some(Value::Null) => None,
                    Some(Value::String(value)) => Some(value.as_str()),
                    Some(_) => {
                        return Err("base_version_id must be an identifier or null".to_owned())
                    }
                    None => {
                        return Err("HEAD_BINDING_REQUIRED: exact geometry prepare requires an explicit base_version_id field".to_owned())
                    }
                };
                runtime
                    .prepare_geometry_candidate_exact(
                        project_id,
                        base_version_id,
                        idempotency_key,
                        request,
                    )
                    .map_err(|error| error.to_string())
            } else {
                let base_version_id = arguments.get("base_version_id").and_then(Value::as_str);
                runtime
                    .prepare_geometry_candidate(project_id, base_version_id, request)
                    .map_err(|error| error.to_string())
            }
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
        "silhouette_evaluation_objective_prepare" => {
            let project_id = required_id(arguments, "project_id")?;
            runtime
                .silhouette_evaluation_objective_prepare(project_id, arguments.clone())
                .map_err(|error| error.to_string())
        }
        "silhouette_objective_compare" => {
            let project_id = required_id(arguments, "project_id")?;
            runtime
                .silhouette_objective_compare(project_id, arguments.clone())
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
        "topology_snapshot_get" => {
            let project_id = required_id(arguments, "project_id")?;
            let artifact_id = required_sha256(arguments, "artifact_id")?;
            let candidate_id = required_id(arguments, "candidate_id")?;
            let part_id = required_id(arguments, "part_id")?;
            let artifact_readback_sha256 = required_sha256(arguments, "artifact_readback_sha256")?;
            let program_sha256 = required_sha256(arguments, "program_sha256")?;
            let operator_catalog_sha256 = required_sha256(arguments, "operator_catalog_sha256")?;
            let readback_config_sha256 = required_sha256(arguments, "readback_config_sha256")?;
            let snapshot_policy_sha256 = required_sha256(arguments, "snapshot_policy_sha256")?;
            let max_face_count = arguments
                .get("max_face_count")
                .and_then(Value::as_u64)
                .ok_or_else(|| "max_face_count is required".to_owned())?;
            runtime
                .topology_snapshot(
                    project_id,
                    artifact_id,
                    candidate_id,
                    part_id,
                    artifact_readback_sha256,
                    program_sha256,
                    operator_catalog_sha256,
                    readback_config_sha256,
                    snapshot_policy_sha256,
                    max_face_count,
                )
                .map_err(|error| error.to_string())
        }
        "authoring_topology_get" => runtime
            .authoring_topology(arguments)
            .map_err(|error| error.to_string()),
        "authoring_mesh_edit_preview" => runtime
            .authoring_mesh_edit_preview(arguments)
            .map_err(|error| error.to_string()),
        "authoring_mesh_edit_prepare" => runtime
            .authoring_mesh_edit_prepare(arguments)
            .map_err(|error| error.to_string()),
        "mechanical_pose_evaluate" => runtime
            .mechanical_pose_evaluate(arguments)
            .map_err(|error| error.to_string()),
        "mechanical_pose_geometry_preview" => runtime
            .mechanical_pose_geometry_preview(arguments)
            .map_err(|error| error.to_string()),
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
        "job_result_get" => {
            let id = required_id(arguments, "job_id")?;
            runtime.job_result(id).map_err(|error| error.to_string())
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

/// Rebind the nested CameraCalibration after a JSON client round-trip.
///
/// Rust and Python both preserve the same IEEE-754 value, but they can emit a
/// different shortest decimal spelling for that value.  A camera produced by
/// Runtime can therefore arrive with a stale floating-point payload even
/// though its typed identity is unchanged.  The outer intent hash is checked
/// against the exact wire payload first; then a complete calibration is
/// reduced to the two Runtime-owned identity hashes. Runtime resolves that
/// compact reference from its candidate/target cache before validating the
/// authoritative calibration, all typed intent fields, CAS bindings, and the
/// residual source hashes. No caller-rounded camera floats enter geometry
/// truth.
fn canonicalize_optimization_job_wire(arguments: &Value) -> Result<Value, String> {
    let object = arguments
        .as_object()
        .ok_or_else(|| "INVALID_INPUT: optimization job arguments must be an object".to_owned())?;
    let intent = object
        .get("intent")
        .and_then(Value::as_object)
        .ok_or_else(|| "INVALID_INPUT: optimization intent must be an object".to_owned())?;
    let supplied_intent_hash = intent
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "INVALID_INPUT: optimization intent canonical_sha256 is required".to_owned()
        })?;

    let mut wire_intent = Value::Object(intent.clone());
    wire_intent["canonical_sha256"] = Value::String(String::new());
    let wire_intent_hash = canonical_json_hash(&wire_intent);

    // A Codex JSON round-trip can spell continuous numbers differently (most
    // commonly `1.0` as `1`).  Accept the same deterministic numeric
    // restoration used by silhouette_fit_prepare. The exact wire hash is still
    // preferred and arbitrary hashes remain rejected; this only closes the
    // typed JSON transport gap without minting a new camera identity.
    let mut normalized_intent = Value::Object(intent.clone());
    if intent.get("schema_version").and_then(Value::as_str) == Some("OptimizationIntent@2") {
        if let Some(camera_rig) = normalized_intent.get("camera_rig").cloned() {
            let camera_rig = rebind_camera_rig_v2_wire(&camera_rig)?;
            let rig_hash = camera_rig
                .get("canonical_sha256")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    "INVALID_INPUT: optimization camera rig canonical_sha256 is required".to_owned()
                })?
                .to_owned();
            normalized_intent["camera_rig_sha256"] = Value::String(rig_hash);
            normalized_intent["camera_rig"] = camera_rig;
        }
        if let Some(views) = normalized_intent.get("views").cloned() {
            let views = views
                .as_array()
                .ok_or_else(|| {
                    "INVALID_INPUT: optimization intent views must be an array".to_owned()
                })?
                .iter()
                .map(|view| {
                    let mut view = view.as_object().cloned().ok_or_else(|| {
                        "INVALID_INPUT: optimization intent view must be an object".to_owned()
                    })?;
                    let camera = view.get("camera").cloned().ok_or_else(|| {
                        "INVALID_INPUT: optimization intent view camera is required".to_owned()
                    })?;
                    let camera = rebind_camera_v2_wire(&camera)?;
                    let camera_hash = camera.get("camera_hash").cloned().ok_or_else(|| {
                        "INVALID_INPUT: optimization intent view camera_hash is required".to_owned()
                    })?;
                    view.insert("camera".to_owned(), camera);
                    view.insert("camera_hash".to_owned(), camera_hash);
                    Ok(Value::Object(view))
                })
                .collect::<Result<Vec<_>, String>>()?;
            normalized_intent["views"] = Value::Array(views);
        }
        if let Some(rig) = normalized_intent.get("rig").cloned() {
            normalized_intent["rig"] = rebind_canonical_object(&rig, true)?;
        }
        if let Some(objective) = normalized_intent.get("objective").cloned() {
            normalized_intent["objective"] = normalize_continuous_numbers(&objective, false);
        }
        let mut normalized_hash_input = normalized_intent.clone();
        normalized_hash_input["canonical_sha256"] = Value::String(String::new());
        let normalized_intent_hash = canonical_json_hash(&normalized_hash_input);
        if supplied_intent_hash != wire_intent_hash
            && supplied_intent_hash != normalized_intent_hash
        {
            return Err(
                "OPTIMIZATION_INTENT_INVALID: canonical_sha256 does not bind the wire payload"
                    .to_owned(),
            );
        }
        normalized_intent["canonical_sha256"] = Value::String(normalized_intent_hash);
        let mut rebound_arguments = arguments.clone();
        rebound_arguments["intent"] = normalized_intent;
        return Ok(rebound_arguments);
    }
    if let Some(camera) = normalized_intent.get("camera").cloned() {
        if camera.get("schema_version").and_then(Value::as_str) == Some("CameraCalibration@1") {
            normalized_intent["camera"] = json!({
                "schema_version":"CameraCalibrationRef@1",
                "camera_hash":camera.get("camera_hash").cloned().unwrap_or(Value::Null),
                "canonical_sha256":camera.get("canonical_sha256").cloned().unwrap_or(Value::Null),
            });
        } else {
            normalized_intent["camera"] = normalize_continuous_numbers(&camera, true);
        }
    }
    if let Some(rig) = normalized_intent.get("rig").cloned() {
        normalized_intent["rig"] = normalize_continuous_numbers(&rig, false);
    }
    if let Some(objective) = normalized_intent.get("objective").cloned() {
        normalized_intent["objective"] = normalize_continuous_numbers(&objective, false);
    }
    let mut normalized_hash_input = normalized_intent.clone();
    normalized_hash_input["canonical_sha256"] = Value::String(String::new());
    let normalized_intent_hash = canonical_json_hash(&normalized_hash_input);
    if supplied_intent_hash != wire_intent_hash && supplied_intent_hash != normalized_intent_hash {
        return Err(
            "OPTIMIZATION_INTENT_INVALID: canonical_sha256 does not bind the wire payload"
                .to_owned(),
        );
    }

    let mut rebound_rig = normalized_intent
        .get("rig")
        .cloned()
        .ok_or_else(|| "INVALID_INPUT: optimization intent rig is required".to_owned())?;
    {
        let rig_object = rebound_rig
            .as_object_mut()
            .ok_or_else(|| "INVALID_INPUT: optimization intent rig must be an object".to_owned())?;
        rig_object.insert("canonical_sha256".to_owned(), Value::String(String::new()));
    }
    let rig_canonical_hash = canonical_json_hash(&rebound_rig);
    rebound_rig["canonical_sha256"] = Value::String(rig_canonical_hash);

    let mut rebound_intent = normalized_intent;
    rebound_intent["rig"] = rebound_rig;
    rebound_intent["canonical_sha256"] = Value::String(String::new());
    let rebound_intent_hash = canonical_json_hash(&rebound_intent);
    rebound_intent["canonical_sha256"] = Value::String(rebound_intent_hash);

    let mut rebound_arguments = arguments.clone();
    rebound_arguments["intent"] = rebound_intent;
    Ok(rebound_arguments)
}

fn rebind_camera_v2_wire(value: &Value) -> Result<Value, String> {
    let mut camera = normalize_continuous_numbers(value, true);
    if camera.get("schema_version").and_then(Value::as_str) != Some("CameraCalibration@2") {
        return Err("INVALID_INPUT: optimization V2 camera schema_version is required".to_owned());
    }
    camera["camera_hash"] = Value::String(String::new());
    camera["canonical_sha256"] = Value::String(String::new());
    let camera_hash = canonical_json_hash(&camera);
    camera["camera_hash"] = Value::String(camera_hash);
    camera["canonical_sha256"] = Value::String(String::new());
    let canonical_hash = canonical_json_hash(&camera);
    camera["canonical_sha256"] = Value::String(canonical_hash);
    Ok(camera)
}

fn rebind_camera_rig_v2_wire(value: &Value) -> Result<Value, String> {
    let mut rig = normalize_continuous_numbers(value, true);
    if let Some(views) = rig.get("views").cloned() {
        let views = views
            .as_array()
            .ok_or_else(|| {
                "INVALID_INPUT: optimization camera rig views must be an array".to_owned()
            })?
            .iter()
            .map(|view| {
                let mut view = view.as_object().cloned().ok_or_else(|| {
                    "INVALID_INPUT: optimization camera rig view must be an object".to_owned()
                })?;
                let camera = view.get("camera").cloned().ok_or_else(|| {
                    "INVALID_INPUT: optimization camera rig view camera is required".to_owned()
                })?;
                let camera = rebind_camera_v2_wire(&camera)?;
                let camera_hash = camera.get("camera_hash").cloned().ok_or_else(|| {
                    "INVALID_INPUT: optimization camera rig view camera_hash is required".to_owned()
                })?;
                view.insert("camera".to_owned(), camera);
                view.insert("camera_hash".to_owned(), camera_hash);
                Ok(Value::Object(view))
            })
            .collect::<Result<Vec<_>, String>>()?;
        rig["views"] = Value::Array(views);
    }
    rig["canonical_sha256"] = Value::String(String::new());
    let canonical_hash = canonical_json_hash(&rig);
    rig["canonical_sha256"] = Value::String(canonical_hash);
    Ok(rig)
}

fn rebind_canonical_object(value: &Value, preserve_resolution: bool) -> Result<Value, String> {
    let mut object = normalize_continuous_numbers(value, preserve_resolution);
    if !object.is_object() {
        return Err("INVALID_INPUT: optimization hashed object must be an object".to_owned());
    }
    object["canonical_sha256"] = Value::String(String::new());
    let canonical_hash = canonical_json_hash(&object);
    object["canonical_sha256"] = Value::String(canonical_hash);
    Ok(object)
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
                    // These fields are discrete contract values, not
                    // continuous optimizer leaves.  Converting a surface
                    // control-point index through f64 changes `0` into a
                    // JSON float and makes the Runtime Rig validator reject
                    // the otherwise valid OptimizationIntent.
                    let normalized = if (preserve_resolution && key == "resolution")
                        || key == "control_point_index"
                    {
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
    fn material_pack_get_exposes_only_the_closed_offline_pack_ids() {
        let tool = tools_with_writes(false)
            .into_iter()
            .find(|tool| tool["name"] == "material_pack_get")
            .expect("material_pack_get tool");
        assert_eq!(tool["annotations"]["readOnlyHint"], true);
        assert_eq!(tool["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            tool["inputSchema"]["properties"]["pack_id"]["enum"],
            json!([
                "forgecad-hard-surface-robot",
                "forgecad-fictional-energy-weapon",
                "forgecad-fictional-energy-weapon-2k"
            ])
        );

        let runtime = Runtime::ephemeral().expect("runtime");
        let weapon = dispatch_in_process(
            &runtime,
            "material_pack_get",
            &json!({"pack_id":"forgecad-fictional-energy-weapon"}),
        )
        .expect("weapon MaterialPack dispatch");
        assert_eq!(weapon["pack_id"], "forgecad-fictional-energy-weapon");
        let weapon_2k = dispatch_in_process(
            &runtime,
            "material_pack_get",
            &json!({"pack_id":"forgecad-fictional-energy-weapon-2k"}),
        )
        .expect("weapon 2K pack call");
        assert_eq!(weapon_2k["pack_id"], "forgecad-fictional-energy-weapon-2k");
        assert_eq!(
            weapon["canonical_sha256"],
            "4a56fa58af1e8a0cd218f880f61112d465725a79eb70ed2aa0076eb5408ac999"
        );
        assert!(dispatch_in_process(
            &runtime,
            "material_pack_get",
            &json!({"path":"/tmp/material-pack"}),
        )
        .is_err());
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
        assert!(
            validate_value_against_tool_schema(&schema, &arguments, 0, &mut value_budget).is_ok()
        );
    }

    #[test]
    fn primary_form_repair_schema_accepts_exclusive_step_fraction() {
        let schema = tools_with_writes(true)
            .into_iter()
            .find(|tool| tool["name"] == "primary_form_repair_prepare")
            .expect("primary_form_repair_prepare tool")["inputSchema"]
            .clone();
        let arguments = json!({
            "project_id":"project-primary-form",
            "candidate_id":"candidate-primary-form",
            "target_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "part_id":"chest-shell",
            "rig":{},
            "base_camera":{},
            "optimizer":{
                "algorithm":"coordinate_descent",
                "max_iterations":2,
                "max_evaluations":16,
                "step_fraction":0.1
            },
            "base_version_id":null,
            "canonical_sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        });
        let mut schema_budget = ToolSchemaValidationBudget::new();
        assert!(validate_tool_schema_shape(&schema, 0, &mut schema_budget).is_ok());
        let mut value_budget = ToolSchemaValidationBudget::new();
        assert!(
            validate_value_against_tool_schema(&schema, &arguments, 0, &mut value_budget).is_ok()
        );

        let mut zero_step = arguments.clone();
        zero_step["optimizer"]["step_fraction"] = Value::from(0.0);
        let mut value_budget = ToolSchemaValidationBudget::new();
        assert!(
            validate_value_against_tool_schema(&schema, &zero_step, 0, &mut value_budget).is_err()
        );
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
        assert_eq!(
            initialize_response["result"]["protocolVersion"],
            MCP_PROTOCOL_VERSION
        );

        let blocked = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"operator_catalog_get","arguments":{}}}),
        )
        .expect("preflight block");
        assert_eq!(blocked["result"]["isError"], true);
        assert_eq!(
            blocked["result"]["structuredContent"]["code"],
            "PONYTAIL_PREFLIGHT_REQUIRED"
        );

        let preflight = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"skill_get","arguments":{"skill_id":"ponytail-preflight","version":"0.1.0"}}}),
        )
        .expect("preflight skill");
        assert_eq!(
            preflight["result"]["structuredContent"]["skill"]["skill_id"],
            "ponytail-preflight"
        );
        assert!(
            preflight["result"]["structuredContent"]["knowledge"]["overview"]
                .as_str()
                .expect("preflight overview")
                .contains("Ponytail preflight")
        );

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
        assert_eq!(
            map_ipc_error(IpcError::RuntimeRequest(
                "PRIMARY_FORM_REPAIR_INVALID: canonical_sha256 does not bind intent".to_owned(),
            )),
            "PRIMARY_FORM_REPAIR_INVALID: Runtime Primary Form request rejected"
        );
        assert_eq!(
            map_ipc_error(IpcError::RuntimeRequest(
                "GEOMETRY_PROGRAM_HASH_REJECTED: GEOMETRY_WORKER_REJECTED".to_owned(),
            )),
            "GEOMETRY_PROGRAM_HASH_REJECTED: Runtime geometry hash request rejected (GEOMETRY_WORKER_REJECTED)"
        );
        assert_eq!(
            map_ipc_error(IpcError::RuntimeRequest("GEOMETRY_REJECTED".to_owned(),)),
            "GEOMETRY_REJECTED: Runtime geometry request rejected"
        );
        assert_eq!(
            map_ipc_error(IpcError::RuntimeRequest(
                "SILHOUETTE_OBJECTIVE_TARGET_LINEAGE_MISMATCH".to_owned(),
            )),
            "SILHOUETTE_OBJECTIVE_TARGET_LINEAGE_MISMATCH: Runtime silhouette evaluation objective request rejected"
        );
        assert_eq!(
            map_ipc_error(IpcError::RuntimeRequest(
                "OPTIMIZATION_INTENT_INVALID".to_owned(),
            )),
            "OPTIMIZATION_INTENT_INVALID: Runtime optimization request rejected"
        );
        assert_eq!(
            map_ipc_error(IpcError::RuntimeRequest(
                "REFERENCE_TRANSFER_UNAVAILABLE: attachment could not be read".to_owned(),
            )),
            "REFERENCE_TRANSFER_UNAVAILABLE: Runtime reference request rejected"
        );
        assert_eq!(
            map_ipc_error(IpcError::RuntimeRequest("STORE_CAS_IO".to_owned())),
            "STORE_CAS_IO: Runtime store rejected the request"
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
        assert_eq!(summary["read_count"], 90);
        assert_eq!(summary["write_count"], 69);
        assert_eq!(summary["total_count"], 159);
        assert_eq!(summary["read_names"].as_array().unwrap().len(), 90);
        assert_eq!(summary["write_names"].as_array().unwrap().len(), 69);
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
    fn mechanical_animation_glb_v2_mcp_surface_is_closed_and_opt_in() {
        let read_tools = tools_with_writes(false);
        let get = read_tools
            .iter()
            .find(|tool| tool["name"] == "mechanical_animation_glb_v2_get")
            .expect("appearance-aware animated GLB get tool");
        assert_eq!(get["annotations"]["readOnlyHint"], true);
        assert_eq!(get["annotations"]["writeIntent"], false);
        assert_eq!(get["inputSchema"]["additionalProperties"], false);
        assert!(!read_tools
            .iter()
            .any(|tool| tool["name"] == "mechanical_animation_glb_v2_prepare"));

        let enabled = tools_with_writes(true);
        let prepare = enabled
            .iter()
            .find(|tool| tool["name"] == "mechanical_animation_glb_v2_prepare")
            .expect("appearance-aware animated GLB prepare tool");
        assert_eq!(prepare["annotations"]["readOnlyHint"], false);
        assert_eq!(prepare["annotations"]["writeIntent"], true);
        assert_eq!(prepare["annotations"]["approvalRequired"], false);
        assert_eq!(prepare["_meta"]["forgecad"]["requiresConfirmation"], false);
        assert_eq!(prepare["inputSchema"]["additionalProperties"], false);
        let hash = "a".repeat(64);
        let request = json!({
            "schema_version":"MechanicalAnimationGlbPrepareRequest@2",
            "project_id":"project-1",
            "appearance_candidate_id":"appearance-1",
            "appearance_candidate_state_sha256":hash,
            "clip_id":"clip-1",
            "clip_object_sha256":"b".repeat(64),
            "clip_sha256":"c".repeat(64),
            "materialization_policy":"appearance-aware-rigid-node-trs-gltf-linear-scheduled-samples@2",
            "input_sha256":"d".repeat(64),
            "idempotency_key":"animation-glb-key-1"
        });
        assert!(validate_declared_tool_input(
            "mechanical_animation_glb_v2_prepare",
            &request,
            true
        )
        .is_ok());
        let mut unknown = request.clone();
        unknown["script"] = json!("bpy.ops");
        assert!(validate_declared_tool_input(
            "mechanical_animation_glb_v2_prepare",
            &unknown,
            true
        )
        .is_err());
        let get_request = json!({
            "schema_version":"MechanicalAnimationGlbGetRequest@2",
            "project_id":"project-1",
            "appearance_candidate_id":"appearance-1",
            "clip_id":"clip-1"
        });
        assert!(validate_declared_tool_input(
            "mechanical_animation_glb_v2_get",
            &get_request,
            false
        )
        .is_ok());
        assert_eq!(
            agentic_write_tools::runtime_method("mechanical_animation_glb_v2_prepare"),
            Some("mechanical_animation_glb_v2_prepare")
        );
        assert_eq!(
            agentic_write_tools::runtime_method("mechanical_animation_glb_v2_get"),
            Some("mechanical_animation_glb_v2_get")
        );
    }

    #[test]
    fn animated_socket_materialization_v2_mcp_surface_is_closed_hidden_and_dispatches() {
        let prepare_name = "game_weapon_animated_glb_socket_v2_prepare";
        let get_name = "game_weapon_animated_glb_socket_v2_get";
        let read_tools = tools_with_writes(false);
        assert!(read_tools.iter().any(|tool| tool["name"] == get_name));
        assert!(!read_tools.iter().any(|tool| tool["name"] == prepare_name));

        let enabled_tools = tools_with_writes(true);
        let prepare_tool = enabled_tools
            .iter()
            .find(|tool| tool["name"] == prepare_name)
            .expect("V2 animated socket materialization prepare tool");
        let get_tool = read_tools
            .iter()
            .find(|tool| tool["name"] == get_name)
            .expect("V2 animated socket materialization get tool");
        assert_eq!(prepare_tool["annotations"]["readOnlyHint"], false);
        assert_eq!(prepare_tool["annotations"]["writeIntent"], true);
        assert_eq!(prepare_tool["annotations"]["approvalRequired"], false);
        assert_eq!(
            prepare_tool["_meta"]["forgecad"]["requiresConfirmation"],
            false
        );
        assert_eq!(prepare_tool["inputSchema"]["additionalProperties"], false);
        assert_eq!(get_tool["annotations"]["readOnlyHint"], true);
        assert_eq!(get_tool["annotations"]["writeIntent"], false);
        assert_eq!(get_tool["annotations"]["approvalRequired"], false);
        assert_eq!(get_tool["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            prepare_tool["inputSchema"]["properties"]["materialization_policy"]["const"],
            "appearance-aware-animation-v2-socket-node-materialization-preserve-renderable-content@2"
        );
        assert_eq!(
            get_tool["inputSchema"]["required"],
            json!([
                "schema_version",
                "project_id",
                "appearance_candidate_id",
                "clip_id",
                "animated_socket_materialization_key_sha256"
            ])
        );

        let get_arguments = json!({
            "schema_version":"GameWeaponAnimatedGlbSocketMaterializationGetRequest@2",
            "project_id":"project-1",
            "appearance_candidate_id":"appearance-1",
            "clip_id":"clip-1",
            "animated_socket_materialization_key_sha256":"a".repeat(64)
        });
        assert!(validate_declared_tool_input(get_name, &get_arguments, false).is_ok());
        let prepare_arguments = json!({
            "schema_version":"GameWeaponAnimatedGlbSocketMaterializationPrepareRequest@2",
            "project_id":"project-1",
            "appearance_candidate_id":"appearance-1",
            "appearance_candidate_state_sha256":"a".repeat(64),
            "clip_id":"clip-1",
            "clip_object_sha256":"b".repeat(64),
            "clip_sha256":"c".repeat(64),
            "appearance_delivery_manifest_object_sha256":"d".repeat(64),
            "anchor_set_object_sha256":"e".repeat(64),
            "anchor_set_canonical_sha256":"f".repeat(64),
            "materialization_policy":"appearance-aware-animation-v2-socket-node-materialization-preserve-renderable-content@2",
            "input_sha256":"0".repeat(64),
            "idempotency_key":"socket-v2-prepare-1"
        });
        assert!(validate_declared_tool_input(prepare_name, &prepare_arguments, true).is_ok());
        for field in [
            "unknown",
            "raw_glb_bytes",
            "base64",
            "path",
            "url",
            "script",
        ] {
            let mut invalid = prepare_arguments.clone();
            invalid[field] = json!("not-allowed");
            assert!(
                validate_declared_tool_input(prepare_name, &invalid, true).is_err(),
                "closed V2 animated socket prepare schema accepted {field}"
            );
        }
        for field in [
            "unknown",
            "raw_glb_bytes",
            "base64",
            "path",
            "url",
            "script",
        ] {
            let mut invalid = get_arguments.clone();
            invalid[field] = json!("not-allowed");
            assert!(
                validate_declared_tool_input(get_name, &invalid, false).is_err(),
                "closed V2 animated socket get schema accepted {field}"
            );
        }
        let appearance_binding = agentic_write_tools::Binding {
            session_id: Some("session-1".to_owned()),
            project_id: Some("project-1".to_owned()),
            candidate_id: Some("appearance-1".to_owned()),
        };
        assert!(agentic_write_tools::validate_call(
            prepare_name,
            &prepare_arguments,
            &appearance_binding
        )
        .is_ok());
        assert!(agentic_write_tools::validate_call(
            prepare_name,
            &prepare_arguments,
            &agentic_write_tools::Binding::default()
        )
        .is_err());
        let mut cross_candidate = prepare_arguments.clone();
        cross_candidate["appearance_candidate_id"] = json!("appearance-other");
        assert!(agentic_write_tools::validate_call(
            prepare_name,
            &cross_candidate,
            &appearance_binding
        )
        .is_err());
        assert_eq!(
            agentic_write_tools::runtime_method(prepare_name),
            Some(prepare_name)
        );
        assert_eq!(
            agentic_write_tools::runtime_method(get_name),
            Some(get_name)
        );

        let runtime = Runtime::ephemeral().expect("V2 animated socket dispatch runtime");
        for (name, arguments) in [(get_name, get_arguments), (prepare_name, prepare_arguments)] {
            let error = dispatch_in_process(&runtime, name, &arguments)
                .expect_err("V2 animated socket Runtime dispatch must reach the named method");
            assert!(
                !error.starts_with("CAPABILITY_UNAVAILABLE:"),
                "{name} fell through the MCP Runtime dispatch table: {error}"
            );
        }
    }

    #[test]
    fn v2_production_stage_get_schema_rejects_unknown_fields_and_preserves_v1() {
        let read_tools = tools_with_writes(false);
        let get = read_tools
            .iter()
            .find(|tool| tool["name"] == "production_stage_transition_v2_get")
            .expect("V2 production-stage get tool");
        let request = json!({
            "schema_version":"ProductionStageTransitionGetRequest@2",
            "transition_id":"transition-v2-1",
            "session_id":"session-1",
            "project_id":"project-1",
            "root_candidate_id":"candidate-1",
            "head_candidate_id":"candidate-material-1"
        });
        assert!(validate_declared_tool_input(
            "production_stage_transition_v2_get",
            &request,
            false
        )
        .is_ok());
        let mut unknown = request;
        unknown["unexpected"] = json!("forbidden");
        assert!(validate_declared_tool_input(
            "production_stage_transition_v2_get",
            &unknown,
            false
        )
        .is_err());
        assert!(read_tools
            .iter()
            .any(|tool| tool["name"] == "production_stage_transition_get"));
        assert_eq!(
            agentic_write_tools::runtime_method("production_stage_transition_get"),
            Some("production_stage_transition_get")
        );
        assert_eq!(
            agentic_write_tools::runtime_method("production_stage_transition_v2_get"),
            Some("production_stage_transition_v2_get")
        );
        assert_eq!(get["annotations"]["readOnlyHint"], true);
    }

    #[test]
    fn v2_production_stage_prepare_rejects_iso_expiry_and_path_like_ids() {
        let prepare = tools_with_writes(true)
            .into_iter()
            .find(|tool| tool["name"] == "production_stage_transition_v2_prepare")
            .expect("V2 production-stage prepare tool");
        let schema = &prepare["inputSchema"];
        let mut request = Map::new();
        for required in schema["required"].as_array().expect("required fields") {
            let field = required.as_str().expect("field name");
            let property = &schema["properties"][field];
            let value = if let Some(constant) = property.get("const") {
                constant.clone()
            } else if field == "approval_expires_at" {
                json!("1700000000")
            } else if field == "approval_summary" {
                json!("promote passed topology to material surface")
            } else if field == "approved" {
                json!(true)
            } else if field.ends_with("_sha256") || field == "camera_hash" {
                json!("a".repeat(64))
            } else {
                json!("id-1")
            };
            request.insert(field.to_owned(), value);
        }
        let request = Value::Object(request);
        assert!(validate_declared_tool_input(
            "production_stage_transition_v2_prepare",
            &request,
            true
        )
        .is_ok());

        let mut iso_expiry = request.clone();
        iso_expiry["approval_expires_at"] = json!("2026-08-21T23:59:59Z");
        assert!(validate_declared_tool_input(
            "production_stage_transition_v2_prepare",
            &iso_expiry,
            true
        )
        .is_err());

        for invalid_id in ["candidate with space", "candidate/child"] {
            let mut invalid = request.clone();
            invalid["root_candidate_id"] = json!(invalid_id);
            assert!(validate_declared_tool_input(
                "production_stage_transition_v2_prepare",
                &invalid,
                true
            )
            .is_err());
        }
    }

    #[test]
    fn reference_mask_tools_advertise_explicit_user_confirmation() {
        let tools = tools_with_writes(true);
        for name in ["reference_mask_prepare", "reference_mask_refine_prepare"] {
            let tool = tools
                .iter()
                .find(|tool| tool["name"] == name)
                .expect("reference mask tool");
            assert_eq!(tool["inputSchema"]["additionalProperties"], false);
            assert_eq!(
                tool["inputSchema"]["properties"]["user_confirmed"]["type"],
                "boolean"
            );
            assert!(
                tool["inputSchema"]["properties"]["user_confirmed"]["description"]
                    .as_str()
                    .unwrap()
                    .contains("Explicit user confirmation")
            );
            let structure = &tool["inputSchema"]["properties"]["visual_structure"];
            assert_eq!(structure["additionalProperties"], false);
            assert_eq!(structure["properties"]["regions"]["minItems"], 1);
            assert!(
                structure["properties"]["regions"]["items"]["properties"]["visual_role"]["enum"]
                    .as_array()
                    .unwrap()
                    .contains(&Value::String("outer-flowing-shell".to_owned()))
            );
            assert!(structure["properties"]["regions"]["items"]["properties"]
                ["boundary_relationship"]["enum"]
                .as_array()
                .unwrap()
                .contains(&Value::String("overlap".to_owned())));
        }
    }

    #[test]
    fn geometry_program_hash_is_a_default_read_only_tool() {
        let tools = tools_with_writes(false);
        let tool = tools
            .iter()
            .find(|tool| tool["name"] == "geometry_program_hash")
            .expect("geometry_program_hash tool");
        assert_eq!(tool["annotations"]["readOnlyHint"], true);
        let branches = tool["inputSchema"]["oneOf"]
            .as_array()
            .expect("geometry_program_hash has direct and PDK branches");
        assert!(branches.iter().any(|branch| {
            branch["properties"]["schema_version"]["const"] == "GeometryProgramHashRequest@1"
        }));
        assert!(branches.iter().any(|branch| {
            branch["properties"]["schema_version"]["const"] == "ParametricDesignKitRequest@1"
        }));
        assert!(branches.iter().any(|branch| {
            branch["oneOf"].as_array().is_some_and(|variants| {
                variants.iter().all(|variant| {
                    variant["properties"]["schema_version"]["const"]
                        == "ParametricDesignKitRequest@2"
                })
            })
        }));
        assert!(branches.iter().any(|branch| {
            branch["properties"]["schema_version"]["const"] == "GeometryModifierStackRequest@1"
        }));
        assert!(branches.iter().any(|branch| {
            branch["properties"]["schema_version"]["const"] == "GeometryModifierEvaluationRequest@2"
        }));
        assert!(branches.iter().any(|branch| {
            branch["properties"]["schema_version"]["const"] == "SubdivisionEvaluationRequest@2"
        }));
        assert!(branches.iter().any(|branch| {
            branch["properties"]["schema_version"]["const"]
                == "SubdivisionCreaseEvaluationRequest@1"
        }));
        assert_eq!(
            tool["inputSchema"]["additionalProperties"], false,
            "the public envelope must remain closed"
        );
    }

    #[test]
    fn boolean_operand_lineage_round_trips_as_a_read_only_mcp_tool() {
        let tool = tools_with_writes(false)
            .into_iter()
            .find(|tool| tool["name"] == "boolean_operand_lineage_preview")
            .expect("Boolean lineage tool");
        assert_eq!(tool["annotations"]["readOnlyHint"], true);
        assert_eq!(tool["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            tool["inputSchema"]["properties"]["max_lineage_runs"]["maximum"],
            4096
        );

        let (mut backend, mut session) = initialized();
        let (project_id, mut program, before) = match &backend {
            Backend::InProcess(runtime) => {
                let project = runtime
                    .create_project("Boolean lineage MCP", json!({"profile":"mvp"}))
                    .expect("project");
                let catalog_sha256 = runtime.active_operator_catalog()["canonical_sha256"]
                    .as_str()
                    .expect("catalog hash")
                    .to_owned();
                let program = json!({
                    "schema_version":"GeometryProgram@2",
                    "project_id":project.project_id,
                    "representation_plan_sha256":"4".repeat(64),
                    "operator_catalog_sha256":catalog_sha256,
                    "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
                    "budgets":{"max_nodes":8,"max_triangles":10000,"max_glb_bytes":67108864,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
                    "nodes":[
                        {"node_id":"left","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[1.0,1.0,1.0],"position_m":[-0.25,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                        {"node_id":"right","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[1.0,1.0,1.0],"position_m":[0.25,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                        {"node_id":"boolean","operator_id":"forgecad.geometry.boolean@1","inputs":["left","right"],"parameters":{"shape":"intersection"}}
                    ],
                    "part_outputs":[{"part_id":"boolean-part","input_node_ids":["boolean"],"material_zone_id":"zone-mechanical","solid":true}]
                });
                let before = json!({
                    "candidates":runtime.candidates(&project.project_id).expect("candidates"),
                    "versions":runtime.versions(Some(&project.project_id)).expect("versions")
                });
                (project.project_id, program, before)
            }
            _ => unreachable!("test backend"),
        };
        program["canonical_sha256"] = Value::String(canonical_json_hash(&program));
        let mut arguments = json!({
            "schema_version":"BooleanOperandLineageRequest@1",
            "geometry_program":program,
            "boolean_node_id":"boolean",
            "max_lineage_runs":4096,
            "canonical_sha256":""
        });
        arguments["canonical_sha256"] = Value::String(canonical_json_hash(&arguments));
        let response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":302,"method":"tools/call","params":{"name":"boolean_operand_lineage_preview","arguments":arguments.clone()}}),
        )
        .expect("Boolean lineage response");
        assert_eq!(
            response["result"]["structuredContent"]["schema_version"], "BooleanOperandLineage@1",
            "unexpected Boolean lineage response: {response}"
        );
        assert_eq!(
            response["result"]["structuredContent"]["runtime_write_performed"],
            false
        );
        assert_eq!(
            response["result"]["structuredContent"]["operation"],
            "intersection"
        );
        assert_eq!(
            serde_json::from_str::<Value>(
                response["result"]["content"][0]["text"]
                    .as_str()
                    .expect("Boolean summary text")
            )
            .expect("Boolean summary JSON")["schema_version"],
            "BooleanOperandLineageMcpSummary@1"
        );
        assert!(serde_json::to_vec(&response).unwrap().len() <= READ_MODEL_MCP_WIRE_MAX_BYTES);

        let after = match &backend {
            Backend::InProcess(runtime) => json!({
                "candidates":runtime.candidates(&project_id).expect("candidates"),
                "versions":runtime.versions(Some(&project_id)).expect("versions")
            }),
            _ => unreachable!("test backend"),
        };
        assert_eq!(
            before, after,
            "MCP lineage read must not create durable state"
        );

        for (id, invalid_max) in [(303, 0), (304, 4097)] {
            let mut invalid = arguments.clone();
            invalid["max_lineage_runs"] = Value::from(invalid_max);
            let rejected = handle(
                &mut backend,
                &mut session,
                &json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"boolean_operand_lineage_preview","arguments":invalid}}),
            )
            .expect("invalid Boolean run budget response");
            assert_eq!(rejected["error"]["data"]["code"], "INVALID_TOOL_PARAMS");
        }

        let mut nested_unknown = arguments.clone();
        nested_unknown["geometry_program"]["nodes"][0]["unexpected"] = Value::Bool(true);
        let rejected = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":305,"method":"tools/call","params":{"name":"boolean_operand_lineage_preview","arguments":nested_unknown}}),
        )
        .expect("nested invalid Boolean program response");
        assert_eq!(rejected["result"]["isError"], true);

        arguments["unexpected"] = Value::Bool(true);
        let rejected = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":306,"method":"tools/call","params":{"name":"boolean_operand_lineage_preview","arguments":arguments}}),
        )
        .expect("invalid Boolean lineage response");
        assert_eq!(rejected["error"]["data"]["code"], "INVALID_TOOL_PARAMS");
    }

    #[test]
    fn subdivision_topology_lineage_round_trips_as_a_bounded_read_only_mcp_tool() {
        let tool = tools_with_writes(false)
            .into_iter()
            .find(|tool| tool["name"] == "subdivision_topology_lineage_preview")
            .expect("subdivision topology lineage tool");
        assert_eq!(tool["annotations"]["readOnlyHint"], true);
        assert_eq!(tool["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            tool["inputSchema"]["properties"]["max_lineage_elements"]["maximum"],
            25_000
        );

        let (mut backend, mut session) = initialized();
        let (project_id, mut program, before) = match &backend {
            Backend::InProcess(runtime) => {
                let project = runtime
                    .create_project("Subdivision lineage MCP", json!({"profile":"mvp"}))
                    .expect("project");
                let catalog_sha256 = runtime.active_operator_catalog()["canonical_sha256"]
                    .as_str()
                    .expect("catalog hash")
                    .to_owned();
                let program = json!({
                    "schema_version":"GeometryProgram@2",
                    "project_id":project.project_id,
                    "representation_plan_sha256":"8".repeat(64),
                    "operator_catalog_sha256":catalog_sha256,
                    "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
                    "budgets":{"max_nodes":4,"max_triangles":128,"max_glb_bytes":67108864,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
                    "nodes":[{
                        "node_id":"cage","operator_id":"forgecad.geometry.subd-cage@2","inputs":[],
                        "parameters":{
                            "shape":"subd-cage",
                            "control_points":[[-1.0,-1.0,0.0],[0.0,-1.0,0.0],[1.0,-1.0,0.0],[-1.0,0.0,0.0],[0.0,0.0,1.0],[1.0,0.0,0.0],[-1.0,1.0,0.0],[0.0,1.0,0.0],[1.0,1.0,0.0]],
                            "u_points":3,"v_points":3,"subdivision_levels":2,
                            "crease_method":"uniform-integer-level-decay@1",
                            "crease_edges":[{"vertex_a":3,"vertex_b":4,"sharpness_levels":2},{"vertex_a":4,"vertex_b":5,"sharpness_levels":2}],
                            "position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]
                        }
                    }],
                    "part_outputs":[{"part_id":"cage","input_node_ids":["cage"],"material_zone_id":"zone-shell","solid":false}]
                });
                let before = json!({
                    "candidates":runtime.candidates(&project.project_id).expect("candidates"),
                    "versions":runtime.versions(Some(&project.project_id)).expect("versions")
                });
                (project.project_id, program, before)
            }
            _ => unreachable!("test backend"),
        };
        program["canonical_sha256"] = Value::String(canonical_json_hash(&program));
        let mut arguments = json!({
            "schema_version":"SubdivisionTopologyLineageRequest@1",
            "geometry_program":program,
            "subdivision_node_id":"cage",
            "max_lineage_elements":25000,
            "canonical_sha256":""
        });
        arguments["canonical_sha256"] = Value::String(canonical_json_hash(&arguments));
        let response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":407,"method":"tools/call","params":{"name":"subdivision_topology_lineage_preview","arguments":arguments.clone()}}),
        )
        .expect("subdivision lineage response");
        let structured = &response["result"]["structuredContent"];
        assert_eq!(
            structured["schema_version"], "SubdivisionTopologyLineage@1",
            "unexpected subdivision topology lineage response: {response}"
        );
        assert_eq!(
            structured["lineage_kind"],
            "control-root-to-evaluated-quad-topology@1"
        );
        assert_eq!(structured["lineage_element_count"], 442);
        assert_eq!(structured["cross_version_stable"], false);
        assert_eq!(
            structured["artifact_binding_status"],
            "unavailable-preview-only"
        );
        assert_eq!(structured["runtime_write_performed"], false);
        let summary = serde_json::from_str::<Value>(
            response["result"]["content"][0]["text"]
                .as_str()
                .expect("subdivision lineage summary"),
        )
        .expect("subdivision lineage summary JSON");
        assert_eq!(
            summary["schema_version"],
            "SubdivisionTopologyLineageMcpSummary@1"
        );
        assert_eq!(summary["structured_content_complete"], true);
        assert!(serde_json::to_vec(&response).unwrap().len() <= READ_MODEL_MCP_WIRE_MAX_BYTES);
        let after = match &backend {
            Backend::InProcess(runtime) => json!({
                "candidates":runtime.candidates(&project_id).expect("candidates"),
                "versions":runtime.versions(Some(&project_id)).expect("versions")
            }),
            _ => unreachable!("test backend"),
        };
        assert_eq!(before, after);

        for (id, invalid_max) in [(408, 0), (409, 25_001)] {
            let mut invalid = arguments.clone();
            invalid["max_lineage_elements"] = Value::from(invalid_max);
            let rejected = handle(
                &mut backend,
                &mut session,
                &json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"subdivision_topology_lineage_preview","arguments":invalid}}),
            )
            .expect("invalid subdivision lineage budget response");
            assert_eq!(rejected["error"]["data"]["code"], "INVALID_TOOL_PARAMS");
        }
        let mut wrong_version = arguments.clone();
        wrong_version["geometry_program"]["nodes"][0]["operator_id"] =
            json!("forgecad.geometry.subd-cage@1");
        wrong_version["geometry_program"]["canonical_sha256"] = json!("");
        let program_hash = canonical_json_hash(&wrong_version["geometry_program"]);
        wrong_version["geometry_program"]["canonical_sha256"] = Value::String(program_hash);
        wrong_version["canonical_sha256"] = json!("");
        wrong_version["canonical_sha256"] = Value::String(canonical_json_hash(&wrong_version));
        let rejected = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":410,"method":"tools/call","params":{"name":"subdivision_topology_lineage_preview","arguments":wrong_version}}),
        )
        .expect("wrong-version subdivision lineage response");
        assert_eq!(rejected["result"]["isError"], true);

        let mut unknown = arguments;
        unknown["python"] = json!("forbidden");
        let rejected = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":411,"method":"tools/call","params":{"name":"subdivision_topology_lineage_preview","arguments":unknown}}),
        )
        .expect("unknown subdivision lineage field response");
        assert_eq!(rejected["error"]["data"]["code"], "INVALID_TOOL_PARAMS");
    }

    #[test]
    fn subdivision_artifact_lineage_round_trips_with_exact_candidate_binding() {
        let tool = tools_with_writes(false)
            .into_iter()
            .find(|tool| tool["name"] == "subdivision_artifact_lineage_get")
            .expect("subdivision artifact lineage tool");
        assert_eq!(tool["annotations"]["readOnlyHint"], true);
        assert_eq!(tool["inputSchema"]["additionalProperties"], false);

        let (mut backend, mut session) = initialized();
        let (project_id, prepared, before) = match &backend {
            Backend::InProcess(runtime) => {
                let project = runtime
                    .create_project("Subdivision artifact MCP", json!({"profile":"mvp"}))
                    .expect("project");
                let mut program = json!({
                    "schema_version":"GeometryProgram@2",
                    "project_id":project.project_id,
                    "representation_plan_sha256":"8".repeat(64),
                    "operator_catalog_sha256":runtime.active_operator_catalog()["canonical_sha256"],
                    "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
                    "budgets":{"max_nodes":1,"max_triangles":128,"max_glb_bytes":67108864,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
                    "nodes":[{
                        "node_id":"cage","operator_id":"forgecad.geometry.subd-cage@2","inputs":[],
                        "parameters":{
                            "shape":"subd-cage",
                            "control_points":[[-1.0,-1.0,0.0],[0.0,-1.0,0.0],[1.0,-1.0,0.0],[-1.0,0.0,0.0],[0.0,0.0,1.0],[1.0,0.0,0.0],[-1.0,1.0,0.0],[0.0,1.0,0.0],[1.0,1.0,0.0]],
                            "u_points":3,"v_points":3,"subdivision_levels":2,
                            "crease_method":"uniform-integer-level-decay@1",
                            "crease_edges":[{"vertex_a":3,"vertex_b":4,"sharpness_levels":2},{"vertex_a":4,"vertex_b":5,"sharpness_levels":2}],
                            "position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]
                        }
                    }],
                    "part_outputs":[{"part_id":"cage-part","input_node_ids":["cage"],"material_zone_id":"zone-shell","solid":false}]
                });
                program["canonical_sha256"] = Value::String(canonical_json_hash(&program));
                let prepared = runtime
                    .prepare_geometry_candidate(
                        &project.project_id,
                        None,
                        json!({"typed":"geometry","geometry_program":program}),
                    )
                    .expect("geometry prepare");
                let before = json!({
                    "candidates":runtime.candidates(&project.project_id).expect("candidates"),
                    "versions":runtime.versions(Some(&project.project_id)).expect("versions")
                });
                (project.project_id, prepared, before)
            }
            _ => unreachable!("test backend"),
        };
        let mut arguments = json!({
            "schema_version":"SubdivisionArtifactLineageRequest@1",
            "project_id":project_id,
            "candidate_id":prepared["candidate"]["candidate_id"],
            "artifact_id":prepared["artifact"]["artifact_id"],
            "artifact_readback_sha256":prepared["artifact"]["canonical_sha256"],
            "subdivision_node_id":"cage",
            "max_lineage_elements":25000,
            "canonical_sha256":""
        });
        arguments["canonical_sha256"] = Value::String(canonical_json_hash(&arguments));
        let response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":412,"method":"tools/call","params":{"name":"subdivision_artifact_lineage_get","arguments":arguments.clone()}}),
        )
        .expect("subdivision artifact lineage response");
        let structured = &response["result"]["structuredContent"];
        assert_eq!(
            structured["schema_version"], "SubdivisionArtifactLineageProjection@1",
            "unexpected subdivision artifact lineage response: {response}"
        );
        assert_eq!(structured["artifact_binding"]["source_triangle_count"], 128);
        assert_eq!(structured["runtime_write_performed"], false);
        let summary = serde_json::from_str::<Value>(
            response["result"]["content"][0]["text"]
                .as_str()
                .expect("artifact lineage summary"),
        )
        .expect("artifact lineage summary JSON");
        assert_eq!(
            summary["schema_version"],
            "SubdivisionArtifactLineageMcpSummary@1"
        );
        assert_eq!(summary["max_lineage_elements"], 25000);
        assert_eq!(summary["lineage_element_count"], 442);
        assert_eq!(summary["structured_content_complete"], true);
        assert!(serde_json::to_vec(&response).unwrap().len() <= READ_MODEL_MCP_WIRE_MAX_BYTES);
        let after = match &backend {
            Backend::InProcess(runtime) => json!({
                "candidates":runtime.candidates(&project_id).expect("candidates"),
                "versions":runtime.versions(Some(&project_id)).expect("versions")
            }),
            _ => unreachable!("test backend"),
        };
        assert_eq!(before, after);

        let mut stale = arguments.clone();
        stale["artifact_readback_sha256"] = json!("f".repeat(64));
        stale["canonical_sha256"] = json!("");
        stale["canonical_sha256"] = Value::String(canonical_json_hash(&stale));
        let rejected = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":413,"method":"tools/call","params":{"name":"subdivision_artifact_lineage_get","arguments":stale}}),
        )
        .expect("stale artifact lineage response");
        assert_eq!(rejected["result"]["isError"], true);

        let mut sidecar_arguments = arguments;
        sidecar_arguments["schema_version"] = json!("SubdivisionArtifactLineageSidecarRequest@1");
        sidecar_arguments["canonical_sha256"] = json!("");
        sidecar_arguments["canonical_sha256"] =
            Value::String(canonical_json_hash(&sidecar_arguments));
        session.write_tools_enabled = true;
        let prepared_sidecar = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":415,"method":"tools/call","params":{"name":"subdivision_artifact_lineage_prepare","arguments":sidecar_arguments.clone()}}),
        )
        .expect("subdivision artifact lineage sidecar prepare response");
        assert_eq!(
            prepared_sidecar["result"]["structuredContent"]["schema_version"],
            "SubdivisionArtifactLineageLink@1"
        );
        assert_eq!(
            prepared_sidecar["result"]["structuredContent"]["materialization_status"],
            "runtime-owned-immutable-cas-sidecar"
        );
        let prepare_summary = serde_json::from_str::<Value>(
            prepared_sidecar["result"]["content"][0]["text"]
                .as_str()
                .expect("sidecar prepare summary"),
        )
        .expect("sidecar prepare summary JSON");
        assert_eq!(
            prepare_summary["schema_version"],
            "SubdivisionArtifactLineageMcpSummary@1"
        );
        assert_eq!(prepare_summary["runtime_write_performed"], true);
        assert!(prepare_summary["text"]
            .as_str()
            .expect("prepare summary text")
            .contains("writes"));

        let sidecar_before_get = match &backend {
            Backend::InProcess(runtime) => json!({
                "candidates":runtime.candidates(&project_id).expect("candidates"),
                "versions":runtime.versions(Some(&project_id)).expect("versions")
            }),
            _ => unreachable!("test backend"),
        };
        let fetched_sidecar = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":416,"method":"tools/call","params":{"name":"subdivision_artifact_lineage_sidecar_get","arguments":sidecar_arguments}}),
        )
        .expect("subdivision artifact lineage sidecar get response");
        assert_eq!(
            fetched_sidecar["result"]["structuredContent"],
            prepared_sidecar["result"]["structuredContent"]
        );
        let get_summary = serde_json::from_str::<Value>(
            fetched_sidecar["result"]["content"][0]["text"]
                .as_str()
                .expect("sidecar get summary"),
        )
        .expect("sidecar get summary JSON");
        assert_eq!(get_summary["runtime_write_performed"], false);
        assert!(get_summary["text"]
            .as_str()
            .expect("get summary text")
            .contains("no write"));
        let sidecar_after_get = match &backend {
            Backend::InProcess(runtime) => json!({
                "candidates":runtime.candidates(&project_id).expect("candidates"),
                "versions":runtime.versions(Some(&project_id)).expect("versions")
            }),
            _ => unreachable!("test backend"),
        };
        assert_eq!(sidecar_before_get, sidecar_after_get);
        assert!(
            serde_json::to_vec(&fetched_sidecar).unwrap().len() <= READ_MODEL_MCP_WIRE_MAX_BYTES
        );
    }

    #[test]
    fn subdivision_artifact_lineage_sidecar_tools_are_closed_and_write_opt_in() {
        let read_tools = tools_with_writes(false);
        let enabled_tools = tools_with_writes(true);
        let read_tool = read_tools
            .iter()
            .find(|tool| tool["name"] == "subdivision_artifact_lineage_sidecar_get")
            .expect("subdivision artifact lineage sidecar getter");
        let prepare_tool = enabled_tools
            .iter()
            .find(|tool| tool["name"] == "subdivision_artifact_lineage_prepare")
            .expect("subdivision artifact lineage sidecar prepare");

        assert_eq!(read_tool["annotations"]["readOnlyHint"], true);
        assert_eq!(read_tool["annotations"]["destructiveHint"], false);
        assert_eq!(prepare_tool["annotations"]["readOnlyHint"], false);
        assert_eq!(prepare_tool["_meta"]["forgecad"]["transaction"], "MCP010F");
        assert_eq!(
            prepare_tool["_meta"]["forgecad"]["requiresConfirmation"],
            true
        );
        assert!(read_tool["description"]
            .as_str()
            .unwrap()
            .contains("Link@1"));
        assert!(prepare_tool["description"]
            .as_str()
            .unwrap()
            .contains("Link@1"));
        assert_eq!(
            read_tool["inputSchema"], prepare_tool["inputSchema"],
            "get and prepare must share one closed request envelope"
        );
        assert_eq!(read_tool["inputSchema"]["additionalProperties"], false);
        assert!(!read_tools
            .iter()
            .any(|tool| tool["name"] == "subdivision_artifact_lineage_prepare"));
        assert!(!is_write_tool("subdivision_artifact_lineage_sidecar_get"));
        assert!(is_write_tool("subdivision_artifact_lineage_prepare"));

        let arguments = json!({
            "schema_version":"SubdivisionArtifactLineageSidecarRequest@1",
            "project_id":"project-sidecar",
            "candidate_id":"candidate-sidecar",
            "artifact_id":"a".repeat(64),
            "artifact_readback_sha256":"b".repeat(64),
            "subdivision_node_id":"cage",
            "max_lineage_elements":25000,
            "canonical_sha256":"c".repeat(64)
        });
        assert!(validate_declared_tool_input(
            "subdivision_artifact_lineage_sidecar_get",
            &arguments,
            false
        )
        .is_ok());
        assert!(validate_declared_tool_input(
            "subdivision_artifact_lineage_prepare",
            &arguments,
            true
        )
        .is_ok());
        assert!(validate_declared_tool_input(
            "subdivision_artifact_lineage_prepare",
            &arguments,
            false
        )
        .is_err());
        let mut unknown = arguments;
        unknown["python"] = json!("forbidden");
        assert!(validate_declared_tool_input(
            "subdivision_artifact_lineage_sidecar_get",
            &unknown,
            false
        )
        .is_err());

        let (mut backend, mut session) = initialized();
        let disabled = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":414,
                "method":"tools/call",
                "params":{"name":"subdivision_artifact_lineage_prepare","arguments":{}}
            }),
        )
        .expect("sidecar prepare disabled response");
        assert_eq!(disabled["result"]["isError"], true);
        assert_eq!(
            disabled["result"]["structuredContent"]["code"],
            "MCP010F_SUBDIVISION_ARTIFACT_LINEAGE_WRITE_TOOLS_DISABLED"
        );
    }

    #[test]
    fn subdivision_evaluation_v2_round_trips_and_remains_closed_and_read_only() {
        let (mut backend, mut session) = initialized();
        let project = match &backend {
            Backend::InProcess(runtime) => runtime
                .create_project("MCP subdivision evaluation v2", json!({"scope":"test"}))
                .expect("project"),
            _ => unreachable!("test backend"),
        };
        let points = (0..3)
            .flat_map(|v| (0..3).map(move |u| json!([u as f64, v as f64, 0.0])))
            .collect::<Vec<_>>();
        let mut request = json!({
            "schema_version":"SubdivisionEvaluationRequest@2",
            "project_id":project.project_id,
            "representation_plan_sha256":"6".repeat(64),
            "part_id":"subd-shell",
            "material_zone_id":"zone-shell",
            "solid":false,
            "control_cage":{"u_points":3,"v_points":3,"control_points":points},
            "policy":{"scheme":"catmull-clark-uniform-regular-quad-grid","subdivision_levels":1,"boundary_interpolation":"edge-and-corner","crease_mode":"unsupported","face_varying_interpolation":"worker-triangle-chart-postprocess","limit_surface":false,"adaptive":false},
            "transform":{"position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]},
            "budgets":{"max_nodes":1,"max_triangles":32,"max_glb_bytes":16777216,"max_worker_memory_bytes":134217728,"max_runtime_ms":5000},
            "input_sha256":""
        });
        let mut binding = request.clone();
        binding.as_object_mut().unwrap().remove("input_sha256");
        request["input_sha256"] = Value::String(canonical_json_hash(&binding));
        let before = match &backend {
            Backend::InProcess(runtime) => json!({
                "candidates":runtime.candidates(&project.project_id).expect("candidates"),
                "versions":runtime.versions(Some(&project.project_id)).expect("versions")
            }),
            _ => unreachable!("test backend"),
        };
        let response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":176,"method":"tools/call","params":{"name":"geometry_program_hash","arguments":request.clone()}}),
        )
        .expect("subdivision MCP response");
        assert_eq!(response["result"]["isError"], Value::Null, "{response}");
        let content = &response["result"]["structuredContent"];
        assert_eq!(content["schema_version"], "SubdivisionEvaluationResult@2");
        assert_eq!(
            content["predicted_topology"]["evaluated_triangle_count"],
            32
        );
        assert_eq!(content["quality_status"], "structural_only");
        assert_eq!(
            content["validator_scope"],
            "typed-policy-and-program-hash-only"
        );
        assert_eq!(content["solid"], false);
        let after = match &backend {
            Backend::InProcess(runtime) => json!({
                "candidates":runtime.candidates(&project.project_id).expect("candidates"),
                "versions":runtime.versions(Some(&project.project_id)).expect("versions")
            }),
            _ => unreachable!("test backend"),
        };
        assert_eq!(before, after);

        let mut unknown = request.clone();
        unknown["policy"]["python"] = json!("exec");
        let mut cross_branch = request.clone();
        cross_branch["base_node"] = json!({});
        let mut solid = request;
        solid["solid"] = json!(true);
        let mut out_of_bounds = solid.clone();
        out_of_bounds["solid"] = json!(false);
        out_of_bounds["control_cage"]["control_points"][0][0] = json!(10.1);
        for (id, arguments) in [
            (177, unknown),
            (179, cross_branch),
            (180, solid),
            (181, out_of_bounds),
        ] {
            let response = handle(
                &mut backend,
                &mut session,
                &json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"geometry_program_hash","arguments":arguments}}),
            )
            .expect("invalid subdivision schema response");
            assert_eq!(response["error"]["data"]["code"], "INVALID_TOOL_PARAMS");
        }
    }

    #[test]
    fn subdivision_crease_evaluation_round_trips_actual_operator_contract_and_stays_read_only() {
        let (mut backend, mut session) = initialized();
        let project = match &backend {
            Backend::InProcess(runtime) => runtime
                .create_project("MCP subdivision crease evaluation", json!({"scope":"test"}))
                .expect("project"),
            _ => unreachable!("test backend"),
        };
        let points = vec![
            json!([-1.0, -1.0, 0.0]),
            json!([0.0, -1.0, 0.0]),
            json!([1.0, -1.0, 0.0]),
            json!([-1.0, 0.0, 0.0]),
            json!([0.0, 0.0, 1.0]),
            json!([1.0, 0.0, 0.0]),
            json!([-1.0, 1.0, 0.0]),
            json!([0.0, 1.0, 0.0]),
            json!([1.0, 1.0, 0.0]),
        ];
        let mut request = json!({
            "schema_version":"SubdivisionCreaseEvaluationRequest@1",
            "project_id":project.project_id,
            "representation_plan_sha256":"9".repeat(64),
            "part_id":"subd-crease-shell",
            "material_zone_id":"zone-shell",
            "solid":false,
            "control_cage":{"u_points":3,"v_points":3,"control_points":points},
            "crease_edges":[
                {"vertex_a":4,"vertex_b":5,"sharpness_levels":2},
                {"vertex_a":3,"vertex_b":4,"sharpness_levels":1}
            ],
            "policy":{"scheme":"catmull-clark-uniform-regular-quad-grid","subdivision_levels":2,"boundary_interpolation":"edge-only","crease_method":"uniform-integer-level-decay@1","sharpness_domain":"integer-levels-1-to-2","face_varying_interpolation":"worker-triangle-chart-postprocess","limit_surface":false,"adaptive":false},
            "transform":{"position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]},
            "budgets":{"max_nodes":1,"max_triangles":128,"max_glb_bytes":16777216,"max_worker_memory_bytes":134217728,"max_runtime_ms":5000},
            "input_sha256":""
        });
        let mut binding = request.clone();
        binding.as_object_mut().unwrap().remove("input_sha256");
        binding["crease_edges"]
            .as_array_mut()
            .unwrap()
            .sort_by_key(|edge| {
                (
                    edge["vertex_a"].as_u64().unwrap(),
                    edge["vertex_b"].as_u64().unwrap(),
                )
            });
        request["input_sha256"] = Value::String(canonical_json_hash(&binding));
        let before = match &backend {
            Backend::InProcess(runtime) => json!({
                "candidates":runtime.candidates(&project.project_id).expect("candidates"),
                "versions":runtime.versions(Some(&project.project_id)).expect("versions")
            }),
            _ => unreachable!("test backend"),
        };
        let response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":307,"method":"tools/call","params":{"name":"geometry_program_hash","arguments":request.clone()}}),
        )
        .expect("crease MCP response");
        assert_eq!(response["result"]["isError"], Value::Null, "{response}");
        let content = &response["result"]["structuredContent"];
        assert_eq!(
            content["schema_version"],
            "SubdivisionCreaseEvaluationResult@1"
        );
        assert_eq!(
            content["geometry_program"]["nodes"][0]["operator_id"],
            "forgecad.geometry.subd-cage@2"
        );
        assert_eq!(
            content["predicted_topology"]["evaluated_triangle_count"],
            128
        );
        assert_eq!(
            content["predicted_topology"]["level_2_crease_application_count"],
            2
        );
        assert_eq!(content["quality_status"], "structural_only");
        assert!(serde_json::to_vec(&response).unwrap().len() <= READ_MODEL_MCP_WIRE_MAX_BYTES);
        let after = match &backend {
            Backend::InProcess(runtime) => json!({
                "candidates":runtime.candidates(&project.project_id).expect("candidates"),
                "versions":runtime.versions(Some(&project.project_id)).expect("versions")
            }),
            _ => unreachable!("test backend"),
        };
        assert_eq!(before, after);

        let mut unknown = request.clone();
        unknown["crease_edges"][0]["script"] = json!("bpy");
        let mut fractional = request.clone();
        fractional["crease_edges"][0]["sharpness_levels"] = json!(1.5);
        let mut cross_branch = request.clone();
        cross_branch["base_node"] = json!({});
        for (id, arguments) in [(308, unknown), (309, fractional), (310, cross_branch)] {
            let rejected = handle(
                &mut backend,
                &mut session,
                &json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"geometry_program_hash","arguments":arguments}}),
            )
            .expect("invalid crease schema response");
            assert_eq!(rejected["error"]["data"]["code"], "INVALID_TOOL_PARAMS");
        }
    }

    #[test]
    fn geometry_program_hash_wire_budget_fails_closed_before_transport() {
        let oversized = "x".repeat(READ_MODEL_MCP_WIRE_MAX_BYTES);
        let response = json!({
            "jsonrpc":"2.0",
            "id":311,
            "result":{
                "content":[{"type":"text","text":oversized}],
                "structuredContent":{"schema_version":"SubdivisionCreaseEvaluationResult@1"}
            }
        });
        assert!(serde_json::to_vec(&response).unwrap().len() > READ_MODEL_MCP_WIRE_MAX_BYTES);
        for name in [
            "geometry_program_hash",
            "mechanical_pose_geometry_preview",
            "mechanical_animation_clip_prepare",
            "mechanical_animation_clip_get",
            "mechanical_animation_clip_preview_get",
            "game_asset_lod_derive",
            "game_weapon_anchor_prepare",
            "game_weapon_anchor_get",
            "game_weapon_animated_glb_socket_prepare",
            "game_weapon_animated_glb_socket_get",
            "fictional_energy_vfx_animated_socket_particles_sequence_prepare",
            "fictional_energy_vfx_animated_socket_particles_sequence_get",
            "fictional_energy_vfx_animated_socket_particles_sequence_v2_prepare",
            "fictional_energy_vfx_animated_socket_particles_sequence_v2_get",
            "fictional_energy_vfx_animated_socket_trails_sequence_prepare",
            "fictional_energy_vfx_animated_socket_trails_sequence_get",
            "fictional_energy_vfx_animated_socket_trails_sequence_v2_prepare",
            "fictional_energy_vfx_animated_socket_trails_sequence_v2_get",
            "fictional_energy_vfx_animated_socket_trails_bloom_sequence_prepare",
            "fictional_energy_vfx_animated_socket_trails_bloom_sequence_get",
            "render_evidence_integrity_get",
            "render_evidence_replay_get",
        ] {
            let rejected = apply_read_model_mcp_wire_budget(name, response.clone());
            assert_eq!(rejected["result"]["isError"], true);
            assert_eq!(
                rejected["result"]["structuredContent"]["code"],
                "MCP_READ_MODEL_RESPONSE_BUDGET_EXCEEDED"
            );
            assert!(serde_json::to_vec(&rejected).unwrap().len() < READ_MODEL_MCP_WIRE_MAX_BYTES);
        }
    }

    #[test]
    fn render_evidence_replay_tool_is_closed_read_only_and_dispatches() {
        let replay_tool = tools_with_writes(false)
            .into_iter()
            .find(|tool| tool["name"] == "render_evidence_replay_get")
            .expect("render evidence replay tool");
        assert_eq!(replay_tool["annotations"]["readOnlyHint"], true);
        assert_eq!(replay_tool["annotations"]["destructiveHint"], false);
        assert_eq!(replay_tool["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            replay_tool["inputSchema"]["properties"]["integrity_request"]["additionalProperties"],
            false
        );

        let mut integrity_request = json!({
            "schema_version":"RenderEvidenceIntegrityRequest@1",
            "project_id":"project-replay",
            "candidate_id":"candidate-replay",
            "artifact_sha256":"a".repeat(64),
            "artifact_readback_object_sha256":"b".repeat(64),
            "program_sha256":"c".repeat(64),
            "reference_id":"reference-replay",
            "reference_sha256":"d".repeat(64),
            "camera_hash":"e".repeat(64),
            "camera_object_sha256":"f".repeat(64),
            "render_set_object_sha256":"1".repeat(64),
            "comparison_report_object_sha256":"2".repeat(64),
            "quality_report_object_sha256":"3".repeat(64),
            "canonical_sha256":""
        });
        integrity_request["canonical_sha256"] =
            Value::String(canonical_json_hash(&integrity_request));
        let mut arguments = json!({
            "schema_version":"RenderEvidenceReplayRequest@1",
            "candidate_state_sha256":"4".repeat(64),
            "integrity_request":integrity_request,
            "replay_policy":"fixed-worker-nine-aov-byte-replay-read-only@1",
            "canonical_sha256":""
        });
        arguments["canonical_sha256"] = Value::String(canonical_json_hash(&arguments));

        let (mut backend, mut session) = initialized();
        let preflight = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":311,"method":"tools/call","params":{"name":"skill_get","arguments":{"skill_id":"ponytail-preflight","version":"0.1.0"}}}),
        )
        .expect("ponytail preflight response");
        assert_eq!(
            preflight["result"]["structuredContent"]["skill"]["skill_id"],
            "ponytail-preflight"
        );
        let response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":312,"method":"tools/call","params":{"name":"render_evidence_replay_get","arguments":arguments.clone()}}),
        )
        .expect("render replay dispatch response");
        assert_eq!(response["result"]["isError"], true);
        assert!(
            response["result"]["content"][0]["text"]
                .as_str()
                .expect("runtime error text")
                .contains("RENDER_EVIDENCE_INTEGRITY_INVALID"),
            "{response}"
        );

        let mut unknown = arguments;
        unknown["integrity_request"]["python"] = json!("bpy");
        let rejected = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":313,"method":"tools/call","params":{"name":"render_evidence_replay_get","arguments":unknown}}),
        )
        .expect("nested unknown field response");
        assert_eq!(rejected["error"]["data"]["code"], "INVALID_TOOL_PARAMS");
    }

    #[test]
    fn parametric_design_kit_request_round_trips_through_read_only_mcp() {
        let (mut backend, mut session) = initialized();
        let project = match &backend {
            Backend::InProcess(runtime) => runtime
                .create_project("MCP PDK v0 transport", json!({"scope":"test"}))
                .expect("project"),
            _ => unreachable!("test backend"),
        };
        let mut request = json!({
            "schema_version":"ParametricDesignKitRequest@1",
            "project_id":project.project_id,
            "representation_plan_sha256":"a".repeat(64),
            "kit_id":"forgecad.kit.sensor@1",
            "part_id":"sensor",
            "material_zone_id":"zone-black-mechanical",
            "intent":{
                "radius_m":0.12,
                "height_m":0.32,
                "radial_segments":16,
                "position_m":[0.0,2.0,0.0],
                "rotation_rad":[0.0,0.0,0.0]
            },
            "input_sha256":""
        });
        let mut input_binding = request.clone();
        input_binding
            .as_object_mut()
            .expect("PDK request object")
            .remove("input_sha256");
        request["input_sha256"] = Value::String(canonical_json_hash(&input_binding));
        let response = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":17,
                "method":"tools/call",
                "params":{"name":"geometry_program_hash","arguments":request.clone()}
            }),
        )
        .expect("PDK MCP response");
        assert_eq!(
            response["result"]["isError"],
            Value::Null,
            "Parametric Design Kit must remain usable through the MCP read-only hash route: {response}"
        );
        assert_eq!(
            response["result"]["structuredContent"]["schema_version"],
            "ParametricDesignKitProgram@1"
        );
        assert_eq!(
            response["result"]["structuredContent"]["geometry_program"]["nodes"][0]["operator_id"],
            "forgecad.geometry.primitive@2"
        );
        let candidates = match &backend {
            Backend::InProcess(runtime) => {
                runtime.candidates(&project.project_id).expect("candidates")
            }
            _ => unreachable!("test backend"),
        };
        assert!(
            candidates.is_empty(),
            "read-only PDK must not create candidates"
        );
    }

    #[test]
    fn parametric_group_v2_round_trips_and_rejects_dynamic_extension_fields() {
        let (mut backend, mut session) = initialized();
        let project = match &backend {
            Backend::InProcess(runtime) => runtime
                .create_project("MCP PDK v2 group transport", json!({"scope":"test"}))
                .expect("project"),
            _ => unreachable!("test backend"),
        };
        let mut request = json!({
            "schema_version":"ParametricDesignKitRequest@2",
            "project_id":project.project_id,
            "representation_plan_sha256":"c".repeat(64),
            "template_id":"forgecad.group.rounded-box@1",
            "instance_id":"rounded-shell-instance",
            "part_id":"rounded-shell",
            "material_zone_id":"zone-white-shell",
            "parameters":{
                "size_m":[1.0,0.8,0.4],
                "position_m":[0.0,0.0,0.0],
                "rotation_rad":[0.0,0.0,0.0],
                "bevel_width_m":0.04,
                "bevel_segments":2,
                "bevel_profile":0.5,
                "crease_angle_rad":1.0
            },
            "input_sha256":""
        });
        let mut binding = request.clone();
        binding.as_object_mut().unwrap().remove("input_sha256");
        request["input_sha256"] = Value::String(canonical_json_hash(&binding));
        let before = match &backend {
            Backend::InProcess(runtime) => json!({
                "candidates":runtime.candidates(&project.project_id).unwrap(),
                "versions":runtime.versions(Some(&project.project_id)).unwrap()
            }),
            _ => unreachable!("test backend"),
        };
        let response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":182,"method":"tools/call","params":{"name":"geometry_program_hash","arguments":request.clone()}}),
        )
        .expect("group MCP response");
        assert_eq!(response["result"]["isError"], Value::Null, "{response}");
        let content = &response["result"]["structuredContent"];
        assert_eq!(content["schema_version"], "ParametricDesignKitProgram@2");
        assert_eq!(content["template_definition"]["nested_group_depth"], 0);
        assert_eq!(
            content["geometry_program"]["nodes"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        assert_eq!(content["runtime_write_performed"], false);
        let after = match &backend {
            Backend::InProcess(runtime) => json!({
                "candidates":runtime.candidates(&project.project_id).unwrap(),
                "versions":runtime.versions(Some(&project.project_id)).unwrap()
            }),
            _ => unreachable!("test backend"),
        };
        assert_eq!(before, after);

        let mut script = request.clone();
        script["parameters"]["script"] = json!("python.exec");
        let mut wrong_template_parameters = request.clone();
        wrong_template_parameters["parameters"] = json!({
            "size_m":[1.0,1.0,1.0],"position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0],
            "mirror_axis":"x","mirror_offset_m":0.0,"crease_angle_rad":1.0
        });
        let mut path = request;
        path["path"] = json!("/tmp/plugin.py");
        for (id, arguments) in [(183, script), (184, wrong_template_parameters), (185, path)] {
            let response = handle(
                &mut backend,
                &mut session,
                &json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"geometry_program_hash","arguments":arguments}}),
            )
            .expect("invalid group MCP response");
            assert_eq!(response["error"]["data"]["code"], "INVALID_TOOL_PARAMS");
        }
    }

    #[test]
    fn geometry_modifier_stack_round_trips_through_read_only_mcp() {
        let (mut backend, mut session) = initialized();
        let project = match &backend {
            Backend::InProcess(runtime) => runtime
                .create_project("MCP modifier stack transport", json!({"scope":"test"}))
                .expect("project"),
            _ => unreachable!("test backend"),
        };
        let mut request = json!({
            "schema_version":"GeometryModifierStackRequest@1",
            "project_id":project.project_id,
            "representation_plan_sha256":"9".repeat(64),
            "part_id":"shell",
            "material_zone_id":"zone-shell",
            "solid":true,
            "base_node":{"node_id":"base","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[1.0,1.0,1.0],"position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}},
            "modifiers":[
                {"modifier_id":"round","enabled":true,"operator_id":"forgecad.geometry.bevel@1","parameters":{"shape":"bevel","width_m":0.05,"segments":2,"profile":0.5,"edge_scope":"all-source-box-edges","clamp_overlap":false}},
                {"modifier_id":"shade","enabled":true,"operator_id":"forgecad.geometry.normal-policy@1","parameters":{"shape":"normal-policy","weighting":"face-area-x-corner-angle","crease_angle_rad":1.0471975511965976,"keep_sharp":true,"output_domain":"corner"}}
            ],
            "input_sha256":""
        });
        let mut binding = request.clone();
        binding
            .as_object_mut()
            .expect("modifier request object")
            .remove("input_sha256");
        request["input_sha256"] = Value::String(canonical_json_hash(&binding));
        let response = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":170,
                "method":"tools/call",
                "params":{"name":"geometry_program_hash","arguments":request.clone()}
            }),
        )
        .expect("modifier MCP response");
        assert_eq!(
            response["result"]["structuredContent"]["schema_version"],
            "GeometryModifierStackProgram@1"
        );
        assert_eq!(
            response["result"]["structuredContent"]["geometry_program"]["nodes"][1]["operator_id"],
            "forgecad.geometry.bevel@1"
        );
        assert_eq!(
            response["result"]["structuredContent"]["quality_status"],
            "structural_only"
        );
        let mut v2_request = json!({
            "schema_version":"GeometryModifierStackRequest@1",
            "project_id":project.project_id,
            "representation_plan_sha256":"a".repeat(64),
            "part_id":"vent-shell",
            "material_zone_id":"zone-shell",
            "solid":true,
            "base_node":{
                "node_id":"vent-base",
                "operator_id":"forgecad.geometry.vent-array@2",
                "inputs":[],
                "parameters":{
                    "shape":"vent-array",
                    "width_m":1.6,
                    "height_m":0.8,
                    "depth_m":0.26,
                    "face_thickness_m":0.08,
                    "backing_depth_m":0.08,
                    "backing_gap_m":0.10,
                    "slot_count":4,
                    "slot_width_m":0.16,
                    "slot_spacing_m":0.12,
                    "slot_margin_m":0.16,
                    "slot_edge_bevel_m":0.02,
                    "bevel_segments":2,
                    "position_m":[0.0,0.0,0.0],
                    "rotation_rad":[0.0,0.0,0.0]
                }
            },
            "modifiers":[
                {"modifier_id":"trace","enabled":false,"operator_id":"forgecad.geometry.normal-policy@1","parameters":{"shape":"normal-policy","weighting":"face-area-x-corner-angle","crease_angle_rad":1.0,"keep_sharp":true,"output_domain":"corner"}}
            ],
            "input_sha256":""
        });
        let mut v2_binding = v2_request.clone();
        v2_binding
            .as_object_mut()
            .expect("v2 modifier request object")
            .remove("input_sha256");
        v2_request["input_sha256"] = Value::String(canonical_json_hash(&v2_binding));
        let v2_response = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":179,
                "method":"tools/call",
                "params":{"name":"geometry_program_hash","arguments":v2_request}
            }),
        )
        .expect("vent-array@2 modifier MCP response");
        assert_eq!(
            v2_response["result"]["structuredContent"]["geometry_program"]["nodes"][0]
                ["operator_id"],
            "forgecad.geometry.vent-array@2"
        );
        let mut channel = json!({
            "schema_version":"GeometryModifierStackRequest@1",
            "project_id":project.project_id,
            "representation_plan_sha256":"a".repeat(64),
            "part_id":"recessed-channel",
            "material_zone_id":"zone-shell",
            "solid":true,
            "base_node":{
                "node_id":"channel-base",
                "operator_id":"forgecad.geometry.recessed-channel@1",
                "inputs":[],
                "parameters":{
                    "shape":"recessed-channel",
                    "stations":[
                        {"point_m":[-0.8,0.0,0.0],"width_m":0.30,"depth_m":0.12},
                        {"point_m":[0.0,0.08,0.0],"width_m":0.36,"depth_m":0.16},
                        {"point_m":[0.82,0.0,0.0],"width_m":0.28,"depth_m":0.10}
                    ],
                    "path_frame":"planar-xy-z-up@1",
                    "floor_width_ratio":0.42,
                    "edge_bevel_m":0.01,
                    "start_transition_m":0.08,
                    "end_transition_m":0.10,
                    "transition_segments":2,
                    "position_m":[0.0,0.0,0.0],
                    "rotation_rad":[0.0,0.0,0.0]
                }
            },
            "modifiers":[
                {"modifier_id":"trace","enabled":false,"operator_id":"forgecad.geometry.normal-policy@1","parameters":{"shape":"normal-policy","weighting":"face-area-x-corner-angle","crease_angle_rad":1.0,"keep_sharp":true,"output_domain":"corner"}}
            ],
            "input_sha256":""
        });
        let mut channel_binding = channel.clone();
        channel_binding
            .as_object_mut()
            .expect("channel modifier request object")
            .remove("input_sha256");
        channel["input_sha256"] = Value::String(canonical_json_hash(&channel_binding));
        let channel_response = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":180,
                "method":"tools/call",
                "params":{"name":"geometry_program_hash","arguments":channel}
            }),
        )
        .expect("recessed-channel modifier MCP response");
        assert_eq!(
            channel_response["result"]["structuredContent"]["geometry_program"]["nodes"][0]
                ["operator_id"],
            "forgecad.geometry.recessed-channel@1"
        );
        let mut wrong_branch = request.clone();
        wrong_branch["modifiers"][0]["parameters"] =
            json!({"shape":"mirror","axis":"x","offset_m":0.0});
        let mut nested_unknown = request.clone();
        nested_unknown["modifiers"][0]["parameters"]["script"] = json!("python.exec");
        let mut base_nested_unknown = request.clone();
        base_nested_unknown["base_node"]["parameters"]["script"] = json!("python.exec");
        let mut base_wrong_branch = request.clone();
        base_wrong_branch["base_node"]["operator_id"] = json!("forgecad.geometry.panel@1");
        let mut cross_branch_field = request;
        cross_branch_field["kit_id"] = json!("forgecad.kit.panel@1");
        for (id, arguments) in [
            (171, wrong_branch),
            (172, nested_unknown),
            (173, cross_branch_field),
            (174, base_nested_unknown),
            (178, base_wrong_branch),
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
            .expect("invalid modifier schema response");
            assert_eq!(response["error"]["data"]["code"], "INVALID_TOOL_PARAMS");
        }
    }

    #[test]
    fn geometry_modifier_evaluation_v2_round_trips_and_remains_closed_and_read_only() {
        let (mut backend, mut session) = initialized();
        let project = match &backend {
            Backend::InProcess(runtime) => runtime
                .create_project("MCP modifier evaluation v2", json!({"scope":"test"}))
                .expect("project"),
            _ => unreachable!("test backend"),
        };
        let make_request = |previous_evaluation: Value| {
            let mut request = json!({
                "schema_version":"GeometryModifierEvaluationRequest@2",
                "project_id":project.project_id,
                "representation_plan_sha256":"6".repeat(64),
                "part_id":"shell",
                "material_zone_id":"zone-shell",
                "solid":true,
                "base_node":{"node_id":"base","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[1.0,1.0,1.0],"position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                "modifiers":[
                    {"modifier_id":"round","enabled":true,"operator_id":"forgecad.geometry.bevel@1","parameters":{"shape":"bevel","width_m":0.05,"segments":2,"profile":0.5,"edge_scope":"all-source-box-edges","clamp_overlap":false}},
                    {"modifier_id":"preview","enabled":false,"operator_id":"forgecad.geometry.mirror@1","parameters":{"shape":"mirror","axis":"x","offset_m":0.0}},
                    {"modifier_id":"shade","enabled":true,"operator_id":"forgecad.geometry.normal-policy@1","parameters":{"shape":"normal-policy","weighting":"face-area-x-corner-angle","crease_angle_rad":1.0,"keep_sharp":true,"output_domain":"corner"}}
                ],
                "previous_evaluation":previous_evaluation,
                "input_sha256":""
            });
            let mut binding = request.clone();
            binding.as_object_mut().unwrap().remove("input_sha256");
            request["input_sha256"] = Value::String(canonical_json_hash(&binding));
            request
        };
        let call = |backend: &mut Backend, session: &mut Session, id: u64, arguments: Value| {
            handle(
                backend,
                session,
                &json!({
                    "jsonrpc":"2.0",
                    "id":id,
                    "method":"tools/call",
                    "params":{"name":"geometry_program_hash","arguments":arguments}
                }),
            )
            .expect("modifier evaluation MCP response")
        };
        let initial = call(&mut backend, &mut session, 175, make_request(Value::Null));
        assert_eq!(initial["result"]["isError"], Value::Null);
        let initial_content = &initial["result"]["structuredContent"];
        assert_eq!(
            initial_content["schema_version"],
            "GeometryModifierEvaluationResult@2"
        );
        assert_eq!(initial_content["cache_decision"], "initial-miss");
        assert_eq!(initial_content["quality_status"], "structural_only");
        assert_eq!(initial_content["reuse_kind"], "semantic-signature-only");
        assert_eq!(
            initial_content["output_kind"],
            "geometry-program-canonical-sha256"
        );

        let repeat = call(
            &mut backend,
            &mut session,
            176,
            make_request(initial_content["evaluation_signature"].clone()),
        );
        assert_eq!(
            repeat["result"]["structuredContent"]["cache_decision"],
            "reusable"
        );
        assert_eq!(
            repeat["result"]["structuredContent"]["evaluation_dirty"],
            false
        );

        let mut nested_unknown = make_request(initial_content["evaluation_signature"].clone());
        nested_unknown["previous_evaluation"]["stages"][0]["runtime_pointer"] = json!("0xdeadbeef");
        let rejected = call(&mut backend, &mut session, 177, nested_unknown);
        assert_eq!(rejected["error"]["data"]["code"], "INVALID_TOOL_PARAMS");

        let mut base_wrong_branch = make_request(initial_content["evaluation_signature"].clone());
        base_wrong_branch["base_node"]["operator_id"] = json!("forgecad.geometry.panel@1");
        let rejected = call(&mut backend, &mut session, 179, base_wrong_branch);
        assert_eq!(rejected["error"]["data"]["code"], "INVALID_TOOL_PARAMS");

        let candidates = match &backend {
            Backend::InProcess(runtime) => {
                runtime.candidates(&project.project_id).expect("candidates")
            }
            _ => unreachable!("test backend"),
        };
        assert!(
            candidates.is_empty(),
            "modifier evaluation v2 must not create candidates"
        );
    }

    #[test]
    fn topology_snapshot_round_trips_as_a_closed_read_only_mcp_tool() {
        let tool = tools_with_writes(false)
            .into_iter()
            .find(|tool| tool["name"] == "topology_snapshot_get")
            .expect("topology snapshot tool");
        assert_eq!(tool["annotations"]["readOnlyHint"], true);
        assert_eq!(tool["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            tool["inputSchema"]["properties"]["snapshot_policy_sha256"]["const"],
            "7d6b64a92c00841d80ec887542ff11b968fd387f7b5bdf5b4b4522a52ff1af28"
        );

        let (mut backend, mut session) = initialized();
        let (project, prepared) = match &backend {
            Backend::InProcess(runtime) => {
                let project = runtime
                    .create_project("MCP topology transport", json!({"scope":"test"}))
                    .expect("project");
                let catalog_sha256 = runtime.active_operator_catalog()["canonical_sha256"]
                    .as_str()
                    .expect("catalog hash")
                    .to_owned();
                let mut program = json!({
                    "schema_version":"GeometryProgram@2",
                    "project_id":project.project_id,
                    "representation_plan_sha256":"8".repeat(64),
                    "operator_catalog_sha256":catalog_sha256,
                    "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
                    "budgets":{"max_nodes":1,"max_triangles":1000,"max_glb_bytes":1048576,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
                    "nodes":[{"node_id":"shell","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[1.0,1.0,1.0],"position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}}],
                    "part_outputs":[{"part_id":"shell","input_node_ids":["shell"],"material_zone_id":"zone-shell","solid":true}]
                });
                program["canonical_sha256"] = Value::String(canonical_json_hash(&program));
                let prepared = runtime
                    .prepare_geometry_candidate(
                        &project.project_id,
                        None,
                        json!({"typed":"geometry","geometry_program":program}),
                    )
                    .expect("V2 geometry prepare");
                (project, prepared)
            }
            _ => unreachable!("test backend"),
        };
        let artifact = &prepared["artifact"];
        let arguments = json!({
            "schema_version":"TopologySnapshotRequest@1",
            "project_id":project.project_id,
            "artifact_id":artifact["artifact_id"],
            "candidate_id":prepared["candidate"]["candidate_id"],
            "part_id":"shell",
            "artifact_readback_sha256":artifact["canonical_sha256"],
            "program_sha256":artifact["program_sha256"],
            "operator_catalog_sha256":artifact["operator_catalog_sha256"],
            "readback_config_sha256":artifact["readback_config_sha256"],
            "snapshot_policy_sha256":"7d6b64a92c00841d80ec887542ff11b968fd387f7b5bdf5b4b4522a52ff1af28",
            "max_face_count":512
        });
        let response = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":175,
                "method":"tools/call",
                "params":{"name":"topology_snapshot_get","arguments":arguments.clone()}
            }),
        )
        .expect("topology MCP response");
        assert_eq!(
            response["result"]["structuredContent"]["schema_version"],
            "TopologySnapshot@1"
        );
        assert_eq!(
            response["result"]["structuredContent"]["counts"]["face_count"],
            12
        );
        assert!(serde_json::to_vec(&response).unwrap().len() <= READ_MODEL_MCP_WIRE_MAX_BYTES);
        let content_summary: Value = serde_json::from_str(
            response["result"]["content"][0]["text"]
                .as_str()
                .expect("bounded topology summary text"),
        )
        .expect("topology summary JSON");
        assert_eq!(
            content_summary["schema_version"],
            "TopologySnapshotMcpSummary@1"
        );
        assert_eq!(content_summary["structured_content_complete"], true);
        assert!(content_summary.get("vertices").is_none());
        let mut unknown = arguments;
        unknown["python"] = json!("bpy.ops");
        let rejected = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":176,
                "method":"tools/call",
                "params":{"name":"topology_snapshot_get","arguments":unknown}
            }),
        )
        .expect("invalid topology schema response");
        assert_eq!(rejected["error"]["data"]["code"], "INVALID_TOOL_PARAMS");
        let candidate_count = match &backend {
            Backend::InProcess(runtime) => runtime
                .candidates(&project.project_id)
                .expect("candidates")
                .len(),
            _ => unreachable!("test backend"),
        };
        assert_eq!(
            candidate_count, 1,
            "topology readback must not create a candidate"
        );
    }

    #[test]
    fn authoring_topology_and_edit_preview_round_trip_as_closed_read_only_mcp_tools() {
        let tools = tools_with_writes(false);
        for name in ["authoring_topology_get", "authoring_mesh_edit_preview"] {
            let tool = tools
                .iter()
                .find(|tool| tool["name"] == name)
                .unwrap_or_else(|| panic!("missing {name}"));
            assert_eq!(tool["annotations"]["readOnlyHint"], true);
            assert_eq!(tool["inputSchema"]["additionalProperties"], false);
        }
        let preview_tool = tools
            .iter()
            .find(|tool| tool["name"] == "authoring_mesh_edit_preview")
            .expect("preview tool");
        assert_eq!(
            preview_tool["inputSchema"]["properties"]["edit"]["oneOf"]
                .as_array()
                .unwrap()
                .len(),
            2
        );

        let (mut backend, mut session) = initialized();
        let (project, prepared) = match &backend {
            Backend::InProcess(runtime) => {
                let project = runtime
                    .create_project("MCP authoring topology", json!({"scope":"test"}))
                    .expect("project");
                let catalog_sha256 = runtime.active_operator_catalog()["canonical_sha256"]
                    .as_str()
                    .expect("catalog hash")
                    .to_owned();
                let mut program = json!({
                    "schema_version":"GeometryProgram@2",
                    "project_id":project.project_id,
                    "representation_plan_sha256":"b".repeat(64),
                    "operator_catalog_sha256":catalog_sha256,
                    "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
                    "budgets":{"max_nodes":1,"max_triangles":32,"max_glb_bytes":67108864,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
                    "nodes":[{
                        "node_id":"authored-panel",
                        "operator_id":"forgecad.geometry.authoring-mesh@1",
                        "inputs":[],
                        "parameters":{
                            "shape":"authoring-mesh",
                            "topology_policy":"triangle-quad-manifold-with-boundary@1",
                            "vertices":[
                                {"element_id":"v0","position_m":[-1.0,-1.0,0.0]},
                                {"element_id":"v1","position_m":[1.0,-1.0,0.0]},
                                {"element_id":"v2","position_m":[1.0,1.0,0.0]},
                                {"element_id":"v3","position_m":[-1.0,1.0,0.0]}
                            ],
                            "edges":[
                                {"element_id":"e01","vertex_ids":["v0","v1"]},
                                {"element_id":"e03","vertex_ids":["v0","v3"]},
                                {"element_id":"e12","vertex_ids":["v1","v2"]},
                                {"element_id":"e23","vertex_ids":["v2","v3"]}
                            ],
                            "loops":[
                                {"element_id":"l0","face_id":"f0","ordinal":0,"vertex_id":"v0","edge_id":"e01","edge_forward":true},
                                {"element_id":"l1","face_id":"f0","ordinal":1,"vertex_id":"v1","edge_id":"e12","edge_forward":true},
                                {"element_id":"l2","face_id":"f0","ordinal":2,"vertex_id":"v2","edge_id":"e23","edge_forward":true},
                                {"element_id":"l3","face_id":"f0","ordinal":3,"vertex_id":"v3","edge_id":"e03","edge_forward":false}
                            ],
                            "faces":[{"element_id":"f0","loop_ids":["l0","l1","l2","l3"]}],
                            "position_m":[0.0,0.0,0.0],
                            "rotation_rad":[0.0,0.0,0.0]
                        }
                    }],
                    "part_outputs":[{"part_id":"authored-panel","input_node_ids":["authored-panel"],"material_zone_id":"zone-authored-shell","solid":false}]
                });
                program["canonical_sha256"] = Value::String(canonical_json_hash(&program));
                let prepared = runtime
                    .prepare_geometry_candidate(
                        &project.project_id,
                        None,
                        json!({"typed":"geometry","geometry_program":program}),
                    )
                    .expect("authoring geometry prepare");
                (project, prepared)
            }
            _ => unreachable!("test backend"),
        };
        let artifact = &prepared["artifact"];
        let arguments = json!({
            "schema_version":"AuthoringTopologyRequest@1",
            "project_id":project.project_id,
            "candidate_id":prepared["candidate"]["candidate_id"],
            "artifact_id":artifact["artifact_id"],
            "artifact_readback_sha256":artifact["canonical_sha256"],
            "program_sha256":artifact["program_sha256"],
            "operator_catalog_sha256":artifact["operator_catalog_sha256"],
            "readback_config_sha256":artifact["readback_config_sha256"],
            "authoring_node_id":"authored-panel",
            "part_id":"authored-panel",
            "authoring_topology_policy_sha256":"a6fb36a530e49537673b66d65ecb6e4fb4f51ffb3e7d01a0980be71f28cb367d",
            "max_response_bytes":1048576
        });
        let topology_response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":181,"method":"tools/call","params":{"name":"authoring_topology_get","arguments":arguments.clone()}}),
        )
        .expect("authoring topology response");
        assert_eq!(
            topology_response["result"]["structuredContent"]["schema_version"],
            "AuthoringTopology@1"
        );
        assert_eq!(
            topology_response["result"]["structuredContent"]["counts"]["face_count"],
            1
        );
        assert!(
            serde_json::to_vec(&topology_response).unwrap().len() <= READ_MODEL_MCP_WIRE_MAX_BYTES
        );
        let base_topology_sha256 =
            topology_response["result"]["structuredContent"]["topology_sha256"].clone();

        let mut preview_arguments = json!({
            "schema_version":"AuthoringMeshEditPreviewRequest@1",
            "topology_request":arguments,
            "base_topology_sha256":base_topology_sha256,
            "edit":{"operation":"single_face_extrude","face_id":"f0","distance_m":0.25},
            "edit_policy_sha256":"1d050226b13848902f44bddb1b88c240cdfa86759703f804443b03964f8ddaae"
        });
        let input_sha256 = canonical_json_hash(&preview_arguments);
        preview_arguments["input_sha256"] = Value::String(input_sha256);
        let preview_response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":182,"method":"tools/call","params":{"name":"authoring_mesh_edit_preview","arguments":preview_arguments.clone()}}),
        )
        .expect("authoring preview response");
        assert_eq!(
            preview_response["result"]["structuredContent"]["schema_version"],
            "AuthoringMeshEditPreview@1"
        );
        assert_eq!(
            preview_response["result"]["structuredContent"]["counts"]["after"]["triangle_count"],
            10
        );
        assert_eq!(
            preview_response["result"]["structuredContent"]["runtime_write_performed"],
            false
        );
        assert!(
            serde_json::to_vec(&preview_response).unwrap().len() <= READ_MODEL_MCP_WIRE_MAX_BYTES
        );

        let read_tools = tools_with_writes(false);
        assert!(!read_tools
            .iter()
            .any(|tool| tool["name"] == "authoring_mesh_edit_prepare"));
        let prepare_tool = tools_with_writes(true)
            .into_iter()
            .find(|tool| tool["name"] == "authoring_mesh_edit_prepare")
            .expect("authoring prepare write tool");
        assert_eq!(prepare_tool["annotations"]["readOnlyHint"], false);
        assert_eq!(prepare_tool["annotations"]["idempotentHint"], true);
        assert_eq!(prepare_tool["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            prepare_tool["inputSchema"]["properties"]["preview_request"]["additionalProperties"],
            false
        );
        assert_eq!(
            prepare_tool["_meta"]["forgecad"]["requiresConfirmation"],
            true
        );
        assert_eq!(prepare_tool["_meta"]["forgecad"]["transaction"], "MCP010F");
        assert!(is_write_tool("authoring_mesh_edit_prepare"));

        let mut expected_candidate_count = 1;
        if build_cohort_sha256().is_some() {
            let mut prepare_arguments = json!({
                "schema_version":"AuthoringMeshEditPrepareRequest@1",
                "project_id":project.project_id,
                "source_candidate_id":prepared["candidate"]["candidate_id"],
                "base_version_id":null,
                "preview_request":preview_arguments.clone(),
                "expected_preview_canonical_sha256":preview_response["result"]["structuredContent"]["canonical_sha256"],
                "idempotency_key":"mcp-authoring-edit-prepare-once",
                "max_response_bytes":1048576
            });
            prepare_arguments["input_sha256"] =
                Value::String(canonical_json_hash(&prepare_arguments));
            let disabled = handle(
                &mut backend,
                &mut session,
                &json!({"jsonrpc":"2.0","id":185,"method":"tools/call","params":{"name":"authoring_mesh_edit_prepare","arguments":prepare_arguments.clone()}}),
            )
            .expect("disabled authoring prepare response");
            assert_eq!(disabled["result"]["isError"], true);

            session.write_tools_enabled = true;
            let prepared_response = handle(
                &mut backend,
                &mut session,
                &json!({"jsonrpc":"2.0","id":186,"method":"tools/call","params":{"name":"authoring_mesh_edit_prepare","arguments":prepare_arguments.clone()}}),
            )
            .expect("authoring prepare response");
            assert_eq!(
                prepared_response["result"]["structuredContent"]["schema_version"],
                "AuthoringMeshEditPrepare@1"
            );
            assert_eq!(
                prepared_response["result"]["structuredContent"]["candidate"]["state"],
                "reviewable"
            );
            assert_eq!(
                prepared_response["result"]["structuredContent"]["confirm_status"],
                "approval-required"
            );
            let summary: Value = serde_json::from_str(
                prepared_response["result"]["content"][0]["text"]
                    .as_str()
                    .expect("prepare summary text"),
            )
            .expect("prepare summary JSON");
            assert_eq!(
                summary["schema_version"],
                "AuthoringMeshEditPrepareMcpSummary@1"
            );
            assert_eq!(summary["structured_content_complete"], true);
            assert_eq!(summary["confirm_status"], "approval-required");
            assert!(
                serde_json::to_vec(&prepared_response).unwrap().len()
                    <= READ_MODEL_MCP_WIRE_MAX_BYTES
            );
            let replay_response = handle(
                &mut backend,
                &mut session,
                &json!({"jsonrpc":"2.0","id":187,"method":"tools/call","params":{"name":"authoring_mesh_edit_prepare","arguments":prepare_arguments.clone()}}),
            )
            .expect("authoring prepare replay response");
            assert_eq!(
                replay_response["result"]["structuredContent"],
                prepared_response["result"]["structuredContent"]
            );
            let mut unknown_prepare = prepare_arguments;
            unknown_prepare["preview_request"]["edit"]["python"] =
                json!("bmesh.ops.extrude_face_region");
            let rejected_prepare = handle(
                &mut backend,
                &mut session,
                &json!({"jsonrpc":"2.0","id":188,"method":"tools/call","params":{"name":"authoring_mesh_edit_prepare","arguments":unknown_prepare}}),
            )
            .expect("invalid authoring prepare schema response");
            assert_eq!(
                rejected_prepare["error"]["data"]["code"],
                "INVALID_TOOL_PARAMS"
            );
            expected_candidate_count = 2;
        }

        let mut stale = preview_arguments.clone();
        stale["base_topology_sha256"] = json!("f".repeat(64));
        let mut stale_preimage = stale.clone();
        stale_preimage
            .as_object_mut()
            .unwrap()
            .remove("input_sha256");
        stale["input_sha256"] = Value::String(canonical_json_hash(&stale_preimage));
        let stale_response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":183,"method":"tools/call","params":{"name":"authoring_mesh_edit_preview","arguments":stale}}),
        )
        .expect("stale preview response");
        assert_eq!(stale_response["result"]["isError"], true);

        let mut unknown = preview_arguments;
        unknown["edit"]["python"] = json!("bmesh.ops.extrude_face_region");
        let rejected = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":184,"method":"tools/call","params":{"name":"authoring_mesh_edit_preview","arguments":unknown}}),
        )
        .expect("invalid authoring preview schema response");
        assert_eq!(rejected["error"]["data"]["code"], "INVALID_TOOL_PARAMS");
        let candidate_count = match &backend {
            Backend::InProcess(runtime) => runtime.candidates(&project.project_id).unwrap().len(),
            _ => unreachable!("test backend"),
        };
        assert_eq!(candidate_count, expected_candidate_count);
    }

    #[test]
    fn mechanical_pose_round_trips_as_a_closed_candidate_bound_read_only_tool() {
        let tool = tools_with_writes(false)
            .into_iter()
            .find(|tool| tool["name"] == "mechanical_pose_evaluate")
            .expect("mechanical pose tool");
        assert_eq!(tool["annotations"]["readOnlyHint"], true);
        let branches = tool["inputSchema"]["oneOf"]
            .as_array()
            .expect("single and sequence pose branches");
        assert_eq!(branches.len(), 2);
        assert_eq!(branches[0]["additionalProperties"], false);
        assert_eq!(
            branches[0]["properties"]["schema_version"]["const"],
            "MechanicalPoseEvaluationRequest@1"
        );
        assert_eq!(
            branches[1]["properties"]["schema_version"]["const"],
            "MechanicalPoseSequencePreviewRequest@1"
        );
        assert_eq!(
            branches[1]["properties"]["sample_time_ticks"]["maxItems"],
            16
        );
        assert_eq!(
            branches[0]["properties"]["rest_frame_draft"]["properties"]["links"]["maxItems"],
            64
        );

        let (mut backend, mut session) = initialized();
        let (project, prepared) = match &backend {
            Backend::InProcess(runtime) => {
                let project = runtime
                    .create_project("MCP mechanical pose transport", json!({"scope":"test"}))
                    .expect("project");
                let catalog_sha256 = runtime.active_operator_catalog()["canonical_sha256"]
                    .as_str()
                    .expect("catalog hash")
                    .to_owned();
                let mut program = json!({
                    "schema_version":"GeometryProgram@2",
                    "project_id":project.project_id,
                    "representation_plan_sha256":"7".repeat(64),
                    "operator_catalog_sha256":catalog_sha256,
                    "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
                    "budgets":{"max_nodes":1,"max_triangles":1000,"max_glb_bytes":1048576,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
                    "nodes":[{"node_id":"root-node","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[1.0,1.0,1.0],"position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}}],
                    "part_outputs":[{"part_id":"root-part","input_node_ids":["root-node"],"material_zone_id":"zone-root","solid":true}]
                });
                program["canonical_sha256"] = Value::String(canonical_json_hash(&program));
                let prepared = runtime
                    .prepare_geometry_candidate(
                        &project.project_id,
                        None,
                        json!({"typed":"geometry","geometry_program":program}),
                    )
                    .expect("geometry prepare");
                (project, prepared)
            }
            _ => unreachable!("test backend"),
        };
        let artifact = &prepared["artifact"];
        let mut arguments = json!({
            "schema_version":"MechanicalPoseEvaluationRequest@1",
            "project_id":project.project_id,
            "artifact_id":artifact["artifact_id"],
            "candidate_id":prepared["candidate"]["candidate_id"],
            "artifact_readback_sha256":artifact["canonical_sha256"],
            "program_sha256":artifact["program_sha256"],
            "operator_catalog_sha256":artifact["operator_catalog_sha256"],
            "readback_config_sha256":artifact["readback_config_sha256"],
            "rest_frame_draft":{
                "schema_version":"MechanicalRestFrameDraft@1",
                "rest_frame_id":"mcp-rest",
                "coordinate_system":"forgecad-rh-y-up-m@1",
                "transform_convention":"column-vector-trs-quaternion@1",
                "root_link_id":"root-link",
                "links":[{"link_id":"root-link","part_id":"root-part","source_node_ids":["root-node"],"joint_type":"fixed","rest_translation_m":[0.0,0.0,0.0],"rest_rotation_quat_xyzw":[0.0,0.0,0.0,1.0],"axis_local":null,"limit_min":null,"limit_max":null,"value_unit":"none"}],
                "parent_map":[]
            },
            "pose_action_draft":null,
            "sample_time_ticks":0,
            "input_sha256":""
        });
        let mut preimage = arguments.clone();
        preimage.as_object_mut().unwrap().remove("input_sha256");
        arguments["input_sha256"] = Value::String(canonical_json_hash(&preimage));
        let response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":177,"method":"tools/call","params":{"name":"mechanical_pose_evaluate","arguments":arguments.clone()}}),
        )
        .expect("mechanical pose MCP response");
        assert_eq!(
            response["result"]["structuredContent"]["schema_version"],
            "MechanicalPoseEvaluationResult@1",
            "{response}"
        );
        assert_eq!(
            response["result"]["structuredContent"]["geometry_materialization"],
            "not-materialized"
        );
        let preview_tool = tools_with_writes(false)
            .into_iter()
            .find(|tool| tool["name"] == "mechanical_pose_geometry_preview")
            .expect("mechanical pose geometry preview tool");
        assert_eq!(preview_tool["annotations"]["readOnlyHint"], true);
        assert_eq!(preview_tool["inputSchema"]["additionalProperties"], false);
        let mut preview = json!({
            "schema_version":"MechanicalPoseGeometryPreviewRequest@1",
            "pose_evaluation_request":arguments.clone(),
            "preview_policy":"transient-derived-program-worker-readback@1",
            "input_sha256":""
        });
        let mut preview_preimage = preview.clone();
        preview_preimage
            .as_object_mut()
            .unwrap()
            .remove("input_sha256");
        preview["input_sha256"] = Value::String(canonical_json_hash(&preview_preimage));
        let preview_response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":181,"method":"tools/call","params":{"name":"mechanical_pose_geometry_preview","arguments":preview.clone()}}),
        )
        .expect("mechanical pose geometry preview MCP response");
        assert_eq!(
            preview_response["result"]["structuredContent"]["schema_version"],
            "MechanicalPoseGeometryPreview@1",
            "{preview_response}"
        );
        assert_eq!(
            preview_response["result"]["structuredContent"]["runtime_write_performed"],
            false
        );
        assert!(
            serde_json::to_vec(&preview_response).unwrap().len() <= READ_MODEL_MCP_WIRE_MAX_BYTES
        );
        let preview_summary: Value = serde_json::from_str(
            preview_response["result"]["content"][0]["text"]
                .as_str()
                .unwrap(),
        )
        .expect("preview summary JSON");
        assert_eq!(
            preview_summary["schema_version"],
            "MechanicalPoseGeometryPreviewMcpSummary@1"
        );
        assert!(preview_summary.get("posed_geometry_program").is_none());
        let mut invalid_preview = preview;
        invalid_preview["python"] = json!("bpy.ops");
        let invalid_preview_response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":182,"method":"tools/call","params":{"name":"mechanical_pose_geometry_preview","arguments":invalid_preview}}),
        )
        .expect("invalid preview schema response");
        assert_eq!(
            invalid_preview_response["error"]["data"]["code"],
            "INVALID_TOOL_PARAMS"
        );
        let mut sequence = arguments.clone();
        sequence["schema_version"] = json!("MechanicalPoseSequencePreviewRequest@1");
        sequence["sample_time_ticks"] = json!([0]);
        let mut preimage = sequence.clone();
        preimage.as_object_mut().unwrap().remove("input_sha256");
        sequence["input_sha256"] = Value::String(canonical_json_hash(&preimage));
        let sequence_response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":178,"method":"tools/call","params":{"name":"mechanical_pose_evaluate","arguments":sequence.clone()}}),
        )
        .expect("mechanical pose sequence MCP response");
        assert_eq!(
            sequence_response["result"]["structuredContent"]["schema_version"],
            "MechanicalPoseSequencePreview@1",
            "{sequence_response}"
        );
        assert_eq!(
            sequence_response["result"]["structuredContent"]["samples"][0]["evaluated_pose_sha256"],
            response["result"]["structuredContent"]["evaluated_pose_sha256"]
        );
        assert!(
            serde_json::to_vec(&sequence_response).unwrap().len() <= READ_MODEL_MCP_WIRE_MAX_BYTES,
            "mechanical pose sequence MCP response must remain inside the 1 MiB wire budget"
        );
        let content_summary: Value = serde_json::from_str(
            sequence_response["result"]["content"][0]["text"]
                .as_str()
                .expect("mechanical pose summary text"),
        )
        .expect("mechanical pose summary JSON");
        assert_eq!(
            content_summary["schema_version"],
            "MechanicalPoseMcpSummary@1"
        );
        assert_eq!(content_summary["structured_content_complete"], true);
        assert!(content_summary.get("samples").is_none());
        let mut duplicate_sequence = sequence.clone();
        duplicate_sequence["sample_time_ticks"] = json!([0, 0]);
        let mut preimage = duplicate_sequence.clone();
        preimage.as_object_mut().unwrap().remove("input_sha256");
        duplicate_sequence["input_sha256"] = Value::String(canonical_json_hash(&preimage));
        let duplicate_rejected = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":179,"method":"tools/call","params":{"name":"mechanical_pose_evaluate","arguments":duplicate_sequence}}),
        )
        .expect("duplicate sequence response");
        assert_eq!(duplicate_rejected["result"]["isError"], true);
        assert!(duplicate_rejected["result"]["structuredContent"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("strictly increasing and unique")));
        let mut unknown_sequence = sequence;
        unknown_sequence["rest_frame_draft"]["links"][0]["script"] = json!("bpy.ops");
        let sequence_rejected = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":180,"method":"tools/call","params":{"name":"mechanical_pose_evaluate","arguments":unknown_sequence}}),
        )
        .expect("invalid mechanical pose sequence schema response");
        assert_eq!(
            sequence_rejected["error"]["data"]["code"],
            "INVALID_TOOL_PARAMS"
        );
        let mut unknown = arguments;
        unknown["rest_frame_draft"]["links"][0]["script"] = json!("bpy.ops");
        let rejected = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":179,"method":"tools/call","params":{"name":"mechanical_pose_evaluate","arguments":unknown}}),
        )
        .expect("invalid mechanical pose schema response");
        assert_eq!(rejected["error"]["data"]["code"], "INVALID_TOOL_PARAMS");
        let candidate_count = match &backend {
            Backend::InProcess(runtime) => runtime
                .candidates(&project.project_id)
                .expect("candidates")
                .len(),
            _ => unreachable!("test backend"),
        };
        assert_eq!(
            candidate_count, 1,
            "pose evaluation must not create a candidate"
        );
    }

    #[test]
    fn mechanical_animation_clip_tools_are_closed_and_write_split() {
        let read_tools = tools_with_writes(false);
        assert!(read_tools
            .iter()
            .any(|tool| tool["name"] == "mechanical_animation_clip_get"));
        assert!(read_tools
            .iter()
            .any(|tool| tool["name"] == "mechanical_animation_clip_preview_get"));
        assert!(read_tools
            .iter()
            .any(|tool| tool["name"] == "game_asset_delivery_get"));
        let anchor_get = read_tools
            .iter()
            .find(|tool| tool["name"] == "game_weapon_anchor_get")
            .expect("weapon anchor get read tool");
        assert_eq!(anchor_get["annotations"]["readOnlyHint"], true);
        assert_eq!(anchor_get["inputSchema"]["additionalProperties"], false);
        let vfx_get = read_tools
            .iter()
            .find(|tool| tool["name"] == "fictional_energy_vfx_get")
            .expect("fictional energy VFX get read tool");
        assert_eq!(vfx_get["annotations"]["readOnlyHint"], true);
        assert_eq!(vfx_get["inputSchema"]["additionalProperties"], false);
        let vfx_frame = read_tools
            .iter()
            .find(|tool| tool["name"] == "fictional_energy_vfx_frame_sample")
            .expect("fictional energy VFX frame sample read tool");
        assert_eq!(vfx_frame["annotations"]["readOnlyHint"], true);
        assert_eq!(vfx_frame["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            vfx_frame["inputSchema"]["properties"]["sampling_policy"]["const"],
            "integer-tick-linear-once-clamp-loop-modulo-duration@1"
        );
        let vfx_appearance_frame = read_tools
            .iter()
            .find(|tool| tool["name"] == "fictional_energy_vfx_appearance_frame_sample")
            .expect("fictional energy VFX appearance frame sample read tool");
        assert_eq!(vfx_appearance_frame["annotations"]["readOnlyHint"], true);
        assert_eq!(
            vfx_appearance_frame["inputSchema"]["additionalProperties"],
            false
        );
        assert_eq!(
            vfx_appearance_frame["inputSchema"]["properties"]["appearance_binding_policy"]["const"],
            "three-lod-appearance-program-glb-material-zone-stable-id@1"
        );
        let vfx_rendered_frame_get = read_tools
            .iter()
            .find(|tool| tool["name"] == "fictional_energy_vfx_rendered_frame_get")
            .expect("fictional energy VFX rendered frame get read tool");
        assert_eq!(vfx_rendered_frame_get["annotations"]["readOnlyHint"], true);
        assert_eq!(
            vfx_rendered_frame_get["inputSchema"]["additionalProperties"],
            false
        );
        let vfx_rendered_sequence_get = read_tools
            .iter()
            .find(|tool| tool["name"] == "fictional_energy_vfx_rendered_sequence_get")
            .expect("fictional energy VFX rendered sequence get read tool");
        assert_eq!(
            vfx_rendered_sequence_get["annotations"]["readOnlyHint"],
            true
        );
        assert_eq!(
            vfx_rendered_sequence_get["inputSchema"]["additionalProperties"],
            false
        );
        let vfx_bloom_get = read_tools
            .iter()
            .find(|tool| tool["name"] == "fictional_energy_vfx_hdr_bloom_get")
            .expect("fictional energy VFX HDR bloom get read tool");
        assert_eq!(vfx_bloom_get["annotations"]["readOnlyHint"], true);
        assert_eq!(vfx_bloom_get["inputSchema"]["additionalProperties"], false);
        let appearance_source_get = read_tools
            .iter()
            .find(|tool| tool["name"] == "appearance_source_lineage_get")
            .expect("Appearance source lineage get read tool");
        assert_eq!(appearance_source_get["annotations"]["readOnlyHint"], true);
        assert_eq!(
            appearance_source_get["inputSchema"]["additionalProperties"],
            false
        );
        let lod_derive = read_tools
            .iter()
            .find(|tool| tool["name"] == "game_asset_lod_derive")
            .expect("automatic LOD derive read tool");
        assert_eq!(lod_derive["annotations"]["readOnlyHint"], true);
        assert_eq!(lod_derive["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            lod_derive["inputSchema"]["properties"]["derive_policy"]["const"],
            "runtime-owned-typed-segment-lowering-lod1-half-lod2-quarter@1"
        );
        assert!(!read_tools
            .iter()
            .any(|tool| tool["name"] == "mechanical_animation_clip_prepare"));
        assert!(!read_tools
            .iter()
            .any(|tool| tool["name"] == "mechanical_animation_glb_prepare"));
        assert!(!read_tools
            .iter()
            .any(|tool| tool["name"] == "game_asset_delivery_prepare"));
        assert!(!read_tools
            .iter()
            .any(|tool| tool["name"] == "game_weapon_anchor_prepare"));
        assert!(!read_tools
            .iter()
            .any(|tool| tool["name"] == "fictional_energy_vfx_prepare"));
        assert!(!read_tools
            .iter()
            .any(|tool| tool["name"] == "fictional_energy_vfx_rendered_frame_prepare"));
        assert!(!read_tools
            .iter()
            .any(|tool| { tool["name"] == "fictional_energy_vfx_rendered_sequence_prepare" }));
        assert!(!read_tools
            .iter()
            .any(|tool| tool["name"] == "fictional_energy_vfx_hdr_bloom_prepare"));
        assert!(!read_tools
            .iter()
            .any(|tool| tool["name"] == "appearance_source_lineage_prepare"));
        assert!(!is_write_tool("mechanical_animation_clip_get"));
        assert!(!is_write_tool("mechanical_animation_clip_preview_get"));
        assert!(!is_write_tool("game_asset_delivery_get"));
        assert!(!is_write_tool("game_asset_lod_derive"));
        assert!(!is_write_tool("game_weapon_anchor_get"));
        assert!(!is_write_tool("fictional_energy_vfx_get"));
        assert!(!is_write_tool("fictional_energy_vfx_frame_sample"));
        assert!(!is_write_tool(
            "fictional_energy_vfx_appearance_frame_sample"
        ));
        assert!(!is_write_tool("fictional_energy_vfx_rendered_frame_get"));
        assert!(!is_write_tool("fictional_energy_vfx_rendered_sequence_get"));
        assert!(!is_write_tool("fictional_energy_vfx_hdr_bloom_get"));
        assert!(!is_write_tool("appearance_source_lineage_get"));
        assert!(is_write_tool("mechanical_animation_clip_prepare"));
        assert!(is_write_tool("mechanical_animation_glb_prepare"));
        assert!(is_write_tool("game_asset_delivery_prepare"));
        assert!(is_write_tool("game_weapon_anchor_prepare"));
        assert!(is_write_tool("fictional_energy_vfx_prepare"));
        assert!(is_write_tool("fictional_energy_vfx_rendered_frame_prepare"));
        assert!(is_write_tool(
            "fictional_energy_vfx_rendered_sequence_prepare"
        ));
        assert!(is_write_tool("fictional_energy_vfx_hdr_bloom_prepare"));
        assert!(is_write_tool("appearance_source_lineage_prepare"));

        let write_tools = tools_with_writes(true);
        let prepare_tool = write_tools
            .iter()
            .find(|tool| tool["name"] == "mechanical_animation_clip_prepare")
            .expect("clip prepare write tool");
        assert_eq!(prepare_tool["annotations"]["readOnlyHint"], false);
        assert_eq!(prepare_tool["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            prepare_tool["inputSchema"]["properties"]["clip_policy"]["const"],
            "runtime-owned-immutable-cas-rigid-mechanical-action@1"
        );
        assert_eq!(
            prepare_tool["inputSchema"]["properties"]["pose_sequence_request"]["properties"]
                ["sample_time_ticks"]["maxItems"],
            16
        );
        assert_eq!(prepare_tool["_meta"]["forgecad"]["transaction"], "MCP010F");

        let glb_tool = write_tools
            .iter()
            .find(|tool| tool["name"] == "mechanical_animation_glb_prepare")
            .expect("animation GLB prepare write tool");
        assert_eq!(glb_tool["annotations"]["readOnlyHint"], false);
        let anchor_prepare = write_tools
            .iter()
            .find(|tool| tool["name"] == "game_weapon_anchor_prepare")
            .expect("weapon anchor prepare write tool");
        assert_eq!(anchor_prepare["annotations"]["readOnlyHint"], false);
        assert_eq!(anchor_prepare["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            anchor_prepare["inputSchema"]["properties"]["anchor_policy"]["const"],
            "weapon-rh-x-forward-y-up-model-space-six-role@1"
        );
        let vfx_prepare = write_tools
            .iter()
            .find(|tool| tool["name"] == "fictional_energy_vfx_prepare")
            .expect("fictional energy VFX prepare write tool");
        assert_eq!(vfx_prepare["annotations"]["readOnlyHint"], false);
        assert_eq!(vfx_prepare["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            vfx_prepare["inputSchema"]["properties"]["vfx_policy"]["const"],
            "fictional-energy-two-effect-time-sampled-emissive-intent@1"
        );
        let vfx_frame_prepare = write_tools
            .iter()
            .find(|tool| tool["name"] == "fictional_energy_vfx_rendered_frame_prepare")
            .expect("fictional energy VFX rendered frame prepare write tool");
        assert_eq!(vfx_frame_prepare["annotations"]["readOnlyHint"], false);
        assert_eq!(
            vfx_frame_prepare["inputSchema"]["properties"]["render_policy"]["const"],
            "lod0-nine-aov-double-worker-byte-exact-reservation-safe@1"
        );
        assert_eq!(
            vfx_frame_prepare["inputSchema"]["properties"]["effect_materialization_policy"]
                ["const"],
            "independent-effect-material-zone@1"
        );
        let vfx_sequence_prepare = write_tools
            .iter()
            .find(|tool| tool["name"] == "fictional_energy_vfx_rendered_sequence_prepare")
            .expect("fictional energy VFX rendered sequence prepare write tool");
        assert_eq!(vfx_sequence_prepare["annotations"]["readOnlyHint"], false);
        assert_eq!(
            vfx_sequence_prepare["inputSchema"]["additionalProperties"],
            false
        );
        assert_eq!(
            vfx_sequence_prepare["inputSchema"]["properties"]["sample_time_ticks"]["minItems"],
            2
        );
        assert_eq!(
            vfx_sequence_prepare["inputSchema"]["properties"]["sample_time_ticks"]["maxItems"],
            16
        );
        let vfx_bloom_prepare = write_tools
            .iter()
            .find(|tool| tool["name"] == "fictional_energy_vfx_hdr_bloom_prepare")
            .expect("fictional energy VFX HDR bloom prepare write tool");
        assert_eq!(vfx_bloom_prepare["annotations"]["readOnlyHint"], false);
        assert_eq!(
            vfx_bloom_prepare["inputSchema"]["additionalProperties"],
            false
        );
        assert_eq!(
            vfx_bloom_prepare["inputSchema"]["properties"]["bloom_profile"]["properties"]
                ["radius_px"]["const"],
            8
        );
        let appearance_source_prepare = write_tools
            .iter()
            .find(|tool| tool["name"] == "appearance_source_lineage_prepare")
            .expect("Appearance source lineage prepare write tool");
        assert_eq!(
            appearance_source_prepare["annotations"]["readOnlyHint"],
            false
        );
        assert_eq!(
            appearance_source_prepare["inputSchema"]["additionalProperties"],
            false
        );
        assert_eq!(
            appearance_source_prepare["_meta"]["forgecad"]["transaction"],
            "MCP010F"
        );
        assert_eq!(glb_tool["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            glb_tool["inputSchema"]["properties"]["materialization_policy"]["const"],
            "rigid-node-trs-gltf-linear-scheduled-samples@1"
        );
        assert!(glb_tool["inputSchema"]["properties"]
            .get("candidate_state_sha256")
            .is_some());

        let delivery_tool = write_tools
            .iter()
            .find(|tool| tool["name"] == "game_asset_delivery_prepare")
            .expect("game asset delivery prepare write tool");
        assert_eq!(delivery_tool["annotations"]["readOnlyHint"], false);
        assert_eq!(delivery_tool["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            delivery_tool["inputSchema"]["properties"]["lods"]["minItems"],
            3
        );
        assert_eq!(
            delivery_tool["inputSchema"]["properties"]["collision_policy"]["const"],
            "per-part-aabb-box-from-lod2-visual-geometry@1"
        );

        let sha = "a".repeat(64);
        let mut prepare = json!({
            "schema_version":"MechanicalAnimationClipPrepareRequest@1",
            "clip_id":"fixture-clip",
            "pose_sequence_request":{
                "schema_version":"MechanicalPoseSequencePreviewRequest@1",
                "project_id":"project-fixture",
                "artifact_id":sha,
                "candidate_id":"candidate-fixture",
                "artifact_readback_sha256":"b".repeat(64),
                "program_sha256":"c".repeat(64),
                "operator_catalog_sha256":"d".repeat(64),
                "readback_config_sha256":"e".repeat(64),
                "rest_frame_draft":{
                    "schema_version":"MechanicalRestFrameDraft@1",
                    "rest_frame_id":"rest-fixture",
                    "coordinate_system":"forgecad-rh-y-up-m@1",
                    "transform_convention":"column-vector-trs-quaternion@1",
                    "root_link_id":"root-link",
                    "links":[{"link_id":"root-link","part_id":"root-part","source_node_ids":["root-node"],"joint_type":"fixed","rest_translation_m":[0.0,0.0,0.0],"rest_rotation_quat_xyzw":[0.0,0.0,0.0,1.0],"axis_local":null,"limit_min":null,"limit_max":null,"value_unit":"none"}],
                    "parent_map":[]
                },
                "pose_action_draft":{
                    "schema_version":"MechanicalPoseActionDraft@1",
                    "action_id":"action-fixture",
                    "timebase_hz":1000,
                    "duration_ticks":1000,
                    "interpolation":"linear@1",
                    "extrapolation":"clamp@1",
                    "unkeyed_policy":"rest@1",
                    "channels":[{"link_id":"root-link","value_unit":"radian","keys":[{"time_ticks":0,"value":0.0}]}]
                },
                "sample_time_ticks":[0],
                "input_sha256":"f".repeat(64)
            },
            "clip_policy":"runtime-owned-immutable-cas-rigid-mechanical-action@1",
            "input_sha256":"1".repeat(64)
        });
        assert!(
            validate_declared_tool_input("mechanical_animation_clip_prepare", &prepare, true)
                .is_ok()
        );
        prepare["pose_sequence_request"]["pose_action_draft"]["python"] =
            json!("bpy.ops.object.modifier_add()");
        assert!(
            validate_declared_tool_input("mechanical_animation_clip_prepare", &prepare, true)
                .is_err()
        );

        let get = json!({
            "schema_version":"MechanicalAnimationClipGetRequest@1",
            "project_id":"project-fixture",
            "candidate_id":"candidate-fixture",
            "clip_id":"fixture-clip",
            "canonical_sha256":"2".repeat(64)
        });
        assert!(validate_declared_tool_input("mechanical_animation_clip_get", &get, false).is_ok());
        let mut glb_prepare = json!({
            "schema_version":"MechanicalAnimationGlbPrepareRequest@1",
            "project_id":"project-fixture",
            "candidate_id":"candidate-fixture",
            "candidate_state_sha256":"4".repeat(64),
            "clip_id":"fixture-clip",
            "materialization_policy":"rigid-node-trs-gltf-linear-scheduled-samples@1",
            "canonical_sha256":"5".repeat(64)
        });
        assert!(validate_declared_tool_input(
            "mechanical_animation_glb_prepare",
            &glb_prepare,
            true
        )
        .is_ok());
        glb_prepare["python"] = json!("bpy.ops.export_scene.gltf()");
        assert!(validate_declared_tool_input(
            "mechanical_animation_glb_prepare",
            &glb_prepare,
            true
        )
        .is_err());
        let mut preview = json!({
            "schema_version":"MechanicalAnimationClipPreviewRequest@1",
            "project_id":"project-fixture",
            "candidate_id":"candidate-fixture",
            "clip_id":"fixture-clip",
            "sample_time_ticks":500,
            "preview_policy":"single-tick-transient-double-worker-replay@1",
            "canonical_sha256":"3".repeat(64)
        });
        assert!(validate_declared_tool_input(
            "mechanical_animation_clip_preview_get",
            &preview,
            false
        )
        .is_ok());
        preview["script"] = json!("python.exec");
        assert!(validate_declared_tool_input(
            "mechanical_animation_clip_preview_get",
            &preview,
            false
        )
        .is_err());

        let (mut backend, mut session) = initialized();
        let disabled = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":1801,
                "method":"tools/call",
                "params":{"name":"mechanical_animation_clip_prepare","arguments":{}}
            }),
        )
        .expect("clip prepare disabled response");
        assert_eq!(disabled["result"]["isError"], true);
        assert_eq!(
            disabled["result"]["structuredContent"]["code"],
            "MCP010F_MECHANICAL_ANIMATION_CLIP_WRITE_TOOLS_DISABLED"
        );

        session.write_tools_enabled = true;
        let enabled_dispatch = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":1802,
                "method":"tools/call",
                "params":{"name":"mechanical_animation_clip_prepare","arguments":{}}
            }),
        )
        .expect("enabled clip prepare validation response");
        assert_eq!(
            enabled_dispatch["error"]["data"]["code"],
            "INVALID_TOOL_PARAMS"
        );
    }

    #[test]
    fn fictional_energy_vfx_trails_tools_are_closed_write_split_and_summarized() {
        let read_tools = tools_with_writes(false);
        let enabled_tools = tools_with_writes(true);
        let trail_get = read_tools
            .iter()
            .find(|tool| tool["name"] == "fictional_energy_vfx_trails_get")
            .expect("typed trails get read tool");
        assert_eq!(trail_get["annotations"]["readOnlyHint"], true);
        assert_eq!(trail_get["annotations"]["destructiveHint"], false);
        assert_eq!(trail_get["annotations"]["idempotentHint"], true);
        assert_eq!(trail_get["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            trail_get["inputSchema"]["required"],
            json!(["schema_version", "project_id", "trail_key_sha256"])
        );
        assert!(!read_tools
            .iter()
            .any(|tool| tool["name"] == "fictional_energy_vfx_trails_prepare"));
        assert!(!is_write_tool("fictional_energy_vfx_trails_get"));

        let trail_prepare = enabled_tools
            .iter()
            .find(|tool| tool["name"] == "fictional_energy_vfx_trails_prepare")
            .expect("typed trails prepare write tool");
        assert_eq!(trail_prepare["annotations"]["readOnlyHint"], false);
        assert_eq!(trail_prepare["annotations"]["destructiveHint"], false);
        assert_eq!(trail_prepare["annotations"]["idempotentHint"], true);
        assert_eq!(trail_prepare["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            trail_prepare["_meta"]["forgecad"]["requiresConfirmation"],
            true
        );
        assert_eq!(trail_prepare["_meta"]["forgecad"]["transaction"], "MCP010F");
        assert!(is_write_tool("fictional_energy_vfx_trails_prepare"));

        let hash = |byte: char| byte.to_string().repeat(64);
        let mut prepare_arguments = json!({
            "schema_version":"FictionalEnergyVfxTrailsFrameRenderPrepareRequest@1",
            "project_id":"project-trails-mcp",
            "delivery_manifest_object_sha256":hash('a'),
            "vfx_profile_object_sha256":hash('b'),
            "anchor_set_object_sha256":hash('c'),
            "base_frame_key_sha256":hash('d'),
            "bloom_key_sha256":hash('e'),
            "current_particle_key_sha256":hash('f'),
            "particle_history_key_sha256s":[hash('1')],
            "sample_time_ticks":50,
            "trail_policy":"two-closed-history-bound-polyline-trails@1",
            "history_policy":"one-to-four-strictly-earlier-particle-frames@1",
            "render_policy":"lod0-three-typed-trail-aov-depth-tested-base-bloom-particles-byte-exact-no-bloom-input@1",
            "bloom_input":false,
            "canonical_sha256":hash('0')
        });
        assert!(validate_declared_tool_input(
            "fictional_energy_vfx_trails_prepare",
            &prepare_arguments,
            true
        )
        .is_ok());
        prepare_arguments["script"] = json!("bpy.ops");
        assert!(validate_declared_tool_input(
            "fictional_energy_vfx_trails_prepare",
            &prepare_arguments,
            true
        )
        .is_err());
        let get_arguments = json!({
            "schema_version":"FictionalEnergyVfxTrailsFrameGetRequest@1",
            "project_id":"project-trails-mcp",
            "trail_key_sha256":hash('9')
        });
        assert!(validate_declared_tool_input(
            "fictional_energy_vfx_trails_get",
            &get_arguments,
            false
        )
        .is_ok());
        let mut unknown_get = get_arguments.clone();
        unknown_get["unexpected"] = json!(true);
        assert!(validate_declared_tool_input(
            "fictional_energy_vfx_trails_get",
            &unknown_get,
            false
        )
        .is_err());

        let (mut backend, mut session) = initialized();
        let disabled = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":1901,
                "method":"tools/call",
                "params":{"name":"fictional_energy_vfx_trails_prepare","arguments":{}}
            }),
        )
        .expect("typed trails disabled response");
        assert_eq!(disabled["result"]["isError"], true);
        assert_eq!(
            disabled["result"]["structuredContent"]["code"],
            "MCP010F_WRITE_TOOLS_DISABLED"
        );

        session.write_tools_enabled = true;
        let invalid_enabled = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":1902,
                "method":"tools/call",
                "params":{"name":"fictional_energy_vfx_trails_prepare","arguments":{}}
            }),
        )
        .expect("typed trails enabled schema response");
        assert_eq!(
            invalid_enabled["error"]["data"]["code"],
            "INVALID_TOOL_PARAMS"
        );

        let receipt = json!({
            "typed_trails_rendered":true,
            "trail_count":2,
            "segment_count":2,
            "current_particle_key_sha256":hash('f'),
            "particle_history_key_sha256s":[hash('1')],
            "history_time_ticks":[40],
            "pass_artifacts":[
                {"pass":"trail-color","object_sha256":hash('2')},
                {"pass":"trail-id","object_sha256":hash('3')},
                {"pass":"trail-depth","object_sha256":hash('4')}
            ],
            "base_aov_byte_exact_verified":true,
            "bloom_pass_byte_exact_reused":true,
            "particle_passes_byte_exact_reused":true
        });
        for (name, expected_write) in [
            ("fictional_energy_vfx_trails_prepare", true),
            ("fictional_energy_vfx_trails_get", false),
        ] {
            let summary = fictional_energy_vfx_mcp_summary(name, &json!({"receipt":receipt}))
                .expect("typed trails summary");
            let summary: Value = serde_json::from_str(&summary).expect("summary JSON");
            assert_eq!(summary["schema_version"], "FictionalEnergyVfxMcpSummary@1");
            assert_eq!(summary["operation"], name);
            assert_eq!(summary["runtime_write_performed"], expected_write);
            assert_eq!(summary["trails_rendered"], true);
            assert_eq!(summary["particles_rendered"], false);
            assert_eq!(summary["trail_count"], 2);
            assert_eq!(summary["segment_count"], 2);
            assert_eq!(summary["current_particle_key_sha256"], hash('f'));
            assert_eq!(summary["particle_history_key_sha256s"], json!([hash('1')]));
            assert_eq!(summary["history_time_ticks"], json!([40]));
            assert_eq!(summary["trail_passes"].as_array().unwrap().len(), 3);
            assert_eq!(summary["base_aov_byte_exact_verified"], true);
            assert_eq!(summary["bloom_pass_byte_exact_reused"], true);
            assert_eq!(summary["particle_passes_byte_exact_reused"], true);
            assert_eq!(summary["structured_content_complete"], true);
        }
    }

    #[test]
    fn game_weapon_glb_socket_tools_are_closed_write_split_and_truthful() {
        let read_tools = tools_with_writes(false);
        let socket_get = read_tools
            .iter()
            .find(|tool| tool["name"] == "game_weapon_glb_socket_get")
            .expect("GLB socket get read tool");
        assert_eq!(socket_get["annotations"]["readOnlyHint"], true);
        assert_eq!(socket_get["annotations"]["destructiveHint"], false);
        assert_eq!(socket_get["annotations"]["idempotentHint"], true);
        assert_eq!(socket_get["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            socket_get["inputSchema"]["required"],
            json!([
                "schema_version",
                "project_id",
                "socket_materialization_key_sha256"
            ])
        );
        assert_eq!(
            socket_get["inputSchema"]["properties"]["schema_version"]["const"],
            "GameWeaponGlbSocketMaterializationGetRequest@1"
        );
        assert!(!is_write_tool("game_weapon_glb_socket_get"));

        let write_tools = tools_with_writes(true);
        let socket_prepare = write_tools
            .iter()
            .find(|tool| tool["name"] == "game_weapon_glb_socket_prepare")
            .expect("GLB socket prepare write tool");
        assert_eq!(socket_prepare["annotations"]["readOnlyHint"], false);
        assert_eq!(socket_prepare["annotations"]["destructiveHint"], false);
        assert_eq!(socket_prepare["annotations"]["idempotentHint"], true);
        assert_eq!(socket_prepare["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            socket_prepare["inputSchema"]["required"],
            json!([
                "schema_version",
                "project_id",
                "delivery_manifest_object_sha256",
                "anchor_set_object_sha256",
                "materialization_policy",
                "lod_scope",
                "canonical_sha256"
            ])
        );
        assert_eq!(
            socket_prepare["inputSchema"]["properties"]["materialization_policy"]["const"],
            "gltf-anchor-node-materialization-preserve-renderable-content@1"
        );
        assert_eq!(
            socket_prepare["inputSchema"]["properties"]["lod_scope"]["const"],
            "lod0-lod1-lod2@1"
        );
        assert_eq!(
            socket_prepare["_meta"]["forgecad"]["requiresConfirmation"],
            true
        );
        assert_eq!(
            socket_prepare["_meta"]["forgecad"]["transaction"],
            "MCP010F"
        );
        assert!(is_write_tool("game_weapon_glb_socket_prepare"));

        let hash = |byte: char| byte.to_string().repeat(64);
        let prepare_arguments = json!({
            "schema_version":"GameWeaponGlbSocketMaterializationPrepareRequest@1",
            "project_id":"project-socket-mcp",
            "delivery_manifest_object_sha256":hash('a'),
            "anchor_set_object_sha256":hash('b'),
            "materialization_policy":"gltf-anchor-node-materialization-preserve-renderable-content@1",
            "lod_scope":"lod0-lod1-lod2@1",
            "canonical_sha256":hash('c')
        });
        assert!(validate_declared_tool_input(
            "game_weapon_glb_socket_prepare",
            &prepare_arguments,
            true
        )
        .is_ok());
        let get_arguments = json!({
            "schema_version":"GameWeaponGlbSocketMaterializationGetRequest@1",
            "project_id":"project-socket-mcp",
            "socket_materialization_key_sha256":hash('d')
        });
        assert!(
            validate_declared_tool_input("game_weapon_glb_socket_get", &get_arguments, false)
                .is_ok()
        );

        let (mut backend, mut session) = initialized();
        let disabled = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":1961,
                "method":"tools/call",
                "params":{"name":"game_weapon_glb_socket_prepare","arguments":{}}
            }),
        )
        .expect("GLB socket disabled response");
        assert_eq!(disabled["result"]["isError"], true);
        assert_eq!(
            disabled["result"]["structuredContent"]["code"],
            "MCP010F_WRITE_TOOLS_DISABLED"
        );

        let lods = (0..3)
            .map(|lod| {
                json!({
                    "lod_level":lod,
                    "source_artifact_sha256":hash((b'e' + lod as u8) as char),
                    "source_artifact_readback_sha256":hash('1'),
                    "derived_artifact_sha256":hash('2'),
                    "derived_artifact_readback_sha256":hash('3'),
                    "source_renderable_inventory_sha256":hash('4'),
                    "derived_renderable_inventory_sha256":hash('5'),
                    "socket_node_inventory_sha256":hash('6'),
                    "source_node_count":10,
                    "derived_node_count":16,
                    "source_renderable_projection_exact":true,
                    "source_bin_byte_exact":true,
                    "socket_nodes_materialized":true,
                    "socket_node_count":6
                })
            })
            .collect::<Vec<_>>();
        let summary = game_weapon_glb_socket_mcp_summary(
            "game_weapon_glb_socket_get",
            &json!({
                "socket_materialization_key_sha256":hash('7'),
                "receipt":{
                    "socket_materialization_key_sha256":hash('7'),
                    "levels":lods.clone(),
                    "runtime_write_performed":true,
                    "candidate_confirmed":false,
                    "export_performed":false,
                    "actual_engine_roundtrip":false,
                    "quality_status":"structural_only"
                },
                "link":{
                    "receipt_object_sha256":hash('8'),
                    "delivery_manifest_object_sha256":hash('9'),
                    "anchor_set_object_sha256":hash('a'),
                    "anchor_set_canonical_sha256":hash('b'),
                    "request_sha256":hash('7'),
                    "socket_materialization_policy":"gltf-anchor-node-materialization-preserve-renderable-content@1",
                    "lod_scope":"lod0-lod1-lod2@1",
                    "socket_node_id_encoding_sha256":hash('c')
                },
                "levels":lods
            }),
        )
        .expect("GLB socket summary");
        let summary: Value = serde_json::from_str(&summary).expect("summary JSON");
        assert_eq!(summary["lod_readback"].as_array().unwrap().len(), 3);
        assert_eq!(summary["socket_node_count"], 6);
        assert_eq!(summary["socket_node_counts"], json!([6, 6, 6]));
        assert_eq!(summary["source_renderable_projection_exact"], true);
        assert_eq!(summary["source_bin_byte_exact"], true);
        assert_eq!(summary["socket_nodes_materialized"], true);
        assert_eq!(summary["restart_hash_verified"], true);
        assert_eq!(summary["runtime_write_performed"], false);
        assert_eq!(summary["candidate_confirmed"], false);
        assert_eq!(summary["export_performed"], false);
        assert_eq!(summary["actual_engine_roundtrip"], false);
        assert_eq!(summary["quality_status"], "structural_only");
        assert_eq!(summary["glb_bytes_in_summary"], false);
        assert!(summary["lod_readback"][0].get("socket_nodes").is_none());
        assert!(serde_json::to_string(&summary)
            .expect("summary serializes")
            .find("glb_bytes")
            .is_some());
    }

    #[test]
    fn fictional_energy_vfx_animated_socket_attachment_tools_are_closed_write_split_and_truthful() {
        let read_tools = tools_with_writes(false);
        let attachment_get = read_tools
            .iter()
            .find(|tool| tool["name"] == "fictional_energy_vfx_animated_socket_attachment_get")
            .expect("animated socket attachment get read tool");
        assert_eq!(attachment_get["annotations"]["readOnlyHint"], true);
        assert_eq!(attachment_get["annotations"]["writeIntent"], false);
        assert_eq!(attachment_get["annotations"]["approvalRequired"], false);
        assert_eq!(attachment_get["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            attachment_get["inputSchema"]["required"],
            json!([
                "schema_version",
                "attachment_key_sha256",
                "project_id",
                "candidate_id"
            ])
        );
        assert_eq!(
            attachment_get["inputSchema"]["properties"]["schema_version"]["const"],
            "FictionalEnergyVfxAnimatedSocketAttachmentGetRequest@1"
        );
        assert!(!is_write_tool(
            "fictional_energy_vfx_animated_socket_attachment_get"
        ));

        let write_tools = tools_with_writes(true);
        let attachment_prepare = write_tools
            .iter()
            .find(|tool| tool["name"] == "fictional_energy_vfx_animated_socket_attachment_prepare")
            .expect("animated socket attachment prepare write tool");
        assert_eq!(attachment_prepare["annotations"]["readOnlyHint"], false);
        assert_eq!(attachment_prepare["annotations"]["writeIntent"], true);
        assert_eq!(attachment_prepare["annotations"]["approvalRequired"], false);
        assert_eq!(
            attachment_prepare["inputSchema"]["additionalProperties"],
            false
        );
        assert_eq!(
            attachment_prepare["inputSchema"]["properties"]["attachment_policy"]["const"],
            "fictional-energy-vfx-animated-socket-attachment-structural-only@1"
        );
        assert_eq!(
            attachment_prepare["inputSchema"]["properties"]["frame_scope"]["const"],
            "lod0-animation-vfx-frame-range-1-16@1"
        );
        assert!(is_write_tool(
            "fictional_energy_vfx_animated_socket_attachment_prepare"
        ));

        let hash = |byte: char| byte.to_string().repeat(64);
        let prepare_arguments = json!({
            "schema_version":"FictionalEnergyVfxAnimatedSocketAttachmentPrepareRequest@1",
            "attachment_key_sha256":hash('a'),
            "project_id":"project-attachment-mcp",
            "delivery_manifest_object_sha256":hash('b'),
            "candidate_id":"candidate-attachment-mcp",
            "candidate_state_sha256":hash('c'),
            "source_artifact_sha256":hash('d'),
            "animated_socket_materialization_key_sha256":hash('e'),
            "animated_socket_anchor_set_object_sha256":hash('f'),
            "animated_socket_anchor_set_canonical_sha256":hash('0'),
            "animation_clip_id":"clip-attachment-mcp",
            "animation_clip_object_sha256":hash('1'),
            "animation_clip_canonical_sha256":hash('2'),
            "animated_artifact_sha256":hash('3'),
            "animation_receipt_object_sha256":hash('4'),
            "animation_receipt_canonical_sha256":hash('5'),
            "vfx_profile_object_sha256":hash('6'),
            "vfx_profile_canonical_sha256":hash('7'),
            "vfx_sequence_key_sha256":hash('8'),
            "vfx_sequence_canonical_sha256":hash('9'),
            "attachment_policy":"fictional-energy-vfx-animated-socket-attachment-structural-only@1",
            "socket_node_id_encoding_sha256":hash('a'),
            "socket_roles_sha256":hash('b'),
            "frame_scope":"lod0-animation-vfx-frame-range-1-16@1",
            "input_sha256":hash('c'),
            "idempotency_key":"attachment-idempotency-mcp"
        });
        assert!(validate_declared_tool_input(
            "fictional_energy_vfx_animated_socket_attachment_prepare",
            &prepare_arguments,
            true
        )
        .is_ok());
        let mut unknown = prepare_arguments.clone();
        unknown["script"] = json!("bpy.ops");
        assert!(validate_declared_tool_input(
            "fictional_energy_vfx_animated_socket_attachment_prepare",
            &unknown,
            true
        )
        .is_err());
        let get_arguments = json!({
            "schema_version":"FictionalEnergyVfxAnimatedSocketAttachmentGetRequest@1",
            "attachment_key_sha256":hash('d'),
            "project_id":"project-attachment-mcp",
            "candidate_id":"candidate-attachment-mcp"
        });
        assert!(validate_declared_tool_input(
            "fictional_energy_vfx_animated_socket_attachment_get",
            &get_arguments,
            false
        )
        .is_ok());

        let (mut backend, mut session) = initialized();
        let disabled = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":1963,
                "method":"tools/call",
                "params":{"name":"fictional_energy_vfx_animated_socket_attachment_prepare","arguments":{}}
            }),
        )
        .expect("animated socket attachment disabled response");
        assert_eq!(disabled["result"]["isError"], true);
        assert_eq!(
            disabled["result"]["structuredContent"]["code"],
            "AGENTIC_WRITE_TOOLS_DISABLED"
        );

        let summary = fictional_energy_vfx_animated_socket_attachment_mcp_summary(
            "fictional_energy_vfx_animated_socket_attachment_get",
            &json!({
                "attachment_key_sha256":hash('e'),
                "runtime_write":false,
                "restart_hash_verified":true,
                "quality_status":"structural_only",
                "visual_quality_status":"NOT_PROVEN",
                "commercial_fps_quality_status":"NOT_PROVEN",
                "human_review_status":"NOT_RUN",
                "commercial_engine_status":"NOT_RUN",
                "attachment":{
                    "attachment_key_sha256":hash('e'),
                    "project_id":"project-attachment-mcp",
                    "candidate_id":"candidate-attachment-mcp",
                    "attachment_policy":"fictional-energy-vfx-animated-socket-attachment-structural-only@1",
                    "frame_scope":"lod0-animation-vfx-frame-range-1-16@1",
                    "frames":[{
                        "frame_index":0,
                        "sample_time_ticks":0,
                        "animation_pose_readback_sha256":hash('f'),
                        "socket_transform_inventory_sha256":hash('0'),
                        "socket_transform_readback_sha256":hash('1'),
                        "emitter_socket_bindings_sha256":hash('2'),
                        "trail_socket_bindings_sha256":hash('3'),
                        "base_frame_key_sha256":hash('4'),
                        "bloom_key_sha256":hash('5'),
                        "particle_key_sha256":hash('6'),
                        "trail_key_sha256":hash('7'),
                        "trail_bloom_key_sha256":hash('8'),
                        "canonical_sha256":hash('9')
                    }]
                }
            }),
        )
        .expect("animated socket attachment summary");
        let summary: Value = serde_json::from_str(&summary).expect("summary JSON");
        assert_eq!(
            summary["schema_version"],
            "FictionalEnergyVfxAnimatedSocketAttachmentMcpSummary@1"
        );
        assert_eq!(summary["frame_count"], 1);
        assert_eq!(summary["runtime_write_performed"], false);
        assert_eq!(summary["quality_status"], "structural_only");
        assert_eq!(summary["glb_bytes_in_summary"], false);
        assert_eq!(summary["png_bytes_in_summary"], false);
        assert_eq!(summary["aov_bytes_in_summary"], false);
        assert!(summary["frames"][0].get("glb_bytes").is_none());
    }

    #[test]
    fn game_weapon_animated_glb_socket_tools_are_closed_write_split_and_truthful() {
        let read_tools = tools_with_writes(false);
        let animated_get = read_tools
            .iter()
            .find(|tool| tool["name"] == "game_weapon_animated_glb_socket_get")
            .expect("animated GLB socket get read tool");
        assert_eq!(animated_get["annotations"]["readOnlyHint"], true);
        assert_eq!(animated_get["annotations"]["destructiveHint"], false);
        assert_eq!(animated_get["annotations"]["idempotentHint"], true);
        assert_eq!(animated_get["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            animated_get["inputSchema"]["required"],
            json!([
                "schema_version",
                "project_id",
                "animated_socket_materialization_key_sha256"
            ])
        );
        assert_eq!(
            animated_get["inputSchema"]["properties"]["schema_version"]["const"],
            "GameWeaponAnimatedGlbSocketMaterializationGetRequest@1"
        );
        assert!(!is_write_tool("game_weapon_animated_glb_socket_get"));

        let write_tools = tools_with_writes(true);
        let animated_prepare = write_tools
            .iter()
            .find(|tool| tool["name"] == "game_weapon_animated_glb_socket_prepare")
            .expect("animated GLB socket prepare write tool");
        assert_eq!(animated_prepare["annotations"]["readOnlyHint"], false);
        assert_eq!(animated_prepare["annotations"]["destructiveHint"], false);
        assert_eq!(animated_prepare["annotations"]["idempotentHint"], true);
        assert_eq!(
            animated_prepare["inputSchema"]["additionalProperties"],
            false
        );
        assert_eq!(
            animated_prepare["inputSchema"]["required"],
            json!([
                "schema_version",
                "project_id",
                "delivery_manifest_object_sha256",
                "anchor_set_object_sha256",
                "source_candidate_id",
                "source_candidate_state_sha256",
                "source_animated_artifact_sha256",
                "source_animation_receipt_object_sha256",
                "materialization_policy",
                "canonical_sha256"
            ])
        );
        assert_eq!(
            animated_prepare["inputSchema"]["properties"]["materialization_policy"]["const"],
            "gltf-animated-anchor-node-materialization-preserve-animations-renderable-content@1"
        );
        assert_eq!(
            animated_prepare["_meta"]["forgecad"]["requiresConfirmation"],
            true
        );
        assert_eq!(
            animated_prepare["_meta"]["forgecad"]["transaction"],
            "MCP010F"
        );
        assert!(is_write_tool("game_weapon_animated_glb_socket_prepare"));

        let hash = |byte: char| byte.to_string().repeat(64);
        let prepare_arguments = json!({
            "schema_version":"GameWeaponAnimatedGlbSocketMaterializationPrepareRequest@1",
            "project_id":"project-animated-socket-mcp",
            "delivery_manifest_object_sha256":hash('a'),
            "anchor_set_object_sha256":hash('b'),
            "source_candidate_id":"candidate-animated-socket-mcp",
            "source_candidate_state_sha256":hash('c'),
            "source_animated_artifact_sha256":hash('d'),
            "source_animation_receipt_object_sha256":hash('e'),
            "materialization_policy":"gltf-animated-anchor-node-materialization-preserve-animations-renderable-content@1",
            "canonical_sha256":hash('f')
        });
        assert!(validate_declared_tool_input(
            "game_weapon_animated_glb_socket_prepare",
            &prepare_arguments,
            true
        )
        .is_ok());
        let get_arguments = json!({
            "schema_version":"GameWeaponAnimatedGlbSocketMaterializationGetRequest@1",
            "project_id":"project-animated-socket-mcp",
            "animated_socket_materialization_key_sha256":hash('0')
        });
        assert!(validate_declared_tool_input(
            "game_weapon_animated_glb_socket_get",
            &get_arguments,
            false
        )
        .is_ok());

        let (mut backend, mut session) = initialized();
        let disabled = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":1962,
                "method":"tools/call",
                "params":{"name":"game_weapon_animated_glb_socket_prepare","arguments":{}}
            }),
        )
        .expect("animated GLB socket disabled response");
        assert_eq!(disabled["result"]["isError"], true);
        assert_eq!(
            disabled["result"]["structuredContent"]["code"],
            "MCP010F_GAME_WEAPON_ANIMATED_GLB_SOCKET_WRITE_TOOLS_DISABLED"
        );

        let summary = game_weapon_animated_glb_socket_mcp_summary(
            "game_weapon_animated_glb_socket_get",
            &json!({
                "animated_socket_materialization_key_sha256":hash('1'),
                "runtime_write_performed":false,
                "restart_hash_verified":true,
                "receipt":{
                    "source_artifact_sha256":hash('2'),
                    "source_artifact_readback_sha256":hash('3'),
                    "animated_artifact_sha256":hash('4'),
                    "animated_artifact_readback_sha256":hash('5'),
                    "derived_animated_socket_artifact_sha256":hash('6'),
                    "derived_animated_socket_artifact_readback_sha256":hash('7'),
                    "animation_receipt_object_sha256":hash('8'),
                    "animation_receipt_canonical_sha256":hash('9'),
                    "source_animation_projection_sha256":hash('a'),
                    "derived_animation_projection_sha256":hash('b'),
                    "source_animation_validation_sha256":hash('c'),
                    "derived_animation_validation_sha256":hash('d'),
                    "source_renderable_inventory_sha256":hash('e'),
                    "derived_renderable_inventory_sha256":hash('f'),
                    "source_bin_sha256":hash('0'),
                    "derived_bin_sha256":hash('1'),
                    "socket_node_inventory_sha256":hash('2'),
                    "sampler_count":2,
                    "channel_count":2,
                    "node_count":8,
                    "source_node_count":8,
                    "derived_node_count":14,
                    "socket_node_count":6,
                    "animations_preserved":true,
                    "channels_preserved":true,
                    "samplers_preserved":true,
                    "renderable_projection_exact":true,
                    "bin_byte_exact":true,
                    "source_static_projection_exact":true,
                    "no_skinning":true,
                    "no_morph_targets":true,
                    "socket_nodes_materialized":true
                },
                "durable_link":{
                    "receipt_object_sha256":hash('3'),
                    "project_id":"project-animated-socket-mcp",
                    "candidate_id":"candidate-animated-socket-mcp",
                    "candidate_state_sha256":hash('4'),
                    "delivery_manifest_object_sha256":hash('5'),
                    "lod0_artifact_sha256":hash('6'),
                    "anchor_set_object_sha256":hash('7')
                }
            }),
        )
        .expect("animated GLB socket summary");
        let summary: Value = serde_json::from_str(&summary).expect("summary JSON");
        assert_eq!(
            summary["animated_socket_materialization_key_sha256"],
            hash('1')
        );
        assert_eq!(summary["source_artifact_sha256"], hash('2'));
        assert_eq!(
            summary["derived_animated_socket_artifact_sha256"],
            hash('6')
        );
        assert_eq!(summary["receipt_object_sha256"], hash('3'));
        assert_eq!(summary["socket_node_count"], 6);
        assert_eq!(summary["animations_preserved"], true);
        assert_eq!(summary["channels_preserved"], true);
        assert_eq!(summary["samplers_preserved"], true);
        assert_eq!(summary["renderable_projection_exact"], true);
        assert_eq!(summary["bin_byte_exact"], true);
        assert_eq!(summary["quality_status"], "structural_only");
        assert_eq!(summary["structural_only"], true);
        assert_eq!(summary["actual_engine_roundtrip"], false);
        assert_eq!(summary["commercial_engine_roundtrip"], false);
        assert_eq!(summary["glb_bytes_in_summary"], false);
        assert!(summary.get("glb_bytes").is_none());
        assert!(!serde_json::to_string(&summary)
            .expect("summary serializes")
            .contains("\"glb_bytes\":"));
    }

    #[test]
    fn fictional_energy_vfx_trails_bloom_tools_are_closed_independent_and_truthful() {
        let read_tools = tools_with_writes(false);
        let enabled_tools = tools_with_writes(true);
        let trail_bloom_get = read_tools
            .iter()
            .find(|tool| tool["name"] == "fictional_energy_vfx_trails_bloom_get")
            .expect("typed trail Bloom get read tool");
        assert_eq!(trail_bloom_get["annotations"]["readOnlyHint"], true);
        assert_eq!(trail_bloom_get["annotations"]["destructiveHint"], false);
        assert_eq!(trail_bloom_get["annotations"]["idempotentHint"], true);
        assert_eq!(
            trail_bloom_get["inputSchema"]["additionalProperties"],
            false
        );
        assert_eq!(
            trail_bloom_get["inputSchema"]["required"],
            json!(["schema_version", "project_id", "trail_bloom_key_sha256"])
        );
        assert!(!is_write_tool("fictional_energy_vfx_trails_bloom_get"));

        let trail_bloom_prepare = enabled_tools
            .iter()
            .find(|tool| tool["name"] == "fictional_energy_vfx_trails_bloom_prepare")
            .expect("typed trail Bloom prepare write tool");
        assert_eq!(trail_bloom_prepare["annotations"]["readOnlyHint"], false);
        assert_eq!(trail_bloom_prepare["annotations"]["destructiveHint"], false);
        assert_eq!(trail_bloom_prepare["annotations"]["idempotentHint"], true);
        assert_eq!(
            trail_bloom_prepare["inputSchema"]["additionalProperties"],
            false
        );
        assert_eq!(
            trail_bloom_prepare["inputSchema"]["required"],
            json!([
                "schema_version",
                "project_id",
                "delivery_manifest_object_sha256",
                "vfx_profile_object_sha256",
                "anchor_set_object_sha256",
                "base_frame_key_sha256",
                "bloom_key_sha256",
                "source_trail_key_sha256",
                "trail_bloom_profile",
                "trail_bloom_policy",
                "input_policy",
                "occlusion_policy",
                "render_policy",
                "canonical_sha256"
            ])
        );
        assert_eq!(
            trail_bloom_prepare["inputSchema"]["properties"]["trail_bloom_profile"]
                ["additionalProperties"],
            false
        );
        assert_eq!(
            trail_bloom_prepare["_meta"]["forgecad"]["requiresConfirmation"],
            true
        );
        assert_eq!(
            trail_bloom_prepare["_meta"]["forgecad"]["transaction"],
            "MCP010F"
        );
        assert!(is_write_tool("fictional_energy_vfx_trails_bloom_prepare"));

        let hash = |byte: char| byte.to_string().repeat(64);
        let prepare_arguments = json!({
            "schema_version":"FictionalEnergyVfxTrailsBloomFrameRenderPrepareRequest@1",
            "project_id":"project-trails-bloom-mcp",
            "delivery_manifest_object_sha256":hash('a'),
            "vfx_profile_object_sha256":hash('b'),
            "anchor_set_object_sha256":hash('c'),
            "base_frame_key_sha256":hash('d'),
            "bloom_key_sha256":hash('e'),
            "source_trail_key_sha256":hash('f'),
            "trail_bloom_profile":{
                "threshold":1.0,
                "source_gain":8.0,
                "radius_px":8,
                "intensity":4.0,
                "hdr_clamp":16.0,
                "blur_passes":2,
                "kernel":"separable-box-two-pass-fixed-radius@1"
            },
            "trail_bloom_policy":"lod0-typed-trails-hdr-source-two-pass-fixed-kernel@1",
            "input_policy":"existing-trail-color-depth-plus-current-base-opaque-depth-byte-exact@1",
            "occlusion_policy":"current-base-opaque-depth-before-trail-depth-reversed-normalized-u8-epsilon-1e-4@1",
            "render_policy":"lod0-trail-bloom-two-new-passes-base-bloom-particles-trails-byte-exact-reused@1",
            "canonical_sha256":hash('0')
        });
        assert!(validate_declared_tool_input(
            "fictional_energy_vfx_trails_bloom_prepare",
            &prepare_arguments,
            true
        )
        .is_ok());
        let get_arguments = json!({
            "schema_version":"FictionalEnergyVfxTrailsBloomFrameGetRequest@1",
            "project_id":"project-trails-bloom-mcp",
            "trail_bloom_key_sha256":hash('9')
        });
        assert!(validate_declared_tool_input(
            "fictional_energy_vfx_trails_bloom_get",
            &get_arguments,
            false
        )
        .is_ok());

        let (mut backend, mut session) = initialized();
        let disabled = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":1951,
                "method":"tools/call",
                "params":{"name":"fictional_energy_vfx_trails_bloom_prepare","arguments":{}}
            }),
        )
        .expect("typed trail Bloom disabled response");
        assert_eq!(disabled["result"]["isError"], true);
        assert_eq!(
            disabled["result"]["structuredContent"]["code"],
            "MCP010F_WRITE_TOOLS_DISABLED"
        );

        let receipt = json!({
            "trail_bloom_key_sha256":hash('e'),
            "source_trail_key_sha256":hash('f'),
            "trail_bloom_rendered":true,
            "trail_bloom_source_rendered":true,
            "trail_bloom_contribution_rendered":true,
            "input_policy":"existing-trail-color-depth-plus-current-base-opaque-depth-byte-exact@1",
            "source_trail_color_object_sha256":hash('1'),
            "source_trail_id_object_sha256":hash('2'),
            "source_trail_depth_object_sha256":hash('3'),
            "base_opaque_depth_object_sha256":hash('4'),
            "source_pass":{"pass":"trail-emissive-source","object_sha256":hash('5')},
            "contribution_pass":{"pass":"trail-bloom-contribution","object_sha256":hash('6')},
            "base_aov_byte_exact_verified":true,
            "base_opaque_depth_byte_exact_reused":true,
            "bloom_pass_byte_exact_reused":true,
            "particle_passes_byte_exact_reused":true,
            "source_trail_passes_byte_exact_reused":true,
            "base_bloom_mutated":false,
            "particle_passes_mutated":false,
            "trail_passes_mutated":false
        });
        let summary = fictional_energy_vfx_mcp_summary(
            "fictional_energy_vfx_trails_bloom_get",
            &json!({"receipt":receipt}),
        )
        .expect("typed trail Bloom summary");
        let summary: Value = serde_json::from_str(&summary).expect("summary JSON");
        assert_eq!(summary["bloom_rendered"], false);
        assert_eq!(summary["particles_rendered"], false);
        assert_eq!(summary["trails_rendered"], false);
        assert_eq!(summary["trail_bloom_rendered"], true);
        assert_eq!(summary["trail_bloom_source_rendered"], true);
        assert_eq!(summary["trail_bloom_contribution_rendered"], true);
        assert_eq!(summary["trail_bloom_key_sha256"], hash('e'));
        assert_eq!(summary["source_trail_key_sha256"], hash('f'));
        assert_eq!(
            summary["input"]["source_trail_color_object_sha256"],
            hash('1')
        );
        assert_eq!(
            summary["input"]["base_opaque_depth_object_sha256"],
            hash('4')
        );
        assert_eq!(
            summary["trail_bloom_pass_artifacts"]["source"]["pass"],
            "trail-emissive-source"
        );
        assert_eq!(summary["base_aov_byte_exact_verified"], true);
        assert_eq!(summary["base_opaque_depth_byte_exact_reused"], true);
        assert_eq!(summary["bloom_pass_byte_exact_reused"], true);
        assert_eq!(summary["particle_passes_byte_exact_reused"], true);
        assert_eq!(summary["source_trail_passes_byte_exact_reused"], true);
        assert_eq!(summary["base_bloom_mutated"], false);
        assert_eq!(summary["particle_passes_mutated"], false);
        assert_eq!(summary["trail_passes_mutated"], false);
        assert_eq!(summary["runtime_write_performed"], false);
        assert_eq!(summary["structured_content_complete"], true);
    }

    #[test]
    fn mechanical_animation_clip_mcp_dispatches_and_summarizes_nested_preview() {
        let (mut backend, mut session) = initialized();
        let (project_id, prepared) = match &backend {
            Backend::InProcess(runtime) => {
                let project = runtime
                    .create_project("MCP mechanical clip transport", json!({"scope":"test"}))
                    .expect("project");
                let catalog_sha256 = runtime.active_operator_catalog()["canonical_sha256"]
                    .as_str()
                    .expect("catalog hash")
                    .to_owned();
                let mut program = json!({
                    "schema_version":"GeometryProgram@2",
                    "project_id":project.project_id,
                    "representation_plan_sha256":"7".repeat(64),
                    "operator_catalog_sha256":catalog_sha256,
                    "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
                    "budgets":{"max_nodes":2,"max_triangles":1000,"max_glb_bytes":1048576,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
                    "nodes":[
                        {"node_id":"root-node","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[1.0,1.0,1.0],"position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                        {"node_id":"arm-node","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[0.5,0.25,0.25],"position_m":[1.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}}
                    ],
                    "part_outputs":[
                        {"part_id":"root-part","input_node_ids":["root-node"],"material_zone_id":"zone-root","solid":true},
                        {"part_id":"arm-part","input_node_ids":["arm-node"],"material_zone_id":"zone-arm","solid":true}
                    ]
                });
                program["canonical_sha256"] = Value::String(canonical_json_hash(&program));
                let prepared = runtime
                    .prepare_geometry_candidate(
                        &project.project_id,
                        None,
                        json!({"typed":"geometry","geometry_program":program}),
                    )
                    .expect("geometry prepare");
                (project.project_id, prepared)
            }
            _ => unreachable!("test backend"),
        };
        let artifact = &prepared["artifact"];
        let candidate_id = prepared["candidate"]["candidate_id"]
            .as_str()
            .expect("candidate id")
            .to_owned();
        let mut sequence = json!({
            "schema_version":"MechanicalPoseSequencePreviewRequest@1",
            "project_id":project_id,
            "artifact_id":artifact["artifact_id"],
            "candidate_id":candidate_id,
            "artifact_readback_sha256":artifact["canonical_sha256"],
            "program_sha256":artifact["program_sha256"],
            "operator_catalog_sha256":artifact["operator_catalog_sha256"],
            "readback_config_sha256":artifact["readback_config_sha256"],
            "rest_frame_draft":{
                "schema_version":"MechanicalRestFrameDraft@1",
                "rest_frame_id":"mcp-clip-rest",
                "coordinate_system":"forgecad-rh-y-up-m@1",
                "transform_convention":"column-vector-trs-quaternion@1",
                "root_link_id":"root-link",
                "links":[
                    {"link_id":"root-link","part_id":"root-part","source_node_ids":["root-node"],"joint_type":"fixed","rest_translation_m":[0.0,0.0,0.0],"rest_rotation_quat_xyzw":[0.0,0.0,0.0,1.0],"axis_local":null,"limit_min":null,"limit_max":null,"value_unit":"none"},
                    {"link_id":"arm-link","part_id":"arm-part","source_node_ids":["arm-node"],"joint_type":"revolute","rest_translation_m":[1.0,0.0,0.0],"rest_rotation_quat_xyzw":[0.0,0.0,0.0,1.0],"axis_local":[0.0,0.0,1.0],"limit_min":-1.0,"limit_max":1.0,"value_unit":"radian"}
                ],
                "parent_map":[{"child_link_id":"arm-link","parent_link_id":"root-link"}]
            },
            "pose_action_draft":{
                "schema_version":"MechanicalPoseActionDraft@1",
                "action_id":"mcp-clip-action",
                "timebase_hz":1000,
                "duration_ticks":1000,
                "interpolation":"linear@1",
                "extrapolation":"clamp@1",
                "unkeyed_policy":"rest@1",
                "channels":[{"link_id":"arm-link","value_unit":"radian","keys":[{"time_ticks":0,"value":0.0},{"time_ticks":1000,"value":0.5}]}]
            },
            "sample_time_ticks":[0,500,1000],
            "input_sha256":""
        });
        let mut sequence_preimage = sequence.clone();
        sequence_preimage
            .as_object_mut()
            .unwrap()
            .remove("input_sha256");
        sequence["input_sha256"] = Value::String(canonical_json_hash(&sequence_preimage));
        let mut prepare = json!({
            "schema_version":"MechanicalAnimationClipPrepareRequest@1",
            "clip_id":"mcp-clip",
            "pose_sequence_request":sequence,
            "clip_policy":"runtime-owned-immutable-cas-rigid-mechanical-action@1",
            "input_sha256":""
        });
        let mut prepare_preimage = prepare.clone();
        prepare_preimage
            .as_object_mut()
            .unwrap()
            .remove("input_sha256");
        prepare["input_sha256"] = Value::String(canonical_json_hash(&prepare_preimage));

        session.write_tools_enabled = true;
        let prepared_response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":1803,"method":"tools/call","params":{"name":"mechanical_animation_clip_prepare","arguments":prepare}}),
        )
        .expect("clip prepare response");
        assert_eq!(
            prepared_response["result"]["structuredContent"]["schema_version"],
            "MechanicalAnimationClipLink@1",
            "{prepared_response}"
        );

        let mut preview = json!({
            "schema_version":"MechanicalAnimationClipPreviewRequest@1",
            "project_id":project_id,
            "candidate_id":candidate_id,
            "clip_id":"mcp-clip",
            "sample_time_ticks":500,
            "preview_policy":"single-tick-transient-double-worker-replay@1",
            "canonical_sha256":""
        });
        let mut preview_preimage = preview.clone();
        preview_preimage
            .as_object_mut()
            .unwrap()
            .remove("canonical_sha256");
        preview["canonical_sha256"] = Value::String(canonical_json_hash(&preview_preimage));
        let preview_response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":1804,"method":"tools/call","params":{"name":"mechanical_animation_clip_preview_get","arguments":preview}}),
        )
        .expect("clip preview response");
        assert!(
            serde_json::to_vec(&preview_response).unwrap().len() <= READ_MODEL_MCP_WIRE_MAX_BYTES,
            "mechanical animation clip preview MCP response must remain inside the 1 MiB wire budget"
        );
        let structured = &preview_response["result"]["structuredContent"];
        assert_eq!(
            structured["schema_version"],
            "MechanicalAnimationClipPreview@1"
        );
        assert_eq!(
            structured["pose_geometry_preview"]["worker_replay"]["byte_exact"],
            true
        );
        let summary: Value = serde_json::from_str(
            preview_response["result"]["content"][0]["text"]
                .as_str()
                .expect("clip preview summary"),
        )
        .expect("clip preview summary JSON");
        assert_eq!(
            summary["transient_artifact"],
            structured["pose_geometry_preview"]["transient_artifact"]
        );
        assert_eq!(
            summary["worker_replay"],
            structured["pose_geometry_preview"]["worker_replay"]
        );
        assert_eq!(summary["runtime_write_performed"], false);
        assert_eq!(summary["structured_content_complete"], true);

        let mut glb_prepare = json!({
            "schema_version":"MechanicalAnimationGlbPrepareRequest@1",
            "project_id":project_id,
            "candidate_id":candidate_id,
            "candidate_state_sha256":prepared["candidate"]["canonical_sha256"],
            "clip_id":"mcp-clip",
            "materialization_policy":"rigid-node-trs-gltf-linear-scheduled-samples@1",
            "canonical_sha256":""
        });
        let mut glb_preimage = glb_prepare.clone();
        glb_preimage
            .as_object_mut()
            .unwrap()
            .remove("canonical_sha256");
        glb_prepare["canonical_sha256"] = Value::String(canonical_json_hash(&glb_preimage));
        let glb_response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":1805,"method":"tools/call","params":{"name":"mechanical_animation_glb_prepare","arguments":glb_prepare}}),
        )
        .expect("animation GLB prepare response");
        assert!(
            serde_json::to_vec(&glb_response).unwrap().len() <= READ_MODEL_MCP_WIRE_MAX_BYTES,
            "mechanical animation GLB MCP response must remain inside the 1 MiB wire budget"
        );
        let glb_structured = &glb_response["result"]["structuredContent"];
        assert_eq!(
            glb_structured["schema_version"], "MechanicalAnimationGlbPrepareResult@1",
            "{glb_response}"
        );
        assert_eq!(glb_structured["receipt"]["hard_gate_passed"], true);
        assert_eq!(
            glb_structured["receipt"]["source_static_projection_exact"],
            true
        );
        assert_eq!(glb_structured["candidate_confirmed"], false);
        assert_eq!(glb_structured["export_performed"], false);
        let glb_summary: Value = serde_json::from_str(
            glb_response["result"]["content"][0]["text"]
                .as_str()
                .expect("animation GLB summary"),
        )
        .expect("animation GLB summary JSON");
        assert_eq!(
            glb_summary["schema_version"],
            "MechanicalAnimationGlbMcpSummary@1"
        );
        assert_eq!(
            glb_summary["animated_artifact_sha256"],
            glb_structured["animated_artifact_sha256"]
        );
        assert_eq!(glb_summary["candidate_confirmed"], false);
        assert_eq!(glb_summary["export_performed"], false);
        assert_eq!(glb_summary["structured_content_complete"], true);
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
    fn optimization_job_wire_reduces_camera_to_runtime_owned_identity() {
        let mut full_camera: Value = serde_json::from_str(
            r#"{
                "schema_version":"CameraCalibration@1",
                "camera_hash":"",
                "projection":"perspective",
                "transform":{
                    "position_m":[3.199191497840016,2.0907832771758006,5.0164046374013855],
                    "target_m":[-0.08276190380161919,1.346800270381924,0.09347453493893176],
                    "up":[-0.06920550542686278,0.992186697935671,-0.10380825814029421]
                },
                "fov_y_degrees":42.0,
                "near_m":0.05,
                "far_m":20.0,
                "resolution":{"width":512,"height":512},
                "coordinate_system":"right-handed-y-up-meter",
                "renderer_revision":"forgecad-renderer-2",
                "canonical_sha256":""
            }"#,
        )
        .expect("full camera JSON");
        let mut camera_hash_input = full_camera.clone();
        camera_hash_input["camera_hash"] = Value::String(String::new());
        camera_hash_input["canonical_sha256"] = Value::String(String::new());
        let full_camera_hash = canonical_json_hash(&camera_hash_input);
        full_camera["camera_hash"] = Value::String(full_camera_hash.clone());
        let mut full_canonical_input = full_camera.clone();
        full_canonical_input["canonical_sha256"] = Value::String(String::new());
        full_camera["canonical_sha256"] = Value::String(canonical_json_hash(&full_canonical_input));

        let mut wire_camera: Value = serde_json::from_str(
            r#"{
                "schema_version":"CameraCalibration@1",
                "camera_hash":"",
                "projection":"perspective",
                "transform":{
                    "position_m":[3.199191497840016,2.0907832771758006,5.0164046374013855],
                    "target_m":[-0.08276190380161919,1.346800270381924,0.09347453493893176],
                    "up":[-0.06920550542686278,0.992186697935671,-0.1038082581402942]
                },
                "fov_y_degrees":42.0,
                "near_m":0.05,
                "far_m":20.0,
                "resolution":{"width":512,"height":512},
                "coordinate_system":"right-handed-y-up-meter",
                "renderer_revision":"forgecad-renderer-2",
                "canonical_sha256":""
            }"#,
        )
        .expect("wire camera JSON");
        wire_camera["camera_hash"] = Value::String(full_camera_hash.clone());
        wire_camera["canonical_sha256"] = full_camera["canonical_sha256"].clone();
        let mut intent = json!({
            "schema_version":"OptimizationIntent@1",
            "intent_id":"intent-camera-wire-test",
            "job_id":"job-camera-wire-test",
            "project_id":"project-camera-wire-test",
            "candidate_id":"candidate-camera-wire-test",
            "reference_id":"reference-camera-wire-test",
            "reference_sha256":"a".repeat(64),
            "program_sha256":"b".repeat(64),
            "target_sha256":"c".repeat(64),
            "camera":wire_camera,
            "camera_hash":full_camera_hash,
            "part_id":"chest-shell",
            "stage":"primary-form",
            "rig":{"parameters":[]},
            "fidelity":{},
            "budget":{},
            "objective":{},
            "canonical_sha256":""
        });
        intent["canonical_sha256"] = Value::String(canonical_json_hash(&intent));
        let request = json!({"intent":intent});
        let restored = canonicalize_optimization_job_wire(&request).expect("wire camera rebind");
        let rebound_camera = &restored["intent"]["camera"];

        assert_eq!(
            rebound_camera["schema_version"],
            json!("CameraCalibrationRef@1")
        );
        assert_eq!(
            rebound_camera["camera_hash"],
            Value::String(full_camera_hash.clone())
        );
        assert_eq!(
            rebound_camera["canonical_sha256"],
            full_camera["canonical_sha256"]
        );
        assert_eq!(
            restored["intent"]["camera_hash"],
            Value::String(full_camera_hash)
        );
        let rebound_rig = &restored["intent"]["rig"];
        let mut rebound_rig_hash_input = rebound_rig.clone();
        rebound_rig_hash_input["canonical_sha256"] = Value::String(String::new());
        assert_eq!(
            rebound_rig["canonical_sha256"],
            Value::String(canonical_json_hash(&rebound_rig_hash_input))
        );
        let mut rebound_intent = restored["intent"].clone();
        let rebound_intent_hash = rebound_intent["canonical_sha256"].clone();
        rebound_intent["canonical_sha256"] = Value::String(String::new());
        assert_eq!(
            rebound_intent_hash,
            Value::String(canonical_json_hash(&rebound_intent))
        );
        assert_eq!(rebound_camera.as_object().map(Map::len), Some(3));

        let mut rejected = request;
        rejected["intent"]["canonical_sha256"] = Value::String("f".repeat(64));
        assert!(canonicalize_optimization_job_wire(&rejected).is_err());
    }

    #[test]
    fn optimization_job_wire_preserves_discrete_surface_control_point_index() {
        let mut intent = json!({
            "schema_version":"OptimizationIntent@1",
            "intent_id":"intent-surface-control-point-wire-test",
            "job_id":"job-surface-control-point-wire-test",
            "project_id":"project-surface-control-point-wire-test",
            "candidate_id":"candidate-surface-control-point-wire-test",
            "camera":{"camera_hash":"a".repeat(64),"canonical_sha256":"b".repeat(64)},
            "rig":{
                "schema_version":"SilhouetteRig@1",
                "rig_id":"rig-surface-control-point-wire-test",
                "candidate_id":"candidate-surface-control-point-wire-test",
                "parameters":[{
                    "parameter_id":"surface-control-point-7-x",
                    "part_id":"chest-shell",
                    "semantic":"surface_control_point",
                    "control_point_index":7,
                    "axis":"x",
                    "value":0.0,
                    "min":-0.25,
                    "max":0.25,
                    "step":0.02,
                    "unit":"meter"
                }],
                "canonical_sha256":""
            },
            "objective":{"silhouette_iou":0.35},
            "canonical_sha256":""
        });
        intent["canonical_sha256"] = Value::String(canonical_json_hash(&intent));
        let restored = canonicalize_optimization_job_wire(&json!({"intent":intent}))
            .expect("surface control point wire normalization");
        assert_eq!(
            restored["intent"]["rig"]["parameters"][0]["control_point_index"],
            json!(7)
        );
        assert!(
            restored["intent"]["rig"]["parameters"][0]["control_point_index"]
                .as_u64()
                .is_some()
        );
    }

    #[test]
    fn optimization_job_wire_rebinds_joint_multiview_camera_rig() {
        fn camera(kind_index: usize) -> Value {
            let mut value = json!({
                "schema_version":"CameraCalibration@2",
                "camera_hash":"",
                "projection":"orthographic",
                "transform":{
                    "position_m":[20.0 + kind_index as f64,0.0,0.0],
                    "target_m":[0.0,0.0,0.0],
                    "up":[0.0,1.0,0.0]
                },
                "fov_y_degrees":null,
                "ortho_scale":2.4,
                "near_m":0.05,
                "far_m":100.0,
                "resolution":{"width":512,"height":512},
                "coordinate_system":"right-handed-y-up-meter",
                "renderer_revision":"forgecad-renderer-2",
                "canonical_sha256":""
            });
            value["camera_hash"] = Value::String(canonical_json_hash(&value));
            value["canonical_sha256"] = Value::String(canonical_json_hash(&value));
            value
        }
        let kinds = ["left", "right", "top", "bottom", "front", "back"];
        let mut rig_views = Vec::new();
        let mut intent_views = Vec::new();
        for (index, kind) in kinds.iter().enumerate() {
            let mut value = camera(index);
            // Simulate a JSON client spelling a continuous coordinate as an
            // integer; nested identities are intentionally stale and must be
            // rebound by the canonical wire adapter.
            if index == 0 {
                value["transform"]["position_m"][0] = json!(20);
            }
            let view_id = format!("weapon-{kind}");
            let target_sha256 = format!("{:0>64}", index + 1);
            let camera_hash = value["camera_hash"].clone();
            rig_views.push(json!({
                "view_id":view_id,
                "kind":kind,
                "camera":value,
                "camera_hash":camera_hash,
                "weight":1.0,
                "primary":index == 0
            }));
            let intent_camera = rig_views.last().expect("rig view")["camera"].clone();
            intent_views.push(json!({
                "view_id":format!("weapon-{kind}"),
                "kind":kind,
                "target_sha256":target_sha256,
                "camera":intent_camera,
                "camera_hash":camera_hash,
                "weight":1.0,
                "primary":index == 0
            }));
        }
        let mut camera_rig = json!({
            "schema_version":"CameraRigCalibration@1",
            "rig_id":"weapon-rig-wire-test",
            "project_id":"project-joint-wire-test",
            "candidate_id":"candidate-joint-wire-test",
            "subject_coordinate_frame":{"schema_version":"SubjectCoordinateFrame@1","frame_id":"frame","canonical_sha256":"a".repeat(64)},
            "origin_m":[0.0,0.0,0.0],
            "object_scale_m":2.4,
            "renderer_revision":"forgecad-renderer-2",
            "views":rig_views,
            "canonical_sha256":""
        });
        camera_rig["canonical_sha256"] = Value::String(canonical_json_hash(&camera_rig));
        let camera_rig_sha256 = camera_rig["canonical_sha256"].clone();
        let rig_frame_sha256 = "b".repeat(64);
        let mut rig = json!({
            "schema_version":"SilhouetteRig@2",
            "rig_id":"weapon-silhouette-rig-wire-test",
            "candidate_id":"candidate-joint-wire-test",
            "subject_coordinate_frame_sha256":rig_frame_sha256,
            "target_part_ids":["receiver"],
            "groups":[{"group_id":"receiver-group","parameter_ids":["receiver-width"],"mode":"independent"}],
            "parameters":[{"parameter_id":"receiver-width","part_id":"receiver","semantic":"width","value":1.0,"min":0.5,"max":1.5,"step":0.1,"unit":"meter"},{"parameter_id":"receiver-height","part_id":"receiver","semantic":"height","value":1.0,"min":0.5,"max":1.5,"step":0.1,"unit":"meter"},{"parameter_id":"receiver-depth","part_id":"receiver","semantic":"depth","value":1.0,"min":0.5,"max":1.5,"step":0.1,"unit":"meter"},{"parameter_id":"receiver-offset","part_id":"receiver","semantic":"offset_x","value":0.0,"min":-0.5,"max":0.5,"step":0.1,"unit":"meter"}],
            "canonical_sha256":""
        });
        rig["canonical_sha256"] = Value::String(canonical_json_hash(&rig));
        let mut intent = json!({
            "schema_version":"OptimizationIntent@2",
            "intent_id":"intent-joint-wire-test",
            "job_id":"job-joint-wire-test",
            "project_id":"project-joint-wire-test",
            "candidate_id":"candidate-joint-wire-test",
            "reference_id":"reference-joint-wire-test",
            "reference_sha256":"a".repeat(64),
            "program_sha256":"b".repeat(64),
            "camera_rig_sha256":camera_rig_sha256,
            "camera_rig":camera_rig,
            "views":intent_views,
            "part_id":"receiver",
            "target_part_ids":["receiver"],
            "stage":"primary-form",
            "rig":rig,
            "fidelity":{"coarse_resolution":128,"mid_resolution":256,"final_resolution":512,"final_top_k":2},
            "budget":{"max_evaluations":48},
            "objective":{"silhouette_iou":1.0},
            "canonical_sha256":""
        });
        intent["canonical_sha256"] = Value::String(canonical_json_hash(&intent));
        let restored = canonicalize_optimization_job_wire(&json!({"intent":intent}))
            .expect("joint camera rig wire rebind");
        let restored_intent = &restored["intent"];
        assert_eq!(restored_intent["schema_version"], "OptimizationIntent@2");
        assert_eq!(
            restored_intent["camera_rig"]["canonical_sha256"],
            restored_intent["camera_rig_sha256"]
        );
        assert_eq!(
            restored_intent["camera_rig"]["views"][0]["camera"]["camera_hash"],
            restored_intent["camera_rig"]["views"][0]["camera_hash"]
        );
        let mut canonical = restored_intent.clone();
        let expected = canonical["canonical_sha256"].clone();
        canonical["canonical_sha256"] = Value::String(String::new());
        assert_eq!(expected, Value::String(canonical_json_hash(&canonical)));
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
    fn geometry_prepare_exact_schema_distinguishes_missing_and_null_head() {
        let exact = json!({
            "project_id":"project-exact-schema",
            "base_version_id":null,
            "idempotency_key":"geometry-exact-schema-once",
            "request":{
                "typed":"geometry",
                "geometry_program":{"schema_version":"GeometryProgram@2"}
            }
        });
        assert!(validate_declared_tool_input("geometry_prepare", &exact, true).is_ok());

        let mut missing_head = exact.clone();
        missing_head
            .as_object_mut()
            .expect("exact envelope")
            .remove("base_version_id");
        assert!(validate_declared_tool_input("geometry_prepare", &missing_head, true).is_err());

        let mut null_key = exact.clone();
        null_key["idempotency_key"] = Value::Null;
        assert!(validate_declared_tool_input("geometry_prepare", &null_key, true).is_err());

        let mut v1_exact = exact;
        v1_exact["request"]["geometry_program"]["schema_version"] =
            Value::String("GeometryProgram@1".to_owned());
        assert!(validate_declared_tool_input("geometry_prepare", &v1_exact, true).is_err());

        let legacy = json!({
            "project_id":"project-legacy-schema",
            "request":{
                "typed":"geometry",
                "geometry_program":{"schema_version":"GeometryProgram@1"}
            }
        });
        assert!(validate_declared_tool_input("geometry_prepare", &legacy, true).is_ok());

        let modifier_exact = json!({
            "project_id":"project-exact-schema",
            "base_version_id":null,
            "idempotency_key":"modifier-apply-exact-schema-once",
            "request":{
                "typed":"geometry",
                "modifier_evaluation_sha256":"a".repeat(64),
                "modifier_evaluation_request":{
                    "schema_version":"GeometryModifierEvaluationRequest@2",
                    "project_id":"project-exact-schema",
                    "representation_plan_sha256":"b".repeat(64),
                    "part_id":"shell",
                    "material_zone_id":"zone-shell",
                    "solid":true,
                    "base_node":{"node_id":"base","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[1.0,1.0,1.0],"position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                    "modifiers":[{"modifier_id":"round","enabled":true,"operator_id":"forgecad.geometry.bevel@1","parameters":{"shape":"bevel","width_m":0.04,"segments":2,"profile":0.5,"edge_scope":"all-source-box-edges","clamp_overlap":false}}],
                    "previous_evaluation":null,
                    "input_sha256":"c".repeat(64)
                }
            }
        });
        assert!(validate_declared_tool_input("geometry_prepare", &modifier_exact, true).is_ok());

        let modifier_apply_v2 = json!({
            "project_id":"project-exact-schema",
            "base_version_id":null,
            "idempotency_key":"modifier-apply-v2-exact-schema-once",
            "request":{
                "typed":"geometry",
                "modifier_apply_sha256":"d".repeat(64),
                "modifier_apply_request":{
                    "schema_version":"GeometryModifierApplyRequest@2",
                    "project_id":"project-exact-schema",
                    "source_candidate_id":"candidate-source",
                    "source_candidate_canonical_sha256":"a".repeat(64),
                    "source_artifact_sha256":"b".repeat(64),
                    "source_artifact_readback_sha256":"c".repeat(64),
                    "source_geometry_program_sha256":"d".repeat(64),
                    "source_operator_catalog_sha256":"e".repeat(64),
                    "source_readback_config_sha256":"f".repeat(64),
                    "source_part_id":"shell",
                    "source_terminal_node_id":"shell-source",
                    "source_authoring_topology_sha256":"1".repeat(64),
                    "source_edge_id":"edge-shell-01",
                    "bevel_m":0.04,
                    "segments":2,
                    "profile":0.5,
                    "clamp_overlap":false,
                    "base_version_id":null,
                    "idempotency_key":"modifier-apply-v2-exact-schema-once",
                    "max_response_bytes":1048576,
                    "input_sha256":"2".repeat(64)
                }
            }
        });
        assert!(validate_declared_tool_input("geometry_prepare", &modifier_apply_v2, true).is_ok());
        let v2_schema = modifier_apply_request_v2_schema();
        assert_eq!(
            v2_schema["required"].as_array().map(Vec::len),
            Some(21),
            "GeometryModifierApplyRequest@2 must keep the exact 21-field contract"
        );
        assert_eq!(
            v2_schema["properties"]
                .as_object()
                .map(serde_json::Map::len),
            Some(21),
            "GeometryModifierApplyRequest@2 must not grow an unreviewed field"
        );
        assert_eq!(v2_schema["additionalProperties"], false);
        let mut modifier_apply_v2_unknown = modifier_apply_v2.clone();
        modifier_apply_v2_unknown["request"]["modifier_apply_request"]["python"] =
            json!("forbidden");
        assert!(
            validate_declared_tool_input("geometry_prepare", &modifier_apply_v2_unknown, true)
                .is_err()
        );
        let mut modifier_apply_v2_missing_edge = modifier_apply_v2.clone();
        modifier_apply_v2_missing_edge["request"]["modifier_apply_request"]
            .as_object_mut()
            .expect("v2 request object")
            .remove("source_edge_id");
        assert!(validate_declared_tool_input(
            "geometry_prepare",
            &modifier_apply_v2_missing_edge,
            true
        )
        .is_err());
        let mut missing_evaluation_hash = modifier_exact.clone();
        missing_evaluation_hash["request"]
            .as_object_mut()
            .expect("modifier exact request")
            .remove("modifier_evaluation_sha256");
        assert!(
            validate_declared_tool_input("geometry_prepare", &missing_evaluation_hash, true)
                .is_err()
        );
        let mut mixed_direct_and_modifier = modifier_exact;
        mixed_direct_and_modifier["request"]["geometry_program"] =
            json!({"schema_version":"GeometryProgram@2"});
        assert!(
            validate_declared_tool_input("geometry_prepare", &mixed_direct_and_modifier, true)
                .is_err()
        );
    }

    #[test]
    fn geometry_prepare_modifier_apply_v2_is_hidden_until_write_opt_in() {
        let read_tool = tools_with_writes(false)
            .into_iter()
            .find(|tool| tool["name"] == "geometry_prepare");
        assert!(
            read_tool.is_none(),
            "geometry_prepare must remain hidden in the default read-only manifest"
        );

        let write_tool = tools_with_writes(true)
            .into_iter()
            .find(|tool| tool["name"] == "geometry_prepare")
            .expect("write-enabled geometry_prepare");
        assert_eq!(write_tool["annotations"]["readOnlyHint"], false);
        assert_eq!(
            write_tool["_meta"]["forgecad"]["requiresConfirmation"],
            true
        );
        let description = write_tool["description"]
            .as_str()
            .expect("geometry_prepare description");
        for phrase in [
            "GeometryModifierApplyRequest@2",
            "one stable source edge",
            "direct authoring-mesh@1",
            "authenticated explicit write opt-in",
            "does not confirm",
            "create a version",
            "export",
            "never raw GLB bytes",
        ] {
            assert!(
                description.contains(phrase),
                "missing description phrase: {phrase}"
            );
        }
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
            "visual_surface_get",
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
        assert_eq!(
            read_tools
                .iter()
                .find(|tool| tool["name"] == "visual_surface_get")
                .expect("surface tool")
                .pointer("/_meta/forgecad/source_schema"),
            Some(&Value::String("VisualSurfaceResult@1".to_owned()))
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
    fn animated_socket_transform_projection_mcp_surface_is_hidden_and_preflight_gated() {
        let prepare_name = "game_weapon_animated_glb_socket_transform_projection_prepare";
        let get_name = "game_weapon_animated_glb_socket_transform_projection_get";
        let read_tools = tools_with_writes(false);
        assert!(read_tools.iter().any(|tool| tool["name"] == get_name));
        assert!(!read_tools.iter().any(|tool| tool["name"] == prepare_name));
        let enabled_tools = tools_with_writes(true);
        assert!(enabled_tools
            .iter()
            .any(|tool| tool["name"] == prepare_name));
        assert!(enabled_tools.iter().any(|tool| tool["name"] == get_name));

        let mut backend = Backend::InProcess(Runtime::ephemeral().expect("runtime"));
        let mut session = Session::new();
        handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":1,
                "method":"initialize",
                "params":{"protocolVersion":MCP_PROTOCOL_VERSION,"capabilities":{},"clientInfo":{"name":"projection-mcp-test","version":"1"}}
            }),
        )
        .expect("initialize response");

        let get_request = json!({
            "jsonrpc":"2.0",
            "id":2,
            "method":"tools/call",
            "params":{"name":get_name,"arguments":{
                "schema_version":"GameWeaponAnimatedGlbSocketTransformProjectionGetRequest@1",
                "projection_key_sha256":"a".repeat(64),
                "project_id":"project-1",
                "candidate_id":"candidate-1"
            }}
        });
        let blocked = handle(&mut backend, &mut session, &get_request).expect("preflight response");
        assert_eq!(
            blocked["result"]["structuredContent"]["code"],
            "PONYTAIL_PREFLIGHT_REQUIRED"
        );

        session.ponytail_preflight_read = true;
        session.write_tools_enabled = false;
        let disabled = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":3,
                "method":"tools/call",
                "params":{"name":prepare_name,"arguments":{}}
            }),
        )
        .expect("disabled response");
        assert_eq!(
            disabled["result"]["structuredContent"]["code"],
            "AGENTIC_WRITE_TOOLS_DISABLED"
        );

        let runtime = Runtime::ephemeral().expect("projection dispatch runtime");
        for (name, arguments) in [
            (
                get_name,
                json!({
                    "schema_version":"GameWeaponAnimatedGlbSocketTransformProjectionGetRequest@1",
                    "projection_key_sha256":"a".repeat(64),
                    "project_id":"project-1",
                    "candidate_id":"candidate-1"
                }),
            ),
            (prepare_name, json!({})),
        ] {
            let error = dispatch_in_process(&runtime, name, &arguments)
                .expect_err("projection Runtime dispatch must reach the named method");
            assert!(
                !error.starts_with("CAPABILITY_UNAVAILABLE:"),
                "{name} fell through the MCP Runtime dispatch table: {error}"
            );
        }

        session.write_tools_enabled = true;
        let dispatched =
            handle(&mut backend, &mut session, &get_request).expect("dispatch response");
        assert_eq!(dispatched["result"]["isError"], true);
        assert_ne!(
            dispatched["result"]["structuredContent"]["code"],
            "CAPABILITY_UNAVAILABLE"
        );
    }

    #[test]
    fn animated_socket_particles_sequence_mcp_surface_is_hidden_and_preflight_gated() {
        let prepare_name = "fictional_energy_vfx_animated_socket_particles_sequence_prepare";
        let get_name = "fictional_energy_vfx_animated_socket_particles_sequence_get";
        let read_tools = tools_with_writes(false);
        assert!(read_tools.iter().any(|tool| tool["name"] == get_name));
        assert!(!read_tools.iter().any(|tool| tool["name"] == prepare_name));
        let enabled_tools = tools_with_writes(true);
        assert!(enabled_tools
            .iter()
            .any(|tool| tool["name"] == prepare_name));
        assert!(enabled_tools.iter().any(|tool| tool["name"] == get_name));
        let prepare_tool = enabled_tools
            .iter()
            .find(|tool| tool["name"] == prepare_name)
            .expect("animated socket particles prepare tool");
        assert_eq!(prepare_tool["annotations"]["readOnlyHint"], false);
        assert_eq!(prepare_tool["annotations"]["writeIntent"], true);
        assert_eq!(prepare_tool["annotations"]["approvalRequired"], false);
        assert_eq!(
            prepare_tool["_meta"]["forgecad"]["requiresConfirmation"],
            false
        );
        assert_eq!(prepare_tool["inputSchema"]["additionalProperties"], false);
        let get_tool = read_tools
            .iter()
            .find(|tool| tool["name"] == get_name)
            .expect("animated socket particles get tool");
        assert_eq!(get_tool["annotations"]["readOnlyHint"], true);
        assert_eq!(get_tool["annotations"]["writeIntent"], false);
        assert_eq!(get_tool["annotations"]["approvalRequired"], false);

        let mut backend = Backend::InProcess(Runtime::ephemeral().expect("runtime"));
        let mut session = Session::new();
        handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":1,
                "method":"initialize",
                "params":{"protocolVersion":MCP_PROTOCOL_VERSION,"capabilities":{},"clientInfo":{"name":"particles-mcp-test","version":"1"}}
            }),
        )
        .expect("initialize response");
        let get_request = json!({
            "jsonrpc":"2.0",
            "id":2,
            "method":"tools/call",
            "params":{"name":get_name,"arguments":{
                "schema_version":"FictionalEnergyVfxAnimatedSocketParticlesSequenceGetRequest@1",
                "sequence_key_sha256":"a".repeat(64),
                "project_id":"project-1",
                "candidate_id":"candidate-1"
            }}
        });
        let get_arguments = get_request["params"]["arguments"].clone();
        assert!(validate_declared_tool_input(get_name, &get_arguments, false).is_ok());
        for field in ["unknown", "raw_glb_bytes", "png_base64", "path", "url"] {
            let mut invalid_arguments = get_arguments.clone();
            invalid_arguments[field] = json!("not-allowed");
            assert!(
                validate_declared_tool_input(get_name, &invalid_arguments, false).is_err(),
                "closed particle get schema accepted {field}"
            );
        }
        let blocked = handle(&mut backend, &mut session, &get_request).expect("preflight response");
        assert_eq!(
            blocked["result"]["structuredContent"]["code"],
            "PONYTAIL_PREFLIGHT_REQUIRED"
        );

        session.ponytail_preflight_read = true;
        session.write_tools_enabled = false;
        let disabled = handle(
            &mut backend,
            &mut session,
            &json!({
                "jsonrpc":"2.0",
                "id":3,
                "method":"tools/call",
                "params":{"name":prepare_name,"arguments":{}}
            }),
        )
        .expect("disabled response");
        assert_eq!(
            disabled["result"]["structuredContent"]["code"],
            "AGENTIC_WRITE_TOOLS_DISABLED"
        );

        let runtime = Runtime::ephemeral().expect("particles dispatch runtime");
        for (name, arguments) in [
            (
                get_name,
                json!({
                    "schema_version":"FictionalEnergyVfxAnimatedSocketParticlesSequenceGetRequest@1",
                    "sequence_key_sha256":"a".repeat(64),
                    "project_id":"project-1",
                    "candidate_id":"candidate-1"
                }),
            ),
            (prepare_name, json!({})),
        ] {
            let error = dispatch_in_process(&runtime, name, &arguments)
                .expect_err("particle Runtime dispatch must reach the named method");
            assert!(
                !error.starts_with("CAPABILITY_UNAVAILABLE:"),
                "{name} fell through the MCP Runtime dispatch table: {error}"
            );
        }

        session.write_tools_enabled = true;
        let dispatched =
            handle(&mut backend, &mut session, &get_request).expect("dispatch response");
        assert_eq!(dispatched["result"]["isError"], true);
        assert_ne!(
            dispatched["result"]["structuredContent"]["code"],
            "CAPABILITY_UNAVAILABLE"
        );
    }

    #[test]
    fn animated_socket_particles_sequence_v2_mcp_surface_is_closed_hidden_and_dispatches() {
        let prepare_name = "fictional_energy_vfx_animated_socket_particles_sequence_v2_prepare";
        let get_name = "fictional_energy_vfx_animated_socket_particles_sequence_v2_get";
        let read_tools = tools_with_writes(false);
        assert!(read_tools.iter().any(|tool| tool["name"] == get_name));
        assert!(!read_tools.iter().any(|tool| tool["name"] == prepare_name));
        let enabled_tools = tools_with_writes(true);
        let prepare_tool = enabled_tools
            .iter()
            .find(|tool| tool["name"] == prepare_name)
            .expect("V2 animated socket particles prepare tool");
        let get_tool = read_tools
            .iter()
            .find(|tool| tool["name"] == get_name)
            .expect("V2 animated socket particles get tool");
        assert_eq!(prepare_tool["annotations"]["readOnlyHint"], false);
        assert_eq!(prepare_tool["annotations"]["writeIntent"], true);
        assert_eq!(prepare_tool["annotations"]["approvalRequired"], false);
        assert_eq!(
            prepare_tool["_meta"]["forgecad"]["requiresConfirmation"],
            false
        );
        assert_eq!(prepare_tool["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            prepare_tool["inputSchema"]["properties"]["sample_count"]["maximum"],
            16
        );
        assert_eq!(
            prepare_tool["inputSchema"]["properties"]["frames"]["maxItems"],
            16
        );
        assert_eq!(get_tool["annotations"]["readOnlyHint"], true);
        assert_eq!(get_tool["annotations"]["writeIntent"], false);
        assert_eq!(get_tool["annotations"]["approvalRequired"], false);

        let get_arguments = json!({
            "schema_version":"FictionalEnergyVfxAnimatedSocketParticlesSequenceGetRequest@2",
            "sequence_key_sha256":"a".repeat(64),
            "project_id":"project-1",
            "geometry_candidate_id":"candidate-geometry-1",
            "appearance_candidate_id":"candidate-appearance-1",
            "geometry_delivery_manifest_object_sha256":"b".repeat(64),
            "appearance_delivery_manifest_object_sha256":"c".repeat(64)
        });
        assert!(validate_declared_tool_input(get_name, &get_arguments, false).is_ok());
        for field in [
            "unknown",
            "raw_glb_bytes",
            "png_base64",
            "path",
            "url",
            "uri",
        ] {
            let mut invalid_arguments = get_arguments.clone();
            invalid_arguments[field] = json!("not-allowed");
            assert!(
                validate_declared_tool_input(get_name, &invalid_arguments, false).is_err(),
                "closed V2 particle get schema accepted {field}"
            );
        }

        let runtime = Runtime::ephemeral().expect("V2 particles dispatch runtime");
        for (name, arguments) in [(get_name, get_arguments), (prepare_name, json!({}))] {
            let error = dispatch_in_process(&runtime, name, &arguments)
                .expect_err("V2 particle Runtime dispatch must reach the named method");
            assert!(
                !error.starts_with("CAPABILITY_UNAVAILABLE:"),
                "{name} fell through the MCP Runtime dispatch table: {error}"
            );
        }
    }

    #[test]
    fn animated_socket_attachment_v2_mcp_surface_is_closed_hidden_and_dispatches() {
        let prepare_name = "fictional_energy_vfx_animated_socket_attachment_v2_prepare";
        let get_name = "fictional_energy_vfx_animated_socket_attachment_v2_get";
        let read_tools = tools_with_writes(false);
        assert!(read_tools.iter().any(|tool| tool["name"] == get_name));
        assert!(!read_tools.iter().any(|tool| tool["name"] == prepare_name));
        let enabled_tools = tools_with_writes(true);
        let prepare_tool = enabled_tools
            .iter()
            .find(|tool| tool["name"] == prepare_name)
            .expect("V2 animated socket attachment prepare tool");
        let get_tool = read_tools
            .iter()
            .find(|tool| tool["name"] == get_name)
            .expect("V2 animated socket attachment get tool");
        assert_eq!(prepare_tool["annotations"]["readOnlyHint"], false);
        assert_eq!(prepare_tool["annotations"]["writeIntent"], true);
        assert_eq!(prepare_tool["annotations"]["approvalRequired"], false);
        assert_eq!(
            prepare_tool["_meta"]["forgecad"]["requiresConfirmation"],
            false
        );
        assert_eq!(prepare_tool["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            prepare_tool["inputSchema"]["properties"]["attachment_policy"]["const"],
            "fictional-energy-vfx-animated-socket-attachment-projection-bound@2"
        );
        assert_eq!(
            prepare_tool["inputSchema"]["properties"]["frame_scope"]["const"],
            "lod0-animation-vfx-trail-frame-range-1-15@2"
        );
        assert_eq!(get_tool["annotations"]["readOnlyHint"], true);
        assert_eq!(get_tool["annotations"]["writeIntent"], false);
        assert_eq!(get_tool["annotations"]["approvalRequired"], false);

        let get_arguments = json!({
            "schema_version":"FictionalEnergyVfxAnimatedSocketAttachmentGetRequest@2",
            "attachment_key_sha256":"a".repeat(64),
            "project_id":"project-1",
            "candidate_id":"candidate-1"
        });
        assert!(validate_declared_tool_input(get_name, &get_arguments, false).is_ok());
        for field in [
            "unknown",
            "raw_glb_bytes",
            "png_base64",
            "path",
            "url",
            "uri",
        ] {
            let mut invalid_arguments = get_arguments.clone();
            invalid_arguments[field] = json!("not-allowed");
            assert!(
                validate_declared_tool_input(get_name, &invalid_arguments, false).is_err(),
                "closed V2 attachment get schema accepted {field}"
            );
        }

        let runtime = Runtime::ephemeral().expect("V2 attachment dispatch runtime");
        for (name, arguments) in [(get_name, get_arguments), (prepare_name, json!({}))] {
            let error = dispatch_in_process(&runtime, name, &arguments)
                .expect_err("V2 attachment Runtime dispatch must reach the named method");
            assert!(
                !error.starts_with("CAPABILITY_UNAVAILABLE:"),
                "{name} fell through the MCP Runtime dispatch table: {error}"
            );
        }
    }

    #[test]
    fn animated_socket_attachment_v3_mcp_surface_is_closed_hidden_and_dispatches() {
        let prepare_name = "fictional_energy_vfx_animated_socket_attachment_v3_prepare";
        let get_name = "fictional_energy_vfx_animated_socket_attachment_v3_get";
        let read_tools = tools_with_writes(false);
        assert!(read_tools.iter().any(|tool| tool["name"] == get_name));
        assert!(!read_tools.iter().any(|tool| tool["name"] == prepare_name));
        let enabled_tools = tools_with_writes(true);
        let prepare_tool = enabled_tools
            .iter()
            .find(|tool| tool["name"] == prepare_name)
            .expect("Attachment@3 prepare tool");
        let get_tool = read_tools
            .iter()
            .find(|tool| tool["name"] == get_name)
            .expect("Attachment@3 get tool");
        assert_eq!(prepare_tool["annotations"]["readOnlyHint"], false);
        assert_eq!(prepare_tool["annotations"]["writeIntent"], true);
        assert_eq!(prepare_tool["annotations"]["approvalRequired"], false);
        assert_eq!(get_tool["annotations"]["readOnlyHint"], true);
        assert_eq!(get_tool["annotations"]["writeIntent"], false);
        assert_eq!(get_tool["annotations"]["approvalRequired"], false);
        assert_eq!(prepare_tool["inputSchema"]["additionalProperties"], false);
        assert_eq!(get_tool["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            prepare_tool["inputSchema"]["properties"]["sample_count"]["const"],
            15
        );
        assert_eq!(
            prepare_tool["inputSchema"]["properties"]["attachment_policy"]["const"],
            "projection-v2-particles-v2-trails-v2-trails-bloom-v2-animated-socket-attachment-bridge@3"
        );

        let get_arguments = json!({
            "schema_version":"FictionalEnergyVfxAnimatedSocketAttachmentGetRequest@3",
            "attachment_key_sha256":"a".repeat(64),
            "project_id":"project-attachment-v3",
            "geometry_candidate_id":"geometry-v3",
            "appearance_candidate_id":"appearance-v3",
            "geometry_delivery_manifest_object_sha256":"b".repeat(64),
            "appearance_delivery_manifest_object_sha256":"c".repeat(64)
        });
        assert!(validate_declared_tool_input(get_name, &get_arguments, false).is_ok());
        for field in [
            "unknown",
            "raw_glb_bytes",
            "png_base64",
            "path",
            "url",
            "script",
            "secret",
        ] {
            let mut invalid_arguments = get_arguments.clone();
            invalid_arguments[field] = json!("not-allowed");
            assert!(
                validate_declared_tool_input(get_name, &invalid_arguments, false).is_err(),
                "closed Attachment@3 get schema accepted {field}"
            );
        }
        let mut same_candidate = get_arguments.clone();
        same_candidate["appearance_candidate_id"] = json!("geometry-v3");
        assert!(agentic_write_tools::validate_call(
            get_name,
            &same_candidate,
            &agentic_write_tools::Binding::default()
        )
        .is_err());

        let runtime = Runtime::ephemeral().expect("Attachment@3 dispatch runtime");
        for (name, arguments) in [(get_name, get_arguments), (prepare_name, json!({}))] {
            let error = dispatch_in_process(&runtime, name, &arguments)
                .expect_err("Attachment@3 Runtime dispatch must reach the named method");
            assert!(
                !error.starts_with("CAPABILITY_UNAVAILABLE:"),
                "{name} fell through the MCP Runtime dispatch table: {error}"
            );
        }
    }

    #[test]
    fn animated_socket_trails_sequence_mcp_surface_is_closed_hidden_and_dispatches() {
        let prepare_name = "fictional_energy_vfx_animated_socket_trails_sequence_prepare";
        let get_name = "fictional_energy_vfx_animated_socket_trails_sequence_get";
        let read_tools = tools_with_writes(false);
        assert!(read_tools.iter().any(|tool| tool["name"] == get_name));
        assert!(!read_tools.iter().any(|tool| tool["name"] == prepare_name));
        let enabled_tools = tools_with_writes(true);
        let prepare_tool = enabled_tools
            .iter()
            .find(|tool| tool["name"] == prepare_name)
            .expect("animated socket trails prepare tool");
        let get_tool = read_tools
            .iter()
            .find(|tool| tool["name"] == get_name)
            .expect("animated socket trails get tool");
        assert_eq!(prepare_tool["annotations"]["readOnlyHint"], false);
        assert_eq!(prepare_tool["annotations"]["writeIntent"], true);
        assert_eq!(prepare_tool["annotations"]["approvalRequired"], false);
        assert_eq!(prepare_tool["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            prepare_tool["inputSchema"]["properties"]["sample_count"]["maximum"],
            15
        );
        assert_eq!(
            prepare_tool["inputSchema"]["properties"]["frames"]["maxItems"],
            15
        );
        assert_eq!(get_tool["annotations"]["readOnlyHint"], true);
        assert_eq!(get_tool["annotations"]["writeIntent"], false);
        assert_eq!(get_tool["annotations"]["approvalRequired"], false);

        let get_arguments = json!({
            "schema_version":"FictionalEnergyVfxAnimatedSocketTrailsSequenceGetRequest@1",
            "sequence_key_sha256":"a".repeat(64),
            "project_id":"project-1",
            "candidate_id":"candidate-1"
        });
        assert!(validate_declared_tool_input(get_name, &get_arguments, false).is_ok());
        for field in ["unknown", "raw_glb_bytes", "png_base64", "path", "url"] {
            let mut invalid_arguments = get_arguments.clone();
            invalid_arguments[field] = json!("not-allowed");
            assert!(
                validate_declared_tool_input(get_name, &invalid_arguments, false).is_err(),
                "closed animated trails get schema accepted {field}"
            );
        }

        let runtime = Runtime::ephemeral().expect("animated trails dispatch runtime");
        for (name, arguments) in [(get_name, get_arguments), (prepare_name, json!({}))] {
            let error = dispatch_in_process(&runtime, name, &arguments)
                .expect_err("animated trails Runtime dispatch must reach the named method");
            assert!(
                !error.starts_with("CAPABILITY_UNAVAILABLE:"),
                "{name} fell through the MCP Runtime dispatch table: {error}"
            );
        }
    }

    #[test]
    fn animated_socket_trails_sequence_v2_mcp_surface_is_closed_hidden_and_dispatches() {
        let prepare_name = "fictional_energy_vfx_animated_socket_trails_sequence_v2_prepare";
        let get_name = "fictional_energy_vfx_animated_socket_trails_sequence_v2_get";
        let read_tools = tools_with_writes(false);
        assert!(read_tools.iter().any(|tool| tool["name"] == get_name));
        assert!(!read_tools.iter().any(|tool| tool["name"] == prepare_name));
        let enabled_tools = tools_with_writes(true);
        let prepare_tool = enabled_tools
            .iter()
            .find(|tool| tool["name"] == prepare_name)
            .expect("Trails@2 prepare tool");
        let get_tool = read_tools
            .iter()
            .find(|tool| tool["name"] == get_name)
            .expect("Trails@2 get tool");
        assert_eq!(prepare_tool["annotations"]["readOnlyHint"], false);
        assert_eq!(prepare_tool["annotations"]["writeIntent"], true);
        assert_eq!(prepare_tool["annotations"]["approvalRequired"], false);
        assert_eq!(prepare_tool["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            prepare_tool["inputSchema"]["properties"]["frame_scope"]["const"],
            "lod0-animation-trails-v2-source-frames-1-15-with-particles-v2-frame-zero-preroll@2"
        );
        assert_eq!(
            prepare_tool["inputSchema"]["properties"]["history_policy"]["const"],
            "particles-v2-history-oldest-to-newest-ordinal-zero-last-immediate-previous-earlier-particle-frames-plus-current@2"
        );
        assert_eq!(get_tool["annotations"]["readOnlyHint"], true);
        assert_eq!(get_tool["annotations"]["writeIntent"], false);
        assert_eq!(get_tool["annotations"]["approvalRequired"], false);

        let get_arguments = json!({
            "schema_version":"FictionalEnergyVfxAnimatedSocketTrailsSequenceGetRequest@2",
            "sequence_key_sha256":"a".repeat(64),
            "project_id":"project-1",
            "geometry_candidate_id":"candidate-1",
            "appearance_candidate_id":"candidate-2",
            "geometry_delivery_manifest_object_sha256":"b".repeat(64),
            "appearance_delivery_manifest_object_sha256":"c".repeat(64)
        });
        assert!(validate_declared_tool_input(get_name, &get_arguments, false).is_ok());
        for field in [
            "unknown",
            "raw_glb_bytes",
            "png_base64",
            "path",
            "url",
            "script",
        ] {
            let mut invalid_arguments = get_arguments.clone();
            invalid_arguments[field] = json!("not-allowed");
            assert!(
                validate_declared_tool_input(get_name, &invalid_arguments, false).is_err(),
                "closed Trails@2 get schema accepted {field}"
            );
        }
        let mut same_candidate = get_arguments.clone();
        same_candidate["appearance_candidate_id"] = json!("candidate-1");
        assert!(agentic_write_tools::validate_call(
            get_name,
            &same_candidate,
            &agentic_write_tools::Binding {
                session_id: Some("session-1".to_owned()),
                project_id: Some("project-1".to_owned()),
                candidate_id: Some("candidate-1".to_owned()),
            }
        )
        .is_err());

        let runtime = Runtime::ephemeral().expect("Trails@2 dispatch runtime");
        for (name, arguments) in [(get_name, get_arguments), (prepare_name, json!({}))] {
            let error = dispatch_in_process(&runtime, name, &arguments)
                .expect_err("Trails@2 Runtime dispatch must reach the named method");
            assert!(
                !error.starts_with("CAPABILITY_UNAVAILABLE:"),
                "{name} fell through the MCP Runtime dispatch table: {error}"
            );
        }
    }

    #[test]
    fn animated_socket_trails_bloom_sequence_mcp_surface_is_closed_hidden_and_dispatches() {
        let prepare_name = "fictional_energy_vfx_animated_socket_trails_bloom_sequence_prepare";
        let get_name = "fictional_energy_vfx_animated_socket_trails_bloom_sequence_get";
        let read_tools = tools_with_writes(false);
        assert!(read_tools.iter().any(|tool| tool["name"] == get_name));
        assert!(!read_tools.iter().any(|tool| tool["name"] == prepare_name));
        let enabled_tools = tools_with_writes(true);
        let prepare_tool = enabled_tools
            .iter()
            .find(|tool| tool["name"] == prepare_name)
            .expect("animated socket trails Bloom prepare tool");
        let get_tool = read_tools
            .iter()
            .find(|tool| tool["name"] == get_name)
            .expect("animated socket trails Bloom get tool");
        assert_eq!(prepare_tool["annotations"]["readOnlyHint"], false);
        assert_eq!(prepare_tool["annotations"]["writeIntent"], true);
        assert_eq!(prepare_tool["annotations"]["approvalRequired"], false);
        assert_eq!(prepare_tool["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            prepare_tool["inputSchema"]["properties"]["sample_count"]["maximum"],
            15
        );
        assert_eq!(
            prepare_tool["inputSchema"]["properties"]["frames"]["maxItems"],
            15
        );
        assert_eq!(
            prepare_tool["inputSchema"]["properties"]["trail_bloom_profile"]
                ["additionalProperties"],
            false
        );
        assert_eq!(get_tool["annotations"]["readOnlyHint"], true);
        assert_eq!(get_tool["annotations"]["writeIntent"], false);
        assert_eq!(get_tool["annotations"]["approvalRequired"], false);

        let get_arguments = json!({
            "schema_version":"FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetRequest@1",
            "sequence_key_sha256":"a".repeat(64),
            "project_id":"project-1",
            "candidate_id":"candidate-1"
        });
        assert!(validate_declared_tool_input(get_name, &get_arguments, false).is_ok());
        for field in [
            "unknown",
            "raw_glb_bytes",
            "png_base64",
            "path",
            "url",
            "uri",
        ] {
            let mut invalid_arguments = get_arguments.clone();
            invalid_arguments[field] = json!("not-allowed");
            assert!(
                validate_declared_tool_input(get_name, &invalid_arguments, false).is_err(),
                "closed animated trails Bloom get schema accepted {field}"
            );
        }

        let runtime = Runtime::ephemeral().expect("animated trails Bloom dispatch runtime");
        for (name, arguments) in [(get_name, get_arguments), (prepare_name, json!({}))] {
            let error = dispatch_in_process(&runtime, name, &arguments)
                .expect_err("animated trails Bloom Runtime dispatch must reach the named method");
            assert!(
                !error.starts_with("CAPABILITY_UNAVAILABLE:"),
                "{name} fell through the MCP Runtime dispatch table: {error}"
            );
        }
    }

    #[test]
    fn animated_socket_trails_bloom_sequence_v2_mcp_surface_is_closed_hidden_and_dispatches() {
        let prepare_name = "fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_prepare";
        let get_name = "fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_get";
        let read_tools = tools_with_writes(false);
        assert!(read_tools.iter().any(|tool| tool["name"] == get_name));
        assert!(!read_tools.iter().any(|tool| tool["name"] == prepare_name));
        let enabled_tools = tools_with_writes(true);
        let prepare_tool = enabled_tools
            .iter()
            .find(|tool| tool["name"] == prepare_name)
            .expect("TrailsBloom@2 prepare tool");
        let get_tool = read_tools
            .iter()
            .find(|tool| tool["name"] == get_name)
            .expect("TrailsBloom@2 get tool");
        assert_eq!(prepare_tool["annotations"]["readOnlyHint"], false);
        assert_eq!(prepare_tool["annotations"]["writeIntent"], true);
        assert_eq!(prepare_tool["annotations"]["approvalRequired"], false);
        assert_eq!(prepare_tool["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            prepare_tool["inputSchema"]["properties"]["frame_scope"]["const"],
            "lod0-animation-trails-bloom-v2-source-frames-1-15-with-trails-v2-frame-zero-preroll@2"
        );
        assert_eq!(
            prepare_tool["inputSchema"]["properties"]["trails_bloom_sequence_policy"]["const"],
            "projection-v2-driven-animated-socket-trails-bloom-dual-candidate@2"
        );
        assert_eq!(get_tool["annotations"]["readOnlyHint"], true);
        assert_eq!(get_tool["annotations"]["writeIntent"], false);
        assert_eq!(get_tool["annotations"]["approvalRequired"], false);

        let get_arguments = json!({
            "schema_version":"FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetRequest@2",
            "sequence_key_sha256":"a".repeat(64),
            "project_id":"project-1",
            "geometry_candidate_id":"candidate-1",
            "appearance_candidate_id":"candidate-2",
            "geometry_delivery_manifest_object_sha256":"b".repeat(64),
            "appearance_delivery_manifest_object_sha256":"c".repeat(64)
        });
        assert!(validate_declared_tool_input(get_name, &get_arguments, false).is_ok());
        for field in [
            "unknown",
            "raw_glb_bytes",
            "png_base64",
            "path",
            "url",
            "script",
        ] {
            let mut invalid_arguments = get_arguments.clone();
            invalid_arguments[field] = json!("not-allowed");
            assert!(
                validate_declared_tool_input(get_name, &invalid_arguments, false).is_err(),
                "closed TrailsBloom@2 get schema accepted {field}"
            );
        }
        let mut same_candidate = get_arguments.clone();
        same_candidate["appearance_candidate_id"] = json!("candidate-1");
        assert!(agentic_write_tools::validate_call(
            get_name,
            &same_candidate,
            &agentic_write_tools::Binding {
                session_id: Some("session-1".to_owned()),
                project_id: Some("project-1".to_owned()),
                candidate_id: Some("candidate-1".to_owned()),
            }
        )
        .is_err());

        let runtime = Runtime::ephemeral().expect("TrailsBloom@2 dispatch runtime");
        for (name, arguments) in [(get_name, get_arguments), (prepare_name, json!({}))] {
            let error = dispatch_in_process(&runtime, name, &arguments)
                .expect_err("TrailsBloom@2 Runtime dispatch must reach the named method");
            assert!(
                !error.starts_with("CAPABILITY_UNAVAILABLE:"),
                "{name} fell through the MCP Runtime dispatch table: {error}"
            );
        }
    }

    #[test]
    fn candidate_topology_quality_tools_are_closed_and_opt_in() {
        let read_only = tools_with_writes(false);
        let topology_get = read_only
            .iter()
            .find(|tool| tool["name"] == "candidate_topology_quality_get")
            .expect("candidate topology read tool");
        assert_eq!(topology_get["annotations"]["readOnlyHint"], true);
        assert_eq!(topology_get["annotations"]["approvalRequired"], false);
        assert!(topology_get["description"]
            .as_str()
            .is_some_and(|description| description.contains("never raw GLB bytes")));
        assert!(!read_only
            .iter()
            .any(|tool| tool["name"] == "candidate_topology_quality_prepare"));

        let enabled = tools_with_writes(true);
        let topology_prepare = enabled
            .iter()
            .find(|tool| tool["name"] == "candidate_topology_quality_prepare")
            .expect("candidate topology prepare tool");
        assert_eq!(topology_prepare["annotations"]["readOnlyHint"], false);
        assert_eq!(topology_prepare["annotations"]["writeIntent"], true);
        assert_eq!(topology_prepare["annotations"]["approvalRequired"], false);
        assert_eq!(
            topology_prepare["inputSchema"]["additionalProperties"],
            false
        );
        assert!(topology_prepare["inputSchema"]["required"]
            .as_array()
            .expect("topology required fields")
            .iter()
            .all(|field| field != "approved" && field != "approval_receipt_id"));

        let get_request = json!({
            "schema_version":"CandidateTopologyQualityGetRequest@1",
            "topology_quality_id":"topology-quality-1",
            "project_id":"project-1",
            "candidate_id":"candidate-1"
        });
        assert!(validate_declared_tool_input(
            "candidate_topology_quality_get",
            &get_request,
            false
        )
        .is_ok());
        let mut unknown = get_request;
        unknown["python"] = json!("bpy.ops");
        assert!(
            validate_declared_tool_input("candidate_topology_quality_get", &unknown, false)
                .is_err()
        );
    }

    #[test]
    fn candidate_material_surface_quality_tools_are_closed_dual_bound_and_opt_in() {
        let read_only = tools_with_writes(false);
        let material_get = read_only
            .iter()
            .find(|tool| tool["name"] == "candidate_material_surface_quality_get")
            .expect("candidate material-surface read tool");
        assert_eq!(material_get["annotations"]["readOnlyHint"], true);
        assert_eq!(material_get["annotations"]["approvalRequired"], false);
        assert!(material_get["description"]
            .as_str()
            .is_some_and(|description| description.contains("never raw GLB or PNG bytes")));
        assert!(!read_only
            .iter()
            .any(|tool| tool["name"] == "candidate_material_surface_quality_prepare"));

        let enabled = tools_with_writes(true);
        let material_prepare = enabled
            .iter()
            .find(|tool| tool["name"] == "candidate_material_surface_quality_prepare")
            .expect("candidate material-surface prepare tool");
        assert_eq!(material_prepare["annotations"]["readOnlyHint"], false);
        assert_eq!(material_prepare["annotations"]["writeIntent"], true);
        assert_eq!(material_prepare["annotations"]["approvalRequired"], false);
        assert_eq!(
            material_prepare["inputSchema"]["additionalProperties"],
            false
        );
        assert!(material_prepare["inputSchema"]["required"]
            .as_array()
            .expect("material-surface required fields")
            .iter()
            .all(|field| field != "approved" && field != "approval_receipt_id"));

        let get_request = json!({
            "schema_version":"CandidateMaterialSurfaceQualityGetRequest@1",
            "material_surface_quality_id":"material-surface-quality-1",
            "project_id":"project-1",
            "source_candidate_id":"candidate-1",
            "output_candidate_id":"candidate-appearance-1"
        });
        assert!(validate_declared_tool_input(
            "candidate_material_surface_quality_get",
            &get_request,
            false
        )
        .is_ok());
        let mut unknown = get_request;
        unknown["python"] = json!("bpy.ops");
        assert!(validate_declared_tool_input(
            "candidate_material_surface_quality_get",
            &unknown,
            false
        )
        .is_err());
    }

    #[test]
    fn candidate_animation_vfx_quality_tools_are_closed_head_bound_and_opt_in() {
        let read_only = tools_with_writes(false);
        let quality_get = read_only
            .iter()
            .find(|tool| tool["name"] == "candidate_animation_vfx_quality_get")
            .expect("candidate animation-vfx read tool");
        assert_eq!(quality_get["annotations"]["readOnlyHint"], true);
        assert_eq!(quality_get["annotations"]["approvalRequired"], false);
        assert!(quality_get["description"]
            .as_str()
            .is_some_and(|description| description.contains("never raw GLB or PNG bytes")));
        assert!(!read_only
            .iter()
            .any(|tool| tool["name"] == "candidate_animation_vfx_quality_prepare"));

        let enabled = tools_with_writes(true);
        let quality_prepare = enabled
            .iter()
            .find(|tool| tool["name"] == "candidate_animation_vfx_quality_prepare")
            .expect("candidate animation-vfx prepare tool");
        assert_eq!(quality_prepare["annotations"]["readOnlyHint"], false);
        assert_eq!(quality_prepare["annotations"]["writeIntent"], true);
        assert_eq!(quality_prepare["annotations"]["approvalRequired"], false);
        assert_eq!(
            quality_prepare["inputSchema"]["additionalProperties"],
            false
        );
        assert!(quality_prepare["inputSchema"]["required"]
            .as_array()
            .expect("animation-vfx required fields")
            .iter()
            .all(|field| field != "approved" && field != "approval_receipt_id"));

        let get_request = json!({
            "schema_version":"CandidateAnimationVfxQualityGetRequest@1",
            "animation_vfx_quality_id":"animation-vfx-quality-1",
            "project_id":"project-1",
            "candidate_id":"candidate-appearance-1"
        });
        assert!(validate_declared_tool_input(
            "candidate_animation_vfx_quality_get",
            &get_request,
            false
        )
        .is_ok());
        let mut unknown = get_request;
        unknown["python"] = json!("bpy.ops");
        assert!(validate_declared_tool_input(
            "candidate_animation_vfx_quality_get",
            &unknown,
            false
        )
        .is_err());
    }

    #[test]
    fn candidate_animation_vfx_quality_v2_tools_are_exposed_and_dispatch_is_wired() {
        let read_only = tools_with_writes(false);
        let get = read_only
            .iter()
            .find(|tool| tool["name"] == "candidate_animation_vfx_quality_v2_get")
            .expect("CandidateAnimationVfxQuality@2 get tool");
        assert_eq!(get["annotations"]["readOnlyHint"], true);
        assert_eq!(get["annotations"]["writeIntent"], false);
        assert_eq!(get["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            get["inputSchema"]["required"]
                .as_array()
                .expect("V2 get request fields")
                .len(),
            4
        );
        assert!(!read_only
            .iter()
            .any(|tool| tool["name"] == "candidate_animation_vfx_quality_v2_prepare"));

        let enabled = tools_with_writes(true);
        let prepare = enabled
            .iter()
            .find(|tool| tool["name"] == "candidate_animation_vfx_quality_v2_prepare")
            .expect("CandidateAnimationVfxQuality@2 prepare tool");
        assert_eq!(prepare["annotations"]["readOnlyHint"], false);
        assert_eq!(prepare["annotations"]["writeIntent"], true);
        assert_eq!(prepare["annotations"]["approvalRequired"], false);
        assert_eq!(
            prepare["inputSchema"]["required"]
                .as_array()
                .expect("V2 prepare request fields")
                .len(),
            69
        );
        let get_request = json!({
            "schema_version":"CandidateAnimationVfxQualityGetRequest@2",
            "animation_vfx_quality_id":"quality-1",
            "project_id":"project-1",
            "candidate_id":"appearance-1"
        });
        assert!(validate_declared_tool_input(
            "candidate_animation_vfx_quality_v2_get",
            &get_request,
            false
        )
        .is_ok());
        let mut unknown = get_request;
        unknown["legacy_sidecar_bool"] = json!(true);
        assert!(validate_declared_tool_input(
            "candidate_animation_vfx_quality_v2_get",
            &unknown,
            false
        )
        .is_err());

        let runtime = Runtime::ephemeral().expect("V2 dispatch runtime");
        for name in [
            "candidate_animation_vfx_quality_v2_prepare",
            "candidate_animation_vfx_quality_v2_get",
        ] {
            let error = dispatch_in_process(&runtime, name, &json!({}))
                .expect_err("malformed V2 request must reach its Runtime parser");
            assert!(!error.contains("unknown tool"), "{name}: {error}");
        }
    }

    #[test]
    fn mcp004_write_tools_are_explicit_and_confirmation_bound() {
        let disabled = tools_with_writes(false);
        assert_eq!(disabled.len(), 90);
        assert!(!disabled
            .iter()
            .any(|tool| { tool["name"].as_str().is_some_and(is_mcp004_write_tool) }));

        let enabled = tools_with_writes(true);
        assert_eq!(enabled.len(), 159);
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

    #[test]
    fn capabilities_write_surface_fails_closed_on_cohort_mismatch() {
        let local = "a".repeat(64);
        let other = "b".repeat(64);
        assert!(!effective_write_tools_enabled(
            true,
            Some(&other),
            Some(&local)
        ));
        assert!(!effective_write_tools_enabled(true, None, Some(&local)));
        assert!(effective_write_tools_enabled(
            true,
            Some(&local),
            Some(&local)
        ));
        assert!(effective_write_tools_enabled(true, None, None));
        assert!(!effective_write_tools_enabled(
            false,
            Some(&local),
            Some(&local)
        ));
    }

    #[test]
    fn tools_list_preserves_explicit_opt_in_for_ordinary_source_builds() {
        let mut backend = Backend::InProcess(Runtime::ephemeral().expect("runtime"));
        let mut session = Session::new();
        session.state = SessionState::Ready;
        session.write_tools_enabled = true;
        let enabled = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
        )
        .expect("tools/list response");
        assert_eq!(
            enabled["result"]["tools"].as_array().map(Vec::len),
            Some(159)
        );

        session.write_tools_enabled = false;
        let disabled = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
        )
        .expect("read-only tools/list response");
        assert_eq!(
            disabled["result"]["tools"].as_array().map(Vec::len),
            Some(90)
        );
    }

    #[test]
    fn game_asset_delivery_mcp_dispatch_is_closed_bounded_and_prepare_only() {
        if build_cohort_sha256().is_none() {
            return;
        }
        fn program(project_id: &str, catalog_sha256: &str, segments: u64) -> Value {
            let mut value = json!({
                "schema_version":"GeometryProgram@2",
                "project_id":project_id,
                "representation_plan_sha256":"6".repeat(64),
                "operator_catalog_sha256":catalog_sha256,
                "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
                "budgets":{"max_nodes":1,"max_triangles":1000,"max_glb_bytes":1048576,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
                "nodes":[{
                    "node_id":"delivery-node",
                    "operator_id":"forgecad.geometry.primitive@2",
                    "inputs":[],
                    "parameters":{"shape":"cylinder","radius_m":0.25,"height_m":1.0,"radial_segments":segments,"position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}
                }],
                "part_outputs":[{"part_id":"delivery-part","input_node_ids":["delivery-node"],"material_zone_id":"zone-delivery","solid":true}]
            });
            value["canonical_sha256"] = Value::String(canonical_json_hash(&value));
            value
        }

        let (mut backend, mut session) = initialized();
        let (project_id, prepared) = match &backend {
            Backend::InProcess(runtime) => {
                let project = runtime
                    .create_project("MCP game delivery", json!({"profile":"mvp"}))
                    .expect("project");
                let catalog_sha256 = runtime.active_operator_catalog()["canonical_sha256"]
                    .as_str()
                    .expect("catalog hash")
                    .to_owned();
                let prepared = [64, 32, 16]
                    .into_iter()
                    .map(|segments| {
                        runtime
                            .prepare_geometry_candidate(
                                &project.project_id,
                                None,
                                json!({
                                    "typed":"geometry",
                                    "geometry_program":program(&project.project_id, &catalog_sha256, segments)
                                }),
                            )
                            .expect("LOD candidate")
                    })
                    .collect::<Vec<_>>();
                (project.project_id, prepared)
            }
            _ => unreachable!("test backend"),
        };
        let lods = prepared
            .iter()
            .enumerate()
            .map(|(level, value)| {
                json!({
                    "level":level,
                    "candidate_id":value["candidate"]["candidate_id"],
                    "candidate_state_sha256":value["candidate"]["canonical_sha256"],
                    "artifact_sha256":value["artifact"]["artifact_id"],
                    "artifact_readback_sha256":value["artifact"]["canonical_sha256"]
                })
            })
            .collect::<Vec<_>>();
        let mut arguments = json!({
            "schema_version":"GameAssetDeliveryPrepareRequest@1",
            "project_id":project_id,
            "lods":lods,
            "animation":null,
            "lod_policy":"authored-three-level-part-stable-progressive-triangles@1",
            "collision_policy":"per-part-aabb-box-from-lod2-visual-geometry@1",
            "readiness_policy":"engine-neutral-gltf2-embedded-assets-stable-names@1",
            "canonical_sha256":""
        });
        let mut preimage = arguments.clone();
        preimage.as_object_mut().unwrap().remove("canonical_sha256");
        arguments["canonical_sha256"] = Value::String(canonical_json_hash(&preimage));

        let source = &prepared[0];
        let mut derive_arguments = json!({
            "schema_version":"GameAssetLodDeriveRequest@1",
            "project_id":project_id,
            "source_candidate_id":source["candidate"]["candidate_id"],
            "source_candidate_state_sha256":source["candidate"]["canonical_sha256"],
            "source_artifact_sha256":source["artifact"]["artifact_id"],
            "source_artifact_readback_sha256":source["artifact"]["canonical_sha256"],
            "source_geometry_program_sha256":source["artifact"]["program_sha256"],
            "source_operator_catalog_sha256":source["artifact"]["operator_catalog_sha256"],
            "source_readback_config_sha256":source["artifact"]["readback_config_sha256"],
            "derive_policy":"runtime-owned-typed-segment-lowering-lod1-half-lod2-quarter@1",
            "canonical_sha256":""
        });
        let mut derive_preimage = derive_arguments.clone();
        derive_preimage
            .as_object_mut()
            .unwrap()
            .remove("canonical_sha256");
        derive_arguments["canonical_sha256"] = Value::String(canonical_json_hash(&derive_preimage));
        let (derive_candidates_before, derive_versions_before) = match &backend {
            Backend::InProcess(runtime) => (
                runtime.candidates(&project_id).expect("derive candidates"),
                runtime
                    .versions(Some(&project_id))
                    .expect("derive versions"),
            ),
            _ => unreachable!("test backend"),
        };
        let derive_response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":1898,"method":"tools/call","params":{"name":"game_asset_lod_derive","arguments":derive_arguments.clone()}}),
        )
        .expect("default-read automatic LOD derivation response");
        assert_eq!(
            derive_response["result"]["structuredContent"]["schema_version"],
            "GameAssetLodDeriveResult@1"
        );
        assert_eq!(
            derive_response["result"]["structuredContent"]["levels"]
                .as_array()
                .unwrap()
                .iter()
                .map(|level| level["triangle_count"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            vec![256, 128, 64]
        );
        assert_eq!(
            derive_response["result"]["structuredContent"]["runtime_write_performed"],
            false
        );
        assert_eq!(
            derive_response["result"]["structuredContent"]["worker_replay_verified"],
            true
        );
        assert!(
            serde_json::to_vec(&derive_response).unwrap().len() <= READ_MODEL_MCP_WIRE_MAX_BYTES
        );
        let derive_summary: Value = serde_json::from_str(
            derive_response["result"]["content"][0]["text"]
                .as_str()
                .expect("automatic LOD summary"),
        )
        .expect("automatic LOD summary JSON");
        assert_eq!(
            derive_summary["schema_version"],
            "GameAssetDeliveryMcpSummary@1"
        );
        assert_eq!(
            derive_summary["derive_levels"]
                .as_array()
                .unwrap()
                .iter()
                .map(|level| level["triangle_count"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            vec![256, 128, 64]
        );
        match &backend {
            Backend::InProcess(runtime) => {
                assert_eq!(
                    serde_json::to_value(runtime.candidates(&project_id).unwrap()).unwrap(),
                    serde_json::to_value(derive_candidates_before).unwrap()
                );
                assert_eq!(
                    serde_json::to_value(runtime.versions(Some(&project_id)).unwrap()).unwrap(),
                    serde_json::to_value(derive_versions_before).unwrap()
                );
            }
            _ => unreachable!("test backend"),
        }
        let mut forbidden_derive = derive_arguments;
        forbidden_derive["python"] = json!("bpy.ops.object.modifier_add(type='DECIMATE')");
        let rejected_derive = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":1899,"method":"tools/call","params":{"name":"game_asset_lod_derive","arguments":forbidden_derive}}),
        )
        .expect("closed automatic LOD schema rejection");
        assert_eq!(
            rejected_derive["error"]["data"]["code"],
            "INVALID_TOOL_PARAMS"
        );

        let disabled = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":1900,"method":"tools/call","params":{"name":"game_asset_delivery_prepare","arguments":arguments.clone()}}),
        )
        .expect("disabled delivery response");
        assert_eq!(disabled["result"]["isError"], true);
        assert_eq!(
            disabled["result"]["structuredContent"]["code"],
            "MCP010F_GAME_ASSET_DELIVERY_WRITE_TOOLS_DISABLED",
            "{disabled}"
        );

        session.write_tools_enabled = true;
        let response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":1901,"method":"tools/call","params":{"name":"game_asset_delivery_prepare","arguments":arguments}}),
        )
        .expect("game delivery response");
        assert_eq!(
            response["result"]["structuredContent"]["schema_version"],
            "GameAssetDeliveryPrepareResult@1",
            "{response}"
        );
        assert_eq!(
            response["result"]["structuredContent"]["candidate_confirmed"],
            false
        );
        assert_eq!(
            response["result"]["structuredContent"]["export_performed"],
            false
        );
        assert_eq!(
            response["result"]["structuredContent"]["actual_engine_roundtrip"],
            false
        );
        assert!(serde_json::to_vec(&response).unwrap().len() <= READ_MODEL_MCP_WIRE_MAX_BYTES);
        let summary: Value = serde_json::from_str(
            response["result"]["content"][0]["text"]
                .as_str()
                .expect("delivery summary"),
        )
        .expect("delivery summary JSON");
        assert_eq!(summary["schema_version"], "GameAssetDeliveryMcpSummary@1");
        assert_eq!(summary["triangle_counts"], json!([256, 128, 64]));
        assert_eq!(summary["collision_proxy_count"], 1);
        assert_eq!(summary["threejs_status"], "NOT_RUN");

        let manifest_sha256 =
            response["result"]["structuredContent"]["delivery_manifest_object_sha256"].clone();
        let get_response = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":1902,"method":"tools/call","params":{"name":"game_asset_delivery_get","arguments":{
                "schema_version":"GameAssetDeliveryGetRequest@1",
                "project_id":project_id,
                "delivery_manifest_object_sha256":manifest_sha256
            }}}),
        )
        .expect("durable game delivery get response");
        assert_eq!(
            get_response["result"]["structuredContent"]["schema_version"],
            "GameAssetDeliveryGetResult@1"
        );
        assert_eq!(
            get_response["result"]["structuredContent"]["restart_hash_verified"],
            true
        );
        assert!(serde_json::to_vec(&get_response).unwrap().len() <= READ_MODEL_MCP_WIRE_MAX_BYTES);

        let mut forbidden = json!({
            "schema_version":"GameAssetDeliveryPrepareRequest@1",
            "project_id":project_id,
            "lods":[],
            "animation":null,
            "lod_policy":"authored-three-level-part-stable-progressive-triangles@1",
            "collision_policy":"per-part-aabb-box-from-lod2-visual-geometry@1",
            "readiness_policy":"engine-neutral-gltf2-embedded-assets-stable-names@1",
            "canonical_sha256":"a".repeat(64),
            "python":"bpy.ops"
        });
        forbidden["canonical_sha256"] = json!("a".repeat(64));
        let rejected = handle(
            &mut backend,
            &mut session,
            &json!({"jsonrpc":"2.0","id":1903,"method":"tools/call","params":{"name":"game_asset_delivery_prepare","arguments":forbidden}}),
        )
        .expect("closed schema rejection");
        assert_eq!(rejected["error"]["data"]["code"], "INVALID_TOOL_PARAMS");
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
        assert_eq!(listed["result"]["tools"].as_array().unwrap().len(), 159);

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
        let geometry_wire = serde_json::to_string(&geometry).expect("geometry response serializes");
        for forbidden in [
            "\"glb_base64\"",
            "\"glb_bytes\"",
            "\"raw_glb\"",
            "\"artifact_bytes\"",
        ] {
            assert!(
                !geometry_wire.contains(forbidden),
                "geometry_prepare MCP response must not expose raw GLB field {forbidden}"
            );
        }
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
                {"zone_id":"zone-black-mechanical","part_ids":["core"],"base_color":[0.16,0.06,0.01,1.0],"metallic":0.2,"roughness":0.25,"emissive":[1.0,0.12,0.01]}
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
            "AppearancePrepareResult@1",
            "appearance_prepare response: {appearance:#}"
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
